use super::*;
use crate::pane_controls::PaneFrame;

#[test]
#[ignore = "requires a controlled GTK display"]
fn stacked_panes_do_not_expand_a_new_neighbor_beyond_the_viewport() {
    gtk::init().expect("GTK display");
    install_shell_styles();
    let widgets = build_shell_widgets();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 1);
    column.set_vexpand(true);
    let frames = (0..3)
        .map(|index| {
            let terminal = gtk::DrawingArea::new();
            terminal.set_size_request(80, 20);
            PaneFrame::new(
                &format!("pane-{index}"),
                terminal.upcast_ref(),
                |_| {},
                |_| {},
            )
        })
        .collect::<Vec<_>>();
    column.append(frames[0].widget());
    widgets.pane_box.append(&column);
    widgets.window.present();
    settle_layout();
    let original_height = widgets.pane_scroll.height();

    // Stack panes, reduce the available content area, then open a full-height
    // neighbor. This isolates minimum-size feedback; it is not a reproduction
    // of the original desktop dock/compositor event.
    column.append(frames[1].widget());
    widgets.window.set_default_size(1000, 600);
    for _ in 0..4 {
        let height = bounded_pane_viewport_height(
            widgets.pane_scroll.height(),
            widgets.window.height(),
            widgets.chrome.widget().height(),
            widgets.window.default_height(),
        );
        for (frame, height) in frames
            .iter()
            .zip(model_heights_to_pixels(&[1.0, 1.0], height))
        {
            frame.widget().set_height_request(height);
            frame.widget().set_vexpand(false);
        }
        settle_layout();
    }
    widgets.pane_box.append(frames[2].widget());
    settle_layout();
    let viewport = widgets.pane_scroll.compute_bounds(&widgets.window).unwrap();
    let neighbor = frames[2].widget().compute_bounds(&widgets.window).unwrap();
    let window_height = widgets.window.height();
    widgets.window.close();
    assert!(
        f64::from(viewport.y() + viewport.height()) <= f64::from(window_height),
        "viewport exceeds window: original={original_height}, window={window_height}, viewport={viewport:?}, neighbor={neighbor:?}"
    );
    assert!(
        f64::from(neighbor.y() + neighbor.height()) <= f64::from(window_height),
        "new pane bottom is clipped: window={window_height}, neighbor={neighbor:?}"
    );
}

fn settle_layout() {
    for _ in 0..30 {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
