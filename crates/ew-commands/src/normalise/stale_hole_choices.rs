// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser `stale_hole_choices` (doc 12 §2.2, doc 12-3d §10.6): a placement's and a carrier's hole choices answer
//! the holes of its pattern by index, so they go when the pattern changes, whichever command changed it.
//!
//! - **Trigger.** A placement whose realised part (its `part`, else its device's first part use whose part mounts)
//!   has other holes than in the base snapshot, by the part's `mech.holes`; a carrier whose path's pieces have other
//!   lengths (a lengthened rail gets another fixing hole). A moved placement or carrier keeps its choices.
//! - **Effect.** The triggered owner's `hole_choices` are emptied.
//!
//! Idempotent: an owner without choices is left alone.

use ew_ids::Id;
use ew_schema::{Carrier, Device, Kind, LibCopy, Part, PartHole, Placement};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser `stale_hole_choices`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StaleHoleChoices;

impl Normaliser for StaleHoleChoices {
    fn id(&self) -> &'static str {
        "stale_hole_choices"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut stale: Vec<Id<Placement>> = Vec::new();
        let mut placements: Vec<Id<Placement>> = tx
            .changed_of(Kind::Placement)
            .iter()
            .map(|k| k.1.cast())
            .collect();
        for key in tx.changed_of(Kind::Device) {
            let device: Id<Device> = key.1.cast();
            placements.extend(
                tx.iter::<Placement>()
                    .iter()
                    .filter(|p| p.device == device)
                    .map(|p| p.id),
            );
        }
        placements.sort();
        placements.dedup();
        for p in placements {
            let Some(now) = tx.get::<Placement>(p) else {
                continue;
            };
            if now.hole_choices.is_empty() {
                continue;
            }
            let before = tx
                .base_row(&(Kind::Placement, p.cast()))
                .and_then(|r| r.get::<Placement>().cloned());
            let pattern_now = pattern(tx, &now, false);
            let pattern_before = before.as_ref().map(|b| pattern(tx, b, true));
            if pattern_before.is_some_and(|b| b != pattern_now) {
                stale.push(p);
            }
        }
        let mut changed = false;
        for p in stale {
            tx.update::<Placement>(p, |x| x.hole_choices.clear())?;
            changed = true;
        }
        let mut carriers: Vec<Id<Carrier>> = Vec::new();
        for key in tx.changed_of(Kind::Carrier) {
            let Some(now) = tx.get::<Carrier>(key.1.cast()) else {
                continue;
            };
            let before = tx.base_row(&key).and_then(|r| r.get::<Carrier>().cloned());
            if !now.hole_choices.is_empty() && before.is_some_and(|b| pieces(&b) != pieces(&now)) {
                carriers.push(now.id);
            }
        }
        for c in carriers {
            tx.update::<Carrier>(c, |x| x.hole_choices.clear())?;
            changed = true;
        }
        Ok(changed)
    }
}

/// The holes of the part a placement realises: its `part`, else its device's first part use whose part has a mount
/// kind that mounts; read from the base snapshot with `base`.
fn pattern(tx: &TxBuilder<'_>, p: &Placement, base: bool) -> Vec<PartHole> {
    let device = if base {
        tx.base_row(&(Kind::Device, p.device.cast()))
            .and_then(|r| r.get::<Device>().cloned())
    } else {
        tx.get::<Device>(p.device)
    };
    let holes = |copy: Id<Part>| {
        tx.get::<LibCopy<Part>>(copy)
            .and_then(|c| c.item.mech.clone())
            .map(|m| m.holes)
    };
    let realised = p.part.as_ref().map(|r| r.copy).or_else(|| {
        device?.parts.iter().map(|u| u.part.copy).find(|c| {
            tx.get::<LibCopy<Part>>(*c)
                .and_then(|x| x.item.mech.clone())
                .and_then(|m| m.mount_kind)
                .is_some_and(|k| !matches!(k, ew_schema::MountKind::None | ew_schema::MountKind::OnParent))
        })
    });
    realised.and_then(holes).unwrap_or_default()
}

/// The lengths of a carrier's pieces, along the axes.
fn pieces(c: &Carrier) -> Vec<i128> {
    c.path
        .windows(2)
        .map(|w| {
            let dx = (i128::from(w[1].x.0) - i128::from(w[0].x.0)).abs();
            let dy = (i128::from(w[1].y.0) - i128::from(w[0].y.0)).abs();
            dx.max(dy)
        })
        .collect()
}
