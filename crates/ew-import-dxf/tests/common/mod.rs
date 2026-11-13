// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! DXF files written by the tests (CC0 by construction: no foreign drawing is used): a drawing as group-code pairs,
//! rendered as ASCII or as binary DXF.

#![allow(dead_code)]

use ew_import_dxf::ew_interop_api::{ImportContext, ImportInput};

/// A drawing as its group-code pairs.
#[derive(Clone, Debug, Default)]
pub struct Dxf {
    pub pairs: Vec<(i32, String)>,
}

impl Dxf {
    /// A drawing of version `acadver` (`AC1009` R12, `AC1015` 2000, `AC1032` 2018) with `$INSUNITS` `units` (none
    /// when `None`).
    pub fn new(acadver: &str, units: Option<i32>) -> Dxf {
        let mut d = Dxf::default();
        d.p(0, "SECTION").p(2, "HEADER").p(9, "$ACADVER").p(1, acadver);
        if let Some(u) = units {
            d.p(9, "$INSUNITS").p(70, u);
        }
        d.p(0, "ENDSEC");
        d
    }

    /// One pair.
    pub fn p(&mut self, code: i32, v: impl std::fmt::Display) -> &mut Dxf {
        self.pairs.push((code, v.to_string()));
        self
    }

    /// Pairs from a list.
    pub fn ps(&mut self, list: &[(i32, &str)]) -> &mut Dxf {
        for (c, v) in list {
            self.p(*c, v);
        }
        self
    }

    /// `0 SECTION 2 <name>`.
    pub fn section(&mut self, name: &str) -> &mut Dxf {
        self.p(0, "SECTION").p(2, name)
    }

    /// `0 ENDSEC`.
    pub fn end(&mut self) -> &mut Dxf {
        self.p(0, "ENDSEC")
    }

    /// The file as ASCII DXF, ended by `0 EOF`.
    pub fn ascii(&self) -> Vec<u8> {
        let mut s = String::new();
        for (c, v) in &self.pairs {
            s.push_str(&format!("{c:>3}\r\n{v}\r\n"));
        }
        s.push_str("  0\r\nEOF\r\n");
        s.into_bytes()
    }

    /// The file as binary DXF (R13 and later: two-byte codes), ended by `0 EOF`.
    pub fn binary(&self) -> Vec<u8> {
        let mut out = b"AutoCAD Binary DXF\r\n\x1a\0".to_vec();
        let mut all = self.pairs.clone();
        all.push((0, "EOF".to_owned()));
        for (c, v) in &all {
            out.extend_from_slice(&(*c as i16).to_le_bytes());
            match c {
                10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => {
                    out.extend_from_slice(&v.parse::<f64>().unwrap().to_le_bytes());
                }
                60..=79 | 170..=179 | 270..=289 | 370..=389 | 400..=409 | 1060..=1070 => {
                    out.extend_from_slice(&v.parse::<i16>().unwrap().to_le_bytes());
                }
                90..=99 | 420..=429 | 440..=459 | 1071 => {
                    out.extend_from_slice(&v.parse::<i32>().unwrap().to_le_bytes());
                }
                160..=169 => out.extend_from_slice(&v.parse::<i64>().unwrap().to_le_bytes()),
                290..=299 => out.push(v.parse::<u8>().unwrap()),
                _ => {
                    out.extend_from_slice(v.as_bytes());
                    out.push(0);
                }
            }
        }
        out
    }
}

/// An input named `name` of `bytes`.
pub fn input(name: &str, bytes: Vec<u8>) -> ImportInput {
    ImportInput::new(name, bytes).unwrap()
}

/// A context for a project of nil ID.
pub fn ctx() -> ImportContext {
    ImportContext::new(ew_import_dxf::ew_interop_api::ew_ids::Id::from_uuid(
        ew_import_dxf::ew_interop_api::ew_ids::Uuid::nil(),
    ))
}
