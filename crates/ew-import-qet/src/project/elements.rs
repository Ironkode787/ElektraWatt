// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Placed elements → devices, functions and representations (doc 14 §4.4–§4.6).
//!
//! Every placed element is classified by its definition's `link_type`:
//!
//! | Element | Records |
//! |---|---|
//! | `simple`, `master`, none | device + main function + representation |
//! | `thumbnail` | nothing: a graphic (doc 14 §4.3), EW-IMP-004 |
//! | `slave` linked to a master | auxiliary function of the master's device + representation (§4.5) |
//! | `slave` without master | own device, function promoted to main (EW-IMP-116) |
//! | `next_report` / `previous_report` | interruption point (`ew.fn.ip`, direction source / target) + representation (§4.6) |
//! | `terminal` | terminal device in a strip ([`super::strips`]) |
//! | one terminal, no label, ≥ 3 conductors | junction (§4.7, EW-IMP-118, emitted by [`super::wires`]) |
//! | definition missing or refused | nothing; wires to it are lost (EW-IMP-003) |
//!
//! Source keys (doc 14 §2.3, suffixed per record kind): device `qet:el:<key>`, function `qet:el:<key>#fn`,
//! representation `qet:el:<key>#rep`, junction `qet:junction:<key>`, where `<key>` is the element's UUID,
//! `<uuid>#<folio key>#<n>` for the n-th further copy of a UUID on a folio (copy-paste, EW-IMP-120), or
//! `<folio key>#el#<n>` for an element without UUID at source position n.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::{Id, Uuid};
use ew_interop_api::{Code, ReportEntry, Severity};
use ew_mltext::{Ident, MlText};
use ew_schema::{
    Align, Device, DeviceAspects, FnRole, FrozenTag, Function, FunctionKind, IpDirection, IpSpec, KeyedList,
    LibCopy, Record, Representation, SlotKind, SlotOverride, SymbolDef, TagScheme, TagSign, TagSpec,
    TextAlign, TextSlot,
};
use ew_units::{Angle2, Nm, Orient2, Pt2};
use serde_json::Value;

use super::Def;
use super::builtins::Builtins;
use super::cx::{ElementKey, Findings, Ids, Placed, PlacedTerminal, Registry, Target, element_keys, turned};
use super::items;
use super::model::{Border, QetElement, QetProject, TextAt, norm_uuid};
use super::pages::{PagesOut, is_qet_border};
use super::strips;
use crate::decimal::Decimal;
use crate::element::ElementImport;
use crate::prims::text::INSTANCE_ALIGN;

/// The inputs of the element mapper.
pub struct Inputs<'a> {
    /// The project.
    pub project: &'a QetProject,
    /// The mapped pages.
    pub pages: &'a PagesOut,
    /// The definitions by collection path.
    pub defs: &'a BTreeMap<String, Def>,
    /// Conductor ends per element.
    pub counts: &'a BTreeMap<ElementKey, u32>,
    /// IDs.
    pub ids: &'a Ids,
    /// Built-in documents.
    pub builtins: &'a Builtins,
}

/// What the element mapper emits.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ElementsOut {
    /// Device, function and representation records.
    pub records: Vec<Record>,
    /// The placed elements for the conductor mapper.
    pub registry: Registry,
    /// The collection paths of the definitions placed elements use.
    pub used_defs: BTreeSet<String>,
    /// Elements dropped (reported).
    pub dropped: u32,
    /// Objects inferred (strips).
    pub inferred: u32,
    /// The font-substitution messages already reported (EW-IMP-006 once per family).
    pub fonts: BTreeSet<String>,
    /// The hidden text slots the placed elements need beyond their symbol's own, by symbol copy ID, in the
    /// order their indices count on from the symbol's slots (doc 14 §4.6): a folio report's cross-reference
    /// and name, a master's or slave's cross-reference, each in the style the element prints it in.
    pub extra_slots: BTreeMap<Uuid, Vec<TextSlot>>,
    /// Whether several explicit QET pairs share a name, so that the project must pair in chain mode to keep
    /// them (doc 14 §4.6).
    pub chain_pairs: bool,
}

/// Where a master or a slave prints its cross-reference against its label text (QET's `<xref>` of the
/// master's type): `gap` px below the label's box, or above it, its text centred `right` px right of the
/// box's middle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct XrefPlace {
    gap: Decimal,
    above: bool,
    right: Decimal,
}

/// How far right of the middle of a master's label QET's contact list (`displayhas="contacts"`) has the middle
/// of its position texts, px. QET centres the list's box under the label; each row draws its contact in the
/// 30 px before its text (which the import does not draw), and the box adds 30 px before the rows and 4 px
/// after them (`CrossRefItem::drawAsContacts`): the texts' middle is (30 + 30 − 4) / 2 px right of the box's.
const CONTACT_TEXT_SHIFT_PX: i64 = 28;

/// The point size QET draws a master's or slave's cross-reference in (`QETApp::diagramTextsFont(5)`).
const XREF_POINT_SIZE: i64 = 5;

/// The override that shows `slot` at `pos` turned to `angle`.
fn shown(slot: u16, pos: Pt2, angle: Angle2) -> SlotOverride {
    SlotOverride {
        slot,
        pos: Some(pos),
        angle: Some(angle),
        visible: Some(true),
        extra: BTreeMap::new(),
    }
}

/// The master type of `e`: its definition's `kindInformation` `type` ("" when it has none).
fn kind_type<'e>(e: &Elem<'e>) -> &'e str {
    e.import
        .fndef
        .as_ref()
        .and_then(|f| f.item.extra.get("qet.kind"))
        .and_then(|k| k.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("")
}

/// The fallback letter of an element without a usable label or `prefix` (EN 81346-2:2009 class A: no
/// main class identified). Recorded as an import mapping decision (crate README).
pub const FALLBACK_LETTER: &str = "A";

/// EW-IMP-122 (proposed, the next free code of the doc 14 §2.5 catalogue): element without label; its tag
/// keeps the prefix letter, the import numbers it (`-1A1`), and the page prints no tag for it, as QET prints none.
pub const UNLABELED: Code = Code::imp(122);

/// The element a link UUID names, seen from `from`: the copy on `from`'s folio when the UUID is repeated
/// (copy-paste), else the first copy in document order.
fn linked<'e, 'a>(
    elems: &'e [Elem<'a>],
    by_uuid: &BTreeMap<&str, Vec<usize>>,
    from: &Elem<'_>,
    uuid: &str,
) -> Option<&'e Elem<'a>> {
    let idxs = by_uuid.get(uuid)?;
    idxs.iter()
        .map(|&i| &elems[i])
        .find(|m| m.diagram == from.diagram)
        .or_else(|| idxs.first().map(|&i| &elems[i]))
}

/// One placed element that survived classification: where it is and what it is.
pub(crate) struct Elem<'a> {
    pub(crate) diagram: usize,
    pub(crate) key: String,
    /// The element key made globally unique (`d<diagram>#<n>` for elements without a UUID).
    pub(crate) ek: String,
    pub(crate) el: &'a QetElement,
    pub(crate) path: &'a str,
    pub(crate) import: &'a ElementImport,
    pub(crate) pos: Pt2,
    pub(crate) orient: Orient2,
}

impl Elem<'_> {
    /// The symbol copy.
    pub(crate) fn symbol(&self) -> &LibCopy<SymbolDef> {
        &self.import.symbol
    }

    /// The definition terminals placed on the page, in definition order.
    pub(crate) fn terminals(&self) -> Vec<PlacedTerminal> {
        self.symbol()
            .item
            .cps
            .as_slice()
            .iter()
            .map(|cp| {
                let rotated = self.orient.apply(cp.pos);
                let page_pos = self
                    .pos
                    .x
                    .0
                    .checked_add(rotated.x.0)
                    .zip(self.pos.y.0.checked_add(rotated.y.0))
                    .map(|(x, y)| Pt2::new(Nm(x), Nm(y)));
                PlacedTerminal {
                    uuid: cp
                        .extra
                        .get("qet.uuid")
                        .and_then(Value::as_str)
                        .and_then(norm_uuid),
                    local: cp.pos,
                    page_pos,
                    dir: turned(&cp.dir, self.orient),
                }
            })
            .collect()
    }

    /// The number of definition terminals.
    pub(crate) fn terminal_count(&self) -> usize {
        self.symbol().item.cps.as_slice().len()
    }
}

/// The orientation of QET's `orientation` 0–3 (quarter turns clockwise on the sheet, doc 14 §4.2).
pub(crate) fn orientation(k: u8) -> Orient2 {
    match k % 4 {
        1 => Orient2::R90,
        2 => Orient2::R180,
        3 => Orient2::R270,
        _ => Orient2::R0,
    }
}

/// What the tag inference made of a label.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TagOutcome {
    /// The label fits `{page}{letter}{counter}` on its own folio: a derived tag.
    Derived,
    /// The label is kept as a frozen tag (EW-IMP-103).
    Frozen,
    /// No label: letter only, no counter (the tag prints `?` until numbered).
    Unlabeled,
    /// The label is not an identifier: letter only, label kept as text (EW-IMP-008).
    Invalid,
}

/// The first run of ASCII letters of `s`, when it is an identifier.
fn letters_of(s: &str) -> Option<Ident> {
    let run: String = s
        .chars()
        .skip_while(|c| !c.is_ascii_alphabetic())
        .take_while(char::is_ascii_alphabetic)
        .collect();
    (!run.is_empty()).then(|| Ident::from_input(&run).ok()).flatten()
}

/// The letter of an element without a usable label: its QET `prefix` when that is letters only, else
/// [`FALLBACK_LETTER`].
fn fallback_letter(prefix: &str) -> Ident {
    let p = prefix.trim();
    if !p.is_empty() && p.chars().all(|c| c.is_ascii_alphabetic()) {
        if let Ok(i) = Ident::from_input(p) {
            return i;
        }
    }
    Ident::from_input(FALLBACK_LETTER).unwrap_or_else(|_| unreachable!("a letter is an identifier"))
}

/// Splits a label into its structure prefix (`+JE2416`, `=P01+J-E-2416`) and its product part (`-X1031`):
/// a label starting with `+` or `=` has its product part after the last `-`.
pub(crate) fn split_structure(label: &str) -> (Option<&str>, &str) {
    if label.starts_with('+') || label.starts_with('=') {
        match label.rfind('-') {
            Some(i) => (Some(&label[..i]), &label[i..]),
            None => (Some(label), ""),
        }
    } else {
        (None, label)
    }
}

/// The tag of an element labelled `label` on folio `folio` (doc 14 §4.4.1, spike form): rule R1
/// `{page}{letter}{counter}` with the product sign gives a derived tag; any other label a frozen tag with
/// the sign as written (`S51` → hidden sign); no label the `prefix` letter (else [`FALLBACK_LETTER`]) and
/// no counter. Returns the tag, the structure prefix of the label and the outcome.
pub(crate) fn infer_tag<'l>(
    label: &'l str,
    folio: &str,
    prefix: &str,
) -> (TagSpec, Option<&'l str>, TagOutcome) {
    let l = label.trim();
    let (structure, product) = split_structure(l);
    let (sign, text) = match product.strip_prefix('-') {
        Some(rest) => (TagSign::Shown, rest),
        None => (TagSign::Hidden, product),
    };
    if text.is_empty() {
        return (
            TagSpec::new(fallback_letter(prefix), None),
            structure,
            TagOutcome::Unlabeled,
        );
    }
    // R1: {page}{letter}{counter} on the element's own folio, with the product sign.
    if sign == TagSign::Shown
        && !folio.is_empty()
        && let Some(rest) = text.strip_prefix(folio)
    {
        let letter: String = rest.chars().take_while(char::is_ascii_alphabetic).collect();
        let digits = &rest[letter.len()..];
        // Only labels that EW prints back unchanged are derived: upper-case letters, a counter without
        // leading zeros (`-101K01` and `-101f1` stay frozen, so the printed text does not change).
        if (1..=3).contains(&letter.len())
            && letter.bytes().all(|b| b.is_ascii_uppercase())
            && !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && let (Ok(l), Ok(n)) = (Ident::from_input(&letter), digits.parse::<u32>())
            && n > 0
            && n.to_string() == digits
        {
            return (TagSpec::new(l, Some(n)), structure, TagOutcome::Derived);
        }
    }
    match Ident::from_input(text) {
        Ok(t) => {
            let letter = letters_of(text).unwrap_or_else(|| fallback_letter(prefix));
            let tag = TagSpec {
                letter,
                counter: None,
                scheme: TagScheme::Default,
                frozen: Some(FrozenTag {
                    text: t,
                    sign,
                    extra: BTreeMap::new(),
                }),
                extra: BTreeMap::new(),
            };
            (tag, structure, TagOutcome::Frozen)
        }
        Err(_) => (
            TagSpec::new(fallback_letter(prefix), None),
            structure,
            TagOutcome::Invalid,
        ),
    }
}

/// The element-information members kept on the device or function: every non-empty entry except `label`.
fn info_extra(el: &QetElement, seen: &mut BTreeMap<String, u32>) -> Option<Value> {
    let mut obj = serde_json::Map::new();
    for (name, value) in &el.infos {
        if name == "label" || value.trim().is_empty() || obj.contains_key(name) {
            continue;
        }
        obj.insert(name.clone(), Value::String(value.clone()));
        *seen.entry(name.clone()).or_default() += 1;
    }
    (!obj.is_empty()).then_some(Value::Object(obj))
}

/// QET's composite text `format` for `el`: every `%{name}` replaced by the element information `name` (empty
/// when the element has none).
fn composite(format: &str, el: &QetElement) -> String {
    let mut out = String::new();
    let mut rest = format;
    while let Some(at) = rest.find("%{") {
        out.push_str(&rest[..at]);
        match rest[at + 2..].find('}') {
            Some(end) => {
                out.push_str(el.info(&rest[at + 2..at + 2 + end]).unwrap_or(""));
                rest = &rest[at + 3 + end..];
            }
            None => {
                out.push_str(&rest[at..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The first non-empty `UserText` dynamic text of `el`.
pub(crate) fn user_text(el: &QetElement) -> Option<&str> {
    el.dyn_texts
        .iter()
        .filter(|t| t.text_from == "UserText")
        .map(|t| t.text.trim())
        .find(|t| !t.is_empty())
}

/// The function an element gets: definition, device, role, kind, the function CP per definition terminal
/// and extra members.
pub(crate) struct FnSpec {
    pub(crate) def: Id<ew_schema::FnDef>,
    pub(crate) device: Option<Id<Device>>,
    pub(crate) role: FnRole,
    pub(crate) kind: FunctionKind,
    pub(crate) cps: Vec<Option<u16>>,
    pub(crate) extra: BTreeMap<String, Value>,
}

impl FnSpec {
    /// A normal function of `device` with `role`, one CP per definition terminal of `e`.
    pub(crate) fn normal(
        e: &Elem<'_>,
        def: Id<ew_schema::FnDef>,
        device: Id<Device>,
        role: FnRole,
    ) -> FnSpec {
        FnSpec {
            def,
            device: Some(device),
            role,
            kind: FunctionKind::Normal,
            cps: (0..e.terminal_count()).map(|i| u16::try_from(i).ok()).collect(),
            extra: BTreeMap::new(),
        }
    }
}

/// What one placed element is.
enum Class {
    Device,
    Slave { master: ElementKey },
    OrphanSlave,
    Report { source: bool },
    Terminal,
    Junction,
    Graphic,
}

/// The shared state of the element mapper.
pub(crate) struct Mx<'a, 'f> {
    pub(crate) inp: &'a Inputs<'a>,
    pub(crate) findings: &'f mut Findings,
    pub(crate) out: ElementsOut,
    /// Element-information names kept as text, with counts.
    pub(crate) infos_seen: BTreeMap<String, u32>,
    /// The elements (by `ek`) whose device tag and function text print exactly the label and the `function`
    /// QET prints, so that their own texts of these may show the symbol's slots (doc 14 §4.4.2).
    pub(crate) live: BTreeSet<String>,
    /// The label of each slave's master, by the slave's `ek`.
    pub(crate) master_label: BTreeMap<String, String>,
    /// Where each master and linked slave prints its cross-reference, by its `ek`.
    pub(crate) xref_place: BTreeMap<String, XrefPlace>,
}

impl Mx<'_, '_> {
    /// The folio label of diagram `d`.
    pub(crate) fn folio(&self, d: usize) -> &str {
        self.inp.pages.pages.get(d).map_or("", |p| p.folio.as_str())
    }

    /// A finding at element `e`.
    pub(crate) fn at(&mut self, e: &Elem<'_>, code: Code, severity: Severity, message: impl Into<String>) {
        let folio = self.folio(e.diagram).to_owned();
        let loc = self
            .findings
            .loc(Some(&folio), Some(e.ek.clone()), Some(e.el.line));
        self.findings.push(code, severity, loc, message);
    }

    /// A finding at element `e` about the device `device` it emits (the entry's target).
    fn at_device(
        &mut self,
        e: &Elem<'_>,
        device: Id<Device>,
        code: Code,
        severity: Severity,
        message: impl Into<String>,
    ) {
        let folio = self.folio(e.diagram).to_owned();
        let loc = self
            .findings
            .loc(Some(&folio), Some(e.ek.clone()), Some(e.el.line));
        self.findings.entry(
            ReportEntry::new(code, severity, message)
                .at(loc)
                .on(ew_schema::Kind::Device, device.as_uuid()),
        );
    }

    /// The function record of element `e` as `spec` says, its representation, and its registry entry.
    pub(crate) fn function_and_rep(&mut self, e: &Elem<'_>, spec: FnSpec) -> Id<ew_schema::Function> {
        let FnSpec {
            def,
            device,
            role,
            kind,
            cps,
            extra,
        } = spec;
        let ids = self.inp.ids;
        let fkey = format!("qet:el:{}#fn", e.ek);
        let rkey = format!("qet:el:{}#rep", e.ek);
        let fid = ids.id(&fkey);
        let ip_name = match &kind {
            FunctionKind::Ip(ip) => Some(ip.name.as_str().to_owned()),
            _ => None,
        };
        self.out.records.push(Record::Function(Function {
            id: fid,
            v: 1,
            device,
            def,
            role,
            kind,
            cp_overrides: Default::default(),
            source_ref: Some(ids.source_ref(&fkey)),
            template_entry: None,
            bus_port: None,
            extra,
        }));
        let page = self.inp.pages.pages[e.diagram].id;
        let text_slots = self.own_texts(e, page, ip_name.as_deref());
        self.out.records.push(Record::Representation(Representation {
            id: ids.id(&rkey),
            v: 1,
            functions: vec![fid],
            page,
            symbol: e.symbol().id,
            variant: 0,
            pos: e.pos,
            orient: e.orient,
            rep_type: e.symbol().item.rep_type.clone(),
            pin_map: Default::default(),
            text_slots,
            arrangement: None,
            anchor: None,
            box_: None,
            source_ref: Some(ids.source_ref(&rkey)),
            extra: BTreeMap::new(),
        }));
        self.out.used_defs.insert(e.path.to_owned());
        let folio = self.folio(e.diagram).to_owned();
        self.out.registry.insert(
            (e.diagram, e.key.clone()),
            Placed {
                ek: e.ek.clone(),
                diagram: e.diagram,
                page,
                folio,
                target: Target::Function { function: fid, cps },
                terminals: e.terminals(),
            },
        );
        fid
    }

    /// The device record of `e` with `tag`, `kind` and `aspects`; returns its ID.
    pub(crate) fn device(
        &mut self,
        key: &str,
        tag: TagSpec,
        function_text: MlText,
        kind: Option<ew_schema::DeviceKind>,
        aspects: DeviceAspects,
        extra: BTreeMap<String, Value>,
    ) -> Id<Device> {
        let ids = self.inp.ids;
        let id = ids.id(key);
        self.out.records.push(Record::Device(Device {
            id,
            v: 1,
            tag,
            kind,
            parent: None,
            position: None,
            structure: Default::default(),
            parts: Vec::new(),
            function_text,
            process_tag: None,
            tech_data: Default::default(),
            state: Default::default(),
            source_ref: Some(ids.source_ref(key)),
            aspects,
            extra,
        }));
        id
    }

    /// The generated FnDef copy of `e` (`None` for a definition that references a built-in FnDef).
    fn generated_def(e: &Elem<'_>) -> Option<Id<ew_schema::FnDef>> {
        e.import.fndef.as_ref().map(|f| f.id)
    }

    /// A device with its main function and representation.
    fn plain_device(&mut self, e: &Elem<'_>, unlabeled: &mut BTreeMap<usize, u32>, orphan: bool) {
        let Some(def) = Self::generated_def(e) else {
            self.drop_element(e, "the definition references a built-in function definition");
            return;
        };
        let folio = self.folio(e.diagram).to_owned();
        let label = e.el.info("label").unwrap_or("");
        let (tag, structure, outcome) = infer_tag(label, &folio, &e.el.prefix);
        let mut extra = BTreeMap::new();
        let dkey = format!("qet:el:{}", e.ek);
        let device_id: Id<Device> = self.inp.ids.id(&dkey);
        match outcome {
            TagOutcome::Derived => {}
            TagOutcome::Frozen => self.at_device(
                e,
                device_id,
                Code::imp(103),
                Severity::Info,
                format!(
                    "label `{}` does not fit the page rule {{page}}{{letter}}{{counter}}; kept as frozen tag",
                    label.trim()
                ),
            ),
            TagOutcome::Unlabeled => *unlabeled.entry(e.diagram).or_default() += 1,
            TagOutcome::Invalid => {
                extra.insert("qet.label".to_owned(), Value::String(label.to_owned()));
                self.at_device(
                    e,
                    device_id,
                    Code::FOREIGN_VALUE,
                    Severity::Warning,
                    format!(
                        "label `{}` is not an identifier; kept as text, tag not numbered",
                        label.trim()
                    ),
                );
            }
        }
        if let Some(s) = structure {
            extra.insert("qet.structure".to_owned(), Value::String(s.to_owned()));
        }
        if let Some(v) = info_extra(e.el, &mut self.infos_seen) {
            extra.insert("qet.info".to_owned(), v);
        }
        let function_text =
            e.el.info_nonempty("function")
                .or_else(|| e.el.info_nonempty("description"))
                .map(MlText::neutral)
                .unwrap_or_default();
        // The tag prints the label as written (derived from it, or frozen with its sign) unless a structure
        // prefix is split off; the function text is the `function` information when there is one.
        if matches!(outcome, TagOutcome::Derived | TagOutcome::Frozen) && structure.is_none() {
            self.live.insert(e.ek.clone());
        }
        let device = self.device(&dkey, tag, function_text, None, DeviceAspects::default(), extra);
        self.function_and_rep(e, FnSpec::normal(e, def, device, FnRole::Main));
        if orphan {
            self.at_device(
                e,
                device,
                Code::imp(116),
                Severity::Warning,
                "slave element without a linked master: own device, function promoted to main",
            );
        }
    }

    /// The texts element `e` prints (doc 14 §4.4.2): its own dynamic texts (QET 0.7+), else its old-style
    /// `<inputs>`, each where QET draws it and in its import style. A label or `function` text of an element in
    /// [`Mx::live`] shows the symbol's tag or function-text slot (the returned overrides); every other text
    /// that is not empty becomes a free text on `page` (`qet:el:<ek>#text<n>`). The symbol's slots stay hidden
    /// otherwise, so an element prints no text QET does not print.
    ///
    /// Cross-references (doc 14 §4.6): a folio report (`ip_name`, its interruption point's name) shows the
    /// cross-reference slot where its label text stands, which QET fills with its partner's position, and
    /// the name slot where a text of its name stands. A master or slave in [`Mx::xref_place`] shows the
    /// cross-reference slot below (or above) its label text, in QET's 5 pt. These slots show whatever the
    /// stored text says, because the printed text is derived.
    fn own_texts(
        &mut self,
        e: &Elem<'_>,
        page: Id<ew_schema::Page>,
        ip_name: Option<&str>,
    ) -> KeyedList<SlotOverride> {
        // (text, where, slot kind, whether the slot shows whatever the text, whether it is the label text, the
        // box's fixed width)
        type Text<'t> = (String, &'t TextAt, Option<SlotKind>, bool, bool, Option<Decimal>);
        let texts: Vec<Text<'_>> = if e.el.has_dyn_texts {
            e.el.dyn_texts
                .iter()
                .filter_map(|t| {
                    let info = t.info_name.as_deref().unwrap_or("");
                    let label = match t.text_from.as_str() {
                        "ElementInfo" => info == "label",
                        "CompositeText" => t.composite.as_deref().is_some_and(|c| c.contains("%{label}")),
                        _ => false,
                    };
                    let (text, kind, derived) = match t.text_from.as_str() {
                        "UserText" if ip_name.is_some_and(|n| n == t.text.trim()) => {
                            (t.text.clone(), Some(SlotKind::FunctionText), true)
                        }
                        "UserText" => (t.text.clone(), None, false),
                        // A report's label text, as an information or a composite text of the label alone, prints
                        // where its partner is (QET's report formula).
                        "ElementInfo" if ip_name.is_some() && info == "label" => {
                            (String::new(), Some(SlotKind::Xref), true)
                        }
                        "CompositeText"
                            if ip_name.is_some()
                                && t.composite.as_deref().is_some_and(|c| c.trim() == "%{label}") =>
                        {
                            (String::new(), Some(SlotKind::Xref), true)
                        }
                        "ElementInfo" => (
                            match (info, self.master_label.get(&e.ek)) {
                                // A slave prints its master's label (QET links the two).
                                ("label", Some(m)) if e.el.info_nonempty("label").is_none() => m.clone(),
                                _ => e.el.info(info).unwrap_or("").to_owned(),
                            },
                            match info {
                                "label" => Some(SlotKind::Tag),
                                "function" => Some(SlotKind::FunctionText),
                                _ => None,
                            },
                            false,
                        ),
                        "CompositeText" => {
                            (composite(t.composite.as_deref().unwrap_or(""), e.el), None, false)
                        }
                        _ => return None,
                    };
                    Some((text, &t.at, kind, derived, label, t.width))
                })
                .collect()
        } else {
            e.el.inputs
                .iter()
                .map(|i| (i.text.clone(), &i.at, None, false, false, None))
                .collect()
        };
        let live = self.live.contains(&e.ek);
        // QET hangs a master's or slave's cross-reference on its first label text.
        let mut place = self.xref_place.get(&e.ek).copied();
        let mut overrides: Vec<SlotOverride> = Vec::new();
        for (n, (raw, at, kind, derived, label, width)) in texts.into_iter().enumerate() {
            let (text, style, em) =
                items::qet_text(&raw, at.font.as_deref(), &mut self.out.fonts, self.findings);
            let Some((across, align)) = items::line_place(width) else {
                continue;
            };
            let slot_align = if align == TextAlign::Centre {
                Align::Centre
            } else {
                INSTANCE_ALIGN
            };
            if label
                && let Some(p) = place.take()
                && let Some(middle) = items::box_middle(width, &text, em)
            {
                self.xref_below(
                    e,
                    at,
                    (text.lines().count().max(1), em),
                    middle,
                    p,
                    &mut overrides,
                );
            }
            if derived
                && let Some(kind) = &kind
                && let Some((local, angle)) = items::text_anchor_across(at, 0, em, across)
                && let Some(slot) = self
                    .slot_for(e, kind, style.clone(), slot_align.clone())
                    .filter(|i| !overrides.iter().any(|o| o.slot == *i))
            {
                overrides.push(shown(slot, local, angle));
                continue;
            }
            if text.trim().is_empty() {
                continue;
            }
            // A slot prints one line in its own style, so only a one-line text of that style shows it; a line
            // aligned otherwise than the symbol's slots takes a slot of its own alignment.
            let slot = match kind.filter(|_| live && text.lines().count() == 1) {
                Some(k) if slot_align == INSTANCE_ALIGN => e
                    .symbol()
                    .item
                    .text_slots
                    .iter()
                    .position(|s| s.kind == k && s.style == style)
                    .and_then(|i| u16::try_from(i).ok()),
                Some(k) => self.slot_for(e, &k, style.clone(), slot_align.clone()),
                None => None,
            }
            .filter(|i| !overrides.iter().any(|o| o.slot == *i));
            for (i, line) in text.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let Some((local, angle)) = items::text_anchor_across(at, i, em, across) else {
                    self.at(
                        e,
                        Code::MALFORMED_ITEM,
                        Severity::Warning,
                        format!("element text {n} skipped: bad x/y"),
                    );
                    break;
                };
                if let Some(slot) = slot {
                    overrides.push(SlotOverride {
                        slot,
                        pos: Some(local),
                        angle: Some(angle),
                        visible: Some(true),
                        extra: BTreeMap::new(),
                    });
                    continue;
                }
                let turned = e.orient.apply(local);
                let (Some(x), Some(y)) = (
                    e.pos.x.0.checked_add(turned.x.0),
                    e.pos.y.0.checked_add(turned.y.0),
                ) else {
                    continue;
                };
                let turns = i32::from(e.orient.rotation()) * 1000;
                let key = items::line_key(&format!("qet:el:{}#text{n}", e.ek), i);
                let at = Pt2::new(Nm(x), Nm(y));
                let angle = Angle2(angle.0.saturating_sub(turns));
                self.out.records.push(Record::PageItem(ew_schema::PageItem {
                    id: self.inp.ids.id(&key),
                    v: 1,
                    page,
                    layer: None,
                    kind: items::free_text(line.trim_end(), at, angle, style.clone(), align.clone()),
                    extra: BTreeMap::new(),
                }));
            }
        }
        KeyedList::from_vec(overrides).unwrap_or_default()
    }

    /// The index of a hidden slot of `kind` in `style` aligned `align` on the symbol of `e`: the symbol's own,
    /// else one added to [`ElementsOut::extra_slots`] (which every element of that symbol shares); `None` past
    /// `u16` slots.
    fn slot_for(&mut self, e: &Elem<'_>, kind: &SlotKind, style: Option<Ident>, align: Align) -> Option<u16> {
        let fits = |s: &TextSlot| s.kind == *kind && s.style == style && s.align == align && !s.visible;
        let own = &e.symbol().item.text_slots;
        if let Some(i) = own.iter().position(fits) {
            return u16::try_from(i).ok();
        }
        let extra = self.out.extra_slots.entry(e.symbol().id.as_uuid()).or_default();
        let i = match extra.iter().position(fits) {
            Some(i) => i,
            None => {
                extra.push(TextSlot {
                    kind: kind.clone(),
                    pos: Pt2::new(Nm(0), Nm(0)),
                    rot: Angle2::default(),
                    align,
                    visible: false,
                    max_width: None,
                    style,
                    fallback: None,
                    extra: BTreeMap::new(),
                });
                extra.len() - 1
            }
        };
        u16::try_from(own.len() + i).ok()
    }

    /// Shows the cross-reference slot of master or slave `e` where QET draws it against its label text `at`
    /// (`lines` lines of em `em`): centred under (or above) the box's middle `middle` nm from its left edge, as
    /// `place` says, in QET's 5 pt.
    fn xref_below(
        &mut self,
        e: &Elem<'_>,
        at: &TextAt,
        lines: (usize, Nm),
        middle: i128,
        place: XrefPlace,
        overrides: &mut Vec<SlotOverride>,
    ) {
        let style = Ident::from_input(&crate::prims::text::style_key(
            Decimal::from_int(XREF_POINT_SIZE),
            false,
            false,
        ))
        .ok();
        let align = if place.above {
            Align::BottomCentre
        } else {
            Align::TopCentre
        };
        let Some(middle) = place
            .right
            .px_to_nm()
            .ok()
            .and_then(|r| middle.checked_add(i128::from(r.0)))
        else {
            return;
        };
        if let Some((local, angle)) = items::below_text(at, lines, middle, place.gap, place.above)
            && let Some(slot) = self
                .slot_for(e, &SlotKind::Xref, style, align)
                .filter(|i| !overrides.iter().any(|o| o.slot == *i))
        {
            overrides.push(shown(slot, local, angle));
        }
    }

    /// Reports `e` as dropped and registers it so that its wires are reported as lost.
    pub(crate) fn drop_element(&mut self, e: &Elem<'_>, why: &str) {
        self.at(
            e,
            Code::UNRESOLVED_REFERENCE,
            Severity::Error,
            format!("element dropped: {why}"),
        );
        self.out.dropped += 1;
        let page = self.inp.pages.pages[e.diagram].id;
        let folio = self.folio(e.diagram).to_owned();
        self.out.registry.insert(
            (e.diagram, e.key.clone()),
            Placed {
                ek: e.ek.clone(),
                diagram: e.diagram,
                page,
                folio,
                target: Target::Dropped,
                terminals: e.terminals(),
            },
        );
    }
}

/// Where QET's first column and row start, px: the border sits 5 px from the scene origin
/// (`Diagram::margin`) and its header bands are 20 px wide (`BorderTitleBlock`), so column `k` starts at
/// `25 + k × colsize` and row `k` at `25 + k × rowsize` (the frame of doc 14 §4.2 as built).
const QET_GRID_ORIGIN_PX: i64 = 25;

/// How far an element lying on a frame line moves up or left, in nm: one past the 1 nm window in which the
/// cell rule of `ew-derive` reads a coordinate as the exact grid position.
pub(crate) const LINE_NUDGE_NM: i64 = 2;

/// The nudge `(dx, dy)` in nm (each 0 or [`LINE_NUDGE_NM`], to subtract) of an element of link type
/// `link_type` at QET position `(x, y)` px on a diagram with border `b`. QET's `convertPosition` takes the
/// cell index as the ceiling of `(v − 25) / size`, so a point exactly on an inner column or row line belongs
/// to the left column or the upper row, while EW's cell rule (floor) puts it right or below. Moving such an
/// element a few nm left or up keeps its printed cross-reference cell as QET prints it (the derivation's
/// cell rule stays).
///
/// Only the elements whose cell a cross-reference prints move: **interruption points** (`next_report`,
/// `previous_report`) and **masters and slaves** (`master`, `slave`, whose cells the slave and master
/// cross-references print). Any other element (terminals, simple elements) keeps its exact position, so
/// its connection points stay on the 10 px grid (FR-IMP-008, doc 14 §4.2: 0 nm deviation). Only on QET's default border, the one the QET
/// frame form reproduces; the frame edges are not moved. A cell-boundary policy for imported projects is
/// the proper home of this rule (an ADR-0027 item; follow-up task by the design pass).
pub(crate) fn qet_cell_nudge(link_type: &str, x: Decimal, y: Decimal, b: &Border) -> (i64, i64) {
    if !matches!(link_type, "next_report" | "previous_report" | "master" | "slave") || !is_qet_border(b) {
        return (0, 0);
    }
    let on_line = |v: Decimal, size: Option<Decimal>, count: Option<u32>| -> bool {
        let (Some(size), Some(count)) = (size, count) else {
            return false;
        };
        (1..count).any(|k| {
            Decimal::from_int(i64::from(k))
                .checked_mul(size)
                .and_then(|l| l.checked_add(Decimal::from_int(QET_GRID_ORIGIN_PX)))
                .is_ok_and(|line| line == v)
        })
    };
    let nudge = |hit: bool| if hit { LINE_NUDGE_NM } else { 0 };
    (
        nudge(on_line(x, b.colsize, b.cols)),
        nudge(on_line(y, b.rowsize, b.rows)),
    )
}

/// Maps the placed elements.
pub fn map(inp: &Inputs<'_>, used: &mut BTreeSet<String>, findings: &mut Findings) -> ElementsOut {
    let mut mx = Mx {
        inp,
        findings,
        out: ElementsOut::default(),
        infos_seen: BTreeMap::new(),
        live: BTreeSet::new(),
        master_label: BTreeMap::new(),
        xref_place: BTreeMap::new(),
    };

    // Pass 1: placed elements with a usable definition and position, and their project-wide keys (doc 14
    // §2.3): the UUID; `<uuid>#<folio key>#<n>` for the n-th further copy of a UUID on a folio (EW-IMP-120);
    // `<folio key>#el#<n>` for an element without UUID at source position n.
    let mut elems: Vec<Elem<'_>> = Vec::new();
    let mut by_uuid: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut uuid_seen: BTreeSet<&str> = BTreeSet::new();
    let mut repeats: BTreeMap<(&str, usize), u32> = BTreeMap::new();
    for d in &inp.project.diagrams {
        if d.index >= inp.pages.pages.len() {
            continue;
        }
        let folio_key = inp.pages.pages[d.index].key.clone();
        let keys = element_keys(d);
        for (el, key) in d.elements.iter().zip(keys) {
            let ek = match el.uuid.as_deref() {
                None => format!("{folio_key}#el#{}", el.n),
                Some(u) if uuid_seen.insert(u) => u.to_owned(),
                Some(u) => {
                    let n = repeats.entry((u, d.index)).or_insert(0);
                    *n += 1;
                    let ek = format!("{u}#{folio_key}#{n}");
                    let folio = mx.folio(d.index).to_owned();
                    let loc = mx.findings.loc(Some(&folio), Some(ek.clone()), Some(el.line));
                    mx.findings.push(
                        Code::imp(120),
                        Severity::Info,
                        loc,
                        "element UUID shared with an earlier element (copy-paste); disambiguated by folio and occurrence",
                    );
                    ek
                }
            };
            let import = inp
                .defs
                .get(el.type_path.as_str())
                .and_then(|def| def.import.as_ref());
            let link_type = import.map_or("", |i| i.link_type.as_str());
            let (dx, dy) = qet_cell_nudge(link_type, el.x, el.y, &d.border);
            let pos =
                el.x.px_to_nm()
                    .ok()
                    .zip(el.y.px_to_nm().ok())
                    .map(|(x, y)| Pt2::new(Nm(x.0 - dx), Nm(y.0 - dy)));
            let (Some(import), Some(pos)) = (import, pos) else {
                let folio = mx.folio(d.index).to_owned();
                let loc = mx.findings.loc(Some(&folio), Some(ek.clone()), Some(el.line));
                let why = if pos.is_none() {
                    "position out of range".to_owned()
                } else {
                    format!("element definition `{}` not found or refused", el.type_path)
                };
                mx.findings.push(
                    Code::UNRESOLVED_REFERENCE,
                    Severity::Error,
                    loc,
                    format!("element dropped: {why}"),
                );
                mx.out.dropped += 1;
                mx.out.registry.insert(
                    (d.index, key.clone()),
                    Placed {
                        ek,
                        diagram: d.index,
                        page: inp.pages.pages[d.index].id,
                        folio,
                        target: Target::Dropped,
                        terminals: Vec::new(),
                    },
                );
                continue;
            };
            if let Some(u) = &el.uuid {
                by_uuid.entry(u.as_str()).or_default().push(elems.len());
            }
            elems.push(Elem {
                diagram: d.index,
                key,
                ek,
                el,
                path: el.type_path.as_str(),
                import,
                pos,
                orient: orientation(el.orientation),
            });
        }
    }

    // Classification.
    let classes: Vec<Class> = elems
        .iter()
        .map(|e| match e.import.link_type.as_str() {
            "slave" => {
                e.el.links
                    .iter()
                    .filter_map(|u| linked(&elems, &by_uuid, e, u))
                    .find(|m| matches!(m.import.link_type.as_str(), "master" | "simple"))
                    .map_or(Class::OrphanSlave, |m| Class::Slave {
                        master: (m.diagram, m.key.clone()),
                    })
            }
            "next_report" => Class::Report { source: true },
            "previous_report" => Class::Report { source: false },
            "terminal" => Class::Terminal,
            "thumbnail" => Class::Graphic,
            _ if e.terminal_count() == 1
                && e.el.info_nonempty("label").is_none()
                && e.import
                    .fndef
                    .as_ref()
                    .is_some_and(|_| e.import.link_type == "simple")
                && inp
                    .counts
                    .get(&(e.diagram, e.key.clone()))
                    .is_some_and(|n| *n >= 3) =>
            {
                Class::Junction
            }
            _ => Class::Device,
        })
        .collect();

    // Where masters and linked slaves print their cross-references: a master directly under its label, a
    // slave as the `<xref>` of its master's type says (QET's defaults when the project has none for it).
    let index_of: BTreeMap<ElementKey, usize> = elems
        .iter()
        .enumerate()
        .map(|(i, e)| ((e.diagram, e.key.clone()), i))
        .collect();
    for (e, c) in elems.iter().zip(&classes) {
        let master = match c {
            Class::Device if e.import.link_type == "master" => e,
            Class::Slave { master } => match index_of.get(master) {
                Some(&i) => &elems[i],
                None => continue,
            },
            _ => continue,
        };
        let props = inp.project.xrefs.iter().find(|x| x.kind == kind_type(master));
        let place = if matches!(c, Class::Slave { .. }) {
            XrefPlace {
                gap: props.map_or(Decimal::ZERO, |x| x.slave_offset),
                above: props.is_some_and(|x| x.slave_pos == "AlignTop"),
                right: Decimal::ZERO,
            }
        } else {
            XrefPlace {
                gap: Decimal::ZERO,
                above: false,
                right: if props.is_some_and(|x| x.display == "contacts") {
                    Decimal::from_int(CONTACT_TEXT_SHIFT_PX)
                } else {
                    Decimal::ZERO
                },
            }
        };
        mx.xref_place.insert(e.ek.clone(), place);
    }

    // Pass 2: devices (masters first, so slaves find their device).
    let mut unlabeled: BTreeMap<usize, u32> = BTreeMap::new();
    let mut device_of: BTreeMap<ElementKey, Id<Device>> = BTreeMap::new();
    for (e, c) in elems.iter().zip(&classes) {
        match c {
            Class::Device | Class::OrphanSlave => {
                mx.plain_device(e, &mut unlabeled, matches!(c, Class::OrphanSlave));
                if matches!(c, Class::Device) {
                    device_of.insert(
                        (e.diagram, e.key.clone()),
                        inp.ids.id(&format!("qet:el:{}", e.ek)),
                    );
                }
            }
            Class::Junction => {
                let jkey = format!("qet:junction:{}", e.ek);
                let terminals = e.terminals();
                let at = terminals.first().and_then(|t| t.page_pos).unwrap_or(e.pos);
                mx.out.registry.insert(
                    (e.diagram, e.key.clone()),
                    Placed {
                        ek: e.ek.clone(),
                        diagram: e.diagram,
                        page: inp.pages.pages[e.diagram].id,
                        folio: mx.folio(e.diagram).to_owned(),
                        target: Target::Junction {
                            id: inp.ids.id(&jkey),
                            key: jkey,
                            at,
                        },
                        terminals,
                    },
                );
            }
            Class::Graphic => {
                mx.at(
                    e,
                    Code::DROPPED_BY_DESIGN,
                    Severity::Info,
                    "thumbnail element is a graphic; not imported (no page-item records yet)",
                );
                let folio = mx.folio(e.diagram).to_owned();
                mx.out.registry.insert(
                    (e.diagram, e.key.clone()),
                    Placed {
                        ek: e.ek.clone(),
                        diagram: e.diagram,
                        page: inp.pages.pages[e.diagram].id,
                        folio,
                        target: Target::Dropped,
                        terminals: e.terminals(),
                    },
                );
            }
            _ => {}
        }
    }

    // Pass 3: slaves.
    let label_of: BTreeMap<ElementKey, &str> = elems
        .iter()
        .filter_map(|m| Some(((m.diagram, m.key.clone()), m.el.info_nonempty("label")?)))
        .collect();
    for (e, c) in elems.iter().zip(&classes) {
        let Class::Slave { master } = c else { continue };
        if let Some(l) = label_of.get(master) {
            mx.master_label.insert(e.ek.clone(), (*l).to_owned());
        }
        let (Some(device), Some(def)) = (device_of.get(master).copied(), Mx::generated_def(e)) else {
            mx.drop_element(e, "its master or its function definition was not imported");
            continue;
        };
        let mut extra = BTreeMap::new();
        if let Some(v) = info_extra(e.el, &mut mx.infos_seen) {
            extra.insert("qet.info".to_owned(), v);
        }
        let mut spec = FnSpec::normal(e, def, device, FnRole::Aux);
        spec.extra = extra;
        mx.function_and_rep(e, spec);
    }

    // Pass 4: folio reports → interruption points.
    map_reports(&mut mx, &elems, &classes, &by_uuid, used);

    // Pass 5: terminal elements → strips and terminals.
    let terminals: Vec<&Elem<'_>> = elems
        .iter()
        .zip(&classes)
        .filter(|(_, c)| matches!(c, Class::Terminal))
        .map(|(e, _)| e)
        .collect();
    strips::map(&mut mx, &terminals);

    for (d, n) in unlabeled {
        let folio = mx.folio(d).to_owned();
        let loc = mx.findings.loc(Some(&folio), None, None);
        let text = if n == 1 {
            format!(
                "1 element without label: its device tag carries the element prefix letter (else `{FALLBACK_LETTER}`) and is numbered on import; the page prints no tag for it, as QET prints none"
            )
        } else {
            format!(
                "{n} elements without label: device tags carry the element prefix letter (else `{FALLBACK_LETTER}`) and are numbered on import; the page prints no tag for them, as QET prints none"
            )
        };
        mx.findings.push(UNLABELED, Severity::Info, loc, text);
    }
    for (name, n) in std::mem::take(&mut mx.infos_seen) {
        mx.findings.file_level(
            Code::FOREIGN_VALUE,
            Severity::Warning,
            format!("element information `{name}` kept as text on {n} elements (not mapped to part, data or structure fields)"),
        );
    }
    mx.out
}

/// Folio reports → interruption points (doc 14 §4.6).
///
/// 1. **Pairs** from the links between `next_report` (source) and `previous_report` (target) elements; a
///    report without a linked partner of the other direction is an unpaired interruption point.
/// 2. **Name** of a pair: the first non-empty of the source's first `UserText` dynamic text, the target's
///    (the name QET prints; its label text prints the partner's position), the source's label, the
///    target's; both ends get it. When both ends carry a name and the names differ, EW-IMP-115 lists both texts. A pair without any
///    name gets one synthesised name `IP-<folio of the source>-<n>`, an unpaired report its own; each is
///    reported (EW-IMP-115).
/// 3. **Groups**: a name shared by several pairs keeps QET's explicit pairs in chain order: pair `i` of the
///    name (document order of the sources) gets `IpSpec.sort` 2i on the source and 2i + 1 on the target,
///    the project pairs in chain mode ([`ElementsOut::chain_pairs`]) and EW-IMP-115 says so.
///
/// An unlinked report keeps its own text as its name only when no other report has that name; otherwise it
/// gets a synthesised name of its own (EW-IMP-115), so that it stays unpaired, as in QET.
fn map_reports(
    mx: &mut Mx<'_, '_>,
    elems: &[Elem<'_>],
    classes: &[Class],
    by_uuid: &BTreeMap<&str, Vec<usize>>,
    used: &mut BTreeSet<String>,
) {
    let reports: Vec<(usize, bool)> = classes
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c {
            Class::Report { source } => Some((i, *source)),
            _ => None,
        })
        .collect();
    if reports.is_empty() {
        return;
    }
    let Some(ip) = mx.inp.builtins.fndef(crate::IP_FNDEF_KEY).map(|c| c.id) else {
        for (i, _) in reports {
            mx.drop_element(
                &elems[i],
                "folio report needs the built-in interruption point `ew.fn.ip`, which is not available",
            );
        }
        return;
    };
    used.insert(crate::IP_FNDEF_KEY.to_owned());
    let is_source: BTreeMap<usize, bool> = reports.iter().copied().collect();
    // 1. Pairs, each report in at most one.
    let mut partner: BTreeMap<usize, usize> = BTreeMap::new();
    for &(s, source) in &reports {
        if !source || partner.contains_key(&s) {
            continue;
        }
        let t = elems[s]
            .el
            .links
            .iter()
            .filter_map(|u| by_uuid.get(u.as_str()))
            .flat_map(|idxs| {
                let same: Vec<usize> = idxs
                    .iter()
                    .copied()
                    .filter(|&i| elems[i].diagram == elems[s].diagram)
                    .collect();
                if same.is_empty() { idxs.clone() } else { same }
            })
            .find(|i| is_source.get(i) == Some(&false) && !partner.contains_key(i));
        if let Some(t) = t {
            partner.insert(s, t);
            partner.insert(t, s);
        }
    }
    let label = |i: usize| elems[i].el.info_nonempty("label").map(str::to_owned);
    let utext = |i: usize| user_text(elems[i].el).map(str::to_owned);
    // The name QET prints is a report's text; its label text prints the partner's position, so the label
    // information names a report only when it has no text.
    let own = |i: usize| utext(i).or_else(|| label(i));
    let valid = |n: Option<String>| n.and_then(|n| Ident::from_input(&n).ok());

    // 2. Names.
    let mut name_of: BTreeMap<usize, Ident> = BTreeMap::new();
    // Per folio: the names synthesised, and how many of them for reports without any text.
    let mut synthesized: BTreeMap<usize, u32> = BTreeMap::new();
    let mut nameless: BTreeMap<usize, u32> = BTreeMap::new();
    let mut synth = |mx: &Mx<'_, '_>, d: usize, without_text: bool| -> Ident {
        let n = synthesized.entry(d).or_default();
        *n += 1;
        if without_text {
            *nameless.entry(d).or_default() += 1;
        }
        Ident::from_input(&format!("IP-{}-{n}", mx.folio(d)))
            .unwrap_or_else(|_| Ident::from_input("IP").unwrap_or_else(|_| unreachable!("an identifier")))
    };
    let mut pair_order: Vec<(usize, usize)> = Vec::new();
    for &(s, source) in &reports {
        if !source {
            continue;
        }
        let Some(&t) = partner.get(&s) else { continue };
        pair_order.push((s, t));
        // The first usable text in the order source text, target text, source label, target label; a text
        // that is not an identifier is skipped, not the rest of the order.
        let name = [utext(s), utext(t), label(s), label(t)]
            .into_iter()
            .flatten()
            .find_map(|n| Ident::from_input(&n).ok())
            .unwrap_or_else(|| synth(mx, elems[s].diagram, true));
        if let (Some(a), Some(b)) = (own(s), own(t))
            && a != b
        {
            mx.at(
                &elems[s],
                Code::imp(115),
                Severity::Warning,
                format!(
                    "folio-report pair names differ: source `{a}`, target `{b}`; the pair is named `{}`",
                    name.as_str()
                ),
            );
        }
        name_of.insert(s, name.clone());
        name_of.insert(t, name);
    }
    // An unlinked report stays unpaired, as in QET: it keeps its own text as its name only when no other
    // report has that name, else it gets a name of its own and its text prints as written.
    let paired_names: BTreeSet<String> = name_of.values().map(|n| n.as_str().to_owned()).collect();
    let mut unlinked_names: BTreeMap<String, u32> = BTreeMap::new();
    for &(i, _) in &reports {
        if !name_of.contains_key(&i)
            && let Some(n) = valid(own(i))
        {
            *unlinked_names.entry(n.as_str().to_owned()).or_default() += 1;
        }
    }
    for &(i, _) in &reports {
        if name_of.contains_key(&i) {
            continue;
        }
        let name = match valid(own(i)) {
            Some(n) if !paired_names.contains(n.as_str()) && unlinked_names.get(n.as_str()) == Some(&1) => n,
            Some(n) => {
                let name = synth(mx, elems[i].diagram, false);
                mx.at(
                    &elems[i],
                    Code::imp(115),
                    Severity::Warning,
                    format!(
                        "folio report `{}` is linked to no other report in QET but shares its name with other reports; named `{}` so that it stays unpaired",
                        n.as_str(),
                        name.as_str()
                    ),
                );
                name
            }
            None => synth(mx, elems[i].diagram, true),
        };
        name_of.insert(i, name);
    }

    // 3. Chain order for names shared by several pairs.
    let mut sort_of: BTreeMap<usize, i32> = BTreeMap::new();
    let mut pairs_per_name: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
    for &(s, t) in &pair_order {
        pairs_per_name
            .entry(name_of[&s].as_str().to_owned())
            .or_default()
            .push((s, t));
    }
    for (name, pairs) in &pairs_per_name {
        if pairs.len() < 2 {
            continue;
        }
        for (k, &(s, t)) in pairs.iter().enumerate() {
            let k = i32::try_from(k).unwrap_or(i32::MAX / 2 - 1);
            sort_of.insert(s, 2 * k);
            sort_of.insert(t, 2 * k + 1);
        }
        mx.out.chain_pairs = true;
        mx.findings.file_level(
            Code::imp(115),
            Severity::Warning,
            format!(
                "interruption-point name `{name}` has {} explicit QET pairs: chain order set (sort 2i, 2i + 1) and the project pairs in chain mode",
                pairs.len()
            ),
        );
    }

    // Emit.
    for &(i, source) in &reports {
        let e = &elems[i];
        if e.terminal_count() != 1 {
            mx.at(
                e,
                Code::MALFORMED_ITEM,
                Severity::Warning,
                format!(
                    "folio report with {} terminals; only the first is the interruption point's connection point",
                    e.terminal_count()
                ),
            );
        }
        let mut cps: Vec<Option<u16>> = vec![None; e.terminal_count()];
        if let Some(first) = cps.first_mut() {
            *first = Some(0);
        }
        mx.function_and_rep(
            e,
            FnSpec {
                def: ip,
                device: None,
                role: FnRole::Main,
                kind: FunctionKind::Ip(IpSpec {
                    name: name_of[&i].clone(),
                    direction: if source {
                        IpDirection::Source
                    } else {
                        IpDirection::Target
                    },
                    star_source: false,
                    sort: sort_of.get(&i).copied(),
                    external: None,
                    extra: BTreeMap::new(),
                }),
                cps,
                extra: BTreeMap::new(),
            },
        );
    }
    for (d, n) in nameless {
        let folio = mx.folio(d).to_owned();
        let loc = mx.findings.loc(Some(&folio), None, None);
        mx.findings.push(
            Code::imp(115),
            Severity::Warning,
            loc,
            format!("{n} folio-report names synthesised as IP-{folio}-<n> (no label or text on either end)"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(label: &str, folio: &str) -> (Option<u32>, Option<String>, String, TagOutcome) {
        let (t, _, o) = infer_tag(label, folio, "");
        (
            t.counter,
            t.frozen.as_ref().map(|f| f.text.as_str().to_owned()),
            t.letter.as_str().to_owned(),
            o,
        )
    }

    fn qet_border() -> Border {
        Border {
            cols: Some(17),
            colsize: Some(Decimal::from_int(60)),
            rows: Some(8),
            rowsize: Some(Decimal::from_int(80)),
            ..Border::default()
        }
    }

    fn px(s: &str) -> Decimal {
        Decimal::parse(s).unwrap()
    }

    #[test]
    fn an_element_on_a_row_line_moves_into_the_upper_row() {
        let b = qet_border();
        // y = 265 px = 25 + 3 × 80 lies on the line between rows C and D; QET prints C.
        assert_eq!(
            qet_cell_nudge("next_report", px("135"), px("265"), &b),
            (0, LINE_NUDGE_NM)
        );
        assert_eq!(
            qet_cell_nudge("next_report", px("135"), px("585.0"), &b),
            (0, LINE_NUDGE_NM)
        );
        // Inside a row, on the frame edges (25, 665) and in the header band: no move.
        for y in ["266", "264.5", "25", "665", "10"] {
            assert_eq!(
                qet_cell_nudge("next_report", px("135"), px(y), &b),
                (0, 0),
                "y = {y}"
            );
        }
    }

    #[test]
    fn an_element_on_a_column_line_moves_into_the_left_column() {
        let b = qet_border();
        // x = 925 px = 25 + 15 × 60 lies on the line between columns 15 and 16; QET prints 15.
        assert_eq!(
            qet_cell_nudge("next_report", px("925"), px("130"), &b),
            (LINE_NUDGE_NM, 0)
        );
        assert_eq!(
            qet_cell_nudge("next_report", px("925"), px("265"), &b),
            (LINE_NUDGE_NM, LINE_NUDGE_NM)
        );
        for x in ["926", "25", "1045", "0"] {
            assert_eq!(
                qet_cell_nudge("next_report", px(x), px("130"), &b),
                (0, 0),
                "x = {x}"
            );
        }
        // Another border has no QET frame form, so nothing moves.
        let other = Border {
            cols: Some(10),
            ..qet_border()
        };
        assert_eq!(
            qet_cell_nudge("next_report", px("925"), px("265"), &other),
            (0, 0)
        );
    }

    #[test]
    fn only_elements_with_a_printed_cell_move() {
        let b = qet_border();
        // A terminal, a simple element and an element without link type on the row line y = 265 px keep
        // their exact position, so their connection points stay on the 10 px grid.
        for link in ["terminal", "simple", ""] {
            assert_eq!(qet_cell_nudge(link, px("925"), px("265"), &b), (0, 0), "{link}");
        }
        // Interruption points, masters and slaves move: a cross-reference prints their cell.
        for link in ["previous_report", "next_report", "master", "slave"] {
            assert_eq!(
                qet_cell_nudge(link, px("925"), px("265"), &b),
                (LINE_NUDGE_NM, LINE_NUDGE_NM),
                "{link}"
            );
        }
    }

    /// The nudged coordinate lies outside the 1 nm snap window of the line and above it.
    #[test]
    fn the_nudge_leaves_the_snap_window_of_the_cell_rule() {
        for v in ["265", "585", "925"] {
            let rounded = i128::from(px(v).px_to_nm().unwrap().0);
            let exact_num = v.parse::<i128>().unwrap() * 793_750; // exact line = exact_num / 3 nm
            assert!(
                (rounded * 3 - exact_num).abs() <= 3,
                "{v}: the rounded line is in the window"
            );
            let nudged = rounded - i128::from(LINE_NUDGE_NM);
            assert!((nudged * 3 - exact_num).abs() > 3, "{v}: outside the window");
            assert!(nudged * 3 < exact_num, "{v}: above or left of the line");
        }
    }

    #[test]
    fn page_rule_labels_are_derived() {
        assert_eq!(
            tag("-101F1", "101"),
            (Some(1), None, "F".into(), TagOutcome::Derived)
        );
        assert_eq!(
            tag("-102KF12", "102"),
            (Some(12), None, "KF".into(), TagOutcome::Derived)
        );
    }

    #[test]
    fn other_labels_are_frozen_with_their_sign() {
        let (t, _, o) = infer_tag("S51", "101", "");
        assert_eq!(o, TagOutcome::Frozen);
        let f = t.frozen.unwrap();
        assert_eq!((f.text.as_str(), f.sign), ("S51", TagSign::Hidden));
        assert_eq!(t.letter.as_str(), "S");
        // A page rule label on another folio, and the strip rule, stay frozen with the sign shown.
        let (t, _, o) = infer_tag("-101F1", "102", "");
        assert_eq!((o, t.frozen.unwrap().sign), (TagOutcome::Frozen, TagSign::Shown));
        let (t, s, o) = infer_tag("+JE2416-X1031", "103", "");
        assert_eq!(o, TagOutcome::Frozen);
        assert_eq!(s, Some("+JE2416"));
        assert_eq!(t.frozen.unwrap().text.as_str(), "X1031");
        assert_eq!(tag("1X1", "101").3, TagOutcome::Frozen);
        assert_eq!(tag("1X1", "101").2, "X");
        // Labels EW would print differently stay frozen: leading zeros, lower-case letters.
        assert_eq!(tag("-101K01", "101").3, TagOutcome::Frozen);
        assert_eq!(tag("-101f1", "101").3, TagOutcome::Frozen);
        assert_eq!(tag("-01K1", "01").3, TagOutcome::Derived);
    }

    #[test]
    fn unlabeled_elements_take_the_prefix_letter() {
        let (t, _, o) = infer_tag("", "101", "K");
        assert_eq!(
            (t.letter.as_str(), t.counter, o),
            ("K", None, TagOutcome::Unlabeled)
        );
        let (t, _, _) = infer_tag("  ", "101", "%x");
        assert_eq!(t.letter.as_str(), FALLBACK_LETTER);
    }

    #[test]
    fn orientations_turn_clockwise() {
        assert_eq!(orientation(0), Orient2::R0);
        assert_eq!(orientation(1), Orient2::R90);
        assert_eq!(orientation(3), Orient2::R270);
    }
}
