// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Matching a foreign image to the project and the per-field three-way merge (doc 10 §4.4, FR-PLC-032, FR-PLC-033).
//!
//! **Matching** (A6) runs kind by kind, a station before its modules and a module before its channels, and in
//! passes over every object of a kind, so that a weaker key never takes an object a stronger key gives to a later
//! object of the file: first the ID (one the project wrote, or one a foreign-ID record holds); then a station's or
//! module's IEC product designation (one that names no free object of the project parks the object), a channel's
//! number and direction, a node's interface label; then a module without a designation by its slot with its order
//! number. A station without an ID or a designation is parked: stations have no position to match by. A tag follows
//! its channel's link, else its ID, its name, then its address. A foreign object that matches nothing, or more than
//! one object, goes to the **parking list** and is never created silently. A module that only holds other modules
//! (a rack) is no object of the project and is passed over.
//!
//! **Scope.** Only the stations the file holds are compared; with `complete`, every station of the configuration
//! project. An object of a compared station that the last exchange held and the file no longer has is listed as
//! removed and never deleted.
//!
//! **Resolution** per field, with the base `S` (the last exchange), ours `E` and theirs `T`: equal sides are no
//! change; a change of theirs alone applies when the PLC or both own the field and is a rejected PLC change when
//! the schematic owns it; a change of ours alone is kept (export pending when the PLC owns the field); a change of
//! both is a conflict whose default is theirs when the PLC owns the field and ours otherwise. Without a snapshot of
//! the object, every difference is a change of both. A value the file leaves out or empty is no deletion: it is
//! never taken without a resolution. A comment with languages and one without never replace each other. A field
//! of no group (structure, designations, derived values) is the schematic's and cannot be accepted.

use std::collections::{BTreeMap, BTreeSet};

use ew_caex::ExchangeImage;
use ew_schema::{AnyId, ExchangeObject, FieldGroup, Owner, Ownership};

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
    /// Whether the file holds objects of kind `object` at all.
    pub fn holds(&self, object: &ExchangeObject) -> bool {
        *self == Vocabulary::All || *object == ExchangeObject::Tag
    }

    /// The comment field a table's comment column is for a tag whose project values are `ours`: the one the table
    /// names, or `comment` where the project's comment has no language and not that one (a text without a language
    /// reads the same in every language).
    fn column(&self, ours: &BTreeMap<String, String>) -> &str {
        match self {
            Vocabulary::Tags { comment }
                if comment != "comment" && !ours.contains_key(comment) && ours.contains_key("comment") =>
            {
                "comment"
            }
            Vocabulary::Tags { comment } => comment,
            Vocabulary::All => "comment",
        }
    }

    /// Whether the file carries `field` of an object of kind `object` whose project values are `ours`.
    pub fn carries(&self, object: &ExchangeObject, field: &str, ours: &BTreeMap<String, String>) -> bool {
        match self {
            Vocabulary::All => true,
            Vocabulary::Tags { .. } => {
                *object == ExchangeObject::Tag
                    && (matches!(field, "name" | "data_type" | "address") || field == self.column(ours))
            }
        }
    }

    /// What the snapshot holds for an object after an exchange of this format: the `carried` values, and for a
    /// field the format does not carry the value of the last exchange `base` (else the project's, `ours`), so that
    /// an exchange never re-bases a field it did not carry.
    pub fn next(
        &self,
        object: &ExchangeObject,
        ours: &BTreeMap<String, String>,
        base: Option<&BTreeMap<String, String>>,
        carried: BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let mut next = match self {
            Vocabulary::All => BTreeMap::new(),
            Vocabulary::Tags { .. } => base.unwrap_or(ours).clone(),
        };
        next.retain(|f, _| !self.carries(object, f, ours));
        next.extend(carried);
        next
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
    /// Changed in the file only and not taken without a resolution: the schematic owns it, or the file leaves out a
    /// value the project has (a rejected PLC change).
    Rejected,
    /// Changed in the project only: kept, and exported next time.
    Kept,
    /// Changed in the project only although the PLC owns it: kept, export pending.
    Pending,
    /// Changed on both sides, or on either side without a snapshot of the object.
    Conflict,
}

impl State {
    /// The key of the state's words (`plc-import-state-apply`, "applied"): the window renders it in its language, a
    /// notice holds it.
    pub fn key(self) -> &'static str {
        match self {
            State::Apply => "plc-import-state-apply",
            State::Rejected => "plc-import-state-rejected",
            State::Kept => "plc-import-state-kept",
            State::Pending => "plc-import-state-pending",
            State::Conflict => "plc-import-state-conflict",
        }
    }

    /// The state's name, as the command line's preview and `--json` give it (`conflict`).
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
    /// Whether the file's value can be taken at all: not for a field of no group, nor for a comment of the other
    /// form (with languages, without a language) than the project's.
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
    /// The field groups of which the file gives a value for a matched object.
    pub carried: Vec<FieldGroup>,
}

/// One object of the project's image.
struct Mine {
    kind: &'static str,
    target: AnyId,
    object: Object,
    taken: bool,
}

/// What matching made of a foreign object.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Decision {
    /// The project's object at this index.
    Mine(usize),
    /// Parked; whether several objects of the project match it.
    Parked(bool),
    /// Not looked at: below a parked object, a rack, or a kind the file's format does not carry.
    Skipped,
}

/// The kinds of objects in the order they are matched: a parent before what is below it.
const LEVELS: [ExchangeObject; 5] = [
    ExchangeObject::Station,
    ExchangeObject::Module,
    ExchangeObject::Channel,
    ExchangeObject::Node,
    ExchangeObject::Tag,
];

/// The passes of matching: see [`find`].
const PASSES: usize = 3;

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
    // every foreign object by its ID in the file (the first of a repeated ID)
    let index: BTreeMap<&str, usize> = foreign
        .iter()
        .enumerate()
        .rev()
        .map(|(i, o)| (o.id.as_str(), i))
        .collect();
    let mut decided: Vec<Option<Decision>> = vec![None; foreign.len()];
    // the project's object that the parent of a foreign object is, when the parent is matched
    let parent_of = |decided: &[Option<Decision>], t: &Object| -> Option<usize> {
        match decided[*index.get(t.parent.as_deref()?)?] {
            Some(Decision::Mine(m)) => Some(m),
            _ => None,
        }
    };

    for level in &LEVELS {
        let at: Vec<usize> = (0..foreign.len())
            .filter(|i| foreign[*i].object == *level)
            .collect();
        for &i in &at {
            let t = &foreign[i];
            let below_parked = [t.station.as_deref(), t.parent.as_deref()]
                .into_iter()
                .flatten()
                .filter_map(|id| index.get(id))
                .any(|j| matches!(decided[*j], Some(Decision::Parked(_) | Decision::Skipped)));
            if !options.vocabulary.holds(&t.object)
                || (t.object == ExchangeObject::Module && t.container)
                || below_parked
            {
                decided[i] = Some(Decision::Skipped);
            }
        }
        for pass in 0..PASSES {
            for &i in &at {
                if decided[i].is_some() {
                    continue;
                }
                let t = &foreign[i];
                let parent = parent_of(&decided, t);
                let Some(hit) = find(&mine, base, t, parent, pass) else {
                    continue;
                };
                decided[i] = Some(match hit {
                    Hit::One(m) => {
                        mine[m].taken = true;
                        Decision::Mine(m)
                    }
                    Hit::New(target) => {
                        // a tag on a matched channel that has none in the project: compared with the values a tag of
                        // the channel would have (its symbol without an address)
                        let channel = parent.map(|p| mine[p].object.id.clone()).unwrap_or_default();
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
                                fields: projection.unaddressed.get(&channel).cloned().unwrap_or_default(),
                            },
                            taken: true,
                        });
                        Decision::Mine(mine.len() - 1)
                    }
                    Hit::None => Decision::Parked(false),
                    Hit::Many => Decision::Parked(true),
                });
            }
        }
        for &i in &at {
            decided[i].get_or_insert(Decision::Parked(false));
        }
    }

    let mut compared: BTreeSet<String> = BTreeSet::new();
    for (i, t) in foreign.iter().enumerate() {
        let m = match decided[i] {
            Some(Decision::Mine(m)) => &mine[m],
            Some(Decision::Parked(ambiguous)) => {
                let station = parent_of(&decided, t).map(|p| mine[p].target);
                plan.parking.push(park(t, &foreign, station, ambiguous));
                continue;
            }
            _ => continue,
        };
        if t.object == ExchangeObject::Station {
            compared.insert(m.object.id.clone());
        }
        let s = base.records.get(&(m.kind, m.target));
        let mut theirs_fields = t.fields.clone();
        if let Vocabulary::Tags { comment } = &options.vocabulary {
            let column = options.vocabulary.column(&m.object.fields);
            if column != comment
                && let Some(text) = theirs_fields.remove(comment)
            {
                theirs_fields.insert(column.to_owned(), text);
            }
        }
        for field in theirs_fields.keys() {
            if let FieldClass::Group(g) = class(&t.object, field)
                && !plan.carried.contains(&g)
            {
                plan.carried.push(g);
            }
        }
        let covers = |f: &str| options.vocabulary.carries(&t.object, f, &m.object.fields);
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
                s,
                m.object.fields.get(field),
                theirs_fields.get(field),
                options.ownership,
            );
            plan.changes.extend(change.map(|mut c| {
                // the file's comment of the other form is never written: what would apply is a conflict
                if c.object == ExchangeObject::Tag
                    && c.theirs.is_some()
                    && crosses_languages(field, &m.object.fields)
                {
                    if c.state == State::Apply {
                        c.state = State::Conflict;
                    }
                    c.default = Choice::Ours;
                    c.acceptable = false;
                }
                c
            }));
        }
        plan.matched.push(Matched {
            object: t.object.clone(),
            target: m.target,
            foreign: (!t.id.is_empty() && t.id != m.object.id).then(|| t.id.clone()),
            fields: options
                .vocabulary
                .next(&t.object, &m.object.fields, s, theirs_fields),
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

/// Whether the file's value of comment field `field` would replace a project comment of the other form: a text
/// without a language over one with languages, or a language over a text without one.
fn crosses_languages(field: &str, ours: &BTreeMap<String, String>) -> bool {
    match field {
        "comment" => ours.keys().any(|k| k.starts_with("comment.")),
        f if f.starts_with("comment.") => ours.contains_key("comment"),
        _ => false,
    }
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

/// Pass `pass` of matching the foreign object `t` (A6), `parent` being the project's object its parent is; `None`
/// when this pass does not decide it. Pass 0: the ID the project wrote, or the one a foreign-ID record holds. Pass
/// 1: a station's or module's IEC designation (one that names no free object parks the object; a station without
/// one is parked too), a channel's number and direction, a node's interface label, a tag's channel link, else its
/// name. Pass 2: a module without a designation by its slot with its order number, a tag without a link by its
/// address.
fn find(mine: &[Mine], base: &Snapshot, t: &Object, parent: Option<usize>, pass: usize) -> Option<Hit> {
    let kind = t.object.name();
    let free: Vec<usize> = (0..mine.len())
        .filter(|i| mine[*i].kind == kind && !mine[*i].taken)
        .collect();
    let one = |candidates: Vec<usize>| match candidates.as_slice() {
        [] => Hit::None,
        [i] => Hit::One(*i),
        _ => Hit::Many,
    };
    let of = |f: &dyn Fn(&Object) -> bool| -> Vec<usize> {
        free.iter().copied().filter(|i| f(&mine[*i].object)).collect()
    };
    // the free objects of the project below the matched parent
    let below = |f: &dyn Fn(&Object) -> bool| -> Vec<usize> {
        let Some(p) = parent else {
            return Vec::new();
        };
        let id = mine[p].object.id.as_str();
        free.iter()
            .copied()
            .filter(|i| mine[*i].object.parent.as_deref() == Some(id) && f(&mine[*i].object))
            .collect()
    };
    match (pass, &t.object) {
        (0, _) => {
            if t.id.is_empty() {
                return None;
            }
            let by_foreign = base
                .foreign
                .get(&t.id)
                .filter(|(k, _)| *k == kind)
                .map(|(_, id)| *id);
            free.iter()
                .copied()
                .find(|i| mine[*i].object.id == t.id || Some(mine[*i].target) == by_foreign)
                .map(Hit::One)
        }
        (1, ExchangeObject::Station) => Some(match t.designation {
            Some(_) => one(of(&|o| o.designation == t.designation)),
            None => Hit::None,
        }),
        (1, ExchangeObject::Module) => t
            .designation
            .is_some()
            .then(|| one(below(&|o| o.designation == t.designation))),
        (2, ExchangeObject::Module) => Some(one(below(&|o| {
            o.position == t.position && o.fields.get("order_number") == t.fields.get("order_number")
        }))),
        (1, ExchangeObject::Channel) => Some(one(below(&|o| {
            o.position == t.position && o.fields.get("io") == t.fields.get("io")
        }))),
        (1, ExchangeObject::Node) => Some(one(below(&|o| o.designation == t.designation))),
        // the tag of the channel its link names; parked when that channel is not matched or its tag is taken
        (1, ExchangeObject::Tag) if t.parent.is_some() => Some(match parent {
            Some(p) => {
                let target = mine[p].target;
                match mine.iter().position(|m| m.kind == kind && m.target == target) {
                    Some(i) if !mine[i].taken => Hit::One(i),
                    Some(_) => Hit::None,
                    None => Hit::New(target),
                }
            }
            None => Hit::None,
        }),
        (1, ExchangeObject::Tag) => match one(of(&|o| o.name == t.name)) {
            Hit::None => None,
            hit => Some(hit),
        },
        (2, ExchangeObject::Tag) if t.parent.is_none() => Some(match t.fields.get("address") {
            Some(a) => one(of(&|o| o.fields.get("address") == Some(a))),
            None => Hit::None,
        }),
        _ => None,
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

/// The change of one field, `None` when both sides agree; `record` is the object's snapshot, `None` before its
/// first exchange.
#[allow(clippy::too_many_arguments)]
fn resolve(
    object: &ExchangeObject,
    target: AnyId,
    subject: &str,
    field: &str,
    record: Option<&BTreeMap<String, String>>,
    e: Option<&String>,
    t: Option<&String>,
    ownership: Option<&Ownership>,
) -> Option<Change> {
    if e == t {
        return None;
    }
    let s = record.and_then(|r| r.get(field));
    let (group, owner) = match class(object, field) {
        FieldClass::Group(g) => {
            let owner = Ownership::owner(ownership, &g);
            (Some(g), Some(owner))
        }
        FieldClass::Ecad => (None, None),
    };
    // without a snapshot every difference is a change of both sides: a conflict with the owner's default
    let (ours_changed, theirs_changed) = match record {
        Some(_) => (e != s, t != s),
        None => (true, true),
    };
    let plc = owner == Some(Owner::Plc);
    let (mut state, mut default) = match (ours_changed, theirs_changed) {
        (false, _) if plc || owner == Some(Owner::Shared) => (State::Apply, Choice::Theirs),
        (false, _) => (State::Rejected, Choice::Ours),
        (true, false) if plc => (State::Pending, Choice::Ours),
        (true, false) => (State::Kept, Choice::Ours),
        (true, true) if plc => (State::Conflict, Choice::Theirs),
        (true, true) if group.is_none() => (State::Rejected, Choice::Ours),
        (true, true) => (State::Conflict, Choice::Ours),
    };
    // a value the file leaves out is no deletion: it is listed and kept unless the file's side is chosen
    if t.is_none() {
        if state == State::Apply {
            state = State::Rejected;
        }
        default = Choice::Ours;
    }
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
