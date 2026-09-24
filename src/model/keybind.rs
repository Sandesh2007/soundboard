#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Keybind {
    pub modifiers: Vec<Modifier>,
    pub key: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Super,
}
