// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `library.update` (`UpdateLibraryCopies`, WP-S1.19 D9, FR-PRT-014, FR-SYM-016) and the update classification of
//! doc 07 §4.5 ([`classify_update`]), which the session's update plan shows on the card.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_model::{AnyKey, AnyRow, EndpointKey};
use ew_schema::{
    AnyId, Bond, Channel, Connection, Device, Endpoint, Function, FunctionKind, LibCopy, LibDocument, Part,
    PotentialDef, Representation, Sha256,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::library::{check_copy, read_copy};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// The class of a library copy's change (doc 07 §4.5), whatever its declared version says. Ordered: `Patch` <
/// `Minor` < `Major`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeClass {
    /// Data only: texts, descriptions, order data, technical values, the item metadata.
    Patch,
    /// Structure or geometry that keeps every CP: a CP added, a moved CP or anchor, graphics, text slots, a
    /// template's CP selection, the part kind or sections, a macro's variants.
    Minor,
    /// A CP disappears or an index changes its designation; the FnDef or the category changes; a template
    /// function is added or removed; a macro variable is removed.
    Major,
}

/// One top-level member of a copy's item that differs (the card's field diff); `None` where the member is absent.
#[derive(Clone, PartialEq, Debug)]
pub struct FieldChange {
    /// The member name, as the `lib/` JSON form spells it.
    pub member: String,
    /// The held value.
    pub old: Option<Value>,
    /// The library's value.
    pub new: Option<Value>,
}

/// The classified difference between a held copy and the library's document of the same item.
#[derive(Clone, PartialEq, Debug)]
pub struct CopyDiff {
    /// The class (doc 07 §4.5).
    pub class: ChangeClass,
    /// The members that differ, in member-name order.
    pub fields: Vec<FieldChange>,
    /// Why the change is major (empty below major), in a fixed order.
    pub major_reasons: Vec<String>,
}

/// The item of a library document as a JSON object (empty for an unknown kind).
fn item_object(doc: &LibDocument) -> serde_json::Map<String, Value> {
    fn obj<T: Serialize>(item: &T) -> serde_json::Map<String, Value> {
        match serde_json::to_value(item) {
            Ok(Value::Object(m)) => m,
            _ => serde_json::Map::new(),
        }
    }
    match doc {
        LibDocument::Part(c) => obj(&c.item),
        LibDocument::Symbol(c) => obj(&c.item),
        LibDocument::Fndef(c) => obj(&c.item),
        LibDocument::Form(c) => obj(&c.item),
        LibDocument::Macro(c) => obj(&c.item),
        LibDocument::Manufacturer(c) => obj(&c.item),
        LibDocument::Unknown(_) => serde_json::Map::new(),
    }
}

/// The members of a kind whose change alone is a patch (doc 07 §4.5: "otherwise minor or patch"; the line drawn
/// here: data that no derived structure or geometry reads). Every other member is minor unless a major rule holds.
fn patch_members(doc: &LibDocument) -> &'static [&'static str] {
    match doc {
        LibDocument::Part(_) => &[
            "meta",
            "manufacturer",
            "part_number",
            "type_designation",
            "order_number",
            "gtin",
            "description",
            "tech_data",
            "free_props",
            "alternatives",
            "macros",
        ],
        LibDocument::Symbol(_) => &["meta", "name", "keywords", "iec60617_ref", "iso1219_ref"],
        LibDocument::Fndef(_) => &["meta"],
        LibDocument::Form(_) => &["meta", "name", "description"],
        LibDocument::Macro(_) => &["meta", "name", "description", "thumbnail"],
        LibDocument::Manufacturer(_) | LibDocument::Unknown(_) => &[],
    }
}

/// One connection point of a copy as the update sees it: the designation it is mapped by and the function CP
/// index a connection endpoint names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct CopyCp {
    pub(crate) designation: String,
    pub(crate) endpoint: u16,
}

/// The CPs of a copy by index: a FnDef's `cps` and a part's `cp_pattern` (an endpoint is the CP index itself), a
/// symbol's `cps` (an endpoint is the FnDef CP `fn_cp` the symbol CP draws; its "designation" is that index).
/// `None` for the kinds without CPs.
pub(crate) fn copy_cps(doc: &LibDocument) -> Option<BTreeMap<u16, CopyCp>> {
    Some(match doc {
        LibDocument::Fndef(c) => c
            .item
            .cps
            .iter()
            .map(|p| {
                (
                    p.idx,
                    CopyCp {
                        designation: p.designation.as_str().to_owned(),
                        endpoint: p.idx,
                    },
                )
            })
            .collect(),
        LibDocument::Part(c) => c
            .item
            .cp_pattern
            .iter()
            .map(|p| {
                (
                    p.idx,
                    CopyCp {
                        designation: p.designation.as_str().to_owned(),
                        endpoint: p.idx,
                    },
                )
            })
            .collect(),
        LibDocument::Symbol(c) => c
            .item
            .cps
            .iter()
            .map(|p| {
                (
                    p.idx,
                    CopyCp {
                        designation: format!("fn_cp {}", p.fn_cp),
                        endpoint: p.fn_cp,
                    },
                )
            })
            .collect(),
        _ => return None,
    })
}

/// Classifies the change from the held copy `old` to the library's document `new` of the same item (doc 07 §4.5),
/// whatever the declared versions say: major when a CP disappears or an index changes its designation (a FnDef's
/// `cps`, a part's `cp_pattern`, a symbol's `cps` by the FnDef CP each draws), the FnDef (a symbol's `fndef`) or the
/// category (a part's, a FnDef's) changes, a template function is added or removed (a part's `function_template` by
/// FnDef reference), or a macro variable is removed (by name); else minor when a member differs that is not a patch
/// member; else patch. The patch members are the data no derived structure or geometry reads: `meta` of every kind; a
/// part's manufacturer, numbers, description, technical data, free properties, alternatives and macro links; a
/// symbol's name, keywords and standard references; a form's name and description; a macro's name, description and
/// thumbnail; every member of a manufacturer. Equal items give patch with no fields.
pub fn classify_update(old: &LibDocument, new: &LibDocument) -> CopyDiff {
    let (a, b) = (item_object(old), item_object(new));
    let members: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    let fields: Vec<FieldChange> = members
        .into_iter()
        .filter(|m| a.get(*m) != b.get(*m))
        .map(|m| FieldChange {
            member: m.clone(),
            old: a.get(m).cloned(),
            new: b.get(m).cloned(),
        })
        .collect();
    let mut major = Vec::new();
    if let (Some(o), Some(n)) = (copy_cps(old), copy_cps(new)) {
        for (idx, cp) in &o {
            match n.get(idx) {
                None => major.push(format!("CP {idx} ({}) disappears", cp.designation)),
                Some(m) if m.designation != cp.designation => major.push(format!(
                    "CP {idx} changes its designation from {} to {}",
                    cp.designation, m.designation
                )),
                Some(_) => {}
            }
        }
    }
    let changed = |m: &str| fields.iter().any(|f| f.member == m);
    match (old, new) {
        (LibDocument::Symbol(_), LibDocument::Symbol(_)) if changed("fndef") => {
            major.push("the FnDef changes".to_owned());
        }
        (LibDocument::Part(_) | LibDocument::Fndef(_), _) if changed("category") => {
            major.push("the category changes".to_owned());
        }
        _ => {}
    }
    if let (LibDocument::Part(o), LibDocument::Part(n)) = (old, new) {
        let refs = |p: &Part| -> Vec<(ew_ids::Uuid, ew_ids::Uuid)> {
            let mut v: Vec<_> = p
                .function_template
                .iter()
                .map(|t| (t.fndef.library.as_uuid(), t.fndef.item.as_uuid()))
                .collect();
            v.sort_unstable();
            v
        };
        if refs(&o.item) != refs(&n.item) {
            major.push("a template function is added or removed".to_owned());
        }
    }
    if let (LibDocument::Macro(o), LibDocument::Macro(n)) = (old, new) {
        for v in &o.item.variables {
            if !n.item.variables.iter().any(|w| w.name == v.name) {
                major.push(format!("macro variable {} is removed", v.name.as_str()));
            }
        }
    }
    let patch = patch_members(old);
    let class = if !major.is_empty() {
        ChangeClass::Major
    } else if fields.iter().any(|f| !patch.contains(&f.member.as_str())) {
        ChangeClass::Minor
    } else {
        ChangeClass::Patch
    };
    CopyDiff {
        class,
        fields,
        major_reasons: major,
    }
}

/// The CP mapping of an update, old CP index → new CP index (doc 07 §4.5 step 3): the explicit pairs first; then each
/// remaining old CP whose index the new copy has with the same designation keeps its index; then the remaining old
/// CPs pair with the remaining new CPs of the same designation, the k-th old with the k-th new in index order (a
/// designation may repeat, REF-029). An old CP that finds no partner is absent from the map.
/// An explicit CP map `[old, new]` of one copy as a map, checked: every old index a CP of the held copy, every new
/// one a CP of the new copy, and the map injective (no old index given twice, no new index the target of two old
/// ones: two CPs joined into one would silently merge their connections). A refusal gives the pair's position, the
/// side (0 old, 1 new) and the reason.
pub(crate) fn explicit_map(
    pairs: &[[u16; 2]],
    old: &BTreeMap<u16, CopyCp>,
    new: &BTreeMap<u16, CopyCp>,
) -> Result<BTreeMap<u16, u16>, (usize, u8, String)> {
    let mut out = BTreeMap::new();
    let mut targets = BTreeSet::new();
    for (k, [o, n]) in pairs.iter().enumerate() {
        if !old.contains_key(o) {
            return Err((k, 0, format!("{o} is no CP index of the held copy")));
        }
        if !new.contains_key(n) {
            return Err((k, 1, format!("{n} is no CP index of the new copy")));
        }
        if out.insert(*o, *n).is_some() {
            return Err((k, 0, format!("CP {o} of the held copy is mapped twice")));
        }
        if !targets.insert(*n) {
            return Err((
                k,
                1,
                format!("CP {n} of the new copy is the target of two CPs of the held copy"),
            ));
        }
    }
    Ok(out)
}

pub(crate) fn cp_mapping(
    old: &BTreeMap<u16, CopyCp>,
    new: &BTreeMap<u16, CopyCp>,
    explicit: &BTreeMap<u16, u16>,
) -> BTreeMap<u16, u16> {
    let mut out = explicit.clone();
    let mut taken: BTreeSet<u16> = explicit.values().copied().collect();
    for (idx, cp) in old {
        if out.contains_key(idx) || taken.contains(idx) {
            continue;
        }
        if new.get(idx).is_some_and(|n| n.designation == cp.designation) {
            out.insert(*idx, *idx);
            taken.insert(*idx);
        }
    }
    for (idx, cp) in old {
        if out.contains_key(idx) {
            continue;
        }
        let partner = new
            .iter()
            .find(|(n_idx, n)| n.designation == cp.designation && !taken.contains(*n_idx))
            .map(|(n_idx, _)| *n_idx);
        if let Some(n) = partner {
            out.insert(*idx, n);
            taken.insert(n);
        }
    }
    out
}

/// A held library copy row as a document.
pub(crate) fn row_document(row: &AnyRow) -> Option<LibDocument> {
    Some(match row {
        AnyRow::Part(c) => LibDocument::Part((**c).clone()),
        AnyRow::Symbol(c) => LibDocument::Symbol((**c).clone()),
        AnyRow::Fndef(c) => LibDocument::Fndef((**c).clone()),
        AnyRow::Form(c) => LibDocument::Form((**c).clone()),
        AnyRow::Macro(c) => LibDocument::Macro((**c).clone()),
        AnyRow::Manufacturer(c) => LibDocument::Manufacturer((**c).clone()),
        _ => return None,
    })
}

/// The numeric core `[major, minor, patch]` of a semantic version (pre-release and build ignored).
pub(crate) fn semver_core(v: &str) -> Option<[u64; 3]> {
    let core = v.trim().split(['-', '+']).next()?;
    let parts: Vec<u64> = core.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    match parts[..] {
        [a, b, c] => Some([a, b, c]),
        _ => None,
    }
}

/// A CP mapping of one copy for `library.update`: pairs `[old_idx, new_idx]` of the held copy's and the new
/// document's CP indices.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CpMapParam {
    /// The copy ID the mapping is for (a copy of `copies`).
    #[schemars(with = "UuidS")]
    pub copy: AnyId,
    /// The pairs `[old_idx, new_idx]`.
    pub map: Vec<[u16; 2]>,
}

/// `library.update` (`UpdateLibraryCopies`, WP-S1.19 D9, doc 07 §4.5, FR-PRT-014, FR-SYM-016): replaces held library
/// copies by other versions of the same items, in place (same copy ID), remapping the connection points the change
/// moves.
///
/// `copies` holds library documents in the `lib/` JSON form (read and checked as `library.embed` reads them, at
/// `/copies/i`; a project-local copy may be updated too). For each: the same hash as the held copy changes nothing;
/// otherwise the copy is replaced. Its CPs are mapped (doc 07 §4.5 step 3): by the pairs of `cp_maps` for that
/// copy, else by index where the designation is unchanged, else by designation; the accepted mapping rewrites the
/// `Cp{function, idx}` endpoints of connections, potential definitions, bonds and channel bindings and the
/// `cp_overrides` of the affected functions (a FnDef's: the functions of that definition; a part's: the functions
/// of its devices that its template or pattern gives CPs; a symbol's: the functions its representations draw, by
/// the FnDef CP each symbol CP draws). Referencing records change nowhere else.
///
/// Rejected: no copy (`/copies`), a copy listed twice, one the project does not hold (`/copies/i`), a version that
/// is not semantic or is lower than the held one (`/copies/i/version`, a downgrade), a mapping for a copy not in
/// `copies` or listed twice (`/cp_maps/j/copy`), a pair whose index is no CP of the held or new copy
/// (`/cp_maps/j/map/k/0` or `/1`), two updated copies that map one function CP differently, and a CP with a
/// connection, potential definition, bond or channel binding that the mapping leaves without a CP (the list in the
/// refusal). **Bulk**: always previewed; the session's update plan shows the class ([`classify_update`]), the field
/// diff, the devices and representations touched and the BOM rows that change. Outcome: the first copy.
///
/// **The new version's closure (FT-640).** `embed` holds the documents the new versions name that the project does
/// not hold (a new manufacturer, a new template FnDef, a symbol's new FnDef); they are embedded in the same
/// transaction, each checked as `library.embed` checks it (`/embed/i`: a copy the project holds already, one listed
/// twice or also in `copies` is refused). A FnDef or manufacturer that a new version names and its held version did
/// not, which the project does not hold and `embed` does not bring, refuses the update with the list (a reference
/// the held version already made is left as it is: the update does not repair an older closure).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateLibraryCopies {
    /// The new library documents (`lib/` JSON form), at least one.
    pub copies: Vec<Value>,
    /// CP mappings, per copy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cp_maps: Vec<CpMapParam>,
    /// The closure documents the new versions name that the project does not hold (FT-640), embedded with the
    /// update (`lib/` JSON form).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embed: Vec<Value>,
}

/// The FnDefs and the manufacturer a part names, the FnDef a symbol names: the closure items an update checks
/// (FT-640), by (kind, copy ID).
fn named_items(doc: &LibDocument) -> BTreeSet<(ew_schema::Kind, AnyId)> {
    use ew_schema::Kind;
    let copy =
        |r: &ew_schema::LibRef| -> AnyId { ew_ids::v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid()) };
    let mut out = BTreeSet::new();
    match doc {
        LibDocument::Part(c) => {
            out.extend(
                c.item
                    .function_template
                    .iter()
                    .map(|t| (Kind::Fndef, copy(&t.fndef))),
            );
            out.insert((Kind::Manufacturer, copy(&c.item.manufacturer)));
        }
        LibDocument::Symbol(c) => {
            out.insert((Kind::Fndef, copy(&c.item.fndef)));
        }
        _ => {}
    }
    out
}

/// The functions whose CP endpoints a copy defines (see [`UpdateLibraryCopies`]).
fn affected_functions(tx: &TxBuilder<'_>, doc: &LibDocument) -> Vec<Id<Function>> {
    let mut out = BTreeSet::new();
    match doc {
        LibDocument::Fndef(c) => {
            let id: Id<ew_schema::FnDef> = c.id;
            for f in tx.iter::<Function>() {
                if f.def == id {
                    out.insert(f.id);
                }
            }
        }
        LibDocument::Part(c) => {
            let id: Id<Part> = c.id;
            let template: BTreeSet<Id<ew_schema::FnDef>> = c
                .item
                .function_template
                .iter()
                .map(|t| ew_ids::v5::lib_copy_id(t.fndef.library.as_uuid(), t.fndef.item.as_uuid()))
                .collect();
            for d in tx.iter::<Device>() {
                if !d.parts.iter().any(|u| u.part.copy == id) {
                    continue;
                }
                for f in tx.functions_of_device(d.id) {
                    let Some(func) = tx.get::<Function>(f) else {
                        continue;
                    };
                    let pattern =
                        matches!(func.kind, FunctionKind::BlackBox) && !c.item.cp_pattern.is_empty();
                    if template.contains(&func.def) || pattern {
                        out.insert(f);
                    }
                }
            }
        }
        LibDocument::Symbol(c) => {
            let id: Id<ew_schema::SymbolDef> = c.id;
            for r in tx.iter::<Representation>() {
                if r.symbol == id {
                    out.extend(r.functions.iter().copied());
                }
            }
        }
        _ => {}
    }
    out.into_iter().collect()
}

/// What is attached at a function CP: connections, potential definitions, bonds and channel bindings, as text.
fn attached(tx: &TxBuilder<'_>, f: Id<Function>, ep: u16) -> Vec<String> {
    let at = EndpointKey::Cp(f, ep);
    let mut out = Vec::new();
    out.extend(
        tx.connections_at(at)
            .into_iter()
            .map(|c| format!("connection {c}")),
    );
    out.extend(
        tx.potential_defs_at(at)
            .into_iter()
            .map(|p| format!("potential definition {p}")),
    );
    out.extend(tx.bonds_at(at).into_iter().map(|b| format!("bond {b}")));
    out.extend(
        tx.channels_at_cp(f, ep)
            .into_iter()
            .map(|c| format!("channel {c}")),
    );
    out
}

impl Command for UpdateLibraryCopies {
    const NAME: &'static str = "library.update";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    #[allow(clippy::too_many_lines)]
    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        if self.copies.is_empty() {
            return Err(CmdError::invalid_params(
                name,
                "/copies",
                "give at least one library document",
            ));
        }
        // (held document, new document, key, whether the hash changed)
        let mut updates: Vec<(LibDocument, LibDocument, AnyKey, bool)> = Vec::new();
        let mut seen = BTreeSet::new();
        let mut primary = None;
        for (i, value) in self.copies.iter().enumerate() {
            let at = format!("/copies/{i}");
            let (doc, l) = read_copy(name, &at, value)?;
            let kind = doc.kind().expect("an implemented kind");
            let key = (kind, l.id);
            if !seen.insert(key) {
                return Err(CmdError::invalid_params(name, at, "the copy is listed twice"));
            }
            primary.get_or_insert(key);
            let Some(held) = tx.row(&key) else {
                return Err(CmdError::invalid_params(
                    name,
                    at,
                    format!(
                        "the project holds no {kind} copy {} (item {}); embed it first (library.embed or part.assign)",
                        l.id, l.item
                    ),
                ));
            };
            let held = row_document(&held).expect("a library copy row");
            let held_version = held_version_of(&held);
            let new_version = held_version_of(&doc);
            let Some(new_core) = semver_core(&new_version) else {
                return Err(CmdError::invalid_params(
                    name,
                    format!("{at}/version"),
                    format!("`{new_version}` is not a semantic version"),
                ));
            };
            if semver_core(&held_version).is_some_and(|h| new_core < h) {
                return Err(CmdError::invalid_params(
                    name,
                    format!("{at}/version"),
                    format!(
                        "version {new_version} is lower than the held {held_version}: a downgrade is not an update"
                    ),
                ));
            }
            let changed = held_sha_of(&held) != Some(l.claimed);
            updates.push((held, doc, key, changed));
        }
        // the explicit mappings, by copy
        let mut maps: BTreeMap<AnyId, (usize, &CpMapParam)> = BTreeMap::new();
        for (j, m) in self.cp_maps.iter().enumerate() {
            if !seen.iter().any(|(_, id)| *id == m.copy) {
                return Err(CmdError::invalid_params(
                    name,
                    format!("/cp_maps/{j}/copy"),
                    format!("copy {} is not one of `copies`", m.copy),
                ));
            }
            if maps.insert(m.copy, (j, m)).is_some() {
                return Err(CmdError::invalid_params(
                    name,
                    format!("/cp_maps/{j}/copy"),
                    "the copy is listed twice",
                ));
            }
        }
        let mut remap: BTreeMap<(Id<Function>, u16), u16> = BTreeMap::new();
        let mut blocked: Vec<String> = Vec::new();
        for (held, new, key, changed) in &updates {
            let (old_cps, new_cps) = (
                copy_cps(held).unwrap_or_default(),
                copy_cps(new).unwrap_or_default(),
            );
            let explicit: BTreeMap<u16, u16> = match maps.get(&key.1) {
                None => BTreeMap::new(),
                Some((j, m)) => explicit_map(&m.map, &old_cps, &new_cps).map_err(|(k, side, why)| {
                    CmdError::invalid_params(name, format!("/cp_maps/{j}/map/{k}/{side}"), why)
                })?,
            };
            if !changed {
                continue;
            }
            let target = cp_mapping(&old_cps, &new_cps, &explicit);
            for f in affected_functions(tx, held) {
                for (idx, cp) in &old_cps {
                    match target.get(idx).copied() {
                        None => {
                            for what in attached(tx, f, cp.endpoint) {
                                blocked.push(format!(
                                    "function {f} CP {} ({}): {what}",
                                    cp.endpoint, cp.designation
                                ));
                            }
                        }
                        Some(n) => {
                            let to = new_cps[&n].endpoint;
                            if to != cp.endpoint {
                                if let Some(prev) = remap.insert((f, cp.endpoint), to) {
                                    if prev != to {
                                        return tx.reject(format!(
                                            "two updated copies map CP {} of function {f} differently ({prev} and \
                                             {to})",
                                            cp.endpoint
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if !blocked.is_empty() {
            return tx.reject(format!(
                "the update leaves connected CPs without a CP; map them in `cp_maps` (doc 07 §4.5): {}",
                blocked.join("; ")
            ));
        }
        // FT-640: the closure documents to embed, and the new version's items the project would lack.
        let project = tx.project().id.as_uuid();
        let mut embed: Vec<(AnyKey, LibDocument)> = Vec::new();
        for (i, value) in self.embed.iter().enumerate() {
            let at = format!("/embed/{i}");
            let (doc, l) = check_copy(name, &at, value, project)?;
            let key = (doc.kind().expect("an implemented kind"), l.id);
            if seen.contains(&key) || embed.iter().any(|(k, _)| *k == key) {
                return Err(CmdError::invalid_params(name, at, "the copy is listed twice"));
            }
            if tx.row(&key).is_some() {
                return Err(CmdError::invalid_params(
                    name,
                    at,
                    format!("the project holds {} copy {} already; leave it out", key.0, key.1),
                ));
            }
            embed.push((key, doc));
        }
        let mut missing: Vec<String> = Vec::new();
        let mut listed: BTreeSet<AnyKey> = BTreeSet::new();
        for (held, new, key, changed) in &updates {
            if !changed {
                continue;
            }
            let before = named_items(held);
            for item in named_items(new).difference(&before) {
                let lacking = tx.row(item).is_none() && !embed.iter().any(|(k, _)| k == item);
                if lacking && listed.insert(*item) {
                    missing.push(format!(
                        "{} copy {} (named by {} {})",
                        item.0, item.1, key.0, key.1
                    ));
                }
            }
        }
        if !missing.is_empty() {
            return tx.reject(format!(
                "the new versions name items the project does not hold; give them in `embed` (FT-640): {}",
                missing.join("; ")
            ));
        }
        for (_, doc) in embed {
            match doc {
                LibDocument::Part(c) => tx.put(c),
                LibDocument::Symbol(c) => tx.put(c),
                LibDocument::Fndef(c) => tx.put(c),
                LibDocument::Form(c) => tx.put(c),
                LibDocument::Macro(c) => tx.put(c),
                LibDocument::Manufacturer(c) => tx.put(c),
                LibDocument::Unknown(_) => unreachable!("check_copy refuses an unknown kind"),
            }
        }
        apply_remap(tx, &remap)?;
        for (_, new, _, changed) in updates {
            if !changed {
                continue;
            }
            match new {
                LibDocument::Part(c) => tx.put(c),
                LibDocument::Symbol(c) => tx.put(c),
                LibDocument::Fndef(c) => tx.put(c),
                LibDocument::Form(c) => tx.put(c),
                LibDocument::Macro(c) => tx.put(c),
                LibDocument::Manufacturer(c) => tx.put(c),
                LibDocument::Unknown(_) => unreachable!("read_copy refuses an unknown kind"),
            }
        }
        Ok(Outcome {
            primary,
            ..Outcome::default()
        })
    }
}

fn held_version_of(doc: &LibDocument) -> String {
    fn v<T>(c: &LibCopy<T>) -> String {
        c.version.clone()
    }
    match doc {
        LibDocument::Part(c) => v(c),
        LibDocument::Symbol(c) => v(c),
        LibDocument::Fndef(c) => v(c),
        LibDocument::Form(c) => v(c),
        LibDocument::Macro(c) => v(c),
        LibDocument::Manufacturer(c) => v(c),
        LibDocument::Unknown(_) => String::new(),
    }
}

fn held_sha_of(doc: &LibDocument) -> Option<Sha256> {
    Some(match doc {
        LibDocument::Part(c) => c.sha256,
        LibDocument::Symbol(c) => c.sha256,
        LibDocument::Fndef(c) => c.sha256,
        LibDocument::Form(c) => c.sha256,
        LibDocument::Macro(c) => c.sha256,
        LibDocument::Manufacturer(c) => c.sha256,
        LibDocument::Unknown(_) => return None,
    })
}

/// Rewrites the CP endpoints `(function, old) → new` of connections, potential definitions, bonds and channel
/// bindings, and the functions' `cp_overrides`, each record once (a swap maps both ends from the state before).
fn apply_remap(tx: &mut TxBuilder<'_>, remap: &BTreeMap<(Id<Function>, u16), u16>) -> Result<(), CmdError> {
    if remap.is_empty() {
        return Ok(());
    }
    let map_ep = |e: &Endpoint| -> Endpoint {
        match e {
            Endpoint::Cp(f, i) => Endpoint::Cp(*f, remap.get(&(*f, *i)).copied().unwrap_or(*i)),
            other => other.clone(),
        }
    };
    let (mut conns, mut defs, mut bonds, mut chans) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut functions = BTreeSet::new();
    for (f, i) in remap.keys() {
        let at = EndpointKey::Cp(*f, *i);
        conns.extend(tx.connections_at(at));
        defs.extend(tx.potential_defs_at(at));
        bonds.extend(tx.bonds_at(at));
        chans.extend(tx.channels_at_cp(*f, *i));
        functions.insert(*f);
    }
    for c in conns {
        tx.update::<Connection>(c, |c| {
            c.a = map_ep(&c.a);
            c.b = map_ep(&c.b);
        })?;
    }
    for d in defs {
        tx.update::<PotentialDef>(d, |d| d.at = map_ep(&d.at))?;
    }
    for b in bonds {
        tx.update::<Bond>(b, |b| {
            b.a = map_ep(&b.a);
            b.b = map_ep(&b.b);
        })?;
    }
    for c in chans {
        tx.update::<Channel>(c, |c| c.binding = c.binding.as_ref().map(map_ep))?;
    }
    for f in functions {
        let func = tx.require::<Function>(f)?;
        if func.cp_overrides.is_empty() {
            continue;
        }
        let mut seen = BTreeSet::new();
        for o in &func.cp_overrides {
            let to = remap.get(&(f, o.idx)).copied().unwrap_or(o.idx);
            if !seen.insert(to) {
                return tx.reject(format!(
                    "the mapping moves two CP overrides of function {f} onto CP {to}; change the overrides first"
                ));
            }
        }
        let moved: Vec<_> = func
            .cp_overrides
            .as_slice()
            .iter()
            .map(|o| {
                let mut o = o.clone();
                o.idx = remap.get(&(f, o.idx)).copied().unwrap_or(o.idx);
                o
            })
            .collect();
        let moved = ew_schema::KeyedList::from_vec(moved).map_err(|e| {
            CmdError::internal(format!("{}: CP overrides of {f}: {e}", UpdateLibraryCopies::NAME))
        })?;
        tx.update::<Function>(f, |func| func.cp_overrides = moved)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use proptest::prelude::*;

    use super::{CopyCp, cp_mapping, explicit_map, semver_core};

    fn cps(designations: &[&str]) -> BTreeMap<u16, CopyCp> {
        designations
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let i = u16::try_from(i).unwrap();
                (
                    i,
                    CopyCp {
                        designation: (*d).to_owned(),
                        endpoint: i,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn repeated_designations_pair_in_index_order() {
        let old = cps(&["L", "N", "+", "+", "-", "-"]);
        let new = cps(&["L", "N", "-", "-", "+", "+"]);
        let m = cp_mapping(&old, &new, &BTreeMap::new());
        assert_eq!(
            m,
            BTreeMap::from([(0, 0), (1, 1), (2, 4), (3, 5), (4, 2), (5, 3)])
        );
    }

    #[test]
    fn an_explicit_pair_wins_and_a_vanished_cp_stays_unmapped() {
        let old = cps(&["1", "2", "3"]);
        let new = cps(&["1", "2"]);
        // 0 → 1 explicitly; 1 cannot keep its index (taken) and finds no other "2"; "3" has no partner
        let m = cp_mapping(&old, &new, &BTreeMap::from([(0, 1)]));
        assert_eq!(m, BTreeMap::from([(0, 1)]));
        let m = cp_mapping(&old, &new, &BTreeMap::new());
        assert_eq!(m, BTreeMap::from([(0, 0), (1, 1)]));
    }

    #[test]
    fn versions_compare_by_their_numeric_core() {
        assert_eq!(semver_core("1.2.3"), Some([1, 2, 3]));
        assert_eq!(semver_core("1.10.0-rc.1+b"), Some([1, 10, 0]));
        assert!(semver_core("1.2").is_none() && semver_core("x").is_none());
        assert!(semver_core("1.10.0") > semver_core("1.9.9"));
    }

    #[test]
    fn an_explicit_map_is_injective() {
        let (old, new) = (cps(&["1", "2", "3"]), cps(&["1", "2", "3"]));
        assert_eq!(
            explicit_map(&[[0, 1], [1, 0]], &old, &new),
            Ok(BTreeMap::from([(0, 1), (1, 0)]))
        );
        let (k, side, why) = explicit_map(&[[0, 1], [2, 1]], &old, &new).unwrap_err();
        assert_eq!((k, side), (1, 1), "{why}");
        let (k, side, why) = explicit_map(&[[0, 1], [0, 2]], &old, &new).unwrap_err();
        assert_eq!((k, side), (1, 0), "{why}");
        assert_eq!(explicit_map(&[[3, 0]], &old, &new).unwrap_err().0, 0);
        assert_eq!(explicit_map(&[[0, 3]], &old, &new).unwrap_err().1, 1);
    }

    proptest! {
        /// Review C35 m2: an explicit map is accepted exactly when every pair names CPs of both copies and no old or
        /// new index repeats; what it accepts is an injection holding every pair.
        #[test]
        fn an_explicit_map_is_accepted_only_as_an_injection(
            n_old in 0usize..5,
            n_new in 0usize..5,
            pairs in proptest::collection::vec((0u16..6, 0u16..6), 0..6),
        ) {
            let names = ["a", "b", "c", "d", "e"];
            let (old, new) = (cps(&names[..n_old]), cps(&names[..n_new]));
            let pairs: Vec<[u16; 2]> = pairs.into_iter().map(|(o, n)| [o, n]).collect();
            let olds: BTreeSet<u16> = pairs.iter().map(|p| p[0]).collect();
            let news: BTreeSet<u16> = pairs.iter().map(|p| p[1]).collect();
            let valid = pairs.iter().all(|[o, n]| old.contains_key(o) && new.contains_key(n))
                && olds.len() == pairs.len()
                && news.len() == pairs.len();
            match explicit_map(&pairs, &old, &new) {
                Ok(m) => {
                    prop_assert!(valid);
                    prop_assert_eq!(m.len(), pairs.len());
                    let targets: BTreeSet<u16> = m.values().copied().collect();
                    prop_assert_eq!(targets.len(), m.len());
                    for [o, n] in &pairs {
                        prop_assert_eq!(m.get(o), Some(n));
                    }
                }
                Err((k, side, _)) => {
                    prop_assert!(!valid);
                    prop_assert!(k < pairs.len() && side <= 1);
                    // The pairs before the refused one are a valid injection.
                    prop_assert!(explicit_map(&pairs[..k], &old, &new).is_ok());
                }
            }
        }

        /// The mapping is a partial injection from old to new indices that joins equal designations, as many pairs per
        /// designation as both sides allow.
        #[test]
        fn the_mapping_is_injective_and_keeps_designations(
            old in proptest::collection::vec(0usize..4, 0..8),
            new in proptest::collection::vec(0usize..4, 0..8),
        ) {
            let names = ["A", "B", "C", "D"];
            let old = cps(&old.iter().map(|d| names[*d]).collect::<Vec<_>>());
            let new = cps(&new.iter().map(|d| names[*d]).collect::<Vec<_>>());
            let m = cp_mapping(&old, &new, &BTreeMap::new());
            let targets: BTreeSet<u16> = m.values().copied().collect();
            prop_assert_eq!(targets.len(), m.len());
            for (o, n) in &m {
                prop_assert_eq!(&old[o].designation, &new[n].designation);
            }
            for d in names {
                let a = old.values().filter(|c| c.designation == d).count();
                let b = new.values().filter(|c| c.designation == d).count();
                prop_assert_eq!(m.keys().filter(|o| old[*o].designation == d).count(), a.min(b));
            }
        }
    }
}
