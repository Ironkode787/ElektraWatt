// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Terminal and cable commands (doc 03 §11–§12): `terminal.add`, `terminal.designate`, `terminal.insert`,
//! `terminal.move`, `cable.create`, `conductor.add`.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{
    Cable, ColourSpec, Conductor, Connection, ConnectionKind, Csa, Device, DeviceAspects, DeviceKind, Extra,
    FnDef, FnRole, Function, FunctionKind, Kind, LevelSpec, LibCopy, Part, PartUse, StructureAssignment,
    TagSpec, Terminal,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{IdentS, MlTextS, UuidS};
use crate::tx::TxBuilder;

/// One terminal of `terminal.add`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TerminalSpec {
    /// The designation, unique within the strip.
    #[schemars(with = "IdentS")]
    pub designation: Ident,
    /// The drawn and mounting order; default one more than the highest in the strip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u32>,
    /// The function text ("1+").
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub function_text: MlText,
    /// Levels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<LevelSpec>,
    /// Bonded to the rail.
    #[serde(default)]
    pub rail_bonded: bool,
}

/// One assignment of [`DesignateTerminals`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TerminalDesignation {
    /// The terminal device.
    #[schemars(with = "UuidS")]
    pub terminal: Id<Device>,
    /// Its new designation (non-empty NFC plain string, never translated).
    #[schemars(with = "IdentS")]
    pub designation: Ident,
}

/// `terminal.designate`: sets the designations of one or more terminals in one transaction (doc 03 §11).
///
/// Rejected: no assignment; a terminal listed twice; a device that is not a terminal (no terminal
/// aspect). A missing device is `NotFound`; an assignment equal to the current designation changes
/// nothing. Uniqueness within the strip is not checked here: it is the prevent rule EW-TERM-002,
/// evaluated on the candidate at the end of the transaction, so a swap or a re-designation needs no
/// temporary names (doc 08 §6.8, FR-TRM-022). Cost Interactive, Outcome none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignateTerminals {
    /// The assignments, at least one; each terminal at most once.
    #[schemars(length(min = 1))]
    pub assignments: Vec<TerminalDesignation>,
}

impl Command for DesignateTerminals {
    const NAME: &'static str = "terminal.designate";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.assignments.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/assignments",
                "give at least one assignment",
            ));
        }
        let mut seen: BTreeSet<Id<Device>> = BTreeSet::new();
        for (i, a) in self.assignments.iter().enumerate() {
            if !seen.insert(a.terminal) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/assignments/{i}/terminal"),
                    "the terminal is listed twice",
                ));
            }
        }
        for (i, a) in self.assignments.iter().enumerate() {
            let device = tx.require::<Device>(a.terminal)?;
            let Some(mut terminal) = device.aspects.terminal.clone() else {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/assignments/{i}/terminal"),
                    "the device is not a terminal",
                ));
            };
            terminal.designation = a.designation.clone();
            tx.update::<Device>(a.terminal, |d| d.aspects.terminal = Some(terminal))?;
        }
        Ok(Outcome::default())
    }
}

/// `terminal.add`: terminals of a strip; a terminal is a device with the terminal aspect (doc 03 §11).
///
/// For each terminal in order: a device `{tag: {letter: the strip's letter, counter: seq}, kind:
/// terminal, structure: the strip's structure, function_text, terminal: {strip, seq, designation, levels,
/// rail_bonded}}` and its main function `{def: fndef}`. Rejected: the strip is not a device with the strip
/// aspect; a designation already used in the strip or twice in the list; a sequence number already used.
/// Outcome: the first new terminal device.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddTerminals {
    /// The strip device.
    #[schemars(with = "UuidS")]
    pub strip: Id<Device>,
    /// The terminal FnDef copy.
    #[schemars(with = "UuidS")]
    pub fndef: Id<FnDef>,
    /// The terminals, at least one.
    pub terminals: Vec<TerminalSpec>,
}

impl Command for AddTerminals {
    const NAME: &'static str = "terminal.add";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let strip = tx.require::<Device>(self.strip)?;
        if strip.aspects.strip.is_none() {
            return tx.reject("the device is not a terminal strip; pick a device with the strip aspect");
        }
        tx.require::<LibCopy<FnDef>>(self.fndef)?;
        if self.terminals.is_empty() {
            return tx.reject("give at least one terminal to add");
        }
        let (mut designations, mut seqs) = strip_usage(tx, self.strip);
        let mut next = seqs.last().copied().unwrap_or(0).saturating_add(1);
        let mut first = None;
        for spec in &self.terminals {
            let seq = match spec.seq {
                Some(seq) => seq,
                None => {
                    let seq = next;
                    next = next.saturating_add(1);
                    seq
                }
            };
            if !designations.insert(spec.designation.clone()) {
                return tx.reject(format!(
                    "designation {} is already used in the strip; choose another",
                    spec.designation.as_str()
                ));
            }
            if !seqs.insert(seq) {
                return tx.reject(format!(
                    "sequence number {seq} is already used in the strip; choose another"
                ));
            }
            let (device, _) = insert_terminal(tx, self.strip, &strip, self.fndef, spec, seq, Vec::new())?;
            first.get_or_insert(device);
        }
        Ok(Outcome {
            primary: first.map(|d| (Kind::Device, d.cast())),
            note: None,
            notices: Vec::new(),
        })
    }
}

/// The designations and sequence numbers the terminals of `strip` use.
pub(crate) fn strip_usage(tx: &TxBuilder<'_>, strip: Id<Device>) -> (BTreeSet<Ident>, BTreeSet<u32>) {
    let mut designations: BTreeSet<Ident> = BTreeSet::new();
    let mut seqs: BTreeSet<u32> = BTreeSet::new();
    for id in tx.terminals_of_strip(strip) {
        if let Some(t) = tx.get::<Device>(id).and_then(|d| d.aspects.terminal.clone()) {
            designations.insert(t.designation);
            seqs.insert(t.seq);
        }
    }
    (designations, seqs)
}

/// Inserts one terminal of `strip` as `terminal.add` creates it: a device `{tag: {letter: the strip's letter,
/// counter: seq}, kind: terminal, structure: the strip's structure, function_text, terminal: {strip, seq,
/// designation, levels, rail_bonded}, parts}` and its main function `{def: fndef}` (`parts` is empty for
/// `terminal.add`; a part-first `new_terminal` passes its part use). The caller checks the designation, the sequence
/// number and the parts. Returns the device and the function.
pub(crate) fn insert_terminal(
    tx: &mut TxBuilder<'_>,
    strip_id: Id<Device>,
    strip: &Device,
    fndef: Id<FnDef>,
    spec: &TerminalSpec,
    seq: u32,
    parts: Vec<PartUse>,
) -> Result<(Id<Device>, Id<Function>), CmdError> {
    let device: Id<Device> = tx.mint();
    tx.insert(Device {
        id: device,
        v: 1,
        tag: TagSpec::new(strip.tag.letter.clone(), Some(seq)),
        kind: Some(DeviceKind::Terminal),
        parent: None,
        position: None,
        structure: strip.structure.clone(),
        parts,
        function_text: spec.function_text.clone(),
        process_tag: None,
        tech_data: Default::default(),
        state: Default::default(),
        source_ref: None,
        aspects: DeviceAspects {
            terminal: Some(Terminal {
                strip: strip_id,
                seq,
                designation: spec.designation.clone(),
                levels: spec.levels.clone(),
                rail_bonded: spec.rail_bonded,
                extra: Extra::new(),
            }),
            ..DeviceAspects::default()
        },
        extra: Extra::new(),
    })?;
    let function: Id<Function> = tx.mint();
    tx.insert(Function {
        id: function,
        v: 1,
        device: Some(device),
        def: fndef,
        role: FnRole::Main,
        kind: FunctionKind::Normal,
        cp_overrides: Default::default(),
        source_ref: None,
        template_entry: None,
        bus_port: None,
        extra: Extra::new(),
    })?;
    Ok((device, function))
}

/// The bridge groups of a strip (doc 08 §2.3): per `BridgeSpec.group`, the terminals of the strip its bridge
/// connections end at. A bridge belongs to a terminal by its connection, so the group follows its terminals through
/// any reorder.
fn bridge_groups(tx: &TxBuilder<'_>, strip: Id<Device>) -> BTreeMap<u16, BTreeSet<Id<Device>>> {
    let mut groups: BTreeMap<u16, BTreeSet<Id<Device>>> = BTreeMap::new();
    let terminal_of = |f: Id<Function>| -> Option<Id<Device>> {
        let d = tx.get::<Function>(f)?.device?;
        let on_strip = tx
            .get::<Device>(d)?
            .aspects
            .terminal
            .as_ref()
            .is_some_and(|t| t.strip == strip);
        on_strip.then_some(d)
    };
    for t in tx.terminals_of_strip(strip) {
        for f in tx.functions_of_device(t) {
            for c in tx.connections_of_function(f) {
                let Some(conn) = tx.get::<Connection>(c) else {
                    continue;
                };
                let ConnectionKind::Bridge(spec) = &conn.kind else {
                    continue;
                };
                let members = groups.entry(spec.group).or_default();
                for end in [&conn.a, &conn.b] {
                    if let Some(d) = end.function().and_then(terminal_of) {
                        members.insert(d);
                    }
                }
            }
        }
    }
    groups
}

/// Whether the members of `group` sit at consecutive places of `order`.
fn consecutive<T: PartialEq>(order: &[T], group: impl Fn(&T) -> bool) -> bool {
    let at: Vec<usize> = order
        .iter()
        .enumerate()
        .filter(|(_, x)| group(x))
        .map(|(i, _)| i)
        .collect();
    at.windows(2).all(|w| w[1] == w[0] + 1)
}

/// The first bridge group (by number) whose terminals are consecutive in `before` but not in `after` (FT-641, CI-06):
/// its number and its terminals' designations in `before` order. A group that was not consecutive already (a lenient
/// import's) is EW-TERM-004's finding, not a refusal here.
fn split_bridge_group(
    tx: &TxBuilder<'_>,
    strip: Id<Device>,
    before: &[Id<Device>],
    after: &[Option<Id<Device>>],
) -> Option<(u16, Vec<String>)> {
    for (group, members) in bridge_groups(tx, strip) {
        let was = consecutive(before, |t| members.contains(t));
        let is = consecutive(after, |t| t.is_some_and(|t| members.contains(&t)));
        if was && !is {
            let names = before
                .iter()
                .filter(|t| members.contains(t))
                .map(|t| designation_of(tx, *t))
                .collect();
            return Some((group, names));
        }
    }
    None
}

/// A terminal's designation, `?` for a device that is not a terminal.
fn designation_of(tx: &TxBuilder<'_>, t: Id<Device>) -> String {
    tx.get::<Device>(t)
        .and_then(|d| {
            d.aspects
                .terminal
                .as_ref()
                .map(|x| x.designation.as_str().to_owned())
        })
        .unwrap_or_else(|| "?".to_owned())
}

/// `terminal.insert`: new terminals of a strip at a position, the following terminals moved up (WP-S2.07 §5.1, D4).
///
/// Let s be the `seq` of `before` (absent: one more than the strip's highest `seq`, or 1 for an empty strip) and n
/// the number of terminals. In one transaction every terminal of the strip with `seq ≥ s` gets `seq + n` (its tag
/// and designation stay), every group bound (`from_seq`, `to_seq`) and every accessory `after_seq` at or above s is
/// raised by n, and the new terminals are inserted as `terminal.add` inserts them, with `seq` s, s + 1, … in list
/// order and `parts` on each. Rejected: the strip is not a device with the strip aspect; `before` is not a terminal of
/// the strip; new terminals between two terminals of one consecutive bridge group (FT-641, CI-06: the message names
/// the group and the places outside it); no terminal; a spec with `seq`; a designation already used in the strip or twice in the list; a
/// sequence number beyond 4294967295. A missing strip, `before`, function definition or part copy is `NotFound`.
/// Cost Interactive. Outcome: the first new terminal device.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InsertTerminals {
    /// The strip device.
    #[schemars(with = "UuidS")]
    pub strip: Id<Device>,
    /// The terminal of the strip the new terminals go before; absent or null: after the last terminal.
    #[serde(default)]
    #[schemars(with = "Option<UuidS>")]
    pub before: Option<Id<Device>>,
    /// The terminal FnDef copy.
    #[schemars(with = "UuidS")]
    pub fndef: Id<FnDef>,
    /// The terminals, at least one, in mounting order; their position follows from `before`, so `seq` is not
    /// allowed.
    #[schemars(length(min = 1))]
    pub terminals: Vec<TerminalSpec>,
    /// The part uses each new terminal gets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PartUse>,
}

impl InsertTerminals {
    /// A rejection of the parameters at the JSON Pointer `path`.
    fn bad<T>(path: impl Into<String>, message: impl Into<String>) -> Result<T, CmdError> {
        Err(CmdError::invalid_params(Self::NAME, path, message))
    }
}

/// The message of a sequence number that would pass `u32::MAX`.
const SEQ_OVERFLOW: &str = "the sequence numbers would pass 4294967295; insert at a lower position";

impl Command for InsertTerminals {
    const NAME: &'static str = "terminal.insert";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let strip = tx.require::<Device>(self.strip)?;
        let Some(mut aspect) = strip.aspects.strip.clone() else {
            return Self::bad(
                "/strip",
                "the device is not a terminal strip; pick a device with the strip aspect",
            );
        };
        tx.require::<LibCopy<FnDef>>(self.fndef)?;
        for part in &self.parts {
            tx.require::<LibCopy<Part>>(part.part.copy)?;
        }
        let at = match self.before {
            Some(b) => {
                let device = tx.require::<Device>(b)?;
                match device.aspects.terminal.as_ref() {
                    Some(t) if t.strip == self.strip => Some(t.seq),
                    _ => return Self::bad("/before", "the device is not a terminal of the strip"),
                }
            }
            None => None,
        };
        if self.terminals.is_empty() {
            return Self::bad("/terminals", "give at least one terminal to insert");
        }
        for (i, spec) in self.terminals.iter().enumerate() {
            if spec.seq.is_some() {
                return Self::bad(
                    format!("/terminals/{i}/seq"),
                    "the position follows from `before`; leave out `seq`",
                );
            }
        }
        let (existing, seqs) = strip_usage(tx, self.strip);
        let mut listed: BTreeSet<&Ident> = BTreeSet::new();
        for (i, spec) in self.terminals.iter().enumerate() {
            if existing.contains(&spec.designation) {
                return Self::bad(
                    format!("/terminals/{i}/designation"),
                    format!(
                        "designation {} is already used in the strip; choose another",
                        spec.designation.as_str()
                    ),
                );
            }
            if !listed.insert(&spec.designation) {
                return Self::bad(
                    format!("/terminals/{i}/designation"),
                    format!(
                        "designation {} is given twice; choose another",
                        spec.designation.as_str()
                    ),
                );
            }
        }
        // FT-641 (CI-06): new terminals inside a bridge group would split it.
        if let Some(b) = self.before {
            let mut order: Vec<(u32, Id<Device>)> = tx
                .terminals_of_strip(self.strip)
                .into_iter()
                .filter_map(|id| {
                    tx.get::<Device>(id)
                        .and_then(|d| d.aspects.terminal.as_ref().map(|t| (t.seq, id)))
                })
                .collect();
            order.sort();
            let current: Vec<Id<Device>> = order.into_iter().map(|(_, id)| id).collect();
            let mut after: Vec<Option<Id<Device>>> = current.iter().copied().map(Some).collect();
            if let Some(k) = current.iter().position(|t| *t == b) {
                after.insert(k, None);
            }
            if let Some((group, names)) = split_bridge_group(tx, self.strip, &current, &after) {
                let (first, last) = (names.first().cloned(), names.last().cloned());
                return tx.reject(format!(
                    "inserting before terminal {} would split bridge group {group} (terminals {}); insert before \
                     terminal {} or after terminal {}, or delete the group's bridge there first",
                    designation_of(tx, b),
                    names.join(", "),
                    first.unwrap_or_default(),
                    last.unwrap_or_default(),
                ));
            }
        }
        let s = match at {
            Some(seq) => seq,
            None => match seqs.last() {
                Some(max) => match max.checked_add(1) {
                    Some(s) => s,
                    None => return Self::bad("/terminals", SEQ_OVERFLOW),
                },
                None => 1,
            },
        };
        let Ok(n) = u32::try_from(self.terminals.len()) else {
            return Self::bad("/terminals", SEQ_OVERFLOW);
        };
        let shift = |v: u32| -> Result<u32, CmdError> {
            if v >= s {
                v.checked_add(n)
                    .map_or_else(|| Self::bad("/terminals", SEQ_OVERFLOW), Ok)
            } else {
                Ok(v)
            }
        };
        // The new terminals take s … s + n − 1.
        if s.checked_add(n - 1).is_none() {
            return Self::bad("/terminals", SEQ_OVERFLOW);
        }
        // The strip's terminals at or after s move up by n (only the sequence number changes).
        let mut moved: Vec<(Id<Device>, u32)> = Vec::new();
        let mut terminals = tx.terminals_of_strip(self.strip);
        terminals.sort();
        for id in terminals {
            if let Some(t) = tx.get::<Device>(id).and_then(|d| d.aspects.terminal.clone()) {
                let new = shift(t.seq)?;
                if new != t.seq {
                    moved.push((id, new));
                }
            }
        }
        // The groups and accessories at or after s move with them.
        for g in &mut aspect.groups {
            g.from_seq = shift(g.from_seq)?;
            g.to_seq = shift(g.to_seq)?;
        }
        for a in &mut aspect.accessories {
            a.after_seq = shift(a.after_seq)?;
        }
        for (id, seq) in moved {
            tx.update::<Device>(id, |d| {
                if let Some(t) = d.aspects.terminal.as_mut() {
                    t.seq = seq;
                }
            })?;
        }
        tx.update::<Device>(self.strip, |d| d.aspects.strip = Some(aspect))?;
        let mut first = None;
        for (k, spec) in (0..n).zip(&self.terminals) {
            let (device, _) = insert_terminal(
                tx,
                self.strip,
                &strip,
                self.fndef,
                spec,
                s + k,
                self.parts.clone(),
            )?;
            first.get_or_insert(device);
        }
        Ok(Outcome {
            primary: first.map(|d| (Kind::Device, d.cast())),
            note: None,
            notices: Vec::new(),
        })
    }
}

/// `terminal.move`: terminals of one strip put before another terminal of the strip, or at its end (WP-S2.07 §5.2,
/// D4).
///
/// Let T be the strip's terminals in (`seq`, ID) order. The new order O is T without the moved terminals, with them
/// (in their T order) inserted directly before `before` (absent: at the end). For every index i from the first to the
/// last where T and O differ, the terminal at index i of O takes the old `seq` of the one at index i of T: the
/// sequence values of the changed range are permuted (the rule of `page.reorder`). Groups, accessories and
/// designations stay, so a moved terminal takes the group of its new place. Bridges belong to their terminals (they
/// are connections), so a bridge group follows its terminals: a move that keeps every bridge group consecutive is
/// accepted, one that splits a group that was consecutive is refused with a message naming the group and the way out
/// (FT-641, CI-06). Rejected: no terminal; a terminal listed
/// twice; a device that is not a terminal; terminals of two strips; a `before` that is one of the moved terminals or
/// not a terminal of their strip; an order that does not change. A missing device is `NotFound`. Cost Interactive, Outcome none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveTerminals {
    /// The terminal devices to move, at least one, all of one strip, in any order (they are taken in strip order).
    #[schemars(with = "Vec<UuidS>", length(min = 1))]
    pub terminals: Vec<Id<Device>>,
    /// The terminal of the same strip they are put before; absent or null: after the last terminal of the strip.
    #[serde(default)]
    #[schemars(with = "Option<UuidS>")]
    pub before: Option<Id<Device>>,
}

impl MoveTerminals {
    /// A rejection of the parameters at the JSON Pointer `path`.
    fn bad<T>(path: impl Into<String>, message: impl Into<String>) -> Result<T, CmdError> {
        Err(CmdError::invalid_params(Self::NAME, path, message))
    }
}

impl Command for MoveTerminals {
    const NAME: &'static str = "terminal.move";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.terminals.is_empty() {
            return Self::bad("/terminals", "name at least one terminal");
        }
        let mut seen: BTreeSet<Id<Device>> = BTreeSet::new();
        for (i, id) in self.terminals.iter().enumerate() {
            if !seen.insert(*id) {
                return Self::bad(
                    format!("/terminals/{i}"),
                    format!("terminal {} is listed twice", crate::words::device(tx, *id)),
                );
            }
        }
        let mut strip: Option<Id<Device>> = None;
        for (i, id) in self.terminals.iter().enumerate() {
            let device = tx.require::<Device>(*id)?;
            let Some(t) = device.aspects.terminal.as_ref() else {
                return Self::bad(format!("/terminals/{i}"), "the device is not a terminal");
            };
            match strip {
                None => strip = Some(t.strip),
                Some(s) if s != t.strip => {
                    return Self::bad(
                        format!("/terminals/{i}"),
                        "the terminal is of another strip; move the terminals of one strip at a time",
                    );
                }
                Some(_) => {}
            }
        }
        let Some(strip) = strip else {
            return Err(CmdError::internal("a named terminal has no strip"));
        };
        if let Some(b) = self.before {
            if seen.contains(&b) {
                return Self::bad(
                    "/before",
                    "the terminal to put the terminals before is one of the moved terminals; name a terminal that stays",
                );
            }
            let device = tx.require::<Device>(b)?;
            if device.aspects.terminal.as_ref().is_none_or(|t| t.strip != strip) {
                return Self::bad("/before", "the device is not a terminal of the same strip");
            }
        }
        // T: the strip's terminals in (seq, ID) order.
        let mut current: Vec<(u32, Id<Device>)> = tx
            .terminals_of_strip(strip)
            .into_iter()
            .filter_map(|id| {
                tx.get::<Device>(id)
                    .and_then(|d| d.aspects.terminal.as_ref().map(|t| (t.seq, id)))
            })
            .collect();
        current.sort();
        // O: T without the moved terminals, which go in as one block (in T order) before `before`, or at the end.
        let moved: Vec<Id<Device>> = current
            .iter()
            .map(|(_, id)| *id)
            .filter(|id| seen.contains(id))
            .collect();
        let mut order: Vec<Id<Device>> = current
            .iter()
            .map(|(_, id)| *id)
            .filter(|id| !seen.contains(id))
            .collect();
        let at = match self.before {
            Some(b) => order.iter().position(|id| *id == b).ok_or_else(|| {
                CmdError::internal("the terminal to put the terminals before is not in its strip")
            })?,
            None => order.len(),
        };
        order.splice(at..at, moved);
        let changed: Vec<usize> = (0..order.len()).filter(|&k| order[k] != current[k].1).collect();
        let (Some(&first), Some(&last)) = (changed.first(), changed.last()) else {
            return Self::bad("/terminals", "the terminals are already there");
        };
        // FT-641 (CI-06): a bridge group follows its terminals, so a move that keeps every group consecutive is
        // allowed; one that splits a group is refused, naming the group and the way out.
        let before_ids: Vec<Id<Device>> = current.iter().map(|(_, id)| *id).collect();
        let after_ids: Vec<Option<Id<Device>>> = order.iter().copied().map(Some).collect();
        if let Some((group, names)) = split_bridge_group(tx, strip, &before_ids, &after_ids) {
            let moved_names: Vec<String> = before_ids
                .iter()
                .filter(|t| seen.contains(t))
                .map(|t| designation_of(tx, *t))
                .filter(|n| names.contains(n))
                .collect();
            let remedy = if moved_names.is_empty() {
                format!(
                    "put the terminals before terminal {} or after terminal {}, or delete the group's bridge there \
                     first",
                    names.first().cloned().unwrap_or_default(),
                    names.last().cloned().unwrap_or_default()
                )
            } else {
                format!(
                    "remove terminal {} from the group first (delete its bridges), or move the whole group",
                    moved_names.join(", ")
                )
            };
            return tx.reject(format!(
                "the move would split bridge group {group} (terminals {}); {remedy}",
                names.join(", ")
            ));
        }
        // The terminal that comes to position k takes the sequence number of the terminal that held k.
        for k in first..=last {
            if order[k] == current[k].1 {
                continue;
            }
            let seq = current[k].0;
            tx.update::<Device>(order[k], |d| {
                if let Some(t) = d.aspects.terminal.as_mut() {
                    t.seq = seq;
                }
            })?;
        }
        Ok(Outcome::default())
    }
}

/// One conductor of `cable.create` and `conductor.add`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConductorSpec {
    /// Core identification ("1", "BK", "A").
    #[schemars(with = "IdentS")]
    pub core: Ident,
    /// Core colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// Core cross-section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csa: Option<Csa>,
    /// A shield rather than a core.
    #[serde(default)]
    pub shield: bool,
    /// Twisted pair number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<u8>,
}

fn w() -> Ident {
    Ident::from_input("W").unwrap_or_else(|_| unreachable!("`W` is an identifier"))
}

/// `cable.create`: a cable device with its conductors (doc 03 §12).
///
/// Creates `Device {tag: {letter, counter}, parts, function_text, structure, cable}` (no function) and one
/// `Conductor {cable, core, colour, csa, shield, pair}` per spec, in order. Rejected: two specs with the
/// same core. Outcome: the cable device.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCable {
    /// The letter code; default "W".
    #[serde(default = "w")]
    #[schemars(with = "IdentS")]
    pub letter: Ident,
    /// An explicit counter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<u32>,
    /// Part uses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PartUse>,
    /// Manual function text.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub function_text: MlText,
    /// Own structure assignment.
    #[serde(default, skip_serializing_if = "StructureAssignment::is_empty")]
    pub structure: StructureAssignment,
    /// The cable aspect.
    #[serde(default)]
    pub cable: Cable,
    /// The conductors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conductors: Vec<ConductorSpec>,
}

impl Command for CreateCable {
    const NAME: &'static str = "cable.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        for part in &self.parts {
            tx.require::<LibCopy<Part>>(part.part.copy)?;
        }
        check_distinct_cores(tx, &self.conductors, &[])?;
        let id: Id<Device> = tx.mint();
        tx.insert(Device {
            id,
            v: 1,
            tag: TagSpec::new(self.letter.clone(), self.counter),
            kind: None,
            parent: None,
            position: None,
            structure: self.structure.clone(),
            parts: self.parts.clone(),
            function_text: self.function_text.clone(),
            process_tag: None,
            tech_data: Default::default(),
            state: Default::default(),
            source_ref: None,
            aspects: DeviceAspects {
                cable: Some(self.cable.clone()),
                ..DeviceAspects::default()
            },
            extra: Extra::new(),
        })?;
        insert_conductors(tx, id, &self.conductors)?;
        Ok(Outcome::primary((Kind::Device, id.cast())))
    }
}

/// `conductor.add`: conductors of an existing cable. Rejected: the device has no cable aspect; a core
/// already used in the cable or twice in the list. Outcome: the first new conductor.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddConductors {
    /// The cable device.
    #[schemars(with = "UuidS")]
    pub cable: Id<Device>,
    /// The conductors, at least one.
    pub conductors: Vec<ConductorSpec>,
}

impl Command for AddConductors {
    const NAME: &'static str = "conductor.add";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let device = tx.require::<Device>(self.cable)?;
        if device.aspects.cable.is_none() {
            return tx.reject("the device is not a cable; pick a device with the cable aspect");
        }
        if self.conductors.is_empty() {
            return tx.reject("give at least one conductor to add");
        }
        let existing: Vec<Ident> = tx
            .conductors_of_cable(self.cable)
            .into_iter()
            .filter_map(|c| tx.get::<Conductor>(c).map(|c| c.core.clone()))
            .collect();
        check_distinct_cores(tx, &self.conductors, &existing)?;
        let first = insert_conductors(tx, self.cable, &self.conductors)?;
        Ok(Outcome {
            primary: first.map(|c| (Kind::Conductor, c.cast())),
            note: None,
            notices: Vec::new(),
        })
    }
}

/// Rejects a core that is used twice in `specs` or already in `existing`.
fn check_distinct_cores(
    tx: &TxBuilder<'_>,
    specs: &[ConductorSpec],
    existing: &[Ident],
) -> Result<(), CmdError> {
    let mut seen: BTreeSet<&Ident> = existing.iter().collect();
    for spec in specs {
        if !seen.insert(&spec.core) {
            return tx.reject(format!(
                "core {} is already used in the cable; choose another",
                spec.core.as_str()
            ));
        }
    }
    Ok(())
}

/// Inserts one conductor of `cable` per spec, in order; returns the first new one.
fn insert_conductors(
    tx: &mut TxBuilder<'_>,
    cable: Id<Device>,
    specs: &[ConductorSpec],
) -> Result<Option<Id<Conductor>>, CmdError> {
    let mut first = None;
    for spec in specs {
        let id: Id<Conductor> = tx.mint();
        tx.insert(Conductor {
            id,
            v: 1,
            cable,
            core: spec.core.clone(),
            colour: spec.colour.clone(),
            csa: spec.csa.clone(),
            shield: spec.shield,
            pair: spec.pair,
            source_ref: None,
            extra: Extra::new(),
        })?;
        first.get_or_insert(id);
    }
    Ok(first)
}
