use super::*;
use std::io::Write;
use std::sync::mpsc;
use std::time::Instant;

fn source(id: &str, pid: u32) -> PaneSource {
    PaneSource {
        window_id: "window".into(),
        window_title: "Window".into(),
        worklane_id: "lane".into(),
        worklane_title: "Lane".into(),
        pane_id: id.into(),
        pane_title: id.into(),
        status_text: None,
        root_pid: Some(pid),
        is_remote: false,
        is_worklane_active: true,
        working_directory: None,
    }
}

fn stat(pid: u32) -> String {
    format!("{pid} (worker) S 1 0 0 0 0 0 0 0 0 0 10 0 0 0 0 0 0 0 100 4096 1\n")
}

fn until(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "GTK/worker test deadline");
        glib::MainContext::default().iteration(false);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn blocked_scan_keeps_gtk_live_and_cannot_restore_stale_pane_ownership() {
    gtk::init().expect("private GTK display");
    let root = std::env::temp_dir().join(format!("task-manager-slow-proc-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    for pid in [100, 200, 300, 400] {
        std::fs::create_dir(root.join(pid.to_string())).unwrap();
        if pid != 100 {
            std::fs::write(root.join(pid.to_string()).join("stat"), stat(pid)).unwrap();
        }
    }
    let fifo = root.join("100/stat");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let (entered, started) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        let mut file = std::fs::OpenOptions::new().write(true).open(fifo).unwrap();
        entered.send(()).unwrap();
        if blocked.recv_timeout(Duration::from_secs(10)).is_ok() {
            file.write_all(stat(100).as_bytes()).unwrap();
        }
    });
    let live = Rc::new(RefCell::new(vec![
        source("closed", 100),
        source("moved", 200),
        source("respawned", 300),
        source("renamed", 400),
    ]));
    let sources = Rc::clone(&live);
    let controller = TaskManagerController::new(
        Rc::new(move || sources.borrow().clone()),
        Rc::new(|_, _, _| {}),
        Rc::new(|_, _, _| {}),
    );
    *controller.sampler.borrow_mut() = Some(ProcSampler::at(root.clone(), 100, 4096));
    let initial_trees = controller
        .sampler
        .borrow_mut()
        .as_mut()
        .unwrap()
        .sample(&[200, 300, 400]);
    controller.apply_sample(live.borrow().clone(), &initial_trees);
    TaskManagerController::show(&controller, None);
    until(|| started.try_recv().is_ok());
    assert!(controller.probe_in_flight.get());
    // Another refresh must not spawn another scan while this one is blocked.
    TaskManagerController::request_refresh(&controller);
    assert!(controller.sampler.borrow().is_none());
    let heartbeat = Rc::new(Cell::new(false));
    let beat = Rc::clone(&heartbeat);
    glib::idle_add_local_once(move || beat.set(true));
    until(|| heartbeat.get());
    {
        let mut live = live.borrow_mut();
        live.remove(0);
        live[0].worklane_id = "other-lane".into();
        live[0].worklane_title = "Moved lane".into();
        live[1].root_pid = Some(999);
        live[2].pane_title = "Current title".into();
        live.push(source("new", 500));
    }
    release.send(()).unwrap();
    until(|| !controller.probe_in_flight.get());
    writer.join().unwrap();
    let rows = controller.view.rows();
    controller.shutdown();
    std::fs::remove_dir_all(root).unwrap();
    assert!(!rows.iter().any(|row| row.source.pane_id == "closed"));
    assert_eq!(rows.len(), 4);
    for id in ["moved", "respawned", "new"] {
        let row = rows.iter().find(|row| row.source.pane_id == id).unwrap();
        assert!(row.processes.is_empty(), "stale metrics for {id}");
        assert_eq!(row.peak_memory_bytes, None, "stale peak for {id}");
    }
    let moved = rows
        .iter()
        .find(|row| row.source.pane_id == "moved")
        .unwrap();
    assert_eq!(moved.source.worklane_id, "other-lane");
    let renamed = rows
        .iter()
        .find(|row| row.source.pane_id == "renamed")
        .unwrap();
    assert_eq!(renamed.source.pane_title, "Current title");
    assert_eq!(renamed.processes.len(), 1);
    assert_eq!(renamed.processes[0].pid, 400);
}
