// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The drawn bond (doc 08 §3.7) where the 0 V is named on another page or only by its name: a wire from that 0 V to an
//! earth point is the bond, so the 0 V and the earth stay two potentials and the 0 V is never typed PE.

use crate::common;

use common::*;
use ew_model::{EndpointKey, Model};
use ew_schema::PotentialType;

/// The potential at `e` and its type.
fn potential(m: &Model, e: Ep) -> (Option<ew_derive::PotentialId>, Option<PotentialType>) {
    let Ep::Cp(n, i) = e else { unreachable!("a CP") };
    let q = ew_derive::Q::new(m);
    let part = q.get::<ew_derive::PotentialPartition>(());
    let Some(p) = part.potential_at(EndpointKey::Cp(uid(n).cast(), i)).map(|p| p.id) else {
        return (None, None);
    };
    let ptype = q
        .get::<ew_derive::PotentialAttrsOf>(p)
        .attrs
        .ptype
        .as_ref()
        .map(|a| a.value.clone());
    (Some(p), ptype)
}

/// The 0 V side `zero` and the earth side `earth` are two potentials, the 0 V is not typed PE, and no EW-POT-001.
fn assert_bonded(m: &Model, zero: Ep, earth: Ep, what: &str) {
    let (z, z_type) = potential(m, zero);
    let (e, e_type) = potential(m, earth);
    assert!(z.is_some() && e.is_some(), "{what}: both sides are wired");
    assert_ne!(
        z, e,
        "{what}: the wire to earth is the bond, so the two stay apart"
    );
    assert_ne!(z_type, Some(PotentialType::Pe), "{what}: the 0 V stays 0 V");
    assert_eq!(e_type, Some(PotentialType::Pe), "{what}: the earth is PE");
    let f = findings_of(m, "EW-POT-001");
    assert!(f.is_empty(), "{what}: {f:#?}");
}

/// A 0 V named on page 1 (an M-role pin) comes to page 2 through a pair of interruption points whose name says nothing
/// (`X-1`); there a wire runs from the point to the earth symbol, and the 0 V goes on to a plain function. The wire is
/// the bond: the 0 V keeps its type M.
#[test]
fn a_0v_from_another_page_wired_to_earth_is_the_bond() {
    let mut fx = Fx::base();
    fx.func_m(0x1001, P1);
    fx.ip(0x1002, P1, "X-1", "source");
    fx.conn(0x2001, P1, Ep::Cp(0x1001, 1), cp(0x1002), &none());
    fx.ip(0x1003, P2, "X-1", "target");
    fx.func_pe(0x1004, P2);
    fx.func(0x1005, P2);
    fx.conn(0x2002, P2, cp(0x1003), Ep::Cp(0x1004, 1), &none());
    fx.conn(0x2003, P2, cp(0x1003), cp(0x1005), &none());
    let m = fx.build();
    assert_bonded(&m, cp(0x1005), Ep::Cp(0x1004, 1), "across one page link");
    assert_eq!(potential(&m, cp(0x1005)).1, Some(PotentialType::M));
}

/// The 0 V goes from page 1 to page 2 as `A`, on page 2 into the point `B`, and back to page 1 as `B`, where it is
/// wired to the earth symbol: the rule follows the potential over every page link, not only the first.
#[test]
fn the_bond_is_found_over_a_chain_of_page_links() {
    let mut fx = Fx::base();
    fx.func_m(0x1001, P1);
    fx.ip(0x1002, P1, "A", "source");
    fx.conn(0x2001, P1, Ep::Cp(0x1001, 1), cp(0x1002), &none());
    fx.ip(0x1003, P2, "A", "target");
    fx.ip(0x1004, P2, "B", "source");
    fx.conn(0x2002, P2, cp(0x1003), cp(0x1004), &none());
    fx.ip(0x1005, P1, "B", "target");
    fx.func_pe(0x1006, P1);
    fx.conn(0x2003, P1, cp(0x1005), Ep::Cp(0x1006, 1), &none());
    let m = fx.build();
    assert_bonded(&m, cp(0x1005), Ep::Cp(0x1006, 1), "over two page links");
}

/// A supply whose pins say nothing (an imported symbol): its minus is wired to a mark named `0VDC-6` and, on the same
/// page, to the earth symbol. The mark's name says 0 V, so the wire is the bond and the 0 V is not typed PE as a whole;
/// so on another page, where the mark `0VDC-6` arrives and is earthed again.
#[test]
fn a_0v_known_only_by_its_name_stays_0v_when_it_is_earthed() {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.ip(0x1002, P1, "0VDC-6", "source");
    fx.func_pe(0x1003, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    fx.conn(0x2002, P1, cp(0x1001), Ep::Cp(0x1003, 1), &none());
    let m = fx.build();
    assert_bonded(&m, cp(0x1001), Ep::Cp(0x1003, 1), "on the supply's page");

    fx.ip(0x1004, P2, "0VDC-6", "target");
    fx.func(0x1005, P2);
    fx.conn(0x2003, P2, cp(0x1004), cp(0x1005), &none());
    let mut fx2 = fx.clone();
    fx2.func_pe(0x1006, P2);
    fx2.conn(0x2004, P2, cp(0x1005), Ep::Cp(0x1006, 1), &none());
    // The second earth wire makes the 0 V's bond twice; that is EW-POT-002's, not a merge.
    let m = fx2.build();
    let (z, z_type) = potential(&m, cp(0x1005));
    let (e, _) = potential(&m, Ep::Cp(0x1006, 1));
    assert_ne!(z, e, "on the page the mark comes to");
    assert_ne!(z_type, Some(PotentialType::Pe));
}
