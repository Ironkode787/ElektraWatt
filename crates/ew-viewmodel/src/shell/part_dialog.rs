// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part selection dialog and the part editor.
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

use ew_api::output::{NavKind, NavRole, Uuid, accessory_part_kinds};
use ew_api::{
    ApiError, AssignMode, Fit, OfferView, PartDraft, PartDraftError, PartOffer, PartOfferQuery, PartPick,
    PartTarget,
};
use serde_json::{Map, Value};

use super::{Dialog, Outcome, ShellModel, failed, flag, string_param};
use crate::key;
use crate::navigators::table::{accessory_index, strip_route};
use crate::properties::{CardOrigin, PartCard};
use crate::strings;
use crate::strings::Key;

/// The result list shows this many rows at a time.
pub const PART_ROWS: usize = 50;

/// The part kinds of the layout tools: a device's part dialog offers one only where panel layout attaches it.
const LAYOUT_KIND_WORDS: [&str; 5] = ["enclosure", "mounting_plate", "rail", "duct", "end_bracket"];

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
    /// Why each offer does not fit a cable, in the offers' order; `None` where it fits (rc.3).
    pub misfits: Vec<Option<String>>,
    /// The active row (0-based).
    pub cursor: usize,
}

impl PartChooser {
    /// The title (`Choose a part for F55`).
    pub fn title(&self) -> String {
        strings::format(key!("dialog-part-choose-title"), &[&self.target_text])
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
                key!("badge-fits-function"),
                &[self.tag.as_deref().unwrap_or_default()],
            )),
            Fit::SameCategory => Some(strings::text(key!("badge-same-category")).to_owned()),
            Fit::Other => None,
        }
    }

    /// The source badge of an offer: "In this project", else the library's name.
    pub fn source(o: &PartOffer) -> String {
        if o.held {
            strings::text(key!("part-choose-view-project")).to_owned()
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
    /// The part dialog that New part… was chosen in: Save opens it again on the new part.
    pub back: Option<Box<PartChooser>>,
    /// New cable type… (rc.3): the small form of a new cable part (its maker, type and number, then the cable section).
    pub cable_type: bool,
    /// The Cable form New cable type… was chosen in: Save opens it again with the new type chosen.
    pub cable_form: Option<Box<super::FormDialog>>,
    /// New bus cable… or New plug… of a bus line's Cable form (rc.5): the small form of a new bus part.
    pub bus_part: Option<BusPart>,
}

/// What New bus cable… and New plug… make (rc.5): a cable or a plug for a bus, and the form field it returns to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BusPart {
    /// A plug; else a bus cable.
    pub plug: bool,
    /// The Cable form's field the new part is chosen in (`part`, `plug_a`, `plug_b`).
    pub field: &'static str,
    /// A plug's cable outlet is angled; else straight.
    pub angled: bool,
    /// A plug with a programming socket.
    pub socket: bool,
}

impl PartEditor {
    /// The title (`Edit part 5SY4204-7 (Siemens)`, `New part`, `New cable type`, `New bus cable`, `New bus plug`).
    pub fn title(&self) -> String {
        match self.bus_part {
            Some(b) if b.plug => return strings::text(key!("dialog-bus-plug-new-title")).to_owned(),
            Some(_) => return strings::text(key!("dialog-bus-cable-new-title")).to_owned(),
            None => {}
        }
        if self.cable_type {
            return strings::text(key!("dialog-cable-type-new-title")).to_owned();
        }
        if self.draft.new {
            return strings::text(key!("dialog-part-new-title")).to_owned();
        }
        strings::format(key!("dialog-part-edit-title"), &[&self.name])
    }
}

/// A new plug's description in words (rc.5): `PROFIBUS DP plug, angled cable outlet, programming socket, switchable
/// terminating resistor`, from its bus, its cable outlet, its socket and its terminating resistor.
pub(crate) fn plug_description(draft: &ew_api::PartDraft, plug: BusPart) -> String {
    let bus = strings::field_choice_text("function", "bus_port.bus", draft.text("bus.0.bus"));
    let mut words = vec![strings::format(key!("part-plug-description"), &[&bus])];
    words.push(
        strings::text(if plug.angled {
            key!("part-plug-angled")
        } else {
            key!("part-plug-straight")
        })
        .to_owned(),
    );
    if plug.socket {
        words.push(strings::text(key!("part-plug-with-socket")).to_owned());
    }
    if !draft.text("bus.0.termination").is_empty() {
        words.push(strings::text(key!("part-plug-with-resistor")).to_owned());
    }
    words.join(", ")
}

/// The words of a draft field (`part_number` is "Part number", `bus.1.ports` "Ports", `tech:rated_current` the key).
pub fn field_label(field: &str) -> String {
    if let Some(key) = field.strip_prefix("tech:") {
        return key.to_owned();
    }
    if let Some(member) = field.strip_prefix("plc.") {
        return strings::text(Key::run_time(&format!("part-plc-{member}"))).to_owned();
    }
    if let Some(member) = field.strip_prefix("cable.") {
        let member = member.split('.').next().unwrap_or_default();
        return strings::text(Key::run_time(&format!("part-cable-{member}"))).to_owned();
    }
    if let Some((_, member)) = field.strip_prefix("bus.").and_then(|r| r.split_once('.')) {
        return strings::text(Key::run_time(&format!("part-bus-{member}"))).to_owned();
    }
    strings::text(Key::run_time(&format!("part-field-{field}"))).to_owned()
}

/// The part kinds the editor's Kind field offers ([`ew_api::PART_KINDS`], tied to the schema's part kinds by a test).
pub use ew_api::PART_KINDS;

/// The word of part kind `kind` (`end_bracket` is "End bracket"; M39: the editor shows words, not raw values),
/// through [`strings::choice_text`].
pub fn part_kind_word(kind: &str) -> String {
    strings::choice_text(kind)
}

/// The words of a draft message.
pub fn message_text(key: Key<'_>, args: &[String]) -> String {
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
        ApiError::OtherVersionHeld { .. } => Outcome::refused(key!("reason-part-other-version")),
        ApiError::Library { .. } => Outcome::refused(key!("reason-part-not-installed")),
        ApiError::Strip(ew_api::output::StripEditError::Refused(reason_key)) => Outcome::Refused {
            reason_key: Key::run_time(reason_key),
        },
        other => Outcome::Failed {
            message: other.to_string(),
        },
    }
}

fn draft_refusal(e: PartDraftError) -> Outcome {
    match e {
        PartDraftError::Refused(reason_key) => Outcome::Refused {
            reason_key: Key::run_time(reason_key),
        },
        PartDraftError::Field(m) if m.args.is_empty() && m.key.starts_with("reason-") => Outcome::Refused {
            reason_key: Key::run_time(m.key),
        },
        PartDraftError::Field(m) => Outcome::Failed {
            message: message_text(Key::run_time(m.key), &m.args),
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
            .map_or_else(String::new, |s| crate::navigators::strip_label(&s.label));
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
                    text: strings::format(
                        key!("part-choose-target-bridge"),
                        &[&format!("{strip_label} {bridge}")],
                    ),
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
                    .find(|k| strings::accessory_text(k) == cursor.label)
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
                        key!("part-choose-target-accessory"),
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
                        strings::format(
                            key!("part-choose-target-devices"),
                            &[&terminals.len().to_string()],
                        )
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
                (Some(_), Some(None)) => return Err(failed(key!("error-bad-param"), &["use"])),
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
        // A cable is offered cable parts only (rc.3: every part was offered).
        let cables = self
            .project
            .as_ref()
            .map(|p| p.session.cables())
            .unwrap_or_default();
        let kinds = if devices.iter().all(|d| cables.iter().any(|(c, _, _)| c == d)) {
            vec!["cable".to_owned()]
        } else {
            Vec::new()
        };
        Ok(Some(Target {
            text: if devices.len() == 1 {
                tag.clone()
            } else {
                strings::format(key!("part-choose-target-devices"), &[&devices.len().to_string()])
            },
            target: PartTarget::Devices { devices, mode },
            tag: Some(tag),
            function,
            kinds,
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
            // A layout kind is offered to a device only where panel layout
            // attaches it (`chooser_layout_kinds`: an end bracket to a strip, an enclosure or a plate to a device
            // without a main function, never a rail or a duct).
            if let PartTarget::Devices { devices, .. } = &c.target {
                let allowed = project.session.chooser_layout_kinds(devices);
                c.offers
                    .retain(|o| !LAYOUT_KIND_WORDS.contains(&o.kind.as_str()) || allowed.contains(&o.kind));
            }
            // A cable's parts that do not fit it come after those that do, each with the reason (rc.3: greyed, not
            // hidden).
            let cable = match &c.target {
                PartTarget::Devices { devices, .. } if devices.len() == 1 => devices
                    .first()
                    .copied()
                    .filter(|d| project.session.cables().iter().any(|(c, _, _)| c == d)),
                _ => None,
            };
            let mut rows: Vec<(Option<String>, PartOffer)> = std::mem::take(&mut c.offers)
                .into_iter()
                .map(|o| {
                    let why = cable
                        .and_then(|cable| project.session.cable_part_misfit(cable, &o.pick))
                        .map(|r| message_text(Key::run_time(r.key), &r.args));
                    (why, o)
                })
                .collect();
            rows.sort_by_key(|(why, _)| why.is_some());
            (c.misfits, c.offers) = rows.into_iter().unzip();
            c.cursor = c.cursor.min(c.offers.len().saturating_sub(1));
        }
    }

    /// The terminals of strip `strip` without a part, in their order on the strip.
    pub(crate) fn strip_terminals_without_part(&self, strip: Uuid) -> Vec<Uuid> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let mut terminals: Vec<(u32, Uuid)> = q
            .devices()
            .iter()
            .filter(|(_, d)| d.parts.is_empty())
            .filter_map(|(id, d)| {
                let t = d
                    .aspects
                    .terminal
                    .as_ref()
                    .filter(|t| t.strip.as_uuid() == strip)?;
                Some((t.seq, id.as_uuid()))
            })
            .collect();
        terminals.sort_unstable();
        terminals.into_iter().map(|(_, id)| id).collect()
    }

    /// The terminals of strip `strip` (its ID) without a part as `part.choose`'s target (a strip's row, FT-1695).
    fn strip_terminals_target(&self, strip: &str) -> Result<Target, Outcome> {
        let terminals = strip
            .parse()
            .map(|s| self.strip_terminals_without_part(s))
            .map_err(|_| failed(key!("error-bad-param"), &["strip"]))?;
        let Some(&first) = terminals.first() else {
            return Err(Outcome::refused(key!("reason-strip-all-parts")));
        };
        let tag = self.tag_of(first);
        Ok(Target {
            text: if terminals.len() == 1 {
                tag.clone()
            } else {
                strings::format(
                    key!("part-choose-target-devices"),
                    &[&terminals.len().to_string()],
                )
            },
            function: self
                .project
                .as_ref()
                .and_then(|p| p.session.main_function(first))
                .map(|f| f.0),
            target: PartTarget::Devices {
                devices: terminals,
                mode: AssignMode::ReplaceAll,
            },
            tag: Some(tag),
            kinds: Vec::new(),
        })
    }

    /// `part.choose {target?, use?, add?, strip?}` (§9.4): opens the part dialog for its target; `strip` (its ID), for
    /// the strip's terminals without a part.
    pub(crate) fn act_part_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        // A cable by its ID (Choose a cable type… on its finding, rc.3): its cable parts, those that fit first.
        if let Some(cable) = or_return!(string_param(p, "cable")) {
            let Some(cable) = cable.parse::<Uuid>().ok().filter(|c| {
                self.project_session()
                    .is_some_and(|s| s.cables().iter().any(|(x, _, _)| x == c))
            }) else {
                return failed(key!("error-bad-param"), &["cable"]);
            };
            let tag = self.tag_of(cable);
            return self.open_part_chooser(Target {
                text: tag.clone(),
                target: PartTarget::Devices {
                    devices: vec![cable],
                    mode: AssignMode::ReplaceAll,
                },
                tag: Some(tag),
                function: None,
                kinds: vec!["cable".to_owned()],
            });
        }
        if let Some(strip) = or_return!(string_param(p, "strip")) {
            return match self.strip_terminals_target(strip) {
                Ok(t) => self.open_part_chooser(t),
                Err(e) => e,
            };
        }
        let want = or_return!(string_param(p, "target"));
        let use_ = match p.get("use") {
            None => None,
            Some(v) => match v.as_u64().and_then(|i| usize::try_from(i).ok()) {
                Some(i) => Some(i),
                None => return failed(key!("error-bad-param"), &["use"]),
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
                    key!("error-nav-value"),
                    &["target", other, "parts, bridge_part, accessory_part"],
                );
            }
        };
        let Some(t) = target else {
            return Outcome::refused(key!("reason-part-choose-target"));
        };
        self.open_part_chooser(t)
    }

    /// Opens the part dialog for `t` with its offers.
    fn open_part_chooser(&mut self, t: Target) -> Outcome {
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
            misfits: Vec::new(),
            cursor: 0,
        };
        let out = self.open_dialog(Dialog::PartChoose(Box::new(chooser)));
        self.part_refresh();
        out
    }

    /// `part.search {text?, kind?, category?, fits?, view?, inactive?}`: changes the query of the open dialog.
    pub(crate) fn act_part_search(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = |name: &str| string_param(p, name).map(|v| v.map(str::to_owned));
        let (t, kind, category, view) = match (text("text"), text("kind"), text("category"), text("view")) {
            (Ok(a), Ok(b), Ok(c), Ok(d)) => (a, b, c, d),
            (Err(e), ..) | (_, Err(e), ..) | (_, _, Err(e), _) | (.., Err(e)) => return e,
        };
        let view = match view.as_deref().map(OfferView::from_word) {
            Some(None) => return failed(key!("error-bad-param"), &["view"]),
            Some(Some(v)) => Some(v),
            None => None,
        };
        let Some(Dialog::PartChoose(c)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-dialog"));
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
        let c = self
            .part_chooser()
            .ok_or(Outcome::refused(key!("reason-no-part-dialog")))?;
        if let Some(v) = p.get("index") {
            let i = v
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .filter(|i| *i >= 1)
                .ok_or_else(|| failed(key!("error-bad-param"), &["index"]))?;
            return c
                .offers
                .get(i - 1)
                .cloned()
                .ok_or(Outcome::refused(key!("reason-part-choose-none")));
        }
        if let Some(text) = string_param(p, "part")? {
            let hits: Vec<&PartOffer> = c
                .offers
                .iter()
                .filter(|o| matches_text(text, &o.part_number, &o.manufacturer, &o.description))
                .collect();
            return match hits.as_slice() {
                [one] => Ok((*one).clone()),
                [] => Err(failed(key!("error-part-unknown"), &[text])),
                many => Err(failed(
                    key!("error-accessory-part-ambiguous"),
                    &[&many
                        .iter()
                        .map(|o| format!("{} ({}) \u{b7} {}", o.part_number, o.manufacturer, o.description))
                        .collect::<Vec<_>>()
                        .join("; ")],
                )),
            };
        }
        c.offers
            .get(c.cursor)
            .cloned()
            .ok_or(Outcome::refused(key!("reason-part-choose-none")))
    }

    /// `part.pick {index?, part?}` (Enter): commits the choice (D16) and records a library part as recently used (D18).
    pub(crate) fn act_part_pick(&mut self, p: &Map<String, Value>) -> Outcome {
        let offer = or_return!(self.part_offer_param(p));
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
    pub(crate) fn act_part_favourite(&mut self, p: &Map<String, Value>) -> Outcome {
        let offer = or_return!(self.part_offer_param(p));
        let Some(item) = offer.item else {
            return Outcome::refused(key!("reason-part-favourite-local"));
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
    pub(super) fn part_edit_copy(&self, p: &Map<String, Value>) -> Result<Option<Uuid>, Outcome> {
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
                [] => Err(failed(key!("error-part-unknown"), &[text])),
                _ => Err(failed(key!("error-accessory-part-ambiguous"), &[text])),
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
    pub(crate) fn act_part_edit(&mut self, p: &Map<String, Value>) -> Outcome {
        let copy = match self.part_edit_copy(p) {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Outcome::refused(key!("reason-part-edit-target"));
            }
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Ok(draft) = project.session.part_draft(copy) else {
            return Outcome::refused(key!("reason-part-edit-target"));
        };
        let name = format!("{} ({})", draft.text("part_number"), draft.text("manufacturer"));
        let manufacturers = project.session.part_manufacturers();
        self.open_dialog(Dialog::PartEdit(Box::new(PartEditor {
            draft,
            name,
            manufacturers,
            error: None,
            back: None,
            cable_type: false,
            cable_form: None,
            bus_part: None,
        })))
    }

    /// `part.new {symbol?}` (Edit ▸ New part…, and New part… in the part dialog and the device browser): opens the part
    /// editor on a new part's draft (FT-1766); with `symbol` (the symbol editor's saved symbol, its library address:
    /// Make a part for this symbol…), a part drawn with that symbol. From the part dialog, Save returns to it on the new
    /// part. With `plc` (the rack dialog's New part…, rc.2): the draft starts as what the dialog was asked for, `cpu`,
    /// `remote`, `valve_terminal` or `compact` a station part with that kind's defaults ([`plc_preset`]), `module` a
    /// module part; Save returns to the rack dialog with the new part chosen.
    pub(crate) fn act_part_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let symbol = or_return!(string_param(p, "symbol")).map(str::to_owned);
        let plc = match or_return!(string_param(p, "plc")) {
            Some(w) if PLC_PRESETS.contains(&w) => Some(w.to_owned()),
            Some(_) => return failed(key!("error-bad-param"), &["plc"]),
            None => None,
        };
        let from = match or_return!(string_param(p, "from")).map(str::parse::<Uuid>) {
            None => None,
            Some(Ok(c)) => Some(c),
            Some(Err(_)) => return failed(key!("error-bad-param"), &["from"]),
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let made = match (&symbol, from) {
            (Some(address), _) => project.session.new_part_draft_for_symbol(address),
            (None, Some(copy)) => project.session.new_part_draft_from(copy),
            (None, None) => project.session.new_part_draft(),
        };
        let mut draft = match made {
            Ok(d) => d,
            Err(e) => return draft_refusal(e),
        };
        if let Some(word) = &plc {
            plc_preset(&project.session, &mut draft, word);
        }
        // New cable type… (rc.3): a cable part, sold by the metre, in the small form.
        let cable_type = flag(p, "cable");
        if cable_type {
            for (field, value) in [("kind", "cable"), ("unit", "m")] {
                let _ = project.session.set_draft_field(&mut draft, field, value);
            }
        }
        // New bus cable… and New plug… of a bus line's Cable form (rc.5): a cable made for the bus, its cores as the bus
        // has them, or a plug with a passive interface of the bus; Save returns to the form's field `field`.
        let bus_part = match or_return!(string_param(p, "bus")) {
            None => None,
            Some(bus) => {
                let plug = flag(p, "plug");
                let field = match or_return!(string_param(p, "field")) {
                    Some("plug_a") => "plug_a",
                    Some("plug_b") => "plug_b",
                    _ if plug => "plug_a",
                    _ => "part",
                };
                let set = |draft: &mut ew_api::PartDraft, f: &str, v: &str| {
                    let _ = project.session.set_draft_field(draft, f, v);
                };
                if plug {
                    set(&mut draft, "kind", "accessory");
                    draft.add_bus_row();
                    for (f, v) in [("bus.0.bus", bus), ("bus.0.role", "passive"), ("bus.0.termination", "")] {
                        set(&mut draft, f, v);
                    }
                } else {
                    for (f, v) in [("kind", "cable"), ("unit", "m"), ("cable.bus", bus)] {
                        set(&mut draft, f, v);
                    }
                }
                let made = BusPart {
                    plug,
                    field,
                    angled: false,
                    socket: false,
                };
                if plug {
                    let words = plug_description(&draft, made);
                    set(&mut draft, "description", &words);
                }
                Some(made)
            }
        };
        let cable_type = cable_type || bus_part.is_some_and(|b| !b.plug);
        let manufacturers = project.session.part_manufacturers();
        // The device browser gives way to the editor; it lists the new part when it opens again. From the symbol
        // editor, its view is left with its draft kept (as Close editor does): the part is the project's.
        self.tools.picker = None;
        if symbol.is_some()
            && let Some(editor) = &mut self.symbol_editor
        {
            editor.shown = false;
        }
        // From the Cable form (its New cable type…), Save returns to it with the new type chosen.
        let (back, cable_form) = match self.dialog.take() {
            Some(Dialog::PartChoose(c)) => (Some(c), None),
            Some(Dialog::Form(f))
                if (cable_type && super::cables::chooses_cable_type(&f))
                    || (bus_part.is_some() && f.action == super::cables::BUS_CABLE) =>
            {
                (None, Some(Box::new(f)))
            }
            other => {
                self.dialog = other;
                (None, None)
            }
        };
        let out = self.open_dialog(Dialog::PartEdit(Box::new(PartEditor {
            draft,
            name: String::new(),
            manufacturers,
            error: None,
            back: None,
            cable_type,
            cable_form: None,
            bus_part,
        })));
        if let Some(Dialog::PartEdit(e)) = &mut self.dialog {
            e.back = back;
            e.cable_form = cable_form;
        }
        out
    }

    /// `part.bus_row {remove?}`: adds a bus interface row to the new part, or removes row `remove` (1-based).
    pub(crate) fn act_part_bus_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let remove = match p.get("remove") {
            None => None,
            Some(v) => match v
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .filter(|i| *i >= 1)
            {
                Some(i) => Some(i - 1),
                None => return failed(key!("error-bad-param"), &["remove"]),
            },
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        if !editor.draft.new && !editor.draft.own {
            return failed(key!("error-part-field"), &["bus"]);
        }
        editor.error = None;
        match remove {
            None => editor.draft.add_bus_row(),
            Some(i) => {
                if !editor.draft.remove_bus_row(i) {
                    return failed(key!("error-bad-param"), &["remove"]);
                }
            }
        }
        Outcome::Done
    }

    /// `part.field {field, value}`: sets one field of the draft; a message shows under the field (D21).
    pub(crate) fn act_part_field(&mut self, p: &Map<String, Value>) -> Outcome {
        let (field, value) = match (string_param(p, "field"), string_param(p, "value")) {
            (Ok(Some(f)), Ok(Some(v))) => (f.to_owned(), v.to_owned()),
            (Ok(None), _) => return failed(key!("error-missing-param"), &["field"]),
            (_, Ok(None)) => return failed(key!("error-missing-param"), &["value"]),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        editor.error = None;
        if field == "to_library" && editor.draft.new {
            editor.draft.to_library = value == "true";
            return Outcome::Done;
        }
        // A new plug's cable outlet and programming socket (rc.5) are words of its description, which follows them, its
        // bus and its terminating resistor.
        if let Some(mut plug) = editor.bus_part.filter(|b| b.plug) {
            match field.as_str() {
                "plug.angled" => plug.angled = value == "true",
                "plug.socket" => plug.socket = value == "true",
                _ => {}
            }
            editor.bus_part = Some(plug);
            let out = if field.starts_with("plug.") {
                Outcome::Done
            } else {
                match project.session.set_draft_field(&mut editor.draft, &field, &value) {
                    Err(PartDraftError::Refused(_)) => return failed(key!("error-part-field"), &[&field]),
                    Ok(()) | Err(PartDraftError::Field(_)) => Outcome::Done,
                }
            };
            if field.starts_with("plug.") || field.starts_with("bus.") {
                let words = plug_description(&editor.draft, plug);
                let _ = project.session.set_draft_field(&mut editor.draft, "description", &words);
            }
            return out;
        }
        match project.session.set_draft_field(&mut editor.draft, &field, &value) {
            Err(PartDraftError::Refused(_)) => failed(key!("error-part-field"), &[&field]),
            Ok(()) | Err(PartDraftError::Field(_)) => Outcome::Done,
        }
    }

    /// `part.save {another?}` (Enter): opens the `part` card of `part.edit_copy` (D20); a field that does not parse
    /// refuses. A new part is saved at once; with `another` (a new cable type's Save and add another size) the form
    /// stays open on the next size of the series.
    pub(crate) fn act_part_save(&mut self, p: &Map<String, Value>) -> Outcome {
        if matches!(&self.dialog, Some(Dialog::PartEdit(e)) if e.draft.new) {
            return self.part_save_new(flag(p, "another"));
        }
        if matches!(&self.dialog, Some(Dialog::PartEdit(e)) if e.draft.own && e.draft.fork) {
            return self.part_save_own_library();
        }
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        let plan = match project.session.part_edit_plan(&editor.draft) {
            Ok(plan) => plan,
            Err(e) => {
                let out = draft_refusal(e);
                editor.error = Some(match &out {
                    Outcome::Refused { reason_key } => strings::text(*reason_key).to_owned(),
                    Outcome::Failed { message } => message.clone(),
                    Outcome::Done => String::new(),
                });
                return out;
            }
        };
        let card = PartCard {
            title: editor.title(),
            fields: card_fields(&plan.fields),
            devices: Vec::new(),
            removed: plan.bom_removed.clone(),
            added: plan.bom_added.clone(),
            changed: plan.bom_changed.clone(),
            lost: plan.lost.clone(),
            fork: plan.fork.is_some(),
        };
        let devices: Vec<String> = plan.devices.iter().map(|d| self.tag_of(d.as_uuid())).collect();
        self.dialog = None;
        self.preview_host_card_as(
            &plan.command.name,
            plan.command.params,
            key!("reason-part-edit"),
            CardOrigin::Part(Box::new(PartCard { devices, ..card })),
        )
    }
}

impl ShellModel {
    /// Save of a corrected part of My parts (FT-1766): its next version goes into My parts, and the card of the
    /// `library.update` that brings the project's copy to it opens in the Preview tab (the changed fields and the
    /// devices that use it); Apply commits it. A field that does not parse refuses at its field.
    fn part_save_own_library(&mut self) -> Outcome {
        let Some(Dialog::PartEdit(mut editor)) = self.dialog.take() else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        match project.session.own_part_update(&editor.draft) {
            Ok(plan) => {
                // The card in words, as a part kept to the project lists its correction (the persona run m3-p4).
                let fields = plan
                    .copies
                    .first()
                    .map(|c| card_fields(&c.fields))
                    .unwrap_or_default();
                let devices = plan.devices.iter().map(|d| self.tag_of(d.as_uuid())).collect();
                let card = PartCard {
                    title: editor.title(),
                    fields,
                    devices,
                    removed: plan.bom_removed,
                    added: plan.bom_added,
                    changed: plan.bom_changed,
                    lost: plan.lost,
                    fork: false,
                };
                self.preview_host_card_as(
                    &plan.command.name,
                    plan.command.params,
                    key!("reason-library-update"),
                    CardOrigin::Part(Box::new(card)),
                )
            }
            Err(e) => {
                if let PartDraftError::Field(m) = &e {
                    editor.draft.messages.retain(|x| x.field != m.field);
                    editor.draft.messages.push(m.clone());
                }
                let out = draft_refusal(e);
                editor.error = Some(match &out {
                    Outcome::Refused { reason_key } => strings::text(*reason_key).to_owned(),
                    Outcome::Failed { message } => message.clone(),
                    Outcome::Done => String::new(),
                });
                self.dialog = Some(Dialog::PartEdit(editor));
                out
            }
        }
    }

    /// Save of a new part (FT-1766): the part goes into My parts and the project, or into the project alone
    /// (`Session::new_part_command`), in one undo step; a field that does not parse refuses at its field. From the
    /// part dialog, the dialog opens again on the new part; from the Cable form, the form with the new cable type
    /// chosen. With `another` the editor opens again on a new part like this one but for its part number and its
    /// cross-section, the next size of a series (rc.3).
    fn part_save_new(&mut self, another: bool) -> Outcome {
        let Some(Dialog::PartEdit(mut editor)) = self.dialog.take() else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        let words = |out: &Outcome| match out {
            Outcome::Refused { reason_key } => strings::text(*reason_key).to_owned(),
            Outcome::Failed { message } => message.clone(),
            Outcome::Done => String::new(),
        };
        let made = match project.session.new_part_command(&editor.draft) {
            Ok(made) => made,
            Err(e) => {
                if let PartDraftError::Field(m) = &e {
                    editor.draft.messages.retain(|x| x.field != m.field);
                    editor.draft.messages.push(m.clone());
                }
                let out = draft_refusal(e);
                editor.error = Some(words(&out));
                self.dialog = Some(Dialog::PartEdit(editor));
                return out;
            }
        };
        let out = self.run_commit(&made.command);
        if !matches!(out, Outcome::Done) {
            editor.error = Some(words(&out));
            self.dialog = Some(Dialog::PartEdit(editor));
            return out;
        }
        let number = editor.draft.text("part_number").trim().to_owned();
        let toast = match &made.library {
            Some(name) => strings::format(key!("toast-part-new-library"), &[&number, name]),
            None => strings::format(key!("toast-part-new"), &[&number]),
        };
        self.notify(toast, true);
        if another {
            return self.part_new_like(*editor);
        }
        if let Some(form) = editor.cable_form.take() {
            match editor.bus_part {
                Some(b) => self.bus_part_made(*form, &number, b.field),
                None => self.cable_type_made(*form, &number),
            }
            return Outcome::Done;
        }
        if let Some(mut chooser) = editor.back.take() {
            chooser.query.text = number.clone();
            chooser.cursor = 0;
            self.dialog = Some(Dialog::PartChoose(chooser));
            self.part_refresh();
        }
        // Made from the view of a station not made yet, a part that can head a station is its head; a card stands in
        // the view's catalogue.
        if self.station_draft().is_some_and(|d| d.head.is_none()) {
            let head = serde_json::Map::from_iter([("head".to_owned(), serde_json::Value::String(number))]);
            let _ = self.act_plc_rack_set(&head);
        }
        Outcome::Done
    }
}

impl ShellModel {
    /// The editor of `saved` open again on a new part that keeps everything but the part number and the cross-section
    /// (Save and add another size, rc.3): its own item ID, the form's way back kept.
    fn part_new_like(&mut self, saved: PartEditor) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let mut draft = saved.draft.clone();
        draft.copy = match project.session.new_part_draft() {
            Ok(d) => d.copy,
            Err(e) => return draft_refusal(e),
        };
        // Both empty until typed; Save says what is missing, not the form as it opens.
        for field in ["part_number", "cable.csa"] {
            let _ = project.session.set_draft_field(&mut draft, field, "");
        }
        draft.messages.clear();
        self.dialog = Some(Dialog::PartEdit(Box::new(PartEditor {
            draft,
            error: None,
            ..saved
        })));
        Outcome::Done
    }
}

/// The words of `part.new`'s `plc`: the rack dialog's station kinds and `module`.
const PLC_PRESETS: [&str; 5] = ["cpu", "remote", "valve_terminal", "compact", "module"];

/// What a new part starts as when the rack dialog asks for it (rc.2): `module` a module part; a station kind a station
/// part with one bus interface (a CPU's leads the bus as a PROFINET controller, the others answer as devices) and, for
/// a valve terminal and a compact device, eight channels of their own (outputs for the valves, inputs for the compact
/// device). The person changes any of it in the editor; a preset the draft does not take is left out.
fn plc_preset(session: &ew_api::Session, draft: &mut ew_api::PartDraft, word: &str) {
    let mut set = |field: &str, value: &str| {
        let _ = session.set_draft_field(draft, field, value);
    };
    if word == "module" {
        set("plc.role", "module");
        return;
    }
    set("plc.role", "station");
    match word {
        "valve_terminal" => {
            set("plc.kind", "do");
            set("plc.count", "8");
        }
        "compact" => {
            set("plc.kind", "di");
            set("plc.count", "8");
        }
        _ => {}
    }
    draft.add_bus_row();
    if word == "cpu" {
        let _ = session.set_draft_field(draft, "bus.0.role", "controller");
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
            // The tier in the words the library navigator uses for it ("System", "Company").
            Some((_, name, tier)) => {
                let key = format!("lib-tier-{}", format!("{tier:?}").to_lowercase());
                match strings::lookup(&key) {
                    Some(word) => format!("{name} ({word})"),
                    None => name.clone(),
                }
            }
            None => strings::text(key!("part-choose-view-project")).to_owned(),
        };
        let head = match &o.successor {
            Some(next) => strings::format(
                key!("part-choose-detail-successor"),
                &[&source, &o.version, &o.status, next],
            ),
            None => strings::format(key!("part-choose-detail"), &[&source, &o.version, &o.status]),
        };
        let functions = project.session.part_offer_functions(&o.pick);
        Some(if functions.is_empty() {
            head
        } else {
            strings::format(
                key!("part-choose-detail-functions"),
                &[&head, &functions.join(", ")],
            )
        })
    }
}

/// A part card's changed members in words (the persona run m3-p4 read the sections as raw JSON): each member by its
/// label, the sections as the interfaces and PLC fields that change ([`section_changes`]), the pins made again as
/// their designations (their function template following them, unlisted).
fn card_fields(fields: &[ew_api::FieldChange]) -> Vec<(String, String, String)> {
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
    let pins = fields.iter().any(|f| f.member == "cp_pattern");
    fields
        .iter()
        .filter(|f| !(pins && f.member == "function_template"))
        .flat_map(|f| match f.member.as_str() {
            "sections" => section_changes(f.old.as_ref(), f.new.as_ref()),
            "cp_pattern" => vec![(
                strings::text(key!("part-field-cps")).to_owned(),
                designations(f.old.as_ref()),
                designations(f.new.as_ref()),
            )],
            member => vec![(member_label(member), text(&f.old), text(&f.new))],
        })
        .collect()
}

/// A member of a part as the part card names it: its `part-field-<member>` text, else its field label.
fn member_label(member: &str) -> String {
    strings::lookup(&format!("part-field-{member}"))
        .map_or_else(|| strings::field_label("part", member), str::to_owned)
}

/// The designations of a CP pattern, joined by commas; empty for none.
fn designations(pattern: Option<&Value>) -> String {
    pattern
        .and_then(Value::as_array)
        .map(|cps| {
            cps.iter()
                .filter_map(|c| c.get("designation").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// Whole numbers as runs ("1-32, 40"), as the Allowed slots field takes them.
fn runs(numbers: &[u64]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < numbers.len() {
        let mut j = i;
        while j + 1 < numbers.len() && numbers[j + 1] == numbers[j] + 1 {
            j += 1;
        }
        out.push(if j > i {
            format!("{}-{}", numbers[i], numbers[j])
        } else {
            numbers[i].to_string()
        });
        i = j + 1;
    }
    out.join(", ")
}

/// A scalar of a part section in words: a string as it is, a number as its digits; empty for anything else.
fn scalar(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// The interfaces of a bus section by their (interface, port label), in list order, the first of each pair.
fn interfaces(bus: Option<&Value>) -> Vec<((String, String), Value)> {
    let mut out: Vec<((String, String), Value)> = Vec::new();
    for p in bus
        .and_then(|b| b.get("ports"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let key = (scalar(p.get("interface")), scalar(p.get("port_label")));
        if !out.iter().any(|(k, _)| *k == key) {
            out.push((key, p.clone()));
        }
    }
    out
}

/// An interface's socket count in words ("8 ports", "1 port"); one where it counts none.
fn ports_words(p: &Value) -> String {
    let n = p
        .get("ports")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(1);
    strings::format_count(crate::count_key!("part-change-ports"), n)
}

/// An interface in words: its bus, its role and its socket count ("PROFINET, Device, 8 ports").
fn interface_words(p: &Value) -> String {
    let choice = |member: &str| {
        strings::field_choice_text("function", &format!("bus_port.{member}"), &scalar(p.get(member)))
    };
    [choice("bus"), choice("role"), ports_words(p)].join(", ")
}

/// The changes of a part's `sections` in words (the persona run m3-p4 read them as JSON): per interface of the bus
/// section that comes, goes or changes, "Interface X1: 8 ports → 6 ports" (its socket count alone when nothing else
/// of it changes, else its bus, role and count); per field of the PLC section the part editor shows ("Channels: 8 →
/// 16", "Pin pattern: {n+1} → X2:{n+1}"), the I/O ranges and bit map that follow the channels left out; anything else
/// as one line "Other part data".
fn section_changes(old: Option<&Value>, new: Option<&Value>) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let empty = Value::Null;
    let (old, new) = (old.unwrap_or(&empty), new.unwrap_or(&empty));
    let (a, b) = (interfaces(old.get("bus")), interfaces(new.get("bus")));
    let mut keys: Vec<&(String, String)> = a.iter().map(|(k, _)| k).collect();
    keys.extend(
        b.iter()
            .map(|(k, _)| k)
            .filter(|k| !a.iter().any(|(x, _)| x == *k)),
    );
    let strip = |p: &Value| {
        let mut p = p.clone();
        if let Some(m) = p.as_object_mut() {
            m.remove("ports");
        }
        p
    };
    for key in keys {
        let x = a.iter().find(|(k, _)| k == key).map(|(_, p)| p);
        let y = b.iter().find(|(k, _)| k == key).map(|(_, p)| p);
        if x == y {
            continue;
        }
        let label = if key.1.is_empty() {
            key.0.clone()
        } else {
            format!("{} {}", key.0, key.1)
        };
        let only_ports = matches!((x, y), (Some(x), Some(y)) if strip(x) == strip(y));
        let words = |p: Option<&Value>| match p {
            None => String::new(),
            Some(p) if only_ports => ports_words(p),
            Some(p) => interface_words(p),
        };
        out.push((
            strings::format(key!("part-change-interface"), &[&label]),
            words(x),
            words(y),
        ));
    }
    let (p, q) = (old.get("plc").unwrap_or(&empty), new.get("plc").unwrap_or(&empty));
    let plc: [(&str, Key<'static>); 9] = [
        ("/role", key!("part-plc-role")),
        ("/channels/kind", key!("part-plc-kind")),
        ("/channels/count", key!("part-plc-count")),
        ("/channels/data_type", key!("part-plc-data_type")),
        ("/channels/designation", key!("part-plc-designation")),
        ("/pin_of", key!("part-plc-pin_of")),
        ("/supply_groups/0/cps/l_plus", key!("part-plc-l_plus")),
        ("/supply_groups/0/cps/m", key!("part-plc-m")),
        ("/limits/max_modules", key!("part-plc-max_modules")),
    ];
    for (path, key) in plc {
        let (x, y) = (p.pointer(path), q.pointer(path));
        if x != y {
            // A role or a channel kind is a choice of the form, in its words ("Station", "DI").
            let words = |v: Option<&Value>| match path {
                "/role" | "/channels/kind" => strings::choice_text(&scalar(v)),
                _ => scalar(v),
            };
            out.push((strings::text(key).to_owned(), words(x), words(y)));
        }
    }
    let slots = |v: &Value| -> Vec<u64> {
        v.get("allowed_slots")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default()
    };
    if slots(p) != slots(q) {
        out.push((
            strings::text(key!("part-plc-slots")).to_owned(),
            runs(&slots(p)),
            runs(&slots(q)),
        ));
    }
    // What the forms do not show, beside the members that follow the channels.
    let rest = |v: &Value| {
        let mut v = v.clone();
        if let Some(m) = v.as_object_mut() {
            m.remove("bus");
            if let Some(plc) = m.get_mut("plc").and_then(Value::as_object_mut) {
                for k in [
                    "role",
                    "channels",
                    "pin_of",
                    "limits",
                    "allowed_slots",
                    "io",
                    "bit_of",
                    "supply_groups",
                ] {
                    plc.remove(k);
                }
            }
        }
        v
    };
    if rest(old) != rest(new) {
        out.push((
            strings::text(key!("part-change-other")).to_owned(),
            String::new(),
            strings::text(key!("part-change-other-changed")).to_owned(),
        ));
    }
    out
}
