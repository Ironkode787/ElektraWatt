// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The status-bar model (WP-S2.01 part 1, spec section 5.3; UX/UI doc 16 section 3.6).
//!
//! Left to right: the hint, the page as `<folio> (<n> of <total>)`, the save state (`*` unsaved, `saved`, or
//! `read-only: <reason>`), the render backend of the page view, the UI language, and while an export runs the
//! export label. Without a project the page and save fields are left out.

use crate::shell::{AccessMode, ShellModel};
use crate::strings;

/// The status-bar fields of `model`; `exporting` adds the label of a running export.
pub fn fields(model: &ShellModel, exporting: bool) -> Vec<String> {
    let mut out = Vec::new();
    match model.project_name() {
        None => out.push(strings::text("status-hint-start").to_owned()),
        Some(_) => {
            out.push(strings::text("status-hint-view").to_owned());
            if let Some(folio) = model.current_folio() {
                out.push(strings::format(
                    "status-page",
                    &[
                        folio,
                        &(model.page_index() + 1).to_string(),
                        &model.page_count().to_string(),
                    ],
                ));
            }
            out.push(match model.access() {
                Some(AccessMode::ReadOnly { reason_key }) => {
                    strings::format("status-read-only", &[strings::text(reason_key)])
                }
                _ if model.is_dirty() => strings::text("status-unsaved").to_owned(),
                _ => strings::text("status-saved").to_owned(),
            });
        }
    }
    out.push(strings::text("status-backend").to_owned());
    out.push(strings::text("status-language").to_owned());
    if exporting {
        out.push(strings::text("status-exporting").to_owned());
    }
    out
}
