// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 4a (§9): the part selection dialog and the part editor.
//!
//! **Choosing** (D16, D17, D19). `part.choose {target?, use?, add?}` opens [`Dialog::PartChoose`] for the property
//! grid's devices (a part use `use` is replaced, `add` appends, a device without part uses appends), the strip editor's
//! marked terminal rows (their Part cell: every part use replaced), a Bridge part cell or an accessory Part cell (the
//! kind filter preset by D7's kinds). `part.search` changes the query and lists `Session::part_offers` again;
//! `part.pick` compiles the active row, a 1-based `index` or a part text into the one command of
//! `Session::part_pick_command` and commits it with one undo step; a library part is then recorded among the recently
//! used parts of the settings (D18). `part.favourite` toggles a library part among the favourites.
//!
//! **Editing** (D20, D21). `part.edit {part?}` opens [`Dialog::PartEdit`] with the draft of a held copy (the `part`
//! text, else the Libraries navigator's part row when it has the focus, else the property grid's device's first part
//! use, else that navigator's cursor row). `part.field {field, value}` sets and checks one field; its message shows
//! under the field. `part.save` builds `part.edit_copy` with `Session::part_edit_plan` and opens the `part` card in the
//! Preview tab, whose Apply commits it (Bulk, always previewed); a field that does not parse refuses with its message.

use ew_api::output::{NavKind, NavRole, Uuid, accessory_kind_word, accessory_part_kinds};
use ew_api::{
    ApiError, AssignMode, Fit, OfferView, PartDraft, PartDraftError, PartOffer, PartOfferQuery, PartPick,
    PartTarget,
};
use serde_json::{Map, Value};

use super::{Dialog, Outcome, ShellModel, failed, flag, string_param};
use crate::navigators::table::{accessory_index, strip_route};
use crate::properties::{CardOrigin, PartCard};
use crate::strings;

/// The result list shows this many rows at a time (WP-S2.06 D15's window).
pub const PART_ROWS: usize = 50;

/// The part selection dialog's state (D19).
#[derive(Clone, PartialEq, Debug)]
pub struct PartChooser {
    /// What the choice is for.
    pub target: PartTarget,
    /// The target in the title's words (`F55`, `the bridge -X1031 BR1`).
    pub target_text: String,
    /// The tag the fit badge names (`F55`); `None` for a strip cell.
    pub tag: Option<String>,
    /// The query; its favourites and recent parts are read from the settings at each search.
    pub query: PartOfferQuery,
    /// The offers of the query.
    pub offers: Vec<PartOffer>,
    /// The active row (0-based).
    pub cursor: usize,
}

impl PartChooser {
    /// The title (`Choose a part for F55`).
    pub fn title(&self) -> String {
        strings::format("dialog-part-choose-title", &[&self.target_text])
    }

    /// The kind filter's word: `all`, or the kinds joined by `|`.
    pub fn kind_word(&self) -> String {
        if self.query.kinds.is_empty() {
            "all".to_owned()
        } else {
            self.query.kinds.join("|")
        }
    }

    /// The category filter's word: `all`, or the levels joined by `/`.
    pub fn category_word(&self) -> String {
        if self.query.category.is_empty() {
            "all".to_owned()
        } else {
            self.query.category.join("/")
        }
    }

    /// The fit badge of an offer (`Fits F55's function`, `Same category`); none for `other`.
    pub fn badge(&self, o: &PartOffer) -> Option<String> {
        match o.fit {
            Fit::SameFnDef => Some(strings::format(
                "badge-fits-function",
                &[self.tag.as_deref().unwrap_or_default()],
            )),
            Fit::SameCategory => Some(strings::text("badge-same-category").to_owned()),
            Fit::Other => None,
        }
    }

    /// The source badge of an offer: "In this project", else the library's name.
    pub fn source(o: &PartOffer) -> String {
        if o.held {
            strings::text("part-choose-view-project").to_owned()
        } else {
            o.library.as_ref().map_or_else(String::new, |l| l.1.clone())
        }
    }
}

/// The part editor's state (D20, D21).
#[derive(Clone, PartialEq, Debug)]
pub struct PartEditor {
    /// The draft.
    pub draft: PartDraft,
    /// The part's name for the title (`5SY4204-7 (Siemens)`).
    pub name: String,
    /// The manufacturers the Manufacturer field offers.
    pub manufacturers: Vec<String>,
    /// The refusal of the last Save.
    pub error: Option<String>,
}

impl PartEditor {
    /// The title (`Edit part 5SY4204-7 (Siemens)`).
    pub fn title(&self) -> String {
        strings::format("dialog-part-edit-title", &[&self.name])
    }
}

/// The part kinds the editor's Kind field offers, in doc 07 §2.5's order (the `kind` field's values).
pub const PART_KINDS: [&str; 15] = [
    "device",
    "terminal",
    "cable",
    "wire",
    "ferrule",
    "marker",
    "duct",
    "rail",
    "busbar",
    "end_bracket",
    "enclosure",
    "mounting_plate",
    "tube",
    "accessory",
    "configured",
];

/// The word of part kind `kind` (`end_bracket` → "End bracket"; M39: the editor shows words, not raw values): the
/// value with its first letter capitalised and its underscores as spaces; an empty value stays empty.
pub fn part_kind_word(kind: &str) -> String {
    let spaced = kind.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The words of a draft message.
pub fn message_text(key: &str, args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    strings::format(key, &args)
}

/// `<library ID>/<item ID>`, the settings' form of a library item (D18).
fn item_text((library, item): (Uuid, Uuid)) -> String {
    format!("{library}/{item}")
}

fn parse_items(list: &[String]) -> Vec<(Uuid, Uuid)> {
    list.iter()
        .filter_map(|t| {
            let (l, i) = t.split_once('/')?;
            Some((l.parse().ok()?, i.parse().ok()?))
        })
        .collect()
}

/// A part text of D7's forms against an offer or a copy: `<part number> (<manufacturer>)`, with ` · <description>`,
/// or the bare part number.
fn matches_text(text: &str, number: &str, manufacturer: &str, description: &str) -> bool {
    let t = text.trim();
    let short = format!("{number} ({manufacturer})");
    t.eq_ignore_ascii_case(number)
        || t.eq_ignore_ascii_case(&short)
        || t.eq_ignore_ascii_case(&format!("{short} \u{b7} {description}"))
}

fn api_refusal(e: ApiError) -> Outcome {
    match e {
        ApiError::OtherVersionHeld { .. } => Outcome::Refused {
            reason_key: "reason-part-other-version",
        },
        ApiError::Library { .. } => Outcome::Refused {
            reason_key: "reason-part-not-installed",
        },
        ApiError::Strip(ew_api::output::StripEditError::Refused(reason_key)) => {
            Outcome::Refused { reason_key }
        }
        other => Outcome::Failed {
            message: other.to_string(),
        },
    }
}

fn draft_refusal(e: PartDraftError) -> Outcome {
    match e {
        PartDraftError::Refused(reason_key) => Outcome::Refused { reason_key },
        PartDraftError::Field(m) if m.args.is_empty() && m.key.starts_with("reason-") => {
            Outcome::Refused { reason_key: m.key }
        }
        PartDraftError::Field(m) => Outcome::Failed {
            message: message_text(m.key, &m.args),
        },
    }
}

/// What `part.choose` opens the dialog for.
struct Target {
    target: PartTarget,
    text: String,
    tag: Option<String>,
    function: Option<Uuid>,
    kinds: Vec<String>,
}

impl ShellModel {
    /// The open part dialog.
    fn part_chooser(&self) -> Option<&PartChooser> {
        match &self.dialog {
            Some(Dialog::PartChoose(c)) => Some(c),
            _ => None,
        }
    }

    /// The device tag of `d`, else its ID text.
    fn tag_of(&self, d: Uuid) -> String {
        self.project
            .as_ref()
            .and_then(|p| p.session.device_tag(d))
            .unwrap_or_else(|| d.to_string())
    }

    /// The strip editor's target of `part.choose` (`bridge_part`, `accessory_part`, or the Part cell of marked
    /// terminal rows for `parts`), from its cursor and marks; `None` when the strip editor does not offer it.
    fn strip_part_target(&self, want: Option<&str>) -> Option<Target> {
        self.strip_editor_shown().ok()?;
        let nav = self.generic(NavKind::Strips)?;
        let table = nav.table()?;
        let (ci, cursor) = nav.cursor_key().and_then(|k| nav.node_of(&k))?;
        let column = nav
            .table_mode
            .as_ref()
            .and_then(|m| m.column.clone())
            .and_then(|c| table.columns.iter().position(|x| x.key == c));
        let route = column.and_then(|c| strip_route(table, cursor, c)).map(|r| r.1);
        let strip_node = |i: u32| table.nodes.get(i as usize);
        let want = want.or(match route.as_deref() {
            Some(p) if p.starts_with("bridge_part:") => Some("bridge_part"),
            Some(p) if p.starts_with("accessory_part:") => Some("accessory_part"),
            _ => None,
        })?;
        let mut terminals: Vec<Uuid> = nav
            .marked()
            .iter()
            .filter_map(|k| nav.node_of(k))
            .filter(|(_, n)| n.role == NavRole::Terminal)
            .filter_map(|(_, n)| n.records.first().copied())
            .collect();
        if terminals.is_empty() && cursor.role == NavRole::Terminal {
            terminals.extend(cursor.records.first().copied());
        }
        let strip_label = cursor
            .parent
            .and_then(strip_node)
            .map_or_else(String::new, |s| s.label.clone());
        let _ = ci;
        match want {
            "bridge_part" => {
                if terminals.is_empty() {
                    return None;
                }
                let bridge = table
                    .columns
                    .iter()
                    .position(|c| c.key == "bridge")
                    .and_then(|i| cursor.cells.get(i).cloned())
                    .unwrap_or_default();
                Some(Target {
                    target: PartTarget::BridgePart { terminals },
                    text: strings::format("part-choose-target-bridge", &[&format!("{strip_label} {bridge}")]),
                    tag: None,
                    function: None,
                    kinds: accessory_part_kinds("bridge")
                        .iter()
                        .map(|k| (*k).to_owned())
                        .collect(),
                })
            }
            "accessory_part" => {
                if cursor.role != NavRole::Accessory {
                    return None;
                }
                let index = accessory_index(cursor)?;
                let strip = cursor.parent.and_then(strip_node)?.records.first().copied()?;
                let kind = ew_api::output::ACCESSORY_KINDS
                    .iter()
                    .find(|k| accessory_kind_word(k) == cursor.label)
                    .copied()
                    .unwrap_or("partition");
                let pos = table
                    .columns
                    .iter()
                    .position(|c| c.key == "pos")
                    .and_then(|i| cursor.cells.get(i).cloned())
                    .unwrap_or_default();
                Some(Target {
                    target: PartTarget::AccessoryPart { strip, index },
                    text: strings::format(
                        "part-choose-target-accessory",
                        &[&cursor.label.to_lowercase(), &format!("{strip_label} {pos}")],
                    ),
                    tag: None,
                    function: None,
                    kinds: accessory_part_kinds(kind)
                        .iter()
                        .map(|k| (*k).to_owned())
                        .collect(),
                })
            }
            _ => {
                if terminals.is_empty() {
                    return None;
                }
                let function = self
                    .project
                    .as_ref()
                    .and_then(|p| p.session.main_function(terminals[0]))
                    .map(|f| f.0);
                let tag = self.tag_of(terminals[0]);
                Some(Target {
                    text: if terminals.len() == 1 {
                        tag.clone()
                    } else {
                        strings::format("part-choose-target-devices", &[&terminals.len().to_string()])
                    },
                    target: PartTarget::Devices {
                        devices: terminals,
                        mode: AssignMode::ReplaceAll,
                    },
                    tag: Some(tag),
                    function,
                    kinds: Vec::new(),
                })
            }
        }
    }

    /// The property grid's devices as `part.choose`'s target (D16).
    fn grid_part_target(&self, use_: Option<usize>, add: bool) -> Result<Option<Target>, Outcome> {
        let Some(sheet) = self.props.sheet() else {
            return Ok(None);
        };
        let Some(section) = sheet
            .sections
            .iter()
            .find(|s| s.kind == "device" && !s.targets.is_empty())
        else {
            return Ok(None);
        };
        let uses = section
            .rows
            .iter()
            .find(|r| r.ty.is_part_uses())
            .map(|r| &r.value);
        let copy_at = |i: usize| -> Option<Option<Uuid>> {
            match uses {
                Some(ew_api::output::RowValue::One { json, .. }) => Some(
                    json.get(i)
                        .and_then(|u| u.get("part"))
                        .and_then(|p| p.get("copy"))
                        .and_then(Value::as_str)
                        .and_then(|s| s.parse().ok()),
                ),
                Some(ew_api::output::RowValue::Varies { .. }) => None,
                _ => Some(None),
            }
        };
        let mode = if add {
            AssignMode::Append
        } else {
            match (use_, copy_at(use_.unwrap_or(0))) {
                (_, None) => AssignMode::ReplaceAll,
                (Some(_), Some(None)) => return Err(failed("error-bad-param", &["use"])),
                (None, Some(None)) => AssignMode::Append,
                (_, Some(Some(from))) => AssignMode::Replace { from },
            }
        };
        let devices = section.targets.clone();
        let tag = self.tag_of(devices[0]);
        let function = self
            .project
            .as_ref()
            .and_then(|p| p.session.main_function(devices[0]))
            .map(|f| f.0);
        Ok(Some(Target {
            text: if devices.len() == 1 {
                tag.clone()
            } else {
                strings::format("part-choose-target-devices", &[&devices.len().to_string()])
            },
            target: PartTarget::Devices { devices, mode },
            tag: Some(tag),
            function,
            kinds: Vec::new(),
        }))
    }

    /// Lists the offers of the open dialog's query again (favourites and recent parts from the settings).
    fn part_refresh(&mut self) {
        let favourites = parse_items(&self.settings.parts_favourites);
        let recent = parse_items(&self.settings.parts_recent);
        let Some(project) = &self.project else {
            return;
        };
        if let Some(Dialog::PartChoose(c)) = &mut self.dialog {
            c.query.favourites = favourites;
            c.query.recent = recent;
            c.offers = project.session.part_offers(&c.query);
            c.cursor = c.cursor.min(c.offers.len().saturating_sub(1));
        }
    }

    /// `part.choose {target?, use?, add?}` (§9.4): opens the part dialog for its target.
    pub(super) fn act_part_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        let want = match string_param(p, "target") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let use_ = match p.get("use") {
            None => None,
            Some(v) => match v.as_u64().and_then(|i| usize::try_from(i).ok()) {
                Some(i) => Some(i),
                None => return failed("error-bad-param", &["use"]),
            },
        };
        let add = flag(p, "add");
        let target = match want {
            Some("bridge_part" | "accessory_part") => self.strip_part_target(want),
            Some("parts") | None => {
                // A strip cell first: the Bridge part or an accessory's Part cell under the cursor, then (the strip
                // editor focused) the marked terminals' Part cell; else the property grid's devices.
                let strip_cell = if want.is_none() {
                    self.strip_part_target(None)
                        .filter(|t| !matches!(t.target, PartTarget::Devices { .. }))
                } else {
                    None
                }
                .or_else(|| {
                    (self.focused_navigator() == Some(NavKind::Strips))
                        .then(|| self.strip_part_target(Some("parts")))
                        .flatten()
                });
                match strip_cell {
                    Some(t) => Some(t),
                    None => match self.grid_part_target(use_, add) {
                        Ok(Some(t)) => Some(t),
                        Ok(None) => self.strip_part_target(Some("parts")),
                        Err(e) => return e,
                    },
                }
            }
            Some(other) => {
                return failed(
                    "error-nav-value",
                    &["target", other, "parts, bridge_part, accessory_part"],
                );
            }
        };
        let Some(t) = target else {
            return Outcome::Refused {
                reason_key: "reason-part-choose-target",
            };
        };
        let chooser = PartChooser {
            target: t.target,
            target_text: t.text,
            tag: t.tag,
            query: PartOfferQuery {
                function: t.function,
                kinds: t.kinds,
                ..PartOfferQuery::default()
            },
            offers: Vec::new(),
            cursor: 0,
        };
        let out = self.open_dialog(Dialog::PartChoose(Box::new(chooser)));
        self.part_refresh();
        out
    }

    /// `part.search {text?, kind?, category?, fits?, view?, inactive?}`: changes the query of the open dialog.
    pub(super) fn act_part_search(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = |name: &str| string_param(p, name).map(|v| v.map(str::to_owned));
        let (t, kind, category, view) = match (text("text"), text("kind"), text("category"), text("view")) {
            (Ok(a), Ok(b), Ok(c), Ok(d)) => (a, b, c, d),
            (Err(e), ..) | (_, Err(e), ..) | (_, _, Err(e), _) | (.., Err(e)) => return e,
        };
        let view = match view.as_deref().map(OfferView::from_word) {
            Some(None) => return failed("error-bad-param", &["view"]),
            Some(Some(v)) => Some(v),
            None => None,
        };
        let Some(Dialog::PartChoose(c)) = &mut self.dialog else {
            return Outcome::Refused {
                reason_key: "reason-no-part-dialog",
            };
        };
        if let Some(t) = t {
            c.query.text = t;
        }
        if let Some(k) = kind {
            c.query.kinds = if k.is_empty() || k == "all" {
                Vec::new()
            } else {
                k.split('|').map(|x| x.trim().to_owned()).collect()
            };
        }
        if let Some(cat) = category {
            c.query.category = if cat.is_empty() || cat == "all" {
                Vec::new()
            } else {
                cat.split('/').map(|x| x.trim().to_owned()).collect()
            };
        }
        if p.contains_key("fits") {
            c.query.fits_only = flag(p, "fits");
        }
        if p.contains_key("inactive") {
            c.query.include_inactive = flag(p, "inactive");
        }
        if let Some(v) = view {
            c.query.view = v;
        }
        c.cursor = 0;
        self.part_refresh();
        Outcome::Done
    }

    /// The offer `index` (1-based), else the offer the part text names, else the active row.
    fn part_offer_param(&self, p: &Map<String, Value>) -> Result<PartOffer, Outcome> {
        let c = self.part_chooser().ok_or(Outcome::Refused {
            reason_key: "reason-no-part-dialog",
        })?;
        if let Some(v) = p.get("index") {
            let i = v
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .filter(|i| *i >= 1)
                .ok_or_else(|| failed("error-bad-param", &["index"]))?;
            return c.offers.get(i - 1).cloned().ok_or(Outcome::Refused {
                reason_key: "reason-part-choose-none",
            });
        }
        if let Some(text) = string_param(p, "part")? {
            let hits: Vec<&PartOffer> = c
                .offers
                .iter()
                .filter(|o| matches_text(text, &o.part_number, &o.manufacturer, &o.description))
                .collect();
            return match hits.as_slice() {
                [one] => Ok((*one).clone()),
                [] => Err(failed("error-part-unknown", &[text])),
                many => Err(failed(
                    "error-accessory-part-ambiguous",
                    &[&many
                        .iter()
                        .map(|o| format!("{} ({}) \u{b7} {}", o.part_number, o.manufacturer, o.description))
                        .collect::<Vec<_>>()
                        .join("; ")],
                )),
            };
        }
        c.offers.get(c.cursor).cloned().ok_or(Outcome::Refused {
            reason_key: "reason-part-choose-none",
        })
    }

    /// `part.pick {index?, part?}` (Enter): commits the choice (D16) and records a library part as recently used (D18).
    pub(super) fn act_part_pick(&mut self, p: &Map<String, Value>) -> Outcome {
        let offer = match self.part_offer_param(p) {
            Ok(o) => o,
            Err(e) => return e,
        };
        let Some(c) = self.part_chooser() else {
            return Outcome::Done;
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let cmd = match project.session.part_pick_command(&c.target, &offer.pick) {
            Ok(cmd) => cmd,
            Err(e) => return api_refusal(e),
        };
        let out = self.run_commit(&cmd);
        if matches!(out, Outcome::Done) {
            self.dialog = None;
            if let Some(item) = offer.item {
                self.settings.record_part_use(&item_text(item));
                self.save_settings();
            }
        }
        out
    }

    /// `part.favourite {index?}`: toggles a library part among the favourites (D18).
    pub(super) fn act_part_favourite(&mut self, p: &Map<String, Value>) -> Outcome {
        let offer = match self.part_offer_param(p) {
            Ok(o) => o,
            Err(e) => return e,
        };
        let Some(item) = offer.item else {
            return Outcome::Refused {
                reason_key: "reason-part-favourite-local",
            };
        };
        self.settings.toggle_favourite_part(&item_text(item));
        self.save_settings();
        if let (Some(Dialog::PartChoose(c)), Some(Value::Number(n))) = (&mut self.dialog, p.get("index")) {
            c.cursor = n
                .as_u64()
                .map_or(0, |i| usize::try_from(i).unwrap_or(1).saturating_sub(1));
        }
        self.part_refresh();
        Outcome::Done
    }

    /// The copy `part.edit` edits (see the module documentation).
    fn part_edit_copy(&self, p: &Map<String, Value>) -> Result<Option<Uuid>, Outcome> {
        let Some(project) = &self.project else {
            return Ok(None);
        };
        if let Some(text) = string_param(p, "part")? {
            if let Some(id) = text.trim().strip_prefix("id:") {
                return Ok(id.parse().ok());
            }
            let held = project.session.part_offers(&PartOfferQuery {
                view: OfferView::Project,
                include_inactive: true,
                ..PartOfferQuery::default()
            });
            let hits: Vec<Uuid> = held
                .iter()
                .filter(|o| matches_text(text, &o.part_number, &o.manufacturer, &o.description))
                .filter_map(|o| match o.pick {
                    PartPick::Held(c) => Some(c),
                    PartPick::Library { .. } => None,
                })
                .collect();
            return match hits.as_slice() {
                [one] => Ok(Some(*one)),
                [] => Err(failed("error-part-unknown", &[text])),
                _ => Err(failed("error-accessory-part-ambiguous", &[text])),
            };
        }
        let lib_row = || {
            self.nav_cursor(NavKind::Libraries).and_then(|k| {
                let nav = self.generic(NavKind::Libraries)?;
                let (_, n) = nav.node_of(&k)?;
                (n.role == NavRole::LibCopy)
                    .then(|| n.records.first().copied())
                    .flatten()
            })
        };
        if self.focused_navigator() == Some(NavKind::Libraries)
            && let Some(c) = lib_row()
        {
            return Ok(Some(c));
        }
        let grid = self.props.sheet().and_then(|sheet| {
            let section = sheet.sections.iter().find(|s| s.kind == "device")?;
            match &section.rows.iter().find(|r| r.ty.is_part_uses())?.value {
                ew_api::output::RowValue::One { json, .. } => json
                    .get(0)?
                    .get("part")?
                    .get("copy")?
                    .as_str()?
                    .parse::<Uuid>()
                    .ok(),
                _ => None,
            }
        });
        Ok(grid.or_else(lib_row))
    }

    /// `part.edit {part?}` (§9.4): opens the part editor with the draft of a held part copy.
    pub(super) fn act_part_edit(&mut self, p: &Map<String, Value>) -> Outcome {
        let copy = match self.part_edit_copy(p) {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Outcome::Refused {
                    reason_key: "reason-part-edit-target",
                };
            }
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Ok(draft) = project.session.part_draft(copy) else {
            return Outcome::Refused {
                reason_key: "reason-part-edit-target",
            };
        };
        let name = format!("{} ({})", draft.text("part_number"), draft.text("manufacturer"));
        let manufacturers = project.session.part_manufacturers();
        self.open_dialog(Dialog::PartEdit(Box::new(PartEditor {
            draft,
            name,
            manufacturers,
            error: None,
        })))
    }

    /// `part.field {field, value}`: sets one field of the draft; a message shows under the field (D21).
    pub(super) fn act_part_field(&mut self, p: &Map<String, Value>) -> Outcome {
        let (field, value) = match (string_param(p, "field"), string_param(p, "value")) {
            (Ok(Some(f)), Ok(Some(v))) => (f.to_owned(), v.to_owned()),
            (Ok(None), _) => return failed("error-missing-param", &["field"]),
            (_, Ok(None)) => return failed("error-missing-param", &["value"]),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::Refused {
                reason_key: "reason-no-part-editor",
            };
        };
        editor.error = None;
        match project.session.set_draft_field(&mut editor.draft, &field, &value) {
            Err(PartDraftError::Refused(_)) => failed("error-part-field", &[&field]),
            Ok(()) | Err(PartDraftError::Field(_)) => Outcome::Done,
        }
    }

    /// `part.save` (Enter): opens the `part` card of `part.edit_copy` (D20); a field that does not parse refuses.
    pub(super) fn act_part_save(&mut self) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::Refused {
                reason_key: "reason-no-part-editor",
            };
        };
        let plan = match project.session.part_edit_plan(&editor.draft) {
            Ok(plan) => plan,
            Err(e) => {
                let out = draft_refusal(e);
                editor.error = Some(match &out {
                    Outcome::Refused { reason_key } => strings::text(reason_key).to_owned(),
                    Outcome::Failed { message } => message.clone(),
                    Outcome::Done => String::new(),
                });
                return out;
            }
        };
        let text = |v: &Option<Value>| match v {
            None => String::new(),
            Some(Value::String(s)) => s.clone(),
            Some(Value::Object(m)) if m.values().all(Value::is_string) && m.len() == 1 => m
                .values()
                .next()
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            Some(other) => other.to_string(),
        };
        let card = PartCard {
            title: editor.title(),
            fields: plan
                .fields
                .iter()
                .map(|f| (f.member.clone(), text(&f.old), text(&f.new)))
                .collect(),
            devices: Vec::new(),
            removed: plan.bom_removed.clone(),
            added: plan.bom_added.clone(),
            fork: plan.fork.is_some(),
        };
        let devices: Vec<String> = plan.devices.iter().map(|d| self.tag_of(d.as_uuid())).collect();
        self.dialog = None;
        self.preview_host_card_as(
            &plan.command.name,
            plan.command.params,
            "reason-part-edit",
            CardOrigin::Part(Box::new(PartCard { devices, ..card })),
        )
    }
}

impl ShellModel {
    /// The detail line of offer `i` (0-based) of the open part dialog (D19): its source and tier, version, status and
    /// successor (FR-PRT-034), and the template's functions; `None` without the dialog or the row.
    pub fn part_offer_detail(&self, i: usize) -> Option<String> {
        let c = self.part_chooser()?;
        let o = c.offers.get(i)?;
        let project = self.project.as_ref()?;
        let source = match &o.library {
            Some((_, name, tier)) => format!("{name} ({})", format!("{tier:?}").to_lowercase()),
            None => strings::text("part-choose-view-project").to_owned(),
        };
        let head = match &o.successor {
            Some(next) => strings::format(
                "part-choose-detail-successor",
                &[&source, &o.version, &o.status, next],
            ),
            None => strings::format("part-choose-detail", &[&source, &o.version, &o.status]),
        };
        let functions = project.session.part_offer_functions(&o.pick);
        Some(if functions.is_empty() {
            head
        } else {
            strings::format("part-choose-detail-functions", &[&head, &functions.join(", ")])
        })
    }
}
