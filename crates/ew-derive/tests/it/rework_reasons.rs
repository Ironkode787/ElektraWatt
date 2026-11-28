// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The reasons of the rework list (doc 13 §3.5, "Because"): each is a cause the release and now show, and a change whose
//! cause they do not tell apart has none. Three wires of one size land on the first CP of -K1 (0x1001), each from
//! another device: in the release 0x2001 and 0x2002 share a twin ferrule and 0x2003 takes its own.

use crate::common;

use common::{Fx, id, uid};
use ew_derive::{
    Q, ReworkAction, ReworkItem, ReworkReason, ReworkSubject, WireRecordSet, WorkshopSetting, rework,
};
use ew_ids::Id;
use ew_schema::Connection;
use serde_json::{Value, json};

const FNDEF: u128 = 0x500;
const SYM: u128 = 0x510;
const P1: u128 = 0x100;
/// The clamp the three wires land on, and the devices at their other ends.
const K1: u128 = 0x1001;
const W1: u128 = 0x2001;
const W2: u128 = 0x2002;
const W3: u128 = 0x2003;

/// Five devices of one two-CP function each on one page.
fn devices() -> Fx {
    let mut fx = Fx::new();
    fx.fndef(FNDEF, ["Misc", "Misc", "Plain"], &[(0, "1"), (1, "2")]);
    fx.symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)]);
    fx.page(P1, 1, None, "multi_line", &[], "one");
    for f in K1..=K1 + 4 {
        fx.device(f + 0x20_000, "K", Some(f as u32));
        fx.function(f, Some(f + 0x20_000), FNDEF, true);
        fx.rep(f + 0x10_000, P1, SYM, &[f], 0, f as i64 * 1_000_000);
    }
    fx
}

/// The wire `n` from the first CP of `from` to the first CP of `to`, 1.5 mm² with `more` overrides.
fn wire(fx: &mut Fx, n: u128, from: u128, to: u128, more: Value) {
    let mut overrides = json!({"csa": "1.5"});
    if let (Some(o), Some(m)) = (overrides.as_object_mut(), more.as_object()) {
        o.extend(m.clone());
    }
    fx.push(json!({
        "$k": "connection", "id": uid(n), "page": uid(P1),
        "a": {"cp": [uid(from), 0]}, "b": {"cp": [uid(to), 0]}, "overrides": overrides,
    }));
}

/// The release: 0x2001, 0x2002 and 0x2003 on -K1's first CP from -K2, -K3 and -K4.
fn release() -> Fx {
    let mut fx = devices();
    wire(&mut fx, W1, K1, K1 + 1, json!({}));
    wire(&mut fx, W2, K1, K1 + 2, json!({}));
    wire(&mut fx, W3, K1, K1 + 3, json!({}));
    fx
}

/// `fx` with its devices in a cabinet with a layout, -K5 in the field: a wire to -K5 is made on site.
fn located(mut fx: Fx) -> Fx {
    fx.node(0x900, "location", "CAB", None);
    fx.node(0x901, "location", "FLD", None);
    fx.push(json!({"$k": "layout_space", "id": uid(0x920), "location": uid(0x900)}));
    for f in K1..=K1 + 4 {
        let at = if f == K1 + 4 { 0x901 } else { 0x900 };
        fx.record(f + 0x20_000)["structure"] = json!({"location": uid(at)});
    }
    fx
}

/// The rework list from the release `base` to `now`.
fn work(base: &Fx, now: &Fx) -> (Vec<ReworkItem>, Q, Q) {
    let (b, a) = (base.build(), now.build());
    let (qb, qa) = (Q::new(&b), Q::new(&a));
    (rework(&qb, &qa), qb, qa)
}

/// The printed number of the wire `n` in `q`.
fn number(q: &Q, n: u128) -> String {
    let w: Id<Connection> = id(n);
    q.get::<WireRecordSet>(())
        .iter()
        .find(|r| r.wire.id == w)
        .and_then(|r| r.number.clone())
        .expect("a numbered wire")
}

/// The reasons of the row of the wire `n` with the action `action`.
fn reasons(items: &[ReworkItem], n: u128, action: ReworkAction) -> Vec<ReworkReason> {
    let w: Id<Connection> = id(n);
    items
        .iter()
        .find(|i| i.action == action && matches!(&i.subject, ReworkSubject::Wire(r) if r.wire.id == w))
        .unwrap_or_else(|| panic!("no {action:?} row of {n:#x}: {items:#?}"))
        .reasons
        .clone()
}

/// The clamp as the reasons name it.
fn clamp(items: &[ReworkItem]) -> String {
    items
        .iter()
        .flat_map(|i| &i.reasons)
        .find_map(|r| match r {
            ReworkReason::PartnerPulled { at, .. }
            | ReworkReason::PartnerLeft { at, .. }
            | ReworkReason::PartnerResized { at, .. }
            | ReworkReason::Joined { at, .. } => Some(at.clone()),
            _ => None,
        })
        .expect("a reason naming the clamp")
}

/// 0x2002 pulled: 0x2001 lost its partner, and 0x2003, which takes the twin now, is not said to land there anew (it
/// did in the release): its new partner was freed by the pull.
#[test]
fn a_pulled_partner_frees_the_wire_beside_it() {
    let base = release();
    let mut now = devices();
    wire(&mut now, W1, K1, K1 + 1, json!({}));
    wire(&mut now, W3, K1, K1 + 3, json!({}));
    let (items, qb, _) = work(&base, &now);
    let (at, pulled) = (clamp(&items), number(&qb, W2));
    assert_eq!(
        reasons(&items, W1, ReworkAction::RemakeEnd),
        vec![ReworkReason::PartnerPulled {
            at: at.clone(),
            partner: pulled.clone()
        }]
    );
    assert_eq!(
        reasons(&items, W3, ReworkAction::RemakeEnd),
        vec![ReworkReason::ClampPulled { at, wire: pulled }]
    );
}

/// 0x2002 moved off the clamp to -K5: 0x2001's partner left, and 0x2003 was freed by it.
#[test]
fn a_partner_moved_off_the_clamp_says_so() {
    let base = release();
    let mut now = devices();
    wire(&mut now, W1, K1, K1 + 1, json!({}));
    wire(&mut now, W2, K1 + 4, K1 + 2, json!({}));
    wire(&mut now, W3, K1, K1 + 3, json!({}));
    let (items, qb, _) = work(&base, &now);
    let at = clamp(&items);
    let moved = number(&qb, W2);
    assert_eq!(
        reasons(&items, W1, ReworkAction::RemakeEnd),
        vec![ReworkReason::PartnerLeft {
            at: at.clone(),
            partner: moved.clone()
        }]
    );
    assert_eq!(
        reasons(&items, W3, ReworkAction::RemakeEnd),
        vec![ReworkReason::ClampLeft { at, wire: moved }]
    );
}

/// 0x2002 made 2.5 mm²: 0x2001's partner changed size, and 0x2003, its new partner, was freed by that.
#[test]
fn a_partner_of_another_size_says_so() {
    let base = release();
    let mut now = devices();
    wire(&mut now, W1, K1, K1 + 1, json!({}));
    wire(&mut now, W2, K1, K1 + 2, json!({"csa": "2.5"}));
    wire(&mut now, W3, K1, K1 + 3, json!({}));
    let (items, qb, _) = work(&base, &now);
    let at = clamp(&items);
    let resized = number(&qb, W2);
    assert_eq!(
        reasons(&items, W1, ReworkAction::RemakeEnd),
        vec![ReworkReason::PartnerResized {
            at: at.clone(),
            partner: resized.clone()
        }]
    );
    assert_eq!(
        reasons(&items, W3, ReworkAction::RemakeEnd),
        vec![ReworkReason::ClampResized { at, wire: resized }]
    );
}

/// A wire new on the clamp shares the ferrule of the one that stood alone there: that one says it lands there now.
#[test]
fn a_wire_new_on_the_clamp_joins_it() {
    let mut base = devices();
    wire(&mut base, W1, K1, K1 + 1, json!({}));
    let mut now = devices();
    wire(&mut now, W1, K1, K1 + 1, json!({}));
    wire(&mut now, W2, K1, K1 + 2, json!({}));
    let (items, _, qa) = work(&base, &now);
    let joined = reasons(&items, W1, ReworkAction::RemakeEnd);
    assert!(
        matches!(&joined[..], [ReworkReason::Joined { wire, .. }] if *wire == number(&qa, W2)),
        "{joined:?}"
    );
}

/// The workshop's Two wires in one clamp made a ferrule each: the twin's two wires say so by the setting's name.
#[test]
fn a_workshop_setting_says_its_name() {
    let base = release();
    let mut now = release();
    now.project_set("settings", json!({"workshop": {"two_wires": "ferrule_each"}}));
    let (items, _, _) = work(&base, &now);
    for w in [W1, W2] {
        assert_eq!(
            reasons(&items, w, ReworkAction::RemakeEnd),
            vec![ReworkReason::Setting(WorkshopSetting::TwoWires)]
        );
    }
}

/// A wire recoloured by hand whose partner is pulled: the colour is the row's own change and no new size, so the
/// re-made end still says why.
#[test]
fn a_new_colour_keeps_the_partners_reason() {
    let base = release();
    let mut now = devices();
    wire(&mut now, W1, K1, K1 + 1, json!({"colour": ["BU"]}));
    wire(&mut now, W3, K1, K1 + 3, json!({}));
    let (items, qb, _) = work(&base, &now);
    let row = reasons(&items, W1, ReworkAction::Replace);
    assert!(
        row.iter()
            .any(|r| matches!(r, ReworkReason::PartnerPulled { partner, .. } if *partner == number(&qb, W2))),
        "{row:?}"
    );
}

/// 0x2002's other end moved out of the cabinet: it is made on site, so 0x2001, the shop's wire it shared a twin with,
/// shares 0x2003's now (doc 13 §4.3 rule 3), and says so: its partner neither left the clamp nor changed size.
#[test]
fn a_twin_split_between_shop_and_site_says_so() {
    let base = located(release());
    let mut now = devices();
    wire(&mut now, W1, K1, K1 + 1, json!({}));
    wire(&mut now, W2, K1, K1 + 4, json!({}));
    wire(&mut now, W3, K1, K1 + 3, json!({}));
    let now = located(now);
    let (items, qb, _) = work(&base, &now);
    let row = reasons(&items, W1, ReworkAction::RemakeEnd);
    assert!(
        matches!(&row[..], [ReworkReason::SiteApart { partner, partner_on_site: true, .. }]
            if *partner == number(&qb, W2)),
        "{row:?}"
    );
}

/// The end of the wire `n` on -K1's first CP in `q`.
fn on_k1(q: &Q, n: u128) -> ew_derive::EndRecord {
    let w: Id<Connection> = id(n);
    q.get::<WireRecordSet>(())
        .iter()
        .find(|r| r.wire.id == w)
        .and_then(|r| {
            let i = r
                .wire
                .ends
                .iter()
                .position(|e| e.cp == ew_derive::CpRef::new(id(K1), 0))?;
            Some(r.ends[i].clone())
        })
        .expect("the wire on -K1")
}

/// On -K1's first CP a wire of the shop and one made on site: by the rule each takes a ferrule and the field wire joins
/// the shop's on site (doc 13 §4.3 rule 3), and the clamp counts them as one conductor, as their twin was before that
/// rule, so a project releases as it did (EW-POT-010). Set twin by hand, they pair with each other: the rule decides
/// only what nobody set.
#[test]
fn a_shop_and_a_site_wire_join_by_the_rule_and_pair_by_hand() {
    let mut fx = devices();
    wire(&mut fx, W1, K1, K1 + 1, json!({}));
    wire(&mut fx, W2, K1, K1 + 4, json!({}));
    let m = located(fx).build();
    let q = Q::new(&m);
    let shop = on_k1(&q, W1);
    assert!(
        !matches!(shop.treatment, ew_derive::EndTreatment::TwinFerrule { .. }),
        "{shop:?}"
    );
    assert_eq!(shop.joined_on_site, Some(id(W2)));
    let counts = ew_derive::clamp_conductors(&q.get::<WireRecordSet>(()));
    assert_eq!(
        counts.get(&ew_derive::CpRef::new(id(K1), 0)),
        Some(&1),
        "{counts:?}"
    );
    let mut by_hand = devices();
    for (n, to) in [(W1, K1 + 1), (W2, K1 + 4)] {
        wire(
            &mut by_hand,
            n,
            K1,
            to,
            json!({"end_a": {"treatment": "twin_ferrule"}}),
        );
    }
    let m = located(by_hand).build();
    let q = Q::new(&m);
    let partner = |n: u128| match on_k1(&q, n).treatment {
        ew_derive::EndTreatment::TwinFerrule { partner, .. } => partner,
        _ => None,
    };
    assert_eq!((partner(W1), partner(W2)), (Some(id(W2)), Some(id(W1))));
    assert_eq!(on_k1(&q, W1).joined_on_site, None);
}
