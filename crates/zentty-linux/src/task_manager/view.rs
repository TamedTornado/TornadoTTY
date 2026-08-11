use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

use gtk::gdk;
use gtk::prelude::*;

use super::model::{NetworkState, PaneRow, ProcessMetric, format_cpu, format_memory};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum NodeId {
    Pane(String),
    Process(String, u32),
}

struct RowWidgets {
    row: gtk::ListBoxRow,
    expander: gtk::Button,
    pane: gtk::Label,
    status: gtk::Label,
    cpu: gtk::Label,
    memory: gtk::Label,
    network: gtk::Label,
    hottest: gtk::Label,
    root_pid: gtk::Label,
}

type PaneAction = Rc<dyn Fn(&str, &str, &str)>;

struct ViewState {
    window: gtk::Window,
    search: gtk::SearchEntry,
    list: gtk::ListBox,
    focus_button: gtk::Button,
    copy_button: gtk::Button,
    end_button: gtk::Button,
    rows: RefCell<Vec<PaneRow>>,
    widgets: RefCell<BTreeMap<NodeId, RowWidgets>>,
    order: RefCell<Vec<NodeId>>,
    expanded: RefCell<BTreeSet<String>>,
    selected: RefCell<Option<NodeId>>,
    focus_pane: PaneAction,
    close_pane: PaneAction,
}

#[derive(Clone)]
pub(crate) struct TaskManagerView {
    state: Rc<ViewState>,
}

impl TaskManagerView {
    pub(crate) fn new(focus_pane: PaneAction, close_pane: PaneAction) -> Self {
        install_styles();
        let window = gtk::Window::builder()
            .title("Task Manager")
            .default_width(1120)
            .default_height(620)
            .build();
        window.set_size_request(880, 420);
        window.set_hide_on_close(true);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.add_css_class("task-manager");
        root.set_margin_start(14);
        root.set_margin_end(14);
        root.set_margin_top(14);
        root.set_margin_bottom(14);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search panes and processes"));
        search.set_hexpand(true);
        search.update_property(&[gtk::accessible::Property::Label("Task Manager Search")]);
        let focus_button = gtk::Button::with_mnemonic("_Focus Pane");
        focus_button.set_tooltip_text(Some("Focus Pane (Ctrl+Enter)"));
        let copy_button = gtk::Button::with_mnemonic("_Copy PID");
        copy_button.set_tooltip_text(Some("Copy PID (Ctrl+Shift+C)"));
        let end_button = gtk::Button::with_mnemonic("_End Task");
        end_button.set_tooltip_text(Some("End Task (Delete)"));
        toolbar.append(&search);
        toolbar.append(&focus_button);
        toolbar.append(&copy_button);
        toolbar.append(&end_button);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("task-manager-table");
        content.append(&header());
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.add_css_class("task-manager-list");
        content.append(&list);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&content));
        root.append(&toolbar);
        root.append(&scroll);
        window.set_child(Some(&root));

        let view = Self {
            state: Rc::new(ViewState {
                window,
                search,
                list,
                focus_button,
                copy_button,
                end_button,
                rows: RefCell::new(Vec::new()),
                widgets: RefCell::new(BTreeMap::new()),
                order: RefCell::new(Vec::new()),
                expanded: RefCell::new(BTreeSet::new()),
                selected: RefCell::new(None),
                focus_pane,
                close_pane,
            }),
        };
        view.install_handlers();
        view.update_buttons();
        view
    }

    pub(crate) fn present(&self, parent: Option<&gtk::Window>) {
        self.state.window.set_transient_for(parent);
        self.state.window.present();
        self.state.search.grab_focus();
        eprintln!("zentty-linux: task-manager=shown");
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.state.window.is_visible()
    }

    pub(crate) fn close(&self) {
        self.state.window.set_visible(false);
    }

    pub(crate) fn rows(&self) -> Vec<PaneRow> {
        self.state.rows.borrow().clone()
    }

    pub(crate) fn apply_rows(&self, rows: Vec<PaneRow>) {
        *self.state.rows.borrow_mut() = rows;
        self.render();
    }

    fn install_handlers(&self) {
        let view = self.clone();
        self.state
            .search
            .connect_search_changed(move |_| view.render());
        let search_keys = gtk::EventControllerKey::new();
        let view = self.clone();
        search_keys.connect_key_pressed(move |_, key, _, _| {
            if key != gdk::Key::Down {
                return gtk::glib::Propagation::Proceed;
            }
            if let Some(row) = view.state.list.row_at_index(0) {
                view.state.list.select_row(Some(&row));
                row.grab_focus();
            }
            gtk::glib::Propagation::Stop
        });
        self.state.search.add_controller(search_keys);
        let view = self.clone();
        self.state.list.connect_row_selected(move |_, row| {
            let selected = row.and_then(|row| {
                usize::try_from(row.index())
                    .ok()
                    .and_then(|index| view.state.order.borrow().get(index).cloned())
            });
            *view.state.selected.borrow_mut() = selected;
            view.update_buttons();
        });
        let view = self.clone();
        self.state.list.connect_row_activated(move |_, row| {
            let Some(NodeId::Pane(pane_id)) = usize::try_from(row.index())
                .ok()
                .and_then(|index| view.state.order.borrow().get(index).cloned())
            else {
                return;
            };
            if !view.state.expanded.borrow_mut().remove(&pane_id) {
                view.state.expanded.borrow_mut().insert(pane_id);
            }
            view.render();
        });
        let view = self.clone();
        self.state.focus_button.connect_clicked(move |_| {
            if let Some(row) = view.selected_pane() {
                (view.state.focus_pane)(
                    &row.source.window_id,
                    &row.source.worklane_id,
                    &row.source.pane_id,
                );
            }
        });
        let view = self.clone();
        self.state.end_button.connect_clicked(move |_| {
            if let Some(row) = view.selected_pane() {
                (view.state.close_pane)(
                    &row.source.window_id,
                    &row.source.worklane_id,
                    &row.source.pane_id,
                );
            }
        });
        let view = self.clone();
        self.state.copy_button.connect_clicked(move |_| {
            let Some(pid) = view.selected_pid() else {
                eprintln!("zentty-linux: task-manager copy-pid=unavailable-no-selection");
                return;
            };
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(&pid.to_string());
                eprintln!("zentty-linux: task-manager copy-pid={pid}");
            }
        });
        self.install_window_keys();
    }

    fn install_window_keys(&self) {
        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let window = self.state.window.clone();
        let search = self.state.search.clone();
        let focus_button = self.state.focus_button.clone();
        let copy_button = self.state.copy_button.clone();
        let end_button = self.state.end_button.clone();
        key.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gdk::Key::Escape
                || (key == gdk::Key::w && modifiers.contains(gdk::ModifierType::CONTROL_MASK))
            {
                window.set_visible(false);
                return gtk::glib::Propagation::Stop;
            }
            if matches!(key, gdk::Key::f | gdk::Key::F)
                && modifiers.contains(gdk::ModifierType::CONTROL_MASK)
            {
                search.grab_focus();
                return gtk::glib::Propagation::Stop;
            }
            if key == gdk::Key::Return && modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                focus_button.emit_clicked();
                return gtk::glib::Propagation::Stop;
            }
            if matches!(key, gdk::Key::c | gdk::Key::C)
                && modifiers
                    .contains(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK)
            {
                eprintln!("zentty-linux: task-manager shortcut=copy-pid");
                copy_button.emit_clicked();
                return gtk::glib::Propagation::Stop;
            }
            if key == gdk::Key::Delete {
                end_button.emit_clicked();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        self.state.window.add_controller(key);
    }

    fn render(&self) {
        let query = self.state.search.text();
        let rows = self
            .state
            .rows
            .borrow()
            .iter()
            .filter(|row| row.matches(&query))
            .cloned()
            .collect::<Vec<_>>();
        let expanded = self.state.expanded.borrow().clone();
        let mut order = Vec::new();
        for row in &rows {
            let stable_id = row.source.stable_id();
            order.push(NodeId::Pane(stable_id.clone()));
            if expanded.contains(&stable_id) {
                order.extend(
                    row.processes
                        .iter()
                        .map(|process| NodeId::Process(stable_id.clone(), process.pid)),
                );
            }
        }
        let selected = self.state.selected.borrow().clone();
        while let Some(child) = self.state.list.first_child() {
            self.state.list.remove(&child);
        }
        {
            let mut widgets = self.state.widgets.borrow_mut();
            let valid = rows
                .iter()
                .flat_map(|row| {
                    let stable_id = row.source.stable_id();
                    std::iter::once(NodeId::Pane(stable_id.clone())).chain(
                        row.processes
                            .iter()
                            .map(move |process| NodeId::Process(stable_id.clone(), process.pid)),
                    )
                })
                .collect::<BTreeSet<_>>();
            widgets.retain(|id, _| valid.contains(id));
            for (index, id) in order.iter().enumerate() {
                let row_widgets = widgets
                    .entry(id.clone())
                    .or_insert_with(|| make_row(Rc::downgrade(&self.state), id.clone()));
                update_row(row_widgets, id, &rows, &expanded);
                self.state.list.insert(
                    &row_widgets.row,
                    i32::try_from(index).expect("bounded task-manager row count fits i32"),
                );
            }
        }
        self.state.order.borrow_mut().clone_from(&order);
        let selected = selected.filter(|selected| order.contains(selected));
        self.state.selected.borrow_mut().clone_from(&selected);
        if let Some(index) =
            selected.and_then(|selected| order.iter().position(|id| id == &selected))
            && let Some(row) = self.state.list.row_at_index(
                i32::try_from(index).expect("bounded task-manager row count fits i32"),
            )
        {
            self.state.list.select_row(Some(&row));
        }
        self.update_buttons();
        eprintln!(
            "zentty-linux: task-manager rows={} visible={} query={:?}",
            self.state.rows.borrow().len(),
            order.len(),
            query.as_str()
        );
    }

    fn selected_pane(&self) -> Option<PaneRow> {
        let pane_id = match self.state.selected.borrow().as_ref()? {
            NodeId::Pane(pane_id) | NodeId::Process(pane_id, _) => pane_id.clone(),
        };
        self.state
            .rows
            .borrow()
            .iter()
            .find(|row| row.source.stable_id() == pane_id)
            .cloned()
    }

    fn selected_pid(&self) -> Option<u32> {
        match self.state.selected.borrow().as_ref()? {
            NodeId::Pane(pane_id) => self
                .state
                .rows
                .borrow()
                .iter()
                .find(|row| &row.source.stable_id() == pane_id)
                .and_then(|row| row.source.root_pid),
            NodeId::Process(_, pid) => Some(*pid),
        }
    }

    fn update_buttons(&self) {
        self.state
            .focus_button
            .set_sensitive(self.selected_pane().is_some());
        self.state
            .end_button
            .set_sensitive(self.selected_pane().is_some());
        self.state
            .copy_button
            .set_sensitive(self.selected_pid().is_some());
    }
}

fn header() -> gtk::Grid {
    let grid = gtk::Grid::new();
    grid.add_css_class("task-manager-header");
    for (column, title) in [
        "Pane",
        "Status",
        "CPU",
        "Memory",
        "Network",
        "Hottest Process",
        "Root PID",
    ]
    .into_iter()
    .enumerate()
    {
        let label = gtk::Label::new(Some(title));
        label.set_xalign(if (2..=4).contains(&column) || column == 6 {
            1.0
        } else {
            0.0
        });
        label.set_width_chars(column_width(column));
        grid.attach(
            &label,
            i32::try_from(column).expect("task-manager has seven columns"),
            0,
            1,
            1,
        );
    }
    grid
}

fn make_row(state: Weak<ViewState>, id: NodeId) -> RowWidgets {
    let row = gtk::ListBoxRow::new();
    let grid = gtk::Grid::new();
    grid.set_column_spacing(8);
    let expander = gtk::Button::new();
    expander.add_css_class("task-manager-expander");
    let pane_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    pane_box.append(&expander);
    let pane = gtk::Label::new(None);
    pane.set_xalign(0.0);
    pane.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    pane_box.append(&pane);
    grid.attach(&pane_box, 0, 0, 1, 1);
    let labels = (1..7)
        .map(|column| {
            let label = gtk::Label::new(None);
            label.set_xalign(if (2..=4).contains(&column) || column == 6 {
                1.0
            } else {
                0.0
            });
            label.set_width_chars(column_width(column));
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            grid.attach(
                &label,
                i32::try_from(column).expect("task-manager has seven columns"),
                0,
                1,
                1,
            );
            label
        })
        .collect::<Vec<_>>();
    row.set_child(Some(&grid));
    if let NodeId::Pane(pane_id) = id {
        expander.connect_clicked(move |_| {
            let Some(state) = state.upgrade() else { return };
            if !state.expanded.borrow_mut().remove(&pane_id) {
                state.expanded.borrow_mut().insert(pane_id.clone());
            }
            TaskManagerView { state }.render();
        });
    }
    RowWidgets {
        row,
        expander,
        pane,
        status: labels[0].clone(),
        cpu: labels[1].clone(),
        memory: labels[2].clone(),
        network: labels[3].clone(),
        hottest: labels[4].clone(),
        root_pid: labels[5].clone(),
    }
}

fn update_row(widgets: &RowWidgets, id: &NodeId, rows: &[PaneRow], expanded: &BTreeSet<String>) {
    match id {
        NodeId::Pane(pane_id) => {
            let Some(row) = rows.iter().find(|row| &row.source.stable_id() == pane_id) else {
                return;
            };
            widgets.row.remove_css_class("task-manager-process-row");
            widgets.expander.set_visible(!row.processes.is_empty());
            widgets.expander.set_label(if expanded.contains(pane_id) {
                "▾"
            } else {
                "▸"
            });
            widgets.pane.set_text(&row.source.pane_title);
            widgets.status.set_text(row.status_text());
            widgets.cpu.set_text(&format_cpu(row.cpu_percent));
            widgets.memory.set_text(&format_memory(row.memory_bytes));
            widgets.network.set_text(match &row.network_state {
                NetworkState::Unavailable(_) => "-",
            });
            widgets.hottest.set_text(
                row.hottest_process
                    .as_ref()
                    .map_or("", |process| &process.name),
            );
            widgets.root_pid.set_text(
                &row.source
                    .root_pid
                    .map_or_else(|| "-".to_owned(), |pid| pid.to_string()),
            );
            widgets
                .row
                .update_property(&[gtk::accessible::Property::Label(&format!(
                    "Pane {}, status {}, CPU {}, memory {}, root PID {}",
                    row.source.pane_title,
                    row.status_text(),
                    format_cpu(row.cpu_percent),
                    format_memory(row.memory_bytes),
                    row.source
                        .root_pid
                        .map_or_else(|| "unavailable".to_owned(), |pid| pid.to_string())
                ))]);
        }
        NodeId::Process(pane_id, pid) => {
            let Some(process) = rows
                .iter()
                .find(|row| &row.source.stable_id() == pane_id)
                .and_then(|row| row.processes.iter().find(|process| process.pid == *pid))
            else {
                return;
            };
            update_process_row(widgets, process);
        }
    }
}

fn update_process_row(widgets: &RowWidgets, process: &ProcessMetric) {
    widgets.row.add_css_class("task-manager-process-row");
    widgets.expander.set_visible(false);
    widgets.pane.set_text(&process.name);
    widgets.status.set_text(&format!("PID {}", process.pid));
    widgets.cpu.set_text(&format_cpu(Some(process.cpu_percent)));
    widgets
        .memory
        .set_text(&format_memory(Some(process.memory_bytes)));
    widgets.network.set_text("-");
    widgets.hottest.set_text("");
    widgets.root_pid.set_text(&process.pid.to_string());
    widgets
        .row
        .update_property(&[gtk::accessible::Property::Label(&format!(
            "Process {}, PID {}, CPU {}, memory {}",
            process.name,
            process.pid,
            format_cpu(Some(process.cpu_percent)),
            format_memory(Some(process.memory_bytes))
        ))]);
}

const fn column_width(column: usize) -> i32 {
    match column {
        0 => 24,
        1 => 15,
        3 => 12,
        4 => 10,
        5 => 18,
        _ => 9,
    }
}

fn install_styles() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".task-manager { background: #17191d; color: #e7e9ee; }\n\
         .task-manager-table { background: #20242b; border: 1px solid #48505d; border-radius: 7px; min-width: 980px; }\n\
         .task-manager-header { background: #292e36; border-bottom: 1px solid #48505d; padding: 8px; font-weight: 700; }\n\
         .task-manager-list { background: #20242b; }\n\
         .task-manager-list row { padding: 6px 8px; border-bottom: 1px solid #303640; }\n\
         .task-manager-list row:selected { background: #094771; color: #ffffff; }\n\
         .task-manager-process-row { background: #1c2026; color: #c5cad3; }\n\
         .task-manager-expander { min-width: 22px; min-height: 20px; padding: 0; background: transparent; border: 0; box-shadow: none; }",
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
