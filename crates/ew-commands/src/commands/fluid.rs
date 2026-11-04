// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid commands of WP-S1.20 (doc 11 §4.3): `valve.set_occupancy` (`SetValveOccupancy`, D9) and
//! `fluid.port.close` (`ClosePort`, D10).

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::MlText;
use ew_model::FluidPortClass;
use ew_schema::{
    Channel, ClosureKind, Connection, ConnectionKind, ContactKind, CpOverride, Device, DeviceAspects,
    DeviceKind, DeviceState, Endpoint, Extra, FnDef, FnRole, Function, FunctionKind, Kind, LibCopy, Part,
    PartRef, PartUse, PortClosure, TagScheme, TagSpec,
};
use ew_units::Dec3;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::commands::connection::cp_fluid_class;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
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

/// `valve.set_occupancy` (`SetValveOccupancy`, doc 11 §4.3, WP-S1.20 D9): what one valve position of a module holds.
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
///   given part; otherwise a child `{kind: blanking_plate, tag: child scheme, letter Q, counter = position, parts:
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

/// The number of valve positions of a module (WP-S1.20 D8): `plc.positions.count` of the part named by
/// `plc_module.part_use`, else of the first part use whose copy has `plc.positions`; `None` for a device without.
pub(crate) fn position_count(tx: &TxBuilder<'_>, module: &Device) -> Option<u16> {
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

/// The child of `module` at `position`: of two, the lower ID (WP-S1.20 D8).
fn child_at(tx: &TxBuilder<'_>, module: Id<Device>, position: u32) -> Option<Arc<Device>> {
    tx.children_of(module)
        .into_iter()
        .filter_map(|c| tx.get::<Device>(c))
        .filter(|d| d.position == Some(position))
        .min_by_key(|d| d.id)
}

/// Removes a child of a module with its delete plan, with the notices of WP-S1.20 §5 T24.
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
    let unbound: Vec<String> = bound.iter().map(|c| crate::words::channel(tx, *c)).collect();
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
            return tx.reject(format!(
                "the function definition of template entry {i} of part {} is not embedded in the project; embed it \
                 first (`library.embed`)",
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
            return tx.reject(format!("{module_name} has no valve positions"));
        };
        let p = self.position;
        if p == 0 || p > u32::from(n) {
            return tx.reject(format!(
                "{module_name} has {n} valve positions; position {p} does not exist"
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
                            return tx.reject(format!(
                                "position {p} of {module_name} holds {name}; assign the new part to {name} instead"
                            ));
                        }
                        let state = state.state();
                        tx.update::<Device>(v.id, |d| d.state = state)?;
                        Some(v.id)
                    }
                    None => {
                        let Some(part) = part else {
                            return tx.reject(format!(
                                "position {p} of {module_name} holds no valve; name the valve part"
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
                        if let Some(part) = part {
                            tx.update::<Device>(plate.id, |d| d.parts = vec![part_use(*part)])?;
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

/// `fluid.port.close` (`ClosePort`, doc 11 §4.3, WP-S1.20 D10, FR-FLU-028): closes a fluid port with a plug, a
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
            return tx.reject(format!("{} is not a fluid port", crate::words::cp(tx, f, idx)));
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
                    return tx.reject(format!(
                        "{} has a fluid line; delete the line before closing the port",
                        crate::words::cp(tx, f, idx)
                    ));
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
