// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Address profiles as data (doc 10 section 2.5).

use crate::address::{Area, PlcAddress, Size};

/// Where a rendered address is used.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Purpose {
    /// On the drawing.
    Drawing,
    /// In an exchange file.
    Exchange,
}

/// A PLC address notation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AddressProfile {
    /// Profile id.
    pub id: &'static str,
    /// Profile version.
    pub version: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Whether addresses are absolute.
    pub absolute: bool,
    /// Number base of rendered numbers (only 10 is rendered; other bases are 1.x).
    pub base: u32,
    /// Bits per addressing unit.
    pub unit_bits: u32,
    /// Alignment of multi-unit values, in units.
    pub word_align: u32,
    /// Area mnemonics, order I, Q, M, PI, PQ.
    pub area_mnemonics: [&'static str; 5],
    /// Size mnemonics, order bit, byte, word, dword, lword.
    pub size_mnemonics: [&'static str; 5],
    /// Bit address template.
    pub render_bit: &'static str,
    /// Unit address template.
    pub render_unit: &'static str,
    /// Prefix for drawings.
    pub drawing_prefix: &'static str,
    /// Prefix for exchange files.
    pub exchange_prefix: &'static str,
    /// Default first unit for digital allocation.
    pub default_start_digital: u32,
    /// Default first unit for analog allocation.
    pub default_start_analog: u32,
    /// Whether the allocation rules are verified against the vendor.
    pub allocation_verified: bool,
}

impl AddressProfile {
    /// The `id@version` reference.
    pub fn reference(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }

    /// Render an address; numbers are decimal (other bases are 1.x).
    pub fn render(&self, a: &PlcAddress, purpose: Purpose) -> String {
        let prefix = match purpose {
            Purpose::Drawing => self.drawing_prefix,
            Purpose::Exchange => self.exchange_prefix,
        };
        let template = if a.size == Size::Bit && a.bit.is_some() {
            self.render_bit
        } else {
            self.render_unit
        };
        let area = self.area_mnemonics[a.area as usize];
        let size = self.size_mnemonics[a.size as usize];
        let byte = a.byte.to_string();
        let bit = a.bit.map(|b| b.to_string()).unwrap_or_default();
        let tokens: [(&str, &str); 4] = [
            ("{area}", area),
            ("{size}", size),
            ("{byte}", &byte),
            ("{bit}", &bit),
        ];
        let mut out = String::from(prefix);
        let mut rest = template;
        while let Some(i) = rest.find('{') {
            out.push_str(&rest[..i]);
            let tail = &rest[i..];
            match tokens.iter().find(|(t, _)| tail.starts_with(t)) {
                Some((t, v)) => {
                    out.push_str(v);
                    rest = &tail[t.len()..];
                }
                None => {
                    out.push('{');
                    rest = &tail[1..];
                }
            }
        }
        out.push_str(rest);
        out
    }

    /// The bit address `bit_offset` bits after `start_byte`; `None` when the byte overflows `u32` (a hostile
    /// start or map) or the bit does not fit a `u8`.
    pub fn bit_at(&self, area: Area, start_byte: u32, bit_offset: u32) -> Option<PlcAddress> {
        let byte = start_byte.checked_add(bit_offset / self.unit_bits)?;
        let bit = u8::try_from(bit_offset % self.unit_bits).ok()?;
        Some(PlcAddress::bit(area, byte, bit))
    }

    /// Units needed for `bits` bits.
    pub fn units(&self, bits: u32) -> u32 {
        bits.div_ceil(self.unit_bits)
    }
}

/// Siemens TIA, international mnemonics (doc 10 section 2.5).
pub static SIEMENS_TIA_INTL: AddressProfile = AddressProfile {
    id: "siemens-tia-intl",
    version: "2027.1",
    label: "Siemens TIA (international)",
    absolute: true,
    base: 10,
    unit_bits: 8,
    word_align: 2,
    area_mnemonics: ["I", "Q", "M", "PI", "PQ"],
    size_mnemonics: ["", "B", "W", "D", "L"],
    render_bit: "{area}{byte}.{bit}",
    render_unit: "{area}{size}{byte}",
    drawing_prefix: "",
    exchange_prefix: "%",
    default_start_digital: 0,
    default_start_analog: 0,
    allocation_verified: false,
};

/// The built-in profiles.
pub fn builtins() -> &'static [&'static AddressProfile] {
    static ALL: [&AddressProfile; 1] = [&SIEMENS_TIA_INTL];
    &ALL
}

/// Look up a built-in profile by exact `id@version`.
pub fn builtin(reference: &str) -> Option<&'static AddressProfile> {
    builtins().iter().copied().find(|p| p.reference() == reference)
}
