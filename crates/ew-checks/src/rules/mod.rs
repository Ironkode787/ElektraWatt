// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shipped Rust rules, one module per pack family (doc 15 §4). Each rule is a unit struct with a
//! `RuleMeta` constant that copies its row of the catalogue (ID, code, pack, kind, modes, severities,
//! scope) and implements [`Rule`](crate::Rule).

pub mod bus;
// The EW-BUS rules of the pack's 1.0 content
pub mod bus_net;
pub mod cable;
pub mod core;
pub mod device;
// The EW-FL MVP rules
pub mod fluid;
// The EW-FL 1.0 rules
pub mod fluid_media;
pub mod imp;
// The EW-IOL rules
pub mod iolink;
// The EW-LAYOUT MVP rules
pub mod layout;
pub mod num;
pub mod page;
pub mod plc;
// The EW-PLC rules of the pack's 1.0 content
pub mod plc_exchange;
pub mod plc_wiring;
pub mod pot;
// EW-PRC-001
pub mod process;
pub mod rds;
pub mod rel;
// The EW-SAF rules
pub mod safety;
pub mod system;
pub mod term;

use std::fmt;

use ew_l10n::{List, Text, TextArg};

/// A record as a finding names it: its printed name, or the words for one without (`a device`), in the reader's
/// language.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Named(TextArg);

impl Named {
    /// The name in English, rendered from its words (to sort by, as the canonical order does).
    pub fn english(&self) -> String {
        self.0.to_string()
    }

    /// The order of names: their English in natural order, then by bytes.
    pub fn order(&self, other: &Named) -> std::cmp::Ordering {
        let (a, b) = (self.english(), other.english());
        ew_mltext::natural_cmp(&a, &b).then_with(|| a.cmp(&b))
    }
}

/// English, as [`Named::english`].
impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Named {
    fn from(s: String) -> Named {
        Named(TextArg::Plain(s))
    }
}

impl From<&str> for Named {
    fn from(s: &str) -> Named {
        Named::from(s.to_owned())
    }
}

impl From<Text> for Named {
    fn from(t: Text) -> Named {
        Named(t.into())
    }
}

impl From<TextArg> for Named {
    fn from(words: TextArg) -> Named {
        Named(words)
    }
}

impl From<Named> for TextArg {
    fn from(n: Named) -> TextArg {
        n.0
    }
}

impl From<&Named> for TextArg {
    fn from(n: &Named) -> TextArg {
        n.0.clone()
    }
}

/// A device as the fluid messages print it: its displayed tag without structure (`-201U3-Q1`), else the word `device`
/// (a device without a tag, or whose tag prints empty).
pub(crate) fn device_tag(q: &ew_derive::Q, d: ew_ids::Id<ew_schema::Device>) -> Named {
    let t = ew_derive::short_tag(q, d);
    if t.is_empty() {
        text!("word-device").into()
    } else {
        t.into()
    }
}

/// Items joined by commas ("A, B, C"), each in the reader's language.
pub(crate) fn listed(items: impl IntoIterator<Item = impl Into<TextArg>>) -> TextArg {
    List::new(
        items.into_iter().map(Into::into).collect(),
        text!("join-comma"),
        text!("join-comma"),
    )
    .into()
}

/// Items in a sentence ("A, B and C"), the last after the reader's language's "and".
pub(crate) fn and_listed(items: impl IntoIterator<Item = impl Into<TextArg>>) -> TextArg {
    List::new(
        items.into_iter().map(Into::into).collect(),
        text!("join-comma"),
        text!("join-and"),
    )
    .into()
}

/// Sentences one after the other, each in the reader's language.
pub(crate) fn sentences(items: impl IntoIterator<Item = impl Into<TextArg>>) -> TextArg {
    List::new(
        items.into_iter().map(Into::into).collect(),
        text!("join-sentence"),
        text!("join-sentence"),
    )
    .into()
}

/// The two ends of a wire ("A–B"), each in the reader's language.
pub(crate) fn wire_ends(a: impl Into<TextArg>, b: impl Into<TextArg>) -> TextArg {
    List::new(vec![a.into(), b.into()], text!("join-wire"), text!("join-wire")).into()
}
