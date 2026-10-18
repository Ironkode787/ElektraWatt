// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Designation`: a parsed or assembled reference designation.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::str::FromStr;

use crate::{Aspect, KnownCodes, NodeMatch, ParseError, ParseErrorKind, RdsSyntax, parse_strict};

/// One single-level designation inside an aspect: `P01`, `J-E-2416`, `201U3`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Level {
    text: String,
    node: Option<NodeMatch>,
}

impl Level {
    /// The level as written.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The known node code or alias the parser matched, if any ([`KnownCodes`]).
    pub fn node(&self) -> Option<&NodeMatch> {
        self.node.as_ref()
    }

    /// The node code if the level matched one (resolving an alias), else the text.
    pub fn code(&self) -> &str {
        self.node.as_ref().map_or(&self.text, |n| &n.code)
    }

    pub(crate) fn parsed(text: &str, node: Option<NodeMatch>) -> Level {
        Level {
            text: text.to_owned(),
            node,
        }
    }
}

/// A reference designation: levels per aspect, an optional connection point and an optional signal
/// (IEC 81346-1; domain model §8.1).
///
/// ```text
/// [==FA][=P01][++SITE][+J-E-2416][&EFS]-101F1[:N][;SIG]
/// ```
///
/// Aspects are held in canonical order ([`Aspect`]); each holds one or more levels (a multi-level
/// designation, FR-PRJ-015: `-201U3-Q1` is two product levels). Designations are **derived** values, never
/// stored: tags are properties of devices, rendered from `TagSpec` and structure nodes (ADR-0001,
/// ADR-0026), so this type has no serde implementation.
///
/// Build one with [`push`](Designation::push), or parse text with [`parse_strict`] or
/// [`parse_lenient`](crate::parse_lenient). [`format`](Designation::format) writes it with a project's
/// signs; `Display` and `FromStr` use the default signs and no known codes.
///
/// ```
/// use ew_rds::{Aspect, Designation};
///
/// let mut d = Designation::new();
/// d.push(Aspect::Product, "X1031").unwrap();
/// d.push(Aspect::Location, "J-E-2416").unwrap();
/// d.push(Aspect::Function, "P01").unwrap();
/// d.set_connection_point(Some("1")).unwrap();
/// assert_eq!(d.to_string(), "=P01+J-E-2416-X1031:1");
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Designation {
    aspects: BTreeMap<Aspect, Vec<Level>>,
    connection_point: Option<String>,
    signal: Option<String>,
}

impl Designation {
    /// An empty designation.
    pub fn new() -> Designation {
        Designation::default()
    }

    pub(crate) fn from_parts(
        aspects: BTreeMap<Aspect, Vec<Level>>,
        connection_point: Option<String>,
        signal: Option<String>,
    ) -> Designation {
        Designation {
            aspects,
            connection_point,
            signal,
        }
    }

    /// Appends a level to `aspect` (a deeper level if the aspect already has one).
    ///
    /// The text is taken as one atomic level, even if it contains prefix characters (`J-E-2416`): that is
    /// how derivations render structure codes (FR-PRJ-017).
    ///
    /// # Errors
    ///
    /// [`ParseErrorKind::EmptyLevel`] for empty text (at offset 0).
    pub fn push(&mut self, aspect: Aspect, text: &str) -> Result<&mut Designation, ParseError> {
        if text.is_empty() {
            return Err(ParseError {
                kind: ParseErrorKind::EmptyLevel,
                offset: 0,
            });
        }
        self.aspects
            .entry(aspect)
            .or_default()
            .push(Level::parsed(text, None));
        Ok(self)
    }

    /// The levels of `aspect`, outermost first; empty if the aspect is absent.
    pub fn levels(&self, aspect: Aspect) -> &[Level] {
        self.aspects.get(&aspect).map_or(&[], Vec::as_slice)
    }

    /// The aspects present, in canonical order, with their levels.
    pub fn aspects(&self) -> impl Iterator<Item = (Aspect, &[Level])> {
        self.aspects.iter().map(|(a, l)| (*a, l.as_slice()))
    }

    /// The connection point after `:`, such as `1` of `-X1031:1`.
    pub fn connection_point(&self) -> Option<&str> {
        self.connection_point.as_deref()
    }

    /// Sets or clears the connection point.
    ///
    /// # Errors
    ///
    /// [`ParseErrorKind::EmptyConnectionPoint`] for `Some("")`.
    pub fn set_connection_point(&mut self, cp: Option<&str>) -> Result<(), ParseError> {
        if cp == Some("") {
            return Err(ParseError {
                kind: ParseErrorKind::EmptyConnectionPoint,
                offset: 0,
            });
        }
        self.connection_point = cp.map(str::to_owned);
        Ok(())
    }

    /// The signal designation after `;`.
    pub fn signal(&self) -> Option<&str> {
        self.signal.as_deref()
    }

    /// Sets or clears the signal designation.
    ///
    /// # Errors
    ///
    /// [`ParseErrorKind::EmptySignal`] for `Some("")`.
    pub fn set_signal(&mut self, signal: Option<&str>) -> Result<(), ParseError> {
        if signal == Some("") {
            return Err(ParseError {
                kind: ParseErrorKind::EmptySignal,
                offset: 0,
            });
        }
        self.signal = signal.map(str::to_owned);
        Ok(())
    }

    /// Whether the designation has no level, connection point or signal.
    pub fn is_empty(&self) -> bool {
        self.aspects.is_empty() && self.connection_point.is_none() && self.signal.is_none()
    }

    /// Replaces every level that matched an alias by the node code it names, so the alias is never
    /// printed (FR-PRJ-028): `+JE2416-X1031` becomes `+J-E-2416-X1031`.
    pub fn resolve_aliases(&mut self) {
        for level in self.aspects.values_mut().flatten() {
            if let Some(node) = &mut level.node {
                if node.via_alias {
                    level.text.clone_from(&node.code);
                    node.via_alias = false;
                }
            }
        }
    }

    /// The text with the signs and separators of `syntax`: aspects in canonical order, the first level of
    /// an aspect after its sign and each deeper level after the aspect's separator (or the sign again),
    /// then `:` and the connection point, then `;` and the signal.
    ///
    /// ```
    /// use ew_rds::{Aspect, Designation, RdsSyntax};
    ///
    /// let d: Designation = "+J+DOOR-K1".parse().unwrap();
    /// let mut syntax = RdsSyntax::default();
    /// syntax.set_separator(Aspect::Location, Some(".")).unwrap();
    /// assert_eq!(d.format(&syntax), "+J.DOOR-K1");
    /// ```
    pub fn format(&self, syntax: &RdsSyntax) -> String {
        let mut out = String::new();
        for (aspect, levels) in &self.aspects {
            for (i, level) in levels.iter().enumerate() {
                let token = match syntax.separator(*aspect) {
                    Some(sep) if i > 0 => sep,
                    _ => syntax.sign(*aspect),
                };
                out.push_str(token);
                out.push_str(&level.text);
            }
        }
        if let Some(cp) = &self.connection_point {
            let _ = write!(out, ":{cp}");
        }
        if let Some(signal) = &self.signal {
            let _ = write!(out, ";{signal}");
        }
        out
    }
}

impl fmt::Display for Designation {
    /// The text with the default signs ([`RdsSyntax::default`]).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format(&RdsSyntax::default()))
    }
}

impl FromStr for Designation {
    type Err = ParseError;

    /// [`parse_strict`] with the default signs and no known codes.
    fn from_str(s: &str) -> Result<Designation, ParseError> {
        parse_strict(s, &RdsSyntax::default(), &KnownCodes::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_format() {
        let mut d = Designation::new();
        assert!(d.is_empty());
        d.push(Aspect::Product, "201U3")
            .unwrap()
            .push(Aspect::Product, "Q1")
            .unwrap();
        d.push(Aspect::Function, "P01").unwrap();
        d.push(Aspect::Location, "J-E-2416").unwrap();
        assert!(!d.is_empty());
        assert_eq!(d.to_string(), "=P01+J-E-2416-201U3-Q1");
        assert_eq!(d.levels(Aspect::Product).len(), 2);
        assert_eq!(d.levels(Aspect::Type), &[]);
        assert_eq!(
            d.aspects().map(|(a, _)| a).collect::<Vec<_>>(),
            [Aspect::Function, Aspect::Location, Aspect::Product]
        );
        d.set_connection_point(Some("14")).unwrap();
        d.set_signal(Some("RUN")).unwrap();
        assert_eq!(d.to_string(), "=P01+J-E-2416-201U3-Q1:14;RUN");
        assert_eq!(d.connection_point(), Some("14"));
        assert_eq!(d.signal(), Some("RUN"));
        d.set_connection_point(None).unwrap();
        d.set_signal(None).unwrap();
        assert_eq!(d.to_string(), "=P01+J-E-2416-201U3-Q1");
        assert_eq!(
            d.push(Aspect::Product, "").unwrap_err().kind,
            ParseErrorKind::EmptyLevel
        );
        assert_eq!(
            d.set_connection_point(Some("")).unwrap_err().kind,
            ParseErrorKind::EmptyConnectionPoint
        );
        assert_eq!(
            d.set_signal(Some("")).unwrap_err().kind,
            ParseErrorKind::EmptySignal
        );
    }

    #[test]
    fn custom_signs_and_separators() {
        let mut d = Designation::new();
        d.push(Aspect::Location, "J-E-2416").unwrap();
        d.push(Aspect::Location, "DOOR").unwrap();
        d.push(Aspect::Product, "K1").unwrap();
        let mut syntax = RdsSyntax::default();
        assert_eq!(d.format(&syntax), "+J-E-2416+DOOR-K1");
        syntax.set_separator(Aspect::Location, Some(".")).unwrap();
        assert_eq!(d.format(&syntax), "+J-E-2416.DOOR-K1");
        syntax.set_sign(Aspect::Location, "++").unwrap_err();
        syntax.set_sign(Aspect::InstallationSite, "+++").unwrap();
        syntax.set_sign(Aspect::Location, "++").unwrap();
        assert_eq!(d.format(&syntax), "++J-E-2416.DOOR-K1");
    }
}
