import XCTest

@testable import Zentty

/// Unit tests for the raw-PTY byte lane producer: ring resume semantics and
/// attach/detach fan-out. Bytes are injected synthetically — the libghostty
/// PTY tee is out of band of these tests.
@MainActor
final class CompanionPaneBytesFeedTests: XCTestCase {
    private func b64Decode(_ s: String) -> Data {
        Data(base64Encoded: s) ?? Data()
    }

    func testColdAttachReplaysRingTail() {
        let feed = CompanionPaneBytesFeed()
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }

        let payload = Data("hello-world".utf8)
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: payload)

        let attached = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        XCTAssertEqual(attached.paneId, "p1")
        XCTAssertEqual(attached.epoch, "e1")
        XCTAssertEqual(attached.startSeq, 0)
        XCTAssertEqual(attached.truncated, false)
        XCTAssertEqual(b64Decode(attached.replay), payload)
        // Cold attach does not re-fan prior bytes as live chunks.
        XCTAssertTrue(chunks.isEmpty)
    }

    func testWarmAttachExclusiveResume() {
        let feed = CompanionPaneBytesFeed()
        let token = feed.addWatcher { _ in }
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("abcdefgh".utf8)) // 8 bytes

        // Phone held [0,4) → lastSeq=4 exclusive.
        let attached = feed.attach(token: token, paneId: "p1", lastSeq: 4, epoch: "e1")
        XCTAssertEqual(attached.startSeq, 4)
        XCTAssertEqual(attached.truncated, false)
        XCTAssertEqual(String(data: b64Decode(attached.replay), encoding: .utf8), "efgh")
    }

    func testWarmAttachTruncatedWhenRingRolled() {
        let feed = CompanionPaneBytesFeed(ringCapacity: 8)
        let token = feed.addWatcher { _ in }
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("0123456789ABCDEF".utf8)) // 16 bytes

        // lastSeq=0 has rolled off a capacity-8 ring.
        let attached = feed.attach(token: token, paneId: "p1", lastSeq: 0, epoch: "e1")
        XCTAssertTrue(attached.truncated)
        XCTAssertEqual(attached.startSeq, 8) // only last 8 retained
        XCTAssertEqual(b64Decode(attached.replay).count, 8)
    }

    func testWarmAttachEpochMismatchIsTruncated() {
        let feed = CompanionPaneBytesFeed()
        let token = feed.addWatcher { _ in }
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("abc".utf8))

        let attached = feed.attach(token: token, paneId: "p1", lastSeq: 0, epoch: "old")
        XCTAssertTrue(attached.truncated)
        XCTAssertEqual(attached.epoch, "e1")
    }

    func testLiveChunksFanOutOnlyToAttachedWatchers() {
        let feed = CompanionPaneBytesFeed()
        var chunksA: [CompanionPaneBytesChunk] = []
        var chunksB: [CompanionPaneBytesChunk] = []
        let tokenA = feed.addWatcher { chunksA.append($0) }
        let tokenB = feed.addWatcher { chunksB.append($0) }

        feed.ensureEpoch(paneId: "p1", epoch: "e1")
        _ = feed.attach(token: tokenA, paneId: "p1", lastSeq: nil, epoch: nil)
        // B never attaches.

        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("xy".utf8))
        feed.ingest(paneId: "p1", epoch: "e1", seq: 2, bytes: Data("z".utf8))

        XCTAssertEqual(chunksA.map(\.seq), [0, 2])
        XCTAssertTrue(chunksB.isEmpty)
        _ = tokenB
    }

    func testDetachStopsFanOut() {
        let feed = CompanionPaneBytesFeed()
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }
        feed.ensureEpoch(paneId: "p1", epoch: "e1")
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("a".utf8))
        let countAfterAttach = chunks.count
        feed.detach(token: token, paneId: "p1")
        feed.ingest(paneId: "p1", epoch: "e1", seq: 1, bytes: Data("b".utf8))
        XCTAssertEqual(chunks.count, countAfterAttach)
    }

    func testChunkSplitRespectsMaxSize() {
        let feed = CompanionPaneBytesFeed()
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }
        feed.ensureEpoch(paneId: "p1", epoch: "e1")
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)

        let big = Data(repeating: 0x41, count: CompanionPaneBytesFeed.maxChunkBytes + 10)
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: big)
        XCTAssertEqual(chunks.count, 2)
        XCTAssertEqual(b64Decode(chunks[0].data).count, CompanionPaneBytesFeed.maxChunkBytes)
        XCTAssertEqual(b64Decode(chunks[1].data).count, 10)
        XCTAssertEqual(chunks[0].seq, 0)
        XCTAssertEqual(chunks[1].seq, CompanionPaneBytesFeed.maxChunkBytes)
    }

    func testPaneClosedDropsRing() {
        let feed = CompanionPaneBytesFeed()
        let token = feed.addWatcher { _ in }
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("hello".utf8))
        feed.handlePaneClosed(paneId: "p1")
        let attached = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        // Fresh empty stream after close.
        XCTAssertEqual(attached.replay, "")
        XCTAssertEqual(attached.startSeq, 0)
    }

    /// Regression: a cold attach to a pane with a full 1 MiB ring used to base64
    /// the whole tail into `replay`, producing a frame far past the relay's
    /// 256 KiB cap — which the relay enforces as the ws `maxPayload` and answers
    /// by closing the connection (1009).
    func testColdAttachClampsFullRingAndMarksTruncated() {
        let feed = CompanionPaneBytesFeed()
        let token = feed.addWatcher { _ in }

        let capacity = CompanionPaneBytesRing.defaultCapacity
        var filler = Data(repeating: 0x2E, count: capacity)
        // Tag the final byte so we can prove the RECENT end is what survives.
        filler[capacity - 1] = 0x5A
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: filler)

        let attached = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        let replay = b64Decode(attached.replay)
        XCTAssertTrue(attached.truncated)
        XCTAssertEqual(replay.count, CompanionPaneBytesFeed.maxReplayBytes)
        XCTAssertEqual(replay.last, 0x5A)
        XCTAssertEqual(attached.startSeq, capacity - CompanionPaneBytesFeed.maxReplayBytes)
        // startSeq + decoded(replay) must still be the ring head so the phone's
        // next expected offset lines up with live chunks.
        XCTAssertEqual(attached.startSeq + replay.count, capacity)
    }

    func testWarmAttachClampsOversizeResumeGap() {
        let feed = CompanionPaneBytesFeed()
        let token = feed.addWatcher { _ in }

        let total = CompanionPaneBytesFeed.maxReplayBytes * 2
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data(repeating: 0x41, count: total))

        // Phone fell behind by more than one frame's worth of bytes.
        let attached = feed.attach(token: token, paneId: "p1", lastSeq: 0, epoch: "e1")
        let replay = b64Decode(attached.replay)
        XCTAssertTrue(attached.truncated)
        XCTAssertEqual(replay.count, CompanionPaneBytesFeed.maxReplayBytes)
        XCTAssertEqual(attached.startSeq, total - CompanionPaneBytesFeed.maxReplayBytes)
    }

    func testWarmAttachWithinCapStaysContiguous() {
        let feed = CompanionPaneBytesFeed()
        let token = feed.addWatcher { _ in }
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data(repeating: 0x41, count: 4096))

        let attached = feed.attach(token: token, paneId: "p1", lastSeq: 1024, epoch: "e1")
        XCTAssertFalse(attached.truncated)
        XCTAssertEqual(attached.startSeq, 1024)
        XCTAssertEqual(b64Decode(attached.replay).count, 3072)
    }

    // MARK: - Seq-authoritative ingest

    func testChunkSeqComesFromProducerNotTheRing() {
        let feed = CompanionPaneBytesFeed()
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)

        // A live surface has been running: its first tee'd byte is at offset 4096.
        feed.ingest(paneId: "p1", epoch: "e1", seq: 4096, bytes: Data("ab".utf8))
        feed.ingest(paneId: "p1", epoch: "e1", seq: 4098, bytes: Data("cd".utf8))

        XCTAssertEqual(chunks.map(\.seq), [4096, 4098])
        XCTAssertEqual(chunks.map(\.epoch), ["e1", "e1"])
    }

    /// A producer-side drop must reach the phone as a forward `seq` jump — the
    /// signal its lane turns into a warm re-attach — not as contiguous bytes.
    func testGapInProducerSeqIsVisibleAndResyncsTruncated() {
        let feed = CompanionPaneBytesFeed()
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)

        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("abcd".utf8))
        // 100 bytes were shed under pressure: the next run starts at 104, not 4.
        feed.ingest(paneId: "p1", epoch: "e1", seq: 104, bytes: Data("efgh".utf8))

        XCTAssertEqual(chunks.map(\.seq), [0, 104])

        // The phone (expecting 4) warm re-attaches; the ring can no longer cover
        // that cursor, so it is told to reset instead of splicing a hole.
        let attached = feed.attach(token: token, paneId: "p1", lastSeq: 4, epoch: "e1")
        XCTAssertTrue(attached.truncated)
        XCTAssertEqual(attached.startSeq, 104)
        XCTAssertEqual(String(data: b64Decode(attached.replay), encoding: .utf8), "efgh")
    }

    func testOverlappingRedeliveryDropsAlreadyHeldPrefix() {
        let feed = CompanionPaneBytesFeed()
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)

        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("abcd".utf8))
        feed.ingest(paneId: "p1", epoch: "e1", seq: 2, bytes: Data("cdef".utf8))
        // Fully-duplicate run is dropped outright.
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("ab".utf8))

        XCTAssertEqual(chunks.map(\.seq), [0, 4])
        XCTAssertEqual(String(data: b64Decode(chunks[1].data), encoding: .utf8), "ef")

        let attached = feed.attach(token: token, paneId: "p1", lastSeq: 0, epoch: "e1")
        XCTAssertEqual(String(data: b64Decode(attached.replay), encoding: .utf8), "abcdef")
    }

    /// A surface restart mints a new epoch; the ring must not carry the old
    /// stream's bytes or offsets into it.
    func testEpochChangeResetsRingAndOffsets() {
        let feed = CompanionPaneBytesFeed()
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)

        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("old-output".utf8))
        feed.ingest(paneId: "p1", epoch: "e2", seq: 0, bytes: Data("new".utf8))

        XCTAssertEqual(chunks.map(\.epoch), ["e1", "e2"])
        let attached = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        XCTAssertEqual(attached.epoch, "e2")
        XCTAssertEqual(attached.startSeq, 0)
        XCTAssertEqual(String(data: b64Decode(attached.replay), encoding: .utf8), "new")
    }

    // MARK: - Tee lifecycle

    func testStreamInstallsOnFirstAttachAndRemovesOnLastDetach() {
        let provider = PaneBytesProviderSpy()
        let feed = CompanionPaneBytesFeed(provider: provider)
        let tokenA = feed.addWatcher { _ in }
        let tokenB = feed.addWatcher { _ in }

        _ = feed.attach(token: tokenA, paneId: "p1", lastSeq: nil, epoch: nil)
        _ = feed.attach(token: tokenB, paneId: "p1", lastSeq: nil, epoch: nil)
        XCTAssertEqual(provider.installs, ["p1"])
        XCTAssertEqual(provider.removals, [])

        feed.detach(token: tokenA, paneId: "p1")
        XCTAssertEqual(provider.removals, [])

        feed.detach(token: tokenB, paneId: "p1")
        XCTAssertEqual(provider.removals, ["p1"])

        // Re-attaching installs again (balanced edges).
        _ = feed.attach(token: tokenA, paneId: "p1", lastSeq: nil, epoch: nil)
        XCTAssertEqual(provider.installs, ["p1", "p1"])
    }

    /// A pane that cannot be resolved yet (its window still restoring) must stay
    /// retryable. Recording it as streaming would make every later attach a
    /// duplicate no-op, leaving the phone with a valid attach reply and then
    /// silence forever — its resync is only ever driven by an arriving chunk.
    func testFailedInstallIsRetriedOnNextAttach() {
        let provider = PaneBytesProviderSpy()
        provider.unresolvablePanes = ["p1"]
        let feed = CompanionPaneBytesFeed(provider: provider)
        let token = feed.addWatcher { _ in }

        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        XCTAssertEqual(provider.installs, [])

        // The pane resolves on a later attempt; the feed must try again.
        provider.unresolvablePanes = []
        feed.detach(token: token, paneId: "p1")
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        XCTAssertEqual(provider.installs, ["p1"])
    }

    /// A user-initiated pane close (Cmd-W) never routes through
    /// `handlePaneClosed` — only shell exit does. Without dropping the ring when
    /// the provider reports the pane gone, a closed-but-watched pane strands its
    /// 1 MiB buffer for the life of the process and still answers a later warm
    /// attach with stale bytes.
    func testRingIsDroppedWhenPaneNoLongerResolves() {
        let provider = PaneBytesProviderSpy()
        let feed = CompanionPaneBytesFeed(provider: provider)
        let token = feed.addWatcher { _ in }

        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        feed.ingest(paneId: "p1", epoch: "e1", seq: 0, bytes: Data("hello".utf8))

        // Pane closed underneath us: the detach can no longer resolve it.
        provider.unresolvablePanes = ["p1"]
        feed.detach(token: token, paneId: "p1")

        // A later warm attach must not be answerable from the stale ring.
        let reattached = feed.attach(token: token, paneId: "p1", lastSeq: 5, epoch: "e1")
        XCTAssertTrue(reattached.truncated)
        XCTAssertEqual(reattached.replay, "")
    }

    func testDisconnectRemovesStream() {
        let provider = PaneBytesProviderSpy()
        let feed = CompanionPaneBytesFeed(provider: provider)
        let token = feed.addWatcher { _ in }
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)

        feed.removeWatcher(token)

        XCTAssertEqual(provider.removals, ["p1"])
    }

    func testPaneClosedRemovesStream() {
        let provider = PaneBytesProviderSpy()
        let feed = CompanionPaneBytesFeed(provider: provider)
        let token = feed.addWatcher { _ in }
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)

        feed.handlePaneClosed(paneId: "p1")

        XCTAssertEqual(provider.removals, ["p1"])
        // The watch is gone too, so a later detach cannot double-remove.
        feed.detach(token: token, paneId: "p1")
        XCTAssertEqual(provider.removals, ["p1"])
    }

    /// End-to-end shape of the producer path: the sink handed to the provider must
    /// land in the ring and fan out with the producer's own epoch and offsets.
    func testInstalledStreamSinkFeedsTheLane() {
        let provider = PaneBytesProviderSpy()
        let feed = CompanionPaneBytesFeed(provider: provider)
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)

        provider.sinks["p1"]?("surface-epoch", 512, Data("hi".utf8))

        XCTAssertEqual(chunks.count, 1)
        XCTAssertEqual(chunks[0].epoch, "surface-epoch")
        XCTAssertEqual(chunks[0].seq, 512)
        XCTAssertEqual(String(data: b64Decode(chunks[0].data), encoding: .utf8), "hi")
    }

    func testRingSliceAtHeadIsEmpty() {
        var ring = CompanionPaneBytesRing(capacity: 64)
        ring.append(Data("abcd".utf8))
        let empty = ring.slice(fromSeq: 4)
        XCTAssertEqual(empty, Data())
        XCTAssertNil(ring.slice(fromSeq: 5))
        XCTAssertNil(ring.slice(fromSeq: -1))
    }

    func testRingResetRebasesToAbsoluteOffset() {
        var ring = CompanionPaneBytesRing(capacity: 64)
        ring.append(Data("abcd".utf8))
        ring.reset(toSeq: 900)
        XCTAssertTrue(ring.isEmpty)
        XCTAssertEqual(ring.nextSeq, 900)
        ring.append(Data("xy".utf8))
        XCTAssertEqual(ring.tail().startSeq, 900)
        XCTAssertEqual(ring.slice(fromSeq: 900), Data("xy".utf8))
        XCTAssertNil(ring.slice(fromSeq: 899))
    }
}

/// Records the feed's PTY-tee install/remove edges and holds the installed sinks
/// so a test can drive the producer path without a real surface.
@MainActor
private final class PaneBytesProviderSpy: CompanionPaneBytesProviding {
    private(set) var installs: [String] = []
    private(set) var removals: [String] = []
    var sinks: [String: CompanionPaneBytesStreamSink] = [:]
    /// Panes the spy pretends it cannot resolve, so a test can exercise the
    /// failed-install path (a pane whose window is still restoring, or gone).
    var unresolvablePanes: Set<String> = []

    @discardableResult
    func companionSetPaneByteStream(paneId: String, onBytes: CompanionPaneBytesStreamSink?) -> Bool {
        guard !unresolvablePanes.contains(paneId) else { return false }
        guard let onBytes else {
            removals.append(paneId)
            sinks[paneId] = nil
            return true
        }
        installs.append(paneId)
        sinks[paneId] = onBytes
        return true
    }
}
