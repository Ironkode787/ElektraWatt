// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connectivity commands (ADR-0007, doc 06 §3.5): `connection.draw`, `connection.route`,
//! `connection.delete`, `junction.edit`, `ip.place`, `ip.continue`, `potential.define`, `bond.create`.

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_model::{EndpointKey, FluidPortClass, fluid_port_class};
use ew_schema::{
    Bond, BondKind, CircuitClass, Connection, ConnectionKind, Device, Endpoint, ExternalRef, Extra, FnDef,
    FnRole, Function, FunctionKind, IpDirection, IpSpec, Junction, JunctionStyle, JunctionWiring, Kind,
    LibCopy, NumberSpec, Page, PotentialDef, PotentialEnv, PotentialType, RepType, Representation, SymbolDef,
    WireOverrides, WireProps,
};
use ew_units::{Dec3, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::commands::common::EndpointParam;
use crate::error::CmdError;
use crate::geometry::{require_on_grid, require_route, require_split_point};
use crate::meta::CommandMeta;
use crate::normalise::split_connection;
use crate::proxy::{Dec3S, IdentS, Orient2S, Pt2S, UuidS};
use crate::text::{Text, TextArg, text};
use crate::tx::TxBuilder;

/// Resolves one end of `connection.draw` to a record endpoint. A point on a connection W creates a
/// junction there and splits W at it; an existing junction gets the new connection appended.
fn resolve_end(
    tx: &mut TxBuilder<'_>,
    end: &EndpointParam,
    field: &str,
    new_id: Id<Connection>,
    split_page: &mut Option<Id<Page>>,
) -> Result<Endpoint, CmdError> {
    match end {
        EndpointParam::Cp { function, idx } => Ok(Endpoint::Cp(*function, *idx)),
        EndpointParam::Junction { junction } => {
            tx.update::<Junction>(*junction, |j| j.sequence.push(new_id))?;
            Ok(Endpoint::Junction(*junction))
        }
        EndpointParam::OnConnection { connection, at } => {
            let w = tx.require::<Connection>(*connection)?;
            let Some(page) = w.page else {
                return tx.reject(text!("refusal-connection-no-page", connection.to_string()));
            };
            require_split_point(tx, &w, page, *at, &format!("{field}/on_connection/at"))?;
            let junction: Id<Junction> = tx.mint();
            tx.insert(Junction {
                id: junction,
                v: 1,
                page,
                at: *at,
                style: JunctionStyle::Dot,
                wiring: None,
                sequence: Vec::new(),
                source_ref: None,
                extra: Extra::new(),
            })?;
            let second = split_connection(
                tx,
                *connection,
                *at,
                Endpoint::Junction(junction),
                *at,
                Endpoint::Junction(junction),
            )?;
            tx.update::<Junction>(junction, |j| j.sequence = vec![*connection, second, new_id])?;
            split_page.get_or_insert(page);
            Ok(Endpoint::Junction(junction))
        }
    }
}

/// Creates an IP function and its one representation; returns the function.
#[allow(clippy::too_many_arguments)]
fn new_ip(
    tx: &mut TxBuilder<'_>,
    def: Id<FnDef>,
    spec: IpSpec,
    page: Id<Page>,
    symbol: Id<SymbolDef>,
    pos: Pt2,
    orient: Orient2,
    rep_type: RepType,
) -> Result<Id<Function>, CmdError> {
    let function: Id<Function> = tx.mint();
    let rep: Id<Representation> = tx.mint();
    tx.insert(Function {
        id: function,
        v: 1,
        device: None,
        def,
        role: FnRole::Main,
        kind: FunctionKind::Ip(spec),
        cp_overrides: Default::default(),
        source_ref: None,
        template_entry: None,
        bus_port: None,
        extra: Extra::new(),
    })?;
    tx.insert(Representation {
        id: rep,
        v: 1,
        functions: vec![function],
        page,
        symbol,
        variant: 0,
        pos,
        orient,
        rep_type,
        pin_map: Default::default(),
        text_slots: Default::default(),
        arrangement: None,
        anchor: None,
        box_: None,
        source_ref: None,
        extra: Extra::new(),
    })?;
    Ok(function)
}

/// `connection.draw` (`Connect`, doc 06 §3.5, §4.4): one explicit connection.
///
/// - Each end is a CP, a junction or a point on an existing connection. A point on connection W (on its
///   route, if W has one; W must have a page) creates a junction `{page: W.page, at, style: dot, wiring:
///   style default}` there and splits W at it (`normalise::split_connection`: W′ keeps W's ID and ends at
///   the junction, W″ starts there); the junction's sequence is `[W′, W″, new connection]`.
/// - The new connection is `{a, b, page, route, kind}`; its number stays unassigned. An end at an
///   existing junction appends the new connection to that junction's sequence.
/// - `page` defaults to the page of a split connection, else none (a page-less bridge).
/// - On a page, the geometry must be on the page's connection grid (D3, doc 06 §5.7,
///   `geometry::require_route`): a split point, every route vertex, and route ends that meet their CPs
///   exactly.
/// - Rejected: both ends the same endpoint (CI-01); both ends on the same connection; a named record
///   that does not exist (`NotFound`); off-grid geometry. Outcome: the new connection.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Connect {
    /// First endpoint.
    pub a: EndpointParam,
    /// Second endpoint.
    pub b: EndpointParam,
    /// The page it is drawn on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub page: Option<Id<Page>>,
    /// Orthogonal route in nm.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<Pt2S>")]
    pub route: Vec<Pt2>,
    /// The connection kind; default a wire.
    #[serde(default)]
    pub kind: ConnectionKind,
}

impl Command for Connect {
    const NAME: &'static str = "connection.draw";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if let Some(page) = self.page {
            tx.require::<Page>(page)?;
        }
        self.a.check(tx)?;
        self.b.check(tx)?;
        if let (Some(a), Some(b)) = (self.a.endpoint(), self.b.endpoint()) {
            if a == b {
                return tx.reject(text!("refusal-connection-same-end"));
            }
        }
        if let (
            EndpointParam::OnConnection { connection: x, .. },
            EndpointParam::OnConnection { connection: y, .. },
        ) = (&self.a, &self.b)
        {
            if x == y {
                return tx.reject(text!("refusal-connection-same-connection"));
            }
        }
        refuse_fluid_mismatch(tx, &self.kind, &[&self.a, &self.b])?;
        let new_id: Id<Connection> = tx.mint();
        let mut split_page = None;
        let a = resolve_end(tx, &self.a, "/a", new_id, &mut split_page)?;
        let b = resolve_end(tx, &self.b, "/b", new_id, &mut split_page)?;
        if let Some(page) = self.page.or(split_page) {
            require_route(tx, page, &a, &b, &self.route, "/route")?;
        }
        let key = (Kind::Connection, new_id.cast());
        tx.insert(Connection {
            id: new_id,
            v: 1,
            a,
            b,
            page: self.page.or(split_page),
            route: self.route.clone(),
            kind: self.kind.clone(),
            overrides: WireOverrides::default(),
            number: NumberSpec::default(),
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary(key))
    }
}

/// The fluid class of CP `idx` of function `f` in the transaction (`ew_model::fluid_port_class`, the rule of
/// EW-INV-024).
pub(crate) fn cp_fluid_class(tx: &TxBuilder<'_>, f: Id<Function>, idx: u16) -> FluidPortClass {
    let Some(function) = tx.get::<Function>(f) else {
        return FluidPortClass::Other;
    };
    let def = tx.get::<LibCopy<FnDef>>(function.def);
    let parts: Vec<Arc<LibCopy<ew_schema::Part>>> = function
        .device
        .and_then(|d| tx.get::<Device>(d))
        .map(|d| {
            d.parts
                .iter()
                .filter_map(|u| tx.get::<LibCopy<ew_schema::Part>>(u.part.copy))
                .collect()
        })
        .unwrap_or_default();
    let parts: Vec<&ew_schema::Part> = parts.iter().map(|p| &p.item).collect();
    fluid_port_class(&function, def.as_ref().map(|d| &d.item), &parts, idx)
}

/// What one end of a drawn connection is to WP-S1.20 §5 T1 to T3: whether it is fluid (`None`: neither, as an exempt
/// CP, a junction without connections or an overview connection), and the fluid ports of its medium it reaches.
struct FluidEnd {
    fluid: Option<bool>,
    media: Vec<(ew_model::FluidMedium, Id<Function>, u16)>,
}

/// The fluid ports the fluid lines at the junctions `todo` reach, across any number of junctions (EW-INV-024's walk).
fn reached_media(
    tx: &TxBuilder<'_>,
    mut todo: Vec<Id<Junction>>,
    media: &mut Vec<(ew_model::FluidMedium, Id<Function>, u16)>,
) {
    let mut seen: std::collections::BTreeSet<Id<Junction>> = std::collections::BTreeSet::new();
    while let Some(j) = todo.pop() {
        if !seen.insert(j) {
            continue;
        }
        for c in tx.connections_at(EndpointKey::Junction(j)) {
            let Some(c) = tx.get::<Connection>(c) else {
                continue;
            };
            if !matches!(c.kind, ConnectionKind::Fluid(_)) {
                continue;
            }
            for e in [&c.a, &c.b] {
                match e {
                    Endpoint::Cp(f, idx) => {
                        if let FluidPortClass::Fluid(m) = cp_fluid_class(tx, *f, *idx) {
                            media.push((m, *f, *idx));
                        }
                    }
                    Endpoint::Junction(k) => todo.push(*k),
                    Endpoint::Unknown(_) => {}
                }
            }
        }
    }
}

/// The [`FluidEnd`] of `end`.
fn fluid_end(tx: &TxBuilder<'_>, end: &EndpointParam) -> FluidEnd {
    let judged = |c: &Connection| match c.kind {
        ConnectionKind::Overview | ConnectionKind::Unknown(_) => None,
        ConnectionKind::Fluid(_) => Some(true),
        _ => Some(false),
    };
    let mut media = Vec::new();
    let fluid = match end {
        EndpointParam::Cp { function, idx } => match cp_fluid_class(tx, *function, *idx) {
            FluidPortClass::Fluid(m) => {
                media.push((m, *function, *idx));
                Some(true)
            }
            FluidPortClass::Other => Some(false),
            FluidPortClass::Exempt => None,
        },
        EndpointParam::Junction { junction } => {
            let kinds: Vec<Option<bool>> = tx
                .connections_at(EndpointKey::Junction(*junction))
                .into_iter()
                .filter_map(|c| tx.get::<Connection>(c))
                .map(|c| judged(&c))
                .collect();
            reached_media(tx, vec![*junction], &mut media);
            kinds.iter().copied().max().flatten()
        }
        EndpointParam::OnConnection { connection, .. } => {
            let c = tx.get::<Connection>(*connection);
            let fluid = c.as_deref().and_then(judged);
            if let (Some(true), Some(c)) = (fluid, &c) {
                let mut junctions = Vec::new();
                for e in [&c.a, &c.b] {
                    match e {
                        Endpoint::Cp(f, idx) => {
                            if let FluidPortClass::Fluid(m) = cp_fluid_class(tx, *f, *idx) {
                                media.push((m, *f, *idx));
                            }
                        }
                        Endpoint::Junction(j) => junctions.push(*j),
                        Endpoint::Unknown(_) => {}
                    }
                }
                reached_media(tx, junctions, &mut media);
            }
            fluid
        }
    };
    FluidEnd { fluid, media }
}

/// A connection in words: a fluid line by its page ("the fluid line on page 203"), another as [`crate::words::wire`].
fn line_words(tx: &TxBuilder<'_>, id: Id<Connection>) -> Text {
    match tx.get::<Connection>(id) {
        Some(c) if matches!(c.kind, ConnectionKind::Fluid(_)) => match c.page {
            Some(p) => text!("refusal-connection-fluid-line-on", crate::words::page(tx, p)),
            None => text!("refusal-connection-fluid-line-any"),
        },
        _ => crate::words::wire(tx, id),
    }
}

/// The refusals of WP-S1.20 §5 T1 to T3 before EW-INV-024 judges the drawn connection: a fluid line to an end that is
/// no fluid port, another kind (except an overview connection or a kind of a newer format) to a fluid end, and a fluid
/// line between a hydraulic and a pneumatic or vacuum port. Every end kind is judged in words (FT-1321): a CP, a
/// junction (by the connections at it) and a point on a connection (by its kind); the media are those the fluid lines
/// reach across any number of junctions.
fn refuse_fluid_mismatch(
    tx: &TxBuilder<'_>,
    kind: &ConnectionKind,
    ends: &[&EndpointParam],
) -> Result<(), CmdError> {
    let judged: Vec<(&EndpointParam, FluidEnd)> = ends.iter().map(|e| (*e, fluid_end(tx, e))).collect();
    match kind {
        ConnectionKind::Overview | ConnectionKind::Unknown(_) => Ok(()),
        ConnectionKind::Fluid(_) => {
            if let Some((end, _)) = judged.iter().find(|(_, j)| j.fluid == Some(false)) {
                let what = match end {
                    EndpointParam::Cp { function, idx } => {
                        text!(
                            "refusal-connection-not-fluid-port",
                            crate::words::cp(tx, *function, *idx)
                        )
                    }
                    EndpointParam::Junction { junction } => {
                        text!(
                            "refusal-connection-junction-joins-wires",
                            crate::words::junction(tx, *junction)
                        )
                    }
                    EndpointParam::OnConnection { connection, .. } => {
                        text!("refusal-connection-no-fluid-line", line_words(tx, *connection))
                    }
                };
                return tx.reject(text!("refusal-connection-fluid-ports-only", what));
            }
            let media: Vec<&(ew_model::FluidMedium, Id<Function>, u16)> =
                judged.iter().flat_map(|(_, j)| &j.media).collect();
            if let Some(((_, fa, ia), (_, fb, ib))) = media
                .iter()
                .find_map(|a| media.iter().find(|b| b.0 != a.0).map(|b| (*a, *b)))
            {
                return tx.reject(text!(
                    "refusal-connection-fluid-media",
                    crate::words::cp(tx, *fa, *ia),
                    crate::words::cp(tx, *fb, *ib)
                ));
            }
            Ok(())
        }
        _ => match judged.iter().find(|(_, j)| j.fluid == Some(true)) {
            Some((EndpointParam::Cp { function, idx }, _)) => tx.reject(text!(
                "refusal-connection-fluid-port",
                crate::words::cp(tx, *function, *idx)
            )),
            Some((EndpointParam::Junction { junction }, _)) => tx.reject(text!(
                "refusal-connection-fluid-junction",
                crate::words::junction(tx, *junction)
            )),
            Some((EndpointParam::OnConnection { connection, .. }, _)) => tx.reject(text!(
                "refusal-connection-fluid-branch",
                line_words(tx, *connection)
            )),
            None => Ok(()),
        },
    }
}

/// The most entries one `connection.draw_many` takes.
pub const DRAW_MANY_MAX: usize = 10_000;

/// `connection.draw_many` (`ConnectMany`, cycle 33): several [`Connect`]s, in the given order, in **one**
/// transaction (one undo step). Edit ▸ Bridge terminals draws the chain of bridges over three or more terminals of
/// a strip with it (BR1 = 1-2, 2-3, 3-4).
///
/// Each entry is applied exactly as `connection.draw` with those parameters would be, in order, on the records the
/// earlier entries left. Rejected with [`CmdError::InvalidParams`] at `/connections`: no entry, or more than
/// 10,000; an entry's own parameter error keeps its message with its JSON Pointer prefixed by `/connections/<i>`,
/// and its refusal keeps its reason prefixed by `entry <i>: `. Every other error passes through unprefixed:
/// [`CmdError::NotFound`] carries only the missing record's key (which names the record, not the entry), and a model
/// or prevention error is the transaction's, not one entry's. Cost: `Bulk` above 500 entries (`BULK_ABOVE`), else
/// interactive. Outcome: the first new connection.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectMany {
    /// The connections, each the parameters of one `connection.draw` (1 to 10,000).
    pub connections: Vec<Connect>,
}

impl Command for ConnectMany {
    const NAME: &'static str = "connection.draw_many";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, _model: &ew_model::Model) -> crate::meta::Cost {
        if self.connections.len() > crate::commands::BULK_ABOVE {
            crate::meta::Cost::Bulk
        } else {
            crate::meta::Cost::Interactive
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.connections.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/connections",
                text!("refusal-params-no-connection"),
            ));
        }
        if self.connections.len() > DRAW_MANY_MAX {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/connections",
                text!(
                    "refusal-connection-draw-many-too-many",
                    self.connections.len().to_string(),
                    DRAW_MANY_MAX.to_string()
                ),
            ));
        }
        let mut first = None;
        for (i, c) in self.connections.iter().enumerate() {
            let out = c.run(tx).map_err(|err| match err {
                CmdError::InvalidParams { path, text, .. } => {
                    CmdError::invalid_params(Self::NAME, format!("/connections/{i}{path}"), *text)
                }
                CmdError::Rejected { source, reason, text } => {
                    let why = text.map_or_else(|| TextArg::from(reason), TextArg::from);
                    CmdError::rejected(
                        source,
                        text!("refusal-connection-draw-many-entry", i.to_string(), why),
                    )
                }
                // NotFound names the missing record by its key; it has no field an index could prefix (see above).
                other => other,
            })?;
            first = first.or(out.primary);
        }
        Ok(Outcome {
            primary: first,
            ..Outcome::default()
        })
    }
}

/// `connection.route`: replaces a connection's route (FR-CON-007). Connectivity is unchanged. On a page
/// the route must be on the page's connection grid and its ends must meet their CPs (D3,
/// `geometry::require_route`). Outcome: the connection. Coalesce route drags with a key.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteConnection {
    /// The connection.
    #[schemars(with = "UuidS")]
    pub connection: Id<Connection>,
    /// The new route in nm.
    #[schemars(with = "Vec<Pt2S>")]
    pub route: Vec<Pt2>,
}

impl Command for RouteConnection {
    const NAME: &'static str = "connection.route";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let c = tx.require::<Connection>(self.connection)?;
        if let Some(page) = c.page {
            require_route(tx, page, &c.a, &c.b, &self.route, "/route")?;
        }
        tx.update::<Connection>(self.connection, |c| c.route = self.route.clone())?;
        Ok(Outcome::primary((Kind::Connection, self.connection.cast())))
    }
}

/// `connection.delete`: removes connections through the delete policies (they leave junction sequences;
/// bonds lose their `via`). A junction left with two or fewer connections is cleaned up by normaliser #8.
/// Every connection must exist. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteConnections {
    /// The connections, at least one.
    #[schemars(with = "Vec<UuidS>")]
    pub connections: Vec<Id<Connection>>,
}

impl Command for DeleteConnections {
    const NAME: &'static str = "connection.delete";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.connections.is_empty() {
            return tx.reject(text!("refusal-connection-delete-none"));
        }
        for c in &self.connections {
            tx.require::<Connection>(*c)?;
        }
        let keys: Vec<ew_model::AnyKey> = self
            .connections
            .iter()
            .map(|c| (Kind::Connection, c.cast()))
            .collect();
        tx.delete_many(&keys)?;
        Ok(Outcome::default())
    }
}

/// The wiring choice of `junction.edit`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WiringChoice {
    /// Star: the first member is wired to every other.
    Star,
    /// Chain: consecutive members are wired.
    Chain,
    /// The style's default (Dot: star, Bevel: chain): the stored member is cleared.
    StyleDefault,
}

/// `junction.edit`: style, wiring and sequence of a junction (FR-CON-005, doc 06 §3.5).
///
/// Each given member replaces the stored one; `sequence` must be a permutation of the connections ending
/// at the junction (else rejected). It takes no coordinates, so the grid rule of D3 has nothing to check
/// here: a junction's position is set (on the grid) by `connection.draw`. Outcome: the junction. Coalesce
/// consecutive reorders with a key.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditJunction {
    /// The junction.
    #[schemars(with = "UuidS")]
    pub junction: Id<Junction>,
    /// A new style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<JunctionStyle>,
    /// A new wiring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wiring: Option<WiringChoice>,
    /// A new wiring order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<UuidS>>")]
    pub sequence: Option<Vec<Id<Connection>>>,
}

impl Command for EditJunction {
    const NAME: &'static str = "junction.edit";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let junction = tx.require::<Junction>(self.junction)?;
        if let Some(seq) = &self.sequence {
            let mut given = seq.clone();
            given.sort();
            let members = tx.connections_at(EndpointKey::Junction(self.junction));
            if given != members {
                return tx.reject(text!("refusal-junction-sequence"));
            }
        }
        let mut next = (*junction).clone();
        if let Some(style) = &self.style {
            next.style = style.clone();
        }
        match self.wiring {
            Some(WiringChoice::Star) => next.wiring = Some(JunctionWiring::Star),
            Some(WiringChoice::Chain) => next.wiring = Some(JunctionWiring::Chain),
            Some(WiringChoice::StyleDefault) => next.wiring = None,
            None => {}
        }
        if let Some(seq) = &self.sequence {
            next.sequence = seq.clone();
        }
        tx.put(next);
        Ok(Outcome::primary((Kind::Junction, self.junction.cast())))
    }
}

/// `ip.place`: an interruption point (FR-XRF-006 … FR-XRF-014, doc 03 §10): the IP function and its one
/// representation in one transaction (EW-INV-013).
///
/// Creates `Function {device: none, def, role: main, kind: ip {name, direction, star_source, sort,
/// external}}` and `Representation {functions: [f], page, symbol, pos, orient, rep_type: multi_line}`.
/// `def` is `fndef` or the copy the symbol's `fndef` resolves to (must be embedded). The page, the symbol
/// copy and an external reference must exist. The pairing scope is the project's `ip_pairing` setting, not
/// a record field. `copies` (FT-673): library documents (`lib/` JSON form) embedded first, in the same transaction,
/// as `device.place` embeds its `copies` (checked as `library.embed` checks them, at `/copies/i`), so placing a starter
/// IP symbol is one undo step and replays without a library. A star source is a source point and an external point
/// takes no star (ID-1146): a starred point with another direction, and a starred external point, are refused
/// before anything is made. Outcome: the IP function.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceIp {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The IP symbol copy.
    #[schemars(with = "UuidS")]
    pub symbol: Id<SymbolDef>,
    /// The FnDef copy; default the symbol's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub fndef: Option<Id<FnDef>>,
    /// The insertion point.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// The orientation.
    #[serde(default)]
    #[schemars(with = "Orient2S")]
    pub orient: Orient2,
    /// The pairing name ("L-1").
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// Signal direction.
    pub direction: IpDirection,
    /// The star source of its name.
    #[serde(default)]
    pub star_source: bool,
    /// Chain order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<i32>,
    /// An external target (an external connection point).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub external: Option<Id<ExternalRef>>,
    /// The library documents to embed with the placement (`lib/` JSON form; FT-673): the IP symbol and its FnDef as
    /// the caller resolved them; empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

impl Command for PlaceIp {
    const NAME: &'static str = "ip.place";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        // FT-673: the copies first, so that the symbol and its FnDef resolve below.
        embed_closure(Self::NAME, tx, &self.copies)?;
        tx.require::<Page>(self.page)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.symbol)?;
        require_on_grid(tx, self.page, self.pos, "/pos")?;
        if let Some(external) = self.external {
            tx.require::<ExternalRef>(external)?;
        }
        if let Some(problem) = star_problem(&self.direction, self.star_source, self.external.is_some()) {
            return tx.reject(problem);
        }
        let def = self.fndef.unwrap_or_else(|| {
            ew_ids::v5::lib_copy_id(
                symbol.item.fndef.library.as_uuid(),
                symbol.item.fndef.item.as_uuid(),
            )
        });
        tx.require::<LibCopy<FnDef>>(def)?;
        let spec = IpSpec {
            name: self.name.clone(),
            direction: self.direction.clone(),
            star_source: self.star_source,
            sort: self.sort,
            external: self.external,
            extra: Extra::new(),
        };
        let function = new_ip(
            tx,
            def,
            spec,
            self.page,
            self.symbol,
            self.pos,
            self.orient,
            RepType::MultiLine,
        )?;
        Ok(Outcome::primary((Kind::Function, function.cast())))
    }
}

/// `ip.continue`: the partner of an interruption point on another page (FR-XRF-014).
///
/// Creates an IP function with the same name, def, `sort` and no `external`, the direction reversed
/// (source ↔ target; undirected stays), `star_source: false`, and its representation on `page` with the
/// original representation's symbol, orientation and rep type, at `pos` (default: the original's position).
/// Rejected: `ip` is not an interruption point; `page` is the original's page; an external connection
/// point (it never pairs). Outcome: the new IP function.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContinueIp {
    /// The interruption point to continue.
    #[schemars(with = "UuidS")]
    pub ip: Id<Function>,
    /// The target page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The insertion point on the target page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Pt2S>")]
    pub pos: Option<Pt2>,
}

impl Command for ContinueIp {
    const NAME: &'static str = "ip.continue";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let original = tx.require::<Function>(self.ip)?;
        let FunctionKind::Ip(spec) = &original.kind else {
            return tx.reject(text!("refusal-ip-continue-not-ip"));
        };
        if spec.external.is_some() {
            return tx.reject(text!("refusal-ip-continue-external"));
        }
        tx.require::<Page>(self.page)?;
        let reps: Vec<Arc<Representation>> = tx
            .reps_of_function(self.ip)
            .into_iter()
            .filter_map(|r| tx.get::<Representation>(r))
            .collect();
        if reps.iter().any(|r| r.page == self.page) {
            return tx.reject(text!("refusal-ip-continue-same-page"));
        }
        let Some(rep) = reps.first() else {
            return tx.reject(text!("refusal-ip-continue-no-rep"));
        };
        let direction = match &spec.direction {
            IpDirection::Source => IpDirection::Target,
            IpDirection::Target => IpDirection::Source,
            other => other.clone(),
        };
        let pos = self.pos.unwrap_or(rep.pos);
        require_on_grid(tx, self.page, pos, "/pos")?;
        let partner = IpSpec {
            name: spec.name.clone(),
            direction,
            star_source: false,
            sort: spec.sort,
            external: None,
            extra: Extra::new(),
        };
        let function = new_ip(
            tx,
            original.def,
            partner,
            self.page,
            rep.symbol,
            pos,
            rep.orient,
            rep.rep_type.clone(),
        )?;
        Ok(Outcome::primary((Kind::Function, function.cast())))
    }
}

/// `potential.define`: names and types a potential at a CP or a junction (FR-CON-010).
///
/// Creates `PotentialDef {at, name, ptype, voltage, env, circuit_class, props}`. `at` must be a CP of an
/// existing function or an existing junction (a point on a connection is rejected), and must not hold a definition
/// already: another point of the potential may (doc 08 §3.4). Outcome: the definition.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DefinePotential {
    /// The definition point.
    pub at: EndpointParam,
    /// The potential name.
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The type; absent means naming only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ptype: Option<PotentialType>,
    /// Voltage (V).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub voltage: Option<Dec3>,
    /// AC/DC, frequency and earthing.
    #[serde(default, skip_serializing_if = "PotentialEnv::is_empty")]
    pub env: PotentialEnv,
    /// Circuit class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circuit_class: Option<CircuitClass>,
    /// Wire properties at the potential level.
    #[serde(default, skip_serializing_if = "WireProps::is_empty")]
    pub props: WireProps,
}

impl Command for DefinePotential {
    const NAME: &'static str = "potential.define";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(at) = self.at.endpoint() else {
            return tx.reject(text!("refusal-potential-define-on-connection"));
        };
        self.at.check(tx)?;
        // A potential may hold several definitions (doc 08 §3.4, §3.5: EW-POT-017 when their names differ), but one
        // point defines it once (decision log ID-1312).
        let existing = EndpointKey::of(&at)
            .and_then(|key| tx.potential_defs_at(key).into_iter().min())
            .and_then(|d| tx.get::<PotentialDef>(d));
        if let Some(existing) = existing {
            return tx.reject(text!("refusal-potential-define-twice", existing.name.to_string()));
        }
        let id: Id<PotentialDef> = tx.mint();
        tx.insert(PotentialDef {
            id,
            v: 1,
            at,
            name: self.name.clone(),
            ptype: self.ptype.clone(),
            voltage: self.voltage,
            env: self.env.clone(),
            circuit_class: self.circuit_class.clone(),
            props: self.props.clone(),
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::PotentialDef, id.cast())))
    }
}

/// `bond.create` (`CreateBond`): declares a bond, a link of two potentials that does not merge them
/// (doc 03 §9.6, doc 08 §3.7; FR-CON-014, the "declare bond" quick fix of EW-POT-001).
///
/// Creates `Bond {a, b, kind, via}`. `a` and `b` must each be a CP of an existing function or an existing
/// junction (a point on a connection is rejected) and must differ; `via`, the connection that realises the
/// bond (W27 for PE-0VDC of Golden A), must exist when given. The bond's label (`B1`) is derived. Whether
/// the two ends lie on different potentials, and the PELV rule of exactly one 0 V-PE bond, are checks
/// (EW-POT-001, EW-POT-002), not refusals. Outcome: the bond.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateBond {
    /// First end.
    pub a: EndpointParam,
    /// Second end.
    pub b: EndpointParam,
    /// The bond kind.
    pub kind: BondKind,
    /// The connection that realises the bond.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub via: Option<Id<Connection>>,
}

impl Command for CreateBond {
    const NAME: &'static str = "bond.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let (Some(a), Some(b)) = (self.a.endpoint(), self.b.endpoint()) else {
            return tx.reject(text!("refusal-bond-on-connection"));
        };
        if a == b {
            return tx.reject(text!("refusal-bond-same-end"));
        }
        if matches!(self.kind, BondKind::Unknown(_)) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/kind",
                text!("refusal-bond-unknown-kind"),
            ));
        }
        self.a.check(tx)?;
        self.b.check(tx)?;
        if let Some(via) = self.via {
            tx.require::<Connection>(via)?;
        }
        let id: Id<Bond> = tx.mint();
        tx.insert(Bond {
            id,
            v: 1,
            a,
            b,
            kind: self.kind.clone(),
            via: self.via,
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Bond, id.cast())))
    }
}

// ---------------------------------------------------------------------------------------------------------
// WP-S2.04 part 3: a wire ending in a new interruption point, and the arrangement of interruption points

/// `false`, for `skip_serializing_if`.
fn is_false(b: &bool) -> bool {
    !*b
}

/// The default orientation, for `skip_serializing_if`.
fn is_r0(o: &Orient2) -> bool {
    o.is_default()
}

/// The refusal of an interruption point's star (ID-1146, ID-1038's ruling (a)): a star source is a source point, and
/// an external point never pairs, so it takes no star. `None` for an unstarred point and a starred source; the
/// external refusal first, since clearing the star answers both.
pub(crate) fn star_problem(direction: &IpDirection, star_source: bool, external: bool) -> Option<Text> {
    if !star_source {
        None
    } else if external {
        Some(text!("refusal-ip-star-external"))
    } else if *direction != IpDirection::Source {
        Some(text!("refusal-ip-star-not-source"))
    } else {
        None
    }
}

/// The interruption point that [`DrawToIp`] creates at the end of its wire: `ip.place`'s members without `page`
/// (the wire's).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IpAtEnd {
    /// The IP symbol copy.
    #[schemars(with = "UuidS")]
    pub symbol: Id<SymbolDef>,
    /// The FnDef copy; default the symbol's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub fndef: Option<Id<FnDef>>,
    /// The insertion point.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// The orientation.
    #[serde(default, skip_serializing_if = "is_r0")]
    #[schemars(with = "Orient2S")]
    pub orient: Orient2,
    /// The pairing name ("24VDC-2").
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// Signal direction.
    pub direction: IpDirection,
    /// The star source of its name.
    #[serde(default, skip_serializing_if = "is_false")]
    pub star_source: bool,
    /// Chain order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<i32>,
    /// An external target (an external connection point; FT-946: one dropped on a pin).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub external: Option<Id<ExternalRef>>,
}

/// `connection.draw_to_ip` (`DrawToIp`, doc 06 §4.4 "P", WP-S2.04 §5.1, §9.2): a wire that ends in a new
/// interruption point, in one transaction and one undo step.
///
/// - First the IP function and its representation exactly as `ip.place` creates them (`ip` holds `ip.place`'s
///   members; the page is the wire's): `Function {device: none, def, role: main, kind: ip {name, direction,
///   star_source, sort, external}}` and `Representation {functions: [f], page, symbol, pos, orient,
///   rep_type: multi_line}`; `def` is `ip.fndef` or the copy the symbol's `fndef` resolves to (must be embedded),
///   and its FnDef must have exactly one connection point, which the symbol must draw.
/// - Then the connection from `a` to that connection point exactly as `connection.draw` creates it: `a` is a CP, a
///   junction or a point on an existing connection (which splits it and makes a junction there); the connection is
///   `{a, b: the IP's CP, page, route, kind}` with its number unassigned; `route`, when given, is on the page's
///   connection grid and meets both CPs exactly (`geometry::require_route`), so it ends at the IP's CP.
///
/// Nothing derived is read inside the transaction (the name and the direction are the tool's). Rejected: as
/// `ip.place` (a missing page, symbol copy or external reference, a FnDef that is not embedded, `ip.pos` off the
/// grid, a starred point that is no source or is external, ID-1146), as `connection.draw` (a missing endpoint record,
/// off-grid geometry, a route that does not meet its CPs) and an `a` end that no representation of `page` draws; an IP
/// symbol whose FnDef has not exactly one CP, or that does not draw it; a fluid line, and another kind from a fluid
/// end, in words (WP-S1.20 §5 T1, T2; FT-1322).
/// `copies` (FT-673): library documents (`lib/` JSON form) embedded first, in the same transaction, as `ip.place`
/// embeds them, so ending a wire in a starter IP symbol is one undo step. Interactive. Outcome: the connection.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawToIp {
    /// The wire's first endpoint.
    pub a: EndpointParam,
    /// The page the wire and the interruption point are drawn on.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// Orthogonal route in nm, ending at the IP's connection point.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<Pt2S>")]
    pub route: Vec<Pt2>,
    /// The connection kind; default a wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ConnectionKind>,
    /// The interruption point at the wire's end.
    pub ip: IpAtEnd,
    /// The library documents to embed with the wire (`lib/` JSON form; FT-673): the IP symbol and its FnDef as the
    /// caller resolved them; empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

impl Command for DrawToIp {
    const NAME: &'static str = "connection.draw_to_ip";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        // FT-673: the copies first, so that the IP symbol and its FnDef resolve below.
        embed_closure(Self::NAME, tx, &self.copies)?;
        tx.require::<Page>(self.page)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.ip.symbol)?;
        require_on_grid(tx, self.page, self.ip.pos, "/ip/pos")?;
        if let Some(external) = self.ip.external {
            tx.require::<ExternalRef>(external)?;
        }
        if let Some(problem) = star_problem(
            &self.ip.direction,
            self.ip.star_source,
            self.ip.external.is_some(),
        ) {
            return tx.reject(problem);
        }
        let def = self.ip.fndef.unwrap_or_else(|| {
            ew_ids::v5::lib_copy_id(
                symbol.item.fndef.library.as_uuid(),
                symbol.item.fndef.item.as_uuid(),
            )
        });
        let fndef = tx.require::<LibCopy<FnDef>>(def)?;
        let idx = match fndef.item.cps.as_slice() {
            [cp] => cp.idx,
            other => {
                return tx.reject(text!("refusal-draw-to-ip-cp-count", other.len().to_string()));
            }
        };
        self.a.check(tx)?;
        // FT-1322: an interruption point's CP is no fluid port (T1), and its start is judged as `connection.draw`
        // judges an end (T2), in words before EW-INV-024 would judge it.
        let kind = self.kind.clone().unwrap_or_default();
        if matches!(kind, ConnectionKind::Fluid(_)) {
            return tx.reject(text!("refusal-draw-to-ip-fluid"));
        }
        refuse_fluid_mismatch(tx, &kind, &[&self.a])?;
        // FT-446: the start must be drawn on the page the wire and the IP are drawn on; refuse another page (or none)
        // before anything is inserted.
        let (drawn, other) = match &self.a {
            EndpointParam::Cp { function, .. } => {
                // A representation draws the CP when it lists the function; a terminal's CP is also drawn by its
                // strip's segment, which lists a function of the strip device.
                let strip = tx
                    .get::<Function>(*function)
                    .and_then(|f| f.device)
                    .and_then(|d| tx.get::<Device>(d))
                    .and_then(|d| d.aspects.terminal.as_ref().map(|t| t.strip));
                let mut reps = Vec::new();
                for f in std::iter::once(*function)
                    .chain(strip.into_iter().flat_map(|s| tx.functions_of_device(s)))
                {
                    reps.extend(
                        tx.reps_of_function(f)
                            .into_iter()
                            .filter_map(|r| tx.get::<Representation>(r))
                            .filter(|rep| rep.functions.contains(&f)),
                    );
                }
                (
                    reps.iter().any(|rep| rep.page == self.page),
                    reps.iter().find(|rep| rep.page != self.page).map(|rep| rep.page),
                )
            }
            EndpointParam::Junction { junction } => {
                let page = tx.require::<Junction>(*junction)?.page;
                (page == self.page, Some(page))
            }
            EndpointParam::OnConnection { connection, .. } => {
                let page = tx.require::<Connection>(*connection)?.page;
                (page == Some(self.page), page)
            }
        };
        if !drawn {
            let page = self.page;
            return match other {
                Some(other) => tx.reject(text!(
                    "refusal-connection-start-on-other-page",
                    crate::words::page(tx, other),
                    crate::words::page(tx, page)
                )),
                None => tx.reject(text!(
                    "refusal-draw-to-ip-start-on-no-page",
                    crate::words::page(tx, page)
                )),
            };
        }
        let spec = IpSpec {
            name: self.ip.name.clone(),
            direction: self.ip.direction.clone(),
            star_source: self.ip.star_source,
            sort: self.ip.sort,
            external: self.ip.external,
            extra: Extra::new(),
        };
        let function = new_ip(
            tx,
            def,
            spec,
            self.page,
            self.ip.symbol,
            self.ip.pos,
            self.ip.orient,
            RepType::MultiLine,
        )?;
        if crate::geometry::cp_position(tx, function, idx, self.page).is_none() {
            return tx.reject(text!("refusal-draw-to-ip-symbol-cp"));
        }
        let new_id: Id<Connection> = tx.mint();
        let mut split_page = None;
        let a = resolve_end(tx, &self.a, "/a", new_id, &mut split_page)?;
        let b = Endpoint::Cp(function, idx);
        require_route(tx, self.page, &a, &b, &self.route, "/route")?;
        let key = (Kind::Connection, new_id.cast());
        tx.insert(Connection {
            id: new_id,
            v: 1,
            a,
            b,
            page: Some(self.page),
            route: self.route.clone(),
            kind,
            overrides: WireOverrides::default(),
            number: NumberSpec::default(),
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary(key))
    }
}

/// `ip.arrange` (`ArrangeIps`, FR-XRF-013, WP-S2.04 §5.1, D12): interruption points of one page exchanged into a new
/// order.
///
/// - `ips` are IP functions in the new order: at least two, none twice, each with exactly one representation, all
///   on one page. The slots are the current positions of their representations sorted by `(y, x)` (ties by the
///   representation ID); slot `i` goes to `ips[i]` (its representation's `pos`; orientation, symbol and type kept).
/// - Routes of the connections on that page at the moved IPs' CPs follow each CP's displacement
///   (`geometry::follow_end`).
/// - With `set_sort`, the `sort` of `ips[i]` becomes `i + 1` (the chain order).
///
/// The order is the caller's (the tool sorts by name or by the derived potential name): nothing derived is read
/// inside the transaction (the transaction-read rule, ID-525). Rejected: fewer than two IPs; an IP listed twice; a
/// function that is not an interruption point or that has not exactly one representation; representations on
/// different pages; a missing function. Interactive. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArrangeIps {
    /// The IP functions in their new order.
    #[schemars(with = "Vec<UuidS>")]
    pub ips: Vec<Id<Function>>,
    /// Whether the chain order (`sort`) follows the new order.
    #[serde(default, skip_serializing_if = "is_false")]
    pub set_sort: bool,
}

impl Command for ArrangeIps {
    const NAME: &'static str = "ip.arrange";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.ips.len() < 2 {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/ips",
                text!("refusal-arrange-few-ips"),
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (i, f) in self.ips.iter().enumerate() {
            if !seen.insert(*f) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/ips/{i}"),
                    text!("refusal-params-ip-twice"),
                ));
            }
        }
        let mut reps: Vec<Arc<Representation>> = Vec::with_capacity(self.ips.len());
        for f in &self.ips {
            let function = tx.require::<Function>(*f)?;
            if !matches!(function.kind, FunctionKind::Ip(_)) {
                return tx.reject(text!("refusal-ip-arrange-not-ip", crate::words::function(tx, *f)));
            }
            let ids = tx.reps_of_function(*f);
            let [rid] = ids.as_slice() else {
                return tx.reject(text!(
                    "refusal-ip-arrange-drawn-times",
                    crate::words::interruption_point(tx, *f),
                    ids.len().to_string()
                ));
            };
            reps.push(tx.require::<Representation>(*rid)?);
        }
        let page_id = reps[0].page;
        if reps.iter().any(|r| r.page != page_id) {
            return tx.reject(text!("refusal-ip-arrange-pages"));
        }
        let page = tx.require::<Page>(page_id)?;
        let mut slots: Vec<(Pt2, Id<Representation>)> = reps.iter().map(|r| (r.pos, r.id)).collect();
        slots.sort_by_key(|(p, id)| (p.y, p.x, *id));
        let mut moves: std::collections::BTreeMap<(Id<Function>, u16), Pt2> = Default::default();
        let mut moved: Vec<Representation> = Vec::new();
        for (rep, (slot, _)) in reps.iter().zip(&slots) {
            if rep.pos == *slot {
                continue;
            }
            let mut next = (**rep).clone();
            next.pos = *slot;
            let (Ok(before), Ok(after)) = (
                crate::geometry::rep_cps_checked(tx, rep),
                crate::geometry::rep_cps_checked(tx, &next),
            ) else {
                return tx.reject(text!("refusal-ip-arrange-out-of-range"));
            };
            for a in &after {
                let Some(b) = before.iter().find(|b| b.function == a.function && b.idx == a.idx) else {
                    continue;
                };
                let Ok(d) = crate::geometry::sub(a.pos, b.pos) else {
                    return tx.reject(text!("refusal-ip-arrange-too-far"));
                };
                if d != Pt2::ORIGIN {
                    moves.entry((a.function, a.idx)).or_insert(d);
                }
            }
            moved.push(next);
        }
        follow_moved_cps(
            tx,
            page_id,
            page.grid,
            &moves,
            text!("refusal-move-what-arrangement"),
        )?;
        for r in moved {
            tx.put(r);
        }
        if self.set_sort {
            for (i, f) in self.ips.iter().enumerate() {
                let sort = i32::try_from(i + 1).unwrap_or(i32::MAX);
                tx.update::<Function>(*f, |function| {
                    if let FunctionKind::Ip(spec) = &mut function.kind {
                        spec.sort = Some(sort);
                    }
                })?;
            }
        }
        Ok(Outcome::default())
    }
}

/// The routes of the connections on `page` that end at a moved CP of `moves` (function CP → displacement) follow
/// their ends (`geometry::follow_ends`, both ends at once). `what` names the edit in a refusal.
pub(crate) fn follow_moved_cps(
    tx: &mut TxBuilder<'_>,
    page: Id<Page>,
    grid: ew_units::Grid,
    moves: &std::collections::BTreeMap<(Id<Function>, u16), Pt2>,
    what: Text,
) -> Result<(), CmdError> {
    let functions: std::collections::BTreeSet<Id<Function>> = moves.keys().map(|(f, _)| *f).collect();
    let mut connections: std::collections::BTreeSet<Id<Connection>> = Default::default();
    for f in functions {
        connections.extend(tx.connections_of_function(f));
    }
    for cid in connections {
        let c = tx.require::<Connection>(cid)?;
        if c.page != Some(page) {
            continue;
        }
        let end = |e: &Endpoint| match e {
            Endpoint::Cp(f, idx) => moves.get(&(*f, *idx)).copied(),
            _ => None,
        };
        let (da, db) = (end(&c.a), end(&c.b));
        if da.is_none() && db.is_none() {
            continue;
        }
        let mut route = c.route.clone();
        if crate::geometry::follow_ends(&mut route, grid, da, db).is_err() {
            return tx.reject(text!(
                "refusal-move-route-out-of-range",
                what.clone(),
                crate::words::wire(tx, cid)
            ));
        }
        if route != c.route {
            let mut changed = (*c).clone();
            changed.route = route;
            tx.put(changed);
        }
    }
    Ok(())
}
