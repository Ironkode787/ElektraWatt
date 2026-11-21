// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests of the pure parts of `bless-oracle` (`check-task`'s are in `check_task/tests.rs`): `bless-oracle` itself,
//! which opens a copy of Golden A with the built binaries, is the lane's command and is run by hand.

use super::*;
use crate::blind::score::SOFT;

fn row(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|s| (*s).to_owned()).collect()
}

/// The golden parts list restricted to the oracle's folios, with the demo variant's quantities, renumbered.
#[test]
fn the_parts_list_is_restricted_to_the_oracles_folios_with_the_demo_quantities() {
    let golden = parse_csv(
        "row,Pos,Qty,Unit,Manufacturer,Part number,Description,Tags,Folios\n\
         data,1,1,pc,Festo,532216,Plug,-3X1,3\n\
         data,2,1,pc,Eaton,533356,\"Main switch, 2-pole\",S51,101\n\
         data,3,20,pc,Phoenix Contact,PT 2.5-TWIN,Terminal,-X1031:1 … 20,103\n\
         data,4,5,pc,Phoenix Contact,PT 2.5-TWIN-PE,PE terminal,-X1032:1 … 5,103\n\
         data,5,2,pc,Festo,550208,Block,\"-201U1, -202U1\",\"201, 202\"\n\
         data,6,4,pc,TBD,TBD,End bracket,\"1X1, -X1031\",\"101, 103\"\n\
         data,7,6,pc,TBD,TBD,Plug-in bridge,-X1031,103\n\
         data,8,1,pc,Schneider Electric,NSYPLA1074G,Enclosure,-UC1,\n\
         heading,,,,,,Bridges,,\n",
    );
    let folios = ["101", "102", "103"].map(str::to_owned);
    let placed = BTreeMap::from([("PT 2.5-TWIN".to_owned(), 11), ("PT 2.5-TWIN-PE".to_owned(), 4)]);
    let bom = restrict_bom(&golden, &folios, &placed, 3).unwrap();
    assert_eq!(
        bom,
        [
            row(&["1", "1", "pc", "Eaton", "533356", "Main switch, 2-pole"]),
            row(&["2", "11", "pc", "Phoenix Contact", "PT 2.5-TWIN", "Terminal"]),
            row(&["3", "4", "pc", "Phoenix Contact", "PT 2.5-TWIN-PE", "PE terminal"]),
            row(&["4", "4", "pc", "TBD", "TBD", "End bracket"]),
            row(&["5", "3", "pc", "TBD", "TBD", "Plug-in bridge"]),
        ],
        "a row of another folio or of none is out; the placed terminals and the bridges are counted"
    );
    assert!(restrict_bom(&parse_csv("row,Qty\n"), &folios, &placed, 3).is_err());
    assert!(restrict_bom(&[], &folios, &placed, 3).is_err());
}

/// `counts.toml` against the golden headers: the demo variant swaps the strip segments for the terminals.
#[test]
fn counts_are_checked_against_the_golden_headers() {
    let count = |folio: &str, reps, connections, junctions, terminals| FolioCount {
        folio: folio.to_owned(),
        reps,
        connections,
        junctions,
        terminals,
        strips: terminals.map(|_| 2),
        source: "t".to_owned(),
        ..FolioCount::default()
    };
    let golden = BTreeMap::from([
        ("101".to_owned(), ((17, 21, 2), 0)),
        ("103".to_owned(), ((23, 20, 0), 3)),
    ]);
    let good = [count("101", 17, 21, 2, None), count("103", 35, 20, 0, Some(15))];
    check_counts(&good, &golden).unwrap();
    let bad = [
        count("101", 16, 21, 3, None),
        count("103", 23, 20, 0, Some(15)),
        count("109", 1, 1, 1, None),
    ];
    let err = check_counts(&bad, &golden).unwrap_err().to_string();
    assert!(
        err.contains("folio 101 reps: counts.toml says 16, the golden says 17"),
        "{err}"
    );
    assert!(
        err.contains("folio 101 junctions: counts.toml says 3, the golden says 2"),
        "{err}"
    );
    assert!(
        err.contains("folio 103 reps: counts.toml says 23, the golden says 35"),
        "{err}"
    );
    assert!(
        err.contains("folio 109: the golden copy has no page text"),
        "{err}"
    );
}

/// The script that describes a copy opens it read-only by a bare name and quits without saving; the CSV of a row set
/// has its header and sorted rows.
#[test]
fn the_describe_script_and_the_row_files() {
    let s = describe_script("J-E-2416.ew", &["101".to_owned(), "102".to_owned()]);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(
        lines[1],
        r#"{"action":"file.open","params":{"dir":"J-E-2416.ew","read_only":true}}"#
    );
    assert_eq!(lines[2], r#"{"action":"page.describe","params":{"folio":"101"}}"#);
    assert_eq!(lines[4], r#"{"action":"app.quit","params":{"discard":true}}"#);
    let text = rows_text("pages", vec![row(&["102", "B"]), row(&["101", "A, with comma"])]);
    assert_eq!(text, "folio,title\n101,\"A, with comma\"\n102,B\n");
    // A row set that has equal rows keeps both (two wires between the same ends).
    let text = rows_text(
        "ends",
        vec![row(&["101", "a", "b", "101"]), row(&["101", "a", "b", "101"])],
    );
    assert_eq!(text.lines().count(), 3);
    for (_, set, _) in SOFT {
        assert!(!header_of(set).is_empty(), "{set}");
    }
}

/// The oracle of `gpp-101-103` ends 1X1's field wires (the external points) on clamp b and its internal wires on
/// clamp a, as Golden A does since its terminals got their side-2 clamp (a feed-through terminal cannot carry the field
/// cable and the internal wire on one side).
#[test]
fn the_gpp_oracle_ends_1x1s_field_side_on_clamp_b() {
    let path = task_dir(&crate::workspace_root(), "gpp-101-103").join("oracle/rows/ends.csv");
    let rows = parse_csv(&std::fs::read_to_string(path).unwrap().replace("\r\n", "\n"));
    let on_1x1: Vec<&Vec<String>> = rows.iter().filter(|r| r[1].starts_with("1X1:")).collect();
    assert_eq!(on_1x1.len(), 5, "{on_1x1:?}");
    for r in on_1x1 {
        let clamp = if r[2].starts_with("ext ") { "/b" } else { "/a" };
        assert!(r[1].ends_with(clamp), "{r:?}");
    }
}
