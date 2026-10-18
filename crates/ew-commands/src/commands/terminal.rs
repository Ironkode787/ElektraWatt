// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Terminal and cable commands (doc 03 §11–§12): `terminal.add`, `cable.create`, `conductor.add`.

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{
    Cable, ColourSpec, Conductor, Csa, Device, DeviceAspects, DeviceKind, Extra, FnDef, FnRole, Function,
    FunctionKind, Kind, LevelSpec, LibCopy, Part, PartUse, StructureAssignment, TagSpec, Terminal,
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
        let mut designations: Vec<Ident> = Vec::new();
        let mut seqs: Vec<u32> = Vec::new();
        for id in tx.terminals_of_strip(self.strip) {
            if let Some(t) = tx.get::<Device>(id).and_then(|d| d.aspects.terminal.clone()) {
                designations.push(t.designation);
                seqs.push(t.seq);
            }
        }
        let mut next = seqs.iter().copied().max().unwrap_or(0).saturating_add(1);
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
            if designations.contains(&spec.designation) {
                return tx.reject(format!(
                    "designation {} is already used in the strip; choose another",
                    spec.designation.as_str()
                ));
            }
            if seqs.contains(&seq) {
                return tx.reject(format!(
                    "sequence number {seq} is already used in the strip; choose another"
                ));
            }
            designations.push(spec.designation.clone());
            seqs.push(seq);
            let device: Id<Device> = tx.mint();
            tx.insert(Device {
                id: device,
                v: 1,
                tag: TagSpec::new(strip.tag.letter.clone(), Some(seq)),
                kind: Some(DeviceKind::Terminal),
                parent: None,
                position: None,
                structure: strip.structure.clone(),
                parts: Vec::new(),
                function_text: spec.function_text.clone(),
                process_tag: None,
                tech_data: Default::default(),
                state: Default::default(),
                source_ref: None,
                aspects: DeviceAspects {
                    terminal: Some(Terminal {
                        strip: self.strip,
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
                def: self.fndef,
                role: FnRole::Main,
                kind: FunctionKind::Normal,
                cp_overrides: Default::default(),
                source_ref: None,
                extra: Extra::new(),
            })?;
            first.get_or_insert(device);
        }
        Ok(Outcome {
            primary: first.map(|d| (Kind::Device, d.cast())),
            note: None,
        })
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
        })
    }
}

/// Rejects a core that is used twice in `specs` or already in `existing`.
fn check_distinct_cores(
    tx: &TxBuilder<'_>,
    specs: &[ConductorSpec],
    existing: &[Ident],
) -> Result<(), CmdError> {
    let mut seen: Vec<&Ident> = existing.iter().collect();
    for spec in specs {
        if seen.contains(&&spec.core) {
            return tx.reject(format!(
                "core {} is already used in the cable; choose another",
                spec.core.as_str()
            ));
        }
        seen.push(&spec.core);
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
