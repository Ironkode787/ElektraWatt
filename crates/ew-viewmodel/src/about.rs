// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The complete About dialog (WP-S2.01 part 2, FT-311; FR-UI-038, NFR-PRIV-07, NFR-LIC-08, NFR-COMP-04): the text of
//! the dialog and of the third-party notices, shared by the egui frame and `ui.describe`.
//!
//! It shows the version, the render backend, the privacy state (update check and crash reports), the licence, the
//! repository, the engineering-verification disclaimer and the support status computed offline from the planned
//! support end embedded at build time. The notices are the repository's `NOTICE` (the bundled fonts) and, when the
//! installation has it, the generated `THIRD-PARTY-NOTICES.txt` of the Rust crates (cargo-about, per release).
//!
//! `PRIVACY.md` (FT-342) is linked by a button of the dialog and of the first-run privacy step ([`PRIVACY_URL`]).
//!
//! Not here yet: the dismissible banner after the support end (at most once a month), which needs a clock and a
//! setting.

use crate::shell::ShellModel;
use crate::strings;

/// The planned end of this release line's support as `YYYY-MM-DD`, embedded at build time (doc 18 section 7.5: the
/// successor's planned date plus six months). A pre-1.0 build has none: only the latest 0.x is supported, and the
/// release process (WP-S6.07) fills this in from the published cadence.
pub const SUPPORT_END: Option<&str> = None;

/// Where `PRIVACY.md` is read: the file on the repository's default branch, opened in the system browser by the
/// user (a click; the application sends nothing). The installed builds of the release phase bundle the file next
/// to the user guide and open that copy instead.
pub const PRIVACY_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/blob/main/PRIVACY.md");

/// The repository's third-party notices of committed material (the bundled fonts).
pub const NOTICE: &str = include_str!("../../../NOTICE");

/// The support line of the About dialog for a planned end and today's date (both `YYYY-MM-DD`), computed without
/// any network access: no end is "pre-release"; an end in the past is "no longer receives security fixes".
/// Without today's date (a run without a clock) an end is shown as the planned date.
pub fn support_line(end: Option<&str>, today: Option<&str>) -> String {
    match (end, today) {
        (None, _) => strings::text("about-support-prerelease").to_owned(),
        (Some(end), Some(today)) if today > end => strings::format("about-support-ended", &[end]),
        (Some(end), _) => strings::format("about-support-until", &[end]),
    }
}

/// The lines of the About dialog, in order (the title is the dialog's).
pub fn about_lines(model: &ShellModel) -> Vec<String> {
    let privacy = if model.settings().update_check {
        strings::text("about-privacy-on")
    } else {
        strings::text("about-privacy")
    };
    vec![
        strings::format("about-version", &[&model.config().app_version]),
        strings::text("about-page-view").to_owned(),
        privacy.to_owned(),
        strings::text("about-privacy-file").to_owned(),
        strings::text("about-licence").to_owned(),
        strings::format("about-repository", &[env!("CARGO_PKG_REPOSITORY")]),
        strings::text("about-disclaimer").to_owned(),
        support_line(SUPPORT_END, None),
    ]
}

/// The text of the third-party notices: the repository's `NOTICE`, then the generated notices of the crates when
/// the installation has them, else the sentence that says where they come from.
pub fn notices_text(model: &ShellModel) -> String {
    let generated = model
        .config()
        .resources
        .notices_file
        .as_deref()
        .and_then(|p| std::fs::read_to_string(p).ok());
    match generated {
        Some(text) => format!("{NOTICE}\n{text}"),
        None => format!("{NOTICE}\n{}\n", strings::text("notices-generated-missing")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_support_status_is_computed_offline_from_the_planned_end() {
        assert_eq!(
            support_line(None, Some("2030-01-01")),
            "Support: pre-release build; only the latest 0.x release is supported."
        );
        assert_eq!(
            support_line(Some("2029-03-19"), Some("2028-12-31")),
            "Support: security and data-loss fixes until 2029-03-19."
        );
        assert_eq!(
            support_line(Some("2029-03-19"), Some("2029-03-19")),
            "Support: security and data-loss fixes until 2029-03-19."
        );
        assert_eq!(
            support_line(Some("2029-03-19"), Some("2029-03-20")),
            "Support: this release line no longer receives security fixes (ended 2029-03-19)."
        );
        // No clock: the planned date is all that is known.
        assert_eq!(
            support_line(Some("2029-03-19"), None),
            "Support: security and data-loss fixes until 2029-03-19."
        );
    }

    #[test]
    fn the_notice_names_the_bundled_fonts() {
        assert!(
            NOTICE.contains("Noto Sans"),
            "the repository NOTICE lists the fonts"
        );
        assert!(NOTICE.contains("OFL-1.1"));
    }
}
