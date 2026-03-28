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
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            strobe: false,
            button_index: 0,
            button_state: 0,
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
