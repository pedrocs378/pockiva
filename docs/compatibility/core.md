# PED-35 Core Compatibility

This evidence covers the deterministic CPU, MMU, cartridge, timer, interrupt, DMA, and joypad core. It makes no graphical PPU or PCM APU compatibility claim; those belong to PED-36 and PED-49. No commercial ROM was used.

## Automated core evidence

| Group | Signal | Observed result |
| --- | --- | --- |
| CPU registers, flags, base/CB decode, loads, ALU, control flow, IME, HALT, STOP | Rust unit assertions and exact T-cycle counts | Pass |
| ROM-only, MBC1, MBC3 RTC/`M3R1`, MBC5, validation and persistence | Synthetic numbered-bank ROMs with fixed clock | Pass |
| Timer and IF/IE | Falling-edge, four-cycle reload, write-glitch, priority assertions | Pass |
| Joypad | Source union, active-low rows, falling-edge interrupt assertions | Pass |
| MMU, serial and OAM DMA | Address-boundary, echo RAM, access timing, 160-byte DMA assertions | Pass |
| Public `GameBoy<C: Clock + Send>` lifecycle | Transactional replacement, budget, reset and compile-time `EmulatorCore + Send` assertions | Pass |

## Checksum-controlled ROM evidence

All Mooneye entries use revision `31510e12eea6286d36eea060a6adde755e1067aa`, MIT licensing, archive SHA-256 `6d4fdda2f1d8d2f5f51b0ff3f6f3cc2fbae047aa395a39c82bda3a0e7cbd2641`, the `[03,05,08,0d,15,22]` register signal, and a `50,000,000` T-cycle maximum.

| ROM | SHA-256 | Observed result |
| --- | --- | --- |
| `acceptance/bits/reg_f.gb` | `4b193e887ee3ac82b38b796729e1503e9a78da3e1140f8bd5600d0884f2e2627` | Pass |
| `acceptance/instr/daa.gb` | `1498d92d70592a07a2493ef764609916616f0b023b21408189e277201e6c14c1` | Pass |
| `acceptance/ei_sequence.gb` | `dcd7f37e8fe7d8eb38cab6732a5826e0bb0278fd1e1d9e297c28d205da1b69e1` | Pass |
| `acceptance/if_ie_registers.gb` | `d055b2b4c44902cf827296a06b17cea4f2c84f6b7d540c777cb1d1049ef35e61` | Pass |
| `acceptance/timer/div_write.gb` | `2be1e4da6fa24b9123d2a8bae47dd0d6f5e97e1855186c0c0f49e6d213eebfff` | Pass |
| `acceptance/timer/tima_reload.gb` | `1ca70c725bd1e027b07d3058839bd140eccddd9f4ca41305c4f8ab3acaff8a98` | Pass |
| `acceptance/oam_dma/basic.gb` | `326b747cac8cc96b62d6ee508e73b87eda24bfe29553d3d32e719f3b6d76c97c` | Pass |

The Blargg mirror revision is `c240dd7d700e5c0b00a7bbba52b53e4ee67b5f15`. Redistribution is not granted by the mirror's readme, so both assets are local-only and were not downloaded.

| ROM | SHA-256 | Signal / maximum | Observed result |
| --- | --- | --- | --- |
| `cpu_instrs/cpu_instrs.gb` | `8c5e12f41e0ba5bbca796944f92ffe6de28809198682c4332e38d1b3cf56fcf2` | serial `Passed` / `2,000,000,000` T-cycles | Blocked: local-only asset absent |
| `instr_timing/instr_timing.gb` | `646067b3d6c79fda810e9c3f1cb7c0efd5abb0a7ac06437c54e65720c15d9925` | serial `Passed` / `200,000,000` T-cycles | Blocked: local-only asset absent |

PED-35 therefore remains incomplete until checksum-matched, user-provisioned Blargg files pass. The absence does not authorize downloading, committing, caching, or redistributing them.
