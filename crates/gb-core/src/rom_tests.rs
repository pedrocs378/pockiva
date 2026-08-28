use std::fs;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use crate::{Clock, EmulatorCore, GameBoy};

struct FixedClock;

impl Clock for FixedClock {
    fn unix_seconds(&self) -> u64 {
        1_000_000
    }
}

enum RomSignal {
    BlarggSerial(&'static str),
    MooneyeRegisters([u8; 6]),
}

fn rom_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/roms/downloads")
}

fn mooneye(path: &str) -> PathBuf {
    rom_root().join("mooneye").join(path)
}

fn blargg(path: &str) -> PathBuf {
    rom_root().join("blargg").join(path)
}

fn run_rom(path: &Path, max_t_cycles: u64, signal: &RomSignal) -> Result<(), String> {
    let rom = fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut core = GameBoy::new(
        FixedClock,
        NonZeroU32::new(48_000).expect("non-zero sample rate"),
    );
    core.load_rom(&rom, None)
        .map_err(|error| error.to_string())?;
    run_core(&mut core, max_t_cycles, signal)
}

fn run_core(
    core: &mut GameBoy<FixedClock>,
    max_t_cycles: u64,
    signal: &RomSignal,
) -> Result<(), String> {
    let mut elapsed = 0_u64;
    while elapsed < max_t_cycles {
        let next = core
            .next_step_for_rom_test()
            .map_err(|error| error.to_string())?;
        if next == 0 {
            return Err("core stopped before producing a pass/fail signal".into());
        }
        if u64::from(next) > max_t_cycles - elapsed {
            break;
        }
        let (cycles, breakpoint) = core
            .run_one_for_rom_test()
            .map_err(|error| error.to_string())?;
        debug_assert_eq!(cycles, next);
        elapsed += u64::from(cycles);
        match signal {
            RomSignal::BlarggSerial(expected) => {
                let output = String::from_utf8_lossy(core.serial_output());
                if output.contains("Failed") {
                    return Err(format!(
                        "Blargg reported failure after {elapsed} T-cycles: {output}"
                    ));
                }
                if output.contains(expected) {
                    return Ok(());
                }
            }
            RomSignal::MooneyeRegisters(expected) if breakpoint => {
                let actual = core.diagnostic_registers();
                if actual == *expected {
                    return Ok(());
                }
                if actual == [0x42; 6] {
                    return Err(format!(
                        "Mooneye reported failure after {elapsed} T-cycles with registers {actual:02x?}"
                    ));
                }
            }
            RomSignal::MooneyeRegisters(_) => {}
        }
    }
    Err(format!(
        "ROM timed out at the exact {max_t_cycles} T-cycle bound"
    ))
}

fn synthetic_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0; 0x8000];
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
    rom[0x0147] = 0;
    rom[0x0148] = 0;
    rom[0x0149] = 0;
    rom[0x014d] = rom[0x0134..=0x014c]
        .iter()
        .fold(0_u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
    rom
}

#[test]
fn rom_signal_after_cycle_limit_is_rejected() {
    let mut core = GameBoy::new(
        FixedClock,
        NonZeroU32::new(48_000).expect("non-zero sample rate"),
    );
    core.load_rom(&synthetic_rom(&[0x40]), None)
        .expect("synthetic ROM loads");
    let expected = core.diagnostic_registers();

    assert_eq!(
        run_core(&mut core, 3, &RomSignal::MooneyeRegisters(expected)),
        Err("ROM timed out at the exact 3 T-cycle bound".into())
    );
}

#[test]
fn fetch_script_extracts_only_the_pinned_members() {
    let script = include_str!("../../../scripts/fetch-core-test-roms.sh");
    assert!(
        script.contains("tar -xJf \"$archive\" -C \"$extract_root\" -- \"${archive_members[@]}\"")
    );
    assert!(!script.contains("tar -xJf \"$archive\" -C \"$extract_root\"\n"));
}

macro_rules! mooneye_test {
    ($name:ident, $path:literal) => {
        #[test]
        #[ignore = "requires explicitly provisioned, checksum-verified ROM asset"]
        fn $name() {
            run_rom(
                &mooneye($path),
                50_000_000,
                &RomSignal::MooneyeRegisters([3, 5, 8, 13, 21, 34]),
            )
            .expect("Mooneye ROM passes");
        }
    };
}

mooneye_test!(rom_mooneye_bits_reg_f, "acceptance/bits/reg_f.gb");
mooneye_test!(rom_mooneye_instr_daa, "acceptance/instr/daa.gb");
mooneye_test!(rom_mooneye_ei_sequence, "acceptance/ei_sequence.gb");
mooneye_test!(rom_mooneye_if_ie_registers, "acceptance/if_ie_registers.gb");
mooneye_test!(rom_mooneye_timer_div_write, "acceptance/timer/div_write.gb");
mooneye_test!(
    rom_mooneye_timer_tima_reload,
    "acceptance/timer/tima_reload.gb"
);
mooneye_test!(rom_mooneye_oam_dma_basic, "acceptance/oam_dma/basic.gb");

#[test]
#[ignore = "requires explicitly provisioned, checksum-verified ROM asset"]
fn rom_blargg_cpu_instrs() {
    run_rom(
        &blargg("cpu_instrs/cpu_instrs.gb"),
        2_000_000_000,
        &RomSignal::BlarggSerial("Passed"),
    )
    .expect("Blargg CPU instructions pass");
}

#[test]
#[ignore = "requires explicitly provisioned, checksum-verified ROM asset"]
fn rom_blargg_instr_timing() {
    run_rom(
        &blargg("instr_timing/instr_timing.gb"),
        200_000_000,
        &RomSignal::BlarggSerial("Passed"),
    )
    .expect("Blargg instruction timing passes");
}
