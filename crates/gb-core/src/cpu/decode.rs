#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Instruction {
    Base(u8),
    Prefixed(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecodedInstruction {
    pub(crate) instruction: Instruction,
    pub(crate) length: u8,
    pub(crate) cycles_taken: u8,
    pub(crate) cycles_not_taken: u8,
    pub(crate) illegal: bool,
}

const ILLEGAL: [u8; 11] = [
    0xd3, 0xdb, 0xdd, 0xe3, 0xe4, 0xeb, 0xec, 0xed, 0xf4, 0xfc, 0xfd,
];

pub(crate) fn decode(opcode: u8, prefixed: bool) -> DecodedInstruction {
    if prefixed {
        let target = opcode & 7;
        let group = opcode >> 6;
        let cycles = if target == 6 {
            if group == 1 { 12 } else { 16 }
        } else {
            8
        };
        return DecodedInstruction {
            instruction: Instruction::Prefixed(opcode),
            length: 2,
            cycles_taken: cycles,
            cycles_not_taken: cycles,
            illegal: false,
        };
    }
    let illegal = ILLEGAL.contains(&opcode);
    let (length, taken, not_taken) = base_metadata(opcode);
    DecodedInstruction {
        instruction: Instruction::Base(opcode),
        length,
        cycles_taken: taken,
        cycles_not_taken: not_taken,
        illegal,
    }
}

#[allow(clippy::match_same_arms)]
fn base_metadata(opcode: u8) -> (u8, u8, u8) {
    let x = opcode >> 6;
    let y = (opcode >> 3) & 7;
    let z = opcode & 7;
    match x {
        0 => match z {
            0 => match y {
                0 => (1, 4, 4),
                1 => (3, 20, 20),
                2 => (2, 4, 4),
                3 => (2, 12, 12),
                _ => (2, 12, 8),
            },
            1 => {
                if y & 1 == 0 {
                    (3, 12, 12)
                } else {
                    (1, 8, 8)
                }
            }
            2 | 3 => (1, 8, 8),
            4 | 5 => (1, if y == 6 { 12 } else { 4 }, if y == 6 { 12 } else { 4 }),
            6 => (2, if y == 6 { 12 } else { 8 }, if y == 6 { 12 } else { 8 }),
            _ => (1, 4, 4),
        },
        1 => {
            let cycles = if opcode == 0x76 {
                4
            } else if y == 6 || z == 6 {
                8
            } else {
                4
            };
            (1, cycles, cycles)
        }
        2 => {
            let cycles = if z == 6 { 8 } else { 4 };
            (1, cycles, cycles)
        }
        _ => match z {
            0 => match y {
                0..=3 => (1, 20, 8),
                4 | 6 => (2, 12, 12),
                5 => (2, 16, 16),
                _ => (2, 12, 12),
            },
            1 => {
                if y & 1 == 0 {
                    (1, 12, 12)
                } else {
                    match y >> 1 {
                        0 | 1 => (1, 16, 16),
                        2 => (1, 4, 4),
                        _ => (1, 8, 8),
                    }
                }
            }
            2 => {
                if y < 4 {
                    (3, 16, 12)
                } else if y == 5 || y == 7 {
                    (3, 16, 16)
                } else {
                    (1, 8, 8)
                }
            }
            3 => match y {
                0 => (3, 16, 16),
                1 => (2, 8, 8),
                _ => (1, 4, 4),
            },
            4 => (3, 24, 12),
            5 => {
                if y & 1 == 0 {
                    (1, 16, 16)
                } else {
                    (3, 24, 24)
                }
            }
            6 => (2, 8, 8),
            _ => (1, 16, 16),
        },
    }
}
