// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The holes of a surface in the window (doc 12-3d §10.6, doc 13 §9.2): Add hole here… (`layout.feature_add`), the form
//! of a hole by hand on the focused panel page's surface, and Hole tables (`lists.holes`), the project's hole table.

use serde_json::{Map, Value, json};

use super::{Mode, Outcome, ShellModel, failed};
use crate::key;
use crate::strings;

/// The fields of Add hole here…, by their parameter names.
const KIND: &str = "kind";
const SIZE: &str = "size";
const X: &str = "x";
const Y: &str = "y";
const PURPOSE: &str = "purpose";

impl ShellModel {
    /// `layout.feature_add {x_mm?, y_mm?, kind?, size?, x?, y?, purpose?}` (Add hole here…, the panel page's context
    /// menu): without a kind in the window, the form "Add a hole to -UC1 door" with Kind, Size, X and Y from the surface's
    /// datum (the menu's sheet point `x_mm`, `y_mm`, on whole millimetres) and For; with a kind and a size, one
    /// `layout.feature.add` on the focused panel page's surface, X and Y taken back from the datum. Headless, the kind
    /// and the size are needed. A text that does not read is refused in the form, in words.
    pub(crate) fn act_layout_feature_add(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "layout.feature_add";
        let Some(tables) = self.panel_tables() else {
            return Outcome::refused(key!("reason-no-panel-page"));
        };
        let text = |name: &str| {
            p.get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
        };
        let Some(session) = self.project.as_ref().map(|x| &x.session) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let (Some(kind), Some(size)) = (text(KIND), text(SIZE)) else {
            // The form's Add without a size says what it needs.
            if self.in_form_of(ACTION) {
                return self.form_failed(
                    ACTION,
                    strings::text(key!("feature-form-size-missing")).to_owned(),
                );
            }
            if self.mode == Mode::Headless {
                return failed(
                    key!("error-missing-param"),
                    &[if text(KIND).is_none() { KIND } else { SIZE }],
                );
            }
            // The menu's point on the surface, measured from its datum, on whole millimetres.
            let at = match super::canvas_actions::point_mm(p) {
                Ok(at) => at,
                Err(out) => return out,
            };
            let measured = at
                .and_then(|s| Some(self.canvas()?.page().snapper.sheet_to_page(s)))
                .map(|page| session.surface_measured(tables.surface, tables.to_surface(page)))
                .unwrap_or_default();
            let mm = |n: i128| (n + 500_000).div_euclid(1_000_000).to_string();
            let kinds = ew_api::HoleKind::ALL
                .iter()
                .map(|k| {
                    let w = k.word();
                    (
                        w.to_owned(),
                        strings::field_choice_text("surface_feature", "derived.kind", w),
                    )
                })
                .collect();
            let form = super::FormDialog {
                action: ACTION,
                title_key: key!("feature-form-title"),
                ok_key: key!("feature-form-add"),
                fields: vec![
                    field(KIND, key!("feature-form-kind"), kinds, "fixing"),
                    field(SIZE, key!("feature-form-size"), Vec::new(), ""),
                    field(X, key!("feature-form-x"), Vec::new(), &mm(measured.0)),
                    field(Y, key!("feature-form-y"), Vec::new(), &mm(measured.1)),
                    field(PURPOSE, key!("feature-form-purpose"), Vec::new(), ""),
                ],
                error: None,
                table: None,
            };
            return self.open_dialog(super::Dialog::Form(form));
        };
        let coord = |name: &str| text(name).and_then(ew_api::output::parse_mm);
        let (Some(x), Some(y)) = (coord(X), coord(Y)) else {
            return self.form_failed(ACTION, strings::text(key!("feature-form-xy")).to_owned());
        };
        let (u, v) = session.surface_measured(tables.surface, (x, y));
        let (Ok(u), Ok(v)) = (i64::try_from(u), i64::try_from(v)) else {
            return self.form_failed(ACTION, strings::text(key!("feature-form-xy")).to_owned());
        };
        let mut params = json!({
            "surface": tables.surface.to_string(),
            "at": [u, v],
            "kind": kind,
            "size": size,
        });
        if let Some(purpose) = text(PURPOSE) {
            params["purpose"] = json!(purpose);
        }
        let out = self.host_create("layout.feature.add", params);
        self.layout_form_done(ACTION, out)
    }

    /// `layout.glands_place {cables?, x?, y?}` (doc 12 §3.9): the glands of `cables` (references separated by commas,
    /// `tag:-101W1`; absent: every gland that waits in the cabinet) as one `layout.glands_place` on the focused panel
    /// page's surface, a row to the right from `x`, `y` mm measured from the surface's datum (absent: 10 mm in from its
    /// bottom-left edge margin). Refused in words as the command refuses.
    pub(crate) fn act_layout_glands_place(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(tables) = self.panel_tables() else {
            return Outcome::refused(key!("reason-no-panel-page"));
        };
        let Some(session) = self.project.as_ref().map(|x| &x.session) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let text = |name: &str| {
            p.get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
        };
        let cables: Vec<String> = match text("cables") {
            Some(list) => {
                let mut out = Vec::new();
                for r in list.split(',').map(str::trim).filter(|r| !r.is_empty()) {
                    match session.resolve_reference(r) {
                        Ok(id) => out.push(id.to_string()),
                        Err(why) => return Outcome::Refused { reason: why },
                    }
                }
                out
            }
            None => session
                .waiting_glands(tables.surface)
                .iter()
                .map(ToString::to_string)
                .collect(),
        };
        if cables.is_empty() {
            return Outcome::refused(key!("reason-no-glands"));
        }
        let mm = |name: &str| text(name).and_then(ew_api::output::parse_mm);
        let measured = (mm("x").unwrap_or(40_000_000), mm("y").unwrap_or(40_000_000));
        let (u, v) = session.surface_measured(tables.surface, measured);
        let (Ok(u), Ok(v)) = (i64::try_from(u), i64::try_from(v)) else {
            return failed(key!("error-bad-param"), &["x"]);
        };
        self.host_create(
            "layout.glands_place",
            json!({"surface": tables.surface.to_string(), "at": [u, v], "cables": cables}),
        )
    }

    /// The title of Add hole here…: "Add a hole to -UC1 door".
    pub fn feature_form_title(&self) -> String {
        let words = self
            .panel_tables()
            .and_then(|t| self.project.as_ref()?.session.surface_in_words(t.surface));
        match words {
            Some(w) => strings::format(key!("feature-form-title-of"), &[&strings::render(&w)]),
            None => strings::text(key!("feature-form-title")).to_owned(),
        }
    }

    /// `lists.holes` (Output ▸ Workshop ▸ Hole tables, doc 13 §9): the first page of the project's hole table, else
    /// Report block… on that kind, whose Create makes it.
    pub(crate) fn act_lists_holes(&mut self) -> Outcome {
        match self.list_folio(&["hole_table"]) {
            Some(folio) => self.open_list(&folio),
            None if self.mode == Mode::Gui => {
                self.act_report_block_new(&Map::from_iter([("kind".to_owned(), Value::from("hole_table"))]))
            }
            None => Outcome::refused(key!("reason-no-hole-table")),
        }
    }
}

/// A field of Add hole here….
fn field(
    name: &'static str,
    label: strings::Key<'static>,
    options: Vec<(String, String)>,
    value: &str,
) -> super::FormField {
    super::FormField {
        name: name.into(),
        label: None,
        label_key: label,
        choices: &super::choices::NONE,
        options,
        value: value.to_owned(),
    }
}
