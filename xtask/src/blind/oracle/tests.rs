// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests of `check-task` and of the pure parts of `bless-oracle` (WP-S6.13 BD14). The committed task is checked against
//! doc 20 and the golden tables without a product: `bless-oracle` itself, which opens a copy of Golden A with the built
//! binaries, is the lane's command and is run by hand.

use super::*;
use crate::blind::run::copy_dir;
use crate::blind::testkit::Tmp;

fn row(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|s| (*s).to_owned()).collect()
}

fn doc20() -> String {
    std::fs::read_to_string(crate::workspace_root().join(DOC20))
        .unwrap()
        .replace("\r\n", "\n")
}

/// BD14: the committed tasks are sound: the files exist and parse, the prelude's classes fit its steps, the gaps name
/// checks, and every identifier-shaped token of the sheet occurs in doc 20.
#[test]
fn the_committed_tasks_are_sound() {
    let root = crate::workspace_root();
    for task in ["gpp-101-103", "free", "selftest"] {
        assert_eq!(check_task(&root, task).unwrap(), Vec::<String>::new(), "{task}");
    }
    assert!(check_task(&root, "no-such-task").is_err());
}

/// BD14: `oracle/counts.toml` equals doc 20 and WP-S2.04 §13.6 where the files can be compared without a product: the
/// pages' titles are doc 20 §5's, the representations of the oracle's cells are the golden pages' (17, 12 and 23, the
/// demo variant of folio 103 swapping its 3 strip segments for the 15 terminals), the junctions are the oracle's. The
/// headers of a copy of Golden A are compared by `bless-oracle`, which fails on a difference.
#[test]
fn the_oracles_counts_are_the_documents_and_the_rows_agree_with_them() {
    let root = crate::workspace_root();
    let dir = task_dir(&root, "gpp-101-103");
    let counts: CountsFile = read_toml(&dir.join("oracle/counts.toml")).unwrap();
    let rows = |set: &str| {
        let text = std::fs::read_to_string(dir.join(format!("oracle/rows/{set}.csv"))).unwrap();
        let mut r = parse_csv(&text);
        r.remove(0);
        r
    };
    let cells = rows("cells");
    let junctions = rows("junctions");
    for c in &counts.folio {
        let mine: Vec<&Vec<String>> = cells.iter().filter(|r| r[0] == c.folio).collect();
        let segments = mine.iter().filter(|r| r[2].contains("strip-segment")).count();
        // The demo variant: segments out, terminals in; the others as the golden page has them.
        let want = match c.terminals {
            Some(t) => u32::try_from(mine.len()).unwrap() - u32::try_from(segments).unwrap() + t,
            None => u32::try_from(mine.len()).unwrap(),
        };
        assert_eq!(c.reps, want, "folio {} reps", c.folio);
        let junctions_here = u32::try_from(junctions.iter().filter(|r| r[0] == c.folio).count()).unwrap();
        assert_eq!(c.junctions, junctions_here, "folio {} junctions", c.folio);
    }
    // The page titles of doc 20 §5.
    let doc = doc20();
    let pages = rows("pages");
    assert_eq!(pages.len(), 3);
    for p in &pages {
        assert!(
            doc.lines()
                .any(|l| l.starts_with(&format!("| {} |", p[0])) && l.contains(&p[1])),
            "doc 20 §5 has no row `{} {}`",
            p[0],
            p[1]
        );
    }
    // The parts list is §14.1 restricted: every row's part number, manufacturer and description are in a §14.1 row.
    let bom = std::fs::read_to_string(dir.join("oracle/bom.csv")).unwrap();
    let bom = parse_csv(&bom);
    assert_eq!(bom.len(), 15, "14 rows and the header");
    for r in &bom[1..] {
        let (maker, part, desc) = (&r[3], &r[4], &r[5]);
        // The manufacturer of a TBD part is `▲` in the table (open item) and `TBD` in the export; a part number may
        // carry an open-item mark in the table.
        assert!(
            doc.lines().any(|l| l.starts_with("| ")
                && l.contains(&format!("| {part}"))
                && (maker == "TBD" || l.contains(&format!("| {maker} |")))
                && l.contains(desc.as_str())),
            "doc 20 §14.1 has no row for {maker} {part} {desc}"
        );
    }
    // Folio 103's demo quantities (WP-S2.04 §13.6): 11 + 4 terminals, 3 bridges, 4 end brackets.
    let qty = |desc: &str, part: &str| {
        bom[1..]
            .iter()
            .find(|r| r[5] == desc && r[4] == part)
            .map(|r| r[1].clone())
    };
    assert_eq!(
        qty("Feed-through terminal, 3 connections", "PT 2.5-TWIN"),
        Some("11".to_owned())
    );
    assert_eq!(
        qty("PE terminal, 3 connections", "PT 2.5-TWIN-PE"),
        Some("4".to_owned())
    );
    assert_eq!(qty("Plug-in bridge", "TBD"), Some("3".to_owned()));
    assert_eq!(qty("End bracket", "TBD"), Some("4".to_owned()));
    // The marker texts of WP-S2.04 §13.6: `2.5 BK BU` twice on 101, the two groups of 102.
    let texts: Vec<(&str, &str)> = counts
        .marker
        .iter()
        .map(|m| (m.folio.as_str(), m.text.as_str()))
        .collect();
    assert_eq!(
        texts,
        [
            ("101", "2.5 BK BU"),
            ("101", "2.5 BK BU"),
            ("102", "1.5 DBU DBU WH WH"),
            ("102", "1.5 DBU DBU"),
        ]
    );
}

/// `check-task` reports each kind of problem of a copy of the committed task, by file.
#[test]
fn check_task_names_what_is_wrong() {
    let tmp = Tmp::new("check-task");
    let root = tmp.0.join("repo");
    std::fs::create_dir_all(root.join("docs/design")).unwrap();
    std::fs::copy(crate::workspace_root().join(DOC20), root.join(DOC20)).unwrap();
    let task = root.join("tools/blind/tasks/gpp-101-103");
    copy_dir(&task_dir(&crate::workspace_root(), "gpp-101-103"), &task).unwrap();
    assert_eq!(check_task(&root, "gpp-101-103").unwrap(), Vec::<String>::new());
    let edit = |rel: &str, f: &dyn Fn(String) -> String| {
        let p = task.join(rel);
        let text = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, f(text)).unwrap();
    };
    let problems = |_: ()| check_task(&root, "gpp-101-103").unwrap().join("\n");
    // The sheet: an identifier doc 20 does not hold, a name it does not hold, an address.
    edit("task.md", &|t| {
        format!("{t}\nDraw -X9999 for Zoltan at https://example.invalid and mail a@b.\n")
    });
    let p = problems(());
    assert!(p.contains("`-X9999` is not in doc 20"), "{p}");
    assert!(p.contains("`Zoltan` is a name that doc 20 does not hold"), "{p}");
    assert!(p.contains("(an address or a path)"), "{p}");
    edit("task.md", &|t| {
        t.replace(
            "\nDraw -X9999 for Zoltan at https://example.invalid and mail a@b.\n",
            "",
        )
    });
    // An unclosed folio block and a block for a folio the oracle has no counts for.
    edit("task.md", &|t| {
        t.replace("<!-- /folio -->", "")
            .replacen("<!-- folio 102 -->", "<!-- folio 109 -->", 1)
    });
    let p = problems(());
    assert!(p.contains("task.md:"), "{p}");
    edit("task.md", &|_| {
        std::fs::read_to_string(crate::workspace_root().join("tools/blind/tasks/gpp-101-103/task.md"))
            .unwrap()
    });
    // The prelude: an entry too many, a wrong action, a drawing line without its follow-up, an unknown class.
    edit("prelude.toml", &|t| {
        format!("{t}\n[[step]]\naction = \"file.save\"\nclass = \"setup\"\n")
    });
    assert!(problems(()).contains("prelude.toml has 6 entries for 5 prelude steps"));
    edit("prelude.toml", &|t| {
        t.replace("\n[[step]]\naction = \"file.save\"\nclass = \"setup\"\n", "")
            .replacen("action = \"wizard.cancel\"", "action = \"wizard.close\"", 1)
    });
    assert!(problems(()).contains("entry 3: action `wizard.close`, the step is `wizard.cancel`"));
    edit("prelude.toml", &|t| {
        t.replacen(
            "action = \"wizard.close\"\nclass = \"setup\"",
            "action = \"wizard.cancel\"\nclass = \"drawing\"",
            1,
        )
    });
    assert!(problems(()).contains("entry 3: a drawing line names its follow-up"));
    edit("prelude.toml", &|t| {
        t.replacen("class = \"drawing\"", "class = \"decor\"", 1)
    });
    assert!(problems(()).contains("class `decor` is setup or drawing"));
    edit("prelude.toml", &|t| {
        t.replacen("class = \"decor\"", "class = \"drawing\"\ngap = \"FT-408\"", 1)
    });
    assert_eq!(problems(()), "");
    // The gaps: an unknown check, a duplicate, a follow-up that is neither FT nor WP, an empty reason.
    edit("gaps.toml", &|t| {
        format!(
            "{t}\n[[gap]]\ncheck = \"H9\"\ngap = \"FT-1\"\nreason = \"x\"\n\n[[gap]]\ncheck = \"H4\"\ngap = \"later\"\nreason = \" \"\n"
        )
    });
    let p = problems(());
    assert!(p.contains("`H9` is not a check of this task"), "{p}");
    assert!(p.contains("`H4` is listed twice"), "{p}");
    assert!(p.contains("`later` is neither a follow-up"), "{p}");
    assert!(p.contains("`H4` has no reason"), "{p}");
    // A missing file and a wrong header.
    std::fs::remove_file(task.join("oracle/rows/ends.csv")).unwrap();
    std::fs::write(task.join("oracle/rows/pages.csv"), "folio,name\n101,x\n").unwrap();
    std::fs::write(task.join("oracle/bom.csv"), "a,b\n").unwrap();
    let p = problems(());
    assert!(p.contains("oracle/rows/ends.csv is missing"), "{p}");
    assert!(
        p.contains("oracle/rows/pages.csv: the header is not folio,title"),
        "{p}"
    );
    assert!(p.contains("oracle/bom.csv: the header"), "{p}");
}

/// The sheet's words: identifiers and mid-sentence names must be doc 20's; sentence starts, cells' first words, the
/// placeholders and comments are free.
#[test]
fn the_sheets_words_must_be_doc_20s() {
    let doc =
        "The tag S51 and -101F1 stand at 101-B2. Festo and Eaton make parts. Folio 101 has PE and BK wires.";
    // Fine: identifiers in doc, names in doc, sentence starts, placeholders, comments, table first cells.
    let ok = "# Job sheet\n\nDraw S51 at 101-B2 with Festo parts. Zoltan starts a sentence? No: this is fine.\n\
              Run `{run}` has {turns} turns. <!-- Mallory is a comment -->\n| Tag | Part |\n|---|---|\n| S51 | Eaton |\n";
    assert_eq!(sheet_problems(ok, doc), Vec::<String>::new());
    // An identifier that doc 20 lacks, in the three shapes: letter and digit, sign-prefixed, capitals; and a 3-digit folio.
    for (text, want) in [
        ("Place S52.", "`S52` is not in doc 20"),
        ("Place -X1.", "`-X1` is not in doc 20"),
        ("Colour GNYE here.", "`GNYE` is not in doc 20"),
        ("Folio 999 here.", "`999` is not in doc 20"),
        (
            "Ask Zoltan today.",
            "`Zoltan` is a name that doc 20 does not hold",
        ),
        ("Mail me@example.org.", "(an address or a path)"),
        ("See C:\\x.", "(an address or a path)"),
    ] {
        let p = sheet_problems(text, doc);
        assert!(p.iter().any(|m| m.contains(want)), "{text}: {p:?}");
    }
    // Small numbers, units and lower-case words are not identifiers.
    assert_eq!(
        sheet_problems("Use 2.5 mm and 17 wires, then 0 to 16.", doc),
        Vec::<String>::new()
    );
    assert!(
        identifier_shaped("101-B2")
            && identifier_shaped("-X1031")
            && identifier_shaped("24VDC-1")
            && identifier_shaped("BK")
    );
    assert!(!identifier_shaped("wire") && !identifier_shaped("17") && !identifier_shaped("Draw"));
    assert!(capitalised("Festo") && !capitalised("FESTO") && !capitalised("S51") && !capitalised("a"));
    assert!(valid_gap("FT-408") && valid_gap("WP-S2.07") && valid_gap("WP-S2.04 part 4b"));
    assert!(!valid_gap("FT-") && !valid_gap("later") && !valid_gap("FT-4a"));
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
