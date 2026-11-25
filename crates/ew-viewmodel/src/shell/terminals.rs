// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Terminals and strips placed on a page: New terminal strip (`strip.new`), Terminal… (`tool.place_terminal`: its
//! form, its proposal of a part, the row of terminals one click places and its spacing), the strip tag the Place tool
//! shows while it places terminals, and the words of a terminal placed.
//!
//! Not here: arming the Place tool and the picker are in `shell/editing.rs`; the Strips editor (a strip's terminals in
//! a table) in `shell/strip_editor.rs`.

use std::sync::Arc;

use ew_api::output::{CatalogueSymbol, PageTables, PlaceCatalogue, PlacedBy, Uuid};
use ew_dl::EntityRef;
use serde_json::{Map, Value, json};

use super::editing::Pick;
use super::{Outcome, ShellModel, failed, flag, string_param};
use crate::key;
use crate::strings;
use crate::tools::{CanvasEvent, Picker, PickerMode, PickerRow, PlaceTarget, PlaceTool};

/// The names of a terminal's designation in `place.pick` and `picker.choose`: `terminal_designation`, and
/// `designation`, its old name (the old name invited a device tag).
pub(super) const TERMINAL_DESIGNATION: [&str; 2] = ["terminal_designation", "designation"];

/// Terminal…'s Part "No part" (a form sends no empty field).
const NO_PART: &str = "none";

/// The most terminals one click of Terminal…'s Place lays in a row.
pub(super) const TERMINAL_ROW_MAX: u32 = 100;

/// The widest spacing of a terminal row, in grid steps.
pub(super) const TERMINAL_SPACING_MAX: u32 = 8;

/// The terminal designation of `p`: `terminal_designation`, else its old name `designation`.
pub(super) fn terminal_designation(p: &Map<String, Value>) -> Result<Option<&str>, Outcome> {
    match string_param(p, TERMINAL_DESIGNATION[0])? {
        Some(d) => Ok(Some(d)),
        None => string_param(p, TERMINAL_DESIGNATION[1]),
    }
}

/// The page's terminal kinds, Terminal…'s Kind: its terminal symbols a device places, allowed and on its grid, by their
/// names ("Terminal" before "Terminal, protective earth").
fn terminal_kinds(cat: &PlaceCatalogue) -> Vec<&CatalogueSymbol> {
    let mut kinds: Vec<&CatalogueSymbol> = cat
        .symbols
        .iter()
        .filter(|s| s.terminal && s.placed_by == PlacedBy::Device && s.allowed && s.grid_mismatch.is_none())
        .collect();
    kinds.sort_by(|a, b| a.name.cmp(&b.name));
    kinds
}

/// The function definition of the main function of the catalogue's part `i` (its first where none is marked); `None`
/// without a template.
fn part_fndef(cat: &PlaceCatalogue, i: usize) -> Option<&str> {
    let functions = &cat.parts.get(i)?.functions;
    functions
        .iter()
        .find(|f| f.main)
        .or(functions.first())
        .map(|f| f.fndef_key.as_str())
}

/// The catalogue's parts of terminal kind `kind`, by their indices: a part is of a kind when its main function is the
/// function definition the kind's symbol draws (never by its part number).
fn kind_parts<'a>(cat: &'a PlaceCatalogue, kind: &'a CatalogueSymbol) -> impl Iterator<Item = usize> + 'a {
    (0..cat.parts.len()).filter(move |&i| part_fndef(cat, i) == Some(kind.fndef_key.as_str()))
}

/// Whether the catalogue's part `i` is a terminal of two levels: its category's level word says so (`2Level`, as
/// `1Level` says one), the word Terminal…'s Levels 2 proposes a part by.
fn two_level_part(cat: &PlaceCatalogue, i: usize) -> bool {
    cat.parts[i].words.iter().any(|w| w == "2Level")
}

/// A terminal part in Terminal…'s Part list as the part dialog names it: `<part number> (<maker>) · <description>`,
/// without the parts it lacks.
fn terminal_part_words(cat: &PlaceCatalogue, i: usize) -> String {
    let part = &cat.parts[i];
    let mut words = part.part_number.clone();
    if !part.manufacturer.is_empty() {
        words.push_str(&format!(" ({})", part.manufacturer));
    }
    if !part.description.is_empty() {
        words.push_str(&format!(" \u{b7} {}", part.description));
    }
    words
}

/// Adds part `copy` to Terminal…'s Part list `options` where it is missing: a strip's held part without a function
/// template, which the proposal may name.
fn offer_part(cat: &PlaceCatalogue, options: &mut Vec<(String, String)>, copy: Option<Uuid>) {
    let Some(copy) = copy.map(|c| c.to_string()) else {
        return;
    };
    if options.iter().any(|(v, _)| *v == copy) {
        return;
    }
    if let Some(i) = cat.parts.iter().position(|r| r.copy.to_string() == copy) {
        options.push((copy, terminal_part_words(cat, i)));
    }
}

impl ShellModel {
    /// A valve terminal placed as its positions: the result line `text` with how many positions it has and how
    /// many are empty, then, while another valve terminal of the project is drawn on no fluid page (made as a station
    /// first), that one and how to draw it: the sentences, in their order.
    pub(super) fn terminal_placed_words(
        &self,
        text: String,
        device: Option<Uuid>,
        cells: &[ew_api::output::SlotCell],
    ) -> Vec<String> {
        let empty = cells.iter().filter(|c| c.holder.is_none() && !c.blank).count();
        let (n, e) = (cells.len().to_string(), empty.to_string());
        let placed = strings::without_stop(&text);
        let text = if empty == cells.len() {
            strings::format(key!("toast-placed-positions-empty"), &[placed, &n])
        } else {
            strings::format(key!("toast-placed-positions"), &[placed, &n, &e])
        };
        let undrawn: Vec<String> = self
            .project_session()
            .map(ew_api::Session::undrawn_fluid_terminals)
            .unwrap_or_default()
            .into_iter()
            .filter(|(t, _)| Some(*t) != device)
            .map(|(_, tag)| tag)
            .collect();
        let undrawn = (!undrawn.is_empty())
            .then(|| strings::format(key!("toast-terminal-undrawn"), &[&undrawn.join(", ")]));
        std::iter::once(text).chain(undrawn).collect()
    }

    /// `strip.new {letter?, tag?, dialog?}`: a host commit of `device.create` with the strip aspect. While the
    /// picker's strip chooser is open it lists the new strip, which becomes its current row.
    ///
    /// With `dialog` (the picker's New row) the New terminal strip form opens instead, its Tag field
    /// holding the tag the strip would take from a terminal on the page on show (empty without one); Create sends
    /// `strip.new {tag}`. A `tag` other than that example is frozen in the creating commit (one Undo step), as the
    /// Properties Tag row freezes a typed tag (`-X5` with the sign shown, `X5` without); the example left as it is, or
    /// no tag, keeps the tag derived. A tag another strip of the project prints already is refused in words.
    pub(crate) fn act_strip_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let letter = match string_param(p, "letter") {
            Ok(v) => v.unwrap_or("X").to_owned(),
            Err(e) => return e,
        };
        let tag = match string_param(p, "tag") {
            Ok(v) => v.unwrap_or_default().trim().to_owned(),
            Err(e) => return e,
        };
        let example = self.tag_example("strip", &letter);
        if flag(p, "dialog") {
            return self.open_strip_form(example.as_deref().unwrap_or_default());
        }
        let mut params = json!({"letter": letter, "strip": {}});
        let frozen = !tag.is_empty() && example.as_deref() != Some(tag.as_str());
        if frozen {
            // A tag another strip prints already would make a second strip of that tag: refused, naming it.
            let bare = |t: &str| t.trim_start_matches('-').to_owned();
            let taken = self.catalogue().and_then(|cat| {
                cat.strips
                    .iter()
                    .find(|st| bare(&st.tag) == bare(&tag))
                    .map(|st| st.tag.clone())
            });
            if let Some(taken) = taken {
                return self.strip_form_done(failed(key!("error-strip-tag-taken"), &[&taken]));
            }
            let Some(project) = &self.project else {
                return Outcome::refused(key!("reason-no-project"));
            };
            match project.session.parse_field_input("device", "tag.frozen", &tag) {
                Ok(parsed) if !parsed.value.is_null() => params["frozen_tag"] = parsed.value,
                // A sign alone, a line break: refused in words, the form kept open with the text as typed.
                _ => {
                    let such_as = example.clone().unwrap_or_else(|| format!("-{letter}1"));
                    return self.strip_form_done(failed(key!("error-strip-tag"), &[&tag, &such_as]));
                }
            }
        }
        let out = self.host_commit("device.create", params);
        let out = self.strip_form_done(out);
        if out != Outcome::Done {
            return out;
        }
        // The strip draws nothing until a terminal is placed, so the toast says where it shows. Its
        // tag (`-X??` until then) is not shown; the toast says when it takes one, with the tag it would take from a
        // terminal on the page on show as the example. A frozen tag is its own from the start.
        self.sync_views();
        let given = frozen
            .then(|| {
                let id = Uuid::parse_str(self.tools.last_primary.as_deref()?).ok()?;
                self.project.as_ref()?.session.device_tag(id)
            })
            .flatten();
        let text = match (given, self.tag_example("strip", &letter)) {
            (Some(tag), _) => strings::format(key!("toast-strip-new-tagged"), &[&tag]),
            (None, Some(tag)) => strings::format(key!("toast-strip-new"), &[&tag]),
            (None, None) => strings::text(key!("toast-strip-new-no-example")).to_owned(),
        };
        self.notify(text, true);
        let pending = self
            .tools
            .picker
            .as_ref()
            .filter(|p| p.mode == PickerMode::Strip)
            .and_then(|p| p.pending.clone());
        if let Some(pending) = pending {
            self.sync_views();
            if let Some(cat) = self.catalogue() {
                let folio = self.current_folio().unwrap_or_default().to_owned();
                let mut picker = Picker::strips(&folio, Arc::clone(&cat), pending);
                let new = self.tools.last_primary.clone();
                if let Some(i) = picker.rows.iter().position(
                    |r| matches!(r, PickerRow::Strip(k) if Some(cat.strips[*k].id.to_string()) == new),
                ) {
                    picker.row = i;
                }
                self.tools.picker = Some(picker);
            }
        }
        out
    }

    /// The HUD's words on a selected terminal's two sides (doc 08 §2.1: side 1 is internal and side 2 external, the
    /// strip's field side deciding): which clamp letter is the internal side, the cabinet's wiring, and which the field
    /// side, read from the first selected terminal drawn alone; `None` without one, or where a side has no lettered
    /// clamp.
    pub fn terminal_sides_hint(&self) -> Option<String> {
        let tables = Arc::clone(&self.canvas()?.page().tables);
        let selected = |id| self.tools.selection.items.contains(&EntityRef(id));
        let rep = tables
            .reps
            .iter()
            .find(|r| r.terminal && (selected(r.id) || r.device.is_some_and(selected)))?;
        let clamp = |side: &str| {
            tables
                .cps
                .iter()
                .filter(|c| c.rep == rep.id && c.side == Some(side))
                .find_map(|c| c.label.rsplit_once('/').map(|(_, l)| l.to_owned()))
        };
        let (internal, field) = (clamp("internal")?, clamp("field")?);
        Some(strings::format(key!("hud-terminal-sides"), &[&internal, &field]))
    }

    /// The strip of the terminal placed last on the page on show (the representation of the greatest ID, IDs being
    /// minted in time order); `None` on a page without a terminal.
    fn page_terminal_strip(&self) -> Option<String> {
        let terminal = self
            .canvas()?
            .page()
            .tables
            .reps
            .iter()
            .filter(|r| r.terminal)
            .max_by_key(|r| r.id)?
            .device?;
        let session = &self.project.as_ref()?.session;
        let q = session.query();
        let d = q.devices().get(terminal.to_string().parse().ok()?)?;
        Some(d.aspects.terminal.as_ref()?.strip.as_uuid().to_string())
    }

    /// `tool.place_terminal {strip?, kind?, levels?, part?, count?, from?, tag?}` (Draw ▸ Place ▸ Terminal…): in the
    /// window without parameters its small form asks which strip (one of the project's, or `new`: a new strip with the
    /// tag it will take, editable in `tag`; `new` stands first, and the strip of the page's terminal placed last is chosen, else
    /// `new`), which kind of terminal (the page's terminal symbols: feed-through, PE…), which part (No part, or a
    /// terminal part of the kinds: [`Self::terminal_part_proposal`] proposes one, and a part chosen sets its kind), how
    /// many and from which number (empty: the strip's next free one). Place then arms the Place tool with the first
    /// terminal floating; a click places the row, each terminal on the next free point the spacing right of the one
    /// before (`place.pick`'s `count`), each with the part in the step that places it. Headless, the parameters left
    /// out take the form's defaults. Spacing (`spacing`, grid steps, shown with their length) is how far apart the row's
    /// terminals stand, kept for the next row as the user setting `terminal_spacing`. Levels (`levels`, 1 or
    /// 2) 2 makes each terminal a block of two levels (doc 08 §6.3): the row lays two symbols for each, which join as
    /// [`Self::join_row_in_pairs`] says, and proposes a part of two levels.
    pub(crate) fn act_place_terminal(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "tool.place_terminal";
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let kinds = terminal_kinds(&cat);
        if kinds.is_empty() {
            return Outcome::refused(key!("reason-no-terminal-kind"));
        }
        // The strip the form adds to: the strip of the terminal placed last on this page, else a new strip (not the strip
        // used last on another page: the field terminals would join the supply page's strip).
        let page_strip: Option<String> = self.page_terminal_strip();
        if p.is_empty() && self.mode == super::Mode::Gui {
            let example = self.tag_example("strip", "X");
            let new_words = match &example {
                Some(tag) => strings::format(key!("form-terminal-strip-new"), &[tag]),
                None => strings::text(key!("form-terminal-strip-new-plain")).to_owned(),
            };
            let mut strips: Vec<(String, String)> = cat
                .strips
                .iter()
                .map(|st| (st.id.to_string(), crate::navigators::strip_label(&st.tag)))
                .collect();
            // New strip stands first, always (it moved in the list).
            strips.insert(0, ("new".to_owned(), new_words));
            let chosen = page_strip.clone().unwrap_or_else(|| "new".to_owned());
            // The Part: No part, then every terminal part of the kinds; it proposes the first strip's and kind's.
            let mut parts = vec![(
                NO_PART.to_owned(),
                strings::text(key!("draw-options-no-part")).to_owned(),
            )];
            parts.extend(
                (0..cat.parts.len())
                    .filter(|&i| {
                        kinds
                            .iter()
                            .any(|k| part_fndef(&cat, i) == Some(k.fndef_key.as_str()))
                    })
                    .map(|i| (cat.parts[i].copy.to_string(), terminal_part_words(&cat, i))),
            );
            let strip = Self::catalogue_strip(&cat, &chosen).map(|i| cat.strips[i].id);
            let proposal = self.terminal_part_proposal(&cat, strip, kinds[0], 1);
            offer_part(&cat, &mut parts, proposal);
            let proposal = proposal.map_or_else(|| NO_PART.to_owned(), |c| c.to_string());
            let field = |name, label_key, options: Vec<(String, String)>, value: String| super::FormField {
                name,
                label_key,
                choices: &super::choices::NONE,
                options,
                value,
            };
            let form = super::FormDialog {
                action: ACTION,
                title_key: key!("terminal-form-title"),
                ok_key: key!("tool-place"),
                fields: vec![
                    field("strip", key!("form-terminal-strip"), strips.clone(), chosen),
                    field(
                        "kind",
                        key!("form-terminal-kind"),
                        kinds.iter().map(|s| (s.key.clone(), s.name.clone())).collect(),
                        kinds[0].key.clone(),
                    ),
                    // A block of two levels is one terminal of one part with a number on each level (doc 08 §6.3).
                    field(
                        "levels",
                        key!("form-terminal-levels"),
                        vec![
                            (
                                "1".to_owned(),
                                strings::text(key!("terminal-levels-one")).to_owned(),
                            ),
                            (
                                "2".to_owned(),
                                strings::text(key!("terminal-levels-two")).to_owned(),
                            ),
                        ],
                        "1".to_owned(),
                    ),
                    field("part", key!("form-terminal-part"), parts, proposal),
                    field("count", key!("form-terminal-count"), Vec::new(), "1".to_owned()),
                    field("from", key!("form-terminal-from"), Vec::new(), String::new()),
                    field(
                        "spacing",
                        key!("form-terminal-spacing"),
                        self.spacing_options(),
                        self.terminal_spacing().to_string(),
                    ),
                    // The tag a new strip gets, editable (another tag is the strip's own, frozen).
                    field(
                        "tag",
                        key!("form-terminal-tag"),
                        Vec::new(),
                        example.unwrap_or_default(),
                    ),
                ],
                error: None,
                table: None,
            };
            return self.open_dialog(super::Dialog::Form(form));
        }
        let text = |name: &str| match p.get(name) {
            Some(Value::String(s)) => s.trim().to_owned(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        let mut strip = Some(text("strip"))
            .filter(|s| !s.is_empty())
            .or(page_strip)
            .unwrap_or_else(|| "new".to_owned());
        let mut kind = Some(text("kind"))
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| kinds[0].key.clone());
        let levels: u8 = match text("levels").as_str() {
            "" | "1" => 1,
            "2" => 2,
            other => {
                return self.form_failed(ACTION, strings::format(key!("error-terminal-levels"), &[other]));
            }
        };
        // The part (its copy ID or its part number; `none` or empty: none): a terminal part of one of the kinds, whose
        // kind it sets (a held part without a function template keeps the kind given). Left out, the form's proposal.
        let part = match p.get("part") {
            Some(_) if matches!(text("part").as_str(), "" | NO_PART) => None,
            Some(_) => {
                let given = text("part");
                let found = cat
                    .parts
                    .iter()
                    .position(|r| r.copy.to_string() == given || r.part_number == given)
                    .and_then(|i| {
                        let Some(fndef) = part_fndef(&cat, i) else {
                            return Some((cat.parts[i].copy, kind.clone()));
                        };
                        let k = kinds
                            .iter()
                            .find(|k| k.key == kind && k.fndef_key == fndef)
                            .or_else(|| kinds.iter().find(|k| k.fndef_key == fndef))?;
                        Some((cat.parts[i].copy, k.key.clone()))
                    });
                let Some((copy, k)) = found else {
                    return self.form_failed(ACTION, strings::format(key!("error-terminal-part"), &[&given]));
                };
                kind = k;
                Some(copy)
            }
            None => kinds.iter().find(|k| k.key == kind).and_then(|k| {
                let strip = Self::catalogue_strip(&cat, &strip).map(|i| cat.strips[i].id);
                self.terminal_part_proposal(&cat, strip, k, levels)
            }),
        };
        let Some(symbol) = cat
            .symbols
            .iter()
            .position(|s| s.key == kind && kinds.iter().any(|k| k.key == kind))
        else {
            return self.form_failed(ACTION, strings::text(key!("error-terminal-kind")).to_owned());
        };
        let count_text = Some(text("count"))
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "1".to_owned());
        // A block of two levels lays two symbols of the row.
        let most = TERMINAL_ROW_MAX / u32::from(levels);
        let count = match count_text.parse::<u32>() {
            Ok(n) if (1..=most).contains(&n) => n,
            _ => {
                let max = most.to_string();
                return self.form_failed(
                    ACTION,
                    strings::format(key!("error-terminal-count"), &[&count_text, &max]),
                );
            }
        };
        let spacing = match self.spacing_param(&text("spacing")) {
            Ok(s) => s,
            Err(message) => return self.form_failed(ACTION, message),
        };
        let from = text("from");
        // A row counts on from a number; a designation such as PE names one terminal.
        if count > 1 && !from.is_empty() && crate::tools::next_designation(&from).is_none() {
            return self.form_failed(ACTION, strings::format(key!("error-terminal-from"), &[&from]));
        }
        let mut cat = cat;
        if strip == "new" {
            let mut new = Map::new();
            let tag = text("tag");
            if !tag.is_empty() {
                new.insert("tag".to_owned(), Value::String(tag));
            }
            let made = self.act_strip_new(&new);
            if made != Outcome::Done {
                return self.form_done(ACTION, made);
            }
            strip = self.tools.last_primary.clone().unwrap_or_default();
            match self.catalogue() {
                Some(fresh) => cat = fresh,
                None => return Outcome::refused(key!("reason-no-catalogue")),
            }
        }
        let mut params = Map::new();
        params.insert("strip".into(), Value::String(strip));
        params.insert("count".into(), Value::from(count * u32::from(levels)));
        params.insert("spacing".into(), Value::from(spacing));
        self.tools.row_levels = levels;
        // Kept between sessions, a user setting.
        if self.settings.terminal_spacing != Some(spacing) {
            self.settings.terminal_spacing = Some(spacing);
            self.save_settings();
        }
        if !from.is_empty() {
            params.insert(TERMINAL_DESIGNATION[0].into(), Value::String(from));
        }
        self.tools.picker = None;
        // Every terminal of the row carries the part, in the step that places it.
        let pick = match part.and_then(|copy| cat.parts.iter().position(|r| r.copy == copy)) {
            Some(pi) => Pick::PartSymbol(pi, symbol),
            None => Pick::Symbol(symbol),
        };
        let out = self.arm(cat, pick, &params);
        self.form_done(ACTION, out)
    }

    /// The spacing of the next terminal row in grid steps: the one last chosen, in this session or an earlier one
    /// (the user setting `terminal_spacing`), else the default.
    pub(crate) fn terminal_spacing(&self) -> u32 {
        self.settings
            .terminal_spacing
            .unwrap_or(super::TERMINAL_SPACING_DEFAULT)
            .clamp(1, TERMINAL_SPACING_MAX)
    }

    /// The pitch of strip `strip` (its device ID text) on the page shown, in grid steps: the least distance between two
    /// of its terminals standing in one row there; `None` for a strip with fewer than two terminals in a row there (a
    /// strip already drawn keeps its pitch when a terminal is added to it).
    pub(super) fn strip_pitch(&self, strip: &str) -> Option<u32> {
        let page = self.canvas()?.page();
        let step = page.snapper.page_of_index(1).max(1);
        let q = self.project_session()?.query();
        let ours = |d: &Uuid| {
            d.to_string()
                .parse()
                .ok()
                .and_then(|d| q.devices().get(d))
                .and_then(|d| d.aspects.terminal.as_ref())
                .is_some_and(|t| t.strip.as_uuid().to_string() == strip)
        };
        let mut at: Vec<(i64, i64)> = page
            .tables
            .reps
            .iter()
            .filter(|r| r.terminal && r.device.as_ref().is_some_and(ours))
            .map(|r| (r.pos.1, r.pos.0))
            .collect();
        at.sort_unstable();
        at.windows(2)
            .filter(|w| w[0].0 == w[1].0 && w[1].1 > w[0].1)
            .map(|w| w[1].1 - w[0].1)
            .min()
            .and_then(|d| u32::try_from((d + step / 2) / step).ok())
            .filter(|n| (1..=TERMINAL_SPACING_MAX).contains(n))
    }

    /// The Spacing field's choices: 1 to [`TERMINAL_SPACING_MAX`] grid steps, each with its length on the page shown
    /// ("2 grid steps · 8 mm").
    fn spacing_options(&self) -> Vec<(String, String)> {
        let step_mm = self.canvas().map_or(0.0, |c| {
            let (num, den) = c.page().tables.grid;
            num as f64 / den.max(1) as f64 / 1e6
        });
        (1..=TERMINAL_SPACING_MAX)
            .map(|k| (k.to_string(), spacing_words(k, step_mm)))
            .collect()
    }

    /// The spacing `given` (grid steps; empty: the one last chosen, [`Self::terminal_spacing`]), refused in words
    /// outside 1 to [`TERMINAL_SPACING_MAX`].
    fn spacing_param(&self, given: &str) -> Result<u32, String> {
        if given.is_empty() {
            return Ok(self.terminal_spacing());
        }
        match given.parse::<u32>() {
            Ok(n @ 1..=TERMINAL_SPACING_MAX) => Ok(n),
            _ => Err(strings::format(
                key!("error-terminal-spacing"),
                &[given, &TERMINAL_SPACING_MAX.to_string()],
            )),
        }
    }

    /// `terminals.spacing {steps?}` (Spacing… on two or more selected terminals of one row): the terminals spaced
    /// `steps` grid steps apart from the leftmost, each moved as a move moves it (its wires' ends follow), in one undo
    /// step; the spacing is kept for the next row too (the user setting `terminal_spacing`). In the window
    /// without `steps` a small form asks it.
    pub(crate) fn act_terminals_spacing(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "terminals.spacing";
        let Some(c) = self.canvas() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let tables = Arc::clone(&c.page().tables);
        let mut row = self.selected_terminal_row(&tables);
        if row.len() < 2 {
            return Outcome::refused(key!("reason-terminals-spacing"));
        }
        if p.is_empty() && self.mode == super::Mode::Gui {
            let form = super::FormDialog {
                action: ACTION,
                title_key: key!("terminals-spacing-title"),
                ok_key: key!("terminals-spacing-ok"),
                fields: vec![super::FormField {
                    name: "steps",
                    label_key: key!("form-terminal-spacing"),
                    choices: &super::choices::NONE,
                    options: self.spacing_options(),
                    value: self.terminal_spacing().to_string(),
                }],
                error: None,
                table: None,
            };
            return self.open_dialog(super::Dialog::Form(form));
        }
        let given = match p.get("steps") {
            Some(Value::String(s)) => s.trim().to_owned(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        let steps = match self.spacing_param(&given) {
            Ok(s) => i64::from(s),
            Err(message) => return self.form_failed(ACTION, message),
        };
        row.sort_by_key(|(_, at)| at.0);
        let grid = crate::tools::wire::PageGridMap::new(tables.grid);
        let first = grid.index(row[0].1.0);
        let moves: Vec<(EntityRef, i64)> = row
            .iter()
            .zip(0..)
            .map(|((e, at), i)| (*e, first + i * steps - grid.index(at.0)))
            .filter(|(_, k)| *k != 0)
            .collect();
        // Those moving right from the right end, then those moving left from the left: none lands on another.
        let order = moves
            .iter()
            .rev()
            .filter(|(_, k)| *k > 0)
            .chain(moves.iter().filter(|(_, k)| *k < 0));
        let cursor = |s: &Self| s.project.as_ref().map(|p| p.session.history().cursor());
        let start = cursor(self);
        for (e, k) in order {
            let cmd = crate::tools::move_keys::move_command(&tables, &[*e], (*k, 0));
            let out = self.commit_from_host(cmd.as_ref());
            if out != Outcome::Done {
                return self.form_done(ACTION, out);
            }
        }
        self.tools.row_steps = start.zip(cursor(self)).filter(|(a, b)| b > a);
        let spacing = u32::try_from(steps).unwrap_or(super::TERMINAL_SPACING_DEFAULT);
        if self.settings.terminal_spacing != Some(spacing) {
            self.settings.terminal_spacing = Some(spacing);
            self.save_settings();
        }
        self.form_done(ACTION, Outcome::Done)
    }

    /// The part Terminal… proposes for a terminal of kind `kind` with `levels` levels on strip `strip` (`None`: a new
    /// strip), by its copy ID: among the kind's parts of that many levels ([`two_level_part`]; every part of the kind
    /// where none has), the part of the strip's last terminal of that kind where the catalogue offers it (or holds it
    /// without a function template), else the starter part, else the first part (the project's own come first);
    /// `None` where no part is of the kind.
    fn terminal_part_proposal(
        &self,
        cat: &PlaceCatalogue,
        strip: Option<Uuid>,
        kind: &CatalogueSymbol,
        levels: u8,
    ) -> Option<Uuid> {
        let all: Vec<usize> = kind_parts(cat, kind).collect();
        let fitting: Vec<usize> = all
            .iter()
            .copied()
            .filter(|&i| two_level_part(cat, i) == (levels == 2))
            .collect();
        let offered = if fitting.is_empty() { all } else { fitting };
        let parts: Vec<Uuid> = offered.iter().map(|&i| cat.parts[i].copy).collect();
        let last = strip.zip(self.project.as_ref()).and_then(|(strip, project)| {
            let session = &project.session;
            let q = session.query();
            q.devices()
                .iter()
                .filter_map(|(id, d)| {
                    let t = d
                        .aspects
                        .terminal
                        .as_ref()
                        .filter(|t| t.strip.as_uuid() == strip)?;
                    Some((
                        t.seq,
                        id.as_uuid(),
                        d.parts.first().map(|u| u.part.copy.as_uuid()),
                    ))
                })
                .filter(|(_, id, _)| {
                    session
                        .main_function(*id)
                        .is_some_and(|(_, k)| k == kind.fndef_key)
                })
                .max_by_key(|(seq, ..)| *seq)
                .and_then(|(.., part)| part)
        });
        let starter = ew_api::library::starter_parts_library();
        // A held part without a function template cannot be of another kind.
        let untyped = |c: &Uuid| cat.parts.iter().any(|r| r.copy == *c && r.functions.is_empty());
        last.filter(|c| parts.contains(c) || untyped(c))
            .or_else(|| {
                offered
                    .iter()
                    .copied()
                    .find(|&i| cat.parts[i].library.is_some_and(|(l, _)| l == starter))
                    .map(|i| cat.parts[i].copy)
            })
            .or_else(|| parts.first().copied())
    }

    /// Terminal…'s form after a choice (`changed`, the field's name): a part chosen sets the kind it draws; another
    /// strip, kind or number of levels proposes its part again, so that Kind and Part never contradict each
    /// other.
    pub(super) fn terminal_form_follow(&mut self, changed: &str) {
        let Some(cat) = self.catalogue() else { return };
        let Some(super::Dialog::Form(form)) = &self.dialog else {
            return;
        };
        let value = |name: &str| {
            form.fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.value.clone())
                .unwrap_or_default()
        };
        let (strip, kind, part) = (value("strip"), value("kind"), value("part"));
        let levels = if value("levels") == "2" { 2 } else { 1 };
        let kinds = terminal_kinds(&cat);
        let (name, new) = match changed {
            "part" if part != NO_PART => {
                let fndef = cat
                    .parts
                    .iter()
                    .position(|r| r.copy.to_string() == part)
                    .and_then(|i| part_fndef(&cat, i));
                let fits = |k: &CatalogueSymbol| Some(k.fndef_key.as_str()) == fndef;
                if kinds.iter().any(|k| k.key == kind && fits(k)) {
                    return;
                }
                let Some(k) = kinds.iter().find(|k| fits(k)) else {
                    return;
                };
                ("kind", k.key.clone())
            }
            "strip" | "kind" | "levels" => {
                let Some(k) = kinds.iter().find(|k| k.key == kind) else {
                    return;
                };
                let strip = Self::catalogue_strip(&cat, &strip).map(|i| cat.strips[i].id);
                let proposal = self.terminal_part_proposal(&cat, strip, k, levels);
                (
                    "part",
                    proposal.map_or_else(|| NO_PART.to_owned(), |c| c.to_string()),
                )
            }
            _ => return,
        };
        if let Some(super::Dialog::Form(form)) = &mut self.dialog
            && let Some(f) = form.fields.iter_mut().find(|f| f.name == name)
        {
            if name == "part" {
                offer_part(&cat, &mut f.options, new.parse().ok());
            }
            f.value = new;
        }
    }

    /// The strip tag of a terminal placement after a new revision: read from the place catalogue (cached per
    /// revision, so a dispatch that changes nothing reads nothing), because a new strip prints its tag only once its
    /// first terminal numbers it.
    pub(super) fn refresh_strip_tag(&mut self) {
        // After an Undo or a Redo the designation is the catalogue's again (the lowest free).
        let reread = std::mem::take(&mut self.tools.reread_designation);
        let strip = self
            .with_tool(|t, _| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<PlaceTool>())
                    .and_then(|p| match p.target() {
                        PlaceTarget::Terminal { strip, .. } => Some(strip.clone()),
                        _ => None,
                    })
            })
            .flatten();
        let Some(strip) = strip else { return };
        let Some(cat) = self.catalogue() else { return };
        let Some((tag, next)) = cat
            .strips
            .iter()
            .find(|st| st.id.to_string() == strip)
            .map(|st| (st.tag.clone(), st.next_designation.clone()))
        else {
            return;
        };
        let _ = self.with_tool(|t, _| {
            if let Some(p) = t.as_any_mut().and_then(|a| a.downcast_mut::<PlaceTool>()) {
                p.set_strip_tag(tag);
                // Every number below the strip's lowest free one is in use, so a counter that a commit stepped onto
                // one (after it filled a gap: 3 placed between 1, 2 and 4 counts on to 4) takes the lowest free
                // instead. A counter above it counts on, and a designation the user gave is left alone.
                let counted = p.take_counted();
                let taken = || match (p.target(), next.parse::<u64>()) {
                    (PlaceTarget::Terminal { designation, .. }, Ok(free)) => designation
                        .parse::<u64>()
                        .is_ok_and(|d| designation.bytes().all(|b| b.is_ascii_digit()) && d < free),
                    _ => false,
                };
                if reread || (counted && taken()) {
                    p.set_designation(next);
                }
            }
        });
    }

    /// After the first terminal of a Terminal… row the rest of the row, each by the Place tool's own commit at
    /// the same point (it lands on the next free point the row's spacing right of a terminal of its strip). Undo and
    /// Redo take the row whole (`row_steps`), and Select takes over: one click places the count once (four clicks with
    /// How many 4 gave sixteen terminals).
    pub(super) fn place_terminal_row(&mut self) {
        let rest = |s: &mut Self| {
            s.with_tool(|t, _| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<PlaceTool>())
                    .map(PlaceTool::take_row_rest)
            })
            .flatten()
            .unwrap_or(0)
        };
        let first = self.tools.before_commit.clone();
        let cursor = |s: &Self| s.project.as_ref().map(|p| p.session.history().cursor());
        let more = rest(self);
        if more == 0 {
            return;
        }
        // The levels of this row (Terminal… sets them before it arms the tool).
        let levels = std::mem::replace(&mut self.tools.row_levels, 1);
        let start = cursor(self).map(|c| c.saturating_sub(1));
        for _ in 0..more {
            if self.send_tool(&CanvasEvent::Activate) != Outcome::Done {
                break;
            }
            self.sync_views();
            self.sync_tool_context();
            self.refresh_strip_tag();
            // The row is counted from its first terminal alone.
            let _ = rest(self);
        }
        if levels == 2 {
            self.join_row_in_pairs(first.as_deref());
        }
        // The selection is the row's terminals and nothing else (a persona found every terminal of the page selected
        // after a row, the placements' own selection being made a dispatch later against no page before).
        self.tools.placed = false;
        let laid: Vec<EntityRef> = self
            .canvas()
            .map(|c| Arc::clone(&c.page().tables))
            .map(|now| {
                now.reps
                    .iter()
                    .filter(|r| {
                        !first
                            .as_ref()
                            .is_some_and(|b| b.reps.iter().any(|o| o.id == r.id))
                    })
                    .map(|r| EntityRef(r.id))
                    .collect()
            })
            .unwrap_or_default();
        if !laid.is_empty()
            && self
                .tools
                .selection
                .apply(&crate::tools::SelectionChange::Replace(laid))
        {
            self.announce_selection();
        }
        self.tools.before_commit = first;
        self.tools.row_steps = start.zip(cursor(self));
        if let Some(select) = crate::tools::tool_named("select") {
            self.tools.tool = select;
            self.tool_epoch = self.tool_epoch.wrapping_add(1);
            self.announce_tool();
        }
    }

    /// A Terminal… row of two levels (doc 08 §6.3): the terminals the row laid (those not on the page in `before`), in
    /// their order on the strip, join in pairs, the second of each the first's next level (`rep.join_device`), so each
    /// pair is one terminal whose levels print the two numbers (1 and 2, 3 and 4 …). The second ones first give up
    /// their part, in one `fields.set`: the block keeps the first's, and a part of two levels on the second would
    /// refuse the join.
    fn join_row_in_pairs(&mut self, before: Option<&PageTables>) {
        let Some(tables) = self.canvas().map(|c| Arc::clone(&c.page().tables)) else {
            return;
        };
        let Some(project) = &self.project else { return };
        let q = project.session.query();
        // (seq, device, representation, whether it has a part) of each terminal the row laid.
        let mut laid: Vec<(u32, Uuid, Uuid, bool)> = tables
            .reps
            .iter()
            .filter(|r| r.terminal && !before.is_some_and(|b| b.reps.iter().any(|o| o.id == r.id)))
            .filter_map(|r| {
                let device = r.device?;
                let d = q.devices().get(device.to_string().parse().ok()?)?;
                Some((
                    d.aspects.terminal.as_ref()?.seq,
                    device,
                    r.id,
                    !d.parts.is_empty(),
                ))
            })
            .collect();
        drop(q);
        laid.sort_unstable();
        let pairs: Vec<(Uuid, Uuid)> = laid.as_chunks::<2>().0.iter().map(|p| (p[0].1, p[1].2)).collect();
        let parted: Vec<String> = laid
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|p| p[1].3)
            .map(|p| p[1].1.to_string())
            .collect();
        if !parted.is_empty() {
            let params = json!({"kind": "device", "targets": parted, "path": "parts", "value": []});
            if self.run_logged_or_commit("fields.set", &params, None).0 != Outcome::Done {
                return;
            }
        }
        for (block, rep) in pairs {
            let params = json!({"rep": rep.to_string(), "device": block.to_string()});
            if self.run_logged_or_commit("rep.join_device", &params, None).0 != Outcome::Done {
                break;
            }
        }
        self.sync_views();
    }
}

/// A spacing of `k` grid steps of `step_mm` millimetres in words, the length with the window's decimal sign as a
/// text style's height is ("2 grid steps · 8.0 mm", "8,0 mm" in Dutch).
fn spacing_words(k: u32, step_mm: f64) -> String {
    let mm = strings::decimal(&format!("{:.1}", step_mm * f64::from(k)));
    if k == 1 {
        strings::format(key!("terminal-spacing-one"), &[&mm])
    } else {
        strings::format(key!("terminal-spacing-steps"), &[&k.to_string(), &mm])
    }
}

#[cfg(test)]
mod tests {
    use super::spacing_words;
    use crate::strings;

    /// The Spacing field's lengths take the window's decimal sign.
    #[test]
    fn a_spacing_reads_with_the_windows_decimal_sign() {
        assert_eq!(spacing_words(2, 4.0), "2 grid steps · 8.0 mm");
        assert!(strings::set_locale("nl"));
        let dutch = (spacing_words(1, 4.0), spacing_words(2, 4.0));
        assert!(strings::set_locale("en"));
        assert_eq!(
            dutch,
            (
                "1 rasterstap · 4,0 mm".to_owned(),
                "2 rasterstappen · 8,0 mm".to_owned()
            )
        );
    }
}
