use std::cell::{Ref, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::DerefMut;
use std::rc::Rc;
use std::sync::Mutex;
use eframe::egui;
use eframe::egui::{Context, ScrollArea, Ui, Window};

#[derive(Default)]
pub struct MegaWrapper {
    pub state: ShortKAppState,
}

#[derive(Default)]
pub struct ContainerLayouts {
    demos: Vec<ShortKContainer>,
    open: HashSet<String>,
}


pub struct ShortKAppState {
    pub pods: Vec<ShortKContainer>,
    pub filter_pod_prefix: String,
    pub conf_file_path: String,
    pub namespace: String,

    pub forwards: HashMap<String, bool>,

    pub log_windows: Vec<PopUp>,
    pub opened_windows: HashSet<String>,

    pub about_is_open: bool,
    pub containers: ContainerLayouts,

}

impl ShortKAppState {
    pub fn new(pods: Vec<ShortKContainer>) -> Self {
        Self {
            pods,
            filter_pod_prefix: "".to_string(),
            conf_file_path: "".to_string(),
            namespace: "".to_string(),
            forwards: HashMap::new(),
            log_windows: vec![],
            opened_windows: Default::default(),
            about_is_open: false,
            containers: Default::default(),
        }
    }

}

impl Default for ShortKAppState {
    fn default() -> Self {
        Self {
            pods: Vec::new(),
            filter_pod_prefix: "yyy".to_string(),
            conf_file_path: "zzz".to_string(),
            namespace: "xxx".to_string(),
            forwards: HashMap::new(),
            log_windows: vec![],
            opened_windows: Default::default(),
            about_is_open: false,
            containers: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct ShortKContainer {
    pub pod_name: String,
    //for internal purposes
    pub age: String,
    pub image: String,
    pub status: String,
    pub restarts: u32,
    pub ports: HashMap<String, u16>,
    pub config_map: HashMap<String, String>,
    pub secrets: HashMap<String, String>,
    pub forward_ons: bool,
    pub forward_on: RefCell<bool>,
    pub forward_onm: Mutex<bool>,
}

impl ShortKContainer {
    pub fn new(pod_name: String, age: String, image: String, status: String, restarts: u32, ports: HashMap<String, u16>, config_map: HashMap<String, String>,
               secrets: HashMap<String, String>) -> Self {
        Self { pod_name, age, image, status, restarts, ports, config_map, secrets, forward_ons: false, forward_on: RefCell::new(false), forward_onm: Mutex::new(false) }
    }
}


impl Default for ShortKContainer {
    fn default() -> Self {
        Self {
            pod_name: "".to_string(),
            age: "age".to_owned(),
            image: "image".to_owned(),
            status: "stub".to_owned(),
            restarts: 1945,
            ports: HashMap::new(),
            config_map: HashMap::new(),
            secrets: HashMap::new(),
            forward_ons: false,
            forward_on: RefCell::new(false),
            forward_onm: Mutex::new(false),
        }
    }
}

#[derive(Default)]
pub struct PopUp {
    pub id: String,
}

impl PopUp {
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    pub fn show(&mut self, ctx: &Context, open: &mut bool, title: String) {
        Window::new(title)
            .default_width(600.0)
            .default_height(400.0)
            .vscroll(true)
            .resizable(true)
            .open(open)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("asdasd".to_string()).small().weak());
                ui.add(egui::Separator::default().grow(8.0));
            });
    }
}
