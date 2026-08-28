# ROM Test Assets

No commercial ROM may enter the repository, generated artifacts, CI cache, release bundle, or ordinary application download path.

Blargg, Mooneye, and homebrew assets may be used only when redistribution or test use is legally permitted. The committed `tests/roms/core-roms.toml` records, for every PED-35 asset:

- project and test-case name;
- upstream source URL;
- license or redistribution terms;
- immutable revision or release tag;
- SHA-256 checksum;
- expected emulator pass/fail signal;
- maximum cycle or wall-clock test bound.

Ordinary install, build, test discovery, and application startup never download ROMs. `scripts/fetch-core-test-roms.sh --mooneye` is an explicit, opt-in fetch of the pinned MIT Mooneye archive. It prints revision, license, URL, and archive SHA-256 before transfer; requires interactive `yes` or the exact `GB_ROM_ASSET_ACCEPT=mooneye-mit-31510e1` acknowledgement; extracts only the seven selected tests; and verifies every checksum.

Blargg's selected mirror does not grant redistribution in its readme. Both Blargg entries are therefore `local_only = true`: the fetch script refuses to download them, while `scripts/verify-core-test-roms.sh --blargg` verifies developer-provided copies and reports missing assets distinctly. CI may use only separately provisioned, checksum-matched copies and must not cache or publish them.

All binaries live below ignored `tests/roms/downloads/`. The manifest, scripts, and ignored Rust harness are committed; `.gb`, `.gbc`, saves, archives, and extracted directories are not. End-to-end tests use legally redistributable homebrew ROMs, never commercial cartridge dumps.

## PED-36 PPU acceptance assets

The PPU acceptance manifest is `crates/gb-core/tests/ppu/roms.toml`. It pins twelve Mooneye PPU ROMs from MIT-licensed revision `31510e12eea6286d36eea060a6adde755e1067aa` and the MIT-licensed dmg-acid2 v1.0 ROM at commit `dc22954`. Every ROM has an immutable SHA-256, an exact pass signal, and a T-cycle bound. The observed results are recorded in `docs/compatibility/ppu.md`.

`crates/gb-core/tests/ppu/fetch-roms.sh` is the only acquisition path. It requires interactive `yes` or the exact `GB_PPU_ROM_ASSET_ACCEPT=mooneye-mit-31510e1+dmg-acid2-mit-v1.0` acknowledgement, verifies the pinned archive and ROM before installation, and extracts only the twelve named Mooneye files. Ordinary install, build, test discovery, test execution, verification, and application startup perform no automatic download. The verifier is offline.

Downloaded ROM binaries live under `crates/gb-core/tests/ppu/downloads/` and are ignored by the repository-wide `*.gb` rule. The official dmg-acid2 reference PNG is provenance only: it is not downloaded, stored, committed, decoded, or used as a runtime/test payload. The automated check compares raw RGBA bytes, so neither PNG nor base64 enters the frame transport. No commercial ROM was used.
