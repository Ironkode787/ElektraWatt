// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The group-code pairs of a DXF file, ASCII or binary (R12 one-byte codes, R13 and later two-byte codes).
//!
//! A pair is a group code and a value whose type the code decides (the DXF reference's group code value types).
//! Strings are decoded as Windows-1252 before AutoCAD 2007 (`$ACADVER` below AC1021) and as UTF-8 from then on, and
//! `\U+XXXX` escapes become their character. Every string is bounded by the context's `max_text_bytes`.

use ew_interop_api::{ImportError, Limits};

/// The sentinel that opens a binary DXF file.
pub(crate) const BINARY_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

/// A pair's value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Val {
    /// A string (codes 0–9, 100–105, 300–369, 390–399, 410–419, 430–439, 470–481, 999, 1000–1009).
    Str(String),
    /// A real number.
    Num(f64),
    /// An integer of any width.
    Int(i64),
    /// Binary data (codes 310–319 and 1004 of a binary file; hex text of an ASCII file stays a string).
    Bin,
}

/// One group-code pair and the 1-based line of its code (ASCII files only).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pair {
    pub code: i32,
    pub val: Val,
    pub line: Option<u32>,
}

impl Pair {
    /// The value as text: a string, or a number written plainly.
    pub fn text(&self) -> String {
        match &self.val {
            Val::Str(s) => s.clone(),
            Val::Num(n) => n.to_string(),
            Val::Int(i) => i.to_string(),
            Val::Bin => String::new(),
        }
    }

    /// The value as a real number (an integer converts).
    pub fn num(&self) -> Option<f64> {
        match self.val {
            Val::Num(n) => Some(n),
            Val::Int(i) => Some(i as f64),
            _ => None,
        }
    }

    /// The value as an integer (a whole real number converts).
    pub fn int(&self) -> Option<i64> {
        match self.val {
            Val::Int(i) => Some(i),
            Val::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Some(n as i64),
            _ => None,
        }
    }

    /// Whether this is the pair `0 <name>`.
    pub fn is(&self, name: &str) -> bool {
        self.code == 0 && matches!(&self.val, Val::Str(s) if s.trim().eq_ignore_ascii_case(name))
    }
}

/// The value type of a group code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Str,
    Num,
    I16,
    I32,
    I64,
    Bool,
    Bin,
}

fn kind(code: i32) -> Option<Kind> {
    Some(match code {
        0..=9 | 100..=102 | 105 | 300..=309 | 320..=369 | 390..=399 | 410..=419 | 430..=439 | 470..=481 => {
            Kind::Str
        }
        999 | 1000..=1003 | 1005..=1009 => Kind::Str,
        10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => Kind::Num,
        60..=79 | 170..=179 | 270..=289 | 370..=389 | 400..=409 | 1060..=1070 => Kind::I16,
        90..=99 | 420..=429 | 440..=459 | 1071 => Kind::I32,
        160..=169 => Kind::I64,
        290..=299 => Kind::Bool,
        310..=319 | 1004 => Kind::Bin,
        _ => return None,
    })
}

/// How a binary file writes its group codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Binary {
    /// R12: one byte, 255 followed by a 16-bit code.
    OneByte,
    /// R13 and later: two bytes.
    TwoBytes,
}

/// The pairs of one file, read on demand with one pair of look-ahead.
pub(crate) struct Pairs<'a> {
    bytes: &'a [u8],
    pos: usize,
    line: u32,
    binary: Option<Binary>,
    /// Strings are UTF-8 (AC1021 and later) rather than Windows-1252.
    pub utf8: bool,
    peeked: Option<Pair>,
    max_text: usize,
}

fn malformed(message: impl Into<String>, line: Option<u32>) -> ImportError {
    ImportError::Malformed {
        message: message.into(),
        line,
    }
}

impl<'a> Pairs<'a> {
    /// The pairs of `bytes` under `limits`.
    pub fn new(bytes: &'a [u8], limits: &Limits) -> Pairs<'a> {
        let (binary, pos) = if bytes.starts_with(BINARY_SENTINEL) {
            let rest = &bytes[BINARY_SENTINEL.len()..];
            let two = rest.len() >= 2 && rest[0] == 0 && rest[1] == 0;
            (
                Some(if two { Binary::TwoBytes } else { Binary::OneByte }),
                BINARY_SENTINEL.len(),
            )
        } else {
            // A UTF-8 byte-order mark before the first code.
            let bom = if bytes.starts_with(b"\xef\xbb\xbf") { 3 } else { 0 };
            (None, bom)
        };
        Pairs {
            bytes,
            pos,
            line: 0,
            binary,
            utf8: false,
            peeked: None,
            max_text: usize::try_from(limits.max_text_bytes).unwrap_or(usize::MAX),
        }
    }

    /// Whether the file is binary DXF.
    pub fn is_binary(&self) -> bool {
        self.binary.is_some()
    }

    /// The next pair without consuming it.
    pub fn peek(&mut self) -> Result<Option<&Pair>, ImportError> {
        if self.peeked.is_none() {
            self.peeked = self.read()?;
        }
        Ok(self.peeked.as_ref())
    }

    /// The next pair.
    pub fn next_pair(&mut self) -> Result<Option<Pair>, ImportError> {
        match self.peeked.take() {
            Some(p) => Ok(Some(p)),
            None => self.read(),
        }
    }

    fn read(&mut self) -> Result<Option<Pair>, ImportError> {
        match self.binary {
            None => self.read_text(),
            Some(b) => self.read_binary(b),
        }
    }

    /// The next line without its line end, or `None` at the end of the input.
    fn line(&mut self) -> Result<Option<&'a [u8]>, ImportError> {
        if self.pos >= self.bytes.len() {
            return Ok(None);
        }
        let rest = &self.bytes[self.pos..];
        let end = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
        if end > self.max_text {
            return Err(ImportError::LimitExceeded {
                what: "DXF line length".to_owned(),
                limit: self.max_text as u64,
            });
        }
        let mut line = &rest[..end];
        if line.last() == Some(&b'\r') {
            line = &line[..line.len() - 1];
        }
        self.pos += (end + 1).min(rest.len());
        self.line = self.line.saturating_add(1);
        Ok(Some(line))
    }

    fn read_text(&mut self) -> Result<Option<Pair>, ImportError> {
        // Blank lines between pairs (a trailing newline at the end of the file) are skipped.
        let code_line = loop {
            match self.line()? {
                None => return Ok(None),
                Some(l) if l.iter().all(u8::is_ascii_whitespace) => continue,
                Some(l) => break l,
            }
        };
        let at = Some(self.line);
        let code = std::str::from_utf8(code_line)
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .ok_or_else(|| malformed("a group code is not a number", at))?;
        let Some(value) = self.line()? else {
            return Err(malformed("the file ends after a group code", at));
        };
        let k = kind(code).ok_or_else(|| malformed(format!("unknown group code {code}"), at))?;
        let val = match k {
            Kind::Str | Kind::Bin => Val::Str(self.decode(value)),
            Kind::Num => Val::Num(
                std::str::from_utf8(value)
                    .ok()
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .ok_or_else(|| malformed(format!("the value of group code {code} is not a number"), at))?,
            ),
            Kind::I16 | Kind::I32 | Kind::I64 | Kind::Bool => {
                let s = std::str::from_utf8(value).unwrap_or("").trim();
                let i = s
                    .parse::<i64>()
                    .ok()
                    .or_else(|| s.parse::<f64>().ok().filter(|f| f.fract() == 0.0).map(|f| f as i64))
                    .ok_or_else(|| {
                        malformed(format!("the value of group code {code} is not an integer"), at)
                    })?;
                Val::Int(i)
            }
        };
        Ok(Some(Pair { code, val, line: at }))
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ImportError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or_else(|| malformed("the binary file ends inside a value", None))?;
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn read_binary(&mut self, b: Binary) -> Result<Option<Pair>, ImportError> {
        if self.pos >= self.bytes.len() {
            return Ok(None);
        }
        let code = match b {
            Binary::TwoBytes => {
                let c = self.take(2)?;
                i32::from(i16::from_le_bytes([c[0], c[1]]))
            }
            Binary::OneByte => {
                let c = self.take(1)?[0];
                if c == 255 {
                    let c = self.take(2)?;
                    i32::from(i16::from_le_bytes([c[0], c[1]]))
                } else {
                    i32::from(c)
                }
            }
        };
        let k = kind(code).ok_or_else(|| malformed(format!("unknown group code {code}"), None))?;
        let val = match k {
            Kind::Str => {
                let rest = &self.bytes[self.pos..];
                let end = rest
                    .iter()
                    .position(|&c| c == 0)
                    .ok_or_else(|| malformed("the binary file ends inside a string", None))?;
                if end > self.max_text {
                    return Err(ImportError::LimitExceeded {
                        what: "DXF string length".to_owned(),
                        limit: self.max_text as u64,
                    });
                }
                let s = self.take(end)?;
                self.take(1)?;
                Val::Str(self.decode(s))
            }
            Kind::Num => {
                let v = self.take(8)?;
                Val::Num(f64::from_le_bytes(v.try_into().unwrap_or([0; 8])))
            }
            Kind::I16 => {
                let v = self.take(2)?;
                Val::Int(i64::from(i16::from_le_bytes([v[0], v[1]])))
            }
            Kind::I32 => {
                let v = self.take(4)?;
                Val::Int(i64::from(i32::from_le_bytes(v.try_into().unwrap_or([0; 4]))))
            }
            Kind::I64 => {
                let v = self.take(8)?;
                Val::Int(i64::from_le_bytes(v.try_into().unwrap_or([0; 8])))
            }
            Kind::Bool => match b {
                Binary::TwoBytes => Val::Int(i64::from(self.take(1)?[0])),
                Binary::OneByte => {
                    let v = self.take(2)?;
                    Val::Int(i64::from(i16::from_le_bytes([v[0], v[1]])))
                }
            },
            Kind::Bin => {
                let n = usize::from(self.take(1)?[0]);
                self.take(n)?;
                Val::Bin
            }
        };
        Ok(Some(Pair { code, val, line: None }))
    }

    /// The text of string bytes: UTF-8 or Windows-1252, with `\U+XXXX` escapes resolved.
    fn decode(&self, bytes: &[u8]) -> String {
        let s = if self.utf8 {
            String::from_utf8_lossy(bytes).into_owned()
        } else {
            bytes.iter().map(|&b| cp1252(b)).collect()
        };
        unescape_unicode(&s)
    }
}

/// The character of a Windows-1252 byte.
fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{2c6}',
        '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}', '\u{90}', '\u{2018}',
        '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}',
        '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
    ];
    match b {
        0x80..=0x9f => HIGH[usize::from(b - 0x80)],
        _ => char::from(b),
    }
}

/// `s` with each `\U+XXXX` (four hex digits) replaced by its character; an escape of no valid character stays.
pub(crate) fn unescape_unicode(s: &str) -> String {
    if !s.contains("\\U+") && !s.contains("\\u+") {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(['\\']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let b = tail.as_bytes();
        let hex = tail
            .get(3..7)
            .filter(|_| matches!(b.get(1), Some(b'U' | b'u')) && b.get(2) == Some(&b'+'));
        match hex.and_then(|h| u32::from_str_radix(h, 16).ok()).and_then(char::from_u32) {
            Some(c) => {
                out.push(c);
                rest = &tail[7..];
            }
            None => {
                out.push('\\');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
