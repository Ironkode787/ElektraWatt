// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-governance-files`: the governance files of NFR-DOC-08 and docs/design/19-open-source-governance.md
//! §2.4 exist and are not empty.
//!
//! This is the M0 set of WP-S0.03. From the first binary release (M2) the check also requires `GOVERNANCE.md`,
//! `PRIVACY.md`, `TRADEMARKS.md`, `ROADMAP.md` and `.github/FUNDING.yml`; add them to [`REQUIRED_FILES`] in the
//! work package that writes them.

use std::path::Path;

/// Files that must exist and be non-empty.
pub const REQUIRED_FILES: &[&str] = &[
    "LICENSE.md",
    "CONTRIBUTING.md",
    "CODE_OF_CONDUCT.md",
    "SECURITY.md",
    ".github/CODEOWNERS",
    ".github/PULL_REQUEST_TEMPLATE.md",
];

/// Directories that must hold at least one non-empty template.
pub const REQUIRED_TEMPLATE_DIRS: &[&str] = &[".github/ISSUE_TEMPLATE"];

/// Checks the files under `root`.
pub fn check(root: &Path) -> Vec<String> {
    let mut v = Vec::new();
    for rel in REQUIRED_FILES {
        match std::fs::metadata(root.join(rel)) {
            Ok(m) if m.is_file() && m.len() > 0 => {}
            Ok(m) if m.is_file() => v.push(format!("{rel}: empty (NFR-DOC-08)")),
            _ => v.push(format!("{rel}: missing (NFR-DOC-08)")),
        }
    }
    for rel in REQUIRED_TEMPLATE_DIRS {
        let has_template = std::fs::read_dir(root.join(rel))
            .map(|entries| {
                entries.flatten().any(|e| {
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    let is_template = [".yml", ".yaml", ".md"].iter().any(|x| name.ends_with(x))
                        && name != "config.yml"
                        && name != "config.yaml";
                    is_template && e.metadata().is_ok_and(|m| m.is_file() && m.len() > 0)
                })
            })
            .unwrap_or(false);
        if !has_template {
            v.push(format!(
                "{rel}/: no issue template (a non-empty .yml, .yaml or .md other than config.yml)"
            ));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_empty_files_fail() {
        let dir = std::env::temp_dir().join(format!("ew-xtask-gov-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".github/ISSUE_TEMPLATE")).unwrap();
        for rel in REQUIRED_FILES {
            std::fs::write(dir.join(rel), "x\n").unwrap();
        }
        std::fs::write(dir.join(".github/ISSUE_TEMPLATE/config.yml"), "x\n").unwrap();
        // Only config.yml: no template yet.
        let v = check(&dir);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].starts_with(".github/ISSUE_TEMPLATE/: no issue template"));

        std::fs::write(dir.join(".github/ISSUE_TEMPLATE/bug.yml"), "x\n").unwrap();
        std::fs::write(dir.join("SECURITY.md"), "").unwrap();
        std::fs::remove_file(dir.join("LICENSE.md")).unwrap();
        let v = check(&dir);
        assert_eq!(
            v,
            [
                "LICENSE.md: missing (NFR-DOC-08)",
                "SECURITY.md: empty (NFR-DOC-08)"
            ]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn real_repository_passes() {
        assert_eq!(check(&crate::workspace_root()), Vec::<String>::new());
    }
}
