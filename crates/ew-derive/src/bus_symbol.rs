// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus symbol (doc 10 §2.7): a device on a bus-topology page is one body, derived from its part's network ports
//! at every draw and never stored. A representation names the built-in symbol `ew.gen.bus`, whose copy holds no
//! graphics; [`drawn`] gives the symbol the page draws it with and the pin map by which its sockets draw the device's
//! port functions.
//!
//! **The body** ([`bus_symbol`]), from the top: a band for the labels of the sockets on its top edge (when one has a
//! label), a header line with the device's tag and its part, a line, then a compartment per interface in the part's
//! order with dividers between them, each with the interface's name and its address, and a band for the labels of the
//! sockets on its bottom edge. Every height is the text it holds rounded up to whole steps of the page's grid, so the
//! sockets stand on that grid; a compartment is as wide as its sockets need and at least as wide as the interface's name
//! and an IPv4 address beside it; the header at least as wide as a tag and a part number. The size follows from the
//! part alone, never from an address or a tag: a text longer than its slot's `max_width` is cut on the page (the view's
//! rule), and stands whole in Properties and the lists.
//!
//! **Sockets.** An interface that counts n ports has the sockets `P1` … `Pn` (function CPs 0 … n−1), else one socket
//! (CP 0) that its compartment names; the first half stand on the compartment's top edge and the rest on its bottom
//! edge ([`crate::box_edges`]), each at the end of a stub one grid step outside the body, so a cable never reaches the
//! body. A socket of an interface with several is labelled by its port inside the body, turned a quarter.
//!
//! **No text of a device lies on a cable:** every text stands inside the body, clear of the socket bands.

use std::borrow::Cow;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::starter::{self, FN_GRAPHIC, GEN_BUS, VERSION_REQ};
use ew_schema::{
    Align, Bounds, BusPort, Dir4, Discipline, Extra, Function, KeyedList, LibRef, LinePrim, Part,
    PartBusPort, Pen, PinMapEntry, Primitive, RectPrim, RepType, Representation, SlotKind, Stroke, SymbolCp,
    SymbolDef, TextPrim, TextSlot,
};
use ew_units::{Angle2, Grid, Nm, Pt2};

use crate::Q;

/// The room between a text and the body's lines above and below it: a quarter millimetre, so a text's box never touches
/// a line.
const PAD: i64 = 250_000;
/// The room between a text and the body's left or right line, or a divider: three quarters of a millimetre, so a text
/// reads as inside its compartment, as on a drawn data sheet.
const INSET: i64 = 750_000;
/// The room between two texts on one line: three quarters of a millimetre.
const GAP: i64 = 750_000;
/// One line of text as a share of its style's em height: the shipped faces' ascent and descent (Noto Sans: 1069 + 293
/// per 1000 em), which the scene's text box measures, rounded up.
const LINE: (i64, i64) = (137, 100);
/// The advance of one character as a share of the em height, the most a tag, a part number or an address takes in the
/// shipped faces on average (Noto Sans' digits 0.57, its capitals up to 0.75); a longer text is cut, never moved.
const ADVANCE: (i64, i64) = (62, 100);
/// The characters the header keeps for the tag (`-105A10`, `=A1-K12`), before the part.
const TAG_CHARS: i64 = 8;
/// The characters the header keeps for the part (maker and number) after the tag.
const PART_CHARS: i64 = 14;
/// The characters of the longest IPv4 address (`255.255.255.255`), which every compartment holds beside its name.
const IPV4_CHARS: i64 = 15;
/// The style of the tag (as every device tag).
const TAG_STYLE: &str = "tag.main";
/// The style of the part, the interfaces' names and their addresses.
const DATA_STYLE: &str = "data.small";
/// The style of a socket's label (as a connection point's designation).
const SOCKET_STYLE: &str = "cp.small";

/// An interface and its port label, as a bus port names them.
type PortKey = (Ident, Option<Ident>);

/// A socket: the function CP it draws and its label.
type Socket = (u16, Option<String>);

/// One interface the body draws: its name, its port label and its socket count.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BusInterface {
    /// The interface ("X1", "DP").
    pub interface: Ident,
    /// The port label of an interface listed port by port ("P1R").
    pub port_label: Option<Ident>,
    /// Its sockets: the function CP each draws and its label (`P1` …), `None` for the one socket of an interface that
    /// counts no ports.
    pub sockets: Vec<Socket>,
}

impl BusInterface {
    /// The interface of `port`: its sockets `P1` … `Pn` for a count n, else one.
    pub fn of(port: &PartBusPort) -> BusInterface {
        Self::with(port.interface.clone(), port.port_label.clone(), port.ports)
    }

    fn with(interface: Ident, port_label: Option<Ident>, count: Option<u16>) -> BusInterface {
        let sockets = match count.filter(|n| *n > 0) {
            Some(n) => (0..n)
                .map(|i| (i, Some(format!("P{}", u32::from(i) + 1))))
                .collect(),
            None => vec![(0, None)],
        };
        BusInterface {
            interface,
            port_label,
            sockets,
        }
    }

    /// The interface in words: its name, then its port label after a space ("X1", "X1 P1R").
    pub fn words(&self) -> String {
        interface_words(&self.interface, self.port_label.as_ref())
    }
}

/// An interface and its port label in words ("X1", "X1 P1R").
pub fn interface_words(interface: &Ident, port_label: Option<&Ident>) -> String {
    match port_label {
        Some(l) => format!("{} {}", interface.as_str(), l.as_str()),
        None => interface.as_str().to_owned(),
    }
}

/// The interfaces of a part's network ports in the part's order, the first of each (interface, port label) pair; an
/// interface whose bus or role is of a newer format is left out (normaliser N5 gives it to no function).
pub fn bus_interfaces(part: &Part) -> Vec<BusInterface> {
    let Some(bus) = &part.sections.bus else {
        return Vec::new();
    };
    let mut out: Vec<BusInterface> = Vec::new();
    for p in &bus.ports {
        if out
            .iter()
            .any(|i| i.interface == p.interface && i.port_label == p.port_label)
            || matches!(p.bus, ew_schema::BusSystem::Unknown(_))
            || matches!(p.role, ew_schema::PortRole::Unknown(_))
        {
            continue;
        }
        out.push(BusInterface::of(p));
    }
    out
}

/// Whether `part` has network ports: a bus section with at least one port (what makes a part a device's bus part).
pub fn has_bus_ports(part: &Part) -> bool {
    part.sections.bus.as_ref().is_some_and(|b| !b.ports.is_empty())
}

/// Whether the symbol copy's item `copy` is the built-in bus symbol `ew.gen.bus` (either edition), which a page draws
/// derived.
pub fn is_bus_copy(copy: &SymbolDef) -> bool {
    copy.meta
        .key
        .as_ref()
        .is_some_and(|k| starter::edition_of(k.as_str()) == GEN_BUS)
}

/// Whether a page draws `symbol` as a bus symbol: one of its connection points names the interface it draws. Its texts
/// then stand where the symbol puts them (inside its body), and its sockets draw by [`bus_pins`].
pub fn is_bus_symbol(symbol: &SymbolDef) -> bool {
    symbol.cps.iter().any(|c| c.interface.is_some())
}

/// The bus symbol of `part` on a page of `grid` (see the module documentation).
pub fn bus_symbol(part: &Part, grid: Grid) -> SymbolDef {
    body(&bus_interfaces(part), grid)
}

/// The representation as a page draws it with `symbol`: for a bus symbol ([`is_bus_symbol`]) its pin map holds, for
/// each connection point that names an interface, function CP `fn_cp` of the first of `rep.functions` whose bus port
/// carries that interface and port label (`port` gives a function's); a connection point no drawn function carries
/// draws nothing. Any other representation is drawn as it is.
pub fn bus_pins<'r>(
    rep: &'r Representation,
    symbol: &SymbolDef,
    port: impl Fn(Id<Function>) -> Option<PortKey>,
) -> Cow<'r, Representation> {
    if !is_bus_symbol(symbol) {
        return Cow::Borrowed(rep);
    }
    let ports: Vec<(Id<Function>, PortKey)> = rep
        .functions
        .iter()
        .filter_map(|f| Some((*f, port(*f)?)))
        .collect();
    let entries: Vec<PinMapEntry> = symbol
        .cps
        .iter()
        .filter_map(|cp| {
            let interface = cp.interface.as_ref()?;
            let (function, _) = ports
                .iter()
                .find(|(_, (i, l))| i == interface && *l == cp.port_label)?;
            Some(PinMapEntry {
                symbol_cp: cp.idx,
                function: *function,
                idx: cp.fn_cp,
                extra: Extra::new(),
            })
        })
        .collect();
    let mut drawn = rep.clone();
    // The symbol's CP indices are unique (a keyed list), so the entries are too.
    drawn.pin_map = KeyedList::from_vec(entries).unwrap_or_default();
    Cow::Owned(drawn)
}

/// The symbol a page draws `rep` with and the representation as it draws it ([`bus_pins`]): a representation of
/// `ew.gen.bus` is drawn with its device's bus symbol (of the device's first part use whose part has network ports:
/// the part's own symbol where it names one the project holds, [`own_bus_symbol`], else [`bus_symbol`] on the page's
/// grid; a device without such a part, with the interfaces its drawn functions carry, a socket each); any other with
/// its copy's item. `None` when the project holds no copy of `rep.symbol`.
pub fn drawn<'q, 'r>(
    q: &'q Q,
    rep: &'r Representation,
) -> Option<(Cow<'q, SymbolDef>, Cow<'r, Representation>)> {
    let copy = &q.symbols().get(rep.symbol)?.item;
    let port = |f: Id<Function>| {
        let p = q.functions().get(f)?.bus_port.clone()?;
        Some((p.interface, p.port_label))
    };
    if !is_bus_copy(copy) {
        let rep = bus_pins(rep, copy, port);
        return Some((Cow::Borrowed(copy), rep));
    }
    let grid = q.pages().get(rep.page).map_or(Grid::MM4, |p| p.grid);
    let device = rep.functions.first().and_then(|f| q.functions().get(*f)?.device);
    let part = device.and_then(|d| {
        let parts = q.parts();
        q.devices()
            .get(d)?
            .parts
            .iter()
            .find_map(|u| parts.get(u.part.copy).filter(|c| has_bus_ports(&c.item)).cloned())
    });
    // His own symbol, where the part names one the project holds; never redrawn by the program.
    if let Some(own) = part.as_ref().and_then(|p| own_bus_symbol(q, &p.item)) {
        let rep = bus_pins(rep, own, port);
        return Some((Cow::Borrowed(own), rep));
    }
    let derived = match part {
        Some(p) => bus_symbol(&p.item, grid),
        None => carried_symbol(
            rep.functions
                .iter()
                .filter_map(|f| q.functions().get(*f)?.bus_port.clone()),
            grid,
        ),
    };
    let symbol = SymbolDef {
        meta: copy.meta.clone(),
        name: copy.name.clone(),
        keywords: copy.keywords.clone(),
        fndef: copy.fndef.clone(),
        ..derived
    };
    let rep = bus_pins(rep, &symbol, port);
    Some((Cow::Owned(symbol), rep))
}

/// The bus symbol of one's own that `part` names (`Part.bus_symbol`), as the project holds its copy; `None` without
/// one, or while the project does not hold it (the derived symbol draws then).
pub fn own_bus_symbol<'q>(q: &'q Q, part: &Part) -> Option<&'q SymbolDef> {
    let r = part.bus_symbol.as_ref()?;
    let copy = ew_ids::v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid());
    Some(&q.symbols().get(copy)?.item)
}

/// A socket of an interface in words: the interface, then the socket's label where it has several ("X1 P3", "DP").
fn socket_words(interface: &BusInterface, socket: &Socket) -> String {
    match &socket.1 {
        Some(label) => format!("{} {label}", interface.words()),
        None => interface.words(),
    }
}

/// How a bus symbol of one's own differs from the part's network ports, each socket in words ("X1 P3", "DP"): the
/// sockets of the ports it does not draw, then those it draws that the ports no longer have. Both empty: it draws
/// every socket. The part's Save card, What it lacks, the drawing's To do and EW-BUS-011 all say it from here.
pub fn socket_mismatch(symbol: &SymbolDef, part: &Part) -> (Vec<String>, Vec<String>) {
    let interfaces = bus_interfaces(part);
    let drawn = |i: &BusInterface, fn_cp: u16| {
        symbol.cps.iter().any(|c| {
            c.interface.as_ref() == Some(&i.interface) && c.port_label == i.port_label && c.fn_cp == fn_cp
        })
    };
    let missing = interfaces
        .iter()
        .flat_map(|i| i.sockets.iter().map(move |s| (i, s)))
        .filter(|(i, s)| !drawn(i, s.0))
        .map(|(i, s)| socket_words(i, s))
        .collect();
    let extra = symbol
        .cps
        .iter()
        .filter_map(|c| Some((c.interface.clone()?, c)))
        .filter(|(interface, c)| {
            !interfaces.iter().any(|i| {
                i.interface == *interface
                    && i.port_label == c.port_label
                    && i.sockets.iter().any(|s| s.0 == c.fn_cp)
            })
        })
        .map(|(interface, c)| {
            let i = BusInterface::with(interface, c.port_label.clone(), None);
            format!("{} P{}", i.words(), u32::from(c.fn_cp) + 1)
        })
        .collect();
    (missing, extra)
}

/// Whether every text of the bus symbol `symbol` stands inside its body (its first rectangle), clear of its sockets'
/// ways out: the one rule that no text of a device lies on a cable (doc 10 §2.7). A socket's way out is the strip inside
/// the body at the edge its stub leaves from, a label's line wide and as deep as its label "Pn" reaches (or a pad for a
/// socket without one); the socket's own label stands in it. A text's box is measured as the derivation sizes it:
/// the characters at their widest advance, a slot its room. A symbol without a body has no rule.
pub fn texts_clear(symbol: &SymbolDef) -> bool {
    let Some(Primitive::Rect(body)) = symbol.graphics.first() else {
        return true;
    };
    let (lo, hi) = (body.rect.0.min(), body.rect.0.max());
    let socket_em = em(SOCKET_STYLE);
    let half = (share(socket_em, LINE) + 2 * PAD) / 2;
    // Each socket's way out (min x, min y, max x, max y) and its label.
    let ways: Vec<([i64; 4], Option<String>)> = symbol
        .cps
        .iter()
        .filter(|c| c.interface.is_some())
        .map(|c| {
            let several = symbol
                .cps
                .iter()
                .filter(|o| o.interface == c.interface && o.port_label == c.port_label)
                .count()
                > 1;
            let label = several.then(|| format!("P{}", u32::from(c.fn_cp) + 1));
            let depth = label.as_deref().map_or(0, |l| advance(socket_em, chars(l))) + PAD;
            let (x, y) = (c.pos.x.0, c.pos.y.0);
            let way = match c.dir {
                Dir4::Up => [x - half, lo.y.0, x + half, lo.y.0 + depth],
                Dir4::Down => [x - half, hi.y.0 - depth, x + half, hi.y.0],
                Dir4::Left => [lo.x.0, y - half, lo.x.0 + depth, y + half],
                _ => [hi.x.0 - depth, y - half, hi.x.0, y + half],
            };
            (way, label)
        })
        .collect();
    let inside = |b: [i64; 4]| b[0] >= lo.x.0 && b[1] >= lo.y.0 && b[2] <= hi.x.0 && b[3] <= hi.y.0;
    let overlaps = |a: [i64; 4], b: [i64; 4]| a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];
    let clear = |b: [i64; 4]| inside(b) && !ways.iter().any(|(w, _)| overlaps(b, *w));
    let texts = symbol.graphics.iter().filter_map(|p| match p {
        Primitive::Text(t) => Some(t),
        _ => None,
    });
    for t in texts {
        let words = t.text.neutral_or_first().unwrap_or_default();
        let at = (t.pos.x.0, t.pos.y.0);
        let label = ways.iter().any(|(w, l)| {
            l.as_deref() == Some(words) && at.0 >= w[0] && at.0 <= w[2] && at.1 >= w[1] && at.1 <= w[3]
        });
        let em = em(t.style.as_ref().map_or(DATA_STYLE, |s| s.as_str()));
        let b = text_box(at, &t.align, t.angle, advance(em, chars(words)), share(em, LINE));
        if !label && !clear(b) {
            return false;
        }
    }
    symbol.text_slots.iter().filter(|s| s.visible).all(|s| {
        let style = match s.kind {
            SlotKind::Tag => TAG_STYLE,
            _ => DATA_STYLE,
        };
        let lines = if s.kind == SlotKind::BusAddress { 2 } else { 1 };
        let em = em(s.style.as_ref().map_or(style, |x| x.as_str()));
        let width = s.max_width.map_or(0, |w| w.0);
        clear(text_box(
            (s.pos.x.0, s.pos.y.0),
            &s.align,
            s.rot,
            width,
            lines * share(em, LINE),
        ))
    })
}

/// The box (min x, min y, max x, max y) a text of `width` and `height` takes at `at` with `align`, turned by `angle`
/// (a quarter turn reads from the bottom), y growing down the page.
fn text_box(at: (i64, i64), align: &Align, angle: Angle2, width: i64, height: i64) -> [i64; 4] {
    use Align as A;
    let (u0, u1) = match align {
        A::TopLeft | A::MiddleLeft | A::BottomLeft | A::BaselineLeft => (0, width),
        A::TopRight | A::MiddleRight | A::BottomRight | A::BaselineRight => (-width, 0),
        _ => (-width / 2, width - width / 2),
    };
    let (v0, v1) = match align {
        A::TopLeft | A::TopCentre | A::TopRight => (0, height),
        A::MiddleLeft | A::Centre | A::MiddleRight => (-height / 2, height - height / 2),
        _ => (-height, 0),
    };
    // Along the text and down its lines, after the turn: a quarter turn sends along to up the page, down to the right.
    let ((ax, ay), (dx, dy)): ((i64, i64), (i64, i64)) =
        match (angle.0.rem_euclid(360_000) + 45_000) / 90_000 % 4 {
            1 => ((0, -1), (1, 0)),
            2 => ((-1, 0), (0, -1)),
            3 => ((0, 1), (-1, 0)),
            _ => ((1, 0), (0, 1)),
        };
    let corners = [(u0, v0), (u0, v1), (u1, v0), (u1, v1)]
        .map(|(u, v)| (at.0 + u * ax + v * dx, at.1 + u * ay + v * dy));
    corners
        .iter()
        .fold([i64::MAX, i64::MAX, i64::MIN, i64::MIN], |b, (x, y)| {
            [b[0].min(*x), b[1].min(*y), b[2].max(*x), b[3].max(*y)]
        })
}

/// The address a bus symbol's compartment prints for `port` (a `bus_address` slot that names its interface): "TBD"
/// for the open item, a PROFINET device name and its IP address on two lines, any other address as the reports print
/// it ([`crate::net_address_text`]); `None` for a port without an address.
pub fn compartment_address(port: &BusPort) -> Option<String> {
    match &port.address {
        ew_schema::AddrValue::Tbd => Some("TBD".to_owned()),
        ew_schema::AddrValue::Value(ew_schema::NetAddress::Profinet(p)) => Some(match p.ip {
            Some(ip) => format!("{}\n{ip}", p.name),
            None => p.name.to_string(),
        }),
        ew_schema::AddrValue::Value(a) => crate::net_address_text(a),
        ew_schema::AddrValue::NotApplicable | ew_schema::AddrValue::Unknown(_) => None,
    }
}

/// The bus symbol of a device without a bus part on a page of `grid`: the interfaces its drawn functions' ports carry,
/// in their order, a socket each.
pub fn carried_symbol(ports: impl IntoIterator<Item = BusPort>, grid: Grid) -> SymbolDef {
    let interfaces: Vec<BusInterface> = ports
        .into_iter()
        .map(|p| BusInterface::with(p.interface, p.port_label, None))
        .collect();
    body(&interfaces, grid)
}

/// The em height of `style` in a new project.
fn em(style: &str) -> i64 {
    ew_schema::standard_text_style(style).map_or(0, |s| s.em.0)
}

/// `em × share`, for a share `(num, den)`.
fn share(em: i64, (num, den): (i64, i64)) -> i64 {
    em.saturating_mul(num) / den
}

/// The width `chars` characters of a style of em height `em` take at most ([`ADVANCE`]).
fn advance(em: i64, chars: i64) -> i64 {
    share(em, ADVANCE).saturating_mul(chars)
}

/// The number of characters of a text.
fn chars(text: &str) -> i64 {
    i64::try_from(text.chars().count()).unwrap_or(i64::MAX / 2)
}

/// The body of the interfaces `interfaces` on a page of `grid` (see the module documentation).
fn body(interfaces: &[BusInterface], grid: Grid) -> SymbolDef {
    // k grid steps in nm, and the fewest whole steps (at least one) that hold `nm`.
    let at = |k: i64| grid.at(k).map_or(0, |n| n.0);
    let steps = |nm: i64| -> i64 {
        let (num, den) = (i128::from(grid.num()), i128::from(grid.den()));
        let k = (i128::from(nm.max(1)) * den + num - 1) / num.max(1);
        i64::try_from(k).unwrap_or(i64::MAX / 4).max(1)
    };
    let (tag_em, data_em, socket_em) = (em(TAG_STYLE), em(DATA_STYLE), em(SOCKET_STYLE));
    let line = |em: i64| share(em, LINE);
    // Two sockets stand at least a turned label's line and its room apart.
    let pitch = steps(line(socket_em) + 2 * PAD);
    // Each interface's sockets on the top and on the bottom edge.
    let edges: Vec<(Vec<&Socket>, Vec<&Socket>)> = interfaces
        .iter()
        .map(|i| {
            let indices: Vec<u16> = i.sockets.iter().map(|s| s.0).collect();
            let (_, top) = crate::box_edges::split(&indices, &[]);
            let (a, b) = i.sockets.split_at(top.min(i.sockets.len()));
            (a.iter().collect(), b.iter().collect())
        })
        .collect();
    let band = |edge: &mut dyn Iterator<Item = &&Socket>| {
        edge.filter_map(|s| s.1.as_deref())
            .map(|l| steps(advance(socket_em, chars(l)) + 2 * PAD))
            .max()
            .unwrap_or(0)
    };
    let top_band = band(&mut edges.iter().flat_map(|(t, _)| t.iter()));
    let bottom_band = band(&mut edges.iter().flat_map(|(_, b)| b.iter()));
    let head = steps(line(tag_em) + 2 * PAD);
    let comp = steps(2 * line(data_em) + 2 * PAD);
    let names: Vec<String> = interfaces.iter().map(BusInterface::words).collect();
    let mut widths: Vec<i64> = interfaces
        .iter()
        .zip(&edges)
        .zip(&names)
        .map(|((_, (t, b)), name)| {
            let m = i64::try_from(t.len().max(b.len())).unwrap_or(i64::MAX / 4);
            let text = 2 * INSET + advance(data_em, chars(name)) + GAP + advance(data_em, IPV4_CHARS);
            steps(text).max(m.saturating_add(1).saturating_mul(pitch))
        })
        .collect();
    let tag_room = advance(tag_em, TAG_CHARS);
    let header = steps(2 * INSET + tag_room + GAP + advance(data_em, PART_CHARS));
    let sum: i64 = widths.iter().fold(0, |a, w| a.saturating_add(*w));
    if sum < header {
        match widths.last_mut() {
            Some(w) => *w += header - sum,
            None => widths.push(header),
        }
    }
    let w_steps: i64 = widths.iter().fold(0, |a, w| a.saturating_add(*w));
    let h_steps = top_band + head + comp + bottom_band;
    let (w, h) = (at(w_steps), at(h_steps));
    let (y_line, stub) = (at(top_band + head), at(1));
    let stroke = Some(Stroke {
        pen: if grid == Grid::QET_10PX {
            // The QElectroTech editions' 1 px pen, as the black-box frame's.
            Pen::Width(Grid::QET_1PX.at(1).unwrap_or(Nm(264_583)))
        } else {
            Pen::P026
        },
        dash: Default::default(),
        colour: Default::default(),
        extra: Extra::new(),
    });
    let p = |x: i64, y: i64| Pt2::new(Nm(x), Nm(y));
    let line_prim = |a: Pt2, b: Pt2| {
        Primitive::Line(LinePrim {
            from: a,
            to: b,
            stroke: stroke.clone(),
            extra: Extra::new(),
        })
    };
    let text_prim = |at: Pt2, text: &str, style: &str, angle: Angle2, align: Align| {
        Primitive::Text(TextPrim {
            pos: at,
            text: MlText::neutral(text),
            style: style.parse().ok(),
            align,
            angle,
            extra: Extra::new(),
        })
    };
    let mut graphics: Vec<Primitive> = Bounds::new(p(0, 0), p(w, h))
        .ok()
        .map(|rect| {
            Primitive::Rect(RectPrim {
                rect,
                rx: Nm::ZERO,
                ry: Nm::ZERO,
                shadow: None,
                stroke: stroke.clone(),
                fill: None,
                extra: Extra::new(),
            })
        })
        .into_iter()
        .collect();
    graphics.push(line_prim(p(0, y_line), p(w, y_line)));
    let header_y = at(top_band) + at(head) / 2;
    let mut text_slots = vec![
        slot(
            SlotKind::Tag,
            p(INSET, header_y),
            Align::MiddleLeft,
            tag_room,
            None,
        ),
        slot(
            SlotKind::PartNumber,
            p(INSET + tag_room + GAP, header_y),
            Align::MiddleLeft,
            w - 2 * INSET - tag_room - GAP,
            None,
        ),
    ];
    let mut cps = Vec::new();
    let mut x0_steps = 0i64;
    for (((interface, (top, bottom)), name), width) in interfaces.iter().zip(&edges).zip(&names).zip(&widths)
    {
        let x0 = at(x0_steps);
        if x0_steps > 0 {
            graphics.push(line_prim(p(x0, y_line), p(x0, h)));
        }
        let top_text = y_line + PAD;
        graphics.push(text_prim(
            p(x0 + INSET, top_text),
            name,
            DATA_STYLE,
            Angle2::ZERO,
            Align::TopLeft,
        ));
        let name_room = advance(data_em, chars(name)) + GAP;
        text_slots.push(slot(
            SlotKind::BusAddress,
            p(x0 + INSET + name_room, top_text),
            Align::TopLeft,
            at(*width) - 2 * INSET - name_room,
            Some(interface),
        ));
        for (edge, on_top) in [(top, true), (bottom, false)] {
            let m = i64::try_from(edge.len()).unwrap_or(0);
            for (k, (fn_cp, label)) in (0i64..).zip(edge.iter().copied()) {
                let x = at(x0_steps + crate::box_edges::spread(*width, m, k));
                let (edge_y, end_y, dir) = if on_top {
                    (0, -stub, Dir4::Up)
                } else {
                    (h, h + stub, Dir4::Down)
                };
                graphics.push(line_prim(p(x, edge_y), p(x, end_y)));
                if let Some(label) = label {
                    // Turned a quarter, reading from the bottom, from the edge into the band (the black box's rule
                    // for a pin text on a top or bottom edge).
                    let (y, align) = if on_top {
                        (PAD, Align::MiddleRight)
                    } else {
                        (h - PAD, Align::MiddleLeft)
                    };
                    graphics.push(text_prim(p(x, y), label, SOCKET_STYLE, Angle2(90_000), align));
                }
                cps.push(SymbolCp {
                    idx: u16::try_from(cps.len()).unwrap_or(u16::MAX),
                    fn_cp: *fn_cp,
                    pos: p(x, end_y),
                    dir,
                    discipline: Discipline::Bus,
                    interface: Some(interface.interface.clone()),
                    port_label: interface.port_label.clone(),
                    extra: Extra::new(),
                });
            }
        }
        x0_steps = x0_steps.saturating_add(*width);
    }
    SymbolDef {
        meta: ew_schema::ItemMeta {
            key: starter::for_grid(GEN_BUS, grid).parse().ok(),
            ..Default::default()
        },
        name: MlText::neutral("Bus symbol"),
        keywords: Default::default(),
        rep_type: RepType::Topology,
        fndef: LibRef {
            library: Id::from_uuid(starter::LIBRARY),
            item: Id::from_uuid(ew_ids::v5::library_item_id(starter::LIBRARY, FN_GRAPHIC)),
            version_req: VERSION_REQ.to_owned(),
            extra: Extra::new(),
        },
        grid,
        graphics,
        cps: KeyedList::from_vec(cps).unwrap_or_default(),
        text_slots,
        arrangements: Vec::new(),
        anchor: Default::default(),
        link_points: Vec::new(),
        extra_variants: Vec::new(),
        iec60617_ref: None,
        iso1219_ref: None,
        extra: Extra::new(),
    }
}

/// A visible text slot of the body, cut to `room` on the page; a `bus_address` slot names its interface.
fn slot(kind: SlotKind, pos: Pt2, align: Align, room: i64, interface: Option<&BusInterface>) -> TextSlot {
    TextSlot {
        kind,
        pos,
        rot: Angle2::ZERO,
        align,
        visible: true,
        max_width: Some(Nm(room.max(0))),
        style: None,
        fallback: None,
        solenoid: None,
        interface: interface.map(|i| i.interface.clone()),
        port_label: interface.and_then(|i| i.port_label.clone()),
        extra: Extra::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part of one's own with the network ports `ports` (JSON).
    fn part(ports: serde_json::Value) -> Part {
        serde_json::from_value(serde_json::json!({
            "manufacturer": {"library": "0190a000-0000-7000-8000-000000000001",
                "item": "0190a000-0000-7000-8000-000000000002", "version_req": "^1"},
            "part_number": "HS-2",
            "sections": {"bus": {"ports": ports}},
        }))
        .expect("a part")
    }

    fn head_station() -> Part {
        part(serde_json::json!([
            {"interface": "X1", "bus": "profinet", "role": "device", "ports": 2},
            {"interface": "X2", "bus": "profinet", "role": "device"},
        ]))
    }

    /// X1 of two ports and X2 of one are two compartments: X1 P1 on the top edge, X1 P2 on the bottom edge, X2's one
    /// socket on the top edge right of them, each a grid step outside the body and on the grid; a `bus_address` slot
    /// names each interface.
    #[test]
    fn two_interfaces_are_two_compartments_with_their_sockets_on_their_edges() {
        for grid in [Grid::QET_10PX, Grid::MM4] {
            let s = bus_symbol(&head_station(), grid);
            let Some(Primitive::Rect(body)) = s.graphics.first() else {
                panic!("the body first")
            };
            let (lo, hi) = (body.rect.0.min(), body.rect.0.max());
            let socket = |i: &str, fn_cp: u16| {
                s.cps
                    .iter()
                    .find(|c| c.interface.as_ref().map(Ident::as_str) == Some(i) && c.fn_cp == fn_cp)
                    .unwrap_or_else(|| panic!("socket {i} {fn_cp}"))
            };
            let (p1, p2, x2) = (socket("X1", 0), socket("X1", 1), socket("X2", 0));
            assert_eq!(
                (p1.dir.clone(), p2.dir.clone(), x2.dir.clone()),
                (Dir4::Up, Dir4::Down, Dir4::Up)
            );
            assert!(p1.pos.y < lo.y && x2.pos.y < lo.y && p2.pos.y > hi.y);
            assert!(x2.pos.x > p1.pos.x && x2.pos.x > p2.pos.x);
            assert_eq!(s.cps.len(), 3);
            for c in s.cps.iter() {
                for v in [c.pos.x, c.pos.y] {
                    assert_eq!(grid.snap_point(v), Some(v), "{c:?} on the grid");
                }
            }
            let addresses: Vec<&str> = s
                .text_slots
                .iter()
                .filter(|t| t.kind == SlotKind::BusAddress)
                .filter_map(|t| t.interface.as_ref().map(Ident::as_str))
                .collect();
            assert_eq!(addresses, ["X1", "X2"]);
        }
    }

    /// Every text slot stands inside the body, clear of the bands that the sockets' labels take on its top and bottom
    /// edge, and its room ends inside the body.
    #[test]
    fn every_text_slot_stands_inside_the_body_clear_of_the_socket_bands() {
        let s = bus_symbol(&head_station(), Grid::QET_10PX);
        let Some(Primitive::Rect(body)) = s.graphics.first() else {
            panic!("the body first")
        };
        let (lo, hi) = (body.rect.0.min(), body.rect.0.max());
        // The bands: from each edge to the far end of the labels standing on it.
        let labels: Vec<&TextPrim> = s
            .graphics
            .iter()
            .filter_map(|p| match p {
                Primitive::Text(t) if t.angle == Angle2(90_000) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(labels.len(), 2, "P1 and P2 labelled, X2's one socket not");
        // A label turned a quarter reaches from its point into the body by its length.
        let reach = |t: &TextPrim| {
            advance(
                em(SOCKET_STYLE),
                chars(t.text.neutral_or_first().unwrap_or_default()),
            )
        };
        for slot in &s.text_slots {
            assert!(slot.pos.x > lo.x && slot.pos.x < hi.x, "{slot:?}");
            assert!(slot.pos.y > lo.y && slot.pos.y < hi.y, "{slot:?}");
            for t in &labels {
                if t.align == Align::MiddleRight {
                    assert!(slot.pos.y.0 > t.pos.y.0 + reach(t), "{slot:?} in the top band");
                } else {
                    assert!(slot.pos.y.0 < t.pos.y.0 - reach(t), "{slot:?} in the bottom band");
                }
            }
            let room = slot.max_width.expect("a room");
            assert!(slot.pos.x + room < hi.x, "{slot:?} runs past the body");
        }
        assert!(
            texts_clear(&s),
            "the rule a drawing of one's own is held to holds for the derived symbol"
        );
    }

    /// A symbol of one's own with a socket at the middle of each of its four edges: a text in the middle of the body is
    /// clear; moved into any socket's way out, or out of the body, it is not (the rule the drawing refuses it by).
    #[test]
    fn a_text_in_a_socket_s_way_out_on_any_edge_or_outside_the_body_is_not_clear() {
        let mut s = bus_symbol(
            &part(serde_json::json!([{"interface": "X1", "bus": "profinet", "role": "device", "ports": 4}])),
            Grid::MM4,
        );
        let mm = |v: i64| v * 1_000_000;
        let (w, h) = (mm(60), mm(40));
        let body = Bounds::new(Pt2::new(Nm(0), Nm(0)), Pt2::new(Nm(w), Nm(h))).expect("a body");
        let Some(Primitive::Rect(rect)) = s.graphics.first().cloned() else {
            panic!("the body first")
        };
        let text = |x: i64, y: i64| {
            Primitive::Text(TextPrim {
                pos: Pt2::new(Nm(x), Nm(y)),
                text: MlText::neutral("X1"),
                style: DATA_STYLE.parse().ok(),
                align: Align::Centre,
                angle: Angle2::ZERO,
                extra: Extra::new(),
            })
        };
        let with = |s: &SymbolDef, x: i64, y: i64| {
            let mut m = s.clone();
            m.graphics = vec![
                Primitive::Rect(RectPrim {
                    rect: body,
                    ..rect.clone()
                }),
                text(x, y),
            ];
            m
        };
        s.text_slots.clear();
        let step = mm(4);
        let places = [
            (w / 2, -step, Dir4::Up),
            (w / 2, h + step, Dir4::Down),
            (-step, h / 2, Dir4::Left),
            (w + step, h / 2, Dir4::Right),
        ];
        let mut cps: Vec<SymbolCp> = s.cps.iter().cloned().collect();
        assert_eq!(cps.len(), 4);
        for (c, (x, y, dir)) in cps.iter_mut().zip(places) {
            c.pos = Pt2::new(Nm(x), Nm(y));
            c.dir = dir;
        }
        s.cps = KeyedList::from_vec(cps).expect("unique");
        assert!(texts_clear(&with(&s, w / 2, h / 2)), "in the middle");
        // A label "P1" reaches this deep into the body from its edge; the text's middle there lies in the way out.
        let depth = advance(em(SOCKET_STYLE), 2) + PAD;
        for (x, y, edge) in [
            (w / 2, depth, "top"),
            (w / 2, h - depth, "bottom"),
            (depth, h / 2, "left"),
            (w - depth, h / 2, "right"),
            (w + mm(10), h / 2, "outside"),
        ] {
            assert!(!texts_clear(&with(&s, x, y)), "{edge}");
        }
    }

    /// A symbol of one's own made when X1 had two ports lacks X1 P3 once it has three, and has one too many when it
    /// has one; it draws every socket of the ports it was made from.
    #[test]
    fn a_symbol_of_one_s_own_names_the_sockets_it_lacks_and_has_too_many() {
        let own = bus_symbol(&head_station(), Grid::MM4);
        let with = |n: u16| {
            part(serde_json::json!([
                {"interface": "X1", "bus": "profinet", "role": "device", "ports": n},
                {"interface": "X2", "bus": "profinet", "role": "device"},
            ]))
        };
        let none: (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        assert_eq!(socket_mismatch(&own, &head_station()), none);
        assert_eq!(
            socket_mismatch(&own, &with(3)),
            (vec!["X1 P3".to_owned()], Vec::new())
        );
        assert_eq!(
            socket_mismatch(&own, &with(1)),
            (Vec::new(), vec!["X1 P2".to_owned()])
        );
    }

    /// A part whose interface is of a newer bus has no compartment for it; one without network ports is a header alone.
    #[test]
    fn an_interface_of_a_newer_bus_is_left_out() {
        let s = bus_symbol(
            &part(serde_json::json!([
                {"interface": "X9", "bus": "a_bus_of_later", "role": "device"},
                {"interface": "DP", "bus": "profibus_dp", "role": "device"},
            ])),
            Grid::MM4,
        );
        assert!(
            s.cps
                .iter()
                .all(|c| c.interface.as_ref().map(Ident::as_str) == Some("DP"))
        );
        assert_eq!(s.cps.len(), 1);
        let none = bus_symbol(&part(serde_json::json!([])), Grid::MM4);
        assert!(none.cps.is_empty());
        assert!(matches!(none.graphics.first(), Some(Primitive::Rect(_))));
    }
}
