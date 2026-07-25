import XCTest

@testable import Zentty

/// Policy tests for `CompanionInputRouter`: which wire messages become pasted
/// text vs. real key events, and how quick actions resolve. The byte encoding
/// itself lives in libghostty (driven via `TerminalSpecialKey` key events), so
/// these assert the routing decision — the exact seam where the arrow-key and
/// Return regressions were introduced.
@MainActor
final class CompanionInputRouterTests: XCTestCase {
    private final class RecordingSink: CompanionInputSink {
        var texts: [(text: String, paneId: String)] = []
        var keys: [(key: TerminalSpecialKey, paneId: String)] = []
        var textResult = true
        var keyResult = true

        func companionSendText(_ text: String, toPaneId paneId: String) -> Bool {
            texts.append((text, paneId))
            return textResult
        }

        func companionSendKey(_ key: TerminalSpecialKey, toPaneId paneId: String) -> Bool {
            keys.append((key, paneId))
            return keyResult
        }
    }

    private var sink: RecordingSink!
    private var router: CompanionInputRouter!

    override func setUp() {
        super.setUp()
        sink = RecordingSink()
        router = CompanionInputRouter(sink: sink)
    }

    override func tearDown() {
        sink = nil
        router = nil
        super.tearDown()
    }

    // MARK: - input.text

    func testInputTextPastesVerbatim() {
        let ack = router.handle(.inputText(CompanionInputText(paneId: "p1", text: "hello")))
        XCTAssertEqual(ack?.ok, true)
        XCTAssertEqual(sink.texts.map(\.text), ["hello"])
        XCTAssertEqual(sink.keys.count, 0, "printable text must not take the key path")
    }

    // MARK: - input.key routes every named key through the key-event path

    func testNamedKeysRouteToKeyEventNeverText() {
        let cases: [(CompanionInputKey, TerminalSpecialKey)] = [
            (.enter, .enter),
            (.escape, .escape),
            (.tab, .tab),
            (.up, .up),
            (.down, .down),
            (.left, .left),
            (.right, .right),
            (.ctrlC, .ctrlC),
            (.ctrlD, .ctrlD),
            (.ctrlZ, .ctrlZ),
            (.ctrlR, .ctrlR),
        ]

        for (wireKey, expected) in cases {
            sink.keys.removeAll()
            sink.texts.removeAll()
            let ack = router.handle(.inputKey(CompanionInputKeyMessage(paneId: "p1", key: wireKey)))
            XCTAssertEqual(ack?.ok, true)
            XCTAssertEqual(sink.keys.map(\.key), [expected], "\(wireKey) should map to \(expected)")
            XCTAssertEqual(sink.texts.count, 0, "\(wireKey) must never be pasted as text")
        }
    }

    /// The exact regression: an arrow must be a key event, so libghostty emits a
    /// real CSI (with ESC), not a paste of "[A".
    func testArrowIsKeyEventNotEscapeSequenceText() {
        _ = router.handle(.inputKey(CompanionInputKeyMessage(paneId: "p1", key: .up)))
        XCTAssertEqual(sink.keys.map(\.key), [.up])
        XCTAssertFalse(sink.texts.contains { $0.text.contains("[") },
                       "arrow must not be delivered as a literal '[A' paste")
    }

    /// The other regression: Return is a key event (which submits) rather than a
    /// pasted CR/LF (which Claude Code treats as a newline insert).
    func testEnterIsKeyEventNotPastedNewline() {
        _ = router.handle(.inputKey(CompanionInputKeyMessage(paneId: "p1", key: .enter)))
        XCTAssertEqual(sink.keys.map(\.key), [.enter])
        XCTAssertEqual(sink.texts.count, 0)
    }

    // MARK: - Failure surfacing

    func testKeyFailureSurfacesPaneNotFound() {
        sink.keyResult = false
        let ack = router.handle(.inputKey(CompanionInputKeyMessage(paneId: "gone", key: .up)))
        XCTAssertEqual(ack?.ok, false)
        XCTAssertEqual(ack?.error, "pane_not_found")
    }

    // MARK: - Quick actions

    func testQuickActionApproveAndDenyAndInterruptAreKeyEvents() {
        for (actionId, expected): (String, TerminalSpecialKey) in [
            ("approve", .enter), ("submit", .enter), ("enter", .enter),
            ("deny", .escape), ("cancel", .escape), ("escape", .escape),
            ("interrupt", .ctrlC),
        ] {
            sink.keys.removeAll()
            sink.texts.removeAll()
            let ack = router.handle(.inputQuickAction(CompanionInputQuickAction(paneId: "p1", actionId: actionId)))
            XCTAssertEqual(ack?.ok, true)
            XCTAssertEqual(sink.keys.map(\.key), [expected], "quick action \(actionId)")
            XCTAssertEqual(sink.texts.count, 0)
        }
    }

    func testQuickActionOptionDigitPastesAsText() {
        let ack = router.handle(.inputQuickAction(CompanionInputQuickAction(paneId: "p1", actionId: "option:3")))
        XCTAssertEqual(ack?.ok, true)
        XCTAssertEqual(sink.texts.map(\.text), ["3"])
        XCTAssertEqual(sink.keys.count, 0)
    }

    func testUnknownQuickActionIsRejected() {
        let ack = router.handle(.inputQuickAction(CompanionInputQuickAction(paneId: "p1", actionId: "option:")))
        XCTAssertEqual(ack?.ok, false)
        XCTAssertEqual(ack?.error, "unknown_action")

        let bogus = router.handle(.inputQuickAction(CompanionInputQuickAction(paneId: "p1", actionId: "frobnicate")))
        XCTAssertEqual(bogus?.ok, false)
        XCTAssertEqual(bogus?.error, "unknown_action")
    }
}
