// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device and representation commands: `device.create`, `device.place`, `rep.move`, `rep.transform`,
//! `rep.set_anchor_policy`, `box.black`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{
    AnchorPolicy, BoxSpec, Connection, ConnectionKind, Device, DeviceAspects, DeviceKind, Endpoint, Extra,
    FnDef, FnRole, Function, FunctionKind, Junction, JunctionStyle, Kind, LibCopy, NumberSpec, Page,
    PageType, PartUse, RepType, Representation, Strip, StructureAssignment, SymbolDef, TagSpec,
};
use ew_units::{Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::common::EndpointParam;
use crate::error::CmdError;
use crate::geometry::{
    OutOfRange, add, follow_end, rep_cps_checked, require_on_grid, require_split_point, sub, translate,
};
use crate::meta::CommandMeta;
use crate::proxy::{IdentS, MlTextS, Orient2S, Pt2S, UuidS};
use crate::tx::TxBuilder;

/// `device.create`: a device without a representation (FR-DEV-001), optionally with its main function.
///
/// Creates `Device {tag: {letter, counter}, kind, parts, function_text, structure, strip aspect}` and, with
/// `main_fndef`, `Function {device, def: main_fndef, role: main, kind: normal}`. An explicit `counter` is
/// kept (a typed tag); without one the device stays unnumbered until its main function is placed (online
/// numbering, normaliser #4). The part copies and the FnDef copy must exist. Outcome: the device.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateDevice {
    /// The letter code ("F").
    #[schemars(with = "IdentS")]
    pub letter: Ident,
    /// An explicit counter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<u32>,
    /// UI and numbering hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<DeviceKind>,
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
    /// The strip aspect, for a terminal strip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip: Option<Strip>,
    /// The FnDef copy of the main function to create.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub main_fndef: Option<Id<FnDef>>,
}

impl Command for CreateDevice {
    const NAME: &'static str = "device.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        for part in &self.parts {
            tx.require::<LibCopy<ew_schema::Part>>(part.part.copy)?;
        }
        if let Some(fndef) = self.main_fndef {
            tx.require::<LibCopy<FnDef>>(fndef)?;
        }
        let id: Id<Device> = tx.mint();
        tx.insert(Device {
            id,
            v: 1,
            tag: TagSpec::new(self.letter.clone(), self.counter),
            kind: self.kind.clone(),
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
                strip: self.strip.clone(),
                ..DeviceAspects::default()
            },
            extra: Extra::new(),
        })?;
        if let Some(def) = self.main_fndef {
            insert_function(tx, Some(id), def, FnRole::Main)?;
        }
        Ok(Outcome::primary((Kind::Device, id.cast())))
    }
}

/// Which device a placed representation belongs to (doc 06 §3.3).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaceTarget {
    /// A new device with a new main function; its counter comes from online numbering.
    NewDevice {
        /// The letter code (the identifier-set lookup from the FnDef category is WP-S1.11).
        #[schemars(with = "IdentS")]
        letter: Ident,
        /// The FnDef copy; default the copy the symbol's `fndef` resolves to.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(with = "Option<UuidS>")]
        fndef: Option<Id<FnDef>>,
        /// UI and numbering hint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<DeviceKind>,
    },
    /// Another representation of an existing function (FR-DEV-003).
    ExistingFunction {
        /// The function.
        #[schemars(with = "UuidS")]
        function: Id<Function>,
    },
    /// A new function of an existing device (FR-DEV-038).
    JoinDevice {
        /// The device.
        #[schemars(with = "UuidS")]
        device: Id<Device>,
        /// The FnDef copy; default the copy the symbol's `fndef` resolves to.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(with = "Option<UuidS>")]
        fndef: Option<Id<FnDef>>,
        /// The role; default auxiliary.
        #[serde(default = "aux")]
        role: FnRole,
    },
}

fn aux() -> FnRole {
    FnRole::Aux
}

/// An accepted autoconnect hint, as an explicit connection (ADR-0007: autoconnect is an editing aid; the
/// command carries the result).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectHint {
    /// The symbol CP of the new representation (`SymbolCp.idx`).
    pub cp: u16,
    /// The other end: a CP or a junction.
    pub to: EndpointParam,
}

/// `device.place` (`PlaceSymbol`, doc 06 §3.3): places a symbol and creates the device and function as the
/// target requires.
///
/// - The page and the symbol copy must exist. The representation is `{functions: [f], page, symbol,
///   variant, pos, orient, rep_type}`; `rep_type` defaults to the symbol's.
/// - `new_device`: a device `{tag: {letter, counter: none}, kind}` and a main function `{def}`; `def` is
///   `fndef` or the FnDef copy `v5(NS_LIBCOPY, library ‖ item)` of the symbol's `fndef` reference, which
///   must be embedded (else rejected with a hint to pass `fndef`).
/// - `existing_function`: the function must exist and must not be an interruption point (`ip.place`).
/// - `join_device`: a new function of the device with the given role (a second main function is rejected
///   by EW-INV-002).
/// - Every hint creates a wire `{a: the hint's CP, b: to, page}` without a route; the hint's symbol CP is
///   mapped to a function CP as the derived pin map does (`(functions[0], fn_cp)`); an unknown symbol CP
///   is rejected. A hint ending at a junction appends the wire to the junction's sequence; a hint ending
///   on a point of a connection (`HintTarget::Split`) puts a new junction there, splits the connection at
///   it (as `connection.draw` does) and ends the wire at the junction (the point must be on the page's
///   connection grid). A hint whose function, junction or connection no longer exists is dropped and
///   reported as a notice instead of failing the command.
/// - The effective `rep_type` must be one the page's type allows (doc 06 §2.2, `allowed_rep_types`);
///   else the placement is rejected with the list of allowed types.
///
/// Online numbering (normaliser #4) gives a new device its counter; a symbol dropped onto a wire splits it
/// (normaliser #7). Outcome: the representation. Alias `symbol.place`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceSymbol {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The symbol copy.
    #[schemars(with = "UuidS")]
    pub symbol: Id<SymbolDef>,
    /// The insertion point.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// The orientation.
    #[serde(default)]
    #[schemars(with = "Orient2S")]
    pub orient: Orient2,
    /// The representation type; default the symbol's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rep_type: Option<RepType>,
    /// The stored contact-image or CP variant.
    #[serde(default)]
    pub variant: u8,
    /// Which device the representation belongs to.
    pub target: PlaceTarget,
    /// Accepted autoconnect hints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connect: Vec<ConnectHint>,
}

impl Command for PlaceSymbol {
    const NAME: &'static str = "device.place";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).with_aliases(&["symbol.place"])
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let page = tx.require::<Page>(self.page)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.symbol)?;
        require_on_grid(tx, self.page, self.pos, "/pos")?;
        let rep_type = self
            .rep_type
            .clone()
            .unwrap_or_else(|| symbol.item.rep_type.clone());
        check_rep_type(tx, &page, &rep_type)?;
        // Check the hints before anything is written.
        for hint in &self.connect {
            if !symbol.item.cps.as_slice().iter().any(|cp| cp.idx == hint.cp) {
                return tx.reject(format!(
                    "the symbol has no connection point {}; pick one of its CPs",
                    hint.cp
                ));
            }
        }
        let mut notices = Vec::new();
        let function = match &self.target {
            PlaceTarget::NewDevice { letter, fndef, kind } => {
                let def = place_fndef(tx, &symbol, *fndef)?;
                let device = insert_new_device(tx, letter, kind)?;
                insert_function(tx, Some(device), def, FnRole::Main)?
            }
            PlaceTarget::ExistingFunction { function } => {
                let f = tx.require::<Function>(*function)?;
                if matches!(f.kind, FunctionKind::Ip(_)) {
                    return tx
                        .reject("an interruption point is placed with `ip.place`; pick another function");
                }
                *function
            }
            PlaceTarget::JoinDevice { device, fndef, role } => {
                tx.require::<Device>(*device)?;
                let def = place_fndef(tx, &symbol, *fndef)?;
                insert_function(tx, Some(*device), def, role.clone())?
            }
        };
        let rep: Id<Representation> = tx.mint();
        let record = Representation {
            id: rep,
            v: 1,
            functions: vec![function],
            page: self.page,
            symbol: self.symbol,
            variant: self.variant,
            pos: self.pos,
            orient: self.orient,
            rep_type,
            pin_map: Default::default(),
            text_slots: Default::default(),
            arrangement: None,
            anchor: None,
            box_: None,
            source_ref: None,
            extra: Extra::new(),
        };
        if rep_cps_checked(tx, &record).is_err() {
            return tx.reject(
                "`/pos` puts a connection point of the symbol outside the drawable range; place it nearer \
                 the page origin",
            );
        }
        tx.insert(record)?;
        for (i, hint) in self.connect.iter().enumerate() {
            if let Some(gone) = missing_target(tx, &hint.to) {
                notices.push(format!(
                    "autoconnect hint {} dropped: {gone} no longer exists",
                    i + 1
                ));
                continue;
            }
            let Some(fn_cp) = symbol
                .item
                .cps
                .as_slice()
                .iter()
                .find(|cp| cp.idx == hint.cp)
                .map(|cp| cp.fn_cp)
            else {
                return tx.reject("the symbol lost a connection point");
            };
            let id: Id<Connection> = tx.mint();
            let (to, joins_sequence) = match &hint.to {
                EndpointParam::OnConnection { connection, at } => (
                    split_for_hint(
                        tx,
                        *connection,
                        *at,
                        id,
                        &format!("/connect/{i}/to/on_connection/at"),
                    )?,
                    false,
                ),
                other => (
                    other
                        .endpoint()
                        .expect("a CP or junction parameter has an endpoint"),
                    true,
                ),
            };
            tx.insert(Connection {
                id,
                v: 1,
                a: Endpoint::Cp(function, fn_cp),
                b: to.clone(),
                page: Some(self.page),
                route: Vec::new(),
                kind: ConnectionKind::Wire,
                overrides: Default::default(),
                number: NumberSpec::default(),
                source_ref: None,
                extra: Extra::new(),
            })?;
            if let (Endpoint::Junction(j), true) = (to, joins_sequence) {
                tx.update::<Junction>(j, |j| j.sequence.push(id))?;
            }
        }
        Ok(Outcome {
            notices,
            ..Outcome::primary((Kind::Representation, rep.cast()))
        })
    }
}

/// What a hint names that does not exist (any more), as a phrase for a notice.
fn missing_target(tx: &TxBuilder<'_>, to: &EndpointParam) -> Option<String> {
    match to {
        EndpointParam::Cp { function, .. } => {
            (!tx.exists::<Function>(*function)).then(|| format!("function {function}"))
        }
        EndpointParam::Junction { junction } => {
            (!tx.exists::<Junction>(*junction)).then(|| format!("junction {junction}"))
        }
        EndpointParam::OnConnection { connection, .. } => {
            (!tx.exists::<Connection>(*connection)).then(|| format!("connection {connection}"))
        }
    }
}

/// A hint ending on a point of a connection: a new junction at `at`, the connection split there, the
/// junction's sequence `[W′, W″, new wire]` (as `connection.draw` does for an `on_connection` end).
fn split_for_hint(
    tx: &mut TxBuilder<'_>,
    connection: Id<Connection>,
    at: Pt2,
    new_wire: Id<Connection>,
    field: &str,
) -> Result<Endpoint, CmdError> {
    let w = tx.require::<Connection>(connection)?;
    let Some(page) = w.page else {
        return tx.reject(format!(
            "connection {connection} has no page, so a junction cannot be placed on it; give it a page first"
        ));
    };
    require_split_point(tx, &w, page, at, field)?;
    let junction: Id<Junction> = tx.mint();
    tx.insert(Junction {
        id: junction,
        v: 1,
        page,
        at,
        style: JunctionStyle::Dot,
        wiring: None,
        sequence: Vec::new(),
        source_ref: None,
        extra: Extra::new(),
    })?;
    let second = crate::normalise::split_connection(
        tx,
        connection,
        at,
        Endpoint::Junction(junction),
        at,
        Endpoint::Junction(junction),
    )?;
    tx.update::<Junction>(junction, |j| j.sequence = vec![connection, second, new_wire])?;
    Ok(Endpoint::Junction(junction))
}

/// The representation types a page type allows (doc 06 §2.2, page-type table); `None` when the page type
/// is not judged (a type of a newer format). An empty list means the page takes no representations
/// (generated or read-only pages, title pages, model views, external documents; panel layouts, whose
/// surface view has no `RepType` yet).
///
/// MultiLine takes multi-line, contact-image, strip-segment and graphic representations; SingleLine takes
/// single-line; Overview and AssemblyOverview take overview; BusTopology takes topology and graphic; Fluid
/// takes fluid and multi-line; Graphic takes graphic.
///
/// The graphic representations on MultiLine and BusTopology pages (cable markers, the screen graphic) and
/// the multi-line ones on Fluid pages (the CPX black boxes of a valve terminal drawn on its fluid pages) are
/// the owner ruling that amends decision log ID-55 (cycle 15): the reference drawings contain exactly these,
/// and Golden A's tables place them so (doc 20, build path (b)).
pub fn allowed_rep_types(page: &PageType) -> Option<&'static [RepType]> {
    Some(match page {
        PageType::MultiLine => &[
            RepType::MultiLine,
            RepType::ContactImage,
            RepType::StripSegment,
            RepType::Graphic,
        ],
        PageType::SingleLine => &[RepType::SingleLine],
        PageType::Overview | PageType::AssemblyOverview => &[RepType::Overview],
        PageType::BusTopology => &[RepType::Topology, RepType::Graphic],
        PageType::Fluid => &[RepType::Fluid, RepType::MultiLine],
        PageType::Graphic => &[RepType::Graphic],
        PageType::PanelLayout
        | PageType::TableOfContents
        | PageType::SectionDivider
        | PageType::Report
        | PageType::TitlePage
        | PageType::ModelView
        | PageType::ExternalDocument => &[],
        PageType::Unknown(_) => return None,
    })
}

/// Rejects a representation type the page's type does not allow, with the list of allowed types. An
/// unknown representation type is not refused.
fn check_rep_type(tx: &TxBuilder<'_>, page: &Page, rep_type: &RepType) -> Result<(), CmdError> {
    let Some(allowed) = allowed_rep_types(&page.kind) else {
        return Ok(());
    };
    if matches!(rep_type, RepType::Unknown(_)) || allowed.contains(rep_type) {
        return Ok(());
    }
    let list = if allowed.is_empty() {
        "none (this page type takes no representations)".to_owned()
    } else {
        allowed
            .iter()
            .map(|t| format!("{t:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    tx.reject(format!(
        "representation type {rep_type:?} is not allowed on a {:?} page ({}); allowed: {list}",
        page.kind, page.id
    ))
}

fn zero() -> Pt2 {
    Pt2::ORIGIN
}

/// `rep.move`: moves representations by a delta, optionally to another page (doc 06 §3.4). Moves never
/// change connectivity (FR-CON-004).
///
/// Every representation's `pos` moves by `delta`; with `page`, its `page` becomes `page`. Routes follow:
/// for each connection with an endpoint CP drawn by a moved representation (a CP of one of its functions,
/// the connection on the representation's old page), if both ends are drawn by moved representations
/// the whole route translates by `delta`; otherwise the route end at the moved endpoint follows
/// (`geometry::follow_end`: the end vertex moves by `delta`, its neighbour keeps the segment orthogonal).
/// A cross-page move also moves every connection whose both ends move to the new page; a connection with
/// only one end on a moved representation (or ending at a junction) is rejected, because the IP-pair
/// replacement of FR-CON-043 is not in v0. Online numbering renumbers a main representation's device whose
/// page-based tag collides on the new page (FR-NUM-029). Outcome: none. Coalesce one drag with a key.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveReps {
    /// The representations, at least one.
    #[schemars(with = "Vec<UuidS>")]
    pub reps: Vec<Id<Representation>>,
    /// The displacement; default `[0, 0]`.
    #[serde(default = "zero")]
    #[schemars(with = "Pt2S")]
    pub delta: Pt2,
    /// The target page of a cross-page move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub page: Option<Id<Page>>,
}

impl Command for MoveReps {
    const NAME: &'static str = "rep.move";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.reps.is_empty() {
            return tx.reject("give at least one representation to move");
        }
        let target_page = match self.page {
            Some(page) => Some(tx.require::<Page>(page)?),
            None => None,
        };
        let ids: BTreeSet<Id<Representation>> = self.reps.iter().copied().collect();
        let mut reps: Vec<Arc<Representation>> = Vec::new();
        let mut moved: Vec<Representation> = Vec::new();
        for id in &ids {
            let rep = tx.require::<Representation>(*id)?;
            let mut next = (*rep).clone();
            next.pos = match add(rep.pos, self.delta) {
                Ok(p) => p,
                Err(OutOfRange) => {
                    return tx.reject(format!(
                        "`/delta` moves representation {id} outside the drawable range; move it less far"
                    ));
                }
            };
            if let (Some(p), Some(page)) = (self.page, &target_page) {
                next.page = p;
                check_rep_type(tx, page, &rep.rep_type)?;
            }
            require_on_grid(tx, next.page, next.pos, "/delta")?;
            if rep_cps_checked(tx, &next).is_err() {
                return tx.reject(format!(
                    "`/delta` moves a connection point of representation {id} outside the drawable range; \
                     move it less far"
                ));
            }
            reps.push(rep);
            moved.push(next);
        }
        // The connections with an end at a function of a moved representation, in ID order.
        let mut connections: BTreeSet<Id<Connection>> = BTreeSet::new();
        for rep in &reps {
            for f in drawn_functions(rep) {
                connections.extend(tx.connections_of_function(f));
            }
        }
        let drawn = |e: &Endpoint, page: Id<Page>| -> bool {
            e.function().is_some_and(|f| {
                reps.iter()
                    .any(|r| r.page == page && drawn_functions(r).contains(&f))
            })
        };
        let mut changed: Vec<Connection> = Vec::new();
        for cid in connections {
            let c = tx.require::<Connection>(cid)?;
            let Some(page) = c.page else {
                continue;
            };
            let (a, b) = (drawn(&c.a, page), drawn(&c.b, page));
            if !a && !b {
                continue;
            }
            let mut next = (*c).clone();
            if let Some(new_page) = self.page.filter(|np| *np != page) {
                if !(a && b) {
                    return tx.reject(format!(
                        "connection {cid} would keep one end behind on the old page; move both ends \
                         together (the interruption-point replacement of FR-CON-043 is not available yet)"
                    ));
                }
                next.page = Some(new_page);
            }
            let followed = if a && b {
                translate(&mut next.route, self.delta)
            } else {
                follow_end(&mut next.route, a, self.delta)
            };
            if followed.is_err() {
                return tx.reject(format!(
                    "`/delta` moves the route of connection {cid} outside the drawable range; move it less far"
                ));
            }
            changed.push(next);
        }
        for c in changed {
            tx.put(c);
        }
        for rep in moved {
            tx.put(rep);
        }
        Ok(Outcome::default())
    }
}

/// `rep.transform`: rotates, mirrors or changes the variant of representations about their insertion point
/// (doc 06 §3.4).
///
/// The new orientation is `orient.then(r(rotate))`, then `.then(m0)` when `mirror` (rotation clockwise in
/// quarter turns, `rotate` in 0..=3, then a mirror about the vertical axis). `variant`, when given,
/// replaces the stored variant. Routes follow: each attached connection's route end at a CP drawn by a
/// transformed representation follows that CP's displacement (`geometry::follow_end`); CPs whose geometry
/// is unknown (symbol without CPs) leave routes unchanged. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransformReps {
    /// The representations, at least one.
    #[schemars(with = "Vec<UuidS>")]
    pub reps: Vec<Id<Representation>>,
    /// Quarter turns clockwise, 0 to 3.
    #[serde(default)]
    pub rotate: u8,
    /// Mirror after rotating.
    #[serde(default)]
    pub mirror: bool,
    /// A new variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<u8>,
}

impl Command for TransformReps {
    const NAME: &'static str = "rep.transform";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.reps.is_empty() {
            return tx.reject("give at least one representation to transform");
        }
        if self.rotate > 3 {
            return tx.reject("`rotate` counts quarter turns and must be 0 to 3");
        }
        let ids: BTreeSet<Id<Representation>> = self.reps.iter().copied().collect();
        // The displacement of each drawn function CP that moves, by (page, function, CP).
        let mut moves: BTreeMap<(Id<Page>, Id<Function>, u16), Pt2> = BTreeMap::new();
        let mut updated: Vec<Representation> = Vec::new();
        for id in &ids {
            let old = tx.require::<Representation>(*id)?;
            let mut orient = old.orient.then(Orient2::from_parts(false, self.rotate));
            if self.mirror {
                orient = orient.then(Orient2::M0);
            }
            let mut new = (*old).clone();
            new.orient = orient;
            if let Some(v) = self.variant {
                new.variant = v;
            }
            let out_of_range = |tx: &TxBuilder<'_>| {
                tx.reject::<()>(format!(
                    "the transform puts a connection point of representation {id} outside the drawable \
                     range"
                ))
            };
            let (Ok(before), Ok(after_cps)) = (rep_cps_checked(tx, &old), rep_cps_checked(tx, &new)) else {
                out_of_range(tx)?;
                continue;
            };
            for after in after_cps {
                let Some(b) = before.iter().find(|c| c.symbol_cp == after.symbol_cp) else {
                    continue;
                };
                let Ok(d) = sub(after.pos, b.pos) else {
                    out_of_range(tx)?;
                    continue;
                };
                if d != Pt2::ORIGIN {
                    moves.entry((old.page, after.function, after.idx)).or_insert(d);
                }
            }
            updated.push(new);
        }
        let functions: BTreeSet<Id<Function>> = moves.keys().map(|(_, f, _)| *f).collect();
        let mut connections: BTreeSet<Id<Connection>> = BTreeSet::new();
        for f in functions {
            connections.extend(tx.connections_of_function(f));
        }
        for cid in connections {
            let c = tx.require::<Connection>(cid)?;
            let Some(page) = c.page else {
                continue;
            };
            let end = |e: &Endpoint| -> Option<Pt2> {
                match e {
                    Endpoint::Cp(f, idx) => moves.get(&(page, *f, *idx)).copied(),
                    _ => None,
                }
            };
            let (da, db) = (end(&c.a), end(&c.b));
            let mut next = (*c).clone();
            let followed = match (da, db) {
                (Some(a), Some(b)) if a == b => translate(&mut next.route, a),
                _ => da
                    .map_or(Ok(()), |a| follow_end(&mut next.route, true, a))
                    .and_then(|()| db.map_or(Ok(()), |b| follow_end(&mut next.route, false, b))),
            };
            if followed.is_err() {
                return tx.reject(format!(
                    "the transform moves the route of connection {cid} outside the drawable range"
                ));
            }
            tx.put(next);
        }
        for rep in updated {
            tx.put(rep);
        }
        Ok(Outcome::default())
    }
}

/// `rep.set_anchor_policy`: sets the cross-reference anchor policy of representations (FR-SYM-010; the
/// quick fix of MF-13, doc 07 §3.6).
///
/// The policy is a per-representation override, `Representation.anchor` (owner ruling, cycle 14): `anchor`
/// is `origin`, `graphic_centre`, `{"slot": <kind>}` or `{"named": [x, y]}`, and `null` removes the override,
/// so the representation follows its symbol copy's policy again. Other representations of the same symbol
/// copy keep theirs. A representation that already holds the value is left untouched. Rejections: an empty
/// or duplicated `reps` list, an unknown (newer-format) policy; a missing representation is not found. Cost:
/// interactive. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetAnchorPolicy {
    /// The representations, at least one; each at most once.
    #[schemars(with = "Vec<UuidS>")]
    #[schemars(length(min = 1))]
    pub reps: Vec<Id<Representation>>,
    /// The cross-reference anchor policy (FR-SYM-010): `origin`, `graphic_centre`, `{"slot": <kind>}`,
    /// `{"named": [x, y]}`; `null` resets to the symbol copy's policy.
    pub anchor: Option<AnchorPolicy>,
}

impl Command for SetAnchorPolicy {
    const NAME: &'static str = "rep.set_anchor_policy";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.reps.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/reps",
                "give at least one representation",
            ));
        }
        let mut seen = BTreeSet::new();
        for (i, id) in self.reps.iter().enumerate() {
            if !seen.insert(*id) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/reps/{i}"),
                    "the representation is listed twice",
                ));
            }
        }
        if matches!(self.anchor, Some(AnchorPolicy::Unknown(_))) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/anchor",
                "unknown anchor policy",
            ));
        }
        for id in &self.reps {
            let rep = tx.require::<Representation>(*id)?;
            if rep.anchor == self.anchor {
                continue;
            }
            tx.update::<Representation>(*id, |r| r.anchor = self.anchor.clone())?;
        }
        Ok(Outcome::default())
    }
}

/// `box.black` (`PlaceBlackBox`, doc 06 §2.7, §3.1): places a black box, a function of kind `black_box`
/// drawn with the black-box symbol and its page geometry.
///
/// - The page and the symbol copy must exist (the built-in black-box symbol `ew.gen.blackbox` draws the
///   outline, CPs and inner graphics of `box`). The representation is `{functions: [f], page, symbol, pos,
///   orient, rep_type, box}`; `rep_type` defaults to the symbol's and must be one the page type allows (as
///   `device.place`); `pos` must be on the page's connection grid.
/// - `box` (`BoxSpec`, doc 03 §6): the outline, the device CPs with their function CP index, position
///   relative to the representation and direction, and the inner graphics. Absent: the function is drawn
///   by the symbol alone (a black-box function shown without its outline, as the CPX blocks of folios
///   203 … 206). Rejected: an outline of fewer than three points; two box CPs with the same index; a box CP
///   off the page's connection grid (with `orient` r0, the check is on `pos + cp.pos`).
/// - `target` as in `device.place`, with the function's kind `black_box`: `new_device` creates a device
///   `{tag: {letter, counter: none}, kind}` and its main black-box function; `join_device` a black-box
///   function of the device with the given role (default auxiliary; a second main function is rejected by
///   EW-INV-002); `existing_function` another representation of an existing function, which must be of kind
///   `black_box` (else rejected with a hint to use `device.place`).
/// - The black box's CPs are derived, not stored on the function: the part's CP pattern (the
///   template-origin rule of doc 03 §7) or the box CPs.
///
/// Containment (a function placed inside the outline joins the box's device, doc 06 §2.7) is a gesture of
/// the place tool and is not done here. Outcome: the representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceBlackBox {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The black-box symbol copy.
    #[schemars(with = "UuidS")]
    pub symbol: Id<SymbolDef>,
    /// The insertion point.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// The orientation.
    #[serde(default)]
    #[schemars(with = "Orient2S")]
    pub orient: Orient2,
    /// The representation type; default the symbol's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rep_type: Option<RepType>,
    /// The outline, CPs and inner graphics; absent draws the symbol alone.
    #[serde(rename = "box", default, skip_serializing_if = "Option::is_none")]
    pub box_: Option<BoxSpec>,
    /// Which device the black box belongs to.
    pub target: PlaceTarget,
}

impl Command for PlaceBlackBox {
    const NAME: &'static str = "box.black";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let page = tx.require::<Page>(self.page)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.symbol)?;
        require_on_grid(tx, self.page, self.pos, "/pos")?;
        let rep_type = self
            .rep_type
            .clone()
            .unwrap_or_else(|| symbol.item.rep_type.clone());
        check_rep_type(tx, &page, &rep_type)?;
        if let Some(b) = &self.box_ {
            if b.outline.len() < 3 {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/box/outline",
                    "an outline has at least three points",
                ));
            }
            let mut seen = BTreeSet::new();
            for (i, cp) in b.cps.iter().enumerate() {
                if !seen.insert(cp.idx) {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        format!("/box/cps/{i}/idx"),
                        "two box CPs have the same function CP index",
                    ));
                }
                if self.orient == Orient2::default() {
                    let Ok(at) = add(self.pos, cp.pos) else {
                        return tx.reject(format!(
                            "box CP {i} lies outside the drawable range; place the box nearer the page origin"
                        ));
                    };
                    require_on_grid(tx, self.page, at, &format!("/box/cps/{i}/pos"))?;
                }
            }
        }
        let function = match &self.target {
            PlaceTarget::NewDevice { letter, fndef, kind } => {
                let def = place_fndef(tx, &symbol, *fndef)?;
                let device = insert_new_device(tx, letter, kind)?;
                insert_function_of_kind(tx, Some(device), def, FnRole::Main, FunctionKind::BlackBox)?
            }
            PlaceTarget::ExistingFunction { function } => {
                let f = tx.require::<Function>(*function)?;
                if f.kind != FunctionKind::BlackBox {
                    return tx.reject(
                        "the function is not a black box; place another representation of it with \
                         `device.place`",
                    );
                }
                *function
            }
            PlaceTarget::JoinDevice { device, fndef, role } => {
                tx.require::<Device>(*device)?;
                let def = place_fndef(tx, &symbol, *fndef)?;
                insert_function_of_kind(tx, Some(*device), def, role.clone(), FunctionKind::BlackBox)?
            }
        };
        let rep: Id<Representation> = tx.mint();
        let record = Representation {
            id: rep,
            v: 1,
            functions: vec![function],
            page: self.page,
            symbol: self.symbol,
            variant: 0,
            pos: self.pos,
            orient: self.orient,
            rep_type,
            pin_map: Default::default(),
            text_slots: Default::default(),
            arrangement: None,
            anchor: None,
            box_: self.box_.clone(),
            source_ref: None,
            extra: Extra::new(),
        };
        if rep_cps_checked(tx, &record).is_err() {
            return tx.reject(
                "`/pos` puts a connection point of the black box outside the drawable range; place it nearer \
                 the page origin",
            );
        }
        tx.insert(record)?;
        Ok(Outcome::primary((Kind::Representation, rep.cast())))
    }
}

// The functions a representation draws: its `functions` and the functions of its pin map.
fn drawn_functions(rep: &Representation) -> BTreeSet<Id<Function>> {
    rep.functions
        .iter()
        .copied()
        .chain(rep.pin_map.as_slice().iter().map(|p| p.function))
        .collect()
}

/// The FnDef copy of a new function: the one given, else the copy the symbol's `fndef` reference resolves
/// to; either must be embedded.
fn place_fndef(
    tx: &TxBuilder<'_>,
    symbol: &LibCopy<SymbolDef>,
    given: Option<Id<FnDef>>,
) -> Result<Id<FnDef>, CmdError> {
    if let Some(def) = given {
        tx.require::<LibCopy<FnDef>>(def)?;
        return Ok(def);
    }
    let r = &symbol.item.fndef;
    let def: Id<FnDef> = ew_ids::v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid());
    if !tx.exists::<LibCopy<FnDef>>(def) {
        return tx.reject(
            "the function definition of the symbol is not embedded in the project; pass `fndef` or embed it",
        );
    }
    Ok(def)
}

/// Inserts a normal function of `device` with the given definition and role.
fn insert_function(
    tx: &mut TxBuilder<'_>,
    device: Option<Id<Device>>,
    def: Id<FnDef>,
    role: FnRole,
) -> Result<Id<Function>, CmdError> {
    insert_function_of_kind(tx, device, def, role, FunctionKind::Normal)
}

/// Inserts a function of `device` with the given definition, role and kind.
fn insert_function_of_kind(
    tx: &mut TxBuilder<'_>,
    device: Option<Id<Device>>,
    def: Id<FnDef>,
    role: FnRole,
    kind: FunctionKind,
) -> Result<Id<Function>, CmdError> {
    let id: Id<Function> = tx.mint();
    tx.insert(Function {
        id,
        v: 1,
        device,
        def,
        role,
        kind,
        cp_overrides: Default::default(),
        source_ref: None,
        extra: Extra::new(),
    })
}

/// Inserts the device of a `new_device` placement target: `{tag: {letter, counter: none}, kind}`.
fn insert_new_device(
    tx: &mut TxBuilder<'_>,
    letter: &Ident,
    kind: &Option<DeviceKind>,
) -> Result<Id<Device>, CmdError> {
    let device: Id<Device> = tx.mint();
    tx.insert(Device {
        id: device,
        v: 1,
        tag: TagSpec::new(letter.clone(), None),
        kind: kind.clone(),
        parent: None,
        position: None,
        structure: StructureAssignment::default(),
        parts: Vec::new(),
        function_text: MlText::default(),
        process_tag: None,
        tech_data: Default::default(),
        state: Default::default(),
        source_ref: None,
        aspects: DeviceAspects::default(),
        extra: Extra::new(),
    })
}
