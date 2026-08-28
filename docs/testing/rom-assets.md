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
