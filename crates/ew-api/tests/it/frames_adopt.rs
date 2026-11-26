// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Use the frames of a template… (doc 09 §3.9, the journey's step 9): a customer's logo put into the template's title
//! block with A picture… comes with the block into a project made before it.

use ew_api::{AnyCommand, Session, StoreOptions};
use serde_json::json;

#[test]
fn the_templates_title_block_brings_the_picture_put_into_it() {
    let tmp = ew_testdir::Tmp::new("frames-adopt-picture");
    let mut template = crate::common::sample();
    let facts = template.frame_facts("101").expect("page 101's frame");
    let block = facts.block.expect("the title block").to_string();
    let item = facts.block_item.clone().expect("its item");
    let logo = ew_api::base64_encode(&crate::common::red_png(8, 4));
    crate::common::run_cmd(
        &mut template,
        "form.edit",
        json!({"copy": block, "form": item, "picture": {"cell": 0, "image": logo},
            "forks": facts.block_forks.iter().map(|u| u.to_string()).collect::<Vec<_>>()}),
    );
    let dir = tmp.join("template.ew");
    template
        .create(&dir, StoreOptions::new("ElektraWatt test"))
        .expect("the template is written");
    // A second picture, put in after the template was written: the save writes it.
    let facts = template.frame_facts("101").expect("page 101's frame");
    crate::common::run_cmd(
        &mut template,
        "form.edit",
        json!({"copy": facts.block.expect("the block").to_string(), "form": facts.block_item.clone().expect("its item"),
            "picture": {"cell": 1, "image": ew_api::base64_encode(&crate::common::red_png(4, 8))}}),
    );
    template.save().expect("the template is saved");

    let params = Session::adopt_frames_params(&dir).expect("the template's frames");
    let mut older = crate::common::sample();
    let cmd = AnyCommand {
        name: "frames.adopt".to_owned(),
        params,
    };
    let preview = older.preview(&cmd).expect("the frames preview");
    older.commit(preview).expect("the frames are taken");
    let q = older.query();
    let pictures: Vec<ew_schema::Sha256> = q
        .forms()
        .iter()
        .flat_map(|(_, f)| f.item.assets().map(|(_, sha)| *sha).collect::<Vec<_>>())
        .collect();
    assert!(pictures.len() >= 2, "the adopted block names both pictures");
    for sha in pictures {
        assert!(
            older.asset(&sha).is_some(),
            "the project holds the picture {sha:?}"
        );
    }
}
