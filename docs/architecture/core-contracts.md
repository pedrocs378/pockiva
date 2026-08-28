# Core Contracts

`gb-core` provides a platform-independent DMG emulator through the frozen `EmulatorCore` contract. The concrete entry point is `GameBoy<C: Clock + Send>::new(clock, sample_rate)`, and compile-time tests prove `GameBoy<TestClock>: EmulatorCore + Send` for the desktop emulation-thread boundary.

The crate contains no Tauri, React, filesystem, network, Tokio, or platform-audio dependency. Test-only ROM harness code may read explicitly provisioned assets; production code receives ROM and persistence bytes from its caller. PED-35 added `sha2` only for stable ROM identities. The shared `Cargo.lock` is coordinator-owned and was neither staged nor committed by the PED-35 lane.

## Emulator lifecycle

`load_rom` validates a complete candidate cartridge, its mapper, and optional persistence before replacing the running machine. A failed replacement leaves the previous cartridge runnable. Successful load and `reset` restore post-boot DMG CPU state (`AF=01B0`, `BC=0013`, `DE=00D8`, `HL=014D`, `SP=FFFE`, `PC=0100`). Reset preserves cartridge RAM/RTC, resets volatile machine state, and reapplies the effective multi-source input snapshot.

`run_cycles` uses T-cycles exclusively. `DMG_CLOCK_HZ` is `4_194_304` and `T_CYCLES_PER_M_CYCLE` is `4`. Execution stops at an instruction boundary and never exceeds the caller's budget; a budget below the next instruction cost executes zero cycles. The injected clock is sampled once before each instruction, so MBC3 state is deterministic and one instruction cannot observe multiple timestamps.

## Internal stable seams

- The LR35902 CPU accesses memory only through `CpuBus`. Every CPU read, write, or idle machine cycle advances the machine bus by four T-cycles.
- `MachineBus: Send` owns `Mapper: Send`, `VideoDevice: Send`, and `AudioDevice: Send` trait objects. It fans every machine cycle into timer, DMA, video, and audio, then unions device interrupt requests into IF.
- The video device owns VRAM, OAM, and `FF40..=FF4B`; the PED-35 implementation is register-only, reports `LY=FF`, and produces no frames. PED-36 owns pixel/scanline behavior behind this seam.
- The audio device owns `FF10..=FF3F`; the PED-35 implementation stores registers and drains an empty batch at the configured non-zero rate. PED-49 owns PCM synthesis behind this seam.
- OAM DMA copies one byte per machine cycle for 160 cycles, permits HRAM CPU access, blocks other CPU accesses, and can be restarted by writing `FF46`.
- Timer input uses divider falling edges at TAC-selected bits `9, 3, 5, 7`; overflow exposes `00` for four T-cycles before reload and requests the timer interrupt.
- IF/IE retain only bits `0..=4`; pending interrupts use DMG priority order and service vectors `0040`, `0048`, `0050`, `0058`, `0060`.

## Cartridge and input behavior

Supported type bytes are ROM-only `00/08/09`, MBC1 `01/02/03`, MBC3 `0F/10/11/12/13`, and MBC5 `19/1A/1B/1C/1D/1E`. DMG and dual-mode cartridges run in DMG mode; a `C0` CGB-only header is rejected transactionally. ROM identity is lowercase SHA-256 over the complete image.

Battery persistence uses contract format version `1` with exact header-declared RAM length. Timer MBC3 mapper bytes use the fixed 22-byte `M3R1` schema: four-byte magic, little-endian counter seconds, little-endian last-update Unix timestamp, halted byte, and carry byte. No platform clock is consulted inside a mapper.

`InputMatrix` stores one complete `JoypadState` per `InputSourceId` and exposes their union. Updating or clearing one source cannot release another source's buttons. `FF00` uses active-low rows and requests Joypad only on a selected line's falling edge, including when row selection reveals a held button.
