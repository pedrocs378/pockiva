# DMG PPU Compatibility

PED-36 implements a DMG scanline renderer behind `Ppu: VideoDevice + Send`. `gb-core` remains platform independent and has no Tauri, React, filesystem, network, Tokio, or platform-audio dependency; only the checksum-gated `#[cfg(test)]` ROM harness reads developer-provisioned files. CGB rendering is unsupported. A dual-mode header is exposed as `DmgCompatible` and runs in DMG mode, while a CGB-only header is rejected.

## Implemented boundary

- LCD timing advances one T-cycle at a time behind the machine bus's four-T-cycle calls: 456 T-cycles per line, 154 lines per frame, and a `160 x 144` visible frame. The implementation covers OAM scan, Drawing, HBlank, VBlank, STAT edge behavior, `LY/LYC`, and LCD enable/disable timing.
- CPU VRAM and OAM restrictions follow the active LCD mode, while OAM DMA uses its dedicated write path. Rendering covers DMG background/window addressing and scrolling, window position/counter behavior, and DMG OBJ selection, priority, transparency, palettes, flips, and 8x8/8x16 sizing. Output uses a neutral four-shade DMG palette.
- The core retains one pending frame and replaces an unread frame with the newest completed frame. The desktop queue then retains one in-flight frame and only the latest pending frame.
- Native transport is a 92_172-byte packet: little-endian sequence, width, and height followed by 92_160 raw RGBA bytes. It contains no base64 or PNG. The browser validates the packet, uses a zero-copy RGBA view, coalesces arrivals to the latest frame per animation-frame callback, draws at `160 x 144`, then acknowledges it.
- The canvas uses pixelated nearest-neighbor scaling at a fixed 10:9 display aspect ratio.

This boundary is scanline-oriented DMG behavior, not a claim of CGB support or a general pixel-FIFO-perfect implementation beyond the evidence below.

## ROM acceptance evidence

All twelve Mooneye cases use project revision `31510e12eea6286d36eea060a6adde755e1067aa` (MIT), the register signal `03,05,08,0d,15,22`, and an independent 50,000,000 T-cycle bound per ROM.

| Project / revision | ROM | ROM SHA-256 | Signal | T-cycle bound | Observed result |
| --- | --- | --- | --- | ---: | --- |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/hblank_ly_scx_timing-GS.gb` | `3adec9174d16b7a4cece42e5525e4363ff956c19070600aa9344de68b0885449` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/intr_1_2_timing-GS.gb` | `3bac47fc79ce514fd7f6bbe0d87f1160b91a5292be27fee7bc3bcea6bc171ee9` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/intr_2_0_timing.gb` | `6ea58d6940ad2dde6d20ef1fc63f1da83bdff842672d757a7a2377a3d0cfb7ff` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/intr_2_mode0_timing.gb` | `be1555d577506073ba1ec4717060aa24075c02b9c787b874623a98bf2ac2da6e` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/intr_2_mode0_timing_sprites.gb` | `52b10bb0d3073ec35d6bc4f0129fcabb788e4d11ea765163a49d519121d5169e` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/intr_2_mode3_timing.gb` | `b5cb7d22162e3ed6fa2dafeaa487cf1d1c042b5e8a3a9877823c33b578b9c31e` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/intr_2_oam_ok_timing.gb` | `38d7acfddce357c8b084f9bb647d6ffc99d1fb85d7a312c2db2c348ba888f7ff` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/lcdon_timing-GS.gb` | `2a9d46b61935ae1a2332abd419bd6d63c2c48697b96ad547c859c207cf531e2f` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/lcdon_write_timing-GS.gb` | `e28b34cef8b5d58bf19e058be2206309129a5896568e918b6b11b6c61dce2a51` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/stat_irq_blocking.gb` | `604436aeb6a37badd71be0fafa526307345f1de6af757193f11fc77e09a01fc7` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/stat_lyc_onoff.gb` | `29f04aaf6b26085bca1dccfab648fb44fbf57d4aa923bca75a30167e45d8670e` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| Mooneye `31510e12eea6286d36eea060a6adde755e1067aa` | `acceptance/ppu/vblank_stat_intr-GS.gb` | `f7de9a3ef1399f73ad16ef23dccf05d38cbd62373215608ee5da53a35850436e` | registers `03,05,08,0d,15,22` | 50,000,000 | Passed within 50,000,000 T-cycles |
| dmg-acid2 v1.0 / `dc22954` | `dmg-acid2.gb` | `464e14b7d42e7feea0b7ede42be7071dc88913f75b9ffa444299424b63d1dff1` | raw RGBA SHA-256 `95afb92675151023d85092a70d513af19b8ce0577fc05aba4b0051e3ccbfddda` | 20,971,520 | Passed within 20,971,520 T-cycles |

The dmg-acid2 official reference PNG is provenance only: upstream commit `dc22954`, `img/reference-dmg.png`, SHA-256 `ca966d50895c7efef05838590d148c2cbfd7fba57dab986f25b35b4da71abb57`. The image is neither stored nor used by automation. The automated comparison is the derived raw `160 x 144 x RGBA8` hash `95afb92675151023d85092a70d513af19b8ce0577fc05aba4b0051e3ccbfddda` shown above.

Mooneye and dmg-acid2 are both MIT licensed. Their ROM binaries are checksum-gated, stored only in ignored download directories, and never fetched by ordinary commands. No commercial ROM was downloaded, stored, or used.

## Desktop acceptance evidence

| Boundary | Automated evidence | Result |
| --- | --- | --- |
| Native packet | Fixed little-endian `u64 + u16 + u16 + RGBA8` layout; exact 92_172-byte length; no text/base64/PNG encoding | Passed |
| Native queue/runtime | One in-flight plus latest-pending replacement, stale/future acknowledgement rejection, bounded stress, clear/close, observer replacement/failure, and continuous-command cadence | Passed |
| Frontend packet/presenter | Packet validation, zero-copy RGBA view, `160 x 144` draw, acknowledgement after draw, latest-frame coalescing, cleanup, missing-context handling, pixelated 10:9 canvas | Passed |
| Optional deterministic visual fixture inspection | Non-acceptance dev/test evidence | not run (optional) |

Production `GameBoy` factory-to-native-window validation is owned by PED-39/PED-40. It was neither required nor claimed by PED-36.
