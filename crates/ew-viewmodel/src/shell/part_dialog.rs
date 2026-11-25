// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part selection dialog and the part editor.
//!
//! **Choosing**. `part.choose {target?, use?, add?}` opens [`Dialog::PartChoose`] for the property
//! grid's devices (a part use `use` is replaced, `add` appends, a device without part uses appends), the strip editor's
//! marked terminal rows (their Part cell: every part use replaced), a Bridge part cell or an accessory Part cell (the
//! kind filter preset by the kinds). `part.search` changes the query and lists `Session::part_offers` again;
//! `part.pick` compiles the active row, a 1-based `index` or a part text into the one command of
//! `Session::part_pick_command` and commits it with one undo step; a library part is then recorded among the recently
//! used parts of the settings. `part.favourite` toggles a library part among the favourites.
//!
//! **Editing**. `part.edit {part?}` opens [`Dialog::PartEdit`] with the draft of a held copy (the `part`
//! text, else the Libraries navigator's part row when it has the focus, else the property grid's device's first part
//! use, else that navigator's cursor row). `part.field {field, value}` sets and checks one field; its message shows
//! under the field. `part.save` builds `part.edit_copy` with `Session::part_edit_plan` and opens the `part` card in the
//! Preview tab, whose Apply commits it (Bulk, always previewed); a field that does not parse refuses with its message.

use ew_api::output::{NavKind, Uuid};
use ew_api::{
    ApiError, AssignMode, Fit, OfferView, PartDraft, PartDraftError, PartOffer, PartOfferQuery, PartTarget,
};
use serde_json::{Map, Value};

use super::{Dialog, Outcome, ShellModel, failed, flag, string_param};
use crate::key;
use crate::properties::{CardOrigin, PartCard};
use crate::strings;
use crate::strings::Key;

mod targets;

mod editor;

/// The result list shows this many rows at a time.
pub const PART_ROWS: usize = 50;

/// The part kinds of the layout tools: a device's part dialog offers one only where panel layout attaches it.
const LAYOUT_KIND_WORDS: [&str; 5] = ["enclosure", "mounting_plate", "rail", "duct", "end_bracket"];

/// The part selection dialog's state.
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
    /// Why each offer does not fit a cable, in the offers' order; `None` where it fits.
    pub misfits: Vec<Option<String>>,
    /// The active row (0-based); `None` until the person chooses one (Enter took a first row the person had
    /// not chosen).
    pub cursor: Option<usize>,
    /// Whether Type not known yet…'s field opens with the dialog (the command search's Type not known yet…).
    pub naming: bool,
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

/// The part editor's state.
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
    /// New cable type…: the small form of a new cable part (its maker, type and number, then the cable section).
    pub cable_type: bool,
    /// The Cable form New cable type… was chosen in: Save opens it again with the new type chosen.
    pub cable_form: Option<Box<super::FormDialog>>,
    /// New bus cable… or New plug… of a bus line's Cable form: the small form of a new bus part.
    pub bus_part: Option<BusPart>,
}

/// What New bus cable… and New plug… make: a cable or a plug for a bus, and the form field it returns to.
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
    /// The label of the draft field `field` in the form's identity rows ([`draft_label`]): a bus part's type
    /// designation is its part name, a cable type's its series.
    pub fn identity_label(&self, field: &str) -> &'static str {
        if self.bus_part.is_some() && field == "type_designation" {
            strings::text(key!("part-bus-part-name"))
        } else if self.cable_type && field == "type_designation" {
            strings::text(key!("part-cable-series"))
        } else {
            draft_label(field)
        }
    }

    /// What the form's foot says beside Save and Cancel, on the one line every form keeps there: the refusal of the
    /// last Save, else the newest message of a field, after the field's words (`Width (mm): "six" is not a length`);
    /// the PLC section says its own in the lines it keeps. `None` while nothing is wrong.
    pub fn foot_message(&self) -> Option<String> {
        if let Some(error) = &self.error {
            return Some(error.clone());
        }
        let m = (self.draft.messages.iter().rev()).find(|m| !m.field.starts_with("plc."))?;
        let label = if m.field.starts_with("mech.") {
            size_label(&m.field).to_owned()
        } else if crate::ew_api::DRAFT_FIELDS.contains(&m.field.as_str()) {
            self.identity_label(&m.field).to_owned()
        } else {
            field_label(&m.field)
        };
        Some(strings::format(
            key!("part-form-message"),
            &[&label, &message_text(Key::run_time(m.key), &m.args)],
        ))
    }

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

/// A new plug's description in words: `PROFIBUS DP plug, angled cable outlet, programming socket, switchable
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
        return plc_label(member).to_owned();
    }
    if let Some(member) = field.strip_prefix("cable.") {
        let member = member.split('.').next().unwrap_or_default();
        return cable_label(member).to_owned();
    }
    if let Some((_, member)) = field.strip_prefix("bus.").and_then(|r| r.split_once('.')) {
        return bus_label(member).to_owned();
    }
    draft_label(field).to_owned()
}

/// The label of the part editor's draft field `field` (`part-field-<field>`).
pub fn draft_label(field: &str) -> &'static str {
    strings::text(Key::run_time(&format!("part-field-{field}")))
}

/// The label of the part editor's size and mounting field `field` (`part-size-<field>`, `.` and `_` as `-`).
pub fn size_label(field: &str) -> &'static str {
    strings::text(Key::run_time(&format!(
        "part-size-{}",
        field.replace(['.', '_'], "-")
    )))
}

/// The label of the part editor's PLC member `member` (`part-plc-<member>`).
pub fn plc_label(member: &str) -> &'static str {
    strings::text(Key::run_time(&format!("part-plc-{member}")))
}

/// The label of the part editor's cable member `member` (`part-cable-<member>`).
pub fn cable_label(member: &str) -> &'static str {
    strings::text(Key::run_time(&format!("part-cable-{member}")))
}

/// The word of the cable marking `marking` (`part-cable-marking-<marking>`).
pub fn marking_word(marking: &str) -> &'static str {
    strings::text(Key::run_time(&format!("part-cable-marking-{marking}")))
}

/// The label of the part editor's bus member `member` (`part-bus-<member>`).
pub fn bus_label(member: &str) -> &'static str {
    strings::text(Key::run_time(&format!("part-bus-{member}")))
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

/// `<library ID>/<item ID>`, the settings' form of a library item.
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

/// A part text of the forms against an offer or a copy: `<part number> (<manufacturer>)`, with ` · <description>`,
/// or the bare part number.
fn matches_text(text: &str, number: &str, manufacturer: &str, description: &str) -> bool {
    let t = text.trim();
    let short = format!("{number} ({manufacturer})");
    t.eq_ignore_ascii_case(number)
        || t.eq_ignore_ascii_case(&short)
        || t.eq_ignore_ascii_case(&format!("{short} \u{b7} {description}"))
}

/// What a typed text names a part as being for (doc 07 §4.3): the function categories and the contact kinds
/// whose words take every word of the text, each word the start of one of theirs (or, from four letters, inside one:
/// a Dutch compound). A category's words are its `function-category-<key>` text in each shipped language and its key
/// split at its capitals ("Fluid Sensor"); a contact kind's are its `symbol-kind-<kind>` text in each. Nothing for a
/// text without words.
pub(crate) fn purposes(text: &str) -> (Vec<String>, Vec<String>) {
    let words = |t: &str| -> Vec<String> {
        t.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_lowercase)
            .collect()
    };
    let typed = words(text);
    if typed.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let names = |texts: Vec<String>| {
        let theirs: Vec<String> = texts.iter().flat_map(|t| words(t)).collect();
        typed.iter().all(|w| {
            theirs
                .iter()
                .any(|t| t.starts_with(w.as_str()) || (w.chars().count() >= 4 && t.contains(w.as_str())))
        })
    };
    let shipped = |key: &str| -> Vec<String> {
        strings::shipped_texts(key)
            .into_iter()
            .map(|(_, t)| t.to_owned())
            .collect()
    };
    let categories = strings::source_keys("function-category-")
        .into_iter()
        .filter(|k| {
            let mut texts = shipped(&format!("function-category-{k}"));
            let mut split = String::new();
            for c in k.chars() {
                if c.is_uppercase() && !split.is_empty() {
                    split.push(' ');
                }
                split.push(c);
            }
            texts.push(split);
            names(texts)
        })
        .map(str::to_owned)
        .collect();
    let kinds = strings::source_keys("symbol-kind-")
        .into_iter()
        .filter(|k| !k.ends_with("-help") && names(shipped(&format!("symbol-kind-{k}"))))
        .map(str::to_owned)
        .collect();
    (categories, kinds)
}

fn api_refusal(e: ApiError) -> Outcome {
    match e {
        ApiError::OtherVersionHeld { .. } => Outcome::refused(key!("reason-part-other-version")),
        ApiError::Library { .. } => Outcome::refused(key!("reason-part-not-installed")),
        ApiError::Strip(ew_api::output::StripEditError::Refused(reason_key)) => Outcome::Refused {
            reason: Key::run_time(reason_key).into(),
        },
        other => Outcome::Failed {
            message: other.to_string(),
        },
    }
}

fn draft_refusal(e: PartDraftError) -> Outcome {
    match e {
        PartDraftError::Refused(reason_key) => Outcome::Refused {
            reason: Key::run_time(reason_key).into(),
        },
        PartDraftError::Field(m) if m.args.is_empty() && m.key.starts_with("reason-") => Outcome::Refused {
            reason: Key::run_time(m.key).into(),
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

    /// `part.choose {target?, use?, add?, strip?}`: opens the part dialog for its target; `strip` (its ID), for
    /// the strip's terminals without a part.
    pub(crate) fn act_part_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        // A cable by its ID (Choose a cable type… on its finding): its cable parts, those that fit first.
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
            cursor: None,
            naming: false,
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
        c.cursor = None;
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
        c.cursor
            .and_then(|i| c.offers.get(i))
            .cloned()
            .ok_or(Outcome::refused(key!("reason-part-choose-none")))
    }

    /// `part.pick {index?, part?}` (Enter): commits the choice and records a library part as recently used.
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

    /// `part.placeholder {description?}` (Type not known yet… in the part dialog, doc 07 §3.3; also in the command search
    /// and the Parts row's menu, where the part dialog of the selection opens first): gives the dialog's devices a
    /// placeholder part described by `description` in the dialog's mode, as Choose would give them a part, and closes
    /// the dialog; without `description` the dialog's field for it opens. A bridge's or an accessory's part is refused: a
    /// placeholder is a device's.
    pub(crate) fn act_part_placeholder(&mut self, p: &Map<String, Value>) -> Outcome {
        let description = match string_param(p, "description") {
            Ok(d) => d.map(str::to_owned),
            Err(e) => return e,
        };
        // Without the dialog (the command search, the Parts row's menu): the part dialog of the selection opens.
        if self.part_chooser().is_none() {
            let out = self.act_part_choose(&Map::new());
            if out != Outcome::Done {
                return out;
            }
        }
        // Without a description its field opens, to be named.
        let Some(description) = description else {
            if let Some(Dialog::PartChoose(c)) = &mut self.dialog {
                c.naming = true;
            }
            return Outcome::Done;
        };
        let Some(c) = self.part_chooser() else {
            return Outcome::refused(key!("reason-no-part-dialog"));
        };
        let PartTarget::Devices { devices, mode } = &c.target else {
            return Outcome::refused(key!("reason-part-placeholder-target"));
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let cmd = match project.session.placeholder_command(devices, *mode, &description) {
            Ok(cmd) => cmd,
            Err(e) => return api_refusal(e),
        };
        let out = self.run_commit(&cmd);
        if matches!(out, Outcome::Done) {
            self.dialog = None;
        }
        out
    }

    /// `part.favourite {index?}`: toggles a library part among the favourites.
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
                .map(|i| usize::try_from(i).unwrap_or(1).saturating_sub(1));
        }
        self.part_refresh();
        Outcome::Done
    }
}

impl ShellModel {
    /// Save of a corrected part of My parts: its next version goes into My parts, and the card of the
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
                // The card in words, as a part kept to the project lists its correction.
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
                    Outcome::Refused { reason: reason_key } => strings::render(reason_key),
                    Outcome::Failed { message } => message.clone(),
                    Outcome::Done => String::new(),
                });
                self.dialog = Some(Dialog::PartEdit(editor));
                out
            }
        }
    }

    /// Save of a new part: the part goes into My parts and the project, or into the project alone
    /// (`Session::new_part_command`), in one undo step; a field that does not parse refuses at its field. From the
    /// part dialog, the dialog opens again on the new part; from the Cable form, the form with the new cable type
    /// chosen. With `another` the editor opens again on a new part like this one but for its part number and its
    /// cross-section, the next size of a series.
    fn part_save_new(&mut self, another: bool) -> Outcome {
        let Some(Dialog::PartEdit(mut editor)) = self.dialog.take() else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        let words = |out: &Outcome| match out {
            Outcome::Refused { reason: reason_key } => strings::render(reason_key),
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
            chooser.cursor = None;
            self.dialog = Some(Dialog::PartChoose(chooser));
            self.part_refresh();
        }
        // Made from the view of a station not made yet with its empty head chosen, a part that can head a station is its
        // head; else the part stands in the view's catalogue, offered, and nothing of the station changes unasked.
        let from_head = self
            .station_tab()
            .is_some_and(|t| t.cursor == Some(crate::area::StationCursor::Head));
        if from_head && self.station_draft().is_some_and(|d| d.head.is_none()) {
            let head = serde_json::Map::from_iter([("head".to_owned(), serde_json::Value::String(number))]);
            let _ = self.act_plc_rack_set(&head);
        }
        Outcome::Done
    }
}

impl ShellModel {
    /// The editor of `saved` open again on a new part that keeps everything but the part number and the cross-section
    /// (Save and add another size): its own item ID, the form's way back kept.
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

/// Whether `rows` is the one bus row [`plc_preset`] gives a station, as it made it: `X1`, PROFINET, a controller or a
/// device, nothing else filled in.
fn preset_bus(rows: &[ew_api::BusRow]) -> bool {
    matches!(rows, [r] if ["controller", "device"].contains(&r.role.as_str())
    && *r == ew_api::BusRow {
        interface: "X1".to_owned(),
        bus: "profinet".to_owned(),
        role: r.role.clone(),
        ..ew_api::BusRow::default()
    })
}

/// The words of `part.new`'s `plc`: the rack dialog's station kinds and `module`.
const PLC_PRESETS: [&str; 5] = ["cpu", "remote", "valve_terminal", "compact", "module"];

/// What a new part starts as when the rack dialog asks for it: `module` a module part; a station kind a station
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
    /// The detail line of offer `i` (0-based) of the open part dialog: its source and tier, version, status and
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

/// A part card's changed members in words (never the sections as raw JSON): each member by its
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

/// The changes of a part's `sections` in words (never as JSON): per interface of the bus
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
        ("/more_channels/0/kind", key!("part-plc-kind2")),
        ("/more_channels/0/count", key!("part-plc-count2")),
        ("/channels/data_type", key!("part-plc-data_type")),
        ("/channels/designation", key!("part-plc-designation")),
        ("/pin_of", key!("part-plc-pin_of")),
        ("/limits/max_modules", key!("part-plc-max_modules")),
    ];
    for (path, key) in plc {
        let (x, y) = (p.pointer(path), q.pointer(path));
        if x != y {
            // A role or a channel kind is a choice of the form, in its words ("Station", "DI").
            let words = |v: Option<&Value>| match path {
                "/role" | "/channels/kind" | "/more_channels/0/kind" => strings::choice_text(&scalar(v)),
                _ => scalar(v),
            };
            out.push((strings::text(key).to_owned(), words(x), words(y)));
        }
    }
    // The supply pins of every group, as the form lists them.
    for (member, key) in [("l_plus", key!("part-plc-l_plus")), ("m", key!("part-plc-m"))] {
        let pins = |v: &Value| -> String {
            let list: Vec<String> = v
                .get("supply_groups")
                .and_then(Value::as_array)
                .map(|g| {
                    g.iter()
                        .map(|g| scalar(g.pointer(&format!("/cps/{member}"))))
                        .collect()
                })
                .unwrap_or_default();
            list.join(", ")
        };
        if pins(p) != pins(q) {
            out.push((strings::text(key).to_owned(), pins(p), pins(q)));
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
                    "more_channels",
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
