// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Title-block values: `title_block(page)` (doc 09 §2.6–§2.7, FR-REP-033, FR-REP-036, FR-REP-037,
//! FR-REV-003, D-05, D-06, D-30).

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_mltext::MlText;
use ew_schema::{Date, Sha256};

use crate::Q;
use crate::pages::PageRef;
use crate::query::{Policy, Query};

/// One bound value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TbValue {
    /// Plain text.
    Text(String),
    /// Multi-language text.
    Ml(MlText),
    /// A date, printed in the project's date format by the form layer.
    Date(Date),
    /// A tag in its two printed forms.
    Tag {
        /// The full tag.
        full: String,
        /// The displayed tag.
        displayed: String,
    },
    /// An image asset by content hash (the logo); prints no text.
    Asset(Sha256),
}

/// The values of every path of the profile's title-block binding set that has a value on one page.
/// A path without a value is absent (it renders empty, decision log ID-40).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TitleBlock {
    /// Path (`page.title`) → value.
    pub values: BTreeMap<String, TbValue>,
}

impl TitleBlock {
    /// The value at a path.
    pub fn get(&self, path: &str) -> Option<&TbValue> {
        self.values.get(path)
    }
}

/// `title_block(page)`.
#[derive(Debug)]
pub struct TitleBlockValues;

impl Query for TitleBlockValues {
    type Key = PageRef;
    type Value = TitleBlock;
    const FAMILY: &'static str = "title_block";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PageRef) -> Arc<TitleBlock> {
        let _ = (q, key);
        Arc::new(TitleBlock::default())
    }
}
