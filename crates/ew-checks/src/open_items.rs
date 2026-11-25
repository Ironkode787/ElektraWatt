// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The open-items list (doc 15 §2.3 `open_item`, §7; FR-REP-021).
//!
//! An open item is an **open** finding (not waived, not superseded) of an **open-item rule** in **release mode** of the
//! project's release scheme ([`release_scheme_of`]), through the engine's own path ([`run_table_rules`]), so a finding
//! is listed exactly when `ew check --mode release` prints it open. The open-item rules are data here
//! ([`OPEN_ITEM_RULES`]), not a [`RuleMeta`](crate::RuleMeta) member; the rule catalogue snapshot and the
//! generated rule pages show them.
//!
//! **Contract:** no open-item rule reads the page order (`PageOrder`): the `open_items` report kind of
//! `ew-reports` paginates the list inside the page order, so a rule that read it would close a query cycle. Adding such
//! a rule to the table is a stop for the lane.

use ew_derive::{Cycle, Q};

use crate::engine::run_table_rules;
use crate::finding::{Finding, FindingState};
use ew_derive::text::OpenItem;

use crate::registry::rules;
use crate::rule::{Rule, RuleMeta};
use crate::scheme::{CheckMode, release_scheme_of};

/// One row of the open-item table: a rule whose open findings are open items, of one category.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OpenItemRule {
    /// The rule (EW-CORE-041's `DEVICE_WITHOUT_PART`).
    pub rule: &'static RuleMeta,
    /// Only the rule's findings with this discriminator are open items (EW-BUS-001's `address`: a missing station
    /// address is an open item, a duplicate one is not); `None`: every finding of the rule.
    pub discriminator: Option<&'static str>,
    /// The category, the words the `category` column of the list prints ([`ew_derive::text::open_item`]).
    pub category: OpenItem,
}

/// The open-item rules, by category in the list's order, then by rule ID.
pub const OPEN_ITEM_RULES: &[OpenItemRule] = &[
    OpenItemRule {
        rule: &crate::rules::device::PLACEHOLDER_PART,
        discriminator: None,
        category: OpenItem::TbdValue,
    },
    OpenItemRule {
        rule: &crate::rules::device::OPEN_TBD_VALUE,
        discriminator: None,
        category: OpenItem::TbdValue,
    },
    OpenItemRule {
        rule: &crate::rules::plc::TBD_ADDRESS,
        discriminator: None,
        category: OpenItem::TbdValue,
    },
    OpenItemRule {
        rule: &crate::rules::bus::BUS_ADDRESS,
        discriminator: Some("address"),
        category: OpenItem::TbdValue,
    },
    OpenItemRule {
        rule: &crate::rules::device::DEVICE_WITHOUT_PART,
        discriminator: None,
        category: OpenItem::DeviceWithoutPart,
    },
    OpenItemRule {
        rule: &crate::rules::layout::UNPLACED_DEVICE,
        discriminator: None,
        category: OpenItem::UnplacedDevice,
    },
    OpenItemRule {
        rule: &crate::rules::pot::UNCONNECTED_CP,
        discriminator: None,
        category: OpenItem::UnconnectedCp,
    },
    OpenItemRule {
        rule: &crate::rules::core::CABLE_WITHOUT_LENGTH,
        discriminator: None,
        category: OpenItem::MissingLength,
    },
];

/// The open-item row a finding falls under: its rule is in [`OPEN_ITEM_RULES`] and, where the row names a
/// discriminator, the finding's discriminator is that one. Says nothing about the finding's state.
pub fn open_item_of(f: &Finding) -> Option<&'static OpenItemRule> {
    OPEN_ITEM_RULES
        .iter()
        .find(|r| r.rule.id == f.rule && r.discriminator.is_none_or(|d| d == f.discriminator))
}

/// The registered rules of [`OPEN_ITEM_RULES`], each once, in registry (rule-ID) order.
fn table_rules() -> Vec<&'static dyn Rule> {
    rules()
        .iter()
        .copied()
        .filter(|r| OPEN_ITEM_RULES.iter().any(|o| o.rule.id == r.meta().id))
        .collect()
}

/// The open items of the snapshot: the open findings of the open-item rules in release mode of the
/// project's release scheme, else `pre-delivery` ([`release_scheme_of`]), that [`open_item_of`] places, in the
/// engine's canonical order. Only the table's rules run, every finding of each (EW-BUS-001's `duplicate` ones
/// take part in supersession), and the filter keeps the open items; the engine's own findings of the run (EW-CORE-081
/// for a waiver of another rule, EW-CORE-083) are not open items.
///
/// # Errors
///
/// The [`Cycle`] when a rule's memo is already evaluating (a rule that reached the list through the page order): the
/// caller records it and lists nothing.
pub fn open_items(q: &Q) -> Result<Vec<Finding>, Cycle> {
    let report = run_table_rules(
        q,
        release_scheme_of(q),
        CheckMode::Release,
        &table_rules(),
        crate::Evidence::default(),
    )?;
    Ok(report
        .findings
        .into_iter()
        .filter(|f| f.state == FindingState::Open && open_item_of(f).is_some())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule;
    use crate::scheme::PRE_DELIVERY;

    /// Every rule of the table is registered, and runs in release mode of the shipped release scheme.
    #[test]
    fn every_open_item_rule_is_registered() {
        for o in OPEN_ITEM_RULES {
            let r = rule(o.rule.id).unwrap_or_else(|| panic!("{} is not registered", o.rule.id));
            assert!(
                PRE_DELIVERY.enabled(r.meta(), CheckMode::Release),
                "{} does not run in release mode of pre-delivery",
                o.rule.id
            );
        }
        assert_eq!(table_rules().len(), OPEN_ITEM_RULES.len(), "each rule once");
    }

    /// Ruling 3: "any registered rule whose `supersedes` names a table rule is in the table", so supersession
    /// inside the subset run equals supersession in `ew check --mode release`.
    #[test]
    fn every_rule_that_supersedes_a_table_rule_is_in_the_table() {
        for r in rules() {
            let m = r.meta();
            for s in m.supersedes {
                if OPEN_ITEM_RULES.iter().any(|o| o.rule.id == *s) {
                    assert!(
                        OPEN_ITEM_RULES.iter().any(|o| o.rule.id == m.id),
                        "{} supersedes the open-item rule {s} but is not in OPEN_ITEM_RULES: add {} to the table \
                         or change the table, or the open-items list differs from `ew check`",
                        m.id,
                        m.id
                    );
                }
            }
        }
    }

    /// `open_item_of` matches the rule and, for EW-BUS-001, the discriminator `address` only.
    #[test]
    fn open_item_of_matches_the_rule_and_the_discriminator() {
        let meta = rule("EW-BUS-001").expect("registered").meta();
        let address = Finding::new(meta, [], "address", "-201U1:DP");
        let duplicate = Finding::new(meta, [], "duplicate", "-201U1:DP");
        assert_eq!(
            open_item_of(&address).map(|o| o.category),
            Some(OpenItem::TbdValue)
        );
        assert_eq!(open_item_of(&duplicate), None);
        let m041 = rule("EW-CORE-041").expect("registered").meta();
        let f = Finding::new(m041, [], "anything", "-K1");
        assert_eq!(
            open_item_of(&f).map(|o| o.category),
            Some(OpenItem::DeviceWithoutPart)
        );
        let other = Finding::new(rule("EW-TERM-002").expect("registered").meta(), [], "", "");
        assert_eq!(open_item_of(&other), None);
    }
}
