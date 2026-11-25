// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `page.delete` with `delete_devices`: a strip whose terminals all go with the page goes too, rather than staying
//! behind empty.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::commands::DeletePages;
use ew_model::fixture::*;
use ew_schema::{Device, Record};

#[test]
fn a_strip_emptied_by_the_delete_goes_with_its_terminals() {
    // The mini project's terminals of X1031 drawn on page 102, where the strip is drawn too.
    let model = mini_model_with(|records| {
        for (n, f) in [FN_T1, FN_T2, FN_T3].into_iter().enumerate() {
            records.push(Record::Representation(rep(
                id(0x9000 + n as u128),
                PAGE_102,
                vec![f],
            )));
        }
    });
    let mut e = engine(model);
    let cmd = DeletePages {
        pages: vec![PAGE_102],
        delete_devices: true,
    };
    let preview = e.preview(&cmd).unwrap();
    let receipt = e.commit(preview).unwrap();
    for device in [DEV_T1, DEV_T2, DEV_T3, DEV_X1031] {
        assert!(
            e.model().table::<Device>().get(&device).is_none(),
            "{device} is gone"
        );
    }
    assert_eq!(receipt.outcome.note, None, "nothing is kept");
}
