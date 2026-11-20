// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A station drawn on pages from its view: Draw on a page… (the boxes of the station and its cards laid out on a
//! schematic page, `plc.draw_station`), the To do list's next step (`plc.rack_next`), and Place here on a PLC part
//! (`plc.module_place`).
//!
//! The box layout ([`ShellModel::station_boxes`]) takes the page's free place, its drawing area's right and foot from
//! `Session::station_draw_params` and lays the boxes out here, in the view-model.
//!
//! Not here: what the view shows is in `shell/station.rs`; the rack's edits in `station/rack.rs`.

use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use crate::area::StationTab;
use crate::key;
use crate::shell::{Outcome, ShellModel, failed, string_param};
use crate::strings;
use crate::strings::Key;

impl ShellModel {
    /// `plc.draw_station`'s parameters for `station` on `page` (`Session::station_draw_params`): each device not drawn
    /// yet, and the rest of a card drawn in part, as boxes. A card, a device whose part's pin map gives its channels
    /// pins, is drawn as docs/design/06a-drafting.md §7.5 says (`ew_api::part_box::card_box`): its inputs in one row on
    /// the upper edge of a box at the foot of the drawing area, its outputs on the lower edge of a box at its head, its
    /// other pins down the left side of the first; of a row wider than the drawing area one box takes as many pins as
    /// fit, at most sixteen, and the rest waits for a page of its own (`part_box::card_share`). Any other device is a box
    /// 39.7 × 31.75 mm (wider where its pins need it, so that their upright texts fit on one line), with its part's
    /// connection points laid out as the black box tool lays them (`tools::black_box::layout`); these stand in slot
    /// order beside each other from the next free place, a new row under the first where the next would pass the drawing
    /// area's right. Each card's box then takes the first place that keeps a gap from what the page draws and what is
    /// drawn now: a card of inputs as near the foot as there is room, a card of outputs as near the head, each line from
    /// the left; a card that finds no room is named under `left` and waits for another page. A device whose part has no
    /// black-box entry is left out. `None` without the page shown, its frame or the catalogue.
    fn station_boxes(&mut self, station: Uuid, page: Uuid) -> Option<Value> {
        use ew_api::part_box::{BoxSteps, card_box, card_share};
        const SIZE: (i64, i64, i64) = (39_687_500, 31_750_000, 7_937_500);
        /// One box to lay out: a box of a device that is no card, by its pins, or a card's.
        enum Piece {
            Plain(Vec<(u16, String)>),
            Card(BoxSteps),
        }
        let mut params = self.project_session()?.station_draw_params(station, page)?;
        let page_grid = self.canvas()?.page().tables.grid;
        let grid = crate::tools::wire::PageGridMap::new(page_grid);
        // What the page draws already, and the highest line a card stands on: two gaps under the frame's head.
        let taken: Vec<[i64; 4]> = self
            .canvas()?
            .page()
            .tables
            .reps
            .iter()
            .map(|r| {
                // A drawn box by its outline, a symbol by its body.
                r.black_box.as_ref().map_or(r.body, |b| {
                    b.outline
                        .iter()
                        .fold([i64::MAX, i64::MAX, i64::MIN, i64::MIN], |a, p| {
                            [a[0].min(p.0), a[1].min(p.1), a[2].max(p.0), a[3].max(p.1)]
                        })
                })
            })
            .collect();
        let head_line = crate::tools::ip::drawing_area(self.canvas()?.page())?[1] + 2 * SIZE.2;
        let grid_x = |k: i64| grid.coord(k);
        // The pins of each device's black-box entry, from the catalogue.
        let entries: Vec<(Uuid, Vec<(u16, String)>)> = {
            let cat = self.catalogue()?;
            cat.parts
                .iter()
                .filter_map(|r| Some((r.copy, r.functions.iter().find(|f| f.black_box)?.cps.clone())))
                .collect()
        };
        let session = self.project_session()?;
        let snap = |v: i64| grid.coord(grid.index(v));
        let at = |i: usize| params["at"][i].as_i64().unwrap_or(0);
        let (x0, y0, right) = (
            snap(at(0)),
            snap(at(1)),
            params["right"].as_i64().unwrap_or(i64::MAX),
        );
        let foot = params["foot"].as_i64().unwrap_or(i64::MAX);
        let room = grid.index(right) - grid.index(x0);
        // The boxes in slot order, each with its device and whether it stands at the foot.
        let mut pieces: Vec<(String, Piece, bool)> = Vec::new();
        for d in params["devices"].as_array().cloned().unwrap_or_default() {
            let (Some(device), Some(part)) =
                (d[0].as_str(), d[1].as_str().and_then(|p| p.parse::<Uuid>().ok()))
            else {
                continue;
            };
            let Some((_, entry)) = entries.iter().find(|(copy, _)| *copy == part) else {
                continue;
            };
            let drawn: Vec<u64> = d[2]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default();
            let cps: Vec<(u16, String)> = entry
                .iter()
                .filter(|(i, _)| !drawn.contains(&u64::from(*i)))
                .cloned()
                .collect();
            let pins = device
                .parse::<Uuid>()
                .map(|id| session.card_pins(id, &cps))
                .unwrap_or_default();
            if pins.inputs.is_empty() && pins.outputs.is_empty() {
                pieces.push((device.to_owned(), Piece::Plain(cps), false));
                continue;
            }
            let mut others = pins.others.as_slice();
            for (row, up) in [(&pins.inputs, true), (&pins.outputs, false)] {
                if row.is_empty() {
                    continue;
                }
                let take = card_share(row.len(), room);
                pieces.push((
                    device.to_owned(),
                    Piece::Card(card_box(&row[..take], up, others)),
                    up,
                ));
                others = &[];
            }
        }
        // A laid-out box as `plc.draw_station` takes it, and its bounds `[x0, y0, x1, y1]` (page nm).
        let to_json = |device: &str, (pos, outline, cps): crate::tools::black_box::BoxLayout| {
            let (w, h) = outline.iter().fold((0, 0), |(w, h), p| (w.max(p.0), h.max(p.1)));
            let json = json!({
                "device": device,
                "pos": [pos.0, pos.1],
                "box": {
                    "outline": outline.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
                    "cps": cps.iter().map(|(i, p, dir)| json!({"idx": i, "pos": [p.0, p.1], "dir": dir.word()}))
                        .collect::<Vec<_>>(),
                },
            });
            (json, [pos.0, pos.1, pos.0 + w, pos.1 + h])
        };
        let card_at = |steps: &BoxSteps, at: (i64, i64)| {
            use ew_api::geometry::Dir;
            use ew_api::part_box::Dir4;
            let (pos, outline, cps) = steps.on_grid(page_grid, at)?;
            let cps = cps
                .into_iter()
                .map(|(i, p, dir)| {
                    let dir = match dir {
                        Dir4::Up => Dir::Up,
                        Dir4::Down => Dir::Down,
                        Dir4::Left => Dir::Left,
                        _ => Dir::Right,
                    };
                    (i, p, dir)
                })
                .collect();
            Some((pos, outline, cps))
        };
        let mut boxes: Vec<(usize, Value)> = Vec::new();
        let mut rects: Vec<[i64; 4]> = Vec::new();
        // The boxes that are no card, in a row from the next free place: (x, y, the lowest edge so far).
        let mut flow = (x0, y0, y0);
        let head = |flow: &mut (i64, i64, i64),
                    i: usize,
                    device: &str,
                    piece: &Piece,
                    boxes: &mut Vec<(usize, Value)>,
                    rects: &mut Vec<[i64; 4]>| {
            let width = match piece {
                Piece::Plain(_) => SIZE.0,
                Piece::Card(s) => grid.coord(grid.index(flow.0) + s.w) - flow.0,
            };
            if flow.0 > x0 && flow.0 + width > right {
                (flow.0, flow.1) = (x0, snap(flow.2 + 2 * SIZE.2));
            }
            let (x, y) = (flow.0, flow.1);
            let laid = match piece {
                Piece::Plain(cps) => crate::tools::black_box::layout(
                    grid,
                    (x, y),
                    (snap(x + SIZE.0), snap(y + SIZE.1)),
                    cps,
                    &[],
                ),
                Piece::Card(s) => card_at(s, (x, y)),
            };
            if let Some(laid) = laid {
                let (json, rect) = to_json(device, laid);
                boxes.push((i, json));
                rects.push(rect);
                flow.2 = flow.2.max(rect[3]);
                flow.0 = snap(rect[2] + SIZE.2);
            }
        };
        for (i, (device, piece, _)) in pieces.iter().enumerate() {
            if matches!(piece, Piece::Plain(_)) {
                head(&mut flow, i, device, piece, &mut boxes, &mut rects);
            }
        }
        // The first place of a card's box `s` that keeps a gap from what the page draws and what is drawn now: a card of
        // inputs (`up`) with its lower edge on the grid line at the foot, else one line higher; a card of outputs with its
        // upper edge on the line two gaps under the frame's head, else one line lower; each line from the left.
        // One gap beside a box, two over and under it, where its texts stand.
        let clear = |r: [i64; 4], drawn: &[[i64; 4]]| {
            !taken.iter().chain(drawn).any(|t| {
                t[0] - SIZE.2 < r[2]
                    && r[0] < t[2] + SIZE.2
                    && t[1] - 2 * SIZE.2 < r[3]
                    && r[1] < t[3] + 2 * SIZE.2
            })
        };
        let find = |s: &BoxSteps, up: bool, drawn: &[[i64; 4]]| -> Option<(i64, i64)> {
            let (lo, hi) = (grid.index(head_line), grid.index(foot));
            let lines: Vec<i64> = if up {
                (lo + s.h..=hi).rev().map(|k| k - s.h).collect()
            } else {
                (lo..=hi - s.h).collect()
            };
            let (first, last) = (grid.index(x0), grid.index(right) - s.w);
            lines.into_iter().find_map(|k| {
                let (t, b) = (grid.coord(k), grid.coord(k + s.h));
                (first..=last)
                    .map(grid_x)
                    .find_map(|x| clear([x, t, grid.coord(grid.index(x) + s.w), b], drawn).then_some((x, t)))
            })
        };
        let mut left: Vec<String> = Vec::new();
        for (i, (device, piece, up)) in pieces.iter().enumerate() {
            let Piece::Card(s) = piece else { continue };
            match find(s, *up, &rects).and_then(|at| card_at(s, at)) {
                Some(laid) => {
                    let (json, rect) = to_json(device, laid);
                    boxes.push((i, json));
                    rects.push(rect);
                }
                // No room on the page: the card waits for another page, which the person chooses.
                None => {
                    if !left.iter().any(|d| d == device) {
                        left.push(device.clone());
                    }
                }
            }
        }
        boxes.sort_by_key(|b| b.0);
        if let Some(m) = params.as_object_mut() {
            for k in ["at", "right", "foot", "devices"] {
                m.remove(k);
            }
            m.insert(
                "boxes".into(),
                Value::Array(boxes.into_iter().map(|b| b.1).collect()),
            );
            m.insert("left".into(), json!(left));
        }
        Some(params)
    }

    /// Draws devices of `station` not drawn yet on a schematic page with one `plc.draw_station`: all of them (`only`
    /// absent: Draw on a page…), or the one device `only` (a card's Draw on a page…), each bound to its channels as a
    /// box drawn by hand is (normaliser N3). The page is `folio` of `p` (`new`: a new page, asking nothing), else the
    /// page the view was opened from; the page is shown with what was drawn. A refusal brings the view back and
    /// stays in it.
    pub(super) fn draw_from_rack(
        &mut self,
        station: Uuid,
        only: Option<Uuid>,
        p: &Map<String, Value>,
    ) -> Outcome {
        let cursor = self.station_tab().and_then(|t| t.cursor);
        let folio = match string_param(p, "folio") {
            Ok(Some("new")) => {
                let folio = self.free_folio("multi_line");
                let title = strings::text(key!("rack-draw-page-title"));
                let made = self.dispatch(
                    "page.new",
                    &json!({ "folio": folio, "type": "multi_line", "title": title }),
                );
                if made != Outcome::Done {
                    return made;
                }
                folio
            }
            Ok(Some(f)) if self.schematic_pages().iter().any(|(x, _)| x == f) => f.to_owned(),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["folio"]),
            Ok(None) => {
                let shown = self
                    .page_rows()
                    .get(self.page_index())
                    .filter(|r| matches!(r.kind.as_str(), "multi_line" | "single_line"))
                    .map(|r| r.folio.clone());
                match shown {
                    Some(folio) => folio,
                    None => {
                        return self
                            .rack_refuse(cursor, strings::text(key!("reason-rack-draw-page")).to_owned());
                    }
                }
            }
            Err(e) => return e,
        };
        let opened = self.dispatch("page.goto", &json!({ "folio": folio }));
        if opened != Outcome::Done {
            return opened;
        }
        // Back in the station's view, where a refusal is said.
        let back = |s: &mut Self, why: String| {
            let _ = s.open_station(StationTab {
                station: Some(station),
                ..StationTab::default()
            });
            s.rack_refuse(cursor, why)
        };
        let page = self.page_rows().get(self.page_index()).map(|r| r.id);
        let Some(mut params) = page.and_then(|page| self.station_boxes(station, page)) else {
            return back(self, strings::text(key!("reason-rack-no-frame")).to_owned());
        };
        // The cards that find no room on the page, by their tags (`station_boxes`).
        let mut left: Vec<String> = params
            .as_object_mut()
            .and_then(|m| m.remove("left"))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|d| d.parse::<Uuid>().ok())
            .filter(|d| only.is_none_or(|m| m == *d))
            .filter_map(|d| self.project_session()?.device_tag(d))
            .collect();
        left.dedup();
        if let (Some(module), Some(boxes)) = (only, params["boxes"].as_array_mut()) {
            boxes.retain(|b| b["device"].as_str() == Some(module.to_string().as_str()));
        }
        let folio = self
            .page_rows()
            .get(self.page_index())
            .map(|r| r.folio.clone())
            .unwrap_or_default();
        let no_room = strings::format(key!("rack-card-no-room"), &[&left.join(", "), &folio]);
        if params["boxes"].as_array().is_some_and(Vec::is_empty) && !left.is_empty() {
            return back(self, no_room);
        }
        let out = self.run_commit(&ew_api::AnyCommand {
            name: "plc.draw_station".into(),
            params,
        });
        match out {
            Outcome::Done => {
                // A card wider than the drawing area: its first box is drawn, the toast says where the rest is drawn.
                let rest: Vec<String> = self.project_session().map_or_else(Vec::new, |s| {
                    s.cards_drawn_in_part(station)
                        .into_iter()
                        .filter_map(|c| s.device_tag(c))
                        .collect()
                });
                if !rest.is_empty() {
                    self.notify_rack(
                        strings::format(key!("rack-card-rest"), &[&rest.join(", ")]),
                        station,
                    );
                }
                if !left.is_empty() {
                    self.notify_rack(no_room, station);
                }
                Outcome::Done
            }
            Outcome::Failed { message } => back(self, message),
            Outcome::Refused { reason_key } => back(self, strings::text(reason_key).to_owned()),
        }
    }

    /// `plc.rack_next {step, folio?}`: what is left to do for the station on show, from its To do list, on the page
    /// `folio` names or a new one (`new`): `draw` draws the station and its cards that no electrical page draws yet on a
    /// schematic page ([`ShellModel::draw_from_rack`]); `fluid` draws a valve terminal on a fluid page, without `folio`
    /// as `fluid.draw_terminal` chooses its page; `bus` opens a bus-topology page, without `folio` the only one or a
    /// new one, and puts the station's interfaces not drawn there on the pointer; `overview` makes a valve terminal's
    /// overview pages (`fluid.generate_overview`, previewed).
    pub(crate) fn act_plc_rack_next(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.rack_station());
        let cursor = self.station_tab().and_then(|t| t.cursor);
        match string_param(p, "step") {
            // A valve terminal on a fluid page: drawn as its positions there, on the fluid page named (`new`: a
            // new one), else the nearest one, else on one made then. A failure is said under the rack.
            Ok(Some("fluid")) => {
                let opened = match string_param(p, "folio") {
                    Ok(Some("new")) => match self.new_fluid_page() {
                        folio if folio.is_empty() => failed(key!("reason-fluid-page-not-made"), &[]),
                        folio => self.dispatch("page.goto", &json!({ "folio": folio })),
                    },
                    Ok(Some(f)) if self.fluid_pages().iter().any(|(x, _)| x == f) => {
                        self.dispatch("page.goto", &json!({ "folio": f }))
                    }
                    Ok(Some(_)) => return failed(key!("error-bad-param"), &["folio"]),
                    Ok(None) => Outcome::Done,
                    Err(e) => return e,
                };
                let out = match opened {
                    Outcome::Done => {
                        self.dispatch("fluid.draw_terminal", &json!({ "terminal": station.to_string() }))
                    }
                    other => other,
                };
                let why = match out {
                    Outcome::Done => return Outcome::Done,
                    Outcome::Failed { message } => message,
                    Outcome::Refused { reason_key } => strings::text(reason_key).to_owned(),
                };
                if self.station_tab().is_none() {
                    let _ = self.open_station(StationTab {
                        station: Some(station),
                        ..StationTab::default()
                    });
                }
                self.rack_refuse(cursor, why)
            }
            // The station and its cards not drawn yet, in a row in slot order, one undo step.
            Ok(Some("draw")) => self.draw_from_rack(station, None, p),
            Ok(Some("overview")) => self.act_fluid_generate_overview(&Map::from_iter([(
                "station".to_owned(),
                Value::String(station.to_string()),
            )])),
            Ok(Some("bus")) => {
                // The bus page named, else the only one; with none one is made, asking nothing; with several the
                // window asks which (the button ended in "make one with New page first").
                // A station whose part has no bus interface cannot stand on a bus page: said in the view.
                let (tag, bus) = self.project_session().map_or((String::new(), false), |s| {
                    (
                        s.device_tag(station).unwrap_or_default(),
                        s.has_bus_interfaces(station),
                    )
                });
                if !bus {
                    return self.rack_refuse(cursor, strings::format(key!("rack-no-bus-interface"), &[&tag]));
                }
                let pages = self.bus_pages();
                let new = |s: &mut Self| {
                    let folio = s.free_folio("bus_topology");
                    let title = strings::text(key!("rack-bus-page-title"));
                    let made = s.dispatch(
                        "page.new",
                        &json!({ "folio": folio, "type": "bus_topology", "title": title }),
                    );
                    (made == Outcome::Done).then_some(folio)
                };
                let folio = match string_param(p, "folio") {
                    Ok(Some(f)) if pages.iter().any(|(x, _)| x == f) => Some(f.to_owned()),
                    Ok(Some("new")) => new(self),
                    Ok(Some(_)) => return failed(key!("error-bad-param"), &["folio"]),
                    Err(e) => return e,
                    Ok(None) => match pages.as_slice() {
                        [(only, _)] => Some(only.clone()),
                        [] => new(self),
                        _ => {
                            return self.rack_refuse(
                                cursor,
                                strings::text(key!("reason-rack-which-bus-page")).to_owned(),
                            );
                        }
                    },
                };
                let Some(folio) = folio else {
                    return self
                        .rack_refuse(cursor, strings::text(key!("reason-rack-no-bus-page")).to_owned());
                };
                // Every interface not drawn there, a box each (the CPU stood there with its DP alone).
                let interfaces = self
                    .project_session()
                    .map(|s| s.undrawn_interfaces(station, &folio))
                    .unwrap_or_default();
                if interfaces.is_empty() {
                    return self
                        .rack_refuse(cursor, strings::format(key!("rack-bus-drawn"), &[&tag, &folio]));
                }
                let opened = self.dispatch("page.goto", &json!({ "folio": folio }));
                if opened != Outcome::Done {
                    return opened;
                }
                self.put_station_here(station)
            }
            Ok(_) => failed(key!("error-bad-param"), &["step"]),
            Err(e) => e,
        }
    }

    /// Place here on PLC part `device`: the side of it the page on show takes and no page of that kind
    /// draws yet, `draw` (its box on an electrical page), `bus` (its bus ports on a bus page) or `fluid` (a valve
    /// terminal's positions on a fluid page); else, for a side not drawn yet, why not in words: the page to open.
    /// `None` when every side of it is drawn, or for a device that is no PLC station's.
    pub fn place_here(&self, device: Uuid) -> Option<Result<&'static str, Key<'static>>> {
        let session = self.project_session()?;
        let station = session.plc_station_of(device)?;
        let electrical = session.off_electrical_pages(station).contains(&device);
        let bus = device == station
            && session.has_bus_interfaces(station)
            && !self
                .bus_pages()
                .iter()
                .any(|(folio, _)| session.undrawn_interfaces(station, folio).is_empty());
        let fluid = device == station
            && session
                .undrawn_fluid_terminals()
                .iter()
                .any(|(t, _)| *t == station);
        let kind = self.page_rows().get(self.page_index()).map(|r| r.kind.as_str());
        Some(match kind {
            Some("multi_line" | "single_line") if electrical => Ok("draw"),
            Some("bus_topology") if bus => Ok("bus"),
            Some("fluid") if fluid => Ok("fluid"),
            _ if electrical => Err(key!("reason-place-electrical")),
            _ if bus => Err(key!("reason-place-bus")),
            _ if fluid => Err(key!("reason-place-fluid")),
            _ => return None,
        })
    }

    /// Whether `station`'s bus side (its bus ports) stands on no bus page yet.
    pub(crate) fn bus_side_undrawn(&self, station: Uuid) -> bool {
        self.project_session().is_some_and(|session| {
            session.has_bus_interfaces(station)
                && !self
                    .bus_pages()
                    .iter()
                    .any(|(folio, _)| session.undrawn_interfaces(station, folio).is_empty())
        })
    }

    /// `plc.module_place {device?}`: Place here on a PLC part (`device`; absent, the device under the
    /// station view's cursor) that the page on show takes ([`ShellModel::place_here`]): its box on the pointer on an
    /// electrical page, as after choosing it in the device browser; its bus ports on a bus page; a valve terminal's
    /// positions on a fluid page. From the station view the page the view stands on is shown first. Refused in words
    /// where that page does not take it.
    pub(crate) fn act_plc_module_place(&mut self, p: &Map<String, Value>) -> Outcome {
        let device = match p.get("device") {
            Some(_) => or_return!(self.record_param(p, "device")),
            None => match self.station_device() {
                Some(device) => device,
                None => return Outcome::refused(key!("reason-rack-no-module")),
            },
        };
        let step = match self.place_here(device) {
            Some(Ok(step)) => step,
            Some(Err(reason)) => return Outcome::refused(reason),
            None => return Outcome::refused(key!("reason-place-drawn")),
        };
        if self.station_tab().is_some() {
            let folio = self.current_folio().unwrap_or_default().to_owned();
            let shown = self.dispatch("page.goto", &json!({ "folio": folio }));
            if shown != Outcome::Done {
                return shown;
            }
        }
        match step {
            "bus" => self.put_station_here(device),
            "fluid" => self.act_fluid_draw_terminal(&Map::from_iter([(
                "terminal".to_owned(),
                Value::String(device.to_string()),
            )])),
            _ => self.arm_station_box(device),
        }
    }

    /// The box of PLC part `device` on the pointer on the electrical page shown, as its part's black-box entry lays
    /// its pins (the station view's Draw on a page… lays it out the same way); one click draws it with
    /// `plc.draw_station`, which makes its functions when it has none.
    fn arm_station_box(&mut self, device: Uuid) -> Outcome {
        let page = self.page_rows().get(self.page_index()).map(|r| r.id);
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let (Some(station), Some(part), Some(page)) =
            (session.plc_station_of(device), session.device_part(device), page)
        else {
            return Outcome::refused(key!("reason-place-no-box"));
        };
        let label = session.device_tag(device).unwrap_or_default();
        let Some(params) = session.station_draw_params(station, page) else {
            return Outcome::refused(key!("reason-rack-no-frame"));
        };
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let cps = cat
            .parts
            .iter()
            .find(|r| r.copy == part)
            .and_then(|r| r.functions.iter().find(|f| f.black_box))
            .map(|f| f.cps.clone());
        let (Some(cps), Some(symbol)) = (cps, params["symbol"].as_str()) else {
            return Outcome::refused(key!("reason-place-no-box"));
        };
        let copies = params["copies"].as_array().cloned().unwrap_or_default();
        let target = crate::tools::black_box::BoxTarget::Station {
            station: station.to_string(),
            device: device.to_string(),
            label,
            cps,
        };
        self.tools.picker = None;
        self.activate_tool(Box::new(
            crate::tools::black_box::BlackBoxTool::new(symbol.to_owned(), target).with_copies(copies),
        ));
        Outcome::Done
    }
}
