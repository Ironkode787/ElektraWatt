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

use ew_ids::Id;
use ew_interop_api::{Code, Severity};
use ew_mltext::{Ident, MlText};
use ew_schema::{
    Device, DeviceAspects, FnRole, FrozenTag, Function, FunctionKind, IpDirection, IpSpec, LibCopy, Record,
    Representation, SymbolDef, TagScheme, TagSign, TagSpec,
};
use ew_units::{Nm, Orient2, Pt2};
use serde_json::Value;

use super::Def;
use super::builtins::Builtins;
use super::cx::{ElementKey, Findings, Ids, Placed, PlacedTerminal, Registry, Target, element_keys};
use super::model::{Border, QetElement, QetProject, norm_uuid};
use super::pages::{PagesOut, is_qet_border};
use super::strips;
use crate::decimal::Decimal;
use crate::element::ElementImport;

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
}

/// The fallback letter of an element without a usable label or `prefix` (EN 81346-2:2009 class A: no
/// main class identified). Recorded as an import mapping decision (crate README).
pub const FALLBACK_LETTER: &str = "A";

/// EW-IMP-122 (proposed, the next free code of the doc 14 §2.5 catalogue): element without label; its tag
/// keeps the prefix letter without counter and prints "?" (owner ruling, cycle 11 review m7).
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
            text_slots: Default::default(),
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
        match outcome {
            TagOutcome::Derived => {}
            TagOutcome::Frozen => self.at(
                e,
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
                self.at(
                    e,
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
        let dkey = format!("qet:el:{}", e.ek);
        let device = self.device(&dkey, tag, function_text, None, DeviceAspects::default(), extra);
        self.function_and_rep(e, FnSpec::normal(e, def, device, FnRole::Main));
        if orphan {
            self.at(
                e,
                Code::imp(116),
                Severity::Warning,
                "slave element without a linked master: own device, function promoted to main",
            );
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

/// The width of QET's frame headers, px: the row header on the left and the column header on top
/// (`BorderTitleBlock`), so column `k` starts at `20 + k × colsize` and row `k` at `20 + k × rowsize`.
const QET_HEADER_PX: i64 = 20;

/// How far an element lying on a frame line moves up or left, in nm: one past the 1 nm window in which the
/// cell rule of `ew-derive` reads a coordinate as the exact grid position.
pub(crate) const LINE_NUDGE_NM: i64 = 2;

/// The nudge `(dx, dy)` in nm (each 0 or [`LINE_NUDGE_NM`], to subtract) of an element of link type
/// `link_type` at QET position `(x, y)` px on a diagram with border `b`. QET's `convertPosition` takes the
/// cell index as the ceiling of `(v − 20) / size`, so a point exactly on an inner column or row line belongs
/// to the left column or the upper row, while EW's cell rule (floor) puts it right or below. Moving such an
/// element a few nm left or up keeps its printed cross-reference cell as QET prints it (owner rulings,
/// reviews of cycle 12; the derivation's cell rule stays).
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
                .and_then(|l| l.checked_add(Decimal::from_int(QET_HEADER_PX)))
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
                    "thumbnail element is a graphic (doc 14 §4.3); not imported (no page-item records yet)",
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
    for (e, c) in elems.iter().zip(&classes) {
        let Class::Slave { master } = c else { continue };
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
        mx.findings.push(
            UNLABELED,
            Severity::Info,
            loc,
            format!("{n} elements without label: device tags carry the element prefix letter (else `{FALLBACK_LETTER}`) and no counter, and print `?`"),
        );
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
/// 2. **Name** of a pair: the first non-empty of the source's label, the target's label, the source's first
///    `UserText` dynamic text, the target's (the reference set writes pairing names there); both ends get
///    it. When both ends carry a name and the names differ, EW-IMP-115 lists both texts. A pair without any
///    name gets one synthesised name `IP-<folio of the source>-<n>`, an unpaired report its own; each is
///    reported (EW-IMP-115).
/// 3. **Groups**: a name shared by several pairs keeps QET's explicit pairs in chain order: pair `i` of the
///    name (document order of the sources) gets `IpSpec.sort` 2i on the source and 2i + 1 on the target,
///    and EW-IMP-115 reports that the pairing needs chain mode.
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
    let own = |i: usize| label(i).or_else(|| utext(i));
    let valid = |n: Option<String>| n.and_then(|n| Ident::from_input(&n).ok());

    // 2. Names.
    let mut name_of: BTreeMap<usize, Ident> = BTreeMap::new();
    let mut synthesized: BTreeMap<usize, u32> = BTreeMap::new();
    let mut synth = |mx: &Mx<'_, '_>, d: usize| -> Ident {
        let n = synthesized.entry(d).or_default();
        *n += 1;
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
        // The first usable text in the order source label, target label, source text, target text; a text
        // that is not an identifier is skipped, not the rest of the order.
        let name = [label(s), label(t), utext(s), utext(t)]
            .into_iter()
            .flatten()
            .find_map(|n| Ident::from_input(&n).ok())
            .unwrap_or_else(|| synth(mx, elems[s].diagram));
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
    for &(i, _) in &reports {
        if let std::collections::btree_map::Entry::Vacant(v) = name_of.entry(i) {
            v.insert(valid(own(i)).unwrap_or_else(|| synth(mx, elems[i].diagram)));
        }
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
        mx.findings.file_level(
            Code::imp(115),
            Severity::Warning,
            format!(
                "interruption-point name `{name}` has {} explicit QET pairs: chain order set (sort 2i, 2i + 1); the project needs chain mode",
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
    for (d, n) in synthesized {
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
            height: None,
        }
    }

    fn px(s: &str) -> Decimal {
        Decimal::parse(s).unwrap()
    }

    #[test]
    fn an_element_on_a_row_line_moves_into_the_upper_row() {
        let b = qet_border();
        // y = 260 px = 20 + 3 × 80 lies on the line between rows C and D; QET prints C.
        assert_eq!(
            qet_cell_nudge("next_report", px("135"), px("260"), &b),
            (0, LINE_NUDGE_NM)
        );
        assert_eq!(
            qet_cell_nudge("next_report", px("135"), px("580.0"), &b),
            (0, LINE_NUDGE_NM)
        );
        // Inside a row, on the frame edges (20, 660) and in the header band: no move.
        for y in ["261", "259.5", "20", "660", "10"] {
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
        // x = 920 px = 20 + 15 × 60 lies on the line between columns 15 and 16; QET prints 15.
        assert_eq!(
            qet_cell_nudge("next_report", px("920"), px("130"), &b),
            (LINE_NUDGE_NM, 0)
        );
        assert_eq!(
            qet_cell_nudge("next_report", px("920"), px("260"), &b),
            (LINE_NUDGE_NM, LINE_NUDGE_NM)
        );
        for x in ["921", "20", "1040", "0"] {
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
            qet_cell_nudge("next_report", px("920"), px("260"), &other),
            (0, 0)
        );
    }

    #[test]
    fn only_elements_with_a_printed_cell_move() {
        let b = qet_border();
        // A terminal, a simple element and an element without link type on the row line y = 260 px keep
        // their exact position, so their connection points stay on the 10 px grid.
        for link in ["terminal", "simple", ""] {
            assert_eq!(qet_cell_nudge(link, px("920"), px("260"), &b), (0, 0), "{link}");
        }
        // Interruption points, masters and slaves move: a cross-reference prints their cell.
        for link in ["previous_report", "next_report", "master", "slave"] {
            assert_eq!(
                qet_cell_nudge(link, px("920"), px("260"), &b),
                (LINE_NUDGE_NM, LINE_NUDGE_NM),
                "{link}"
            );
        }
    }

    /// The nudged coordinate lies outside the 1 nm snap window of the line and above it.
    #[test]
    fn the_nudge_leaves_the_snap_window_of_the_cell_rule() {
        for v in ["260", "580", "920"] {
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
