use std::collections::HashMap;
use std::rc::Rc;
use crossbeam::channel::{Sender};
use eframe::egui;
use eframe::egui::{Context, RichText, TextBuffer, TextEdit, Ui};
use crate::k8ui::appstate::ShortKContainer;
use crate::k8ui::components::log_window::LogWindow;
use crate::k8ui::my_thread::{ApiCommand, ThreadMessage};
use crate::k8ui::ui_component_bus::UiAction::PinColumn;
use crate::k8ui::ui_component_bus::UiBus;
use crate::k8ui::uinormdz::UNIFIED_HEIGHT;

pub struct ContainerColumn {
    pub name: String,
    pub container: Rc<ShortKContainer>,
    forwarded: bool,
    pub log_window: LogWindow,
    pub log_opened: bool,
    pub log_loading: bool,
    pub log_text: Option<String>,
    pub thread_sender: Sender<ThreadMessage>,
    pub state_upstream_sender: Sender<UiBus>,
    pub is_pinned: bool,
}

impl ContainerColumn {
    pub fn new(container: Rc<ShortKContainer>, thread_sender: Sender<ThreadMessage>, state_upstream_sender: Sender<UiBus>) -> Self {
        Self {
            name: container.pod_name.clone(),
            log_window: LogWindow::new(container.pod_name.clone() + " Logs"),
            container,
            forwarded: false,
            log_opened: false,
            log_text: None,
            log_loading: false,
            thread_sender,
            state_upstream_sender,
            is_pinned: false,
        }
    }

    fn draw_log_window(&mut self, ctx: &Context, option: Option<String>) {
        let Self { log_window, log_opened, .. } = self;

        if *log_opened {
            log_window.log_text = option;
            // println!("text... {:?}", &log_window.log_text);
            log_window.draw(ctx, log_opened);
        }
    }

    pub fn draw(&mut self, ctx: &Context, ui: &mut Ui) {
        self.draw_log_window(ctx, self.log_text.clone());

        if self.log_opened && !self.log_loading {
            self.log_loading = true;
            // println!("loading!");
        }

        ui.vertical(|ui| {
            ui.set_min_width(300.0);
            ui.set_min_height(UNIFIED_HEIGHT);
            ui.label(RichText::new(self.name.as_str()).heading());
            if ui.checkbox(&mut self.is_pinned, "Pin").changed() {
                match self.state_upstream_sender.try_send(UiBus::Action(PinColumn(self.name.clone()))) {
                    Ok(_) => println!("ok send"),
                    Err(_) => println!("err send"),
                };
            }

            let mut image_tag_split = self.container.image.split_once(":").unwrap();

            ui.horizontal(|ui| {
                let image_label = ui.label("Image");
                ui.text_edit_singleline(&mut image_tag_split.0.to_owned());//.labelled_by(image_label.id);
            });

            ui.horizontal(|ui| {
                let tag_label = ui.label("Tag");
                ui.text_edit_singleline(&mut image_tag_split.1.to_owned());//.labelled_by(tag_label.id);
            });

            ui.horizontal(|ui| {
                ui.label("Age");
                ui.text_edit_singleline(&mut self.container.age.clone());//.labelled_by(tag_label.id);
            });

            ui.horizontal(|ui| {
                ui.label("Status");
                ui.text_edit_singleline(&mut self.container.status.to_owned());//.labelled_by(tag_label.id);
            });

            ui.horizontal(|ui| {
                ui.label("Restarts");
                ui.text_edit_singleline(&mut self.container.restarts.clone().to_string());//.labelled_by(age_label.id);
            });

            if ui.button("Logs").clicked() {
                match self.thread_sender.try_send(ThreadMessage::Api(ApiCommand::PullLogsForPodName(self.name.clone()))) {
                    Ok(_) => println!("ok send"),
                    Err(_) => println!("err send"),
                };
                self.log_opened = !self.log_opened;
                if self.log_opened {}
            };

            let ports_label = ui.label("Ports");
            for (typ, num) in self.container.ports.iter() {
                if (ui.checkbox(&mut self.forwarded, format!("{}:{}", typ, num)).labelled_by(ports_label.id)).changed() {
                    if self.forwarded {
                        match self.thread_sender.try_send(ThreadMessage::Api(ApiCommand::PortForwardForPodNamePort(self.name.clone(), num.clone()))) {
                            Ok(_) => println!("ok send"),
                            Err(_) => println!("err send"),
                        };
                    }
                }
            }
            let mut theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(),ui.style());
            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                let mut layout_job =
                    egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, string, "rs");
                layout_job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(layout_job))
            };

            let secrets_label = ui.label("Secrets");
            ui.add(TextEdit::multiline(&mut join_multiline2(&self.container.secrets)).code_editor().layouter(&mut layouter)).labelled_by(secrets_label.id);

            let cfm_label = ui.label("Config Map");
            ui.add(TextEdit::multiline(&mut join_multiline2(&self.container.config_map)).code_editor().layouter(&mut layouter)).labelled_by(cfm_label.id);
        });
    }
}


fn join_multiline(map: &HashMap<String, u32>) -> String {
    return map.iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect::<Vec<String>>()
        .join("\n");
}

fn join_multiline2(map: &HashMap<String, String>) -> String {
    return map.iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect::<Vec<String>>()
        .join("\n");
}

