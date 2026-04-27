pub struct InputState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub turn_left: bool,
    pub turn_right: bool,
    pub fire: bool,
    pub use_action: bool,
    pub mouse_delta_x: f64,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            forward: false,
            backward: false,
            left: false,
            right: false,
            turn_left: false,
            turn_right: false,
            fire: false,
            use_action: false,
            mouse_delta_x: 0.0,
        }
    }
}
