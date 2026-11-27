// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The representations of a page's view (see the module documentation of `view`): their symbols, their slot texts,
//! a black box's pin texts and room, and the terminals a strip segment draws.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A valve standing in a cell of a valve terminal's box drawn on `page` stands with its insertion point on the cell's
/// top line, so a PLC address place of its symbol higher than one grid step under that point ran across the line (the
/// valves with one solenoid). There each such place is drawn one step of the symbol's grid under it, the row of the
/// two-solenoid valves' places, inside the cell; a place a person moved, a turned valve and a valve drawn anywhere else
/// keep the symbol's place. Drawn only: the record is not changed.
fn addresses_in_cell(
    q: &Q,
    page: Id<Page>,
    rep: Arc<Representation>,
    def: &SymbolDef,
) -> Arc<Representation> {
    let Some(step) = def.grid.at(1).zip(def.grid.at(0)).map(|(a, b)| a - b) else {
        return rep;
    };
    let device_of = |r: &Representation| r.functions.first().and_then(|f| q.functions().get(*f)?.device);
    // The modules that hold the valve: its parent and theirs.
    let holders: Vec<Id<Device>> = device_of(&rep)
        .map(|d| ew_derive::parent_chain(q, d).into_iter().skip(1).collect())
        .unwrap_or_default();
    let in_cell = rep.orient == ew_units::Orient2::R0
        && !holders.is_empty()
        && q.idx().reps_by_page(page).any(|r| {
            q.reps().get(r).is_some_and(|b| {
                b.box_.as_ref().is_some_and(|x| x.slots.is_some())
                    && device_of(b).is_some_and(|d| holders.contains(&d))
            })
        });
    if !in_cell {
        return rep;
    }
    let mut out = (*rep).clone();
    for (i, slot) in def.text_slots.iter().enumerate() {
        let Ok(index) = u16::try_from(i) else { break };
        if slot.kind == SlotKind::PlcAddress && slot.pos.y < step && out.text_slots.get(&index).is_none() {
            out.text_slots.insert(ew_schema::SlotOverride {
                slot: index,
                pos: Some(ew_units::Pt2::new(slot.pos.x, step)),
                angle: None,
                visible: None,
                extra: Default::default(),
            });
        }
    }
    Arc::new(out)
}

/// `page_reps(page)`: the representations of a drawn page with their symbol copies and, for a strip segment, the
/// terminals it draws; no slot texts (those are [`PageSlotsOf`]).
#[derive(Debug)]
pub struct PageRepsOf;

impl Query for PageRepsOf {
    type Key = Id<Page>;
    type Value = Vec<RepView>;
    const FAMILY: &'static str = "page_reps";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<RepView>> {
        let mut out = Vec::new();
        for rid in q.idx().reps_by_page(*key) {
            let Some(stored) = q.reps().get(rid).cloned() else {
                continue;
            };
            let terminals = segment_terminals(q, &stored);
            // The symbol the page draws it with, and the pin map a bus symbol's sockets draw by (`ew_derive::drawn`).
            let (symbol, rep) = match ew_derive::drawn(q, &stored) {
                Some((symbol, rep)) => (Some(Arc::new(symbol.into_owned())), Arc::new(rep.into_owned())),
                None => (None, stored),
            };
            let rep = match &symbol {
                Some(def) => addresses_in_cell(q, *key, rep, def),
                None => rep,
            };
            let terminal = symbol.as_ref().is_some_and(|s| ew_derive::terminal_symbol(q, s));
            out.push(RepView {
                rep,
                symbol,
                terminal,
                slots: Vec::new(),
                terminals,
                pins: Vec::new(),
            });
        }
        Arc::new(out)
    }
}

/// `page_slots(page)`: the slot texts and the pin texts of every representation of a drawn page that prints one, by
/// representation ID (see the module documentation).
#[derive(Debug)]
pub struct PageSlotsOf;

impl Query for PageSlotsOf {
    type Key = Id<Page>;
    type Value = Vec<(Id<Representation>, Vec<SlotValue>, Vec<PinText>)>;
    const FAMILY: &'static str = "page_slots";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Self::Value> {
        let mut out = Vec::new();
        for rid in q.idx().reps_by_page(*key) {
            let Some(stored) = q.reps().get(rid) else {
                continue;
            };
            let Some((symbol, rep)) = ew_derive::drawn(q, stored) else {
                continue;
            };
            let slots = slot_values(q, *key, &rep, &symbol);
            let pins = pin_texts(q, &rep, &symbol);
            if !slots.is_empty() || !pins.is_empty() {
                out.push((rep.id, slots, pins));
            }
        }
        out.sort_by_key(|(id, _, _)| *id);
        Arc::new(out)
    }
}

/// The pin texts of a black box: per box CP of a representation whose symbol has no CPs, the
/// designation of that CP of its first function as stored (repeated designations print as they are), then, when the
/// CP is bound to a PLC channel, the channel's printed address after one space; in box-CP order. A designation that
/// a `cp_designation` slot of the symbol already prints, or an empty one, is left out (no text twice); a box CP with
/// neither a designation to print nor an address has no entry.
fn pin_texts(q: &Q, rep: &Representation, def: &SymbolDef) -> Vec<PinText> {
    let (Some(b), Some(&function)) = (&rep.box_, rep.functions.first()) else {
        return Vec::new();
    };
    if !def.cps.is_empty() || b.cps.is_empty() {
        return Vec::new();
    }
    // The one socket of an interface's box (an interface that counts no ports, doc 10 §2.7) is named by the
    // interface, as a single-line bus page names it, not by the port function's first pin ("A" of the DP port).
    if let ([cp], Some(label)) = (b.cps.as_slice(), port_box_label(q, rep, def)) {
        return vec![PinText {
            idx: cp.idx,
            text: label,
            cell: None,
        }];
    }
    let texts = q.get::<PlcAddressText>(function);
    let cps = q.get::<FunctionCps>(function);
    let designation = |idx: u16| cps.iter().find(|c| c.idx == idx).map(|c| c.designation.as_str());
    let mut pins = compose_pin_texts(rep, def, function, designation, &texts.pins);
    card_cells(q, rep, function, &mut pins);
    pins
}

/// The least width and the least height in grid steps of the black box `r` that keep the pin texts of its opposite
/// sides apart and inside it (the box collapsed through them), by one rule for the four sides: a pin's text reaches
/// in from its pin by the scene's `PIN_TEXT_GAP` and its extent along the pin's inward normal as the scene prints it
/// (on a left or right side horizontal, so its length; on a top or bottom side turned a quarter, so its length, or one
/// line's height for a text of signs only, which the scene prints horizontally), measured in the project's text styles
/// and fonts as the page prints it (`ew_scene::page::pin_text_length`); across the box the longest text of
/// each of the two sides, with half a millimetre between them. 0 across a box whose two sides there print no pin text,
/// and both for a representation that is no black box.
pub(crate) fn box_text_room(q: &Q, r: &Representation) -> (i64, i64) {
    let (Some(symbol), Some(b)) = (q.symbols().get(r.symbol), r.box_.as_ref()) else {
        return (0, 0);
    };
    let step = page_grid(q, PageRef::Drawn(r.page))
        .at(1)
        .map_or(1, |n| n.0.max(1));
    let texts = pin_texts(q, r, &symbol.item);
    let (styles, fonts) = (crate::output::text_style_table(q), crate::output::fonts(q));
    let line = ew_scene::page::pin_text_line(&styles, fonts).unwrap_or(0);
    // How far the longest text of the side `dir` reaches into the box from it.
    let side = |dir: &ew_schema::Dir4| {
        let turned = matches!(dir, ew_schema::Dir4::Up | ew_schema::Dir4::Down);
        b.cps
            .iter()
            .filter(|cp| ew_scene::symbol::transform_dir(&cp.dir, r.orient) == *dir)
            .filter_map(|cp| texts.iter().find(|t| t.idx == cp.idx))
            .map(|t| {
                let signs = !t.text.chars().any(char::is_alphanumeric);
                let reach = if turned && t.cell.is_some() {
                    // Level in a card's cell: its line and the function text's under it. A card's box turned a
                    // quarter prints no cells (the scene's `cell_pins`), so its pins take the rule of any box.
                    2 * line
                } else if turned && signs {
                    line
                } else {
                    ew_scene::page::pin_text_length(&t.text, &styles, fonts)
                };
                ew_scene::page::PIN_TEXT_GAP.0 + reach
            })
            .max()
            .unwrap_or(0)
    };
    let steps = |a: ew_schema::Dir4, b: ew_schema::Dir4| match side(&a) + side(&b) {
        0 => 0,
        reach => (reach + 500_000 + step - 1) / step,
    };
    (
        steps(ew_schema::Dir4::Left, ew_schema::Dir4::Right),
        steps(ew_schema::Dir4::Up, ew_schema::Dir4::Down),
    )
}

/// The interface a box of a port function draws, in words ("X1", "X1 P1R"; doc 10 §2.7): a black box (a symbol
/// without CPs on a representation with a box) whose first function carries a bus port; `None` otherwise.
fn port_box_label(q: &Q, rep: &Representation, def: &SymbolDef) -> Option<String> {
    if !def.cps.is_empty() || rep.box_.is_none() {
        return None;
    }
    let port = q.functions().get(*rep.functions.first()?)?.bus_port.clone()?;
    Some(match &port.port_label {
        Some(l) => format!("{} {}", port.interface.as_str(), l.as_str()),
        None => port.interface.as_str().to_owned(),
    })
}

/// [`pin_texts`] from the function's CP designations (`designation`, by function CP index) and the printed
/// addresses of its bound CPs (`addresses`, `(idx, text)`).
pub(super) fn compose_pin_texts<'a>(
    rep: &Representation,
    def: &SymbolDef,
    function: Id<Function>,
    designation: impl Fn(u16) -> Option<&'a str>,
    addresses: &[(u16, String)],
) -> Vec<PinText> {
    let Some(b) = &rep.box_ else {
        return Vec::new();
    };
    // The function CPs the symbol's own `cp_designation` slots print, resolved as `slot_values` does: through the
    // representation's pin map, else (a symbol without CPs) slot CP `i` is function CP `i`. A slot that the pin map
    // sends to another function prints none of this function's CPs.
    let slotted: Vec<u16> = def
        .text_slots
        .iter()
        .filter_map(|s| match s.kind {
            SlotKind::CpDesignation(i) => match rep.pin_map.iter().find(|p| p.symbol_cp == i) {
                Some(p) => (p.function == function).then_some(p.idx),
                None => Some(i),
            },
            _ => None,
        })
        .collect();
    b.cps
        .iter()
        .filter_map(|cp| {
            let designation = designation(cp.idx).filter(|d| !d.is_empty() && !slotted.contains(&cp.idx));
            let address = addresses
                .iter()
                .find(|(idx, _)| *idx == cp.idx)
                .map(|(_, a)| a.as_str());
            let text = match (designation, address) {
                (Some(d), Some(a)) => format!("{d} {a}"),
                (Some(d), None) => d.to_owned(),
                (None, Some(a)) => a.to_owned(),
                (None, None) => return None,
            };
            Some(PinText {
                idx: cp.idx,
                text,
                cell: None,
            })
        })
        .collect()
}

/// The cells of a PLC card drawn in one row (doc 10 §3.6, "A card drawn in cells"): when at least two of the box's CPs
/// are bound to channels and all of those leave its upper edge, or all its lower edge, each of their pin texts gets its
/// channel's function text (doc 10 §3.4), else its symbol name, empty when it has neither, as its `cell`. Any other
/// box keeps its pin texts.
fn card_cells(q: &Q, rep: &Representation, function: Id<Function>, pins: &mut [PinText]) {
    let Some(b) = &rep.box_ else {
        return;
    };
    let bound = q.get::<ew_derive::ChannelsByCp>(());
    let channels: Vec<(u16, Id<ew_schema::Channel>, &ew_schema::Dir4)> = b
        .cps
        .iter()
        .filter_map(|cp| Some((cp.idx, bound.at_cp(function, cp.idx).next()?, &cp.dir)))
        .collect();
    let one_edge = |d: &ew_schema::Dir4| channels.iter().all(|(_, _, dir)| *dir == d);
    if channels.len() < 2 || !(one_edge(&ew_schema::Dir4::Up) || one_edge(&ew_schema::Dir4::Down)) {
        return;
    }
    let table = q.table::<ew_schema::Channel>();
    for (idx, channel, _) in channels {
        let text = table
            .get(channel)
            .map(|c| q.get::<ew_derive::ChannelTexts>(c.module))
            .and_then(|texts| {
                let t = &texts.iter().find(|(id, _)| *id == channel)?.1;
                // Until a device is wired to it the channel prints its own text, its symbol name.
                t.function_text
                    .as_ref()
                    .map(|t| t.neutral_or_first().unwrap_or_default().to_owned())
                    .filter(|f| !f.is_empty())
                    .or_else(|| t.symbol.clone())
            })
            .unwrap_or_default();
        if let Some(pin) = pins.iter_mut().find(|p| p.idx == idx) {
            pin.cell = Some(text);
        }
    }
}

fn slot_values(q: &Q, page: Id<Page>, rep: &Representation, def: &SymbolDef) -> Vec<SlotValue> {
    let Some(&function) = rep.functions.first() else {
        return Vec::new();
    };
    let f = q.functions().get(function).cloned();
    let device = f.as_ref().and_then(|f| f.device);
    let dev = device.and_then(|d| q.devices().get(d).cloned());
    let label = dev
        .as_ref()
        .filter(|_| rep.rep_type != RepType::StripSegment)
        .and_then(|d| terminal_label(q, d, rep));
    let mut out = Vec::new();
    let bus = ew_derive::is_bus_symbol(def);
    for (i, slot) in def.text_slots.iter().enumerate() {
        let mut item_links = Vec::new();
        let text = match &slot.kind {
            SlotKind::Tag => match (&f, device) {
                (Some(f), _) if matches!(f.kind, FunctionKind::Ip(_)) => None,
                // Doc 08 §6.1, §6.7: a terminal's own representation prints its label, never
                // the full tag.
                (_, Some(_)) if label.is_some() => label.as_ref().map(|(l, _)| l.clone()),
                // `tag_print` PerPage prints a strip's tag at its first segment on each page only.
                (_, Some(d)) if !prints_segment_tag(q, d, rep) => None,
                (_, Some(d)) => Some((*q.get::<DisplayedTag>((d, page))).clone()),
                _ => None,
            },
            SlotKind::CpDesignation(idx) => {
                let (fun, fidx) = match rep.pin_map.iter().find(|p| p.symbol_cp == *idx) {
                    Some(p) => (p.function, p.idx),
                    None => (
                        function,
                        def.cps.iter().find(|c| c.idx == *idx).map_or(*idx, |c| c.fn_cp),
                    ),
                };
                q.get::<FunctionCps>(fun)
                    .iter()
                    .find(|c| c.idx == fidx)
                    .map(|c| c.designation.clone())
            }
            // Interruption points have `function_text` and `xref` slots, no `tag`; the former prints the pairing
            // name.
            SlotKind::FunctionText => match f.as_ref().map(|f| &f.kind) {
                Some(FunctionKind::Ip(ip)) => Some(ip.name.as_str().to_owned()),
                // The box of an interface with several sockets names it in its middle (doc 10 §2.7); a box of one
                // socket names it at the socket.
                _ if rep.box_.as_ref().is_some_and(|b| b.cps.len() > 1)
                    && port_box_label(q, rep, def).is_some() =>
                {
                    port_box_label(q, rep, def)
                }
                // A terminal labelled by its function text prints it once, in its tag slot.
                _ if label.as_ref().is_some_and(|(_, by_text)| *by_text) => None,
                // A slot with the fallback `process_tag` prints, when the function text is empty, the
                // process tag of the valve's tracked target, else the valve's own.
                _ => {
                    let own = dev.as_ref().map(|d| neutral(&d.function_text));
                    match (&own, &slot.fallback) {
                        (Some(t), Some(ew_schema::SlotFallback::ProcessTag)) if t.is_empty() => {
                            device.and_then(|d| ew_derive::tracked_process_tag(q, d)).or(own)
                        }
                        _ => own,
                    }
                }
            },
            SlotKind::ProcessTag => dev
                .as_ref()
                .and_then(|d| d.process_tag.as_ref().map(|t| t.as_str().to_owned())),
            SlotKind::TechData => device.map(|d| tech_data_text(q, d)),
            // Beside an `order_code` slot the manufacturer is the configured part's of that order code (the
            // DP port of folio 3), else the main part's.
            SlotKind::Manufacturer => device
                .and_then(|d| {
                    def.text_slots
                        .iter()
                        .any(|s| matches!(s.kind, SlotKind::OrderCode))
                        .then(|| order_code_part(q, d))
                        .flatten()
                        .or_else(|| main_part(q, d))
                })
                // The starter library's "Generic" names no maker, so the slot prints nothing for it
                // (the chooser and the tables keep the word; `manufacturer_name` is unchanged).
                .filter(|p| p.item.manufacturer.item.as_uuid() != ew_library::STARTER_GENERIC_MANUFACTURER)
                .map(|p| manufacturer_name(q, &p.item.manufacturer)),
            // A bus symbol's part is its maker and its number in one text (doc 10 §2.7), the starter's "Generic" left out.
            SlotKind::PartNumber if bus => device.and_then(|d| main_part(q, d)).map(|p| {
                let number = p.item.part_number.as_str();
                let maker = (p.item.manufacturer.item.as_uuid() != ew_library::STARTER_GENERIC_MANUFACTURER)
                    .then(|| manufacturer_name(q, &p.item.manufacturer))
                    .filter(|m| !m.is_empty());
                match maker {
                    Some(m) => format!("{m} {number}"),
                    None => number.to_owned(),
                }
            }),
            SlotKind::PartNumber => device
                .and_then(|d| main_part(q, d))
                .map(|p| p.item.part_number.as_str().to_owned()),
            SlotKind::OrderCode => device.map(|d| order_code_text(q, d)),
            SlotKind::TypeDesignation => device
                .and_then(|d| main_part(q, d))
                .and_then(|p| p.item.type_designation.as_ref().map(|t| t.as_str().to_owned())),
            SlotKind::Xref => {
                let x = q.get::<XrefTextOf>(rep.id);
                // One link per listed item, so the main list "∞ 3-B2, 106-G3" links both targets.
                item_links = x
                    .links
                    .iter()
                    .map(|l| ItemLink {
                        range: l.range.clone(),
                        dest: Destination::Representation(l.rep.as_uuid()),
                    })
                    .collect();
                Some(x.text.clone())
            }
            // A bus symbol's compartment prints its interface's address (doc 10 §2.7).
            SlotKind::BusAddress if slot.interface.is_some() => rep
                .functions
                .iter()
                .filter_map(|f| q.functions().get(*f)?.bus_port.clone())
                .find(|p| Some(&p.interface) == slot.interface.as_ref() && p.port_label == slot.port_label)
                .and_then(|p| ew_derive::compartment_address(&p)),
            // The drawn function's bus port, its station address and its termination (FR-BUS-004).
            SlotKind::BusAddress => f
                .as_ref()
                .and_then(|f| f.bus_port.as_ref())
                .and_then(ew_derive::bus_address_text)
                .map(|t| crate::output::print::in_print_language(&t, ew_derive::print_language(q).as_ref())),
            SlotKind::BusTermination => f
                .as_ref()
                .and_then(|f| f.bus_port.as_ref())
                .and_then(ew_derive::bus_termination_text),
            // The first drawn function's derived PLC address text; a slot at one solenoid of a valve, that solenoid's
            // address alone.
            SlotKind::PlcAddress => rep.functions.iter().find_map(|f| {
                let texts = q.get::<PlcAddressText>(*f);
                match &slot.solenoid {
                    None => texts.text.clone(),
                    Some(s) => texts
                        .solenoids
                        .iter()
                        .find(|(d, _)| d == s.as_str())
                        .map(|(_, t)| t.clone()),
                }
            }),
            _ => None,
        };
        // A bus symbol's text longer than its slot's room is cut, line by line: its size follows from the part alone.
        let text = match (text, slot.max_width) {
            (Some(t), Some(room)) if bus => Some(cut_lines(q, def, slot, &t, room)),
            (t, _) => t,
        };
        if let Some(text) = text.filter(|t| !t.is_empty()) {
            out.push(SlotValue {
                slot: u16::try_from(i).unwrap_or(u16::MAX),
                text,
                link: None,
                item_links,
            });
        }
    }
    out
}

/// Each line of `text`, printed in `slot`'s style, cut with "…" to `room` where it is wider (doc 10 §2.7).
fn cut_lines(q: &Q, def: &SymbolDef, slot: &ew_schema::TextSlot, text: &str, room: ew_units::Nm) -> String {
    let styles = crate::output::text_style_table(q);
    let fonts = crate::output::fonts(q);
    let terminal = ew_derive::terminal_symbol(q, def);
    let Some(style) = styles.resolve(ew_scene::symbol::slot_style_key(terminal, slot, &styles).0) else {
        return text.to_owned();
    };
    let width = |s: &str| fonts.shape(s, &style, None).advance();
    text.split('\n')
        .map(|line| {
            if width(line) <= room {
                line.to_owned()
            } else {
                ew_scene::forms::ellipsis_cut(line, room, width)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The label a terminal's own representation `rep` prints in its tag slot (doc 08 §6.1, FR-TRM-021):
/// its function text when its strip's `label_mode` is FunctionText and the text is not empty (then `true`: the
/// function-text slot stays empty), else the designation of the level the representation draws (doc 08 §6.3: the
/// symbol of a block's second level prints that level's number); `None` for a device that is not a terminal.
fn terminal_label(q: &Q, dev: &ew_schema::Device, rep: &Representation) -> Option<(String, bool)> {
    let t = dev.aspects.terminal.as_ref()?;
    let by_text = q
        .devices()
        .get(t.strip)
        .and_then(|s| s.aspects.strip.as_ref())
        .is_some_and(|s| s.label_mode == LabelMode::FunctionText);
    let text = neutral(&dev.function_text);
    Some(if by_text && !text.is_empty() {
        (text, true)
    } else {
        let level = ew_derive::rep_level(q, rep);
        (t.level_designation(level).as_str().to_owned(), false)
    })
}

/// The (row, column) cell of a representation's anchor, the frame order of doc 08 §6.7; a representation outside
/// the frame grid sorts last.
fn cell_key(q: &Q, rep: Id<Representation>) -> (u32, u32) {
    q.get::<RepAnchorCell>(rep)
        .as_ref()
        .as_ref()
        .map_or((u32::MAX, u32::MAX), |c| (c.row, c.column))
}

/// Whether strip segment `rep` of device `device` prints the strip's tag (doc 08 §6.7): every segment
/// with `tag_print` PerSegment; with PerPage only the strip's first segment on its page in (row, column, ID)
/// order. A representation that is not a strip segment prints its tag.
fn prints_segment_tag(q: &Q, device: Id<ew_schema::Device>, rep: &Representation) -> bool {
    if rep.rep_type != RepType::StripSegment {
        return true;
    }
    let per_page = q
        .devices()
        .get(device)
        .and_then(|d| d.aspects.strip.as_ref())
        .is_some_and(|s| s.tag_print == TagPrint::PerPage);
    if !per_page {
        return true;
    }
    let reps = q.reps();
    let first = q
        .idx()
        .functions_of_device(device)
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
        .filter(|r| {
            reps.get(*r)
                .is_some_and(|r| r.rep_type == RepType::StripSegment && r.page == rep.page)
        })
        .min_by_key(|r| (cell_key(q, *r), *r));
    first == Some(rep.id)
}

/// `page_strip_tags(page)`: the strip tags of the terminals drawn alone on a drawn page (doc 08 §6.7), by
/// representation ID. A strip with a terminal representation on the page, no segment there and no text bound to its
/// name there (`strip.tag`, doc 06 §3.8) prints its displayed
/// tag for the page once, at the first terminal representation on the page whose tag slot is shown
/// (`ew_derive::tag_slot_shown`), in the strip's own order (designation in natural order, then terminal ID, as
/// `ew_derive::MainRep` takes a strip's terminal; then representation ID); a strip without such a terminal on the page
/// prints no tag there. So an unnumbered strip's terminals name it: its displayed tag for the page, with "?" for what
/// is not derived ("-X101?" under a strip rule with `{page}`, "-X??" without a page token).
pub fn page_strip_tags(q: &Q, page: Id<Page>) -> Vec<StripTagView> {
    let (reps, functions, devices, symbols) = (q.reps(), q.functions(), q.devices(), q.symbols());
    let mut segmented = std::collections::BTreeSet::new();
    let mut firsts = BTreeMap::new();
    for rid in q.idx().reps_by_page(page) {
        let Some(rep) = reps.get(rid) else { continue };
        let Some(d) = rep
            .functions
            .first()
            .and_then(|f| functions.get(*f))
            .and_then(|f| f.device)
        else {
            continue;
        };
        match rep.rep_type {
            RepType::StripSegment => {
                segmented.insert(d);
                continue;
            }
            RepType::ContactImage => continue,
            _ => {}
        }
        let Some(t) = devices.get(d).and_then(|d| d.aspects.terminal.as_ref()) else {
            continue;
        };
        let shown = symbols
            .get(rep.symbol)
            .and_then(|s| ew_derive::tag_slot_shown(rep, &s.item));
        if shown.is_none() {
            continue;
        }
        let key = (t.designation.natural_key(), d, rid);
        let first = firsts.entry(t.strip).or_insert_with(|| key.clone());
        if key < *first {
            *first = key;
        }
    }
    // A strip a text bound to its terminal names on the page (doc 06 §3.8) is named there by that text alone.
    let named = ew_derive::strips_named_on(q, page);
    let mut out: Vec<StripTagView> = firsts
        .into_iter()
        .filter(|(strip, _)| !segmented.contains(strip) && !named.contains(strip))
        .map(|(strip, (_, _, rep))| StripTagView {
            rep,
            text: (*q.get::<DisplayedTag>((strip, page))).clone(),
        })
        .filter(|t| !t.text.is_empty())
        .collect();
    out.sort_by_key(|t| t.rep);
    out
}

/// The terminals a strip-segment representation draws (see the module documentation); empty for any other
/// representation. The page order is read only for a strip segment.
fn segment_terminals(q: &Q, rep: &Representation) -> Vec<SegmentTerminal> {
    if rep.rep_type != RepType::StripSegment {
        return Vec::new();
    }
    let Some(strip) = rep
        .functions
        .first()
        .and_then(|f| q.functions().get(*f))
        .and_then(|f| f.device)
    else {
        return Vec::new();
    };
    // The strip's representations with their page's place, and its terminals: which of them this segment draws, in
    // its order, is `ew_derive::strip_segment_of`'s.
    let order = q.get::<ew_derive::DrawnPageOrder>(());
    let reps: Vec<_> = q
        .idx()
        .functions_of_device(strip)
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
        .filter_map(|r| q.reps().get(r).cloned())
        .collect();
    let segments = || {
        reps.iter()
            .map(|r| (order.position(PageRef::Drawn(r.page)).unwrap_or(usize::MAX), &**r))
    };
    let terminals: Vec<(u32, Id<ew_schema::Device>)> = q
        .idx()
        .terminals_of_strip(strip)
        .filter_map(|t| Some((q.devices().get(t)?.aspects.terminal.as_ref()?.seq, t)))
        .collect();
    let mut drawn: Vec<(usize, Id<ew_schema::Device>)> = terminals
        .iter()
        .filter_map(|&(_, t)| {
            let (seg, k) = ew_derive::strip_segment_of(segments(), terminals.iter().copied(), t)?;
            (seg.id == rep.id).then_some((k, t))
        })
        .collect();
    drawn.sort_unstable();
    drawn
        .into_iter()
        .filter_map(|(_, t)| {
            let fns = q
                .idx()
                .functions_of_device(t)
                .filter_map(|f| Some((f, q.functions().get(f)?.role.clone())));
            let main = ew_derive::terminal_drawn_function(fns)?;
            let cps = q.get::<FunctionCps>(main);
            let side = |want: Side| {
                cps.iter()
                    .filter(|c| c.side == Some(want.clone()))
                    .map(|c| c.idx)
                    .collect()
            };
            Some(SegmentTerminal {
                function: main,
                side1: side(Side::Side1),
                side2: side(Side::Side2),
            })
        })
        .collect()
}
