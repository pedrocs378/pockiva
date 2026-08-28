use std::cell::Cell;
use std::num::NonZeroU32;

use gb_core::{Clock, CoreError, EmulatorCore, GameBoy};

struct TestClock(Cell<u64>);

impl Clock for TestClock {
    fn unix_seconds(&self) -> u64 {
        self.0.get()
    }
}

fn test_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0; 0x8000];
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
    rom[0x0134..0x013b].copy_from_slice(b"PED-35 ");
    rom[0x0147] = 0;
    rom[0x0148] = 0;
    rom[0x0149] = 0;
    rom[0x014d] = rom[0x0134..=0x014c]
        .iter()
        .fold(0_u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
    rom
}

fn core() -> GameBoy<TestClock> {
    GameBoy::new(
        TestClock(Cell::new(1_000)),
        NonZeroU32::new(48_000).expect("non-zero sample rate"),
    )
}

fn assert_send<T: Send>() {}
fn assert_emulator_thread_bound<T: EmulatorCore + Send>() {}

#[test]
fn concrete_core_can_move_to_the_desktop_emulation_thread() {
    assert_send::<GameBoy<TestClock>>();
    assert_emulator_thread_bound::<GameBoy<TestClock>>();
}

#[test]
fn unloaded_operations_are_typed_and_audio_retains_sample_rate() {
    let mut core = core();
    assert_eq!(core.reset(), Err(CoreError::NotLoaded));
    assert_eq!(core.run_cycles(4), Err(CoreError::NotLoaded));
    assert_eq!(core.battery_state(), None);
    assert_eq!(core.drain_audio().sample_rate().get(), 48_000);
    assert_eq!(core.take_frame(), None);
}

#[test]
fn run_cycles_stops_at_instruction_boundary_without_exceeding_budget() {
    let mut core = core();
    core.load_rom(&test_rom(&[0x00, 0x01, 0x34, 0x12]), None)
        .expect("ROM loads");
    assert_eq!(
        core.run_cycles(15).expect("run succeeds").cycles_executed(),
        4
    );
    assert_eq!(
        core.run_cycles(12).expect("run succeeds").cycles_executed(),
        12
    );
    assert_eq!(
        core.run_cycles(3)
            .expect("small run succeeds")
            .cycles_executed(),
        0
    );
}

#[test]
fn four_cycle_budget_executes_halt() {
    let mut core = core();
    core.load_rom(&test_rom(&[0x76]), None).expect("ROM loads");
    assert_eq!(
        core.run_cycles(4).expect("HALT executes").cycles_executed(),
        4
    );
}

#[test]
fn failed_replacement_leaves_loaded_machine_runnable() {
    let mut core = core();
    core.load_rom(&test_rom(&[0x00]), None).expect("ROM loads");
    assert!(core.load_rom(&[0; 16], None).is_err());
    assert_eq!(
        core.run_cycles(4)
            .expect("old ROM remains runnable")
            .cycles_executed(),
        4
    );
}

#[test]
fn reset_restores_post_boot_execution_state() {
    let mut core = core();
    core.load_rom(&test_rom(&[0x00, 0x00]), None)
        .expect("ROM loads");
    core.run_cycles(8).expect("run succeeds");
    core.reset().expect("reset succeeds");
    assert_eq!(
        core.run_cycles(4).expect("run succeeds").cycles_executed(),
        4
    );
}
