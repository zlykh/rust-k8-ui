use eframe::egui::{Context, TextBuffer, TextEdit, Window};

pub struct LogWindow {
    pub name: String,
    pub log_text: Option<String>,
}

impl LogWindow {
    pub fn new(name: String) -> Self {
        Self {
            name,
            log_text: None,
        }
    }

    pub fn draw(&mut self, ctx: &Context, open: &mut bool) {
        let window = Window::new(self.name.clone())
            .min_width(1000.0)
            .vscroll(true)
            .resizable(true)
            .open(open);
        window.show(ctx, |ui| {
            let mut text = if self.log_text.is_some() {
                self.log_text.as_ref().clone().unwrap().as_str()
            } else {
                "No logs pulled!".as_str()
            };
            ui.set_min_width(1000.0);
            let mut panel = TextEdit::multiline(&mut text);
            panel = panel.desired_width(1000.0);
            ui.add(panel);
            ui.separator();
        });
    }
}