// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S6.04 D13 (NFR-COMP-04, SE-3): the engineering-verification disclaimer in one wording. The About dialog's
//! sentence (`about-disclaimer`) is the check report's `DISCLAIMER`.

use ew_viewmodel::ew_api::checks;
use ew_viewmodel::key;
use ew_viewmodel::strings;

/// S3 of the specification, whole.
const S3: &str = "ElektraWatt's checks do not replace engineering verification by a competent person.";

#[test]
fn the_check_report_and_the_about_dialog_say_the_same_sentence() {
    assert_eq!(strings::text(key!("about-disclaimer")), S3);
    assert_eq!(checks::report::DISCLAIMER, S3);
}
