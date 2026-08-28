//! Local-only checksum-gated Blargg compatibility harness.

use std::fs;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{Clock, EmulatorCore, GameBoy};

const EXPECTED_SHA256: &str = "c34e740664eb14b42c39750434e3e105fc92d774a98fb671594a48e972401630";
const MAX_T_CYCLES: u64 = 2_000_000_000;

struct FixedClock;

impl Clock for FixedClock {
    fn unix_seconds(&self) -> u64 {
        1_000_000
    }
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn verify_asset(path: &Path, expected_sha256: &str) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "missing local-only Blargg asset {}: {error}",
            path.display()
        )
    })?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != expected_sha256 {
        return Err(format!(
            "Blargg asset checksum mismatch for {}: expected {expected_sha256}, got {actual}",
            path.display()
        ));
    }
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryVerdict {
    Pending,
    Passed,
    Failed(u8),
}

fn memory_verdict(signature: [u8; 3], status: u8, output: &str) -> MemoryVerdict {
    if signature != [0xde, 0xb0, 0x61] {
        return MemoryVerdict::Pending;
    }
    if status == 0 && output.contains("Passed") {
        MemoryVerdict::Passed
    } else if status != 0 && status != 0x80 {
        MemoryVerdict::Failed(status)
    } else {
        MemoryVerdict::Pending
    }
}

fn diagnostic_text(core: &GameBoy<FixedClock>) -> String {
    let bytes = (0..4_096_u16)
        .map(|offset| core.diagnostic_read(0xa004 + offset))
        .take_while(|byte| *byte != 0)
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn run_dmg_sound(path: &Path, expected_sha256: &str, max_t_cycles: u64) -> Result<(), String> {
    let rom = verify_asset(path, expected_sha256)?;
    let mut core = GameBoy::new(
        FixedClock,
        NonZeroU32::new(48_000).expect("non-zero sample rate"),
    );
    core.load_rom(&rom, None)
        .map_err(|error| error.to_string())?;

    let mut elapsed = 0_u64;
    while elapsed < max_t_cycles {
        let next = core
            .next_step_for_rom_test()
            .map_err(|error| error.to_string())?;
        if next == 0 || u64::from(next) > max_t_cycles - elapsed {
            break;
        }
        let (cycles, _) = core
            .run_one_for_rom_test()
            .map_err(|error| error.to_string())?;
        elapsed += u64::from(cycles);

        let signature = [
            core.diagnostic_read(0xa001),
            core.diagnostic_read(0xa002),
            core.diagnostic_read(0xa003),
        ];
        let status = core.diagnostic_read(0xa000);
        let memory = if signature == [0xde, 0xb0, 0x61] && status != 0x80 {
            diagnostic_text(&core)
        } else {
            String::new()
        };
        match memory_verdict(signature, status, &memory) {
            MemoryVerdict::Pending => {}
            MemoryVerdict::Passed => return Ok(()),
            MemoryVerdict::Failed(result_code) => {
                return Err(format!(
                    "Blargg reported failure code {result_code} after {elapsed} T-cycles: {memory}"
                ));
            }
        }
    }
    let signature = [
        core.diagnostic_read(0xa001),
        core.diagnostic_read(0xa002),
        core.diagnostic_read(0xa003),
    ];
    let status = core.diagnostic_read(0xa000);
    let memory = diagnostic_text(&core);
    let serial = String::from_utf8_lossy(core.serial_output());
    let registers = (0xff10..=0xff26)
        .map(|address| core.diagnostic_read(address))
        .collect::<Vec<_>>();
    Err(format!(
        "Blargg dmg_sound timed out at the exact {max_t_cycles} T-cycle bound: status={status:#04x}, signature={signature:02x?}, memory={memory:?}, serial={serial:?}, NR10-NR26={registers:02x?}"
    ))
}

#[test]
fn memory_verdict_ignores_transitional_and_incomplete_results() {
    assert_eq!(
        memory_verdict([0xde, 0xb0, 0x61], 0, ""),
        MemoryVerdict::Pending
    );
    assert_eq!(
        memory_verdict([0xde, 0xb0, 0x61], 0x80, "dmg_sound\n01:ok"),
        MemoryVerdict::Pending
    );
    assert_eq!(
        memory_verdict([0xde, 0xb0, 0x61], 0, "dmg_sound\nPassed"),
        MemoryVerdict::Passed
    );
    assert_eq!(
        memory_verdict([0xde, 0xb0, 0x61], 3, ""),
        MemoryVerdict::Failed(3)
    );
    assert_eq!(
        memory_verdict([0, 0, 0], 0, "Passed"),
        MemoryVerdict::Pending
    );
}

#[test]
#[ignore = "requires user-provisioned local-only SHA-256-verified Blargg ROM"]
fn blargg_dmg_sound_passes_all_twelve_cases() {
    run_dmg_sound(
        &workspace_path("tests/roms/downloads/blargg/dmg_sound/dmg_sound.gb"),
        EXPECTED_SHA256,
        MAX_T_CYCLES,
    )
    .expect("dmg_sound compatibility result");
}
