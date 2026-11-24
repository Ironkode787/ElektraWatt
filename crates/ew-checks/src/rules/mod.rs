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

/// A record as a finding names it: its printed name, or the words for one without (`a device`), as the subject prints
/// it ([`fmt::Display`], English) and as a pattern reads it (in the reader's language).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Named {
    english: String,
    words: TextArg,
}

impl Named {
    /// The name as the subject prints it.
    pub fn english(&self) -> &str {
        &self.english
    }
}

impl std::ops::Deref for Named {
    type Target = str;

    fn deref(&self) -> &str {
        &self.english
    }
}

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.english)
    }
}

impl From<String> for Named {
    fn from(s: String) -> Named {
        Named {
            words: TextArg::Plain(s.clone()),
            english: s,
        }
    }
}

impl From<&str> for Named {
    fn from(s: &str) -> Named {
        Named::from(s.to_owned())
    }
}

impl From<Text> for Named {
    fn from(t: Text) -> Named {
        Named {
            english: t.to_string(),
            words: t.into(),
        }
    }
}

impl From<TextArg> for Named {
    fn from(words: TextArg) -> Named {
        Named {
            english: words.to_string(),
            words,
        }
    }
}

impl From<Named> for String {
    fn from(n: Named) -> String {
        n.english
    }
}

impl From<Named> for TextArg {
    fn from(n: Named) -> TextArg {
        n.words
    }
}

impl From<&Named> for TextArg {
    fn from(n: &Named) -> TextArg {
        n.words.clone()
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
