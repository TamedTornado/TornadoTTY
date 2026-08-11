use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use gtk::{gio, glib};

use super::model::{PaneRow, PaneSource, format_cpu, format_memory, stable_hot_sort};
use super::{ProcSampler, TaskManagerView};

type SourcesProvider = Rc<dyn Fn() -> Vec<PaneSource>>;
type PaneAction = Rc<dyn Fn(&str, &str, &str)>;

pub(crate) struct TaskManagerController {
    view: TaskManagerView,
    sources: SourcesProvider,
    sampler: RefCell<Option<ProcSampler>>,
    probe_in_flight: Cell<bool>,
    previous_order: RefCell<Vec<String>>,
    refresh_source: RefCell<Option<glib::SourceId>>,
    shutting_down: Cell<bool>,
}

impl TaskManagerController {
    pub(crate) fn new(
        sources: SourcesProvider,
        focus_pane: PaneAction,
        close_pane: PaneAction,
    ) -> Result<Rc<Self>, String> {
        let controller = Rc::new(Self {
            view: TaskManagerView::new(focus_pane, close_pane),
            sources,
            sampler: RefCell::new(Some(ProcSampler::system()?)),
            probe_in_flight: Cell::new(false),
            previous_order: RefCell::new(Vec::new()),
            refresh_source: RefCell::new(None),
            shutting_down: Cell::new(false),
        });
        let weak = Rc::downgrade(&controller);
        let source = glib::timeout_add_local(Duration::from_millis(1500), move || {
            let Some(controller) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if controller.shutting_down.get() {
                return glib::ControlFlow::Break;
            }
            if controller.view.is_visible() {
                Self::request_refresh(&controller);
            }
            glib::ControlFlow::Continue
        });
        *controller.refresh_source.borrow_mut() = Some(source);
        Ok(controller)
    }

    pub(crate) fn show(controller: &Rc<Self>, parent: Option<&gtk::Window>) {
        controller.view.present(parent);
        Self::request_refresh(controller);
    }

    pub(crate) fn shutdown(&self) {
        if self.shutting_down.replace(true) {
            return;
        }
        if let Some(source) = self.refresh_source.borrow_mut().take() {
            source.remove();
        }
        self.view.close();
    }

    fn request_refresh(controller: &Rc<Self>) {
        if controller.shutting_down.get() || controller.probe_in_flight.replace(true) {
            return;
        }
        let sources = (controller.sources)();
        let root_pids = sources
            .iter()
            .filter_map(|source| source.root_pid)
            .collect::<Vec<_>>();
        let Some(mut sampler) = controller.sampler.borrow_mut().take() else {
            controller.probe_in_flight.set(false);
            return;
        };
        let weak = Rc::downgrade(controller);
        glib::spawn_future_local(async move {
            let worker_result = gio::spawn_blocking(move || {
                let trees = sampler.sample(&root_pids);
                (sampler, trees)
            })
            .await;
            let Some(controller) = weak.upgrade() else {
                return;
            };
            controller.probe_in_flight.set(false);
            let Ok((sampler, trees)) = worker_result else {
                eprintln!("zentty-linux: task-manager probe=worker-panic");
                return;
            };
            *controller.sampler.borrow_mut() = Some(sampler);
            if controller.shutting_down.get() || !controller.view.is_visible() {
                return;
            }
            controller.apply_sample(sources, &trees);
        });
    }

    fn apply_sample(
        &self,
        sources: Vec<PaneSource>,
        trees: &BTreeMap<u32, super::model::ProcessTree>,
    ) {
        let previous = self
            .view
            .rows()
            .into_iter()
            .map(|row| (row.source.stable_id(), row))
            .collect::<BTreeMap<_, _>>();
        let mut rows = sources
            .into_iter()
            .map(|source| {
                let stable_id = source.stable_id();
                let tree = source.root_pid.and_then(|pid| trees.get(&pid).cloned());
                PaneRow::project(source, tree, previous.get(&stable_id))
            })
            .collect::<Vec<_>>();
        stable_hot_sort(&mut rows, &self.previous_order.borrow());
        *self.previous_order.borrow_mut() = rows.iter().map(|row| row.source.stable_id()).collect();
        for row in &rows {
            eprintln!(
                "zentty-linux: task-manager-sample window={} worklane={} pane={} root={} processes={} cpu={} memory={} network=unavailable",
                row.source.window_id,
                row.source.worklane_id,
                row.source.pane_id,
                row.source
                    .root_pid
                    .map_or_else(|| "none".to_owned(), |pid| pid.to_string()),
                row.processes.len(),
                format_cpu(row.cpu_percent),
                format_memory(row.memory_bytes),
            );
        }
        self.view.apply_rows(rows);
    }
}

impl Drop for TaskManagerController {
    fn drop(&mut self) {
        if let Some(source) = self.refresh_source.get_mut().take() {
            source.remove();
        }
    }
}
