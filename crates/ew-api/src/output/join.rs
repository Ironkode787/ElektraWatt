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
use ew_derive::ew_schema::{ContactKind, Device, FnDef, FnRole, Function, FunctionKind, LibRef};
use ew_derive::{DeviceTag, FunctionCps, Q};

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
fn slot_of(q: &Q, function: Id<Function>) -> String {
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

impl Session {
    /// Whether the FnDef copy `fndef` can be an auxiliary function (WP-S2.18 D2): an embedded contact (contact kind
    /// NO, NC or change-over).
    pub fn auxiliary_fndef(&self, fndef: Uuid) -> bool {
        self.query().fndefs().get(Id::from_uuid(fndef)).is_some_and(|c| {
            matches!(
                c.item.contact_kind,
                ContactKind::No | ContactKind::Nc | ContactKind::Co
            )
        })
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
    /// whose parts' templates has an entry of the FnDef; terminals and strips are never listed. An FnDef the
    /// project has not embedded is not auxiliary and lists nothing.
    pub fn join_candidates(&self, fndef: Uuid) -> JoinCandidates {
        let q = self.query();
        let def: Id<FnDef> = Id::from_uuid(fndef);
        let Some(copy) = q.fndefs().get(def) else {
            return JoinCandidates::default();
        };
        let auxiliary = matches!(
            copy.item.contact_kind,
            ContactKind::No | ContactKind::Nc | ContactKind::Co
        );
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
