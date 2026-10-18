// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-dco <range>`: every non-merge commit in the range carries a Developer Certificate of Origin sign-off,
//! a `Signed-off-by: Name <address>` trailer (NFR-LIC-03, docs/design/19-open-source-governance.md §3.2). The
//! check only reads the history; it never rewrites a commit message.

use std::path::Path;

use anyhow::Result;

use crate::repo::git;

/// One commit of the range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// Full hash.
    pub sha: String,
    /// First line of the message.
    pub subject: String,
    /// The values of its `Signed-off-by` trailers, as git parses the trailer block.
    pub signoffs: Vec<String>,
}

/// The non-merge commits of `range` (any `git log` revision range, e.g. `origin/main..HEAD`) in `repo`.
pub fn commits(repo: &Path, range: &str) -> Result<Vec<Commit>> {
    // Fields are separated by 0x1f, records by 0x1e; trailer values by newlines.
    let format = "--format=%H%x1f%s%x1f%(trailers:key=Signed-off-by,valueonly)%x1e";
    let out = git(repo, &["log", "--no-merges", format, range, "--"])?;
    Ok(out
        .split('\u{1e}')
        .filter_map(|record| {
            let record = record.trim_start_matches(['\n', '\r']);
            let mut fields = record.split('\u{1f}');
            let sha = fields.next()?.trim();
            if sha.is_empty() {
                return None;
            }
            Some(Commit {
                sha: sha.to_owned(),
                subject: fields.next().unwrap_or_default().to_owned(),
                signoffs: fields
                    .next()
                    .unwrap_or_default()
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect(),
            })
        })
        .collect())
}

/// Whether a trailer value has the form `Name <address>`: a non-empty name, then an address in angle brackets
/// containing `@`.
pub fn is_valid_signoff(value: &str) -> bool {
    let Some(rest) = value.strip_suffix('>') else {
        return false;
    };
    let Some((name, address)) = rest.rsplit_once(" <") else {
        return false;
    };
    !name.trim().is_empty()
        && address.contains('@')
        && !address.starts_with('@')
        && !address.ends_with('@')
        && !address.contains(['<', '>', ' '])
}

/// The commits without a valid sign-off.
pub fn check(commits: &[Commit]) -> Vec<String> {
    commits
        .iter()
        .filter(|c| !c.signoffs.iter().any(|s| is_valid_signoff(s)))
        .map(|c| {
            format!(
                "{} {}: no `Signed-off-by: Name <address>` trailer (DCO, NFR-LIC-03); add it with \
                 `git commit --amend -s` or `git rebase --signoff <base>`",
                &c.sha[..c.sha.len().min(12)],
                c.subject
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn signoff_shape() {
        // Addresses are assembled at run time: the repository carries no literal e-mail address.
        let addr = ["dco-test", "example.invalid"].join("@");
        assert!(is_valid_signoff(&format!("Test Signer <{addr}>")));
        assert!(!is_valid_signoff(&format!("<{addr}>")));
        assert!(!is_valid_signoff("Test Signer"));
        assert!(!is_valid_signoff("Test Signer <no-address>"));
    }

    /// A fresh temporary git repository with one signed and one unsigned commit.
    fn temp_repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ew-xtask-dco-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let addr = ["dco-test", "example.invalid"].join("@");
        let name_cfg = "user.name=DCO Test".to_owned();
        let mail_cfg = format!("user.email={addr}");
        let run = |args: &[&str]| {
            let mut full = vec!["-c", &name_cfg, "-c", &mail_cfg, "-c", "commit.gpgsign=false"];
            full.extend_from_slice(args);
            git(&dir, &full).unwrap();
        };
        run(&["init", "--quiet"]);
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        run(&["add", "a.txt"]);
        run(&["commit", "--quiet", "-s", "-m", "feat: signed commit"]);
        std::fs::write(dir.join("b.txt"), "b\n").unwrap();
        run(&["add", "b.txt"]);
        run(&[
            "commit",
            "--quiet",
            "-m",
            "fix: unsigned commit\n\nThe body mentions Signed-off-by: in prose, which is not a trailer.",
        ]);
        dir
    }

    #[test]
    fn unsigned_commit_in_temp_repo_fails() {
        let dir = temp_repo();
        let all = commits(&dir, "HEAD").unwrap();
        assert_eq!(all.len(), 2, "{all:#?}");
        let v = check(&all);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].contains("fix: unsigned commit"), "{v:#?}");
        // The signed commit alone passes.
        assert_eq!(check(&commits(&dir, "HEAD~1").unwrap()), Vec::<String>::new());
        std::fs::remove_dir_all(&dir).ok();
    }
}
