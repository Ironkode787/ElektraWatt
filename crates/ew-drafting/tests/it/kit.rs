// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Small pages for the tests, built in grid steps, and the fixtures of docs/design/06a-drafting.md §9.1: seven kinds
//! of page with invented devices, each as wired by hand and untidy.

use ew_drafting::consts::TICKS_PER_STEP;
use ew_drafting::{
    DraftPage, End, FixedText, Junction, JunctionId, Label, LabelId, LabelKind, Mark, MarkId, Owner, Part,
    PartId, Pin, PinId, Places, Pt, Rect, Role, Side, Tick, Wire, WireClass, WireId,
};
use ew_ids::Uuid;

/// `n` grid steps in ticks.
pub fn s(n: Tick) -> Tick {
    n * TICKS_PER_STEP
}

/// The point `(x, y)` in steps.
pub fn p(x: Tick, y: Tick) -> Pt {
    Pt::new(s(x), s(y))
}

/// The ID named `name`.
pub fn id(name: &str) -> Uuid {
    ew_ids::v5::imported_id::<()>(Uuid::nil(), name).as_uuid()
}

/// A tag's size: about three characters.
pub const TAG: (Tick, Tick) = (28, 10);
/// A short text's size: one or two characters.
pub const SHORT: (Tick, Tick) = (10, 10);

/// A page being built.
pub struct Page {
    pub page: DraftPage,
}

impl Page {
    /// An empty page whose drawing area is `w` by `h` steps.
    pub fn new(w: Tick, h: Tick) -> Page {
        Page {
            page: DraftPage {
                area: Rect::new(Pt::new(0, 0), p(w, h)),
                ..DraftPage::default()
            },
        }
    }

    /// A part `name` at `at` (steps) with its body `body` (steps, relative) and its pins (name, point in steps
    /// relative, side).
    pub fn part(&mut self, name: &str, at: Pt, body: (Pt, Pt), pins: &[(&str, Pt, Side)]) -> PartId {
        let part = PartId(id(name));
        self.page.parts.push(Part {
            id: part,
            at,
            body: Rect::new(body.0, body.1),
            frame_only: false,
            pins: pins
                .iter()
                .map(|(n, at, side)| Pin {
                    id: PinId(id(&format!("{name}:{n}"))),
                    at: *at,
                    side: Some(*side),
                    off_grid: false,
                })
                .collect(),
            unit: self.page.parts.len() as u32,
            role: Role::Other,
            locked: false,
            off_grid: false,
        });
        part
    }

    /// A two-pin upright symbol (a contact, a coil, a lamp, a fuse) at `at`: body two steps wide and four high, pins
    /// `1` on top and `2` at the bottom, with its tag at the upper left.
    pub fn device(&mut self, name: &str, at: Pt) -> PartId {
        let part = self.part(
            name,
            at,
            (p(-1, 0), p(1, 4)),
            &[("1", p(0, 0), Side::Up), ("2", p(0, 4), Side::Down)],
        );
        self.tag(part, TAG);
        part
    }

    /// A three-pole upright symbol at `at` (its left pole there): poles two steps apart, pins `1`, `3`, `5` on top and
    /// `2`, `4`, `6` at the bottom, `h` steps high, with its tag.
    pub fn three_pole(&mut self, name: &str, at: Pt, h: Tick) -> PartId {
        let part = self.part(
            name,
            at,
            (p(-1, 0), p(5, h)),
            &[
                ("1", p(0, 0), Side::Up),
                ("3", p(2, 0), Side::Up),
                ("5", p(4, 0), Side::Up),
                ("2", p(0, h), Side::Down),
                ("4", p(2, h), Side::Down),
                ("6", p(4, h), Side::Down),
            ],
        );
        self.tag(part, TAG);
        part
    }

    /// An incoming mark at `at` whose wire leaves right, with its name.
    pub fn mark_in(&mut self, name: &str, at: Pt) -> PartId {
        let part = self.part(name, at, (p(-2, -1), p(0, 1)), &[("1", p(0, 0), Side::Right)]);
        self.label(
            Owner::Part(part),
            0,
            LabelKind::MarkName,
            SHORT,
            Places::At(Vec::new()),
        );
        part
    }

    /// An outgoing mark at `at` whose wire leaves left, with its name.
    pub fn mark_out(&mut self, name: &str, at: Pt) -> PartId {
        let part = self.part(name, at, (p(0, -1), p(2, 1)), &[("1", p(0, 0), Side::Left)]);
        self.label(
            Owner::Part(part),
            0,
            LabelKind::MarkName,
            SHORT,
            Places::At(Vec::new()),
        );
        part
    }

    /// A terminal at `at` (its centre): pins one step above and below, its name.
    pub fn terminal(&mut self, name: &str, at: Pt) -> PartId {
        let part = self.part(
            name,
            at,
            (Pt::new(-6, -s(1)), Pt::new(6, s(1))),
            &[("1", p(0, -1), Side::Up), ("2", p(0, 1), Side::Down)],
        );
        self.label(
            Owner::Part(part),
            0,
            LabelKind::TerminalName,
            (8, 10),
            Places::At(Vec::new()),
        );
        part
    }

    /// A drawn box `w` by `h` steps at `at` (its upper left corner) with pins at `xs` (steps from its left edge) on
    /// its upper edge (`up`) or lower edge, each with a pin text inside, and its tag.
    pub fn card(&mut self, name: &str, at: Pt, (w, h): (Tick, Tick), xs: &[Tick], up: bool) -> PartId {
        let names: Vec<String> = (0..xs.len()).map(|i| format!("{i}")).collect();
        let pins: Vec<(&str, Pt, Side)> = xs
            .iter()
            .zip(&names)
            .map(|(x, n)| {
                if up {
                    (n.as_str(), p(*x, 0), Side::Up)
                } else {
                    (n.as_str(), p(*x, h), Side::Down)
                }
            })
            .collect();
        let part = self.part(name, at, (p(0, 0), p(w, h)), &pins);
        for x in xs {
            let (x, y0) = (at.x + s(*x), if up { at.y + 2 } else { at.y + s(h) - 22 });
            self.page.fixed_texts.push(FixedText {
                owner: part.0,
                rect: Rect::new(Pt::new(x - 5, y0), Pt::new(x + 5, y0 + 20)),
            });
        }
        self.label(
            Owner::Part(part),
            0,
            LabelKind::BoxTag,
            TAG,
            Places::At(Vec::new()),
        );
        part
    }

    /// The tag of `part`, at the places of its kind.
    pub fn tag(&mut self, part: PartId, size: (Tick, Tick)) {
        self.label(Owner::Part(part), 0, LabelKind::Tag, size, Places::At(Vec::new()));
    }

    /// A label.
    pub fn label(
        &mut self,
        owner: Owner,
        n: u32,
        kind: LabelKind,
        size: (Tick, Tick),
        places: Places,
    ) -> LabelId {
        let owner_id = match owner {
            Owner::Part(p) => p.0,
            Owner::Wire(w) => w.0,
            Owner::Mark(m) => m.0,
        };
        let label = LabelId { owner: owner_id, n };
        self.page.labels.retain(|l| l.id != label);
        self.page.labels.push(Label {
            id: label,
            owner,
            kind,
            size,
            places,
        });
        label
    }

    /// Pin `pin` of part `part`.
    pub fn pin(&self, part: &str, pin: &str) -> End {
        End::Pin(PinId(id(&format!("{part}:{pin}"))))
    }

    /// A junction `name` at `at`.
    pub fn junction(&mut self, name: &str, at: Pt) -> End {
        let j = JunctionId(id(name));
        self.page.junctions.push(Junction { id: j, at });
        End::Junction(j)
    }

    /// A wire `name` from `a` to `b` along `route` (both ends included; empty: unrouted).
    pub fn wire(&mut self, name: &str, a: End, b: End, route: &[Pt]) -> WireId {
        let w = WireId(id(name));
        self.page.wires.push(Wire {
            id: w,
            ends: [a, b],
            route: route.to_vec(),
            computed: false,
            class: WireClass::Wire,
            supply: None,
        });
        w
    }

    /// The point of an end on the page.
    pub fn at(&self, e: End) -> Pt {
        match e {
            End::Pin(pin) => self
                .page
                .parts
                .iter()
                .find_map(|part| part.pins.iter().find(|x| x.id == pin).map(|x| part.at.plus(x.at)))
                .expect("a pin of the page"),
            End::Junction(j) => {
                self.page
                    .junctions
                    .iter()
                    .find(|x| x.id == j)
                    .expect("a junction")
                    .at
            }
        }
    }

    /// A wire from `a` to `b` through the corners `via`.
    pub fn run(&mut self, name: &str, a: End, b: End, via: &[Pt]) -> WireId {
        let mut route = vec![self.at(a)];
        route.extend_from_slice(via);
        route.push(self.at(b));
        self.wire(name, a, b, &route)
    }

    /// A mark across wires.
    pub fn mark(&mut self, name: &str, line: Rect, horizontal: bool, wires: &[WireId]) -> MarkId {
        let m = MarkId(id(name));
        self.page.marks.push(Mark {
            id: m,
            line,
            horizontal,
            wires: wires.to_vec(),
        });
        m
    }
}

/// One kind of page of §1.4 as wired by hand, with the two parts the untidy state puts on the wrong side of their pin
/// and the move that does it.
pub struct Fixture {
    pub name: &'static str,
    pub hand: DraftPage,
    pub wrong_side: [(PartId, Pt); 2],
}

impl Fixture {
    /// The untidy state: every part shifted off its line by a fixed pattern, the two parts of `wrong_side` moved to
    /// the other side of the pin they are wired to, every wire unrouted.
    pub fn untidy(&self) -> DraftPage {
        let mut page = self.hand.clone();
        let mut parts: Vec<&mut Part> = page.parts.iter_mut().collect();
        parts.sort_by_key(|p| p.id);
        for (i, part) in parts.into_iter().enumerate() {
            let k = i as Tick;
            part.at = part.at.plus(Pt::new(s(k % 3 - 1), s(k % 2)));
            if let Some((_, d)) = self.wrong_side.iter().find(|(w, _)| *w == part.id) {
                part.at = part.at.plus(*d);
            }
        }
        for w in &mut page.wires {
            w.route.clear();
        }
        page
    }
}

/// The seven fixtures, the PLC input page twice (with marks only, and with four devices over terminals).
pub fn fixtures() -> Vec<Fixture> {
    vec![
        supply(),
        control(),
        plc_inputs(false),
        plc_inputs(true),
        plc_outputs(),
        bus(),
        pneumatic(),
        distribution(),
    ]
}

/// Supply and power: three feeders under four rails that run from marks at the left to marks at the right.
fn supply() -> Fixture {
    let mut b = Page::new(102, 64);
    let rails = ["L1", "L2", "L3", "N"];
    for (r, name) in rails.iter().enumerate() {
        let y = 4 + 2 * r as Tick;
        b.mark_in(&format!("in-{name}"), p(4, y));
        b.mark_out(&format!("out-{name}"), p(96, y));
    }
    let feeders = [20, 45, 70];
    for (f, &x) in feeders.iter().enumerate() {
        b.three_pole(&format!("Q{f}"), p(x, 14), 4);
        b.three_pole(&format!("K{f}"), p(x, 22), 4);
        let motor = b.part(
            &format!("M{f}"),
            p(x, 32),
            (p(-1, 0), p(5, 6)),
            &[
                ("U", p(0, 0), Side::Up),
                ("V", p(2, 0), Side::Up),
                ("W", p(4, 0), Side::Up),
            ],
        );
        b.tag(motor, TAG);
        for (a, c) in [("2", "1"), ("4", "3"), ("6", "5")] {
            b.run(
                &format!("Q{f}-K{f}:{a}"),
                b.pin(&format!("Q{f}"), a),
                b.pin(&format!("K{f}"), c),
                &[],
            );
        }
        for (a, c) in [("2", "U"), ("4", "V"), ("6", "W")] {
            b.run(
                &format!("K{f}-M{f}:{a}"),
                b.pin(&format!("K{f}"), a),
                b.pin(&format!("M{f}"), c),
                &[],
            );
        }
    }
    for (r, name) in rails.iter().enumerate() {
        let y = 4 + 2 * r as Tick;
        let mut from = b.pin(&format!("in-{name}"), "1");
        if r < 3 {
            for (f, &x) in feeders.iter().enumerate() {
                let j = b.junction(&format!("J{name}{f}"), p(x + 2 * r as Tick, y));
                b.run(&format!("{name}-{f}"), from, j, &[]);
                let pole = ["1", "3", "5"][r];
                b.run(&format!("{name}-drop{f}"), j, b.pin(&format!("Q{f}"), pole), &[]);
                from = j;
            }
        }
        b.run(
            &format!("{name}-end"),
            from,
            b.pin(&format!("out-{name}"), "1"),
            &[],
        );
    }
    Fixture {
        name: "supply",
        wrong_side: [(PartId(id("M1")), p(0, -20)), (PartId(id("K2")), p(0, -16))],
        hand: b.page,
    }
}

/// A control circuit: four paths between two trunks, with a holding branch and contact lists.
fn control() -> Fixture {
    let mut b = Page::new(102, 64);
    b.mark_in("in-L+", p(6, 6));
    b.mark_in("in-M", p(6, 40));
    let xs = [20, 31, 42, 53];
    for (i, &x) in xs.iter().enumerate() {
        b.device(&format!("S{i}"), p(x, 10));
        let contact = b.device(&format!("KC{i}"), p(x, 18));
        b.label(
            Owner::Part(contact),
            1,
            LabelKind::Xref,
            TAG,
            Places::At(vec![Pt::new(s(1) + 2, s(2))]),
        );
        let coil = b.part(
            &format!("KA{i}"),
            p(x, 30),
            (p(-1, 0), p(1, 4)),
            &[("1", p(0, 0), Side::Up), ("2", p(0, 4), Side::Down)],
        );
        b.label(
            Owner::Part(coil),
            0,
            LabelKind::Tag,
            TAG,
            Places::At(vec![Pt::new(-s(1) - 2 - 28, 0)]),
        );
        b.label(
            Owner::Part(coil),
            1,
            LabelKind::ContactList,
            (30, 30),
            Places::At(vec![Pt::new(-s(1) - 2 - 30, 12)]),
        );
        b.run(
            &format!("S{i}-KC{i}"),
            b.pin(&format!("S{i}"), "2"),
            b.pin(&format!("KC{i}"), "1"),
            &[],
        );
        b.run(
            &format!("KC{i}-KA{i}"),
            b.pin(&format!("KC{i}"), "2"),
            b.pin(&format!("KA{i}"), "1"),
            &[],
        );
    }
    // The holding branch beside path 0: a contact from a junction above the push button to one below it.
    b.device("H", p(26, 10));
    let (mut top, mut bottom) = (b.pin("in-L+", "1"), b.pin("in-M", "1"));
    for (i, &x) in xs.iter().enumerate() {
        let (s_top, coil_bottom) = (b.pin(&format!("S{i}"), "1"), b.pin(&format!("KA{i}"), "2"));
        if i + 1 == xs.len() {
            b.run(&format!("L+-{i}"), top, s_top, &[p(x, 6)]);
            b.run(&format!("M-{i}"), bottom, coil_bottom, &[p(x, 40)]);
            break;
        }
        let jt = b.junction(&format!("JT{i}"), p(x, 6));
        let jb = b.junction(&format!("JB{i}"), p(x, 40));
        b.run(&format!("L+-{i}"), top, jt, &[]);
        b.run(&format!("M-{i}"), bottom, jb, &[]);
        b.run(&format!("M-drop{i}"), coil_bottom, jb, &[]);
        if i == 0 {
            let h1 = b.junction("H1", p(x, 8));
            b.run("L+-drop0", jt, h1, &[]);
            b.run("H1-S0", h1, s_top, &[]);
            b.run("H1-H", h1, b.pin("H", "1"), &[p(26, 8)]);
        } else {
            b.run(&format!("L+-drop{i}"), jt, s_top, &[]);
        }
        (top, bottom) = (jt, jb);
    }
    // The holding contact's lower end joins the wire under the push button.
    let wire = b
        .page
        .wires
        .iter()
        .position(|w| w.id == WireId(id("S0-KC0")))
        .expect("the wire under S0");
    b.page.wires.remove(wire);
    let h2 = b.junction("H2", p(20, 16));
    b.run("S0-H2", b.pin("S0", "2"), h2, &[]);
    b.run("H2-KC0", h2, b.pin("KC0", "1"), &[]);
    b.run("H-H2", b.pin("H", "2"), h2, &[p(26, 16)]);
    Fixture {
        name: "control",
        wrong_side: [(PartId(id("KA1")), p(0, -16)), (PartId(id("KC2")), p(0, -16))],
        hand: b.page,
    }
}

/// PLC inputs: a card row at the foot, eight signals from a staircase of marks; with `devices`, four of them from
/// devices standing over their terminals instead.
fn plc_inputs(devices: bool) -> Fixture {
    let mut b = Page::new(102, 64);
    let xs: Vec<Tick> = (0..8).map(|i| 2 + 5 * i).collect();
    b.card("A1", p(10, 50), (44, 6), &xs, true);
    for (i, &x) in xs.iter().enumerate() {
        let x = 10 + x;
        let card = b.pin("A1", &format!("{i}"));
        if devices && i < 4 {
            b.device(&format!("B{i}"), p(x, 30));
            b.terminal(&format!("X{i}"), p(x, 42));
            b.run(
                &format!("B{i}-X{i}"),
                b.pin(&format!("B{i}"), "2"),
                b.pin(&format!("X{i}"), "1"),
                &[],
            );
            b.run(&format!("X{i}-A1"), b.pin(&format!("X{i}"), "2"), card, &[]);
        } else {
            let y = 46 - 2 * i as Tick;
            let mark = format!("in{i}");
            b.mark_in(&mark, p(x - 2, y));
            b.run(&format!("{mark}-A1"), b.pin(&mark, "1"), card, &[p(x, y)]);
        }
    }
    Fixture {
        name: if devices {
            "plc inputs with devices"
        } else {
            "plc inputs"
        },
        wrong_side: [
            if devices {
                (PartId(id("B1")), p(0, 30))
            } else {
                (PartId(id("in1")), p(0, 14))
            },
            (PartId(id("in6")), p(0, 24)),
        ],
        hand: b.page,
    }
}

/// PLC outputs: a card row at the head, eight loads over one trunk.
fn plc_outputs() -> Fixture {
    let mut b = Page::new(102, 64);
    let xs: Vec<Tick> = (0..8).map(|i| 2 + 5 * i).collect();
    b.card("A2", p(10, 8), (44, 6), &xs, false);
    b.mark_in("in-M", p(6, 34));
    let mut from = b.pin("in-M", "1");
    for (i, &x) in xs.iter().enumerate() {
        let x = 10 + x;
        b.device(&format!("P{i}"), p(x, 22));
        b.run(
            &format!("A2-P{i}"),
            b.pin("A2", &format!("{i}")),
            b.pin(&format!("P{i}"), "1"),
            &[],
        );
        let bottom = b.pin(&format!("P{i}"), "2");
        if i + 1 == xs.len() {
            b.run(&format!("M-{i}"), from, bottom, &[p(x, 34)]);
        } else {
            let j = b.junction(&format!("J{i}"), p(x, 34));
            b.run(&format!("M-{i}"), from, j, &[]);
            b.run(&format!("M-drop{i}"), bottom, j, &[]);
            from = j;
        }
    }
    Fixture {
        name: "plc outputs",
        wrong_side: [(PartId(id("P2")), p(0, -20)), (PartId(id("P5")), p(0, -20))],
        hand: b.page,
    }
}

/// A bus layout: a line of four stations and a switch, ports on top.
fn bus() -> Fixture {
    let mut b = Page::new(102, 64);
    let names = ["D0", "D1", "D2", "D3", "SW"];
    for (i, name) in names.iter().enumerate() {
        b.card(name, p(8 + 18 * i as Tick, 20), (8, 6), &[2, 6], true);
    }
    for i in 0..names.len() - 1 {
        let (a, c) = (b.pin(names[i], "1"), b.pin(names[i + 1], "0"));
        let (pa, pc) = (b.at(a), b.at(c));
        b.run(
            &format!("bus{i}"),
            a,
            c,
            &[Pt::new(pa.x, s(17)), Pt::new(pc.x, s(17))],
        );
    }
    for w in &mut b.page.wires {
        w.class = WireClass::Bus;
    }
    Fixture {
        name: "bus",
        wrong_side: [(PartId(id("D1")), p(0, 12)), (PartId(id("SW")), p(0, 12))],
        hand: b.page,
    }
}

/// A pneumatic circuit: a valve terminal with two valves and their actuators above, the service unit below.
fn pneumatic() -> Fixture {
    let mut b = Page::new(102, 64);
    let frame = b.part("VT", p(20, 30), (p(0, 0), p(20, 10)), &[]);
    if let Some(part) = b.page.parts.iter_mut().find(|x| x.id == frame) {
        part.frame_only = true;
    }
    b.tag(frame, TAG);
    for (i, x) in [22, 32].into_iter().enumerate() {
        let valve = b.part(
            &format!("V{i}"),
            p(x, 30),
            (p(0, 0), p(6, 10)),
            &[
                ("2", p(2, 0), Side::Up),
                ("4", p(4, 0), Side::Up),
                ("1", p(3, 10), Side::Down),
            ],
        );
        b.tag(valve, TAG);
        let cylinder = b.part(
            &format!("C{i}"),
            p(x, 14),
            (p(0, 0), p(6, 6)),
            &[("a", p(2, 6), Side::Down), ("b", p(4, 6), Side::Down)],
        );
        b.tag(cylinder, TAG);
        for (port, end) in [("2", "a"), ("4", "b")] {
            b.run(
                &format!("V{i}-C{i}:{port}"),
                b.pin(&format!("V{i}"), port),
                b.pin(&format!("C{i}"), end),
                &[],
            );
        }
    }
    let unit = b.part(
        "FRL",
        p(28, 50),
        (p(0, 0), p(4, 4)),
        &[("out", p(2, 0), Side::Up)],
    );
    b.tag(unit, TAG);
    let j = b.junction("JP", p(30, 46));
    b.run("FRL-JP", b.pin("FRL", "out"), j, &[]);
    b.run("JP-V0", j, b.pin("V0", "1"), &[p(25, 46)]);
    b.run("JP-V1", j, b.pin("V1", "1"), &[p(35, 46)]);
    for w in &mut b.page.wires {
        w.class = WireClass::Tube;
    }
    Fixture {
        name: "pneumatic",
        wrong_side: [(PartId(id("C0")), p(0, 30)), (PartId(id("FRL")), p(0, -40))],
        hand: b.page,
    }
}

/// Distribution: two terminal strips with stacks of marks, their wires nested.
fn distribution() -> Fixture {
    let mut b = Page::new(102, 64);
    let xs = [30, 32, 34, 36, 38, 40];
    for (i, &x) in xs.iter().enumerate() {
        b.terminal(&format!("X1-{i}"), p(x, 20));
        b.terminal(&format!("X2-{i}"), p(x, 40));
        b.run(
            &format!("X1-X2:{i}"),
            b.pin(&format!("X1-{i}"), "2"),
            b.pin(&format!("X2-{i}"), "1"),
            &[],
        );
    }
    for strip in ["X1", "X2"] {
        let first = PartId(id(&format!("{strip}-0")));
        b.label(
            Owner::Part(first),
            1,
            LabelKind::StripTag,
            (20, 10),
            Places::At(Vec::new()),
        );
    }
    for (k, &x) in xs.iter().rev().enumerate() {
        let i = xs.len() - 1 - k;
        let y = 8 + 2 * k as Tick;
        let mark = format!("in{k}");
        b.mark_in(&mark, p(8, y));
        b.run(
            &format!("{mark}-X1"),
            b.pin(&mark, "1"),
            b.pin(&format!("X1-{i}"), "1"),
            &[p(x, y)],
        );
        let y = 44 + 2 * k as Tick;
        let mark = format!("out{k}");
        b.mark_out(&mark, p(60, y));
        b.run(
            &format!("X2-{mark}"),
            b.pin(&format!("X2-{i}"), "2"),
            b.pin(&mark, "1"),
            &[p(x, y)],
        );
    }
    Fixture {
        name: "distribution",
        wrong_side: [(PartId(id("in2")), p(50, 0)), (PartId(id("out4")), p(-56, 0))],
        hand: b.page,
    }
}

impl Page {
    /// The ID of pin `pin` of part `part`.
    pub fn pin_id(&self, part: &str, pin: &str) -> PinId {
        PinId(id(&format!("{part}:{pin}")))
    }
}

/// The full sheet of §9.1: twelve feeders under three rails and forty control paths between two pairs of rails on one
/// A3 page, every wire unrouted, every junction on its rail.
pub fn full_sheet() -> DraftPage {
    let mut b = Page::new(102, 64);
    let rails = ["L1", "L2", "L3"];
    for (r, name) in rails.iter().enumerate() {
        let y = 4 + 2 * r as Tick;
        b.mark_in(&format!("in-{name}"), p(3, y));
        b.mark_out(&format!("out-{name}"), p(99, y));
    }
    let feeders: Vec<Tick> = (0..12).map(|k| 8 + 7 * k).collect();
    for (f, &x) in feeders.iter().enumerate() {
        b.three_pole(&format!("Q{f}"), p(x, 12), 4);
        b.three_pole(&format!("K{f}"), p(x, 19), 4);
        for (a, c) in [("2", "1"), ("4", "3"), ("6", "5")] {
            b.wire(
                &format!("Q{f}-K{f}:{a}"),
                b.pin(&format!("Q{f}"), a),
                b.pin(&format!("K{f}"), c),
                &[],
            );
        }
    }
    for (r, name) in rails.iter().enumerate() {
        let y = 4 + 2 * r as Tick;
        let mut from = b.pin(&format!("in-{name}"), "1");
        for (f, &x) in feeders.iter().enumerate() {
            let j = b.junction(&format!("J{name}{f}"), p(x + 2 * r as Tick, y));
            b.wire(&format!("{name}-{f}"), from, j, &[]);
            b.wire(
                &format!("{name}-drop{f}"),
                j,
                b.pin(&format!("Q{f}"), ["1", "3", "5"][r]),
                &[],
            );
            from = j;
        }
        b.wire(&format!("{name}-end"), from, b.pin(&format!("out-{name}"), "1"), &[]);
    }
    for (row, (top, bottom)) in [(30, 42), (46, 58)].into_iter().enumerate() {
        b.mark_in(&format!("in-L+{row}"), p(3, top));
        b.mark_in(&format!("in-M{row}"), p(3, bottom));
        let (mut t, mut m) = (b.pin(&format!("in-L+{row}"), "1"), b.pin(&format!("in-M{row}"), "1"));
        for k in 0..20 {
            let x = 8 + 4 * k;
            let (s_name, k_name) = (format!("S{row}-{k}"), format!("K{row}-{k}"));
            b.device(&s_name, p(x, top + 2));
            b.device(&k_name, p(x, top + 7));
            b.wire(&format!("{s_name}-{k_name}"), b.pin(&s_name, "2"), b.pin(&k_name, "1"), &[]);
            let jt = b.junction(&format!("JT{row}-{k}"), p(x, top));
            let jb = b.junction(&format!("JB{row}-{k}"), p(x, bottom));
            b.wire(&format!("L+{row}-{k}"), t, jt, &[]);
            b.wire(&format!("M{row}-{k}"), m, jb, &[]);
            b.wire(&format!("L+{row}-drop{k}"), jt, b.pin(&s_name, "1"), &[]);
            b.wire(&format!("M{row}-drop{k}"), b.pin(&k_name, "2"), jb, &[]);
            (t, m) = (jt, jb);
        }
    }
    b.page
}
