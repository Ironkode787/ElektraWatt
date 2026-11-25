// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `terminal.face_card` (`FaceCard`, docs/design/06a-drafting.md §4.8, doc 10 §3.6): the wire to a PLC card on
//! the terminal's pin that faces the card.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{Channel, Connection, Device, Endpoint, Function, Kind, Page, Representation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::geometry::{RepCp, drawn_cp, rep_cps};
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// `terminal.face_card` (`FaceCard`): puts each wire between a PLC card's channel pin and a terminal on the terminal's
/// pin that faces the card (docs/design/06a-drafting.md §4.8), in one undo step: of a terminal drawn with two pins on
/// the page the wire is drawn on, the pin that leaves the opposite way to the channel pin (a terminal over a card whose
/// pins face up takes the card's wire on its lower pin). Where the wire is on the other pin, the wires of the
/// terminal's two pins on that page change places and lose their routes, to be routed anew (Arrange and route draws
/// them); the terminal stays in the circuit between the card and the field as before. Exactly one of `card` and
/// `strip`. Rejected: neither or both; a device that does not exist; a `card` without channels, a `strip` that is no
/// strip; no wire on a far pin. Interactive. Outcome: the card or the strip.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FaceCard {
    /// The card: the terminals wired to the pins of its channels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub card: Option<Id<Device>>,
    /// The strip: its terminals wired to the pin of a card's channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub strip: Option<Id<Device>>,
}

/// A wire between a card's channel pin and a terminal's pin, on its page.
struct CardWire {
    card: (Id<Function>, u16),
    terminal: (Id<Function>, u16),
    page: Id<Page>,
}

impl FaceCard {
    /// The wires between a channel pin and a terminal that the command weighs.
    fn wires(&self, tx: &TxBuilder<'_>, device: Id<Device>) -> Vec<CardWire> {
        let terminal_of = |f: Id<Function>| {
            let d = tx.get::<Function>(f)?.device?;
            tx.get::<Device>(d)?.aspects.terminal.as_ref().map(|t| t.strip)
        };
        let ends = |c: &Connection| match (&c.a, &c.b) {
            (Endpoint::Cp(f, i), Endpoint::Cp(g, j)) => Some([(*f, *i), (*g, *j)]),
            _ => None,
        };
        let mut out = Vec::new();
        let mut push = |c: &Connection, card: (Id<Function>, u16), terminal: (Id<Function>, u16)| {
            if let Some(page) = c.page {
                out.push(CardWire { card, terminal, page });
            }
        };
        if self.card.is_some() {
            for ch in tx.channels_of_module(device) {
                let Some(Endpoint::Cp(f, i)) = tx.get::<Channel>(ch).and_then(|c| c.binding.clone()) else {
                    continue;
                };
                for id in tx.connections_at(EndpointKey::Cp(f, i)) {
                    let Some(c) = tx.get::<Connection>(id) else {
                        continue;
                    };
                    for [mine, other] in ends(&c).into_iter().flat_map(|[a, b]| [[a, b], [b, a]]) {
                        if mine == (f, i) && terminal_of(other.0).is_some() {
                            push(&c, mine, other);
                        }
                    }
                }
            }
        } else {
            for t in tx.terminals_of_strip(device) {
                for f in tx.functions_of_device(t) {
                    for id in tx.connections_of_function(f) {
                        let Some(c) = tx.get::<Connection>(id) else {
                            continue;
                        };
                        for [mine, other] in ends(&c).into_iter().flat_map(|[a, b]| [[a, b], [b, a]]) {
                            if mine.0 == f && !tx.channels_at_cp(other.0, other.1).is_empty() {
                                push(&c, other, mine);
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

impl Command for FaceCard {
    const NAME: &'static str = "terminal.face_card";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let device = match (self.card, self.strip) {
            (Some(d), None) | (None, Some(d)) => d,
            _ => return tx.reject(text!("refusal-face-card-target")),
        };
        let d = tx.require::<Device>(device)?;
        let what = crate::words::device(tx, device);
        if self.card.is_some() && tx.channels_of_module(device).is_empty() {
            return tx.reject(text!("refusal-face-card-no-card", what));
        }
        if self.strip.is_some() && d.aspects.strip.is_none() {
            return tx.reject(text!("refusal-face-card-no-strip", what));
        }
        // Each terminal's two pins on a page, swapped once.
        let mut swaps: BTreeSet<(Id<Function>, u16, u16, Id<Page>)> = BTreeSet::new();
        for w in self.wires(tx, device) {
            let Some(card_out) = drawn_cp(tx, w.card.0, w.card.1, w.page).and_then(|c| c.out) else {
                continue;
            };
            let (f, idx) = w.terminal;
            // The terminal's pins as its representation on the page draws them: two, the one at the wire and the other.
            let pins: Option<Vec<RepCp>> = tx
                .reps_of_function(f)
                .into_iter()
                .filter_map(|r| tx.get::<Representation>(r))
                .filter(|r| r.page == w.page)
                .map(|r| {
                    rep_cps(tx, &r)
                        .into_iter()
                        .filter(|p| p.function == f)
                        .collect::<Vec<_>>()
                })
                .find(|pins| pins.iter().any(|p| p.idx == idx));
            let Some([a, b]) = pins
                .as_deref()
                .and_then(|p| <&[RepCp; 2]>::try_from(p).ok())
                .copied()
            else {
                continue;
            };
            let (here, other) = if a.idx == idx { (a, b) } else { (b, a) };
            let faces = |p: &RepCp| {
                p.out
                    .is_some_and(|o| o.x.0 == -card_out.x.0 && o.y.0 == -card_out.y.0)
            };
            if !faces(&here) && faces(&other) {
                swaps.insert((f, here.idx.min(other.idx), here.idx.max(other.idx), w.page));
            }
        }
        if swaps.is_empty() {
            return tx.reject(text!("refusal-face-card-nothing", what));
        }
        for (f, i, j, page) in swaps {
            let swap = |e: &Endpoint| match e {
                Endpoint::Cp(g, k) if *g == f && *k == i => Endpoint::Cp(f, j),
                Endpoint::Cp(g, k) if *g == f && *k == j => Endpoint::Cp(f, i),
                other => other.clone(),
            };
            let mut ids = tx.connections_at(EndpointKey::Cp(f, i));
            ids.extend(tx.connections_at(EndpointKey::Cp(f, j)));
            ids.sort_unstable();
            ids.dedup();
            for id in ids {
                let Some(c) = tx.get::<Connection>(id).filter(|c| c.page == Some(page)) else {
                    continue;
                };
                let (a, b) = (swap(&c.a), swap(&c.b));
                // A wire between the two pins themselves stays as it is.
                if a == c.b && b == c.a {
                    continue;
                }
                tx.update::<Connection>(id, |c| {
                    c.a = a;
                    c.b = b;
                    c.route.clear();
                })?;
            }
        }
        Ok(Outcome::primary((Kind::Device, device.cast())))
    }
}
