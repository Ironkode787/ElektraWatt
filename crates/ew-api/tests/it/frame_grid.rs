// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The columns and rows of the sample's frame set to 10 numbered from 1 and 6 lettered from A (doc 09 §3.9, the
//! journey's step 5): the preview lists the cross-references that print differently, page by page, before Apply.

use ew_api::AnyCommand;
use serde_json::json;

#[test]
fn ten_columns_and_six_rows_preview_the_cross_references_they_move() {
    let mut s = crate::common::sample();
    let facts = s.frame_facts("101").expect("page 101's frame");
    let cmd = AnyCommand {
        name: "frame.set_grid".to_owned(),
        params: json!({
            "frame": facts.frame.to_string(),
            "columns": {"count": 10, "first": 1},
            "rows": {"count": 6},
            "forks": facts.frame_forks.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
        }),
    };
    let preview = s.preview(&cmd).expect("the change previews");
    let diff = s.derived_diff(&preview);
    let moved: Vec<(String, String)> = diff
        .xrefs
        .iter()
        .map(|c| (c.before.clone(), c.after.clone()))
        .collect();
    for (before, after) in [("103-C1", "103-B1"), ("102-H0, 103-A1", "102-F1, 103-A1")] {
        assert!(
            moved.iter().any(|(b, a)| b == before && a == after),
            "{before} → {after} in {moved:?}"
        );
    }
}

/// The same change lists the wire numbers whose order it changes, old → new, matched by the one matcher of wires of two
/// states; none prints the same number before and after.
#[test]
fn ten_columns_and_six_rows_preview_the_wire_numbers_they_renumber() {
    let mut s = crate::common::sample();
    let facts = s.frame_facts("101").expect("page 101's frame");
    let cmd = AnyCommand {
        name: "frame.set_grid".to_owned(),
        params: json!({
            "frame": facts.frame.to_string(),
            "columns": {"count": 10, "first": 1},
            "rows": {"count": 6},
            "forks": facts.frame_forks.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
        }),
    };
    let preview = s.preview(&cmd).expect("the change previews");
    let diff = s.derived_diff(&preview);
    assert!(
        !diff.wires.is_empty(),
        "a grid of other cells orders some wires anew"
    );
    assert!(diff.wires.iter().all(|w| w.before != w.after), "{:?}", diff.wires);
}

/// A page that prints the frame without the grid because it names no frame and the project keeps no frame defaults (doc
/// 09 §3.8) follows a first change of its title block: after the fork it prints the project's own copies, never the
/// program's left unused.
#[test]
fn a_page_without_frame_defaults_follows_the_fork_of_its_frame() {
    let mut s = crate::common::sample();
    crate::common::run_cmd(
        &mut s,
        "project.settings.set",
        json!({ "member": "frames", "value": {} }),
    );
    let before = s.frame_facts("1").expect("the title page's frame");
    assert!(!before.frame_local && !before.block_local, "the starter forms");
    let mut item = before.block_item.clone().expect("its title block");
    item["cells"][0]["label"] = json!("Changed:");
    // Forks whose copies sort after the program's, so that no order of the copies finds them first by chance.
    let project = s.query().project().id.as_uuid();
    let forks: Vec<String> = (1u128..)
        .map(ew_ids::Uuid::from_u128)
        .filter(|item| ew_ids::v5::lib_copy_id::<()>(project, *item).as_uuid() > before.frame)
        .take(before.block_forks.len())
        .map(|u| u.to_string())
        .collect();
    crate::common::run_cmd(
        &mut s,
        "form.edit",
        json!({"copy": before.block.expect("the block").to_string(), "form": item, "forks": forks}),
    );
    let after = s.frame_facts("1").expect("the title page's frame");
    assert!(after.frame_local, "the page prints the frame's fork");
    assert!(after.block_local, "and the block's fork");
    assert!(
        after.cells[0].printed.starts_with("Changed:"),
        "{}",
        after.cells[0].printed
    );
}

/// What every order of the project reads off one candidate: each potential's name, the interruption points' pairing
/// (each chain in its order) and each device's printed tag.
fn orders(q: &ew_derive::Q) -> (Vec<String>, ew_derive::IpPairing, Vec<Option<ew_derive::Tag>>) {
    let partition = q.get::<ew_derive::PotentialPartition>(());
    let mut names: Vec<String> = partition
        .potentials()
        .map(|p| q.get::<ew_derive::PotentialNameOf>(p.id).name.text.clone())
        .collect();
    names.sort();
    let pairing = q.get::<ew_derive::IpPairingQ>(()).as_ref().clone();
    let tags = q
        .devices()
        .iter()
        .map(|(id, _)| q.get::<ew_derive::DeviceTag>(id).as_ref().clone())
        .collect();
    (names, pairing, tags)
}

/// The sample's frame with its rulers hidden (doc 06 §5.1, the owner's ruling): the cross-references into its pages
/// print the page alone, and nothing else changes: no wire number, no potential's name, no chain of interruption
/// points' order, no device's tag. Where a thing is on the page orders it whether the rulers print or not.
#[test]
fn hiding_the_rulers_changes_the_cross_references_alone() {
    let mut s = crate::common::sample();
    let facts = s.frame_facts("101").expect("page 101's frame");
    let cmd = AnyCommand {
        name: "frame.set_grid".to_owned(),
        params: json!({
            "frame": facts.frame.to_string(),
            "drawn": false,
            "forks": facts.frame_forks.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
        }),
    };
    let preview = s.preview(&cmd).expect("the change previews");
    let diff = s.derived_diff(&preview);
    assert!(diff.wires.is_empty(), "no wire renumbered: {:?}", diff.wires);
    assert!(diff.tags.is_empty(), "no tag changed: {:?}", diff.tags);
    assert!(
        diff.xrefs
            .iter()
            .any(|c| c.before == "103-C1" && c.after == "103"),
        "{:?}",
        diff.xrefs
    );
    let before = orders(&s.query());
    let after = orders(&s.query_over(preview.candidate()));
    assert_eq!(before.0, after.0, "the potentials' names");
    assert!(before.1 == after.1, "the interruption points' chains");
    assert_eq!(before.2, after.2, "the devices' tags");
}
