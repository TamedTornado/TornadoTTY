import AppKit

/// The desktop overlay shown over a pane while a phone holds its control lease
/// (spec §2.6). The pane's live surface keeps rendering behind the overlay (a
/// render keep-alive streams it to the phone mirror), so the overlay lets that
/// content stay visible but *recede*: a translucent scrim darkens the evolving
/// terminal while a centered frosted card names the controlling device and
/// offers a "Take Back Control" button that reclaims the pane instantly.
///
/// Compositing note: the scrim is a plain layer-backed view (a dynamic dark
/// fill) rather than a full-pane vibrancy view. Within-window vibrancy is not a
/// dependable way to sample the pane's Metal-backed surface, so we darken with a
/// translucent layer that always composites correctly over Metal and keeps the
/// live content visible underneath. The frosted look is reserved for the small
/// card, which sits over the scrim (a normal layer) and therefore blurs
/// predictably.
///
/// The view stands alone from system materials so it can be exercised in a
/// detached AppKit component test with no window or theme injection.
@MainActor
final class CompanionLeasePlaceholderView: NSView {
    private enum Layout {
        static let cornerRadius: CGFloat = 14
        static let cardInset: CGFloat = 26
        static let cardMaxWidth: CGFloat = 340
        static let glyphPointSize: CGFloat = 30
        static let glyphToTitle: CGFloat = 14
        static let titleToMessage: CGFloat = 5
        static let messageToButton: CGFloat = 18
    }

    private enum Animation {
        static let fadeDuration: TimeInterval = 0.18
        static let appearScale: CGFloat = 0.96
    }

    private let onTakeBack: () -> Void

    private let scrimView = ScrimView()
    private let cardContainer = NSView()
    private let cardView = NSVisualEffectView()
    private let glyphView = NSImageView()
    private let titleLabel = NSTextField(labelWithString: "Controlled remotely")
    private let messageLabel = NSTextField(wrappingLabelWithString: "")
    private let takeBackButton = NSButton(title: "Take Back Control", target: nil, action: nil)

    init(deviceName: String, onTakeBack: @escaping () -> Void) {
        self.onTakeBack = onTakeBack
        super.init(frame: .zero)
        setup(deviceName: deviceName)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// Updates the controlling-device line without rebuilding the view (used when a
    /// lease is superseded by another device without an intervening restore).
    func updateDeviceName(_ deviceName: String) {
        messageLabel.stringValue = Self.message(for: deviceName)
    }

    private static func message(for deviceName: String) -> String {
        let trimmed = deviceName.trimmingCharacters(in: .whitespacesAndNewlines)
        let name = trimmed.isEmpty ? "another device" : trimmed
        return "This pane is controlled by \(name)."
    }

    private func setup(deviceName: String) {
        wantsLayer = true
        translatesAutoresizingMaskIntoConstraints = false

        scrimView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(scrimView)

        // The container carries the (unclipped) drop shadow; the material inside
        // clips itself to the rounded corners. Keeping them separate lets the card
        // both cast a shadow and mask its blur.
        cardContainer.wantsLayer = true
        cardContainer.layer?.masksToBounds = false
        cardContainer.translatesAutoresizingMaskIntoConstraints = false
        addSubview(cardContainer)

        cardView.material = .hudWindow
        cardView.blendingMode = .withinWindow
        cardView.state = .active
        cardView.wantsLayer = true
        cardView.layer?.cornerRadius = Layout.cornerRadius
        cardView.layer?.cornerCurve = .continuous
        cardView.layer?.masksToBounds = true
        cardView.translatesAutoresizingMaskIntoConstraints = false
        cardContainer.addSubview(cardView)

        if let glyph = NSImage(systemSymbolName: "iphone", accessibilityDescription: nil) {
            glyphView.image = glyph
            glyphView.symbolConfiguration = NSImage.SymbolConfiguration(
                pointSize: Layout.glyphPointSize,
                weight: .regular
            )
        }
        glyphView.contentTintColor = .secondaryLabelColor
        glyphView.imageScaling = .scaleProportionallyUpOrDown
        glyphView.translatesAutoresizingMaskIntoConstraints = false
        glyphView.setContentHuggingPriority(.required, for: .vertical)

        titleLabel.font = NSFont.systemFont(ofSize: 15, weight: .semibold)
        titleLabel.textColor = .labelColor
        titleLabel.alignment = .center
        titleLabel.translatesAutoresizingMaskIntoConstraints = false

        messageLabel.stringValue = Self.message(for: deviceName)
        messageLabel.font = NSFont.systemFont(ofSize: 12, weight: .regular)
        messageLabel.textColor = .secondaryLabelColor
        messageLabel.alignment = .center
        messageLabel.maximumNumberOfLines = 0
        messageLabel.translatesAutoresizingMaskIntoConstraints = false

        takeBackButton.bezelStyle = .rounded
        takeBackButton.controlSize = .large
        takeBackButton.keyEquivalent = "\r"
        takeBackButton.target = self
        takeBackButton.action = #selector(handleTakeBack)
        takeBackButton.translatesAutoresizingMaskIntoConstraints = false

        cardView.addSubview(glyphView)
        cardView.addSubview(titleLabel)
        cardView.addSubview(messageLabel)
        cardView.addSubview(takeBackButton)

        NSLayoutConstraint.activate([
            scrimView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrimView.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrimView.topAnchor.constraint(equalTo: topAnchor),
            scrimView.bottomAnchor.constraint(equalTo: bottomAnchor),

            cardContainer.centerXAnchor.constraint(equalTo: centerXAnchor),
            cardContainer.centerYAnchor.constraint(equalTo: centerYAnchor),
            cardContainer.widthAnchor.constraint(lessThanOrEqualToConstant: Layout.cardMaxWidth),
            cardContainer.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: 12),
            cardContainer.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -12),

            cardView.leadingAnchor.constraint(equalTo: cardContainer.leadingAnchor),
            cardView.trailingAnchor.constraint(equalTo: cardContainer.trailingAnchor),
            cardView.topAnchor.constraint(equalTo: cardContainer.topAnchor),
            cardView.bottomAnchor.constraint(equalTo: cardContainer.bottomAnchor),

            glyphView.topAnchor.constraint(equalTo: cardView.topAnchor, constant: Layout.cardInset),
            glyphView.centerXAnchor.constraint(equalTo: cardView.centerXAnchor),

            titleLabel.topAnchor.constraint(equalTo: glyphView.bottomAnchor, constant: Layout.glyphToTitle),
            titleLabel.leadingAnchor.constraint(equalTo: cardView.leadingAnchor, constant: Layout.cardInset),
            titleLabel.trailingAnchor.constraint(equalTo: cardView.trailingAnchor, constant: -Layout.cardInset),

            messageLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: Layout.titleToMessage),
            messageLabel.leadingAnchor.constraint(equalTo: titleLabel.leadingAnchor),
            messageLabel.trailingAnchor.constraint(equalTo: titleLabel.trailingAnchor),

            takeBackButton.topAnchor.constraint(equalTo: messageLabel.bottomAnchor, constant: Layout.messageToButton),
            takeBackButton.centerXAnchor.constraint(equalTo: cardView.centerXAnchor),
            takeBackButton.bottomAnchor.constraint(equalTo: cardView.bottomAnchor, constant: -Layout.cardInset),
        ])

        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        setAccessibilityLabel("Pane controlled remotely")

        applyCardShadow()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyCardShadow()
    }

    /// A soft drop shadow lifts the card off the receding surface. Rebuilt on
    /// appearance changes so it reads on both light and dark backdrops.
    private func applyCardShadow() {
        guard let layer = cardContainer.layer else { return }
        let isDark = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        layer.shadowColor = NSColor.black.cgColor
        layer.shadowOpacity = isDark ? 0.55 : 0.28
        layer.shadowRadius = 22
        layer.shadowOffset = CGSize(width: 0, height: -6)
        layer.masksToBounds = false
    }

    // MARK: - Appearance transitions

    /// Fades the overlay in from transparent with a subtle card lift, matching the
    /// app's short overlay timings (~180ms, ease-out).
    func animateIn() {
        alphaValue = 0
        cardContainer.layer?.transform = CATransform3DMakeScale(Animation.appearScale, Animation.appearScale, 1)
        NSAnimationContext.runAnimationGroup { context in
            context.duration = Animation.fadeDuration
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            context.allowsImplicitAnimation = true
            animator().alphaValue = 1
            cardContainer.layer?.transform = CATransform3DIdentity
        }
    }

    /// Fades the overlay out, then removes it from its superview. Safe to call on a
    /// view that has already been detached from the lease (the host clears its
    /// reference first so a fresh lease builds a new overlay).
    func animateOutAndRemove() {
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = Animation.fadeDuration
            context.timingFunction = CAMediaTimingFunction(name: .easeIn)
            animator().alphaValue = 0
        }, completionHandler: { [weak self] in
            self?.removeFromSuperview()
        })
    }

    @objc
    private func handleTakeBack() {
        onTakeBack()
    }

    // MARK: - Testing hooks

    var messageTextForTesting: String {
        messageLabel.stringValue
    }

    /// Fires the button's action exactly as a click would, for the detached
    /// component test (no window / run loop needed).
    func simulateTakeBackTapForTesting() {
        handleTakeBack()
    }
}

/// Translucent dark fill that darkens (but keeps visible) the live pane surface
/// behind the overlay. Re-resolves its color through `updateLayer` so it tracks
/// light/dark appearance changes — a one-time `cgColor` snapshot would not.
private final class ScrimView: NSView {
    override var wantsUpdateLayer: Bool { true }

    override func updateLayer() {
        let isDark = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        let alpha: CGFloat = isDark ? 0.55 : 0.42
        layer?.backgroundColor = NSColor.black.withAlphaComponent(alpha).cgColor
    }
}
