import Foundation
import OSLog

private let companionInputLogger = Logger(subsystem: "be.zenjoy.zentty", category: "CompanionInput")

// MARK: - Input sink seam

/// The injection primitives the router needs. Implemented by `AppDelegate`
/// (resolve pane → terminal runtime); faked in tests. Both return `false` when
/// the pane is unknown or has no live runtime.
///
/// Two paths, deliberately: printable text goes through `companionSendText`
/// (libghostty's text/paste path), while non-printable keys go through
/// `companionSendKey` (a real key event). The split matters — the text path wraps
/// input in bracketed paste, which strips the `ESC` from cursor-key CSI sequences
/// and turns a submitting `CR` into a literal `LF`. Control keys must not take it.
@MainActor
protocol CompanionInputSink: AnyObject {
    func companionSendText(_ text: String, toPaneId paneId: String) -> Bool
    func companionSendKey(_ key: TerminalSpecialKey, toPaneId paneId: String) -> Bool
}

// MARK: - Router

/// Turns `input.text` / `input.key` / `input.quickAction` messages into terminal
/// byte injections on the resolved pane, and produces the correlated
/// `input.ack`. `@MainActor` because injection touches the runtime graph.
@MainActor
final class CompanionInputRouter {
    private weak var sink: CompanionInputSink?

    init(sink: CompanionInputSink) {
        self.sink = sink
    }

    /// Handles an input-family message. Returns the ack payload the session
    /// sends back (correlated to the request via the envelope `replyTo`), or
    /// `nil` for a message this router does not own.
    func handle(_ message: CompanionMessage) -> CompanionInputAck? {
        switch message {
        case .inputText(let payload):
            return injectText(payload.text, into: payload.paneId)
        case .inputKey(let payload):
            return injectKey(Self.specialKey(for: payload.key), into: payload.paneId)
        case .inputQuickAction(let payload):
            switch Self.action(forQuickAction: payload.actionId) {
            case .key(let key):
                return injectKey(key, into: payload.paneId)
            case .text(let text):
                return injectText(text, into: payload.paneId)
            case .none:
                return CompanionInputAck(ok: false, error: "unknown_action")
            }
        default:
            return nil
        }
    }

    private func injectText(_ text: String, into paneId: String) -> CompanionInputAck {
        guard let sink else {
            return CompanionInputAck(ok: false, error: "unavailable")
        }
        let ok = sink.companionSendText(text, toPaneId: paneId)
        return CompanionInputAck(ok: ok, error: ok ? nil : "pane_not_found")
    }

    private func injectKey(_ key: TerminalSpecialKey, into paneId: String) -> CompanionInputAck {
        guard let sink else {
            return CompanionInputAck(ok: false, error: "unavailable")
        }
        let ok = sink.companionSendKey(key, toPaneId: paneId)
        return CompanionInputAck(ok: ok, error: ok ? nil : "pane_not_found")
    }

    // MARK: Key mapping

    /// Named wire key → the terminal's `TerminalSpecialKey`. The surface encodes
    /// the actual bytes via a real key event, so arrows honor the pane's DECCKM
    /// (application-cursor-key) mode and Return submits — neither of which survives
    /// the paste/text path. This 1:1 map is the whole "key policy": every named key
    /// is a key event, never pasted text.
    static func specialKey(for key: CompanionInputKey) -> TerminalSpecialKey {
        switch key {
        case .enter: return .enter
        case .escape: return .escape
        case .tab: return .tab
        case .up: return .up
        case .down: return .down
        case .right: return .right
        case .left: return .left
        case .ctrlC: return .ctrlC
        case .ctrlD: return .ctrlD
        case .ctrlZ: return .ctrlZ
        case .ctrlR: return .ctrlR
        }
    }

    /// How a quick action is delivered: a real key event, pasted text, or nothing.
    enum QuickAction: Equatable {
        case key(TerminalSpecialKey)
        case text(String)
        case none
    }

    /// Quick-action id → delivery.
    ///
    /// v1 is deliberately coarse: without the pane's current prompt shape the
    /// bridge cannot know which numbered option "approve" maps to, so it sends
    /// the safe defaults — Enter selects the highlighted choice (usually "Yes"),
    /// Escape cancels — plus explicit `option:N` presets the phone can build
    /// from a numbered menu. Enter/Escape/interrupt go through the key path (so
    /// Enter actually submits); `option:N` is a printable digit, so it pastes as
    /// text. M4 refines this once prompt heuristics feed the dashboard the
    /// concrete choices per pane.
    static func action(forQuickAction actionId: String) -> QuickAction {
        switch actionId {
        case "approve", "enter", "submit":
            return .key(.enter)
        case "deny", "escape", "cancel":
            return .key(.escape)
        case "interrupt":
            return .key(.ctrlC)
        default:
            if actionId.hasPrefix("option:") {
                let value = String(actionId.dropFirst("option:".count))
                guard !value.isEmpty, value.allSatisfy(\.isNumber) else { return .none }
                return .text(value)
            }
            return .none
        }
    }
}
