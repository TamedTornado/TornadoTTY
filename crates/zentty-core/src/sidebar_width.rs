/// Source-derived sidebar sizing policy shared by Linux presentation and tests.
pub struct SidebarWidthPreference;

impl SidebarWidthPreference {
    pub const DEFAULT: i32 = 280;
    pub const MINIMUM: i32 = 180;
    pub const MAXIMUM: i32 = 420;
    pub const MINIMUM_CONTENT_WIDTH: i32 = 200;

    #[must_use]
    pub fn maximum(available_width: i32) -> i32 {
        if available_width <= 0 {
            return Self::MAXIMUM;
        }
        let fraction_based = available_width.saturating_mul(33) / 100;
        let content_guard = available_width - Self::MINIMUM_CONTENT_WIDTH;
        Self::MINIMUM.max(Self::MAXIMUM.min(fraction_based).min(content_guard))
    }

    #[must_use]
    pub fn clamped(width: i32, available_width: i32) -> i32 {
        width.clamp(Self::MINIMUM, Self::maximum(available_width))
    }
}

#[cfg(test)]
mod tests {
    use super::SidebarWidthPreference as Width;

    #[test]
    fn source_sidebar_width_contract_clamps_to_screen_and_content() {
        assert_eq!(Width::DEFAULT, 280);
        assert_eq!(Width::clamped(Width::DEFAULT, 1_200), 280);
        assert_eq!(Width::maximum(1_200), 396);
        assert_eq!(Width::clamped(Width::DEFAULT, 600), 198);
        assert_eq!(Width::clamped(50, 1_200), 180);
        assert_eq!(Width::clamped(900, 2_000), 420);
        assert_eq!(Width::clamped(Width::DEFAULT, 300), 180);
        assert_eq!(Width::maximum(0), 420);
    }
}
