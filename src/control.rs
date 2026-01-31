pub struct Control {
    pub button: ControlButton,
    pub selected: bool,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ControlButton {
    Repeat,
    MinusTen,
    Previous,
    Play,
    Next,
    PlusTen,
    Shuffle
}