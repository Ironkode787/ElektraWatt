// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `macro.insert` (WP-S1.19 part 5, D12, D13; doc 07 §3.5, §4.6): inserts a window or page macro in one transaction,
//! with its values, its placeholder groups and its closure as parameters, so a command log replays the insertion
//! exactly on a machine with no library installed (D7).
//!
//! The macro's variant is a project fragment (`#ew-fragment v1` in its object form, widened for macros by ADR-0011
//! amendment 5); the insertion itself is `edit.paste`'s engine with three differences: the new IDs are
//! `v5(instance, local ID)` (a channel `plc_channel_id(new module, number)`), frozen tags stay (saving a macro clears
//! them unless marked keep), and a tag counter a variable sets stays, as does a child tag's counter under a parent that
//! comes along.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::{Id, Uuid};
use ew_model::{LINKS, Policy};
use ew_schema::{
    Kind, LibDocument, MacroDef, MacroInstance, MacroKind, MacroVariant, Page, Record, VarDef, VarType,
};
use ew_units::{Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::device::allowed_rep_types;
use super::fields::SETTABLE;
use super::library::check_copy;
use super::paste::{Fragment, Insertion, NewIds, cleared, occurrences};
use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{IdentS, Orient2S, Pt2S, UuidS};
use crate::tx::TxBuilder;

/// The record kinds a macro fragment may carry (ADR-0011 amendment 5): those of a paste, PLC channels and device
/// links; a page macro's page records come in the fragment's `pages`.
pub const MACRO_KINDS: [Kind; 10] = [
    Kind::Device,
    Kind::Function,
    Kind::Representation,
    Kind::Connection,
    Kind::Junction,
    Kind::Channel,
    Kind::PotentialDef,
    Kind::DeviceLink,
    Kind::PageItem,
    Kind::StructureBox,
];

/// The field paths a macro variable may bind beyond the field catalogue (`SETTABLE`, which `fields.set` edits):
/// an assembly child's position and parent (a `DeviceRef`), and a channel's authored members (doc 07 §9.3: the
/// valve typical binds `device.position`, the module's parent and the channel numbers).
pub const MACRO_FIELDS: &[(Kind, &[&str])] = &[
    (Kind::Device, &["position", "parent"]),
    (
        Kind::Channel,
        &[
            "number",
            "designation",
            "address",
            "state",
            "comment",
            "binding",
            "target",
        ],
    ),
    (Kind::Page, &["title", "description_override"]),
];

/// Whether `field` (`<kind>.<path>`, such as `device.function_text` or `device.parts[0].part`) is a path a variable
/// may bind on a record of kind `kind`: the kind's name, then a path of the field catalogue (`SETTABLE`) or of
/// [`MACRO_FIELDS`], or a path below one (`.member` or `[n]`).
pub fn binding_field_ok(kind: Kind, field: &str) -> bool {
    let Some(path) = field
        .strip_prefix(kind.name())
        .and_then(|rest| rest.strip_prefix('.'))
    else {
        return false;
    };
    if path.is_empty() || path_segments(path).is_none() {
        return false;
    }
    let under = |base: &str| {
        path == base
            || path
                .strip_prefix(base)
                .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
    };
    let settable = SETTABLE
        .iter()
        .filter(|(k, _)| k.kind() == kind)
        .flat_map(|(_, paths)| paths.iter());
    let extra = MACRO_FIELDS
        .iter()
        .filter(|(k, _)| *k == kind)
        .flat_map(|(_, paths)| paths.iter());
    settable.chain(extra).any(|p| under(p))
}

/// One step of a binding path.
enum Seg<'a> {
    /// An object member.
    Key(&'a str),
    /// A list element.
    At(usize),
}

/// The steps of a binding path (`parts[0].part`): members separated by `.`, each optionally followed by `[n]`
/// indices; `None` when it does not parse.
fn path_segments(path: &str) -> Option<Vec<Seg<'_>>> {
    let mut out = Vec::new();
    for part in path.split('.') {
        let (name, mut rest) = match part.find('[') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        if name.is_empty() {
            return None;
        }
        out.push(Seg::Key(name));
        while let Some(r) = rest.strip_prefix('[') {
            let end = r.find(']')?;
            out.push(Seg::At(r[..end].parse().ok()?));
            rest = &r[end + 1..];
        }
        if !rest.is_empty() {
            return None;
        }
    }
    Some(out)
}

/// Sets `value` at `path` below `root`, creating the objects on the way; a list index must exist. `false` when
/// the path does not fit the record.
fn set_path(root: &mut Value, path: &str, value: Value) -> bool {
    let Some(segs) = path_segments(path) else {
        return false;
    };
    let mut cur = root;
    for (i, seg) in segs.iter().enumerate() {
        let last = i + 1 == segs.len();
        cur = match seg {
            Seg::Key(k) => {
                let Some(obj) = cur.as_object_mut() else {
                    return false;
                };
                if last {
                    obj.insert((*k).to_owned(), value);
                    return true;
                }
                obj.entry((*k).to_owned())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()))
            }
            Seg::At(n) => {
                let Some(slot) = cur.as_array_mut().and_then(|a| a.get_mut(*n)) else {
                    return false;
                };
                if last {
                    *slot = value;
                    return true;
                }
                slot
            }
        };
    }
    false
}

/// Coerces one value of a macro variable to its type (doc 07 §2.7), returning the value as it is written into the
/// records: `text` a text or a language object; `ident`, `state`, `process_tag`, `address` a text; `number` a
/// number or a decimal text; `bool` a boolean; `tag_counter` an integer from 1; `channel_binding` an integer from
/// 0 to 65535; `part` a part use object (`{"copy": …}`); `device_ref` a UUID text. A value of a type this version does
/// not know is taken as it is.
///
/// # Errors
///
/// The reason, when the value does not coerce.
pub fn coerce_value(ty: &VarType, v: &Value) -> Result<Value, String> {
    let fail = |what: &str| Err(format!("{v} is not {what}"));
    match ty {
        VarType::Text => match v {
            Value::String(_) | Value::Object(_) => Ok(v.clone()),
            _ => fail("a text"),
        },
        VarType::Ident | VarType::State | VarType::ProcessTag | VarType::Address => match v {
            Value::String(s) if !s.is_empty() => Ok(v.clone()),
            _ => fail("a non-empty text"),
        },
        VarType::Number => match v {
            Value::Number(_) => Ok(v.clone()),
            Value::String(s) if s.parse::<ew_units::Dec3>().is_ok() => Ok(v.clone()),
            _ => fail("a number"),
        },
        VarType::Bool => match v {
            Value::Bool(_) => Ok(v.clone()),
            _ => fail("true or false"),
        },
        VarType::TagCounter => match v.as_u64() {
            Some(n) if n >= 1 && u32::try_from(n).is_ok() => Ok(v.clone()),
            _ => fail("a tag counter (an integer from 1)"),
        },
        VarType::ChannelBinding => match v.as_u64() {
            Some(n) if u16::try_from(n).is_ok() => Ok(v.clone()),
            _ => fail("a channel number (an integer from 0 to 65535)"),
        },
        VarType::Part => match v.get("copy").and_then(Value::as_str) {
            Some(s) if s.parse::<Uuid>().is_ok() => Ok(v.clone()),
            _ => fail("a part reference ({\"copy\": <copy ID>})"),
        },
        VarType::DeviceRef => match v.as_str() {
            Some(s) if s.parse::<Uuid>().is_ok() => Ok(v.clone()),
            _ => fail("a device ID"),
        },
        VarType::Unknown(_) => Ok(v.clone()),
    }
}

/// The variant a page takes: for a window macro the first variant whose representation type the page's type takes
/// (any variant on a page type this version does not know); for a page macro the first variant.
pub fn pick_variant<'m>(def: &'m MacroDef, page: Option<&Page>) -> Option<&'m MacroVariant> {
    match (&def.kind, page) {
        (MacroKind::Window, Some(page)) => def
            .variants
            .iter()
            .find(|v| allowed_rep_types(&page.kind).is_none_or(|allowed| allowed.contains(&v.rep_type))),
        _ => def.variants.first(),
    }
}

/// Why a macro could not be instantiated: the parameter's JSON Pointer and the reason.
pub type InstantiateError = (String, String);

/// A macro instantiated with its values: the fragment to insert, the effective values and the devices (local IDs)
/// whose tag counter a variable sets.
#[derive(Clone, Debug)]
pub struct Instantiated {
    /// The fragment, values applied and excluded groups dropped.
    pub fragment: Fragment,
    /// The effective value of every variable that has one, by name.
    pub values: BTreeMap<String, Value>,
    /// The devices whose tag counter stays: a variable sets it, or the device has a child tag and its parent comes
    /// along.
    pub keep_counter: BTreeSet<Uuid>,
    /// The project records the `device_ref` variables name (doc 07 §2.7: "an existing device chosen at insertion"):
    /// a reference to one of them stays when the project holds it.
    pub refs: BTreeSet<Uuid>,
}

/// Applies values to a macro's variant (doc 07 §4.6 steps 2 and 3, without rhai): each variable takes its literal
/// value, else the value set's, else its default; the value is coerced to the variable's type and written at every
/// binding; a placeholder group whose `include` variable is false is dropped before the remap, with every record
/// that cannot stand without it (a mandatory or cascading reference) and every optional reference to it cleared
/// (FR-MAC-018).
///
/// # Errors
///
/// `/value_set` a value set the macro does not have; `/values/<name>` a variable it does not have, or a value that
/// does not coerce or that a bound record does not take; `/macro` a fragment that does not read, a binding to a
/// record outside the fragment or a group whose `include` is not a boolean variable.
pub fn instantiate(
    def: &MacroDef,
    variant: &MacroVariant,
    value_set: Option<&str>,
    literals: &BTreeMap<String, Value>,
) -> Result<Instantiated, InstantiateError> {
    let err = |at: &str, m: String| (at.to_owned(), m);
    let mut fragment: Fragment = serde_json::from_value(variant.fragment.clone())
        .map_err(|e| err("/macro", format!("the macro's fragment does not read: {e}")))?;
    let set = match value_set {
        None => None,
        Some(name) => Some(
            def.value_sets
                .iter()
                .find(|s| s.name.as_str() == name)
                .ok_or_else(|| err("/value_set", format!("the macro has no value set `{name}`")))?,
        ),
    };
    let var_of = |name: &str| def.variables.iter().find(|v| v.name.as_str() == name);
    for name in literals.keys() {
        if var_of(name).is_none() {
            return Err(err(
                &format!("/values/{name}"),
                format!("the macro has no variable `{name}`"),
            ));
        }
    }
    // The effective values, coerced.
    let mut values: BTreeMap<String, Value> = BTreeMap::new();
    for var in &def.variables {
        let name = var.name.as_str();
        let (raw, at) = match (literals.get(name), set.and_then(|s| s.values.get(name))) {
            (Some(v), _) => (Some(v), format!("/values/{name}")),
            (None, Some(v)) => (Some(v), "/value_set".to_owned()),
            (None, None) => (var.default.as_ref(), "/macro".to_owned()),
        };
        let Some(raw) = raw else { continue };
        let v = coerce_value(&var.ty, raw).map_err(|m| err(&at, format!("variable `{name}`: {m}")))?;
        values.insert(name.to_owned(), v);
    }
    // The bindings.
    let index: BTreeMap<Uuid, (bool, usize)> = fragment
        .records
        .iter()
        .enumerate()
        .map(|(i, r)| (false, i, r))
        .chain(fragment.pages.iter().enumerate().map(|(i, r)| (true, i, r)))
        .filter_map(|(page, i, r)| {
            let id = r.get("id")?.as_str()?.parse::<Uuid>().ok()?;
            Some((id, (page, i)))
        })
        .collect();
    let mut keep_counter = BTreeSet::new();
    let refs: BTreeSet<Uuid> = def
        .variables
        .iter()
        .filter(|v| v.ty == VarType::DeviceRef)
        .filter_map(|v| values.get(v.name.as_str())?.as_str()?.parse().ok())
        .collect();
    for var in &def.variables {
        let name = var.name.as_str();
        let Some(value) = values.get(name) else { continue };
        for b in &var.bindings {
            let local = b.record.as_uuid();
            let Some(&(page, i)) = index.get(&local) else {
                return Err(err(
                    "/macro",
                    format!("variable `{name}` binds record {local}, which is not in the fragment"),
                ));
            };
            let record = if page {
                &mut fragment.pages[i]
            } else {
                &mut fragment.records[i]
            };
            let kind = record
                .get("$k")
                .and_then(Value::as_str)
                .and_then(Kind::from_name)
                .ok_or_else(|| err("/macro", format!("record {local} has no kind")))?;
            let at = format!("/values/{name}");
            let path = b
                .field
                .strip_prefix(kind.name())
                .and_then(|p| p.strip_prefix('.'))
                .filter(|_| binding_field_ok(kind, &b.field))
                .ok_or_else(|| {
                    err(
                        "/macro",
                        format!(
                            "variable `{name}` binds `{}`, which a {kind} does not take",
                            b.field
                        ),
                    )
                })?;
            if !set_path(record, path, value.clone()) {
                return Err(err(&at, format!("`{}` does not fit record {local}", b.field)));
            }
            if let Err(e) = Record::from_value(record.clone()) {
                return Err(err(
                    &at,
                    format!("`{}` of record {local} does not take {value}: {e}", b.field),
                ));
            }
            if kind == Kind::Device && path == "tag.counter" {
                keep_counter.insert(local);
            }
        }
    }
    // The placeholder groups (FR-MAC-018).
    let mut dropped: BTreeSet<Uuid> = BTreeSet::new();
    for g in &def.groups {
        let Some(include) = &g.include else { continue };
        let var: &VarDef = var_of(include).ok_or_else(|| {
            err(
                "/macro",
                format!(
                    "group `{}` is included by `{include}`, which is no variable",
                    g.name.as_str()
                ),
            )
        })?;
        if var.ty != VarType::Bool {
            return Err(err(
                "/macro",
                format!(
                    "group `{}` is included by `{include}`, which is not a boolean",
                    g.name.as_str()
                ),
            ));
        }
        if values.get(include) == Some(&Value::Bool(false)) {
            dropped.extend(g.members.iter().map(|m| m.as_uuid()));
        }
    }
    if !dropped.is_empty() {
        drop_records(&mut fragment, &mut dropped);
    }
    // A child tag (`-201U3-Q1`) counts under its parent, which online numbering does not number: a child whose parent
    // comes along keeps its counter under the new parent (doc 07 §9.3: the valves of a module typical).
    let devices: BTreeSet<Uuid> = fragment
        .records
        .iter()
        .filter(|r| r.get("$k").and_then(Value::as_str) == Some(Kind::Device.name()))
        .filter_map(id_of)
        .collect();
    for r in &fragment.records {
        let child = r.pointer("/tag/scheme").and_then(Value::as_str) == Some("child");
        let parent = r
            .get("parent")
            .and_then(Value::as_str)
            .and_then(|p| p.parse().ok());
        if let (true, Some(id), Some(parent)) = (child, id_of(r), parent)
            && devices.contains(&parent)
        {
            keep_counter.insert(id);
        }
    }
    Ok(Instantiated {
        fragment,
        values,
        keep_counter,
        refs,
    })
}

/// The ID of a record value.
fn id_of(v: &Value) -> Option<Uuid> {
    v.get("id")?.as_str()?.parse().ok()
}

/// Drops the records of `dropped` and, to a fixed point, every record with a cascading or mandatory reference to a
/// dropped record; an optional reference to one is cleared.
fn drop_records(fragment: &mut Fragment, dropped: &mut BTreeSet<Uuid>) {
    loop {
        let mut more = false;
        for list in [&mut fragment.records, &mut fragment.pages] {
            list.retain(|r| id_of(r).is_none_or(|id| !dropped.contains(&id)));
            for r in list.iter_mut() {
                let Some(kind) = r.get("$k").and_then(Value::as_str).and_then(Kind::from_name) else {
                    continue;
                };
                // later occurrences first, so the earlier paths stay valid
                let mut hits: Vec<(Vec<_>, Policy)> = LINKS
                    .iter()
                    .filter(|d| d.from == kind)
                    .flat_map(|d| {
                        occurrences(r, d.field)
                            .into_iter()
                            .filter(|(_, t)| dropped.contains(t))
                            .map(move |(p, _)| (p, d.policy))
                    })
                    .collect();
                hits.sort_by(|a, b| b.0.cmp(&a.0));
                for (path, policy) in hits {
                    match (policy != Policy::Cascade).then(|| cleared(r, &path)).flatten() {
                        Some(next) => *r = next,
                        None => {
                            if let Some(id) = id_of(r) {
                                more |= dropped.insert(id);
                            }
                            break;
                        }
                    }
                }
            }
        }
        if !more {
            break;
        }
    }
}

/// `macro.insert` (`InsertMacro`, WP-S1.19 D13, doc 07 §3.5 and §4.6, FR-MAC-002 to FR-MAC-005, FR-MAC-018): inserts a
/// window macro onto `page` with its anchor at `at`, turned and mirrored about the anchor by `orient` first (FT-672),
/// or a page macro's pages after `page`, in one transaction (one undo step).
///
/// `copies` holds the macro's document and the documents of its closure the project does not hold (the `lib/` JSON
/// form; the session resolves them, D7); `macro` is the macro's copy ID, held or in `copies`. The pipeline: the variant
/// the page takes (a window macro: the first whose representation type the page's type takes; a page macro: the first);
/// the values (`values` over the value set `value_set` over the defaults), coerced and written at their bindings; the
/// placeholder groups whose `include` is false dropped with what cannot stand without them; then `edit.paste`'s effects
/// with these differences: `orient` turns the fragment about its anchor before effect 5's move (a grid coordinate keeps
/// its whole grid steps from the anchor, any other its nanometres; a representation's orientation becomes `orient`
/// after its own, so its text-slot overrides and black-box outline, which are relative to it, turn with it; route
/// vertices, junctions, page items and structure-box outlines turn as points, an arc's start and a free text's angle as
/// directions, a wire marker stays along its wire; a route end then meets its CP's new point); each record's new ID is
/// `v5(instance, local ID)` and a channel's `plc_channel_id(new module, number)` (ADR-0004 amendment 2); frozen tags
/// stay; a tag counter a variable sets stays, and a child tag's counter when its parent comes along (online numbering
/// numbers page-based tags only); a reference to a project device that a `device_ref` variable names stays
/// (the assembly head a module is added to, doc 07 §2.7), where a paste would clear it; a page macro's pages get folios
/// proposed after `page` and then after each other (doc 06 §5.6) in its section, and its records keep their positions.
/// Finally the macro copy is embedded when the project lacks it, and a `macro_instance` `instance` records the macro,
/// the variant, the value set, the effective values, every created record by its local ID, and the anchor page (`page`,
/// or a page macro's first page).
///
/// **Rejected**: `/macro` no such macro, a symbol macro, a fragment that does not read or does not fit the macro's
/// kind; `/copies/<i>` a document that `library.embed` refuses, a second macro, or a macro the project holds in another
/// version ("update from library first"); `/page` a missing page, or no variant for its representation types; `/at`
/// missing for a window macro, or off the page's grid; `/orient` other than `r0` for a page macro; `/value_set` and
/// `/values/<name>` as [`instantiate`]; `/macro/fragment/…` as `edit.paste` refuses a fragment (a grid other than the
/// page's, a record that does not read, …). Tag, address, interruption-point and process-tag conflicts are not
/// refusals: the session lists them on the preview (FR-MAC-005); the project's prevent rules apply as to every edit (a
/// second source point of an interruption point, EW-CORE-052), and the preview lists their findings first. Cost:
/// Interactive. Outcome: the first created device, else representation, else the instance.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InsertMacro {
    /// The macro's document and the closure documents the project does not hold (`lib/` JSON form).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
    /// The macro's copy ID.
    #[serde(rename = "macro")]
    #[schemars(with = "UuidS")]
    pub macro_copy: Id<MacroDef>,
    /// The target page (a window macro), or the page after which a page macro's pages go.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The new position of a window macro's anchor, a grid point of the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Pt2S>")]
    pub at: Option<Pt2>,
    /// The orientation of a window macro about its anchor, applied before the move to `at` (FT-672): one of the
    /// eight variants `r0` … `m270` of a symbol; default `r0`. A page macro takes none.
    #[serde(default, skip_serializing_if = "is_r0")]
    #[schemars(with = "Orient2S")]
    pub orient: Orient2,
    /// The value set, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub value_set: Option<ew_mltext::Ident>,
    /// Explicit values, by variable name; they win over the value set's.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, Value>,
    /// The instance, minted by the caller: the `macro_instance`'s ID and the namespace of the new IDs.
    #[schemars(with = "UuidS")]
    pub instance: Id<MacroInstance>,
}

/// Whether an orientation is the default `r0` (omitted from the parameters, so a log written before FT-672 reads and
/// writes unchanged).
fn is_r0(o: &Orient2) -> bool {
    o.is_default()
}

impl InsertMacro {
    fn invalid(at: impl Into<String>, message: impl Into<String>) -> CmdError {
        CmdError::invalid_params(Self::NAME, at, message)
    }

    /// The macro document and the other copies: the macro (held or embedded here) and the rest for the fragment.
    fn split_copies(
        &self,
        tx: &TxBuilder<'_>,
    ) -> Result<(MacroDef, Option<LibDocument>, Vec<Value>), CmdError> {
        let project = tx.project().id.as_uuid();
        let mut def = None;
        let mut embed = None;
        let mut rest = Vec::new();
        for (i, value) in self.copies.iter().enumerate() {
            if value.get("$k").and_then(Value::as_str) != Some("macro") {
                rest.push(value.clone());
                continue;
            }
            let at = format!("/copies/{i}");
            let (doc, lineage) = check_copy(Self::NAME, &at, value, project)?;
            let LibDocument::Macro(copy) = &doc else {
                unreachable!("the kind is macro")
            };
            if copy.id != self.macro_copy || def.is_some() {
                return Err(Self::invalid(at, "only the inserted macro's document is given"));
            }
            match tx.get::<ew_schema::LibCopy<MacroDef>>(self.macro_copy) {
                Some(held) if held.sha256 != lineage.claimed => {
                    return Err(Self::invalid(
                        at,
                        format!(
                            "the project holds version {} of this macro, not {}; update from library first",
                            held.version, copy.version
                        ),
                    ));
                }
                Some(_) => {}
                None => embed = Some(doc.clone()),
            }
            def = Some(copy.item.clone());
        }
        let def = match def {
            Some(d) => d,
            None => tx
                .get::<ew_schema::LibCopy<MacroDef>>(self.macro_copy)
                .map(|c| c.item.clone())
                .ok_or_else(|| {
                    Self::invalid(
                        "/macro",
                        format!("no macro copy {}: give its document in `copies`", self.macro_copy),
                    )
                })?,
        };
        Ok((def, embed, rest))
    }
}

impl Command for InsertMacro {
    const NAME: &'static str = "macro.insert";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let (def, embed, rest) = self.split_copies(tx)?;
        let page = tx
            .get::<Page>(self.page)
            .ok_or_else(|| Self::invalid("/page", format!("no page {}", self.page)))?;
        let page_macro = match &def.kind {
            MacroKind::Window => false,
            MacroKind::Page => true,
            MacroKind::Symbol | MacroKind::Unknown(_) => {
                return Err(Self::invalid(
                    "/macro",
                    "only window and page macros are inserted at this version",
                ));
            }
        };
        let variant = pick_variant(&def, (!page_macro).then_some(&*page)).ok_or_else(|| {
            Self::invalid(
                "/page",
                "the macro has no variant for this page's representation types",
            )
        })?;
        let mut inst = instantiate(
            &def,
            variant,
            self.value_set.as_ref().map(ew_mltext::Ident::as_str),
            &self.values,
        )
        .map_err(|(at, m)| Self::invalid(at, m))?;
        if page_macro && inst.fragment.pages.is_empty() {
            return Err(Self::invalid("/macro", "the page macro's fragment holds no page"));
        }
        if !page_macro && !inst.fragment.pages.is_empty() {
            return Err(Self::invalid(
                "/macro",
                "a window macro's fragment holds no pages",
            ));
        }
        if page_macro && !self.orient.is_default() {
            return Err(Self::invalid(
                "/orient",
                "a page macro keeps the layout of its pages and takes no orientation; insert it with `r0`",
            ));
        }
        let at = match (page_macro, self.at) {
            (true, _) => inst.fragment.anchor,
            (false, Some(at)) => at,
            (false, None) => return Err(Self::invalid("/at", "a window macro needs the point `at`")),
        };
        inst.fragment.copies = rest;
        inst.fragment.lib.clear();
        if let Some(LibDocument::Macro(c)) = embed {
            tx.insert(c)?;
        }
        let insertion = Insertion {
            name: Self::NAME,
            prefix: "/macro/fragment",
            page: self.page,
            at,
            orient: self.orient,
            fragment: &inst.fragment,
            kinds: &MACRO_KINDS,
            kinds_refusal: "is not inserted (only devices, functions, representations, connections, junctions, \
                            channels, potential definitions, device links, page items and structure boxes)",
            ids: NewIds::Instance(self.instance.as_uuid()),
            keep_frozen: true,
            keep_counter: inst.keep_counter.clone(),
            keep_refs: inst.refs.clone(),
        };
        let out = insertion.run(tx)?;
        let anchor = out.pages.first().copied().unwrap_or(self.page);
        let instance = MacroInstance {
            id: self.instance,
            v: 1,
            macro_copy: self.macro_copy,
            variant: variant.key.clone(),
            value_set: self.value_set.clone(),
            values: inst.values,
            members: out
                .members
                .iter()
                .map(|(l, c)| (Id::from_uuid(*l), Id::from_uuid(*c)))
                .collect(),
            page: anchor,
            extra: ew_schema::Extra::new(),
        };
        let id = tx.insert(instance)?;
        let primary = out
            .first_device
            .or(out.first_rep)
            .or(Some((Kind::MacroInstance, id.cast())));
        Ok(Outcome {
            primary,
            note: None,
            notices: out.notices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn binding_paths_follow_the_field_catalogue() {
        assert!(binding_field_ok(Kind::Device, "device.function_text"));
        assert!(binding_field_ok(Kind::Device, "device.parts[0].part"));
        assert!(binding_field_ok(Kind::Device, "device.plc_module.slot"));
        assert!(binding_field_ok(Kind::Device, "device.position"));
        assert!(binding_field_ok(Kind::Channel, "channel.number"));
        assert!(binding_field_ok(
            Kind::Function,
            "function.cp_overrides[3].designation"
        ));
        assert!(!binding_field_ok(Kind::Device, "device.id"));
        assert!(!binding_field_ok(Kind::Device, "function.def"));
        assert!(!binding_field_ok(Kind::Device, "device.function_textx"));
        assert!(!binding_field_ok(Kind::Device, "device.parts[x]"));
        assert!(!binding_field_ok(Kind::Device, "device."));
    }

    #[test]
    fn values_coerce_by_type() {
        assert!(coerce_value(&VarType::Bool, &json!(true)).is_ok());
        assert!(coerce_value(&VarType::Bool, &json!("yes")).is_err());
        assert!(coerce_value(&VarType::TagCounter, &json!(3)).is_ok());
        assert!(coerce_value(&VarType::TagCounter, &json!(0)).is_err());
        assert!(coerce_value(&VarType::ChannelBinding, &json!(0)).is_ok());
        assert!(coerce_value(&VarType::ChannelBinding, &json!(70_000)).is_err());
        assert!(coerce_value(&VarType::Number, &json!("2.5")).is_ok());
        assert!(coerce_value(&VarType::Number, &json!("two")).is_err());
        assert!(coerce_value(&VarType::Text, &json!({"en-GB": "Spare"})).is_ok());
        assert!(coerce_value(&VarType::State, &json!("")).is_err());
        assert!(coerce_value(&VarType::DeviceRef, &json!("not a uuid")).is_err());
    }

    #[test]
    fn set_path_creates_objects_and_needs_list_elements() {
        let mut v = json!({"parts": [{"part": {"copy": "a"}}]});
        assert!(set_path(&mut v, "plc_module.slot", json!(3)));
        assert_eq!(v["plc_module"]["slot"], json!(3));
        assert!(set_path(&mut v, "parts[0].qty", json!("2")));
        assert_eq!(v["parts"][0]["qty"], json!("2"));
        assert!(!set_path(&mut v, "parts[1].qty", json!("2")));
    }
}
