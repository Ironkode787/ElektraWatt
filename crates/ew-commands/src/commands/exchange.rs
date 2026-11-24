// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `rep.exchange_symbol` (`ExchangeSymbol`, FR-SYM-022): the symbol of a placed representation
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
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::commands::device::check_rep_type;
use crate::error::CmdError;
use crate::geometry::{RepCp, follow_ends, rep_cps_checked, sub};
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
/// - **`copies`**: library documents (`lib/` JSON form) embedded first, in the same transaction, as
///   `device.place` embeds its `copies` (each checked as `library.embed` checks it, at `/copies/i`; a copy the project
///   holds with the same hash skipped, one held with another hash refused), so an exchange to a symbol from a library
///   is one undo step and replays without a library.
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
    /// The library documents to embed with the exchange (`lib/` JSON form): the new symbol and its FnDef as
    /// the caller resolved them; empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

/// The ASCII-folded designation of a CP.
fn fold(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// `(symbol CP, function CP)` pairs of a derived CP map.
pub type PinPairs = Vec<(u16, u16)>;

/// The CP map `rep.exchange_symbol` derives without `pin_map`, as a pure function of the CPs:
///
/// - `function_cps`: the function's CPs, `(index, designation)`;
/// - `same_def`: whether the new symbol's FnDef is the function's definition;
/// - `symbol_cps`: the new symbol's CPs, `(symbol CP, FnDef CP, designation of that FnDef CP)`, the designation
///   `None` when the FnDef is not embedded or has no such CP.
///
/// `Ok(None)` is the map by index (nothing stored, a symbol CP draws its `fn_cp`); `Ok(Some(entries))` the map by
/// designation (ASCII case-insensitive), `(symbol CP, function CP)` in symbol-CP order, a symbol CP without a
/// counterpart left out (it draws no CP of the function); `Err((symbol CP, FnDef CP))` the first symbol CP without a
/// counterpart whose index default names a CP of the function, which the command refuses.
pub fn derive_pin_map(
    function_cps: &[(u16, &str)],
    same_def: bool,
    symbol_cps: &[(u16, u16, Option<&str>)],
) -> Result<Option<PinPairs>, (u16, u16)> {
    if same_def {
        return Ok(None);
    }
    let mut entries = Vec::new();
    for &(symbol_cp, fn_cp, designation) in symbol_cps {
        let target = designation
            .map(fold)
            .and_then(|d| function_cps.iter().find(|(_, o)| fold(o) == d))
            .map(|(idx, _)| *idx);
        match target {
            Some(idx) => entries.push((symbol_cp, idx)),
            None if function_cps.iter().any(|(c, _)| *c == fn_cp) => return Err((symbol_cp, fn_cp)),
            None => {}
        }
    }
    Ok(Some(entries))
}

impl Command for ExchangeSymbol {
    const NAME: &'static str = "rep.exchange_symbol";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        // The copies first, so that the new symbol and its FnDef resolve below.
        embed_closure(Self::NAME, tx, &self.copies)?;
        let old = tx.require::<Representation>(self.rep)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.symbol)?;
        let Some(&function_id) = old.functions.first() else {
            return tx.reject(text!("refusal-exchange-no-function"));
        };
        let function = tx.require::<Function>(function_id)?;
        if matches!(function.kind, FunctionKind::Ip(_)) {
            return tx.reject(text!("refusal-exchange-ip"));
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
        let pin_map = match &self.pin_map {
            Some(entries) => given_pin_map(entries, function_id, &own_cps, &symbol_cps)?,
            None => {
                let same_def = new_def == function.def;
                derived_pin_map(tx, function_id, new_def, same_def, &own_cps, &symbol_cps)?
            }
        };
        let mut next = (*old).clone();
        next.symbol = self.symbol;
        next.variant = 0;
        next.pin_map = KeyedList::from_vec(pin_map)
            .map_err(|e| CmdError::internal(format!("rep.exchange_symbol: pin map ({e})")))?;
        let mut notices = Vec::new();
        let slots = u16::try_from(symbol.item.text_slots.len()).unwrap_or(u16::MAX);
        let (kept, dropped): (Vec<_>, Vec<_>) = old.text_slots.iter().cloned().partition(|o| o.slot < slots);
        for o in &dropped {
            notices.push(text!("notice-exchange-slot-dropped", o.slot.to_string()));
        }
        next.text_slots = KeyedList::from_vec(kept)
            .map_err(|e| CmdError::internal(format!("rep.exchange_symbol: text slots ({e})")))?;
        let (Ok(before), Ok(after)) = (rep_cps_checked(tx, &old), rep_cps_checked(tx, &next)) else {
            return tx.reject(text!("refusal-exchange-out-of-range"));
        };
        check_lost_cps(tx, old.page, &before, &after, &mut notices)?;
        follow_routes(tx, &page, &before, &after)?;
        tx.put(next);
        Ok(Outcome {
            notices,
            ..Outcome::primary((Kind::Representation, self.rep.cast()))
        })
    }
}

/// The pin map the parameters give: each entry names a CP of the new symbol and one of the function, a symbol CP
/// once; a symbol CP the map leaves out keeps its index default, which must name no CP of the function.
fn given_pin_map(
    entries: &[ExchangePin],
    function_id: Id<Function>,
    own_cps: &[(u16, String)],
    symbol_cps: &[(u16, u16)],
) -> Result<Vec<PinMapEntry>, CmdError> {
    let mut pin_map: Vec<PinMapEntry> = Vec::new();
    let mut seen = BTreeSet::new();
    for (i, e) in entries.iter().enumerate() {
        if !symbol_cps.iter().any(|(s, _)| *s == e.symbol_cp) {
            return Err(CmdError::invalid_params(
                ExchangeSymbol::NAME,
                format!("/pin_map/{i}/symbol_cp"),
                text!("refusal-exchange-no-symbol-cp", e.symbol_cp.to_string()),
            ));
        }
        if !own_cps.iter().any(|(c, _)| *c == e.function_cp) {
            return Err(CmdError::invalid_params(
                ExchangeSymbol::NAME,
                format!("/pin_map/{i}/function_cp"),
                text!("refusal-exchange-no-function-cp", e.function_cp.to_string()),
            ));
        }
        if !seen.insert(e.symbol_cp) {
            return Err(CmdError::invalid_params(
                ExchangeSymbol::NAME,
                format!("/pin_map/{i}/symbol_cp"),
                text!("refusal-exchange-symbol-cp-twice"),
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
    for (symbol_cp, fn_cp) in symbol_cps {
        if !seen.contains(symbol_cp) && own_cps.iter().any(|(c, _)| c == fn_cp) {
            return Err(CmdError::invalid_params(
                ExchangeSymbol::NAME,
                "/pin_map",
                text!(
                    "refusal-exchange-cp-unmapped",
                    symbol_cp.to_string(),
                    fn_cp.to_string()
                ),
            ));
        }
    }
    Ok(pin_map)
}

/// The pin map derived without one in the parameters ([`derive_pin_map`]: by index for the same FnDef, else by
/// designation against the new FnDef `new_def`); refused when a symbol CP has no counterpart.
fn derived_pin_map(
    tx: &TxBuilder<'_>,
    function_id: Id<Function>,
    new_def: Id<FnDef>,
    same_def: bool,
    own_cps: &[(u16, String)],
    symbol_cps: &[(u16, u16)],
) -> Result<Vec<PinMapEntry>, CmdError> {
    let new_cps: BTreeMap<u16, String> = tx
        .get::<LibCopy<FnDef>>(new_def)
        .map(|d| {
            d.item
                .cps
                .iter()
                .map(|c| (c.idx, c.designation.as_str().to_owned()))
                .collect()
        })
        .unwrap_or_default();
    let own: Vec<(u16, &str)> = own_cps.iter().map(|(c, d)| (*c, d.as_str())).collect();
    let symbol: Vec<(u16, u16, Option<&str>)> = symbol_cps
        .iter()
        .map(|(s, f)| (*s, *f, new_cps.get(f).map(String::as_str)))
        .collect();
    match derive_pin_map(&own, same_def, &symbol) {
        Ok(entries) => Ok(entries
            .into_iter()
            .flatten()
            .map(|(symbol_cp, idx)| PinMapEntry {
                symbol_cp,
                function: function_id,
                idx,
                extra: Extra::new(),
            })
            .collect()),
        Err((symbol_cp, fn_cp)) => tx.reject(text!(
            "refusal-exchange-no-counterpart",
            symbol_cp.to_string(),
            fn_cp.to_string()
        )),
    }
}

/// Function CPs drawn before and not now: refused when connected on this page, else a notice.
fn check_lost_cps(
    tx: &TxBuilder<'_>,
    page: Id<Page>,
    before: &[RepCp],
    after: &[RepCp],
    notices: &mut Vec<ew_l10n::Text>,
) -> Result<(), CmdError> {
    let now: BTreeSet<(Id<Function>, u16)> = after.iter().map(|c| (c.function, c.idx)).collect();
    for b in before {
        if now.contains(&(b.function, b.idx)) {
            continue;
        }
        let connected = tx.connections_of_function(b.function).into_iter().any(|cid| {
            tx.get::<Connection>(cid).is_some_and(|c| {
                c.page == Some(page)
                    && [&c.a, &c.b]
                        .into_iter()
                        .any(|e| matches!(e, Endpoint::Cp(f, i) if *f == b.function && *i == b.idx))
            })
        });
        if connected {
            return tx.reject(text!("refusal-exchange-cp-connected", b.idx.to_string()));
        }
        notices.push(text!("notice-exchange-cp-not-drawn", b.idx.to_string()));
    }
    Ok(())
}

/// Routes on the page follow each CP's displacement.
fn follow_routes(
    tx: &mut TxBuilder<'_>,
    page: &Page,
    before: &[RepCp],
    after: &[RepCp],
) -> Result<(), CmdError> {
    let mut moves: BTreeMap<(Id<Function>, u16), Pt2> = BTreeMap::new();
    for a in after {
        let Some(b) = before.iter().find(|b| b.function == a.function && b.idx == a.idx) else {
            continue;
        };
        let Ok(d) = sub(a.pos, b.pos) else {
            return tx.reject(text!("refusal-exchange-too-far"));
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
        if c.page != Some(page.id) {
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
            return tx.reject(text!(
                "refusal-exchange-route-out-of-range",
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
