// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC exchange (doc 10 §3.9, §4): `plc.record_exchange` (`RecordExchange`), `plc.apply_exchange`
//! (`ApplyExchange`, Bulk), `plc.unlock` (`Unlock`) and `plc.apply_instrument_list` (`ApplyInstrumentList`, Bulk),
//! and the protection of PLC-owned fields ([`refuse_owned_edits`]).
//!
//! The exchange image, the matching and the three-way merge are `ew-plc`'s; the commands here read them on the
//! transaction's base snapshot, whose derived values a command may read (ADR-0008 amendment 4), and write the
//! result: the field edits, the snapshot records of `exchange/<config-project>.jsonl`, the stamp and the ownership.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_plc::ew_caex::ExchangeImage;
use ew_plc::instrument::{InstrumentRow, Signal};
use ew_plc::{Change, Choice, MergeOptions, Parked, ParkedModule, Vocabulary};
use ew_schema::{
    AddrSpec, AddrValue, AnyId, BusSystem, Channel, ChannelState, Date, Device, DeviceAspects,
    ExchangeDirection, ExchangeFormat, ExchangeObject, ExchangeRecord, ExchangeStamp, Extra, FieldGroup,
    ForeignId, FrozenTag, Function, Kind, NetAddress, Ownership, PlcAddress, PlcArea, PlcConfigProject,
    PlcModule, PlcSize, PlcStation, ProfinetAddress, RackSlot, Sha256, SlotPath, StationRole, TagSpec,
    TypeIdent,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::text::{Text, text};
use crate::tx::TxBuilder;

// ---------------------------------------------------------------------------------------------------------
// The protection of PLC-owned fields (doc 10 §2.2, §3.9)

/// The configuration project a device belongs to: that of the device's own `plc_station` aspect, else of its
/// nearest ancestor's. A hostile parent cycle ends at a device already visited.
fn config_project_of(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<std::sync::Arc<PlcConfigProject>> {
    let mut seen = BTreeSet::new();
    let mut at = Some(device);
    while let Some(d) = at.filter(|d| seen.insert(*d)) {
        let dev = tx.get::<Device>(d)?;
        if let Some(station) = &dev.aspects.plc_station {
            return tx.get::<PlcConfigProject>(station.config_project);
        }
        at = dev.parent;
    }
    None
}

/// The words of a field group.
fn group_words(group: &FieldGroup) -> Text {
    match group {
        FieldGroup::ModuleStarts => text!("refusal-plc-group-module-starts"),
        FieldGroup::Symbols => text!("refusal-plc-group-symbols"),
        FieldGroup::PlcData | FieldGroup::Unknown(_) => text!("refusal-plc-group-plc-data"),
    }
}

/// Refuses the transaction when it edits a field the PLC owns (doc 10 §3.9, FR-PLC-033): a module's start, a
/// station's firmware or the address of a bus port, in a configuration project whose first import protected that
/// group and `plc.unlock` has not lifted it since. Judged on what the transaction changed against its base, so
/// every path of an edit is covered (`plc.set_start`, `plc.readdress`, `plc.convert_manual_to_auto`, `fields.set`
/// and `fields.set_many` call it); a device or function the transaction creates is no edit. The import itself
/// (`plc.apply_exchange`) does not call it.
pub(crate) fn refuse_owned_edits(tx: &TxBuilder<'_>) -> Result<(), CmdError> {
    let refuse = |device: Id<Device>, group: FieldGroup| -> Result<(), CmdError> {
        let Some(cfg) = config_project_of(tx, device) else {
            return Ok(());
        };
        match Ownership::protected_since(cfg.ownership.as_ref(), &group) {
            Some(date) => tx.reject(text!(
                "refusal-plc-owned",
                crate::words::device(tx, device),
                group_words(&group),
                cfg.name.to_string(),
                date.to_string()
            )),
            None => Ok(()),
        }
    };
    for key in tx.changed_of(Kind::Device) {
        let id: Id<Device> = key.1.cast();
        let (Some(before), Some(now)) = (tx.base_row(&key), tx.get::<Device>(id)) else {
            continue;
        };
        let Some(before) = before.get::<Device>() else {
            continue;
        };
        let start = |d: &Device| d.aspects.plc_module.as_ref().map(|m| m.start.clone());
        if start(before) != start(&now) && start(before).is_some() {
            refuse(id, FieldGroup::ModuleStarts)?;
        }
        let firmware = |d: &Device| d.aspects.plc_station.as_ref().and_then(|s| s.firmware.clone());
        if firmware(before) != firmware(&now) {
            refuse(id, FieldGroup::PlcData)?;
        }
    }
    for key in tx.changed_of(Kind::Function) {
        let id: Id<Function> = key.1.cast();
        let (Some(before), Some(now)) = (tx.base_row(&key), tx.get::<Function>(id)) else {
            continue;
        };
        let Some(before) = before.get::<Function>() else {
            continue;
        };
        let address = |f: &Function| f.bus_port.as_ref().map(|p| p.address.clone());
        if let (true, Some(device)) = (address(before) != address(&now), now.device) {
            refuse(device, FieldGroup::PlcData)?;
        }
    }
    Ok(())
}

/// `plc.unlock` (`Unlock`, doc 10 §3.9 `plc.unlock{scope, group}`, FR-PLC-033): lifts the protection of a PLC-owned
/// field group of a configuration project, so that its fields can be edited again.
///
/// The group is `module_starts` or `plc_data` (firmware, bus addresses); `symbols` when the configuration project
/// gives symbols to the PLC. The protection starts with the first applied PLC import and covers the groups the PLC
/// owns; an unlocked group stays unlocked until the next applied import, which protects it again. An edit made
/// meanwhile is an ordinary difference of that import's merge. Rejected: a configuration project that does not exist
/// ([`CmdError::NotFound`]); a group that is not protected (no import yet, another owner, unlocked already); a group
/// of a newer format (at `/group`). Outcome: the configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Unlock {
    /// The configuration project.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
    /// The field group to unlock.
    pub group: FieldGroup,
}

impl Command for Unlock {
    const NAME: &'static str = "plc.unlock";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if let FieldGroup::Unknown(v) = &self.group {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/group",
                text!("refusal-plc-exchange-format", v.to_string()),
            ));
        }
        let cfg = tx.require::<PlcConfigProject>(self.config_project)?;
        if Ownership::protected_since(cfg.ownership.as_ref(), &self.group).is_none() {
            return tx.reject(text!(
                "refusal-plc-unlock-not-protected",
                group_words(&self.group),
                cfg.name.to_string()
            ));
        }
        let group = self.group.clone();
        tx.update::<PlcConfigProject>(self.config_project, |c| {
            c.ownership.get_or_insert_default().unlocked.push(group);
        })?;
        Ok(Outcome::primary((
            Kind::PlcConfigProject,
            self.config_project.cast(),
        )))
    }
}

// ---------------------------------------------------------------------------------------------------------
// The snapshot

/// One object of the next snapshot: its kind, the record it is, the ID an exchange file gave it and its values.
type Entry = (ExchangeObject, AnyId, Option<String>, BTreeMap<String, String>);

/// Writes the snapshot records of `entries` and removes those of `gone`; with `whole`, every other record of the
/// configuration project's stations, modules, channels, tags and nodes goes too (the exchange covered the whole
/// configuration project), else only those whose record the project no longer has. Instrument rows stay.
fn write_snapshot(
    tx: &mut TxBuilder<'_>,
    cfg: Id<PlcConfigProject>,
    entries: Vec<Entry>,
    gone: &[(ExchangeObject, AnyId)],
    whole: bool,
) -> Result<(), CmdError> {
    let mut kept: BTreeSet<Id<ExchangeRecord>> = BTreeSet::new();
    for (object, target, foreign, fields) in entries {
        let id: Id<ExchangeRecord> =
            ew_ids::v5::exchange_id(cfg, "exchange", object.name(), target.as_uuid());
        kept.insert(id);
        let record = ExchangeRecord {
            id,
            v: 1,
            config_project: cfg,
            object: object.clone(),
            target,
            fields,
            extra: Extra::new(),
        };
        if tx.get::<ExchangeRecord>(id).as_deref() != Some(&record) {
            tx.put(record);
        }
        if let Some(foreign) = foreign {
            let record = ForeignId {
                id: ew_ids::v5::exchange_id(cfg, "foreign", object.name(), target.as_uuid()),
                v: 1,
                config_project: cfg,
                object,
                target,
                foreign,
                extra: Extra::new(),
            };
            if tx.get::<ForeignId>(record.id).as_deref() != Some(&record) {
                tx.put(record);
            }
        }
    }
    let gone: BTreeSet<Id<ExchangeRecord>> = gone
        .iter()
        .map(|(o, t)| ew_ids::v5::exchange_id(cfg, "exchange", o.name(), t.as_uuid()))
        .collect();
    for r in tx.iter::<ExchangeRecord>() {
        if r.config_project != cfg || r.object == ExchangeObject::Instrument || kept.contains(&r.id) {
            continue;
        }
        let lost = !(tx.exists::<Device>(r.target.cast())
            || tx.exists::<Channel>(r.target.cast())
            || tx.exists::<Function>(r.target.cast()));
        if whole || lost || gone.contains(&r.id) {
            tx.delete_if_exists((Kind::ExchangeRecord, r.id.cast()))?;
        }
    }
    Ok(())
}

/// The format of a recorded or applied exchange, refused when it is a value of a newer format.
fn known_format(name: &'static str, format: &ExchangeFormat) -> Result<(), CmdError> {
    match format {
        ExchangeFormat::Unknown(v) => Err(CmdError::invalid_params(
            name,
            "/format",
            text!("refusal-plc-exchange-format", v.to_string()),
        )),
        _ => Ok(()),
    }
}

/// `plc.record_exchange` (`RecordExchange`, doc 10 §4.4 "Record AML exchange"): records an export of a configuration
/// project as the base of the next import's merge.
///
/// Writes the exchange snapshot (`exchange/<config-project>.jsonl`): one `exchange_record` per station, module,
/// channel, tag and bus node of the exchange image, with the values as exported, and the configuration project's
/// stamp `exchange {format, version, file_sha256, direction: export, date}`. With `stations`, the snapshot of those
/// stations only; the records of the others stay. The image is the one the export wrote: the configuration project
/// as it is on the transaction's base. The date is the export's document date, a parameter: a command reads no clock.
/// EW-PLC-016 reports the configuration project once it differs from this snapshot (FR-PLC-035). Rejected: a record
/// that does not exist ([`CmdError::NotFound`]); a format of a newer build (at `/format`). Undoable. Outcome: the
/// configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordExchange {
    /// The configuration project.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
    /// The exported stations; empty: every station of the configuration project.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub stations: Vec<Id<Device>>,
    /// The format of the written file.
    pub format: ExchangeFormat,
    /// The format's version ("1.4.0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// SHA-256 of the written file.
    pub file_sha256: Sha256,
    /// The export's document date.
    pub date: Date,
}

impl Command for RecordExchange {
    const NAME: &'static str = "plc.record_exchange";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        known_format(Self::NAME, &self.format)?;
        tx.require::<PlcConfigProject>(self.config_project)?;
        for s in &self.stations {
            tx.require::<Device>(*s)?;
        }
        let base = tx.base_model().clone();
        let q = ew_derive::Q::new(&base);
        let scope = (!self.stations.is_empty()).then_some(self.stations.as_slice());
        let projection = ew_plc::exchange_image(&q, self.config_project, scope)
            .ok_or_else(|| CmdError::internal("plc.record_exchange: no exchange image"))?;
        let entries = ew_plc::ours(&projection)
            .into_iter()
            .map(|(_, target, object)| (object.object, target, None, object.fields))
            .collect();
        write_snapshot(tx, self.config_project, entries, &[], scope.is_none())?;
        let stamp = ExchangeStamp {
            format: self.format.clone(),
            version: self.version.clone(),
            file_sha256: self.file_sha256,
            direction: ExchangeDirection::Export,
            date: self.date,
            extra: Extra::new(),
        };
        tx.update::<PlcConfigProject>(self.config_project, |c| c.exchange = Some(stamp))?;
        Ok(Outcome::primary((
            Kind::PlcConfigProject,
            self.config_project.cast(),
        )))
    }
}

// ---------------------------------------------------------------------------------------------------------
// The import

/// What an import does with one change or one parked object (doc 10 §4.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// The project's value stays.
    Ours,
    /// The file's value is written.
    Theirs,
    /// The parked station or module is created, unplaced.
    Create,
}

/// `plc.apply_exchange` (`ApplyExchange`, doc 10 §3.9 `plc.apply_exchange{cfg, image, resolutions}`, §4.4;
/// FR-PLC-032, FR-PLC-033): applies an exchange file of PLC software to a configuration project.
///
/// `image` is the file as the neutral exchange image (an AML file, or the rows of a TIA tag table). Its objects are
/// matched to the project's (the ID, the IEC designation, the position, the order number; a tag by its channel, its
/// name, its address), and every field is resolved three ways against the last exchange's snapshot under the
/// configuration project's ownership: a change of the file alone is applied when the PLC or both own the field and
/// listed as a rejected PLC change when the schematic owns it; a change of the project alone is kept; a change of
/// both is a conflict, taken from the file when the PLC owns the field and kept otherwise. `resolutions` overrides
/// that per change (`ours` or `theirs`), by the change's key `<object>/<record>/<field>`; a field of no group
/// (structure, designations, derived values) cannot be taken. A foreign object without a counterpart is parked and
/// never created silently: `create` under its ID creates a parked station with its modules, or a parked module of a
/// matched station, as unplaced devices. An object the last exchange held and the file no longer has is listed and
/// kept. Only the stations in the file are compared, every station of the configuration project with `complete`.
///
/// Written: a tag's name, data type and comment (`Channel.symbol`, `data_type`, `comment`); a module's start as a
/// `manual` start; a station's firmware; a bus node's address (the PROFIBUS station address; the PROFINET device
/// name, IP address, mask and gateway). Channel addresses stay `auto`. In the same transaction the snapshot becomes
/// the file's image, the stamp `exchange {…, direction: import}` is set, and the PLC-owned groups are protected from
/// here on (`ownership.imported`, every unlocked group locked again), so one undo restores all of it.
///
/// **Bulk:** it runs as a preview first; the outcome's notices list every change with its sides and what is done,
/// the parked objects and the removed ones. A file equal to the last exchange and the project gives no change.
/// Rejected: a record that does not exist ([`CmdError::NotFound`]); a format of a newer build or `plcopen`, which is
/// not imported (at `/format`); a resolution that names no change or parked object, `theirs` on a field that cannot
/// be taken, `create` on an object that cannot be created (at `/resolutions/<key>`); a file value that cannot be
/// written (a start that is no number, an address that is no address), with the value and the reason. Outcome: the
/// configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyExchange {
    /// The configuration project.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
    /// The file's content.
    pub image: ExchangeImage,
    /// The format of the read file: `aml` or `tia_xlsx`.
    pub format: ExchangeFormat,
    /// The format's version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// SHA-256 of the read file.
    pub file_sha256: Sha256,
    /// The import's document date.
    pub date: Date,
    /// The file is complete: a station it does not hold was removed in the PLC software.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub complete: bool,
    /// The language of a tag table's comment column; absent: a comment without a language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_lang: Option<String>,
    /// The choices, by the key of a change or the ID of a parked object.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resolutions: BTreeMap<String, Resolution>,
}

impl ApplyExchange {
    /// The merge options of this import under `ownership`.
    fn options<'a>(&self, ownership: Option<&'a Ownership>) -> MergeOptions<'a> {
        MergeOptions {
            ownership,
            complete: self.complete,
            vocabulary: match self.format {
                ExchangeFormat::TiaXlsx => Vocabulary::Tags {
                    comment: ew_plc::tia::comment_field(self.comment_lang.as_deref()),
                },
                _ => Vocabulary::All,
            },
        }
    }
}

impl Command for ApplyExchange {
    const NAME: &'static str = "plc.apply_exchange";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        known_format(Self::NAME, &self.format)?;
        if self.format == ExchangeFormat::Plcopen {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/format",
                text!("refusal-plc-exchange-format", "plcopen"),
            ));
        }
        let cfg = tx.require::<PlcConfigProject>(self.config_project)?;
        let base = tx.base_model().clone();
        let q = ew_derive::Q::new(&base);
        let projection = ew_plc::exchange_image(&q, self.config_project, None)
            .ok_or_else(|| CmdError::internal("plc.apply_exchange: no exchange image"))?;
        let snapshot = ew_plc::snapshot(&q, self.config_project);
        let plan = ew_plc::merge(
            &projection,
            &snapshot,
            &self.image,
            &self.options(cfg.ownership.as_ref()),
        );

        let pointer = |key: &str| format!("/resolutions/{}", key.replace('~', "~0").replace('/', "~1"));
        for key in self.resolutions.keys() {
            let known =
                plan.changes.iter().any(|c| c.id == *key) || plan.parking.iter().any(|p| p.id == *key);
            if !known {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    pointer(key),
                    text!("refusal-plc-exchange-resolution", key.clone()),
                ));
            }
        }

        let mut notices = Vec::new();
        let mut nodes: BTreeMap<AnyId, Vec<&Change>> = BTreeMap::new();
        for c in &plan.changes {
            let choice = match self.resolutions.get(&c.id) {
                Some(Resolution::Ours) => Choice::Ours,
                Some(Resolution::Theirs) => Choice::Theirs,
                Some(Resolution::Create) => {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        pointer(&c.id),
                        text!("refusal-plc-exchange-change-choice", c.id.clone()),
                    ));
                }
                None => c.default,
            };
            if choice == Choice::Theirs {
                if !c.acceptable {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        pointer(&c.id),
                        text!(
                            "refusal-plc-exchange-not-acceptable",
                            ew_plc::field_words(&c.object, &c.field),
                            c.subject.clone()
                        ),
                    ));
                }
                if c.object == ExchangeObject::Node {
                    nodes.entry(c.target).or_default().push(c);
                } else {
                    write_field(tx, c)?;
                }
            }
            let side = |v: &Option<String>| v.clone().unwrap_or_else(|| "nothing".to_owned());
            notices.push(format!(
                "{}: {} {} → {} ({}, {})",
                c.subject,
                ew_plc::field_words(&c.object, &c.field),
                side(&c.ours),
                side(&c.theirs),
                c.state.word(),
                if choice == Choice::Theirs {
                    "taken from the file"
                } else {
                    "kept"
                }
            ));
        }
        for (target, changes) in nodes {
            write_node(tx, target.cast(), &changes)?;
        }

        let mut entries: Vec<Entry> = plan
            .matched
            .iter()
            .map(|m| (m.object.clone(), m.target, m.foreign.clone(), m.fields.clone()))
            .collect();
        for p in &plan.parking {
            match self.resolutions.get(&p.id) {
                Some(Resolution::Create) if p.creatable => {
                    entries.extend(create_parked(tx, self.config_project, p)?);
                    notices.push(format!(
                        "{} {}: created unplaced",
                        ew_plc::object_words(&p.object),
                        p.name
                    ));
                }
                Some(Resolution::Create) => {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        pointer(&p.id),
                        text!("refusal-plc-exchange-not-creatable", p.name.clone()),
                    ));
                }
                Some(_) => {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        pointer(&p.id),
                        text!("refusal-plc-exchange-parked-choice", p.id.clone()),
                    ));
                }
                None => notices.push(format!(
                    "{} {}: parked ({})",
                    ew_plc::object_words(&p.object),
                    p.name,
                    if p.ambiguous {
                        "several objects of the project match it"
                    } else {
                        "nothing in the project matches it"
                    }
                )),
            }
        }
        for (object, _, name) in &plan.removed {
            notices.push(format!(
                "{} {name}: no longer in the file (kept in the project)",
                ew_plc::object_words(object)
            ));
        }

        let gone: Vec<(ExchangeObject, AnyId)> =
            plan.removed.iter().map(|(o, t, _)| (o.clone(), *t)).collect();
        write_snapshot(tx, self.config_project, entries, &gone, false)?;
        let stamp = ExchangeStamp {
            format: self.format.clone(),
            version: self.version.clone(),
            file_sha256: self.file_sha256,
            direction: ExchangeDirection::Import,
            date: self.date,
            extra: Extra::new(),
        };
        let date = self.date;
        tx.update::<PlcConfigProject>(self.config_project, |c| {
            c.exchange = Some(stamp);
            let ownership = c.ownership.get_or_insert_default();
            ownership.imported.get_or_insert(date);
            ownership.unlocked.clear();
        })?;
        let mut outcome = Outcome::primary((Kind::PlcConfigProject, self.config_project.cast()));
        outcome.notices = notices;
        Ok(outcome)
    }
}

/// The refusal of a file value that cannot be written.
fn bad_value(c: &Change, why: impl std::fmt::Display) -> CmdError {
    CmdError::invalid_params(
        ApplyExchange::NAME,
        "/image",
        text!(
            "refusal-plc-exchange-value",
            c.theirs.clone().unwrap_or_default(),
            ew_plc::field_words(&c.object, &c.field),
            c.subject.clone(),
            why.to_string()
        ),
    )
}

/// The file's value of `c` as an identifier; `None` when the file has none.
fn ident(c: &Change) -> Result<Option<Ident>, CmdError> {
    c.theirs
        .as_deref()
        .map(|t| t.parse::<Ident>().map_err(|e| bad_value(c, e)))
        .transpose()
}

/// Writes the file's value of one change of a tag, a module or a station.
fn write_field(tx: &mut TxBuilder<'_>, c: &Change) -> Result<(), CmdError> {
    match (&c.object, c.field.as_str()) {
        (ExchangeObject::Tag, "name") => {
            let v = ident(c)?;
            tx.update::<Channel>(c.target.cast(), |ch| ch.symbol = v)
        }
        (ExchangeObject::Tag, "data_type") => {
            let v = ident(c)?;
            tx.update::<Channel>(c.target.cast(), |ch| ch.data_type = v)
        }
        (ExchangeObject::Tag, field) => {
            let lang = match field.strip_prefix("comment.") {
                Some(l) => Some(l.parse::<ew_mltext::LangTag>().map_err(|e| bad_value(c, e))?),
                None => None,
            };
            let text = c.theirs.clone();
            tx.update::<Channel>(c.target.cast(), |ch| match (lang, text) {
                (Some(l), Some(t)) => ch.comment.set_lang(l, &t),
                (Some(l), None) => {
                    ch.comment.remove_lang(&l);
                }
                (None, Some(t)) => ch.comment.set_neutral(&t),
                (None, None) => ch.comment = MlText::default(),
            })
        }
        (ExchangeObject::Module, field) => {
            let Some(text) = &c.theirs else {
                // the file gives the module no range in this direction: its start stays
                return Ok(());
            };
            let byte: u32 = text
                .parse()
                .map_err(|_| bad_value(c, "a start is a byte number"))?;
            let input = field == "start.input";
            let start = AddrSpec::Manual(PlcAddress {
                area: if input { PlcArea::I } else { PlcArea::Q },
                size: PlcSize::Bit,
                byte,
                bit: None,
                extra: Extra::new(),
            });
            tx.update::<Device>(c.target.cast(), |d| {
                if let Some(m) = d.aspects.plc_module.as_mut() {
                    if input {
                        m.start.input = start;
                    } else {
                        m.start.output = start;
                    }
                }
            })
        }
        (ExchangeObject::Station, _) => {
            let v = ident(c)?;
            tx.update::<Device>(c.target.cast(), |d| {
                if let Some(s) = d.aspects.plc_station.as_mut() {
                    s.firmware = v;
                }
            })
        }
        _ => Ok(()),
    }
}

/// Writes the file's values of the changes of one bus node into its port's address: the station address of a
/// PROFIBUS port, the device name and IPv4 data of a PROFINET port.
fn write_node(tx: &mut TxBuilder<'_>, function: Id<Function>, changes: &[&Change]) -> Result<(), CmdError> {
    let f = tx.require::<Function>(function)?;
    let Some(port) = f.bus_port.clone() else {
        return Ok(());
    };
    let value = |name: &str| changes.iter().find(|c| c.field == name).copied();
    let address =
        match port.bus {
            BusSystem::ProfibusDp => match value("address") {
                Some(c) => match &c.theirs {
                    Some(t) => AddrValue::Value(NetAddress::Profibus(t.parse().map_err(|_| {
                        bad_value(c, "a PROFIBUS station address is a number from 0 to 255")
                    })?)),
                    None => AddrValue::Tbd,
                },
                None => port.address.clone(),
            },
            BusSystem::Profinet => {
                let current = match &port.address {
                    AddrValue::Value(NetAddress::Profinet(p)) => Some(p.clone()),
                    _ => None,
                };
                let name = match value("device_name") {
                    Some(c) => ident(c)?,
                    None => current.as_ref().map(|p| p.name.clone()),
                };
                let Some(name) = name else {
                    let c = changes[0];
                    return Err(bad_value(c, "a PROFINET address needs the device name"));
                };
                let ip = |field: &str, now: Option<std::net::Ipv4Addr>| match value(field) {
                    Some(c) => c
                        .theirs
                        .as_deref()
                        .map(|t| t.parse().map_err(|_| bad_value(c, "it is no IPv4 address")))
                        .transpose(),
                    None => Ok(now),
                };
                AddrValue::Value(NetAddress::Profinet(ProfinetAddress {
                    name,
                    ip: ip("address", current.as_ref().and_then(|p| p.ip))?,
                    mask: ip("mask", current.as_ref().and_then(|p| p.mask))?,
                    gateway: ip("router", current.as_ref().and_then(|p| p.gateway))?,
                    extra: current.map(|p| p.extra).unwrap_or_default(),
                }))
            }
            _ => {
                return Err(bad_value(
                    changes[0],
                    "the port's bus has no address an import writes",
                ));
            }
        };
    tx.update::<Function>(function, |f| {
        if let Some(p) = f.bus_port.as_mut() {
            p.address = address;
        }
    })
}

/// A new unplaced device named by `designation` (an IEC product designation, `-K01`) or, without one, by `name`: its
/// tag is frozen to that text, its letter the text's leading letters (`K` when it has none).
fn new_device(
    tx: &mut TxBuilder<'_>,
    designation: Option<&str>,
    name: &str,
    parent: Option<Id<Device>>,
    aspects: DeviceAspects,
) -> Result<Id<Device>, CmdError> {
    let text = designation.unwrap_or(name).trim();
    let product = text
        .rsplit(['-', '+', '='])
        .next()
        .filter(|p| !p.is_empty())
        .unwrap_or("PLC");
    let letters: String = product.chars().take_while(char::is_ascii_alphabetic).collect();
    let bad = |e: ew_mltext::TextError| {
        CmdError::invalid_params(
            ApplyExchange::NAME,
            "/image",
            text!(
                "refusal-plc-exchange-value",
                text.to_owned(),
                "designation",
                name.to_owned(),
                e.to_string()
            ),
        )
    };
    let letter: Ident = if letters.is_empty() { "K" } else { &letters }
        .parse()
        .map_err(bad)?;
    let id: Id<Device> = tx.mint();
    tx.insert(Device {
        id,
        v: 1,
        tag: TagSpec {
            frozen: Some(FrozenTag {
                text: product.parse().map_err(bad)?,
                sign: Default::default(),
                extra: Extra::new(),
            }),
            ..TagSpec::new(letter, None)
        },
        kind: None,
        parent,
        position: None,
        structure: Default::default(),
        parts: Vec::new(),
        function_text: MlText::default(),
        process_tag: None,
        tech_data: Default::default(),
        state: Default::default(),
        source_ref: None,
        aspects,
        extra: Extra::new(),
    })?;
    Ok(id)
}

/// A parked module as an unplaced child device of `station`: a rack slot at its position, its starts as `manual`
/// starts, its order number in `type_ident`. It has no part, so no channels: assigning its part creates them.
fn new_module(tx: &mut TxBuilder<'_>, station: Id<Device>, m: &ParkedModule) -> Result<Entry, CmdError> {
    let start = |area: PlcArea, byte: Option<u32>| match byte {
        Some(byte) => AddrSpec::Manual(PlcAddress {
            area,
            size: PlcSize::Bit,
            byte,
            bit: None,
            extra: Extra::new(),
        }),
        None => AddrSpec::Auto,
    };
    let mut module = PlcModule {
        slot: SlotPath::Rack(RackSlot {
            rack: 0,
            slot: u16::try_from(m.position).unwrap_or(u16::MAX),
            subslot: None,
            extra: Extra::new(),
        }),
        type_ident: m
            .order_number
            .as_deref()
            .and_then(|n| n.parse().ok())
            .map(|n| TypeIdent {
                order_number: Some(n),
                extra: Extra::new(),
            }),
        ..PlcModule::default()
    };
    module.start.input = start(PlcArea::I, m.starts.0);
    module.start.output = start(PlcArea::Q, m.starts.1);
    let id = new_device(
        tx,
        m.designation.as_deref(),
        &m.name,
        Some(station),
        DeviceAspects {
            plc_module: Some(module),
            ..DeviceAspects::default()
        },
    )?;
    Ok((
        ExchangeObject::Module,
        id.cast(),
        Some(m.id.clone()),
        m.fields.clone(),
    ))
}

/// "Create unplaced" (doc 10 §4.4): the parked station with its modules, or the parked module under its matched
/// station; the snapshot entries of what was created, with their IDs in the file.
fn create_parked(
    tx: &mut TxBuilder<'_>,
    cfg: Id<PlcConfigProject>,
    p: &Parked,
) -> Result<Vec<Entry>, CmdError> {
    let mut out = Vec::new();
    let station = match (&p.object, p.station) {
        (ExchangeObject::Station, _) => {
            let id = new_device(
                tx,
                p.designation.as_deref(),
                &p.name,
                None,
                DeviceAspects {
                    plc_station: Some(PlcStation {
                        config_project: cfg,
                        role: StationRole::IoDevice,
                        order: None,
                        firmware: p.fields.get("firmware").and_then(|f| f.parse().ok()),
                        extra: Extra::new(),
                    }),
                    ..DeviceAspects::default()
                },
            )?;
            out.push((
                ExchangeObject::Station,
                id.cast(),
                Some(p.id.clone()),
                p.fields.clone(),
            ));
            id
        }
        (_, Some(station)) => station.cast(),
        _ => return Ok(out),
    };
    for m in &p.modules {
        out.push(new_module(tx, station, m)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------
// The instrument list

/// What an earlier list brought for a tag: its placeholder device and its channel, each while it is still there.
type Known = (Option<Id<Device>>, Option<Id<Channel>>);

/// `plc.apply_instrument_list` (`ApplyInstrumentList`, doc 10 §4.7, FR-PLC-050): takes the rows of an instrument
/// list into a configuration project.
///
/// Each row (tag, description, signal type, range, field location, preferred station) gets a placeholder field
/// device with the tag as its process tag and the description as its function text, unless a device carries that
/// process tag already; and the first spare channel of its kind (`DI`, `DO`, `AI`, `AO`): of the preferred station
/// when the row names one by its tag, then of every station of the configuration project in allocation order. The
/// channel is reserved for the tag: its state becomes `reserved`, and where they are empty its symbol becomes the
/// tag (cleaned into an IEC identifier under the strict policy), its comment the description and its signal range
/// the row's. The row is kept as an `exchange_record` of the configuration project (object `instrument`, the
/// placeholder device as its target, the channel among its values), so that EW-PRC-002 lists a tag without a channel
/// or a device as an open item. A row whose tag an earlier list brought keeps its device and channel. The
/// placeholder's letter is `B` for an input and `Q` for an output; the field location is kept in the record and not
/// assigned to the device. **Bulk:** the preview's notices name every row's channel, or that it has none. Rejected:
/// a configuration project that does not exist ([`CmdError::NotFound`]); no row (at `/rows`); a tag that is no
/// identifier text (at `/rows/<i>/tag`). Outcome: the configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyInstrumentList {
    /// The configuration project.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
    /// The rows.
    pub rows: Vec<InstrumentRow>,
}

impl Command for ApplyInstrumentList {
    const NAME: &'static str = "plc.apply_instrument_list";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let cfg = tx.require::<PlcConfigProject>(self.config_project)?;
        if self.rows.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/rows",
                text!("refusal-params-no-entry"),
            ));
        }
        let strict = cfg.symbols.as_ref().is_none_or(|s| s.strict_iec);
        // what earlier lists brought: per tag, the placeholder device and the channel that are still there
        let mut known: BTreeMap<String, Known> = BTreeMap::new();
        for r in tx.iter::<ExchangeRecord>() {
            if r.config_project != self.config_project || r.object != ExchangeObject::Instrument {
                continue;
            }
            let device: Id<Device> = r.target.cast();
            let channel = r
                .fields
                .get("channel")
                .and_then(|c| c.parse::<Id<Channel>>().ok())
                .filter(|c| tx.exists::<Channel>(*c));
            if let Some(tag) = r.fields.get("tag") {
                known.insert(
                    tag.clone(),
                    (tx.exists::<Device>(device).then_some(device), channel),
                );
            }
        }
        let taken: BTreeSet<Id<Channel>> = known.values().filter_map(|(_, c)| *c).collect();
        let open: Vec<InstrumentRow> = self
            .rows
            .iter()
            .filter(|r| known.get(&r.tag).is_none_or(|(_, c)| c.is_none()))
            .cloned()
            .collect();
        let base = tx.base_model().clone();
        let q = ew_derive::Q::new(&base);
        let mut assigned = ew_plc::instrument::assign(&q, self.config_project, &open, &taken).into_iter();

        let mut notices = Vec::new();
        let mut entries: Vec<Entry> = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            let tag: Ident = row.tag.parse().map_err(|e: ew_mltext::TextError| {
                CmdError::invalid_params(
                    Self::NAME,
                    format!("/rows/{i}/tag"),
                    text!("refusal-plc-instrument-tag", row.tag.clone(), e.to_string()),
                )
            })?;
            let (device, channel) = match known.get(&row.tag) {
                Some((device, Some(channel))) => (*device, Some(*channel)),
                Some((device, None)) => (*device, assigned.next().flatten()),
                None => (None, assigned.next().flatten()),
            };
            let device = match device.or_else(|| {
                tx.iter::<Device>()
                    .into_iter()
                    .find(|d| d.process_tag.as_ref() == Some(&tag))
                    .map(|d| d.id)
            }) {
                Some(d) => d,
                None => {
                    let letter = match row.signal {
                        Signal::Di | Signal::Ai => "B",
                        Signal::Do | Signal::Ao => "Q",
                    };
                    let id: Id<Device> = tx.mint();
                    tx.insert(Device {
                        id,
                        v: 1,
                        tag: TagSpec::new(
                            letter
                                .parse()
                                .map_err(|_| CmdError::internal("a placeholder's letter"))?,
                            None,
                        ),
                        kind: None,
                        parent: None,
                        position: None,
                        structure: Default::default(),
                        parts: Vec::new(),
                        function_text: if row.description.is_empty() {
                            MlText::default()
                        } else {
                            MlText::neutral(&row.description)
                        },
                        process_tag: Some(tag.clone()),
                        tech_data: Default::default(),
                        state: Default::default(),
                        source_ref: None,
                        aspects: DeviceAspects::default(),
                        extra: Extra::new(),
                    })?;
                    id
                }
            };
            if let Some(channel) = channel {
                let symbol = if strict {
                    ew_derive::iec_ident(&row.tag)
                } else {
                    row.tag.clone()
                };
                let symbol: Option<Ident> = symbol.parse().ok();
                let range: Option<Ident> = row.range.parse().ok();
                let description = row.description.clone();
                let before = tx.require::<Channel>(channel)?;
                let mut c = (*before).clone();
                c.state = Some(ChannelState::Reserved);
                if c.symbol.is_none() {
                    c.symbol = symbol;
                }
                if c.comment.is_empty() && !description.is_empty() {
                    c.comment = MlText::neutral(&description);
                }
                if c.signal_range.is_none() {
                    c.signal_range = range;
                }
                // a row an earlier list brought has its channel as it is: nothing is written
                if c != *before {
                    tx.put(c);
                }
                let module = tx.get::<Channel>(channel).map(|c| (c.module, c.number));
                notices.push(match module {
                    Some((m, n)) => format!(
                        "{}: channel {n} of {} reserved",
                        row.tag,
                        crate::words::device(tx, m)
                    ),
                    None => format!("{}: a channel reserved", row.tag),
                });
            } else {
                notices.push(format!(
                    "{}: no spare {} channel; the tag is an open item (EW-PRC-002)",
                    row.tag,
                    row.signal.word()
                ));
            }
            let mut fields = BTreeMap::from([
                ("tag".to_owned(), row.tag.clone()),
                ("signal".to_owned(), row.signal.word().to_owned()),
            ]);
            for (name, value) in [
                ("description", &row.description),
                ("range", &row.range),
                ("location", &row.location),
                ("station", &row.station),
            ] {
                if !value.is_empty() {
                    fields.insert(name.to_owned(), value.clone());
                }
            }
            if let Some(channel) = channel {
                fields.insert("channel".to_owned(), channel.to_string());
            }
            entries.push((ExchangeObject::Instrument, device.cast(), None, fields));
        }
        write_snapshot(tx, self.config_project, entries, &[], false)?;
        let mut outcome = Outcome::primary((Kind::PlcConfigProject, self.config_project.cast()));
        outcome.notices = notices;
        Ok(outcome)
    }
}
