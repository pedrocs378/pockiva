use super::decode::decode;
use super::registers::{Flags, Registers, add8, sub8};
use super::{Cpu, CpuBus, CpuMode};
use crate::CoreError;
use crate::interrupts::{Interrupt, InterruptMask};

struct TestBus {
    memory: Box<[u8; 0x1_0000]>,
    elapsed_t_cycles: u64,
    pending: InterruptMask,
    divider_reset: bool,
}

impl Default for TestBus {
    fn default() -> Self {
        Self {
            memory: Box::new([0; 0x1_0000]),
            elapsed_t_cycles: 0,
            pending: InterruptMask::default(),
            divider_reset: false,
        }
    }
}

impl TestBus {
    fn with_program(program: &[u8]) -> Self {
        let mut bus = Self::default();
        bus.memory[0x0100..0x0100 + program.len()].copy_from_slice(program);
        bus
    }
}

impl CpuBus for TestBus {
    fn read8(&mut self, address: u16) -> u8 {
        self.elapsed_t_cycles += 4;
        self.memory[usize::from(address)]
    }

    fn write8(&mut self, address: u16, value: u8) {
        self.elapsed_t_cycles += 4;
        self.memory[usize::from(address)] = value;
    }

    fn idle_m_cycle(&mut self) {
        self.elapsed_t_cycles += 4;
    }

    fn peek8(&self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn elapsed_t_cycles(&self) -> u64 {
        self.elapsed_t_cycles
    }

    fn pending_interrupts(&self) -> InterruptMask {
        self.pending
    }

    fn acknowledge_interrupt(&mut self, interrupt: Interrupt) {
        self.pending = self
            .pending
            .without(InterruptMask::from_bits(interrupt.bit()));
    }

    fn reset_divider(&mut self) {
        self.divider_reset = true;
    }
}

#[test]
fn register_pairs_mask_the_flag_low_nibble() {
    let mut registers = Registers::post_boot_dmg();
    registers.set_af(0x12ff);
    assert_eq!(registers.af(), 0x12f0);
}

#[test]
fn add_and_subtract_report_half_carry() {
    assert_eq!(add8(0x0f, 0x01, false), (0x10, Flags::H));
    assert_eq!(sub8(0x10, 0x01, false), (0x0f, Flags::N.union(Flags::H)));
}

#[test]
fn all_prefixed_opcodes_decode() {
    for opcode in u8::MIN..=u8::MAX {
        let decoded = decode(opcode, true);
        assert_eq!(decoded.length, 2);
        assert!(!decoded.illegal);
    }
}

#[test]
fn base_decode_marks_only_the_lr35902_illegal_opcodes() {
    let illegal = [
        0xd3, 0xdb, 0xdd, 0xe3, 0xe4, 0xeb, 0xec, 0xed, 0xf4, 0xfc, 0xfd,
    ];

    for opcode in u8::MIN..=u8::MAX {
        assert_eq!(decode(opcode, false).illegal, illegal.contains(&opcode));
    }
}

#[test]
fn load_add_and_cb_swap_execute_with_exact_cycles() {
    let mut cpu = Cpu::post_boot_dmg();
    let mut bus = TestBus::with_program(&[
        0x3e, 0x0f, // LD A,$0F
        0xc6, 0x01, // ADD A,$01
        0x06, 0xf0, // LD B,$F0
        0xcb, 0x30, // SWAP B
    ]);

    assert_eq!(cpu.step(&mut bus).expect("LD succeeds").t_cycles, 8);
    assert_eq!(cpu.registers.a, 0x0f);
    assert_eq!(cpu.step(&mut bus).expect("ADD succeeds").t_cycles, 8);
    assert_eq!(cpu.registers.a, 0x10);
    assert_eq!(cpu.registers.f, Flags::H);
    assert_eq!(cpu.step(&mut bus).expect("LD succeeds").t_cycles, 8);
    assert_eq!(cpu.step(&mut bus).expect("SWAP succeeds").t_cycles, 8);
    assert_eq!(cpu.registers.b, 0x0f);
}

#[test]
fn conditional_branch_reports_taken_and_not_taken_timings() {
    let mut taken_cpu = Cpu::post_boot_dmg();
    taken_cpu.registers.f = Flags::default();
    let mut taken_bus = TestBus::with_program(&[0x20, 0x02]);
    assert_eq!(taken_cpu.next_step_t_cycles(&taken_bus), Ok(12));
    assert_eq!(
        taken_cpu
            .step(&mut taken_bus)
            .expect("JR succeeds")
            .t_cycles,
        12
    );
    assert_eq!(taken_cpu.registers.pc, 0x0104);

    let mut skipped_cpu = Cpu::post_boot_dmg();
    skipped_cpu.registers.f = Flags::Z;
    let mut skipped_bus = TestBus::with_program(&[0x20, 0x02]);
    assert_eq!(skipped_cpu.next_step_t_cycles(&skipped_bus), Ok(8));
    assert_eq!(
        skipped_cpu
            .step(&mut skipped_bus)
            .expect("JR succeeds")
            .t_cycles,
        8
    );
    assert_eq!(skipped_cpu.registers.pc, 0x0102);
}

#[test]
fn call_and_return_round_trip_the_stack() {
    let mut cpu = Cpu::post_boot_dmg();
    let mut bus = TestBus::with_program(&[0xcd, 0x00, 0x02]);
    bus.memory[0x0200] = 0xc9;

    assert_eq!(cpu.step(&mut bus).expect("CALL succeeds").t_cycles, 24);
    assert_eq!(cpu.registers.pc, 0x0200);
    assert_eq!(cpu.registers.sp, 0xfffc);
    assert_eq!(bus.memory[0xfffc], 0x03);
    assert_eq!(bus.memory[0xfffd], 0x01);

    assert_eq!(cpu.step(&mut bus).expect("RET succeeds").t_cycles, 16);
    assert_eq!(cpu.registers.pc, 0x0103);
    assert_eq!(cpu.registers.sp, 0xfffe);
}

#[test]
fn ei_defers_interrupt_service_until_after_the_following_instruction() {
    let mut cpu = Cpu::post_boot_dmg();
    let mut bus = TestBus::with_program(&[0xfb, 0x00]);
    bus.pending = InterruptMask::from_bits(Interrupt::Timer.bit());

    assert_eq!(cpu.step(&mut bus).expect("EI succeeds").t_cycles, 4);
    assert_eq!(cpu.registers.pc, 0x0101);
    assert_eq!(cpu.step(&mut bus).expect("NOP succeeds").t_cycles, 4);
    assert_eq!(cpu.registers.pc, 0x0102);
    assert_eq!(cpu.step(&mut bus).expect("interrupt succeeds").t_cycles, 20);
    assert_eq!(cpu.registers.pc, Interrupt::Timer.vector());
    assert_eq!(cpu.registers.sp, 0xfffc);
    assert_eq!(bus.memory[0xfffc], 0x02);
    assert_eq!(bus.memory[0xfffd], 0x01);
    assert_eq!(bus.pending, InterruptMask::default());
}

#[test]
fn consecutive_ei_instructions_do_not_postpone_the_first_enable() {
    let mut cpu = Cpu::post_boot_dmg();
    let mut bus = TestBus::with_program(&[0xfb, 0xfb, 0xfb]);
    bus.pending = InterruptMask::from_bits(Interrupt::Serial.bit());

    cpu.step(&mut bus).expect("first EI succeeds");
    cpu.step(&mut bus).expect("second EI succeeds");
    assert_eq!(cpu.registers.pc, 0x0102);
    assert_eq!(cpu.next_step_t_cycles(&bus), Ok(20));
    cpu.step(&mut bus).expect("interrupt succeeds");
    assert_eq!(cpu.registers.pc, Interrupt::Serial.vector());
}

#[test]
fn halt_bug_reuses_the_next_opcode_byte() {
    let mut cpu = Cpu::post_boot_dmg();
    let mut bus = TestBus::with_program(&[0x76, 0x3e, 0x42]);
    bus.pending = InterruptMask::from_bits(Interrupt::Timer.bit());

    cpu.step(&mut bus).expect("HALT succeeds");
    cpu.step(&mut bus).expect("LD succeeds");

    assert_eq!(cpu.registers.a, 0x3e);
    assert_eq!(cpu.registers.pc, 0x0102);
}

#[test]
fn stop_consumes_padding_byte_and_resets_divider() {
    let mut cpu = Cpu::post_boot_dmg();
    let mut bus = TestBus::with_program(&[0x10, 0x00]);

    assert_eq!(cpu.step(&mut bus).expect("STOP succeeds").t_cycles, 4);
    assert_eq!(cpu.registers.pc, 0x0102);
    assert_eq!(cpu.mode, CpuMode::Stopped);
    assert!(bus.divider_reset);
}

#[test]
fn illegal_opcode_returns_a_typed_invariant_error() {
    let mut cpu = Cpu::post_boot_dmg();
    let mut bus = TestBus::with_program(&[0xd3]);

    assert!(matches!(
        cpu.step(&mut bus),
        Err(CoreError::InternalInvariant(reason)) if reason.contains("0xd3")
    ));
}
