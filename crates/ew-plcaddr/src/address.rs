// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed PLC addresses.

/// Address area.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Area {
    /// Process image of inputs.
    I,
    /// Process image of outputs.
    Q,
    /// Memory (flag) area.
    M,
    /// Peripheral inputs.
    Pi,
    /// Peripheral outputs.
    Pq,
}

/// Access size.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Size {
    /// One bit.
    Bit,
    /// 8 bits.
    Byte,
    /// 16 bits.
    Word,
    /// 32 bits.
    DWord,
    /// 64 bits.
    LWord,
}

impl Size {
    /// Width in bits.
    pub const fn bits(self) -> u32 {
        match self {
            Size::Bit => 1,
            Size::Byte => 8,
            Size::Word => 16,
            Size::DWord => 32,
            Size::LWord => 64,
        }
    }
}

/// An absolute PLC address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PlcAddress {
    /// The area.
    pub area: Area,
    /// The access size.
    pub size: Size,
    /// The byte (unit) offset.
    pub byte: u32,
    /// The bit within the unit, for bit addresses.
    pub bit: Option<u8>,
}

impl PlcAddress {
    /// A bit address.
    pub const fn bit(area: Area, byte: u32, bit: u8) -> PlcAddress {
        PlcAddress {
            area,
            size: Size::Bit,
            byte,
            bit: Some(bit),
        }
    }

    /// A byte/word/dword/lword address without a bit.
    pub const fn unit(area: Area, size: Size, byte: u32) -> PlcAddress {
        PlcAddress {
            area,
            size,
            byte,
            bit: None,
        }
    }
}
