import Foundation
import OSLog

private let companionPaneBytesLogger = Logger(subsystem: "be.zenjoy.zentty", category: "CompanionPaneBytes")

// MARK: - Watcher token

/// Opaque handle for one connection's pane.bytes subscriptions. Distinct from
/// `CompanionPaneWatchToken` (text lane) so the two paths do not share teardown.
struct CompanionPaneBytesToken: Hashable, Sendable {
    fileprivate let id: UUID
}

// MARK: - Ring buffer

/// Fixed-capacity byte ring for warm-resume of `pane.bytes.*`. Stores the most
/// recent `capacity` bytes of a surface epoch and answers exclusive-end resume
/// queries (`fromSeq` = first missing byte on the phone).
///
/// Offsets are absolute within the epoch: the first byte ever written is at
/// offset 0, and `nextSeq` is always the exclusive end of written history
/// (whether or not older bytes have rolled off the ring).
struct CompanionPaneBytesRing: Sendable {
    /// Max retained bytes per epoch (1 MiB — covers a long scrollback of shell
    /// output while staying cheap in RAM for a handful of watched panes).
    static let defaultCapacity = 1 * 1024 * 1024

    private let capacity: Int
    private var storage: [UInt8]
    /// Absolute epoch offset of `storage[0]` (increases when the ring rolls).
    private var baseSeq: Int = 0
    /// Absolute exclusive end of written bytes (next write lands at this offset).
    private(set) var nextSeq: Int = 0

    init(capacity: Int = CompanionPaneBytesRing.defaultCapacity) {
        precondition(capacity > 0)
        self.capacity = capacity
        self.storage = []
        self.storage.reserveCapacity(min(capacity, 64 * 1024))
    }

    var isEmpty: Bool { nextSeq == baseSeq }

    mutating func append(_ bytes: Data) {
        guard !bytes.isEmpty else { return }
        if bytes.count >= capacity {
            // Whole write is larger than the ring: keep only the tail.
            storage = Array(bytes.suffix(capacity))
            nextSeq += bytes.count
            baseSeq = nextSeq - capacity
            return
        }
        storage.append(contentsOf: bytes)
        nextSeq += bytes.count
        if storage.count > capacity {
            let overflow = storage.count - capacity
            storage.removeFirst(overflow)
            baseSeq += overflow
        }
    }

    /// Bytes covering absolute range `[fromSeq, nextSeq)` when still in the ring.
    /// Returns `nil` when `fromSeq` has rolled off (`fromSeq < baseSeq`) or is
    /// ahead of the write head (`fromSeq > nextSeq`).
    func slice(fromSeq: Int) -> Data? {
        if fromSeq == nextSeq {
            return Data()
        }
        guard fromSeq >= baseSeq, fromSeq <= nextSeq else {
            return nil
        }
        let start = fromSeq - baseSeq
        let end = nextSeq - baseSeq
        return Data(storage[start..<end])
    }

    /// Full retained tail (cold attach replay).
    func tail() -> (startSeq: Int, data: Data) {
        (baseSeq, Data(storage))
    }
}

// MARK: - Feed

/// Streams raw PTY bytes to phone watchers via `pane.bytes.chunk` and answers
/// `pane.bytes.attach` with ring replay. The producer side is fed through
/// ``ingest(paneId:epoch:bytes:)`` — typically a libghostty PTY tee callback
/// once available; tests inject synthetic bytes directly.
///
/// Multi-watcher: many connections may attach the same pane. Each live chunk
/// fans out to every attached token for that pane. Attach is request/reply per
/// connection (like scrollback), not a shared snapshot.
///
/// `lastSeq` on warm attach is the phone's exclusive resume cursor (first missing
/// byte). Non-truncated same-epoch replies use `startSeq == lastSeq`.
@MainActor
final class CompanionPaneBytesFeed {
    /// Decoded chunk size cap — mirrors `PANE_BYTES_MAX_CHUNK_BYTES` on the wire.
    static let maxChunkBytes = 32 * 1024

    private struct Watcher {
        /// Panes this connection is currently attached to.
        var paneIds: Set<String> = []
        let sendChunk: (CompanionPaneBytesChunk) -> Void
    }

    private struct PaneState {
        var epoch: String
        var ring: CompanionPaneBytesRing
    }

    private var watchers: [CompanionPaneBytesToken: Watcher] = [:]
    private var panes: [String: PaneState] = [:]
    private let ringCapacity: Int

    init(ringCapacity: Int = CompanionPaneBytesRing.defaultCapacity) {
        self.ringCapacity = ringCapacity
    }

    // MARK: Watcher lifecycle

    func addWatcher(sendChunk: @escaping (CompanionPaneBytesChunk) -> Void) -> CompanionPaneBytesToken {
        let token = CompanionPaneBytesToken(id: UUID())
        watchers[token] = Watcher(sendChunk: sendChunk)
        return token
    }

    func removeWatcher(_ token: CompanionPaneBytesToken) {
        watchers.removeValue(forKey: token)
    }

    // MARK: Attach / detach

    /// Handles `pane.bytes.attach`. Registers the token for live chunks on success.
    func attach(
        token: CompanionPaneBytesToken,
        paneId: String,
        lastSeq: Int?,
        epoch: String?
    ) -> CompanionPaneBytesAttached {
        // Reject partial warm shapes (both required or both absent).
        let warm = lastSeq != nil || epoch != nil
        if warm && (lastSeq == nil || epoch == nil) {
            // Treat as cold: no reliable resume point.
            return coldAttach(token: token, paneId: paneId)
        }

        if let lastSeq, let epoch {
            return warmAttach(token: token, paneId: paneId, lastSeq: lastSeq, epoch: epoch)
        }
        return coldAttach(token: token, paneId: paneId)
    }

    func detach(token: CompanionPaneBytesToken, paneId: String) {
        guard var watcher = watchers[token] else { return }
        watcher.paneIds.remove(paneId)
        watchers[token] = watcher
    }

    // MARK: Ingest (producer)

    /// Append raw PTY output for a pane/epoch and fan out chunks to attached
    /// watchers. Creates the pane state if this is the first byte for the epoch.
    /// When `epoch` differs from the stored one, the ring is reset (surface restart).
    func ingest(paneId: String, epoch: String, bytes: Data) {
        guard !bytes.isEmpty else { return }

        var state = panes[paneId] ?? PaneState(epoch: epoch, ring: CompanionPaneBytesRing(capacity: ringCapacity))
        if state.epoch != epoch {
            state = PaneState(epoch: epoch, ring: CompanionPaneBytesRing(capacity: ringCapacity))
        }

        var offset = 0
        while offset < bytes.count {
            let end = min(offset + Self.maxChunkBytes, bytes.count)
            let slice = bytes.subdata(in: offset..<end)
            let seq = state.ring.nextSeq
            state.ring.append(slice)
            fanOut(paneId: paneId, epoch: state.epoch, seq: seq, data: slice)
            offset = end
        }
        panes[paneId] = state
    }

    /// Surface closed or recreated: drop ring so a later attach starts cold.
    func handlePaneClosed(paneId: String) {
        panes.removeValue(forKey: paneId)
        for (token, var watcher) in watchers {
            watcher.paneIds.remove(paneId)
            watchers[token] = watcher
        }
    }

    /// Mint or look up the current epoch for a pane (used by producers that need
    /// a stable id before the first byte arrives).
    func ensureEpoch(paneId: String, epoch: String) {
        if panes[paneId] == nil {
            panes[paneId] = PaneState(epoch: epoch, ring: CompanionPaneBytesRing(capacity: ringCapacity))
        } else if panes[paneId]?.epoch != epoch {
            panes[paneId] = PaneState(epoch: epoch, ring: CompanionPaneBytesRing(capacity: ringCapacity))
        }
    }

    // MARK: Private

    private func coldAttach(token: CompanionPaneBytesToken, paneId: String) -> CompanionPaneBytesAttached {
        markAttached(token: token, paneId: paneId)
        guard let state = panes[paneId] else {
            // No surface output yet — empty stream, mint a placeholder epoch so
            // the phone has a baseline; live chunks will arrive once ingest runs.
            let epoch = UUID().uuidString
            panes[paneId] = PaneState(epoch: epoch, ring: CompanionPaneBytesRing(capacity: ringCapacity))
            return CompanionPaneBytesAttached(
                paneId: paneId,
                epoch: epoch,
                startSeq: 0,
                replay: "",
                truncated: false
            )
        }
        let (startSeq, data) = state.ring.tail()
        return CompanionPaneBytesAttached(
            paneId: paneId,
            epoch: state.epoch,
            startSeq: startSeq,
            replay: data.base64EncodedString(),
            truncated: false
        )
    }

    private func warmAttach(
        token: CompanionPaneBytesToken,
        paneId: String,
        lastSeq: Int,
        epoch: String
    ) -> CompanionPaneBytesAttached {
        markAttached(token: token, paneId: paneId)
        guard let state = panes[paneId], state.epoch == epoch else {
            // Epoch mismatch or unknown pane → truncated fresh tail (or empty).
            return truncatedReply(paneId: paneId)
        }
        if let data = state.ring.slice(fromSeq: lastSeq) {
            return CompanionPaneBytesAttached(
                paneId: paneId,
                epoch: state.epoch,
                startSeq: lastSeq,
                replay: data.base64EncodedString(),
                truncated: false
            )
        }
        // Ring rolled past lastSeq (or lastSeq ahead of head).
        let (startSeq, data) = state.ring.tail()
        return CompanionPaneBytesAttached(
            paneId: paneId,
            epoch: state.epoch,
            startSeq: startSeq,
            replay: data.base64EncodedString(),
            truncated: true
        )
    }

    private func truncatedReply(paneId: String) -> CompanionPaneBytesAttached {
        if let state = panes[paneId] {
            let (startSeq, data) = state.ring.tail()
            return CompanionPaneBytesAttached(
                paneId: paneId,
                epoch: state.epoch,
                startSeq: startSeq,
                replay: data.base64EncodedString(),
                truncated: true
            )
        }
        let epoch = UUID().uuidString
        panes[paneId] = PaneState(epoch: epoch, ring: CompanionPaneBytesRing(capacity: ringCapacity))
        return CompanionPaneBytesAttached(
            paneId: paneId,
            epoch: epoch,
            startSeq: 0,
            replay: "",
            truncated: true
        )
    }

    private func markAttached(token: CompanionPaneBytesToken, paneId: String) {
        guard var watcher = watchers[token] else {
            companionPaneBytesLogger.error("attach for unknown token")
            return
        }
        watcher.paneIds.insert(paneId)
        watchers[token] = watcher
    }

    private func fanOut(paneId: String, epoch: String, seq: Int, data: Data) {
        let chunk = CompanionPaneBytesChunk(
            paneId: paneId,
            epoch: epoch,
            seq: seq,
            data: data.base64EncodedString()
        )
        for watcher in watchers.values where watcher.paneIds.contains(paneId) {
            watcher.sendChunk(chunk)
        }
    }
}
