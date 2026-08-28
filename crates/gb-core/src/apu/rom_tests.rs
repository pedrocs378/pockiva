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

fn diagnostic_text(core: &GameBoy<FixedClock>) -> Result<String, String> {
    let mut bytes = Vec::new();
    for offset in 0..4_096_u16 {
        let byte = core.diagnostic_read(0xa004_u16.wrapping_add(offset));
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|error| format!("Blargg result text is not UTF-8: {error}"))
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

        let magic = [
            core.diagnostic_read(0xa001),
            core.diagnostic_read(0xa002),
            core.diagnostic_read(0xa003),
        ];
        if magic != [0xde, 0xb0, 0x61] {
            continue;
        }
        match core.diagnostic_read(0xa000) {
            0x80 => {
                let text = diagnostic_text(&core)?;
                if text.contains("Failed") {
                    return Err(format!(
                        "Blargg reported failure after {elapsed} T-cycles: {text}"
                    ));
                }
            }
            0x00 => {
                let text = diagnostic_text(&core)?;
                if text.contains("Passed") {
                    return Ok(());
                }
                return Err(format!(
                    "Blargg ended without Passed after {elapsed} T-cycles: {text}"
                ));
            }
            status => {
                let text = diagnostic_text(&core)?;
                return Err(format!(
                    "Blargg reported status {status:#04x} after {elapsed} T-cycles: {text}"
                ));
            }
        }
    }
    Err(format!(
        "Blargg dmg_sound timed out at the exact {max_t_cycles} T-cycle bound"
    ))
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
