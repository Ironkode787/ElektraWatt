// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part editor's sections and declared rows (doc 07 §5, picture A): a terminal lists its accessories and
//! variants, not a ferrule's or a wire's section; a row's text is checked by its schema type (a new part's size above zero, a
//! part number a part has, no ferrule as an accessory); a draft left as opened saves nothing; a changed cell writes
//! only its member; `part.delete` deletes a part of the project's own and refuses a library's.

use crate::common;

use ew_api::{AnyCommand, PartDraftError, Session};
use ew_ids::Uuid;
use serde_json::json;

fn run(s: &mut Session, cmd: &AnyCommand) -> Result<ew_api::Receipt, ew_api::CmdError> {
    match s.execute(cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(cmd).and_then(|p| s.commit(p)),
        other => other,
    }
}

/// The held copy numbered `number`.
fn copy_of(s: &Session, number: &str) -> Uuid {
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(id, _)| id.as_uuid())
        .unwrap_or_else(|| panic!("the sample holds {number}"))
}

fn field_key(r: Result<(), PartDraftError>) -> &'static str {
    match r {
        Err(PartDraftError::Field(m)) => m.key,
        other => panic!("a field's message: {other:?}"),
    }
}

#[test]
fn a_terminals_sections_rows_and_tables_are_read_checked_and_saved() {
    let mut s = common::sample();
    let copy = copy_of(&s, "EW-GEN-TRM-1");
    let mut d = s.part_draft(copy).expect("the terminal's draft");
    let words: Vec<&str> = d.sections().iter().map(|x| x.word).collect();
    for w in ["identity", "pins", "size", "accessories", "variants"] {
        assert!(words.contains(&w), "{words:?}");
    }
    for w in ["wire", "ferrule", "marker"] {
        assert!(!words.contains(&w), "{words:?}");
    }
    // A ferrule lists its own section and none of a mounted part's.
    let ferrule = copy_of(&s, "EW-GEN-FER-1.5");
    let ferrule = s.part_draft(ferrule).expect("the ferrule's draft");
    let theirs: Vec<&str> = ferrule.sections().iter().map(|x| x.word).collect();
    assert!(theirs.contains(&"ferrule"), "{theirs:?}");
    for w in ["pins", "size", "accessories", "variants", "wire", "marker"] {
        assert!(!theirs.contains(&w), "{theirs:?}");
    }
    // The accessories read as part numbers and words.
    let acc = d.table("accessories").expect("the accessories' table").clone();
    let col = |m: &str| acc.columns.iter().position(|(c, _)| *c == m).expect(m);
    let plate = acc
        .rows
        .iter()
        .position(|r| r[col("part")] == "EW-GEN-EP-1")
        .expect("the end plate row");
    assert_eq!(acc.rows[plate][col("per")], "strip");
    // Nothing changed: nothing to save.
    assert!(matches!(
        s.part_edit_command(&d),
        Err(PartDraftError::Refused(k)) if k == "reason-part-unchanged"
    ));
    // A size of zero, a number no part has, a ferrule as an accessory: each refused in words under its field.
    // A library part's fork keeps its size: the rows are refused; a new part's size of zero is refused in words.
    assert!(matches!(
        s.set_draft_field(&mut d, "mech.width", "9"),
        Err(PartDraftError::Refused(_))
    ));
    let mut new = s.new_part_draft().expect("a new part");
    assert_eq!(
        field_key(s.set_draft_field(&mut new, "mech.width", "0")),
        "error-part-length"
    );
    assert_eq!(
        field_key(s.set_draft_field(&mut d, &format!("accessories[{plate}].part"), "NO-SUCH-PART")),
        "error-part-accessory"
    );
    assert_eq!(
        field_key(s.set_draft_field(&mut d, &format!("accessories[{plate}].part"), "EW-GEN-FER-1.5")),
        "error-part-accessory-kind"
    );
    s.set_draft_field(&mut d, &format!("accessories[{plate}].part"), "EW-GEN-EP-1")
        .expect("the end plate again");
    // Two end plates per strip: the fork carries the changed member, the other members and rows stay.
    s.set_draft_field(&mut d, &format!("accessories[{plate}].qty"), "2")
        .expect("a quantity");
    let held = s
        .query()
        .parts()
        .iter()
        .find(|(id, _)| id.as_uuid() == copy)
        .map(|(_, c)| c.item.clone())
        .unwrap();
    let cmd = s.part_edit_command(&d).expect("the fork");
    run(&mut s, &cmd).expect("saved");
    let q = s.query();
    let fork = q
        .parts()
        .iter()
        .find(|(_, c)| {
            c.item.part_number.as_str() == "EW-GEN-TRM-1"
                && c.library_id.as_uuid() == q.project().id.as_uuid()
        })
        .map(|(_, c)| c.item.clone())
        .expect("the project's own copy");
    assert_eq!(fork.accessories.len(), held.accessories.len());
    assert_eq!(fork.accessories[plate].qty.to_string(), "2");
    assert_eq!(fork.accessories[plate].per, held.accessories[plate].per);
    assert_eq!(fork.variants, held.variants);
    assert_eq!(fork.mech, held.mech);
}

#[test]
fn delete_part_deletes_a_part_of_the_projects_own_and_keeps_a_librarys() {
    let mut s = common::sample();
    let library = copy_of(&s, "EW-GEN-TRM-1");
    let refused = run(
        &mut s,
        &AnyCommand {
            name: "part.delete".into(),
            params: json!({ "copy": library.to_string() }),
        },
    )
    .unwrap_err();
    assert!(refused.to_string().contains("is a library's part"), "{refused}");
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    s.set_draft_field(&mut d, "manufacturer", "Acme")
        .expect("a maker");
    s.set_draft_field(&mut d, "part_number", "OWN-1")
        .expect("a number");
    let made = s.new_part_command(&d).expect("the new part's command");
    run(&mut s, &made.command).expect("the part is made");
    let own = copy_of(&s, "OWN-1");
    run(
        &mut s,
        &AnyCommand {
            name: "part.delete".into(),
            params: json!({ "copy": own.to_string() }),
        },
    )
    .expect("deleted");
    assert!(s.query().parts().iter().all(|(id, _)| id.as_uuid() != own));
}
