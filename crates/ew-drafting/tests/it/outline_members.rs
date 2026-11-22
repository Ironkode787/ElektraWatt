// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What joins an outline drawn round other parts (docs/design/06a-drafting.md §4.2, FT-2097): the parts set inside a
//! valve terminal's outline move with it, but a cylinder dragged inside it is no part of the terminal and Arrange
//! moves it out as any other.

use crate::arrange::{at, run};
use crate::kit::{Page, id, p, s};
use ew_drafting::{PageKind, PartId, Role, Side, WireClass};

/// A valve terminal of two valves (its outline 26 × 8 steps, the valves inside it), the second valve's cylinder over
/// it, the first valve's cylinder dragged inside the outline beside the first valve: Arrange sets that cylinder over
/// the terminal, in the order of the valves; the terminal stays.
#[test]
fn a_cylinder_dragged_inside_a_valve_terminal_s_outline_is_moved_out() {
    let mut b = Page::new(100, 64);
    b.page.kind = PageKind::Fluid;
    let frame = b.part("VT", p(30, 30), (p(0, 0), p(26, 8)), &[]);
    let unit = b.page.parts.iter().find(|x| x.id == frame).map(|x| x.unit);
    if let Some(part) = b.page.parts.iter_mut().find(|x| x.id == frame) {
        part.frame_only = true;
    }
    for (i, x) in [34, 46].into_iter().enumerate() {
        let valve = b.part(
            &format!("V{i}"),
            p(x, 30),
            (p(0, 0), p(8, 4)),
            &[("4", p(3, 0), Side::Up), ("2", p(5, 0), Side::Up)],
        );
        b.role(valve, Role::Valve);
        if let (Some(u), Some(part)) = (unit, b.page.parts.iter_mut().find(|x| x.id == valve)) {
            part.unit = u;
        }
        let place = if i == 0 { p(31, 31) } else { p(x + 8, 16) };
        let cylinder = b.part(
            &format!("C{i}"),
            place,
            (p(0, 0), p(2, 6)),
            &[("a", p(0, 1), Side::Left), ("b", p(0, 4), Side::Left)],
        );
        b.role(cylinder, Role::Actuator);
        for (port, end) in [("4", "a"), ("2", "b")] {
            b.wire(
                &format!("{port}{i}"),
                b.pin(&format!("V{i}"), port),
                b.pin(&format!("C{i}"), end),
                &[],
            );
        }
    }
    for w in &mut b.page.wires {
        w.class = WireClass::Tube;
    }
    let (a, done) = run(&b.page, &[]);
    assert!(
        a.moves.iter().any(|(x, _)| *x == PartId(id("C0"))),
        "{:?}",
        a.moves
    );
    let c0 = at(&done, "C0");
    assert!(c0.y + s(6) <= s(30), "the cylinder over the terminal: {c0:?}");
    assert!(c0.x < at(&done, "C1").x, "in the order of the valves");
    assert_eq!(at(&done, "VT"), p(30, 30), "the terminal stays");
}
