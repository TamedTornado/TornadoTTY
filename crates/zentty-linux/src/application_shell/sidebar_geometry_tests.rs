use super::*;

#[test]
#[ignore = "requires a controlled GTK display"]
fn populated_sidebar_does_not_cover_terminal_at_minimum_width() {
    gtk::init().expect("GTK display");
    sidebar::install_styles();
    pane_controls::install_styles();
    let widgets = build_shell_widgets();
    let mut state = WorkspaceState::new("lane", "pane");
    state.set_worklane_title("lane", Some("TornadoTTY"));
    state.set_pane_title("pane", "Ready | Projects");
    let mut summaries = state.sidebar_summaries();
    summaries[0].pane_rows[0].working_directory = Some("/home/jason/Projects".into());
    summaries[0].pane_rows[0].agent_status = Some(zentty_core::PaneAgentStatus {
        session_id: "session".into(),
        parent_session_id: None,
        agent_name: "Codex".into(),
        phase: AgentPhase::NeedsInput,
        text: Some("Agent ready".into()),
        interaction: zentty_core::AgentInteractionKind::None,
        progress: None,
        tracked_pid: None,
        transcript_path: None,
        artifact_link: None,
        working_directory: None,
        agent_launch_snapshot: None,
        signal_origin: zentty_core::AgentSignalOrigin::ExplicitHook,
        signal_confidence: zentty_core::AgentSignalConfidence::Explicit,
        updated_at: 1,
    });
    sidebar::render(
        &widgets.sidebar,
        &widgets.window,
        &summaries,
        Default::default(),
        &[],
        &[],
        None,
        "window",
        None,
        Default::default(),
        None,
    );
    let _search = GlobalSearchView::attach(&widgets.sidebar);
    assert!(crate::activity_title::show_activity(
        widgets.sidebar.upcast_ref(),
        "zentty-pane-title-pane",
        "Working ⠧ Projects"
    ));
    assert!(crate::activity_title::show_stable(
        widgets.sidebar.upcast_ref(),
        "zentty-pane-title-pane",
        "Ready | Projects"
    ));
    widgets
        .sidebar_motion
        .apply(SidebarVisibilityMode::PinnedOpen);
    widgets.window.present();
    for (width, working) in [
        (420, true),
        (180, true),
        (180, false),
        (280, false),
        (180, false),
    ] {
        if working {
            crate::activity_title::show_activity(
                widgets.sidebar.upcast_ref(),
                "zentty-pane-title-pane",
                "Working ⠧ Projects",
            );
        } else {
            crate::activity_title::show_stable(
                widgets.sidebar.upcast_ref(),
                "zentty-pane-title-pane",
                "Ready | Projects",
            );
        }
        widgets.sidebar_scroll.set_width_request(width);
        widgets.sidebar_reservation.set_width_request(width);
        widgets.body.set_position(width);
        for _ in 0..30 {
            while glib::MainContext::default().pending() {
                glib::MainContext::default().iteration(false);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let sidebar = widgets
            .sidebar_scroll
            .compute_bounds(&widgets.body)
            .unwrap();
        let terminal = widgets.pane_scroll.compute_bounds(&widgets.body).unwrap();
        assert!(
            sidebar.x() + sidebar.width() <= terminal.x(),
            "sidebar covers terminal: requested={width}, sidebar={sidebar:?}, terminal={terminal:?}"
        );
        assert!(
            widgets.sidebar_scroll.width() <= width,
            "sidebar exceeds reservation: requested={width}, actual={}",
            widgets.sidebar_scroll.width()
        );
    }
    widgets.window.close();
}
