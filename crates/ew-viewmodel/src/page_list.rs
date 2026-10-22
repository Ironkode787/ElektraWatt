// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page-list model (WP-S2.01 part 1, spec section 5.3): the printed folio labels of the open project in page
//! order, the current one selected. A plain list, not the page navigator of WP-S2.03; a click is `page.goto`.

use crate::shell::ShellModel;

/// The folio labels and the selected one.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PageList {
    /// The printed folio labels, in page order.
    pub labels: Vec<String>,
    /// The index of the current page, `None` when the project has no pages.
    pub current: Option<usize>,
}

impl PageList {
    /// The page list of `model`.
    pub fn of(model: &ShellModel) -> PageList {
        let labels = model.page_labels().to_vec();
        let current = (!labels.is_empty()).then(|| model.page_index());
        PageList { labels, current }
    }
}
