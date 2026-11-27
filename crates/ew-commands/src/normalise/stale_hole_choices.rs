// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser `stale_hole_choices` (doc 12 §2.2, doc 12-3d §10.6): the hole choices that answer their owner's pattern
//! by index go when the pattern changes, whichever command changed it. A placement's choice that names its part's hole
//! (`HoleChoice::hole`) is no such choice: it stays on that hole, or applies to none and EW-LAY-017 says so.
//!
//! - **Trigger.** A placement whose realised part (its `part`, else its device's first part use whose part mounts)
//!   has other holes than in the base snapshot, by the part's `mech.holes`: the placement or its device changed, or
//!   the part's copy changed (Front view and holes saved on an own part, a library update replacing the copy), the
//!   base copy read from the base snapshot; a carrier whose path's pieces have other lengths (a lengthened rail gets
//!   another fixing hole); the project's Rail fixing or Duct fixing changed (doc 12-3d §10.3). A moved placement or
//!   carrier keeps its choices.
//! - **Effect.** The triggered placement's choices without a named hole and the triggered carrier's choices go; of a
//!   changed fixing, a carrier's choice goes where the hole at its index no longer stands where it stood, and the
//!   outcome says how many went and on which rails and ducts, so that a hole moved or left out never lands on another
//!   fixing hole in silence.
//!
//! Idempotent: an owner without such choices is left alone.

use ew_ids::Id;
use ew_schema::{Carrier, Device, HoleChoice, Kind, LibCopy, Part, PartHole, Placement};
use ew_units::Pt2;

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
        // A part copy whose holes may have changed: every placement with choices may realise it.
        if !tx.changed_of(Kind::Part).is_empty() {
            placements.extend(
                tx.iter::<Placement>()
                    .iter()
                    .filter(|p| !p.hole_choices.is_empty())
                    .map(|p| p.id),
            );
        }
        placements.sort();
        placements.dedup();
        for p in placements {
            let Some(now) = tx.get::<Placement>(p) else {
                continue;
            };
            if !now.hole_choices.iter().any(|c| c.hole.is_none()) {
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
            tx.update::<Placement>(p, |x| x.hole_choices.retain(|c: &HoleChoice| c.hole.is_some()))?;
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
        changed |= fixing_changed(tx)?;
        Ok(changed)
    }
}

/// The choices of the rails and ducts whose fixing the project's settings changed: a choice stays where the hole at its
/// index stands where it stood, else it goes, with the notice of how many went on which carriers. Whether one went.
fn fixing_changed(tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
    let before = tx.base_model().project().settings.workshop.clone();
    let now = tx.project().settings.workshop.clone();
    if before.rail_fixing == now.rail_fixing && before.duct_fixing == now.duct_fixing {
        return Ok(false);
    }
    let q = ew_derive::Q::new(tx.base_model());
    let (mut went, mut on) = (0usize, Vec::new());
    for c in tx.iter::<Carrier>() {
        if c.hole_choices.is_empty() {
            continue;
        }
        let at = |settings| -> Vec<Pt2> {
            ew_derive::layout::carrier_fixings(&c, settings)
                .into_iter()
                .map(|(at, _)| at)
                .collect()
        };
        let (was, is) = (at(&before), at(&now));
        let kept: Vec<HoleChoice> = c
            .hole_choices
            .iter()
            .filter(|h| {
                let i = usize::from(h.index);
                was.get(i).is_some_and(|w| is.get(i) == Some(w))
            })
            .cloned()
            .collect();
        if kept.len() == c.hole_choices.len() {
            continue;
        }
        went += c.hole_choices.len() - kept.len();
        on.extend(
            ew_derive::layout::carrier_label(&q, c.id)
                .map(|l| ew_derive::text::carrier_in_sentence(l).into()),
        );
        tx.update::<Carrier>(c.id, move |x| x.hole_choices = kept)?;
    }
    if went == 0 {
        return Ok(false);
    }
    let on = crate::words::and_list(on);
    tx.notice(if went == 1 {
        text!("notice-fixing-choice-dropped", on)
    } else {
        text!("notice-fixing-choices-dropped", on, went.to_string())
    });
    Ok(true)
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
    // A copy as it is now, or as the base snapshot holds it.
    let copy_of = |copy: Id<Part>| {
        if base {
            tx.base_row(&(Kind::Part, copy.cast()))
                .and_then(|r| r.get::<LibCopy<Part>>().cloned())
        } else {
            tx.get::<LibCopy<Part>>(copy)
        }
    };
    let holes = |copy: Id<Part>| copy_of(copy).and_then(|c| c.item.mech.clone()).map(|m| m.holes);
    let realised = p.part.as_ref().map(|r| r.copy).or_else(|| {
        device?.parts.iter().map(|u| u.part.copy).find(|c| {
            copy_of(*c)
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
