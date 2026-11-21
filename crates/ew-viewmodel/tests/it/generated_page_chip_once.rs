// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1761: a report page's Properties showed a "generated" chip on each of its rows. Where every value row shares
//! one source that is not authored, the header says it once and the rows show none; a drawn page's rows keep theirs.

use crate::common;

use common::Tmp;
use ew_viewmodel::ew_api::output::Provenance;
use ew_viewmodel::properties::shared_provenance;
use ew_viewmodel::shell::Mode;

#[test]
fn a_report_pages_rows_share_one_generated_chip() {
    let tmp = Tmp::new("generated-chip");
    // The sample's 901 is a generated parts list; its 101 a drawn page.
    let s = tmp.sample_in(Mode::Headless, false, "901");
    let sheet = s.properties().sheet().expect("the page's sheet");
    assert!(
        matches!(shared_provenance(sheet), Some(Provenance::Generated { .. })),
        "{:?}",
        shared_provenance(sheet)
    );
    let s = tmp.sample_in(Mode::Headless, false, "101");
    let sheet = s.properties().sheet().expect("the page's sheet");
    assert_eq!(shared_provenance(sheet), None);
}
