// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Devices, terminals, functions and representations (doc 20 §6, §9, §15; doc 07 §9.1).

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{
    AnchorPolicy, Cable, CpOverride, Device, DeviceAspects, DeviceKind, DeviceState, FnRole, FrozenTag,
    Function, FunctionKind, IpDirection, IpSpec, KeyedList, LabelMode, LengthSource, PartRef, PartUse,
    Record, Representation, Side, Strip, TagPrint, TagScheme, TagSign, TagSpec, Terminal, TerminalGroup,
};
use ew_units::{Dec3, Pt2};

use super::project::structure_assignment;
use super::{CpInfo, Cx, DeviceInfo, FnInfo, cell_point, dec3_mm_to_nm, fresh, parse_cell, place_on_grid};
use crate::ids::{record_id, source_ref};
use crate::table::{Rd, Row, Table, parse_dec3, parse_ident};

const STATES: &[(&str, DeviceState)] = &[
    ("used", DeviceState::Used),
    ("spare", DeviceState::Spare),
    ("reserved", DeviceState::Reserved),
    ("not_fitted", DeviceState::NotFitted),
];

const KINDS: &[(&str, DeviceKind)] = &[
    ("psu", DeviceKind::Psu),
    ("blanking_plate", DeviceKind::BlankingPlate),
];

const SIDES: &[(&str, Side)] = &[("side1", Side::Side1), ("side2", Side::Side2)];

/// `parts` cells: `<part>` or `<part>*<qty>`.
fn part_uses(cx: &Cx<'_>, r: &mut Rd<'_>) -> Vec<PartUse> {
    let mut out = Vec::new();
    for item in r.list("parts") {
        let (key, qty) = match item.rsplit_once('*') {
            Some((k, q)) => match parse_dec3(q) {
                Ok(q) => (k, q),
                Err(e) => {
                    r.fail(format!("column `parts`: {e}"));
                    continue;
                }
            },
            None => (item, Dec3(1_000)),
        };
        match cx.parts.get(key) {
            Some(p) => out.push(PartUse {
                part: PartRef::new(p.id),
                qty,
                extra: Default::default(),
            }),
            None => r.fail(format!("column `parts`: unknown part `{key}`")),
        }
    }
    out
}

fn tag_scheme(r: &mut Rd<'_>) -> TagScheme {
    match r.text("scheme") {
        None | Some("default") => TagScheme::Default,
        Some("child") => TagScheme::Child,
        Some(s) => match s.strip_prefix("named:").map(parse_ident) {
            Some(Ok(name)) => TagScheme::Named(name),
            _ => {
                r.fail(format!(
                    "column `scheme`: `{s}` is not default, child or named:<name>"
                ));
                TagScheme::Default
            }
        },
    }
}

/// `strip_groups` cells: `<from>-<to>:<caption>` items.
fn strip_groups(r: &mut Rd<'_>) -> Vec<TerminalGroup> {
    let mut out = Vec::new();
    for item in r.list("strip_groups") {
        let parsed = item.split_once(':').and_then(|(range, label)| {
            let (a, b) = range.split_once('-')?;
            let (a, b) = (a.parse::<u32>().ok()?, b.parse::<u32>().ok()?);
            (a <= b && !label.is_empty()).then_some((a, b, label))
        });
        match parsed {
            Some((from_seq, to_seq, label)) => out.push(TerminalGroup {
                from_seq,
                to_seq,
                label: ew_mltext::MlText::neutral(label),
                extra: Default::default(),
            }),
            None => r.fail(format!(
                "column `strip_groups`: `{item}` is not <from>-<to>:<caption>"
            )),
        }
    }
    out
}

/// `devices.csv` → `device` (strips and cables with their aspects). Parents are resolved once every key is
/// known; parent cycles are refused (EW-INV-009).
pub(crate) fn emit_devices(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut pending: Vec<(Row<'_>, Option<String>, Device)> = Vec::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &cx.devices) {
            continue;
        }
        let mut r = row.reader();
        let letter = r.req_ident("letter");
        let counter = r.uint::<u32>("counter");
        let scheme = tag_scheme(&mut r);
        let frozen = r.ident("frozen");
        let sign = r
            .choice(
                "frozen_sign",
                &[("hidden", TagSign::Hidden), ("shown", TagSign::Shown)],
            )
            .unwrap_or_default();
        let parent = r.text("parent").map(str::to_owned);
        let position = r.uint::<u32>("position");
        let structure = structure_assignment(cx, &mut r);
        let kind = r.choice("kind", KINDS);
        let parts = part_uses(cx, &mut r);
        let function_text = r.mltext("function_text");
        let process_tag = r.ident("process_tag");
        let state = r.choice("state", STATES).unwrap_or_default();
        let is_strip = ["strip_label_mode", "strip_groups", "strip_field_side"]
            .iter()
            .any(|c| r.text(c).is_some());
        let strip = is_strip.then(|| Strip {
            label_mode: r
                .choice(
                    "strip_label_mode",
                    &[
                        ("designation", LabelMode::Designation),
                        ("function_text", LabelMode::FunctionText),
                    ],
                )
                .unwrap_or_default(),
            designation_scheme: None,
            accessories: Vec::new(),
            tag_print: TagPrint::PerSegment,
            groups: strip_groups(&mut r),
            field_side: r.choice("strip_field_side", SIDES).unwrap_or(Side::Side2),
            extra: Default::default(),
        });
        let cable = r.flag("cable").then(|| {
            let length = match r.text("cable_length") {
                None | Some("missing") => LengthSource::Missing,
                Some("from_part") => LengthSource::FromPart,
                Some(s) => match parse_dec3(s).ok().and_then(dec3_mm_to_nm) {
                    Some(nm) if nm.0 > 0 => LengthSource::Manual(nm),
                    _ => {
                        r.fail(format!(
                            "column `cable_length`: `{s}` is not missing, from_part or a length in mm"
                        ));
                        LengthSource::Missing
                    }
                },
            };
            Cable {
                length,
                core_id_mode: None,
                pre_assembled: false,
                extra: Default::default(),
            }
        });
        if strip.is_some() && cable.is_some() {
            r.fail("a device is a strip or a cable, not both (EW-INV-010)");
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let Some(letter) = letter else { continue };
        let id = record_id::<Device>("devices", row.key());
        cx.devices.insert(
            row.key().to_owned(),
            DeviceInfo {
                id,
                strip_field_side: strip.as_ref().map(|s| s.field_side.clone()),
                cable: cable.is_some(),
            },
        );
        let device = Device {
            id,
            v: 1,
            tag: TagSpec {
                letter,
                counter,
                scheme,
                frozen: frozen.map(|text| FrozenTag {
                    text,
                    sign,
                    extra: Default::default(),
                }),
                extra: Default::default(),
            },
            kind,
            parent: None,
            position,
            structure,
            parts,
            function_text,
            process_tag,
            tech_data: KeyedList::default(),
            state,
            source_ref: Some(source_ref("devices", row.key())),
            aspects: DeviceAspects {
                strip,
                terminal: None,
                cable,
            },
            extra: Default::default(),
        };
        pending.push((row, parent, device));
    }

    // Parents: resolve, then refuse cycles.
    let mut parent_of: BTreeMap<String, String> = BTreeMap::new();
    let mut dropped: BTreeSet<String> = BTreeSet::new();
    for (row, parent, _) in &pending {
        if let Some(p) = parent {
            if cx.devices.contains_key(p) && p != row.key() {
                parent_of.insert(row.key().to_owned(), p.clone());
            } else {
                cx.unresolved(*row, &format!("unknown parent `{p}`"));
                dropped.insert(row.key().to_owned());
            }
        }
    }
    for (row, _, _) in &pending {
        let mut cur = row.key().to_owned();
        let mut steps = 0;
        while let Some(p) = parent_of.get(&cur) {
            steps += 1;
            if p == row.key() || steps > parent_of.len() {
                cx.malformed(*row, "parent chain is a cycle (EW-INV-009)");
                dropped.insert(row.key().to_owned());
                break;
            }
            cur = p.clone();
        }
    }
    // A child of a dropped device is dropped too.
    loop {
        let more: Vec<String> = parent_of
            .iter()
            .filter(|(c, p)| dropped.contains(*p) && !dropped.contains(*c))
            .map(|(c, _)| c.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        dropped.extend(more);
    }
    for (row, _, mut device) in pending {
        if dropped.contains(row.key()) {
            cx.devices.remove(row.key());
            continue;
        }
        if let Some(p) = parent_of.get(row.key()) {
            device.parent = cx.devices.get(p).map(|d| d.id);
        }
        cx.records.push(Record::Device(device));
    }
}

/// `terminals.csv` → `device` with the terminal aspect. The key must be `<strip>:<designation>`.
pub(crate) fn emit_terminals(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &cx.devices) {
            continue;
        }
        let mut r = row.reader();
        let strip = r.req("strip").map(str::to_owned);
        let seq = r.req_uint::<u32>("seq");
        let designation = r.req_ident("designation");
        let function_text = r.mltext("function_text");
        let parts = part_uses(cx, &mut r);
        let state = r.choice("state", STATES).unwrap_or_default();
        let rail_bonded = r.flag("rail_bonded");
        let structure = structure_assignment(cx, &mut r);
        if let (Some(s), Some(d)) = (&strip, &designation)
            && row.key() != format!("{s}:{d}")
        {
            r.fail(format!("the key must be `{s}:{d}`"));
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(strip), Some(seq), Some(designation)) = (strip, seq, designation) else {
            continue;
        };
        let Some(strip_id) = cx
            .devices
            .get(&strip)
            .filter(|d| d.strip_field_side.is_some())
            .map(|d| d.id)
        else {
            cx.unresolved(row, &format!("`{strip}` is not a strip"));
            continue;
        };
        let id = record_id::<Device>("terminals", row.key());
        cx.devices.insert(
            row.key().to_owned(),
            DeviceInfo {
                id,
                strip_field_side: None,
                cable: false,
            },
        );
        cx.records.push(Record::Device(Device {
            id,
            v: 1,
            tag: TagSpec::new("X".parse().expect("a valid identifier"), None),
            kind: Some(DeviceKind::Terminal),
            parent: None,
            position: None,
            structure,
            parts,
            function_text,
            process_tag: None,
            tech_data: KeyedList::default(),
            state,
            source_ref: Some(source_ref("terminals", row.key())),
            aspects: DeviceAspects {
                strip: None,
                terminal: Some(Terminal {
                    strip: strip_id,
                    seq,
                    designation,
                    levels: Vec::new(),
                    rail_bonded,
                    extra: Default::default(),
                }),
                cable: None,
            },
            extra: Default::default(),
        }));
    }
}

/// The CP list of a function row: the part template's pattern entries, else the FnDef's CPs.
fn function_cps(cx: &Cx<'_>, r: &mut Rd<'_>, fndef: &str) -> Vec<CpInfo> {
    if let Some(part) = r.text("part") {
        let template = r.uint::<usize>("template").unwrap_or(0);
        let Some(info) = cx.parts.get(part) else {
            r.fail(format!("column `part`: unknown part `{part}`"));
            return Vec::new();
        };
        let Some((_, idxs)) = info.templates.get(template) else {
            r.fail(format!("part `{part}` has no function template {template}"));
            return Vec::new();
        };
        return idxs
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let cp = info.pattern.get(p)?;
                Some(CpInfo {
                    idx: u16::try_from(i).ok()?,
                    ..cp.clone()
                })
            })
            .collect();
    }
    match cx.lib.fndef(fndef) {
        Some(c) => c
            .item
            .cps
            .as_slice()
            .iter()
            .map(|d| CpInfo {
                idx: d.idx,
                designation: d.designation.clone(),
                side: d.side.clone(),
                clamp: d.clamp.clone(),
            })
            .collect(),
        None => Vec::new(),
    }
}

/// `functions.csv` → `function`. A device that ends with functions but no main function loses them
/// (EW-INV-002); an interruption point has no device (EW-INV-003).
pub(crate) fn emit_functions(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut pending: Vec<(Row<'_>, Function, FnInfo)> = Vec::new();
    let mut mains: BTreeSet<String> = BTreeSet::new();
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let fndef = r.req("fndef").map(str::to_owned);
        let device = r.text("device").map(str::to_owned);
        let role = r
            .choice("role", &[("main", FnRole::Main), ("aux", FnRole::Aux)])
            .unwrap_or_default();
        let kind_name = r
            .choice("kind", &[("normal", 0u8), ("black_box", 1), ("ip", 2)])
            .unwrap_or(0);
        let mut kind = match kind_name {
            1 => FunctionKind::BlackBox,
            _ => FunctionKind::Normal,
        };
        if kind_name == 2 {
            let name = r.req_ident("ip_name");
            let direction = r.req_choice(
                "ip_direction",
                &[
                    ("source", IpDirection::Source),
                    ("target", IpDirection::Target),
                    ("undirected", IpDirection::Undirected),
                ],
            );
            let external = match r.text("ip_external") {
                None => None,
                Some(k) => match cx.external_refs.get(k) {
                    Some(id) => Some(*id),
                    None => {
                        r.fail(format!("column `ip_external`: unknown external reference `{k}`"));
                        None
                    }
                },
            };
            if device.is_some() {
                r.fail("an interruption point has no device (EW-INV-003)");
            }
            if let (Some(name), Some(direction)) = (name, direction) {
                kind = FunctionKind::Ip(IpSpec {
                    name,
                    direction,
                    star_source: false,
                    sort: None,
                    external,
                    extra: Default::default(),
                });
            }
        } else if device.is_none() {
            r.fail("column `device` is empty (only interruption points have no device)");
        }
        let cps = fndef
            .as_deref()
            .map(|f| function_cps(cx, &mut r, f))
            .unwrap_or_default();
        let mut overrides = Vec::new();
        for item in r.list("cp_aliases") {
            let Some((d, alias)) = item.split_once('=') else {
                r.fail(format!(
                    "column `cp_aliases`: `{item}` is not <designation>=<alias>"
                ));
                continue;
            };
            let hits: Vec<&CpInfo> = cps.iter().filter(|c| c.designation.as_str() == d).collect();
            match (hits.as_slice(), parse_ident(alias)) {
                ([cp], Ok(alias)) => overrides.push(CpOverride {
                    idx: cp.idx,
                    designation: None,
                    alias: Some(alias),
                    description: None,
                    unused: false,
                    extra: Default::default(),
                }),
                _ => r.fail(format!("column `cp_aliases`: `{item}` names no single CP")),
            }
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let Some(fndef) = fndef else { continue };
        let ip = matches!(kind, FunctionKind::Ip(_));
        if kind_name == 2 && !ip {
            continue;
        }
        let Some(def) = cx.fndef_id(&fndef) else {
            cx.unresolved(row, &format!("unknown FnDef `{fndef}`"));
            continue;
        };
        let device_id = match &device {
            None => None,
            Some(d) => match cx.devices.get(d) {
                Some(info) => Some(info.id),
                None => {
                    cx.unresolved(row, &format!("unknown device `{d}`"));
                    continue;
                }
            },
        };
        if let Some(d) = &device
            && role == FnRole::Main
            && !mains.insert(d.clone())
        {
            cx.malformed(
                row,
                &format!("device `{d}` already has a main function (EW-INV-002)"),
            );
            continue;
        }
        let Ok(cp_overrides) = KeyedList::from_vec(overrides) else {
            cx.malformed(row, "two aliases for one CP");
            continue;
        };
        seen.insert(row.key().to_owned(), ());
        let id = record_id::<Function>("functions", row.key());
        pending.push((
            row,
            Function {
                id,
                v: 1,
                device: device_id,
                def,
                role,
                kind,
                cp_overrides,
                source_ref: Some(source_ref("functions", row.key())),
                extra: Default::default(),
            },
            FnInfo { id, device, cps, ip },
        ));
    }
    for (row, function, info) in pending {
        if let Some(d) = &info.device
            && !mains.contains(d)
        {
            cx.unresolved(row, &format!("device `{d}` has no main function (EW-INV-002)"));
            continue;
        }
        if let Some(d) = &info.device {
            cx.device_functions
                .entry(d.clone())
                .or_default()
                .push(row.key().to_owned());
        }
        cx.functions.insert(row.key().to_owned(), info);
        cx.records.push(Record::Function(function));
    }
}

/// `representations.csv` → `representation`. Each interruption point keeps exactly one representation
/// (EW-INV-013): a second one is dropped, and a point without one is dropped with its function.
pub(crate) fn emit_representations(cx: &mut Cx<'_>, table: Option<&Table>) {
    let mut ip_reps: BTreeSet<String> = BTreeSet::new();
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.into_iter().flat_map(Table::rows) {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let functions: Vec<String> = r.list("functions").into_iter().map(str::to_owned).collect();
        let page = r.req("page").map(str::to_owned);
        let symbol = r.req("symbol").map(str::to_owned);
        let cell = r.req("cell").and_then(|c| match parse_cell(c) {
            Ok(c) => Some(c),
            Err(e) => {
                r.fail(e);
                None
            }
        });
        let dx = r.int("dx").unwrap_or(0);
        let dy = r.int("dy").unwrap_or(0);
        if functions.is_empty() {
            r.fail("column `functions` is empty");
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(page), Some(symbol), Some(cell)) = (page, symbol, cell) else {
            continue;
        };
        let mut ids = Vec::new();
        let mut owner: Option<Option<String>> = None;
        let mut problem = None;
        let mut ip_fn = None;
        for f in &functions {
            match cx.functions.get(f) {
                None => problem = Some(format!("unknown function `{f}`")),
                Some(info) => {
                    if info.ip {
                        if functions.len() > 1 {
                            problem = Some("an interruption point is drawn alone".into());
                        }
                        ip_fn = Some(f.clone());
                    }
                    match &owner {
                        None => owner = Some(info.device.clone()),
                        Some(o) if *o != info.device => {
                            problem = Some("the functions belong to different devices (EW-INV-004)".into());
                        }
                        Some(_) => {}
                    }
                    ids.push(info.id);
                }
            }
        }
        if let Some(p) = problem {
            cx.unresolved(row, &p);
            continue;
        }
        if let Some(f) = &ip_fn
            && ip_reps.contains(f)
        {
            cx.malformed(
                row,
                &format!("interruption point `{f}` already has a representation (EW-INV-013)"),
            );
            continue;
        }
        let Some(page_info) = cx.pages.get(&page).copied() else {
            cx.unresolved(row, &format!("unknown page `{page}`"));
            continue;
        };
        let Some(sym) = cx.lib.symbol(&symbol) else {
            cx.unresolved(row, &format!("unknown symbol `{symbol}`"));
            continue;
        };
        let anchor = match &sym.item.anchor {
            AnchorPolicy::Named(p) => *p,
            _ => Pt2::ORIGIN,
        };
        let (sym_id, rep_type) = (sym.id, sym.item.rep_type.clone());
        let placed = cell_point(cell, dx, dy).and_then(|t| place_on_grid(page_info.grid, t, anchor));
        let Some((pos, off_grid)) = placed else {
            cx.malformed(row, "position out of range");
            continue;
        };
        if off_grid {
            cx.corrected(
                row,
                "position off the page grid; snapped to the nearest grid point",
            );
        }
        cx.used_lib.insert(symbol);
        if let Some(f) = ip_fn {
            ip_reps.insert(f);
        }
        seen.insert(row.key().to_owned(), ());
        cx.records.push(Record::Representation(Representation {
            id: record_id("representations", row.key()),
            v: 1,
            functions: ids,
            page: page_info.id,
            symbol: sym_id,
            variant: 0,
            pos,
            orient: Default::default(),
            rep_type,
            pin_map: KeyedList::default(),
            text_slots: KeyedList::default(),
            arrangement: None,
            box_: None,
            source_ref: Some(source_ref("representations", row.key())),
            extra: Default::default(),
        }));
    }
    // Interruption points without a representation.
    let orphans: Vec<(String, Id<Function>)> = cx
        .functions
        .iter()
        .filter(|(k, f)| f.ip && !ip_reps.contains(*k))
        .map(|(k, f)| (k.clone(), f.id))
        .collect();
    for (key, id) in orphans {
        cx.findings.at(
            ew_interop_api::Code::UNRESOLVED_REFERENCE,
            ew_interop_api::Severity::Error,
            "functions.csv",
            Some(&key),
            None,
            "row dropped: interruption point without a representation (EW-INV-013)",
        );
        cx.functions.remove(&key);
        cx.records
            .retain(|r| !matches!(r, Record::Function(f) if f.id == id));
    }
}
