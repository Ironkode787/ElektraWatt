// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rework list (doc 13 §3.5): what the fitter changes in a built cabinet between a release and now.
//!
//! [`wire_changes`] matches the wires of two states: a wire of now is the wire of the release with the same ID, and a
//! wire whose carrier changed (a junction edit) is the wire of the release that ends at the same two CPs, so it is not
//! pulled and added again. The same matching gives the wire numbers before and after a change that a preview lists
//! (doc 16 §7.6). [`rework`] reads the matched pairs as the fitter's work, one [`ReworkItem`] per thing he does, in the
//! order he does it ([`ReworkAction`]): a wire that only moved on a page or took another route, a new number, a change
//! of source that keeps the value and a renamed potential are no work and give no item.

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_schema::{ColourSpec, Connection, Csa, PartRef};

use crate::joins::cp_label;
use crate::labels::{LabelRecord, LabelRows, LabelSet, LabelTarget};
use crate::nets::CpRef;
use crate::q::Q;
use crate::treatment::EndTreatment;
use crate::wire_record::{EndRecord, WireLength, WireRecord, WireRecordSet};
use crate::wires::{EndKind, PhysicalWire};

/// One wire of two states: its record in the earlier state (`before`, the release) and in the later one (`after`,
/// now); one of them is `None` for a wire that is gone or new. A record is a [`WireRecord`], or a bare [`PhysicalWire`]
/// where nothing more is read (a command's normaliser, doc 13 §2.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WirePair<'a, W = WireRecord> {
    /// The wire as it was.
    pub before: Option<&'a W>,
    /// The wire as it is.
    pub after: Option<&'a W>,
}

/// The wires of two states matched (see the module documentation): first the kept wires in the order of `after`, a
/// new wire among them where it stands; then the wires that are gone, in the order of `before`.
pub fn wire_changes<'a, W: AsRef<PhysicalWire>>(before: &'a [W], after: &'a [W]) -> Vec<WirePair<'a, W>> {
    let by_id: BTreeMap<_, usize> = before
        .iter()
        .enumerate()
        .map(|(i, r)| (r.as_ref().id, i))
        .collect();
    let mut taken = vec![false; before.len()];
    let mut matched: Vec<Option<usize>> = after
        .iter()
        .map(|r| {
            let i = *by_id.get(&r.as_ref().id)?;
            taken[i] = true;
            Some(i)
        })
        .collect();
    // A wire whose carrier changed keeps its two ends.
    let mut by_ends: BTreeMap<[CpRef; 2], Vec<usize>> = BTreeMap::new();
    for (i, r) in before.iter().enumerate().filter(|(i, _)| !taken[*i]) {
        by_ends.entry(ends_key(r)).or_default().push(i);
    }
    for (slot, r) in matched.iter_mut().zip(after) {
        if slot.is_none()
            && let Some(free) = by_ends.get_mut(&ends_key(r))
            && !free.is_empty()
        {
            let i = free.remove(0);
            taken[i] = true;
            *slot = Some(i);
        }
    }
    let mut out: Vec<WirePair<'a, W>> = matched
        .into_iter()
        .zip(after)
        .map(|(i, r)| WirePair {
            before: i.map(|i| &before[i]),
            after: Some(r),
        })
        .collect();
    out.extend(
        before
            .iter()
            .zip(&taken)
            .filter(|(_, t)| !**t)
            .map(|(r, _)| WirePair {
                before: Some(r),
                after: None,
            }),
    );
    out
}

/// A wire's two end CPs in order, the key of a wire matched by its ends.
fn ends_key<W: AsRef<PhysicalWire>>(r: &W) -> [CpRef; 2] {
    let w = r.as_ref();
    let mut ends = [w.ends[0].cp, w.ends[1].cp];
    ends.sort();
    ends
}

/// What the fitter does, in the order he does it (doc 13 §3.5); a row with several changes takes the first that
/// applies.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ReworkAction {
    /// A device label of the release is gone: the device comes out.
    RemoveDevice,
    /// A terminal marker of the release is gone: the terminal comes off its strip.
    RemoveTerminal,
    /// A cable marker of the release is gone: the cable comes out.
    RemoveCable,
    /// A wire of the release is gone.
    Pull,
    /// A kept wire changed its cross-section, colour or wire part.
    Replace,
    /// An end of a kept wire lands on another CP.
    MoveEnd,
    /// An end of a kept wire, on the same CP, takes another treatment or part.
    RemakeEnd,
    /// The length changed.
    CheckLength,
    /// A wire end's marker says another text.
    Remark,
    /// A device, terminal, strip or cable label says another text.
    Relabel,
    /// A device label is new: the device goes in.
    MountDevice,
    /// A terminal marker is new: the terminal goes on its strip.
    AddTerminal,
    /// A cable marker is new: the cable goes in.
    AddCable,
    /// A wire is new.
    Add,
}

/// One change of a kept wire, as the Change cell names it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReworkChange {
    /// The cross-section, before and after.
    Size(Option<Csa>, Option<Csa>),
    /// The colour, before and after.
    Colour(Option<ColourSpec>, Option<ColourSpec>),
    /// The wire part, by its part number before and after.
    WirePart(Option<String>, Option<String>),
    /// An end that lands on another CP: the CP before and after (`-104A1-A1:2`).
    End(String, String),
    /// An end on the same CP made another way: the CP, and the end before and after.
    RemakeEnd(String, EndMade, EndMade),
    /// A marker on the same CP that says another text: the CP, and its lines before and after (none: not marked).
    Remark(String, Vec<String>, Vec<String>),
    /// A label that says another text: its lines before and after.
    Relabel(Vec<String>, Vec<String>),
    /// The length, before and after.
    Length(WireLength, WireLength),
}

/// An end as a Re-make end change prints it: its treatment, the colour and stud of its part, the twin partner's number.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EndMade {
    /// The treatment.
    pub treatment: EndTreatment,
    /// The colour of its part.
    pub colour: Option<ColourSpec>,
    /// The stud of its part (a ring lug's).
    pub stud: Option<String>,
    /// The number of a twin ferrule's partner wire.
    pub partner: Option<String>,
}

/// What a row of the rework list is about.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReworkSubject {
    /// A wire, as the row prints it (as the release had it for a pulled wire, as it is now for every other).
    Wire(Box<WireRecord>),
    /// The label of a device, terminal, strip or cable, by its label set and target (as the release had it for one that
    /// is gone, as it is now for every other).
    Label(LabelSet, LabelTarget),
}

/// What a size or a colour that is not a wire's own follows ([`ReworkReason::Follows`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FollowedFrom {
    /// A connection the wire shares with other wires, by their numbers.
    Shared(Vec<String>),
    /// A potential's definition, by the potential's name.
    Potential(String),
    /// The project's rules: a profile's row, the project's default or the built-in one.
    Rules,
    /// The cable whose core it is, by the cable's tag.
    Cable(String),
    /// The workshop's stock, by the wire's size and colour (a wire part only).
    Stock,
}

/// A workshop setting a reason names ([`ReworkReason::Setting`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkshopSetting {
    /// Reserve per end.
    Reserve,
    /// Door loop.
    DoorLoop,
    /// Cut to: the cut increment.
    CutTo,
    /// Ferrules: the shop's ferrule rule.
    Ferrules,
    /// Two wires in one clamp.
    TwoWires,
    /// Stock.
    Stock,
}

/// Why a row of the rework list is there when nobody changed its wire by name (doc 13 §3.5): what other change it
/// follows from. Numbers and tags are those of the state that names them, the release's for what is gone.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReworkReason {
    /// The twin ferrule's partner on the clamp `at` is pulled.
    PartnerPulled {
        /// The clamp.
        at: String,
        /// The partner's number.
        partner: String,
    },
    /// The twin ferrule's partner changed its size.
    PartnerResized {
        /// The clamp.
        at: String,
        /// The partner's number.
        partner: String,
    },
    /// The twin ferrule's partner no longer lands on the clamp.
    PartnerLeft {
        /// The clamp.
        at: String,
        /// The partner's number.
        partner: String,
    },
    /// The twin ferrule's partner on the clamp `at` lands there still, and one of the two is made on site, the other in
    /// the shop: a twin is never made of the two (doc 13 §4.3 rule 3).
    SiteApart {
        /// The clamp.
        at: String,
        /// The partner's number.
        partner: String,
        /// The partner is the one made on site (else this wire is).
        partner_on_site: bool,
    },
    /// A wire that did not land on the clamp `at` in the release lands there now and shares the end's ferrule.
    Joined {
        /// The clamp.
        at: String,
        /// The other wire's number.
        wire: String,
    },
    /// The release's twin partner of the end's new partner on the clamp `at` is pulled, which freed the new partner.
    ClampPulled {
        /// The clamp.
        at: String,
        /// The pulled wire's number in the release.
        wire: String,
    },
    /// The release's twin partner of the end's new partner no longer lands on the clamp `at`.
    ClampLeft {
        /// The clamp.
        at: String,
        /// That wire's number.
        wire: String,
    },
    /// The release's twin partner of the end's new partner on the clamp `at` changed its size.
    ClampResized {
        /// The clamp.
        at: String,
        /// That wire's number.
        wire: String,
    },
    /// The part of the clamp `at` says other things of its clamp.
    ClampPart(String),
    /// A workshop setting that decides the row's change is another now.
    Setting(WorkshopSetting),
    /// The device an end landed on is gone, by its tag in the release.
    DeviceGone(String),
    /// The size (`size` true) or the colour is not set on the wire itself: it follows `from`.
    Follows {
        /// The size, else the colour.
        size: bool,
        /// What it follows.
        from: FollowedFrom,
    },
    /// The wire part is not named on the wire itself: it follows `from`.
    PartFollows(FollowedFrom),
    /// Its way was laid for the wiring of the release; Route lays it again.
    WayGone,
    /// Route laid its way, which it had none of in the release.
    Laid,
    /// Route laid its way again: its run or its door crossing is another.
    Rerouted,
    /// Its stored way no longer holds, and why.
    WayStale(crate::layout::StaleWhy),
}

/// One row of the rework list: an action on one wire or label, the thing as the row prints it, and its changes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReworkItem {
    /// What the fitter does.
    pub action: ReworkAction,
    /// What it is about.
    pub subject: ReworkSubject,
    /// What changed, in the order of [`ReworkAction`]; empty for a wire pulled or added.
    pub changes: Vec<ReworkChange>,
    /// Why the row is there where nobody changed its wire by name; empty for every other row.
    pub reasons: Vec<ReworkReason>,
    /// Where a device goes in (Mount device: now) or comes out (Remove device: in the release): its places in the
    /// layout ([`crate::layout::device_places`]); empty for every other row and a device not placed.
    pub places: Vec<crate::layout::DevicePlace>,
}

/// The rework list between the release `base` and `now` (doc 13 §3.5): the wires of [`wire_changes`] as the fitter's
/// work, by action and then in the numbering order of the state each row prints.
pub fn rework(base: &Q, now: &Q) -> Vec<ReworkItem> {
    let (before, after) = (base.get::<WireRecordSet>(()), now.get::<WireRecordSet>(()));
    let numbers = |set: &[WireRecord]| -> BTreeMap<Id<Connection>, Option<String>> {
        set.iter().map(|r| (r.wire.id, r.number.clone())).collect()
    };
    let (numbers_before, numbers_now) = (numbers(&before), numbers(&after));
    let wire = |action, r: &WireRecord| ReworkItem {
        action,
        subject: ReworkSubject::Wire(Box::new(r.clone())),
        changes: Vec::new(),
        reasons: Vec::new(),
        places: Vec::new(),
    };
    let states = Pair {
        base: State::of(base, &before, &numbers_before),
        now: State::of(now, &after, &numbers_now),
    };
    let mut items: Vec<ReworkItem> = wire_changes(&before, &after)
        .into_iter()
        .filter_map(|pair| match (pair.before, pair.after) {
            (Some(b), None) => Some(wire(ReworkAction::Pull, b)),
            (None, Some(a)) => Some(wire(ReworkAction::Add, a)),
            (Some(b), Some(a)) => {
                let changes = changes((base, &numbers_before), b, (now, &numbers_now), a);
                let action = changes.iter().map(action_of).min()?;
                Some(ReworkItem {
                    reasons: reasons(&states, b, a, &changes),
                    changes,
                    ..wire(action, a)
                })
            }
            (None, None) => None,
        })
        .collect();
    items.extend(label_work(base, now));
    // Stable: within an action the order of `wire_changes`, each state's numbering order, then the labels' order.
    items.sort_by_key(|i| i.action);
    items
}

/// The labels of devices, terminals, strips and cables between the two states (doc 13 §3.5): one gone (Remove
/// device, Remove terminal, Remove cable), one new (Mount device, Add terminal, Add cable) or one that says another text
/// (Re-label), matched by set and target. Wire ends' markers are the wires' (Re-mark); tubes are the fluid lists'.
fn label_work(base: &Q, now: &Q) -> Vec<ReworkItem> {
    let (before, after) = (base.get::<LabelRows>(()), now.get::<LabelRows>(()));
    let kept = |l: &&LabelRecord| {
        matches!(
            l.set,
            LabelSet::TerminalMarkers
                | LabelSet::StripMarkers
                | LabelSet::CableMarkers
                | LabelSet::DeviceLabels
        )
    };
    let key = |l: &LabelRecord| (l.set, l.target.clone());
    let find = |rows: &[LabelRecord], l: &LabelRecord| rows.iter().find(|x| key(x) == key(l)).cloned();
    // A device that goes in or comes out says where, in the state it stands in.
    let item = |action, l: &LabelRecord, changes| ReworkItem {
        action,
        subject: ReworkSubject::Label(l.set, l.target.clone()),
        changes,
        reasons: Vec::new(),
        places: match (action, &l.target) {
            (ReworkAction::MountDevice, LabelTarget::Device(d)) => crate::layout::device_places(now, *d),
            (ReworkAction::RemoveDevice, LabelTarget::Device(d)) => crate::layout::device_places(base, *d),
            _ => Vec::new(),
        },
    };
    let mut out = Vec::new();
    for b in before.iter().filter(kept) {
        let action = match b.set {
            LabelSet::DeviceLabels => ReworkAction::RemoveDevice,
            LabelSet::TerminalMarkers => ReworkAction::RemoveTerminal,
            LabelSet::CableMarkers => ReworkAction::RemoveCable,
            _ => continue,
        };
        if find(&after, b).is_none() {
            out.push(item(action, b, Vec::new()));
        }
    }
    for a in after.iter().filter(kept) {
        match find(&before, a) {
            None => {
                let action = match a.set {
                    LabelSet::DeviceLabels => ReworkAction::MountDevice,
                    LabelSet::TerminalMarkers => ReworkAction::AddTerminal,
                    LabelSet::CableMarkers => ReworkAction::AddCable,
                    _ => continue,
                };
                out.push(item(action, a, Vec::new()));
            }
            Some(b) if b.lines != a.lines => {
                out.push(item(
                    ReworkAction::Relabel,
                    a,
                    vec![ReworkChange::Relabel(b.lines.clone(), a.lines.clone())],
                ));
            }
            Some(_) => {}
        }
    }
    out
}

/// The action a change asks for.
fn action_of(change: &ReworkChange) -> ReworkAction {
    match change {
        ReworkChange::Size(..) | ReworkChange::Colour(..) | ReworkChange::WirePart(..) => {
            ReworkAction::Replace
        }
        ReworkChange::End(..) => ReworkAction::MoveEnd,
        ReworkChange::RemakeEnd(..) => ReworkAction::RemakeEnd,
        ReworkChange::Remark(..) => ReworkAction::Remark,
        ReworkChange::Relabel(..) => ReworkAction::Relabel,
        ReworkChange::Length(..) => ReworkAction::CheckLength,
    }
}

/// The printed numbers of a state's wires, by wire ID: what a twin ferrule names its partner by.
type Numbers = BTreeMap<Id<Connection>, Option<String>>;

/// What changed on a kept wire, `b` in the release and `a` now, each state with its wires' numbers; the values, never
/// their sources.
fn changes(
    (base, numbers_base): (&Q, &Numbers),
    b: &WireRecord,
    (now, numbers_now): (&Q, &Numbers),
    a: &WireRecord,
) -> Vec<ReworkChange> {
    let mut out = Vec::new();
    let size = |r: &WireRecord| r.csa.as_ref().map(|p| p.value.clone());
    let (size_b, size_a) = (size(b), size(a));
    if size_b != size_a {
        out.push(ReworkChange::Size(size_b, size_a));
    }
    let colour = |r: &WireRecord| r.colour.as_ref().map(|p| p.value.clone());
    let (colour_b, colour_a) = (colour(b), colour(a));
    if colour_b != colour_a {
        out.push(ReworkChange::Colour(colour_b, colour_a));
    }
    let (part_b, part_a) = (
        part_number(base, b.wire_part.as_ref()),
        part_number(now, a.wire_part.as_ref()),
    );
    if part_b != part_a {
        out.push(ReworkChange::WirePart(part_b, part_a));
    }
    // An end is compared with the release's end at the same place of the wire, From with From: orientation follows
    // the page order, so a moved end can turn a wire round; it is compared by whichever pairing keeps an end.
    let [b0, b1] = b.wire.ends.map(|e| e.cp);
    let [a0, a1] = a.wire.ends.map(|e| e.cp);
    let pairs = if a0 == b1 || a1 == b0 {
        [(b1, a0), (b0, a1)]
    } else {
        [(b0, a0), (b1, a1)]
    };
    for (end_b, end_a) in pairs {
        if end_b != end_a {
            out.push(ReworkChange::End(
                end_label(base, b, end_b),
                end_label(now, a, end_a),
            ));
        }
    }
    // An end kept on its CP that is made another way: its treatment (a twin ferrule by its partner wire, not by the
    // partner's number) or its part.
    for (end_b, end_a) in pairs {
        let at = |r: &WireRecord, cp: CpRef| r.wire.ends.iter().position(|e| e.cp == cp);
        let (Some(ib), Some(ia)) = (at(b, end_b), at(a, end_a)) else {
            continue;
        };
        let (eb, ea) = (&b.ends[ib], &a.ends[ia]);
        if end_b == end_a
            && (eb.treatment != ea.treatment
                || part_number(base, eb.part.map(PartRef::new).as_ref())
                    != part_number(now, ea.part.map(PartRef::new).as_ref()))
        {
            out.push(ReworkChange::RemakeEnd(
                end_label(now, a, end_a),
                made(base, numbers_base, eb),
                made(now, numbers_now, ea),
            ));
        }
        // The marker on the same CP that says another text (doc 13 §3.5 Re-mark).
        let lines = |e: &EndRecord| match &e.marker {
            crate::marking::MarkerText::Lines(l) => l.clone(),
            crate::marking::MarkerText::NotMarked(_) => Vec::new(),
        };
        if end_b == end_a && lines(eb) != lines(ea) {
            out.push(ReworkChange::Remark(
                end_label(now, a, end_a),
                lines(eb),
                lines(ea),
            ));
        }
    }
    // A length is compared by its value: a way laid again to the same length, or one gone stale, is no change.
    let changed = match (b.length.value(), a.length.value()) {
        (None, None) => b.length != a.length,
        (x, y) => x != y,
    };
    if changed {
        out.push(ReworkChange::Length(b.length, a.length));
    }
    out
}

/// An end as a Re-make end change prints it, the twin partner by its number in `numbers`.
fn made(q: &Q, numbers: &Numbers, e: &EndRecord) -> EndMade {
    let ferrule = e
        .part
        .and_then(|p| q.parts().get(p))
        .and_then(|c| c.item.sections.ferrule.clone());
    let partner = match e.treatment {
        EndTreatment::TwinFerrule { partner: Some(p), .. } => numbers.get(&p).cloned().flatten(),
        _ => None,
    };
    EndMade {
        treatment: e.treatment,
        colour: ferrule.as_ref().and_then(|f| f.colour.clone()),
        stud: ferrule.and_then(|f| f.stud).map(|s| s.as_str().to_owned()),
        partner,
    }
}

/// One state of the rework list as its reasons read it: its wires by ID and their printed numbers.
struct State<'a> {
    q: &'a Q,
    by_id: BTreeMap<Id<Connection>, &'a WireRecord>,
    numbers: &'a Numbers,
}

impl<'a> State<'a> {
    fn of(q: &'a Q, set: &'a [WireRecord], numbers: &'a Numbers) -> State<'a> {
        State {
            q,
            by_id: set.iter().map(|r| (r.wire.id, r)).collect(),
            numbers,
        }
    }

    /// The printed number of the wire `w`; `None` for a core and a wire this state does not have.
    fn number(&self, w: Id<Connection>) -> Option<String> {
        self.numbers.get(&w).cloned().flatten()
    }

    /// Whether the wire `w` lands on `cp` in this state.
    fn lands(&self, w: Id<Connection>, cp: CpRef) -> bool {
        self.by_id
            .get(&w)
            .is_some_and(|r| r.wire.ends.iter().any(|e| e.cp == cp))
    }
}

/// The release and now, as the reasons read them.
struct Pair<'a> {
    base: State<'a>,
    now: State<'a>,
}

/// Why a kept wire, `b` in the release and `a` now, has the changes `changes` where nobody changed it by name (doc 13
/// §3.5, "Because"). Each reason is a cause the two states show; a change whose cause they do not tell apart gets none:
///
/// - Replace: a size or colour not set on the wire itself follows its connection shared with other wires, its
///   potential, the project's rules or its cable ([`ReworkReason::Follows`]); a wire part not named on the wire follows
///   the same or the workshop's stock by size and colour ([`ReworkReason::PartFollows`]).
/// - Re-make end (an end on the same clamp, not set by hand, of a wire whose own size and wire part are kept): the
///   release's twin partner pulled, moved off the clamp, of another size, or now made on the other side of shop and
///   site ([`ReworkReason::SiteApart`]); the new partner new on the clamp ([`ReworkReason::Joined`]), of another size,
///   or freed because its own partner of the release was pulled, moved off or resized; else the workshop's Ferrules or Two wires
///   in one clamp, or the clamp's part, where they changed; a part alone re-made, the workshop's Stock.
/// - Move end: the device the end landed on is gone.
/// - Check length (a length no person typed): the way gone stale and why, the way laid for the release's wiring gone,
///   the way laid where there was none, laid again (another run or door crossing), and each of Reserve per end, Door
///   loop (a way into a door) and Cut to that changed.
fn reasons(s: &Pair, b: &WireRecord, a: &WireRecord, changes: &[ReworkChange]) -> Vec<ReworkReason> {
    let mut out = Vec::new();
    for c in changes {
        match c {
            ReworkChange::Size(..) => out.extend(follows(&s.now, a, true)),
            ReworkChange::Colour(..) => out.extend(follows(&s.now, a, false)),
            ReworkChange::WirePart(..) => out.extend(part_follows(&s.now, a)),
            _ => {}
        }
    }
    // A wire's own new size or wire part re-makes its ends: that change is in the row itself.
    let own = changes
        .iter()
        .any(|c| matches!(c, ReworkChange::Size(..) | ReworkChange::WirePart(..)));
    let size = |r: &WireRecord| r.csa.as_ref().map(|p| p.value.clone());
    let partner_at = |r: &WireRecord, cp: CpRef| {
        r.wire
            .ends
            .iter()
            .position(|e| e.cp == cp)
            .and_then(|i| match r.ends[i].treatment {
                EndTreatment::TwinFerrule { partner, .. } => partner,
                _ => None,
            })
    };
    let workshop = |q: &Q| q.project().settings.workshop.clone();
    let (shop_b, shop_a) = (workshop(s.base.q), workshop(s.now.q));
    for (ia, end) in a.wire.ends.iter().enumerate() {
        let cp = end.cp;
        let Some(ib) = b.wire.ends.iter().position(|e| e.cp == cp) else {
            continue;
        };
        let (eb, ea) = (&b.ends[ib], &a.ends[ia]);
        let by_hand = [eb, ea]
            .iter()
            .any(|e| e.treated_by == crate::treatment::TreatedBy::Hand);
        let parts = |e: &EndRecord, q: &Q| part_number(q, e.part.map(PartRef::new).as_ref());
        let reparted = parts(eb, s.base.q) != parts(ea, s.now.q);
        if own || by_hand || (eb.treatment == ea.treatment && !reparted) {
            continue;
        }
        if eb.treatment == ea.treatment {
            // The same treatment with another part: the stock gave it.
            if shop_b.stock != shop_a.stock {
                out.push(ReworkReason::Setting(WorkshopSetting::Stock));
            }
            continue;
        }
        let at = end_label(s.now.q, a, cp);
        let first = out.len();
        let (pb, pa) = (partner_at(b, cp), partner_at(a, cp));
        // The release's partner: pulled, moved off, of another size, or made on the other side of shop and site.
        if let Some(p) = pb.filter(|p| Some(*p) != pa)
            && let Some(partner) = s.base.number(p)
        {
            let at = at.clone();
            let before = s.base.by_id.get(&p);
            out.extend(match s.now.by_id.get(&p) {
                None => Some(ReworkReason::PartnerPulled { at, partner }),
                Some(_) if !s.now.lands(p, cp) => Some(ReworkReason::PartnerLeft { at, partner }),
                Some(r) if before.map(|x| size(x)) != Some(size(r)) => {
                    Some(ReworkReason::PartnerResized { at, partner })
                }
                Some(r)
                    if r.flags.field != a.flags.field
                        && before.is_some_and(|x| x.flags.field == b.flags.field) =>
                {
                    Some(ReworkReason::SiteApart {
                        at,
                        partner,
                        partner_on_site: r.flags.field,
                    })
                }
                Some(_) => None,
            });
        }
        // Now's partner: new on the clamp, of another size, or freed by its own partner of the release.
        if let Some(p) = pa.filter(|p| Some(*p) != pb)
            && let Some(wire) = s.now.number(p)
        {
            let before = s.base.by_id.get(&p);
            if !s.base.lands(p, cp) {
                out.push(ReworkReason::Joined { at: at.clone(), wire });
            } else if before.map(|x| size(x)) != s.now.by_id.get(&p).map(|r| size(r)) {
                out.push(ReworkReason::PartnerResized {
                    at: at.clone(),
                    partner: wire,
                });
            } else if let Some(freed) = before.and_then(|x| partner_at(x, cp)).filter(|o| *o != a.wire.id)
                && let Some(wire) = s.base.number(freed)
            {
                let at = at.clone();
                match s.now.by_id.get(&freed) {
                    None => out.push(ReworkReason::ClampPulled { at, wire }),
                    Some(_) if !s.now.lands(freed, cp) => out.push(ReworkReason::ClampLeft { at, wire }),
                    Some(r) if s.base.by_id.get(&freed).map(|x| size(x)) != Some(size(r)) => {
                        out.push(ReworkReason::ClampResized { at, wire });
                    }
                    Some(_) => {}
                }
            }
        }
        // Else what decides the end's treatment, where it changed.
        if out.len() == first {
            if shop_b.ferrules != shop_a.ferrules {
                out.push(ReworkReason::Setting(WorkshopSetting::Ferrules));
            }
            if shop_b.two_wires != shop_a.two_wires {
                out.push(ReworkReason::Setting(WorkshopSetting::TwoWires));
            }
            if crate::treatment::clamp_of(s.base.q, cp) != crate::treatment::clamp_of(s.now.q, cp) {
                out.push(ReworkReason::ClampPart(at));
            }
        }
    }
    // An end moved off a device that is gone.
    for e in &b.wire.ends {
        if a.wire.ends.iter().any(|x| x.cp == e.cp) {
            continue;
        }
        let device = s.base.q.functions().get(e.cp.function).and_then(|f| f.device);
        if let Some(d) = device
            && s.now.q.devices().get(d).is_none()
        {
            out.push(ReworkReason::DeviceGone(crate::short_tag(s.base.q, d)));
        }
    }
    // A length no person typed.
    if changes.iter().any(|c| matches!(c, ReworkChange::Length(..))) {
        // A way that went stale, or stale for another reason.
        if let WireLength::Stale { why, .. } = a.length
            && !matches!(b.length, WireLength::Stale { why: was, .. } if was == why)
            && !matches!(b.length, WireLength::Typed(_))
        {
            out.push(ReworkReason::WayStale(why));
        }
        match (b.length, a.length) {
            (WireLength::Typed(_), _) | (_, WireLength::Typed(_)) => {}
            (WireLength::Routed { .. } | WireLength::Stale { .. }, WireLength::Missing) => {
                out.push(ReworkReason::WayGone);
            }
            (WireLength::Missing, WireLength::Routed { .. } | WireLength::Stale { .. }) => {
                out.push(ReworkReason::Laid);
            }
            (
                WireLength::Routed { run, door, .. } | WireLength::Stale { run, door, .. },
                WireLength::Routed {
                    run: run_now,
                    door: door_now,
                    ..
                }
                | WireLength::Stale {
                    run: run_now,
                    door: door_now,
                    ..
                },
            ) => {
                if (run, door) != (run_now, door_now) {
                    out.push(ReworkReason::Rerouted);
                }
                use crate::layout::{door_loop, reserve};
                if reserve(s.base.q) != reserve(s.now.q) {
                    out.push(ReworkReason::Setting(WorkshopSetting::Reserve));
                }
                if (door || door_now) && door_loop(s.base.q) != door_loop(s.now.q) {
                    out.push(ReworkReason::Setting(WorkshopSetting::DoorLoop));
                }
                // The increment in force: what one nanometre is cut to.
                if crate::cut_to(s.base.q, 1) != crate::cut_to(s.now.q, 1) {
                    out.push(ReworkReason::Setting(WorkshopSetting::CutTo));
                }
            }
            _ => {}
        }
    }
    out.dedup();
    out
}

/// What the size (`size`) or the colour of `a` follows when it is not set on the wire's own connections; `None` when
/// it is, or when the wire has none.
fn follows(now: &State, a: &WireRecord, size: bool) -> Option<ReworkReason> {
    let source = if size {
        &a.csa.as_ref()?.source
    } else {
        &a.colour.as_ref()?.source
    };
    Some(ReworkReason::Follows {
        size,
        from: followed_from(now, a, source)?,
    })
}

/// What the wire part of `a` follows when it is not named on the wire's own connections: the source of the one its
/// chain names, else the workshop's stock; `None` when the wire names it.
fn part_follows(now: &State, a: &WireRecord) -> Option<ReworkReason> {
    let props = a.wire.potentials.iter().find_map(|p| {
        now.q
            .get::<crate::wire_props::WirePropsOf>(*p)
            .wires
            .get(&a.wire.id)
            .cloned()
    });
    let from = match props.and_then(|p| p.wire_part) {
        Some(named) => followed_from(now, a, &named.source)?,
        None => FollowedFrom::Stock,
    };
    Some(ReworkReason::PartFollows(from))
}

/// What a value of `a` from `source` follows; `None` for one set on the wire's own connections.
fn followed_from(
    now: &State,
    a: &WireRecord,
    source: &crate::wire_props::PropSource,
) -> Option<FollowedFrom> {
    use crate::wire_props::PropSource;
    Some(match source {
        PropSource::Override(c) if a.wire.own.contains(c) => return None,
        PropSource::Override(c) => FollowedFrom::Shared(
            now.by_id
                .values()
                .filter(|r| r.wire.id != a.wire.id && r.wire.constituents.contains(c))
                .filter_map(|r| now.number(r.wire.id))
                .collect(),
        ),
        PropSource::Conductor(c) => {
            let cable = now.q.table::<ew_schema::Conductor>().get(*c).map(|k| k.cable)?;
            FollowedFrom::Cable(crate::short_tag(now.q, cable))
        }
        PropSource::PotentialDef(p) => FollowedFrom::Potential(
            now.q
                .table::<ew_schema::PotentialDef>()
                .get(*p)
                .map(|d| d.name.as_str().to_owned())?,
        ),
        PropSource::ColourRow { .. }
        | PropSource::SizeRow { .. }
        | PropSource::ProjectDefault
        | PropSource::BuiltIn { .. } => FollowedFrom::Rules,
    })
}

/// An end as the Change cell names it: its CP's label, `ext. <name>` for an external end.
fn end_label(q: &Q, r: &WireRecord, cp: CpRef) -> String {
    let external = r
        .wire
        .ends
        .iter()
        .any(|e| e.cp == cp && e.kind == EndKind::External);
    let label = cp_label(q, cp);
    if external { format!("ext. {label}") } else { label }
}

/// The part number a wire part prints (`printed_part`), `None` without one or for a part the project does not hold.
fn part_number(q: &Q, part: Option<&PartRef>) -> Option<String> {
    part.and_then(|p| crate::printed_part(q, p))
        .map(|c| c.item.part_number.as_str().to_owned())
}
