// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The complete About dialog (FR-UI-038, NFR-PRIV-07, NFR-LIC-08, NFR-COMP-04): the text of
//! the dialog and of the third-party notices, shared by the egui frame and `ui.describe`.
//!
//! It shows the version, the render backend, the privacy state (update check and crash reports), the licence, the
//! repository, the engineering-verification disclaimer and the support line (only the latest 0.x release is
//! supported). The notices are the repository's `NOTICE` (the bundled fonts) and, when the
//! installation has it, the generated `THIRD-PARTY-NOTICES.txt` of the Rust crates (cargo-about, per release).
//!
//! `PRIVACY.md` is opened by a button of the dialog and of the first-run privacy step: the bundled copy when
//! the installation has it, else the repository's ([`privacy_target`]).
//!
//! Not here yet: the dismissible banner after the support end (at most once a month), which needs a clock and a
//! setting.

use crate::key;
use crate::shell::ShellModel;
use crate::strings;

/// Where `PRIVACY.md` is read: the file on the repository's default branch, opened in the system browser by the
/// user (a click; the application sends nothing). The installed builds of the release phase bundle the file next
/// to the user guide and open that copy instead.
pub const PRIVACY_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/blob/main/PRIVACY.md");

/// What the privacy buttons open: the bundled `PRIVACY.md` as a `file://` URL when the
/// installation has it ([`crate::shell::Resources::privacy_file`]), offline; else [`PRIVACY_URL`].
pub fn privacy_target(model: &ShellModel) -> String {
    model
        .config()
        .resources
        .privacy_file
        .as_deref()
        .map_or_else(|| PRIVACY_URL.to_owned(), crate::shell::file_url_of)
}

/// The repository's third-party notices of committed material (the bundled fonts).
pub const NOTICE: &str = include_str!("../../../NOTICE");

/// The lines of the About dialog, in order (the title is the dialog's).
pub fn about_lines(model: &ShellModel) -> Vec<String> {
    // This version has no update check, whatever the stored setting says.
    let privacy = strings::text(key!("about-privacy"));
    vec![
        strings::format(key!("about-version"), &[&model.config().app_version]),
        strings::text(key!("about-page-view")).to_owned(),
        privacy.to_owned(),
        strings::text(key!("about-privacy-file")).to_owned(),
        strings::text(key!("about-licence")).to_owned(),
        strings::format(key!("about-repository"), &[env!("CARGO_PKG_REPOSITORY")]),
        strings::text(key!("about-disclaimer")).to_owned(),
        strings::text(key!("about-support-prerelease")).to_owned(),
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
        None => format!("{NOTICE}\n{}\n", strings::text(key!("notices-generated-missing"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_names_the_bundled_fonts() {
        assert!(
            NOTICE.contains("Noto Sans"),
            "the repository NOTICE lists the fonts"
        );
        assert!(NOTICE.contains("OFL-1.1"));
    }
}
