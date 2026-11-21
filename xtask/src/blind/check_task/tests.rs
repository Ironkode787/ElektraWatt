// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests of `check-task`: a brief's sheet is checked against the sample's script without a product.

use super::*;
use crate::blind::run::copy_dir;
use crate::blind::testkit::Tmp;

/// `check-task` on a copy of a brief that starts on the sample: the committed brief is sound; an identifier and a name
/// that the sample's script does not hold and an address in its sheet are named, and a missing sheet.
#[test]
fn check_task_names_what_is_wrong() {
    let tmp = Tmp::new("check-task");
    let root = tmp.0.join("repo");
    let script = root.join(SAMPLE_SCRIPT);
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    std::fs::copy(crate::workspace_root().join(SAMPLE_SCRIPT), &script).unwrap();
    let brief = "brief-ut-t1-place";
    let task = root.join("tools/blind/tasks").join(brief);
    copy_dir(&task_dir(&crate::workspace_root(), brief), &task).unwrap();
    assert_eq!(check_task(&root, brief).unwrap(), Vec::<String>::new());
    let sheet = std::fs::read_to_string(task.join("task.md"))
        .unwrap()
        .replace("\r\n", "\n");
    std::fs::write(
        task.join("task.md"),
        sheet.replace(
            "\n## History",
            "\nDraw -X9999 for Zoltan at https://example.invalid and mail a@b.\n\n## History",
        ),
    )
    .unwrap();
    let p = check_task(&root, brief).unwrap().join("\n");
    assert!(p.contains("`-X9999` is not in the sample's script"), "{p}");
    assert!(
        p.contains("`Zoltan` is a name that the sample's script does not hold"),
        "{p}"
    );
    assert!(p.contains("(an address or a path)"), "{p}");
    std::fs::remove_file(task.join("task.md")).unwrap();
    assert_eq!(check_task(&root, brief).unwrap(), ["task.md is missing"]);
}

/// The sheet's words: identifiers and mid-sentence names must be the source's; sentence starts, cells' first words,
/// the placeholders and comments are free.
#[test]
fn the_sheets_words_must_be_the_sources() {
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
}
