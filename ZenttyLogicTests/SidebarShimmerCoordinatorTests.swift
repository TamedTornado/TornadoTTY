import XCTest
@testable import Zentty

@MainActor
final class SidebarShimmerCoordinatorTests: XCTestCase {
    func test_shared_sidebar_shimmer_uses_shorter_pause_between_sweeps() {
        XCTAssertEqual(SidebarShimmerCoordinator.pauseRangeForTesting.lowerBound, 2.5)
        XCTAssertEqual(SidebarShimmerCoordinator.pauseRangeForTesting.upperBound, 4.0)
    }

    func test_braille_spinner_advances_without_mutating_source_title() {
        let view = SidebarShimmerTextView()
        view.stringValue = "Working ⠹ zentty"
        view.animatesBrailleSpinner = true
        view.isShimmering = true
        view.isVisibleForSharedAnimation = true

        XCTAssertEqual(view.displayedStringValueForTesting, "Working ⠋ zentty")

        for _ in 0..<SidebarShimmerTextView.spinnerTicksPerFrameForTesting {
            view.applySharedShimmerState(phase: 0, inSweep: false)
        }

        XCTAssertEqual(view.displayedStringValueForTesting, "Working ⠙ zentty")
        XCTAssertEqual(view.stringValue, "Working ⠹ zentty")
    }

    func test_braille_spinner_respects_reduced_motion() {
        let view = SidebarShimmerTextView()
        view.stringValue = "Working ⠹ zentty"
        view.animatesBrailleSpinner = true
        view.isShimmering = true
        view.isVisibleForSharedAnimation = true
        view.reducedMotion = true

        for _ in 0..<(SidebarShimmerTextView.spinnerTicksPerFrameForTesting * 2) {
            view.applySharedShimmerState(phase: 0, inSweep: false)
        }

        XCTAssertEqual(view.displayedStringValueForTesting, "Working ⠋ zentty")
    }

    func test_braille_in_a_regular_title_is_never_rewritten() {
        let view = SidebarShimmerTextView()
        view.stringValue = "Working on ⠹ literal braille"
        view.animatesBrailleSpinner = true

        XCTAssertEqual(view.displayedStringValueForTesting, "Working on ⠹ literal braille")
    }

    func test_codex_spinner_replacement_preserves_braille_in_the_subject() {
        let view = SidebarShimmerTextView()
        view.stringValue = "Working ⠹ review ⠸ accessibility"
        view.animatesBrailleSpinner = true

        XCTAssertEqual(view.displayedStringValueForTesting, "Working ⠋ review ⠸ accessibility")
    }
}
