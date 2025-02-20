use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::convert::TryFrom;
use std::rc::Rc;
use crossbeam::channel::{bounded, Receiver, Sender, TryRecvError};
use eframe::{egui};
use eframe::egui::{Align, CentralPanel, Context, Layout, RichText, ScrollArea, SidePanel, TextBuffer, Ui};
use crate::k8ui::appstate::ShortKContainer;
use crate::k8ui::components::container_column::ContainerColumn;
use crate::k8ui::components::log_window::LogWindow;
use crate::k8ui::my_thread::{ApiCommand, ApiThread, ThreadMessage, UIData};
use crate::k8ui::my_thread::ThreadMessage::Api;
use crate::k8ui::ui_component_bus::{UiAction, UiBus};

pub const UNIFIED_HEIGHT: f32 = 800.0;

pub fn run_ui() -> Result<(), eframe::Error> {
    let (thread_sender, thread_receiver) = bounded(5);
    let (ui_sender, ui_receiver) = bounded(5);
    let (state_upstream_sender, state_upstream_receiver) = bounded(5);

    ApiThread::new(thread_receiver, ui_sender);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, UNIFIED_HEIGHT])
            .with_resizable(true),
        ..Default::default()
    };

    let conf_file_path = "aaa".to_string();
    let namespace = "xxx".to_string();
    let filter_pod_prefix = "yyy".to_string();

    let app = DemoApp::new(Some(conf_file_path), Some(namespace), Some(filter_pod_prefix),
                           thread_sender, ui_receiver, state_upstream_receiver, state_upstream_sender);
    let wrapper = AppStateWrapper::new(RefCell::new(app));

    eframe::run_native("Microscope", options, Box::new(|_|  Ok(Box::new(wrapper))))
}


impl eframe::App for AppStateWrapper {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.state.borrow_mut().update_state(ctx, _frame);
    }
}


pub struct AppStateWrapper {
    pub state: RefCell<DemoApp>,
}

impl AppStateWrapper {
    pub fn new(state: RefCell<DemoApp>) -> Self {
        Self { state }
    }
}

pub struct DemoApp {
    conf_file_path: Option<String>,
    namespace: Option<String>,
    filter_pod_prefix: Option<String>,

    thread_sender: Sender<ThreadMessage>,
    ui_receiver: Receiver<ThreadMessage>,

    state_upstream_receiver: Receiver<UiBus>,
    state_upstream_sender: Sender<UiBus>,

    pods: Vec<Rc<ShortKContainer>>,
    pinned: BTreeMap<String, ContainerColumn>,

    my_windows: Vec<LogWindow>,
    container_columns: Option<Vec<ContainerColumn>>,
    open: HashSet<String>,
}

impl DemoApp {
    pub fn new(conf_file_path: Option<String>, namespace: Option<String>, filter_pod_prefix: Option<String>,
               thread_sender: Sender<ThreadMessage>, ui_receiver: Receiver<ThreadMessage>,
               state_upstream_receiver: Receiver<UiBus>, state_upstream_sender: Sender<UiBus>, ) -> Self {
        let mut open = HashSet::new();

        Self {
            conf_file_path,
            namespace,
            filter_pod_prefix,

            thread_sender,
            ui_receiver,
            state_upstream_receiver,
            state_upstream_sender,

            pods: vec![],

            //test
            my_windows: vec![LogWindow::new("panels".to_owned())],
            container_columns: None,
            open,
            pinned: BTreeMap::new(),
        }
    }

    pub fn draw_pinned_right_panel(&mut self, ui: &mut Ui) {
        for (name, _) in self.pinned.iter() {
            ui.toggle_value(&mut false, name);
        }
    }

    pub fn draw_checkboxes(&mut self, ui: &mut Ui) {
        let Self { my_windows, open, pods, .. } = self;
        for demo in my_windows {
            let mut is_open = open.contains(demo.name.as_str());
            ui.toggle_value(&mut is_open, demo.name.clone());
            set_open(open, demo.name.as_str(), is_open);
        }
        for pod in pods {
            ui.toggle_value(&mut true, pod.pod_name.clone());
        }
    }


    pub fn redraw_windows_based_on_visibility(&mut self, ctx: &Context) {
        let Self { my_windows, open, .. } = self;
        for win in my_windows {
            let mut is_open = open.contains(win.name.as_str());
            win.draw(ctx, &mut is_open);
            set_open(open, win.name.as_str(), is_open);
        }
    }

    pub fn toggle_window(&mut self, ctx: &Context, win_name: &str) {
        let Self { open, .. } = self;
        let mut is_open = open.contains(win_name);
        set_open(open, win_name.to_owned().as_str(), !is_open);
    }

    pub fn redraw_columns(&mut self, ctx: &Context, ui: &mut Ui) {
        if let Some(columns) = &mut self.container_columns {
            for x in columns.iter_mut() {
                x.draw(ctx, ui);
                ui.separator();
            }
        } else {
            ui.spinner();
        }

        ui.separator();

        for (_, col) in self.pinned.iter_mut() {
            col.draw(ctx, ui);
            ui.separator();
        }
    }

    pub fn update_state(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        match self.state_upstream_receiver.try_recv() {
            Ok(msg) => match msg {
                UiBus::Action(action) => match action {
                    UiAction::PinColumn(col_name) => {
                        if let Some(columns) = &mut self.container_columns {
                            for (i, col) in columns.iter_mut().enumerate() {
                                if col.name == col_name {
                                    let removed = columns.remove(i);
                                    // self.pinned.insert(0, removed);
                                    self.pinned.insert(removed.name.clone(), removed);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                match e {
                    TryRecvError::Empty => {}
                    TryRecvError::Disconnected => println!("Error receive from ui bus {}", e),
                }
            }
        }


        match self.ui_receiver.try_recv() {
            Ok(msg) => match msg {
                Api(_) => println!("This shouldn't happen on ui"),

                ThreadMessage::Data(data) => match data {
                    UIData::Pods(new_pods) => {

                        self.pods = new_pods.into_iter()
                            .filter(|p| !self.pinned.contains_key(p.pod_name.as_str()))
                            .map(|data| Rc::new(data))
                            .collect();
                        self.container_columns = Some(self.pods.iter()
                            .map(|p| ContainerColumn::new(
                                Rc::clone(p),
                                self.thread_sender.clone(),
                                self.state_upstream_sender.clone()))
                            .collect());
                    }
                    UIData::Logs(lines) => {
                        if let Some(columns) = &mut self.container_columns {
                            for col in columns.iter_mut() {
                                col.log_text = Some(lines.join("\n"));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                match e {
                    TryRecvError::Empty => {}
                    TryRecvError::Disconnected => println!("Error receive from thread {}", e),
                }
            }
        }

        SidePanel::right("right")
            .resizable(false)
            .default_width(150.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                        ui.label(RichText::new("Pinned").heading());
                        self.draw_pinned_right_panel(ui);
                        ui.separator();
                        self.draw_checkboxes(ui);
                        ui.separator();

                        if ui.button("add custom").clicked() {
                            // self.demos.push(MyWindow::new("_Custom_".to_string(), self.stub.borrow()));
                        }

                        if ui.button("show custom").clicked() {
                            self.toggle_window(ctx, "_Custom_");
                        }

                        if ui.button("show window").clicked() {
                            self.toggle_window(ctx, "panels");
                        }
                        ui.separator();
                        ui.label("Active Forwards")
                    });
                });
            });

        //TOP
        CentralPanel::default().show(ctx, |ui| {
            ui.heading("My Pods");

            ui.horizontal(|ui| {
                let conf_path_label = ui.label("Kubeconfig path");
                if let Some(path) = &mut self.conf_file_path {
                    ui.text_edit_singleline(path).labelled_by(conf_path_label.id);
                }

                if ui.button("Reload Config").clicked() {
                    if let Some(config_path) = self.conf_file_path.as_ref() {
                        match self.thread_sender.try_send(ThreadMessage::Api(ApiCommand::ReloadClientWithConfig(config_path.clone()))) {
                            Ok(_) => println!("ok send"),
                            Err(_) => println!("err send"),
                        };
                    }
                };
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.set_max_width(600.0);

                let namespace_label = ui.label("Namespace");
                if let Some(namespace) = &mut self.namespace {
                    ui.text_edit_singleline(namespace).labelled_by(namespace_label.id);
                }

                ui.separator();

                let filter_pod_label = ui.label("Filter pod prefix");
                if let Some(prefix) = &mut self.filter_pod_prefix {
                    ui.text_edit_singleline(prefix).labelled_by(filter_pod_label.id);
                }

                if ui.button("Refresh").clicked() {
                    if let Some(namespace) = self.namespace.as_ref() {
                        match self.thread_sender.try_send(ThreadMessage::Api(ApiCommand::ReloadApisWithNameSpace(namespace.clone()))) {
                            Ok(_) => println!("ok send"),
                            Err(_) => println!("err send"),
                        };
                    }

                    //https://rust-unofficial.github.io/patterns/idioms/temporary-mutability.html
                    if let Some(prefix) = self.filter_pod_prefix.as_ref() {
                        let clone = prefix.clone();
                        self.pods.clear();
                        self.container_columns = None;
                        match self.thread_sender.try_send(ThreadMessage::Api(ApiCommand::PullPodsWithPrefix(clone))) {
                            Ok(_) => println!("ok send"),
                            Err(_) => println!("err send"),
                        };
                    }
                };
            });
            //TOP END

            ui.separator();

            ScrollArea::vertical().show(ui, |ui| {
                ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.redraw_columns(ctx, ui);
                    });
                });
            });
        });


        self.redraw_windows_based_on_visibility(ctx);
    }
}


fn set_open(open: &mut HashSet<String>, key: &str, is_open: bool) {
    if is_open {
        if !open.contains(key) {
            open.insert(key.to_owned());
        }
    } else {
        open.remove(key);
    }
}


