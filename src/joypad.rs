#[derive(Debug, Clone, Copy)]
pub enum Button {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Up = 4,
    Down = 5,
    Left = 6,
    Right = 7,
}

pub struct Joypad {
    strobe: bool,
    button_index: u8,
    button_state: u8,
    // Turbo state
    turbo_a: bool,
    turbo_b: bool,
    turbo_counter: u8,
    pub turbo_rate: u8,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            strobe: false,
            button_index: 0,
            button_state: 0,
            turbo_a: false,
            turbo_b: false,
            turbo_counter: 0,
            turbo_rate: 2,
        }
    }

    pub fn set_turbo_a(&mut self, held: bool) {
        self.turbo_a = held;
        if !held {
            self.set_button(Button::A, false);
        }
    }

    pub fn set_turbo_b(&mut self, held: bool) {
        self.turbo_b = held;
        if !held {
            self.set_button(Button::B, false);
        }
    }

    /// Call once per frame to update turbo state
    pub fn update_turbo(&mut self) {
        self.turbo_counter = (self.turbo_counter + 1) % (self.turbo_rate * 2);
        let turbo_on = self.turbo_counter < self.turbo_rate;

        if self.turbo_a {
            self.set_button(Button::A, turbo_on);
        }
        if self.turbo_b {
            self.set_button(Button::B, turbo_on);
        }
    }

    pub fn write(&mut self, data: u8) {
        self.strobe = data & 1 == 1;
        if self.strobe {
            self.button_index = 0;
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.button_index > 7 {
            return 1;
        }
        let result = (self.button_state >> self.button_index) & 1;
        if !self.strobe {
            self.button_index += 1;
        }
        result
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        if pressed {
            self.button_state |= 1 << button as u8;
        } else {
            self.button_state &= !(1 << button as u8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strobe_reset() {
        let mut pad = Joypad::new();
        pad.set_button(Button::A, true);
        pad.set_button(Button::Start, true);
        pad.write(1); // Strobe on
        pad.write(0); // Strobe off
        assert_eq!(pad.read(), 1); // A
        assert_eq!(pad.read(), 0); // B
        assert_eq!(pad.read(), 0); // Select
        assert_eq!(pad.read(), 1); // Start
    }

    #[test]
    fn test_strobe_holds_a() {
        let mut pad = Joypad::new();
        pad.set_button(Button::A, true);
        pad.write(1); // Strobe on — keeps returning A
        assert_eq!(pad.read(), 1);
        assert_eq!(pad.read(), 1);
        assert_eq!(pad.read(), 1);
    }

    #[test]
    fn test_after_8_reads_returns_1() {
        let mut pad = Joypad::new();
        pad.write(1);
        pad.write(0);
        for _ in 0..8 {
            pad.read();
        }
        assert_eq!(pad.read(), 1); // After all buttons, returns 1
    }
}
