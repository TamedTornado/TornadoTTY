use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    command_palette::CommandPaletteView,
    global_search_view::GlobalSearchView,
    pane_controls::{self, PaneControlAction, PanePresentation},
    pane_dividers::{self, PaneDivider},
    pane_scroll_switch::{PaneScrollSwitch, ScrollSwitchResult, ScrollUnit},
    peek_scroll_navigation::{
        Direction as PeekScrollDirection, PeekScrollNavigation, Result as PeekScrollResult,
        ScrollUnit as PeekScrollUnit,
    },
    restore_notice::RestoreNotice,
    sidebar,
    sidebar_visibility::{Event as SidebarVisibilityEvent, Mode as SidebarVisibilityMode},
    source_ui,
    window_chrome::WindowChrome,
    worklane_peek::{
        self, Direction as PeekDirection, PanePreview, Phase as PeekPhase,
        SpatialDirection as PeekSpatialDirection, WorklanePeekView,
    },
};
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use zentty_core::{
    AgentPhase, AppConfig, ClosePaneOutcome, ColumnRecipe, CommandPaletteItem,
    GlobalSearchCoordinator, GlobalSearchDirection, PaneColumnState, PaneLayoutPolicy, PaneRecipe,
    PaneReference, PaneResizeDirection, PaneRestoreDraft, PaneWindowTransfer, ServerPortRule,
    ServerRelevanceContext, SidebarWidthPreference, TaskRunnerAction, WindowFrame, WindowRecipe,
    WorklaneColor, WorklaneRecipe, WorkspaceState, discover_task_runners, rank_servers,
};
use zentty_ghostty::GhosttyRuntime;

use crate::agent_runtime::AgentRuntime;

mod action_router;
mod agent_events;
mod bookmark_runtime;
mod clipboard_actions;
mod global_search;
pub(crate) mod open_with_runtime;
mod pane_runtime;
mod project_context_runtime;
mod remote_paste;
pub(crate) mod server_runtime;
pub(crate) mod shortcut_registry;
pub(crate) mod shortcut_runtime;
mod ssh_identity;
mod task_runner_runtime;
mod tmux_runtime;

use action_router::{
    ACTION_ADD_PANE_LEFT, ACTION_ADD_PANE_RIGHT, ACTION_ARRANGE_GOLDEN_NARROW,
    ACTION_ARRANGE_GOLDEN_SHORT, ACTION_ARRANGE_GOLDEN_TALL, ACTION_ARRANGE_GOLDEN_WIDE,
    ACTION_ARRANGE_HEIGHT_FOUR, ACTION_ARRANGE_HEIGHT_FULL, ACTION_ARRANGE_HEIGHT_THREE,
    ACTION_ARRANGE_HEIGHT_TWO, ACTION_ARRANGE_WIDTH_FULL, ACTION_ARRANGE_WIDTH_HALF,
    ACTION_ARRANGE_WIDTH_QUARTERS, ACTION_ARRANGE_WIDTH_THIRDS, ACTION_CLEAN_COPY,
    ACTION_CLOSE_ACTIVE_WORKLANE, ACTION_CLOSE_PANE, ACTION_CLOSE_WINDOW, ACTION_CLOSE_WORKLANE,
    ACTION_COPY, ACTION_COPY_AS_MARKDOWN, ACTION_COPY_RAW, ACTION_CYCLE_WORKLANE_COLOR,
    ACTION_DUPLICATE_PANE, ACTION_FIND, ACTION_FIND_NEXT, ACTION_FIND_PREVIOUS,
    ACTION_FOCUS_PANE_DOWN, ACTION_FOCUS_PANE_LEFT, ACTION_FOCUS_PANE_RIGHT, ACTION_FOCUS_PANE_UP,
    ACTION_GLOBAL_FIND, ACTION_IGNORE_SERVER_PORT, ACTION_MOVE_PANE_DOWN, ACTION_MOVE_PANE_LEFT,
    ACTION_MOVE_PANE_RIGHT, ACTION_MOVE_PANE_TO_NEW_WINDOW, ACTION_MOVE_PANE_UP,
    ACTION_MOVE_WORKLANE_DOWN, ACTION_MOVE_WORKLANE_UP, ACTION_NAVIGATE_BACK,
    ACTION_NAVIGATE_FORWARD, ACTION_NEW_PANE_RIGHT, ACTION_NEW_WINDOW, ACTION_NEW_WORKLANE,
    ACTION_NEXT_PANE, ACTION_NEXT_WORKLANE, ACTION_OPEN_BRANCH_REMOTE, ACTION_OPEN_PULL_REQUEST,
    ACTION_OPEN_SERVER, ACTION_OPEN_SERVER_BROWSER, ACTION_OPEN_SETTINGS,
    ACTION_OPEN_SETTINGS_SECTION, ACTION_OPEN_WITH_PRIMARY, ACTION_OPEN_WITH_TARGET,
    ACTION_PREVIOUS_PANE, ACTION_PREVIOUS_WORKLANE, ACTION_REFRESH_REVIEW_STATUS,
    ACTION_REFRESH_SERVERS, ACTION_RESET_PANE_LAYOUT, ACTION_RESIZE_PANE_DOWN,
    ACTION_RESIZE_PANE_LEFT, ACTION_RESIZE_PANE_RIGHT, ACTION_RESIZE_PANE_UP,
    ACTION_RESTORE_CLOSED_PANE, ACTION_SELECT_ALL, ACTION_SHOW_TASK_MANAGER,
    ACTION_SPLIT_PANE_BELOW, ACTION_SPLIT_PANE_RIGHT, ACTION_STOP_IGNORING_SERVER_PORT,
    ACTION_STOP_SERVER, ACTION_TOGGLE_LIGHT_DARK_THEME, ACTION_TOGGLE_SIDEBAR,
    ACTION_USE_AUTO_THEME, ACTION_USE_DARK_THEME, ACTION_USE_LIGHT_THEME,
    ACTION_USE_SELECTION_FOR_FIND, ActionRouter,
};
use agent_events::AgentEventCoordinator;
use pane_runtime::DetachedPaneRuntime;
use pane_runtime::PaneRuntimeCoordinator;
const WORKLANE_PEEK_TAB_HOLD_THRESHOLD: Duration = Duration::from_millis(200);
type SidebarTrackingState = (Rc<Cell<i32>>, Rc<Cell<bool>>, Rc<Cell<u64>>);

#[derive(Default)]
struct RemotePaneContext {
    probe_source: Option<glib::SourceId>,
    probe_in_flight: bool,
    identities: BTreeMap<String, ssh_identity::PaneSshIdentity>,
    uploads: BTreeMap<String, remote_paste::RemoteUploadActivity>,
}

fn install_shell_styles() {
    sidebar::install_styles();
    pane_controls::install_styles();
    pane_dividers::install_styles();
}

fn sidebar_tracking_state() -> SidebarTrackingState {
    (
        Rc::new(Cell::new(SidebarWidthPreference::DEFAULT)),
        Rc::new(Cell::new(false)),
        Rc::new(Cell::new(0)),
    )
}

pub(crate) struct ApplicationRuntimes {
    pub(crate) ghostty: GhosttyRuntime,
    pub(crate) agent: Rc<RefCell<AgentRuntime>>,
    pub(crate) config: AppConfig,
}

pub(crate) struct ApplicationShell {
    window: gtk::Window,
    chrome: WindowChrome,
    body: gtk::Paned,
    sidebar_reservation: gtk::Box,
    sidebar_hover_rail: gtk::Box,
    sidebar: gtk::Box,
    sidebar_scroll: gtk::ScrolledWindow,
    pane_scroll: gtk::ScrolledWindow,
    pane_box: gtk::Box,
    rendered_columns: RefCell<BTreeMap<String, gtk::Overlay>>,
    last_vertical_divider: RefCell<Option<(String, String)>>,
    background_agent_host: gtk::Box,
    state: WorkspaceState,
    pane_runtime: PaneRuntimeCoordinator,
    config: AppConfig,
    restored_pane_commands: BTreeMap<String, String>,
    main_loop: glib::MainLoop,
    next_worklane_number: usize,
    next_pane_number: usize,
    window_template: WindowRecipe,
    shutting_down: bool,
    preferred_sidebar_width: Rc<Cell<i32>>,
    adjusting_sidebar_width: Rc<Cell<bool>>,
    sidebar_reveal_generation: Rc<Cell<u64>>,
    sidebar_visibility: crate::sidebar_visibility::State,
    sidebar_visibility_generation: u64,
    peek_phase: PeekPhase,
    peek_generation: u64,
    peek_tab_down: bool,
    peek_view: WorklanePeekView,
    command_palette: CommandPaletteView,
    restore_notice: RestoreNotice,
    global_search_view: GlobalSearchView,
    global_search: GlobalSearchCoordinator,
    global_search_generation: u64,
    focus_follow_generation: u64,
    remote_panes: RemotePaneContext,
    server_runtime: server_runtime::ServerRuntime,
    project_context_runtime: project_context_runtime::ProjectContextRuntime,
    open_with_runtime: open_with_runtime::OpenWithRuntime,
    bookmark_runtime: bookmark_runtime::BookmarkRuntime,
    task_runner_actions: BTreeMap<String, TaskRunnerAction>,
    last_pane_viewport_height: Cell<i32>,
    action_router: Option<ActionRouter>,
    shortcut_manager: Rc<RefCell<zentty_core::ShortcutManager>>,
    shortcut_controller: Option<gtk::EventControllerKey>,
    shortcut_settings_window: Option<gtk::Window>,
    agent_events: AgentEventCoordinator,
    tmux_compat: crate::tmux_compat::TmuxCompatProduct,
    new_window_handler: Option<Rc<dyn Fn()>>,
    move_pane_to_new_window_handler: Option<Rc<dyn Fn(String)>>,
    show_task_manager_handler: Option<Rc<dyn Fn()>>,
    close_window_handler: Option<Rc<dyn Fn()>>,
    quit_handler: Option<Rc<dyn Fn()>>,
    self_handle: RefCell<Weak<RefCell<Self>>>,
}

pub(crate) struct ExtractedWindowPane {
    pub(crate) model: PaneWindowTransfer,
    pub(crate) destination_recipe: WindowRecipe,
    pub(crate) runtime: DetachedPaneRuntime,
    pub(crate) source_before: WorkspaceState,
}

struct ShellWidgets {
    window: gtk::Window,
    chrome: WindowChrome,
    body: gtk::Paned,
    sidebar: gtk::Box,
    sidebar_scroll: gtk::ScrolledWindow,
    sidebar_reservation: gtk::Box,
    sidebar_hover_rail: gtk::Box,
    pane_scroll: gtk::ScrolledWindow,
    pane_box: gtk::Box,
    background_agent_host: gtk::Box,
    peek_view: WorklanePeekView,
    command_palette: CommandPaletteView,
    restore_notice: RestoreNotice,
}

fn report_config_projection(shell: &ApplicationShell) {
    eprintln!(
        "zentty-linux: config-projected window={} automatic-clean-copy={}",
        shell.window_template.id, shell.config.clipboard.always_clean_copies
    );
}

fn finish_initial_render(shell: &Rc<RefCell<ApplicationShell>>) {
    shell.borrow().render();
    report_config_projection(&shell.borrow());
}

fn initialize_open_with(
    chrome: &WindowChrome,
    config: &zentty_core::OpenWithConfig,
) -> open_with_runtime::OpenWithRuntime {
    let runtime = open_with_runtime::OpenWithRuntime::discover(config);
    chrome.configure_open_with(&runtime.catalog);
    runtime
}

fn initialize_bookmarks() -> Result<bookmark_runtime::BookmarkRuntime, String> {
    bookmark_runtime::BookmarkRuntime::load_default()
}

fn create_initial_pane_surfaces(
    shell: &Rc<RefCell<ApplicationShell>>,
    pane_ids: Vec<String>,
    deferred_live_pane_id: Option<&str>,
) -> Result<(), String> {
    for pane_id in pane_ids {
        if deferred_live_pane_id == Some(pane_id.as_str()) {
            shell.borrow_mut().pane_runtime.mark_deferred(&pane_id)?;
        } else {
            PaneRuntimeCoordinator::create_surface(shell, &pane_id)?;
        }
    }
    Ok(())
}

fn finish_shell_setup(
    shell: &Rc<RefCell<ApplicationShell>>,
    body: &gtk::Paned,
    preferred_sidebar_width: Rc<Cell<i32>>,
    adjusting_sidebar_width: Rc<Cell<bool>>,
    initial_pane_ids: Vec<String>,
    deferred_live_pane_id: Option<&str>,
) -> Result<(), String> {
    initialize_shell_coordinators(shell);
    install_sidebar_width_tracking(
        body,
        &shell.borrow().sidebar_scroll,
        preferred_sidebar_width,
        adjusting_sidebar_width,
    );
    let action_router = ActionRouter::install(shell)?;
    shell.borrow_mut().action_router = Some(action_router);
    shortcut_registry::validate()?;
    let shortcut_controller = shortcut_runtime::install(
        &shell.borrow().window,
        Rc::clone(&shell.borrow().shortcut_manager),
    );
    shell.borrow_mut().shortcut_controller = Some(shortcut_controller);
    ApplicationShell::install_global_search_callbacks(shell);
    ApplicationShell::install_sidebar_visibility(shell);
    ApplicationShell::install_pane_traversal_shortcuts(shell);
    ApplicationShell::install_peek_scroll_navigation(shell);
    ApplicationShell::install_pane_scroll_switching(shell);
    ApplicationShell::install_search_shortcuts(shell);
    create_initial_pane_surfaces(shell, initial_pane_ids, deferred_live_pane_id)?;
    shell.borrow().mount_background_restored_agents();
    finish_initial_render(shell);
    Ok(())
}

fn render_empty_sidebar(
    sidebar: &gtk::Box,
    window: &gtk::Window,
    clipboard: zentty_core::ClipboardConfig,
) {
    sidebar::render(sidebar, window, &[], clipboard, &[], &[], None);
}

impl ApplicationShell {
    #[allow(clippy::too_many_lines)] // Initializes the single window authority and its owned coordinators.
    pub(crate) fn new(
        runtimes: &ApplicationRuntimes,
        command: Option<String>,
        main_loop: &glib::MainLoop,
        restored_window: Option<WindowRecipe>,
        restored_drafts: &[PaneRestoreDraft],
        fresh_window_id: &str,
        deferred_live_pane_id: Option<&str>,
    ) -> Result<Rc<RefCell<Self>>, String> {
        install_shell_styles();
        let ShellWidgets {
            window,
            chrome,
            body,
            sidebar,
            sidebar_scroll,
            sidebar_reservation,
            sidebar_hover_rail,
            pane_scroll,
            pane_box,
            background_agent_host,
            peek_view,
            command_palette,
            restore_notice,
        } = build_shell_widgets();
        render_empty_sidebar(&sidebar, &window, runtimes.config.clipboard);
        let global_search_view = GlobalSearchView::attach(&sidebar);
        let window_template = restored_or_default_window(restored_window, fresh_window_id);
        apply_restored_window_size(&window, &window_template);
        let (state, restored_pane_commands) =
            restore_workspace_state(&window_template, restored_drafts)?;
        let agent_events =
            AgentEventCoordinator::start(window_template.id.clone(), Rc::clone(&runtimes.agent));
        let open_with_runtime = initialize_open_with(&chrome, &runtimes.config.open_with);
        let bookmark_runtime = initialize_bookmarks()?;
        let (next_worklane_number, next_pane_number) = next_workspace_identities(&state);
        let initial_pane_ids = workspace_pane_ids(&state);
        let (preferred_sidebar_width, adjusting_sidebar_width, sidebar_reveal_generation) =
            sidebar_tracking_state();
        let shell = Rc::new(RefCell::new(Self {
            window,
            chrome,
            body: body.clone(),
            sidebar_reservation,
            sidebar_hover_rail,
            sidebar,
            sidebar_scroll,
            pane_scroll,
            pane_box,
            rendered_columns: RefCell::new(BTreeMap::new()),
            last_vertical_divider: RefCell::new(None),
            background_agent_host,
            state,
            pane_runtime: PaneRuntimeCoordinator::new(&runtimes.ghostty, command),
            config: runtimes.config.clone(),
            restored_pane_commands,
            main_loop: main_loop.clone(),
            next_worklane_number,
            next_pane_number,
            window_template,
            shutting_down: false,
            preferred_sidebar_width: Rc::clone(&preferred_sidebar_width),
            adjusting_sidebar_width: Rc::clone(&adjusting_sidebar_width),
            sidebar_reveal_generation,
            sidebar_visibility: crate::sidebar_visibility::State::default(),
            sidebar_visibility_generation: 0,
            peek_phase: PeekPhase::Idle,
            peek_generation: 0,
            peek_tab_down: false,
            peek_view,
            command_palette,
            restore_notice,
            global_search_view,
            global_search: GlobalSearchCoordinator::default(),
            global_search_generation: 0,
            focus_follow_generation: 0,
            remote_panes: RemotePaneContext::default(),
            server_runtime: server_runtime::ServerRuntime::discover(
                &runtimes.config.server_detection,
            ),
            project_context_runtime: project_context_runtime::ProjectContextRuntime::default(),
            open_with_runtime,
            bookmark_runtime,
            task_runner_actions: BTreeMap::new(),
            last_pane_viewport_height: Cell::new(0),
            action_router: None,
            shortcut_manager: Rc::new(RefCell::new(zentty_core::ShortcutManager::new(
                &shortcut_registry::definitions(),
                &runtimes.config.shortcuts,
            )?)),
            shortcut_controller: None,
            shortcut_settings_window: None,
            agent_events,
            tmux_compat: default_tmux_product()?,
            new_window_handler: None,
            move_pane_to_new_window_handler: None,
            show_task_manager_handler: None,
            close_window_handler: None,
            quit_handler: None,
            self_handle: RefCell::new(Weak::new()),
        }));
        finish_shell_setup(
            &shell,
            &body,
            preferred_sidebar_width,
            adjusting_sidebar_width,
            initial_pane_ids,
            deferred_live_pane_id,
        )?;
        Ok(shell)
    }

    pub(crate) fn window(&self) -> &gtk::Window {
        &self.window
    }

    pub(crate) fn task_manager_pane_sources(&self) -> Vec<crate::task_manager::PaneSource> {
        let status_by_pane = self
            .state
            .sidebar_summaries()
            .into_iter()
            .flat_map(|worklane| worklane.pane_rows)
            .filter_map(|pane| pane.agent_status.map(|status| (pane.pane_id, status)))
            .collect::<BTreeMap<_, _>>();
        let mut sources = Vec::new();
        let active_worklane_id = self.state.active_worklane_id().to_owned();
        for (worklane_index, worklane) in self.state.worklanes().iter().enumerate() {
            for column in &worklane.columns {
                for pane in &column.panes {
                    let status_text = status_by_pane.get(&pane.id).map(|status| {
                        let phase = match status.phase {
                            AgentPhase::Starting => "starting",
                            AgentPhase::Running => "running",
                            AgentPhase::NeedsInput => "needs input",
                            AgentPhase::Idle => "idle",
                            AgentPhase::UnresolvedStop => "stopped",
                        };
                        format!("{} {phase}", status.agent_name)
                    });
                    sources.push(crate::task_manager::PaneSource {
                        window_id: self.window_template.id.clone(),
                        window_title: self
                            .window
                            .title()
                            .map_or_else(|| "Zentty".to_owned(), |title| title.to_string()),
                        worklane_id: worklane.id.clone(),
                        worklane_title: worklane
                            .title
                            .clone()
                            .unwrap_or_else(|| format!("Worklane {}", worklane_index + 1)),
                        pane_id: pane.id.clone(),
                        pane_title: pane.display_title().to_owned(),
                        status_text,
                        root_pid: self
                            .pane_runtime
                            .surface(&pane.id)
                            .and_then(zentty_ghostty::GhosttySurface::foreground_process_id)
                            .and_then(|pid| u32::try_from(pid).ok()),
                        is_remote: pane.ssh_connection_label.is_some()
                            || self.remote_panes.identities.contains_key(&pane.id),
                        is_worklane_active: worklane.id == active_worklane_id,
                        working_directory: pane
                            .working_directory
                            .as_deref()
                            .map(std::path::PathBuf::from),
                    });
                }
            }
        }
        sources
    }

    pub(crate) fn focus_task_manager_pane(
        shell: &Rc<RefCell<Self>>,
        worklane_id: &str,
        pane_id: &str,
    ) -> bool {
        let mut shell = shell.borrow_mut();
        let worklane_changed = shell.state.active_worklane_id() != worklane_id;
        if !shell.state.select_worklane_and_pane(worklane_id, pane_id) {
            return false;
        }
        if worklane_changed {
            shell.render();
        } else {
            shell.refresh_sidebar_metadata();
        }
        shell.window.present();
        shell.focus_selected_surface();
        eprintln!(
            "zentty-linux: task-manager focus-pane window={} worklane={worklane_id} pane={pane_id}",
            shell.window_template.id
        );
        true
    }

    pub(crate) fn close_task_manager_pane(shell: &Rc<RefCell<Self>>, pane_id: &str) {
        Self::request_close_pane(shell, pane_id);
    }

    pub(crate) fn set_application_handlers(
        &mut self,
        new_window_handler: Rc<dyn Fn()>,
        move_pane_to_new_window_handler: Rc<dyn Fn(String)>,
        show_task_manager_handler: Rc<dyn Fn()>,
        close_window_handler: Rc<dyn Fn()>,
        quit_handler: Rc<dyn Fn()>,
    ) {
        self.new_window_handler = Some(new_window_handler);
        self.move_pane_to_new_window_handler = Some(move_pane_to_new_window_handler);
        self.show_task_manager_handler = Some(show_task_manager_handler);
        self.close_window_handler = Some(close_window_handler);
        self.quit_handler = Some(quit_handler);
    }

    pub(crate) fn extract_live_pane_to_new_window(
        &mut self,
        pane_id: &str,
        destination_worklane_id: &str,
    ) -> Result<ExtractedWindowPane, String> {
        let source_before = self.state.clone();
        let source_recipe = self.window_recipe();
        let model = self
            .state
            .split_pane_to_new_window(pane_id, destination_worklane_id)
            .ok_or_else(|| format!("pane {pane_id} cannot move to a new window"))?;
        let Some(destination_recipe) =
            model.destination_window_recipe(&source_recipe, "pending-window-transfer")
        else {
            self.state = source_before;
            return Err(format!(
                "pane {pane_id} has no source recipe metadata for window transfer"
            ));
        };
        let runtime = match self.pane_runtime.detach_for_window_transfer(pane_id) {
            Ok(runtime) => runtime,
            Err(error) => {
                self.state = source_before;
                return Err(error);
            }
        };
        self.render();
        self.focus_selected_surface();
        Ok(ExtractedWindowPane {
            model,
            destination_recipe,
            runtime,
            source_before,
        })
    }

    pub(crate) fn has_worklane(&self, worklane_id: &str) -> bool {
        self.state.worklane_ids().contains(&worklane_id)
    }

    pub(crate) fn rollback_live_pane_window_transfer(
        shell: &Rc<RefCell<Self>>,
        transfer: ExtractedWindowPane,
    ) -> Result<(), String> {
        let pane_id = transfer.model.moved_pane_id.clone();
        shell.borrow_mut().state = transfer.source_before;
        PaneRuntimeCoordinator::adopt_window_transfer(shell, &pane_id, transfer.runtime)
            .map_err(|(error, _)| error)?;
        let shell_ref = shell.borrow();
        shell_ref.render();
        shell_ref.focus_selected_surface();
        Ok(())
    }

    pub(crate) fn adopt_live_pane_window_transfer(
        shell: &Rc<RefCell<Self>>,
        pane_id: &str,
        runtime: DetachedPaneRuntime,
    ) -> Result<(), (String, DetachedPaneRuntime)> {
        PaneRuntimeCoordinator::adopt_window_transfer(shell, pane_id, runtime)?;
        let shell_ref = shell.borrow();
        shell_ref.render();
        shell_ref.focus_selected_surface();
        Ok(())
    }

    fn request_new_window(&self) {
        if let Some(handler) = self.new_window_handler.clone() {
            handler();
        }
    }

    fn request_show_task_manager(&self) {
        if let Some(handler) = self.show_task_manager_handler.clone() {
            handler();
        }
    }

    fn request_move_pane_to_new_window(&self) {
        let pane_count = self
            .state
            .worklanes()
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .map(|column| column.panes.len())
            .sum::<usize>();
        if pane_count < 2 {
            eprintln!("zentty-linux: action=move-pane-to-new-window available=false");
            return;
        }
        let Some(pane_id) = self.state.focused_pane_id() else {
            return;
        };
        if let Some(handler) = self.move_pane_to_new_window_handler.clone() {
            handler(pane_id.to_owned());
        }
    }

    fn request_quit(&self) {
        let handler = self.quit_handler.clone();
        let window = self.window.clone();
        let action: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(handler) = handler.clone() {
                handler();
            } else {
                window.close();
            }
        });
        if self.config.confirmations.confirm_before_quitting {
            Self::confirm_action(
                &self.window,
                "Quit Zentty?",
                "Terminal processes in all windows will be closed.",
                "Quit",
                action,
            );
        } else {
            action();
        }
    }

    fn request_close_window(&self) {
        let handler = self.close_window_handler.clone();
        let window = self.window.clone();
        let action: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(handler) = handler.clone() {
                handler();
            } else {
                window.close();
            }
        });
        if self.config.confirmations.confirm_before_closing_window
            && self.pane_runtime.live_children() > 0
        {
            Self::confirm_action(
                &self.window,
                "Close this window?",
                "Running terminal processes in this window will be closed.",
                "Close Window",
                action,
            );
        } else {
            action();
        }
    }

    fn confirm_action(
        parent: &gtk::Window,
        title: &str,
        detail: &str,
        accept_label: &str,
        action: Rc<dyn Fn()>,
    ) {
        let dialog = gtk::AlertDialog::builder()
            .modal(true)
            .message(title)
            .detail(detail)
            .buttons(["Cancel", accept_label])
            .cancel_button(0)
            .default_button(1)
            .build();
        dialog.choose(
            Some(parent),
            None::<&gtk::gio::Cancellable>,
            move |response| {
                let accepted = response == Ok(1);
                eprintln!("zentty-linux: confirmation accepted={accepted}");
                if accepted {
                    // AlertDialog completes its response callback before GTK has
                    // finished removing the modal. Run the accepted action on
                    // the next main-loop turn so any terminal focus it restores
                    // is not immediately taken back by the disappearing dialog.
                    glib::idle_add_local_once(move || action());
                }
            },
        );
        eprintln!("zentty-linux: confirmation shown title={title:?}");
    }

    fn mount_background_restored_agents(&self) {
        let active_panes = self.state.active_pane_ids();
        for pane_id in self.restored_pane_commands.keys() {
            if active_panes.iter().any(|active| active == pane_id) {
                continue;
            }
            if let Some(frame) = self.pane_runtime.frame(pane_id) {
                self.background_agent_host.append(frame.widget());
                eprintln!("zentty-linux: background-agent-host pane={pane_id}");
            }
        }
    }

    pub(crate) fn sidebar_container(&self) -> &gtk::ScrolledWindow {
        &self.sidebar_scroll
    }

    pub(crate) fn reconcile_sidebar_width(&self) {
        let available_width = self.body.width();
        if available_width <= 0 {
            return;
        }
        let target =
            SidebarWidthPreference::clamped(self.preferred_sidebar_width.get(), available_width);
        self.sidebar_scroll.set_width_request(target);
        self.sidebar_reservation.set_width_request(target);
        if self.sidebar_visibility.mode() == SidebarVisibilityMode::PinnedOpen
            && self.body.position() != target
        {
            self.adjusting_sidebar_width.set(true);
            self.body.set_position(target);
            self.adjusting_sidebar_width.set(false);
        }
        self.refresh_right_insertion_behavior();
    }

    pub(crate) fn reconcile_pane_heights(&self) {
        self.apply_pane_height_requests(false);
    }

    pub(crate) fn live_children(&self) -> usize {
        self.pane_runtime.live_children()
    }

    pub(crate) fn window_recipe(&self) -> WindowRecipe {
        let mut recipe = self.state.to_window_recipe(&self.window_template);
        recipe.frame = snapshot_window_frame(
            self.window.width(),
            self.window.height(),
            recipe.frame.as_ref(),
        );
        recipe
    }

    pub(crate) fn agent_restore_drafts(&self) -> Vec<PaneRestoreDraft> {
        self.state.agent_restore_drafts()
    }

    pub(crate) fn present(&self) {
        self.window.present();
        self.focus_selected_surface_unchecked();
    }

    pub(crate) fn detach_and_close(&mut self) {
        self.shutting_down = true;
        for upload in self.remote_panes.uploads.values() {
            upload.cancel();
        }
        if let Some(source) = self.remote_panes.probe_source.take() {
            source.remove();
        }
        if let Some(source) = self.server_runtime.probe_source.take() {
            source.remove();
        }
        // A deferred pane belongs to a cross-window transfer but is not yet
        // owned by this shell. Only revoke capabilities for live runtimes this
        // shell actually owns, or a failed adoption would strand the source
        // pane after rollback.
        let owned_live_pane_ids = self.pane_runtime.live_pane_ids();
        self.agent_events.shutdown(&owned_live_pane_ids);
        if let Some(router) = self.action_router.take() {
            router.uninstall(&self.window);
        }
        self.peek_phase = PeekPhase::Idle;
        self.peek_tab_down = false;
        self.peek_view.hide();
        self.command_palette.hide();
        gtk::prelude::GtkWindowExt::set_focus(&self.window, gtk::Widget::NONE);
        self.window.set_default_widget(gtk::Widget::NONE);
        clear_pane_columns(&self.pane_box);
        clear_box_children(&self.background_agent_host);
        self.pane_runtime.detach_widgets();
        // The shell retains `sidebar` after detaching the root widget. Clear
        // its cards explicitly so their menu popovers and window-capturing
        // callbacks are finalized before Ghostty's process-global teardown.
        sidebar::clear(&self.sidebar);
        self.window.set_child(gtk::Widget::NONE);
        self.window.close();
    }

    pub(crate) fn release_surfaces(&mut self) -> Result<(), String> {
        self.pane_runtime.release_all()
    }

    fn finish_pane_layout_action(&self, action: &str) {
        eprintln!(
            "zentty-linux: action={action} pane={}",
            self.state.focused_pane_id().unwrap_or("none")
        );
        self.render();
        self.focus_selected_surface();
        self.scroll_panes_to_focused();
    }

    fn install_sidebar_visibility(shell: &Rc<RefCell<Self>>) {
        let rail_motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(shell);
        rail_motion.connect_enter(move |_, _, _| {
            if let Some(shell) = weak.upgrade() {
                let Ok(mut shell) = shell.try_borrow_mut() else {
                    return;
                };
                shell
                    .sidebar_visibility
                    .handle(SidebarVisibilityEvent::HoverRailEntered);
                shell.apply_sidebar_visibility();
            }
        });
        let weak = Rc::downgrade(shell);
        rail_motion.connect_leave(move |_| {
            if let Some(shell) = weak.upgrade() {
                let changed = shell.try_borrow_mut().is_ok_and(|mut shell| {
                    shell
                        .sidebar_visibility
                        .handle(SidebarVisibilityEvent::HoverRailExited);
                    true
                });
                if changed {
                    Self::schedule_sidebar_dismissal(&shell);
                }
            }
        });
        shell
            .borrow()
            .sidebar_hover_rail
            .add_controller(rail_motion);

        let sidebar_motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(shell);
        sidebar_motion.connect_enter(move |_, _, _| {
            if let Some(shell) = weak.upgrade() {
                let Ok(mut shell) = shell.try_borrow_mut() else {
                    return;
                };
                shell.sidebar_visibility_generation =
                    shell.sidebar_visibility_generation.wrapping_add(1);
                shell
                    .sidebar_visibility
                    .handle(SidebarVisibilityEvent::SidebarEntered);
            }
        });
        let weak = Rc::downgrade(shell);
        sidebar_motion.connect_leave(move |_| {
            if let Some(shell) = weak.upgrade() {
                let changed = shell.try_borrow_mut().is_ok_and(|mut shell| {
                    shell
                        .sidebar_visibility
                        .handle(SidebarVisibilityEvent::SidebarExited);
                    true
                });
                if changed {
                    Self::schedule_sidebar_dismissal(&shell);
                }
            }
        });
        shell.borrow().sidebar_scroll.add_controller(sidebar_motion);
    }

    fn schedule_sidebar_dismissal(shell: &Rc<RefCell<Self>>) {
        let generation = {
            let Ok(mut shell) = shell.try_borrow_mut() else {
                return;
            };
            shell.sidebar_visibility_generation =
                shell.sidebar_visibility_generation.wrapping_add(1);
            shell.sidebar_visibility_generation
        };
        let weak = Rc::downgrade(shell);
        glib::timeout_add_local_once(Duration::from_millis(250), move || {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            let Ok(mut shell) = shell.try_borrow_mut() else {
                return;
            };
            if shell.sidebar_visibility_generation != generation {
                return;
            }
            if shell
                .sidebar_visibility
                .handle(SidebarVisibilityEvent::DismissTimerElapsed)
            {
                shell.apply_sidebar_visibility();
                shell.focus_selected_surface();
            }
        });
    }

    fn apply_sidebar_visibility(&mut self) {
        self.sidebar_visibility_generation = self.sidebar_visibility_generation.wrapping_add(1);
        let width = SidebarWidthPreference::clamped(
            self.preferred_sidebar_width.get(),
            self.window.width().max(1),
        );
        self.sidebar_scroll.set_width_request(width);
        match self.sidebar_visibility.mode() {
            SidebarVisibilityMode::PinnedOpen => {
                if self.body.start_child().is_none() {
                    self.body.set_start_child(Some(&self.sidebar_reservation));
                }
                self.sidebar_reservation.set_width_request(width);
                self.body.set_position(width);
                self.sidebar_scroll
                    .remove_css_class("zentty-sidebar-floating");
                self.sidebar_scroll.set_visible(true);
                self.sidebar_hover_rail.set_visible(false);
                eprintln!("zentty-linux: sidebar-visibility=pinned-open");
            }
            SidebarVisibilityMode::Hidden => {
                self.body.set_start_child(None::<&gtk::Widget>);
                self.sidebar_scroll.set_visible(false);
                self.sidebar_hover_rail.set_visible(true);
                eprintln!("zentty-linux: sidebar-visibility=hidden");
            }
            SidebarVisibilityMode::HoverPeek => {
                self.body.set_start_child(None::<&gtk::Widget>);
                self.sidebar_scroll.add_css_class("zentty-sidebar-floating");
                self.sidebar_scroll.set_visible(true);
                self.sidebar_hover_rail.set_visible(true);
                eprintln!("zentty-linux: sidebar-visibility=hover-peek");
            }
        }
    }

    fn install_pane_traversal_shortcuts(shell: &Rc<RefCell<Self>>) {
        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(shell);
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            let is_tab = key == gdk::Key::Tab || key == gdk::Key::ISO_Left_Tab;
            let Some(shell) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if is_close_window_shortcut(key, modifiers) {
                shell.borrow().request_quit();
                return glib::Propagation::Stop;
            }
            if is_tab && modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                let repeated = {
                    let mut shell = shell.borrow_mut();
                    let repeated = shell.peek_tab_down;
                    shell.peek_tab_down = true;
                    repeated
                };
                if repeated {
                    // GTK key-pressed signals include keyboard auto-repeat.
                    // A held physical Tab is one command, not a stream of taps.
                    return glib::Propagation::Stop;
                }
                let forward = key != gdk::Key::ISO_Left_Tab
                    && !modifiers.contains(gdk::ModifierType::SHIFT_MASK);
                let direction = if forward {
                    PeekDirection::Forward
                } else {
                    PeekDirection::Backward
                };
                Self::handle_peek_tab(&shell, direction);
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Escape && shell.borrow().peek_phase.is_active() {
                Self::cancel_peek(&shell);
                return glib::Propagation::Stop;
            }
            if modifiers.contains(gdk::ModifierType::CONTROL_MASK)
                && matches!(shell.borrow().peek_phase, PeekPhase::Peeking { .. })
            {
                let direction = match key {
                    gdk::Key::Left => Some(PeekSpatialDirection::Left),
                    gdk::Key::Right => Some(PeekSpatialDirection::Right),
                    gdk::Key::Up => Some(PeekSpatialDirection::Up),
                    gdk::Key::Down => Some(PeekSpatialDirection::Down),
                    _ => None,
                };
                if let Some(direction) = direction {
                    Self::spatially_navigate_peek(&shell, direction);
                    return glib::Propagation::Stop;
                }
            }
            if matches!(shell.borrow().peek_phase, PeekPhase::Peeking { .. }) {
                // A visible picker owns the interaction. No ordinary key
                // press may leak into the still-focused Ghostty surface.
                return glib::Propagation::Stop;
            }
            if let Some(gesture) = codex_terminal_gesture(key, modifiers) {
                Self::record_terminal_gesture(&shell, gesture);
            }
            glib::Propagation::Proceed
        });
        Self::install_pane_traversal_key_release(shell, &controller);
        shell.borrow().window.add_controller(controller);
    }

    fn install_pane_traversal_key_release(
        shell: &Rc<RefCell<Self>>,
        controller: &gtk::EventControllerKey,
    ) {
        let weak = Rc::downgrade(shell);
        controller.connect_key_released(move |_, key, _, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            if key == gdk::Key::Tab || key == gdk::Key::ISO_Left_Tab {
                shell.borrow_mut().peek_tab_down = false;
                Self::handle_peek_tab_released(&shell);
            } else if key == gdk::Key::Control_L || key == gdk::Key::Control_R {
                shell.borrow_mut().peek_tab_down = false;
                Self::commit_peek(&shell);
            }
        });
    }

    fn record_terminal_gesture(shell: &Rc<RefCell<Self>>, gesture: TerminalGesture) {
        let changed = {
            let mut shell = shell.borrow_mut();
            let Some(pane_id) = shell.state.focused_pane_id().map(str::to_owned) else {
                return;
            };
            let now = unix_time_ms();
            match gesture {
                TerminalGesture::InputSubmitted => {
                    shell.state.record_terminal_input_submitted(&pane_id, now)
                }
                TerminalGesture::Interrupted => {
                    shell.state.record_terminal_interrupt(&pane_id, now)
                }
            }
        };
        if changed {
            eprintln!("zentty-linux: codex-terminal-gesture={gesture:?}");
            shell.borrow().render_sidebar();
        }
    }

    fn handle_peek_tab(shell: &Rc<RefCell<Self>>, direction: PeekDirection) {
        let phase = shell.borrow().peek_phase.clone();
        match phase {
            PeekPhase::Idle => {
                let generation = {
                    let mut shell = shell.borrow_mut();
                    shell.peek_generation = shell.peek_generation.wrapping_add(1);
                    let generation = shell.peek_generation;
                    shell.peek_phase = PeekPhase::Armed {
                        generation,
                        pending: direction,
                    };
                    generation
                };
                eprintln!("zentty-linux: worklane-peek=armed generation={generation}");
                let weak = Rc::downgrade(shell);
                glib::timeout_add_local_once(WORKLANE_PEEK_TAB_HOLD_THRESHOLD, move || {
                    let Some(shell) = weak.upgrade() else {
                        return;
                    };
                    Self::open_peek_after_tab_hold(&shell, generation);
                });
            }
            PeekPhase::Armed { .. } => {}
            PeekPhase::Peeking {
                original,
                current,
                traversal,
            } => {
                let Some(next) = worklane_peek::step(&traversal, &current, direction) else {
                    return;
                };
                eprintln!(
                    "zentty-linux: worklane-peek=preview worklane={} pane={}",
                    next.worklane_id, next.pane_id
                );
                shell.borrow_mut().peek_phase = PeekPhase::Peeking {
                    original,
                    current: next,
                    traversal,
                };
                Self::refresh_peek_view(shell);
            }
        }
    }

    fn handle_peek_tab_released(shell: &Rc<RefCell<Self>>) {
        let phase = shell.borrow().peek_phase.clone();
        if let PeekPhase::Armed { pending, .. } = phase {
            shell.borrow_mut().peek_phase = PeekPhase::Idle;
            shell
                .borrow_mut()
                .select_adjacent_pane(pending == PeekDirection::Forward);
            Self::refocus_after_peek_key_release(shell);
            eprintln!("zentty-linux: worklane-peek=quick-tab-release");
        }
    }

    fn open_peek_after_tab_hold(shell: &Rc<RefCell<Self>>, generation: u64) {
        if !shell.borrow().peek_tab_down
            || !matches!(
                shell.borrow().peek_phase,
                PeekPhase::Armed {
                    generation: armed_generation,
                    ..
                } if armed_generation == generation
            )
        {
            return;
        }
        Self::open_peek(shell);
    }

    fn open_peek(shell: &Rc<RefCell<Self>>) {
        if matches!(shell.borrow().peek_phase, PeekPhase::Peeking { .. }) {
            return;
        }
        let traversal = shell.borrow().pane_references_in_sidebar_order();
        let Some(origin) = shell.borrow().current_pane_reference() else {
            Self::cancel_peek(shell);
            return;
        };
        shell.borrow_mut().peek_phase = PeekPhase::Peeking {
            original: origin.clone(),
            current: origin,
            traversal,
        };
        eprintln!("zentty-linux: worklane-peek=open trigger=tab-hold");
        Self::refresh_peek_view(shell);
    }

    fn commit_peek(shell: &Rc<RefCell<Self>>) {
        let phase = shell.borrow().peek_phase.clone();
        match phase {
            PeekPhase::Idle => {}
            PeekPhase::Armed { pending, .. } => {
                shell.borrow_mut().peek_phase = PeekPhase::Idle;
                shell
                    .borrow_mut()
                    .select_adjacent_pane(pending == PeekDirection::Forward);
                Self::refocus_after_peek_key_release(shell);
                eprintln!("zentty-linux: worklane-peek=quick-modifier-release");
            }
            PeekPhase::Peeking { current, .. } => {
                shell.borrow_mut().peek_phase = PeekPhase::Idle;
                shell.borrow().peek_view.hide();
                shell.borrow_mut().select_pane_reference(&current, true);
                eprintln!(
                    "zentty-linux: worklane-peek=commit worklane={} pane={}",
                    current.worklane_id, current.pane_id
                );
            }
        }
    }

    fn cancel_peek(shell: &Rc<RefCell<Self>>) {
        let phase = shell.borrow().peek_phase.clone();
        shell.borrow_mut().peek_phase = PeekPhase::Idle;
        shell.borrow().peek_view.hide();
        if let PeekPhase::Peeking { original, .. } = phase {
            shell.borrow_mut().select_pane_reference(&original, true);
        }
        eprintln!("zentty-linux: worklane-peek=cancel");
    }

    fn refocus_after_peek_key_release(shell: &Rc<RefCell<Self>>) {
        // GTK restores the widget that owned the key event after dispatching
        // the release. Let the horizontal viewport reveal the selected column,
        // then reassert the surface so Ctrl+Tab changes physical input focus.
        let weak = Rc::downgrade(shell);
        glib::timeout_add_local_once(Duration::from_millis(75), move || {
            if let Some(shell) = weak.upgrade() {
                let shell = shell.borrow();
                if !shell.command_palette.is_visible() && !shell.global_search.state().visible {
                    shell.focus_selected_surface_unchecked();
                }
            }
        });
    }

    fn spatially_navigate_peek(shell: &Rc<RefCell<Self>>, direction: PeekSpatialDirection) {
        let phase = shell.borrow().peek_phase.clone();
        let PeekPhase::Peeking {
            original,
            current,
            traversal,
        } = phase
        else {
            return;
        };
        let Some(target) =
            worklane_peek::spatial_target(shell.borrow().state.worklanes(), &current, direction)
        else {
            return;
        };
        shell.borrow_mut().peek_phase = PeekPhase::Peeking {
            original,
            current: target.clone(),
            traversal,
        };
        eprintln!(
            "zentty-linux: worklane-peek=spatial worklane={} pane={}",
            target.worklane_id, target.pane_id
        );
        Self::refresh_peek_view(shell);
    }

    fn preview_peek_selection(shell: &Rc<RefCell<Self>>, target: &PaneReference) {
        let phase = shell.borrow().peek_phase.clone();
        let PeekPhase::Peeking {
            original,
            traversal,
            ..
        } = phase
        else {
            return;
        };
        if !traversal.contains(target) {
            return;
        }
        shell.borrow_mut().peek_phase = PeekPhase::Peeking {
            original,
            current: target.clone(),
            traversal,
        };
        eprintln!(
            "zentty-linux: worklane-peek=click worklane={} pane={}",
            target.worklane_id, target.pane_id
        );
        Self::refresh_peek_view(shell);
    }

    fn refresh_peek_view(shell: &Rc<RefCell<Self>>) {
        let selected = shell.borrow().peek_phase.selected().cloned();
        let Some(selected) = selected else {
            return;
        };
        let previews = shell.borrow().peek_previews();
        let weak = Rc::downgrade(shell);
        shell
            .borrow()
            .peek_view
            .render(previews, &selected, move |target| {
                if let Some(shell) = weak.upgrade() {
                    Self::preview_peek_selection(&shell, &target);
                }
            });
    }

    fn install_pane_scroll_switching(shell: &Rc<RefCell<Self>>) {
        let controller = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::KINETIC,
        );
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let gesture = Rc::new(RefCell::new(PaneScrollSwitch::default()));
        let beginning = Rc::clone(&gesture);
        controller.connect_scroll_begin(move |_| beginning.borrow_mut().reset());
        let ending = Rc::clone(&gesture);
        controller.connect_scroll_end(move |_| ending.borrow_mut().reset());
        let weak = Rc::downgrade(shell);
        controller.connect_scroll(move |controller, dx, dy| {
            let unit = match controller.unit() {
                gdk::ScrollUnit::Wheel => ScrollUnit::Wheel,
                gdk::ScrollUnit::Surface => ScrollUnit::Surface,
                _ => return glib::Propagation::Proceed,
            };
            if unit == ScrollUnit::Wheel {
                gesture.borrow_mut().reset();
            }
            let shifted = controller
                .current_event_state()
                .contains(gdk::ModifierType::SHIFT_MASK);
            match gesture
                .borrow_mut()
                .handle(dx, dy, shifted, unit, glib::monotonic_time())
            {
                ScrollSwitchResult::Previous => {
                    if let Some(shell) = weak.upgrade() {
                        shell.borrow_mut().select_adjacent_pane(false);
                    }
                    glib::Propagation::Stop
                }
                ScrollSwitchResult::Next => {
                    if let Some(shell) = weak.upgrade() {
                        shell.borrow_mut().select_adjacent_pane(true);
                    }
                    glib::Propagation::Stop
                }
                ScrollSwitchResult::Consumed => glib::Propagation::Stop,
                ScrollSwitchResult::Unhandled => glib::Propagation::Proceed,
            }
        });
        shell.borrow().pane_scroll.add_controller(controller);
    }

    fn toggle_command_palette(&mut self) {
        if self.command_palette.is_visible() {
            self.command_palette.hide();
            self.focus_selected_surface();
            return;
        }
        let (items, current) = self.command_palette_items();
        self.command_palette
            .show(items, self.state.recent_pane_references(), current);
    }

    fn request_show_shortcut_settings(&mut self) {
        self.request_show_settings(crate::settings_navigation::SettingsSection::General);
    }

    #[allow(clippy::too_many_lines)] // Coordinates construction of the typed settings pages.
    fn request_show_settings(&mut self, section: crate::settings_navigation::SettingsSection) {
        if let Some(window) = self.shortcut_settings_window.as_ref() {
            let _ = window.activate_action(
                "settings.select-section",
                Some(&crate::settings_shell::section_target(section)),
            );
            window.present();
            return;
        }
        let weak = self.self_handle.borrow().clone();
        let appearance = self.config.appearance.clone();
        let appearance_weak = self.self_handle.borrow().clone();
        let general_weak = self.self_handle.borrow().clone();
        let notifications_weak = self.self_handle.borrow().clone();
        let updates_weak = self.self_handle.borrow().clone();
        let workspace_panes_weak = self.self_handle.borrow().clone();
        let open_with_weak = self.self_handle.borrow().clone();
        let dev_servers_weak = self.self_handle.borrow().clone();
        let agents_weak = self.self_handle.borrow().clone();
        let focus_weak = self.self_handle.borrow().clone();
        let restore_parent_focus: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(shell) = focus_weak.upgrade() {
                // The parent is presented in the same event that withdraws the
                // settings toplevel, before the window manager reports it active.
                // Restore GTK's intended child focus now; compositor activation
                // follows without leaving focus on a withdrawn settings entry.
                shell.borrow().focus_selected_surface_unchecked();
            }
        });
        let window = crate::shortcut_settings::show(
            &self.window,
            Rc::clone(&self.shortcut_manager),
            Rc::new(move |bindings| {
                let manager = match zentty_core::ShortcutManager::new(
                    &shortcut_registry::definitions(),
                    &bindings,
                ) {
                    Ok(manager) => manager,
                    Err(error) => {
                        eprintln!("zentty-linux: shortcuts rejected: {error}");
                        return Err(error);
                    }
                };
                crate::config_store::ConfigStore::update_default_shortcuts(&bindings)?;
                if let Some(shell) = weak.upgrade() {
                    let mut shell = shell.borrow_mut();
                    shell.config.shortcuts = bindings;
                    *shell.shortcut_manager.borrow_mut() = manager;
                }
                Ok(())
            }),
            crate::shortcut_settings::SettingsContext {
                appearance,
                apply_appearance: Rc::new(move |appearance| {
                    let shell = appearance_weak.upgrade().ok_or_else(|| {
                        "Zentty window closed while applying appearance".to_owned()
                    })?;
                    shell.borrow_mut().apply_appearance(appearance, false)
                }),
                general: crate::general_settings::GeneralSettings {
                    confirmations: self.config.confirmations,
                    restore: self.config.restore,
                    clipboard: self.config.clipboard,
                },
                apply_general: Rc::new(move |general| {
                    if let Some(shell) = general_weak.upgrade() {
                        shell.borrow_mut().apply_general(general);
                    }
                }),
                notifications: self.config.notifications.clone(),
                apply_notifications: Rc::new(move |notifications| {
                    if let Some(shell) = notifications_weak.upgrade() {
                        shell.borrow_mut().apply_notifications(notifications);
                    }
                }),
                updates: self.config.updates,
                error_reporting: self.config.error_reporting,
                apply_updates: Rc::new(move |updates| {
                    if let Some(shell) = updates_weak.upgrade() {
                        shell.borrow_mut().apply_updates(updates);
                    }
                }),
                worklanes: self.config.worklanes,
                pane_layout: self.config.pane_layout,
                panes: self.config.panes,
                apply_workspace_panes: Rc::new(move |worklanes, pane_layout, panes| {
                    if let Some(shell) = workspace_panes_weak.upgrade() {
                        shell
                            .borrow_mut()
                            .apply_workspace_panes(worklanes, pane_layout, panes);
                    }
                }),
                open_with: self.config.open_with.clone(),
                open_with_targets: open_with_runtime::discover_available_targets(
                    &self.config.open_with,
                    std::env::var_os("PATH").as_deref(),
                ),
                apply_open_with: Rc::new(move |config| {
                    let shell = open_with_weak.upgrade().ok_or_else(|| {
                        "Zentty window closed while applying Open With settings".to_owned()
                    })?;
                    shell.borrow_mut().apply_open_with(config)
                }),
                server_detection: self.config.server_detection.clone(),
                server_browser_targets: server_runtime::discover_browser_targets(
                    &self.config.server_detection,
                    std::env::var_os("PATH").as_deref(),
                ),
                apply_dev_servers: Rc::new(move |config| {
                    let shell = dev_servers_weak.upgrade().ok_or_else(|| {
                        "Zentty window closed while applying Dev Servers settings".to_owned()
                    })?;
                    shell.borrow_mut().apply_dev_servers(config)
                }),
                agent_teams: self.config.agent_teams,
                agent_caffeination: self.config.agent_caffeination,
                menu_bar: self.config.menu_bar,
                agent_integrations: self.config.agent_integrations.clone(),
                apply_agents: Rc::new(move |teams, caffeination, menu_bar, integrations| {
                    let shell = agents_weak.upgrade().ok_or_else(|| {
                        "Zentty window closed while applying Agents settings".to_owned()
                    })?;
                    shell
                        .borrow_mut()
                        .apply_agents(teams, caffeination, menu_bar, integrations)
                }),
                initial_section: section,
            },
            &restore_parent_focus,
        );
        self.shortcut_settings_window = Some(window);
    }

    fn apply_general(&mut self, general: crate::general_settings::GeneralSettings) {
        match crate::config_store::ConfigStore::update_default_general(
            general.confirmations,
            general.restore,
            general.clipboard,
        ) {
            Ok(path) => {
                self.config.confirmations = general.confirmations;
                self.config.restore = general.restore;
                self.config.clipboard = general.clipboard;
                self.render_sidebar();
                eprintln!(
                    "zentty-linux: general-settings result=persisted path={} restore={} clean-copy={}",
                    path.display(),
                    general.restore.restore_workspace_on_launch,
                    general.clipboard.always_clean_copies
                );
            }
            Err(error) => eprintln!("zentty-linux: general-settings result=error detail={error}"),
        }
    }

    fn apply_notifications(&mut self, notifications: zentty_core::NotificationsConfig) {
        match crate::config_store::ConfigStore::update_default_notifications(&notifications) {
            Ok(path) => {
                self.config.notifications = notifications;
                eprintln!(
                    "zentty-linux: notification-settings result=persisted path={} sound={:?}",
                    path.display(),
                    self.config.notifications.sound_name
                );
            }
            Err(error) => {
                eprintln!("zentty-linux: notification-settings result=error detail={error}");
            }
        }
    }

    fn apply_updates(&mut self, updates: zentty_core::UpdatesConfig) {
        match crate::config_store::ConfigStore::update_default_updates(updates) {
            Ok(path) => {
                self.config.updates = updates;
                eprintln!(
                    "zentty-linux: updates-privacy-settings result=persisted path={} channel={}",
                    path.display(),
                    updates.channel.config_value()
                );
            }
            Err(error) => {
                eprintln!("zentty-linux: updates-privacy-settings result=error detail={error}");
            }
        }
    }

    fn apply_workspace_panes(
        &mut self,
        worklanes: zentty_core::WorklaneConfig,
        pane_layout: zentty_core::PaneLayoutConfig,
        panes: zentty_core::PaneConfig,
    ) {
        match crate::config_store::ConfigStore::update_default_workspace_panes(
            worklanes,
            pane_layout,
            panes,
        ) {
            Ok(path) => {
                self.config.worklanes = worklanes;
                self.config.pane_layout = pane_layout;
                self.config.panes = panes;
                self.render_sidebar();
                self.refresh_pane_presentation();
                eprintln!(
                    "zentty-linux: workspace-pane-settings result=persisted path={} placement={} right-behavior={}",
                    path.display(),
                    worklanes.new_worklane_placement.config_value(),
                    pane_layout.right_split_behavior.config_value(),
                );
            }
            Err(error) => {
                eprintln!("zentty-linux: workspace-pane-settings result=error detail={error}");
            }
        }
    }

    fn apply_open_with(&mut self, config: zentty_core::OpenWithConfig) -> Result<(), String> {
        let path = crate::config_store::ConfigStore::update_default_open_with(&config)?;
        self.config.open_with = config;
        self.open_with_runtime =
            open_with_runtime::OpenWithRuntime::discover(&self.config.open_with);
        self.chrome
            .configure_open_with(&self.open_with_runtime.catalog);
        self.chrome
            .set_open_with_context_available(open_with_runtime::focused_context_is_available(self));
        eprintln!(
            "zentty-linux: open-with-settings result=persisted path={} primary={}",
            path.display(),
            self.config.open_with.primary_target_id,
        );
        Ok(())
    }

    fn apply_dev_servers(
        &mut self,
        config: zentty_core::ServerDetectionConfig,
    ) -> Result<(), String> {
        let path = crate::config_store::ConfigStore::update_default_server_detection(&config)?;
        let was_enabled = self.config.server_detection.passive_detection_enabled;
        self.config.server_detection = config;
        self.server_runtime.browser_catalog = zentty_core::ServerBrowserCatalog::resolve(
            &self.config.server_detection,
            server_runtime::discover_browser_targets(
                &self.config.server_detection,
                std::env::var_os("PATH").as_deref(),
            ),
        );
        if was_enabled && !self.config.server_detection.passive_detection_enabled {
            if let Some(source) = self.server_runtime.probe_source.take() {
                source.remove();
            }
        } else if !was_enabled && self.config.server_detection.passive_detection_enabled {
            let weak = self.self_handle.borrow().clone();
            glib::idle_add_local_once(move || {
                if let Some(shell) = weak.upgrade() {
                    let source = server_runtime::install(&shell);
                    shell.borrow_mut().server_runtime.probe_source = source;
                }
            });
        }
        eprintln!(
            "zentty-linux: dev-server-settings result=persisted path={} passive={} preferred={}",
            path.display(),
            self.config.server_detection.passive_detection_enabled,
            self.config.server_detection.preferred_browser_id,
        );
        Ok(())
    }

    fn apply_agents(
        &mut self,
        teams: zentty_core::AgentTeamsConfig,
        caffeination: zentty_core::AgentCaffeinationConfig,
        menu_bar: zentty_core::MenuBarConfig,
        integrations: zentty_core::AgentIntegrationsConfig,
    ) -> Result<(), String> {
        let path = crate::config_store::ConfigStore::update_default_agents(
            teams,
            caffeination,
            menu_bar,
            &integrations,
        )?;
        self.agent_events.set_agent_teams_enabled(teams.enabled);
        self.agent_events
            .set_agent_integrations(integrations.states.clone());
        self.config.agent_teams = teams;
        self.config.agent_caffeination = caffeination;
        self.config.menu_bar = menu_bar;
        self.config.agent_integrations = integrations;
        eprintln!(
            "zentty-linux: agent-settings result=persisted path={} teams={} new-panes-only=true",
            path.display(),
            teams.enabled
        );
        Ok(())
    }

    fn reload_ghostty_config(&mut self) {
        self.reload_ghostty_config_with_focus(true);
    }

    fn reload_ghostty_config_with_focus(&mut self, restore_terminal_focus: bool) {
        self.report_ghostty_reload_metrics("before");
        match self.pane_runtime.runtime().reload_config() {
            Ok(()) => {
                if restore_terminal_focus {
                    self.schedule_layout_render();
                    self.focus_selected_surface();
                }
                eprintln!("zentty-linux: action=reload-config result=applied");
                let weak = self.self_handle.borrow().clone();
                glib::timeout_add_local_once(Duration::from_millis(250), move || {
                    if let Some(shell) = weak.upgrade()
                        && let Ok(shell) = shell.try_borrow_mut()
                    {
                        shell.report_ghostty_reload_metrics("after");
                        if restore_terminal_focus {
                            shell.focus_selected_surface();
                        }
                    }
                });
            }
            Err(error) => {
                eprintln!("zentty-linux: action=reload-config result=failed detail={error}");
            }
        }
    }

    fn apply_theme_mode_command(&mut self, command: zentty_core::ThemeModeCommand) {
        let current = match crate::config_store::ConfigStore::load_default() {
            Ok(snapshot) => snapshot.config.appearance,
            Err(error) => {
                eprintln!("zentty-linux: action=theme-mode result=failed detail={error}");
                return;
            }
        };
        let desktop_is_dark = gtk::Settings::default()
            .is_some_and(|settings| settings.is_gtk_application_prefer_dark_theme());
        let mut appearance = current;
        appearance.theme_mode = command.resolve(appearance.theme_mode, desktop_is_dark);
        if let Err(error) = self.apply_appearance(appearance, true) {
            eprintln!("zentty-linux: action=theme-mode result=failed detail={error}");
            return;
        }
        eprintln!(
            "zentty-linux: action=theme-mode result=persisted mode={}",
            self.config.appearance.theme_mode.config_value()
        );
    }

    fn apply_appearance(
        &mut self,
        appearance: zentty_core::AppearanceConfig,
        restore_terminal_focus: bool,
    ) -> Result<(), String> {
        let spec = appearance.theme_spec();
        crate::config_store::ConfigStore::install_default_fallback_theme_if_referenced(&spec)?;
        crate::config_store::ConfigStore::update_default_ghostty_theme(&spec)?;
        if let Some(opacity) = appearance.background_opacity {
            crate::config_store::ConfigStore::update_default_ghostty_value(
                "background-opacity",
                &opacity.to_string(),
            )?;
        }
        crate::config_store::ConfigStore::update_default_appearance(&appearance)?;
        self.config.appearance = appearance;
        self.reload_ghostty_config_with_focus(restore_terminal_focus);
        Ok(())
    }

    fn report_ghostty_reload_metrics(&self, stage: &str) {
        let mut pane_ids = self.pane_runtime.live_pane_ids();
        pane_ids.sort();
        for pane_id in pane_ids {
            let Some(surface) = self.pane_runtime.surface(&pane_id) else {
                continue;
            };
            let Ok(cell) = surface.cell_size() else {
                continue;
            };
            eprintln!(
                "zentty-linux: reload-config-metric stage={stage} pane={pane_id} cell={:.3}x{:.3} pid={}",
                cell.width,
                cell.height,
                surface.foreground_process_id().unwrap_or(0)
            );
        }
    }

    fn request_rename_current_worklane(&self) {
        let worklane = self.state.active_worklane();
        crate::sidebar::present_rename_dialog(
            &self.window,
            "Rename Worklane",
            "workspace.rename-worklane",
            &worklane.id,
            worklane.title.as_deref().unwrap_or_default(),
        );
    }

    fn request_rename_current_pane(&self) {
        let Some(pane) = self
            .state
            .focused_pane_id()
            .and_then(|pane_id| self.state.pane(pane_id))
        else {
            return;
        };
        crate::sidebar::present_rename_dialog(
            &self.window,
            "Rename Pane",
            "workspace.rename-pane",
            &pane.id,
            pane.custom_title.as_deref().unwrap_or_default(),
        );
    }

    fn copy_focused_pane_path(&self) {
        let Some((pane_id, path)) = self
            .state
            .focused_pane_id()
            .and_then(|pane_id| self.state.pane(pane_id).map(|pane| (pane_id, pane)))
            .and_then(|(pane_id, pane)| {
                pane.working_directory
                    .as_deref()
                    .map(|path| (pane_id, path))
            })
        else {
            eprintln!("zentty-linux: action=copy-pane-path unavailable=no-working-directory");
            return;
        };
        gtk::prelude::WidgetExt::display(&self.window)
            .clipboard()
            .set_text(path);
        eprintln!("zentty-linux: action=copy-pane-path pane={pane_id}");
    }

    fn install_search_shortcuts(shell: &Rc<RefCell<Self>>) {
        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(shell);
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(shell) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if shell.borrow().command_palette.is_visible() || shell.borrow().peek_phase.is_active()
            {
                return glib::Propagation::Proceed;
            }
            if shell.borrow().global_search.state().visible
                && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter)
            {
                let direction = if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
                    GlobalSearchDirection::Previous
                } else {
                    GlobalSearchDirection::Next
                };
                shell.borrow_mut().navigate_global_find(direction);
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Escape {
                if shell.borrow().global_search.state().visible {
                    let mut shell = shell.borrow_mut();
                    let effects = shell.global_search.end();
                    shell.apply_global_search_effects(effects);
                    shell.render_global_search();
                    shell.focus_selected_surface();
                    return glib::Propagation::Stop;
                }
                let hidden = {
                    let shell = shell.borrow();
                    let Some(pane_id) = shell.state.focused_pane_id() else {
                        return glib::Propagation::Proceed;
                    };
                    let Some(surface) = shell.pane_runtime.surface(pane_id) else {
                        return glib::Propagation::Proceed;
                    };
                    let Some(overlay) = find_ghostty_search_overlay(surface.widget()) else {
                        return glib::Propagation::Proceed;
                    };
                    let Some(entry) = find_search_entry(&overlay) else {
                        return glib::Propagation::Proceed;
                    };
                    if !overlay.property::<bool>("active") || entry.text().is_empty() {
                        false
                    } else {
                        overlay.set_property("active", false);
                        eprintln!("zentty-linux: action=hide-search pane={pane_id}");
                        true
                    }
                };
                if hidden {
                    shell.borrow().focus_selected_surface();
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        shell.borrow().window.add_controller(controller);
    }

    fn command_palette_items(&mut self) -> (Vec<CommandPaletteItem>, Option<PaneReference>) {
        let current = self
            .state
            .focused_pane_id()
            .map(|pane_id| PaneReference::new(self.state.active_worklane_id(), pane_id));
        let mut items = self
            .state
            .worklanes()
            .iter()
            .enumerate()
            .flat_map(|(index, worklane)| {
                let lane_title = worklane
                    .title
                    .clone()
                    .unwrap_or_else(|| format!("Worklane {}", index + 1));
                worklane.columns.iter().flat_map(move |column| {
                    let lane_title = lane_title.clone();
                    column.panes.iter().map(move |pane| {
                        CommandPaletteItem::pane(
                            pane.custom_title
                                .clone()
                                .unwrap_or_else(|| pane.live_title.clone()),
                            lane_title.clone(),
                            PaneReference::new(&worklane.id, &pane.id),
                        )
                    })
                })
            })
            .collect::<Vec<_>>();
        items.extend(self.command_palette_server_items());
        items.extend(self.command_palette_task_items());
        items.extend(self.command_palette_open_with_items());
        items.extend(Self::command_palette_action_items());
        (items, current)
    }

    fn command_palette_open_with_items(&self) -> Vec<CommandPaletteItem> {
        let context_available = open_with_runtime::focused_context_is_available(self);
        let mut items = Vec::new();
        if let Some(primary) = &self.open_with_runtime.catalog.primary {
            let mut item = CommandPaletteItem::action(
                format!("Open focused pane in {}", primary.name),
                "Open With · Primary application",
                "editor file manager terminal directory project",
                ACTION_OPEN_WITH_PRIMARY,
            );
            item.enabled = context_available;
            items.push(item);
        }
        items.extend(self.open_with_runtime.catalog.enabled.iter().map(|target| {
            CommandPaletteItem::parameterized_action_with_enabled(
                format!("Open With: {}", target.name),
                "Open the focused pane directory",
                "editor file manager terminal directory project",
                ACTION_OPEN_WITH_TARGET,
                target.id.clone(),
                context_available,
            )
        }));
        items
    }

    fn command_palette_task_items(&mut self) -> Vec<CommandPaletteItem> {
        self.task_runner_actions.clear();
        let Some(focused_directory) = self
            .state
            .focused_pane_id()
            .and_then(|pane_id| self.state.pane(pane_id))
            .and_then(|pane| pane.working_directory.as_deref())
        else {
            return Vec::new();
        };
        let actions = match discover_task_runners(std::path::Path::new(focused_directory)) {
            Ok(actions) => actions,
            Err(error) => {
                eprintln!("zentty-linux: task-discovery error={error}");
                return Vec::new();
            }
        };
        actions
            .into_iter()
            .map(|action| {
                let id = action.id.clone();
                let keywords = format!(
                    "run task task runner {} {}",
                    action.execution_command,
                    action
                        .disabled_reason
                        .as_ref()
                        .map_or("", zentty_core::TaskRunnerDisabledReason::display_text)
                );
                let item = CommandPaletteItem::parameterized_action(
                    format!("Run task: {}", action.title),
                    action.subtitle(),
                    &keywords,
                    action_router::ACTION_RUN_TASK,
                    id.clone(),
                );
                self.task_runner_actions.insert(id, action);
                item
            })
            .collect()
    }

    fn command_palette_server_items(&mut self) -> Vec<CommandPaletteItem> {
        self.server_runtime.browser_actions.clear();
        let browser_targets = self.server_runtime.browser_catalog.enabled.clone();
        let mut next_browser_action = 0_u64;
        self.ranked_servers()
            .into_iter()
            .flat_map(|server| {
                let lane = self
                    .state
                    .worklanes()
                    .iter()
                    .find(|worklane| worklane.id == server.server.worklane_id)
                    .and_then(|worklane| worklane.title.as_deref())
                    .unwrap_or("Worklane");
                if server.tier == zentty_core::ServerRelevanceTier::Hidden {
                    return server.server.ports.first().map_or_else(Vec::new, |port| {
                        vec![CommandPaletteItem::parameterized_action(
                            format!("Stop Ignoring Port {port}"),
                            format!("Development server · {lane}"),
                            "unignore show server port",
                            ACTION_STOP_IGNORING_SERVER_PORT,
                            server.server.origin,
                        )]
                    });
                }
                let mut actions = vec![CommandPaletteItem::parameterized_action(
                    format!("Open {}", server.server.display),
                    format!("Development server · {lane}"),
                    "browser localhost server URL",
                    ACTION_OPEN_SERVER,
                    server.server.origin.clone(),
                )];
                for target in &browser_targets {
                    let action_id = format!("server-browser-{next_browser_action}");
                    next_browser_action += 1;
                    self.server_runtime.browser_actions.insert(
                        action_id.clone(),
                        (server.server.origin.clone(), target.id.clone()),
                    );
                    actions.push(CommandPaletteItem::parameterized_action(
                        format!("Open {} in {}", server.server.display, target.name),
                        format!("Development server browser · {lane}"),
                        "browser target localhost server URL",
                        ACTION_OPEN_SERVER_BROWSER,
                        action_id,
                    ));
                }
                if server.server.source != zentty_core::DetectedServerSource::Manual
                    && let Some(port) = server.server.ports.first()
                {
                    actions.push(CommandPaletteItem::parameterized_action(
                        format!("Ignore Port {port}"),
                        format!("Hide this development server · {lane}"),
                        "server noisy port suppress",
                        ACTION_IGNORE_SERVER_PORT,
                        server.server.origin.clone(),
                    ));
                }
                if server.server.source == zentty_core::DetectedServerSource::Scanner
                    && server.server.confidence == zentty_core::DetectedServerConfidence::Pid
                {
                    actions.push(CommandPaletteItem::parameterized_action(
                        format!("Stop {}", server.server.display),
                        format!("Owned development server · {lane}"),
                        "terminate interrupt server process",
                        ACTION_STOP_SERVER,
                        server.server.origin,
                    ));
                }
                actions
            })
            .collect()
    }

    #[allow(clippy::too_many_lines)] // Interim until the source command registry is ported.
    fn command_palette_action_items() -> Vec<CommandPaletteItem> {
        let mut items = crate::settings_navigation::SettingsSection::ALL
            .into_iter()
            .map(|section| {
                CommandPaletteItem::parameterized_action(
                    format!("{} Settings", section.title()),
                    format!("Jump to {}", section.subtitle()),
                    section.search_keywords(),
                    ACTION_OPEN_SETTINGS_SECTION,
                    section.id(),
                )
            })
            .collect::<Vec<_>>();
        items.extend([
            CommandPaletteItem::action(
                "Settings",
                "Open Zentty settings",
                "preferences configuration general",
                ACTION_OPEN_SETTINGS,
            ),
            CommandPaletteItem::action(
                "Toggle Light/Dark Theme",
                "Switch between the remembered light and dark themes",
                "appearance colors automatic",
                ACTION_TOGGLE_LIGHT_DARK_THEME,
            ),
            CommandPaletteItem::action(
                "Use Dark Theme",
                "Use the remembered dark terminal theme",
                "appearance colors",
                ACTION_USE_DARK_THEME,
            ),
            CommandPaletteItem::action(
                "Use Light Theme",
                "Use the remembered light terminal theme",
                "appearance colors",
                ACTION_USE_LIGHT_THEME,
            ),
            CommandPaletteItem::action(
                "Use Auto Theme",
                "Follow the Linux desktop light or dark appearance",
                "appearance colors automatic system",
                ACTION_USE_AUTO_THEME,
            ),
            CommandPaletteItem::action(
                "Task Manager",
                "Inspect CPU, memory, and process trees for every pane",
                "diagnostics processes performance",
                ACTION_SHOW_TASK_MANAGER,
            ),
            CommandPaletteItem::action(
                "Refresh Development Servers",
                "Rescan real listening processes now",
                "server browser port listener",
                ACTION_REFRESH_SERVERS,
            ),
            CommandPaletteItem::action(
                "Refresh Git and Review Status",
                "Refresh repository, branch, dirty tree, and pull-request state",
                "git github pull request ci approval conflict",
                ACTION_REFRESH_REVIEW_STATUS,
            ),
            CommandPaletteItem::action(
                "Open Branch on Remote",
                "Open the focused branch on its configured Git remote",
                "git github gitlab bitbucket browser",
                ACTION_OPEN_BRANCH_REMOTE,
            ),
            CommandPaletteItem::action(
                "Open Pull Request",
                "Open the pull request associated with the focused branch",
                "git github review browser pr",
                ACTION_OPEN_PULL_REQUEST,
            ),
            CommandPaletteItem::action(
                "New Window",
                "Create another Zentty window",
                "application window",
                ACTION_NEW_WINDOW,
            ),
            CommandPaletteItem::action(
                "Close Window",
                "Close this Zentty window",
                "application window",
                ACTION_CLOSE_WINDOW,
            ),
            CommandPaletteItem::action(
                "New Worklane",
                "Create another worklane",
                "workspace lane",
                ACTION_NEW_WORKLANE,
            ),
            CommandPaletteItem::action(
                "Split Right",
                "Split the focused pane into a visible right column",
                "pane column",
                ACTION_SPLIT_PANE_RIGHT,
            ),
            CommandPaletteItem::action(
                "Add Pane Right",
                "Add a full-width pane to the right of the focused column",
                "pane column canvas",
                ACTION_NEW_PANE_RIGHT,
            ),
            CommandPaletteItem::action(
                "Add Pane Left",
                "Add a full-width pane to the left of the focused column",
                "pane column canvas",
                ACTION_ADD_PANE_LEFT,
            ),
            CommandPaletteItem::action(
                "New Pane Below",
                "Split the focused pane vertically",
                "pane split down",
                ACTION_SPLIT_PANE_BELOW,
            ),
            CommandPaletteItem::action(
                "Toggle Sidebar",
                "Show or hide the worklane sidebar",
                "navigation",
                ACTION_TOGGLE_SIDEBAR,
            ),
            CommandPaletteItem::action(
                source_ui::CLOSE_WORKLANE,
                "Close the active worklane and all of its panes",
                "workspace lane remove",
                ACTION_CLOSE_ACTIVE_WORKLANE,
            ),
            CommandPaletteItem::action(
                "Close Pane",
                "Close the focused pane",
                "terminal",
                ACTION_CLOSE_PANE,
            ),
            CommandPaletteItem::action(
                source_ui::UNDO_CLOSE_PANE,
                "Reopen the most recently closed pane",
                "terminal restore reopen",
                ACTION_RESTORE_CLOSED_PANE,
            ),
            CommandPaletteItem::action(
                "Navigate Back",
                "Return to the previously focused pane",
                "history browser previous",
                ACTION_NAVIGATE_BACK,
            ),
            CommandPaletteItem::action(
                "Navigate Forward",
                "Move forward through pane focus history",
                "history browser next",
                ACTION_NAVIGATE_FORWARD,
            ),
            CommandPaletteItem::action(
                "Focus Next Pane",
                "Focus the next pane in sidebar order",
                "navigation terminal",
                ACTION_NEXT_PANE,
            ),
            CommandPaletteItem::action(
                "Focus Previous Pane",
                "Focus the previous pane in sidebar order",
                "navigation terminal",
                ACTION_PREVIOUS_PANE,
            ),
            CommandPaletteItem::action(
                "Next Worklane",
                "Focus the next worklane",
                "navigation workspace lane",
                ACTION_NEXT_WORKLANE,
            ),
            CommandPaletteItem::action(
                "Previous Worklane",
                "Focus the previous worklane",
                "navigation workspace lane",
                ACTION_PREVIOUS_WORKLANE,
            ),
            CommandPaletteItem::action(
                "Move Worklane Up",
                "Move the active worklane earlier in the sidebar",
                "reorder workspace lane",
                ACTION_MOVE_WORKLANE_UP,
            ),
            CommandPaletteItem::action(
                "Move Worklane Down",
                "Move the active worklane later in the sidebar",
                "reorder workspace lane",
                ACTION_MOVE_WORKLANE_DOWN,
            ),
            CommandPaletteItem::action(
                "Move Pane Left",
                "Move the focused pane one column left",
                "reorder terminal column",
                ACTION_MOVE_PANE_LEFT,
            ),
            CommandPaletteItem::action(
                "Move Pane Right",
                "Move the focused pane one column right",
                "reorder terminal column",
                ACTION_MOVE_PANE_RIGHT,
            ),
            CommandPaletteItem::action(
                "Move Pane Up",
                "Move the focused pane upward in its column",
                "reorder terminal split",
                ACTION_MOVE_PANE_UP,
            ),
            CommandPaletteItem::action(
                "Move Pane Down",
                "Move the focused pane downward in its column",
                "reorder terminal split",
                ACTION_MOVE_PANE_DOWN,
            ),
            CommandPaletteItem::action(
                source_ui::MOVE_PANE_TO_NEW_WINDOW,
                "Move the focused live terminal into a new Zentty window",
                "pane terminal window detach",
                ACTION_MOVE_PANE_TO_NEW_WINDOW,
            ),
            CommandPaletteItem::action(
                "Focus Left Pane",
                "Focus the neighboring column to the left",
                "navigation terminal column",
                ACTION_FOCUS_PANE_LEFT,
            ),
            CommandPaletteItem::action(
                "Focus Right Pane",
                "Focus the neighboring column to the right",
                "navigation terminal column",
                ACTION_FOCUS_PANE_RIGHT,
            ),
            CommandPaletteItem::action(
                "Focus Up In Column",
                "Focus the pane above in the current column",
                "navigation terminal split",
                ACTION_FOCUS_PANE_UP,
            ),
            CommandPaletteItem::action(
                "Focus Down In Column",
                "Focus the pane below in the current column",
                "navigation terminal split",
                ACTION_FOCUS_PANE_DOWN,
            ),
            CommandPaletteItem::action(
                source_ui::RESIZE_PANE_LEFT,
                "Move the focused pane's horizontal edge left by one terminal cell",
                "layout pane resize keyboard",
                ACTION_RESIZE_PANE_LEFT,
            ),
            CommandPaletteItem::action(
                source_ui::RESIZE_PANE_RIGHT,
                "Move the focused pane's horizontal edge right by one terminal cell",
                "layout pane resize keyboard",
                ACTION_RESIZE_PANE_RIGHT,
            ),
            CommandPaletteItem::action(
                source_ui::RESIZE_PANE_UP,
                "Move the preferred focused-pane divider up by one terminal cell",
                "layout pane resize keyboard",
                ACTION_RESIZE_PANE_UP,
            ),
            CommandPaletteItem::action(
                source_ui::RESIZE_PANE_DOWN,
                "Move the preferred focused-pane divider down by one terminal cell",
                "layout pane resize keyboard",
                ACTION_RESIZE_PANE_DOWN,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Full Width",
                "Make every column one viewport wide",
                "layout pane columns",
                ACTION_ARRANGE_WIDTH_FULL,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Half Width",
                "Fit two equal columns in the viewport",
                "layout pane columns",
                ACTION_ARRANGE_WIDTH_HALF,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Thirds",
                "Fit three equal columns in the viewport",
                "layout pane columns",
                ACTION_ARRANGE_WIDTH_THIRDS,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Quarters",
                "Fit four equal columns in the viewport",
                "layout pane columns",
                ACTION_ARRANGE_WIDTH_QUARTERS,
            ),
            CommandPaletteItem::action(
                "Arrange Height: Full Height",
                "Place one pane in each column",
                "layout pane rows",
                ACTION_ARRANGE_HEIGHT_FULL,
            ),
            CommandPaletteItem::action(
                "Arrange Height: 2 Per Column",
                "Reflow panes two per column",
                "layout pane rows",
                ACTION_ARRANGE_HEIGHT_TWO,
            ),
            CommandPaletteItem::action(
                "Arrange Height: 3 Per Column",
                "Reflow panes three per column",
                "layout pane rows",
                ACTION_ARRANGE_HEIGHT_THREE,
            ),
            CommandPaletteItem::action(
                "Arrange Height: 4 Per Column",
                "Reflow panes four per column",
                "layout pane rows",
                ACTION_ARRANGE_HEIGHT_FOUR,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Golden — Focus Wide",
                "Give the focused column the larger golden share",
                "layout pane golden ratio",
                ACTION_ARRANGE_GOLDEN_WIDE,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Golden — Focus Narrow",
                "Give the focused column the smaller golden share",
                "layout pane golden ratio",
                ACTION_ARRANGE_GOLDEN_NARROW,
            ),
            CommandPaletteItem::action(
                "Arrange Height: Golden — Focus Tall",
                "Give the focused pane the larger golden share",
                "layout pane golden ratio",
                ACTION_ARRANGE_GOLDEN_TALL,
            ),
            CommandPaletteItem::action(
                "Arrange Height: Golden — Focus Short",
                "Give the focused pane the smaller golden share",
                "layout pane golden ratio",
                ACTION_ARRANGE_GOLDEN_SHORT,
            ),
            CommandPaletteItem::action(
                "Reset Pane Layout",
                "Restore default column widths and equal pane heights",
                "layout pane reset",
                ACTION_RESET_PANE_LAYOUT,
            ),
            CommandPaletteItem::action(
                "Cycle Worklane Color",
                "Choose the next worklane identity color",
                "appearance workspace lane",
                ACTION_CYCLE_WORKLANE_COLOR,
            ),
            CommandPaletteItem::action(
                "Global Find",
                "Search across every live pane in this window",
                "search all panes worklanes",
                ACTION_GLOBAL_FIND,
            ),
            CommandPaletteItem::action(
                "Find",
                "Search the focused terminal's real scrollback",
                "search pane terminal",
                ACTION_FIND,
            ),
            CommandPaletteItem::action(
                "Use Selection for Find",
                "Search for the focused terminal selection",
                "search pane selection terminal",
                ACTION_USE_SELECTION_FOR_FIND,
            ),
            CommandPaletteItem::action(
                "Find Next",
                "Select the next terminal search match",
                "search pane navigation",
                ACTION_FIND_NEXT,
            ),
            CommandPaletteItem::action(
                "Find Previous",
                "Select the previous terminal search match",
                "search pane navigation",
                ACTION_FIND_PREVIOUS,
            ),
            CommandPaletteItem::action(
                source_ui::COPY,
                "Copy the focused terminal selection",
                "clipboard selection default",
                ACTION_COPY,
            ),
            CommandPaletteItem::action(
                "Clean Copy",
                "Copy the selection after conservative terminal-text cleanup",
                "clipboard selection format ansi prompt url path",
                ACTION_CLEAN_COPY,
            ),
            CommandPaletteItem::action(
                "Copy Raw",
                "Copy the selection without Zentty transformations",
                "clipboard selection original escape hatch",
                ACTION_COPY_RAW,
            ),
            CommandPaletteItem::action(
                "Copy as Markdown",
                "Reflow a Markdown selection while preserving its structure",
                "clipboard selection markdown format",
                ACTION_COPY_AS_MARKDOWN,
            ),
            CommandPaletteItem::action(
                source_ui::SELECT_ALL,
                "Select all text in the focused terminal",
                "terminal selection clipboard",
                ACTION_SELECT_ALL,
            ),
        ]);
        items
    }

    fn perform_focused_binding_action(&self, action: &str, binding: &str) {
        let Some(pane_id) = self.state.focused_pane_id() else {
            eprintln!("zentty-linux: action={action} error=no-focused-pane");
            return;
        };
        let Some(surface) = self.pane_runtime.surface(pane_id) else {
            eprintln!("zentty-linux: action={action} pane={pane_id} error=no-live-surface");
            return;
        };
        match surface.perform_binding_action(binding) {
            Ok(()) => eprintln!("zentty-linux: action={action} pane={pane_id} binding={binding:?}"),
            Err(error) => {
                eprintln!("zentty-linux: action={action} pane={pane_id} error={error}");
            }
        }
    }

    fn install_peek_scroll_navigation(shell: &Rc<RefCell<Self>>) {
        let controller = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::KINETIC,
        );
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let gesture = Rc::new(RefCell::new(PeekScrollNavigation::default()));
        let beginning = Rc::clone(&gesture);
        controller.connect_scroll_begin(move |_| beginning.borrow_mut().reset());
        let ending = Rc::clone(&gesture);
        controller.connect_scroll_end(move |_| ending.borrow_mut().reset());
        let weak = Rc::downgrade(shell);
        controller.connect_scroll(move |controller, dx, dy| {
            let Some(shell) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !matches!(shell.borrow().peek_phase, PeekPhase::Peeking { .. }) {
                gesture.borrow_mut().reset();
                return glib::Propagation::Proceed;
            }
            let unit = match controller.unit() {
                gdk::ScrollUnit::Wheel => PeekScrollUnit::Wheel,
                gdk::ScrollUnit::Surface => PeekScrollUnit::Surface,
                _ => return glib::Propagation::Stop,
            };
            if unit == PeekScrollUnit::Wheel {
                gesture.borrow_mut().reset();
            }
            match gesture.borrow_mut().handle(dx, dy, unit) {
                PeekScrollResult::Navigate(direction) => {
                    let direction = match direction {
                        PeekScrollDirection::Left => PeekSpatialDirection::Left,
                        PeekScrollDirection::Right => PeekSpatialDirection::Right,
                        PeekScrollDirection::Up => PeekSpatialDirection::Up,
                        PeekScrollDirection::Down => PeekSpatialDirection::Down,
                    };
                    Self::spatially_navigate_peek(&shell, direction);
                    glib::Propagation::Stop
                }
                PeekScrollResult::Consumed | PeekScrollResult::Unhandled => glib::Propagation::Stop,
            }
        });
        shell.borrow().peek_view.widget().add_controller(controller);
    }

    pub(crate) fn focus_terminal_after_present(shell: &Rc<RefCell<Self>>) {
        let weak = Rc::downgrade(shell);
        glib::timeout_add_local_once(Duration::from_millis(50), move || {
            if let Some(shell) = weak.upgrade() {
                let shell = shell.borrow();
                if !shell.shutting_down {
                    shell.present();
                }
            }
        });
    }

    fn move_active_worklane(&mut self, delta: isize) {
        let active_id = self.state.active_worklane_id().to_owned();
        self.move_worklane(&active_id, delta);
    }

    fn move_worklane(&mut self, worklane_id: &str, delta: isize) -> bool {
        let Some(index) = self
            .state
            .worklanes()
            .iter()
            .position(|worklane| worklane.id == worklane_id)
        else {
            return false;
        };
        let Some(target) = index.checked_add_signed(delta) else {
            return false;
        };
        if self.state.move_worklane(worklane_id, target) {
            eprintln!("zentty-linux: action=move-worklane target={target}");
            self.render_sidebar();
            return true;
        }
        false
    }

    fn create_worklane(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let (worklane_id, pane_id) = {
            let mut shell = shell.borrow_mut();
            let worklane_id = format!("worklane-{}", shell.next_worklane_number);
            shell.next_worklane_number += 1;
            let pane_id = shell.take_pane_id();
            let placement = shell.config.worklanes.new_worklane_placement;
            if !shell.state.create_worklane_with_placement(
                worklane_id.clone(),
                pane_id.clone(),
                placement,
            ) {
                return Err("generated duplicate worklane or pane identity".to_owned());
            }
            (worklane_id, pane_id)
        };
        if let Err(error) = PaneRuntimeCoordinator::create_surface(shell, &pane_id) {
            shell.borrow_mut().state.close_active_worklane();
            return Err(error);
        }
        let shell_ref = shell.borrow();
        eprintln!("zentty-linux: action=new-worklane id={worklane_id} pane={pane_id}");
        shell_ref.render();
        shell_ref.focus_selected_surface();
        Ok(())
    }

    fn split_focused_pane_right(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let width = f64::from(PaneLayoutPolicy::visible_split_width(
            shell.borrow().pane_viewport_width(),
        ));
        Self::split_focused_pane(shell, ACTION_SPLIT_PANE_RIGHT, move |state, pane_id| {
            state.split_focused_pane_right_visibly(pane_id, width)
        })?;
        Self::scroll_panes_to_end(shell);
        Ok(())
    }

    fn create_focused_pane_right(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let behavior = {
            let shell = shell.borrow();
            shell
                .config
                .pane_layout
                .right_insertion_behavior(shell.pane_viewport_width())
        };
        match behavior {
            zentty_core::PaneRightInsertionBehavior::VisibleSplit => {
                Self::split_focused_pane_right(shell)
            }
            zentty_core::PaneRightInsertionBehavior::WorklaneAdd => {
                Self::add_focused_pane_right(shell)
            }
        }
    }

    fn add_focused_pane_right(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let width = shell.borrow().focused_column_render_width();
        Self::split_focused_pane(shell, ACTION_ADD_PANE_RIGHT, move |state, pane_id| {
            state.add_pane_right_without_resizing(pane_id, f64::from(width))
        })?;
        Self::scroll_panes_to_end(shell);
        Ok(())
    }

    fn add_focused_pane_left(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let width = shell.borrow().focused_column_render_width();
        Self::split_focused_pane(shell, ACTION_ADD_PANE_LEFT, move |state, pane_id| {
            state.insert_focused_pane_left(pane_id, f64::from(width))
        })?;
        shell.borrow().scroll_panes_to_focused();
        Ok(())
    }

    fn duplicate_focused_pane(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let (width, working_directory) = {
            let shell = shell.borrow();
            let pane = shell
                .state
                .focused_pane_id()
                .and_then(|pane_id| shell.state.pane(pane_id))
                .ok_or_else(|| "there is no focused pane to duplicate".to_owned())?;
            (
                shell.focused_column_render_width(),
                pane.working_directory.clone(),
            )
        };
        Self::split_focused_pane(shell, ACTION_DUPLICATE_PANE, move |state, pane_id| {
            let inserted = state.add_pane_right_without_resizing(pane_id.clone(), f64::from(width));
            inserted && state.configure_pane_launch(&pane_id, working_directory, None)
        })?;
        Self::scroll_panes_to_end(shell);
        Ok(())
    }

    fn split_focused_pane_below(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        Self::split_focused_pane(shell, ACTION_SPLIT_PANE_BELOW, |state, pane_id| {
            state.split_focused_pane_below(pane_id)
        })
    }

    fn split_focused_pane(
        shell: &Rc<RefCell<Self>>,
        action: &str,
        update: impl FnOnce(&mut WorkspaceState, String) -> bool,
    ) -> Result<(), String> {
        let pane_id = {
            let mut shell = shell.borrow_mut();
            let pane_id = shell.take_pane_id();
            if !update(&mut shell.state, pane_id.clone()) {
                return Err("generated duplicate pane identity".to_owned());
            }
            pane_id
        };
        if let Err(error) = PaneRuntimeCoordinator::create_surface(shell, &pane_id) {
            let _ = shell.borrow_mut().state.close_focused_pane();
            return Err(error);
        }
        let shell_ref = shell.borrow();
        eprintln!("zentty-linux: action={action} pane={pane_id}");
        shell_ref.render();
        shell_ref.focus_selected_surface();
        Ok(())
    }

    fn close_focused_pane(shell: &Rc<RefCell<Self>>) {
        let pane_id = shell.borrow().state.focused_pane_id().map(str::to_owned);
        if let Some(pane_id) = pane_id {
            Self::request_close_pane(shell, &pane_id);
        }
    }

    fn request_close_pane(shell: &Rc<RefCell<Self>>, pane_id: &str) {
        let should_confirm = shell
            .borrow()
            .config
            .confirmations
            .confirm_before_closing_pane;
        if should_confirm {
            let weak = Rc::downgrade(shell);
            let confirmed_pane = pane_id.to_owned();
            let action: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(shell) = weak.upgrade() {
                    Self::close_pane(&shell, &confirmed_pane);
                }
            });
            Self::confirm_action(
                &shell.borrow().window,
                "Close this pane?",
                "The terminal process in this pane will be closed.",
                "Close Pane",
                action,
            );
        } else {
            Self::close_pane(shell, pane_id);
        }
    }

    fn scroll_panes_to_end(shell: &Rc<RefCell<Self>>) {
        let weak = Rc::downgrade(shell);
        glib::timeout_add_local_once(Duration::from_millis(50), move || {
            if let Some(shell) = weak.upgrade() {
                let adjustment = shell.borrow().pane_scroll.hadjustment();
                let maximum = (adjustment.upper() - adjustment.page_size()).max(0.0);
                adjustment.set_value(maximum);
                eprintln!(
                    "zentty-linux: pane-scroll value={:.0} maximum={maximum:.0}",
                    adjustment.value()
                );
            }
        });
    }

    fn scroll_panes_to_focused(&self) {
        let worklane = self.state.active_worklane();
        let widths = self.resolved_column_widths();
        let focused_index = worklane
            .columns
            .iter()
            .position(|column| column.id == worklane.focused_column_id)
            .unwrap_or(0);
        let target = widths
            .iter()
            .take(focused_index)
            .fold(0_i32, |total, width| {
                total.saturating_add(*width + PaneLayoutPolicy::INTER_PANE_SPACING)
            });
        let adjustment = self.pane_scroll.hadjustment();
        glib::timeout_add_local_once(Duration::from_millis(50), move || {
            let maximum = (adjustment.upper() - adjustment.page_size()).max(0.0);
            adjustment.set_value(f64::from(target).min(maximum));
            eprintln!(
                "zentty-linux: pane-scroll-focused value={:.0} maximum={maximum:.0}",
                adjustment.value()
            );
        });
    }

    fn activate_pane_control(shell: &Rc<RefCell<Self>>, pane_id: &str, action: PaneControlAction) {
        {
            let mut shell_ref = shell.borrow_mut();
            if shell_ref.shutting_down || !shell_ref.state.select_pane(pane_id) {
                return;
            }
            shell_ref.refresh_sidebar_metadata();
        }
        eprintln!(
            "zentty-linux: pane-control action={} target={pane_id}",
            action.id()
        );
        match action {
            PaneControlAction::SplitRight => {
                if let Err(error) = Self::split_focused_pane_right(shell) {
                    Self::report_action_error(shell, ACTION_SPLIT_PANE_RIGHT, &error);
                }
            }
            PaneControlAction::AddPaneRight => {
                if let Err(error) = Self::add_focused_pane_right(shell) {
                    Self::report_action_error(shell, ACTION_ADD_PANE_RIGHT, &error);
                }
            }
            PaneControlAction::NewPaneBelow => {
                if let Err(error) = Self::split_focused_pane_below(shell) {
                    Self::report_action_error(shell, ACTION_SPLIT_PANE_BELOW, &error);
                }
            }
            PaneControlAction::MoveToNewWindow => {
                shell.borrow().request_move_pane_to_new_window();
            }
            PaneControlAction::ClosePane => Self::request_close_pane(shell, pane_id),
        }
    }

    fn close_worklane(shell: &Rc<RefCell<Self>>, worklane_id: &str) {
        let pane_ids = {
            let shell = shell.borrow();
            shell
                .state
                .worklanes()
                .iter()
                .find(|worklane| worklane.id == worklane_id)
                .map(|worklane| {
                    worklane
                        .columns
                        .iter()
                        .flat_map(|column| &column.panes)
                        .map(|pane| pane.id.clone())
                        .collect::<Vec<_>>()
                })
        };
        let Some(pane_ids) = pane_ids else {
            return;
        };
        let mut shell_ref = shell.borrow_mut();
        if !shell_ref.state.close_worklane(worklane_id) {
            return;
        }
        for pane_id in &pane_ids {
            if let Err(error) = shell_ref.remove_live_surface(pane_id) {
                drop(shell_ref);
                Self::report_action_error(shell, ACTION_CLOSE_WORKLANE, &error);
                return;
            }
        }
        eprintln!(
            "zentty-linux: action=close-worklane id={worklane_id} panes={}",
            pane_ids.len()
        );
        shell_ref.render();
        shell_ref.focus_selected_surface();
    }

    fn close_pane(shell: &Rc<RefCell<Self>>, pane_id: &str) {
        let mut shell_ref = shell.borrow_mut();
        match shell_ref.state.close_pane(pane_id) {
            ClosePaneOutcome::Closed => {
                if let Err(error) = shell_ref.remove_live_surface(pane_id) {
                    drop(shell_ref);
                    Self::report_action_error(shell, ACTION_CLOSE_PANE, &error);
                    return;
                }
                eprintln!("zentty-linux: action=close-pane pane={pane_id}");
                shell_ref.render();
                shell_ref.scroll_panes_to_focused();
                shell_ref.focus_selected_surface();
            }
            ClosePaneOutcome::CloseWindow => {
                if let Err(error) = shell_ref.remove_live_surface(pane_id) {
                    drop(shell_ref);
                    Self::report_action_error(shell, ACTION_CLOSE_PANE, &error);
                    return;
                }
                eprintln!("zentty-linux: action=close-pane pane={pane_id} close-window=true");
                shell_ref.main_loop.quit();
            }
            ClosePaneOutcome::NotFound => {}
        }
    }

    fn restore_closed_pane(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let restored = {
            let mut shell = shell.borrow_mut();
            let pane_id = shell.take_pane_id();
            shell.state.restore_closed_pane(pane_id)
        };
        let Some(restored) = restored else {
            eprintln!("zentty-linux: action=restore-closed-pane available=false");
            return Ok(());
        };
        if let Some(prefill) = &restored.prefill_text {
            shell
                .borrow_mut()
                .pane_runtime
                .queue_prefill(&restored.pane_id, prefill.clone());
        }
        if let Err(error) = PaneRuntimeCoordinator::create_surface(shell, &restored.pane_id) {
            let mut shell = shell.borrow_mut();
            shell.pane_runtime.cancel_prefill(&restored.pane_id);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            let _ = shell
                .state
                .rollback_restored_pane_at(&restored.pane_id, now);
            return Err(error);
        }
        let shell_ref = shell.borrow();
        eprintln!(
            "zentty-linux: action=restore-closed-pane pane={} worklane={} cwd={} prefill={}",
            restored.pane_id,
            restored.worklane_id,
            restored.working_directory.as_deref().unwrap_or("none"),
            restored.prefill_text.as_deref().unwrap_or("none")
        );
        shell_ref.render();
        shell_ref.focus_selected_surface();
        Ok(())
    }

    fn report_action_error(shell: &Rc<RefCell<Self>>, action: &str, error: &str) {
        eprintln!("zentty-linux: action={action} failed: {error}");
        shell.borrow().main_loop.quit();
    }

    pub(crate) fn apply_agent_inputs(
        shell: &Rc<RefCell<Self>>,
        tmux_commands: Vec<zentty_agent_ipc::AuthenticatedTmuxRequest>,
        events: Vec<zentty_core::AuthenticatedAgentEvent>,
    ) {
        AgentEventCoordinator::apply_inputs(shell, tmux_commands, events);
    }

    fn schedule_codex_transcript_enrichment(&mut self, pane_id: &str) {
        self.agent_events.schedule_for_pane(&self.state, pane_id);
    }

    pub(crate) fn sync_agent_targets(&mut self) {
        if let Err(error) = self.agent_events.sync_targets(&self.state) {
            eprintln!("zentty-linux: agent-target-sync failed={error}");
        }
    }

    fn remove_live_surface(&mut self, pane_id: &str) -> Result<(), String> {
        self.agent_events.unregister_pane(pane_id);
        project_context_runtime::forget_pane(self, pane_id);
        self.pane_runtime.remove(pane_id, false).map(|_| ())
    }

    fn take_pane_id(&mut self) -> String {
        let id = format!("pane-{}", self.next_pane_number);
        self.next_pane_number += 1;
        id
    }

    fn render(&self) {
        clear_pane_columns(&self.pane_box);
        self.rendered_columns.borrow_mut().clear();
        self.render_sidebar();
        self.refresh_pane_presentation();

        let column_widths = self.resolved_column_widths();
        let single_column = column_widths.len() == 1;
        let content_width = column_widths
            .iter()
            .copied()
            .fold(0_i32, i32::saturating_add)
            .saturating_add(
                i32::try_from(column_widths.len().saturating_sub(1)).unwrap_or(i32::MAX),
            )
            .max(self.pane_viewport_width());
        self.pane_box.set_width_request(content_width);
        eprintln!(
            "zentty-linux: pane-layout viewport={} content={} columns={}",
            self.pane_viewport_width(),
            content_width,
            self.state
                .active_columns()
                .iter()
                .zip(&column_widths)
                .map(|(column, width)| format!("{}:{width}", column.id))
                .collect::<Vec<_>>()
                .join(",")
        );

        let columns = self.state.active_columns();
        let viewport_height = self.pane_viewport_height();
        for (column_index, (column, width)) in columns.iter().zip(column_widths).enumerate() {
            let column_overlay = self.build_column_overlay(
                column,
                width,
                single_column,
                viewport_height,
                column_index + 1 < columns.len(),
            );
            self.pane_box.append(&column_overlay);
            self.rendered_columns
                .borrow_mut()
                .insert(column.id.clone(), column_overlay);
        }
        self.apply_pane_height_requests(true);
        self.refresh_pane_layout_action_availability();
        eprintln!("zentty-linux: topology={}", self.topology_receipt());
        eprintln!("zentty-linux: geometry={}", self.geometry_receipt());
    }

    fn build_column_overlay(
        &self,
        column: &PaneColumnState,
        width: i32,
        single_column: bool,
        viewport_height: i32,
        has_trailing_column: bool,
    ) -> gtk::Overlay {
        let column_box = gtk::Box::new(gtk::Orientation::Vertical, 1);
        column_box.set_homogeneous(false);
        column_box.set_width_request(width);
        column_box.set_hexpand(single_column);
        column_box.set_vexpand(true);
        for pane in &column.panes {
            if let Some(frame) = self.pane_runtime.frame(&pane.id) {
                remove_frame_from_box_parent(frame.widget());
                column_box.append(frame.widget());
            }
        }
        let overlay = gtk::Overlay::new();
        overlay.set_width_request(width);
        overlay.set_hexpand(single_column);
        overlay.set_vexpand(true);
        overlay.set_child(Some(&column_box));

        let heights = model_heights_to_pixels(&column.pane_heights, viewport_height);
        let mut boundary = 0_i32;
        for (index, (pane, height)) in column.panes.iter().zip(heights).enumerate() {
            boundary = boundary.saturating_add(height);
            if index + 1 < column.panes.len() {
                let handle = self.new_pane_divider_handle(&column.id, &pane.id);
                handle.set_margin_top(boundary.saturating_sub(4));
                overlay.add_overlay(&handle);
                boundary = boundary.saturating_add(1);
            }
        }
        if has_trailing_column {
            overlay.add_overlay(&self.new_column_divider_handle(&column.id));
        }
        overlay
    }

    fn new_pane_divider_handle(&self, column_id: &str, pane_id: &str) -> gtk::Box {
        let divider = PaneDivider::Pane {
            column_id: column_id.to_owned(),
            after_pane_id: pane_id.to_owned(),
        };
        let delta_handle = self.self_handle.borrow().clone();
        let delta_column_id = column_id.to_owned();
        let delta_pane_id = pane_id.to_owned();
        let equalize_handle = self.self_handle.borrow().clone();
        let equalize_column_id = column_id.to_owned();
        let equalize_pane_id = pane_id.to_owned();
        pane_dividers::new_handle(
            &divider,
            move |delta| {
                if let Some(shell) = delta_handle.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    return shell.resize_pane_divider_interactively(
                        &delta_column_id,
                        &delta_pane_id,
                        delta,
                    );
                }
                0.0
            },
            move || {
                if let Some(shell) = equalize_handle.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.equalize_pane_divider_interactively(
                        &equalize_column_id,
                        &equalize_pane_id,
                    );
                }
            },
        )
    }

    fn new_column_divider_handle(&self, column_id: &str) -> gtk::Box {
        let divider = PaneDivider::Column {
            after_column_id: column_id.to_owned(),
        };
        let delta_handle = self.self_handle.borrow().clone();
        let delta_column_id = column_id.to_owned();
        let equalize_handle = self.self_handle.borrow().clone();
        let equalize_column_id = column_id.to_owned();
        pane_dividers::new_handle(
            &divider,
            move |delta| {
                if let Some(shell) = delta_handle.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    return shell.resize_column_divider_interactively(&delta_column_id, delta);
                }
                0.0
            },
            move || {
                if let Some(shell) = equalize_handle.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.equalize_column_divider_interactively(&equalize_column_id);
                }
            },
        )
    }

    fn pane_viewport_width(&self) -> i32 {
        let allocated = self.pane_scroll.width();
        if allocated > 1 {
            return allocated;
        }
        self.window
            .default_width()
            .saturating_sub(SidebarWidthPreference::DEFAULT)
            .max(200)
    }

    fn pane_viewport_height(&self) -> i32 {
        bounded_pane_viewport_height(
            self.pane_scroll.height(),
            self.window.height(),
            self.chrome.widget().height(),
            self.window.default_height(),
        )
    }

    fn apply_pane_height_requests(&self, force: bool) {
        let viewport_height = self.pane_viewport_height();
        if !force && self.last_pane_viewport_height.get() == viewport_height {
            return;
        }
        self.last_pane_viewport_height.set(viewport_height);
        for column in self.state.active_columns() {
            let pane_count = column.panes.len();
            if pane_count == 1 {
                if let Some(frame) = self.pane_runtime.frame(&column.panes[0].id) {
                    frame.widget().set_height_request(-1);
                    frame.widget().set_vexpand(true);
                }
                continue;
            }
            let heights = model_heights_to_pixels(&column.pane_heights, viewport_height);
            for (pane, height) in column.panes.iter().zip(heights) {
                if let Some(frame) = self.pane_runtime.frame(&pane.id) {
                    frame.widget().set_height_request(height);
                    frame.widget().set_vexpand(false);
                }
            }
            eprintln!(
                "zentty-linux: pane-height-layout column={} viewport={viewport_height} panes={}",
                column.id,
                column
                    .panes
                    .iter()
                    .filter_map(|pane| self.pane_runtime.frame(&pane.id).map(|frame| {
                        format!("{}:{}", pane.id, frame.widget().height_request())
                    }))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
    }

    fn materialize_active_column_widths(&mut self) {
        let widths = self.resolved_column_widths();
        let pane_widths = self
            .state
            .active_columns()
            .iter()
            .zip(widths)
            .filter_map(|(column, width)| {
                column
                    .panes
                    .first()
                    .map(|pane| (pane.id.clone(), f64::from(width)))
            })
            .collect::<Vec<_>>();
        for (pane_id, width) in pane_widths {
            let _ = self.state.restore_column_width(&pane_id, width);
        }
    }

    fn apply_column_width_requests(&self) {
        for (column, width) in self
            .state
            .active_columns()
            .iter()
            .zip(self.resolved_column_widths())
        {
            if let Some(overlay) = self.rendered_columns.borrow().get(&column.id) {
                overlay.set_width_request(width);
                if let Some(child) = overlay.child() {
                    child.set_width_request(width);
                }
            }
        }
    }

    fn resize_column_divider_interactively(&mut self, column_id: &str, delta: f64) -> f64 {
        self.materialize_active_column_widths();
        let before = self
            .state
            .active_columns()
            .iter()
            .find(|column| column.id == column_id)
            .map_or(0.0, |column| column.width);
        let minimum_width = self.minimum_column_width(column_id);
        if !self
            .state
            .resize_column_divider(column_id, delta, minimum_width)
        {
            return 0.0;
        }
        let after = self
            .state
            .active_columns()
            .iter()
            .find(|column| column.id == column_id)
            .map_or(before, |column| column.width);
        self.apply_column_width_requests();
        eprintln!(
            "zentty-linux: pane-divider-resize axis=horizontal after={column_id} leading={after:.3}"
        );
        after - before
    }

    fn resize_pane_divider_interactively(
        &mut self,
        column_id: &str,
        pane_id: &str,
        delta: f64,
    ) -> f64 {
        let viewport_height = f64::from(self.pane_viewport_height());
        let before = self
            .state
            .active_columns()
            .iter()
            .find(|column| column.id == column_id)
            .and_then(|column| {
                column
                    .panes
                    .iter()
                    .position(|pane| pane.id == pane_id)
                    .map(|index| {
                        (
                            column.pane_heights[index],
                            column.pane_heights.iter().sum::<f64>(),
                        )
                    })
            });
        let Some((before_weight, total_weight)) = before else {
            return 0.0;
        };
        let minimum_height = self.minimum_pane_pair_height(column_id, pane_id);
        if !self.state.resize_pane_divider(
            column_id,
            pane_id,
            delta,
            viewport_height,
            minimum_height,
        ) {
            return 0.0;
        }
        self.last_vertical_divider
            .replace(Some((column_id.to_owned(), pane_id.to_owned())));
        let after_weight = self
            .state
            .active_columns()
            .iter()
            .find(|column| column.id == column_id)
            .and_then(|column| {
                column
                    .panes
                    .iter()
                    .position(|pane| pane.id == pane_id)
                    .map(|index| column.pane_heights[index])
            })
            .unwrap_or(before_weight);
        self.apply_pane_height_requests(true);
        eprintln!(
            "zentty-linux: pane-divider-resize axis=vertical column={column_id} after={pane_id} leading={after_weight:.6}"
        );
        (after_weight - before_weight) / total_weight * viewport_height
    }

    fn equalize_column_divider_interactively(&mut self, column_id: &str) {
        self.materialize_active_column_widths();
        let minimum_width = self.minimum_column_width(column_id);
        if self.state.equalize_column_divider(column_id, minimum_width) {
            self.apply_column_width_requests();
            eprintln!("zentty-linux: pane-divider-equalize axis=horizontal after={column_id}");
            self.schedule_layout_render();
        }
    }

    fn equalize_pane_divider_interactively(&mut self, column_id: &str, pane_id: &str) {
        if self.state.equalize_pane_divider(column_id, pane_id) {
            self.last_vertical_divider
                .replace(Some((column_id.to_owned(), pane_id.to_owned())));
            self.apply_pane_height_requests(true);
            eprintln!(
                "zentty-linux: pane-divider-equalize axis=vertical column={column_id} after={pane_id}"
            );
            self.schedule_layout_render();
        }
    }

    fn schedule_layout_render(&self) {
        let shell = self.self_handle.borrow().clone();
        glib::idle_add_local_once(move || {
            if let Some(shell) = shell.upgrade()
                && let Ok(shell) = shell.try_borrow()
            {
                shell.render();
            }
        });
    }

    fn pane_cell_size(&self, pane_id: &str) -> Option<zentty_ghostty::CellSize> {
        self.pane_runtime.surface(pane_id)?.cell_size().ok()
    }

    fn minimum_column_width(&self, column_id: &str) -> f64 {
        self.state
            .active_columns()
            .iter()
            .find(|column| column.id == column_id)
            .into_iter()
            .flat_map(|column| &column.panes)
            .filter_map(|pane| self.pane_cell_size(&pane.id))
            .map(|cell| (cell.width * 5.0).max(120.0))
            .fold(120.0, f64::max)
    }

    fn minimum_pane_pair_height(&self, column_id: &str, pane_id: &str) -> f64 {
        let Some(column) = self
            .state
            .active_columns()
            .iter()
            .find(|column| column.id == column_id)
        else {
            return 120.0;
        };
        let Some(index) = column.panes.iter().position(|pane| pane.id == pane_id) else {
            return 120.0;
        };
        column
            .panes
            .get(index..=index.saturating_add(1))
            .into_iter()
            .flatten()
            .filter_map(|pane| self.pane_cell_size(&pane.id))
            .map(|cell| (cell.height * 5.0).max(120.0))
            .fold(120.0, f64::max)
    }

    fn resize_focused_pane_by_cell(&mut self, direction: PaneResizeDirection) -> bool {
        let Some(pane_id) = self.state.focused_pane_id().map(str::to_owned) else {
            return false;
        };
        let Some(cell) = self.pane_cell_size(&pane_id) else {
            eprintln!(
                "zentty-linux: pane-keyboard-resize unavailable pane={pane_id} reason=cell-size"
            );
            return false;
        };
        self.materialize_active_column_widths();
        let changed = match direction {
            PaneResizeDirection::Left | PaneResizeDirection::Right => {
                let focused_column = self.state.active_worklane().focused_column_id.clone();
                let minimum = self.minimum_column_width(&focused_column);
                let step = (cell.width * 5.0).max(120.0) / 5.0;
                self.state.resize_focused_column(
                    direction,
                    step,
                    minimum,
                    f64::from(self.pane_viewport_width()),
                )
            }
            PaneResizeDirection::Up | PaneResizeDirection::Down => {
                let preferred = self.last_vertical_divider.borrow().clone();
                let preferred_pane = preferred
                    .as_ref()
                    .filter(|(column, _)| self.state.active_worklane().focused_column_id == *column)
                    .map(|(_, pane)| pane.as_str());
                let Some((column_id, divider_after)) = self
                    .state
                    .focused_vertical_divider_after(preferred_pane)
                    .map(|(column, pane)| (column.to_owned(), pane.to_owned()))
                else {
                    return false;
                };
                let minimum = self.minimum_pane_pair_height(&column_id, &divider_after);
                let step = (cell.height * 5.0).max(120.0) / 5.0;
                self.state.resize_focused_pane_vertically(
                    direction,
                    step,
                    f64::from(self.pane_viewport_height()),
                    minimum,
                    preferred_pane,
                )
            }
        };
        if changed {
            eprintln!(
                "zentty-linux: pane-keyboard-resize direction={direction:?} pane={pane_id} cell={:.3}x{:.3}",
                cell.width, cell.height
            );
        }
        changed
    }

    fn refresh_pane_layout_action_availability(&self) {
        let columns = self.state.active_columns();
        let focused_column = self.state.active_worklane().focused_column_id.as_str();
        let focused_column_panes = columns
            .iter()
            .find(|column| column.id == focused_column)
            .map_or(0, |column| column.panes.len());
        let Some(router) = &self.action_router else {
            return;
        };
        let workspace_panes = self
            .state
            .worklanes()
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .map(|column| column.panes.len())
            .sum();
        router.refresh_availability(columns.len(), focused_column_panes, workspace_panes);
    }

    fn resolved_column_widths(&self) -> Vec<i32> {
        let columns = self.state.active_columns();
        let viewport_width = self.pane_viewport_width();
        let fallback = viewport_width
            .saturating_sub(i32::try_from(columns.len().saturating_sub(1)).unwrap_or(i32::MAX))
            .checked_div(i32::try_from(columns.len()).unwrap_or(i32::MAX).max(1))
            .unwrap_or(viewport_width)
            .max(1);
        columns
            .iter()
            .map(|column| {
                if columns.len() == 1 || column.width <= 1.0 {
                    fallback
                } else {
                    model_width_to_pixels(column.width)
                }
            })
            .collect()
    }

    fn focused_column_render_width(&self) -> i32 {
        let worklane = self.state.active_worklane();
        let widths = self.resolved_column_widths();
        worklane
            .columns
            .iter()
            .position(|column| column.id == worklane.focused_column_id)
            .and_then(|index| widths.get(index).copied())
            .unwrap_or_else(|| self.pane_viewport_width())
    }

    fn render_sidebar(&self) {
        let summaries = self.sidebar_summaries();
        let servers = self.ranked_servers();
        sidebar::render(
            &self.sidebar,
            &self.window,
            &summaries,
            self.config.clipboard,
            &servers,
            self.bookmark_runtime.templates(),
            self.state.active_worklane().bookmark_origin_id.as_deref(),
        );
        self.render_chrome(&summaries);
        self.schedule_active_worklane_reveal();
    }

    fn refresh_sidebar_metadata(&self) {
        let summaries = self.sidebar_summaries();
        let servers = self.ranked_servers();
        if !sidebar::update_metadata(&self.sidebar, &summaries) {
            sidebar::render(
                &self.sidebar,
                &self.window,
                &summaries,
                self.config.clipboard,
                &servers,
                self.bookmark_runtime.templates(),
                self.state.active_worklane().bookmark_origin_id.as_deref(),
            );
        }
        self.render_chrome(&summaries);
        self.schedule_active_worklane_reveal();
        self.refresh_pane_presentation();
        self.refresh_pane_layout_action_availability();
    }

    fn refresh_project_context_presentation(&self) {
        if self.command_palette.is_visible()
            || self.global_search_view.root.is_visible()
            || sidebar::has_worklane_reorder_preview(&self.sidebar)
        {
            return;
        }
        let summaries = self.sidebar_summaries();
        sidebar::update_project_context_metadata(&self.sidebar, &summaries);
        sidebar::update_project_icon_metadata(&self.sidebar, &summaries);
        self.render_chrome(&summaries);
    }

    fn render_chrome(&self, summaries: &[zentty_core::SidebarWorklaneSummary]) {
        self.chrome.render(
            summaries,
            self.state.can_navigate_back(),
            self.state.can_navigate_forward(),
        );
        self.chrome
            .set_open_with_context_available(open_with_runtime::focused_context_is_available(self));
    }

    fn sidebar_summaries(&self) -> Vec<zentty_core::SidebarWorklaneSummary> {
        let mut summaries = self.state.sidebar_summaries();
        for pane in summaries
            .iter_mut()
            .flat_map(|worklane| &mut worklane.pane_rows)
        {
            pane.project_context = self
                .project_context_runtime
                .contexts
                .get(&pane.pane_id)
                .cloned();
            pane.project_icon_path = self
                .config
                .panes
                .show_project_icons
                .then(|| {
                    self.project_context_runtime
                        .icons
                        .get(&pane.pane_id)
                        .cloned()
                })
                .flatten();
        }
        summaries
    }

    fn ranked_servers(&self) -> Vec<zentty_core::RankedServer> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
            .unwrap_or_default();
        let focused_pane_id = self.state.focused_pane_id().map(str::to_owned);
        let running_pane_ids: std::collections::BTreeSet<String> =
            self.pane_runtime.live_pane_ids().iter().cloned().collect();
        let ignored_port_rules =
            ServerPortRule::normalize(&self.config.server_detection.ignored_port_rules);
        let mut ranked = Vec::new();
        for worklane in self.state.worklanes() {
            ranked.extend(rank_servers(
                self.server_runtime
                    .servers
                    .iter()
                    .filter(|server| server.worklane_id == worklane.id)
                    .cloned()
                    .collect(),
                &ServerRelevanceContext {
                    focused_pane_id: focused_pane_id.clone(),
                    running_pane_ids: running_pane_ids.clone(),
                    ignored_port_rules: ignored_port_rules.clone(),
                    selected_origin: None,
                    now_ms,
                },
            ));
        }
        ranked
    }

    fn schedule_active_worklane_reveal(&self) {
        let generation = self.sidebar_reveal_generation.get().wrapping_add(1);
        self.sidebar_reveal_generation.set(generation);
        let tracker = Rc::clone(&self.sidebar_reveal_generation);
        let sidebar = self.sidebar.clone();
        let scroll = self.sidebar_scroll.clone();
        let worklane_id = self.state.active_worklane_id().to_owned();
        glib::idle_add_local_once(move || {
            if tracker.get() != generation {
                return;
            }
            let tracker_again = Rc::clone(&tracker);
            glib::idle_add_local_once(move || {
                if tracker_again.get() != generation {
                    return;
                }
                let Some(card) = sidebar::worklane_card(&sidebar, &worklane_id) else {
                    return;
                };
                let Some(bounds) = card.compute_bounds(&sidebar) else {
                    return;
                };
                let adjustment = scroll.vadjustment();
                let top = adjustment.value();
                let card_top = f64::from(bounds.y());
                if let Some((card_top, card_bottom)) = sidebar::reveal_range(
                    top,
                    adjustment.page_size(),
                    card_top,
                    f64::from(bounds.height()),
                ) {
                    adjustment.clamp_page(card_top, card_bottom);
                    eprintln!(
                        "zentty-linux: sidebar-reveal worklane={} value={}",
                        worklane_id,
                        adjustment.value()
                    );
                }
                eprintln!(
                    "zentty-linux: sidebar-active-visible worklane={} value={}",
                    worklane_id,
                    adjustment.value()
                );
            });
        });
    }

    fn navigate_history(&mut self, backward: bool) {
        let previous_worklane = self.state.active_worklane_id().to_owned();
        let changed = if backward {
            self.state.navigate_back()
        } else {
            self.state.navigate_forward()
        };
        if !changed {
            // Navigation purges closed/stale references while searching. Even
            // without a target, refresh button sensitivity to reflect the
            // resulting history stacks.
            self.refresh_sidebar_metadata();
            return;
        }
        eprintln!(
            "zentty-linux: action={} worklane={} pane={}",
            if backward {
                ACTION_NAVIGATE_BACK
            } else {
                ACTION_NAVIGATE_FORWARD
            },
            self.state.active_worklane_id(),
            self.state.focused_pane_id().unwrap_or("none")
        );
        if previous_worklane == self.state.active_worklane_id() {
            self.refresh_sidebar_metadata();
        } else {
            self.render();
        }
        self.focus_selected_surface();
    }

    fn select_adjacent_pane(&mut self, forward: bool) {
        let previous_worklane = self.state.active_worklane_id().to_owned();
        if !self.state.select_adjacent_pane(forward) {
            return;
        }
        eprintln!(
            "zentty-linux: action={} worklane={} pane={}",
            if forward {
                ACTION_NEXT_PANE
            } else {
                ACTION_PREVIOUS_PANE
            },
            self.state.active_worklane_id(),
            self.state.focused_pane_id().unwrap_or("none")
        );
        if previous_worklane == self.state.active_worklane_id() {
            self.refresh_sidebar_metadata();
        } else {
            self.render();
        }
        self.scroll_panes_to_focused();
        self.focus_selected_surface();
    }

    fn select_adjacent_worklane(&mut self, forward: bool) {
        if !self.state.select_adjacent_worklane(forward) {
            return;
        }
        eprintln!(
            "zentty-linux: action={} worklane={} pane={}",
            if forward {
                ACTION_NEXT_WORKLANE
            } else {
                ACTION_PREVIOUS_WORKLANE
            },
            self.state.active_worklane_id(),
            self.state.focused_pane_id().unwrap_or("none")
        );
        self.render();
        self.focus_selected_surface();
    }

    fn pane_references_in_sidebar_order(&self) -> Vec<PaneReference> {
        self.state
            .worklanes()
            .iter()
            .flat_map(|worklane| {
                worklane.columns.iter().flat_map(|column| {
                    column
                        .panes
                        .iter()
                        .map(|pane| PaneReference::new(&worklane.id, &pane.id))
                })
            })
            .collect()
    }

    fn current_pane_reference(&self) -> Option<PaneReference> {
        self.state
            .focused_pane_id()
            .map(|pane_id| PaneReference::new(self.state.active_worklane_id(), pane_id))
    }

    fn select_pane_reference(&mut self, target: &PaneReference, focus_terminal: bool) {
        let previous_worklane = self.state.active_worklane_id().to_owned();
        if !self
            .state
            .select_worklane_and_pane(&target.worklane_id, &target.pane_id)
        {
            return;
        }
        if previous_worklane == self.state.active_worklane_id() {
            self.refresh_sidebar_metadata();
        } else {
            self.render();
        }
        if focus_terminal {
            self.focus_selected_surface();
        }
    }

    fn peek_previews(&self) -> Vec<PanePreview> {
        self.state
            .worklanes()
            .iter()
            .enumerate()
            .flat_map(|(worklane_index, worklane)| {
                let worklane_title = worklane
                    .title
                    .clone()
                    .unwrap_or_else(|| format!("Worklane {}", worklane_index + 1));
                worklane.columns.iter().flat_map(move |column| {
                    let worklane_title = worklane_title.clone();
                    column.panes.iter().filter_map(move |pane| {
                        let terminal = self.pane_runtime.surface(&pane.id)?.widget().clone();
                        Some(PanePreview {
                            reference: PaneReference::new(&worklane.id, &pane.id),
                            worklane_title: worklane_title.clone(),
                            pane_title: pane
                                .custom_title
                                .clone()
                                .unwrap_or_else(|| pane.live_title.clone()),
                            terminal,
                            project_icon_path: self
                                .config
                                .panes
                                .show_project_icons
                                .then(|| self.project_context_runtime.icons.get(&pane.id).cloned())
                                .flatten(),
                        })
                    })
                })
            })
            .collect()
    }

    fn refresh_pane_presentation(&self) {
        let focused_pane_id = self.state.focused_pane_id();
        let worklane_color = self.state.active_worklane().color;
        let pane_labels = self
            .state
            .sidebar_summaries()
            .into_iter()
            .find(|worklane| worklane.is_active)
            .map(|worklane| {
                worklane
                    .pane_rows
                    .into_iter()
                    .map(|pane| (pane.pane_id, pane.primary_text))
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let window_transfer_available = self
            .state
            .worklanes()
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .map(|column| column.panes.len())
            .sum::<usize>()
            >= 2;
        for pane_id in self.state.active_pane_ids() {
            if let Some(frame) = self.pane_runtime.frame(pane_id) {
                frame.set_presentation(&PanePresentation {
                    focused: Some(pane_id) == focused_pane_id,
                    worklane_color,
                    show_borders: self.config.panes.show_borders,
                    label: self
                        .config
                        .panes
                        .show_labels
                        .then(|| pane_labels.get(pane_id).cloned())
                        .flatten(),
                });
                frame.set_window_transfer_available(window_transfer_available);
            }
        }
        self.refresh_right_insertion_behavior();
    }

    fn refresh_right_insertion_behavior(&self) {
        for pane_id in self.state.active_pane_ids() {
            if let Some(frame) = self.pane_runtime.frame(pane_id) {
                // Linux does not yet provide Zentty's horizontal gesture,
                // Worklane Peek, and recent-pane management. Keep the
                // pane-local primary action visible until that navigation
                // ecosystem makes full-width offscreen panes discoverable.
                frame.set_right_behavior(
                    self.config
                        .pane_layout
                        .right_insertion_behavior(self.pane_viewport_width()),
                );
            }
        }
    }

    fn focus_selected_surface(&self) {
        if !self.window.is_active() {
            return;
        }
        self.focus_selected_surface_unchecked();
    }

    fn handle_pane_pointer_presence(shell: &Rc<RefCell<Self>>, pane_id: &str, present: bool) {
        let generation = {
            let mut shell = shell.borrow_mut();
            shell.focus_follow_generation = shell.focus_follow_generation.wrapping_add(1);
            if !present || !shell.config.panes.focus_follows_mouse {
                return;
            }
            shell.focus_follow_generation
        };
        let delay = match shell.borrow().config.panes.focus_follows_mouse_delay {
            zentty_core::FocusFollowsMouseDelay::Immediate => std::time::Duration::ZERO,
            zentty_core::FocusFollowsMouseDelay::Short => std::time::Duration::from_millis(150),
        };
        let weak = Rc::downgrade(shell);
        let pane_id = pane_id.to_owned();
        glib::timeout_add_local_once(delay, move || {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            let mut shell = shell.borrow_mut();
            let settings_visible = shell
                .shortcut_settings_window
                .as_ref()
                .is_some_and(gtk::prelude::WidgetExt::is_visible);
            if !focus_follow_should_apply(
                generation,
                shell.focus_follow_generation,
                shell.window.is_active(),
                shell.shutting_down
                    || settings_visible
                    || shell.command_palette.is_visible()
                    || shell.global_search.state().visible
                    || shell.peek_phase.is_active(),
            ) {
                return;
            }
            let target = PaneReference::new(shell.state.active_worklane_id(), &pane_id);
            shell.select_pane_reference(&target, true);
            eprintln!("zentty-linux: focus-follows-mouse pane={pane_id} result=focused");
        });
    }

    fn focus_selected_surface_unchecked(&self) {
        if let Some(pane_id) = self.state.focused_pane_id()
            && let Some(surface) = self.pane_runtime.surface(pane_id)
        {
            gtk::prelude::GtkWindowExt::set_focus(&self.window, Some(surface.widget()));
            surface.grab_focus();
        }
    }

    fn topology_receipt(&self) -> String {
        self.state
            .worklanes()
            .iter()
            .map(|worklane| {
                format!(
                    "{}[title={},color={}]:{}{}",
                    worklane.id,
                    worklane.title.as_deref().unwrap_or("none"),
                    worklane.color.map_or("none", WorklaneColor::as_str),
                    worklane
                        .columns
                        .iter()
                        .flat_map(|column| &column.panes)
                        .map(|pane| pane.id.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    if worklane.id == self.state.active_worklane_id() {
                        "*"
                    } else {
                        ""
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    fn geometry_receipt(&self) -> String {
        self.state
            .worklanes()
            .iter()
            .map(|worklane| {
                format!(
                    "{}:{}",
                    worklane.id,
                    worklane
                        .columns
                        .iter()
                        .map(|column| format!(
                            "{}[{}]",
                            column.id,
                            column
                                .panes
                                .iter()
                                .map(|pane| pane.id.as_str())
                                .collect::<Vec<_>>()
                                .join(",")
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    }
}

fn focus_follow_should_apply(
    requested_generation: u64,
    current_generation: u64,
    window_active: bool,
    transient_ui_active: bool,
) -> bool {
    requested_generation == current_generation && window_active && !transient_ui_active
}

fn initialize_shell_coordinators(shell: &Rc<RefCell<ApplicationShell>>) {
    shell.borrow().self_handle.replace(Rc::downgrade(shell));
    shell.borrow_mut().remote_panes.probe_source = Some(ssh_identity::install(shell));
    shell.borrow_mut().server_runtime.probe_source = server_runtime::install(shell);
    shell.borrow_mut().project_context_runtime.probe_source =
        Some(project_context_runtime::install(shell));
}

fn observe_ghostty_search_state(
    root: &gtk::Widget,
    pane_id: &str,
    shell: Weak<RefCell<ApplicationShell>>,
) {
    let Some(overlay) = find_ghostty_search_overlay(root) else {
        eprintln!("zentty-linux: search-overlay pane={pane_id} error=not-found");
        return;
    };
    log_ghostty_search_state(&overlay, pane_id);
    let pane_id = pane_id.to_owned();
    overlay.connect_notify_local(None, move |overlay, property| {
        if matches!(
            property.name(),
            "active"
                | "has-search-total"
                | "search-total"
                | "has-search-selected"
                | "search-selected"
                | "halign-target"
                | "valign-target"
        ) {
            log_ghostty_search_state(overlay, &pane_id);
            if matches!(
                property.name(),
                "search-total" | "has-search-total" | "search-selected" | "has-search-selected"
            ) {
                let total = overlay
                    .property::<bool>("has-search-total")
                    .then(|| usize::try_from(overlay.property::<u64>("search-total")).ok())
                    .flatten();
                let selected = overlay
                    .property::<bool>("has-search-selected")
                    .then(|| usize::try_from(overlay.property::<u64>("search-selected")).ok())
                    .flatten();
                let pane_id = pane_id.clone();
                let shell = shell.clone();
                glib::idle_add_local_once(move || {
                    if let Some(shell) = shell.upgrade() {
                        shell
                            .borrow_mut()
                            .handle_global_search_state(&pane_id, total, selected);
                    }
                });
            }
        }
    });
}

fn find_ghostty_search_overlay(root: &gtk::Widget) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.type_().name() == "GhosttySearchOverlay" {
            return Some(widget);
        }
        if let Some(found) = find_ghostty_search_overlay(&widget) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn find_search_entry(root: &gtk::Widget) -> Option<gtk::SearchEntry> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Ok(entry) = widget.clone().downcast::<gtk::SearchEntry>() {
            return Some(entry);
        }
        if let Some(entry) = find_search_entry(&widget) {
            return Some(entry);
        }
        child = widget.next_sibling();
    }
    None
}

fn log_ghostty_search_state(overlay: &gtk::Widget, pane_id: &str) {
    let active = overlay.property::<bool>("active");
    let total = overlay
        .property::<bool>("has-search-total")
        .then(|| overlay.property::<u64>("search-total"));
    let selected = overlay
        .property::<bool>("has-search-selected")
        .then(|| overlay.property::<u64>("search-selected"));
    let horizontal = overlay.property::<gtk::Align>("halign-target");
    let vertical = overlay.property::<gtk::Align>("valign-target");
    eprintln!(
        "zentty-linux: search-state pane={pane_id} active={active} total={total:?} selected={selected:?} halign={horizontal:?} valign={vertical:?}"
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalGesture {
    InputSubmitted,
    Interrupted,
}

fn codex_terminal_gesture(key: gdk::Key, modifiers: gdk::ModifierType) -> Option<TerminalGesture> {
    let command_modifiers = modifiers
        & (gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::SHIFT_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK);
    if (key == gdk::Key::Return || key == gdk::Key::KP_Enter) && command_modifiers.is_empty() {
        return Some(TerminalGesture::InputSubmitted);
    }
    if (key == gdk::Key::c || key == gdk::Key::C)
        && command_modifiers == gdk::ModifierType::CONTROL_MASK
    {
        return Some(TerminalGesture::Interrupted);
    }
    None
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn is_close_window_shortcut(key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
    key == gdk::Key::q
        && modifiers.contains(gdk::ModifierType::CONTROL_MASK)
        && !modifiers.intersects(
            gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SHIFT_MASK
                | gdk::ModifierType::SUPER_MASK,
        )
}

fn install_sidebar_width_tracking(
    body: &gtk::Paned,
    sidebar: &gtk::ScrolledWindow,
    preferred_width: Rc<Cell<i32>>,
    adjusting_width: Rc<Cell<bool>>,
) {
    let sidebar = sidebar.clone();
    body.connect_position_notify(move |body| {
        if adjusting_width.get() || body.width() <= 0 {
            return;
        }
        let position = body.position();
        let clamped = SidebarWidthPreference::clamped(position, body.width());
        preferred_width.set(clamped);
        sidebar.set_width_request(clamped);
        if position != clamped {
            adjusting_width.set(true);
            body.set_position(clamped);
            adjusting_width.set(false);
        }
        eprintln!("zentty-linux: sidebar-preferred-width={clamped}");
    });
}

fn build_shell_widgets() -> ShellWidgets {
    let window = gtk::Window::new();
    window.set_title(Some(zentty_core::PRODUCT_NAME));
    window.set_default_size(1000, 700);
    let body = gtk::Paned::new(gtk::Orientation::Horizontal);
    body.set_position(SidebarWidthPreference::DEFAULT);
    body.set_resize_start_child(false);
    body.set_shrink_start_child(true);
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let sidebar_scroll = gtk::ScrolledWindow::new();
    sidebar_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sidebar_scroll.set_child(Some(&sidebar));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_hexpand(true);
    content.set_vexpand(true);
    let pane_box = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    pane_box.set_hexpand(true);
    pane_box.set_vexpand(true);
    let pane_scroll = gtk::ScrolledWindow::new();
    pane_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    pane_scroll.set_hexpand(true);
    pane_scroll.set_vexpand(true);
    pane_scroll.set_child(Some(&pane_box));
    let terminal_overlay = gtk::Overlay::new();
    terminal_overlay.set_hexpand(true);
    terminal_overlay.set_vexpand(true);
    let background_agent_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    background_agent_host.set_hexpand(true);
    background_agent_host.set_vexpand(true);
    background_agent_host.set_can_target(false);
    // Root and map restored background agents behind the active pane tree.
    // The active pane is an opaque, full-allocation overlay, so the compositor
    // never exposes the startup surface and no synthetic focus is required.
    terminal_overlay.set_child(Some(&background_agent_host));
    terminal_overlay.add_overlay(&pane_scroll);
    content.append(&terminal_overlay);
    let sidebar_reservation = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar_reservation.set_width_request(SidebarWidthPreference::DEFAULT);
    body.set_start_child(Some(&sidebar_reservation));
    body.set_end_child(Some(&content));
    let chrome = WindowChrome::new();
    let (overlay, peek_view, command_palette, restore_notice, sidebar_hover_rail) =
        build_root(&chrome, &body, &sidebar_scroll);
    window.set_child(Some(&overlay));
    ShellWidgets {
        window,
        chrome,
        body,
        sidebar,
        sidebar_scroll,
        sidebar_reservation,
        sidebar_hover_rail,
        pane_scroll,
        pane_box,
        background_agent_host,
        peek_view,
        command_palette,
        restore_notice,
    }
}

fn build_root(
    chrome: &WindowChrome,
    body: &gtk::Paned,
    sidebar: &gtk::ScrolledWindow,
) -> (
    gtk::Overlay,
    WorklanePeekView,
    CommandPaletteView,
    RestoreNotice,
    gtk::Box,
) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(chrome.widget());
    root.append(body);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&root));
    let hover_rail = gtk::Box::new(gtk::Orientation::Vertical, 0);
    hover_rail.set_width_request(8);
    hover_rail.set_halign(gtk::Align::Start);
    hover_rail.set_valign(gtk::Align::Fill);
    hover_rail.set_margin_top(38);
    hover_rail.set_visible(false);
    overlay.add_overlay(&hover_rail);
    sidebar.set_width_request(SidebarWidthPreference::DEFAULT);
    sidebar.set_halign(gtk::Align::Start);
    sidebar.set_valign(gtk::Align::Fill);
    sidebar.set_margin_top(38);
    overlay.add_overlay(sidebar);
    let peek_view = WorklanePeekView::new();
    overlay.add_overlay(peek_view.widget());
    let command_palette = CommandPaletteView::new();
    overlay.add_overlay(command_palette.widget());
    let restore_notice = RestoreNotice::new();
    overlay.add_overlay(restore_notice.widget());
    (
        overlay,
        peek_view,
        command_palette,
        restore_notice,
        hover_rail,
    )
}

fn workspace_pane_ids(state: &WorkspaceState) -> Vec<String> {
    state
        .worklanes()
        .iter()
        .flat_map(|worklane| &worklane.columns)
        .flat_map(|column| &column.panes)
        .map(|pane| pane.id.clone())
        .collect()
}

fn next_workspace_identities(state: &WorkspaceState) -> (usize, usize) {
    let next_worklane = next_numeric_identity(
        state
            .worklanes()
            .iter()
            .map(|worklane| worklane.id.as_str()),
        "worklane-",
    );
    let next_pane = next_numeric_identity(
        state
            .worklanes()
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .flat_map(|column| &column.panes)
            .map(|pane| pane.id.as_str()),
        "pane-",
    );
    (next_worklane, next_pane)
}

fn window_contains_pane(window: &WindowRecipe, pane_id: &str) -> bool {
    window
        .worklanes
        .iter()
        .flat_map(|worklane| &worklane.columns)
        .flat_map(|column| &column.panes)
        .any(|pane| pane.id == pane_id)
}

fn restored_pane_commands(
    window: &WindowRecipe,
    drafts: &[PaneRestoreDraft],
) -> BTreeMap<String, String> {
    drafts
        .iter()
        .filter_map(|draft| {
            draft
                .resume_command()
                .map(|command| (draft.pane_id.clone(), command))
        })
        .filter(|(pane_id, _)| window_contains_pane(window, pane_id))
        .collect()
}

fn restore_workspace_state(
    window: &WindowRecipe,
    drafts: &[PaneRestoreDraft],
) -> Result<(WorkspaceState, BTreeMap<String, String>), String> {
    let commands = restored_pane_commands(window, drafts);
    eprintln!(
        "zentty-linux: agent-restore-drafts requested={} accepted={}",
        drafts.len(),
        commands.len()
    );
    let mut state = WorkspaceState::from_window_recipe(window)
        .map_err(|error| format!("workspace restore failed: {error}"))?;
    for draft in drafts {
        if commands.contains_key(&draft.pane_id) {
            let _ = state.seed_restored_agent(draft, unix_time_ms());
        }
    }
    Ok((state, commands))
}

fn clear_pane_columns(container: &gtk::Box) {
    while let Some(column_widget) = container.first_child() {
        if let Ok(column) = column_widget.clone().downcast::<gtk::Box>() {
            while let Some(surface) = column.first_child() {
                column.remove(&surface);
            }
        }
        container.remove(&column_widget);
    }
}

fn clear_box_children(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn remove_frame_from_box_parent(frame: &impl IsA<gtk::Widget>) {
    if let Some(parent) = frame
        .as_ref()
        .parent()
        .and_then(|parent| parent.downcast::<gtk::Box>().ok())
    {
        parent.remove(frame);
    }
}

fn model_width_to_pixels(width: f64) -> i32 {
    let rounded = width.round().clamp(1.0, f64::from(i32::MAX));
    // The value is finite and clamped to the complete positive i32 range.
    #[allow(clippy::cast_possible_truncation)]
    {
        rounded as i32
    }
}

fn model_heights_to_pixels(weights: &[f64], viewport_height: i32) -> Vec<i32> {
    if weights.is_empty() {
        return Vec::new();
    }
    let total_weight = weights.iter().sum::<f64>();
    let valid = total_weight.is_finite()
        && total_weight > 0.0
        && weights
            .iter()
            .all(|weight| weight.is_finite() && *weight > 0.0);
    let resolved_total = if valid {
        total_weight
    } else {
        small_count_as_f64(weights.len())
    };
    let spacing = i32::try_from(weights.len().saturating_sub(1)).unwrap_or(i32::MAX);
    let usable_height = viewport_height.saturating_sub(spacing).max(1);
    let mut assigned = 0_i32;
    weights
        .iter()
        .enumerate()
        .map(|(index, weight)| {
            let remaining = usable_height.saturating_sub(assigned).max(1);
            let height = if index + 1 == weights.len() {
                remaining
            } else {
                let resolved_weight = if valid { *weight } else { 1.0 };
                let share = f64::from(usable_height) * (resolved_weight / resolved_total);
                model_width_to_pixels(share).min(remaining)
            };
            assigned = assigned.saturating_add(height);
            height
        })
        .collect()
}

fn bounded_pane_viewport_height(
    scroll_height: i32,
    window_height: i32,
    chrome_height: i32,
    default_window_height: i32,
) -> i32 {
    let fallback = default_window_height.saturating_sub(52).max(200);
    let window_content_height = window_height.saturating_sub(chrome_height).max(1);
    match (scroll_height > 1, window_height > 1) {
        (true, true) => scroll_height.min(window_content_height),
        (true, false) => scroll_height,
        (false, true) => window_content_height,
        (false, false) => fallback,
    }
}

fn small_count_as_f64(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap_or(u32::MAX))
}

fn next_numeric_identity<'a>(ids: impl Iterator<Item = &'a str>, prefix: &str) -> usize {
    ids.filter_map(|id| id.strip_prefix(prefix)?.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn default_tmux_product() -> Result<crate::tmux_compat::TmuxCompatProduct, String> {
    crate::tmux_compat::TmuxCompatProduct::persistent(crate::tmux_store::TmuxStoreFile::new(
        crate::tmux_store::TmuxStoreFile::default_path()?,
    ))
}

fn restored_or_default_window(
    restored_window: Option<WindowRecipe>,
    fresh_window_id: &str,
) -> WindowRecipe {
    restored_window.unwrap_or_else(|| {
        default_window_recipe(
            fresh_window_id,
            std::env::current_dir()
                .ok()
                .and_then(|path| path.into_os_string().into_string().ok()),
        )
    })
}

fn apply_restored_window_size(window: &gtk::Window, template: &WindowRecipe) {
    let Some((width, height)) = validated_window_size(template.frame.as_ref()) else {
        return;
    };
    window.set_default_size(width, height);
    eprintln!(
        "zentty-linux: window-frame-restore-request id={} size={}x{} placement=compositor",
        template.id, width, height
    );
}

#[allow(clippy::cast_possible_truncation)] // Finite rounded values are range-checked below.
fn validated_window_size(frame: Option<&WindowFrame>) -> Option<(i32, i32)> {
    let frame = frame?;
    let width = frame.width.round();
    let height = frame.height.round();
    if !frame.x.is_finite()
        || !frame.y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || width < 320.0
        || height < 240.0
        || width > f64::from(i32::MAX)
        || height > f64::from(i32::MAX)
    {
        return None;
    }
    Some((width as i32, height as i32))
}

fn snapshot_window_frame(
    width: i32,
    height: i32,
    prior: Option<&WindowFrame>,
) -> Option<WindowFrame> {
    if width < 320 || height < 240 {
        return prior
            .filter(|frame| validated_window_size(Some(frame)).is_some())
            .cloned();
    }
    Some(WindowFrame {
        x: prior.map_or(0.0, |frame| frame.x),
        y: prior.map_or(0.0, |frame| frame.y),
        width: f64::from(width),
        height: f64::from(height),
        screen_x: prior.and_then(|frame| frame.screen_x),
        screen_y: prior.and_then(|frame| frame.screen_y),
        screen_width: prior.and_then(|frame| frame.screen_width),
        screen_height: prior.and_then(|frame| frame.screen_height),
    })
}

fn default_window_recipe(id: &str, working_directory: Option<String>) -> WindowRecipe {
    let (worklane_id, column_id, pane_id) = if id == "window-1" {
        (
            "worklane-1".to_owned(),
            "column-worklane-1".to_owned(),
            "pane-1".to_owned(),
        )
    } else {
        (
            format!("worklane-{id}"),
            format!("column-{id}"),
            format!("pane-{id}"),
        )
    };
    WindowRecipe {
        id: id.to_owned(),
        frame: None,
        worklanes: vec![WorklaneRecipe {
            id: worklane_id.clone(),
            title: None,
            next_pane_number: 2,
            focused_column_id: Some(column_id.clone()),
            columns: vec![ColumnRecipe {
                id: column_id,
                width: 1.0,
                focused_pane_id: Some(pane_id.clone()),
                last_focused_pane_id: Some(pane_id.clone()),
                pane_heights: vec![1.0],
                panes: vec![PaneRecipe {
                    id: pane_id,
                    custom_title: None,
                    title_seed: Some("shell".to_owned()),
                    working_directory,
                    last_activity_title: None,
                    last_run_command: None,
                }],
            }],
            color: None,
            bookmark_origin_id: None,
        }],
        active_worklane_id: Some(worklane_id),
    }
}

#[cfg(test)]
mod allocation_tests {
    use super::{
        TerminalGesture, bounded_pane_viewport_height, codex_terminal_gesture,
        default_window_recipe, focus_follow_should_apply, is_close_window_shortcut,
        model_heights_to_pixels, snapshot_window_frame, validated_window_size,
    };
    use gtk::gdk;
    use zentty_core::WindowFrame;

    #[test]
    fn pointer_focus_requires_current_generation_active_window_and_no_transient_ui() {
        assert!(focus_follow_should_apply(4, 4, true, false));
        assert!(!focus_follow_should_apply(3, 4, true, false));
        assert!(!focus_follow_should_apply(4, 4, false, false));
        assert!(!focus_follow_should_apply(4, 4, true, true));
    }

    #[test]
    fn default_pane_records_the_directory_in_which_its_real_child_starts() {
        let recipe = default_window_recipe("window-test", Some("/tmp/zentty-project".to_owned()));
        assert_eq!(
            recipe.worklanes[0].columns[0].panes[0]
                .working_directory
                .as_deref(),
            Some("/tmp/zentty-project")
        );
    }

    #[test]
    fn fresh_windows_namespace_workspace_and_pane_identities() {
        let first = default_window_recipe("window-1", Some("/tmp".to_owned()));
        let second = default_window_recipe("window-2", Some("/tmp".to_owned()));

        assert_eq!(first.worklanes[0].id, "worklane-1");
        assert_eq!(first.worklanes[0].columns[0].panes[0].id, "pane-1");
        assert_eq!(second.worklanes[0].id, "worklane-window-2");
        assert_eq!(second.worklanes[0].columns[0].panes[0].id, "pane-window-2");
    }

    #[test]
    fn restored_window_size_requires_finite_source_minimums() {
        let frame = WindowFrame {
            x: 12.0,
            y: 34.0,
            width: 999.6,
            height: 699.6,
            screen_x: None,
            screen_y: None,
            screen_width: None,
            screen_height: None,
        };
        assert_eq!(validated_window_size(Some(&frame)), Some((1000, 700)));
        assert_eq!(
            validated_window_size(Some(&WindowFrame {
                width: 320.0,
                height: 240.0,
                ..frame.clone()
            })),
            Some((320, 240))
        );

        for invalid in [
            WindowFrame {
                width: 319.0,
                ..frame.clone()
            },
            WindowFrame {
                height: 239.0,
                ..frame.clone()
            },
            WindowFrame {
                x: f64::NAN,
                ..frame.clone()
            },
            WindowFrame {
                y: f64::NAN,
                ..frame.clone()
            },
            WindowFrame {
                width: f64::INFINITY,
                ..frame.clone()
            },
            WindowFrame {
                height: f64::INFINITY,
                ..frame.clone()
            },
            WindowFrame {
                height: f64::NAN,
                ..frame.clone()
            },
            WindowFrame {
                width: f64::from(i32::MAX) + 1.0,
                ..frame.clone()
            },
            WindowFrame {
                height: f64::from(i32::MAX) + 1.0,
                ..frame.clone()
            },
        ] {
            assert_eq!(validated_window_size(Some(&invalid)), None);
        }
        assert_eq!(validated_window_size(None), None);
        assert_eq!(
            validated_window_size(Some(&WindowFrame {
                width: f64::from(i32::MAX),
                height: 700.0,
                ..frame.clone()
            })),
            Some((i32::MAX, 700))
        );
        assert_eq!(
            validated_window_size(Some(&WindowFrame {
                width: 1000.0,
                height: f64::from(i32::MAX),
                ..frame
            })),
            Some((1000, i32::MAX))
        );
    }

    #[test]
    fn window_snapshot_updates_size_without_inventing_or_discarding_position_metadata() {
        let prior = WindowFrame {
            x: 1721.0,
            y: 48.0,
            width: 1000.0,
            height: 700.0,
            screen_x: Some(1440.0),
            screen_y: Some(0.0),
            screen_width: Some(2560.0),
            screen_height: Some(1440.0),
        };

        let resized = snapshot_window_frame(1110, 730, Some(&prior)).unwrap();
        assert_eq!((resized.width, resized.height), (1110.0, 730.0));
        assert_eq!((resized.x, resized.y), (1721.0, 48.0));
        assert_eq!(resized.screen_width, Some(2560.0));

        let fresh = snapshot_window_frame(1000, 700, None).unwrap();
        assert_eq!((fresh.x, fresh.y), (0.0, 0.0));
        assert_eq!((fresh.width, fresh.height), (1000.0, 700.0));
        assert!(snapshot_window_frame(320, 240, None).is_some());
        assert!(snapshot_window_frame(1, 1, None).is_none());
        assert!(snapshot_window_frame(1, 700, None).is_none());
        assert!(snapshot_window_frame(1000, 1, None).is_none());
        assert_eq!(snapshot_window_frame(1, 1, Some(&prior)).unwrap(), prior);
    }

    #[test]
    fn pane_height_pixels_preserve_ratios_spacing_and_invalid_weight_fallback() {
        let golden = model_heights_to_pixels(&[0.618_033_988_75, 0.381_966_011_25], 648);
        let golden_content = golden.iter().sum::<i32>();
        assert_eq!(golden_content + 1, 648);
        assert!((f64::from(golden[0]) / f64::from(golden_content) - 0.618).abs() < 0.002);

        let equal = model_heights_to_pixels(&[1.0, 1.0, 1.0], 648);
        assert_eq!(equal.iter().sum::<i32>() + 2, 648);
        assert!(equal.windows(2).all(|pair| (pair[0] - pair[1]).abs() <= 1));

        assert_eq!(model_heights_to_pixels(&[0.0, f64::NAN], 101), [50, 50]);
        assert!(model_heights_to_pixels(&[], 648).is_empty());
    }

    #[test]
    fn pane_viewport_is_bounded_by_the_real_window_when_content_requests_expand() {
        assert_eq!(bounded_pane_viewport_height(656, 700, 38, 700), 656);
        assert_eq!(bounded_pane_viewport_height(1_475, 700, 38, 700), 662);
        assert_eq!(bounded_pane_viewport_height(1_200, 1_300, 38, 700), 1_200);
        assert_eq!(bounded_pane_viewport_height(0, 900, 38, 700), 862);
        assert_eq!(bounded_pane_viewport_height(1, 0, 0, 700), 648);
        assert_eq!(bounded_pane_viewport_height(0, 1, 0, 700), 648);
        assert_eq!(bounded_pane_viewport_height(0, 0, 0, 700), 648);
    }

    #[test]
    fn close_window_shortcut_is_exact_linux_ctrl_q() {
        assert!(is_close_window_shortcut(
            gdk::Key::q,
            gdk::ModifierType::CONTROL_MASK
        ));
        assert!(!is_close_window_shortcut(
            gdk::Key::q,
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK
        ));
        assert!(!is_close_window_shortcut(
            gdk::Key::w,
            gdk::ModifierType::CONTROL_MASK
        ));
    }

    #[test]
    fn terminal_lifecycle_gestures_require_the_source_exact_physical_chords() {
        assert_eq!(
            codex_terminal_gesture(gdk::Key::Return, gdk::ModifierType::empty()),
            Some(TerminalGesture::InputSubmitted)
        );
        assert_eq!(
            codex_terminal_gesture(gdk::Key::KP_Enter, gdk::ModifierType::LOCK_MASK),
            Some(TerminalGesture::InputSubmitted)
        );
        assert_eq!(
            codex_terminal_gesture(gdk::Key::Return, gdk::ModifierType::SHIFT_MASK),
            None
        );
        assert_eq!(
            codex_terminal_gesture(gdk::Key::c, gdk::ModifierType::CONTROL_MASK),
            Some(TerminalGesture::Interrupted)
        );
        assert_eq!(
            codex_terminal_gesture(
                gdk::Key::C,
                gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK
            ),
            None
        );
        assert_eq!(
            codex_terminal_gesture(gdk::Key::c, gdk::ModifierType::empty()),
            None
        );
    }
}
