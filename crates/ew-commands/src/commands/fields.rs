// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `fields.set` (`SetFields`): the property grid's command, also over a multi-selection (doc 06 §3.1, §3.7).

use std::collections::BTreeSet;

use ew_model::Entity;
use ew_schema::{
    AnyId, Conductor, Connection, Device, ExternalRef, Function, Kind, Page, PotentialDef, ReportBlock,
    Representation, Revision, Section, StructureNode,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::{Command, Outcome};
use crate::commands::common::KindName;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// The settable field paths per kind: dotted JSON member paths of the record's serde form (device aspects
/// are flattened, so `strip.label_mode` is the strip aspect's member). Identity, links that commands own
/// (`device`, `page`, `functions`, endpoints, `sequence`, `cable`, `strip`), positions and
/// `source_ref`/`extra` are not settable here; they have their own commands (`assembly.attach` sets
/// `parent` and `position`).
///
/// The device aspects `assembly`, `plc_station` and `plc_module` are set as a whole (a JSON object adds
/// or replaces the aspect, `null` removes it) or member by member once they exist (FT-169, FT-270).
/// Adding a `plc_module` (or changing the module's parts) instantiates its channels from the part data
/// (normaliser N1, `channel_instantiation`). An assembly configuration's `covers` names the assembly
/// device itself or its descendants only (doc 03 §7); any other device is rejected.
///
/// A revision's members are settable while it is open; a released or withdrawn revision is immutable
/// (FR-REV-009) and refused at `/targets/<i>`. Its `state` is not settable (Open → Released is
/// `revision.release`). A `page_notes` value names existing pages, one note per page (EW-INV-019).
pub const SETTABLE: &[(KindName, &[&str])] = &[
    (
        KindName::Device,
        &[
            "tag.letter",
            "tag.counter",
            "tag.frozen",
            "tag.scheme",
            "kind",
            "parts",
            "function_text",
            "process_tag",
            "tech_data",
            "state",
            "structure",
            "strip.label_mode",
            "strip.designation_scheme",
            "strip.tag_print",
            "strip.groups",
            "strip.field_side",
            "terminal.seq",
            "terminal.designation",
            "terminal.levels",
            "terminal.rail_bonded",
            "cable.length",
            "cable.core_id_mode",
            "cable.pre_assembled",
            "assembly",
            "assembly.configs",
            "assembly.bom_basis",
            "plc_station",
            "plc_station.config_project",
            "plc_station.role",
            "plc_station.order",
            "plc_module",
            "plc_module.slot",
            "plc_module.start",
            "plc_module.part_use",
        ],
    ),
    (KindName::Function, &["cp_overrides"]),
    (
        KindName::Page,
        &[
            "title",
            "description_override",
            "structure",
            "form",
            "scale",
            "grid",
            "kind",
        ],
    ),
    (KindName::Representation, &["text_slots", "arrangement"]),
    (
        KindName::Connection,
        &[
            "kind",
            "number",
            "overrides.csa",
            "overrides.colour",
            "overrides.wire_part",
            "overrides.insulation",
            "overrides.function",
            "overrides.flags",
            "overrides.show_marker",
            "overrides.length",
        ],
    ),
    (KindName::Conductor, &["core", "colour", "csa", "shield", "pair"]),
    (
        KindName::PotentialDef,
        &["name", "ptype", "voltage", "env", "circuit_class", "props"],
    ),
    (
        KindName::StructureNode,
        &["code", "aliases", "description", "order"],
    ),
    (KindName::Section, &["title", "divider", "folio_range"]),
    (
        KindName::ReportBlock,
        &[
            "title",
            "first_folio",
            "sort",
            "group",
            "continuation",
            "empty",
            "manual",
        ],
    ),
    (KindName::ExternalRef, &["project", "device", "connection"]),
    (
        KindName::Revision,
        &["label", "date", "author", "checker", "description", "page_notes"],
    ),
];

/// The aspect members of a device: a path below one of them needs the aspect to exist.
pub const ASPECTS: &[&str] = &[
    "strip",
    "terminal",
    "cable",
    "assembly",
    "plc_station",
    "plc_module",
];

/// `fields.set` (`SetFields`): sets one field on one or more records of one kind.
///
/// For each target in order: the record's serde JSON form gets `value` at `path` (a JSON `null` removes the
/// member, which resets it to its default); intermediate objects are created, except that a path below a
/// device aspect ([`ASPECTS`]) requires the aspect to exist; the JSON is read back into the record type
/// and put. Rejected with [`CmdError::InvalidParams`] at `/path`: a path not in [`SETTABLE`] for the kind;
/// at `/value`: a value the record type does not accept (the deserialisation error as message). Targets
/// must exist and be distinct; there is at least one. Invariants and prevent rules apply at the end of the
/// transaction, so a swap of two terminal designations over two commands is not possible here, but one
/// `fields.set` that leaves a duplicate is caught later by prevent rules (ADR-0009). Outcome: none.
/// Coalesce typing in one cell with a key (FR-UI-009).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetFields {
    /// The kind of every target.
    pub kind: KindName,
    /// The records.
    #[schemars(with = "Vec<UuidS>")]
    pub targets: Vec<AnyId>,
    /// The dotted field path.
    pub path: String,
    /// The new value in the field's JSON encoding; `null` resets it.
    pub value: Value,
}

impl Command for SetFields {
    const NAME: &'static str = "fields.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.targets.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/targets",
                "give at least one target",
            ));
        }
        let mut seen = BTreeSet::new();
        for (i, t) in self.targets.iter().enumerate() {
            if !seen.insert(*t) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/targets/{i}"),
                    "the target is listed twice",
                ));
            }
        }
        let allowed = SETTABLE
            .iter()
            .find(|(k, _)| *k == self.kind)
            .map_or(&[][..], |(_, paths)| *paths);
        if !allowed.contains(&self.path.as_str()) {
            let kind = self.kind.kind();
            let message = if allowed.is_empty() {
                format!("`{}` is not settable: {kind} has no settable fields", self.path)
            } else {
                format!(
                    "`{}` is not a settable field of {kind}; settable: {}",
                    self.path,
                    allowed.join(", ")
                )
            };
            return Err(CmdError::invalid_params(Self::NAME, "/path", message));
        }
        let (path, value) = (self.path.as_str(), &self.value);
        if self.kind == KindName::Revision {
            for (i, t) in self.targets.iter().enumerate() {
                crate::commands::revision::refuse_closed_revision(
                    tx,
                    Self::NAME,
                    &format!("/targets/{i}"),
                    t.cast(),
                )?;
            }
        }
        for t in &self.targets {
            match self.kind {
                KindName::Device => set_one::<Device>(tx, *t, path, value)?,
                KindName::Function => set_one::<Function>(tx, *t, path, value)?,
                KindName::Page => set_one::<Page>(tx, *t, path, value)?,
                KindName::Representation => set_one::<Representation>(tx, *t, path, value)?,
                KindName::Connection => set_one::<Connection>(tx, *t, path, value)?,
                KindName::Conductor => set_one::<Conductor>(tx, *t, path, value)?,
                KindName::PotentialDef => set_one::<PotentialDef>(tx, *t, path, value)?,
                KindName::StructureNode => set_one::<StructureNode>(tx, *t, path, value)?,
                KindName::Section => set_one::<Section>(tx, *t, path, value)?,
                KindName::ReportBlock => set_one::<ReportBlock>(tx, *t, path, value)?,
                KindName::ExternalRef => set_one::<ExternalRef>(tx, *t, path, value)?,
                KindName::Revision => {
                    set_one::<Revision>(tx, *t, path, value)?;
                    if path == "page_notes" {
                        let notes = tx.require::<Revision>(t.cast())?.page_notes.clone();
                        crate::commands::revision::check_page_notes(tx, Self::NAME, "/value", &notes)?;
                    }
                }
                // Junctions and bonds have no settable fields: rejected above.
                KindName::Junction | KindName::Bond => {
                    return Err(CmdError::internal("fields.set: kind without settable fields"));
                }
            }
        }
        if self.kind == KindName::Device && (path == "assembly" || path.starts_with("assembly.")) {
            for t in &self.targets {
                refuse_foreign_covers(tx, t.cast())?;
            }
        }
        Ok(Outcome::default())
    }
}

/// Doc 03 §7: an assembly configuration covers the assembly device itself or its descendants, each named
/// explicitly. A covered device outside that subtree that **this edit adds** (absent from every
/// configuration of the base row) is refused at `/value`; a cover that was out of scope already (a child
/// moved away by `assembly.attach`) is a check finding (decision log ID-371, FT-260), never a refusal, so an
/// unrelated assembly edit such as `assembly.bom_basis` still commits.
fn refuse_foreign_covers(tx: &TxBuilder<'_>, device: ew_ids::Id<Device>) -> Result<(), CmdError> {
    let Some(dev) = tx.get::<Device>(device) else {
        return Ok(());
    };
    let Some(assembly) = &dev.aspects.assembly else {
        return Ok(());
    };
    let before: std::collections::BTreeSet<ew_ids::Id<Device>> = tx
        .base_row(&(Kind::Device, device.cast()))
        .and_then(|r| r.get::<Device>().cloned())
        .and_then(|d| d.aspects.assembly.clone())
        .map(|a| a.configs.iter().flat_map(|c| c.covers.iter().copied()).collect())
        .unwrap_or_default();
    for config in &assembly.configs {
        for covered in config.covers.iter().filter(|c| !before.contains(*c)) {
            // walk up from the covered device; the chain is acyclic (EW-INV-009), bounded anyway
            let mut at = Some(*covered);
            let mut steps = 0usize;
            let inside = loop {
                match at {
                    Some(d) if d == device => break true,
                    Some(d) if steps <= 4096 => {
                        at = tx.get::<Device>(d).and_then(|x| x.parent);
                        steps += 1;
                    }
                    _ => break false,
                }
            };
            if !inside {
                return Err(CmdError::invalid_params(
                    SetFields::NAME,
                    "/value",
                    format!(
                        "the configuration {} covers device {covered}, which is neither the assembly device \
                         {device} nor one of its descendants",
                        config.order_code
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Sets `value` at the dotted `path` of one record: serialise, edit the JSON, read it back, put.
fn set_one<T: Entity + Serialize + DeserializeOwned>(
    tx: &mut TxBuilder<'_>,
    target: AnyId,
    path: &str,
    value: &Value,
) -> Result<(), CmdError> {
    let id = target.cast::<<T as Entity>::Marker>();
    let record = tx.require::<T>(id)?;
    let mut json = serde_json::to_value(&*record)
        .map_err(|e| CmdError::internal(format!("fields.set: cannot serialise the record ({e})")))?;
    let segments: Vec<&str> = path.split('.').collect();
    if T::KIND == Kind::Device
        && segments.len() > 1
        && ASPECTS.contains(&segments[0])
        && !json.get(segments[0]).is_some_and(Value::is_object)
    {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/path",
            format!(
                "the device has no `{}` aspect; `{path}` cannot be set",
                segments[0]
            ),
        ));
    }
    if !set_member(&mut json, &segments, value) {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/path",
            format!("`{path}` lies below a value that is not an object"),
        ));
    }
    let next: T = serde_json::from_value(json)
        .map_err(|e| CmdError::invalid_params(SetFields::NAME, "/value", e.to_string()))?;
    tx.update::<T>(id, |r| *r = next)?;
    if T::KIND == Kind::Device && matches!(path, "tag.counter" | "tag.letter" | "tag.scheme") {
        refuse_tag_collision(tx, target.cast())?;
    }
    Ok(())
}

/// D2: a counter or letter set through `fields.set` that makes the device's page-based tag collide on its
/// page is refused with the next free counter as the remedy, never replaced silently (automatic
/// renumbering, FR-NUM-029, applies only to new devices, moves and page changes). A tag that the edit
/// leaves unchanged, an unplaced device and a device outside page-based numbering are not checked.
fn refuse_tag_collision(tx: &TxBuilder<'_>, device: ew_ids::Id<Device>) -> Result<(), CmdError> {
    use crate::numbering::{CounterScope, devices_in_scope, main_page, next_free_counter, numbered};
    let Some(dev) = tx.get::<Device>(device) else {
        return Ok(());
    };
    let unchanged = tx
        .base_row(&(Kind::Device, device.cast()))
        .and_then(|r| r.get::<Device>().map(|b| b.tag == dev.tag))
        .unwrap_or(false);
    let Some(counter) = dev.tag.counter else {
        return Ok(());
    };
    if unchanged || !numbered(&dev) {
        return Ok(());
    }
    let Some(page) = main_page(tx, device) else {
        return Ok(());
    };
    let scope = CounterScope::Page(page);
    let taken = devices_in_scope(tx, &dev.tag.letter, scope)
        .into_iter()
        .any(|(other, c)| other != device && c == Some(counter));
    if !taken {
        return Ok(());
    }
    let free = next_free_counter(tx, &dev.tag.letter, scope, Some(device));
    tx.reject(format!(
        "counter {counter} with letter {} is already used on page {page} by another device; use {free}, \
         the next free counter",
        dev.tag.letter
    ))
}

/// Sets (or, for `null`, removes) the member at `segments` below `root`, creating intermediate objects
/// when setting. `false` if an intermediate value exists and is not an object.
pub(crate) fn set_member(root: &mut Value, segments: &[&str], value: &Value) -> bool {
    let Some((last, parents)) = segments.split_last() else {
        return false;
    };
    let remove = value.is_null();
    let mut cur = root;
    for seg in parents {
        let Some(obj) = cur.as_object_mut() else {
            return false;
        };
        if remove && !obj.contains_key(*seg) {
            return true;
        }
        cur = obj
            .entry((*seg).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    let Some(obj) = cur.as_object_mut() else {
        return false;
    };
    if remove {
        obj.remove(*last);
    } else {
        obj.insert((*last).to_string(), value.clone());
    }
    true
}
