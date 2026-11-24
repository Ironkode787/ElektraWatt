// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page items of a page's view (see the module documentation of `view`): its graphics and free texts, its
//! structure boxes, the break marks of assembly overviews, the valve glyphs and cells and the notes beside a plug's pins.

#[allow(clippy::wildcard_imports)]
use super::*;

/// The page items of a drawn page in ID order. With `texts`, each with what it prints (a free
/// text's text or bound value, a wire marker's group text and whether it is shown); without, the records alone
/// (the geometry the symbols layer draws).
pub fn page_items(q: &Q, page: Id<Page>, texts: bool) -> Vec<ew_scene::items::ItemView> {
    let items = q.page_items();
    let mut ids: Vec<_> = q.idx().page_items_by_page(page).collect();
    ids.sort();
    ids.into_iter()
        .filter_map(|id| items.get(id).cloned())
        .map(|item| {
            // An image's format comes from the asset the project holds (assets are only added; the read is recorded
            // against the project row and the asset store, `Q::logo_asset`: the asset can arrive with an item of
            // another page).
            let asset = match &item.kind {
                ew_schema::PageItemKind::Image(i) => q.logo_asset(&i.asset),
                _ => None,
            };
            let (text, shown) = if texts {
                match &item.kind {
                    ew_schema::PageItemKind::FreeText(t) => (free_text(q, page, t), true),
                    ew_schema::PageItemKind::WireMarker(_) => {
                        group_marker(q, &item).map_or((String::new(), false), |g| (g.text, g.shown))
                    }
                    _ => (String::new(), false),
                }
            } else {
                (String::new(), false)
            };
            // A marker over a cable's cores is the cable's mark, drawn whatever the marker mode.
            let cable = texts.then(|| ew_derive::cable_mark(q, &item)).flatten();
            ew_scene::items::ItemView {
                item,
                text,
                shown,
                asset,
                cable,
            }
        })
        .collect()
}

/// The structure boxes of a drawn page in ID order, with their captions when `captions` is set.
pub fn page_boxes(q: &Q, page: Id<Page>, captions: bool) -> Vec<ew_scene::items::StructureBoxView> {
    let boxes = q.structure_boxes();
    let mut ids: Vec<_> = q.idx().structure_boxes_by_page(page).collect();
    ids.sort();
    ids.into_iter()
        .filter_map(|id| boxes.get(id).cloned())
        .map(|sbox| {
            let caption = if captions {
                box_caption(q, &sbox)
            } else {
                String::new()
            };
            ew_scene::items::StructureBoxView { sbox, caption }
        })
        .collect()
}

/// The break marks of the assembly overviews on a drawn page (`assembly_break_marks()`, D-41), in the derived
/// order. A plain function of the tracked reads of `q`: the symbols layer that calls it depends on the marks.
pub fn page_break_marks(q: &Q, page: Id<Page>) -> Vec<BreakMarkView> {
    q.get::<ew_derive::AssemblyBreakMarks>(())
        .iter()
        .filter(|m| m.page == page)
        .map(|m| BreakMarkView {
            rep: m.rep,
            side: match m.side {
                ew_derive::BreakSide::Left => BreakSide::Left,
                ew_derive::BreakSide::Right => BreakSide::Right,
            },
        })
        .collect()
}

/// The valve glyphs of the module overviews on a drawn page (doc 11 §5.5 step 3): for each overview
/// representation of a module with valve positions, one glyph per position from `valve_slots`, in representation-ID
/// order; a valve prints the part number of its first part use. A plain function of the tracked reads of `q`, like
/// `page_break_marks`.
pub fn page_valve_glyphs(q: &Q, page: Id<Page>) -> Vec<ValveGlyphsView> {
    let reps = q.reps();
    let functions = q.functions();
    let devices = q.devices();
    let parts = q.parts();
    let mut ids: Vec<Id<Representation>> = q.idx().reps_by_page(page).collect();
    ids.sort();
    let mut out = Vec::new();
    for rid in ids {
        let Some(rep) = reps.get(rid) else { continue };
        if rep.rep_type != RepType::Overview {
            continue;
        }
        let Some(module) = rep
            .functions
            .first()
            .and_then(|f| functions.get(*f))
            .and_then(|f| f.device)
        else {
            continue;
        };
        let slots = q.get::<ew_derive::ValveSlots>(module);
        if slots.slots.is_empty() {
            continue;
        }
        let glyphs = slots
            .slots
            .iter()
            .enumerate()
            .map(|(i, slot)| match slot {
                ew_derive::ValveSlot::Valve { device, .. } => ValveGlyph::Valve {
                    entity: ew_ids::v5::valve_glyph_entity(rid.as_uuid(), device.as_uuid()),
                    part_number: devices
                        .get(*device)
                        .and_then(|d| d.parts.first())
                        .and_then(|u| parts.get(u.part.copy))
                        .map(|c| c.item.part_number.as_str().to_owned())
                        .unwrap_or_default(),
                },
                ew_derive::ValveSlot::Blank { device } => ValveGlyph::Blank {
                    entity: ew_ids::v5::valve_glyph_entity(rid.as_uuid(), device.as_uuid()),
                },
                ew_derive::ValveSlot::Empty => ValveGlyph::Empty {
                    entity: ew_ids::v5::valve_position_entity(rid.as_uuid(), i + 1),
                },
            })
            .collect();
        out.push(ValveGlyphsView { rep: rid, glyphs });
    }
    out
}

/// The cells of the valve terminals drawn as their positions on a drawn page (doc 11 §3.2): for each box
/// representation with `slots`, in representation-ID order, its frame and per cell (`terminal_cells`) its number and
/// what it holds (a valve not drawn on this page with its tag and where it is drawn), whether its supply stands on the
/// page, and the valves' ports that end on its sub-base with the gallery each joins. A plain
/// function of the tracked reads of `q`, like `page_valve_glyphs`.
pub fn page_valve_cells(q: &Q, page: Id<Page>) -> Vec<ValveCellsView> {
    let reps = q.reps();
    let functions = q.functions();
    let mut ids: Vec<Id<Representation>> = q.idx().reps_by_page(page).collect();
    ids.sort();
    let drawn_here = |device: Id<Device>| {
        q.idx().functions_of_device(device).any(|f| {
            q.idx()
                .reps_of_function(f)
                .any(|r| reps.get(r).is_some_and(|r| r.page == page))
        })
    };
    let mut out = Vec::new();
    for rid in ids {
        let Some(rep) = reps.get(rid) else { continue };
        if rep.box_.as_ref().and_then(|b| b.slots.as_ref()).is_none() {
            continue;
        }
        let Some(terminal) = rep
            .functions
            .first()
            .and_then(|f| functions.get(*f))
            .and_then(|f| f.device)
        else {
            continue;
        };
        // The page is read only where a box is drawn as its positions: a page record's change (a renumber) leaves the
        // symbol layers of the other pages alone. Each cell is as wide as its valve needs.
        let Some(frame) = ew_derive::box_frame(q, rep) else {
            continue;
        };
        let (_, band, _) = frame.rows();
        let mut taps: Vec<(Nm, usize)> = Vec::new();
        let cells = ew_derive::terminal_cells(q, terminal)
            .into_iter()
            .map(|c| {
                let view = match c.slot {
                    ew_derive::ValveSlot::Valve { device, .. } if drawn_here(device) => {
                        // The ports that end on the sub-base, each joined to its gallery.
                        for f in ew_derive::valve_functions(q, device) {
                            for cp in q.get::<ew_derive::FunctionCps>(f).iter() {
                                let k = ew_derive::GALLERY_PORTS
                                    .iter()
                                    .position(|g| cp.port_role.as_ref() == Some(g));
                                if let (Some(k), Some(at)) = (k, ew_derive::cp_point(q, f, cp.idx, page))
                                    && at.y == band
                                {
                                    taps.push((at.x, k));
                                }
                            }
                        }
                        CellView::Valve {
                            elsewhere: None,
                            drawn_at: None,
                        }
                    }
                    ew_derive::ValveSlot::Valve { device, .. } => CellView::Valve {
                        elsewhere: Some(crate::output::properties::device_tag_text(q, device)),
                        drawn_at: drawn_elsewhere(q, device),
                    },
                    ew_derive::ValveSlot::Blank { .. } => CellView::Blank,
                    ew_derive::ValveSlot::Empty => CellView::Empty,
                };
                (c.position, view)
            })
            .collect();
        let supply = ew_derive::terminal_supply(q, terminal).is_some_and(|f| {
            q.idx()
                .reps_of_function(f)
                .any(|r| reps.get(r).is_some_and(|r| r.page == page))
        });
        out.push(ValveCellsView {
            rep: rid,
            frame,
            cells,
            supply,
            taps,
        });
    }
    out
}

/// Where a valve of a cell that is not drawn on the cell's page is drawn: the place of its main symbol
/// (`<folio>-<cell>`, the folio alone off the frame grid) and that symbol; `None` for a valve drawn nowhere.
fn drawn_elsewhere(q: &Q, device: Id<Device>) -> Option<(String, Id<Representation>)> {
    let rep = (*q.get::<ew_derive::MainRep>(device))?;
    let r = q.reps().get(rep)?;
    let folio = crate::output::properties::folio_of(q, r.page)?;
    let place = match q.get::<ew_derive::CellOf>((r.page, r.pos)).as_ref() {
        Some(cell) => format!("{folio}-{cell}"),
        None => folio,
    };
    Some((place, rep))
}

/// The texts beside the pins of the multipin valve terminals' plugs on a drawn page: for each box
/// representation of a multipin terminal that is no box drawn as its positions, in representation-ID order, a text
/// per box CP that a pin of the plug is (`ew_derive::plug_pins`): the valve's tag and the solenoid where a valve holds
/// the position (`-201K2-Q1 14`), else the position and the solenoid (`1 · 14`), and `COM` for the common. A plain
/// function of the tracked reads of `q`, like `page_valve_cells`.
pub fn page_plug_notes(q: &Q, page: Id<Page>) -> Vec<PlugNoteView> {
    let reps = q.reps();
    let functions = q.functions();
    let mut ids: Vec<Id<Representation>> = q.idx().reps_by_page(page).collect();
    ids.sort();
    let mut out = Vec::new();
    for rid in ids {
        let Some(rep) = reps.get(rid) else { continue };
        let Some(b) = rep
            .box_
            .as_ref()
            .filter(|b| b.slots.is_none() && !b.cps.is_empty())
        else {
            continue;
        };
        let Some(&function) = rep.functions.first() else {
            continue;
        };
        let Some(terminal) = functions.get(function).and_then(|f| f.device) else {
            continue;
        };
        let pins = ew_derive::plug_pins(q, terminal);
        if pins.is_empty() {
            continue;
        }
        let cps = q.get::<ew_derive::FunctionCps>(function);
        for cp in &b.cps {
            let Some(designation) = cps
                .iter()
                .find(|c| c.idx == cp.idx)
                .map(|c| c.designation.clone())
            else {
                continue;
            };
            let Some((_, carried)) = pins.iter().find(|(d, _)| *d == designation) else {
                continue;
            };
            let text = match carried {
                ew_derive::PlugPin::Solenoid {
                    valve: Some(v),
                    solenoid,
                    ..
                } => {
                    format!("{} {solenoid}", crate::output::properties::device_tag_text(q, *v))
                }
                ew_derive::PlugPin::Solenoid {
                    position, solenoid, ..
                } => format!("{position} · {solenoid}"),
                ew_derive::PlugPin::Common => "COM".to_owned(),
            };
            let o = rep.orient;
            let at = rep.pos + o.apply(cp.pos);
            let dir = crate::geometry::orient_dir(
                crate::geometry::orient_index(o),
                crate::output::editing::dir_of(&cp.dir),
            );
            let (vertical, forward) = match dir {
                crate::geometry::Dir::Up => (true, false),
                crate::geometry::Dir::Down => (true, true),
                crate::geometry::Dir::Left => (false, false),
                crate::geometry::Dir::Right => (false, true),
            };
            out.push(PlugNoteView {
                rep: rid,
                at,
                vertical,
                forward,
                text,
            });
        }
    }
    out
}
