// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `KnownCodes`: the node codes and aliases the parser treats as atomic tokens.

use std::collections::BTreeMap;

use crate::{Aspect, RdsError, RdsSyntax};

/// A level that the parser matched against a known node code or alias.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct NodeMatch {
    /// The node code the spelling names: `J-E-2416` for both `J-E-2416` and its alias `JE2416`.
    pub code: String,
    /// Whether the text was an alias (FR-PRJ-028); aliases are mapped, never printed.
    pub via_alias: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Entry {
    code: String,
    alias: bool,
}

/// The structure-node codes and aliases of a project, per aspect: the atomic tokens of parsing
/// (FR-PRJ-017, FR-PRJ-028).
///
/// When the parser reads a level of aspect `a`, it first takes the **longest** known spelling of `a` that
/// starts there and ends at a boundary (the end, `:`, `;`, a sign or `a`'s separator), and only then
/// splits at prefix characters (architecture §3, domain model §8.4). So with `J-E-2416` known,
/// `+J-E-2416-X1031` is the location `J-E-2416` and the product `X1031`, never `+J`, `-E`, `-2416`.
/// Matching is exact (code points); case and separator variants are a check (EW-RDS-010), see
/// [`spelling_key`].
///
/// Spellings are expected in NFC, as every `Ident` is. Codes are not unique per aspect in the model
/// (only among siblings), so registering the same code twice is fine; a spelling that would name two
/// different codes is refused.
///
/// ```
/// use ew_rds::{Aspect, KnownCodes};
///
/// let mut known = KnownCodes::new();
/// known.add_code(Aspect::Location, "J-E-2416").unwrap();
/// known.add_alias(Aspect::Location, "JE2416", "J-E-2416").unwrap();
/// assert_eq!(known.resolve(Aspect::Location, "JE2416").unwrap().code, "J-E-2416");
/// assert!(known.add_alias(Aspect::Location, "JE2416", "J-E-206").is_err());
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KnownCodes {
    by_aspect: BTreeMap<Aspect, BTreeMap<String, Entry>>,
}

impl KnownCodes {
    /// No known codes: the parser then splits at every prefix character.
    pub fn new() -> KnownCodes {
        KnownCodes::default()
    }

    fn add(&mut self, aspect: Aspect, spelling: &str, entry: Entry) -> Result<(), RdsError> {
        if spelling.is_empty() || entry.code.is_empty() {
            return Err(RdsError::EmptyCode);
        }
        let map = self.by_aspect.entry(aspect).or_default();
        match map.get(spelling) {
            None => {
                map.insert(spelling.to_owned(), entry);
                Ok(())
            }
            // The same code or alias again (a node's own code given as its alias is its code): no-op.
            Some(e) if *e == entry => Ok(()),
            Some(_) => Err(RdsError::CodeConflict),
        }
    }

    /// Registers node code `code` of `aspect`.
    ///
    /// # Errors
    ///
    /// [`RdsError::EmptyCode`], or [`RdsError::CodeConflict`] if `code` is already an alias of another
    /// code (or of itself, registered as alias first).
    pub fn add_code(&mut self, aspect: Aspect, code: &str) -> Result<(), RdsError> {
        self.add(
            aspect,
            code,
            Entry {
                code: code.to_owned(),
                alias: false,
            },
        )
    }

    /// Registers `alias` as another spelling of node code `code` of `aspect` (FR-PRJ-028).
    ///
    /// # Errors
    ///
    /// [`RdsError::EmptyCode`], or [`RdsError::CodeConflict`] if `alias` already names another code.
    pub fn add_alias(&mut self, aspect: Aspect, alias: &str, code: &str) -> Result<(), RdsError> {
        self.add(
            aspect,
            alias,
            Entry {
                code: code.to_owned(),
                alias: alias != code,
            },
        )
    }

    /// The node a complete spelling names, if known.
    pub fn resolve(&self, aspect: Aspect, spelling: &str) -> Option<NodeMatch> {
        self.by_aspect.get(&aspect)?.get(spelling).map(|e| NodeMatch {
            code: e.code.clone(),
            via_alias: e.alias,
        })
    }

    /// Whether no code is known.
    pub fn is_empty(&self) -> bool {
        self.by_aspect.values().all(BTreeMap::is_empty)
    }

    /// The longest known spelling of `aspect` at the start of `rest` that ends at a boundary; its length
    /// in bytes and the node it names.
    pub(crate) fn longest_match(
        &self,
        aspect: Aspect,
        rest: &str,
        syntax: &RdsSyntax,
    ) -> Option<(usize, NodeMatch)> {
        let map = self.by_aspect.get(&aspect)?;
        let mut ends: Vec<usize> = rest.char_indices().map(|(i, _)| i).skip(1).collect();
        ends.push(rest.len());
        ends.into_iter().rev().find_map(|end| {
            let e = map.get(&rest[..end])?;
            syntax.at_boundary(&rest[end..], aspect).then(|| {
                (
                    end,
                    NodeMatch {
                        code: e.code.clone(),
                        via_alias: e.alias,
                    },
                )
            })
        })
    }
}

/// Whether structure code `code` contains a prefix or separator character of `syntax`, or `:`, `;`,
/// `/`, `.` or white space, so that third-party tools would split the exported designation (EW-RDS-002,
/// FR-CHK-027). EW itself keeps such codes atomic ([`KnownCodes`]).
///
/// ```
/// use ew_rds::{RdsSyntax, code_has_separators};
///
/// let syntax = RdsSyntax::default();
/// assert!(code_has_separators("J-E-2416", &syntax));
/// assert!(!code_has_separators("JE2416", &syntax));
/// assert!(!code_has_separators("P01", &syntax));
/// ```
pub fn code_has_separators(code: &str, syntax: &RdsSyntax) -> bool {
    code.chars().any(|c| syntax.is_separator_char(c))
}

/// The spelling key of a structure code: its letters and digits, upper-cased. Two codes with the same key
/// differ only in separators or case, such as `JE2416` and `J-E-2416` (EW-RDS-010, FR-PRJ-018).
///
/// ```
/// use ew_rds::spelling_key;
///
/// assert_eq!(spelling_key("J-E-2416"), spelling_key("je2416"));
/// assert_ne!(spelling_key("J-E-2416"), spelling_key("J-E-206"));
/// ```
pub fn spelling_key(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_rules() {
        let mut k = KnownCodes::new();
        assert!(k.is_empty());
        assert_eq!(k.add_code(Aspect::Location, ""), Err(RdsError::EmptyCode));
        assert_eq!(k.add_alias(Aspect::Location, "", "A"), Err(RdsError::EmptyCode));
        assert_eq!(k.add_alias(Aspect::Location, "A", ""), Err(RdsError::EmptyCode));
        k.add_code(Aspect::Location, "J-E-2416").unwrap();
        k.add_code(Aspect::Location, "J-E-2416").unwrap();
        k.add_alias(Aspect::Location, "JE2416", "J-E-2416").unwrap();
        k.add_alias(Aspect::Location, "JE2416", "J-E-2416").unwrap();
        k.add_alias(Aspect::Location, "J-E-2416", "J-E-2416").unwrap();
        assert_eq!(
            k.add_alias(Aspect::Location, "JE2416", "J-E-206"),
            Err(RdsError::CodeConflict)
        );
        assert_eq!(
            k.add_code(Aspect::Location, "JE2416"),
            Err(RdsError::CodeConflict)
        );
        assert_eq!(
            k.add_alias(Aspect::Location, "J-E-2416", "J-E-206"),
            Err(RdsError::CodeConflict)
        );
        // Aspects are separate namespaces.
        k.add_code(Aspect::Function, "JE2416").unwrap();
        assert!(!k.is_empty());
        assert_eq!(
            k.resolve(Aspect::Location, "JE2416"),
            Some(NodeMatch {
                code: "J-E-2416".to_owned(),
                via_alias: true
            })
        );
        assert_eq!(
            k.resolve(Aspect::Location, "J-E-2416"),
            Some(NodeMatch {
                code: "J-E-2416".to_owned(),
                via_alias: false
            })
        );
        assert_eq!(k.resolve(Aspect::Product, "J-E-2416"), None);
    }

    #[test]
    fn longest_match_respects_boundaries() {
        let s = RdsSyntax::default();
        let mut k = KnownCodes::new();
        k.add_code(Aspect::Location, "J-E").unwrap();
        k.add_code(Aspect::Location, "J-E-2416").unwrap();
        let m = |rest: &str| {
            k.longest_match(Aspect::Location, rest, &s)
                .map(|(n, m)| (n, m.code))
        };
        assert_eq!(m("J-E-2416-X1"), Some((8, "J-E-2416".to_owned())));
        assert_eq!(m("J-E-2416"), Some((8, "J-E-2416".to_owned())));
        assert_eq!(m("J-E-2416:1"), Some((8, "J-E-2416".to_owned())));
        // "J-E-24167" does not end at a boundary after "J-E-2416"; "J-E" does (before "-").
        assert_eq!(m("J-E-24167"), Some((3, "J-E".to_owned())));
        assert_eq!(m("J-EX"), None);
        assert_eq!(m("X"), None);
    }

    #[test]
    fn helpers() {
        let s = RdsSyntax::default();
        for c in [
            "J-E-2416", "A+B", "A=B", "A.B", "A B", "A:B", "A;B", "A/B", "A&B", "A#B", "A%B",
        ] {
            assert!(code_has_separators(c, &s), "{c}");
        }
        for c in ["JE2416", "P01", "FIELD", "EFS", "Ü1"] {
            assert!(!code_has_separators(c, &s), "{c}");
        }
        assert_eq!(spelling_key("+J-E-2416"), "JE2416");
        assert_eq!(spelling_key("je 2416"), "JE2416");
    }
}
