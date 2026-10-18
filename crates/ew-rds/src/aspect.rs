// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Aspect`: the structuring aspects of IEC 81346-1 and EPLAN's identifier blocks.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An aspect of a reference designation (IEC 81346-1; domain model §5, "Aspects and their signs").
///
/// The declaration order is the **canonical order** of a full designation, `[==FA][=P01][++SITE]
/// [+J-E-2416][&EFS][#…][%…]-101F1` (domain model §8.1), and `Ord` follows it. The product aspect is the
/// device tree; the others are structure-node trees.
///
/// | Aspect | Default sign |
/// |---|---|
/// | [`FunctionalAssignment`](Aspect::FunctionalAssignment) | `==` |
/// | [`Function`](Aspect::Function) | `=` |
/// | [`InstallationSite`](Aspect::InstallationSite) | `++` |
/// | [`Location`](Aspect::Location) | `+` |
/// | [`DocumentKind`](Aspect::DocumentKind) | `&` |
/// | [`UserDefined`](Aspect::UserDefined) | `#` |
/// | [`Type`](Aspect::Type) | `%` (sign unverified, standards §1.2) |
/// | [`Product`](Aspect::Product) | `-` |
///
/// Signs are project settings (FR-PRJ-016), held by [`RdsSyntax`](crate::RdsSyntax).
///
/// **JSON:** the snake_case name (doc 05 §3.2 enum rule): `"functional_assignment"`, `"function"`,
/// `"installation_site"`, `"location"`, `"document_kind"`, `"user_defined"`, `"type"`, `"product"`.
///
/// ```
/// use ew_rds::Aspect;
///
/// assert!(Aspect::Function < Aspect::Location && Aspect::Location < Aspect::Product);
/// assert_eq!(Aspect::Location.default_sign(), "+");
/// assert_eq!(serde_json::to_string(&Aspect::DocumentKind).unwrap(), "\"document_kind\"");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Aspect {
    /// Functional assignment, EPLAN's block above the function aspect (`==`).
    FunctionalAssignment,
    /// Function aspect (`=`): `=P01`.
    Function,
    /// Installation site, EPLAN's block above the location aspect (`++`).
    InstallationSite,
    /// Location aspect (`+`): `+J-E-2416`.
    Location,
    /// Document kind (`&`, IEC 61355-1): `&EFS`; pages only.
    DocumentKind,
    /// A user-defined block (`#`).
    UserDefined,
    /// Type aspect of IEC 81346-1:2022 (`%`).
    Type,
    /// Product aspect (`-`): `-101F1`, `-201U3-Q1`.
    Product,
}

impl Aspect {
    /// All aspects in canonical order.
    pub const ALL: [Aspect; 8] = [
        Aspect::FunctionalAssignment,
        Aspect::Function,
        Aspect::InstallationSite,
        Aspect::Location,
        Aspect::DocumentKind,
        Aspect::UserDefined,
        Aspect::Type,
        Aspect::Product,
    ];

    /// The default prefix sign (EN 81346 and EPLAN's identifier blocks).
    pub const fn default_sign(self) -> &'static str {
        match self {
            Aspect::FunctionalAssignment => "==",
            Aspect::Function => "=",
            Aspect::InstallationSite => "++",
            Aspect::Location => "+",
            Aspect::DocumentKind => "&",
            Aspect::UserDefined => "#",
            Aspect::Type => "%",
            Aspect::Product => "-",
        }
    }

    /// The snake_case name used in JSON.
    pub const fn as_str(self) -> &'static str {
        match self {
            Aspect::FunctionalAssignment => "functional_assignment",
            Aspect::Function => "function",
            Aspect::InstallationSite => "installation_site",
            Aspect::Location => "location",
            Aspect::DocumentKind => "document_kind",
            Aspect::UserDefined => "user_defined",
            Aspect::Type => "type",
            Aspect::Product => "product",
        }
    }

    /// Index into [`Aspect::ALL`].
    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for Aspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The error of [`Aspect::from_str`]: not one of the eight snake_case names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UnknownAspect;

impl fmt::Display for UnknownAspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown aspect name")
    }
}

impl std::error::Error for UnknownAspect {}

impl FromStr for Aspect {
    type Err = UnknownAspect;

    /// The aspect with this snake_case name; nothing else is accepted.
    fn from_str(s: &str) -> Result<Aspect, UnknownAspect> {
        Aspect::ALL
            .into_iter()
            .find(|a| a.as_str() == s)
            .ok_or(UnknownAspect)
    }
}

impl Serialize for Aspect {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Aspect {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(AspectVisitor)
    }
}

struct AspectVisitor;

impl Visitor<'_> for AspectVisitor {
    type Value = Aspect;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an aspect name such as \"location\"")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Aspect, E> {
        v.parse().map_err(E::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_order_and_names() {
        let mut sorted = Aspect::ALL;
        sorted.sort();
        assert_eq!(sorted, Aspect::ALL);
        for (i, a) in Aspect::ALL.into_iter().enumerate() {
            assert_eq!(a.index(), i);
            assert_eq!(a.as_str().parse::<Aspect>(), Ok(a));
            assert_eq!(a.to_string(), a.as_str());
        }
    }

    #[test]
    fn json_encoding() {
        for a in Aspect::ALL {
            let s = serde_json::to_string(&a).unwrap();
            assert_eq!(s, format!("\"{}\"", a.as_str()));
            assert_eq!(serde_json::from_str::<Aspect>(&s).unwrap(), a);
        }
        for bad in [
            "\"Location\"",
            "\"LOCATION\"",
            "\"+\"",
            "\"\"",
            "3",
            "null",
            "{\"location\":1}",
        ] {
            assert!(serde_json::from_str::<Aspect>(bad).is_err(), "{bad}");
        }
    }
}
