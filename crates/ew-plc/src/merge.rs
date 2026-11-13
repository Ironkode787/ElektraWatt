// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Matching a foreign image to the project and the per-field three-way merge (doc 10 §4.4, FR-PLC-032, FR-PLC-033).
//!
//! **Matching** (A6), first hit wins: the object's ID is one the project wrote or one a foreign-ID record holds;
//! its IEC product designation is that of exactly one object of the project; its position is that of exactly one
//! (a module's slot in its station, a channel's number in its module); its order number is that of exactly one
//! module of the station. A tag follows its channel's link, else its ID, its name, then its address. A foreign
//! object that matches nothing, or more than one object, goes to the **parking list** and is never created
//! silently. A module that only holds other modules (a rack) is no object of the project and is passed over.
//!
//! **Scope.** Only the stations the file holds are compared; with `complete`, every station of the configuration
//! project. An object of a compared station that the last exchange held and the file no longer has is listed as
//! removed and never deleted.
//!
//! **Resolution** per field, with the base `S` (the last exchange), ours `E` and theirs `T`: equal sides are no
//! change; a change of theirs alone applies when the PLC or both own the field and is a rejected PLC change when
//! the schematic owns it; a change of ours alone is kept (export pending when the PLC owns the field); a change of
//! both is a conflict whose default is theirs when the PLC owns the field and ours otherwise. A field of no group
//! (structure, designations, derived values) is the schematic's and cannot be accepted.

use std::collections::{BTreeMap, BTreeSet};

use ew_caex::ExchangeImage;
use ew_derive::ew_schema::{AnyId, ExchangeObject, FieldGroup, Owner, Ownership};

use crate::fields::{FieldClass, Object, Snapshot, class, objects, ours};
use crate::image::Projection;

/// Which fields a foreign file carries.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Vocabulary {
    /// Every field of every object (AML).
    All,
    /// A tag table: the tags' `name`, `data_type` and `address`, and the one comment field `comment` names
    /// (`comment`, or `comment.<language>`).
    Tags {
        /// The comment field the table's comment column is.
        comment: String,
    },
}

impl Vocabulary {
    fn covers(&self, object: &ExchangeObject, field: &str) -> bool {
        match self {
            Vocabulary::All => true,
            Vocabulary::Tags { comment } => {
                *object == ExchangeObject::Tag
                    && (matches!(field, "name" | "data_type" | "address") || field == comment)
            }
        }
    }

    fn holds(&self, object: &ExchangeObject) -> bool {
        *self == Vocabulary::All || *object == ExchangeObject::Tag
    }
}

/// How a merge runs.
#[derive(Clone, Debug)]
pub struct MergeOptions<'a> {
    /// The configuration project's ownership.
    pub ownership: Option<&'a Ownership>,
    /// The file is complete: what it does not hold was removed in the PLC software.
    pub complete: bool,
    /// The fields the file carries.
    pub vocabulary: Vocabulary,
}

/// Which side of a field is taken.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    /// The project's value stays.
    Ours,
    /// The file's value is written.
    Theirs,
}

/// What the merge makes of a differing field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// Changed in the file only, and the PLC or both own it: applied.
    Apply,
    /// Changed in the file only, and the schematic owns it: a rejected PLC change.
    Rejected,
    /// Changed in the project only: kept, and exported next time.
    Kept,
    /// Changed in the project only although the PLC owns it: kept, export pending.
    Pending,
    /// Changed on both sides.
    Conflict,
}

impl State {
    /// The state's word in a preview.
    pub fn word(self) -> &'static str {
        match self {
            State::Apply => "apply",
            State::Rejected => "rejected",
            State::Kept => "kept",
            State::Pending => "pending",
            State::Conflict => "conflict",
        }
    }
}

/// One differing field of a matched object.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Change {
    /// The change's key in a set of resolutions: `<object>/<target>/<field>`.
    pub id: String,
    /// The kind of the object.
    pub object: ExchangeObject,
    /// The project's record the object is.
    pub target: AnyId,
    /// The object in words.
    pub subject: String,
    /// The field.
    pub field: String,
    /// The value of the last exchange.
    pub base: Option<String>,
    /// The project's value.
    pub ours: Option<String>,
    /// The file's value.
    pub theirs: Option<String>,
    /// The field's group; `None`: the schematic's own field.
    pub group: Option<FieldGroup>,
    /// The group's owner.
    pub owner: Option<Owner>,
    /// What the merge makes of it.
    pub state: State,
    /// The side taken without a resolution.
    pub default: Choice,
    /// Whether the file's value can be taken at all.
    pub acceptable: bool,
}

/// A module of a parked station, or a parked module, as "create unplaced" needs it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParkedModule {
    /// Its ID in the file.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its IEC product designation.
    pub designation: Option<String>,
    /// Its slot.
    pub position: u32,
    /// Its order number.
    pub order_number: Option<String>,
    /// Its input and output start bytes.
    pub starts: (Option<u32>, Option<u32>),
    /// Its exchanged values: what the snapshot holds for it once it is created.
    pub fields: BTreeMap<String, String>,
}

/// A foreign object without a counterpart in the project.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Parked {
    /// Its ID in the file: the entry's key in a set of resolutions.
    pub id: String,
    /// The kind of the object.
    pub object: ExchangeObject,
    /// Its name in words.
    pub name: String,
    /// Whether several objects of the project match it (else none does).
    pub ambiguous: bool,
    /// The station device a parked module would be created under.
    pub station: Option<AnyId>,
    /// Its IEC product designation.
    pub designation: Option<String>,
    /// Its exchanged values: what the snapshot holds for it once it is created.
    pub fields: BTreeMap<String, String>,
    /// What "create unplaced" creates: the module itself, or the modules of a parked station. Empty for an object
    /// that cannot be created (a channel, a tag, a bus node).
    pub modules: Vec<ParkedModule>,
    /// Whether "create unplaced" applies.
    pub creatable: bool,
}

/// A foreign object and the project's record it is, with what the next snapshot holds for it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Matched {
    /// The kind of the object.
    pub object: ExchangeObject,
    /// The project's record.
    pub target: AnyId,
    /// The object's ID in the file, when it is not the ID the project writes.
    pub foreign: Option<String>,
    /// The exchanged values: the file's, and for a field the file's format does not carry the last exchange's
    /// (else the project's).
    pub fields: BTreeMap<String, String>,
}

/// The result of a merge: what a preview shows and what applying it writes.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Plan {
    /// The differing fields, in the file's order.
    pub changes: Vec<Change>,
    /// The objects of the last exchange that the file no longer has: kind, record and name.
    pub removed: Vec<(ExchangeObject, AnyId, String)>,
    /// The parking list.
    pub parking: Vec<Parked>,
    /// The matched objects.
    pub matched: Vec<Matched>,
}

/// One object of the project's image.
struct Mine {
    kind: &'static str,
    target: AnyId,
    object: Object,
    taken: bool,
}

/// The merge of `theirs` into the project whose image is `projection` and whose last exchange is `base`.
pub fn merge(
    projection: &Projection,
    base: &Snapshot,
    theirs: &ExchangeImage,
    options: &MergeOptions<'_>,
) -> Plan {
    let mut mine: Vec<Mine> = ours(projection)
        .into_iter()
        .map(|(kind, target, object)| Mine {
            kind,
            target,
            object,
            taken: false,
        })
        .collect();
    let foreign = objects(theirs);
    let mut plan = Plan::default();
    // the project's record of every matched foreign object, by its ID in the file
    let mut matched: BTreeMap<&str, usize> = BTreeMap::new();
    // the foreign stations and modules that are parked: what is below them is not looked at
    let mut skipped: BTreeSet<&str> = BTreeSet::new();
    let mut compared: BTreeSet<String> = BTreeSet::new();
    let channels: BTreeSet<&str> = foreign
        .iter()
        .filter(|o| o.object == ExchangeObject::Channel)
        .map(|o| o.id.as_str())
        .collect();

    for t in &foreign {
        if !options.vocabulary.holds(&t.object) {
            continue;
        }
        if t.station.as_deref().is_some_and(|s| skipped.contains(s))
            || t.parent.as_deref().is_some_and(|p| skipped.contains(p))
        {
            continue;
        }
        if t.object == ExchangeObject::Module && t.container {
            continue;
        }
        // a tag of a channel that is in the file and has no counterpart goes with that channel's module
        if t.object == ExchangeObject::Tag
            && t.parent
                .as_deref()
                .is_some_and(|c| channels.contains(c) && !matched.contains_key(c))
        {
            continue;
        }
        let parent = t.parent.as_deref().and_then(|p| matched.get(p).copied());
        let hit = find(&mine, base, t, parent);
        let index = match hit {
            Hit::One(i) => i,
            Hit::New(target) => {
                // a tag on a matched channel that has none in the project
                mine.push(Mine {
                    kind: ExchangeObject::Tag.name(),
                    target,
                    object: Object {
                        object: ExchangeObject::Tag,
                        id: String::new(),
                        name: t.name.clone(),
                        station: None,
                        parent: None,
                        designation: None,
                        position: None,
                        container: false,
                        fields: BTreeMap::new(),
                    },
                    taken: false,
                });
                mine.len() - 1
            }
            Hit::None | Hit::Many => {
                skipped.insert(&t.id);
                plan.parking.push(park(
                    t,
                    &foreign,
                    parent.map(|i| mine[i].target),
                    hit == Hit::Many,
                ));
                continue;
            }
        };
        mine[index].taken = true;
        matched.insert(&t.id, index);
        let m = &mine[index];
        if t.object == ExchangeObject::Station {
            compared.insert(m.object.id.clone());
        }
        let empty = BTreeMap::new();
        let s = base.records.get(&(m.kind, m.target));
        // A table's comment column is the comment in one language. Where the project's comment has no language,
        // the column is that text: a text without a language reads the same in every language.
        let mut theirs_fields = t.fields.clone();
        let neutral = match &options.vocabulary {
            Vocabulary::Tags { comment } if comment != "comment" => {
                let neutral =
                    !m.object.fields.contains_key(comment) && m.object.fields.contains_key("comment");
                if neutral && let Some(text) = theirs_fields.remove(comment) {
                    theirs_fields.insert("comment".to_owned(), text);
                }
                neutral.then_some(comment.as_str())
            }
            _ => None,
        };
        let covers = |f: &str| match neutral {
            Some(language) if f == language => false,
            Some(_) if f == "comment" => true,
            _ => options.vocabulary.covers(&t.object, f),
        };
        let subject = if m.object.name.is_empty() {
            &t.name
        } else {
            &m.object.name
        };
        let fields: BTreeSet<&String> = theirs_fields
            .keys()
            .chain(m.object.fields.keys())
            .chain(s.into_iter().flat_map(|s| s.keys()))
            .filter(|f| covers(f))
            .collect();
        for field in fields {
            let change = resolve(
                &t.object,
                m.target,
                subject,
                field,
                s.unwrap_or(&empty).get(field),
                m.object.fields.get(field),
                theirs_fields.get(field),
                options.ownership,
            );
            plan.changes.extend(change);
        }
        // the next snapshot: the file's values over what the file's format does not carry
        let mut next = match &options.vocabulary {
            Vocabulary::All => BTreeMap::new(),
            Vocabulary::Tags { .. } => s.cloned().unwrap_or_else(|| m.object.fields.clone()),
        };
        next.retain(|f, _| !covers(f));
        next.extend(theirs_fields);
        plan.matched.push(Matched {
            object: t.object.clone(),
            target: m.target,
            foreign: (!t.id.is_empty() && t.id != m.object.id).then(|| t.id.clone()),
            fields: next,
        });
    }

    // what the last exchange held, the file's scope covers and the file no longer has
    let complete_ids: BTreeSet<&String> = mine
        .iter()
        .filter(|m| m.object.object == ExchangeObject::Station)
        .map(|m| &m.object.id)
        .collect();
    for m in mine.iter().filter(|m| !m.taken) {
        let in_scope = match &options.vocabulary {
            Vocabulary::Tags { .. } => options.complete && m.object.object == ExchangeObject::Tag,
            Vocabulary::All => m
                .object
                .station
                .as_ref()
                .is_some_and(|s| compared.contains(s) || (options.complete && complete_ids.contains(s))),
        };
        if in_scope && base.records.contains_key(&(m.kind, m.target)) {
            plan.removed
                .push((m.object.object.clone(), m.target, m.object.name.clone()));
        }
    }
    plan
}

#[derive(PartialEq, Eq)]
enum Hit {
    /// The project's object at this index.
    One(usize),
    /// No tag yet on the matched channel that is this record.
    New(AnyId),
    None,
    Many,
}

/// The project's object that the foreign object `t` is (A6).
fn find(mine: &[Mine], base: &Snapshot, t: &Object, parent: Option<usize>) -> Hit {
    let kind = t.object.name();
    let free = |i: &usize| mine[*i].kind == kind && !mine[*i].taken;
    let all: Vec<usize> = (0..mine.len()).filter(free).collect();
    // 1. the ID the project wrote, or the one a foreign-ID record holds
    if !t.id.is_empty() {
        let by_foreign = base
            .foreign
            .get(&t.id)
            .filter(|(k, _)| *k == kind)
            .map(|(_, id)| *id);
        if let Some(i) = all
            .iter()
            .copied()
            .find(|i| mine[*i].object.id == t.id || Some(mine[*i].target) == by_foreign)
        {
            return Hit::One(i);
        }
    }
    let one = |candidates: Vec<usize>| match candidates.as_slice() {
        [] => Hit::None,
        [i] => Hit::One(*i),
        _ => Hit::Many,
    };
    // the objects of the project below the matched parent
    let below: Vec<usize> = match parent {
        Some(p) => all
            .iter()
            .copied()
            .filter(|i| mine[*i].object.parent.as_deref() == Some(mine[p].object.id.as_str()))
            .collect(),
        None => Vec::new(),
    };
    match t.object {
        ExchangeObject::Station => {
            // 2. the IEC designation, then 4. the type
            let by = |f: &dyn Fn(&Object) -> bool| -> Vec<usize> {
                all.iter().copied().filter(|i| f(&mine[*i].object)).collect()
            };
            if t.designation.is_some() {
                let hit = one(by(&|o| o.designation == t.designation));
                if hit != Hit::None {
                    return hit;
                }
            }
            match t.fields.get("type") {
                Some(ty) => one(by(&|o| o.fields.get("type") == Some(ty))),
                None => Hit::None,
            }
        }
        ExchangeObject::Module => {
            let by = |f: &dyn Fn(&Object) -> bool| -> Vec<usize> {
                below.iter().copied().filter(|i| f(&mine[*i].object)).collect()
            };
            // 2. the IEC designation, 3. the slot, 4. the order number
            if t.designation.is_some() {
                let hit = one(by(&|o| o.designation == t.designation));
                if hit != Hit::None {
                    return hit;
                }
            }
            let hit = one(by(&|o| o.position == t.position));
            if hit != Hit::None {
                return hit;
            }
            match t.fields.get("order_number") {
                Some(n) => one(by(&|o| o.fields.get("order_number") == Some(n))),
                None => Hit::None,
            }
        }
        ExchangeObject::Channel => one(below
            .iter()
            .copied()
            .filter(|i| {
                let o = &mine[*i].object;
                o.position == t.position && o.fields.get("io") == t.fields.get("io")
            })
            .collect()),
        ExchangeObject::Node => one(below
            .iter()
            .copied()
            .filter(|i| mine[*i].object.designation == t.designation)
            .collect()),
        ExchangeObject::Tag => {
            // the tag of the channel its link names
            if t.parent.is_some() {
                return match parent {
                    Some(p) => {
                        let target = mine[p].target;
                        match all.iter().copied().find(|i| mine[*i].target == target) {
                            Some(i) => Hit::One(i),
                            None => Hit::New(target),
                        }
                    }
                    // the channel its link names is not in the file
                    None => Hit::None,
                };
            }
            let by = |f: &dyn Fn(&Object) -> bool| -> Vec<usize> {
                all.iter().copied().filter(|i| f(&mine[*i].object)).collect()
            };
            let hit = one(by(&|o| o.name == t.name));
            if hit != Hit::None {
                return hit;
            }
            match t.fields.get("address") {
                Some(a) => one(by(&|o| o.fields.get("address") == Some(a))),
                None => Hit::None,
            }
        }
        _ => Hit::None,
    }
}

/// The parking entry of the foreign object `t`.
fn park(t: &Object, foreign: &[Object], station: Option<AnyId>, ambiguous: bool) -> Parked {
    let module = |o: &Object| ParkedModule {
        id: o.id.clone(),
        name: o.name.clone(),
        designation: o.designation.clone(),
        position: o.position.unwrap_or(0),
        order_number: o.fields.get("order_number").cloned(),
        starts: (
            o.fields.get("start.input").and_then(|s| s.parse().ok()),
            o.fields.get("start.output").and_then(|s| s.parse().ok()),
        ),
        fields: o.fields.clone(),
    };
    let modules: Vec<ParkedModule> = match t.object {
        ExchangeObject::Station => foreign
            .iter()
            .filter(|o| {
                o.object == ExchangeObject::Module && !o.container && o.station.as_deref() == Some(&t.id)
            })
            .map(module)
            .collect(),
        ExchangeObject::Module => vec![module(t)],
        _ => Vec::new(),
    };
    let creatable = !ambiguous
        && match t.object {
            ExchangeObject::Station => true,
            ExchangeObject::Module => station.is_some(),
            _ => false,
        };
    Parked {
        id: t.id.clone(),
        object: t.object.clone(),
        name: t.name.clone(),
        ambiguous,
        station: if t.object == ExchangeObject::Module {
            station
        } else {
            None
        },
        designation: t.designation.clone(),
        fields: t.fields.clone(),
        modules,
        creatable,
    }
}

/// The change of one field, `None` when both sides agree.
#[allow(clippy::too_many_arguments)]
fn resolve(
    object: &ExchangeObject,
    target: AnyId,
    subject: &str,
    field: &str,
    s: Option<&String>,
    e: Option<&String>,
    t: Option<&String>,
    ownership: Option<&Ownership>,
) -> Option<Change> {
    if e == t {
        return None;
    }
    let (group, owner) = match class(object, field) {
        FieldClass::Group(g) => {
            let owner = Ownership::owner(ownership, &g);
            (Some(g), Some(owner))
        }
        FieldClass::Ecad => (None, None),
    };
    let (ours_changed, theirs_changed) = (e != s, t != s);
    let plc = owner == Some(Owner::Plc);
    let (state, default) = match (ours_changed, theirs_changed) {
        (false, _) if plc || owner == Some(Owner::Shared) => (State::Apply, Choice::Theirs),
        (false, _) => (State::Rejected, Choice::Ours),
        (true, false) if plc => (State::Pending, Choice::Ours),
        (true, false) => (State::Kept, Choice::Ours),
        (true, true) if plc => (State::Conflict, Choice::Theirs),
        (true, true) if group.is_none() => (State::Rejected, Choice::Ours),
        (true, true) => (State::Conflict, Choice::Ours),
    };
    Some(Change {
        id: format!("{}/{target}/{field}", object.name()),
        object: object.clone(),
        target,
        subject: subject.to_owned(),
        field: field.to_owned(),
        base: s.cloned(),
        ours: e.cloned(),
        theirs: t.cloned(),
        acceptable: group.is_some(),
        group,
        owner,
        state,
        default,
    })
}
