// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Identifier sets: function category → letter code, as data (ADR-0026).

use std::borrow::Cow;

use crate::{LetterCode, RdsError};

// Generated from data/identifier-sets.toml by tests/identifier_sets.rs; long literal lines stay unwrapped.
#[rustfmt::skip]
mod data;

/// Key of the EN 81346-2:2009 single-letter set, the default for new projects (ADR-0026 decision 1).
pub const EN_81346_2_2009: &str = "EN81346-2:2009";
/// Key of the IEC 81346-2:2019 set of classes and two-letter subclasses.
pub const IEC_81346_2_2019: &str = "IEC81346-2:2019";
/// Key of the IEC 60750 / DIN 40719-2 legacy set, for migrated QET projects.
pub const IEC_60750_LEGACY: &str = "IEC60750-legacy";

/// One row of an identifier set: the letter code of a function category (optionally refined by its group)
/// and the letter codes the set also accepts for it (ADR-0026 decision 3).
///
/// `verified` says whether the row was checked against a licensed copy of the standard; until it is,
/// conformance findings from it are advisory. The description is the project's own wording, never
/// standard text (NFR-COMP-05).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Mapping {
    category: Cow<'static, str>,
    group: Option<Cow<'static, str>>,
    letter: LetterCode,
    alternatives: Cow<'static, [LetterCode]>,
    verified: bool,
    description: Cow<'static, str>,
}

fn check_category(category: &str, group: Option<&str>) -> Result<(), RdsError> {
    let mut chars = category.chars();
    let key_ok =
        chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric());
    let group_ok = group.is_none_or(|g| !g.is_empty() && g.trim() == g && !g.chars().any(char::is_control));
    if key_ok && group_ok {
        Ok(())
    } else {
        Err(RdsError::InvalidCategory)
    }
}

impl Mapping {
    /// A row for a company or project set.
    ///
    /// # Errors
    ///
    /// [`RdsError::InvalidCategory`] for a category key that is not UpperCamelCase ASCII or an empty or
    /// padded group; [`RdsError::DuplicateMapping`] if an alternative repeats the letter code or another
    /// alternative.
    pub fn new(
        category: &str,
        group: Option<&str>,
        letter: LetterCode,
        alternatives: Vec<LetterCode>,
        verified: bool,
        description: &str,
    ) -> Result<Mapping, RdsError> {
        let m = Mapping {
            category: Cow::Owned(category.to_owned()),
            group: group.map(|g| Cow::Owned(g.to_owned())),
            letter,
            alternatives: Cow::Owned(alternatives),
            verified,
            description: Cow::Owned(description.to_owned()),
        };
        m.check()?;
        Ok(m)
    }

    /// A row of the shipped tables (`data.rs`); checked by the tests instead of at run time.
    pub(crate) const fn from_static(
        category: &'static str,
        group: Option<&'static str>,
        letter: LetterCode,
        alternatives: &'static [LetterCode],
        verified: bool,
        description: &'static str,
    ) -> Mapping {
        Mapping {
            category: Cow::Borrowed(category),
            group: match group {
                Some(g) => Some(Cow::Borrowed(g)),
                None => None,
            },
            letter,
            alternatives: Cow::Borrowed(alternatives),
            verified,
            description: Cow::Borrowed(description),
        }
    }

    fn check(&self) -> Result<(), RdsError> {
        check_category(&self.category, self.group.as_deref())?;
        for (i, alt) in self.alternatives.iter().enumerate() {
            if *alt == self.letter || self.alternatives[..i].contains(alt) {
                return Err(RdsError::DuplicateMapping);
            }
        }
        Ok(())
    }

    /// The function category key (the first level of an FnDef category): `ProtectionOvercurrent`.
    pub fn category(&self) -> &str {
        &self.category
    }

    /// The group (second level) this row is limited to, if any.
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    /// The letter code new devices of the category get (FR-NUM-004), and the suggestion of EW-RDS-003.
    pub fn letter(&self) -> LetterCode {
        self.letter
    }

    /// Further letter codes accepted without an EW-RDS-003 finding.
    pub fn alternatives(&self) -> &[LetterCode] {
        &self.alternatives
    }

    /// Whether the row was checked against a licensed copy of the standard.
    pub fn verified(&self) -> bool {
        self.verified
    }

    /// The project's short description of the category.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Whether `letter` is the row's letter code or one of its alternatives.
    pub fn accepts(&self, letter: &str) -> bool {
        self.letter.as_str() == letter || self.alternatives.iter().any(|a| a.as_str() == letter)
    }
}

/// The verdict of [`IdentifierSet::check_letter`] on a device's letter code (EW-RDS-003, FR-CHK-026).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LetterCheck<'a> {
    /// The letter is the row's letter code.
    Preferred(&'a Mapping),
    /// The letter is one of the row's accepted alternatives.
    Alternative(&'a Mapping),
    /// The letter does not match; the row's [`letter`](Mapping::letter) is the suggestion, and
    /// [`verified`](Mapping::verified) tells whether the finding is more than advisory.
    Mismatch(&'a Mapping),
    /// The set has no row for the category, so it suggests nothing.
    UnknownCategory,
}

/// Why a letter code is unsuitable in a set (EW-RDS-007, FR-CHK-060).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum ReservedLetter {
    /// `I` or `O` in a letter code, at this position (0-based), in every set.
    IOrO {
        /// Position in the letter code.
        position: usize,
        /// The letter.
        letter: char,
    },
    /// A first (class) letter that the set reserves or does not use.
    ReservedClass(char),
}

/// An identifier set: rows mapping function categories to letter codes (ADR-0026).
///
/// Three sets ship as data ([`shipped`](IdentifierSet::shipped)): [`EN_81346_2_2009`] (the default for
/// new projects), [`IEC_81346_2_2019`] and [`IEC_60750_LEGACY`]. A company or project set is built with
/// [`new`](IdentifierSet::new). The project pins its set (ADR-0026 decision 5); the record form of that
/// setting is an `ew-schema` decision, so this type has no serde implementation.
///
/// Every shipped row is `verified = false` until checked against a licensed copy of the standards, so
/// EW-RDS-003 findings stay advisory (ADR-0026, Consequences).
///
/// ```
/// use ew_rds::{IdentifierSet, LetterCheck, IEC_81346_2_2019};
///
/// let set = IdentifierSet::default_set();
/// assert_eq!(set.mapping("ProtectionOvercurrent", None).unwrap().letter().as_str(), "F");
/// // T55, a thermostat drawn with T: a mismatch, suggestion B.
/// match set.check_letter("SensingTemperature", None, "T") {
///     LetterCheck::Mismatch(row) => assert_eq!(row.letter().as_str(), "B"),
///     other => panic!("{other:?}"),
/// }
/// // Under the 2019 set the suggestion is BT (FR-CHK-026).
/// let set_2019 = IdentifierSet::by_key(IEC_81346_2_2019).unwrap();
/// assert!(matches!(set_2019.check_letter("SensingTemperature", None, "T"),
///                  LetterCheck::Mismatch(row) if row.letter().as_str() == "BT"));
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct IdentifierSet {
    key: Cow<'static, str>,
    title: Cow<'static, str>,
    reserved: Cow<'static, [char]>,
    rows: Cow<'static, [Mapping]>,
}

impl IdentifierSet {
    /// A company or project set.
    ///
    /// # Errors
    ///
    /// [`RdsError::InvalidSetKey`] for an empty key or title or a key with white space;
    /// [`RdsError::InvalidLetterCode`] for a `reserved` entry other than `A`–`Z`; for the rows,
    /// [`RdsError::InvalidCategory`], [`RdsError::DuplicateMapping`] (two rows for one category and group)
    /// and [`RdsError::ReservedLetter`] (a letter code with `I` or `O`, or starting with a letter in
    /// `reserved`).
    pub fn new(
        key: &str,
        title: &str,
        reserved: Vec<char>,
        rows: Vec<Mapping>,
    ) -> Result<IdentifierSet, RdsError> {
        let set = IdentifierSet {
            key: Cow::Owned(key.to_owned()),
            title: Cow::Owned(title.to_owned()),
            reserved: Cow::Owned(reserved),
            rows: Cow::Owned(rows),
        };
        set.check()?;
        Ok(set)
    }

    /// A shipped set (`data.rs`).
    pub(crate) const fn from_static(
        key: &'static str,
        title: &'static str,
        reserved: &'static [char],
        rows: &'static [Mapping],
    ) -> IdentifierSet {
        IdentifierSet {
            key: Cow::Borrowed(key),
            title: Cow::Borrowed(title),
            reserved: Cow::Borrowed(reserved),
            rows: Cow::Borrowed(rows),
        }
    }

    pub(crate) fn check(&self) -> Result<(), RdsError> {
        if self.key.is_empty() || self.key.chars().any(char::is_whitespace) || self.title.is_empty() {
            return Err(RdsError::InvalidSetKey);
        }
        if !self.reserved.iter().all(char::is_ascii_uppercase) {
            return Err(RdsError::InvalidLetterCode);
        }
        for (i, row) in self.rows.iter().enumerate() {
            row.check()?;
            if self.rows[..i]
                .iter()
                .any(|r| r.category == row.category && r.group == row.group)
            {
                return Err(RdsError::DuplicateMapping);
            }
            for code in std::iter::once(&row.letter).chain(row.alternatives.iter()) {
                if !self.reserved_letter_findings(*code).is_empty() {
                    return Err(RdsError::ReservedLetter);
                }
            }
        }
        Ok(())
    }

    /// The shipped sets, in the order of `data/identifier-sets.toml`.
    pub fn shipped() -> &'static [IdentifierSet] {
        &data::SHIPPED
    }

    /// The shipped set with this key.
    pub fn by_key(key: &str) -> Option<&'static IdentifierSet> {
        data::SHIPPED.iter().find(|s| s.key == key)
    }

    /// The default set of new projects: EN 81346-2:2009 single letters (ADR-0026 decision 1).
    pub fn default_set() -> &'static IdentifierSet {
        // The data file lists the default first; a test holds it to that.
        &data::SHIPPED[0]
    }

    /// The key: `EN81346-2:2009`.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The display title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The letters this set does not use as class letters (besides `I` and `O`, which no set uses).
    pub fn reserved(&self) -> &[char] {
        &self.reserved
    }

    /// The rows, in data order.
    pub fn rows(&self) -> &[Mapping] {
        &self.rows
    }

    /// The row for `category` and `group`: the row of that group if there is one, else the row of the
    /// category without a group.
    pub fn mapping(&self, category: &str, group: Option<&str>) -> Option<&Mapping> {
        let exact = group.and_then(|g| {
            self.rows
                .iter()
                .find(|r| r.category == category && r.group.as_deref() == Some(g))
        });
        exact.or_else(|| {
            self.rows
                .iter()
                .find(|r| r.category == category && r.group.is_none())
        })
    }

    /// The verdict on letter code `letter` (a `TagSpec.letter`) for a device of `category` and `group`
    /// (EW-RDS-003). Comparison is exact: `f` does not match `F`.
    pub fn check_letter(&self, category: &str, group: Option<&str>, letter: &str) -> LetterCheck<'_> {
        match self.mapping(category, group) {
            None => LetterCheck::UnknownCategory,
            Some(row) if row.letter.as_str() == letter => LetterCheck::Preferred(row),
            Some(row) if row.accepts(letter) => LetterCheck::Alternative(row),
            Some(row) => LetterCheck::Mismatch(row),
        }
    }

    /// Whether some row of the set has `letter` as its letter code or an alternative: the QET importer
    /// proposes the set that alone explains a project's letters (ADR-0026 decision 7).
    pub fn explains(&self, letter: &str) -> bool {
        self.rows.iter().any(|r| r.accepts(letter))
    }

    /// Why `letter` is unsuitable in this set, if it is: `I` or `O` anywhere, or a reserved class letter
    /// (EW-RDS-007).
    ///
    /// ```
    /// use ew_rds::{IdentifierSet, LetterCode, ReservedLetter, IEC_81346_2_2019};
    ///
    /// let set = IdentifierSet::by_key(IEC_81346_2_2019).unwrap();
    /// assert!(set.reserved_letter_findings(LetterCode::from_static("FC")).is_empty());
    /// assert_eq!(
    ///     set.reserved_letter_findings(LetterCode::from_static("VO")),
    ///     [ReservedLetter::IOrO { position: 1, letter: 'O' }, ReservedLetter::ReservedClass('V')]
    /// );
    /// ```
    pub fn reserved_letter_findings(&self, letter: LetterCode) -> Vec<ReservedLetter> {
        let mut findings: Vec<ReservedLetter> = letter
            .letters()
            .enumerate()
            .filter(|(_, c)| matches!(c, 'I' | 'O'))
            .map(|(position, letter)| ReservedLetter::IOrO { position, letter })
            .collect();
        let class = letter.class_letter();
        if self.reserved.contains(&class) {
            findings.push(ReservedLetter::ReservedClass(class));
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(s: &str) -> LetterCode {
        s.parse().unwrap()
    }

    #[test]
    fn shipped_sets_are_valid() {
        let keys: Vec<&str> = IdentifierSet::shipped().iter().map(IdentifierSet::key).collect();
        assert_eq!(keys, [EN_81346_2_2009, IEC_81346_2_2019, IEC_60750_LEGACY]);
        assert_eq!(IdentifierSet::default_set().key(), EN_81346_2_2009);
        for set in IdentifierSet::shipped() {
            assert_eq!(set.check(), Ok(()), "{}", set.key());
            assert!(!set.rows().is_empty());
            // ADR-0026: nothing is verified against a licensed copy yet.
            assert!(set.rows().iter().all(|r| !r.verified()), "{}", set.key());
            assert!(set.rows().iter().all(|r| !r.description().is_empty()));
        }
        assert_eq!(IdentifierSet::by_key("NFPA79"), None);
    }

    /// Golden A under the default set (ADR-0026 amendment 1, golden EF-18): only T55 is reported.
    #[test]
    fn golden_letters_under_the_2009_set() {
        let set = IdentifierSet::default_set();
        let preferred = [
            ("ProtectionOvercurrent", "F"), // -101F1, F55, -102F1, -102F2
            ("PowerSupplyAcDc", "G"),       // -102G1
            ("Terminal", "X"),              // X1031, 1X1
            ("ConnectorSignal", "X"),       // -3X1
            ("CableSignal", "W"),           // -3W1
            ("CablePower", "W"),            // -106W1
            ("Lighting", "E"),              // E56
            ("Heating", "E"),               // E55
            ("ValveTerminalModule", "U"),   // -201U3 … -201U8
            ("ValveDirectional", "Q"),      // -201U3-Q1
        ];
        for (category, letter) in preferred {
            assert!(
                matches!(
                    set.check_letter(category, None, letter),
                    LetterCheck::Preferred(_)
                ),
                "{category} {letter}"
            );
        }
        let alternatives = [
            ("SwitchingMain", "S"),     // S51
            ("SensingPosition", "S"),   // S56
            ("ValveTerminalNode", "U"), // -201U1
            ("ValveDirectional", "QM"),
            ("Fan", "E"),
        ];
        for (category, letter) in alternatives {
            assert!(
                matches!(
                    set.check_letter(category, None, letter),
                    LetterCheck::Alternative(_)
                ),
                "{category} {letter}"
            );
        }
        match set.check_letter("SensingTemperature", None, "T") {
            LetterCheck::Mismatch(row) => {
                assert_eq!(row.letter(), l("B"));
                assert!(!row.verified());
            }
            other => panic!("T55: {other:?}"),
        }
        assert_eq!(
            set.check_letter("NoSuchCategory", None, "F"),
            LetterCheck::UnknownCategory
        );
        assert!(matches!(
            set.check_letter("ProtectionOvercurrent", None, "f"),
            LetterCheck::Mismatch(_)
        ));
    }

    /// The renumber examples of ADR-0026 decision 6 and FR-CHK-026.
    #[test]
    fn letters_under_the_2019_set() {
        let set = IdentifierSet::by_key(IEC_81346_2_2019).unwrap();
        let letter = |c: &str| set.mapping(c, None).unwrap().letter().as_str().to_owned();
        assert_eq!(letter("ProtectionOvercurrent"), "FC"); // -101F1 → -101FC1
        assert_eq!(letter("PowerSupplyAcDc"), "TC"); // -102G1 → -102TC1
        assert_eq!(letter("SensingTemperature"), "BT"); // T55, suggestion BT
        assert_eq!(letter("SwitchingMain"), "QB");
        assert_eq!(letter("SensingPosition"), "BG");
        assert_eq!(letter("Terminal"), "XD");
        assert_eq!(letter("ValveTerminalNode"), "KE");
        assert!(matches!(
            set.check_letter("PowerSupplyAcDc", None, "TB"),
            LetterCheck::Alternative(_)
        ));
        // The 2019 set does not use A as a class letter.
        assert_eq!(
            set.check_letter("Assembly", None, "A"),
            LetterCheck::UnknownCategory
        );
    }

    #[test]
    fn explains() {
        let legacy = IdentifierSet::by_key(IEC_60750_LEGACY).unwrap();
        let set_2009 = IdentifierSet::default_set();
        // H for an indicator light: only the legacy set explains it.
        assert!(legacy.explains("H"));
        assert!(!set_2009.explains("H"));
        assert!(set_2009.explains("F"));
        assert!(!set_2009.explains("Z"));
    }

    #[test]
    fn reserved_letters() {
        let set_2019 = IdentifierSet::by_key(IEC_81346_2_2019).unwrap();
        let set_2009 = IdentifierSet::default_set();
        assert_eq!(
            set_2019.reserved_letter_findings(l("A")),
            [ReservedLetter::ReservedClass('A')]
        );
        assert!(set_2009.reserved_letter_findings(l("A")).is_empty());
        assert_eq!(
            set_2009.reserved_letter_findings(l("QIO")),
            [
                ReservedLetter::IOrO {
                    position: 1,
                    letter: 'I'
                },
                ReservedLetter::IOrO {
                    position: 2,
                    letter: 'O'
                }
            ]
        );
        assert_eq!(
            set_2009.reserved_letter_findings(l("D")),
            [ReservedLetter::ReservedClass('D')]
        );
    }

    #[test]
    fn group_rows_refine_category_rows() {
        let rows = vec![
            Mapping::new("ManualCommand", None, l("S"), vec![], true, "manual command").unwrap(),
            Mapping::new(
                "ManualCommand",
                Some("EmergencyStop"),
                l("SF"),
                vec![l("S")],
                true,
                "e-stop",
            )
            .unwrap(),
        ];
        let set = IdentifierSet::new("Company", "Company letters", vec!['D'], rows).unwrap();
        assert_eq!(
            set.mapping("ManualCommand", Some("EmergencyStop"))
                .unwrap()
                .letter(),
            l("SF")
        );
        assert_eq!(
            set.mapping("ManualCommand", Some("PushButton")).unwrap().letter(),
            l("S")
        );
        assert_eq!(set.mapping("ManualCommand", None).unwrap().letter(), l("S"));
        assert!(matches!(
            set.check_letter("ManualCommand", Some("EmergencyStop"), "S"),
            LetterCheck::Alternative(_)
        ));
        assert_eq!(set.key(), "Company");
        assert_eq!(set.title(), "Company letters");
        assert_eq!(set.reserved(), ['D']);
    }

    #[test]
    fn company_set_validation() {
        let row = |c: &str, letter: &str| Mapping::new(c, None, l(letter), vec![], false, "x");
        assert_eq!(row("protection", "F"), Err(RdsError::InvalidCategory));
        assert_eq!(row("Protection Overcurrent", "F"), Err(RdsError::InvalidCategory));
        assert_eq!(row("", "F"), Err(RdsError::InvalidCategory));
        assert_eq!(
            Mapping::new("Fan", Some(""), l("G"), vec![], false, "x"),
            Err(RdsError::InvalidCategory)
        );
        assert_eq!(
            Mapping::new("Fan", Some(" MCB"), l("G"), vec![], false, "x"),
            Err(RdsError::InvalidCategory)
        );
        assert_eq!(
            Mapping::new("Fan", None, l("G"), vec![l("G")], false, "x"),
            Err(RdsError::DuplicateMapping)
        );
        assert_eq!(
            Mapping::new("Fan", None, l("G"), vec![l("E"), l("E")], false, "x"),
            Err(RdsError::DuplicateMapping)
        );
        let fan = row("Fan", "G").unwrap();
        assert_eq!(
            IdentifierSet::new("", "t", vec![], vec![fan.clone()]),
            Err(RdsError::InvalidSetKey)
        );
        assert_eq!(
            IdentifierSet::new("A B", "t", vec![], vec![fan.clone()]),
            Err(RdsError::InvalidSetKey)
        );
        assert_eq!(
            IdentifierSet::new("K", "", vec![], vec![]),
            Err(RdsError::InvalidSetKey)
        );
        assert_eq!(
            IdentifierSet::new("K", "t", vec!['g'], vec![]),
            Err(RdsError::InvalidLetterCode)
        );
        assert_eq!(
            IdentifierSet::new("K", "t", vec![], vec![fan.clone(), fan.clone()]),
            Err(RdsError::DuplicateMapping)
        );
        assert_eq!(
            IdentifierSet::new("K", "t", vec!['G'], vec![fan.clone()]),
            Err(RdsError::ReservedLetter)
        );
        assert_eq!(
            IdentifierSet::new("K", "t", vec![], vec![row("Fan", "GO").unwrap()]),
            Err(RdsError::ReservedLetter)
        );
        assert!(IdentifierSet::new("K", "t", vec![], vec![fan]).is_ok());
    }
}
