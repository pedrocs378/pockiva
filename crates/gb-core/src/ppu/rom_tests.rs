//! Checksum-gated graphical ROM tests.

use std::fmt::Write as _;
use std::fs;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{Clock, EmulatorCore, GameBoy};

const MOONEYE_LIMIT: u64 = 50_000_000;
const ACID2_LIMIT: u64 = 20_971_520;
const MOONEYE_PASS: [u8; 6] = [3, 5, 8, 13, 21, 34];
const MOONEYE_FAIL: [u8; 6] = [0x42; 6];
const ACID2_RGBA_HASH: &str = "95afb92675151023d85092a70d513af19b8ce0577fc05aba4b0051e3ccbfddda";

pub(super) struct FixedClock;

impl Clock for FixedClock {
    fn unix_seconds(&self) -> u64 {
        1_000_000
    }
}

fn download_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ppu/downloads")
}

fn sha256(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(result, "{byte:02x}").expect("writing to a String cannot fail");
    }
    result
}

pub(super) fn verify_bytes(
    asset: &str,
    bytes: &[u8],
    expected_hash: &str,
    max_t_cycles: u64,
) -> Result<(), String> {
    let actual = sha256(bytes);
    if actual == expected_hash {
        Ok(())
    } else {
        Err(format!(
            "{asset} failed checksum preflight before the {max_t_cycles} T-cycle bound: expected {expected_hash}, received {actual}"
        ))
    }
}

pub(super) fn run_mooneye_bytes(
    asset: &str,
    bytes: &[u8],
    max_t_cycles: u64,
) -> Result<(), String> {
    let mut core = GameBoy::new(
        FixedClock,
        NonZeroU32::new(48_000).expect("non-zero sample rate"),
    );
    core.load_rom(bytes, None)
        .map_err(|error| format!("{asset}: {error}"))?;
    let mut elapsed = 0_u64;
    while elapsed < max_t_cycles {
        let next = core
            .next_step_for_rom_test()
            .map_err(|error| format!("{asset}: {error}"))?;
        if next == 0 || u64::from(next) > max_t_cycles - elapsed {
            break;
        }
        let (cycles, breakpoint) = core
            .run_one_for_rom_test()
            .map_err(|error| format!("{asset}: {error}"))?;
        elapsed += u64::from(cycles);
        if breakpoint {
            let registers = core.diagnostic_registers();
            if registers == MOONEYE_PASS {
                return Ok(());
            }
            if registers == MOONEYE_FAIL {
                let hram = (0xff80..=0xfffe)
                    .map(|address| core.diagnostic_read(address))
                    .collect::<Vec<_>>();
                return Err(format!(
                    "{asset} reported failure after {elapsed} T-cycles with registers {registers:02x?}; HRAM={hram:02x?}"
                ));
            }
        }
    }
    Err(format!(
        "{asset} timed out at the exact {max_t_cycles} T-cycle bound"
    ))
}

fn run_mooneye(path: &str, expected_hash: &str) -> Result<(), String> {
    let full_path = download_root().join("mooneye").join(path);
    let bytes = fs::read(&full_path)
        .map_err(|error| format!("cannot read {}: {error}", full_path.display()))?;
    verify_bytes(path, &bytes, expected_hash, MOONEYE_LIMIT)?;
    run_mooneye_bytes(path, &bytes, MOONEYE_LIMIT)
}

fn run_until_frame_hash(
    asset: &str,
    bytes: &[u8],
    max_t_cycles: u64,
    expected_hash: &str,
) -> Result<(), String> {
    let mut core = GameBoy::new(
        FixedClock,
        NonZeroU32::new(48_000).expect("non-zero sample rate"),
    );
    core.load_rom(bytes, None)
        .map_err(|error| format!("{asset}: {error}"))?;
    let mut elapsed = 0_u64;
    while elapsed < max_t_cycles {
        let next = core
            .next_step_for_rom_test()
            .map_err(|error| format!("{asset}: {error}"))?;
        if next == 0 || u64::from(next) > max_t_cycles - elapsed {
            break;
        }
        let (cycles, _) = core
            .run_one_for_rom_test()
            .map_err(|error| format!("{asset}: {error}"))?;
        elapsed += u64::from(cycles);
        if let Some(frame) = core.take_frame()
            && sha256(frame.rgba()) == expected_hash
        {
            return Ok(());
        }
    }
    Err(format!(
        "{asset} did not produce raw RGBA SHA-256 {expected_hash} at the exact {max_t_cycles} T-cycle bound"
    ))
}

macro_rules! mooneye_test {
    ($name:ident, $path:literal, $hash:literal) => {
        #[test]
        #[ignore = "requires explicitly provisioned checksum-verified PPU ROMs"]
        fn $name() {
            run_mooneye($path, $hash).expect("Mooneye PPU ROM passes");
        }
    };
}

mooneye_test!(
    mooneye_hblank_ly_scx_timing,
    "acceptance/ppu/hblank_ly_scx_timing-GS.gb",
    "3adec9174d16b7a4cece42e5525e4363ff956c19070600aa9344de68b0885449"
);
mooneye_test!(
    mooneye_intr_1_2_timing,
    "acceptance/ppu/intr_1_2_timing-GS.gb",
    "3bac47fc79ce514fd7f6bbe0d87f1160b91a5292be27fee7bc3bcea6bc171ee9"
);
mooneye_test!(
    mooneye_intr_2_0_timing,
    "acceptance/ppu/intr_2_0_timing.gb",
    "6ea58d6940ad2dde6d20ef1fc63f1da83bdff842672d757a7a2377a3d0cfb7ff"
);
mooneye_test!(
    mooneye_intr_2_mode0_timing,
    "acceptance/ppu/intr_2_mode0_timing.gb",
    "be1555d577506073ba1ec4717060aa24075c02b9c787b874623a98bf2ac2da6e"
);
mooneye_test!(
    mooneye_intr_2_mode0_timing_sprites,
    "acceptance/ppu/intr_2_mode0_timing_sprites.gb",
    "52b10bb0d3073ec35d6bc4f0129fcabb788e4d11ea765163a49d519121d5169e"
);
mooneye_test!(
    mooneye_intr_2_mode3_timing,
    "acceptance/ppu/intr_2_mode3_timing.gb",
    "b5cb7d22162e3ed6fa2dafeaa487cf1d1c042b5e8a3a9877823c33b578b9c31e"
);
mooneye_test!(
    mooneye_intr_2_oam_ok_timing,
    "acceptance/ppu/intr_2_oam_ok_timing.gb",
    "38d7acfddce357c8b084f9bb647d6ffc99d1fb85d7a312c2db2c348ba888f7ff"
);
mooneye_test!(
    mooneye_lcdon_timing,
    "acceptance/ppu/lcdon_timing-GS.gb",
    "2a9d46b61935ae1a2332abd419bd6d63c2c48697b96ad547c859c207cf531e2f"
);
mooneye_test!(
    mooneye_lcdon_write_timing,
    "acceptance/ppu/lcdon_write_timing-GS.gb",
    "e28b34cef8b5d58bf19e058be2206309129a5896568e918b6b11b6c61dce2a51"
);
mooneye_test!(
    mooneye_stat_irq_blocking,
    "acceptance/ppu/stat_irq_blocking.gb",
    "604436aeb6a37badd71be0fafa526307345f1de6af757193f11fc77e09a01fc7"
);
mooneye_test!(
    mooneye_stat_lyc_onoff,
    "acceptance/ppu/stat_lyc_onoff.gb",
    "29f04aaf6b26085bca1dccfab648fb44fbf57d4aa923bca75a30167e45d8670e"
);
mooneye_test!(
    mooneye_vblank_stat_intr,
    "acceptance/ppu/vblank_stat_intr-GS.gb",
    "f7de9a3ef1399f73ad16ef23dccf05d38cbd62373215608ee5da53a35850436e"
);

#[test]
#[ignore = "requires explicitly provisioned checksum-verified PPU ROMs"]
fn dmg_acid2_raw_rgba() {
    let path = download_root().join("dmg-acid2/dmg-acid2.gb");
    let bytes =
        fs::read(&path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    verify_bytes(
        "dmg-acid2.gb",
        &bytes,
        "464e14b7d42e7feea0b7ede42be7071dc88913f75b9ffa444299424b63d1dff1",
        ACID2_LIMIT,
    )
    .expect("dmg-acid2 checksum passes");
    run_until_frame_hash("dmg-acid2.gb", &bytes, ACID2_LIMIT, ACID2_RGBA_HASH)
        .expect("dmg-acid2 raw frame passes");
}
