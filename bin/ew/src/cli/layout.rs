// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew layout [<project-dir>] [--page <folio>] [--json]`: the layout measure (docs/design/06a-drafting.md §1.2) of
//! one page or of every schematic page, in text or as `ew.layout-result/1`: its faults, unrouted wires and cost, the
//! figures a reader compares, and each item behind them.

use ew_api::cli_results as results;
use ew_api::output::PageLayout;

use super::{LayoutArgs, reader_dir};

/// `ew layout`: opens the project as a reader (no lock) and prints the layout measures. Exit 0 when measured, 2 on
/// usage, open errors and a folio that names no drawn page.
pub(super) fn run_layout(a: &LayoutArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let session = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "layout") {
        Ok((session, _)) => session,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let pages = match session.layout_report(a.page.as_deref()) {
        Ok(pages) => pages,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if a.json {
        print!(
            "{}",
            results::to_text(&results::LayoutResult {
                schema: results::LAYOUT.to_owned(),
                pages,
            })
        );
    } else {
        print!("{}", text(&pages));
    }
    0
}

/// ` in <cell>`, or nothing without a cell.
fn place(cell: &str) -> String {
    if cell.is_empty() {
        String::new()
    } else {
        format!(" in {cell}")
    }
}

/// The text of the measure: per page one line of figures, then one line per item.
fn text(pages: &[PageLayout]) -> String {
    let mut out = String::new();
    for p in pages {
        out.push_str(&format!(
            "{}: faults {} (through points {}, on wires {}, through symbols {}, four-way junctions {}, false joints {}, \
             parts on parts {}); unrouted {}; cost {}; crossings {}, bends {}, straight {} of {}, turn-backs {}, \
             detour {} %, labels moved {}; texts through {}, on texts {}, on symbols {}, outside {}\n",
            p.folio,
            p.faults,
            p.touches.len(),
            p.overlaps.len(),
            p.through_symbols.len(),
            p.four_way_junctions.len(),
            p.false_joints.len(),
            p.parts_on_parts.len(),
            p.unrouted,
            p.cost,
            p.crossings.len(),
            p.bend_total(),
            p.straight,
            p.wires,
            p.turn_backs.iter().map(|b| b.bends).sum::<usize>(),
            p.detour,
            p.labels_moved,
            p.through_texts.len(),
            p.texts_on_texts.len(),
            p.texts_on_symbols.len(),
            p.texts_outside.len(),
        ));
        for t in &p.touches {
            out.push_str(&format!(
                "  touch: {} touches {}{}\n",
                t.connection,
                t.point,
                place(&t.cell)
            ));
        }
        for o in &p.overlaps {
            out.push_str(&format!(
                "  overlap: {} runs along {}{}\n",
                o.connection,
                o.other,
                place(&o.cell)
            ));
        }
        for t in &p.through_symbols {
            out.push_str(&format!(
                "  through a symbol: {} runs through {}{}\n",
                t.connection,
                t.symbol,
                place(&t.cell)
            ));
        }
        for j in &p.four_way_junctions {
            out.push_str(&format!(
                "  four-way junction: the junction{} has {} arms\n",
                place(&j.cell),
                j.arms
            ));
        }
        for c in &p.false_joints {
            out.push_str(&format!(
                "  false joint: {} and {} turn on one point{}\n",
                c.connection,
                c.other,
                place(&c.cell)
            ));
        }
        for s in &p.parts_on_parts {
            if s.other.is_empty() {
                out.push_str(&format!(
                    "  part on a part: {} stands outside the drawing area\n",
                    s.symbol
                ));
            } else {
                out.push_str(&format!("  part on a part: {} lies on {}\n", s.symbol, s.other));
            }
        }
        for t in &p.through_texts {
            out.push_str(&format!(
                "  through a text: {} runs through a text of {}{}\n",
                t.connection,
                t.symbol,
                place(&t.cell)
            ));
        }
        for t in &p.texts_on_texts {
            out.push_str(&format!(
                "  text on a text: \"{}\" lies on \"{}\"{}\n",
                t.text,
                t.other,
                place(&t.cell)
            ));
        }
        for t in &p.texts_on_symbols {
            out.push_str(&format!(
                "  text on a symbol: \"{}\" lies on {}{}\n",
                t.text,
                t.other,
                place(&t.cell)
            ));
        }
        for t in &p.texts_outside {
            out.push_str(&format!(
                "  text outside: \"{t}\" stands outside the drawing area\n"
            ));
        }
        for c in &p.crossings {
            out.push_str(&format!(
                "  crossing: {} crosses {}{}\n",
                c.connection,
                c.other,
                place(&c.cell)
            ));
        }
        for b in &p.bends {
            out.push_str(&format!("  bends: {}: {}\n", b.connection, b.bends));
        }
        for b in &p.turn_backs {
            out.push_str(&format!("  turn-backs: {}: {}\n", b.connection, b.bends));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use ew_api::output::{
        LayoutBends, LayoutCrossing, LayoutJunction, LayoutPair, LayoutParts, LayoutTexts, LayoutThrough,
        LayoutTouch, PageLayout,
    };

    use super::text;

    #[test]
    fn the_text_has_a_line_of_figures_and_one_line_per_item_its_place_only_with_a_cell() {
        let s = |t: &str| t.to_owned();
        let wire = s("-K1:13 → -X1:2");
        let other = s("-K2:A1 → -X1:3");
        let page = PageLayout {
            folio: s("7"),
            faults: 6,
            unrouted: 1,
            cost: 120,
            straight: 3,
            wires: 5,
            detour: 130,
            labels_moved: 2,
            touches: vec![LayoutTouch {
                connection: wire.clone(),
                point: s("the junction"),
                at: [0, 0],
                cell: s("A1"),
            }],
            overlaps: vec![LayoutPair {
                connection: wire.clone(),
                other: other.clone(),
                stretches: vec![[[0, 0], [4, 0]]],
                cell: String::new(),
            }],
            through_symbols: vec![LayoutThrough {
                connection: wire.clone(),
                symbol: s("-K3"),
                stretches: vec![[[0, 0], [4, 0]]],
                cell: s("B2"),
            }],
            four_way_junctions: vec![LayoutJunction {
                arms: 4,
                at: [0, 0],
                cell: s("C4"),
            }],
            false_joints: vec![LayoutCrossing {
                connection: wire.clone(),
                other: other.clone(),
                at: [0, 0],
                cell: s("C5"),
            }],
            parts_on_parts: vec![
                LayoutParts {
                    symbol: s("-K3"),
                    other: s("-K4"),
                },
                LayoutParts {
                    symbol: s("-K5"),
                    other: String::new(),
                },
            ],
            through_texts: vec![LayoutThrough {
                connection: wire.clone(),
                symbol: s("interruption point L1"),
                stretches: vec![[[0, 0], [4, 0]]],
                cell: s("B3"),
            }],
            texts_on_texts: vec![LayoutTexts {
                text: s("-K3"),
                other: s("13"),
                cell: s("B4"),
            }],
            texts_on_symbols: vec![LayoutTexts {
                text: s("-K4"),
                other: s("-K3"),
                cell: String::new(),
            }],
            texts_outside: vec![s("-K6")],
            crossings: vec![LayoutCrossing {
                connection: wire.clone(),
                other,
                at: [0, 0],
                cell: s("D5"),
            }],
            bends: vec![LayoutBends {
                connection: wire.clone(),
                bends: 2,
            }],
            turn_backs: vec![LayoutBends {
                connection: wire,
                bends: 1,
            }],
        };
        assert_eq!(
            text(&[page]),
            "7: faults 6 (through points 1, on wires 1, through symbols 1, four-way junctions 1, false joints 1, parts \
             on parts 2); unrouted 1; cost 120; crossings 1, bends 2, straight 3 of 5, turn-backs 1, detour 130 %, labels \
             moved 2; texts through 1, on texts 1, on symbols 1, outside 1\n\
             \x20 touch: -K1:13 → -X1:2 touches the junction in A1\n\
             \x20 overlap: -K1:13 → -X1:2 runs along -K2:A1 → -X1:3\n\
             \x20 through a symbol: -K1:13 → -X1:2 runs through -K3 in B2\n\
             \x20 four-way junction: the junction in C4 has 4 arms\n\
             \x20 false joint: -K1:13 → -X1:2 and -K2:A1 → -X1:3 turn on one point in C5\n\
             \x20 part on a part: -K3 lies on -K4\n\
             \x20 part on a part: -K5 stands outside the drawing area\n\
             \x20 through a text: -K1:13 → -X1:2 runs through a text of interruption point L1 in B3\n\
             \x20 text on a text: \"-K3\" lies on \"13\" in B4\n\
             \x20 text on a symbol: \"-K4\" lies on -K3\n\
             \x20 text outside: \"-K6\" stands outside the drawing area\n\
             \x20 crossing: -K1:13 → -X1:2 crosses -K2:A1 → -X1:3 in D5\n\
             \x20 bends: -K1:13 → -X1:2: 2\n\
             \x20 turn-backs: -K1:13 → -X1:2: 1\n"
        );
    }
}
