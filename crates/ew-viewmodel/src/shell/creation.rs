// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Creating pages, structure nodes and external references from the user interface:
//! `page.new`, `structure.new`
//! and `external_ref.new`.
//!
//! **One path.** Each action builds one existing command (`page.create`, `structure.create_node`,
//! `external_ref.create`) and commits it as the property grid does: `Session::preview_with` once, the
//! prepared preview kept in the host, then the host's commit path (the window `Session::commit`, headless one command-log
//! line through `Session::run_script`, so `--record` and the exposure rules apply). A plain create is a single target
//! and no identifier change on several records, so the policy of FR-UI-044 commits it at once: no card.
//!
//! **Parameters.** A parameter that names a record takes the record's designation as the user types it (`J-E-206`,
//! the node's code), or its alias `@n` or UUID (the action's alias parameters); a code that two nodes of the aspect share is
//! refused with the instruction to give the alias. Nothing outside the project is read.
//!
//! **The dialog** (the pattern of Go to page and Save layout as: a small dialog, opened by the action when it is
//! dispatched in the window without its required parameters, never headless). One [`FormDialog`] holds the fields of
//! the action, Enter in a field creates, Escape cancels, Tab moves between the fields; a refusal of the command stays
//! in the dialog under the fields.
//!
//! **After a page is created** the focused tab shows it: a person who creates a page works on it next, and a page
//! change ends a modal tool like any other.

use ew_api::AnyCommand;
use ew_api::output::PageRow;
use serde_json::{Map, Value, json};

use super::forms::{Field, FormDialog, Spec, field, text_param};
use super::{Dialog, Outcome, ShellModel};
use crate::key;
use crate::strings;

mod pages;

mod forms;

/// The page types `page.new` creates: `PageType`'s snake_case spellings except `report`, whose pages are generated
/// (EW-INV-012). The signature of the action in `actions.rs` lists the same words (a test compares them).
pub const PAGE_TYPES: &[&str] = &[
    "title_page",
    "table_of_contents",
    "section_divider",
    "graphic",
    "single_line",
    "multi_line",
    "overview",
    "bus_topology",
    "fluid",
    "assembly_overview",
    "panel_layout",
    "model_view",
    "external_document",
];

/// The connection grids `page.new` gives a page: the native 4 mm grid and QET's 10 px, as `Grid::MM4` and
/// `Grid::QET_10PX`. The signature of the action lists the same words (a test compares them).
pub const PAGE_GRIDS: &[&str] = &["mm4", "qet_10px"];

/// The grid of a word of [`PAGE_GRIDS`] as `page.create` takes it (`{num, den}` nanometres per step).
pub(crate) fn page_grid_value(word: &str) -> Option<Value> {
    match word {
        "mm4" => Some(json!({"num": 4_000_000, "den": 1})),
        "qet_10px" => Some(json!({"num": 7_937_500, "den": 3})),
        _ => None,
    }
}

/// The word of [`PAGE_GRIDS`] of a grid of `num` nanometres per `den` steps, `None` for any other grid.
pub(crate) fn page_grid_word(num: i64, den: i64) -> Option<&'static str> {
    PAGE_GRIDS
        .iter()
        .copied()
        .find(|w| page_grid_value(w) == Some(json!({"num": num, "den": den})))
}

/// The aspects of a structure tree `structure.new` creates a node in (the product aspect is the device tree).
pub const STRUCTURE_KINDS: &[&str] = &[
    "functional_assignment",
    "function",
    "installation_site",
    "location",
    "document_kind",
    "user_defined",
    "type",
];

/// The prefix sign of each structure aspect (doc 03 §5, `Aspect::default_sign`), the longer signs first so that `==` is
/// read before `=` and `++` before `+`.
pub(super) const SIGNS: [(&str, &str); 7] = [
    ("==", "functional_assignment"),
    ("++", "installation_site"),
    ("=", "function"),
    ("+", "location"),
    ("&", "document_kind"),
    ("#", "user_defined"),
    ("%", "type"),
];

/// `text` as the code of a node of `aspect`: the aspect's own prefix sign (`+J-E-206`) is taken off, another aspect's sign
/// is refused with a text naming both, and any sign left in the code (`+K1+K2`) is refused: a designation names one
/// aspect, so it holds one sign.
pub(super) fn strip_sign(aspect: &str, text: &str) -> Result<String, String> {
    let code = match SIGNS.iter().find(|e| text.starts_with(e.0)) {
        Some(&(sign, of)) if of == aspect => &text[sign.len()..],
        Some(&(sign, of)) => {
            return Err(strings::format(
                key!("error-wrong-sign"),
                &[text, sign, &of.replace('_', " "), &aspect.replace('_', " ")],
            ));
        }
        None => text,
    };
    let code = code.trim();
    if let Some((second, _)) = SIGNS.iter().find(|(sign, _)| code.contains(sign)) {
        return Err(strings::format(key!("error-second-sign"), &[text, second]));
    }
    Ok(code.to_owned())
}

/// The codes from the root to node `id` in `nodes` (id, code, parent id).
fn node_path(nodes: &[(String, String, Option<String>)], id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = Some(id.to_owned());
    while let Some(c) = cur.take() {
        let Some(n) = nodes.iter().find(|n| n.0 == c) else {
            break;
        };
        out.push(n.1.clone());
        if out.len() < 64 {
            cur = n.2.clone();
        }
    }
    out.reverse();
    out
}

/// New medium… (doc 11 §2.5, FR-FLU-008): a usual medium to start from, then the medium's name and its nominal
/// pressure in bar. The ISO 1219-2 letter (`code`) is no field: nothing reads it yet; the action takes it.
pub(super) static MEDIUM_NEW: Spec = Spec {
    action: "fluid.medium_new",
    title_key: key!("medium-new-title"),
    fields: &[
        field("preset", key!("form-medium-preset")),
        field("name", key!("form-medium-name")),
        field("p_nom", key!("form-medium-pressure")),
    ],
};

/// Fluid ▸ Valve terminal… on a fluid page while a valve terminal of the project is drawn on no fluid page: which
/// to draw, each such terminal ("Draw -104A1 here") or a new one.
static VALVE_TERMINAL: Spec = Spec {
    action: "fluid.valve_terminal",
    title_key: key!("valve-terminal-title"),
    fields: &[field("terminal", key!("form-valve-terminal"))],
};

/// Put a station on this page: on a bus-topology page, which station or bus device of the project the page does
/// not draw yet ("Put -1A1 here"), or a new one.
static PUT_STATION: Spec = Spec {
    action: "bus.put_station",
    title_key: key!("put-station-title"),
    fields: &[field("station", key!("form-put-station"))],
};

/// Draw the plug… from a page that is no electrical page (Draw plug went to page 1 without a word):
/// on which electrical page, or a new one. The plug's terminal stands in the form as what it draws.
static DRAW_PLUG: Spec = Spec {
    action: "fluid.draw_plug",
    title_key: key!("draw-page-title"),
    fields: &[
        field("terminal", key!("form-draw-what")),
        field("page", key!("form-draw-page")),
    ],
};

/// Draw solenoid 14… from a page that is no electrical page: as [`DRAW_PLUG`], the coil standing as what it draws.
static DRAW_SOLENOID: Spec = Spec {
    action: "fluid.draw_solenoid",
    title_key: key!("draw-page-title"),
    fields: &[
        field("function", key!("form-draw-what")),
        field("page", key!("form-draw-page")),
    ],
};

/// The usual media New medium… starts from, each with its usual nominal pressure in bar (gauge: a vacuum
/// below the atmosphere); its name is the text `medium-preset-<word>`.
const MEDIUM_PRESETS: &[(&str, &str)] = &[
    ("compressed_air", "6"),
    ("vacuum", "-0.9"),
    ("hydraulic_oil", "160"),
    ("cooling_water", "4"),
    ("lubricant", "5"),
    ("inert_gas", "10"),
];

/// The name and the nominal pressure of preset `word` of [`MEDIUM_PRESETS`] in the UI language.
pub(super) fn medium_preset(word: &str) -> Option<(String, &'static str)> {
    let (_, p_nom) = MEDIUM_PRESETS.iter().find(|(w, _)| *w == word)?;
    let name = strings::lookup(&format!("medium-preset-{}", word.replace('_', "-")))?;
    Some((name.to_owned(), p_nom))
}

/// The value of a usual medium in a Medium row's choice: `preset:<word>`.
pub(super) const PRESET_PREFIX: &str = "preset:";

/// The usual media as a Medium row's choices: "Compressed air, 6 bar", its value `preset:<word>`.
pub(super) fn medium_preset_choices() -> Vec<(String, Value)> {
    MEDIUM_PRESETS
        .iter()
        .filter_map(|(w, _)| {
            let (name, p_nom) = medium_preset(w)?;
            Some((
                strings::format(key!("medium-preset-choice"), &[&name, p_nom]),
                Value::String(format!("{PRESET_PREFIX}{w}")),
            ))
        })
        .collect()
}

/// The `set` of `fluid.medium.edit` that makes usual medium `word`, as New medium… makes it from that preset.
pub(super) fn medium_preset_set(word: &str) -> Option<Value> {
    let (name, p_nom) = medium_preset(word)?;
    Some(json!({"name": name, "p_nom": p_nom}))
}

/// PLC exchange ▸ Export…'s form (doc 10 §6): the format and version, the stations (tags; empty: all) and the document
/// date (in the window today's to start with; empty: the model's).
static PLC_EXPORT: Spec = Spec {
    action: "plc.export",
    title_key: key!("plc-export-title"),
    fields: &[
        field("format", key!("form-plc-format")),
        field("stations", key!("form-plc-stations")),
        field("date", key!("form-plc-date")),
    ],
};

/// Project ▸ Process-tag scheme… (FR-PLC-049, doc 10 §3.4): the regular expression with named groups a process tag
/// matches; empty removes it.
static PROCESS_TAG_SCHEME: Spec = Spec {
    action: "plc.process_tag_scheme",
    title_key: key!("process-tag-scheme-title"),
    fields: &[field("scheme", key!("form-process-tag-scheme"))],
};

/// Project ▸ Fluid settings… (doc 11 §2.4, §5.4): the default tube of every fluid line (outer and inner diameter in
/// mm, material) and whether fluid lines carry a tag of their page and a counter.
static FLUID_SETTINGS: Spec = Spec {
    action: "fluid.settings",
    title_key: key!("fluid-settings-title"),
    fields: &[
        field("od", key!("form-tube-od")),
        field("id", key!("form-tube-id")),
        field("material", key!("form-tube-material")),
        Field {
            name: "line_tags",
            label_key: key!("form-line-tags"),
            choices: &super::choices::BOOL,
            default: "false",
        },
    ],
};

/// A configuration project row's Symbol policy… (doc 10 §2.2, FR-PLC-047): the template of the automatic symbol, the
/// PLC tag table and whether symbols are strict IEC 61131-3 identifiers.
static SYMBOL_POLICY: Spec = Spec {
    action: "plc.symbol_policy",
    title_key: key!("symbol-policy-title"),
    fields: &[
        field("template", key!("form-symbol-template")),
        field("tag_table", key!("form-tag-table")),
        Field {
            name: "strict_iec",
            label_key: key!("form-strict-iec"),
            choices: &super::choices::BOOL,
            default: "true",
        },
    ],
};

/// A module row's Safety data… (doc 10 §2.9, FR-PLC-041): the F-source and F-destination addresses and the
/// F-watchdog time in ms.
static MODULE_SAFETY: Spec = Spec {
    action: "plc.module_safety",
    title_key: key!("module-safety-title"),
    fields: &[
        field("f_source", key!("form-f-source")),
        field("f_dest", key!("form-f-dest")),
        field("f_wd_ms", key!("form-f-watchdog")),
    ],
};

/// A channel row's Mode and safety… (doc 10 §2.4, §2.9): a multi-function port's mode and the channel's safety
/// evaluation and requirement.
static CHANNEL_SETTINGS: Spec = Spec {
    action: "plc.channel_settings",
    title_key: key!("channel-settings-title"),
    fields: &[
        field("mode", key!("form-channel-mode")),
        field("evaluation", key!("form-safety-evaluation")),
        field("requirement", key!("form-safety-requirement")),
    ],
};

/// The field whose type gives the tube materials Fluid settings… offers (a medium's default tube has the project
/// default's type).
const TUBE_MATERIAL: (&str, &str) = ("medium", "default_tube.material");

pub(super) static PAGE_NEW: Spec = Spec {
    action: "page.new",
    title_key: key!("new-page-title"),
    fields: &[
        field("folio", key!("form-folio")),
        Field {
            name: "type",
            label_key: key!("form-page-type"),
            choices: &super::choices::PAGE_TYPES,
            default: "multi_line",
        },
        field("title", key!("form-title")),
        field("section", key!("form-section")),
        // Where the page goes: after the page right-clicked (else the open page) or at the end of its section;
        // its two choices in words are set when the dialog opens.
        Field {
            name: "place",
            label_key: key!("form-page-place"),
            choices: &super::choices::NONE,
            default: "after",
        },
        // The dialog starts with the current page's grid.
        Field {
            name: "grid",
            label_key: key!("form-grid"),
            choices: &super::choices::PAGE_GRIDS,
            default: "",
        },
    ],
};

/// The question before the Connection grid of the page shown changes under its items: the grid, in its words;
/// the form's foot says what will happen ([`ShellModel::form_foot`]); Change and fit or Cancel.
pub(super) static PAGE_GRID_FIT: Spec = Spec {
    action: "page.fit_grid",
    title_key: key!("page-grid-fit-title"),
    fields: &[Field {
        name: "grid",
        label_key: key!("form-grid"),
        choices: &super::choices::PAGE_GRIDS,
        default: "",
    }],
};

/// New section's form (`section.new`), its number and folio range proposed.
/// The action of Edit section…'s form.
const SECTION_EDIT: &str = "section.edit";

static SECTION_NEW: Spec = Spec {
    action: "section.new",
    title_key: key!("new-section-title"),
    fields: &[
        field("title", key!("form-title")),
        field("first", key!("form-section-first")),
        field("last", key!("form-section-last")),
        field("number", key!("form-section")),
    ],
};

static STRUCTURE_NEW: Spec = Spec {
    action: "structure.new",
    title_key: key!("new-structure-title"),
    fields: &[
        Field {
            name: "kind",
            label_key: key!("form-kind"),
            choices: &super::choices::STRUCTURE_KINDS,
            default: "location",
        },
        field("designation", key!("form-designation")),
        field("parent", key!("form-parent")),
    ],
};

static EXTERNAL_REF_NEW: Spec = Spec {
    action: "external_ref.new",
    title_key: key!("new-external-title"),
    fields: &[
        field("location", key!("form-location")),
        field("device", key!("form-device")),
        field("connection", key!("form-connection")),
        field("project", key!("form-project")),
    ],
};

/// The picker's New terminal strip form (`strip.new {dialog: true}`), its tag proposed.
static STRIP_NEW: Spec = Spec {
    action: "strip.new",
    title_key: key!("new-strip-title"),
    fields: &[field("tag", key!("form-tag"))],
};

/// The report kinds Output ▸ Reports ▸ Report block… adds: every registered kind whose blocks fill pages of their own
/// (`ew_api::output::page_report_kinds`), the parts list first, as the Output tab leads with its Parts list,
/// then ordered by their words in the UI language (the form opened on the first, which was "Bus cable list").
pub fn report_kinds() -> Vec<&'static str> {
    let mut kinds = ew_api::output::page_report_kinds();
    kinds.sort_by_cached_key(|k| (*k != "parts_list", report_kind_words(k).to_lowercase()));
    kinds
}

/// The kind of [`report_kinds`] that the command search's `query` finds best, by its words or by its search words
/// (`report-kind-<kind>-words`, comma-separated: "cable list" for the cable overview, looked for by
/// that name), with its palette rank (`crate::palette::rank`, the words weighed as an ID, the search words as an
/// alias); `None` when none aligns.
pub(super) fn report_kind_found(query: &str, uses: usize, enabled: bool) -> Option<(i64, &'static str)> {
    report_kinds()
        .into_iter()
        .filter_map(|k| {
            let search =
                strings::lookup(&format!("report-kind-{}-words", k.replace('_', "-"))).unwrap_or_default();
            let aliases = search.split(',').map(str::trim).filter(|w| !w.is_empty());
            crate::palette::rank(query, "", aliases, &report_kind_words(k), uses, enabled).map(|r| (r, k))
        })
        .max()
}

/// Output ▸ Reports ▸ Report block…, its kind offered in words.
static REPORT_NEW: Spec = Spec {
    action: "report_block.new",
    title_key: key!("report-block-new-title"),
    fields: &[
        field("kind", key!("form-report-kind")),
        field("folio", key!("form-first-folio")),
        field("title", key!("form-title")),
    ],
};

/// A report kind in the window's words (`report-kind-<kind>`).
pub(super) fn report_kind_words(kind: &str) -> String {
    strings::lookup(&format!("report-kind-{}", kind.replace('_', "-")))
        .unwrap_or(kind)
        .to_owned()
}

/// A report kind's name in every language the program ships, as a block's multi-language title
/// (`{"en-GB": "Terminal diagram", "nl-NL": "Klemmenschema"}`); its key for a kind without words.
fn report_kind_title(kind: &str) -> Value {
    let texts = strings::shipped_texts(&format!("report-kind-{}", kind.replace('_', "-")));
    if texts.is_empty() {
        return Value::String(kind.to_owned());
    }
    Value::Object(
        texts
            .into_iter()
            .map(|(tag, text)| (tag.to_owned(), Value::String(text.to_owned())))
            .collect(),
    )
}

/// The ID text of the section whose folio range contains `number`, else `None` (a page outside every section is front
/// matter).
fn section_of(shell: &ShellModel, number: u32) -> Option<String> {
    let q = shell.project.as_ref()?.session.query();
    let mut sections: Vec<(u16, u32, u32, String)> = q
        .sections()
        .iter()
        .map(|(id, s)| {
            (
                s.number,
                s.folio_range.0,
                s.folio_range.1,
                id.as_uuid().to_string(),
            )
        })
        .collect();
    sections.sort();
    sections
        .into_iter()
        .find(|(_, first, last, _)| (*first..=*last).contains(&number))
        .map(|(_, _, _, id)| id)
}

/// The folio of `text`: its number is the leading digits; any more text makes the whole text the folio's label
/// (`101a`, `101.1`), as the page order prints it.
fn parse_folio(text: &str) -> Result<(u32, Option<String>), String> {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    let Ok(number) = digits.parse::<u32>() else {
        return Err(strings::format(key!("error-folio-number"), &[text]));
    };
    Ok((number, (text.len() > digits.len()).then(|| text.to_owned())))
}

impl ShellModel {
    /// The folio and the tables of page `page` (any page of the project, not only the one on show).
    fn tables_of_page(&self, page: ew_api::output::Uuid) -> Option<(String, ew_api::output::PageTables)> {
        let folio = self.page_rows().iter().find(|r| r.id == page)?.folio.clone();
        let tables = self.project.as_ref()?.session.page_tables(&folio)?;
        Some((folio, tables))
    }

    /// The question before page `page`'s Connection grid changes to `word`, when the page holds anything a
    /// change of its grid moves (what `fields.set` refuses to change under: a symbol, a wire, a junction, a page item or
    /// a structure box) and `word` is another grid than its own; `None` otherwise, where the grid is set as typed.
    pub(super) fn grid_question(&mut self, page: ew_api::output::Uuid, word: &str) -> Option<Outcome> {
        let (folio, t) = self.tables_of_page(page)?;
        let holds = !t.reps.is_empty()
            || !t.junctions.is_empty()
            || !t.connections.is_empty()
            || !t.items.is_empty()
            || !t.structure_boxes.is_empty();
        if !holds || page_grid_word(t.grid.0, t.grid.1) == Some(word) {
            return None;
        }
        self.tools.grid_fit = Some(crate::tools::GridFit {
            page,
            folio,
            word: String::new(),
            moved: 0,
            items: t.reps.len() + t.junctions.len() + t.items.len() + t.structure_boxes.len(),
            wires: t.connections.len(),
        });
        let p = Map::from_iter([("grid".to_owned(), Value::String(word.to_owned()))]);
        let out = self.open_dialog(Dialog::Form(PAGE_GRID_FIT.dialog(&p)));
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && form.action == PAGE_GRID_FIT.action
        {
            form.ok_key = key!("page-grid-fit-ok");
        }
        self.refresh_grid_fit(word);
        Some(out)
    }

    /// `page.grid` (Page ▸ Connection grid…): the question of the page on show's grid, its own grid chosen; Change
    /// and fit (`page.fit_grid`) sets it and fits what the page holds.
    pub(crate) fn act_page_grid(&mut self) -> Outcome {
        let Some(page) = self.focused_page_id() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let Some((folio, t)) = self.tables_of_page(page) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let word = page_grid_word(t.grid.0, t.grid.1).unwrap_or_default();
        self.tools.grid_fit = Some(crate::tools::GridFit {
            page,
            folio,
            word: String::new(),
            moved: 0,
            items: t.reps.len() + t.junctions.len() + t.items.len() + t.structure_boxes.len(),
            wires: t.connections.len(),
        });
        let p = Map::from_iter([("grid".to_owned(), Value::String(word.to_owned()))]);
        let out = self.open_dialog(Dialog::Form(PAGE_GRID_FIT.dialog(&p)));
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && form.action == PAGE_GRID_FIT.action
        {
            form.ok_key = key!("page-grid-fit-ok");
        }
        if !word.is_empty() {
            self.refresh_grid_fit(word);
        }
        out
    }

    /// The items a grid change moves, from the records it changes (`diff`): its symbols, junctions and structure boxes
    /// (the question and the message after it count by this one rule).
    fn grid_fit_moved(diff: &ew_api::ModelDiff) -> usize {
        diff.changes
            .iter()
            .filter(|c| matches!(c.key.0.name(), "representation" | "junction" | "structure_box"))
            .count()
    }

    /// `page.set_grid`'s parameters for page `page` and the grid `grid`, with the page's drawing area as the router
    /// reads it (`Session::page_draft`, its ticks back in nm), so that the fit keeps a pin's wire its way to the edge
    /// (doc 06 §8 item 3).
    fn set_grid_params(&self, page: ew_api::output::Uuid, grid: Value) -> Value {
        let mut params = json!({"page": page.to_string(), "grid": grid});
        let area = self.tables_of_page(page).and_then(|(folio, t)| {
            let area = self.project.as_ref()?.session.page_draft(&folio)?.area;
            let ticks = ew_api::routing::Ticks::new(t.grid);
            Some(json!([
                [ticks.nm(area.x0), ticks.nm(area.y0)],
                [ticks.nm(area.x1), ticks.nm(area.y1)]
            ]))
        });
        if let (Some(area), Some(p)) = (area, params.as_object_mut()) {
            p.insert("area".into(), area);
        }
        params
    }

    /// Counts what moves when the question's page takes the grid `word` (`page.set_grid`'s preview), for its foot.
    pub(super) fn refresh_grid_fit(&mut self, word: &str) {
        let Some(page) = self.tools.grid_fit.as_ref().map(|g| g.page) else {
            return;
        };
        let moved = (|| {
            let cmd = AnyCommand {
                name: "page.set_grid".into(),
                params: self.set_grid_params(page, page_grid_value(word)?),
            };
            let submit = self.tool_submit();
            let preview = self.project.as_mut()?.session.preview_with(&cmd, submit).ok()?;
            Some(Self::grid_fit_moved(preview.diff()))
        })();
        self.count(|c| c.preview_with += 1);
        if let Some(g) = self.tools.grid_fit.as_mut() {
            g.word = if moved.is_some() {
                word.to_owned()
            } else {
                String::new()
            };
            g.moved = moved.unwrap_or(0);
        }
    }

    /// The question's foot: "Page 101 holds 12 items and 9 wires. On the 4 mm grid 7 of them move onto it and
    /// the wires are drawn anew; Undo takes it all back in one step."
    pub(super) fn grid_fit_foot(&self, form: &FormDialog) -> Option<String> {
        let word = form.fields.iter().find(|f| f.name == "grid")?.value.as_str();
        let g = self.tools.grid_fit.as_ref().filter(|g| g.word == word)?;
        // An empty page changes its grid with nothing to say.
        if g.items + g.wires == 0 {
            return None;
        }
        Some(strings::format(
            key!("page-grid-fit-foot"),
            &[
                &g.folio,
                &g.items.to_string(),
                &g.wires.to_string(),
                &strings::choice_text(word),
                &g.moved.to_string(),
            ],
        ))
    }

    /// `page.fit_grid {grid}` (the question's Change and fit): the question's page (else the page on show) takes
    /// the Connection grid `grid` (`page.set_grid`: its items fitted, its wires unrouted), its wires are routed anew
    /// between the same ends in the same undo step, and a message says how many items moved, counted as the question
    /// counted them.
    pub(crate) fn act_page_fit_grid(&mut self, p: &Map<String, Value>) -> Outcome {
        let word = match text_param(p, "grid") {
            Ok(w) => w,
            Err(e) => return e,
        };
        if word.is_empty() {
            return self.form_missing(&PAGE_GRID_FIT, p, "grid");
        }
        let Some(value) = page_grid_value(&word) else {
            return self.form_failed(
                PAGE_GRID_FIT.action,
                strings::format(key!("error-page-grid"), &[&word, &PAGE_GRIDS.join(", ")]),
            );
        };
        let asked = self.tools.grid_fit.take().map(|g| g.page);
        let Some((page, (folio, before))) = asked
            .or_else(|| self.focused_page_id())
            .and_then(|page| Some((page, self.tables_of_page(page)?)))
        else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let out = self.host_create("page.set_grid", self.set_grid_params(page, value));
        let out = self.form_done(PAGE_GRID_FIT.action, out);
        if out != Outcome::Done {
            return out;
        }
        let moved = self
            .project
            .as_ref()
            .and_then(|pr| pr.session.history().entries().last()?.changes().ok())
            .map_or(0, |d| Self::grid_fit_moved(&d));
        // The wires, unrouted by the change, routed anew in the same undo step.
        let ids: Vec<ew_api::output::Uuid> = before.connections.iter().map(|c| c.id).collect();
        let plan = self.project.as_ref().and_then(|pr| {
            pr.session
                .route_plan(&folio, &ew_api::routing::RouteScope::Selection(ids))
        });
        if let Some(params) = plan.as_ref().and_then(|p| p.params.clone()) {
            let rev = self.revision();
            let routed = self.run_commit(&AnyCommand {
                name: "layout.apply".into(),
                params,
            });
            if routed == Outcome::Done
                && self.revision() != rev
                && let Some(pr) = &mut self.project
            {
                pr.session.fold_last_step();
            }
        }
        let mut text = if before.reps.is_empty()
            && before.junctions.is_empty()
            && before.connections.is_empty()
            && before.items.is_empty()
            && before.structure_boxes.is_empty()
        {
            strings::format(
                key!("toast-page-grid-set"),
                &[&folio, &strings::choice_text(&word)],
            )
        } else if moved == 0 {
            strings::format(
                key!("toast-page-grid-fitted-none"),
                &[&folio, &strings::choice_text(&word)],
            )
        } else {
            strings::format(
                key!("toast-page-grid-fitted"),
                &[&folio, &strings::choice_text(&word), &moved.to_string()],
            )
        };
        // The wires the router found no way for: counted, what their line means and what to do (two wires were
        // thin and dashed after the change with no word why).
        let lost = plan.as_ref().map_or(0, |p| p.plan.unrouted.len());
        if lost > 0 {
            text.push(' ');
            text.push_str(&strings::format_count(
                crate::count_key!("toast-wires-unrouted"),
                lost,
            ));
        }
        self.notify(text, true);
        Outcome::Done
    }

    /// New page's folio and section for a kind or a place chosen in the dialog: `None` where the person typed
    /// a number of his own (the folio is no longer the one proposed). `place` `end`: the next free number after the
    /// section's last page; `after` or a kind: the proposal for the row the menu was opened on, else the open page.
    pub(super) fn page_refolio(&self, name: &str, value: &str) -> Option<(String, String)> {
        let Some(Dialog::Form(form)) = &self.dialog else {
            return None;
        };
        let field = |n: &str| form.fields.iter().find(|f| f.name == n).map(|f| f.value.clone());
        let (proposed, at) = self.tools.page_proposal.clone()?;
        if field("folio")? != proposed {
            return None;
        }
        let kind = if name == "type" {
            value.to_owned()
        } else {
            field("type").unwrap_or_default()
        };
        let place = if name == "place" {
            value.to_owned()
        } else {
            field("place").unwrap_or_default()
        };
        let (folio, section) = self.new_page_proposal(at.as_deref(), Some(&kind));
        if place == "end" {
            let end = self.section_free_folio(&section)?;
            return Some((end.to_string(), section));
        }
        Some((folio, section))
    }

    /// New page's Place choices in words: "After 103 Wiring", the page the folio of that place stands after (the
    /// folio in the field while the place is chosen, else the proposal for it), so that the choice and the line under
    /// the folio always name the same page; "First in section 1 …" where none comes before it; and "At the end of
    /// section 1 Wiring Definition".
    pub(super) fn page_place_options(&mut self) {
        let field = |s: &Self, n: &str| match &s.dialog {
            Some(Dialog::Form(form)) => form
                .fields
                .iter()
                .find(|f| f.name == n)
                .map(|f| f.value.trim().to_owned()),
            _ => None,
        };
        let at = self.tools.page_proposal.as_ref().and_then(|p| p.1.clone());
        let kind = field(self, "type").unwrap_or_default();
        // The folio in the field while the place is After and the folio reads, else the proposal for After.
        let after_folio = field(self, "folio")
            .filter(|f| field(self, "place").as_deref() == Some("after") && parse_folio(f).is_ok())
            .unwrap_or_else(|| self.new_page_proposal(at.as_deref(), Some(&kind)).0);
        let after_folio = Some(after_folio);
        let after =
            after_folio.and_then(|f| self.page_before(&f, &field(self, "section").unwrap_or_default()));
        let rows = self.page_rows();
        let section = match &self.dialog {
            Some(Dialog::Form(form)) => form
                .fields
                .iter()
                .find(|f| f.name == "section")
                .map(|f| f.value.trim().to_owned()),
            _ => None,
        };
        let section_words = section
            .as_deref()
            .and_then(|n| {
                rows.iter()
                    .find(|r| r.section.as_ref().is_some_and(|(m, _)| m.to_string() == n))
            })
            .and_then(|r| r.section.as_ref())
            .map(|(n, t)| format!("{n} {t}"))
            .or(section)
            .unwrap_or_default();
        let Some(Dialog::Form(form)) = &mut self.dialog else {
            return;
        };
        if let Some(f) = form.fields.iter_mut().find(|f| f.name == "place") {
            f.options = vec![
                (
                    "after".to_owned(),
                    after.map_or_else(
                        || strings::format(key!("form-page-place-first"), &[&section_words]),
                        |words| strings::format(key!("form-page-place-after"), &[&words]),
                    ),
                ),
                (
                    "end".to_owned(),
                    strings::format(key!("form-page-place-end"), &[&section_words]),
                ),
            ];
        }
    }

    /// The words of the page of section `section` (any section when empty) whose folio comes last before `folio`
    /// ("103 Wiring"), as New page's line under the folio names it.
    fn page_before(&self, folio: &str, section: &str) -> Option<String> {
        let (number, _) = parse_folio(folio).ok()?;
        let rows = self.page_rows();
        let in_section = rows.iter().filter(|r| {
            section.is_empty() || r.section.as_ref().is_some_and(|(n, _)| n.to_string() == section)
        });
        page_before_in(in_section, number, folio)
            .map(|r| format!("{} {}", r.folio, r.title.trim()).trim().to_owned())
    }

    /// Where the page New page makes will stand, under its folio field: "Stands after 101 Wiring", the page of
    /// the form's section whose folio comes last before the folio typed; first in its section when none does; the page
    /// that has the folio when one has it already. `None` for a folio that does not read.
    pub(super) fn new_page_place(&self, form: &FormDialog) -> Option<String> {
        let value = |name: &str| {
            form.fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.value.trim())
        };
        let typed = value("folio")?;
        let (number, _) = parse_folio(typed).ok()?;
        let section = value("section").unwrap_or_default();
        let words = |r: &PageRow| {
            if r.title.trim().is_empty() {
                r.folio.clone()
            } else {
                format!("{} {}", r.folio, r.title.trim())
            }
        };
        let rows = self.page_rows();
        let in_section = rows.iter().filter(|r| {
            section.is_empty() || r.section.as_ref().is_some_and(|(n, _)| n.to_string() == section)
        });
        if let Some(held) = PageRow::by_folio(rows, typed) {
            return Some(strings::format(key!("form-page-new-held"), &[&words(held)]));
        }
        let before = page_before_in(in_section, number, typed);
        let lead = |r: &PageRow| r.folio.split('.').next().and_then(|k| k.parse::<u32>().ok());
        // What follows it in its section, so that the line says where the number in the field puts it.
        let next = rows
            .iter()
            .filter(|r| {
                section.is_empty() || r.section.as_ref().is_some_and(|(n, _)| n.to_string() == section)
            })
            .filter_map(|r| Some((lead(r)?, r)))
            .filter(|(n, r)| *n > number || (*n == number && r.folio.as_str() > typed))
            .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.folio.cmp(&b.1.folio)));
        Some(match (before, next) {
            (Some(r), Some((_, n))) => {
                strings::format(key!("form-page-new-between"), &[&words(r), &words(n)])
            }
            (Some(r), None) => strings::format(key!("form-page-new-last"), &[&words(r)]),
            (None, _) => strings::text(key!("form-page-new-first")).to_owned(),
        })
    }
}

/// The page of `rows` whose folio comes last before the folio `typed` (lead number `number`), in folio order.
fn page_before_in<'r>(
    rows: impl Iterator<Item = &'r PageRow>,
    number: u32,
    typed: &str,
) -> Option<&'r PageRow> {
    let lead = |r: &PageRow| r.folio.split('.').next().and_then(|k| k.parse::<u32>().ok());
    rows.filter_map(|r| Some((lead(r)?, r)))
        .filter(|(n, r)| *n < number || (*n == number && r.folio.as_str() < typed))
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.folio.cmp(&b.1.folio)))
        .map(|(_, r)| r)
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // The dialog

    /// The New terminal strip form, its Tag field holding `tag` (in the window and headless alike: the
    /// action asks for it by its `dialog` flag).
    pub(super) fn open_strip_form(&mut self, tag: &str) -> Outcome {
        let mut p = Map::new();
        p.insert("tag".into(), Value::String(tag.to_owned()));
        self.open_dialog(Dialog::Form(STRIP_NEW.dialog(&p)))
    }

    /// The end of a `strip.new`: a failure stays in its form when the form is open, a success closes it.
    pub(super) fn strip_form_done(&mut self, out: Outcome) -> Outcome {
        self.form_done(STRIP_NEW.action, out)
    }

    /// The UUID text of the structure node `text` names in the aspect `aspect`: its alias or UUID (resolved by
    /// dispatch), or its code (with the aspect's own prefix sign, which is taken off) when exactly one node of the aspect
    /// has it. Several nodes with the code are refused listing them as aliases `@n` with their parent's code, ordered by
    /// their codes from the root, because no navigator prints a node's alias yet.
    pub(super) fn structure_node(&self, aspect: &str, text: &str) -> Result<String, String> {
        let Some(project) = &self.project else {
            return Err(strings::text(key!("reason-no-project")).to_owned());
        };
        let whole_uuid =
            crate::alias::find_uuid(text).is_some_and(|(at, u)| at == 0 && u.len() == text.len());
        let code = if whole_uuid {
            text.to_owned()
        } else {
            strip_sign(aspect, text)?
        };
        let q = project.session.query();
        let mut all: Vec<(String, String, Option<String>)> = Vec::new();
        for (id, node) in q.structure_nodes().iter() {
            let node_aspect = serde_json::to_value(&node.aspect).ok();
            if node_aspect.as_ref().and_then(Value::as_str) != Some(aspect) {
                continue;
            }
            all.push((
                id.as_uuid().to_string(),
                node.code.as_str().to_owned(),
                node.parent.map(|p| p.as_uuid().to_string()),
            ));
        }
        let hits: Vec<&(String, String, Option<String>)> = all
            .iter()
            .filter(|n| {
                if whole_uuid {
                    n.0.eq_ignore_ascii_case(&code)
                } else {
                    n.1 == code
                }
            })
            .collect();
        match hits.len() {
            0 => Err(strings::format(
                key!("error-no-node"),
                &[&aspect.replace('_', " "), text],
            )),
            1 => Ok(hits[0].0.clone()),
            n => {
                let mut cands: Vec<(Vec<String>, String)> = hits
                    .iter()
                    .map(|h| (node_path(&all, &h.0), h.0.clone()))
                    .collect();
                cands.sort();
                let list: Vec<String> = cands
                    .iter()
                    .map(
                        |(path, id)| match path.len().checked_sub(2).and_then(|i| path.get(i)) {
                            Some(parent) => strings::format(key!("node-candidate-under"), &[id, parent]),
                            None => strings::format(key!("node-candidate-top"), &[id]),
                        },
                    )
                    .collect();
                Err(self.alias_text(&strings::format(
                    key!("error-ambiguous-node"),
                    &[&n.to_string(), text, &list.join(", ")],
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{find, parse_signature};

    #[test]
    fn the_signatures_list_the_words_the_actions_accept() {
        let enum_of = |action: &str, name: &str| -> Vec<String> {
            let sig = parse_signature(find(action).unwrap().params).unwrap();
            let ty = &sig.iter().find(|p| p.name == name).unwrap().ty;
            ty.strip_prefix("enum(")
                .and_then(|t| t.strip_suffix(')'))
                .unwrap()
                .split('|')
                .map(str::to_owned)
                .collect()
        };
        assert_eq!(enum_of("page.new", "type"), PAGE_TYPES);
        assert_eq!(enum_of("page.new", "grid"), PAGE_GRIDS);
        assert_eq!(enum_of("structure.new", "kind"), STRUCTURE_KINDS);
        for spec in [&PAGE_NEW, &STRUCTURE_NEW, &EXTERNAL_REF_NEW] {
            let sig = parse_signature(find(spec.action).unwrap().params).unwrap();
            let names: Vec<&str> = sig.iter().map(|p| p.name.as_str()).collect();
            let mut fields: Vec<&str> = spec.fields.iter().map(|f| f.name).collect();
            // New page also takes the Pages row its menu was opened on, which is no field.
            if spec.action == "page.new" {
                fields.push("at");
            }
            assert_eq!(names, fields, "{}", spec.action);
        }
    }

    #[test]
    fn a_folio_is_its_number_and_a_label_when_it_has_more() {
        assert_eq!(parse_folio("101"), Ok((101, None)));
        assert_eq!(parse_folio("101a"), Ok((101, Some("101a".to_owned()))));
        assert_eq!(parse_folio("101.1"), Ok((101, Some("101.1".to_owned()))));
        assert!(parse_folio("A1").is_err());
        assert!(parse_folio("").is_err());
    }
}
