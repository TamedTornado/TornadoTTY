#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScrollUnit {
    Wheel,
    Surface,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScrollSwitchResult {
    Previous,
    Next,
    Consumed,
    Unhandled,
}

#[derive(Debug, Default)]
pub(crate) struct PaneScrollSwitch {
    accumulated: f64,
    triggered: bool,
    cooldown_until_micros: i64,
}

impl PaneScrollSwitch {
    const SURFACE_THRESHOLD: f64 = 40.0;
    const WHEEL_THRESHOLD: f64 = 1.0;
    const SURFACE_COOLDOWN_MICROS: i64 = 150_000;

    pub(crate) fn reset(&mut self) {
        self.accumulated = 0.0;
        self.triggered = false;
    }

    pub(crate) fn handle(
        &mut self,
        dx: f64,
        dy: f64,
        shifted: bool,
        unit: ScrollUnit,
        now_micros: i64,
    ) -> ScrollSwitchResult {
        // Linux divergence (GH-211): tilt wheels emit discrete horizontal
        // notches under light sideways pressure during ordinary scrolling.
        // Only an explicit Shift+wheel switches from a discrete wheel; an
        // unshifted tilt stays with the terminal like any other wheel input.
        let delta = match unit {
            ScrollUnit::Wheel if shifted && dy != 0.0 => dy,
            ScrollUnit::Wheel if shifted && dx != 0.0 => dx,
            ScrollUnit::Surface if dx.abs() > dy.abs() && dx != 0.0 => dx,
            ScrollUnit::Wheel | ScrollUnit::Surface => return ScrollSwitchResult::Unhandled,
        };
        if unit == ScrollUnit::Surface && now_micros < self.cooldown_until_micros {
            return ScrollSwitchResult::Consumed;
        }
        if self.triggered {
            return ScrollSwitchResult::Consumed;
        }
        self.accumulated += delta;
        let threshold = match unit {
            ScrollUnit::Wheel => Self::WHEEL_THRESHOLD,
            ScrollUnit::Surface => Self::SURFACE_THRESHOLD,
        };
        if self.accumulated.abs() < threshold {
            return ScrollSwitchResult::Consumed;
        }
        self.triggered = true;
        if unit == ScrollUnit::Surface {
            self.cooldown_until_micros = now_micros.saturating_add(Self::SURFACE_COOLDOWN_MICROS);
        }
        if self.accumulated > 0.0 {
            ScrollSwitchResult::Next
        } else {
            ScrollSwitchResult::Previous
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PaneScrollSwitch, ScrollSwitchResult, ScrollUnit};

    const SOURCE: &str =
        include_str!("../../../Zentty/UI/PaneStrip/ScrollSwitchGestureHandler.swift");

    #[test]
    fn gestures_switch_once_and_leave_ordinary_vertical_scrollback_unhandled() {
        let mut gesture = PaneScrollSwitch::default();
        assert_eq!(
            gesture.handle(0.0, 20.0, false, ScrollUnit::Surface, 0),
            ScrollSwitchResult::Unhandled
        );
        assert_eq!(
            gesture.handle(0.0, 40.0, true, ScrollUnit::Surface, 0),
            ScrollSwitchResult::Unhandled
        );
        assert_eq!(
            gesture.handle(15.0, 2.0, false, ScrollUnit::Surface, 0),
            ScrollSwitchResult::Consumed
        );
        assert_eq!(
            gesture.handle(25.0, 0.0, false, ScrollUnit::Surface, 0),
            ScrollSwitchResult::Next
        );
        assert_eq!(
            gesture.handle(80.0, 0.0, false, ScrollUnit::Surface, 0),
            ScrollSwitchResult::Consumed
        );

        gesture.reset();
        assert_eq!(
            gesture.handle(40.0, 0.0, false, ScrollUnit::Surface, 100_000),
            ScrollSwitchResult::Consumed
        );
        assert_eq!(
            gesture.handle(40.0, 0.0, false, ScrollUnit::Surface, 150_000),
            ScrollSwitchResult::Next
        );
        gesture.reset();
        assert_eq!(
            gesture.handle(0.0, -1.0, true, ScrollUnit::Wheel, 150_000),
            ScrollSwitchResult::Previous
        );
        assert!(SOURCE.contains("case switchLeft"));
        assert!(SOURCE.contains("case switchRight"));
        assert!(SOURCE.contains("static let precise: CGFloat = 40"));
        assert!(SOURCE.contains("static let wheel: CGFloat = 1"));
        assert!(SOURCE.contains("postSwitchCooldown: TimeInterval = 0.15"));
    }

    #[test]
    fn discrete_wheel_tilt_never_switches_without_shift() {
        let mut gesture = PaneScrollSwitch::default();
        for dx in [-1.0, 1.0, -0.25, 3.0] {
            gesture.reset();
            assert_eq!(
                gesture.handle(dx, 0.0, false, ScrollUnit::Wheel, 0),
                ScrollSwitchResult::Unhandled
            );
        }
        gesture.reset();
        assert_eq!(
            gesture.handle(-1.0, 0.0, true, ScrollUnit::Wheel, 0),
            ScrollSwitchResult::Previous
        );
        gesture.reset();
        assert_eq!(
            gesture.handle(0.0, 1.0, true, ScrollUnit::Wheel, 0),
            ScrollSwitchResult::Next
        );
    }
}
