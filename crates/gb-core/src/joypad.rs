use std::collections::HashMap;

use crate::interrupts::{Interrupt, InterruptMask};
use crate::{Button, InputSourceId, JoypadState};

#[derive(Default)]
pub(crate) struct InputMatrix {
    sources: HashMap<InputSourceId, JoypadState>,
}

impl InputMatrix {
    pub(crate) fn set(&mut self, source: InputSourceId, state: JoypadState) {
        self.sources.insert(source, state);
    }
    pub(crate) fn clear(&mut self, source: InputSourceId) {
        self.sources.remove(&source);
    }
    pub(crate) fn effective(&self) -> JoypadState {
        self.sources
            .values()
            .copied()
            .fold(JoypadState::default(), JoypadState::union)
    }
}

pub(crate) struct JoypadRegister {
    select: u8,
    state: JoypadState,
    visible_low: u8,
}

impl Default for JoypadRegister {
    fn default() -> Self {
        Self {
            select: 0x30,
            state: JoypadState::default(),
            visible_low: 0x0f,
        }
    }
}

impl JoypadRegister {
    pub(crate) const fn read(&self) -> u8 {
        0xc0 | self.select | self.visible_low
    }

    pub(crate) fn write(&mut self, value: u8) -> InterruptMask {
        self.select = value & 0x30;
        self.update_visible()
    }

    pub(crate) fn set_state(&mut self, state: JoypadState) -> InterruptMask {
        self.state = state;
        self.update_visible()
    }

    fn update_visible(&mut self) -> InterruptMask {
        let old = self.visible_low;
        let mut visible = 0x0f;
        if self.select & 0x10 == 0 {
            for (bit, button) in [Button::Right, Button::Left, Button::Up, Button::Down]
                .into_iter()
                .enumerate()
            {
                if self.state.is_pressed(button) {
                    visible &= !(1 << bit);
                }
            }
        }
        if self.select & 0x20 == 0 {
            for (bit, button) in [Button::A, Button::B, Button::Select, Button::Start]
                .into_iter()
                .enumerate()
            {
                if self.state.is_pressed(button) {
                    visible &= !(1 << bit);
                }
            }
        }
        self.visible_low = visible;
        if old & !visible != 0 {
            InterruptMask::from_bits(Interrupt::Joypad.bit())
        } else {
            InterruptMask::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InputMatrix, JoypadRegister};
    use crate::interrupts::{Interrupt, InterruptMask};
    use crate::{Button, InputSourceId, JoypadState};

    fn state_with(button: Button) -> JoypadState {
        let mut state = JoypadState::default();
        state.press(button);
        state
    }

    #[test]
    fn clearing_one_source_preserves_other_buttons() {
        let mut inputs = InputMatrix::default();
        inputs.set(InputSourceId::new(1), state_with(Button::A));
        inputs.set(InputSourceId::new(2), state_with(Button::Left));
        inputs.clear(InputSourceId::new(2));
        assert!(inputs.effective().is_pressed(Button::A));
        assert!(!inputs.effective().is_pressed(Button::Left));
    }

    #[test]
    fn selected_key_transition_requests_joypad() {
        let mut register = JoypadRegister::default();
        register.write(0x10);
        assert_eq!(
            register.set_state(state_with(Button::A)),
            InterruptMask::from_bits(Interrupt::Joypad.bit())
        );
        assert_eq!(register.read() & 0x0f, 0x0e);
    }

    #[test]
    fn selecting_a_row_with_held_key_requests_joypad() {
        let mut register = JoypadRegister::default();
        register.set_state(state_with(Button::Right));
        assert_eq!(
            register.write(0x20),
            InterruptMask::from_bits(Interrupt::Joypad.bit())
        );
        assert_eq!(register.read(), 0xee);
    }
}
