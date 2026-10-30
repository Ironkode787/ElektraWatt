// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The devices a placed symbol can join (WP-S2.18 part 1, D1 and D2; FR-DEV-038): the read behind the Place tool's
//! Join chooser and `place.pick`'s printed tags. A tool never calls the `Session`: the shell's tool host fetches
//! this once per (revision, FnDef) and hands the rows to the chooser.
//!
//! **Joining** is `device.place` with `existing_function` when the device has an unplaced function of the symbol's
//! FnDef (a part's template slot, FR-UI-017), else with `join_device {device, role: aux}` (D1): a slot is filled
//! before a new auxiliary function is made.

use std::cmp::Ordering;

use ew_derive::ew_ids::{Id, Uuid, v5};
use ew_derive::ew_schema::{
    ContactKind, Device, FnDef, FnRole, Function, FunctionKind, LibRef, Representation,
};
use ew_derive::{DeviceTag, FunctionCps, PageOrder, PageRef, Q, RepAnchorCell};

use crate::Session;

/// What a symbol drawing one FnDef can join (WP-S2.18 D2).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct JoinCandidates {
    /// Whether the FnDef can be an auxiliary function: a contact (contact kind NO, NC or change-over). The chooser
    /// is offered only then.
    pub auxiliary: bool,
    /// The rows: every unplaced function of the FnDef (a slot), by its device's tag in natural order and then by
    /// its slot; then every other device whose family takes auxiliary functions, in natural tag order.
    pub rows: Vec<JoinCandidate>,
}

/// One device a symbol can join.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JoinCandidate {
    /// The device.
    pub device: Uuid,
    /// The device's tag as a report prints it (`-101K1`; `Tag::short`).
    pub tag: String,
    /// The unplaced function of the FnDef the placement fills; `None` for a new auxiliary function.
    pub function: Option<Uuid>,
    /// The slot's CP designations joined by `-` (`13-14`); empty for a new auxiliary function.
    pub slot: String,
}

/// The join the Properties Tag field proposes (WP-S2.18 D3): typing `tag`, an existing device's, into the Tag field of
/// a device whose one representation can join it; accepting it is `rep.join_device {rep, device}`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JoinProposal {
    /// The representation that joins.
    pub rep: Uuid,
    /// The device it joins.
    pub device: Uuid,
    /// That device's tag as a report prints it (`-K1`).
    pub tag: String,
}

/// What the toast of a join says (WP-S2.18 D3: "Joined -K1 (NO contact 13-14) at 102-C4").
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JoinedText {
    /// The device's tag as a report prints it.
    pub tag: String,
    /// The function's kind in words (`NO contact 13-14`).
    pub kind: String,
    /// `<folio>-<cell>` of the representation, the folio alone without a cell.
    pub place: String,
}

/// The natural order of two tags: runs of decimal digits compare by their value, other runs by their text.
pub(crate) fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn runs(s: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let mut start = 0;
        let bytes = s.as_bytes();
        for i in 1..=bytes.len() {
            if i == bytes.len() || bytes[i].is_ascii_digit() != bytes[start].is_ascii_digit() {
                out.push((bytes[start].is_ascii_digit(), &s[start..i]));
                start = i;
            }
        }
        out
    }
    if a.is_empty() || b.is_empty() {
        return a.cmp(b);
    }
    for (x, y) in runs(a).into_iter().zip(runs(b)) {
        let o = match (x, y) {
            ((true, x), (true, y)) => {
                let (tx, ty) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                tx.len().cmp(&ty.len()).then_with(|| tx.cmp(ty))
            }
            ((_, x), (_, y)) => x.cmp(y),
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    a.cmp(b)
}

/// The FnDef copy a library reference names.
fn fndef_copy(r: &LibRef) -> Id<FnDef> {
    v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid())
}

/// The tag a report prints for `device`; empty without one.
fn tag_of(q: &Q, device: Id<Device>) -> String {
    q.get::<DeviceTag>(device)
        .as_ref()
        .as_ref()
        .map(|t| t.short())
        .unwrap_or_default()
}

/// The slot of `function`: its CP designations in index order, joined by `-`.
pub(crate) fn slot_of(q: &Q, function: Id<Function>) -> String {
    q.get::<FunctionCps>(function)
        .iter()
        .map(|c| c.designation.as_str())
        .collect::<Vec<_>>()
        .join("-")
}

/// Whether `device`'s family takes auxiliary functions of FnDef `def` (D2): its main function is a coil (a relay or
/// a contactor), or one of its parts' templates has an entry of `def`. Terminals and strips never do.
fn takes_auxiliary(q: &Q, id: Id<Device>, device: &Device, def: Id<FnDef>) -> bool {
    if device.aspects.terminal.is_some() || device.aspects.strip.is_some() {
        return false;
    }
    let coil = q.idx().functions_of_device(id).any(|f| {
        q.functions().get(f).is_some_and(|f| {
            f.role == FnRole::Main
                && q.fndefs()
                    .get(f.def)
                    .is_some_and(|d| d.item.contact_kind == ContactKind::Coil)
        })
    });
    coil || device.parts.iter().any(|u| {
        q.parts().get(u.part.copy).is_some_and(|p| {
            p.item
                .function_template
                .iter()
                .any(|t| fndef_copy(&t.fndef) == def)
        })
    })
}

/// The one representation of `device` when D3 lets it join another device: the device has exactly one function, a
/// normal one, no part use and that one representation, which draws only it; terminals and strips never.
fn joinable_rep(q: &Q, device: Id<Device>) -> Option<(Id<Representation>, Id<Function>)> {
    let d = q.devices().get(device)?;
    if d.aspects.terminal.is_some() || d.aspects.strip.is_some() || !d.parts.is_empty() {
        return None;
    }
    let idx = q.idx();
    let mut functions = idx.functions_of_device(device);
    let (f, None) = (functions.next()?, functions.next()) else {
        return None;
    };
    if q.functions().get(f)?.kind != FunctionKind::Normal {
        return None;
    }
    let mut reps = idx.reps_of_function(f);
    let (r, None) = (reps.next()?, reps.next()) else {
        return None;
    };
    (q.reps().get(r)?.functions.as_slice() == [f]).then_some((r, f))
}

impl Session {
    /// The join that typing `text` into the Tag field of `device` proposes (WP-S2.18 D3): `text` names another
    /// existing device (FT-544's reference grammar: a bare tag such as `-K1`, `tag:`, `id:`), `device` has exactly one
    /// function, no part and no other representation, and the named device takes the function's FnDef by D2's rule
    /// (an unplaced function of it, else a contact joining a device whose family takes auxiliary functions). `None`
    /// otherwise: the Tag field then keeps its other behaviour.
    pub fn join_proposal(&self, device: Uuid, text: &str) -> Option<JoinProposal> {
        let target = self.resolve_reference(text.trim()).ok()?;
        let q = self.query();
        let target_id: Id<Device> = Id::from_uuid(target);
        let target_device = q.devices().get(target_id)?;
        if target == device {
            return None;
        }
        let (rep, f) = joinable_rep(&q, Id::from_uuid(device))?;
        let def = q.functions().get(f)?.def;
        let fndef = def.as_uuid();
        let slot = !self.unplaced_slots(target, fndef).is_empty();
        let joins =
            slot || (self.auxiliary_fndef(fndef) && takes_auxiliary(&q, target_id, target_device, def));
        joins.then(|| JoinProposal {
            rep: rep.as_uuid(),
            device: target,
            tag: tag_of(&q, target_id),
        })
    }

    /// The words of the toast after `rep` joined a device (WP-S2.18 D3): the device's tag, the kind of the function
    /// the representation draws (its FnDef's last category entries and CP designations, `NO contact 13-14`) and the
    /// representation's place (`102-C4`). `None` for a representation that draws no function of a device.
    pub fn joined_text(&self, rep: Uuid) -> Option<JoinedText> {
        let q = self.query();
        let r = q.reps().get(Id::from_uuid(rep))?;
        let f = q.functions().get(*r.functions.first()?)?;
        let device = f.device?;
        let folio = q
            .get::<PageOrder>(())
            .entries
            .iter()
            .find(|e| e.page == PageRef::Drawn(r.page))
            .map(|e| e.folio.clone())?;
        let place = match q.get::<RepAnchorCell>(Id::from_uuid(rep)).as_ref() {
            Some(cell) => format!("{folio}-{cell}"),
            None => folio,
        };
        Some(JoinedText {
            tag: tag_of(&q, device),
            kind: super::navigators::function_kind_text(&q, f),
            place,
        })
    }

    /// Whether the FnDef copy `fndef` can be an auxiliary function (WP-S2.18 D2): an embedded contact (contact kind
    /// NO, NC or change-over), or a starter one the project does not embed yet (FT-656).
    pub fn auxiliary_fndef(&self, fndef: Uuid) -> bool {
        let id = Id::from_uuid(fndef);
        let kind = match self.query().fndefs().get(id) {
            Some(c) => c.item.contact_kind.clone(),
            // A starter FnDef the placement embeds with its symbol (FT-656).
            None => match super::editing::starter_fndef(id) {
                Some(c) => c.item.contact_kind.clone(),
                None => return false,
            },
        };
        matches!(kind, ContactKind::No | ContactKind::Nc | ContactKind::Co)
    }

    /// The tag a report prints for `device` (`-101K1`; `Tag::short`); `None` for a missing device.
    pub fn device_tag(&self, device: Uuid) -> Option<String> {
        let q = self.query();
        let id: Id<Device> = Id::from_uuid(device);
        q.devices().contains(id).then(|| tag_of(&q, id))
    }

    /// What a symbol drawing the FnDef copy `fndef` can join (WP-S2.18 D2): whether the FnDef can be an auxiliary
    /// function, and the rows of the Join chooser. A slot is an unplaced normal function of the FnDef (no
    /// representation draws it) on a device; its row comes first, so that a slot is filled before a new auxiliary
    /// function is made (D1). The second group is every other device whose main function is a coil, or one of
    /// whose parts' templates has an entry of the FnDef; terminals and strips are never listed. An FnDef neither
    /// embedded nor a starter FnDef (which a placement embeds with its symbol, FT-656) is not auxiliary and lists
    /// nothing.
    pub fn join_candidates(&self, fndef: Uuid) -> JoinCandidates {
        let q = self.query();
        let def: Id<FnDef> = Id::from_uuid(fndef);
        // A starter FnDef the placement embeds with its symbol counts as held (FT-656).
        let kind = match q.fndefs().get(def) {
            Some(copy) => copy.item.contact_kind.clone(),
            None => match super::editing::starter_fndef(def) {
                Some(copy) => copy.item.contact_kind.clone(),
                None => return JoinCandidates::default(),
            },
        };
        let auxiliary = matches!(kind, ContactKind::No | ContactKind::Nc | ContactKind::Co);
        let idx = q.idx();
        let mut slots: Vec<JoinCandidate> = Vec::new();
        for (id, f) in q.functions().iter() {
            let Some(device) = f.device else { continue };
            if f.def != def || f.kind != FunctionKind::Normal || idx.reps_of_function(id).next().is_some() {
                continue;
            }
            slots.push(JoinCandidate {
                device: device.as_uuid(),
                tag: tag_of(&q, device),
                function: Some(id.as_uuid()),
                slot: slot_of(&q, id),
            });
        }
        slots.sort_by(|a, b| {
            natural_cmp(&a.tag, &b.tag)
                .then_with(|| a.device.cmp(&b.device))
                .then_with(|| natural_cmp(&a.slot, &b.slot))
                .then_with(|| a.function.cmp(&b.function))
        });
        let mut devices: Vec<JoinCandidate> = Vec::new();
        if auxiliary {
            for (id, d) in q.devices().iter() {
                if slots.iter().any(|s| s.device == id.as_uuid()) || !takes_auxiliary(&q, id, d, def) {
                    continue;
                }
                devices.push(JoinCandidate {
                    device: id.as_uuid(),
                    tag: tag_of(&q, id),
                    function: None,
                    slot: String::new(),
                });
            }
        }
        devices.sort_by(|a, b| natural_cmp(&a.tag, &b.tag).then_with(|| a.device.cmp(&b.device)));
        slots.extend(devices);
        JoinCandidates {
            auxiliary,
            rows: slots,
        }
    }

    /// The unplaced functions of FnDef copy `fndef` on `device` (WP-S2.18 D1, D4) as `(function, slot)`, in slot
    /// order: what `device.place`'s `existing_function` fills before `join_device` makes a new auxiliary function.
    pub fn unplaced_slots(&self, device: Uuid, fndef: Uuid) -> Vec<(Uuid, String)> {
        let q = self.query();
        let def: Id<FnDef> = Id::from_uuid(fndef);
        let idx = q.idx();
        let mut out: Vec<(Uuid, String)> = idx
            .functions_of_device(Id::from_uuid(device))
            .filter(|f| {
                q.functions()
                    .get(*f)
                    .is_some_and(|r| r.def == def && r.kind == FunctionKind::Normal)
                    && idx.reps_of_function(*f).next().is_none()
            })
            .map(|f| (f.as_uuid(), slot_of(&q, f)))
            .collect();
        out.sort_by(|a, b| natural_cmp(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::natural_cmp;
    use std::cmp::Ordering;

    #[test]
    fn tags_sort_naturally() {
        assert_eq!(natural_cmp("-101K2", "-101K10"), Ordering::Less);
        assert_eq!(natural_cmp("-101K1", "-102K1"), Ordering::Less);
        assert_eq!(natural_cmp("13-14", "23-24"), Ordering::Less);
        assert_eq!(natural_cmp("", "-K1"), Ordering::Less);
    }
}
