// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A card drawn from the station view one box at a time (docs/design/06a-drafting.md §7.5): a box of a card whose
//! black-box function a page draws already is another drawing of it when none of its pins is drawn yet; until every
//! pin of its channels is drawn, the card is drawn in part and "Draw on a page" offers the rest.

use ew_api::{AnyCommand, Session};
use ew_ids::Uuid;
use serde_json::{Value, json};

use crate::common::{first_page, library_part, new_project};

/// Three steps of the QET 10 px grid of the new project's page.
const STEP3: i64 = 7_937_500;

/// A station of the starter CPU with one starter input card, drawn nowhere: the station and the card.
fn station_with_input_card(s: &mut Session) -> (Uuid, Uuid) {
    let [cpu, di] = ["EW-GEN-CPU-1", "EW-GEN-DI-1"].map(|n| library_part(s, n));
    let mut copies: Vec<Value> = Vec::new();
    for p in [cpu, di] {
        for d in s.module_part_copies(p) {
            if !copies.contains(&d) {
                copies.push(d);
            }
        }
    }
    let part = |p: Uuid| json!({"part": {"copy": p.to_string()}, "qty": "1"});
    s.execute(&AnyCommand {
        name: "plc.add_station".into(),
        params: json!({"part": part(cpu), "modules": [{"part": part(di)}], "copies": copies}),
    })
    .expect("the station");
    let q = s.query();
    let station = q
        .devices()
        .iter()
        .find(|(_, d)| d.aspects.plc_station.is_some())
        .map(|(id, _)| id.as_uuid())
        .expect("the station");
    let card = q
        .devices()
        .iter()
        .find(|(_, d)| d.parent.is_some_and(|p| p.as_uuid() == station))
        .map(|(id, _)| id.as_uuid())
        .expect("the card");
    (station, card)
}

/// `plc.draw_station` of one box of `card` at column `at` (in threes of steps) with the pins `pins` on its upper edge.
fn draw(s: &mut Session, station: Uuid, card: Uuid, at: i64, pins: &[u16]) -> Result<(), String> {
    let page = first_page(s);
    let params = s.station_draw_params(station, page).expect("the page's frame");
    let w = STEP3 * (i64::try_from(pins.len()).unwrap() + 1);
    let cps: Vec<Value> = (1..)
        .zip(pins)
        .map(|(k, i)| json!({"idx": i, "pos": [k * STEP3, 0], "dir": "up"}))
        .collect();
    s.execute(&AnyCommand {
        name: "plc.draw_station".into(),
        params: json!({
            "station": station.to_string(), "page": page.to_string(), "symbol": params["symbol"],
            "copies": params.get("copies").cloned().unwrap_or_else(|| json!([])),
            "boxes": [{"device": card.to_string(), "pos": [at * STEP3, 20 * STEP3],
                "box": {"outline": [[0, 0], [w, 0], [w, 2 * STEP3], [0, 2 * STEP3]], "cps": cps}}],
        }),
    })
    .map(|_| ())
    .map_err(|e| format!("{e}"))
}

/// The drawings of `card`'s functions.
fn drawings(s: &Session, card: Uuid) -> usize {
    let q = s.query();
    q.idx()
        .functions_of_device(ew_ids::Id::from_uuid(card))
        .map(|f| q.idx().reps_of_function(f).count())
        .sum()
}

/// The first half of the input card's channels drawn: the card is drawn in part, and "Draw on a page" offers it with
/// the pins drawn; the second half is another drawing of the same function, and then nothing of it is left; a box
/// with a pin drawn already draws nothing.
#[test]
fn a_card_drawn_one_box_at_a_time_is_finished_by_its_further_box() {
    let mut s = new_project(711);
    let (station, card) = station_with_input_card(&mut s);
    let page = first_page(&s);
    draw(&mut s, station, card, 2, &[0, 1, 2, 3]).expect("the first box");
    assert_eq!(s.cards_drawn_in_part(station), [card]);
    let offered = s.station_draw_params(station, page).expect("the page's frame")["devices"].clone();
    assert!(
        offered
            .as_array()
            .into_iter()
            .flatten()
            .any(|d| d[0] == card.to_string() && d[2] == json!([0, 1, 2, 3])),
        "the rest of the card with its pins drawn: {offered}"
    );
    draw(&mut s, station, card, 10, &[4, 5, 6, 7]).expect("the further box");
    assert_eq!(drawings(&s, card), 2, "two drawings of the card");
    assert!(s.cards_drawn_in_part(station).is_empty(), "every channel's pin drawn");
    assert!(
        draw(&mut s, station, card, 20, &[0]).is_err(),
        "a pin drawn already is not drawn again"
    );
    assert_eq!(drawings(&s, card), 2);
}

/// How many channel pins one box of a card takes: all of them where the box fits the drawing area, else as many as
/// fit, sixteen at most.
#[test]
fn a_row_wider_than_the_drawing_area_is_split_into_boxes_of_at_most_sixteen_pins() {
    use ew_api::drafting::consts::{CARD_PIN_PITCH, CARD_PINS_MOST, TICKS_PER_STEP};
    use ew_api::part_box::card_share;
    let pitch = i64::from(CARD_PIN_PITCH / TICKS_PER_STEP);
    let room = |pins: i64| pins * pitch + 2;
    assert_eq!(card_share(20, room(20)), 20, "a row that fits is one box");
    assert_eq!(card_share(40, room(40) - 1), CARD_PINS_MOST);
    assert_eq!(card_share(16, room(10)), 10, "a narrow sheet takes fewer");
}
