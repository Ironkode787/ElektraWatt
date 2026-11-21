// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part of the project with an enclosure section is offered for an enclosure whatever its kind, as the command
//! that makes the enclosure takes it (FT-1865: a parts import that named no kind gave `device`, and Enclosure…
//! refused the part as not on offer).

use crate::common;
use ew_schema::{LibDocument, PartKind};

#[test]
fn a_part_with_an_enclosure_section_and_no_enclosure_kind_is_offered() {
    let (project, records, mut lib) = common::sample_records();
    let mut changed = 0;
    for doc in &mut lib {
        if let LibDocument::Part(copy) = doc
            && copy.item.sections.enclosure.is_some()
        {
            copy.item.kind = PartKind::Device;
            changed += 1;
        }
    }
    assert!(changed > 0, "the sample holds an enclosure part");
    let s = common::session_of(project, records, lib);
    let offers = s.layout_part_offers("enclosure");
    assert!(
        offers
            .iter()
            .any(|o| o.copies.is_empty() && o.sections.enclosure.is_some()),
        "the project's part is offered: {:?}",
        offers.iter().map(|o| &o.number).collect::<Vec<_>>()
    );
}
