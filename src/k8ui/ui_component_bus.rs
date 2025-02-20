#[derive(Debug)]
pub enum UiBus {
    Action(UiAction),
}

#[derive(Debug)]
pub enum UiAction {
    PinColumn(String),
}