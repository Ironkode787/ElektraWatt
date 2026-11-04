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

/// The members of an address profile, as [`AddressProfile::new`] takes them (a profile file's data, doc 10 section
/// 2.5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProfileFields {
    /// Profile id.
    pub id: String,
    /// Profile version.
    pub version: String,
    /// Display label.
    pub label: String,
    /// Whether addresses are absolute.
    pub absolute: bool,
    /// Number base of rendered numbers (only 10 is rendered; other bases are 1.x).
    pub base: u32,
    /// Bits per addressing unit.
    pub unit_bits: u32,
    /// Alignment of multi-unit values, in units.
    pub word_align: u32,
    /// Area mnemonics, order I, Q, M, PI, PQ.
    pub area_mnemonics: [String; 5],
    /// Size mnemonics, order bit, byte, word, dword, lword.
    pub size_mnemonics: [String; 5],
    /// Bit address template.
    pub render_bit: String,
    /// Unit address template.
    pub render_unit: String,
    /// Prefix for drawings.
    pub drawing_prefix: String,
    /// Prefix for exchange files.
    pub exchange_prefix: String,
    /// Default first unit for digital allocation.
    pub default_start_digital: u32,
    /// Default first unit for analog allocation.
    pub default_start_analog: u32,
    /// Whether the allocation rules are verified against the vendor.
    pub allocation_verified: bool,
}

/// Why profile data is no address profile this engine renders (WP-S7.03 D7, T2's reasons).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProfileError {
    /// The bit template has no `{bit}`.
    NoBit,
    /// The unit template has no `{byte}`.
    NoByte,
    /// An area mnemonic is empty.
    EmptyArea,
    /// The base is not 10.
    Base,
    /// A unit is not 8 bits.
    UnitBits,
}

impl core::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            ProfileError::NoBit => "its bit template has no {bit}",
            ProfileError::NoByte => "its unit template has no {byte}",
            ProfileError::EmptyArea => "an area mnemonic is empty",
            ProfileError::Base => "only base 10 is rendered",
            ProfileError::UnitBits => "only 8 bits per unit are rendered",
        })
    }
}

impl std::error::Error for ProfileError {}

/// A PLC address notation: profile data that [`AddressProfile::new`] validated. Its members read through
/// [`ProfileFields`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AddressProfile(ProfileFields);

impl core::ops::Deref for AddressProfile {
    type Target = ProfileFields;

    fn deref(&self) -> &ProfileFields {
        &self.0
    }
}

impl AddressProfile {
    /// The profile of `fields`, when this engine renders it: a bit template with `{bit}`, a unit template with
    /// `{byte}`, no empty area mnemonic, base 10 and 8 bits per unit.
    ///
    /// # Errors
    ///
    /// The first [`ProfileError`] in that order.
    pub fn new(fields: ProfileFields) -> Result<AddressProfile, ProfileError> {
        if !fields.render_bit.contains("{bit}") {
            return Err(ProfileError::NoBit);
        }
        if !fields.render_unit.contains("{byte}") {
            return Err(ProfileError::NoByte);
        }
        if fields.area_mnemonics.iter().any(String::is_empty) {
            return Err(ProfileError::EmptyArea);
        }
        if fields.base != 10 {
            return Err(ProfileError::Base);
        }
        if fields.unit_bits != 8 {
            return Err(ProfileError::UnitBits);
        }
        Ok(AddressProfile(fields))
    }

    /// The `id@version` reference.
    pub fn reference(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }

    /// Render an address; numbers are decimal (other bases are 1.x).
    pub fn render(&self, a: &PlcAddress, purpose: Purpose) -> String {
        let prefix = match purpose {
            Purpose::Drawing => self.drawing_prefix.as_str(),
            Purpose::Exchange => self.exchange_prefix.as_str(),
        };
        let template = if a.size == Size::Bit && a.bit.is_some() {
            self.render_bit.as_str()
        } else {
            self.render_unit.as_str()
        };
        let area = self.area_mnemonics[a.area as usize].as_str();
        let size = self.size_mnemonics[a.size as usize].as_str();
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

    /// The example address of this notation that a refusal names (`Q0.1`): output byte 0, bit 1, rendered for the
    /// drawing.
    pub fn example(&self) -> String {
        self.render(&PlcAddress::bit(Area::Q, 0, 1), Purpose::Drawing)
    }

    /// Parses an address text of this notation, the inverse of [`AddressProfile::render`] for either purpose: the
    /// drawing or exchange prefix (when the profile has one) is optional, surrounding white space is ignored and the
    /// mnemonics match without regard to ASCII case. A bit address follows the bit template and its bit must be below
    /// `unit_bits` (`I4.9` is no address of an 8-bit unit); a unit address follows the unit template with a size
    /// other than a bit. When the bit template names a size (`{size}`, the IEC profile's `X`), a bit address may omit
    /// it (doc 10 §2.5: `%Q0.1` parses as the bit `%QX0.1`); a unit address always names its size. `None` for any
    /// other text, a byte beyond `u32` included.
    pub fn parse(&self, text: &str) -> Option<PlcAddress> {
        let text = text.trim();
        let mut bodies = vec![text];
        for prefix in [self.exchange_prefix.as_str(), self.drawing_prefix.as_str()] {
            if let Some(rest) = strip_prefix_ci(text, prefix).filter(|_| !prefix.is_empty()) {
                bodies.push(rest);
            }
        }
        let bit = tokens(&self.render_bit);
        let bit_unsized: Vec<Token> = bit.iter().copied().filter(|t| *t != Token::Size).collect();
        let unit = tokens(&self.render_unit);
        for body in bodies {
            if let Some(a) = self.parse_template(&bit, body, true) {
                return Some(a);
            }
            if bit_unsized.len() < bit.len() {
                if let Some(a) = self.parse_template(&bit_unsized, body, true) {
                    return Some(a);
                }
            }
            if let Some(a) = self.parse_template(&unit, body, false) {
                return Some(a);
            }
        }
        None
    }

    /// The start byte a module-start text names (WP-S1.21 D8, `plc.set_start`): the profile's bit address at bit 0
    /// (`Q10.0`), a unit address of any size (`QB10`, `%QX10` under IEC) or the area and the byte alone (`Q10`), each
    /// with the optional drawing or exchange prefix, white space and ASCII case as [`AddressProfile::parse`] takes
    /// them. `None` for any other text, a bit address at another bit included.
    pub fn parse_start(&self, text: &str) -> Option<(Area, u32)> {
        if let Some(a) = self.parse(text) {
            return match a.bit {
                Some(0) | None => Some((a.area, a.byte)),
                Some(_) => None,
            };
        }
        let text = text.trim();
        let mut bodies = vec![text];
        for prefix in [self.exchange_prefix.as_str(), self.drawing_prefix.as_str()] {
            if let Some(rest) = strip_prefix_ci(text, prefix).filter(|_| !prefix.is_empty()) {
                bodies.push(rest);
            }
        }
        let bare = [Token::Area, Token::Byte];
        let unit = tokens(&self.render_unit);
        for body in bodies {
            for template in [&unit[..], &bare[..]] {
                // No let chain: the permissive crates build on Rust 1.85 (the MSRV check in ci.yml's supply-chain job).
                if let Some(p) = self
                    .match_tokens(template, body, Parsed::default())
                    .filter(|p| p.bit.is_none())
                {
                    return Some((p.area?, p.byte?));
                }
            }
        }
        None
    }

    /// `text` matched against the template `tokens`: a bit address when `bit`, else a unit address.
    fn parse_template(&self, tokens: &[Token], text: &str, bit: bool) -> Option<PlcAddress> {
        let parsed = self.match_tokens(tokens, text, Parsed::default())?;
        let area = parsed.area?;
        let size = parsed.size.unwrap_or(Size::Bit);
        let byte = parsed.byte?;
        if bit {
            (size == Size::Bit).then_some(())?;
            Some(PlcAddress::bit(area, byte, parsed.bit?))
        } else {
            (size != Size::Bit && parsed.bit.is_none()).then_some(())?;
            Some(PlcAddress::unit(area, size, byte))
        }
    }

    /// The first way `tokens` match all of `text` (backtracking over the mnemonics and digit runs).
    fn match_tokens(&self, tokens: &[Token], text: &str, at: Parsed) -> Option<Parsed> {
        let Some((first, rest)) = tokens.split_first() else {
            return text.is_empty().then_some(at);
        };
        match first {
            Token::Lit(c) => {
                let mut chars = text.chars();
                let next = chars.next()?;
                if next.eq_ignore_ascii_case(c) {
                    self.match_tokens(rest, chars.as_str(), at)
                } else {
                    None
                }
            }
            Token::Area => AREAS.iter().zip(&self.area_mnemonics).find_map(|(area, m)| {
                let tail = strip_prefix_ci(text, m).filter(|_| !m.is_empty())?;
                self.match_tokens(
                    rest,
                    tail,
                    Parsed {
                        area: Some(*area),
                        ..at
                    },
                )
            }),
            Token::Size => SIZES.iter().zip(&self.size_mnemonics).find_map(|(size, m)| {
                let tail = strip_prefix_ci(text, m)?;
                self.match_tokens(
                    rest,
                    tail,
                    Parsed {
                        size: Some(*size),
                        ..at
                    },
                )
            }),
            Token::Byte | Token::Bit => {
                // A byte is a `u32` (at most 10 digits) and a bit fewer: longer splits never parse, and capping the
                // run keeps a long digit string from costing a quadratic search (FX37a).
                let run = text
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .take(MAX_DIGITS)
                    .count();
                (1..=run).rev().find_map(|n| {
                    let (digits, tail) = text.split_at(n);
                    let next = if *first == Token::Byte {
                        Parsed {
                            byte: Some(digits.parse().ok()?),
                            ..at
                        }
                    } else {
                        let b: u8 = digits.parse().ok()?;
                        if u32::from(b) >= self.unit_bits {
                            return None;
                        }
                        Parsed { bit: Some(b), ..at }
                    };
                    self.match_tokens(rest, tail, next)
                })
            }
        }
    }
}

/// The most digits a byte or bit number of an address has: the width of `u32::MAX`.
const MAX_DIGITS: usize = 10;

/// The areas in the order of [`AddressProfile::area_mnemonics`].
const AREAS: [Area; 5] = [Area::I, Area::Q, Area::M, Area::Pi, Area::Pq];

/// The sizes in the order of [`AddressProfile::size_mnemonics`].
const SIZES: [Size; 5] = [Size::Bit, Size::Byte, Size::Word, Size::DWord, Size::LWord];

/// One element of a render template.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Token {
    Lit(char),
    Area,
    Size,
    Byte,
    Bit,
}

/// What a parse has read so far.
#[derive(Clone, Copy, Default)]
struct Parsed {
    area: Option<Area>,
    size: Option<Size>,
    byte: Option<u32>,
    bit: Option<u8>,
}

/// The tokens of a render template; a `{` that opens no known token is a literal, as [`AddressProfile::render`]
/// treats it.
fn tokens(template: &str) -> Vec<Token> {
    const NAMED: [(&str, Token); 4] = [
        ("{area}", Token::Area),
        ("{size}", Token::Size),
        ("{byte}", Token::Byte),
        ("{bit}", Token::Bit),
    ];
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(c) = rest.chars().next() {
        match NAMED.iter().find(|(name, _)| rest.starts_with(name)) {
            Some((name, token)) => {
                out.push(*token);
                rest = &rest[name.len()..];
            }
            None => {
                out.push(Token::Lit(c));
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// `text` without `prefix`, compared without regard to ASCII case.
fn strip_prefix_ci<'t>(text: &'t str, prefix: &str) -> Option<&'t str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &text[prefix.len()..])
}
