// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew layout [<project-dir>] [--page <folio>] [--json]`: the layout measures
//! of one page or of every schematic page, in text or as `ew.layout-result/1`.

use std::path::PathBuf;

use ew_api::cli_results as results;
use ew_api::output::PageLayout;

use super::reader_dir;

/// The arguments of `ew layout`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LayoutArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The page by its folio label (`--page`); `None`: every drawn page but the panel-layout pages.
    pub page: Option<String>,
    /// Print the result as JSON (`--json`).
    pub json: bool,
}

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

/// The text of the measures: per page one line of counts, then one line per item.
fn text(pages: &[PageLayout]) -> String {
    let mut out = String::new();
    for p in pages {
        out.push_str(&format!(
            "{}: hard rules {} (touches {}, overlaps {}, through symbols {}, through texts {}, four-way junctions {}); \
             costs: crossings {}, bends {}\n",
            p.folio,
            p.hard(),
            p.touches.len(),
            p.overlaps.len(),
            p.through_symbols.len(),
            p.through_texts.len(),
            p.four_way_junctions.len(),
            p.crossings.len(),
            p.bend_total(),
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
        for t in &p.through_texts {
            out.push_str(&format!(
                "  through a text: {} runs through a text of {}{}\n",
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
    }
    out
}

#[cfg(test)]
mod tests {
    use ew_api::output::{
        LayoutBends, LayoutCrossing, LayoutJunction, LayoutPair, LayoutThrough, LayoutTouch, PageLayout,
    };

    use super::text;

    #[test]
    fn the_text_has_a_line_of_counts_and_one_line_per_item_its_place_only_with_a_cell() {
        let s = |t: &str| t.to_owned();
        let page = PageLayout {
            folio: s("7"),
            touches: vec![LayoutTouch {
                connection: s("-K1:13 → -X1:2"),
                point: s("the junction"),
                at: [0, 0],
                cell: s("A1"),
            }],
            overlaps: vec![LayoutPair {
                connection: s("-K1:13 → -X1:2"),
                other: s("-K2:A1 → -X1:3"),
                stretches: vec![[[0, 0], [4, 0]]],
                cell: String::new(),
            }],
            through_symbols: vec![LayoutThrough {
                connection: s("-K1:13 → -X1:2"),
                symbol: s("-K3"),
                stretches: vec![[[0, 0], [4, 0]]],
                cell: s("B2"),
            }],
            through_texts: vec![LayoutThrough {
                connection: s("-K1:13 → -X1:2"),
                symbol: s("interruption point L1"),
                stretches: vec![[[0, 0], [4, 0]]],
                cell: s("B3"),
            }],
            four_way_junctions: vec![LayoutJunction {
                arms: 4,
                at: [0, 0],
                cell: s("C4"),
            }],
            crossings: vec![LayoutCrossing {
                connection: s("-K1:13 → -X1:2"),
                other: s("-K2:A1 → -X1:3"),
                at: [0, 0],
                cell: s("D5"),
            }],
            bends: vec![LayoutBends {
                connection: s("-K1:13 → -X1:2"),
                bends: 2,
            }],
        };
        assert_eq!(
            text(&[page]),
            "7: hard rules 5 (touches 1, overlaps 1, through symbols 1, through texts 1, four-way junctions 1); \
             costs: crossings 1, bends 2\n\
             \x20 touch: -K1:13 → -X1:2 touches the junction in A1\n\
             \x20 overlap: -K1:13 → -X1:2 runs along -K2:A1 → -X1:3\n\
             \x20 through a symbol: -K1:13 → -X1:2 runs through -K3 in B2\n\
             \x20 through a text: -K1:13 → -X1:2 runs through a text of interruption point L1 in B3\n\
             \x20 four-way junction: the junction in C4 has 4 arms\n\
             \x20 crossing: -K1:13 → -X1:2 crosses -K2:A1 → -X1:3 in D5\n\
             \x20 bends: -K1:13 → -X1:2: 2\n"
        );
    }
}
