# `zentty-linux` inline-test audit — 2026-09-01

## Scope and interpretation

This audit covers every inline `#[test]` function under
`crates/zentty-linux/src`; integration tests under `crates/zentty-linux/tests`
are intentionally outside the count. Classification describes a test's
**primary intent**, not its strength. In particular, `behavioral` does not mean
mutation-proven, and an exact string assertion is a `contract/snapshot` when
the string is an external UI, protocol, or compatibility contract rather than
an implementation detail.

Category precedence is widget smoke, contract/snapshot, formatter-mirroring,
then behavioral. The focused audit check verifies the source count, listed
identities, uniqueness, and category arithmetic so this document cannot stay
green after tests silently appear or disappear.

## Counts

| Primary classification | Count | Meaning |
| --- | ---: | --- |
| Behavioral | 335 | Exercises state, branching, boundaries, parsing, persistence, or side effects. Strength remains subject to mutation. |
| Contract/snapshot | 29 | Pins an intentional source-parity, UI-copy, protocol, action, or accessibility contract. |
| Formatter-mirroring | 0 | No remaining test was classified as merely reconstructing an internal formatter. The pane-divider example was removed and replaced by semantic integration tests. |
| Widget smoke | 5 | Requires GTK initialization and establishes widget/accessibility construction, not product behavior. |
| **Total** | **369** | Inline tests only. |

Zero confirmed formatter-mirroring tests is not a claim that the other 369
tests have strong assertions. Several presentation tests contain exact output;
they are classified as contracts because changing the output is externally
observable. GH-146 must still prove protected behavioral and contract cohorts
with current-source mutation evidence.

## Explicit contract/snapshot inventory

<!-- category:contract-snapshot:start -->
```text
crates/zentty-linux/src/agents_settings.rs::source_agent_inventory_never_disappears_silently
crates/zentty-linux/src/appearance_settings.rs::theme_slots_explain_active_and_saved_inactive_behavior
crates/zentty-linux/src/application_shell/action_router.rs::registry_is_unique_complete_and_typed
crates/zentty-linux/src/application_shell/action_router.rs::every_registered_action_has_explicit_palette_policy
crates/zentty-linux/src/application_shell/clipboard_actions.rs::action_names_are_stable_product_receipts
crates/zentty-linux/src/application_shell/pane_runtime.rs::terminal_accessibility_label_is_stable_and_pane_specific
crates/zentty-linux/src/application_shell/shortcut_registry.rs::registry_ids_defaults_and_action_references_are_valid
crates/zentty-linux/src/application_shell/shortcut_registry.rs::source_categories_remain_complete_and_ordered
crates/zentty-linux/src/bookmarks_view.rs::template_rows_expose_standard_keyboard_context_menu_shortcuts
crates/zentty-linux/src/agent_status_view.rs::presents_running_progress_without_hiding_the_agent
crates/zentty-linux/src/agent_status_view.rs::presents_attention_reason_and_attention_state
crates/zentty-linux/src/agent_status_view.rs::idle_and_unresolved_stop_remain_distinct
crates/zentty-linux/src/agent_fleet.rs::status_and_accessibility_share_visible_incomplete_progress_copy
crates/zentty-linux/src/global_search_view.rs::query_visibility_navigation_sensitivity_and_actions_are_exact
crates/zentty-linux/src/global_search_view.rs::sidebar_vocabulary_and_accessibility_are_source_exact
crates/zentty-linux/src/pane_controls.rs::pane_local_controls_use_current_source_commands_without_conflation
crates/zentty-linux/src/settings_navigation.rs::source_sections_have_exact_identity_order_and_search_vocabulary
crates/zentty-linux/src/updates_privacy_settings.rs::source_channel_order_and_tokens_are_stable
crates/zentty-linux/src/updates_privacy_settings.rs::local_crash_capture_copy_never_claims_automatic_transmission
crates/zentty-linux/src/status_notifier.rs::aggregate_presentation_uses_protocol_status_without_a_second_fleet_model
crates/zentty-linux/src/status_notifier.rs::required_item_properties_have_the_declared_dbus_types
crates/zentty-linux/src/window_chrome.rs::leading_chrome_matches_the_source_control_order_and_availability
crates/zentty-linux/src/task_manager/view.rs::no_backend_column_contract_does_not_advertise_network_telemetry
crates/zentty-linux/src/workspace_pane_settings.rs::source_page_controls_are_named_and_owned_by_the_shared_config_models
crates/zentty-linux/src/source_ui.rs::linux_command_vocabulary_is_present_in_the_checked_in_zentty_source
crates/zentty-linux/src/source_ui.rs::distinct_rightward_behaviors_are_not_conflated
crates/zentty-linux/src/source_ui.rs::linux_action_surfaces_do_not_reintroduce_rejected_aliases
crates/zentty-linux/src/sidebar.rs::drag_feedback_contract_is_pinned_to_the_source_preview
crates/zentty-linux/src/sidebar.rs::active_worklane_reveal_only_scrolls_when_the_whole_card_is_not_visible
```
<!-- category:contract-snapshot:end -->

## Explicit widget-smoke inventory

<!-- category:widget-smoke:start -->
```text
crates/zentty-linux/src/activity_title.rs::activity_changes_only_the_fixed_width_spinner_label
crates/zentty-linux/src/application_shell/pane_runtime.rs::actual_terminal_widget_exposes_terminal_accessibility_semantics
crates/zentty-linux/src/sidebar.rs::actual_worklane_widgets_expose_the_accessibility_contract
crates/zentty-linux/src/sidebar.rs::detached_pane_context_menu_releases_its_widget_tree
crates/zentty-linux/src/worklane_peek.rs::actual_worklane_peek_widgets_expose_the_accessibility_contract
```
<!-- category:widget-smoke:end -->

The formatter-mirroring inventory is empty. Every inline test not explicitly
listed above is classified as behavioral by complement.

## Prioritized remediation

1. **P0 — mutation autonomy gate:** GH-146 must cover current source, reject a
   zero-mutant or stale receipt, require zero viable survivors/timeouts for its
   protected cohorts, and treat compiler-unviable mutations separately.
2. **P0 — source-text snapshots:** the `source_ui`, sidebar source scans,
   settings inventory, and pane-control parity tests should consume one typed
   parity manifest rather than searching Swift/Rust source text independently.
   Until then they are useful drift alarms, not behavioral proof.
3. **P0 — largest inline cohorts:** prioritize `config_store` (40),
   `tmux_compat` (20), `persistence_coordinator` (15), `application_shell`
   (15), `pane_runtime` (14), and `sidebar` (12 after the divider removal) for
   mutation sampling and semantic integration extraction. Their volume makes
   assertion weakness most consequential.
4. **P1 — widget smoke:** keep the four smoke tests, but do not count them as
   proof of pointer, focus, keyboard, or compositor behavior. Those claims
   belong in controlled nested-display journeys.
5. **P1 — presentation contracts:** mutate agent-status, fleet, action-name,
   settings, and task-manager projections. Exact copy can be a legitimate
   contract, but a surviving branch or constant replacement is still a weak
   oracle.
6. **P2 — placement:** move newly extracted pure semantics to public focused
   modules with `zentty-linux/tests` integration coverage. Do not mechanically
   relocate stateful GTK smoke tests merely to improve the integration-file
   count.
