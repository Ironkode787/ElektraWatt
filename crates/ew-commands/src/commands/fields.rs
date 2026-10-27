// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `fields.set` (`SetFields`): the property grid's command, also over a multi-selection (doc 06 §3.1, §3.7); and
//! `fields.set_many` (`SetManyFields`, WP-S2.05 part 2): one value per target set, the command of find and replace and
//! of "freeze" on several devices, one transaction of `fields.set` steps.

use std::collections::BTreeSet;

use ew_model::Entity;
use ew_schema::{
    AnyId, Conductor, Connection, Device, ExternalRef, Function, Kind, Page, PageItem, PotentialDef,
    ReportBlock, Representation, Revision, Section, StructureBox, StructureNode,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::{Command, Outcome};
use crate::commands::common::KindName;
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
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
/// A page item's members (WP-S2.04 part 4a) are settable within its own variant only (`kind.free_text.text` on a
/// free text, refused at `/path` on a graphic), never `kind` as a whole or a wire marker's `connections` (a group is
/// changed by deleting and placing it); after the value is read back, the checks of `item.place`, `marker.place` and
/// `box.structure` apply (degenerate shapes, an unknown `bind`, an empty text without `bind`, the angles, fewer than
/// three outline vertices, a node of the wrong aspect, a missing external reference), refused at `/value`. An edit of
/// a wire marker writes a clean list: entries naming a connection that does not exist or is not on its page (a file
/// of an older build) are dropped.
///
/// A revision's members are settable while it is open; a released or withdrawn revision is immutable
/// (FR-REV-009) and refused at `/targets/<i>`. Its `state` is not settable (Open → Released is
/// `revision.release`). A `page_notes` value names existing pages, one note per page (EW-INV-019), and its
/// notes take no member other than `page` and `description`.
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
    (
        KindName::PageItem,
        &[
            "layer",
            "kind.graphic.shape",
            "kind.graphic.stroke",
            "kind.graphic.dash",
            "kind.graphic.fill",
            "kind.free_text.text",
            "kind.free_text.bind",
            "kind.free_text.at",
            "kind.free_text.angle",
            "kind.free_text.style",
            "kind.free_text.align",
            "kind.wire_marker.at",
            "kind.wire_marker.angle",
        ],
    ),
    (
        KindName::StructureBox,
        &["outline", "style", "caption", "assigns", "external"],
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
/// at `/value`: a value the record type does not accept (the deserialisation error as message), and a
/// connection's `overrides.colour` with a code that is neither IEC 60757 nor a company code of the project's
/// colour profile (FR-CON-019, `marker.place`'s check and text). Targets
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
        self.apply(tx)?;
        Ok(Outcome::default())
    }
}

impl SetFields {
    /// The edit of [`SetFields`] on `tx`, with `fields.set`'s pointers (`fields.set_many` prefixes them).
    fn apply(&self, tx: &mut TxBuilder<'_>) -> Result<(), CmdError> {
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
                KindName::Connection => {
                    set_one::<Connection>(tx, *t, path, value)?;
                    if path == "overrides.colour" {
                        refuse_unknown_colour(tx, t.cast())?;
                    }
                }
                KindName::Conductor => set_one::<Conductor>(tx, *t, path, value)?,
                KindName::PotentialDef => set_one::<PotentialDef>(tx, *t, path, value)?,
                KindName::StructureNode => set_one::<StructureNode>(tx, *t, path, value)?,
                KindName::Section => set_one::<Section>(tx, *t, path, value)?,
                KindName::ReportBlock => set_one::<ReportBlock>(tx, *t, path, value)?,
                KindName::ExternalRef => set_one::<ExternalRef>(tx, *t, path, value)?,
                KindName::Revision => {
                    if path == "page_notes" {
                        crate::commands::revision::refuse_open_page_notes(Self::NAME, value)?;
                    }
                    set_one::<Revision>(tx, *t, path, value)?;
                    if path == "page_notes" {
                        let notes = tx.require::<Revision>(t.cast())?.page_notes.clone();
                        crate::commands::revision::check_page_notes(tx, Self::NAME, "/value", &notes)?;
                    }
                }
                KindName::PageItem => {
                    refuse_other_variant(tx, *t, path)?;
                    set_one::<PageItem>(tx, *t, path, value)?;
                    let mut item = (*tx.require::<PageItem>(t.cast())?).clone();
                    if let Some(p) = crate::commands::items::item_problem(&item) {
                        return Err(CmdError::invalid_params(Self::NAME, "/value", p));
                    }
                    let before = item.clone();
                    crate::commands::items::clean_marker(tx, &mut item);
                    if item != before {
                        tx.put(item);
                    }
                }
                KindName::StructureBox => {
                    set_one::<StructureBox>(tx, *t, path, value)?;
                    let b = tx.require::<StructureBox>(t.cast())?;
                    crate::commands::structure_box::check_box(tx, Self::NAME, "", &b).map_err(
                        |e| match e {
                            CmdError::InvalidParams { message, .. } => {
                                CmdError::invalid_params(Self::NAME, "/value", message)
                            }
                            other => other,
                        },
                    )?;
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
        Ok(())
    }
}

/// FR-CON-019 for `fields.set` on a connection's `overrides.colour` (FT-451): the colour as read back must be IEC
/// 60757 codes or company codes of the project's colour profile, the same check and text as `marker.place`'s;
/// refused at `/value`. A removed colour (`null`) passes.
fn refuse_unknown_colour(tx: &TxBuilder<'_>, conn: ew_ids::Id<Connection>) -> Result<(), CmdError> {
    let c = tx.require::<Connection>(conn)?;
    if let Some(colour) = &c.overrides.colour {
        let company = crate::commands::items::company_codes(tx);
        if let Some(p) = crate::commands::items::colour_problem(&company, colour) {
            return Err(CmdError::invalid_params(SetFields::NAME, "/value", p));
        }
    }
    Ok(())
}

// WP-S2.05 part 2 ------------------------------------------------------------------------------------------------

/// The most entries one `fields.set_many` takes.
pub const SET_MANY_MAX_EDITS: usize = 10_000;

/// `fields.set_many` (`SetManyFields`, WP-S2.05 part 2; FR-DEV-032, FR-UI-020): sets one value per (kind, targets,
/// path) entry, the entries in the given order, in **one** transaction (one undo step).
///
/// Each entry is applied exactly as `fields.set` with those parameters would be: the same [`SETTABLE`] paths, the same
/// per-kind checks (closed revisions, aspects, assembly covers, the tag-collision refusal) and the same rejections,
/// their JSON Pointer prefixed by `/edits/<i>`. The caller computes every value (find and replace computes the
/// replaced texts, "freeze" each device's own product designation): the command searches nothing and reads no derived
/// value, so a command log replays it exactly. Rejected with [`CmdError::InvalidParams`] at `/edits`: no entry, or
/// more than 10,000 entries; at `/edits/<j>/targets/<k>`: a (target, path) pair that an earlier entry already names.
/// Cost: `Bulk` when the entries name more than 500 targets in total (`BULK_ABOVE`, as `edit.delete`),
/// else interactive. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetManyFields {
    /// The entries, each the parameters of one `fields.set` (1 to 10,000).
    pub edits: Vec<SetFields>,
}

impl Command for SetManyFields {
    const NAME: &'static str = "fields.set_many";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, _model: &ew_model::Model) -> Cost {
        let targets: usize = self.edits.iter().map(|e| e.targets.len()).sum();
        if targets > crate::commands::BULK_ABOVE {
            Cost::Bulk
        } else {
            Cost::Interactive
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.edits.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/edits",
                "give at least one entry",
            ));
        }
        if self.edits.len() > SET_MANY_MAX_EDITS {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/edits",
                format!(
                    "{} entries are more than the {SET_MANY_MAX_EDITS} one command takes",
                    self.edits.len()
                ),
            ));
        }
        // One (target, path) pair per command: a later entry naming it again is refused before anything is applied.
        let mut pairs: BTreeSet<(AnyId, &str)> = BTreeSet::new();
        for (j, e) in self.edits.iter().enumerate() {
            let mut own = BTreeSet::new();
            for (k, t) in e.targets.iter().enumerate() {
                // A target twice within one entry is `fields.set`'s own refusal, at its own pointer below.
                if own.insert(*t) && !pairs.insert((*t, e.path.as_str())) {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        format!("/edits/{j}/targets/{k}"),
                        format!(
                            "an earlier entry already sets `{}` on this target; give each (target, path) once",
                            e.path
                        ),
                    ));
                }
            }
        }
        for (i, e) in self.edits.iter().enumerate() {
            e.apply(tx).map_err(|err| match err {
                CmdError::InvalidParams { path, message, .. } => {
                    CmdError::invalid_params(Self::NAME, format!("/edits/{i}{path}"), message)
                }
                other => other,
            })?;
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

/// A page-item path names a variant (`kind.<variant>.<member>`): it must be the record's own (WP-S2.04 §5.2).
fn refuse_other_variant(tx: &TxBuilder<'_>, target: AnyId, path: &str) -> Result<(), CmdError> {
    let Some(variant) = path.strip_prefix("kind.").and_then(|r| r.split('.').next()) else {
        return Ok(());
    };
    let item = tx.require::<PageItem>(target.cast())?;
    let own = item.kind.name().unwrap_or("");
    if own != variant {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/path",
            format!("page item {target} is a {own}, not a {variant}; `{path}` is not one of its fields"),
        ));
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
