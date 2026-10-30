// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 5: macros in the session (D12, D13; doc 07 §3.5, §4.6). [`Session::save_macro`] writes a selection
//! (a window macro) or whole pages (a page macro) into a writable library as a macro document, a library write that
//! is confirmed in the shell and not undoable; [`Session::insert_macro`] composes `macro.insert` with the macro and the
//! closure documents the project lacks, so the command replays with no library installed (D7);
//! [`Session::macro_conflicts`] lists on the preview what the user decides on before Apply (FR-MAC-005).
//!
//! A library write is a front-end operation (§13.2 item 7): it takes the session by `&mut` and the registry through
//! [`Session::libraries_mut`]; no command, command log, script or MCP tool reaches it.

use std::collections::{BTreeMap, BTreeSet};

use ew_commands::commands::{Fragment, InsertMacro, binding_field_ok, coerce_value, instantiate};
use ew_derive::ew_ids::{Id, Uuid, v5};
use ew_derive::{DeviceTag, Q};
use ew_mltext::{Ident, MlText};
use ew_schema::{
    Device, Endpoint, Function, Kind, LibCopy, LibDocument, LibRef, MacroDef, MacroKind, MacroVariant, Page,
    PlaceholderGroup, Record, Representation, Slot, StructureAssignment, ValueSet, VarDef, VarType,
};
use ew_units::Pt2;
use serde_json::{Value, json};

use crate::output::clipboard::node_path;
use crate::output::{CopyError, parse_fragment};
use crate::{AnyCommand, Session, library};

/// What a macro is saved from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MacroScope {
    /// A window macro: a selection on one page (as Copy takes it).
    Window(Vec<Uuid>),
    /// A page macro: whole pages, in the order given.
    Pages(Vec<Uuid>),
}

/// What [`Session::save_macro`] saves (doc 07 §3.5; the Save as macro… card of part 6 fills it).
#[derive(Clone, PartialEq, Debug)]
pub struct MacroSpec {
    /// The macro's key (`meta.key`): a later save of the same key into the same library is the next minor version
    /// of the same item.
    pub key: String,
    /// The name.
    pub name: MlText,
    /// The description.
    pub description: MlText,
    /// The selection or the pages.
    pub scope: MacroScope,
    /// A window macro's insertion point, snapped down to the macro's grid; default the selection's anchor.
    pub origin: Option<Pt2>,
    /// The devices whose frozen tag stays ("keep tag"); every other frozen tag is cleared.
    pub keep_tags: Vec<Uuid>,
    /// The variables; a binding names a record of the scope by its ID (the fragment keeps the source IDs as its
    /// local IDs) and a field path `<kind>.<path>` of the field catalogue.
    pub variables: Vec<VarDef>,
    /// The placeholder groups, by the records' IDs.
    pub groups: Vec<PlaceholderGroup>,
    /// The value sets.
    pub value_sets: Vec<ValueSet>,
}

/// A saved macro ([`Session::save_macro`]).
#[derive(Clone, PartialEq, Debug)]
pub struct SavedMacro {
    /// The macro in its library from the version written (`^version`; a library holds the latest version of an
    /// item, so an exact requirement would stop resolving at the next save of the key).
    pub reference: LibRef,
    /// The copy ID the macro has in a project.
    pub copy: Uuid,
    /// The version written: 1.0.0, or the next minor version of the key's item.
    pub version: String,
    /// The open ends: the connection points of connections to records outside the scope, which the macro drops
    /// (`-102G1:1`), in text order.
    pub open_ends: Vec<String>,
    /// The closure references no installed library holds: the macro inserts only into a project that holds them.
    pub unresolved: Vec<LibRef>,
    /// The library write.
    pub write: library::WriteReport,
}

/// Why a macro was not saved.
#[derive(Clone, PartialEq, Debug)]
pub enum MacroError {
    /// The selection could not be copied.
    Copy(CopyError),
    /// A page macro names no page, or a page that is not a drawn page of this project, or pages of different
    /// grids.
    Pages(String),
    /// The specification is not consistent: where (`variables/0/bindings/1`) and why.
    Spec {
        /// The member of the specification.
        at: String,
        /// The reason.
        message: String,
    },
    /// The library refused the write, or is not installed.
    Library(library::LibraryError),
}

impl std::fmt::Display for MacroError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MacroError::Copy(e) => write!(f, "{e}"),
            MacroError::Pages(m) => f.write_str(m),
            MacroError::Spec { at, message } => write!(f, "{at}: {message}"),
            MacroError::Library(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for MacroError {}

/// Where [`Session::insert_macro`] inserts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InsertAt {
    /// The target page of a window macro, or the page after which a page macro's pages go.
    pub page: Uuid,
    /// A window macro's anchor point on the page (a grid point); ignored for a page macro.
    pub at: Option<Pt2>,
}

/// The values of an insertion: a value set and literals that win over it.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MacroValues {
    /// The value set, by name.
    pub value_set: Option<String>,
    /// Literal values by variable name.
    pub values: BTreeMap<String, Value>,
}

/// One conflict of an insertion (FR-MAC-005): information the user decides on, never a refusal.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MacroConflict {
    /// A prevent rule refuses the insertion until the user resolves it (a second source point of an interruption
    /// point, EW-CORE-052): the rule and its finding. The command is refused while one is listed.
    Refused {
        /// The rule ID.
        rule: String,
        /// The finding.
        message: String,
    },
    /// A new device prints a tag another device prints.
    Tag(String),
    /// A new interruption point pairs with points that were there before: its name and how many.
    Pairs {
        /// The point's name.
        name: String,
        /// The existing points it pairs with.
        with: usize,
    },
    /// A PLC address a new channel takes is taken already (the address text).
    Address(String),
    /// A new device carries a process tag another device carries.
    ProcessTag(String),
}

impl std::fmt::Display for MacroConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MacroConflict::Refused { rule, message } => write!(f, "{rule}: {message}"),
            MacroConflict::Tag(t) => write!(f, "tag {t} is used twice"),
            MacroConflict::Pairs { name, with } => {
                write!(f, "interruption point {name} pairs with {with} existing point(s)")
            }
            MacroConflict::Address(a) => write!(f, "PLC address {a} is used twice"),
            MacroConflict::ProcessTag(p) => write!(f, "process tag {p} is used twice"),
        }
    }
}

fn spec(at: impl Into<String>, message: impl Into<String>) -> MacroError {
    MacroError::Spec {
        at: at.into(),
        message: message.into(),
    }
}

/// The item ID of the macro `key` in `library`: `v5(library, "macro" ‖ key)`, so a later save of the key is the same
/// item.
pub fn macro_item_id(library: Uuid, key: &str) -> Uuid {
    Uuid::new_v5(&library, format!("macro\u{1f}{key}").as_bytes())
}

/// The next minor version after `v` (`1.2.3` → `1.3.0`); 1.0.0 when `v` does not parse; `None` when the minor
/// number is the largest one (a held version read from a library file has no next minor version then).
fn next_minor(v: &str) -> Option<String> {
    let core = v.split(['-', '+']).next().unwrap_or_default();
    let parts: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    match parts.as_deref() {
        Some([a, b, _]) => Some(format!("{a}.{}.0", b.checked_add(1)?)),
        _ => Some("1.0.0".to_owned()),
    }
}

/// Clears the structure slots of `s` that equal the page's (doc 07 §3.5: page-relative, the record inherits the
/// target page's structure on insertion).
fn page_relative(s: &mut StructureAssignment, page: &StructureAssignment) {
    let pairs = [
        (&mut s.functional_assignment, &page.functional_assignment),
        (&mut s.function, &page.function),
        (&mut s.installation_site, &page.installation_site),
        (&mut s.location, &page.location),
        (&mut s.document_kind, &page.document_kind),
        (&mut s.user_defined, &page.user_defined),
        (&mut s.type_, &page.type_),
    ];
    for (slot, of_page) in pairs {
        if slot.is_some() && slot == of_page {
            *slot = None;
        }
    }
}

/// The structure nodes a structure assignment names.
fn nodes_of(s: &StructureAssignment) -> impl Iterator<Item = Id<ew_schema::StructureNode>> + '_ {
    [
        &s.functional_assignment,
        &s.function,
        &s.installation_site,
        &s.location,
        &s.document_kind,
        &s.user_defined,
        &s.type_,
    ]
    .into_iter()
    .filter_map(|slot| match slot {
        Some(Slot::Node(n)) => Some(*n),
        _ => None,
    })
}

fn record_value(r: Record) -> Value {
    serde_json::to_value(r).expect("a record has a JSON form")
}

/// The fragment-local IDs of a fragment's records and pages, with their kinds.
fn local_kinds(f: &Fragment) -> BTreeMap<Uuid, Kind> {
    f.records
        .iter()
        .chain(&f.pages)
        .filter_map(|r| {
            let id = r.get("id")?.as_str()?.parse().ok()?;
            let kind = Kind::from_name(r.get("$k")?.as_str()?)?;
            Some((id, kind))
        })
        .collect()
}

impl Session {
    /// The project's records of a window selection or of whole pages, as a macro fragment (doc 07 §3.5): the
    /// fragment Copy writes, widened by the PLC channels of its modules and the device links between its devices
    /// (ADR-0011 amendment 5); a page macro adds its page records. Returns the fragment and the source pages.
    fn macro_fragment(&self, scope: &MacroScope) -> Result<(Fragment, Vec<Page>), MacroError> {
        let q = self.query();
        let parse = |ids: &[Uuid]| -> Result<Fragment, MacroError> {
            let copied = self.copy_fragment(ids).map_err(MacroError::Copy)?;
            let parsed = parse_fragment(&copied.text).map_err(|e| MacroError::Pages(e.to_string()))?;
            serde_json::from_value(parsed.params).map_err(|e| MacroError::Pages(e.to_string()))
        };
        let (mut fragment, pages) = match scope {
            MacroScope::Window(ids) => {
                let f = parse(ids)?;
                let page = f
                    .records
                    .iter()
                    .find_map(|r| r.get("page").and_then(Value::as_str))
                    .and_then(|p| p.parse::<Uuid>().ok())
                    .and_then(|p| q.pages().get(Id::from_uuid(p)).map(|p| (**p).clone()));
                (f, page.into_iter().collect())
            }
            MacroScope::Pages(ids) => {
                if ids.is_empty() {
                    return Err(MacroError::Pages("a page macro needs at least one page".into()));
                }
                let mut pages: Vec<Page> = Vec::new();
                let mut merged: Option<Fragment> = None;
                let mut seen: BTreeSet<Uuid> = BTreeSet::new();
                for id in ids {
                    let page = q
                        .pages()
                        .get(Id::from_uuid(*id))
                        .map(|p| (**p).clone())
                        .ok_or_else(|| MacroError::Pages(format!("no drawn page {id}")))?;
                    if pages.iter().any(|p| p.id == page.id) {
                        return Err(MacroError::Pages(format!("page {id} is named twice")));
                    }
                    if pages.first().is_some_and(|p| p.grid != page.grid) {
                        return Err(MacroError::Pages(
                            "the pages of a page macro share one connection grid".into(),
                        ));
                    }
                    let content: Vec<Uuid> = q
                        .idx()
                        .reps_by_page(page.id)
                        .map(|r| r.as_uuid())
                        .chain(q.idx().connections_by_page(page.id).map(|c| c.as_uuid()))
                        .chain(q.idx().junctions_by_page(page.id).map(|j| j.as_uuid()))
                        .chain(
                            q.page_items()
                                .iter()
                                .filter(|(_, i)| i.page == page.id)
                                .map(|(i, _)| i.as_uuid()),
                        )
                        .chain(
                            q.structure_boxes()
                                .iter()
                                .filter(|(_, b)| b.page == page.id)
                                .map(|(b, _)| b.as_uuid()),
                        )
                        .collect();
                    let part = match self.copy_fragment(&content) {
                        Err(CopyError::Empty) => None,
                        Err(e) => return Err(MacroError::Copy(e)),
                        Ok(_) => Some(parse(&content)?),
                    };
                    let target = merged.get_or_insert_with(|| Fragment {
                        format: ew_schema::FORMAT.to_owned(),
                        project: q.project().id.cast(),
                        grid: [page.grid.num(), page.grid.den()],
                        page_kind: serde_json::to_value(&page.kind)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_owned))
                            .unwrap_or_default(),
                        anchor: Pt2::new(ew_units::Nm(0), ew_units::Nm(0)),
                        records: Vec::new(),
                        copies: Vec::new(),
                        nodes: Vec::new(),
                        pages: Vec::new(),
                        lib: Vec::new(),
                    });
                    if let Some(part) = part {
                        for r in part.records {
                            if let Some(id) = r.get("id").and_then(Value::as_str).and_then(|s| s.parse().ok())
                                && seen.insert(id)
                            {
                                target.records.push(r);
                            }
                        }
                        for c in part.copies {
                            if !target.copies.contains(&c) {
                                target.copies.push(c);
                            }
                        }
                        for n in part.nodes {
                            if !target.nodes.iter().any(|m| m.id == n.id) {
                                target.nodes.push(n);
                            }
                        }
                    }
                    pages.push(page);
                }
                (merged.expect("one page at least"), pages)
            }
        };
        // The PLC channels of the fragment's modules and the device links between its devices.
        let devices: BTreeSet<Uuid> = local_kinds(&fragment)
            .into_iter()
            .filter(|(_, k)| *k == Kind::Device)
            .map(|(id, _)| id)
            .collect();
        let mut extra: Vec<Record> = Vec::new();
        for (_, ch) in q.table::<ew_schema::Channel>().iter() {
            if devices.contains(&ch.module.as_uuid()) {
                let mut ch = (**ch).clone();
                ch.source_ref = None;
                extra.push(Record::Channel(ch));
            }
        }
        for (_, l) in q.table::<ew_schema::DeviceLink>().iter() {
            if devices.contains(&l.from.as_uuid()) && devices.contains(&l.to.as_uuid()) {
                let mut l = (**l).clone();
                l.source_ref = None;
                extra.push(Record::DeviceLink(l));
            }
        }
        fragment.records.extend(extra.into_iter().map(record_value));
        // A page macro's page records; their forms and structure nodes travel with them.
        for p in &pages {
            if !matches!(scope, MacroScope::Pages(_)) {
                break;
            }
            let mut p = p.clone();
            p.source_ref = None;
            for n in nodes_of(&p.structure) {
                if fragment.nodes.iter().any(|m| m.id == n) {
                    continue;
                }
                let Some(node) = q.structure_nodes().get(n) else {
                    continue;
                };
                fragment.nodes.push(ew_commands::commands::FragmentNode {
                    id: n,
                    aspect: serde_json::to_value(&node.aspect)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    path: node_path(&q, n),
                    aliases: node.aliases.iter().map(|a| a.as_str().to_owned()).collect(),
                });
            }
            if let Some(form) = p.form.and_then(|f| q.forms().get(f).cloned()) {
                let v = serde_json::to_value(LibDocument::Form((*form).clone())).expect("a form copy");
                if !fragment.copies.contains(&v) {
                    fragment.copies.push(v);
                }
            }
            fragment.pages.push(record_value(Record::Page(p)));
        }
        Ok((fragment, pages))
    }

    /// The open ends of a fragment: the CPs of the project's connections that end at a function of the fragment but
    /// are not in it, as `<tag>:<cp index>`, sorted.
    fn open_ends(&self, q: &Q, fragment: &Fragment) -> Vec<String> {
        let locals = local_kinds(fragment);
        let functions: BTreeSet<Id<Function>> = locals
            .iter()
            .filter(|(_, k)| **k == Kind::Function)
            .map(|(id, _)| Id::from_uuid(*id))
            .collect();
        let mut out = BTreeSet::new();
        for (id, c) in q.connections().iter() {
            if locals.contains_key(&id.as_uuid()) {
                continue;
            }
            for e in [&c.a, &c.b] {
                if let Endpoint::Cp(f, idx) = e
                    && functions.contains(f)
                {
                    let tag = q
                        .functions()
                        .get(*f)
                        .and_then(|func| func.device)
                        .and_then(|d| q.get::<DeviceTag>(d).as_ref().as_ref().map(|t| t.full.clone()))
                        .unwrap_or_else(|| f.to_string());
                    out.insert(format!("{tag}:{idx}"));
                }
            }
        }
        out.into_iter().collect()
    }

    /// Saves a selection or pages as a macro into the writable library `target` (doc 07 §3.5, FR-MAC-001,
    /// FR-MAC-002, FR-MAC-007; WP-S1.19 D12): a library write, not a project command. The fragment keeps the source
    /// IDs as its local IDs; structure references equal to the source page's are cleared (page-relative: the records
    /// inherit the target page's structure); frozen tags are cleared unless kept; connections to records outside the
    /// scope are dropped and their CPs listed; the closure's library copies become references (`lib`, `^version`);
    /// the variables, groups and value sets are checked against the fragment and the field catalogue, and every
    /// value set is applied once to prove it. The item is `v5(target, "macro" ‖ key)` at 1.0.0, or the next minor
    /// version when the library holds the key; the licence is the library's. Equal inputs give an equal document.
    ///
    /// # Errors
    ///
    /// [`MacroError::Copy`] or [`MacroError::Pages`] for a scope that does not copy; [`MacroError::Spec`] for a
    /// variable, group or value set that does not fit, or at `version` for a held version with no next minor; [`MacroError::Library`] for a library that is not installed,
    /// read-only, locked, or a write that fails.
    pub fn save_macro(&mut self, target: Uuid, spec_in: &MacroSpec) -> Result<SavedMacro, MacroError> {
        let info = self
            .libraries()
            .libraries()
            .into_iter()
            .find(|l| l.id == target)
            .ok_or(MacroError::Library(library::LibraryError::NotFound))?;
        if !info.writable {
            return Err(MacroError::Library(library::LibraryError::ReadOnly));
        }
        let key: Ident = spec_in
            .key
            .parse()
            .map_err(|_| spec("key", format!("`{}` is not an identifier", spec_in.key)))?;
        let (mut fragment, pages) = self.macro_fragment(&spec_in.scope)?;
        let q = self.query();
        let open_ends = self.open_ends(&q, &fragment);
        // Records: page-relative structure, frozen tags.
        let page_of = |r: &Value| -> Option<&Page> {
            let p = r.get("page").and_then(Value::as_str)?.parse::<Uuid>().ok()?;
            pages.iter().find(|page| page.id.as_uuid() == p)
        };
        let first_page = pages.first().cloned();
        for r in &mut fragment.records {
            let page = page_of(r).cloned().or_else(|| first_page.clone());
            let Ok(mut record) = Record::from_value(r.clone()) else {
                continue;
            };
            let changed = match &mut record {
                Record::Device(d) => {
                    // a device has no page: its structure is relative to the page of its first representation
                    if let Some(p) = &page_device(&q, d, &pages).or_else(|| first_page.clone()) {
                        page_relative(&mut d.structure, &p.structure);
                    }
                    if !spec_in.keep_tags.contains(&d.id.as_uuid()) {
                        d.tag.frozen = None;
                    }
                    true
                }
                Record::StructureBox(b) => {
                    if let Some(p) = &page {
                        page_relative(&mut b.assigns, &p.structure);
                    }
                    true
                }
                _ => false,
            };
            if changed {
                *r = record_value(record);
            }
        }
        // The nodes still named.
        let named: BTreeSet<Id<ew_schema::StructureNode>> = fragment
            .records
            .iter()
            .chain(&fragment.pages)
            .filter_map(|r| Record::from_value(r.clone()).ok())
            .flat_map(|r| match r {
                Record::Device(d) => nodes_of(&d.structure).collect::<Vec<_>>(),
                Record::StructureBox(b) => nodes_of(&b.assigns).collect(),
                Record::Page(p) => nodes_of(&p.structure).collect(),
                _ => Vec::new(),
            })
            .collect();
        fragment.nodes.retain(|n| named.contains(&n.id));
        // The anchor.
        if let (Some(origin), MacroScope::Window(_)) = (spec_in.origin, &spec_in.scope) {
            let [num, den] = fragment.grid;
            let grid = ew_units::Grid::new(num, den).map_err(|e| spec("origin", e.to_string()))?;
            let floor = |v: ew_units::Nm| {
                let k = grid.snap(v);
                match grid.at(k) {
                    Some(p) if p.0 <= v.0 => Some(p),
                    _ => grid.at(k.saturating_sub(1)),
                }
            };
            let (Some(x), Some(y)) = (floor(origin.x), floor(origin.y)) else {
                return Err(spec("origin", "the point is outside the drawable range"));
            };
            fragment.anchor = Pt2::new(x, y);
        }
        // The closure: references in place of copies.
        let mut lib: Vec<LibRef> = Vec::new();
        for c in &fragment.copies {
            let (Some(l), Some(i), Some(v)) = (
                c.get("library_id").and_then(Value::as_str),
                c.get("item_id").and_then(Value::as_str),
                c.get("version").and_then(Value::as_str),
            ) else {
                continue;
            };
            let r: LibRef =
                serde_json::from_value(json!({"library": l, "item": i, "version_req": format!("^{v}")}))
                    .map_err(|e| spec("scope", format!("a library copy's reference: {e}")))?;
            if !lib.contains(&r) {
                lib.push(r);
            }
        }
        lib.sort_by_key(|r| (r.library.as_uuid(), r.item.as_uuid()));
        let unresolved: Vec<LibRef> = lib
            .iter()
            .filter(|r| self.libraries().resolve(r).is_err())
            .cloned()
            .collect();
        fragment.copies.clear();
        fragment.lib = lib;
        // Variables, groups, value sets.
        let locals = local_kinds(&fragment);
        let mut names = BTreeSet::new();
        for (i, var) in spec_in.variables.iter().enumerate() {
            if !names.insert(var.name.as_str()) {
                return Err(spec(format!("variables/{i}"), "the variable is named twice"));
            }
            for (j, b) in var.bindings.iter().enumerate() {
                let at = format!("variables/{i}/bindings/{j}");
                let kind = locals
                    .get(&b.record.as_uuid())
                    .ok_or_else(|| spec(&at, format!("record {} is not in the macro", b.record)))?;
                if !binding_field_ok(*kind, &b.field) {
                    return Err(spec(
                        &at,
                        format!("`{}` is no field of the field catalogue for a {kind}", b.field),
                    ));
                }
            }
            if let Some(d) = &var.default {
                coerce_value(&var.ty, d).map_err(|m| spec(format!("variables/{i}/default"), m))?;
            }
        }
        for (i, g) in spec_in.groups.iter().enumerate() {
            for m in &g.members {
                if !locals.contains_key(&m.as_uuid()) {
                    return Err(spec(
                        format!("groups/{i}"),
                        format!("record {m} is not in the macro"),
                    ));
                }
            }
            if let Some(inc) = &g.include {
                let ok = spec_in
                    .variables
                    .iter()
                    .any(|v| v.name.as_str() == inc && v.ty == VarType::Bool);
                if !ok {
                    return Err(spec(
                        format!("groups/{i}"),
                        format!("`{inc}` is no boolean variable"),
                    ));
                }
            }
        }
        let kind = match spec_in.scope {
            MacroScope::Window(_) => MacroKind::Window,
            MacroScope::Pages(_) => MacroKind::Page,
        };
        let rep_type = ew_schema::RepType::MultiLine;
        let rep_type = fragment
            .records
            .iter()
            .find_map(|r| match Record::from_value(r.clone()) {
                Ok(Record::Representation(rep)) => Some(rep.rep_type),
                _ => None,
            })
            .unwrap_or(rep_type);
        let mut def = MacroDef {
            meta: ew_schema::ItemMeta {
                key: Some(key.clone()),
                licence: Some(info.licence.clone()),
                ..ew_schema::ItemMeta::default()
            },
            name: spec_in.name.clone(),
            description: spec_in.description.clone(),
            kind,
            variants: vec![MacroVariant {
                rep_type,
                key: "A".parse().expect("an identifier"),
                fragment: serde_json::to_value(&fragment).expect("a fragment has a JSON form"),
                extra: ew_schema::Extra::new(),
            }],
            variables: spec_in.variables.clone(),
            groups: spec_in.groups.clone(),
            value_sets: Vec::new(),
            thumbnail: None,
            extra: ew_schema::Extra::new(),
        };
        let mut set_names = BTreeSet::new();
        for (i, set) in spec_in.value_sets.iter().enumerate() {
            if !set_names.insert(set.name.as_str()) {
                return Err(spec(format!("value_sets/{i}"), "the value set is named twice"));
            }
            for (name, v) in &set.values {
                let var = spec_in
                    .variables
                    .iter()
                    .find(|x| x.name.as_str() == name)
                    .ok_or_else(|| spec(format!("value_sets/{i}"), format!("no variable `{name}`")))?;
                coerce_value(&var.ty, v).map_err(|m| spec(format!("value_sets/{i}/{name}"), m))?;
            }
        }
        def.value_sets.clone_from(&spec_in.value_sets);
        // Every value set (and the defaults) applied once.
        let variant = def.variants[0].clone();
        instantiate(&def, &variant, None, &BTreeMap::new())
            .map_err(|(at, m)| spec(format!("variables{at}"), m))?;
        for (i, set) in def.value_sets.iter().enumerate() {
            instantiate(&def, &variant, Some(set.name.as_str()), &BTreeMap::new())
                .map_err(|(_, m)| spec(format!("value_sets/{i}"), m))?;
        }
        // The document.
        let item = macro_item_id(target, key.as_str());
        let held = self.libraries().resolve(&LibRef {
            library: Id::from_uuid(target),
            item: Id::from_uuid(item),
            version_req: "*".to_owned(),
            extra: ew_schema::Extra::new(),
        });
        let version = match held {
            Ok(LibDocument::Macro(c)) => next_minor(&c.version).ok_or_else(|| {
                spec(
                    "version",
                    format!("the held version {} has no next minor version", c.version),
                )
            })?,
            _ => "1.0.0".to_owned(),
        };
        let copy: Id<MacroDef> = v5::lib_copy_id(target, item);
        let mut doc = LibCopy {
            id: copy,
            v: 1,
            library_id: Id::from_uuid(target),
            item_id: Id::from_uuid(item),
            version: version.clone(),
            sha256: ew_schema::Sha256([0; 32]),
            item: def,
        };
        let hash = ew_commands::commands::library_copy_hash(&LibDocument::Macro(doc.clone()))
            .expect("a macro has a hash");
        doc.sha256 = ew_schema::Sha256(hash);
        let write = self
            .libraries_mut()
            .write(target, &[LibDocument::Macro(doc)])
            .map_err(MacroError::Library)?;
        Ok(SavedMacro {
            reference: LibRef {
                library: Id::from_uuid(target),
                item: Id::from_uuid(item),
                version_req: format!("^{version}"),
                extra: ew_schema::Extra::new(),
            },
            copy: copy.as_uuid(),
            version,
            open_ends,
            unresolved,
            write,
        })
    }

    /// `macro.insert` by reference (D7, D13): resolves the macro from [`Session::libraries`] and the closure items its
    /// fragment names (`lib`) that the project does not hold, each with its own closure, and builds the command. A
    /// closure item the project holds at any version is not resolved (the records keep naming the held copy); the
    /// macro's document is always carried, and the command reuses a held copy of the same hash.
    ///
    /// # Errors
    ///
    /// [`crate::ApiError::Library`] for the macro or a closure item the libraries do not resolve;
    /// [`crate::ApiError::NotAPart`]'s sibling [`crate::ApiError::NotAMacro`] when the reference resolves to
    /// another kind; [`crate::ApiError::Cmd`] with a refusal at `/value_set` for a value set the macro does not have
    /// (the message names it and the macro's sets).
    pub fn insert_macro(
        &self,
        m: &LibRef,
        at: &InsertAt,
        values: &MacroValues,
        instance: Uuid,
    ) -> Result<AnyCommand, crate::ApiError> {
        let lib_err = |r: &LibRef, error| crate::ApiError::Library {
            reference: format!("{}/{}@{}", r.library, r.item, r.version_req),
            error,
        };
        let doc = self.libraries().resolve(m).map_err(|e| lib_err(m, e))?;
        let LibDocument::Macro(copy) = &doc else {
            let id: Id<()> = v5::lib_copy_id(m.library.as_uuid(), m.item.as_uuid());
            return Err(crate::ApiError::NotAMacro(id.as_uuid()));
        };
        let q = self.query();
        let mut docs: Vec<Value> = vec![serde_json::to_value(&doc).expect("a library document")];
        let mut seen: BTreeSet<Uuid> = BTreeSet::new();
        for variant in &copy.item.variants {
            let refs: Vec<LibRef> = variant
                .fragment
                .get("lib")
                .and_then(|l| serde_json::from_value(l.clone()).ok())
                .unwrap_or_default();
            for r in refs {
                let id: Id<()> = v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid());
                if held_any(&q, id.as_uuid()) {
                    continue;
                }
                for d in self.libraries().closure(&r).map_err(|e| lib_err(&r, e))? {
                    let Some(id) = copy_id(&d) else { continue };
                    if held_any(&q, id) || !seen.insert(id) {
                        continue;
                    }
                    docs.push(serde_json::to_value(&d).expect("a library document"));
                }
            }
        }
        let cmd = InsertMacro {
            copies: docs,
            macro_copy: copy.id,
            page: Id::from_uuid(at.page),
            at: at.at,
            value_set: values
                .value_set
                .as_deref()
                .map(|s| {
                    let known = |n: &Ident| copy.item.value_sets.iter().any(|v| v.name == *n);
                    s.parse::<Ident>().ok().filter(known).ok_or_else(|| {
                        let names: Vec<&str> = copy.item.value_sets.iter().map(|v| v.name.as_str()).collect();
                        let has = if names.is_empty() {
                            "the macro has none".to_owned()
                        } else {
                            format!("the macro has {}", names.join(", "))
                        };
                        crate::ApiError::Cmd(crate::CmdError::invalid_params(
                            <InsertMacro as ew_commands::Command>::NAME,
                            "/value_set",
                            format!("no value set `{s}`: {has}"),
                        ))
                    })
                })
                .transpose()?,
            values: values.values.clone(),
            instance: Id::from_uuid(instance),
        };
        AnyCommand::of(&cmd).map_err(crate::ApiError::Cmd)
    }

    /// The conflicts of a previewed `macro.insert` (FR-MAC-005, doc 07 §3.5): read on a throwaway engine over the
    /// current snapshot without the prevent hook; the session is not touched. Every prevent finding the insertion
    /// causes (the command is refused until they are resolved), duplicate printed tags of the new devices, new
    /// interruption points that pair with points that were there, PLC addresses of new channels that other
    /// channels take, and duplicate process tags; in that order, each sorted.
    ///
    /// # Errors
    ///
    /// The command's refusal.
    pub fn macro_conflicts(&self, cmd: &AnyCommand) -> Result<Vec<MacroConflict>, crate::CmdError> {
        let before = self.query();
        let model = self.world().model.clone();
        let mut engine = ew_commands::Engine::new(model, ew_commands::EngineOptions::default());
        let preview = engine.preview(cmd)?;
        let after = self.query_over(preview.candidate());
        let touched: Vec<(Kind, ew_schema::AnyId)> = preview
            .diff()
            .changes
            .iter()
            .map(|c| (c.key.0, c.key.1))
            .collect();
        let refused: BTreeSet<(String, String)> = crate::checks::prevent(
            &Q::new(&self.world().model),
            &Q::new(preview.candidate()),
            &touched,
        )
        .into_iter()
        .map(|f| (f.rule.to_owned(), f.message))
        .collect();
        let new_devices: BTreeSet<Id<Device>> = after
            .devices()
            .iter()
            .map(|(id, _)| id)
            .filter(|id| !before.devices().contains(*id))
            .collect();
        let tag = |q: &Q, d: Id<Device>| q.get::<DeviceTag>(d).as_ref().as_ref().map(|t| t.full.clone());
        let mut out: Vec<MacroConflict> = refused
            .into_iter()
            .map(|(rule, message)| MacroConflict::Refused { rule, message })
            .collect();
        // tags
        let mut tags: BTreeMap<String, usize> = BTreeMap::new();
        for (id, _) in after.devices().iter() {
            if let Some(t) = tag(&after, id).filter(|t| !t.contains('?')) {
                *tags.entry(t).or_default() += 1;
            }
        }
        let mut dup: BTreeSet<String> = BTreeSet::new();
        for d in &new_devices {
            if let Some(t) = tag(&after, *d)
                && tags.get(&t).copied().unwrap_or(0) > 1
            {
                dup.insert(t);
            }
        }
        out.extend(dup.into_iter().map(MacroConflict::Tag));
        // interruption points
        let pairing = after.get::<ew_derive::IpPairingQ>(());
        let mut pairs: BTreeMap<String, usize> = BTreeMap::new();
        for (f, info) in &pairing.ips {
            if before.functions().contains(*f) {
                continue;
            }
            let old = pairing
                .partners(*f)
                .iter()
                .filter(|p| before.functions().contains(**p))
                .count();
            if old > 0 {
                *pairs.entry(info.name.clone()).or_default() += old;
            }
        }
        out.extend(
            pairs
                .into_iter()
                .map(|(name, with)| MacroConflict::Pairs { name, with }),
        );
        // PLC addresses
        let modules: Vec<Id<Device>> = after
            .devices()
            .iter()
            .filter(|(_, d)| d.aspects.plc_module.is_some())
            .map(|(id, _)| id)
            .collect();
        let mut by_text: BTreeMap<String, (usize, bool)> = BTreeMap::new();
        for m in modules {
            let new = new_devices.contains(&m);
            for e in after.get::<ew_derive::ChannelAddresses>(m).iter() {
                if let ew_derive::ChannelAddr::At { text, .. } = &e.address {
                    let slot = by_text.entry(text.clone()).or_default();
                    slot.0 += 1;
                    slot.1 |= new;
                }
            }
        }
        out.extend(
            by_text
                .into_iter()
                .filter(|(_, (n, new))| *n > 1 && *new)
                .map(|(t, _)| MacroConflict::Address(t)),
        );
        // process tags
        let mut pts: BTreeMap<String, (usize, bool)> = BTreeMap::new();
        for (id, d) in after.devices().iter() {
            if let Some(pt) = d
                .process_tag
                .as_ref()
                .map(|p| p.as_str().to_owned())
                .filter(|p| !p.is_empty())
            {
                let slot = pts.entry(pt).or_default();
                slot.0 += 1;
                slot.1 |= new_devices.contains(&id);
            }
        }
        out.extend(
            pts.into_iter()
                .filter(|(_, (n, new))| *n > 1 && *new)
                .map(|(t, _)| MacroConflict::ProcessTag(t)),
        );
        Ok(out)
    }
}

/// The page of a device: the page of its first representation among the fragment's pages.
fn page_device(q: &Q, d: &Device, pages: &[Page]) -> Option<Page> {
    let fns: BTreeSet<Id<Function>> = q.idx().functions_of_device(d.id).collect();
    q.reps()
        .iter()
        .filter(|(_, r): &(Id<Representation>, _)| r.functions.iter().any(|f| fns.contains(f)))
        .find_map(|(_, r)| pages.iter().find(|p| p.id == r.page).cloned())
}

/// The copy ID of a library document.
fn copy_id(d: &LibDocument) -> Option<Uuid> {
    Some(match d {
        LibDocument::Part(c) => c.id.as_uuid(),
        LibDocument::Symbol(c) => c.id.as_uuid(),
        LibDocument::Fndef(c) => c.id.as_uuid(),
        LibDocument::Form(c) => c.id.as_uuid(),
        LibDocument::Macro(c) => c.id.as_uuid(),
        LibDocument::Manufacturer(c) => c.id.as_uuid(),
        LibDocument::Unknown(_) => return None,
    })
}

/// Whether the project holds a library copy with this ID.
fn held_any(q: &Q, id: Uuid) -> bool {
    q.parts().contains(Id::from_uuid(id))
        || q.symbols().contains(Id::from_uuid(id))
        || q.fndefs().contains(Id::from_uuid(id))
        || q.forms().contains(Id::from_uuid(id))
        || q.manufacturers().contains(Id::from_uuid(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_minor_version_resets_the_patch() {
        assert_eq!(next_minor("1.0.0").as_deref(), Some("1.1.0"));
        assert_eq!(next_minor("1.4.7").as_deref(), Some("1.5.0"));
        assert_eq!(next_minor("one").as_deref(), Some("1.0.0"));
        // The largest minor number has no successor: refused, not an overflow.
        assert_eq!(next_minor(&format!("1.{}.0", u64::MAX)), None);
    }

    #[test]
    fn page_relative_clears_only_equal_slots() {
        let n1 = Id::from_uuid(Uuid::from_u128(1));
        let n2 = Id::from_uuid(Uuid::from_u128(2));
        let mut s = StructureAssignment {
            location: Some(Slot::Node(n1)),
            function: Some(Slot::Node(n2)),
            ..StructureAssignment::default()
        };
        let page = StructureAssignment {
            location: Some(Slot::Node(n1)),
            function: Some(Slot::Node(n1)),
            ..StructureAssignment::default()
        };
        page_relative(&mut s, &page);
        assert_eq!(s.location, None);
        assert_eq!(s.function, Some(Slot::Node(n2)));
    }
}
