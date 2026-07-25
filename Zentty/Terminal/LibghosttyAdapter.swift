import AppKit
import GhosttyKit

extension TerminalSurfaceContext {
    var libghosttyValue: ghostty_surface_context_e {
        switch self {
        case .window:
            GHOSTTY_SURFACE_CONTEXT_WINDOW
        case .tab:
            GHOSTTY_SURFACE_CONTEXT_TAB
        case .split:
            GHOSTTY_SURFACE_CONTEXT_SPLIT
        }
    }
}

@MainActor
protocol LibghosttyRuntimeProviding: AnyObject {
    func makeSurface(
        for hostView: LibghosttyView,
        paneID: PaneID,
        request: TerminalSessionRequest,
        configTemplate: ghostty_surface_config_s?,
        metadataDidChange: @escaping (TerminalMetadata) -> Void,
        eventDidOccur: @escaping (TerminalEvent) -> Void
    ) throws -> any LibghosttySurfaceControlling

    func reloadConfig()
    func applyBackgroundBlur(to window: NSWindow)
}

enum TerminalKeyAction: Equatable {
    case press
    case release
    case repeatPress
}

@MainActor
protocol LibghosttySurfaceControlling: AnyObject {
    var hasScrollback: Bool { get }
    var mouseCaptured: Bool { get }
    var mouseScrollIsTerminalInput: Bool { get }
    var cellWidth: CGFloat { get }
    var cellHeight: CGFloat { get }
    var searchDidChange: ((TerminalSearchEvent) -> Void)? { get set }
    func updateViewport(size: CGSize, scale: CGFloat, displayID: UInt32?)
    func setFocused(_ isFocused: Bool)
    func setOcclusionVisible(_ isVisible: Bool)
    func refresh()
    func translatedKeyEvent(for event: NSEvent) -> NSEvent
    func sendKey(event: NSEvent, action: TerminalKeyAction, text: String?, composing: Bool) -> Bool
    func sendMouseScroll(x: Double, y: Double, precision: Bool, momentum: NSEvent.Phase)
    func setSmoothScrollingEnabled(_ enabled: Bool)
    func sendMousePosition(_ position: CGPoint, modifiers: NSEvent.ModifierFlags)
    func sendMouseButton(
        state: ghostty_input_mouse_state_e,
        button: ghostty_input_mouse_button_e,
        modifiers: NSEvent.ModifierFlags
    ) -> Bool
    func sendText(_ text: String)
    func sendSpecialKey(_ key: TerminalSpecialKey) -> Bool
    func cancelPromptInput()
    @discardableResult func submitReturn() -> Bool
    func performBindingAction(_ action: String) -> Bool
    func scroll(toOffset offset: Double)
    func hasSelection() -> Bool
    func close()
    func inheritedConfig(for context: ghostty_surface_context_e) -> ghostty_surface_config_s?
}

extension LibghosttySurfaceControlling {
    var mouseCaptured: Bool { false }
    var mouseScrollIsTerminalInput: Bool { mouseCaptured }
    func cancelPromptInput() {}
    func sendSpecialKey(_ key: TerminalSpecialKey) -> Bool { false }
    func translatedKeyEvent(for event: NSEvent) -> NSEvent { event }
    func setSmoothScrollingEnabled(_ enabled: Bool) {}
    func scroll(toOffset offset: Double) {
        _ = performBindingAction("scroll_to_row:\(Int(offset.rounded(.down)))")
    }
}

@MainActor
protocol LibghosttySurfaceTextReading: AnyObject {
    func readText(includeScrollback: Bool, lineLimit: Int?) -> String?
    var gridSize: (cols: Int, rows: Int)? { get }
}

@MainActor
final class LibghosttyAdapter: TerminalAdapter, TerminalSearchControlling, TerminalTextReading, TerminalControlLeasing, TerminalRenderKeepAliving {
    private let runtime: any LibghosttyRuntimeProviding
    private let paneID: PaneID
    private let diagnostics: TerminalDiagnostics
    private let hostView = LibghosttyView()
    private lazy var scrollHostView = LibghosttySurfaceScrollHostView(
        surfaceView: hostView,
        paneID: paneID,
        diagnostics: diagnostics
    )
    private var surfaceController: (any LibghosttySurfaceControlling)?
    private var lastSurfaceActivity = TerminalSurfaceActivity(isVisible: false, isFocused: false)
    private var hasAppliedSurfaceActivity = false
    /// True while a companion control lease pins the surface to a fixed grid; the
    /// desktop occlusion is normally suspended in that state.
    private var isUnderControlLease = false
    /// True while the phone is mirroring this pane; forces the surface un-occluded
    /// (overriding both backgrounding and a control lease) so it keeps repainting.
    private var companionRenderKeepAlive = false
    private var inheritedConfigTemplate: ghostty_surface_config_s?

    var hasScrollback: Bool { surfaceController?.hasScrollback ?? false }
    var cellWidth: CGFloat { surfaceController?.cellWidth ?? 0 }
    var cellHeight: CGFloat { surfaceController?.cellHeight ?? 0 }
    var metadataDidChange: ((TerminalMetadata) -> Void)?
    var eventDidOccur: ((TerminalEvent) -> Void)?
    var searchDidChange: ((TerminalSearchEvent) -> Void)?

    init(
        paneID: PaneID = PaneID("unknown"),
        runtime: any LibghosttyRuntimeProviding = LibghosttyRuntime.shared,
        diagnostics: TerminalDiagnostics = .shared
    ) {
        self.paneID = paneID
        self.runtime = runtime
        self.diagnostics = diagnostics
        hostView.onLocalEventDidOccur = { [weak self] event in
            self?.eventDidOccur?(event)
        }
    }

    func makeTerminalView() -> NSView {
        scrollHostView
    }

    func startSession(using request: TerminalSessionRequest) throws {
        guard surfaceController == nil else {
            return
        }

        try ZenttyPerformanceSignposts.interval("LibghosttyAdapterStartSession") {
            let surfaceController = try runtime.makeSurface(
                for: hostView,
                paneID: paneID,
                request: request,
                configTemplate: inheritedConfigTemplate,
                metadataDidChange: { [weak self] metadata in
                    self?.metadataDidChange?(metadata)
                },
                eventDidOccur: { [weak self] event in
                    self?.eventDidOccur?(event)
                }
            )

            hostView.bind(surfaceController: surfaceController)
            surfaceController.searchDidChange = { [weak self] event in
                self?.searchDidChange?(event)
            }
            self.surfaceController = surfaceController
            hasAppliedSurfaceActivity = false
            setSurfaceActivity(lastSurfaceActivity)
        }
    }

    func close() {
        surfaceController?.close()
        surfaceController = nil
    }

    func sendText(_ text: String) {
        surfaceController?.sendText(text)
    }

    // Companion control keys route here (not through sendText) so the ESC in
    // cursor-key CSI and a real Return survive libghostty's bracketed-paste
    // wrapping. Returns `false` when no live surface backs the pane.
    func sendSpecialKey(_ key: TerminalSpecialKey) -> Bool {
        surfaceController?.sendSpecialKey(key) ?? false
    }

    func cancelPromptInput() {
        surfaceController?.cancelPromptInput()
    }

    // Paste the command via ghostty_surface_text (which paste-wraps the bytes
    // when bracketed paste is enabled) and then fire a separate synthetic
    // Return *key* event. The key event bypasses bracketed-paste wrapping, so
    // zsh's zle widget sees a real `accept-line` instead of a literal `\r`
    // inside paste content (which it would otherwise treat as a multi-line
    // edit and never execute).
    func submitCommand(_ command: String) {
        surfaceController?.sendText(command)
        surfaceController?.submitReturn()
    }

    func readText(includeScrollback: Bool, lineLimit: Int?) -> String? {
        (surfaceController as? LibghosttySurfaceTextReading)?.readText(
            includeScrollback: includeScrollback,
            lineLimit: lineLimit
        )
    }

    var gridSize: (cols: Int, rows: Int)? {
        (surfaceController as? LibghosttySurfaceTextReading)?.gridSize
    }

    // MARK: - Control lease (companion §2.6)

    @discardableResult
    func applyControlLease(cols: Int, rows: Int) -> Bool {
        guard hostView.applyLeasedViewport(cols: cols, rows: rows) else { return false }
        // Suspend desktop rendering while the phone owns the surface. The
        // placeholder overlay covers the pane regardless, so this is a best-effort
        // optimization rather than the correctness guarantee — but it must yield to
        // a companion render keepalive, otherwise an occluded surface stops
        // repainting and the phone's own mirror goes dark.
        isUnderControlLease = true
        reapplyOcclusion()
        return true
    }

    func releaseControlLease() {
        hostView.releaseLeasedViewport()
        isUnderControlLease = false
        // Restore occlusion to whatever the pane's current activity (or an active
        // companion keepalive) implies.
        reapplyOcclusion()
    }

    // MARK: - Companion render keepalive

    func setCompanionRenderKeepAlive(_ active: Bool) {
        guard companionRenderKeepAlive != active else { return }
        companionRenderKeepAlive = active
        reapplyOcclusion()
    }

    /// Desired surface visibility, resolving the three inputs by precedence: a
    /// companion mirror pins it visible; otherwise a control lease occludes it;
    /// otherwise it follows the pane's activity (visible = foreground).
    private var shouldSurfaceRender: Bool {
        if companionRenderKeepAlive { return true }
        if isUnderControlLease { return false }
        return lastSurfaceActivity.isVisible
    }

    private func reapplyOcclusion() {
        surfaceController?.setOcclusionVisible(shouldSurfaceRender)
    }

    func setSurfaceActivity(_ activity: TerminalSurfaceActivity) {
        ZenttyPerformanceSignposts.interval("LibghosttyAdapterSetSurfaceActivity") {
            let isFirstApplication = !hasAppliedSurfaceActivity
            let previouslyAppliedActivity = isFirstApplication
                ? TerminalSurfaceActivity(isVisible: false, isFocused: false)
                : lastSurfaceActivity
            lastSurfaceActivity = activity

            guard let surfaceController else {
                return
            }

            if !isFirstApplication, previouslyAppliedActivity == activity {
                return
            }

            hasAppliedSurfaceActivity = true

            if isFirstApplication || previouslyAppliedActivity.isFocused != activity.isFocused {
                surfaceController.setFocused(activity.isFocused)
            }

            if isFirstApplication || previouslyAppliedActivity.isVisible != activity.isVisible {
                // Resolve through the shared precedence: a companion keepalive or a
                // control lease can override the raw activity visibility.
                surfaceController.setOcclusionVisible(shouldSurfaceRender)
            }

            if !previouslyAppliedActivity.isVisible && activity.isVisible {
                scrollHostView.needsLayout = true
                scrollHostView.layoutSubtreeIfNeeded()
                surfaceController.refresh()
            }
        }
    }

    func showSearch() {
        _ = surfaceController?.performBindingAction("start_search")
    }

    func useSelectionForFind() {
        _ = surfaceController?.performBindingAction("search_selection")
    }

    func updateSearch(needle: String) {
        _ = surfaceController?.performBindingAction("search:\(needle)")
    }

    func findNext() {
        _ = surfaceController?.performBindingAction("navigate_search:next")
    }

    func findPrevious() {
        _ = surfaceController?.performBindingAction("navigate_search:previous")
    }

    func endSearch() {
        _ = surfaceController?.performBindingAction("end_search")
    }
}

extension LibghosttyAdapter: TerminalSessionInheritanceConfiguring {
    func prepareSessionStart(
        from sourceAdapter: (any TerminalAdapter)?,
        context: TerminalSurfaceContext
    ) {
        guard surfaceController == nil else {
            return
        }

        guard
            let sourceAdapter = sourceAdapter as? LibghosttyAdapter,
            let inheritedConfig = sourceAdapter.surfaceController?.inheritedConfig(
                for: context.libghosttyValue
            )
        else {
            inheritedConfigTemplate = nil
            return
        }

        inheritedConfigTemplate = inheritedConfig
    }
}
