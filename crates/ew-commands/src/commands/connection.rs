// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connectivity commands (ADR-0007, doc 06 §3.5): `connection.draw`, `connection.route`,
//! `connection.delete`, `junction.edit`, `ip.place`, `ip.continue`, `potential.define`.

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_model::EndpointKey;
use ew_schema::{
    CircuitClass, Connection, ConnectionKind, Endpoint, ExternalRef, Extra, FnDef, FnRole, Function,
    FunctionKind, IpDirection, IpSpec, Junction, JunctionStyle, JunctionWiring, Kind, LibCopy, NumberSpec,
    Page, PotentialDef, PotentialEnv, PotentialType, RepType, Representation, SymbolDef, WireOverrides,
    WireProps,
};
use ew_units::{Dec3, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::common::EndpointParam;
use crate::error::CmdError;
use crate::geometry::{require_on_grid, require_route};
use crate::meta::CommandMeta;
use crate::normalise::split_connection;
use crate::proxy::{Dec3S, IdentS, Orient2S, Pt2S, UuidS};
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
                return tx.reject(format!(
                    "connection {connection} has no page, so a junction cannot be placed on it; \
                     give it a page first"
                ));
            };
            require_on_grid(tx, page, *at, &format!("{field}/on_connection/at"))?;
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
                return tx.reject("both ends are the same endpoint (CI-01); pick two different ends");
            }
        }
        if let (
            EndpointParam::OnConnection { connection: x, .. },
            EndpointParam::OnConnection { connection: y, .. },
        ) = (&self.a, &self.b)
        {
            if x == y {
                return tx.reject("both ends are on the same connection; pick two different connections");
            }
        }
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
            return tx.reject("no connection given; name at least one connection to delete");
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
                return tx.reject(
                    "`sequence` is not a permutation of the connections ending at the junction; \
                     list each of them exactly once",
                );
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
/// a record field. Outcome: the IP function.
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
}

impl Command for PlaceIp {
    const NAME: &'static str = "ip.place";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        tx.require::<Page>(self.page)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.symbol)?;
        require_on_grid(tx, self.page, self.pos, "/pos")?;
        if let Some(external) = self.external {
            tx.require::<ExternalRef>(external)?;
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
            return tx.reject("the function is not an interruption point; pick an IP function");
        };
        if spec.external.is_some() {
            return tx.reject("an external connection point never pairs; it cannot be continued");
        }
        tx.require::<Page>(self.page)?;
        let reps: Vec<Arc<Representation>> = tx
            .reps_of_function(self.ip)
            .into_iter()
            .filter_map(|r| tx.get::<Representation>(r))
            .collect();
        if reps.iter().any(|r| r.page == self.page) {
            return tx.reject(
                "the target page is the page of the interruption point; continue it on another page",
            );
        }
        let Some(rep) = reps.first() else {
            return tx.reject("the interruption point has no representation to continue");
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
/// existing function or an existing junction (a point on a connection is rejected). Outcome: the
/// definition.
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
            return tx.reject(
                "a potential is defined at a CP or a junction, not on a connection; name one of them",
            );
        };
        self.at.check(tx)?;
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
