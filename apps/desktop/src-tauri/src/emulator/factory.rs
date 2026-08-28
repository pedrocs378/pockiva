use std::num::NonZeroU32;
use std::time::{SystemTime, UNIX_EPOCH};

use gb_core::{Clock, GameBoy};

use super::runtime::{CoreFactory, RuntimeCore};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn unix_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GameBoyCoreFactory {
    sample_rate: NonZeroU32,
}

impl GameBoyCoreFactory {
    #[must_use]
    pub(crate) const fn new(sample_rate: NonZeroU32) -> Self {
        Self { sample_rate }
    }
}

impl CoreFactory for GameBoyCoreFactory {
    fn create(&self) -> Box<dyn RuntimeCore> {
        Box::new(GameBoy::new(SystemClock, self.sample_rate))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use gb_core::{MapperKind, T_CYCLES_PER_M_CYCLE};

    use super::GameBoyCoreFactory;
    use crate::emulator::runtime::CoreFactory;

    fn synthetic_valid_rom(program: &[u8]) -> Vec<u8> {
        let mut rom = vec![0_u8; 32 * 1024];
        rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
        rom[0x0134..0x013a].copy_from_slice(b"PED-39");
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom[0x014d] = rom[0x0134..=0x014c]
            .iter()
            .fold(0_u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
        rom
    }

    #[test]
    fn factory_constructs_a_real_core_at_the_negotiated_sample_rate() {
        let rate = NonZeroU32::new(48_000).expect("non-zero rate");
        let factory = GameBoyCoreFactory::new(rate);
        let mut core = factory.create();

        let metadata = core
            .load_rom(&synthetic_valid_rom(&[0x00, 0x76]), None)
            .expect("real ROM loads");
        assert_eq!(metadata.mapper, MapperKind::RomOnly);
        assert!(
            core.run_cycles(4 * T_CYCLES_PER_M_CYCLE)
                .expect("real core advances")
                .cycles_executed()
                > 0
        );
        assert_eq!(core.drain_audio().sample_rate(), rate);
    }
}
