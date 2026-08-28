# Core Contracts

`gb-core` provides a platform-independent DMG emulator through the frozen `EmulatorCore` contract. The concrete entry point is `GameBoy<C: Clock + Send>::new(clock, sample_rate)`, and compile-time tests prove `GameBoy<TestClock>: EmulatorCore + Send` for the desktop emulation-thread boundary.

The crate contains no Tauri, React, filesystem, network, Tokio, or platform-audio dependency. Test-only ROM harness code may read explicitly provisioned assets; production code receives ROM and persistence bytes from its caller. PED-35 added `sha2` only for stable ROM identities. The shared `Cargo.lock` is coordinator-owned and was neither staged nor committed by the PED-35 lane.

## Emulator lifecycle

`load_rom` validates a complete candidate cartridge, its mapper, and optional persistence before replacing the running machine. A failed replacement leaves the previous cartridge runnable. Successful load and `reset` restore post-boot DMG CPU state (`AF=01B0`, `BC=0013`, `DE=00D8`, `HL=014D`, `SP=FFFE`, `PC=0100`). Reset preserves cartridge RAM/RTC, resets volatile machine state, and reapplies the effective multi-source input snapshot.

`run_cycles` uses T-cycles exclusively. `DMG_CLOCK_HZ` is `4_194_304` and `T_CYCLES_PER_M_CYCLE` is `4`. Execution stops at an instruction boundary and never exceeds the caller's budget; a budget below the next instruction cost executes zero cycles. The injected clock is sampled once before each instruction, so MBC3 state is deterministic and one instruction cannot observe multiple timestamps.

## Internal stable seams

- The LR35902 CPU accesses memory only through `CpuBus`. Every CPU read, write, or idle machine cycle advances the machine bus by four T-cycles.
- `MachineBus: Send` owns `Mapper: Send`, `VideoDevice: Send`, and `AudioDevice: Send` trait objects. It fans every machine cycle into timer, DMA, video, and audio, then unions device interrupt requests into IF.
- The video device owns VRAM, OAM, and `FF40..=FF4B`. PED-36's `Ppu: VideoDevice + Send` advances one LCD T-cycle at a time behind each four-T-cycle machine-bus call and publishes fixed `160 x 144` RGBA8 frames through the existing `frame_ready`/`take_frame` contract.
- The audio device owns `FF10..=FF3F`. PED-49's `Apu: AudioDevice + Send` implements the two pulse channels, programmable wave channel, noise channel, frame sequencer, mixer, high-pass filtering, and bounded stereo PCM at the configured non-zero rate. The bus forwards every machine cycle as four individual APU T-cycles and mirrors `FF04` divider resets so frame-sequencer falling edges remain deterministic.
- OAM DMA copies one byte per machine cycle for 160 cycles, permits HRAM CPU access, blocks other CPU accesses, and can be restarted by writing `FF46`.
- Timer input uses divider falling edges at TAC-selected bits `9, 3, 5, 7`; overflow exposes `00` for four T-cycles before reload and requests the timer interrupt.
- IF/IE retain only bits `0..=4`; pending interrupts use DMG priority order and service vectors `0040`, `0048`, `0050`, `0058`, `0060`.

## Cartridge and input behavior

Supported type bytes are ROM-only `00/08/09`, MBC1 `01/02/03`, MBC3 `0F/10/11/12/13`, and MBC5 `19/1A/1B/1C/1D/1E`. DMG and dual-mode cartridges run in DMG mode; a `C0` CGB-only header is rejected transactionally. ROM identity is lowercase SHA-256 over the complete image.

Battery persistence uses contract format version `1` with exact header-declared RAM length. Timer MBC3 mapper bytes use the fixed 22-byte `M3R1` schema: four-byte magic, little-endian counter seconds, little-endian last-update Unix timestamp, halted byte, and carry byte. No platform clock is consulted inside a mapper.

`InputMatrix` stores one complete `JoypadState` per `InputSourceId` and exposes their union. Updating or clearing one source cannot release another source's buttons. `FF00` uses active-low rows and requests Joypad only on a selected line's falling edge, including when row selection reveals a held button.

## DMG video behavior

The PPU models 456 T-cycles per line and 154 lines per frame, with 144 visible lines followed by VBlank. Visible lines progress through OAM scan, Drawing, and HBlank; Drawing begins after the 80-dot OAM interval and has a 172-dot base plus the implemented scroll, window, and sprite penalties. Entering line 144 publishes a frame and requests VBlank. LCD STAT sources are combined onto an edge-triggered line for mode 0, mode 1, mode 2, and `LY=LYC`, including the line-153 `LY` transition behavior and LCD enable/disable timing exercised by the pinned tests.

CPU VRAM access is blocked during Drawing, and CPU OAM access is blocked during OAM scan and Drawing; OAM DMA writes use the dedicated device path. Rendering covers DMG background scrolling and tile addressing, the `WX-7` window origin and window-line counter, and at most ten objects per scanline. Objects support 8x8/8x16 selection, flips, transparent color zero, OBP0/OBP1 selection, background priority, and DMG priority by X coordinate then OAM index. BGP/OBP color numbers map to a neutral white, light gray, dark gray, and black RGBA palette.

The core keeps one pending frame: publishing a newer frame replaces an unread one. Dual-mode cartridge headers are reported as `DmgCompatible` and execute with this DMG renderer. CGB-only cartridges are rejected, and CGB rendering remains unsupported.

## DMG audio behavior

The APU starts from the post-boot DMG divider value `ABCC` and clocks its frame sequencer from falling edges of divider bit 12, including edges caused by `FF04` writes. Register power behavior, length counters, envelopes, channel-one sweep, pulse duty/frequency, wave RAM access/retrigger, noise LFSR modes, NR50/NR51 mixing, VIN-ignore policy, DAC-off silence, and high-pass reset behavior are covered by deterministic unit and integration vectors.

PCM sample counts use an integer phase accumulator against `DMG_CLOCK_HZ`; output is stereo-pair safe, bounded, and drained through the unchanged `AudioBatch` contract. The platform-independent crate contains no CPAL, ring buffer, OS audio, Tauri, or async-runtime dependency.

## Desktop frame boundary

The native adapter applies two-slot backpressure: one frame may be in flight and only the latest pending frame is retained. It transports a fixed 92_172-byte raw packet: little-endian `u64` sequence, little-endian `u16` width, little-endian `u16` height, then 92_160 raw RGBA bytes. Frame transport contains no base64, JSON image representation, or PNG encoding.

The browser validates and exposes the RGBA payload as a zero-copy typed-array view, coalesces arrivals to the latest frame for the next animation frame, draws at the native resolution, and acknowledges only after presentation. Its `160 x 144` canvas uses pixelated nearest-neighbor scaling and a fixed 10:9 display aspect ratio.
