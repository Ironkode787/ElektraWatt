// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `RdsSyntax`: the prefix signs and sub-level separators of a project.

use crate::{Aspect, RdsError};

/// The prefix sign and optional sub-level separator of every aspect: project settings (FR-PRJ-015,
/// FR-PRJ-016) that the parser and formatter read.
///
/// The [`Default`] is the EN 81346 and EPLAN set of [`Aspect::default_sign`] with no separators, so a
/// deeper level repeats the sign: `+J-E-2416+DOOR`. With the location separator set to `.` the same node
/// formats as `+J-E-2416.DOOR`, and the parser accepts both forms.
///
/// The connection-point separator `:` and the signal separator `;` are fixed (domain model §8.4), and `/`
/// separates pages; none of them can be a sign or separator.
///
/// # Rules
///
/// - Every sign and separator is non-empty ASCII punctuation without `:`, `;` and `/`.
/// - No two signs are equal, and a separator equals no sign and no other separator.
/// - A sign may be a prefix of another (`=` and `==`): the parser takes the longest match.
///
/// The record form of these settings is an `ew-schema` decision; this type has no serde implementation.
///
/// ```
/// use ew_rds::{Aspect, RdsSyntax};
///
/// let mut syntax = RdsSyntax::default();
/// assert_eq!(syntax.sign(Aspect::InstallationSite), "++");
/// syntax.set_separator(Aspect::Location, Some(".")).unwrap();
/// assert!(syntax.set_sign(Aspect::Type, "=").is_err()); // already the function sign
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RdsSyntax {
    signs: [String; 8],
    separators: [Option<String>; 8],
}

impl Default for RdsSyntax {
    fn default() -> RdsSyntax {
        RdsSyntax {
            signs: Aspect::ALL.map(|a| a.default_sign().to_owned()),
            separators: Default::default(),
        }
    }
}

fn check_token(t: &str) -> Result<(), RdsError> {
    if t.is_empty() {
        Err(RdsError::EmptyToken)
    } else if t
        .chars()
        .all(|c| c.is_ascii_punctuation() && !matches!(c, ':' | ';' | '/'))
    {
        Ok(())
    } else {
        Err(RdsError::InvalidTokenCharacter)
    }
}

impl RdsSyntax {
    /// The prefix sign of `aspect`.
    pub fn sign(&self, aspect: Aspect) -> &str {
        &self.signs[aspect.index()]
    }

    /// The sub-level separator of `aspect`, if one is set; without one, a deeper level repeats the sign.
    pub fn separator(&self, aspect: Aspect) -> Option<&str> {
        self.separators[aspect.index()].as_deref()
    }

    fn check(&self) -> Result<(), RdsError> {
        let tokens: Vec<&str> = self
            .signs
            .iter()
            .map(String::as_str)
            .chain(self.separators.iter().flatten().map(String::as_str))
            .collect();
        for (i, t) in tokens.iter().enumerate() {
            check_token(t)?;
            if tokens[..i].contains(t) {
                return Err(RdsError::DuplicateToken);
            }
        }
        Ok(())
    }

    /// Sets the prefix sign of `aspect`.
    ///
    /// # Errors
    ///
    /// [`RdsError::EmptyToken`], [`RdsError::InvalidTokenCharacter`] or [`RdsError::DuplicateToken`]; the
    /// syntax is then unchanged.
    pub fn set_sign(&mut self, aspect: Aspect, sign: &str) -> Result<(), RdsError> {
        let mut next = self.clone();
        next.signs[aspect.index()] = sign.to_owned();
        next.check()?;
        *self = next;
        Ok(())
    }

    /// Sets or clears the sub-level separator of `aspect`.
    ///
    /// # Errors
    ///
    /// As [`set_sign`](RdsSyntax::set_sign).
    pub fn set_separator(&mut self, aspect: Aspect, separator: Option<&str>) -> Result<(), RdsError> {
        let mut next = self.clone();
        next.separators[aspect.index()] = separator.map(str::to_owned);
        next.check()?;
        *self = next;
        Ok(())
    }

    /// The longest token at the start of `rest`: any aspect's sign, or the separator of `current`.
    /// Returns the aspect it introduces and its length in bytes.
    pub(crate) fn token_at(&self, rest: &str, current: Option<Aspect>) -> Option<(Aspect, usize)> {
        let signs = Aspect::ALL.into_iter().map(|a| (a, self.sign(a)));
        let separator = current.and_then(|a| self.separator(a).map(|s| (a, s)));
        signs
            .chain(separator)
            .filter(|(_, t)| rest.starts_with(t))
            .max_by_key(|(_, t)| t.len())
            .map(|(a, t)| (a, t.len()))
    }

    /// Whether a level of `aspect` may end where `rest` begins: at the end, before `:` or `;`, or before
    /// a sign or `aspect`'s separator.
    pub(crate) fn at_boundary(&self, rest: &str, aspect: Aspect) -> bool {
        rest.is_empty() || rest.starts_with([':', ';']) || self.token_at(rest, Some(aspect)).is_some()
    }

    /// Whether `c` occurs in any sign or separator, or is `:`, `;`, `/` or white space: the characters
    /// that make a structure code ambiguous in exported designations (EW-RDS-002).
    pub(crate) fn is_separator_char(&self, c: char) -> bool {
        matches!(c, ':' | ';' | '/' | '.')
            || c.is_whitespace()
            || self.signs.iter().any(|s| s.contains(c))
            || self.separators.iter().flatten().any(|s| s.contains(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let s = RdsSyntax::default();
        for a in Aspect::ALL {
            assert_eq!(s.sign(a), a.default_sign());
            assert_eq!(s.separator(a), None);
        }
    }

    #[test]
    fn longest_token_wins() {
        let s = RdsSyntax::default();
        assert_eq!(s.token_at("==FA", None), Some((Aspect::FunctionalAssignment, 2)));
        assert_eq!(s.token_at("=P01", None), Some((Aspect::Function, 1)));
        assert_eq!(s.token_at("++S", None), Some((Aspect::InstallationSite, 2)));
        assert_eq!(s.token_at("+L", None), Some((Aspect::Location, 1)));
        assert_eq!(s.token_at("-K1", None), Some((Aspect::Product, 1)));
        assert_eq!(s.token_at("K1", None), None);
        assert_eq!(s.token_at(".X", Some(Aspect::Location)), None);
        let mut d = s.clone();
        d.set_separator(Aspect::Location, Some(".")).unwrap();
        assert_eq!(
            d.token_at(".X", Some(Aspect::Location)),
            Some((Aspect::Location, 1))
        );
        assert_eq!(d.token_at(".X", Some(Aspect::Product)), None);
    }

    #[test]
    fn validation() {
        let mut s = RdsSyntax::default();
        assert_eq!(s.set_sign(Aspect::Type, ""), Err(RdsError::EmptyToken));
        assert_eq!(
            s.set_sign(Aspect::Type, "T"),
            Err(RdsError::InvalidTokenCharacter)
        );
        assert_eq!(
            s.set_sign(Aspect::Type, ":"),
            Err(RdsError::InvalidTokenCharacter)
        );
        assert_eq!(
            s.set_sign(Aspect::Type, "%/"),
            Err(RdsError::InvalidTokenCharacter)
        );
        assert_eq!(
            s.set_sign(Aspect::Type, " "),
            Err(RdsError::InvalidTokenCharacter)
        );
        assert_eq!(s.set_sign(Aspect::Type, "+"), Err(RdsError::DuplicateToken));
        assert_eq!(
            s.set_separator(Aspect::Location, Some("-")),
            Err(RdsError::DuplicateToken)
        );
        assert_eq!(s, RdsSyntax::default(), "failed edits leave the syntax unchanged");
        s.set_separator(Aspect::Location, Some(".")).unwrap();
        assert_eq!(
            s.set_separator(Aspect::Function, Some(".")),
            Err(RdsError::DuplicateToken)
        );
        s.set_sign(Aspect::Type, "%%").unwrap();
        assert_eq!(s.sign(Aspect::Type), "%%");
        s.set_separator(Aspect::Location, None).unwrap();
        assert_eq!(s.separator(Aspect::Location), None);
    }
}
