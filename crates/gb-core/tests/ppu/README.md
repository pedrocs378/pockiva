# PED-36 graphical ROM assets

This directory records the immutable, redistributable acceptance inputs for the DMG PPU. Mooneye Test Suite revision `31510e12eea6286d36eea060a6adde755e1067aa` and dmg-acid2 release `v1.0` (`dc22954`) are both MIT licensed; their pinned license and source URLs are recorded in `roms.toml`.

ROM binaries are downloaded only after an explicit interactive `yes` or the exact acknowledgement `GB_PPU_ROM_ASSET_ACCEPT=mooneye-mit-31510e1+dmg-acid2-mit-v1.0`. Ordinary builds, tests, test discovery, verification, and application startup never download anything. The fetcher verifies the archive/ROM, extracts only the twelve selected Mooneye members, and stores ignored files under `downloads/`. The offline verifier never accesses the network.

The Mooneye pass signal is the register sequence `03,05,08,0d,15,22`, bounded at 50,000,000 T-cycles per ROM. dmg-acid2 is bounded at 20,971,520 T-cycles and compares a raw `160 x 144 x RGBA8` hash of `95afb92675151023d85092a70d513af19b8ce0577fc05aba4b0051e3ccbfddda`.

The official dmg-acid2 reference PNG is provenance only. It is neither downloaded nor stored and never appears in the runtime or test payload; its pinned URL/hash and the derived raw RGBA hash are recorded in `roms.toml`. No commercial ROM is used.
