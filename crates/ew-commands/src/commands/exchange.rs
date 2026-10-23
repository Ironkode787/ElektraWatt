// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `rep.exchange_symbol` (`ExchangeSymbol`, FR-SYM-022, WP-S2.04 §5.1): the symbol of a placed representation
//! exchanged, its function and connections kept.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{
    Connection, Endpoint, Extra, FnDef, Function, FunctionKind, KeyedList, Kind, LibCopy, Page, PinMapEntry,
    Representation, SymbolDef,
};
use ew_units::Pt2;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::device::check_rep_type;
use crate::error::CmdError;
use crate::geometry::{follow_ends, rep_cps_checked, sub};
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// One entry of an explicit CP map of [`ExchangeSymbol`]: a CP of the new symbol and the function CP it draws.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExchangePin {
    /// The new symbol's CP index (`SymbolCp.idx`).
    pub symbol_cp: u16,
    /// The function CP it draws (the representation's first function).
    pub function_cp: u16,
}

/// `rep.exchange_symbol` (`ExchangeSymbol`, FR-SYM-022): the representation's `symbol` becomes `symbol`, `variant` 0,
/// its orientation, position and functions kept.
///
/// - **The CP map.** With `pin_map`, each entry maps a CP of the new symbol to a CP of the representation's first
///   function (every symbol CP and function CP named must exist, each symbol CP at most once). Without it the map is
///   derived: when the new symbol's FnDef is the function's definition, by index (symbol CP → its `fn_cp`, the
///   stored pin map empty); otherwise by designation: a symbol CP draws the function CP whose designation equals,
///   ASCII case-insensitively, the designation of the CP of the new symbol's FnDef that the symbol CP draws. A
///   symbol CP with no counterpart keeps the index default (`fn_cp`), which must name no CP of the function, so
///   it draws no CP of the function; one whose default names a CP of the function is refused (pass `pin_map`).
///   The same holds for a symbol CP an explicit `pin_map` leaves out (invalid at `/pin_map`).
/// - **Text slots.** Overrides whose slot index the new symbol has are kept; the others are dropped with a notice.
/// - **Routes** of the connections on the representation's page at its CPs follow each CP's displacement
///   (`geometry::follow_end`; a connection with both ends on the representation translates when both ends move
///   alike).
/// - Function CPs drawn before and not drawn now are listed as notices.
///
/// Rejected: a connected function CP (a connection on this page ends there) that the new map does not draw; a
/// representation type the page does not allow; a symbol copy or representation that does not exist; an
/// interruption point's representation. Interactive. Outcome: the representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExchangeSymbol {
    /// The representation.
    #[schemars(with = "UuidS")]
    pub rep: Id<Representation>,
    /// The new symbol copy (embedded).
    #[schemars(with = "UuidS")]
    pub symbol: Id<SymbolDef>,
    /// An explicit CP map; absent derives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_map: Option<Vec<ExchangePin>>,
}

/// The ASCII-folded designation of a CP.
fn fold(s: &str) -> String {
    s.to_ascii_lowercase()
}

impl Command for ExchangeSymbol {
    const NAME: &'static str = "rep.exchange_symbol";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let old = tx.require::<Representation>(self.rep)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.symbol)?;
        let Some(&function_id) = old.functions.first() else {
            return tx.reject("the representation draws no function; there is nothing to keep");
        };
        let function = tx.require::<Function>(function_id)?;
        if matches!(function.kind, FunctionKind::Ip(_)) {
            return tx.reject(
                "an interruption point keeps its interruption-point symbol; its symbol cannot be exchanged",
            );
        }
        let page = tx.require::<Page>(old.page)?;
        check_rep_type(tx, &page, &symbol.item.rep_type)?;
        // The function's CPs by designation (its FnDef's).
        let own: Option<std::sync::Arc<LibCopy<FnDef>>> = tx.get::<LibCopy<FnDef>>(function.def);
        let own_cps: Vec<(u16, String)> = own
            .as_ref()
            .map(|d| {
                d.item
                    .cps
                    .iter()
                    .map(|c| (c.idx, c.designation.as_str().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let new_def: Id<FnDef> = ew_ids::v5::lib_copy_id(
            symbol.item.fndef.library.as_uuid(),
            symbol.item.fndef.item.as_uuid(),
        );
        let symbol_cps: Vec<(u16, u16)> = symbol.item.cps.iter().map(|c| (c.idx, c.fn_cp)).collect();
        let mut pin_map: Vec<PinMapEntry> = Vec::new();
        match &self.pin_map {
            Some(entries) => {
                let mut seen = BTreeSet::new();
                for (i, e) in entries.iter().enumerate() {
                    if !symbol_cps.iter().any(|(s, _)| *s == e.symbol_cp) {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("/pin_map/{i}/symbol_cp"),
                            format!("the new symbol has no connection point {}", e.symbol_cp),
                        ));
                    }
                    if !own_cps.iter().any(|(c, _)| *c == e.function_cp) {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("/pin_map/{i}/function_cp"),
                            format!("the function has no connection point {}", e.function_cp),
                        ));
                    }
                    if !seen.insert(e.symbol_cp) {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("/pin_map/{i}/symbol_cp"),
                            "the symbol CP is mapped twice",
                        ));
                    }
                    pin_map.push(PinMapEntry {
                        symbol_cp: e.symbol_cp,
                        function: function_id,
                        idx: e.function_cp,
                        extra: Extra::new(),
                    });
                }
                // A symbol CP the map leaves out keeps its index default, which must name no CP of the function
                // (the rule of the derived map).
                for (symbol_cp, fn_cp) in &symbol_cps {
                    if !seen.contains(symbol_cp) && own_cps.iter().any(|(c, _)| c == fn_cp) {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            "/pin_map",
                            format!(
                                "connection point {symbol_cp} of the new symbol is not in the map and would draw \
                                 function CP {fn_cp}; map it"
                            ),
                        ));
                    }
                }
            }
            None if new_def == function.def => {}
            None => {
                let new_cps: BTreeMap<u16, String> = tx
                    .get::<LibCopy<FnDef>>(new_def)
                    .map(|d| {
                        d.item
                            .cps
                            .iter()
                            .map(|c| (c.idx, fold(c.designation.as_str())))
                            .collect()
                    })
                    .unwrap_or_default();
                for (symbol_cp, fn_cp) in &symbol_cps {
                    let target = new_cps
                        .get(fn_cp)
                        .and_then(|d| own_cps.iter().find(|(_, o)| fold(o) == *d))
                        .map(|(idx, _)| *idx);
                    match target {
                        Some(idx) => pin_map.push(PinMapEntry {
                            symbol_cp: *symbol_cp,
                            function: function_id,
                            idx,
                            extra: Extra::new(),
                        }),
                        None if own_cps.iter().any(|(c, _)| c == fn_cp) => {
                            return tx.reject(format!(
                                "connection point {symbol_cp} of the new symbol has no counterpart by designation \
                                 and would draw function CP {fn_cp}; give `pin_map`"
                            ));
                        }
                        None => {}
                    }
                }
            }
        }
        let mut next = (*old).clone();
        next.symbol = self.symbol;
        next.variant = 0;
        next.pin_map = KeyedList::from_vec(pin_map)
            .map_err(|e| CmdError::internal(format!("rep.exchange_symbol: pin map ({e})")))?;
        let mut notices = Vec::new();
        let slots = u16::try_from(symbol.item.text_slots.len()).unwrap_or(u16::MAX);
        let (kept, dropped): (Vec<_>, Vec<_>) = old.text_slots.iter().cloned().partition(|o| o.slot < slots);
        for o in &dropped {
            notices.push(format!(
                "the override of text slot {} is dropped: the new symbol has no such slot",
                o.slot
            ));
        }
        next.text_slots = KeyedList::from_vec(kept)
            .map_err(|e| CmdError::internal(format!("rep.exchange_symbol: text slots ({e})")))?;
        let (Ok(before), Ok(after)) = (rep_cps_checked(tx, &old), rep_cps_checked(tx, &next)) else {
            return tx.reject(
                "the new symbol puts a connection point outside the drawable range; place the representation \
                 nearer the page origin",
            );
        };
        // Function CPs drawn before and not now: refused when connected on this page, else a notice.
        let now: BTreeSet<(Id<Function>, u16)> = after.iter().map(|c| (c.function, c.idx)).collect();
        for b in &before {
            if now.contains(&(b.function, b.idx)) {
                continue;
            }
            let connected = tx.connections_of_function(b.function).into_iter().any(|cid| {
                tx.get::<Connection>(cid).is_some_and(|c| {
                    c.page == Some(old.page)
                        && [&c.a, &c.b]
                            .into_iter()
                            .any(|e| matches!(e, Endpoint::Cp(f, i) if *f == b.function && *i == b.idx))
                })
            });
            if connected {
                return tx.reject(format!(
                    "function CP {} is connected on this page and the new symbol does not draw it; pick a \
                     symbol that draws it or give `pin_map`",
                    b.idx
                ));
            }
            notices.push(format!(
                "function CP {} is no longer drawn by this representation",
                b.idx
            ));
        }
        // Routes follow each CP's displacement.
        let mut moves: BTreeMap<(Id<Function>, u16), Pt2> = BTreeMap::new();
        for a in &after {
            let Some(b) = before.iter().find(|b| b.function == a.function && b.idx == a.idx) else {
                continue;
            };
            let Ok(d) = sub(a.pos, b.pos) else {
                return tx.reject("the exchange moves a connection point too far");
            };
            if d != Pt2::ORIGIN {
                moves.entry((a.function, a.idx)).or_insert(d);
            }
        }
        let functions: BTreeSet<Id<Function>> = moves.keys().map(|(f, _)| *f).collect();
        let mut connections: BTreeSet<Id<Connection>> = BTreeSet::new();
        for f in functions {
            connections.extend(tx.connections_of_function(f));
        }
        for cid in connections {
            let c = tx.require::<Connection>(cid)?;
            if c.page != Some(old.page) {
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
            let followed = follow_ends(&mut route, page.grid, da, db);
            if followed.is_err() {
                return tx.reject(format!(
                    "the exchange moves the route of connection {cid} outside the drawable range"
                ));
            }
            if route != c.route {
                let mut changed = (*c).clone();
                changed.route = route;
                tx.put(changed);
            }
        }
        tx.put(next);
        Ok(Outcome {
            notices,
            ..Outcome::primary((Kind::Representation, self.rep.cast()))
        })
    }
}
