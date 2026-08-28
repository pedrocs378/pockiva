use std::num::NonZeroU32;

use crate::DMG_CLOCK_HZ;

#[derive(Debug, Clone)]
pub(crate) struct Mixer {
    coefficient: f64,
    left_capacitor: f64,
    right_capacitor: f64,
}

impl Mixer {
    pub(crate) fn new(sample_rate: NonZeroU32) -> Self {
        let coefficient =
            0.999_958_f64.powf(f64::from(DMG_CLOCK_HZ) / f64::from(sample_rate.get()));
        Self {
            coefficient,
            left_capacitor: 0.0,
            right_capacitor: 0.0,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn mix_with_dacs(
        &mut self,
        samples: [u8; 4],
        dacs: [bool; 4],
        nr50: u8,
        nr51: u8,
    ) -> [f32; 2] {
        let analog: [f64; 4] = std::array::from_fn(|index| {
            if dacs[index] {
                f64::from(samples[index]).mul_add(1.0 / 7.5, -1.0)
            } else {
                0.0
            }
        });
        let mut left = 0.0;
        let mut right = 0.0;
        for (channel, value) in analog.into_iter().enumerate() {
            if nr51 & (1 << channel) != 0 {
                right += value;
            }
            if nr51 & (1 << (channel + 4)) != 0 {
                left += value;
            }
        }
        left = left / 4.0 * f64::from(((nr50 >> 4) & 7) + 1) / 8.0;
        right = right / 4.0 * f64::from((nr50 & 7) + 1) / 8.0;
        let left = self.high_pass_left(left).clamp(-1.0, 1.0) as f32;
        let right = self.high_pass_right(right).clamp(-1.0, 1.0) as f32;
        [left, right]
    }

    fn high_pass_left(&mut self, input: f64) -> f64 {
        let output = input - self.left_capacitor;
        self.left_capacitor = input - output * self.coefficient;
        output
    }

    fn high_pass_right(&mut self, input: f64) -> f64 {
        let output = input - self.right_capacitor;
        self.right_capacitor = input - output * self.coefficient;
        output
    }

    pub(crate) fn reset(&mut self) {
        self.left_capacitor = 0.0;
        self.right_capacitor = 0.0;
    }
}
