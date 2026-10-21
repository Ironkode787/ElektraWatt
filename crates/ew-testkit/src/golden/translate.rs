// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Path (b) from the bootstrap batch: the batch's records, in dependency order, through the builder DSL.
//!
//! Every reference of a record is rewritten from the batch's ID to the ID the command minted for that
//! record before the record is handed to the builder (`Remap`), so each builder step sees a record of the
//! built project. The order:
//!
//! 1. the project settings members of [`super::SETTINGS_BY_COMMAND`];
//! 2. structure nodes (parents first), sections, external references, pages;
//! 3. devices (with their main normal function), terminals per strip, cables with their conductors;
//! 4. representations in batch order: the first representation of a function creates it (`ip.place`,
//!    `box.black` or `device.place` joining its device), the others place it again;
//! 5. connections: the three connections of a junction are drawn as one CP-to-CP connection that the
//!    third connection splits at the junction's point, so the junction is created by `connection.draw`;
//!    then `junction.edit` gives it its style, wiring and sequence; the other connections are drawn
//!    directly; then their overrides;
//! 6. potential definitions, bonds, report blocks.
//!
//! Not written (no command yet, FT numbers of doc 20 §20.2 and §23): PLC channels, device links, the PLC
//! configuration project, revisions, the import lineage (path (a) only), the valve-terminal assembly
//! (`parent`, `position`, the child tag scheme and the devices that carry it: FT-169) and the PLC aspects.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_import_tables::ew_interop_api::ImportBatch;
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, FnRole, Function, FunctionKind, Junction, Kind, Record,
    Representation,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::SETTINGS_BY_COMMAND;
use super::dsl::{BuildError, Builder, CommandSink, End, Placement, Result};

/// What path (b) built.
#[derive(Clone, Debug, Default)]
pub struct Built {
    /// The commands issued, in order, as `(name, params)`: replayed on the [`super::Shell`] with the same
    /// ID source they mint the same IDs.
    pub script: Vec<(String, serde_json::Value)>,
    /// How many commands of each name were issued.
    pub by_command: BTreeMap<String, usize>,
    /// What was not written because no command writes it yet, by record kind or `<kind>.<member>`, with
    /// the number of records or values.
    pub unwritten: BTreeMap<String, usize>,
    /// Batch record ID text to built record ID text, for every record path (b) created.
    pub ids: BTreeMap<String, String>,
}

impl Built {
    /// The commands as a command log (`.ewcmd`, doc 17 §3.5) with the given label.
    pub fn command_log(&self, label: &str) -> String {
        let mut out = serde_json::json!({"$schema": "ew.cmdlog/1", "label": label}).to_string();
        out.push('\n');
        for (name, params) in &self.script {
            out.push_str(&serde_json::json!({"cmd": name, "params": params}).to_string());
            out.push('\n');
        }
        out
    }

    /// The unwritten items as one line: `kind: n, …`.
    pub fn unwritten_summary(&self) -> String {
        self.unwritten
            .iter()
            .map(|(k, n)| format!("{k}: {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Batch ID text to built ID text.
#[derive(Default)]
struct Remap(BTreeMap<String, String>);

impl Remap {
    fn insert<T>(&mut self, batch: Id<T>, built: Id<T>) {
        self.0.insert(batch.to_string(), built.to_string());
    }

    fn get<T>(&self, batch: Id<T>) -> Option<Id<T>> {
        self.0
            .get(&batch.to_string())
            .and_then(|s| serde_json::from_value(Value::String(s.clone())).ok())
    }

    fn has<T>(&self, batch: Id<T>) -> bool {
        self.0.contains_key(&batch.to_string())
    }

    fn value(&self, v: &mut Value) {
        match v {
            Value::String(s) => {
                if let Some(to) = self.0.get(s.as_str()) {
                    *s = to.clone();
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|x| self.value(x)),
            Value::Object(m) => m.values_mut().for_each(|x| self.value(x)),
            _ => {}
        }
    }

    /// `record` with every known batch ID replaced by its built ID (its own `id` included, which the
    /// builder ignores).
    fn apply<T: Serialize + DeserializeOwned>(&self, record: &T) -> T {
        let mut v = serde_json::to_value(record).expect("a record serialises");
        self.value(&mut v);
        serde_json::from_value(v).expect("a remapped record deserialises")
    }
}

fn error(command: &str, message: String) -> BuildError {
    BuildError {
        command: command.into(),
        params: Value::Null,
        message,
    }
}

/// The child tag scheme (`tag.scheme = child`): the valve-terminal assembly's children (FT-169).
fn child_scheme(d: &Device) -> bool {
    serde_json::to_value(&d.tag)
        .ok()
        .and_then(|t| t.get("scheme").cloned())
        .is_some_and(|s| s == "child")
}

/// The reason of a `Rejected { source, reason }` error text without parenthesised IDs and without the
/// advice after `;`.
fn short_reason(message: &str) -> String {
    let reason = message
        .split_once("reason: \"")
        .map_or(message, |(_, r)| r.trim_end_matches(['"', ' ', '}']));
    let reason = reason.split(';').next().unwrap_or(reason);
    let mut out = String::new();
    let mut depth = 0usize;
    for ch in reason.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn endpoint_function(e: &Endpoint) -> Option<Id<Function>> {
    match e {
        Endpoint::Cp(f, _) => Some(*f),
        _ => None,
    }
}

/// Builds Golden A through commands from the bootstrap `batch` (path (b) of doc 20 §3.2), running every
/// command through `sink` against a session that holds the [`super::shell`] of `batch`.
///
/// # Errors
///
/// The first command that fails, with its parameters.
pub fn build<S: CommandSink + ?Sized>(batch: &ImportBatch, sink: &mut S) -> Result<Built> {
    let mut b = Builder::new(sink);
    let mut map = Remap::default();
    let recs = &batch.records;

    macro_rules! of {
        ($variant:ident) => {
            recs.iter().filter_map(|r| match r {
                Record::$variant(x) => Some(x),
                _ => None,
            })
        };
    }

    // 1. Settings.
    if let Some(project) = of!(Project).next() {
        for member in SETTINGS_BY_COMMAND {
            if let Some(v) = project.settings.extra.get(member) {
                b.setting(member, v.clone())?;
            }
        }
        if project
            .supply
            .as_ref()
            .and_then(|s| s.origin.as_ref())
            .is_some_and(|o| o.source.is_some())
        {
            b.not_written("project.supply.origin.source", 1);
        }
    }

    // 2. Structure, sections, external references, pages.
    let mut nodes: Vec<_> = of!(StructureNode).collect();
    let depth = |n: &ew_schema::StructureNode| {
        let mut d = 0;
        let mut p = n.parent;
        while let Some(id) = p {
            d += 1;
            p = of!(StructureNode).find(|x| x.id == id).and_then(|x| x.parent);
            if d > 64 {
                break;
            }
        }
        d
    };
    nodes.sort_by_key(|n| depth(n));
    for n in nodes {
        let id = b.structure_node(&map.apply(n))?;
        map.insert(n.id, id);
    }
    for s in of!(Section) {
        let id = b.section(&map.apply(s))?;
        map.insert(s.id, id);
    }
    for r in of!(ExternalRef) {
        let id = b.external_ref(&map.apply(r))?;
        map.insert(r.id, id);
    }
    for p in of!(Page) {
        let id = b.page(&map.apply(p))?;
        map.insert(p.id, id);
    }

    // 3. Devices, terminals, cables.
    let functions: Vec<&Function> = of!(Function).collect();
    let main_normal = |d: Id<Device>| {
        functions
            .iter()
            .copied()
            .find(|f| f.device == Some(d) && f.role == FnRole::Main && f.kind == FunctionKind::Normal)
    };
    let skipped: BTreeSet<Id<Device>> = of!(Device).filter(|d| child_scheme(d)).map(|d| d.id).collect();
    b.not_written(
        "device (child tag scheme: valve-terminal assembly, FT-169)",
        skipped.len(),
    );
    for d in of!(Device) {
        if skipped.contains(&d.id) || d.aspects.terminal.is_some() || d.aspects.cable.is_some() {
            continue;
        }
        let main = main_normal(d.id);
        let (id, f) = b.device(&map.apply(d), main.map(|f| map.apply(f)).as_ref())?;
        map.insert(d.id, id);
        if let (Some(main), Some(f)) = (main, f) {
            map.insert(main.id, f);
        }
    }
    // Terminals per strip in sequence order, one `terminal.add` per run of the same FnDef.
    let mut by_strip: BTreeMap<Id<Device>, Vec<&Device>> = BTreeMap::new();
    for d in of!(Device) {
        if let Some(t) = &d.aspects.terminal {
            by_strip.entry(t.strip).or_default().push(d);
        }
    }
    for (strip, mut terminals) in by_strip {
        terminals.sort_by_key(|t| t.aspects.terminal.as_ref().map_or(0, |a| a.seq));
        let strip_record = of!(Device)
            .find(|d| d.id == strip)
            .ok_or_else(|| error("terminal.add", format!("strip {strip} is not in the batch")))?;
        let strip_built = map
            .get(strip)
            .ok_or_else(|| error("terminal.add", format!("strip {strip} was not built")))?;
        let mut runs: Vec<(Id<ew_schema::FnDef>, Vec<(&Device, &Function)>)> = Vec::new();
        for t in terminals {
            let Some(f) = main_normal(t.id) else {
                b.not_written("device (terminal without a main function)", 1);
                continue;
            };
            match runs.last_mut() {
                Some((def, run)) if *def == f.def => run.push((t, f)),
                _ => runs.push((f.def, vec![(t, f)])),
            }
        }
        for (def, run) in runs {
            let mapped: Vec<Device> = run.iter().map(|(t, _)| map.apply(*t)).collect();
            let refs: Vec<&Device> = mapped.iter().collect();
            let made = b.terminals(&map.apply(strip_record), strip_built, def, &refs)?;
            for ((t, f), (dev, func)) in run.iter().zip(made) {
                map.insert(t.id, dev);
                map.insert(f.id, func);
                b.function_details(func, &map.apply(*f))?;
            }
        }
    }
    let conductors: Vec<&ew_schema::Conductor> = of!(Conductor).collect();
    for d in of!(Device) {
        if d.aspects.cable.is_none() || skipped.contains(&d.id) {
            continue;
        }
        let own: Vec<&ew_schema::Conductor> =
            conductors.iter().copied().filter(|c| c.cable == d.id).collect();
        let mapped: Vec<ew_schema::Conductor> = own.iter().map(|c| map.apply(*c)).collect();
        let refs: Vec<&ew_schema::Conductor> = mapped.iter().collect();
        let (id, made) = b.cable(&map.apply(d), &refs)?;
        map.insert(d.id, id);
        for (c, m) in own.iter().zip(made) {
            map.insert(c.id, m);
        }
    }

    // 4. Representations.
    let by_id: BTreeMap<Id<Function>, &Function> = functions.iter().map(|f| (f.id, *f)).collect();
    let mut skipped_fns: BTreeSet<Id<Function>> = BTreeSet::new();
    for f in &functions {
        if f.device.is_some_and(|d| skipped.contains(&d)) {
            skipped_fns.insert(f.id);
        }
    }
    for r in of!(Representation) {
        let rep: &Representation = r;
        let [fid] = rep.functions.as_slice() else {
            b.not_written("representation (more than one function)", 1);
            continue;
        };
        if skipped_fns.contains(fid) {
            b.not_written("representation (of a child-scheme device)", 1);
            continue;
        }
        if !rep.pin_map.as_slice().is_empty() {
            b.not_written("representation.pin_map", 1);
        }
        let f = by_id
            .get(fid)
            .ok_or_else(|| error("device.place", format!("function {fid} is not in the batch")))?;
        let placement = if let Some(built) = map.get(*fid) {
            if matches!(f.kind, FunctionKind::Ip(_)) {
                b.not_written(
                    "representation (second representation of an interruption point)",
                    1,
                );
                continue;
            }
            Placement::Existing {
                function: built,
                kind: f.kind.clone(),
            }
        } else if let FunctionKind::Ip(spec) = &f.kind {
            Placement::Ip {
                def: f.def,
                spec: map.apply(spec),
            }
        } else {
            let Some(device) = f.device.and_then(|d| map.get(d)) else {
                b.not_written("representation (function without a built device)", 1);
                continue;
            };
            Placement::Join {
                device,
                def: f.def,
                role: f.role.clone(),
                kind: f.kind.clone(),
            }
        };
        let created_now = !matches!(placement, Placement::Existing { .. });
        let (rep_id, func) = match b.place(&map.apply(rep), &placement) {
            Ok(x) => x,
            Err(e) if e.message.starts_with("Rejected") => {
                // a representation the command layer refuses (a domain rule the bootstrap does not apply)
                b.not_written(
                    &format!("representation (refused: {})", short_reason(&e.message)),
                    1,
                );
                continue;
            }
            Err(e) => return Err(e),
        };
        map.insert(rep.id, rep_id);
        if created_now {
            map.insert(*fid, func);
            b.function_details(func, &map.apply(*f))?;
        }
    }
    let unplaced = functions
        .iter()
        .filter(|f| !map.has(f.id) && !skipped_fns.contains(&f.id))
        .count();
    b.not_written("function (no representation built)", unplaced);
    b.not_written("function (of a child-scheme device)", skipped_fns.len());

    // 5. Connections and junctions.
    let connections: Vec<&Connection> = of!(Connection).collect();
    let junctions: Vec<&Junction> = of!(Junction).collect();
    let touches_skipped = |c: &Connection| {
        [&c.a, &c.b]
            .into_iter()
            .filter_map(endpoint_function)
            .any(|f| skipped_fns.contains(&f) || !by_id.contains_key(&f))
    };
    let mut done: BTreeSet<Id<Connection>> = BTreeSet::new();
    let end = |map: &Remap, e: &Endpoint| -> Option<End> {
        match e {
            Endpoint::Cp(f, idx) => map.get(*f).map(|f| End::Cp(f, *idx)),
            Endpoint::Junction(j) => map.get(*j).map(End::Junction),
            _ => None,
        }
    };
    for j in &junctions {
        let seq: Vec<&Connection> = j
            .sequence
            .iter()
            .filter_map(|id| connections.iter().copied().find(|c| c.id == *id))
            .collect();
        let at_j = |e: &Endpoint| matches!(e, Endpoint::Junction(x) if *x == j.id);
        let into = seq.iter().copied().find(|c| at_j(&c.b) && !at_j(&c.a));
        let out = seq
            .iter()
            .copied()
            .find(|c| at_j(&c.a) && !at_j(&c.b) && Some(c.id) != into.map(|x| x.id));
        let (Some(x), Some(y)) = (into, out) else {
            b.not_written("junction (no connection into and out of it)", 1);
            continue;
        };
        if seq.len() < 3 || seq.iter().any(|c| touches_skipped(c)) {
            b.not_written("junction (fewer than three built connections)", 1);
            continue;
        }
        let page = j.page;
        let (Some(xa), Some(yb)) = (end(&map, &x.a), end(&map, &y.b)) else {
            b.not_written("junction (an end not built)", 1);
            continue;
        };
        let page_built = map.get(page);
        // x: CP -> CP of y; the third connection splits it at the junction's point.
        let temp = b.connect(&xa, &yb, page_built, &map.apply(&x.kind))?;
        let mut rest = seq.iter().copied().filter(|c| c.id != x.id && c.id != y.id);
        let z = rest
            .next()
            .ok_or_else(|| error("connection.draw", "a junction of fewer than three".into()))?;
        let on = End::On(temp.connection, j.at);
        let (za, zb) = if at_j(&z.a) {
            (on, end(&map, &z.b))
        } else {
            (end(&map, &z.a).unwrap_or(on), Some(on))
        };
        let Some(zb) = zb else {
            return Err(error(
                "connection.draw",
                format!("an end of {} is not built", z.id),
            ));
        };
        let drawn = b.connect(&za, &zb, page_built, &map.apply(&z.kind))?;
        let (Some(jid), Some(second)) = (drawn.junction, drawn.split) else {
            return Err(error("connection.draw", "the split created no junction".into()));
        };
        map.insert(j.id, jid);
        map.insert(x.id, temp.connection);
        map.insert(y.id, second);
        map.insert(z.id, drawn.connection);
        if map.apply(&y.kind) != map.apply(&x.kind) {
            b.not_written("connection.kind (second half of a split connection)", 1);
        }
        for c in rest {
            let (Some(a), Some(bb)) = (end(&map, &c.a), end(&map, &c.b)) else {
                return Err(error(
                    "connection.draw",
                    format!("an end of {} is not built", c.id),
                ));
            };
            let d = b.connect(&a, &bb, c.page.and_then(|p| map.get(p)), &map.apply(&c.kind))?;
            map.insert(c.id, d.connection);
        }
        for c in &seq {
            done.insert(c.id);
        }
    }
    for c in &connections {
        if done.contains(&c.id) {
            continue;
        }
        if touches_skipped(c) {
            b.not_written("connection (to a child-scheme device)", 1);
            continue;
        }
        let (Some(a), Some(bb)) = (end(&map, &c.a), end(&map, &c.b)) else {
            b.not_written("connection (an end not built)", 1);
            continue;
        };
        let kind: ConnectionKind = map.apply(&c.kind);
        let d = b.connect(&a, &bb, c.page.and_then(|p| map.get(p)), &kind)?;
        map.insert(c.id, d.connection);
        if !c.route.is_empty() {
            b.not_written("connection.route", 1);
        }
    }
    for c in &connections {
        if let Some(id) = map.get(c.id) {
            b.connection_details(id, &map.apply(*c))?;
        }
    }
    for j in &junctions {
        if let Some(id) = map.get(j.id) {
            b.junction(id, &map.apply(*j))?;
        }
    }

    // 6. Potentials, bonds, report blocks.
    for d in of!(PotentialDef) {
        let id = b.potential(&map.apply(d))?;
        map.insert(d.id, id);
    }
    for x in of!(Bond) {
        let id = b.bond(&map.apply(x))?;
        map.insert(x.id, id);
    }
    for x in of!(ReportBlock) {
        let id = b.report_block(&map.apply(x))?;
        map.insert(x.id, id);
    }

    // Kinds no command writes yet.
    for r in recs {
        let what = match r.kind() {
            Some(Kind::Channel) => "channel",
            Some(Kind::DeviceLink) => "device_link",
            Some(Kind::PlcConfigProject) => "plc_config_project",
            Some(Kind::ImportLineage) | Some(Kind::Project) => continue,
            Some(
                Kind::StructureNode
                | Kind::Section
                | Kind::ExternalRef
                | Kind::Page
                | Kind::Device
                | Kind::Function
                | Kind::Representation
                | Kind::Connection
                | Kind::Junction
                | Kind::Conductor
                | Kind::PotentialDef
                | Kind::Bond
                | Kind::ReportBlock,
            ) => continue,
            Some(_) => "other kind",
            None => "revision or other kind without a schema type",
        };
        b.not_written(what, 1);
    }

    let (script, unwritten) = b.finish();
    let mut by_command = BTreeMap::new();
    for (name, _) in &script {
        *by_command.entry(name.clone()).or_default() += 1;
    }
    Ok(Built {
        script,
        by_command,
        unwritten,
        ids: map.0,
    })
}
