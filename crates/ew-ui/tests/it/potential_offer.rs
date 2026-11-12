// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1700: a wire whose potential can be reasoned is offered it on a card at the wire, with Accept and Decline, by the
//! pointer on a new project: relay -1K1's coil fed from L1 (a potential of type L1) and relay -1K2's from N (type N),
//! and a wire drawn between their A2 pins. Selected, that wire offers N ("-1K1:A2 returns a coil fed from L1"); Accept
//! defines it in one step, Decline keeps it from coming back for that wire.

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::visual::{Driver, Input};
use serde_json::json;

use crate::wires_and_cables::{click, key, new_project, page_pixel, tables, two_wires};

/// A new project with the two relays fed from the points L1 and N, the feed wires given the potentials `feeds` (the
/// wire, the name, the type), and a wire from -1K1:A2 to -1K2:A2 drawn with the Wire tool; the driver, that wire's
/// window pixel and the L1 feed wire's (the one ending at -1K1:A1).
fn a2_wire(tmp: &Tmp, feeds: &[(usize, &str, &str)]) -> (Driver, (i64, i64), (i64, i64)) {
    let mut d = new_project(tmp);
    let pixel = two_wires(&mut d);
    let t = tables(&mut d);
    // The feeds' types, as Define potential… sets them (not under test).
    for &(wire, name, ty) in feeds {
        let id = t.connections[wire].id.to_string();
        let shell = d.shell_mut();
        assert_eq!(
            shell.dispatch("select.set", &json!({ "ids": [id] })),
            Outcome::Done
        );
        let define = json!({ "name": name, "type": ty });
        assert_eq!(shell.dispatch("potential.draw", &define), Outcome::Done);
    }
    d.settle();
    let cp = |label: &str| t.cps.iter().find(|c| c.label == label).expect("the coil's A2").at;
    let (a, b) = (cp("-1K1:A2"), cp("-1K2:A2"));
    d.input(&click(d.find("Wire").unwrap()));
    d.settle();
    for p in [a, b] {
        let at = page_pixel(&mut d, &pixel, p);
        d.input(&Input::Hover { at });
        d.input(&click(at));
        d.settle();
    }
    d.input(&key("escape"));
    d.settle();
    d.input(&key("escape"));
    d.settle();
    let t = tables(&mut d);
    let wire = t
        .connections
        .iter()
        .find(|c| c.polyline.contains(&a) || c.polyline.contains(&b))
        .expect("the A2 wire");
    let longest = |c: &ew_ui::ew_viewmodel::ew_api::output::TableConnection| {
        c.polyline
            .windows(2)
            .map(|s| (s[0], s[1]))
            .max_by_key(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
            .expect("a segment")
    };
    let (p, q) = longest(wire);
    let on = page_pixel(&mut d, &pixel, ((2 * p.0 + q.0) / 3, (2 * p.1 + q.1) / 3));
    let a1 = t.cps.iter().find(|c| c.label == "-1K1:A1").expect("-1K1:A1");
    let feed = t
        .connections
        .iter()
        .find(|c| a1.connections.contains(&c.id))
        .expect("the L1 feed wire");
    let (p, q) = longest(feed);
    let l1 = page_pixel(&mut d, &pixel, ((p.0 + q.0) / 2, (p.1 + q.1) / 2));
    (d, on, l1)
}

/// Both feeds typed by Define potential….
const TYPED: &[(usize, &str, &str)] = &[(0, "L1", "l1"), (1, "N", "n")];

/// The card on show, if any: its title and why, as it prints them, while its Accept and Decline buttons show.
fn card(d: &mut Driver) -> Option<(String, String)> {
    d.find("Accept")?;
    d.find("Decline")?;
    d.shell_mut().potential_offer()
}

#[test]
fn a_reasoned_potential_is_offered_at_the_wire_and_accept_defines_it() {
    let tmp = Tmp::new();
    let (mut d, on, _) = a2_wire(&tmp, TYPED);
    assert_eq!(card(&mut d), None, "nothing selected, nothing offered");
    d.input(&click(on));
    d.settle();
    assert_eq!(
        card(&mut d),
        Some((
            "Potential N?".to_owned(),
            "-1K1:A2 returns a coil fed from L1".to_owned()
        ))
    );
    let steps = d.shell_mut().recorded_commands().len();
    d.input(&click(d.find("Accept").expect("Accept")));
    d.settle();
    let shell = d.shell_mut();
    assert_eq!(shell.recorded_commands().len(), steps + 1, "one step");
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "potential.define");
    assert_eq!(
        (last.params["name"].as_str(), last.params["ptype"].as_str()),
        (Some("N"), Some("n"))
    );
    d.settle();
    assert_eq!(card(&mut d), None, "named now: nothing more to offer");
    d.shell_mut().shutdown();
}

#[test]
fn a_declined_offer_does_not_come_back_for_that_wire() {
    let tmp = Tmp::new();
    let (mut d, on, _) = a2_wire(&tmp, TYPED);
    d.input(&click(on));
    d.settle();
    assert!(card(&mut d).is_some());
    let steps = d.shell_mut().recorded_commands().len();
    d.input(&click(d.find("Decline").expect("Decline")));
    d.settle();
    assert_eq!(card(&mut d), None);
    // Away and back: still not offered, and nothing was set.
    d.input(&key("escape"));
    d.settle();
    d.input(&Input::Click {
        at: on,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    d.settle();
    assert_eq!(card(&mut d), None, "declined for this wire");
    assert_eq!(d.shell_mut().recorded_commands().len(), steps);
    d.shell_mut().shutdown();
}

/// FT-1700, widened: with no potential of type N in the project (the N feed defined as NX, no type), the A2 wire is
/// still offered N, as a new potential of that type; Accept defines it with the type.
#[test]
fn without_a_named_potential_of_the_type_a_new_one_is_offered() {
    let tmp = Tmp::new();
    let (mut d, on, _) = a2_wire(&tmp, &[(0, "L1", "l1"), (1, "NX", "")]);
    d.input(&click(on));
    d.settle();
    let (title, why) = card(&mut d).expect("the card");
    assert!(title.starts_with("New potential N"), "{title}");
    assert_eq!(why, "-1K1:A2 returns a coil fed from L1");
    d.input(&click(d.find("Accept").expect("Accept")));
    d.settle();
    let shell = d.shell_mut();
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "potential.define");
    assert_eq!(last.params["ptype"].as_str(), Some("n"));
    shell.shutdown();
}

/// A play-tester of the second candidate: the coils fed from the points L1 and N, which name their potentials but give
/// them no type, and the A2 wire selected: no card (the feed's potential had no type). A name that is a type's own
/// word counts as that type: the A2 wire is offered the existing N, fed from L1; and the L1 wire, clicked, is offered
/// its type ("Type L1?"), which Accept sets in one step, keeping its name.
#[test]
fn a_feed_named_by_a_types_word_gives_the_offer_and_its_own_type() {
    let tmp = Tmp::new();
    let (mut d, on, at) = a2_wire(&tmp, &[]);
    d.input(&click(on));
    d.settle();
    assert_eq!(
        card(&mut d),
        Some((
            "Potential N?".to_owned(),
            "-1K1:A2 returns a coil fed from L1".to_owned()
        ))
    );
    d.input(&key("escape"));
    d.settle();
    // The L1 feed wire, clicked by the pointer.
    d.input(&Input::Hover { at });
    d.input(&click(at));
    d.settle();
    let (title, why) = card(&mut d).expect("the type is offered");
    assert_eq!(title, "Type L1?");
    assert!(why.contains("L1") && why.contains("colour"), "{why}");
    let steps = d.shell_mut().recorded_commands().len();
    d.input(&click(d.find("Accept").expect("Accept")));
    d.settle();
    let shell = d.shell_mut();
    assert_eq!(shell.recorded_commands().len(), steps + 1, "one step");
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "potential.define");
    assert_eq!(
        (last.params["name"].as_str(), last.params["ptype"].as_str()),
        (Some("L1"), Some("l1"))
    );
    d.settle();
    assert_eq!(card(&mut d), None, "typed now: nothing more to offer");
    d.shell_mut().shutdown();
}
