// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A member added after a format's release is appended after the released members, `source_ref` included (ADR-0011
//! amendment 4): an older build keeps it in `extra` and writes it after them, so the line it writes is this build's
//! canonical line too.

use ew_format::record_line;
use ew_schema::Record;

/// An imported list block with the members 0.3.5 added: its line holds them after `source_ref`, in the order an older
/// build writes them from `extra`.
#[test]
fn a_report_blocks_new_members_follow_its_source_ref() {
    let line = concat!(
        r#"{"$k":"report_block","id":"3c5e7a90-1b2d-5f46-8a0c-e2d4f6b8a1c3","source":{"kind":"parts_summary"},"#,
        r#""form":"7d9f1b3e-5a7c-5e02-9b4d-f6a8c0e2b4d6","first_folio":907,"#,
        r#""source_ref":{"lineage":"4e6a8c0e-2b4d-5f68-8a1c-3e5f7a9b1d2f","key":"lists|907"},"#,
        r#""caption":{"en":"Above"},"legend":{"en":"Below"},"added_rows":[{"cells":{"tag":{"en":"-X9"}}}],"#,
        r#""layout_page":"9b1d3f5a-7c9e-5b24-8d6f-0a2c4e6a8c0e"}"#,
    );
    let record: Record = serde_json::from_str(line).expect("a report block");
    assert_eq!(record_line(&record).expect("its line"), line);
}
