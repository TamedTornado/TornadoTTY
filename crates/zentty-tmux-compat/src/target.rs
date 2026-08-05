pub struct PaneTarget;

impl PaneTarget {
    #[must_use]
    pub fn resolve(
        selector: Option<&str>,
        available_pane_ids: &[String],
        fallback: &str,
    ) -> String {
        selector
            .map(|value| value.strip_prefix('%').unwrap_or(value))
            .filter(|candidate| available_pane_ids.iter().any(|pane| pane == candidate))
            .unwrap_or(fallback)
            .to_owned()
    }
}
