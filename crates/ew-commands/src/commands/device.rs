// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device and representation commands: `device.create`, `device.place`, `rep.move`, `rep.transform`,
//! `rep.set_anchor_policy`, `box.black`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{
    AnchorPolicy, AnyId, BoxSpec, Connection, ConnectionKind, ContactKind, Device, DeviceAspects, DeviceKind,
    Endpoint, Extra, FnDef, FnRole, FrozenTag, Function, FunctionKind, Junction, Kind, LibCopy, NumberSpec,
    Page, PageItem, PageItemKind, PageType, Part, PartRef, PartUse, RepType, Representation, SlotOverride,
    Strip, StructureAssignment, StructureBox, SymbolDef, TagSpec,
};
use ew_units::{Angle2, Dec3, Grid, Nm, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::commands::common::EndpointParam;
use crate::commands::layout::push_after_growth;
use crate::commands::terminal::{TerminalSpec, insert_terminal, strip_usage};
use crate::error::CmdError;
use crate::geometry::{
    OutOfRange, add, follow_end, follow_ends, rep_cps_checked, require_on_grid, sub, translate,
};
use crate::meta::CommandMeta;
use crate::proxy::{Angle2S, IdentS, MlTextS, Orient2S, Pt2S, UuidS};
use crate::text::{Text, text};
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
    /// A frozen product designation, set in the creating commit (a new strip's tag typed in its form).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_tag: Option<FrozenTag>,
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
            tag: TagSpec {
                frozen: self.frozen_tag.clone(),
                ..TagSpec::new(self.letter.clone(), self.counter)
            },
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
        /// The letter code (the identifier-set lookup from the FnDef category is not built yet).
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
    /// A new device made from a part copy (part-first placement; FR-UI-017, FR-DEV-017): the device
    /// `{tag: {letter, counter: none}, parts: [the part, quantity 1]}` and one function per entry of the part's
    /// `function_template`; the representation draws entry `template`, the others stay unplaced.
    FromPart {
        /// The part copy.
        #[schemars(with = "UuidS")]
        part: Id<Part>,
        /// The 0-based index of the template entry the representation draws.
        template: u16,
        /// The letter code of the new device.
        #[schemars(with = "IdentS")]
        letter: Ident,
    },
    /// One new terminal of an existing strip, created as `terminal.add` creates it and placed (`device.place`
    /// only; AC-28).
    NewTerminal {
        /// The strip device.
        #[schemars(with = "UuidS")]
        strip: Id<Device>,
        /// The terminal's designation, unique within the strip.
        #[schemars(with = "IdentS")]
        designation: Ident,
        /// The sequence number; default one more than the highest in the strip.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seq: Option<u32>,
        /// The terminal's part copy (part-first placement of a terminal part): the terminal gets the part use
        /// `{part, qty: 1}`; when the part has a function template, its main entry's FnDef must be the symbol's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(with = "Option<UuidS>")]
        part: Option<Id<Part>>,
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
    /// The wire's route in nm (a pole joined to a pole out of line); empty: the straight run between the ends.
    /// Checked as `connection.create` checks a route (`geometry::require_route`, at `/connect/<i>/route`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<Pt2S>")]
    pub route: Vec<Pt2>,
}

/// `device.place` (`PlaceSymbol`, doc 06 §3.3): places a symbol and creates the device and function as the
/// target requires.
///
/// - `copies`: library documents (`lib/` JSON form) embedded first, in the same transaction, as
///   `part.assign` embeds its closure: each is checked as `library.embed` checks it, at `/copies/i`; a copy the
///   project holds with the same hash is skipped, one it holds with another hash is refused ("update from library
///   first", naming the item and the held version), the others are inserted. So a placement from a library is one
///   undo step and the command replays without a library.
/// - The page and the symbol copy must exist. The representation is `{functions: [f], page, symbol,
///   variant, pos, orient, rep_type}`; `rep_type` defaults to the symbol's.
/// - `new_device`: a device `{tag: {letter, counter: none}, kind}` and a main function `{def}`; `def` is
///   `fndef` or the FnDef copy `v5(NS_LIBCOPY, library ‖ item)` of the symbol's `fndef` reference, which
///   must be embedded (else rejected with a hint to pass `fndef`).
/// - `existing_function`: the function must exist and must not be an interruption point (`ip.place`).
/// - `join_device`: a new function of the device with the given role (a second main function is rejected
///   by EW-INV-002).
/// - `from_part` (FR-UI-017, FR-DEV-017): a device `{tag: {letter, counter: none}, parts: [the part, quantity
///   1]}` and one function per entry of the part copy's `function_template`, in template order (def: the entry's
///   FnDef copy, which must be embedded; role main for `main`, else auxiliary; kind `black_box` when the FnDef's
///   contact kind is `black_box`, else normal; `template_entry` the entry's index, so the CPs follow from that
///   entry, doc 03 §7); the representation
///   draws entry `template`, the others stay unplaced. The symbol's FnDef must be entry `template`'s, and that
///   entry must not be a black box (`box.black` places those). A part without a function template takes
///   `template` 0 and gets one main function of the symbol's FnDef.
/// - `new_terminal` (AC-28): one terminal of the strip, created as `terminal.add` creates it (the strip's letter,
///   structure and the sequence number as counter; the next sequence number when `seq` is absent) with its main
///   function `{def: the symbol's FnDef}`, then placed. With `part` (part-first placement of a terminal part) the
///   terminal also gets `parts: [the part, quantity 1]`; the part copy must exist and, when it has a function
///   template, its main entry's FnDef must be the symbol's.
/// - Every hint creates a wire `{a: the hint's CP, b: to, page}` with the straight route between its two ends' points
///   (none when they do not lie on one line, so the wire is unrouted); the hint's symbol CP is
///   mapped to a function CP as the derived pin map does (`(functions[0], fn_cp)`); an unknown symbol CP
///   is rejected. A hint ending at a junction appends the wire to the junction's sequence; a hint ending
///   on a point of a connection (`HintTarget::Split`) puts a new junction there, splits the connection at
///   it (as `connection.draw` does) and ends the wire at the junction (the point must be on the page's
///   connection grid). A hint whose function, junction or connection no longer exists is dropped and
///   reported as a notice instead of failing the command.
/// - The effective `rep_type` must be one the page's type allows (doc 06 §2.2, `allowed_rep_types`);
///   else the placement is rejected with the list of allowed types.
/// - `into` (the window sends the valve terminal whose box the insertion point lies in): the new device joins that
///   terminal as `assembly::seat_on_terminal` says (a valve in the cell it is placed in when the terminal is drawn as
///   its positions, else at the first free position; a supply plate without one), in the same transaction; any other
///   device is left as it is. With `replace`, a held cell's valve or blanking plate is removed for the new valve.
///   Without `into`, a function of a terminal's valve drawn on a page where the terminal is drawn as its positions
///   stands in its cell.
///
/// Rejected besides: a missing part copy or template index; a template without exactly one main entry (a device
/// has one main function); a template FnDef that is neither embedded nor in `copies` (embed it
/// first); a strip that is not a strip; a designation or sequence number the strip uses already.
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
    /// The library documents to embed with the placement (`lib/` JSON form): the symbol, the part and their
    /// closure as the caller resolved it; empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
    /// The valve terminal whose box the symbol is placed in: a new valve or supply plate joins it (`seat_on_terminal`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub into: Option<Id<Device>>,
    /// With `into`: a valve placed in a held cell of a terminal drawn as its positions replaces the cell's valve or
    /// blanking plate (Replace… on the cell's menu); default false (refused).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replace: bool,
}

impl Command for PlaceSymbol {
    const NAME: &'static str = "device.place";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).with_aliases(&["symbol.place"])
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        // The copies first, so that the symbol, the part and the FnDefs they bring resolve below.
        embed_closure(Self::NAME, tx, &self.copies)?;
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
                return tx.reject(text!("refusal-device-place-no-cp", hint.cp.to_string()));
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
                    return tx.reject(text!("refusal-device-place-ip"));
                }
                *function
            }
            PlaceTarget::JoinDevice { device, fndef, role } => {
                tx.require::<Device>(*device)?;
                let def = place_fndef(tx, &symbol, *fndef)?;
                insert_function(tx, Some(*device), def, role.clone())?
            }
            PlaceTarget::FromPart {
                part,
                template,
                letter,
            } => {
                let symbol_def = place_fndef(tx, &symbol, None)?;
                insert_from_part(tx, *part, *template, letter, PartEntry::Normal(symbol_def))?
            }
            PlaceTarget::NewTerminal {
                strip,
                designation,
                seq,
                part,
            } => {
                let def = place_fndef(tx, &symbol, None)?;
                insert_new_terminal(tx, *strip, designation, *seq, def, *part)?
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
            locked: false,
            extra: Extra::new(),
        };
        if rep_cps_checked(tx, &record).is_err() {
            return tx.reject(text!("refusal-device-place-out-of-range"));
        }
        tx.insert(record)?;
        for (i, hint) in self.connect.iter().enumerate() {
            if let Some(gone) = missing_target(tx, &hint.to) {
                notices.push(text!("notice-hint-dropped", (i + 1).to_string(), gone));
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
                return tx.reject(text!("refusal-device-place-lost-cp"));
            };
            hint_wire(
                tx,
                Endpoint::Cp(function, fn_cp),
                &hint.to,
                &hint.route,
                self.page,
                i,
            )?;
        }
        if let Some(terminal) = self.into
            && let Some(device) = tx.require::<Function>(function)?.device
        {
            super::assembly::seat_on_terminal(tx, device, terminal, Some(rep), self.replace)?;
        } else if let Some(parent) = tx
            .require::<Function>(function)?
            .device
            .and_then(|d| tx.get::<Device>(d)?.parent)
        {
            // A valve of a terminal drawn as its positions, drawn on the box's page, stands in its cell.
            super::assembly::seat_reps(tx, parent)?;
        }
        Ok(Outcome {
            notices,
            ..Outcome::primary((Kind::Representation, rep.cast()))
        })
    }
}

/// The wire of the autoconnect hint `i` (of `device.place`, or of `rep.move`'s `connect`): `{a, b: to, page}` with the
/// hint's route when it has one (checked), else the straight route between its two ends' points (none when they do not
/// lie on one line); a hint ending at a junction
/// appends the wire to the junction's sequence, one ending on a point of a connection splits it there
/// ([`split_for_hint`], the point checked at `/connect/<i>/to/on_connection/at`).
fn hint_wire(
    tx: &mut TxBuilder<'_>,
    a: Endpoint,
    to: &EndpointParam,
    route: &[Pt2],
    page: Id<Page>,
    i: usize,
) -> Result<(), CmdError> {
    let id: Id<Connection> = tx.mint();
    let (to, joins_sequence) = match to {
        EndpointParam::OnConnection { connection, at } => (
            Endpoint::Junction(crate::commands::connection::split_at(
                tx,
                *connection,
                *at,
                id,
                &format!("/connect/{i}/to/on_connection/at"),
                false,
            )?),
            false,
        ),
        other => (
            other
                .endpoint()
                .expect("a CP or junction parameter has an endpoint"),
            true,
        ),
    };
    let route = if route.is_empty() {
        crate::geometry::hint_route(tx, &a, &to, page)
    } else {
        crate::geometry::require_route(tx, page, &a, &to, route, &format!("/connect/{i}/route"))?;
        route.to_vec()
    };
    // A fluid port joined to a fluid port is a working fluid line (FT-1733, doc 06 §2.2), any other hint a wire.
    let fluid = |e: &Endpoint| {
        matches!(e, Endpoint::Cp(f, idx)
            if matches!(crate::commands::connection::cp_fluid_class(tx, *f, *idx), ew_model::FluidPortClass::Fluid(_)))
    };
    let kind = if fluid(&a) && fluid(&to) {
        ConnectionKind::Fluid(Default::default())
    } else {
        ConnectionKind::Wire
    };
    tx.insert(Connection {
        id,
        v: 1,
        a,
        b: to.clone(),
        page: Some(page),
        route,
        kind,
        overrides: Default::default(),
        number: NumberSpec::default(),
        source_ref: None,
        computed: false,
        extra: Extra::new(),
    })?;
    match (to, joins_sequence) {
        (Endpoint::Junction(j), true) => {
            tx.update::<Junction>(j, |j| j.sequence.push(id))?;
        }
        (Endpoint::Junction(j), false) => crate::commands::connection::point_bevels(tx, &[j])?,
        _ => {}
    }
    Ok(())
}

/// What a hint names that does not exist (any more), as a phrase for a notice.
fn missing_target(tx: &TxBuilder<'_>, to: &EndpointParam) -> Option<Text> {
    match to {
        EndpointParam::Cp { function, .. } => {
            (!tx.exists::<Function>(*function)).then(|| text!("notice-hint-function"))
        }
        EndpointParam::Junction { junction } => {
            (!tx.exists::<Junction>(*junction)).then(|| text!("notice-hint-junction"))
        }
        EndpointParam::OnConnection { connection, .. } => {
            (!tx.exists::<Connection>(*connection)).then(|| text!("notice-hint-wire"))
        }
    }
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
/// allowed: the reference drawings contain exactly these.
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
pub(crate) fn check_rep_type(tx: &TxBuilder<'_>, page: &Page, rep_type: &RepType) -> Result<(), CmdError> {
    let Some(allowed) = allowed_rep_types(&page.kind) else {
        return Ok(());
    };
    if matches!(rep_type, RepType::Unknown(_)) || allowed.contains(rep_type) {
        return Ok(());
    }
    let list = if allowed.is_empty() {
        text!("refusal-rep-type-none").into()
    } else {
        crate::words::comma_list(allowed.iter().map(crate::words::rep_type).collect())
    };
    tx.reject(text!(
        "refusal-rep-type",
        crate::words::rep_type(rep_type),
        crate::words::page_kind(&page.kind),
        crate::words::page(tx, page.id),
        list
    ))
}

fn zero() -> Pt2 {
    Pt2::ORIGIN
}

/// `rep.move`: moves representations by a delta or by grid steps, optionally (by a delta) to another page (doc 06
/// §3.4, §5.7). Moves never change connectivity (FR-CON-004).
///
/// **By a delta.** Every representation's `pos` moves by `delta`; with `page`, its `page` becomes `page`. Routes
/// follow: for each connection with an endpoint CP drawn by a moved representation (a CP of one of its functions,
/// the connection on the representation's old page), if both ends are drawn by moved representations the whole
/// route translates by `delta`; otherwise the route end at the moved endpoint follows (`geometry::follow_end`: the
/// end vertex moves by `delta`, its neighbour keeps the segment orthogonal). A cross-page move also moves every
/// connection whose both ends move to the new page; a connection with only one end on a moved representation (or
/// ending at a junction) is rejected, because the IP-pair replacement of FR-CON-043 is not in v0. Online numbering
/// renumbers a main representation's device whose page-based tag collides on the new page (FR-NUM-029). A wire
/// marker on the old page lets go of the connections that leave it, and a marker left empty is removed (the
/// unlink policy of its list, EW-INV-023).
///
/// **By grid steps** (`steps: [kx, ky]`, what the canvas tools send). Each representation's position
/// becomes the grid point at its grid index plus the steps, exactly, per axis (`Grid::at(Grid::snap(x) + k)`), so
/// several representations stay on the grid even where a uniform nm delta cannot keep them there (the QET grid
/// `7_937_500/3` nm). A symbol fitted to a grid its pin spacing does not fit (its position between grid points, its
/// first connection point on one) steps by that pin instead, its position moving by the same displacement
/// (`geometry::grid_anchor`). A representation with neither on the grid cannot be moved by steps (rejected, named);
/// a move by `delta` checks the same.
/// Routes follow: a connection with an end drawn by a moved representation has that end follow its CP's own
/// displacement (`follow_end`); a connection with both ends moved keeps its shape vertex by vertex: an end vertex
/// becomes its CP's new position, an interior coordinate equal to the old coordinate of an end CP moves with that
/// CP, a coordinate on the grid moves by the same number of steps, and any other coordinate (the router
/// re-solves those later) by `round_half_away(k × num / den)` on that axis; a horizontal or vertical segment whose
/// two coordinates part by 1 nm (two end CPs on one coordinate whose representations' grid indices differ in
/// `k mod 3`) keeps its axis with a jog at the grid point nearest its middle. `steps` is exclusive with a non-zero
/// `delta` and with `page`: the tools never move across pages, and a script's cross-page move keeps `delta`.
///
/// **Page items and structure boxes** (`items`): the page items and structure boxes named move
/// with the representations, translated by `delta`, or with `steps` by `round_half_away(k × num / den)` nm per axis
/// on their page's grid (they need not lie on the grid). `reps` and `items` together name at least one record; every
/// item lies on one page, the page of every named representation; an item of a kind this build does not move (a
/// variant or shape it keeps verbatim) and `items` with a cross-page `page` are rejected. A moved wire marker writes
/// a clean list (its entries naming a connection that does not exist or is not on its page are dropped).
///
/// **Re-solved routes and connect on move** (D15; both optional, so a script's move keeps the follow):
/// after the follow, each entry of `routes` replaces a connection's route exactly as `connection.route` would (errors
/// prefixed `/routes/<i>`); then each hint of `connect` makes a wire from
/// the named representation's symbol CP as a `device.place` hint does, with the straight route between its ends (a
/// hint whose target no longer exists is dropped with a notice; a CP the representation does not draw is refused). A
/// hint onto a point of a connection is refused: a move never splits an existing wire (ADR-0007 decision 4).
///
/// **Into a valve terminal** (`into`, which the Select tool sends for a representation dropped inside a valve
/// terminal's box): after the move, each device the moved representations draw joins the terminal as `device.place`'s
/// `into` makes a new one join it; a device already on it stays where it is.
///
/// Outcome: none; the notices of dropped hints. Coalesce one drag with a key.
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
    /// Grid steps `[kx, ky]` on the page's connection grid, instead of `delta`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<[i64; 2]>,
    /// Page items and structure boxes moving with the representations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub items: Vec<AnyId>,
    /// Routes that replace the followed routes of the connections named, after the follow, each as
    /// `connection.route` takes it (the Select tool's re-solve).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<crate::commands::connection::RouteConnection>,
    /// Autoconnect hints committed with the move, each a `device.place` hint naming its representation
    /// (connect on move).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connect: Vec<MoveHint>,
    /// The valve terminal whose box the representations are moved into: the devices they draw join it as
    /// `device.place`'s `into` makes them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub into: Option<Id<Device>>,
}

/// An autoconnect hint of `rep.move`: a `device.place` hint ([`ConnectHint`]) of a representation,
/// its CP named by function and index (the CP the representation draws, a black box's included), as the window knows
/// a placed representation's CPs.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveHint {
    /// The representation whose CP the wire starts at.
    #[schemars(with = "UuidS")]
    pub rep: Id<Representation>,
    /// The CP the representation draws.
    pub cp: HintCp,
    /// The other end: a CP or a junction (a point on a connection is refused: a move never splits a wire).
    pub to: EndpointParam,
    /// The wire's route in nm after the move, as [`ConnectHint`]'s; empty: the straight run between the ends.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<Pt2S>")]
    pub route: Vec<Pt2>,
}

/// A function's connection point, as a [`MoveHint`] names it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HintCp {
    /// The function.
    #[schemars(with = "UuidS")]
    pub function: Id<Function>,
    /// The function's CP index.
    pub idx: u16,
}

/// `round(k × num / den)`, halves away from zero, saturating (the nm length of `k` grid steps).
pub(crate) fn steps_nm(grid: Grid, k: i64) -> i64 {
    let num = i128::from(k) * i128::from(grid.num());
    let den = i128::from(grid.den());
    let q = num / den;
    let r = num % den;
    let v = if 2 * r.abs() >= den { q + num.signum() } else { q };
    i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
}

/// The grid position `k` steps from the grid position `x`; `None` on overflow.
pub(crate) fn step_axis(grid: Grid, x: Nm, k: i64) -> Option<Nm> {
    grid.at(grid.snap(x).checked_add(k)?)
}

/// A page item or structure box named by `rep.move`'s `items`.
enum MovedItem {
    Item(Arc<PageItem>),
    Box(Arc<StructureBox>),
}

impl MovedItem {
    fn page(&self) -> Id<Page> {
        match self {
            MovedItem::Item(i) => i.page,
            MovedItem::Box(b) => b.page,
        }
    }
}

impl MoveReps {
    /// The page items and structure boxes of `items`, each checked: it exists (not found), it is not listed twice,
    /// all lie on one page, which is the page of every named representation.
    fn moved_items(&self, tx: &TxBuilder<'_>) -> Result<Vec<MovedItem>, CmdError> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for (i, id) in self.items.iter().enumerate() {
            if !seen.insert(*id) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/items/{i}"),
                    text!("refusal-params-item-twice"),
                ));
            }
            let item = if let Some(it) = tx.get::<PageItem>(id.cast()) {
                MovedItem::Item(it)
            } else if let Some(b) = tx.get::<StructureBox>(id.cast()) {
                MovedItem::Box(b)
            } else {
                return Err(CmdError::NotFound {
                    key: (Kind::PageItem, *id),
                });
            };
            out.push(item);
        }
        if out.is_empty() {
            return Ok(out);
        }
        if self.page.is_some() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/items",
                text!("refusal-rep-move-items-other-page"),
            ));
        }
        let page = out[0].page();
        if let Some(i) = out.iter().position(|m| m.page() != page) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                format!("/items/{i}"),
                text!("refusal-rep-move-one-page"),
            ));
        }
        for (i, r) in self.reps.iter().enumerate() {
            let rep = tx.require::<Representation>(*r)?;
            if rep.page != page {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/reps/{i}"),
                    text!(
                        "refusal-rep-move-other-page",
                        crate::words::symbol(tx, *r),
                        crate::words::page(tx, rep.page),
                        crate::words::page(tx, page)
                    ),
                ));
            }
        }
        Ok(out)
    }

    /// After the move: `routes` replace the followed routes, marked `computed` (the move's re-solve), as `layout.apply` applies
    /// them, then each hint of `connect` makes its wire as `device.place` does (a hint whose target no longer exists is
    /// dropped with a notice; a CP the representation does not draw is refused). The notices.
    fn routes_and_hints(&self, tx: &mut TxBuilder<'_>) -> Result<Vec<Text>, CmdError> {
        crate::commands::connection::run_routes(tx, Self::NAME, &self.routes)?;
        let mut notices = Vec::new();
        for (i, hint) in self.connect.iter().enumerate() {
            // ADR-0007 decision 4 holds on a move, so no existing connection is split.
            if matches!(hint.to, EndpointParam::OnConnection { .. }) {
                return tx.reject(text!("refusal-rep-move-split"));
            }
            if let Some(gone) = missing_target(tx, &hint.to) {
                notices.push(text!("notice-hint-dropped", (i + 1).to_string(), gone));
                continue;
            }
            let rep = tx.require::<Representation>(hint.rep)?;
            let (function, idx) = (hint.cp.function, hint.cp.idx);
            if !crate::geometry::rep_cps(tx, &rep)
                .iter()
                .any(|c| c.function == function && c.idx == idx)
            {
                return tx.reject(text!(
                    "refusal-rep-move-no-cp",
                    crate::words::symbol(tx, hint.rep),
                    crate::words::cp(tx, function, idx)
                ));
            }
            hint_wire(
                tx,
                Endpoint::Cp(function, idx),
                &hint.to,
                &hint.route,
                rep.page,
                i,
            )?;
        }
        Ok(notices)
    }

    /// Puts the moved page items and structure boxes, translated by `d` (a moved wire marker with a clean list).
    fn put_items(tx: &mut TxBuilder<'_>, items: &[MovedItem], d: Pt2) -> Result<(), CmdError> {
        for m in items {
            match m {
                MovedItem::Item(item) => {
                    let Some(mut next) = crate::commands::items::translate_item(item, d) else {
                        // The item in words and the true reason
                        let page = crate::words::page(tx, item.page);
                        let why = match &item.kind {
                            PageItemKind::SurfaceView(_) => text!("refusal-rep-move-surface-view", page),
                            PageItemKind::Unknown(_) => text!("refusal-rep-move-unknown-item", page),
                            kind => text!(
                                "refusal-rep-move-item-out-of-range",
                                match kind {
                                    PageItemKind::Graphic(_) => text!("refusal-rep-move-item-graphic"),
                                    PageItemKind::FreeText(_) => text!("refusal-rep-move-item-text"),
                                    PageItemKind::WireMarker(_) => text!("refusal-rep-move-item-wire-marker"),
                                    _ => text!("refusal-rep-move-item-image"),
                                },
                                page
                            ),
                        };
                        return tx.reject(why);
                    };
                    crate::commands::items::clean_marker(tx, &mut next);
                    tx.put(next);
                }
                MovedItem::Box(b) => {
                    let Some(next) = crate::commands::items::translate_box(b, d) else {
                        return tx.reject(text!(
                            "refusal-rep-move-box-out-of-range",
                            crate::words::page(tx, b.page)
                        ));
                    };
                    tx.put(next);
                }
            }
        }
        Ok(())
    }

    /// The step move (see the type's documentation).
    fn run_steps(&self, tx: &mut TxBuilder<'_>, steps: [i64; 2]) -> Result<Outcome, CmdError> {
        if self.delta != Pt2::ORIGIN {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/steps",
                text!("refusal-rep-move-steps-and-delta"),
            ));
        }
        if self.page.is_some() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/steps",
                text!("refusal-rep-move-steps-other-page"),
            ));
        }
        let items = self.moved_items(tx)?;
        let ids: BTreeSet<Id<Representation>> = self.reps.iter().copied().collect();
        let mut reps: Vec<Arc<Representation>> = Vec::new();
        let mut moved: Vec<Representation> = Vec::new();
        for id in &ids {
            let rep = tx.require::<Representation>(*id)?;
            let next = stepped(tx, &rep, steps)?;
            reps.push(rep);
            moved.push(next);
        }
        let mut connections: BTreeSet<Id<Connection>> = BTreeSet::new();
        for rep in &reps {
            for f in drawn_functions(rep) {
                connections.extend(tx.connections_of_function(f));
            }
        }
        // The moved representation drawing an endpoint CP on `page`: the CP's own displacement and its old point (a
        // CP's point is its grid position's own number, so it can move by 1 nm more or less than its representation).
        let end_of = |tx: &TxBuilder<'_>, e: &Endpoint, page: Id<Page>| -> Option<(Pt2, Pt2)> {
            let Endpoint::Cp(f, idx) = e else { return None };
            reps.iter()
                .zip(&moved)
                .filter(|(r, _)| r.page == page && drawn_functions(r).contains(f))
                .find_map(|(r, next)| {
                    let point = |r: &Representation| {
                        crate::geometry::rep_cps(tx, r)
                            .into_iter()
                            .find(|c| c.function == *f && c.idx == *idx)
                            .map(|c| c.pos)
                    };
                    let (at, to) = (point(r)?, point(next)?);
                    Some((sub(to, at).ok()?, at))
                })
        };
        let mut changed: Vec<Connection> = Vec::new();
        for cid in connections {
            let c = tx.require::<Connection>(cid)?;
            let Some(page) = c.page else {
                continue;
            };
            let (a, b) = (end_of(tx, &c.a, page), end_of(tx, &c.b, page));
            if a.is_none() && b.is_none() {
                continue;
            }
            let grid = tx.require::<Page>(page)?.grid;
            let mut next = (*c).clone();
            let followed = match (a, b) {
                (Some((da, pa)), Some((db, pb))) => {
                    step_route(&mut next.route, grid, steps, (da, pa), (db, pb))
                }
                (Some((da, _)), None) => follow_end(&mut next.route, grid, true, da),
                (None, Some((db, _))) => follow_end(&mut next.route, grid, false, db),
                (None, None) => Ok(()),
            };
            if followed.is_err() {
                return tx.reject(text!(
                    "refusal-rep-move-route-out-of-range",
                    crate::words::wire(tx, cid)
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
        if let Some(first) = items.first() {
            let grid = tx.require::<Page>(first.page())?.grid;
            let d = Pt2::new(Nm(steps_nm(grid, steps[0])), Nm(steps_nm(grid, steps[1])));
            Self::put_items(tx, &items, d)?;
        }
        Ok(Outcome {
            notices: self.routes_and_hints(tx)?,
            ..Outcome::default()
        })
    }
}

/// `rep` moved by `steps` grid steps (see [`MoveReps`]): the point the steps are counted from (its position, or a
/// fitted symbol's first pin) at its grid index plus the steps, exactly; refused when it has neither on the
/// grid, or the move leaves the drawable range for it or a connection point.
pub(crate) fn stepped(
    tx: &TxBuilder<'_>,
    rep: &Representation,
    steps: [i64; 2],
) -> Result<Representation, CmdError> {
    let id = rep.id;
    let grid = tx.require::<Page>(rep.page)?.grid;
    let Some(anchor) = crate::geometry::grid_anchor(tx, rep, grid) else {
        return tx.reject(text!(
            "refusal-rep-move-steps-off-grid",
            crate::geometry::off_grid(tx, crate::words::symbol(tx, id), rep.page, rep.pos)
        ));
    };
    let (Some(x), Some(y)) = (
        step_axis(grid, anchor.x, steps[0]),
        step_axis(grid, anchor.y, steps[1]),
    ) else {
        return tx.reject(text!(
            "refusal-rep-move-steps-out-of-range",
            crate::words::symbol(tx, id)
        ));
    };
    let mut next = rep.clone();
    let Ok(pos) = sub(Pt2::new(x, y), anchor).and_then(|d| add(rep.pos, d)) else {
        return tx.reject(text!(
            "refusal-rep-move-steps-out-of-range",
            crate::words::symbol(tx, id)
        ));
    };
    next.pos = pos;
    crate::geometry::require_rep_on_grid(tx, next.page, &next, "/steps")?;
    if rep_cps_checked(tx, &next).is_err() {
        return tx.reject(text!(
            "refusal-rep-move-steps-cp-out-of-range",
            crate::words::symbol(tx, id)
        ));
    }
    Ok(next)
}

/// The route of a connection whose both ends move by grid steps (see [`MoveReps`]): the end vertices become their
/// CPs' new positions; an interior coordinate equal to an end CP's old coordinate moves with that CP (end `a`
/// first), one on the grid by the steps, any other by the steps' rounded nm length; a segment whose two coordinates
/// part by the moves keeps its axis with a jog. On [`OutOfRange`] the route is unchanged.
pub(crate) fn step_route(
    route: &mut Vec<Pt2>,
    grid: Grid,
    steps: [i64; 2],
    a: (Pt2, Pt2),
    b: (Pt2, Pt2),
) -> Result<(), OutOfRange> {
    let n = route.len();
    let axis = |x: Nm, k: i64, ca: Nm, da: Nm, cb: Nm, db: Nm| -> Result<Nm, OutOfRange> {
        let v = if x == ca {
            x.0.checked_add(da.0)
        } else if x == cb {
            x.0.checked_add(db.0)
        } else if crate::geometry::on_grid(grid, x) {
            step_axis(grid, x, k).map(|v| v.0)
        } else {
            x.0.checked_add(steps_nm(grid, k))
        };
        v.map(Nm)
            .filter(|v| crate::geometry::in_range(*v))
            .ok_or(OutOfRange)
    };
    let mut next = Vec::with_capacity(n);
    for (i, v) in route.iter().enumerate() {
        let p = if i == 0 && v.x == a.1.x && v.y == a.1.y {
            add(*v, a.0)?
        } else if i + 1 == n && v.x == b.1.x && v.y == b.1.y {
            add(*v, b.0)?
        } else {
            Pt2::new(
                axis(v.x, steps[0], a.1.x, a.0.x, b.1.x, b.0.x)?,
                axis(v.y, steps[1], a.1.y, a.0.y, b.1.y, b.0.y)?,
            )
        };
        next.push(p);
    }
    // Two coordinates that were equal can part by 1 nm (two CPs on one coordinate whose representations' grid
    // indices differ in `k mod 3` move by displacements that differ by 1 nm): such a segment would turn diagonal.
    // It keeps its axis with a jog at the grid point nearest its middle:
    // every new vertex has the jog's grid coordinate on one axis and an end's coordinate on the other.
    let mut out: Vec<Pt2> = Vec::with_capacity(next.len() + 4);
    for i in 0..n {
        out.push(next[i]);
        if i + 1 == n {
            break;
        }
        let (o0, o1, p0, p1) = (route[i], route[i + 1], next[i], next[i + 1]);
        let horizontal = o0.y == o1.y && o0.x != o1.x && p0.y != p1.y;
        let vertical = o0.x == o1.x && o0.y != o1.y && p0.x != p1.x;
        if horizontal {
            let xm = grid
                .snap_point(Nm(p0.x.0 + (p1.x.0 - p0.x.0) / 2))
                .ok_or(OutOfRange)?;
            out.push(Pt2::new(xm, p0.y));
            out.push(Pt2::new(xm, p1.y));
        } else if vertical {
            let ym = grid
                .snap_point(Nm(p0.y.0 + (p1.y.0 - p0.y.0) / 2))
                .ok_or(OutOfRange)?;
            out.push(Pt2::new(p0.x, ym));
            out.push(Pt2::new(p1.x, ym));
        }
    }
    out.dedup();
    *route = out;
    Ok(())
}

impl Command for MoveReps {
    const NAME: &'static str = "rep.move";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let out = self.run_move(tx)?;
        if let Some(terminal) = self.into {
            // Each device with the first moved representation that draws it: the cell it lands in is that one's.
            let mut devices: BTreeMap<Id<Device>, Id<Representation>> = BTreeMap::new();
            for r in &self.reps {
                for f in drawn_functions(&*tx.require::<Representation>(*r)?) {
                    if let Some(d) = tx.get::<Function>(f).and_then(|f| f.device) {
                        devices.entry(d).or_insert(*r);
                    }
                }
            }
            for (d, r) in devices {
                super::assembly::seat_on_terminal(tx, d, terminal, Some(r), false)?;
            }
        }
        Ok(out)
    }
}

impl MoveReps {
    /// The move itself (see the type's documentation), without `into`.
    pub(crate) fn run_move(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.reps.is_empty() && self.items.is_empty() {
            return tx.reject(text!("refusal-rep-move-nothing"));
        }
        if let Some(steps) = self.steps {
            return self.run_steps(tx, steps);
        }
        let items = self.moved_items(tx)?;
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
                    return tx.reject(text!(
                        "refusal-rep-move-out-of-range",
                        crate::words::symbol(tx, *id)
                    ));
                }
            };
            if let (Some(p), Some(page)) = (self.page, &target_page) {
                next.page = p;
                check_rep_type(tx, page, &rep.rep_type)?;
            }
            crate::geometry::require_rep_on_grid(tx, next.page, &next, "/delta")?;
            if rep_cps_checked(tx, &next).is_err() {
                return tx.reject(text!(
                    "refusal-rep-move-cp-out-of-range",
                    crate::words::symbol(tx, *id)
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
        let mut off_page: Vec<Id<Connection>> = Vec::new();
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
                    return tx.reject(text!("refusal-rep-move-end-behind", crate::words::wire(tx, cid)));
                }
                next.page = Some(new_page);
                off_page.push(cid);
            }
            // Each moved end follows its CP's own displacement, which can differ from `delta` by 1 nm (a CP's point is
            // its grid position's own number); a route with both ends moved translates by `delta` first.
            let grid = tx.require::<Page>(page)?.grid;
            let shift = |e: &Endpoint| -> Option<Pt2> {
                let Endpoint::Cp(f, idx) = e else { return None };
                reps.iter().zip(&moved).find_map(|(r, n)| {
                    let point = |r: &Representation| {
                        crate::geometry::rep_cps(tx, r)
                            .into_iter()
                            .find(|c| c.function == *f && c.idx == *idx)
                            .map(|c| c.pos)
                    };
                    (r.page == page && drawn_functions(r).contains(f))
                        .then(|| sub(point(n)?, point(r)?).ok())
                        .flatten()
                })
            };
            let (da, db) = (
                a.then(|| shift(&c.a)).flatten().unwrap_or(self.delta),
                b.then(|| shift(&c.b)).flatten().unwrap_or(self.delta),
            );
            let followed = if a && b {
                translate(&mut next.route, self.delta).and_then(|()| {
                    let rest = |d: Pt2| sub(d, self.delta).ok().filter(|r| *r != Pt2::ORIGIN);
                    follow_ends(&mut next.route, grid, rest(da), rest(db))
                })
            } else if a {
                follow_end(&mut next.route, grid, true, da)
            } else {
                follow_end(&mut next.route, grid, false, db)
            };
            if followed.is_err() {
                return tx.reject(text!(
                    "refusal-rep-move-route-out-of-range",
                    crate::words::wire(tx, cid)
                ));
            }
            changed.push(next);
        }
        for c in changed {
            tx.put(c);
        }
        // A wire marker stays on its page (a cross-page move takes no `items`): one on the old page lets go of the
        // connections that leave it, as the marker list's unlink policy does on a delete (EW-INV-023).
        for cid in off_page {
            tx.unlink_markers(cid, None, self.page);
        }
        for rep in moved {
            tx.put(rep);
        }
        Self::put_items(tx, &items, self.delta)?;
        Ok(Outcome {
            notices: self.routes_and_hints(tx)?,
            ..Outcome::default()
        })
    }
}

/// `rep.transform`: rotates, mirrors or changes the variant of representations, each about the middle of its body
/// (doc 06 §3.4), so that it stays where it stands.
///
/// The new orientation is `orient.then(r(rotate))`, then `.then(m0)` when `mirror` (rotation clockwise in
/// quarter turns, `rotate` in 0..=3, then a mirror about the vertical axis). `variant`, when given,
/// replaces the stored variant.
///
/// **The pivot.** A representation turns about the centre of its body's bounding box: a black box's (a
/// representation with a `box`: the outline's, else its CPs'), a symbol's graphics (texts left out: turned about its
/// insertion point, a cylinder would jump onto its neighbour). `pos` becomes the grid point that keeps that centre,
/// computed in whole grid half-steps, so a mirror and a half turn keep the bounding box (to the 1 nm of a rational
/// grid's rounding) and a quarter turn turns it about its centre; when the centre falls between two grid points (a
/// body whose width plus height is an odd number of steps) the tie goes up toward an odd number of quarter turns and
/// down toward an even one (`Grid::index_of_half_steps`), so four quarter turns, or a turn and its reverse, bring it
/// back exactly and two quarter turns land where a half turn does, wherever it lies. The stored outline and CPs are
/// not rewritten. A representation whose orientation does not change keeps its `pos`, and one without graphics or a
/// box turns about its insertion point. A fitted symbol (its position between grid points, its first connection point
/// on one) is fitted again after the turn (`geometry::fitted_pos`), so its first pin stays on the grid.
///
/// Routes follow: each attached connection's route end at a CP drawn by a transformed representation (a symbol's
/// CP or a black box's) follows that CP's displacement (`geometry::follow_end`); CPs whose geometry is unknown
/// (a symbol without CPs and without a box) leave routes unchanged. Outcome: none.
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
            return tx.reject(text!("refusal-rep-transform-nothing"));
        }
        if self.rotate > 3 {
            return tx.reject(text!("refusal-rep-transform-rotate"));
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
            // One of EW's interruption-point forms is never turned.
            if orient != old.orient && crate::normalise::ip_form::form(tx, old.symbol).is_some() {
                return tx.reject(crate::normalise::ip_form::turned());
            }
            let mut new = (*old).clone();
            new.orient = orient;
            if let Some(v) = self.variant {
                new.variant = v;
            }
            let out_of_range = |tx: &TxBuilder<'_>| {
                tx.reject::<()>(text!(
                    "refusal-rep-transform-cp-out-of-range",
                    crate::words::symbol(tx, *id)
                ))
            };
            if orient != old.orient {
                let body: Vec<Pt2> = match old.box_.as_ref() {
                    Some(b) if b.outline.is_empty() => b.cps.iter().map(|c| c.pos).collect(),
                    Some(b) => b.outline.clone(),
                    // A symbol turned about its insertion point jumped onto its neighbour.
                    None => tx
                        .get::<LibCopy<SymbolDef>>(old.symbol)
                        .and_then(|s| s.item.graphic_bounds())
                        .map(|(lo, hi)| vec![lo, hi])
                        .unwrap_or_default(),
                };
                let grid = tx.require::<Page>(old.page)?.grid;
                // A fitted symbol (its position between grid points, its first pin on one) is fitted again after the
                // turn, so its first pin stays on the grid.
                let fitted = !crate::geometry::point_on_grid(grid, old.pos)
                    && crate::geometry::grid_anchor(tx, &old, grid).is_some();
                match pivot(grid, old.pos, old.orient, orient, &body) {
                    Ok(Some(pos)) if fitted => {
                        new.pos = pos;
                        match crate::geometry::fitted_pos(tx, &new, grid) {
                            Ok(pos) => new.pos = pos,
                            Err(OutOfRange) => {
                                out_of_range(tx)?;
                                continue;
                            }
                        }
                    }
                    Ok(Some(pos)) => new.pos = pos,
                    Ok(None) => {}
                    Err(OutOfRange) => {
                        out_of_range(tx)?;
                        continue;
                    }
                }
            }
            // No text override is written. A black box turned by a half turn places its texts as on
            // the same box unturned (`ew_derive::text_frame`), so a second half turn gives back the record as it was.
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
            let grid = tx.require::<Page>(page)?.grid;
            let followed = follow_ends(&mut next.route, grid, da, db);
            if followed.is_err() {
                return tx.reject(text!(
                    "refusal-rep-transform-route-out-of-range",
                    crate::words::wire(tx, cid)
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

/// The insertion point at which a representation whose body has the points `local` (relative to `pos`, unturned)
/// keeps the centre of its bounding box when its orientation changes from `from` to `to` (see [`TransformReps`]),
/// computed in whole grid half-steps: `pos`
/// and every point of the body under `from` and under `to` are snapped to grid indices first (a stored point
/// lies within 1 nm of its grid point, a step is far longer), so the centre's offset is an exact number of
/// half-steps on every grid, the rational QET grid included. A whole result is exact; a half one (a quarter
/// turn of a box whose width plus height is odd) goes up toward an odd number of quarter turns of `to` and
/// down toward an even one ([`Grid::index_of_half_steps`]), so every two quarter turns cancel: four turns
/// bring the body back, a turn and its reverse too, two land where one half turn does, and the offset is the
/// same wherever the body lies. `Ok(None)` without a point (the insertion point stays). [`OutOfRange`] when a
/// point of the body or the result leaves ±[`MAX_COORD`](crate::geometry::MAX_COORD).
fn pivot(grid: Grid, pos: Pt2, from: Orient2, to: Orient2, local: &[Pt2]) -> Result<Option<Pt2>, OutOfRange> {
    if local.is_empty() {
        return Ok(None);
    }
    if !local.iter().all(|p| crate::geometry::point_in_range(*p)) || !crate::geometry::point_in_range(pos) {
        return Err(OutOfRange);
    }
    // min + max of each axis of the points' grid indices under `o`: twice the bounding box's centre relative to
    // `pos`, in half-steps. `Orient2::apply` only negates and swaps, and `Grid::snap` is odd, so snapping the turned
    // offset is turning the snapped one.
    let twice_centre = |o: Orient2| -> (i64, i64) {
        let pts: Vec<(i64, i64)> = local
            .iter()
            .map(|p| o.apply(*p))
            .map(|p| (grid.snap(p.x), grid.snap(p.y)))
            .collect();
        let span = |axis: fn(&(i64, i64)) -> i64| {
            let lo = pts.iter().map(axis).min().unwrap_or(0);
            let hi = pts.iter().map(axis).max().unwrap_or(0);
            lo + hi
        };
        (span(|p| p.0), span(|p| p.1))
    };
    let (cf, ct) = (twice_centre(from), twice_centre(to));
    let tie_up = to.quarter_turns() % 2 == 1;
    // Every index lies within ±2^54 (a step is at least 1 nm), so the sums stay far inside i64.
    let index = |p: Nm, f: i64, t: i64| Grid::index_of_half_steps(2 * grid.snap(p) + f - t, tie_up);
    let x = grid.at(index(pos.x, cf.0, ct.0));
    let y = grid.at(index(pos.y, cf.1, ct.1));
    match (x, y) {
        (Some(x), Some(y)) if crate::geometry::point_in_range(Pt2::new(x, y)) => Ok(Some(Pt2::new(x, y))),
        _ => Err(OutOfRange),
    }
}

#[cfg(test)]
mod box_pivot_tests;

mod port_boxes;
pub use port_boxes::{PlacePortBoxes, PortBox, PortsTarget};

/// `rep.text_set` (`SetRepText`, doc 06 §2.5): moves, turns or hides one text of a representation's symbol on its page:
/// sets the members given of the representation's override of slot `slot` and keeps the others. `pos` is a point of
/// the representation's text frame (the symbol's coordinates, `ew_derive::text_frame`), `angle` the text's angle in the
/// symbol's coordinates (a turned symbol prints it as it is, made readable), `visible` shows or hides it. An override
/// left with no member is removed. Rejected: a slot the symbol does not have (`/slot`); a missing representation is
/// not found. Interactive. Outcome: the representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetRepText {
    /// The representation.
    #[schemars(with = "UuidS")]
    pub rep: Id<Representation>,
    /// The symbol's text slot (its index).
    pub slot: u16,
    /// The text's new anchor in the symbol's coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Pt2S>")]
    pub pos: Option<Pt2>,
    /// The text's new angle in the symbol's coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Angle2S>")]
    pub angle: Option<Angle2>,
    /// Shown or hidden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

impl Command for SetRepText {
    const NAME: &'static str = "rep.text_set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let rep = tx.require::<Representation>(self.rep)?;
        let slots = tx
            .get::<LibCopy<SymbolDef>>(rep.symbol)
            .map_or(0, |s| s.item.text_slots.len());
        if usize::from(self.slot) >= slots {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/slot",
                text!("refusal-rep-text-slot", self.slot.to_string()),
            ));
        }
        let (slot, pos, angle, visible) = (self.slot, self.pos, self.angle, self.visible);
        tx.update::<Representation>(self.rep, |r| {
            let mut o = r.text_slots.get(&slot).cloned().unwrap_or(SlotOverride {
                slot,
                pos: None,
                angle: None,
                visible: None,
                extra: Extra::new(),
            });
            o.pos = pos.or(o.pos);
            o.angle = angle.or(o.angle);
            o.visible = visible.or(o.visible);
            if o.pos.is_none() && o.angle.is_none() && o.visible.is_none() && o.extra.is_empty() {
                r.text_slots.remove(&slot);
            } else {
                r.text_slots.insert(o);
            }
        })?;
        Ok(Outcome::primary((Kind::Representation, self.rep.cast())))
    }
}

/// `rep.texts_reset` (`ResetRepTexts`, doc 06 §2.5): puts every text of the representations' symbols back where the
/// symbol places it: removes the position and the angle of each slot override (a hidden text stays hidden); an override
/// left with no member is removed. A representation without such an override is left untouched. Rejected: an empty
/// `reps` list; a missing representation is not found. Interactive. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResetRepTexts {
    /// The representations, at least one.
    #[schemars(with = "Vec<UuidS>")]
    #[schemars(length(min = 1))]
    pub reps: Vec<Id<Representation>>,
}

impl Command for ResetRepTexts {
    const NAME: &'static str = "rep.texts_reset";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.reps.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/reps",
                text!("refusal-params-no-rep"),
            ));
        }
        for id in &self.reps {
            let rep = tx.require::<Representation>(*id)?;
            if !rep
                .text_slots
                .iter()
                .any(|o| o.pos.is_some() || o.angle.is_some())
            {
                continue;
            }
            tx.update::<Representation>(*id, |r| {
                let kept: Vec<SlotOverride> = r
                    .text_slots
                    .iter()
                    .cloned()
                    .map(|o| SlotOverride {
                        pos: None,
                        angle: None,
                        ..o
                    })
                    .filter(|o| o.visible.is_some() || !o.extra.is_empty())
                    .collect();
                r.text_slots = ew_schema::KeyedList::from_vec(kept).unwrap_or_default();
            })?;
        }
        Ok(Outcome::default())
    }
}

/// `rep.set_anchor_policy`: sets the cross-reference anchor policy of representations (FR-SYM-010; the
/// quick fix of MF-13, doc 07 §3.6).
///
/// The policy is a per-representation override, `Representation.anchor`: `anchor`
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
                text!("refusal-params-no-rep"),
            ));
        }
        let mut seen = BTreeSet::new();
        for (i, id) in self.reps.iter().enumerate() {
            if !seen.insert(*id) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/reps/{i}"),
                    text!("refusal-params-rep-twice"),
                ));
            }
        }
        if matches!(self.anchor, Some(AnchorPolicy::Unknown(_))) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/anchor",
                text!("refusal-rep-anchor-policy-unknown"),
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
/// - `copies` as in `device.place`: embedded first, in the same transaction.
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
///   `black_box` (else rejected with a hint to use `device.place`); `from_part` the device and every template
///   function of the part as in `device.place`, the representation drawing entry `template`, which must be a
///   black box or a port function (a box with the sockets of a multi-port interface, doc 10 §2.7; the function
///   stays a normal one); `new_terminal` is invalid here.
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
    /// The library documents to embed with the placement (`lib/` JSON form): the symbol, the part and their
    /// closure as the caller resolved it; empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

impl Command for PlaceBlackBox {
    const NAME: &'static str = "box.black";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        // The copies first, so that the symbol, the part and the FnDefs they bring resolve below.
        embed_closure(Self::NAME, tx, &self.copies)?;
        let page = tx.require::<Page>(self.page)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.symbol)?;
        require_on_grid(tx, self.page, self.pos, "/pos")?;
        let rep_type = self
            .rep_type
            .clone()
            .unwrap_or_else(|| symbol.item.rep_type.clone());
        check_rep_type(tx, &page, &rep_type)?;
        if let Some(b) = &self.box_ {
            check_box(Self::NAME, tx, self.page, self.pos, self.orient, b, "/box")?;
        }
        let function = match &self.target {
            PlaceTarget::NewDevice { letter, fndef, kind } => {
                let def = place_fndef(tx, &symbol, *fndef)?;
                let device = insert_new_device(tx, letter, kind)?;
                insert_function_of_kind(tx, Some(device), def, FnRole::Main, FunctionKind::BlackBox, None)?
            }
            PlaceTarget::ExistingFunction { function } => {
                let f = tx.require::<Function>(*function)?;
                if f.kind != FunctionKind::BlackBox {
                    return tx.reject(text!("refusal-box-black-not-black-box"));
                }
                *function
            }
            PlaceTarget::JoinDevice { device, fndef, role } => {
                tx.require::<Device>(*device)?;
                let def = place_fndef(tx, &symbol, *fndef)?;
                insert_function_of_kind(tx, Some(*device), def, role.clone(), FunctionKind::BlackBox, None)?
            }
            PlaceTarget::FromPart {
                part,
                template,
                letter,
            } => insert_from_part(tx, *part, *template, letter, PartEntry::BlackBox)?,
            PlaceTarget::NewTerminal { .. } => {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/target/new_terminal",
                    text!("refusal-box-black-terminal"),
                ));
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
            locked: false,
            extra: Extra::new(),
        };
        if rep_cps_checked(tx, &record).is_err() {
            return tx.reject(text!("refusal-box-black-out-of-range"));
        }
        let slotted = record.box_.as_ref().is_some_and(|b| b.slots.is_some());
        tx.insert(record)?;
        // A valve terminal drawn as its positions: its valves drawn on this page stand in their cells.
        if slotted && let Some(device) = tx.require::<Function>(function)?.device {
            super::assembly::seat_reps(tx, device)?;
        }
        Ok(Outcome::primary((Kind::Representation, rep.cast())))
    }
}

/// The checks of a black box's `box` (`box.black`, `bus.place_ports`), its pointers led by `at`: an outline of at
/// least three points, no box CP index twice, and, with `orient` r0, every box CP on the page's connection grid.
pub(crate) fn check_box(
    command: &'static str,
    tx: &TxBuilder<'_>,
    page: Id<Page>,
    pos: Pt2,
    orient: Orient2,
    b: &BoxSpec,
    at: &str,
) -> Result<(), CmdError> {
    if b.outline.len() < 3 {
        return Err(CmdError::invalid_params(
            command,
            format!("{at}/outline"),
            text!("refusal-box-black-few-points"),
        ));
    }
    let mut seen = BTreeSet::new();
    for (i, cp) in b.cps.iter().enumerate() {
        if !seen.insert(cp.idx) {
            return Err(CmdError::invalid_params(
                command,
                format!("{at}/cps/{i}/idx"),
                text!("refusal-box-cp-index-twice"),
            ));
        }
        if orient == Orient2::default() {
            let Ok(point) = add(pos, cp.pos) else {
                return tx.reject(text!("refusal-box-black-cp-out-of-range", i.to_string()));
            };
            require_on_grid(tx, page, point, &format!("{at}/cps/{i}/pos"))?;
        }
    }
    Ok(())
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
        return tx.reject(text!("refusal-device-place-fndef-missing"));
    }
    Ok(def)
}

/// Inserts a normal function of `device` with the given definition and role.
pub(crate) fn insert_function(
    tx: &mut TxBuilder<'_>,
    device: Option<Id<Device>>,
    def: Id<FnDef>,
    role: FnRole,
) -> Result<Id<Function>, CmdError> {
    insert_function_of_kind(tx, device, def, role, FunctionKind::Normal, None)
}

/// Inserts a function of `device` with the given definition, role and kind; `template_entry` is the index of the
/// part template entry it is made from, `None` for a function not made from a template.
pub(crate) fn insert_function_of_kind(
    tx: &mut TxBuilder<'_>,
    device: Option<Id<Device>>,
    def: Id<FnDef>,
    role: FnRole,
    kind: FunctionKind,
    template_entry: Option<u16>,
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
        template_entry,
        bus_port: None,
        extra: Extra::new(),
    })
}

/// What the placed entry of a `from_part` target must be.
pub(crate) enum PartEntry {
    /// A normal function whose FnDef is the symbol's.
    Normal(Id<FnDef>),
    /// A black box (`box.black`).
    BlackBox,
    /// Any entry: the device of a part placed by its interfaces (`bus.place_ports`), the entry staying unplaced.
    Any,
}

/// The device and template functions of a `from_part` target (see [`PlaceSymbol`]): returns the function of entry
/// `template`. Checks everything before it writes.
pub(crate) fn insert_from_part(
    tx: &mut TxBuilder<'_>,
    part: Id<Part>,
    template: u16,
    letter: &Ident,
    placed: PartEntry,
) -> Result<Id<Function>, CmdError> {
    insert_template_functions(tx, part, template, letter, placed, None)
}

/// [`insert_from_part`], the template functions given to the existing device `existing` when it is `Some` (a device
/// made without functions, as `plc.add_station` makes a station's modules) instead of to a new device.
pub(crate) fn insert_template_functions(
    tx: &mut TxBuilder<'_>,
    part: Id<Part>,
    template: u16,
    letter: &Ident,
    placed: PartEntry,
    existing: Option<Id<Device>>,
) -> Result<Id<Function>, CmdError> {
    let copy = tx.require::<LibCopy<Part>>(part)?;
    let number = copy.item.part_number.as_str().to_owned();
    let entries = copy.item.function_template.clone();
    if entries.is_empty() {
        // A part without a function template is placed with the symbol the user chose, entry 0 of
        // `device.place` only: the device carries the part and one main function of the symbol's FnDef.
        return match placed {
            PartEntry::Normal(symbol_def) if template == 0 && existing.is_none() => {
                let device = insert_part_device(tx, part, letter)?;
                insert_function(tx, Some(device), symbol_def, FnRole::Main)
            }
            _ => tx.reject(text!("refusal-from-part-no-template", number)),
        };
    }
    if usize::from(template) >= entries.len() {
        return tx.reject(text!(
            "refusal-from-part-no-entry",
            number,
            template.to_string(),
            entries.len().to_string(),
            (entries.len() - 1).to_string()
        ));
    }
    // Every entry's FnDef must be embedded; the kinds follow their contact kinds.
    let mut defs: Vec<(Id<FnDef>, FunctionKind)> = Vec::new();
    // The entries on a port function: a box may draw one (its sockets, doc 10 §2.7).
    let mut ports: Vec<bool> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let def: Id<FnDef> = ew_ids::v5::lib_copy_id(e.fndef.library.as_uuid(), e.fndef.item.as_uuid());
        let Some(fndef) = tx.get::<LibCopy<FnDef>>(def) else {
            return tx.reject(text!("refusal-part-entry-fndef-missing", i.to_string(), number));
        };
        let kind = if fndef.item.contact_kind == ContactKind::BlackBox {
            FunctionKind::BlackBox
        } else {
            FunctionKind::Normal
        };
        ports.push(fndef.item.contact_kind == ContactKind::BusPort);
        defs.push((def, kind));
    }
    // A device has exactly one main function (EW-INV-002): a template without or with several main entries is
    // refused here, naming the part, rather than by the invariant at the end of the transaction.
    let mains = entries.iter().filter(|e| e.main).count();
    if mains != 1 {
        return tx.reject(text!("refusal-from-part-mains", number, mains.to_string()));
    }
    let (def, kind) = defs[usize::from(template)].clone();
    // A symbol of its own drawn for an entry's black-box FnDef (a power supply the user drew) draws it as any symbol:
    // the entry's function is then a normal one.
    let mut drawn_by_symbol = false;
    match placed {
        PartEntry::Normal(symbol_def) => {
            if kind == FunctionKind::BlackBox && symbol_def != def {
                return tx.reject(text!("refusal-from-part-black-box", template.to_string(), number));
            }
            if symbol_def != def {
                return tx.reject(text!(
                    "refusal-from-part-other-fndef",
                    template.to_string(),
                    number
                ));
            }
            drawn_by_symbol = kind == FunctionKind::BlackBox;
        }
        PartEntry::BlackBox => {
            if kind != FunctionKind::BlackBox && !ports[usize::from(template)] {
                return tx.reject(text!(
                    "refusal-from-part-not-black-box",
                    template.to_string(),
                    number
                ));
            }
        }
        PartEntry::Any => {}
    }
    let device = match existing {
        Some(d) => d,
        None => insert_part_device(tx, part, letter)?,
    };
    let mut placed_fn = None;
    for (i, (e, (def, kind))) in entries.iter().zip(defs).enumerate() {
        let role = if e.main { FnRole::Main } else { FnRole::Aux };
        // The entry index is stored; an index beyond u16 (never in practice) is left out and the derivation
        // falls back to the first entry of the FnDef.
        let entry = u16::try_from(i).ok();
        let kind = if drawn_by_symbol && i == usize::from(template) {
            FunctionKind::Normal
        } else {
            kind
        };
        let f = insert_function_of_kind(tx, Some(device), def, role, kind, entry)?;
        if i == usize::from(template) {
            placed_fn = Some(f);
        }
    }
    placed_fn.ok_or_else(|| CmdError::internal("the template entry was not created"))
}

/// The device of a `from_part` target: `{tag: {letter, counter: none}, parts: [the part, quantity 1]}`.
pub(crate) fn insert_part_device(
    tx: &mut TxBuilder<'_>,
    part: Id<Part>,
    letter: &Ident,
) -> Result<Id<Device>, CmdError> {
    let device: Id<Device> = tx.mint();
    tx.insert(Device {
        id: device,
        v: 1,
        tag: TagSpec::new(letter.clone(), None),
        kind: None,
        parent: None,
        position: None,
        structure: StructureAssignment::default(),
        parts: vec![PartUse {
            part: PartRef {
                copy: part,
                variant: None,
                extra: Extra::new(),
            },
            qty: Dec3::ONE,
            extra: Extra::new(),
        }],
        function_text: MlText::default(),
        process_tag: None,
        tech_data: Default::default(),
        state: Default::default(),
        source_ref: None,
        aspects: DeviceAspects::default(),
        extra: Extra::new(),
    })?;
    Ok(device)
}

/// The terminal of a `new_terminal` target (see [`PlaceSymbol`]): returns its main function.
fn insert_new_terminal(
    tx: &mut TxBuilder<'_>,
    strip_id: Id<Device>,
    designation: &Ident,
    seq: Option<u32>,
    def: Id<FnDef>,
    part: Option<Id<Part>>,
) -> Result<Id<Function>, CmdError> {
    let strip = tx.require::<Device>(strip_id)?;
    if strip.aspects.strip.is_none() {
        return tx.reject(text!("refusal-terminal-not-strip"));
    }
    let mut parts = Vec::new();
    if let Some(part) = part {
        let copy = tx.require::<LibCopy<Part>>(part)?;
        let main = copy.item.function_template.iter().find(|e| e.main);
        if let Some(e) = main {
            let main_def: Id<FnDef> =
                ew_ids::v5::lib_copy_id(e.fndef.library.as_uuid(), e.fndef.item.as_uuid());
            if main_def != def {
                return tx.reject(text!("refusal-new-terminal-part", copy.item.part_number.as_str()));
            }
        }
        parts.push(PartUse {
            part: PartRef {
                copy: part,
                variant: None,
                extra: Extra::new(),
            },
            qty: Dec3::ONE,
            extra: Extra::new(),
        });
    }
    let (designations, seqs) = strip_usage(tx, strip_id);
    if designations.contains(designation) {
        return tx.reject(text!("refusal-terminal-designation-used", designation.as_str()));
    }
    let seq = match seq {
        Some(seq) => {
            if seqs.contains(&seq) {
                return tx.reject(text!("refusal-terminal-seq-used", seq.to_string()));
            }
            seq
        }
        None => seqs.last().copied().unwrap_or(0).saturating_add(1),
    };
    let spec = TerminalSpec {
        designation: designation.clone(),
        seq: Some(seq),
        function_text: MlText::default(),
        levels: Vec::new(),
        rail_bonded: false,
    };
    let (_, function) = insert_terminal(tx, strip_id, &strip, def, &spec, seq, parts)?;
    push_after_growth(tx, strip_id)?;
    Ok(function)
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
