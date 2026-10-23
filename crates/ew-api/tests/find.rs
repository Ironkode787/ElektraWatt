// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Session::find` and `FindResults::replace_command` on Golden A (WP-S2.05 part 2, acceptance check FR2 and the
//! session half of FR3 and FR4): plain, case, whole-word and regular-expression matches with their ranges, the order
//! rule, the folio and selection scopes, the errors, the truncation, the hits that are found but not replaced, and the
//! one `fields.set_many` a replace commits.

use std::path::Path;

use ew_api::output::{FindField, FindQuery, FindResults};
use ew_api::{AnyCommand, Session};
use serde_json::{Value, json};

/// Golden A's fixture, read-only (no lock; commands run in memory).
fn golden() -> Session {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    Session::open_read_only(&dir, "ElektraWatt test")
        .expect("the golden fixture opens")
        .0
}

fn query(text: &str) -> FindQuery {
    FindQuery {
        text: text.into(),
        ..FindQuery::default()
    }
}

fn spare(folios: &[&str]) -> FindQuery {
    FindQuery {
        fields: vec![FindField::FunctionText],
        folios: folios.iter().map(|f| (*f).to_owned()).collect(),
        ..query("Spare")
    }
}

/// Commits `fields.set_many` with `params` through preview and commit (a bulk command may not `execute`).
fn commit(s: &mut Session, params: Value) {
    let cmd = AnyCommand {
        name: "fields.set_many".into(),
        params,
    };
    let preview = s.preview(&cmd).expect("the replace previews");
    s.commit(preview).expect("the replace commits");
}

fn subjects(r: &FindResults) -> Vec<String> {
    r.hits.iter().map(|h| h.subject.clone()).collect()
}

/// FR3 (D16): "Spare" in the function texts of 204 to 206 is 20 devices, of 204 and 205 16.
#[test]
fn spare_on_204_to_206_is_twenty_results_and_sixteen_on_204_and_205() {
    let s = golden();
    let r = s.find(&spare(&["204", "205", "206"]));
    assert_eq!(r.error, None);
    assert!(!r.truncated);
    assert_eq!(r.hits.len(), 20, "{:#?}", subjects(&r));
    let two = s.find(&spare(&["204", "205"]));
    assert_eq!(two.hits.len(), 16, "{:#?}", subjects(&two));
    // The order rule: page order, then cell, then subject.
    let folios: Vec<&str> = r.hits.iter().map(|h| h.folio.as_deref().unwrap_or("")).collect();
    let mut sorted = folios.clone();
    sorted.sort();
    assert_eq!(folios, sorted, "page order");
    for h in &r.hits {
        assert_eq!(h.kind, "device");
        assert_eq!(h.row_path, "function_text");
        assert_eq!(h.label_key, "find-field-function-text");
        assert_eq!(h.before, "Spare");
        assert_eq!(h.ranges, [(0, 5)]);
        assert_eq!(h.after, None);
        assert_eq!(h.replaceable, Ok(()));
        let cell = h.cell.as_deref().expect("a placed device has a cell");
        assert!(
            cell.starts_with(&format!("{}-", h.folio.as_deref().unwrap())),
            "{cell}"
        );
    }
    for want in ["-201U5-Q1", "-201U8-Q4", "-202U2-Q1"] {
        assert!(
            subjects(&r).iter().any(|s| s == want),
            "{want}: {:?}",
            subjects(&r)
        );
    }
    // Twice the same bytes.
    assert_eq!(s.find(&spare(&["204", "205", "206"])), r);
}

#[test]
fn case_whole_word_and_regular_expressions() {
    let s = golden();
    let lower = s.find(&FindQuery {
        match_case: true,
        ..spare(&["204", "205", "206"])
    });
    assert_eq!(lower.hits.len(), 20, "the stored text is capitalised");
    let none = s.find(&FindQuery {
        text: "spare".into(),
        match_case: true,
        ..spare(&["204", "205", "206"])
    });
    assert!(none.hits.is_empty());
    let part = s.find(&FindQuery {
        text: "par".into(),
        ..spare(&["204"])
    });
    assert!(!part.hits.is_empty());
    assert!(part.hits.iter().all(|h| h.ranges == [(1, 4)]));
    let word = s.find(&FindQuery {
        text: "par".into(),
        whole_word: true,
        ..spare(&["204"])
    });
    assert!(word.hits.is_empty(), "`par` is inside a word");
    let re = s.find(&FindQuery {
        text: "^S(p)are$".into(),
        regex: true,
        replace: Some("$1".into()),
        ..spare(&["204", "205", "206"])
    });
    assert_eq!(re.hits.len(), 20);
    assert!(re.hits.iter().all(|h| h.after.as_deref() == Some("p")));
}

#[test]
fn errors_and_empty_queries() {
    let s = golden();
    assert_eq!(s.find(&query("")), FindResults::default());
    let long = s.find(&FindQuery {
        text: "a".repeat(1_001),
        regex: true,
        ..FindQuery::default()
    });
    assert!(long.error.as_deref().unwrap().contains("1000"), "{long:?}");
    assert!(long.hits.is_empty());
    let bad = s.find(&FindQuery {
        text: "(Spare".into(),
        regex: true,
        ..FindQuery::default()
    });
    assert!(
        bad.error.as_deref().unwrap().contains("does not compile"),
        "{bad:?}"
    );
    let folio = s.find(&FindQuery {
        folios: vec!["999".into()],
        ..query("Spare")
    });
    assert_eq!(folio.error.as_deref(), Some("no page 999"));
    assert_eq!(folio.replace_command(), None);
}

/// Every string of Golden A matched (an empty match at the start of each): fewer than 10,000, none truncated (the cap
/// itself is the unit test `beyond_the_cap_the_results_are_truncated_and_replace_is_refused` of `output::find`).
#[test]
fn every_string_and_the_cap() {
    let s = golden();
    let all = s.find(&FindQuery {
        text: "^".into(),
        regex: true,
        replace: Some("X".into()),
        ..FindQuery::default()
    });
    assert!(all.error.is_none());
    assert!(all.hits.len() < 10_000 && !all.truncated, "{}", all.hits.len());
    // Records without a page come last.
    let first_without = all.hits.iter().position(|h| h.folio.is_none());
    if let Some(i) = first_without {
        assert!(all.hits[i..].iter().all(|h| h.folio.is_none()));
    }
    // Every field is present.
    for key in [
        "find-field-function-text",
        "find-field-tag",
        "find-field-property",
        "find-field-part-number",
        "find-field-title",
    ] {
        assert!(all.hits.iter().any(|h| h.label_key == key), "{key}");
    }
}

/// D11, D16: a derived tag and a part number are found but not replaced, with their reasons.
#[test]
fn derived_tags_and_part_numbers_are_found_but_not_replaced() {
    let s = golden();
    let mico = s.find(&FindQuery {
        fields: vec![FindField::PartNumber],
        replace: Some("Other".into()),
        ..query("Murrelektronik")
    });
    assert!(!mico.hits.is_empty(), "MICO 4.10 is a Murrelektronik part");
    let h = mico
        .hits
        .iter()
        .find(|h| h.before.starts_with("MICO 4.10"))
        .expect("MICO 4.10");
    assert_eq!(h.before, "MICO 4.10 (Murrelektronik)");
    assert_eq!(h.replaceable, Err("library copy: update from library".into()));
    assert_eq!(h.after, None);
    assert!(h.row_path.starts_with("parts[") && h.row_path.ends_with("].part"));
    assert_eq!(mico.replace_command(), None, "nothing replaceable");
    let tag = s.find(&FindQuery {
        fields: vec![FindField::Tag],
        whole_word: false,
        ..query("101F1")
    });
    let h = tag.hits.iter().find(|h| h.subject == "-101F1").expect("-101F1");
    assert_eq!(h.row_path, "derived.tag");
    assert_eq!(
        h.replaceable,
        Err("derived: change the numbering or freeze the tag".into())
    );
    // A frozen tag (S51, sign hidden) is replaceable in its text.
    let frozen = s.find(&FindQuery {
        fields: vec![FindField::Tag],
        match_case: true,
        replace: Some("S52".into()),
        ..query("S51")
    });
    let h = frozen.hits.iter().find(|h| h.subject == "S51").expect("S51");
    assert_eq!(h.row_path, "tag.frozen.text");
    assert_eq!(h.replaceable, Ok(()));
    assert_eq!(h.after.as_deref(), Some("S52"));
    let cmd = frozen.replace_command().expect("one entry");
    assert_eq!(cmd["edits"][0]["path"], "tag.frozen");
    assert_eq!(cmd["edits"][0]["value"]["text"], "S52");
}

/// The scopes: `within` keeps the records named and the records they reach.
#[test]
fn the_selection_scope_keeps_the_records_reached() {
    let s = golden();
    let all = s.find(&spare(&["204", "205", "206"]));
    let first = all.hits[0].record;
    let within = s.find(&FindQuery {
        within: vec![first],
        ..spare(&[])
    });
    assert_eq!(within.hits.len(), 1);
    assert_eq!(within.hits[0].record, first);
}

/// FR3 and FR4 (the session half): the replace is one `fields.set_many` with one entry per value, it commits once,
/// and undo restores every text.
#[test]
fn a_replace_is_one_set_many_and_one_undo_step() {
    let mut s = golden();
    let r = s.find(&FindQuery {
        replace: Some("Reserve".into()),
        ..spare(&["204", "205", "206"])
    });
    let cmd = r.replace_command().expect("a replace");
    let edits = cmd["edits"].as_array().unwrap();
    assert_eq!(edits.len(), 1, "one value for every device: {cmd}");
    assert_eq!(edits[0]["kind"], "device");
    assert_eq!(edits[0]["path"], "function_text");
    assert_eq!(edits[0]["value"], "Reserve");
    assert_eq!(edits[0]["targets"].as_array().unwrap().len(), 20);
    let targets: Vec<&str> = edits[0]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    let mut sorted = targets.clone();
    sorted.sort_unstable();
    assert_eq!(targets, sorted, "targets ascending");
    let before = s.history().cursor();
    commit(&mut s, cmd);
    assert_eq!(s.history().cursor(), before + 1, "one undo step");
    assert!(s.find(&spare(&["204", "205", "206"])).hits.is_empty());
    assert_eq!(
        s.find(&FindQuery {
            text: "Reserve".into(),
            ..spare(&["204", "205", "206"])
        })
        .hits
        .len(),
        20
    );
    s.undo().unwrap();
    assert_eq!(s.find(&spare(&["204", "205", "206"])).hits.len(), 20);

    // FR4: `(Spare)` → `$1 (unused)`.
    let r = s.find(&FindQuery {
        text: "(Spare)".into(),
        regex: true,
        replace: Some("$1 (unused)".into()),
        ..spare(&["204", "205", "206"])
    });
    commit(&mut s, r.replace_command().unwrap());
    let unused = s.find(&FindQuery {
        text: "Spare (unused)".into(),
        whole_word: true,
        ..spare(&["204", "205", "206"])
    });
    assert_eq!(unused.hits.len(), 20);
    assert!(unused.hits.iter().all(|h| h.before == "Spare (unused)"));
}

/// FR4: a replace in an identifier technical value of a device (added with `fields.set` first) writes the whole list
/// with that item changed; a quantity value is found but not replaced, and a match in the key is refused.
#[test]
fn a_technical_value_is_replaced_in_its_list() {
    let mut s = golden();
    let tag = s.find(&FindQuery {
        fields: vec![FindField::Tag],
        match_case: true,
        whole_word: true,
        ..query("-101F1")
    });
    let device = tag.hits.iter().find(|h| h.subject == "-101F1").unwrap().record;
    let set = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [device.to_string()], "path": "tech_data",
            "value": [{"key": "In", "value": {"value": "16", "unit": "A"}}, {"key": "curve", "value": "C"}]}),
    };
    s.execute(&set).expect("tech data");
    let within = |q: FindQuery| FindQuery {
        within: vec![device],
        fields: vec![FindField::Property],
        ..q
    };
    let r = s.find(&within(FindQuery {
        replace: Some("B".into()),
        match_case: true,
        ..query("C")
    }));
    assert_eq!(r.hits.len(), 1, "{:?}", r.hits);
    assert_eq!(r.hits[0].before, "curve=C");
    assert_eq!(r.hits[0].ranges, [(6, 7)]);
    assert_eq!(r.hits[0].after.as_deref(), Some("curve=B"));
    let cmd = r.replace_command().unwrap();
    assert_eq!(
        cmd["edits"][0]["value"],
        json!([{"key": "In", "value": {"value": "16", "unit": "A"}}, {"key": "curve", "value": "B"}])
    );
    commit(&mut s, cmd);
    let q = s.find(&within(query("16")));
    assert_eq!(
        q.hits[0].replaceable,
        Err("a quantity: edit it in the property grid".into())
    );
    let key = s.find(&within(FindQuery {
        replace: Some("x".into()),
        ..query("curve")
    }));
    assert!(key.hits[0].replaceable.is_err());
    assert_eq!(key.replace_command(), None);
}

/// A device of Golden A with `tech_data` set to `items` (JSON), by its tag.
fn device_with_tech_data(s: &mut Session, tag: &str, items: Value) -> ew_derive::ew_ids::Uuid {
    let found = s.find(&FindQuery {
        fields: vec![FindField::Tag],
        match_case: true,
        whole_word: true,
        ..query(tag)
    });
    let device = found.hits.iter().find(|h| h.subject == tag).unwrap().record;
    let set = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [device.to_string()], "path": "tech_data", "value": items}),
    };
    s.execute(&set).expect("tech data");
    device
}

/// The replaced string is NFC (the replacement is as typed): a combining mark composes with the letter before it.
#[test]
fn the_replacement_is_taken_in_nfc() {
    let s = golden();
    let r = s.find(&FindQuery {
        replace: Some("Cafe\u{301}".into()),
        ..spare(&["204"])
    });
    assert!(!r.hits.is_empty());
    assert!(
        r.hits.iter().all(|h| h.after.as_deref() == Some("Caf\u{e9}")),
        "{:?}",
        r.hits[0]
    );
    let cmd = r.replace_command().unwrap();
    assert_eq!(cmd["edits"][0]["value"], "Caf\u{e9}");
}

/// A replacement that NFC would merge with the unchanged `key=` is not written: `=` and a combining slash are `≠`.
#[test]
fn a_replacement_that_merges_with_the_key_is_refused() {
    let mut s = golden();
    let device = device_with_tech_data(&mut s, "-101F1", json!([{"key": "curve", "value": "C"}]));
    let r = s.find(&FindQuery {
        within: vec![device],
        fields: vec![FindField::Property],
        replace: Some("\u{338}".into()),
        match_case: true,
        ..query("C")
    });
    assert_eq!(r.hits.len(), 1);
    assert!(r.hits[0].replaceable.is_err(), "{:?}", r.hits[0]);
    assert_eq!(r.hits[0].after, None);
    assert_eq!(r.replace_command(), None);
}

/// The refusal for a match that reaches past the stored text names the structure prefix and the `:cp` suffix (a
/// frozen tag shown with its sign: "-K9" holds "K9").
#[test]
fn a_match_in_the_structure_prefix_names_prefix_and_cp() {
    let mut s = golden();
    let found = s.find(&FindQuery {
        fields: vec![FindField::Tag],
        match_case: true,
        whole_word: true,
        ..query("-101F1")
    });
    let device = found.hits.iter().find(|h| h.subject == "-101F1").unwrap().record;
    let freeze = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [device.to_string()], "path": "tag.frozen",
            "value": {"text": "K9", "sign": "shown"}}),
    };
    s.execute(&freeze).expect("a frozen tag");
    let r = s.find(&FindQuery {
        within: vec![device],
        fields: vec![FindField::Tag],
        replace: Some("+".into()),
        ..query("-K")
    });
    assert_eq!(r.hits.len(), 1, "{:?}", r.hits);
    assert_eq!(r.hits[0].before, "-K9");
    assert_eq!(
        r.hits[0].replaceable,
        Err("the match reaches into the structure prefix or the :cp suffix: change the structure".into())
    );
}

/// Row paths sort in natural order (`tech_data[2]` before `tech_data[10]`).
#[test]
fn row_paths_sort_in_natural_order() {
    let mut s = golden();
    let items: Vec<Value> = (0..12)
        .map(|i| json!({"key": ((b'a' + i) as char).to_string(), "value": "x"}))
        .collect();
    let device = device_with_tech_data(&mut s, "-101F1", Value::Array(items));
    let r = s.find(&FindQuery {
        within: vec![device],
        fields: vec![FindField::Property],
        ..query("x")
    });
    let paths: Vec<&str> = r.hits.iter().map(|h| h.row_path.as_str()).collect();
    let expected: Vec<String> = (0..12).map(|i| format!("tech_data[{i}].value")).collect();
    assert_eq!(paths, expected.iter().map(String::as_str).collect::<Vec<_>>());
}

/// `changes` counts the hits whose string a replace changes: a replacement equal to the text changes nothing, and a
/// mixed search counts only the hits that differ.
#[test]
fn changes_count_only_hits_that_differ() {
    let mut s = golden();
    let same = s.find(&FindQuery {
        replace: Some("Spare".into()),
        match_case: true,
        ..spare(&["204", "205", "206"])
    });
    assert_eq!(same.hits.len(), 20);
    assert!(same.hits.iter().all(|h| h.after.is_some() && !h.changes()));
    assert_eq!(same.changes(), 0);
    assert_eq!(same.replace_command(), None);
    // 204's texts become "Reserve"; then the search `^(Spare|Reserve)` finds those and the Spare ones of 205 and 206.
    let to_reserve = s.find(&FindQuery {
        replace: Some("Reserve".into()),
        ..spare(&["204"])
    });
    let n = to_reserve.hits.len();
    commit(&mut s, to_reserve.replace_command().unwrap());
    let mixed = s.find(&FindQuery {
        text: "^(Spare|Reserve)".into(),
        regex: true,
        match_case: true,
        fields: vec![FindField::FunctionText],
        folios: vec!["204".into(), "205".into(), "206".into()],
        replace: Some("Reserve".into()),
        ..FindQuery::default()
    });
    assert!(mixed.hits.len() >= 20, "{}", mixed.hits.len());
    assert_eq!(
        mixed.changes(),
        mixed.hits.len() - n,
        "the hits already Reserve do not change"
    );
}
