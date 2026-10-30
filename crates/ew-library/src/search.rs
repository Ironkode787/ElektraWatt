// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Part and item search: the queries, the hits, `pn_norm` and the order of D6 (doc 07 §4.3).
//!
//! **Order** (WP-S1.19 D6, deterministic and without a clock):
//! 1. with a compatibility filter, compatible parts before the others (doc 07 §4.3: "offers MCB 1P+N parts first");
//!    when the filter names the function's FnDef ([`FnCompat::fndef`], WP-S2.07 D17), the parts whose function
//!    template declares that FnDef come before the other compatible ones;
//! 2. the rank tier: exact `pn_norm` (1), `pn_norm` prefix (2), trigram substring (3), full text (4); a query
//!    without text lists every part in tier 0;
//! 3. inside a tier: parts the project holds, then the caller's boost list (favourites, recently used);
//! 4. the full-text score, `bm25` quantised to 10⁻⁶ ([`quantise`]) so that platforms that differ in the last bits of
//!    a float order alike;
//! 5. the natural order of the part number (`ew_natural`'s order), the manufacturer key, the library ID, the item ID.

use ew_format::ew_schema::{CategoryKey, ContactKind, ItemStatus, Kind, PartKind};
use ew_ids::Uuid;

use crate::Tier;

/// `pn_norm` (doc 07 §4.3): upper case, without spaces, `-`, `.`, `/` and `_`. "SK 3110.000" gives "SK3110000";
/// "5SY4 204-7" and "5SY4204-7" both give "5SY42047".
pub fn pn_norm(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '.' | '/' | '_'))
        .flat_map(char::to_uppercase)
        .collect()
}

/// The function a part is chosen for (doc 07 §4.3, FR-PRT-015 at MVP): a part is compatible when its function
/// template has a function whose FnDef has the same first two category levels and the same contact kind.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FnCompat {
    /// The first two levels of the function's category (`["ProtectionOvercurrent", "MCB"]`).
    pub category: [String; 2],
    /// The function's contact kind.
    pub contact_kind: ContactKind,
    /// `(library, item)` of the function's FnDef (WP-S2.07 D17, FT-629): a part whose function template declares
    /// that FnDef, at any version, ranks before the other compatible parts and reports [`PartHit::same_fndef`]; `None`
    /// keeps WP-S1.19 D6's order and meaning unchanged.
    pub fndef: Option<(Uuid, Uuid)>,
}

/// A part search (WP-S1.19 §5.2, D6).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PartQuery {
    /// The typed text: a part number in any spelling, or words of the type, manufacturer and description.
    pub text: String,
    /// Only these kinds; all when empty.
    pub kinds: Vec<PartKind>,
    /// A category prefix, one level per element; all when empty.
    pub category: Vec<String>,
    /// Rank compatible parts first and report `compatible` on every hit.
    pub compatible_with: Option<FnCompat>,
    /// Only these tiers; all when empty.
    pub tiers: Vec<Tier>,
    /// Also deprecated and withdrawn parts (FR-PRT-034).
    pub include_inactive: bool,
    /// `(library, item)` of the project's copies: first inside a tier.
    pub held: Vec<(Uuid, Uuid)>,
    /// `(library, item)` of favourites and recently used parts: next inside a tier.
    pub boost: Vec<(Uuid, Uuid)>,
    /// At most this many hits; 0 for all.
    pub limit: u32,
}

/// One part found (WP-S1.19 §5.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartHit {
    /// The library.
    pub library: Uuid,
    /// The item.
    pub item: Uuid,
    /// The item's version.
    pub version: String,
    /// The library's tier.
    pub tier: Tier,
    /// The part number, verbatim.
    pub part_number: String,
    /// The manufacturer's name (the manufacturer record's, else the part reference's `key`).
    pub manufacturer: String,
    /// The description in every language, joined by " / ".
    pub description: String,
    /// The part kind.
    pub kind: PartKind,
    /// The item status; a deprecated part names its successor (FR-PRT-034).
    pub status: ItemStatus,
    /// Whether the part fits [`PartQuery::compatible_with`]; `None` without that filter.
    pub compatible: Option<bool>,
    /// Whether the part's function template declares [`FnCompat::fndef`] (the same library and item at any version);
    /// `None` without a filter or without `fndef` (WP-S2.07 D17).
    pub same_fndef: Option<bool>,
    /// 0 (no text), 1 exact part number, 2 prefix, 3 trigram, 4 full text.
    pub rank_tier: u8,
}

/// A search over symbols, FnDefs, macros, manufacturers and forms (WP-S1.19 §5.2).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ItemQuery {
    /// The typed text: a key, or words of the name, keywords and aliases.
    pub text: String,
    /// Only these kinds (of `symbol`, `fndef`, `form`, `macro`, `manufacturer`); all when empty.
    pub kinds: Vec<Kind>,
    /// Only these tiers; all when empty.
    pub tiers: Vec<Tier>,
    /// Also deprecated and withdrawn items.
    pub include_inactive: bool,
    /// At most this many hits; 0 for all.
    pub limit: u32,
}

/// One item found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ItemHit {
    /// The library.
    pub library: Uuid,
    /// The item.
    pub item: Uuid,
    /// The item's version.
    pub version: String,
    /// The library's tier.
    pub tier: Tier,
    /// The kind.
    pub kind: Kind,
    /// The human key (`meta.key`, a manufacturer's `key`), empty when none.
    pub key: String,
    /// The name in every language, joined by " / "; the key when the kind has no name.
    pub name: String,
    /// The item status.
    pub status: ItemStatus,
    /// 0 (no text), 1 exact key, 2 key prefix, 4 full text.
    pub rank_tier: u8,
}

/// The library kind named `name` (`part`, `symbol`, `fndef`, `form`, `macro`, `manufacturer`), for front ends that
/// take kinds as text (`ew lib search --kind`); `None` for any other name.
pub fn item_kind(name: &str) -> Option<Kind> {
    Kind::from_name(name).filter(|k| k.is_library())
}

/// Whether a status is shown by default (deprecated and withdrawn are hidden, FR-PRT-034).
pub(crate) fn active(status: &ItemStatus) -> bool {
    !matches!(status, ItemStatus::Deprecated(_) | ItemStatus::Withdrawn)
}

/// The `bm25` score as an integer: −score × 10⁶, rounded (higher is better). Scores that differ below 10⁻⁶ tie and
/// fall through to the natural order, so the order does not depend on a platform's last float bits.
pub fn quantise(bm25: f64) -> i64 {
    let q = (-bm25 * 1_000_000.0).round();
    if q.is_nan() {
        0
    } else {
        // In range for every score SQLite gives; `as` saturates beyond it.
        q as i64
    }
}

/// The natural order of `ew_natural` (`ew_mltext::natural_cmp`) on part numbers: digit runs by value, then accents
/// and leading zeros, then case. The key comes from the identifier type of `ew-schema` (a [`CategoryKey`] element is
/// one), so the order is that crate's, not a copy; a text that is not an identifier sorts by its bytes after every
/// one that is.
pub(crate) fn natural_key(s: &str) -> Result<impl Ord + use<>, String> {
    let key: Result<CategoryKey, _> = serde_json::from_value(serde_json::json!([s, "x", "x"]));
    match key {
        Ok(k) => Ok(k.0.natural_key()),
        Err(_) => Err(s.to_owned()),
    }
}

/// The sort key of a hit after its rank: held, boost, score, natural part number, manufacturer key, library, item.
pub(crate) struct OrderKey {
    /// 0 the same FnDef (D17), 1 compatible or no filter, 2 incompatible.
    pub fit: u8,
    pub rank_tier: u8,
    pub not_held: bool,
    pub not_boosted: bool,
    pub score: i64,
    pub part_number: String,
    pub manufacturer_key: String,
    pub library: Uuid,
    pub item: Uuid,
}

/// Sorts `items` by their [`OrderKey`]s (D6).
pub(crate) fn sort_by_order<T>(items: Vec<(OrderKey, T)>) -> Vec<T> {
    let mut keyed: Vec<_> = items
        .into_iter()
        .map(|(k, t)| (natural_key(&k.part_number), k, t))
        .collect();
    keyed.sort_by(|(na, a, _), (nb, b, _)| {
        a.fit
            .cmp(&b.fit)
            .then(a.rank_tier.cmp(&b.rank_tier))
            .then(a.not_held.cmp(&b.not_held))
            .then(a.not_boosted.cmp(&b.not_boosted))
            .then(b.score.cmp(&a.score))
            .then_with(|| na.cmp(nb))
            .then_with(|| a.manufacturer_key.cmp(&b.manufacturer_key))
            .then(a.library.cmp(&b.library))
            .then(a.item.cmp(&b.item))
    });
    keyed.into_iter().map(|(_, _, t)| t).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_kinds_by_name() {
        assert_eq!(item_kind("symbol"), Some(Kind::Symbol));
        assert_eq!(item_kind("part"), Some(Kind::Part));
        assert_eq!(item_kind("manufacturer"), Some(Kind::Manufacturer));
        assert_eq!(item_kind("device"), None, "a project kind");
        assert_eq!(item_kind("frobnicator"), None);
    }

    #[test]
    fn pn_norm_follows_doc_07() {
        assert_eq!(pn_norm("SK 3110.000"), "SK3110000");
        assert_eq!(pn_norm("5SY4 204-7"), "5SY42047");
        assert_eq!(pn_norm("5SY4204-7"), "5SY42047");
        assert_eq!(pn_norm("a9p 52610"), "A9P52610");
        assert_eq!(pn_norm("PT 2,5/1P_x"), "PT2,51PX");
        assert_eq!(pn_norm(""), "");
    }

    #[test]
    fn scores_below_the_quantum_tie() {
        assert_eq!(quantise(-1.000_000_1), quantise(-1.000_000_2));
        assert!(quantise(-2.0) > quantise(-1.0), "a lower bm25 is better");
        assert_eq!(quantise(f64::NAN), 0);
    }

    fn key(pn: &str, score: f64, item: u128) -> (OrderKey, u128) {
        (
            OrderKey {
                fit: 1,
                rank_tier: 4,
                not_held: true,
                not_boosted: true,
                score: quantise(score),
                part_number: pn.into(),
                manufacturer_key: "SE".into(),
                library: Uuid::from_u128(1),
                item: Uuid::from_u128(item),
            },
            item,
        )
    }

    /// IX-7 and the ruling on the quantum: two hits whose bm25 differ only below 10⁻⁶ order by the natural part
    /// number, not by the float.
    #[test]
    fn scores_that_differ_below_the_quantum_order_by_natural_part_number() {
        let a = key("X10", -3.000_000_01, 1);
        let b = key("X2", -3.000_000_02, 2);
        assert_eq!(sort_by_order(vec![a, b]), [2, 1], "X2 before X10");
        let a = key("X10", -3.000_000_01, 1);
        let b = key("X2", -3.000_000_02, 2);
        assert_eq!(sort_by_order(vec![b, a]), [2, 1], "input order does not matter");
        // A clearly better score wins over the natural order.
        let a = key("X10", -4.0, 1);
        let b = key("X2", -3.0, 2);
        assert_eq!(sort_by_order(vec![b, a]), [1, 2]);
    }

    #[test]
    fn ties_break_by_manufacturer_library_and_item() {
        let mut a = key("A1", -1.0, 9);
        let mut b = key("A1", -1.0, 3);
        a.0.manufacturer_key = "PXC".into();
        b.0.manufacturer_key = "SE".into();
        assert_eq!(sort_by_order(vec![b, a]), [9, 3], "manufacturer key");
        let a = key("A1", -1.0, 9);
        let b = key("A1", -1.0, 3);
        assert_eq!(sort_by_order(vec![a, b]), [3, 9], "item ID");
    }

    #[test]
    fn held_and_boosted_parts_come_first_inside_a_tier_only() {
        let mut held = key("Z9", -1.0, 1);
        held.0.not_held = false;
        let mut boosted = key("Z8", -1.0, 2);
        boosted.0.not_boosted = false;
        let plain = key("A1", -9.0, 3);
        let mut exact = key("ZZ", 0.0, 4);
        exact.0.rank_tier = 1;
        assert_eq!(sort_by_order(vec![plain, boosted, held, exact]), [4, 1, 2, 3]);
    }

    #[test]
    fn natural_keys_follow_ew_natural() {
        let mut v = vec!["X10", "X2", "x1", "X1"];
        v.sort_by_key(|s| natural_key(s));
        assert_eq!(v, ["X1", "x1", "X2", "X10"]);
        assert!(natural_key("A\u{7}").is_err(), "not an identifier");
    }
}
