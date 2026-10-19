// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Devices, terminals, functions and representations (doc 20 §6, §9, §15; doc 07 §9.1).

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{
    Cable, CpOverride, Device, DeviceAspects, DeviceKind, DeviceState, FnRole, FrozenTag, Function,
    FunctionKind, IpDirection, IpSpec, KeyedList, LabelMode, LengthSource, PartRef, PartUse, Record,
    Representation, Side, Strip, TagPrint, TagScheme, TagSign, TagSpec, Terminal, TerminalGroup,
};
use ew_units::Dec3;

use super::project::structure_assignment;
use super::{
    CpInfo, Cx, DeviceInfo, Fate, FnInfo, PlaceError, cell_point, dec3_mm_to_nm, fresh, parent_fates,
    parse_cell, place,
};
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

/// `parts` cells: `<part>` or `<part>*<qty>`; the part uses and their `parts` keys, in cell order.
fn part_uses(cx: &Cx<'_>, r: &mut Rd<'_>) -> (Vec<PartUse>, Vec<String>) {
    let mut out = Vec::new();
    let mut keys = Vec::new();
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
            Some(p) => {
                out.push(PartUse {
                    part: PartRef::new(p.id),
                    qty,
                    extra: Default::default(),
                });
                keys.push(key.to_owned());
            }
            None => r.fail(format!("column `parts`: unknown part `{key}`")),
        }
    }
    (out, keys)
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
        let (parts, part_keys) = part_uses(cx, &mut r);
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
                parts: part_keys,
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

    // Parents: resolve, then refuse cycles (EW-INV-009) and drop the children of dropped devices, in one
    // linear walk (doc 14 §11: hostile chains must not cost O(N²)).
    let index: BTreeMap<&str, usize> = pending
        .iter()
        .enumerate()
        .map(|(i, (row, _, _))| (row.key(), i))
        .collect();
    let mut parents: Vec<Option<usize>> = vec![None; pending.len()];
    let mut pre = vec![false; pending.len()];
    for (i, (row, parent, _)) in pending.iter().enumerate() {
        if let Some(p) = parent {
            match index.get(p.as_str()) {
                Some(j) => parents[i] = Some(*j),
                None => {
                    cx.unresolved(*row, &format!("unknown parent `{p}`"));
                    pre[i] = true;
                }
            }
        }
    }
    let fates = parent_fates(&parents, &pre);
    let rows: Vec<Row<'_>> = pending.iter().map(|(row, _, _)| *row).collect();
    cx.report_fates(&rows, &fates, "EW-INV-009");
    let ids: Vec<Id<Device>> = pending.iter().map(|(_, _, d)| d.id).collect();
    for (i, (row, _, mut device)) in pending.into_iter().enumerate() {
        if fates[i] != Fate::Kept {
            cx.devices.remove(row.key());
            continue;
        }
        device.parent = parents[i].map(|j| ids[j]);
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
        let (parts, part_keys) = part_uses(cx, &mut r);
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
                parts: part_keys,
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

/// The template-origin rule (doc 03 §7 "CP instantiation", README "CP and endpoint notation"): the first part
/// use of `device` (in `Device.parts` order) whose part template declares a function with FnDef `fndef` and the
/// role `role` (`TemplateFn.main` ⇔ `FnRole::Main`), first matching template entry in `n` order; its part key
/// and template position. `ew-derive`'s `function_cps` applies the same rule.
fn template_origin<'c>(
    cx: &'c Cx<'_>,
    device: Option<&str>,
    fndef: &str,
    role: &FnRole,
) -> Option<(&'c str, usize, &'c super::PartInfo, &'c super::TemplateInfo)> {
    let main = *role == FnRole::Main;
    cx.devices.get(device?)?.parts.iter().find_map(|key| {
        let part = cx.parts.get(key)?;
        let (n, t) = part
            .templates
            .iter()
            .enumerate()
            .find(|(_, t)| t.fndef == fndef && t.main == main)?;
        Some((key.as_str(), n, part, t))
    })
}

/// The CP list of a function row, in CP index order:
///
/// 1. **template origin** ([`template_origin`]): the template's pattern entries, each with its **pattern**
///    index (the part's CP pattern defines the CP index space);
/// 2. a **black box**: every entry of the first part use whose part has a CP pattern, with its pattern index;
/// 3. otherwise the FnDef's CPs.
///
/// The optional `part` and `template` columns state the expected origin: the row is malformed (EW-IMP-002) when
/// the rule does not select exactly that part and template position.
fn function_cps(
    cx: &Cx<'_>,
    r: &mut Rd<'_>,
    fndef: &str,
    device: Option<&str>,
    role: &FnRole,
    black_box: bool,
) -> Vec<CpInfo> {
    let named = r.text("part");
    let template = r.uint::<usize>("template");
    let origin = template_origin(cx, device, fndef, role);
    if let Some(part) = named {
        let n = template.unwrap_or(0);
        if !cx.parts.contains_key(part) {
            r.fail(format!("column `part`: unknown part `{part}`"));
        } else if origin.is_none_or(|(k, m, _, _)| k != part || m != n) {
            let role = if *role == FnRole::Main { "main" } else { "aux" };
            r.fail(format!(
                "column `part`: the device carries no part `{part}` whose template {n} is the first to declare \
                 FnDef `{fndef}` with role {role} (template-origin rule)"
            ));
        }
    } else if template.is_some() {
        r.fail("column `template` needs column `part`");
    }
    if let Some((_, _, part, t)) = origin {
        let mut cps: Vec<CpInfo> = t
            .cps
            .iter()
            .filter_map(|p| part.pattern.get(p).cloned())
            .collect();
        cps.sort_by_key(|c| c.idx);
        return cps;
    }
    if black_box
        && let Some(part) = device
            .and_then(|d| cx.devices.get(d))
            .into_iter()
            .flat_map(|d| d.parts.iter())
            .filter_map(|k| cx.parts.get(k))
            .find(|p| !p.pattern.is_empty())
    {
        return part.pattern.values().cloned().collect();
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
        let black_box = kind_name == 1;
        let cps = fndef
            .as_deref()
            .map(|f| function_cps(cx, &mut r, f, device.as_deref(), &role, black_box))
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
        // The anchor of the symbol's policy, exactly as `rep_anchor_cell` derives it (FR-SYM-010, D-14): the
        // cell column names where the anchor lands, not the symbol origin.
        let (sym_id, rep_type) = (sym.id, sym.item.rep_type.clone());
        let placed = sym
            .item
            .anchor_point()
            .zip(cell_point(cell, dx, dy))
            .ok_or(PlaceError::OutOfRange)
            .and_then(|(anchor, target)| place(page_info.grid, target, anchor));
        let pos = match placed {
            Ok(pos) => pos,
            Err(e) => {
                cx.malformed(row, e.message());
                continue;
            }
        };
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
    // Interruption points without a representation: reported in key order, then removed with one pass over
    // the records (doc 14 §11: no O(N²) clean-up).
    let orphans: Vec<(String, Id<Function>)> = cx
        .functions
        .iter()
        .filter(|(k, f)| f.ip && !ip_reps.contains(*k))
        .map(|(k, f)| (k.clone(), f.id))
        .collect();
    let doomed: BTreeSet<Id<Function>> = orphans.iter().map(|(_, id)| *id).collect();
    for (key, _) in orphans {
        cx.findings.at(
            ew_interop_api::Code::UNRESOLVED_REFERENCE,
            ew_interop_api::Severity::Error,
            "functions.csv",
            Some(&key),
            None,
            "row dropped: interruption point without a representation (EW-INV-013)",
        );
        cx.functions.remove(&key);
    }
    if !doomed.is_empty() {
        cx.records
            .retain(|r| !matches!(r, Record::Function(f) if doomed.contains(&f.id)));
    }
}
