// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests of `check-task`: the committed task is checked against doc 20 without a product.

use super::*;
use crate::blind::run::copy_dir;
use crate::blind::testkit::Tmp;

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
    assert_eq!(sheet_problems(ok, doc, "doc 20"), Vec::<String>::new());
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
        let p = sheet_problems(text, doc, "doc 20");
        assert!(p.iter().any(|m| m.contains(want)), "{text}: {p:?}");
    }
    // Small numbers, units and lower-case words are not identifiers.
    assert_eq!(
        sheet_problems("Use 2.5 mm and 17 wires, then 0 to 16.", doc, "doc 20"),
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
