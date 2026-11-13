// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid commands (doc 11 §4.3): `valve.set_occupancy` (`SetValveOccupancy`), `fluid.port.close`
//! (`ClosePort`), `fluid.medium.edit` (`EditMedium`), `fluid.medium.define` (`DefineMedium`), `fluid.tube.set`
//! (`SetTube`) and `fluid.length.set` (`SetLength`).

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_model::FluidPortClass;
use ew_schema::{
    Channel, ClosureKind, ColourSpec, Connection, ConnectionKind, ContactKind, CpOverride, Device,
    DeviceAspects, DeviceKind, DeviceState, Discipline, Endpoint, Extra, FnDef, FnRole, Function,
    FunctionKind, Kind, LengthSource, LibCopy, LineKindTube, Medium, MediumDef, Part, PartRef, PartUse,
    PortClosure, TagScheme, TagSpec, TempRange, TubeSpec,
};
use ew_units::{Dec3, Nm};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::commands::connection::cp_fluid_class;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{Dec3S, IdentS, MlTextS, NmS, UuidS};
use crate::text::{Text, TextArg, text};
use crate::tx::TxBuilder;

/// The state of a valve set by [`SetValveOccupancy`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValveStateParam {
    /// In use.
    Used,
    /// Spare.
    Spare,
    /// Reserved.
    Reserved,
    /// Not fitted.
    NotFitted,
}

impl ValveStateParam {
    fn state(self) -> DeviceState {
        match self {
            ValveStateParam::Used => DeviceState::Used,
            ValveStateParam::Spare => DeviceState::Spare,
            ValveStateParam::Reserved => DeviceState::Reserved,
            ValveStateParam::NotFitted => DeviceState::NotFitted,
        }
    }
}

/// What a valve position holds after [`SetValveOccupancy`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum OccupancyParam {
    /// A valve in a state; `part` is required when the position holds no valve.
    Valve {
        /// The valve's state.
        state: ValveStateParam,
        /// The valve part's copy ID.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(with = "Option<UuidS>")]
        part: Option<Id<Part>>,
    },
    /// A blanking plate, with its part when given.
    Blank {
        /// The plate's part copy ID.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(with = "Option<UuidS>")]
        part: Option<Id<Part>>,
    },
    /// Nothing.
    Empty,
}

/// `valve.set_occupancy` (`SetValveOccupancy`, doc 11 §4.3): what one valve position of a module holds.
///
/// A module is a device whose part (`plc_module.part_use`, else the first part use with `plc.positions`) has a
/// solenoid-position map; its positions are 1 … `plc.positions.count`. `copies` embeds library documents first, as
/// `part.assign` does.
/// - *valve:* a valve at the position takes the state (a `part` other than its own is refused); a blanking plate there
///   is removed, and on a position without a valve a valve is made as `valve.add` makes it (`part` required): the
///   child `{tag: child scheme, letter Q, counter = position, parts: [part]}` in the state, whose functions come from
///   the part (N-FL-1).
/// - *blank:* a valve at the position is removed with its delete plan (functions, representations, the connections
///   at its CPs, its `drives` links; channels bound to its coils are unbound), with notices; a plate there takes the
///   given part, the old part's template functions replaced by the new part's; otherwise a child `{kind: blanking_plate, tag: child scheme, letter Q, counter = position, parts:
///   [part]?}` is made, with the part's template functions when a part is given.
/// - *empty:* the child at the position is removed as above.
///
/// Refused: a device without valve positions; a position outside them. Interactive, one undo step. Outcome: the
/// child at the position, else the module.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetValveOccupancy {
    /// The module (sub-base).
    #[schemars(with = "UuidS")]
    pub module: Id<Device>,
    /// The position, 1-based.
    pub position: u32,
    /// What the position holds.
    pub occupancy: OccupancyParam,
    /// The library documents to embed (`lib/` JSON form); may be empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

/// The number of valve positions of a module, as `ew_derive::valve_positions` reads it: the `fluid.positions` of
/// the first part use whose copy has it, else `plc.positions.count` of the part named by `plc_module.part_use`, else
/// of the first part use whose copy has `plc.positions`; `None` for a device without.
pub(crate) fn position_count(tx: &TxBuilder<'_>, module: &Device) -> Option<u16> {
    if let Some(n) = module.parts.iter().find_map(|u| {
        tx.get::<LibCopy<Part>>(u.part.copy)?
            .item
            .sections
            .fluid
            .as_ref()?
            .positions
    }) {
        return Some(n);
    }
    let count = |i: usize| {
        let copy = tx.get::<LibCopy<Part>>(module.parts.get(i)?.part.copy)?;
        let positions = copy.item.sections.plc.as_ref()?.positions.as_ref()?;
        Some(positions.count)
    };
    match module.aspects.plc_module.as_ref().and_then(|m| m.part_use) {
        Some(i) => count(usize::from(i)),
        None => (0..module.parts.len()).find_map(count),
    }
}

/// The child of `module` at `position`: of two, the lower ID.
fn child_at(tx: &TxBuilder<'_>, module: Id<Device>, position: u32) -> Option<Arc<Device>> {
    tx.children_of(module)
        .into_iter()
        .filter_map(|c| tx.get::<Device>(c))
        .filter(|d| d.position == Some(position))
        .min_by_key(|d| d.id)
}

/// Removes a child of a module with its delete plan, with its notices.
fn remove_child(
    tx: &mut TxBuilder<'_>,
    child: &Device,
    position: u32,
    module: Id<Device>,
) -> Result<(), CmdError> {
    let name = crate::words::device(tx, child.id);
    let module_name = crate::words::device(tx, module);
    let functions = tx.functions_of_device(child.id);
    let mut lines: Vec<Id<Connection>> = functions
        .iter()
        .flat_map(|f| tx.connections_of_function(*f))
        .filter(|c| {
            tx.get::<Connection>(*c)
                .is_some_and(|c| matches!(c.kind, ConnectionKind::Fluid(_)))
        })
        .collect();
    lines.sort();
    lines.dedup();
    let mut bound: Vec<Id<Channel>> = Vec::new();
    for f in &functions {
        for ch in tx.iter::<Channel>() {
            if matches!(ch.binding, Some(Endpoint::Cp(bf, _)) if bf == *f) {
                bound.push(ch.id);
            }
        }
    }
    bound.sort();
    bound.dedup();
    let unbound: Vec<String> = bound
        .iter()
        .map(|c| crate::words::channel(tx, *c).to_string())
        .collect();
    tx.delete((Kind::Device, child.id.cast()))?;
    tx.notice(match lines.len() {
        0 => format!("removed {name} from position {position} of {module_name}"),
        k => format!("removed {name} from position {position} of {module_name} with {k} fluid lines"),
    });
    for c in unbound {
        tx.notice(format!("{c} is unbound: its coil went with {name}"));
    }
    Ok(())
}

/// Whether a child of a module is a blanking plate; every other child at a position is taken as its valve.
fn is_plate(d: &Device) -> bool {
    d.kind == Some(DeviceKind::BlankingPlate)
}

fn part_use(part: Id<Part>) -> PartUse {
    PartUse {
        part: PartRef {
            copy: part,
            variant: None,
            extra: Extra::new(),
        },
        qty: Dec3::ONE,
        extra: Extra::new(),
    }
}

/// A child of a module at a position, in the tag scheme of `valve.add`.
fn new_child(
    tx: &mut TxBuilder<'_>,
    module: Id<Device>,
    position: u32,
    kind: Option<DeviceKind>,
    parts: Vec<PartUse>,
    state: DeviceState,
) -> Result<Id<Device>, CmdError> {
    let id: Id<Device> = tx.mint();
    tx.insert(Device {
        id,
        v: 1,
        tag: TagSpec {
            scheme: TagScheme::Child,
            ..TagSpec::new("Q".parse().expect("`Q` is an identifier"), Some(position))
        },
        kind,
        parent: Some(module),
        position: Some(position),
        structure: Default::default(),
        parts,
        function_text: MlText::default(),
        process_tag: None,
        tech_data: Default::default(),
        state,
        source_ref: None,
        aspects: DeviceAspects::default(),
        extra: Extra::new(),
    })?;
    Ok(id)
}

/// The template functions of a blanking plate's part (ID `v5(device, template_index)`, ADR-0004 amendment 2).
fn plate_functions(tx: &mut TxBuilder<'_>, plate: Id<Device>, part: Id<Part>) -> Result<(), CmdError> {
    let copy = tx.require::<LibCopy<Part>>(part)?;
    for (i, e) in copy.item.function_template.iter().enumerate() {
        let def: Id<FnDef> = ew_ids::v5::lib_copy_id(e.fndef.library.as_uuid(), e.fndef.item.as_uuid());
        let Some(fndef) = tx.get::<LibCopy<FnDef>>(def) else {
            return tx.reject(text!(
                "refusal-part-entry-fndef-missing",
                i.to_string(),
                copy.item.part_number.as_str()
            ));
        };
        let kind = if fndef.item.contact_kind == ContactKind::BlackBox {
            FunctionKind::BlackBox
        } else {
            FunctionKind::Normal
        };
        let index = u32::try_from(i).unwrap_or(u32::MAX);
        tx.insert(Function {
            id: ew_ids::v5::template_function_id(plate, index),
            v: 1,
            device: Some(plate),
            def,
            role: if e.main { FnRole::Main } else { FnRole::Aux },
            kind,
            cp_overrides: Default::default(),
            source_ref: None,
            template_entry: u16::try_from(i).ok(),
            bus_port: None,
            extra: Extra::new(),
        })?;
    }
    Ok(())
}

impl Command for SetValveOccupancy {
    const NAME: &'static str = "valve.set_occupancy";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        embed_closure(Self::NAME, tx, &self.copies)?;
        let module = tx.require::<Device>(self.module)?;
        let module_name = crate::words::device(tx, self.module);
        let Some(n) = position_count(tx, &module) else {
            return tx.reject(text!("refusal-valve-no-positions", module_name));
        };
        let p = self.position;
        if p == 0 || p > u32::from(n) {
            return tx.reject(text!(
                "refusal-valve-no-position",
                module_name,
                n.to_string(),
                p.to_string()
            ));
        }
        let held = child_at(tx, self.module, p);
        let child = match &self.occupancy {
            OccupancyParam::Valve { state, part } => {
                let valve = match held {
                    Some(h) if !is_plate(&h) => Some(h),
                    Some(plate) => {
                        remove_child(tx, &plate, p, self.module)?;
                        None
                    }
                    None => None,
                };
                match valve {
                    Some(v) => {
                        let own = v.parts.first().map(|u| u.part.copy);
                        if let Some(part) = part.filter(|part| Some(*part) != own) {
                            tx.require::<LibCopy<Part>>(part)?;
                            let name = crate::words::device(tx, v.id);
                            return tx.reject(text!(
                                "refusal-valve-position-holds",
                                p.to_string(),
                                module_name,
                                name
                            ));
                        }
                        let state = state.state();
                        tx.update::<Device>(v.id, |d| d.state = state)?;
                        Some(v.id)
                    }
                    None => {
                        let Some(part) = part else {
                            return tx.reject(text!(
                                "refusal-valve-position-empty",
                                p.to_string(),
                                module_name
                            ));
                        };
                        tx.require::<LibCopy<Part>>(*part)?;
                        Some(new_child(
                            tx,
                            self.module,
                            p,
                            None,
                            vec![part_use(*part)],
                            state.state(),
                        )?)
                    }
                }
            }
            OccupancyParam::Blank { part } => {
                if let Some(part) = part {
                    tx.require::<LibCopy<Part>>(*part)?;
                }
                match held {
                    Some(plate) if is_plate(&plate) => {
                        if let Some(part) = part
                            && plate.parts.first().map(|u| u.part.copy) != Some(*part)
                        {
                            // The template functions follow the part, the old part's going.
                            let old: Vec<_> = tx
                                .functions_of_device(plate.id)
                                .into_iter()
                                .filter(|f| {
                                    tx.get::<Function>(*f).is_some_and(|f| f.template_entry.is_some())
                                })
                                .map(|f| (Kind::Function, f.cast()))
                                .collect();
                            tx.delete_many(&old)?;
                            tx.update::<Device>(plate.id, |d| d.parts = vec![part_use(*part)])?;
                            plate_functions(tx, plate.id, *part)?;
                        }
                        Some(plate.id)
                    }
                    other => {
                        if let Some(valve) = other {
                            remove_child(tx, &valve, p, self.module)?;
                        }
                        let parts = part.map(part_use).into_iter().collect();
                        let plate = new_child(
                            tx,
                            self.module,
                            p,
                            Some(DeviceKind::BlankingPlate),
                            parts,
                            DeviceState::default(),
                        )?;
                        if let Some(part) = part {
                            plate_functions(tx, plate, *part)?;
                        }
                        Some(plate)
                    }
                }
            }
            OccupancyParam::Empty => {
                if let Some(h) = held {
                    remove_child(tx, &h, p, self.module)?;
                }
                None
            }
        };
        let primary = child.unwrap_or(self.module);
        Ok(Outcome::primary((Kind::Device, primary.cast())))
    }
}

/// The closure of [`ClosePort`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClosureParam {
    /// Plug, silencer or blanked.
    pub kind: ClosureKindParam,
    /// The plug's or silencer's part copy ID; optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub part: Option<Id<Part>>,
}

/// The kind of a [`ClosureParam`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClosureKindParam {
    /// A plug.
    Plug,
    /// A silencer.
    Silencer,
    /// Blanked.
    Blanked,
}

/// `fluid.port.close` (`ClosePort`, doc 11 §4.3, FR-FLU-028): closes a fluid port with a plug, a
/// silencer or a blanking (`CpOverride.closure`), or opens it again without `closure`.
///
/// A closed port is exempt from EW-FL-001. Refused: a CP that is no fluid port; a fluid port with a fluid line (when
/// closing); a part copy the project does not hold (`NotFound`). Opening a port whose override then holds nothing
/// else removes the override. Interactive, one undo step. Outcome: the function.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClosePort {
    /// The function.
    #[schemars(with = "UuidS")]
    pub function: Id<Function>,
    /// The CP index.
    pub idx: u16,
    /// The closure; absent opens the port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure: Option<ClosureParam>,
}

impl Command for ClosePort {
    const NAME: &'static str = "fluid.port.close";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        tx.require::<Function>(self.function)?;
        let (f, idx) = (self.function, self.idx);
        if !matches!(cp_fluid_class(tx, f, idx), FluidPortClass::Fluid(_)) {
            return tx.reject(text!(
                "refusal-connection-not-fluid-port",
                crate::words::cp(tx, f, idx)
            ));
        }
        let closure = match &self.closure {
            Some(c) => {
                if let Some(part) = c.part {
                    tx.require::<LibCopy<Part>>(part)?;
                }
                let lined = tx
                    .connections_at(ew_model::EndpointKey::Cp(f, idx))
                    .into_iter()
                    .filter_map(|c| tx.get::<Connection>(c))
                    .any(|c| matches!(c.kind, ConnectionKind::Fluid(_)));
                if lined {
                    return tx.reject(text!("refusal-fluid-port-has-line", crate::words::cp(tx, f, idx)));
                }
                Some(PortClosure {
                    kind: match c.kind {
                        ClosureKindParam::Plug => ClosureKind::Plug,
                        ClosureKindParam::Silencer => ClosureKind::Silencer,
                        ClosureKindParam::Blanked => ClosureKind::Blanked,
                    },
                    part: c.part.map(PartRef::new),
                    extra: Extra::new(),
                })
            }
            None => None,
        };
        tx.update::<Function>(f, |func| {
            let mut overrides: Vec<CpOverride> = func.cp_overrides.iter().cloned().collect();
            match overrides.iter_mut().find(|o| o.idx == idx) {
                Some(o) => o.closure = closure,
                None if closure.is_some() => overrides.push(CpOverride {
                    idx,
                    designation: None,
                    alias: None,
                    description: None,
                    unused: false,
                    closure,
                    extra: Extra::new(),
                }),
                None => {}
            }
            overrides.retain(|o| {
                o.designation.is_some()
                    || o.alias.is_some()
                    || o.description.is_some()
                    || o.unused
                    || o.closure.is_some()
                    || !o.extra.is_empty()
            });
            func.cp_overrides = ew_schema::KeyedList::from_vec(overrides).expect("one override per CP");
        })?;
        Ok(Outcome::primary((Kind::Function, f.cast())))
    }
}

/// A fluid line in words: "-201U3-Q1:2 → -203Q1:A" by its ends.
fn line_words(tx: &TxBuilder<'_>, c: &Connection) -> Text {
    let end = |e: &Endpoint| -> TextArg {
        match e {
            Endpoint::Cp(f, idx) => crate::words::cp(tx, *f, *idx).into(),
            Endpoint::Junction(j) => crate::words::junction(tx, *j).into(),
            _ => text!("refusal-words-connection-end").into(),
        }
    };
    text!("refusal-words-connection", end(&c.a), end(&c.b))
}

/// A length in millimetres as words take it.
fn mm(n: Nm) -> String {
    Dec3(n.0 / 1_000).to_string()
}

/// Checks a tube specification against FI-05 (doc 11 §4.1): diameters larger than zero, the outer larger than the
/// inner, a pressure rating of zero or more; refused in words.
pub(crate) fn check_tube(tx: &TxBuilder<'_>, spec: &TubeSpec) -> Result<(), CmdError> {
    for d in [spec.od, spec.id].into_iter().flatten() {
        if d.0 <= 0 {
            return tx.reject(text!("refusal-tube-diameter-positive", mm(d)));
        }
    }
    if let (Some(od), Some(id)) = (spec.od, spec.id)
        && od <= id
    {
        return tx.reject(text!("refusal-tube-diameters", mm(od), mm(id)));
    }
    if let Some(p) = spec.p_rating
        && p < Dec3::ZERO
    {
        return tx.reject(text!("refusal-tube-rating-negative", p.to_string()));
    }
    if let Some(part) = &spec.part {
        tx.require::<LibCopy<Part>>(part.copy)?;
    }
    Ok(())
}

/// The values of a project medium ([`EditMedium`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MediumParam {
    /// The name ("compressed air 6 bar").
    #[schemars(with = "MlTextS")]
    pub name: MlText,
    /// The ISO 1219-2 medium letter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub code: Option<Ident>,
    /// Pneumatic (the default), vacuum or hydraulic.
    #[serde(default = "pneumatic")]
    pub discipline: Discipline,
    /// The nominal pressure (bar, gauge).
    #[schemars(with = "Dec3S")]
    pub p_nom: Dec3,
    /// The quality class per ISO 8573-1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub quality: Option<Ident>,
    /// The temperature range (°C).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t_range: Option<TempRange>,
    /// The colour of its tubes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// The default tube of its lines.
    #[serde(default, skip_serializing_if = "TubeSpec::is_empty")]
    pub default_tube: TubeSpec,
    /// The default tube of the lines of a kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_kind_tubes: Vec<LineKindTube>,
}

fn pneumatic() -> Discipline {
    Discipline::Pneumatic
}

/// `fluid.medium.edit` (`EditMedium`, doc 11 §4.3, FR-FLU-008): makes, changes or deletes a project medium.
///
/// - Without `medium`: a new medium with the values of `set` (required).
/// - With `medium` and `set`: the medium takes the values of `set`, its ID and provenance kept.
/// - With `medium` and no `set`: the medium is deleted with its definition points, with a notice naming how many.
///
/// Refused: an empty name; a discipline other than pneumatic, vacuum or hydraulic; a default tube against FI-05
/// (diameters larger than zero, the outer larger than the inner, a rating of zero or more); a tube part the project
/// does not hold (`NotFound`); a medium that does not exist (`NotFound`). Interactive, one undo step. Outcome: the
/// medium (none after a deletion).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditMedium {
    /// The medium; absent makes a new one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub medium: Option<Id<Medium>>,
    /// Its values; absent deletes `medium`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<MediumParam>,
}

impl Command for EditMedium {
    const NAME: &'static str = "fluid.medium.edit";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if let Some(m) = self.medium {
            tx.require::<Medium>(m)?;
        }
        let Some(set) = &self.set else {
            let Some(m) = self.medium else {
                return tx.reject(text!("refusal-medium-nothing"));
            };
            let defs = tx
                .iter::<MediumDef>()
                .into_iter()
                .filter(|d| d.medium == m)
                .count();
            let name = tx
                .get::<Medium>(m)
                .map(|x| ew_derive::neutral(&x.name))
                .unwrap_or_default();
            tx.delete((Kind::Medium, m.cast()))?;
            tx.notice(match defs {
                0 => format!("deleted medium {name}"),
                n => format!("deleted medium {name} with {n} definition points"),
            });
            return Ok(Outcome::default());
        };
        if set.name.is_empty() {
            return tx.reject(text!("refusal-medium-name-required"));
        }
        if !matches!(
            set.discipline,
            Discipline::Pneumatic | Discipline::Vacuum | Discipline::Hydraulic
        ) {
            return tx.reject(text!("refusal-medium-discipline"));
        }
        check_tube(tx, &set.default_tube)?;
        for t in &set.line_kind_tubes {
            check_tube(tx, &t.tube)?;
        }
        let value = |id: Id<Medium>, source_ref, extra| Medium {
            id,
            v: 1,
            name: set.name.clone(),
            code: set.code.clone(),
            discipline: set.discipline.clone(),
            p_nom: set.p_nom,
            quality: set.quality.clone(),
            t_range: set.t_range.clone(),
            colour: set.colour.clone(),
            default_tube: set.default_tube.clone(),
            line_kind_tubes: set.line_kind_tubes.clone(),
            source_ref,
            extra,
        };
        let id = match self.medium {
            Some(m) => {
                let old = tx.require::<Medium>(m)?;
                let new = value(m, old.source_ref.clone(), old.extra.clone());
                tx.update::<Medium>(m, |x| *x = new)?;
                m
            }
            None => {
                let id: Id<Medium> = tx.mint();
                tx.insert(value(id, None, Extra::new()))?;
                id
            }
        };
        Ok(Outcome::primary((Kind::Medium, id.cast())))
    }
}

/// `fluid.medium.define` (`DefineMedium`, doc 11 §4.3, FR-FLU-009): the medium a fluid port carries, the seed of its
/// propagation; without `medium` the port's definition is removed.
///
/// FI-04: the port must be a fluid port, and holds at most one definition: a new one replaces the old. Refused: a CP
/// that is no fluid port (in words); a function or medium that does not exist (`NotFound`). Interactive, one undo
/// step. Outcome: the definition (the function after a removal).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DefineMedium {
    /// The function.
    #[schemars(with = "UuidS")]
    pub function: Id<Function>,
    /// The CP index.
    pub idx: u16,
    /// The medium; absent removes the port's definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub medium: Option<Id<Medium>>,
}

impl Command for DefineMedium {
    const NAME: &'static str = "fluid.medium.define";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        tx.require::<Function>(self.function)?;
        let (f, idx) = (self.function, self.idx);
        if !matches!(cp_fluid_class(tx, f, idx), FluidPortClass::Fluid(_)) {
            return tx.reject(text!(
                "refusal-connection-not-fluid-port",
                crate::words::cp(tx, f, idx)
            ));
        }
        if let Some(m) = self.medium {
            tx.require::<Medium>(m)?;
        }
        let at = Endpoint::Cp(f, idx);
        let held: Vec<(Kind, ew_ids::Id<()>)> = tx
            .iter::<MediumDef>()
            .into_iter()
            .filter(|d| d.at == at)
            .map(|d| (Kind::MediumDef, d.id.cast()))
            .collect();
        tx.delete_many(&held)?;
        let Some(medium) = self.medium else {
            return Ok(Outcome::primary((Kind::Function, f.cast())));
        };
        let id: Id<MediumDef> = tx.mint();
        tx.insert(MediumDef {
            id,
            v: 1,
            at,
            medium,
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::MediumDef, id.cast())))
    }
}

/// A value of a tube specification or a fitting, for [`SetTube`]'s `reset`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TubeField {
    /// The outer diameter.
    Od,
    /// The inner diameter.
    Id,
    /// The material.
    Material,
    /// The colour.
    Colour,
    /// The tube part.
    Part,
    /// The pressure rating.
    PRating,
    /// The temperature range.
    TRange,
    /// The fitting at end a.
    FittingA,
    /// The fitting at end b.
    FittingB,
}

/// The fluid lines of a selection, each checked; refused in words for a connection that is no fluid line.
fn fluid_lines(tx: &TxBuilder<'_>, lines: &[Id<Connection>]) -> Result<Vec<Id<Connection>>, CmdError> {
    let mut out = Vec::new();
    for l in lines {
        let c = tx.require::<Connection>(*l)?;
        if !matches!(c.kind, ConnectionKind::Fluid(_)) {
            return tx.reject(text!("refusal-fluid-line-not", line_words(tx, &c)));
        }
        if !out.contains(l) {
            out.push(*l);
        }
    }
    if out.is_empty() {
        return tx.reject(text!("refusal-fluid-lines-none"));
    }
    Ok(out)
}

/// `fluid.tube.set` (`SetTube`, doc 11 §4.3, FR-FLU-010): sets tube overrides and end fittings on a selection of
/// fluid lines.
///
/// Each value `set` gives replaces the line's override of it; each value `reset` names goes back to the derived
/// value (doc 11 §5.4); `fitting_a` and `fitting_b` set the fitting parts at the line's ends. The resulting overrides
/// of every line are checked against FI-05. Refused: no line; a connection that is no fluid line (in words); a tube
/// against FI-05 (in words); a part the project does not hold (`NotFound`). Interactive, one undo step. Outcome: the
/// first line.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetTube {
    /// The lines.
    #[schemars(with = "Vec<UuidS>")]
    pub lines: Vec<Id<Connection>>,
    /// The values to set.
    #[serde(default, skip_serializing_if = "TubeSpec::is_empty")]
    pub set: TubeSpec,
    /// The fitting part at end a.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub fitting_a: Option<Id<Part>>,
    /// The fitting part at end b.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub fitting_b: Option<Id<Part>>,
    /// The values that go back to the derived value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reset: Vec<TubeField>,
}

impl Command for SetTube {
    const NAME: &'static str = "fluid.tube.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let lines = fluid_lines(tx, &self.lines)?;
        for p in [self.fitting_a, self.fitting_b].into_iter().flatten() {
            tx.require::<LibCopy<Part>>(p)?;
        }
        for l in &lines {
            let c = tx.require::<Connection>(*l)?;
            let ConnectionKind::Fluid(mut line) = c.kind.clone() else {
                continue;
            };
            let s = &self.set;
            let t = &mut line.tube;
            if s.od.is_some() {
                t.od = s.od;
            }
            if s.id.is_some() {
                t.id = s.id;
            }
            if s.material.is_some() {
                t.material.clone_from(&s.material);
            }
            if s.colour.is_some() {
                t.colour.clone_from(&s.colour);
            }
            if s.part.is_some() {
                t.part.clone_from(&s.part);
            }
            if s.p_rating.is_some() {
                t.p_rating = s.p_rating;
            }
            if s.t_range.is_some() {
                t.t_range.clone_from(&s.t_range);
            }
            if let Some(p) = self.fitting_a {
                line.fitting_a = Some(PartRef::new(p));
            }
            if let Some(p) = self.fitting_b {
                line.fitting_b = Some(PartRef::new(p));
            }
            for f in &self.reset {
                match f {
                    TubeField::Od => line.tube.od = None,
                    TubeField::Id => line.tube.id = None,
                    TubeField::Material => line.tube.material = None,
                    TubeField::Colour => line.tube.colour = None,
                    TubeField::Part => line.tube.part = None,
                    TubeField::PRating => line.tube.p_rating = None,
                    TubeField::TRange => line.tube.t_range = None,
                    TubeField::FittingA => line.fitting_a = None,
                    TubeField::FittingB => line.fitting_b = None,
                }
            }
            check_tube(tx, &line.tube)?;
            tx.update::<Connection>(*l, |c| c.kind = ConnectionKind::Fluid(line))?;
        }
        Ok(Outcome::primary((Kind::Connection, lines[0].cast())))
    }
}

/// `fluid.length.set` (`SetLength`, doc 11 §4.3, FR-FLU-011): the manual length of a selection of fluid lines, or
/// none (`length` absent: the length is missing again). Refused: no line; a connection that is no fluid line; a
/// length of zero or less (in words). Interactive, one undo step. Outcome: the first line.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetLength {
    /// The lines.
    #[schemars(with = "Vec<UuidS>")]
    pub lines: Vec<Id<Connection>>,
    /// The length in nanometres; absent: missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub length: Option<Nm>,
}

impl Command for SetLength {
    const NAME: &'static str = "fluid.length.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let lines = fluid_lines(tx, &self.lines)?;
        let length = match self.length {
            Some(n) if n.0 <= 0 => return tx.reject(text!("refusal-fluid-length-positive", mm(n))),
            Some(n) => LengthSource::Manual(n),
            None => LengthSource::Missing,
        };
        for l in &lines {
            tx.update::<Connection>(*l, |c| {
                if let ConnectionKind::Fluid(line) = &mut c.kind {
                    line.length = length.clone();
                }
            })?;
        }
        Ok(Outcome::primary((Kind::Connection, lines[0].cast())))
    }
}
