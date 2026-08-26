#![forbid(unsafe_code)]

mod agent_adapters;
mod agent_consent;
mod agent_fleet;
mod agent_launch;
mod agent_protocol;
mod agent_sleep_inhibition;
mod agent_status;
mod app_config;
mod appearance;
mod atomic_file_store;
mod attention_inbox;
mod bookmark_store;
mod clean_copy;
mod close_decision;
mod codex_title;
mod codex_transcript;
mod command_palette;
mod dev_server;
mod git_review_context;
mod global_search;
mod open_with;
mod pane_focus_history;
mod pane_layout;
mod pane_token_registry;
mod project_icon;
mod remote_transfer;
mod server_browser;
mod session_restore_store;
mod shortcut;
mod sidebar_width;
mod ssh_identity;
mod support_diagnostics;
mod task_runner;
mod workspace_recipe;
mod workspace_state;
mod workspace_template;

pub use agent_consent::{
    AgentIntegrationClass, AgentIntegrationGate, AgentIntegrationState, resolve_integration_gate,
};
pub use agent_fleet::{
    FleetPaneSnapshot, FleetState, FleetSummary, FleetWindowSource, build_fleet_snapshots,
};
pub use agent_launch::{
    AgentLaunchAction, AgentLaunchError, AgentLaunchPlan, AgentLaunchTool,
    agent_launch_requires_bootstrap, build_agent_launch_plan, build_copilot_config,
    build_gemini_settings, build_small_harness_hooks,
};
pub use agent_protocol::{
    AgentArtifactKind, AgentArtifactLink, AgentEvent, AgentInteractionKind, AgentProtocolError,
};
pub use agent_sleep_inhibition::{AgentSleepInhibitionState, SleepInhibitionTransition};
pub use agent_status::{
    AgentPhase, AgentProgress, AgentSignalConfidence, AgentSignalOrigin, AgentStatusStore,
    PaneAgentStatus, TerminalProgressState,
};
pub use app_config::{
    AgentCaffeinationConfig, AgentIntegrationsConfig, AgentTeamsConfig, AppConfig,
    AppearanceConfig, ClipboardConfig, ConfirmationsConfig, ErrorReportingConfig,
    FocusFollowsMouseDelay, MenuBarConfig, NewWorklanePlacement, NotificationsConfig,
    OpenWithConfig, OpenWithCustomApp, PaneConfig, PaneLayoutConfig, PaneRightBehaviorMode,
    PartialAppConfig, RestoreConfig, ServerBrowserCustomApp, ServerDetectionConfig, SidebarConfig,
    SidebarVisibilityMode, UpdateChannel, UpdatesConfig, WorklaneConfig,
};
pub use appearance::{
    BackgroundOpacity, FALLBACK_DARK_THEME, FALLBACK_LIGHT_THEME, SidebarSelectionEmphasis,
    ThemeMode, ThemeModeCommand, ThemeSpec, update_ghostty_value,
};
pub use atomic_file_store::{AtomicFileAction, AtomicFileStore, AtomicFileStoreError};
pub use attention_inbox::{
    AttentionDelivery, AttentionInbox, AttentionItem, AttentionState, AttentionTarget,
    interaction_fallback, interaction_status,
};
pub use bookmark_store::{
    BookmarkStore, BookmarkStoreError, BookmarkStoreSnapshot, WorkspaceTemplateExportEnvelope,
};
pub use pane_focus_history::PaneReference;
pub use pane_layout::{PaneLayoutPolicy, PaneRightInsertionBehavior};
pub use pane_token_registry::{
    AgentTarget, AuthenticatedAgentEvent, AuthenticatedTarget, CapabilityAuthority, PaneTokenError,
    PaneTokenRegistry,
};
pub use project_icon::{ProjectIconCache, ProjectIconLookup};
pub use remote_transfer::{
    MAX_REMOTE_FILE_BYTES, MAX_REMOTE_IMAGE_BYTES, RemoteTransferFailure, RemoteTransferMethod,
    RemoteTransferPrerequisites, RemoteUploadPath, RemoteUploadPathError, RemoteVerificationPlan,
    RemoteVerificationPlanError, escape_remote_path_for_shell, scp_connection_arguments,
    ssh_connection_arguments,
};
pub use server_browser::{
    SYSTEM_DEFAULT_BROWSER_ID, ServerBrowserCatalog, ServerBrowserLaunchError,
    ServerBrowserLaunchPlan, ServerBrowserLauncher, ServerBrowserTarget,
};
pub use session_restore_store::{
    LaunchDecision, LaunchReason, PersistenceRequest, SessionRestoreStore,
    SessionRestoreStoreError, SnapshotPersistence,
};
pub use shortcut::{
    KeyboardShortcut, ShortcutBinding, ShortcutConflict, ShortcutDefinition, ShortcutKey,
    ShortcutManager, ShortcutModifier,
};
pub use sidebar_width::SidebarWidthPreference;
pub use ssh_identity::{SshConnectionOption, SshDestination, parse_ssh_destination};
pub use support_diagnostics::{
    DIAGNOSTIC_SCHEMA_VERSION, DiagnosticDraft, DiagnosticReason, DiagnosticReport,
    DiagnosticState, MAX_DIAGNOSTIC_CONTEXT_FIELDS, MAX_DIAGNOSTIC_CONTEXT_VALUE_BYTES,
    MAX_DIAGNOSTIC_DETAIL_BYTES, redact_text,
};
pub use task_runner::{
    TaskRunnerAction, TaskRunnerDisabledReason, TaskRunnerSourceKind, discover_task_runners,
    revalidate_task_runner,
};
pub use workspace_recipe::{
    AgentLaunchSnapshot, ColumnRecipe, PaneRecipe, PaneRestoreDraft, RestoreDraftKind, SaveReason,
    SessionRestoreDraftWindow, SessionRestoreEnvelope, WindowFrame, WindowRecipe, WorklaneRecipe,
    WorkspaceRecipe,
};
pub use workspace_state::{
    ClosePaneOutcome, CodexTranscriptEnrichmentCandidate, PaneColumnState, PaneCrossWindowTransfer,
    PaneMoveSplitAxis, PaneMoveTarget, PaneResizeDirection, PaneState, PaneWindowTransfer,
    SidebarPaneSummary, SidebarWorklaneSummary, WorklaneColor, WorklaneState, WorkspaceState,
    WorkspaceStateImportError,
};
pub use workspace_template::{
    TemplateKind, TemplateRestoreError, TemplateRestoreFallback, WorkspaceTemplate,
    WorkspaceTemplateBundle, WorkspaceTemplateCaptureContext, WorkspaceTemplateColumn,
    WorkspaceTemplatePane, WorkspaceTemplatePaneLaunch, WorkspaceTemplateRestore,
};

/// Stable product identity shared by platform composition roots.
pub const PRODUCT_NAME: &str = "Zentty";
pub const APPLICATION_ID: &str = "com.zentty.zentty";
pub use agent_adapters::{
    AgentAdapterError, adapt_agy_hook, adapt_claude_hook, adapt_codex_hook, adapt_codex_notify,
    adapt_copilot_hook, adapt_cursor_hook, adapt_droid_hook, adapt_gemini_hook, adapt_grok_hook,
    adapt_hermes_hook, adapt_kimi_hook, adapt_small_harness_hook, adapt_vibe_hook,
};
pub use clean_copy::{
    CleanCopyOptions, CleanCopyResult, CommandFlattenAggressiveness, clean_copy,
    is_likely_markdown, reformat_markdown,
};
pub use close_decision::{
    CloseDecision, CloseEvidence, ClosePaneEvidence, CloseReason, CloseTarget, decide_close,
};
pub use codex_title::{
    CodexTitlePhase, CodexTitleSignal, classify_codex_terminal_title, stable_codex_terminal_title,
};
pub use codex_transcript::{
    CodexTranscriptCacheKey, CodexTranscriptQuestion, codex_question_from_transcript_path,
    codex_question_from_transcript_text, codex_transcript_cache_key,
    locate_recent_codex_transcript_path,
};
pub use command_palette::{
    CommandPaletteGroup, CommandPaletteItem, CommandPaletteSection, CommandPaletteSectionKind,
    CommandPaletteTarget, RecentCommandTargets, resolve_command_palette,
    resolve_command_palette_sections,
};
pub use dev_server::{
    DetectedServer, DetectedServerConfidence, DetectedServerSource, RankedServer, ServerPortRule,
    ServerRegistry, ServerRelevanceContext, ServerRelevanceReason, ServerRelevanceTier,
    ServerTerminationObservation, ServerTerminationTarget, ServerUrlCandidate, ServerUrlError,
    authorize_server_termination, detect_server_urls, normalize_server_url, rank_servers,
};
pub use git_review_context::{
    ChecksState, GitHostKind, GitReference, GitRemote, ProjectContext, ProjectContextError,
    PullRequestState, PullRequestSummary, ReviewChip, ReviewChipStyle, ReviewContext,
    SystemProjectContextResolver, parse_git_remote,
};
pub use global_search::{
    GlobalSearchCoordinator, GlobalSearchDirection, GlobalSearchEffect, GlobalSearchState,
    GlobalSearchTarget,
};
pub use open_with::{
    LINUX_OPEN_WITH_BUILTIN_IDS, OpenWithCatalog, OpenWithLaunchError, OpenWithLaunchPlan,
    OpenWithLauncher, OpenWithTarget, OpenWithTargetKind, SYSTEM_FILE_MANAGER_ID,
    SYSTEM_TERMINAL_ID,
};
