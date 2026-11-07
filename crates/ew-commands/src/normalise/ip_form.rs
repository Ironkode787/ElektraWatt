// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #18, the interruption point's form (WP-S2.19 D16, doc 06 §3.2 row 18), and the checks the commands
//! share.
//!
//! EW's interruption-point symbol comes in two forms, both drawn with the arrow pointing right: the source form
//! (`ew.gen.ip`, the wire on the tail) and the target form (`ew.gen.ip-target`, the wire on the tip), each with its
//! `@qet10` twin ([`FORMS`]). A representation drawn with one of them is never turned (its orientation is the
//! default), and its form is its direction's: a source point the source form, a target point the target form; an
//! undirected point either. A symbol copy is one of the forms by its key; its twin is the copy of the other key in
//! the same library.
//!
//! The normaliser takes the interruption-point functions this transaction touched (a changed function row, or a
//! changed representation of one) and gives each representation drawn with a form that is not its direction's the
//! twin, with the default orientation; a project that does not hold the twin refuses the command in words.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::AnyRow;
use ew_schema::{Function, FunctionKind, IpDirection, Kind, LibCopy, LibItem, Representation, SymbolDef};
use ew_units::Orient2;

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::text::{Text, text};
use crate::tx::TxBuilder;

/// EW's interruption-point forms: (source key, target key) per edition.
pub const FORMS: [(&str, &str); 2] = [
    ("ew.gen.ip", "ew.gen.ip-target"),
    ("ew.gen.ip@qet10", "ew.gen.ip-target@qet10"),
];

/// One of EW's interruption-point forms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Form {
    /// Whether it is the target form.
    pub target: bool,
    /// The copy ID of its twin, the other form of its edition, in the same library.
    pub twin: Id<SymbolDef>,
}

/// The form of the symbol copy `symbol`; `None` for a symbol that is not one of EW's forms (an imported or a company
/// symbol keeps a free orientation).
pub(crate) fn form(tx: &TxBuilder<'_>, symbol: Id<SymbolDef>) -> Option<Form> {
    let copy = tx.get::<LibCopy<SymbolDef>>(symbol)?;
    let key = copy.item.meta.key.as_ref()?.to_string();
    let library = copy.library_id.as_uuid();
    FORMS.iter().find_map(|(source, target)| {
        let (is_target, twin) = if key == *source {
            (false, *target)
        } else if key == *target {
            (true, *source)
        } else {
            return None;
        };
        let item: Id<LibItem> = ew_ids::v5::imported_id(library, twin);
        Some(Form {
            target: is_target,
            twin: ew_ids::v5::lib_copy_id(library, item.as_uuid()),
        })
    })
}

/// Whether a point of `direction` is drawn with the target form; `None` for an undirected point (either form).
pub(crate) fn wants_target(direction: &IpDirection) -> Option<bool> {
    match direction {
        IpDirection::Source => Some(false),
        IpDirection::Target => Some(true),
        _ => None,
    }
}

/// The refusal of a turned form: its arrow points the way its signal flows.
pub(crate) fn turned() -> Text {
    text!("refusal-ip-form-turned")
}

/// Refuses a new interruption point drawn with one of EW's forms turned, or with the other direction's form.
pub(crate) fn check_new(
    tx: &TxBuilder<'_>,
    symbol: Id<SymbolDef>,
    orient: Orient2,
    direction: &IpDirection,
) -> Result<(), CmdError> {
    let Some(f) = form(tx, symbol) else {
        return Ok(());
    };
    if orient != Orient2::default() {
        return tx.reject(turned());
    }
    if wants_target(direction).is_some_and(|t| t != f.target) {
        return tx.reject(text!("refusal-ip-form-direction"));
    }
    Ok(())
}

/// The symbol a point of `direction` is drawn with in place of `symbol`: the twin when `symbol` is the other
/// direction's form, which the project must hold (else the refusal names it), else `symbol` itself.
pub(crate) fn symbol_for(
    tx: &TxBuilder<'_>,
    symbol: Id<SymbolDef>,
    direction: &IpDirection,
) -> Result<Id<SymbolDef>, CmdError> {
    let Some(f) = form(tx, symbol) else {
        return Ok(symbol);
    };
    match wants_target(direction) {
        Some(t) if t != f.target => {
            if !tx.exists::<LibCopy<SymbolDef>>(f.twin) {
                let arrow = if t {
                    text!("refusal-ip-form-incoming")
                } else {
                    text!("refusal-ip-form-outgoing")
                };
                return tx.reject(text!(
                    "refusal-ip-form-missing",
                    arrow,
                    crate::words::symbol_copy(tx, symbol)
                ));
            }
            Ok(f.twin)
        }
        _ => Ok(symbol),
    }
}

/// Normaliser #18.
#[derive(Clone, Copy, Debug, Default)]
pub struct IpForm;

impl Normaliser for IpForm {
    fn id(&self) -> &'static str {
        "ip_form"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut functions: BTreeSet<Id<Function>> = BTreeSet::new();
        for key in tx.changed_keys() {
            match key.0 {
                Kind::Function => {
                    functions.insert(key.1.cast());
                }
                Kind::Representation => {
                    if let Some(AnyRow::Representation(r)) = tx.row(&key) {
                        functions.extend(r.functions.iter().copied());
                    }
                }
                _ => {}
            }
        }
        let mut changed = false;
        for f in functions {
            let Some(function) = tx.get::<Function>(f) else {
                continue;
            };
            let FunctionKind::Ip(spec) = &function.kind else {
                continue;
            };
            for rid in tx.reps_of_function(f) {
                let Some(rep) = tx.get::<Representation>(rid) else {
                    continue;
                };
                let symbol = symbol_for(tx, rep.symbol, &spec.direction)?;
                if symbol != rep.symbol {
                    tx.update::<Representation>(rid, |r| {
                        r.symbol = symbol;
                        r.orient = Orient2::default();
                    })?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}
