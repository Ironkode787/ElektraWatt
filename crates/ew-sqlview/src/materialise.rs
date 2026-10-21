// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rows of every internal table, read from the model through tracked queries (`Q`) and from the check
//! report the caller ran.
//!
//! Row order is deterministic (NFR-REL-07): every table is filled in a total order (page order, reading
//! order, natural tag order, ID), so two builds over equal models write equal databases. SQL must still say
//! `ORDER BY`: a view has no order of its own.

use std::collections::{BTreeMap, BTreeSet};

use ew_checks::{CheckReport, FindingState};
use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{NaturalKey, natural_key};
use ew_derive::ew_rds::{Aspect, RdsSyntax};
use ew_derive::ew_schema::{
    AcDc, CircuitClass, Device, DeviceState, FnRole, FunctionKind, LengthSource, PotentialType,
};
use ew_derive::{
    BomMode, BomRows, CpRef, DevicePartLines, DeviceTag, EndKind, MainRep, PageOrder, PageRef,
    PageRefStructure, PageTitle, Potential, PotentialAttrsOf, PotentialFlow, PotentialId, PropSource, Q,
    RepAnchorCell, ResolvedStructure, SizeRule, XrefTextOf, main_part, manufacturer_name, tech_data_text,
};
use rusqlite::types::Value as SqlValue;

use crate::conn::{self, Bridge, WireRec, Wires, cp_of, device_of, ip_name, tag_short};
use crate::rows::{
    Row, colour_iec, colour_text, csa_parts, decimal, enum_text, json_array, ml_json, ml_text, non_empty,
};
use crate::schema::{
    self, BOM_ASSEMBLY, BOM_COMPONENT, BOND, CABLE, CONDUCTOR, DEVICE, FUNCTION, MESSAGE, META, PAGE,
    POTENTIAL, TERMINAL, View, WIRE,
};

/// What the caller adds to the model: the facts the model does not hold.
#[derive(Clone, Copy, Debug, Default)]
pub struct BuildInput<'a> {
    /// The model revision (`api_v1_meta.rev`).
    pub rev: Option<i64>,
    /// The store generation the project was opened at (`api_v1_meta.generation`).
    pub generation: Option<i64>,
    /// The version of the program (`api_v1_meta.ew_version`).
    pub ew_version: &'a str,
    /// The date of the release context (`api_v1_meta.as_of`); never the wall clock.
    pub as_of: Option<&'a str>,
    /// The check run whose findings fill `api_v1_message`; `None` leaves it empty.
    pub checks: Option<&'a CheckReport>,
}

/// The rows of every table, by view.
pub(crate) type Tables = Vec<(&'static View, Vec<Vec<SqlValue>>)>;

/// Reads every table.
pub(crate) fn tables(q: &Q, input: &BuildInput<'_>) -> Tables {
    let wires = Wires::of(q);
    let bridges = conn::bridges(q);
    let mut out: BTreeMap<&'static str, (&'static View, Vec<Vec<SqlValue>>)> = BTreeMap::new();
    let mut put = |view: &'static View, rows: Vec<Row>| {
        out.insert(view.name, (view, rows.into_iter().map(Row::finish).collect()));
    };
    put(&META, meta(q, input));
    put(&PAGE, pages(q));
    put(&DEVICE, devices(q));
    put(&FUNCTION, functions(q));
    put(&WIRE, wire_rows(q, &wires, &bridges));
    put(&TERMINAL, terminals(q, &wires, &bridges));
    put(&CABLE, cables(q, &wires));
    put(&CONDUCTOR, conductors(q, &wires));
    put(&POTENTIAL, potentials(q, &wires));
    put(&BOND, bonds(q, &wires));
    put(&BOM_COMPONENT, bom(q, BomMode::Component, &BOM_COMPONENT));
    put(&BOM_ASSEMBLY, bom(q, BomMode::Assembly, &BOM_ASSEMBLY));
    put(&MESSAGE, messages(input.checks));
    debug_assert_eq!(out.len(), schema::VIEWS.len(), "every view has a table");
    out.into_values().collect()
}

fn uuid<T>(id: Id<T>) -> String {
    id.as_uuid().to_string()
}

fn render_aspect(s: &ResolvedStructure, aspect: Aspect) -> Option<String> {
    non_empty(s.render_aspect(aspect, &RdsSyntax::default()))
}

fn page_folio(q: &Q, page: PageRef) -> Option<String> {
    q.get::<PageOrder>(()).entry(page).map(|e| e.folio.clone())
}

fn folios_json(q: &Q, pages: impl IntoIterator<Item = PageRef>) -> String {
    let order = q.get::<PageOrder>(());
    let folios: Vec<String> = pages
        .into_iter()
        .filter_map(|p| order.entry(p).map(|e| e.folio.clone()))
        .collect();
    json_array(&folios)
}

// ---------------------------------------------------------------------------------------------------------
// meta, page

fn meta(q: &Q, input: &BuildInput<'_>) -> Vec<Row> {
    let mut r = Row::new(&META);
    let project = q.project();
    let languages: Vec<String> = project
        .languages
        .as_ref()
        .map(|l| {
            std::iter::once(l.source.as_str().to_owned())
                .chain(l.translations.iter().map(|t| t.as_str().to_owned()))
                .collect()
        })
        .unwrap_or_default();
    r.set("rev", input.rev)
        .set("generation", input.generation)
        .set("format", "1")
        .set("derivation_profile", q.profile().version.as_str())
        .set("ew_version", input.ew_version)
        .set("languages", json_array(&languages))
        .set("as_of", input.as_of)
        .set("api_status", schema::API_STATUS)
        .set("preview_additions", json_array(&schema::preview_additions()));
    vec![r]
}

fn pages(q: &Q) -> Vec<Row> {
    let order = q.get::<PageOrder>(());
    order
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut r = Row::new(&PAGE);
            let (id, label, description, generated, block) = match e.page {
                PageRef::Drawn(p) => {
                    let page = q.pages().get(p);
                    (
                        uuid(p),
                        page.and_then(|pg| pg.folio.label.as_ref().map(|l| l.as_str().to_owned())),
                        page.and_then(|pg| pg.description_override.as_ref().map(ml_text)),
                        false,
                        None,
                    )
                }
                PageRef::Divider(s) => (uuid(s), None, None, false, None),
                PageRef::Generated { block, index } => (
                    uuid(ew_derive::ew_ids::v5::report_page_id::<_, ew_derive::GeneratedPage>(block, index)),
                    None,
                    None,
                    true,
                    Some(uuid(block)),
                ),
            };
            let title = q.get::<PageTitle>(e.page);
            let structure = q.get::<PageRefStructure>(e.page);
            r.set("id", id)
                .set("physical_index", i)
                .set("folio", &e.folio)
                .set("number", e.number)
                .set("label", label)
                .set("section_id", e.section.map(uuid))
                .set("kind", enum_text(&e.kind))
                .set("title", ml_text(&title))
                .set("title_ml", ml_json(&title))
                .set("description_overridden", description.is_some())
                .set("description", description)
                .set("structure_function", render_aspect(&structure, Aspect::Function))
                .set("structure_location", render_aspect(&structure, Aspect::Location))
                .set("prev_folio", order.prev(e.page).map(|p| p.folio.clone()))
                .set("next_folio", order.next(e.page).map(|p| p.folio.clone()))
                .set("generated", generated)
                .set("report_block_id", block);
            r
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// device, function

/// The page, folio and cell of a representation.
fn rep_place(
    q: &Q,
    rep: Id<ew_derive::ew_schema::Representation>,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(page) = q.reps().get(rep).map(|r| r.page) else {
        return (None, None, None);
    };
    let cell = q.get::<RepAnchorCell>(rep);
    (
        Some(uuid(page)),
        page_folio(q, PageRef::Drawn(page)),
        cell.as_ref().as_ref().map(ToString::to_string),
    )
}

fn devices(q: &Q) -> Vec<Row> {
    let mut ids: Vec<(NaturalKey, Id<Device>)> =
        q.devices().ids().map(|d| (conn::tag_key(q, d), d)).collect();
    ids.sort();
    ids.into_iter()
        .filter_map(|(_, id)| {
            let d = q.devices().get(id)?;
            let tag = q.get::<DeviceTag>(id);
            let tag = tag.as_ref().as_ref();
            let (page, folio, cell) = match *q.get::<MainRep>(id) {
                Some(rep) => rep_place(q, rep),
                None => (None, None, None),
            };
            let part = main_part(q, id);
            let mut aspects = Vec::new();
            if d.aspects.strip.is_some() {
                aspects.push("strip");
            }
            if d.aspects.terminal.is_some() {
                aspects.push("terminal");
            }
            if d.aspects.cable.is_some() {
                aspects.push("cable");
            }
            let mut r = Row::new(&DEVICE);
            r.set("id", uuid(id))
                .set("tag", tag.map(|t| t.short()))
                .set("tag_full", tag.map(|t| t.full.clone()))
                .set("letter", d.tag.letter.as_str())
                .set("counter", d.tag.counter)
                .set("frozen", d.tag.frozen.is_some())
                .set("kind", d.kind.as_ref().map(enum_text))
                .set("parent_id", d.parent.map(uuid))
                .set(
                    "structure_function",
                    tag.and_then(|t| render_aspect(&t.structure, Aspect::Function)),
                )
                .set(
                    "structure_location",
                    tag.and_then(|t| render_aspect(&t.structure, Aspect::Location)),
                )
                .set("function_text", non_empty(ml_text(&d.function_text)))
                .set(
                    "function_text_ml",
                    (!d.function_text.is_empty()).then(|| ml_json(&d.function_text)),
                )
                .set(
                    "process_tag",
                    d.process_tag.as_ref().map(|p| p.as_str().to_owned()),
                )
                .set("state", enum_text(&d.state))
                .set("main_page_id", page)
                .set("main_folio", folio)
                .set("main_cell", cell)
                .set("has_part", !d.parts.is_empty())
                .set(
                    "manufacturer",
                    part.as_ref().map(|p| manufacturer_name(&p.item.manufacturer)),
                )
                .set(
                    "part_number",
                    part.as_ref().map(|p| p.item.part_number.as_str().to_owned()),
                )
                .set("tech_data", non_empty(tech_data_text(q, id)))
                .set("aspects", json_array(&aspects));
            Some(r)
        })
        .collect()
}

fn functions(q: &Q) -> Vec<Row> {
    let order = q.get::<PageOrder>(());
    q.functions()
        .iter()
        .map(|(id, f)| {
            // The first representation in page order, then by ID.
            let rep = q
                .idx()
                .reps_of_function(id)
                .filter_map(|r| {
                    let page = q.reps().get(r)?.page;
                    Some((order.position(PageRef::Drawn(page)).unwrap_or(usize::MAX), r))
                })
                .min();
            let (page, folio, cell) = rep.map_or((None, None, None), |(_, r)| rep_place(q, r));
            let xref = rep.and_then(|(_, r)| non_empty(q.get::<XrefTextOf>(r).text.clone()));
            let fndef = q.fndefs().get(f.def);
            let (kind, ip) = match &f.kind {
                FunctionKind::Ip(ip) => ("ip".to_owned(), Some(ip.name.as_str().to_owned())),
                other => (enum_text(other), None),
            };
            let mut r = Row::new(&FUNCTION);
            r.set("id", uuid(id))
                .set("device_id", f.device.map(uuid))
                .set("device_tag", f.device.and_then(|d| tag_short(q, d)))
                .set(
                    "role",
                    match &f.role {
                        FnRole::Main => "main".to_owned(),
                        FnRole::Aux => "aux".to_owned(),
                        other => enum_text(other),
                    },
                )
                .set("kind", kind)
                .set("ip_name", ip)
                .set(
                    "fndef",
                    fndef.and_then(|d| d.item.meta.key.as_ref().map(|k| k.as_str().to_owned())),
                )
                .set("category", fndef.map(|d| enum_text(&d.item.category)))
                .set("page_id", page)
                .set("folio", folio)
                .set("cell", cell)
                .set("xref_text", xref);
            r
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// wire

fn source_name(s: &PropSource) -> &'static str {
    match s {
        PropSource::Override(_) => "cdp",
        PropSource::Conductor(_) => "core",
        PropSource::SizeRow {
            rule: SizeRule::Bond, ..
        } => "bond",
        PropSource::ColourRow { .. } | PropSource::SizeRow { .. } => "rule",
        PropSource::PotentialDef(_) => "potential",
        PropSource::ProjectDefault => "project",
        PropSource::BuiltIn { .. } => "built-in",
    }
}

fn source_words(q: &Q, s: &PropSource) -> String {
    match s {
        PropSource::Override(_) => "override on the wire".to_owned(),
        PropSource::Conductor(c) => match q.conductors().get(*c) {
            Some(c) => format!(
                "core {} of {}",
                c.core.as_str(),
                tag_short(q, c.cable).unwrap_or_default()
            ),
            None => "cable core".to_owned(),
        },
        PropSource::PotentialDef(_) => "potential definition".to_owned(),
        PropSource::ColourRow { profile, .. } => format!("colour profile {profile}"),
        PropSource::SizeRow { profile, .. } => format!("size profile {profile}"),
        PropSource::ProjectDefault => "project default".to_owned(),
        PropSource::BuiltIn { profile } => format!("built-in default of derivation profile {profile}"),
    }
}

/// The columns of one wire end.
fn set_end(q: &Q, r: &mut Row, prefix: &str, end: &ew_derive::WireEnd) {
    let col = |name: &str| format!("{prefix}_{name}");
    let kind = match end.kind {
        EndKind::Device => "device",
        EndKind::External => "external",
        EndKind::Open => "open",
    };
    r.set(&col("end"), kind);
    match end.kind {
        EndKind::Device => {
            let cp = cp_of(q, end.cp);
            let device = device_of(q, end.cp.function);
            r.set(&col("device_id"), device.map(uuid))
                .set(&col("tag"), device.and_then(|d| tag_short(q, d)))
                .set(&col("cp"), cp.as_ref().map(|c| c.designation.clone()))
                .set(&col("level"), cp.as_ref().and_then(|c| c.level))
                .set(
                    &col("clamp"),
                    cp.as_ref().and_then(|c| match &c.clamp {
                        Some(ew_derive::ew_schema::ClampKey::Clamp(ch)) => Some(ch.to_string()),
                        _ => None,
                    }),
                )
                .set(
                    &col("side"),
                    cp.as_ref().and_then(|c| match c.side {
                        Some(ew_derive::ew_schema::Side::Side1) => Some(1u8),
                        Some(ew_derive::ew_schema::Side::Side2) => Some(2u8),
                        _ => None,
                    }),
                );
        }
        EndKind::External | EndKind::Open => {
            r.set(&col("ip"), non_empty(ip_name(q, end.cp.function)));
        }
    }
}

fn wire_rows(q: &Q, wires: &Wires, bridges: &[Bridge]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (seq, rec) in wires.wires.iter().enumerate() {
        rows.push(wire_row(q, wires, rec, seq));
    }
    for (i, b) in bridges.iter().enumerate() {
        rows.push(bridge_row(q, wires, b, wires.wires.len() + i));
    }
    rows
}

fn wire_row(q: &Q, wires: &Wires, rec: &WireRec, seq: usize) -> Row {
    let w = &rec.wire;
    let p = &rec.props;
    let mut r = Row::new(&WIRE);
    r.set("id", uuid(w.id))
        .set("seq", seq)
        .set("number", rec.number.clone())
        .set("kind", "wire");
    set_end(q, &mut r, "from", &w.ends[0]);
    set_end(q, &mut r, "to", &w.ends[1]);
    let potential = if w.bond.is_some() {
        None
    } else {
        w.potentials.first().copied()
    };
    r.set("potential_id", potential.map(potential_uuid))
        .set("potential_name", non_empty(wires.potential_text(w)));
    if let Some((value, unit)) = p.csa.as_ref().and_then(|c| csa_parts(&c.value)) {
        r.set("csa", value).set("csa_unit", unit);
    }
    if let Some(c) = &p.colour {
        r.set("colour", colour_text(&c.value))
            .set("colour_iec", colour_iec(&c.value));
    }
    r.set(
        "wire_part",
        p.wire_part
            .as_ref()
            .and_then(|wp| q.parts().get(wp.value.copy))
            .map(|c| c.item.part_number.as_str().to_owned()),
    );
    if let Some(core) = w.core.and_then(|c| q.conductors().get(c)) {
        r.set("cable_id", uuid(core.cable))
            .set("cable_tag", tag_short(q, core.cable))
            .set("core", core.core.as_str())
            .set("conductor_id", uuid(core.id));
    }
    r.set("bond_id", w.bond.map(uuid));
    // The interruption points the wire passes, by name (the two points of a pair share one), not the
    // external point it ends at.
    let mut ips: Vec<String> = Vec::new();
    for f in w
        .ips
        .iter()
        .filter(|f| w.ends.iter().all(|e| e.cp.function != **f))
    {
        let name = ip_name(q, *f);
        if !name.is_empty() && !ips.contains(&name) {
            ips.push(name);
        }
    }
    r.set("ips", json_array(&ips))
        .set(
            "pages",
            folios_json(q, w.pages.iter().map(|pg| PageRef::Drawn(*pg))),
        )
        .set("csa_source", p.csa.as_ref().map(|c| source_name(&c.source)))
        .set("colour_source", p.colour.as_ref().map(|c| source_name(&c.source)))
        .set("prov_csa", p.csa.as_ref().map(|c| source_words(q, &c.source)))
        .set(
            "prov_colour",
            p.colour.as_ref().map(|c| source_words(q, &c.source)),
        );
    r
}

fn potential_uuid(p: PotentialId) -> String {
    match p {
        PotentialId::Def(d) => uuid(d),
        PotentialId::Cp(c) => uuid(c),
    }
}

/// The potential a bridge joins, by the first terminal's first connected CP.
fn bridge_potential(q: &Q, wires: &Wires, b: &Bridge) -> Option<PotentialId> {
    let t = b.terminals.first()?;
    terminal_potential(q, wires, *t)
}

fn terminal_potential(q: &Q, wires: &Wires, t: Id<Device>) -> Option<PotentialId> {
    for f in q.idx().functions_of_device(t) {
        for cp in q.get::<ew_derive::FunctionCps>(f).iter() {
            if let Some(p) = wires.partition.potential_at(CpRef::new(f, cp.idx).key()) {
                return Some(p.id);
            }
        }
    }
    None
}

fn bridge_row(q: &Q, wires: &Wires, b: &Bridge, seq: usize) -> Row {
    let mut r = Row::new(&WIRE);
    let potential = bridge_potential(q, wires, b);
    r.set("id", format!("{}/bridge/{}", uuid(b.strip), b.group))
        .set("seq", seq)
        .set("number", b.label.as_str())
        .set("kind", "bridge")
        .set("bridge_kind", b.kind)
        .set("potential_id", potential.map(potential_uuid))
        .set(
            "potential_name",
            potential.map(|p| wires.potential_name(p).to_owned()),
        )
        .set("ips", "[]")
        .set("pages", folios_json(q, b.page.map(PageRef::Drawn)));
    for (prefix, t) in [("from", b.terminals.first()), ("to", b.terminals.last())] {
        if let Some(t) = t {
            r.set(&format!("{prefix}_end"), "device")
                .set(&format!("{prefix}_device_id"), uuid(*t))
                .set(&format!("{prefix}_tag"), tag_short(q, *t));
        }
    }
    r
}

// ---------------------------------------------------------------------------------------------------------
// terminal

fn terminals(q: &Q, wires: &Wires, bridges: &[Bridge]) -> Vec<Row> {
    let mut wired: BTreeSet<Id<Device>> = BTreeSet::new();
    for rec in &wires.wires {
        for e in &rec.wire.ends {
            if let Some(d) = device_of(q, e.cp.function) {
                wired.insert(d);
            }
        }
    }
    let mut strips: Vec<(NaturalKey, Id<Device>)> = q
        .devices()
        .iter()
        .filter(|(_, d)| d.aspects.strip.is_some())
        .map(|(id, _)| (natural_key(&tag_short(q, id).unwrap_or_default()), id))
        .collect();
    strips.sort();
    let mut rows = Vec::new();
    for (_, strip) in strips {
        let Some(strip_dev) = q.devices().get(strip) else {
            continue;
        };
        let aspect = strip_dev.aspects.strip.as_ref();
        let mut terms: Vec<(u32, Id<Device>)> = q
            .idx()
            .terminals_of_strip(strip)
            .filter_map(|t| Some((q.devices().get(t)?.aspects.terminal.as_ref()?.seq, t)))
            .collect();
        terms.sort_unstable();
        for (seq, t) in terms {
            let Some(dev) = q.devices().get(t) else { continue };
            let Some(term) = dev.aspects.terminal.as_ref() else {
                continue;
            };
            let group = aspect
                .and_then(|a| a.groups.iter().find(|g| g.from_seq <= seq && seq <= g.to_seq))
                .map(|g| ml_text(&g.label));
            let part = q
                .get::<DevicePartLines>(t)
                .first()
                .and_then(|l| q.parts().get(l.part))
                .map(|c| c.item.part_number.as_str().to_owned());
            let bridge = bridges.iter().find(|b| b.terminals.contains(&t));
            let potential = terminal_potential(q, wires, t);
            let ptype = potential.and_then(|p| {
                q.get::<PotentialAttrsOf>(p)
                    .ptype
                    .as_ref()
                    .map(|a| type_name(&a.value))
            });
            let state = match &dev.state {
                DeviceState::Spare => "spare",
                DeviceState::Reserved => "reserved",
                DeviceState::NotFitted => "not fitted",
                DeviceState::Used | DeviceState::Unknown(_) => {
                    if wired.contains(&t) || bridge.is_some() {
                        "used"
                    } else {
                        "unused"
                    }
                }
            };
            let (page, _, cell) = match *q.get::<MainRep>(t) {
                Some(rep) => rep_place(q, rep),
                None => (None, None, None),
            };
            let mut r = Row::new(&TERMINAL);
            r.set("id", uuid(t))
                .set("tag", tag_short(q, t))
                .set("strip_id", uuid(strip))
                .set("strip_tag", tag_short(q, strip))
                .set("designation", term.designation.as_str())
                .set("printed", term.designation.as_str())
                .set("seq", seq)
                .set("function_text", non_empty(ml_text(&dev.function_text)))
                .set("strip_group", group)
                .set("part_number", part)
                .set("levels", term.levels.len().max(1))
                .set("potential_id", potential.map(potential_uuid))
                .set(
                    "potential_name",
                    potential.map(|p| wires.potential_name(p).to_owned()),
                )
                .set("potential_type", ptype)
                .set("state", state)
                .set("bridge", bridge.map(|b| b.label.clone()))
                .set("rail_bonded", term.rail_bonded)
                .set("page_id", page)
                .set("cell", cell);
            rows.push(r);
        }
    }
    rows
}

// ---------------------------------------------------------------------------------------------------------
// cable, conductor

/// The wire running over each conductor.
fn wires_by_core(wires: &Wires) -> BTreeMap<Id<ew_derive::ew_schema::Conductor>, &WireRec> {
    wires
        .wires
        .iter()
        .filter_map(|w| Some((w.wire.core?, w)))
        .collect()
}

fn cables(q: &Q, wires: &Wires) -> Vec<Row> {
    let by_core = wires_by_core(wires);
    let mut cables: Vec<(NaturalKey, Id<Device>)> = q
        .devices()
        .iter()
        .filter(|(_, d)| d.aspects.cable.is_some())
        .map(|(id, _)| (natural_key(&tag_short(q, id).unwrap_or_default()), id))
        .collect();
    cables.sort();
    cables
        .into_iter()
        .filter_map(|(_, id)| {
            let d = q.devices().get(id)?;
            let cable = d.aspects.cable.as_ref()?;
            let cores: Vec<_> = q.idx().conductors_of_cable(id).collect();
            let (mut a, mut b) = (Vec::new(), Vec::new());
            let mut external = false;
            for c in &cores {
                if let Some(w) = by_core.get(c) {
                    let end = |i: usize| {
                        device_of(q, w.wire.ends[i].cp.function).map(|d| {
                            q.devices()
                                .get(d)
                                .and_then(|x| x.aspects.terminal.as_ref().map(|t| t.strip))
                                .unwrap_or(d)
                        })
                    };
                    a.extend(end(0));
                    b.extend(end(1));
                    external |= w.wire.ends.iter().any(|e| e.kind == EndKind::External);
                }
            }
            let shield = cores
                .iter()
                .any(|c| q.conductors().get(*c).is_some_and(|c| c.shield));
            let order = q.get::<PageOrder>(());
            let mut pages: Vec<(usize, PageRef)> = q
                .idx()
                .functions_of_device(id)
                .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
                .filter_map(|r| q.reps().get(r).map(|r| PageRef::Drawn(r.page)))
                .map(|p| (order.position(p).unwrap_or(usize::MAX), p))
                .collect();
            pages.sort();
            pages.dedup();
            let tag = q.get::<DeviceTag>(id);
            let (length_nm, source) = match &cable.length {
                LengthSource::Manual(nm) => (Some(nm.0), "manual"),
                LengthSource::FromPart => (None, "part"),
                LengthSource::Missing => (None, "missing"),
                LengthSource::Unknown(_) => (None, "unknown"),
            };
            let mut r = Row::new(&CABLE);
            r.set("id", uuid(id))
                .set("tag", tag.as_ref().as_ref().map(|t| t.short()))
                .set("tag_full", tag.as_ref().as_ref().map(|t| t.full.clone()))
                .set(
                    "part_number",
                    q.get::<DevicePartLines>(id)
                        .first()
                        .and_then(|l| q.parts().get(l.part))
                        .map(|c| c.item.part_number.as_str().to_owned()),
                )
                .set("cores", cores.len())
                .set("length_nm", length_nm)
                .set("length_source", source)
                .set("end_a", device_set(q, a))
                .set("end_b", device_set(q, b))
                .set("external_end", external)
                .set("shield", shield)
                .set("pages", folios_json(q, pages.into_iter().map(|(_, p)| p)));
            Some(r)
        })
        .collect()
}

/// The short tags of a set of devices in natural order without repeats, as a JSON array.
fn device_set(q: &Q, devices: Vec<Id<Device>>) -> String {
    let mut tags: Vec<(NaturalKey, String)> = devices
        .into_iter()
        .filter_map(|d| tag_short(q, d))
        .filter(|t| !t.is_empty())
        .map(|t| (natural_key(&t), t))
        .collect();
    tags.sort();
    tags.dedup();
    json_array(&tags.into_iter().map(|(_, t)| t).collect::<Vec<_>>())
}

/// The order of the conductor rows: cable tag, then the core-list order of doc 20 §10 (shield last, pair,
/// natural core name).
type CoreKey = (NaturalKey, bool, Option<u8>, NaturalKey);

fn conductors(q: &Q, wires: &Wires) -> Vec<Row> {
    let by_core = wires_by_core(wires);
    let mut keyed: Vec<(CoreKey, Row)> = q
        .conductors()
        .iter()
        .map(|(id, c)| {
            let cable_tag = tag_short(q, c.cable);
            let mut r = Row::new(&CONDUCTOR);
            r.set("id", uuid(id))
                .set("cable_id", uuid(c.cable))
                .set("cable_tag", cable_tag.clone())
                .set("core", c.core.as_str())
                .set("pair", c.pair)
                .set("colour", c.colour.as_ref().map(colour_text))
                .set("colour_iec", c.colour.as_ref().and_then(colour_iec))
                .set("shield", c.shield)
                .set("wire_id", by_core.get(&id).map(|w| uuid(w.wire.id)));
            if let Some((value, unit)) = c.csa.as_ref().and_then(csa_parts) {
                r.set("csa", value).set("csa_unit", unit);
            }
            (
                (
                    natural_key(&cable_tag.unwrap_or_default()),
                    c.shield,
                    c.pair,
                    natural_key(c.core.as_str()),
                ),
                r,
            )
        })
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.into_iter().map(|(_, r)| r).collect()
}

// ---------------------------------------------------------------------------------------------------------
// potential, bond

fn type_name(t: &PotentialType) -> String {
    match t {
        PotentialType::L1 => "L1",
        PotentialType::L2 => "L2",
        PotentialType::L3 => "L3",
        PotentialType::L => "L",
        PotentialType::N => "N",
        PotentialType::Pe => "PE",
        PotentialType::Pen => "PEN",
        PotentialType::Fe => "FE",
        PotentialType::LPlus => "L+",
        PotentialType::LMinus => "L-",
        PotentialType::M => "M",
        PotentialType::Signal => "signal",
        PotentialType::Undefined => "undefined",
        PotentialType::Unknown(_) => "other",
    }
    .to_owned()
}

fn class_name(c: &CircuitClass) -> String {
    match c {
        CircuitClass::Mains => "mains",
        CircuitClass::Control => "control",
        CircuitClass::Analogue => "analogue",
        CircuitClass::Bus => "bus",
        CircuitClass::Pe => "PE",
        CircuitClass::Unknown(_) => "other",
    }
    .to_owned()
}

/// The members of a potential as doc 20 §7.3 counts them: device CPs, a terminal once whatever its clamps,
/// and external interruption points; the interruption points between pages are not members.
fn member_count(q: &Q, p: &Potential) -> usize {
    let mut members: BTreeSet<(ew_derive::ew_ids::Uuid, u16)> = BTreeSet::new();
    for cp in &p.cps {
        let Some(f) = q.functions().get(cp.function) else {
            continue;
        };
        match (&f.kind, f.device) {
            (FunctionKind::Ip(ip), _) => {
                if ip.external.is_some() {
                    members.insert((cp.function.as_uuid(), cp.idx));
                }
            }
            (_, Some(d)) if q.devices().get(d).is_some_and(|d| d.aspects.terminal.is_some()) => {
                members.insert((d.as_uuid(), 0));
            }
            _ => {
                members.insert((cp.function.as_uuid(), cp.idx));
            }
        }
    }
    members.len()
}

fn potentials(q: &Q, wires: &Wires) -> Vec<Row> {
    let order = q.get::<ew_derive::DrawnPageOrder>(());
    let mut keyed: Vec<((NaturalKey, PotentialId), Row)> = wires
        .partition
        .potentials()
        .map(|p| {
            let names = &wires.names[&p.id];
            let attrs = q.get::<PotentialAttrsOf>(p.id);
            let flow = q.get::<PotentialFlow>(p.id);
            // The profile default: the first wire by ID whose value comes from a profile row or a definition.
            let profile = |s: &PropSource| {
                matches!(
                    s,
                    PropSource::ColourRow { .. } | PropSource::SizeRow { .. } | PropSource::PotentialDef(_)
                )
            };
            let mut by_id: Vec<&WireRec> = wires
                .wires
                .iter()
                .filter(|w| w.wire.potentials.contains(&p.id) && w.wire.bond.is_none())
                .collect();
            by_id.sort_by_key(|w| w.wire.id);
            let colour = by_id
                .iter()
                .find_map(|w| w.props.colour.as_ref().filter(|c| profile(&c.source)));
            let csa = by_id
                .iter()
                .find_map(|w| w.props.csa.as_ref().filter(|c| profile(&c.source)));
            let mut pages: BTreeSet<(usize, Id<ew_derive::ew_schema::Page>)> = BTreeSet::new();
            for w in &wires.wires {
                if w.wire.potentials.contains(&p.id) {
                    for page in &w.wire.pages {
                        pages.insert((order.position(PageRef::Drawn(*page)).unwrap_or(usize::MAX), *page));
                    }
                }
            }
            // The pages of its drawn bus lines too, as the potential overview counts them (FT-105).
            for page in ew_derive::bus_line_pages(q, &wires.partition, p.id) {
                pages.insert((order.position(PageRef::Drawn(page)).unwrap_or(usize::MAX), page));
            }
            let protected = flow.as_ref().as_ref().and_then(|f| f.protected_by);
            let protected_device = protected.and_then(|f| device_of(q, f));
            let protected_cp = flow.as_ref().as_ref().and_then(|f| {
                let up = f.upstream.as_ref()?;
                if Some(up.function) == f.protected_by {
                    Some(conn::cp_designation(q, CpRef::new(up.function, up.to_idx)))
                } else {
                    None
                }
            });
            let bond_ids: Vec<String> = wires
                .partition
                .bonds()
                .iter()
                .filter(|b| b.a == Some(p.id) || b.b == Some(p.id))
                .map(|b| uuid(b.bond))
                .collect();
            let mut r = Row::new(&POTENTIAL);
            r.set("id", potential_uuid(p.id))
                .set("name", names.name.text.as_str())
                .set("aliases", json_array(&names.aliases))
                .set("ptype", attrs.ptype.as_ref().map(|t| type_name(&t.value)))
                .set("voltage", attrs.voltage.as_ref().map(|v| decimal(v.value)))
                .set(
                    "ac_dc",
                    attrs.env.as_ref().and_then(|e| match e.value.ac_dc.as_ref() {
                        Some(AcDc::Ac) => Some("ac".to_owned()),
                        Some(AcDc::Dc) => Some("dc".to_owned()),
                        Some(other) => Some(enum_text(other)),
                        None => None,
                    }),
                )
                .set(
                    "circuit_class",
                    attrs.circuit_class.as_ref().map(|c| class_name(&c.value)),
                )
                .set("colour", colour.map(|c| colour_text(&c.value)))
                .set("colour_iec", colour.and_then(|c| colour_iec(&c.value)))
                .set("switched", flow.as_ref().as_ref().is_some_and(|f| f.switched))
                .set("protected_by_device_id", protected_device.map(uuid))
                .set("protected_by_tag", protected_device.and_then(|d| tag_short(q, d)))
                .set("protected_by_cp", protected_cp.and_then(non_empty))
                .set("bond_ids", json_array(&bond_ids))
                .set("member_count", member_count(q, p))
                .set(
                    "pages",
                    folios_json(q, pages.into_iter().map(|(_, pg)| PageRef::Drawn(pg))),
                );
            if let Some((value, unit)) = csa.and_then(|c| csa_parts(&c.value)) {
                r.set("default_csa", value).set("default_csa_unit", unit);
            }
            ((natural_key(&names.name.text), p.id), r)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.into_iter().map(|(_, r)| r).collect()
}

fn bonds(q: &Q, wires: &Wires) -> Vec<Row> {
    wires
        .partition
        .bonds()
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let name = |p: Option<PotentialId>| p.map(|p| wires.potential_name(p).to_owned());
            let wire = b.via.and_then(|v| wires.wires.iter().find(|w| w.wire.id == v));
            let mut r = Row::new(&BOND);
            r.set("id", uuid(b.bond))
                .set("label", format!("B{}", i + 1))
                .set("kind", enum_text(&b.kind))
                .set("a_potential_id", b.a.map(potential_uuid))
                .set("a_potential_name", name(b.a))
                .set("b_potential_id", b.b.map(potential_uuid))
                .set("b_potential_name", name(b.b))
                .set("wire_id", wire.map(|w| uuid(w.wire.id)))
                .set("names", json_array(&b.names));
            r.set(
                "pages",
                folios_json(
                    q,
                    wire.into_iter()
                        .flat_map(|w| w.wire.pages.iter().map(|p| PageRef::Drawn(*p))),
                ),
            );
            r
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// bom, message

fn bom(q: &Q, mode: BomMode, view: &'static View) -> Vec<Row> {
    q.get::<BomRows>(mode)
        .iter()
        .map(|b| {
            let mut tags: Vec<String> = b.devices.iter().filter_map(|d| tag_short(q, *d)).collect();
            tags.dedup();
            let mut r = Row::new(view);
            r.set("position", b.pos)
                .set("configured", b.configured)
                .set("part_id", uuid(b.key.part))
                .set("variant", b.key.variant.as_ref().map(|v| v.as_str().to_owned()))
                .set("manufacturer", manufacturer_name(&b.manufacturer))
                .set("part_number", b.part_number.as_str())
                .set("type", b.type_designation.clone())
                .set("description", ml_text(&b.description))
                .set("description_ml", ml_json(&b.description))
                .set("quantity", decimal(b.qty))
                .set("unit", enum_text(&b.unit))
                .set("tags", json_array(&tags))
                .set("folios", folios_json(q, b.pages.iter().copied()));
            if mode == BomMode::Assembly {
                // The head of a configured product when the row has exactly one; the assembly level is not
                // derived yet (NULL).
                let head = match (b.configured, b.devices.as_slice()) {
                    (true, [one]) => tag_short(q, *one),
                    _ => None,
                };
                r.set("assembly_tag", head);
            }
            r
        })
        .collect()
}

fn messages(report: Option<&CheckReport>) -> Vec<Row> {
    let Some(report) = report else {
        return Vec::new();
    };
    let mode = report.mode.name();
    report
        .findings
        .iter()
        .map(|f| {
            let meta = ew_checks::rule(f.rule);
            let ids: Vec<String> = f.targets.iter().map(|t| t.id.as_uuid().to_string()).collect();
            let (state, waiver) = match &f.state {
                FindingState::Open => ("open", None),
                FindingState::Waived { waiver } => ("waived", Some(waiver.clone())),
            };
            let mut r = Row::new(&MESSAGE);
            r.set("fingerprint", format!("{:032x}", f.fingerprint.0))
                .set("rule", f.rule)
                .set("pack", meta.map(|m| m.meta().pack))
                .set("severity", f.severity.name())
                .set("mode", mode)
                .set("state", state)
                .set("target_ids", json_array(&ids))
                .set("target_tags", non_empty(f.subject.clone()))
                .set("page_id", f.page.map(uuid))
                .set("message", f.message.as_str())
                .set("args", non_empty(f.discriminator.clone()))
                .set("waiver_id", waiver);
            r
        })
        .collect()
}
