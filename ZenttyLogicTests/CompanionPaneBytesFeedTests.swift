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
        feed.ingest(paneId: "p1", epoch: "e1", bytes: payload)

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
        feed.ingest(paneId: "p1", epoch: "e1", bytes: Data("abcdefgh".utf8)) // 8 bytes

        // Phone held [0,4) → lastSeq=4 exclusive.
        let attached = feed.attach(token: token, paneId: "p1", lastSeq: 4, epoch: "e1")
        XCTAssertEqual(attached.startSeq, 4)
        XCTAssertEqual(attached.truncated, false)
        XCTAssertEqual(String(data: b64Decode(attached.replay), encoding: .utf8), "efgh")
    }

    func testWarmAttachTruncatedWhenRingRolled() {
        let feed = CompanionPaneBytesFeed(ringCapacity: 8)
        let token = feed.addWatcher { _ in }
        feed.ingest(paneId: "p1", epoch: "e1", bytes: Data("0123456789ABCDEF".utf8)) // 16 bytes

        // lastSeq=0 has rolled off a capacity-8 ring.
        let attached = feed.attach(token: token, paneId: "p1", lastSeq: 0, epoch: "e1")
        XCTAssertTrue(attached.truncated)
        XCTAssertEqual(attached.startSeq, 8) // only last 8 retained
        XCTAssertEqual(b64Decode(attached.replay).count, 8)
    }

    func testWarmAttachEpochMismatchIsTruncated() {
        let feed = CompanionPaneBytesFeed()
        let token = feed.addWatcher { _ in }
        feed.ingest(paneId: "p1", epoch: "e1", bytes: Data("abc".utf8))

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

        _ = feed.attach(token: tokenA, paneId: "p1", lastSeq: nil, epoch: nil)
        // B never attaches.

        feed.ingest(paneId: "p1", epoch: chunksA.isEmpty ? "e1" : "e1", bytes: Data("xy".utf8))
        // First attach may have minted an epoch before ingest; re-attach after ensuring epoch.
        // Re-ingest with explicit epoch for a clean fan-out assert:
        feed.ensureEpoch(paneId: "p1", epoch: "e1")
        // Clear and re-attach A on known epoch by detaching + cold again is messy —
        // just assert A received at least one chunk and B none.
        feed.ingest(paneId: "p1", epoch: "e1", bytes: Data("z".utf8))
        XCTAssertFalse(chunksA.isEmpty)
        XCTAssertTrue(chunksB.isEmpty)
        _ = tokenB
    }

    func testDetachStopsFanOut() {
        let feed = CompanionPaneBytesFeed()
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }
        feed.ensureEpoch(paneId: "p1", epoch: "e1")
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        feed.ingest(paneId: "p1", epoch: "e1", bytes: Data("a".utf8))
        let countAfterAttach = chunks.count
        feed.detach(token: token, paneId: "p1")
        feed.ingest(paneId: "p1", epoch: "e1", bytes: Data("b".utf8))
        XCTAssertEqual(chunks.count, countAfterAttach)
    }

    func testChunkSplitRespectsMaxSize() {
        let feed = CompanionPaneBytesFeed()
        var chunks: [CompanionPaneBytesChunk] = []
        let token = feed.addWatcher { chunks.append($0) }
        feed.ensureEpoch(paneId: "p1", epoch: "e1")
        _ = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)

        let big = Data(repeating: 0x41, count: CompanionPaneBytesFeed.maxChunkBytes + 10)
        feed.ingest(paneId: "p1", epoch: "e1", bytes: big)
        XCTAssertEqual(chunks.count, 2)
        XCTAssertEqual(b64Decode(chunks[0].data).count, CompanionPaneBytesFeed.maxChunkBytes)
        XCTAssertEqual(b64Decode(chunks[1].data).count, 10)
        XCTAssertEqual(chunks[0].seq, 0)
        XCTAssertEqual(chunks[1].seq, CompanionPaneBytesFeed.maxChunkBytes)
    }

    func testPaneClosedDropsRing() {
        let feed = CompanionPaneBytesFeed()
        let token = feed.addWatcher { _ in }
        feed.ingest(paneId: "p1", epoch: "e1", bytes: Data("hello".utf8))
        feed.handlePaneClosed(paneId: "p1")
        let attached = feed.attach(token: token, paneId: "p1", lastSeq: nil, epoch: nil)
        // Fresh empty stream after close.
        XCTAssertEqual(attached.replay, "")
        XCTAssertEqual(attached.startSeq, 0)
    }

    func testRingSliceAtHeadIsEmpty() {
        var ring = CompanionPaneBytesRing(capacity: 64)
        ring.append(Data("abcd".utf8))
        let empty = ring.slice(fromSeq: 4)
        XCTAssertEqual(empty, Data())
        XCTAssertNil(ring.slice(fromSeq: 5))
        XCTAssertNil(ring.slice(fromSeq: -1))
    }
}
