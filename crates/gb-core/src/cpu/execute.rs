use super::decode::decode;
use super::registers::{Flags, add8, daa, dec8, inc8, sub8};
use super::{Cpu, CpuBus, CpuMode};
use crate::CoreError;
use crate::interrupts::Interrupt;

pub(crate) struct StepResult {
    pub(crate) t_cycles: u32,
    #[cfg(test)]
    pub(crate) debug_breakpoint: bool,
}

pub(super) fn next_step_t_cycles(cpu: &Cpu, bus: &impl CpuBus) -> Result<u32, CoreError> {
    let pending = bus.pending_interrupts();
    if cpu.mode == CpuMode::Stopped && pending.bits() == 0 {
        return Ok(0);
    }
    if cpu.ime && pending.bits() != 0 {
        return Ok(20);
    }
    if cpu.mode == CpuMode::Halted && pending.bits() == 0 {
        return Ok(4);
    }
    let opcode = bus.peek8(cpu.registers.pc);
    let decoded = if opcode == 0xcb {
        decode(bus.peek8(cpu.registers.pc.wrapping_add(1)), true)
    } else {
        decode(opcode, false)
    };
    if decoded.illegal {
        return Err(CoreError::InternalInvariant(format!(
            "illegal opcode {opcode:#04x} at {:#06x}",
            cpu.registers.pc
        )));
    }
    Ok(u32::from(if branch_taken(cpu, opcode) {
        decoded.cycles_taken
    } else {
        decoded.cycles_not_taken
    }))
}

pub(super) fn step(cpu: &mut Cpu, bus: &mut impl CpuBus) -> Result<StepResult, CoreError> {
    let start = bus.elapsed_t_cycles();
    let pending = bus.pending_interrupts();
    if cpu.ime && pending.bits() != 0 {
        service_interrupt(
            cpu,
            bus,
            pending.highest_priority().expect("non-empty pending mask"),
        );
        return Ok(StepResult {
            t_cycles: elapsed(bus, start),
            #[cfg(test)]
            debug_breakpoint: false,
        });
    }
    if cpu.mode == CpuMode::Stopped {
        if pending.bits() == 0 {
            return Ok(StepResult {
                t_cycles: 0,
                #[cfg(test)]
                debug_breakpoint: false,
            });
        }
        cpu.mode = CpuMode::Running;
    }
    if cpu.mode == CpuMode::Halted {
        if pending.bits() == 0 {
            bus.idle_m_cycle();
            return Ok(StepResult {
                t_cycles: elapsed(bus, start),
                #[cfg(test)]
                debug_breakpoint: false,
            });
        }
        cpu.mode = CpuMode::Running;
    }

    let opcode_address = cpu.registers.pc;
    let opcode = fetch8(cpu, bus);
    if opcode == 0xcb {
        let prefixed = fetch8(cpu, bus);
        execute_cb(cpu, bus, prefixed);
        pad_to(bus, start, u32::from(decode(prefixed, true).cycles_taken));
    } else {
        let decoded = decode(opcode, false);
        if decoded.illegal {
            return Err(CoreError::InternalInvariant(format!(
                "illegal opcode {opcode:#04x} at {opcode_address:#06x}"
            )));
        }
        let taken = execute_base(cpu, bus, opcode);
        let target = if taken {
            decoded.cycles_taken
        } else {
            decoded.cycles_not_taken
        };
        pad_to(bus, start, u32::from(target));
    }

    if cpu.ime_enable_delay > 0 {
        cpu.ime_enable_delay -= 1;
        if cpu.ime_enable_delay == 0 {
            cpu.ime = true;
        }
    }
    Ok(StepResult {
        t_cycles: elapsed(bus, start),
        #[cfg(test)]
        debug_breakpoint: opcode == 0x40,
    })
}

fn elapsed(bus: &impl CpuBus, start: u64) -> u32 {
    u32::try_from(bus.elapsed_t_cycles() - start).expect("one CPU step fits u32")
}

fn pad_to(bus: &mut impl CpuBus, start: u64, target: u32) {
    while elapsed(bus, start) < target {
        bus.idle_m_cycle();
    }
    debug_assert_eq!(elapsed(bus, start), target);
}

fn fetch8(cpu: &mut Cpu, bus: &mut impl CpuBus) -> u8 {
    let value = bus.read8(cpu.registers.pc);
    if cpu.halt_bug {
        cpu.halt_bug = false;
    } else {
        cpu.registers.pc = cpu.registers.pc.wrapping_add(1);
    }
    value
}

fn fetch16(cpu: &mut Cpu, bus: &mut impl CpuBus) -> u16 {
    let low = fetch8(cpu, bus);
    let high = fetch8(cpu, bus);
    u16::from_le_bytes([low, high])
}

fn branch_taken(cpu: &Cpu, opcode: u8) -> bool {
    match opcode {
        0x20 | 0xc0 | 0xc2 | 0xc4 => !cpu.registers.f.contains(Flags::Z),
        0x28 | 0xc8 | 0xca | 0xcc => cpu.registers.f.contains(Flags::Z),
        0x30 | 0xd0 | 0xd2 | 0xd4 => !cpu.registers.f.contains(Flags::C),
        0x38 | 0xd8 | 0xda | 0xdc => cpu.registers.f.contains(Flags::C),
        _ => true,
    }
}

fn condition(cpu: &Cpu, code: u8) -> bool {
    match code {
        0 => !cpu.registers.f.contains(Flags::Z),
        1 => cpu.registers.f.contains(Flags::Z),
        2 => !cpu.registers.f.contains(Flags::C),
        _ => cpu.registers.f.contains(Flags::C),
    }
}

fn rp(cpu: &Cpu, code: u8) -> u16 {
    match code {
        0 => cpu.registers.bc(),
        1 => cpu.registers.de(),
        2 => cpu.registers.hl(),
        _ => cpu.registers.sp,
    }
}

fn set_rp(cpu: &mut Cpu, code: u8, value: u16) {
    match code {
        0 => cpu.registers.set_bc(value),
        1 => cpu.registers.set_de(value),
        2 => cpu.registers.set_hl(value),
        _ => cpu.registers.sp = value,
    }
}

fn rp2(cpu: &Cpu, code: u8) -> u16 {
    match code {
        0 => cpu.registers.bc(),
        1 => cpu.registers.de(),
        2 => cpu.registers.hl(),
        _ => cpu.registers.af(),
    }
}

fn set_rp2(cpu: &mut Cpu, code: u8, value: u16) {
    match code {
        0 => cpu.registers.set_bc(value),
        1 => cpu.registers.set_de(value),
        2 => cpu.registers.set_hl(value),
        _ => cpu.registers.set_af(value),
    }
}

fn read_r(cpu: &Cpu, bus: &mut impl CpuBus, code: u8) -> u8 {
    match code {
        0 => cpu.registers.b,
        1 => cpu.registers.c,
        2 => cpu.registers.d,
        3 => cpu.registers.e,
        4 => cpu.registers.h,
        5 => cpu.registers.l,
        6 => bus.read8(cpu.registers.hl()),
        _ => cpu.registers.a,
    }
}

fn write_r(cpu: &mut Cpu, bus: &mut impl CpuBus, code: u8, value: u8) {
    match code {
        0 => cpu.registers.b = value,
        1 => cpu.registers.c = value,
        2 => cpu.registers.d = value,
        3 => cpu.registers.e = value,
        4 => cpu.registers.h = value,
        5 => cpu.registers.l = value,
        6 => bus.write8(cpu.registers.hl(), value),
        _ => cpu.registers.a = value,
    }
}

fn push(cpu: &mut Cpu, bus: &mut impl CpuBus, value: u16) {
    let [high, low] = value.to_be_bytes();
    cpu.registers.sp = cpu.registers.sp.wrapping_sub(1);
    bus.write8(cpu.registers.sp, high);
    cpu.registers.sp = cpu.registers.sp.wrapping_sub(1);
    bus.write8(cpu.registers.sp, low);
}

fn pop(cpu: &mut Cpu, bus: &mut impl CpuBus) -> u16 {
    let low = bus.read8(cpu.registers.sp);
    cpu.registers.sp = cpu.registers.sp.wrapping_add(1);
    let high = bus.read8(cpu.registers.sp);
    cpu.registers.sp = cpu.registers.sp.wrapping_add(1);
    u16::from_le_bytes([low, high])
}

#[allow(
    clippy::many_single_char_names,
    clippy::match_same_arms,
    clippy::too_many_lines
)]
fn execute_base(cpu: &mut Cpu, bus: &mut impl CpuBus, opcode: u8) -> bool {
    let x = opcode >> 6;
    let y = (opcode >> 3) & 7;
    let z = opcode & 7;
    let p = y >> 1;
    let q = y & 1;
    match x {
        0 => match z {
            0 => match y {
                0 => {}
                1 => {
                    let address = fetch16(cpu, bus);
                    let [low, high] = cpu.registers.sp.to_le_bytes();
                    bus.write8(address, low);
                    bus.write8(address.wrapping_add(1), high);
                }
                2 => {
                    cpu.registers.pc = cpu.registers.pc.wrapping_add(1);
                    bus.reset_divider();
                    cpu.mode = CpuMode::Stopped;
                }
                3 => {
                    let offset = i8::from_ne_bytes([fetch8(cpu, bus)]);
                    cpu.registers.pc = cpu.registers.pc.wrapping_add_signed(i16::from(offset));
                }
                _ => {
                    let offset = i8::from_ne_bytes([fetch8(cpu, bus)]);
                    if condition(cpu, y - 4) {
                        cpu.registers.pc = cpu.registers.pc.wrapping_add_signed(i16::from(offset));
                    } else {
                        return false;
                    }
                }
            },
            1 => {
                if q == 0 {
                    let value = fetch16(cpu, bus);
                    set_rp(cpu, p, value);
                } else {
                    add_hl(cpu, rp(cpu, p));
                }
            }
            2 => {
                let address = match p {
                    0 => cpu.registers.bc(),
                    1 => cpu.registers.de(),
                    _ => cpu.registers.hl(),
                };
                if q == 0 {
                    bus.write8(address, cpu.registers.a);
                } else {
                    cpu.registers.a = bus.read8(address);
                }
                if p == 2 {
                    cpu.registers.set_hl(address.wrapping_add(1));
                }
                if p == 3 {
                    cpu.registers.set_hl(address.wrapping_sub(1));
                }
            }
            3 => {
                let value = if q == 0 {
                    rp(cpu, p).wrapping_add(1)
                } else {
                    rp(cpu, p).wrapping_sub(1)
                };
                set_rp(cpu, p, value);
            }
            4 => {
                let value = read_r(cpu, bus, y);
                let (result, flags) = inc8(value, cpu.registers.f.contains(Flags::C));
                write_r(cpu, bus, y, result);
                cpu.registers.f = flags;
            }
            5 => {
                let value = read_r(cpu, bus, y);
                let (result, flags) = dec8(value, cpu.registers.f.contains(Flags::C));
                write_r(cpu, bus, y, result);
                cpu.registers.f = flags;
            }
            6 => {
                let value = fetch8(cpu, bus);
                write_r(cpu, bus, y, value);
            }
            _ => accumulator_misc(cpu, y),
        },
        1 => {
            if opcode == 0x76 {
                if !cpu.ime && bus.pending_interrupts().bits() != 0 {
                    cpu.halt_bug = true;
                } else {
                    cpu.mode = CpuMode::Halted;
                }
            } else {
                let value = read_r(cpu, bus, z);
                write_r(cpu, bus, y, value);
            }
        }
        2 => {
            let value = read_r(cpu, bus, z);
            alu(cpu, y, value);
        }
        _ => match z {
            0 => match y {
                0..=3 => {
                    if condition(cpu, y) {
                        cpu.registers.pc = pop(cpu, bus);
                    } else {
                        return false;
                    }
                }
                4 => {
                    let offset = fetch8(cpu, bus);
                    bus.write8(0xff00 | u16::from(offset), cpu.registers.a);
                }
                5 => {
                    let offset = i8::from_ne_bytes([fetch8(cpu, bus)]);
                    add_sp(cpu, offset, true);
                }
                6 => {
                    let offset = fetch8(cpu, bus);
                    cpu.registers.a = bus.read8(0xff00 | u16::from(offset));
                }
                _ => {
                    let offset = i8::from_ne_bytes([fetch8(cpu, bus)]);
                    add_sp(cpu, offset, false);
                }
            },
            1 => {
                if q == 0 {
                    let value = pop(cpu, bus);
                    set_rp2(cpu, p, value);
                } else {
                    match p {
                        0 => cpu.registers.pc = pop(cpu, bus),
                        1 => {
                            cpu.registers.pc = pop(cpu, bus);
                            cpu.ime = true;
                            cpu.ime_enable_delay = 0;
                        }
                        2 => cpu.registers.pc = cpu.registers.hl(),
                        _ => cpu.registers.sp = cpu.registers.hl(),
                    }
                }
            }
            2 => {
                if y < 4 {
                    let address = fetch16(cpu, bus);
                    if condition(cpu, y) {
                        cpu.registers.pc = address;
                    } else {
                        return false;
                    }
                } else {
                    match y {
                        4 => bus.write8(0xff00 | u16::from(cpu.registers.c), cpu.registers.a),
                        5 => {
                            let address = fetch16(cpu, bus);
                            bus.write8(address, cpu.registers.a);
                        }
                        6 => cpu.registers.a = bus.read8(0xff00 | u16::from(cpu.registers.c)),
                        _ => {
                            let address = fetch16(cpu, bus);
                            cpu.registers.a = bus.read8(address);
                        }
                    }
                }
            }
            3 => match y {
                0 => cpu.registers.pc = fetch16(cpu, bus),
                1 => unreachable!("CB handled by step"),
                6 => {
                    cpu.ime = false;
                    cpu.ime_enable_delay = 0;
                }
                7 if cpu.ime_enable_delay == 0 => cpu.ime_enable_delay = 2,
                7 => {}
                _ => {}
            },
            4 => {
                let address = fetch16(cpu, bus);
                if condition(cpu, y) {
                    push(cpu, bus, cpu.registers.pc);
                    cpu.registers.pc = address;
                } else {
                    return false;
                }
            }
            5 => {
                if q == 0 {
                    push(cpu, bus, rp2(cpu, p));
                } else if p == 0 {
                    let address = fetch16(cpu, bus);
                    push(cpu, bus, cpu.registers.pc);
                    cpu.registers.pc = address;
                }
            }
            6 => {
                let value = fetch8(cpu, bus);
                alu(cpu, y, value);
            }
            _ => {
                push(cpu, bus, cpu.registers.pc);
                cpu.registers.pc = u16::from(y) * 8;
            }
        },
    }
    true
}

fn alu(cpu: &mut Cpu, operation: u8, value: u8) {
    match operation {
        0 => {
            let (r, f) = add8(cpu.registers.a, value, false);
            cpu.registers.a = r;
            cpu.registers.f = f;
        }
        1 => {
            let (r, f) = add8(cpu.registers.a, value, cpu.registers.f.contains(Flags::C));
            cpu.registers.a = r;
            cpu.registers.f = f;
        }
        2 => {
            let (r, f) = sub8(cpu.registers.a, value, false);
            cpu.registers.a = r;
            cpu.registers.f = f;
        }
        3 => {
            let (r, f) = sub8(cpu.registers.a, value, cpu.registers.f.contains(Flags::C));
            cpu.registers.a = r;
            cpu.registers.f = f;
        }
        4 => {
            cpu.registers.a &= value;
            cpu.registers.f = Flags::H;
            cpu.registers.f.set(Flags::Z, cpu.registers.a == 0);
        }
        5 => {
            cpu.registers.a ^= value;
            cpu.registers.f = Flags::default();
            cpu.registers.f.set(Flags::Z, cpu.registers.a == 0);
        }
        6 => {
            cpu.registers.a |= value;
            cpu.registers.f = Flags::default();
            cpu.registers.f.set(Flags::Z, cpu.registers.a == 0);
        }
        _ => {
            let (_, f) = sub8(cpu.registers.a, value, false);
            cpu.registers.f = f;
        }
    }
}

fn add_hl(cpu: &mut Cpu, value: u16) {
    let hl = cpu.registers.hl();
    let result = hl.wrapping_add(value);
    let z = cpu.registers.f.contains(Flags::Z);
    let mut flags = Flags::default();
    flags.set(Flags::Z, z);
    flags.set(Flags::H, (hl & 0x0fff) + (value & 0x0fff) > 0x0fff);
    flags.set(Flags::C, u32::from(hl) + u32::from(value) > 0xffff);
    cpu.registers.set_hl(result);
    cpu.registers.f = flags;
}

fn add_sp(cpu: &mut Cpu, offset: i8, to_sp: bool) {
    let sp = cpu.registers.sp;
    let unsigned = u16::from(offset.to_ne_bytes()[0]);
    let result = sp.wrapping_add_signed(i16::from(offset));
    let mut flags = Flags::default();
    flags.set(Flags::H, (sp & 0x0f) + (unsigned & 0x0f) > 0x0f);
    flags.set(Flags::C, (sp & 0xff) + (unsigned & 0xff) > 0xff);
    cpu.registers.f = flags;
    if to_sp {
        cpu.registers.sp = result;
    } else {
        cpu.registers.set_hl(result);
    }
}

fn accumulator_misc(cpu: &mut Cpu, operation: u8) {
    let a = cpu.registers.a;
    let carry = cpu.registers.f.contains(Flags::C);
    match operation {
        0 => {
            cpu.registers.a = a.rotate_left(1);
            cpu.registers.f = Flags::default();
            cpu.registers.f.set(Flags::C, a & 0x80 != 0);
        }
        1 => {
            cpu.registers.a = a.rotate_right(1);
            cpu.registers.f = Flags::default();
            cpu.registers.f.set(Flags::C, a & 1 != 0);
        }
        2 => {
            cpu.registers.a = (a << 1) | u8::from(carry);
            cpu.registers.f = Flags::default();
            cpu.registers.f.set(Flags::C, a & 0x80 != 0);
        }
        3 => {
            cpu.registers.a = (a >> 1) | (u8::from(carry) << 7);
            cpu.registers.f = Flags::default();
            cpu.registers.f.set(Flags::C, a & 1 != 0);
        }
        4 => {
            (cpu.registers.a, cpu.registers.f) = daa(a, cpu.registers.f);
        }
        5 => {
            cpu.registers.a = !a;
            let mut f = cpu.registers.f;
            f.set(Flags::N, true);
            f.set(Flags::H, true);
            cpu.registers.f = f;
        }
        6 => {
            let z = cpu.registers.f.contains(Flags::Z);
            cpu.registers.f = Flags::C;
            cpu.registers.f.set(Flags::Z, z);
        }
        _ => {
            let z = cpu.registers.f.contains(Flags::Z);
            cpu.registers.f = Flags::default();
            cpu.registers.f.set(Flags::Z, z);
            cpu.registers.f.set(Flags::C, !carry);
        }
    }
}

fn execute_cb(cpu: &mut Cpu, bus: &mut impl CpuBus, opcode: u8) {
    let x = opcode >> 6;
    let y = (opcode >> 3) & 7;
    let z = opcode & 7;
    let value = read_r(cpu, bus, z);
    match x {
        0 => {
            let carry_in = u8::from(cpu.registers.f.contains(Flags::C));
            let (result, carry) = match y {
                0 => (value.rotate_left(1), value & 0x80 != 0),
                1 => (value.rotate_right(1), value & 1 != 0),
                2 => ((value << 1) | carry_in, value & 0x80 != 0),
                3 => ((value >> 1) | (carry_in << 7), value & 1 != 0),
                4 => (value << 1, value & 0x80 != 0),
                5 => ((value >> 1) | (value & 0x80), value & 1 != 0),
                6 => (value.rotate_left(4), false),
                _ => (value >> 1, value & 1 != 0),
            };
            let mut f = Flags::default();
            f.set(Flags::Z, result == 0);
            f.set(Flags::C, carry);
            cpu.registers.f = f;
            write_r(cpu, bus, z, result);
        }
        1 => {
            let carry = cpu.registers.f.contains(Flags::C);
            let mut f = Flags::H;
            f.set(Flags::Z, value & (1 << y) == 0);
            f.set(Flags::C, carry);
            cpu.registers.f = f;
        }
        2 => write_r(cpu, bus, z, value & !(1 << y)),
        _ => write_r(cpu, bus, z, value | (1 << y)),
    }
}

fn service_interrupt(cpu: &mut Cpu, bus: &mut impl CpuBus, interrupt: Interrupt) {
    cpu.ime = false;
    cpu.ime_enable_delay = 0;
    cpu.mode = CpuMode::Running;
    bus.acknowledge_interrupt(interrupt);
    bus.idle_m_cycle();
    bus.idle_m_cycle();
    push(cpu, bus, cpu.registers.pc);
    bus.idle_m_cycle();
    cpu.registers.pc = interrupt.vector();
}
