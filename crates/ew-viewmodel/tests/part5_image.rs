// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 5 in the view model (acceptance check IM2): the Image tool places an asset the project holds by two
//! fine-snapped corners keeping the asset's aspect ratio; `image.pick`'s refusals and failures; Back and Escape;
//! the ghost's transcript member and describe line and their vocabulary; `page.describe`'s `image` item line; one
//! undo step; and no `Session` read on pointer moves. Every test works on a writable temporary copy of Golden A (the
//! asset is a placeholder SVG written here).

mod common;

use std::sync::Arc;

use common::{Tmp, at, done, refused, state, structural_tokens};
use ew_viewmodel::ew_api::output::PageTables;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_viewmodel::tools::{Ghost, fit_rect};
use serde_json::{Value, json};

/// The placeholder asset: an SVG of 40 × 10 user units.
const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10"><rect width="40" height="10" fill="#123456"/></svg>"##;
/// Its hash.
const SVG_HASH: &str = "sha256:ec57251790a327f9afde596a256f674c950610537531d4ed0b07bb248882a5d1";

/// A headless shell with a writable copy of Golden A on `folio`, at actual size.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// Brings the placeholder asset into the project (the company logo command is the one that stores an asset) and takes
/// the logo off again: the asset stays in the project's store.
fn hold_asset(s: &mut ShellModel) {
    for logo in [ew_viewmodel::ew_api::base64_encode(SVG), String::new()] {
        done(
            s,
            "command.run",
            json!({"cmd": "project.company.set", "params": {"logo": logo}}),
        );
    }
}

fn tables(s: &ShellModel) -> Arc<PageTables> {
    Arc::clone(&s.canvas().unwrap().page().tables)
}

fn sheet(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    s.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The page point of grid index `k`.
fn grid_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// A click at page point `p` through the grid cursor.
fn click_at(s: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(s, p);
    done(s, "cursor.set", at(q, json!({})));
    done(s, "canvas.activate", json!({}));
}

fn canvas(s: &ShellModel) -> Value {
    state(s)["canvas"].clone()
}

fn describe_line(s: &ShellModel, prefix: &str) -> Option<String> {
    s.describe()
        .lines()
        .find(|l| l.starts_with(prefix))
        .map(str::to_owned)
}

fn item_count(s: &ShellModel) -> usize {
    tables(s).items.len()
}

/// A shell with the Image tool armed with the placeholder asset.
fn armed(tag: &str) -> (Tmp, ShellModel) {
    let tmp = Tmp::new(tag);
    let mut s = shell(&tmp, "101");
    hold_asset(&mut s);
    done(&mut s, "tool.image", json!({}));
    done(&mut s, "image.pick", json!({ "asset": SVG_HASH }));
    (tmp, s)
}

/// IM2: Insert ▸ Image…, the picked asset, two corners: one `item.place` image whose rectangle keeps the 4:1 ratio of
/// the SVG, listed by the page tables and `page.describe`, one undo step.
#[test]
fn the_image_tool_places_the_asset_by_two_corners_keeping_its_aspect() {
    let tmp = Tmp::new("image-place");
    let mut s = shell(&tmp, "101");
    hold_asset(&mut s);
    let page = tables(&s).page.to_string();

    done(&mut s, "tool.image", json!({}));
    assert_eq!(s.tool().name(), "image");
    assert_eq!(s.tool().state(), "picking");
    assert_eq!(s.tool_ghost(), Some(Ghost::Image { asset: None }));
    assert_eq!(canvas(&s)["ghost"], json!({"kind": "image", "asset": null}));
    assert_eq!(
        describe_line(&s, "canvas ghost item image").as_deref(),
        Some("canvas ghost item image")
    );
    let a = grid_point(&s, (40, 20));
    let b = grid_point(&s, (60, 26));
    click_at(&mut s, a);
    assert_eq!(
        s.tool().state(),
        "picking",
        "a click before the pick does nothing"
    );
    assert!(s.recorded_commands().iter().all(|c| c.cmd != "item.place"));

    done(&mut s, "image.pick", json!({ "asset": SVG_HASH }));
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(canvas(&s)["ghost"], json!({"kind": "image", "asset": SVG_HASH}));
    assert_eq!(
        describe_line(&s, "canvas ghost item image").as_deref(),
        Some(format!("canvas ghost item image \"{SVG_HASH}\"").as_str())
    );

    let before = item_count(&s);
    click_at(&mut s, a);
    assert_eq!(s.tool().state(), "placing");
    click_at(&mut s, b);
    assert_eq!(s.tool().state(), "armed", "armed again with the asset kept");
    let (min, max) = fit_rect(a, b, Some((40, 10))).expect("a box with area");
    // 20 steps wide and 6 high is wider than 4:1 allows: the width limits, the height is a quarter of it.
    assert_eq!(max.0 - min.0, (b.0 - a.0).abs(), "the box's width");
    // The height is rounded to the nearest nm: four times it is within 2 nm of the width.
    assert!(
        (4 * (max.1 - min.1) - (max.0 - min.0)).abs() <= 2,
        "the 4:1 ratio of the SVG: {min:?} {max:?}"
    );
    assert_eq!(
        (min, max.1 > min.1),
        (a, true),
        "from the first corner towards the second"
    );
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "item.place");
    assert_eq!(
        last.params,
        json!({"page": page, "kind": {"image": {"asset": SVG_HASH,
            "rect": [[min.0, min.1], [max.0, max.1]]}}}),
        "{last:?}"
    );
    let t = tables(&s);
    assert_eq!(t.items.len(), before + 1);
    let row = t.items.iter().find(|i| i.kind == "image").expect("the image row");
    assert_eq!(row.bounds, [min.0, min.1, max.0, max.1]);

    // `page.describe` names it by its word and cell.
    done(&mut s, "page.describe", json!({}));
    let text = s.take_page_text().expect("the page text");
    let line = text
        .lines()
        .find(|l| l.starts_with("  item image cell "))
        .unwrap_or_else(|| panic!("an image item line in:\n{text}"));
    for token in structural_tokens(line) {
        assert!(
            ew_viewmodel::describe::VOCABULARY.contains(&token.as_str()),
            "`{token}` in {line}"
        );
    }

    // One undo step takes it away.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(item_count(&s), before);
}

/// The rectangle grows towards the second corner and fits the box: a drag up and to the left.
#[test]
fn the_rectangle_grows_towards_the_second_corner() {
    let (_tmp, mut s) = armed("image-direction");
    let a = grid_point(&s, (60, 30));
    let b = grid_point(&s, (50, 28));
    click_at(&mut s, a);
    click_at(&mut s, b);
    let (min, max) = fit_rect(a, b, Some((40, 10))).unwrap();
    // 10 steps left and 2 up: the height limits, the width is four times it.
    assert_eq!(max, a, "the first corner stays");
    assert!(
        ((max.0 - min.0) - 4 * (max.1 - min.1)).abs() <= 2,
        "the 4:1 ratio of the SVG"
    );
    assert!(min.0 > b.0 && min.1 == b.1, "{min:?} against {b:?}");
    let last = s.recorded_commands().last().unwrap();
    assert_eq!(
        last.params["kind"]["image"]["rect"],
        json!([[min.0, min.1], [max.0, max.1]])
    );
}

/// A box without area commits nothing; Back and Escape drop the first corner; Escape on the armed tool returns to
/// Select.
#[test]
fn a_flat_box_commits_nothing_and_back_and_escape_drop_the_corner() {
    let (_tmp, mut s) = armed("image-back");
    let before = item_count(&s);
    let a = grid_point(&s, (40, 20));
    click_at(&mut s, a);
    assert_eq!(s.tool().state(), "placing");
    let flat = grid_point(&s, (50, 20));
    click_at(&mut s, flat);
    assert_eq!(s.tool().state(), "placing", "a box without height");
    let narrow = grid_point(&s, (40, 25));
    click_at(&mut s, narrow);
    assert_eq!(s.tool().state(), "placing", "a box without width");
    assert_eq!(item_count(&s), before);
    done(&mut s, "tool.back", json!({}));
    assert_eq!(s.tool().state(), "armed");
    click_at(&mut s, a);
    assert_eq!(s.tool().state(), "placing");
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(s.tool().name(), "image");
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select", "Escape on the armed tool ends it");
    assert_eq!(item_count(&s), before);
}

/// The dwell previews the command a click would commit; the preview changes nothing.
#[test]
fn the_dwell_previews_the_placement() {
    let (_tmp, mut s) = armed("image-dwell");
    let before = item_count(&s);
    let a = grid_point(&s, (40, 20));
    let b = grid_point(&s, (60, 26));
    click_at(&mut s, a);
    let q = sheet(&s, b);
    done(&mut s, "cursor.set", at(q, json!({})));
    done(&mut s, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(s.tool().state(), "placing");
    assert_eq!(item_count(&s), before, "a preview commits nothing");
}

/// `image.pick`: refused outside the tool; fails without `asset`, on text that is no hash and on a hash the project
/// does not hold; the tool is unchanged by a failure.
#[test]
fn image_pick_refusals_and_failures() {
    let tmp = Tmp::new("image-pick");
    let mut s = shell(&tmp, "101");
    hold_asset(&mut s);
    assert_eq!(
        refused(&mut s, "image.pick", json!({ "asset": SVG_HASH })),
        "reason-image-tool-only"
    );
    done(&mut s, "tool.image", json!({}));
    for params in [
        json!({}),
        json!({"asset": 5}),
        json!({"asset": "sha256:zz"}),
        json!({"asset": SVG_HASH.to_uppercase()}),
        json!({"asset": format!("sha256:{}", "ab".repeat(32))}),
    ] {
        match s.dispatch("image.pick", &params) {
            Outcome::Failed { message } => assert!(!message.is_empty()),
            other => panic!("{params}: {other:?}"),
        }
        assert_eq!(s.tool().state(), "picking", "{params}");
    }
    let held = format!("sha256:{}", "ab".repeat(32));
    match s.dispatch("image.pick", &json!({ "asset": held })) {
        Outcome::Failed { message } => {
            assert!(
                message.contains(&held) && message.contains("no image asset"),
                "{message}"
            )
        }
        other => panic!("{other:?}"),
    }
    done(&mut s, "image.pick", json!({ "asset": SVG_HASH }));
    assert_eq!(s.tool().state(), "armed");
    // Picking again in the placing state drops the corner and keeps the tool.
    let a = grid_point(&s, (40, 20));
    click_at(&mut s, a);
    assert_eq!(s.tool().state(), "placing");
    done(&mut s, "image.pick", json!({ "asset": SVG_HASH }));
    assert_eq!(s.tool().state(), "armed");
}

/// A page change ends the tool like every editing tool, and pointer moves read nothing from the session.
#[test]
fn a_page_change_ends_the_tool_and_moves_read_nothing() {
    let (_tmp, mut s) = armed("image-moves");
    let p = grid_point(&s, (40, 30));
    let step = |k: i64| (p.0 + k * 300_000, p.1 + k * 200_000);
    let calls = s.session_calls();
    for k in 0..20 {
        let q = sheet(&s, step(k));
        done(&mut s, "canvas.pointer", at(q, json!({})));
    }
    assert_eq!(s.session_calls(), calls, "the armed Image tool reads nothing");
    click_at(&mut s, p);
    let calls = s.session_calls();
    for k in 0..20 {
        let q = sheet(&s, step(k));
        done(&mut s, "canvas.pointer", at(q, json!({})));
    }
    assert_eq!(s.session_calls(), calls, "the placing Image tool reads nothing");
    done(&mut s, "page.goto", json!({"folio": "102"}));
    assert_eq!(s.tool().name(), "select");
}

/// The ghost line's words are vocabulary words, the one word part 5 adds is `image`, and the menu has Insert ▸ Image….
#[test]
fn the_image_ghost_and_menu_use_the_vocabulary() {
    let (_tmp, s) = armed("image-words");
    for l in s.describe().lines().filter(|l| l.starts_with("canvas ")) {
        for t in structural_tokens(l) {
            assert!(
                ew_viewmodel::describe::VOCABULARY.contains(&t.as_str()),
                "`{t}` is no vocabulary word; line: {l}"
            );
        }
    }
    assert_eq!(ew_viewmodel::describe::IMAGE_WORDS, ["image"]);
    assert_eq!(
        ew_viewmodel::describe::VOCABULARY
            .iter()
            .filter(|w| **w == "image")
            .count(),
        1
    );
    let text = s.describe();
    assert!(
        text.lines()
            .any(|l| l.contains("Image…") && l.contains("tool.image")),
        "Insert ▸ Image… in the menu:\n{text}"
    );
}
