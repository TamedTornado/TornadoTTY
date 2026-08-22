#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScrollUnit {
    Wheel,
    Surface,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Result {
    Navigate(Direction),
    Consumed,
    Unhandled,
}

#[derive(Debug, Default)]
pub(crate) struct PeekScrollNavigation {
    axis: Option<Axis>,
    accumulated: f64,
    triggered: bool,
}

impl PeekScrollNavigation {
    const PRECISE_THRESHOLD: f64 = 40.0;
    const WHEEL_THRESHOLD: f64 = 1.0;

    pub(crate) fn reset(&mut self) {
        self.axis = None;
        self.accumulated = 0.0;
        self.triggered = false;
    }

    pub(crate) fn handle(&mut self, dx: f64, dy: f64, unit: ScrollUnit) -> Result {
        let axis = if dx.abs() > dy.abs() && dx != 0.0 {
            Axis::Horizontal
        } else if dy != 0.0 {
            Axis::Vertical
        } else {
            return Result::Unhandled;
        };
        if let Some(active) = self.axis {
            if active != axis {
                return Result::Consumed;
            }
        } else {
            self.axis = Some(axis);
        }

        let delta = match axis {
            Axis::Horizontal => dx,
            Axis::Vertical => dy,
        };
        if unit == ScrollUnit::Surface {
            if self.triggered {
                return Result::Consumed;
            }
            self.accumulated += delta;
            if self.accumulated.abs() < Self::PRECISE_THRESHOLD {
                return Result::Consumed;
            }
            self.triggered = true;
        } else if delta.abs() < Self::WHEEL_THRESHOLD {
            return Result::Consumed;
        }

        Result::Navigate(match (axis, delta.is_sign_positive()) {
            (Axis::Horizontal, true) => Direction::Right,
            (Axis::Horizontal, false) => Direction::Left,
            (Axis::Vertical, true) => Direction::Down,
            (Axis::Vertical, false) => Direction::Up,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, PeekScrollNavigation, Result, ScrollUnit};

    const SOURCE: &str =
        include_str!("../../../Zentty/UI/WorklanePeek/WorklanePeekKeyMonitor.swift");

    #[test]
    fn precise_scroll_locks_axis_accumulates_and_fires_once() {
        let mut gesture = PeekScrollNavigation::default();
        assert_eq!(
            gesture.handle(21.0, 2.0, ScrollUnit::Surface),
            Result::Consumed
        );
        assert_eq!(
            gesture.handle(20.0, 1.0, ScrollUnit::Surface),
            Result::Navigate(Direction::Right)
        );
        assert_eq!(
            gesture.handle(100.0, 0.0, ScrollUnit::Surface),
            Result::Consumed
        );
        assert_eq!(
            gesture.handle(0.0, 100.0, ScrollUnit::Surface),
            Result::Consumed
        );
        assert!(SOURCE.contains("static let precise: CGFloat = 40"));
        assert!(SOURCE.contains("guard activeAxis == axis"));
    }

    #[test]
    fn wheel_navigation_preserves_both_spatial_axes() {
        let mut gesture = PeekScrollNavigation::default();
        assert_eq!(
            gesture.handle(0.0, -1.0, ScrollUnit::Wheel),
            Result::Navigate(Direction::Up)
        );
        gesture.reset();
        assert_eq!(
            gesture.handle(-1.0, 0.0, ScrollUnit::Wheel),
            Result::Navigate(Direction::Left)
        );
        assert!(SOURCE.contains("static let wheel: CGFloat = 1"));
        assert!(SOURCE.contains("delta > 0 ? .down : .up"));
    }

    #[test]
    fn precise_scroll_reset_cancels_partial_and_completed_gestures() {
        let mut gesture = PeekScrollNavigation::default();
        assert_eq!(
            gesture.handle(-39.0, -2.0, ScrollUnit::Surface),
            Result::Consumed
        );
        gesture.reset();
        assert_eq!(
            gesture.handle(-2.0, -39.0, ScrollUnit::Surface),
            Result::Consumed
        );
        assert_eq!(
            gesture.handle(-1.0, -2.0, ScrollUnit::Surface),
            Result::Navigate(Direction::Up)
        );
        assert_eq!(
            gesture.handle(0.0, -100.0, ScrollUnit::Surface),
            Result::Consumed
        );
        gesture.reset();
        assert_eq!(
            gesture.handle(-40.0, 0.0, ScrollUnit::Surface),
            Result::Navigate(Direction::Left)
        );
    }

    #[test]
    fn delivered_natural_scroll_sign_is_not_inverted() {
        let mut gesture = PeekScrollNavigation::default();
        assert_eq!(
            gesture.handle(0.0, 40.0, ScrollUnit::Surface),
            Result::Navigate(Direction::Down)
        );
        gesture.reset();
        assert_eq!(
            gesture.handle(40.0, 0.0, ScrollUnit::Surface),
            Result::Navigate(Direction::Right)
        );
    }
}
