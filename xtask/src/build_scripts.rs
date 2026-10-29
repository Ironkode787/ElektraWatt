// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-build-scripts`: every package in the dependency graph (workspace or external) that has a
//! `custom-build` target (a `build.rs`) must be listed in `ci/build-scripts.allowlist` at the version in the
//! graph (ADR-0023 decision 1, NFR-SEC-10). The graph is the all-features metadata, so a build script behind a
//! non-default feature is seen too.
//!
//! The allowlist holds one `name@version` per line, optionally followed by a `#` comment; the version is the
//! exact string cargo reports (build metadata included). A new version of a listed crate therefore fails the
//! check until its `build.rs` is read again and `--bless` re-versions the line. `--bless` never adds a crate: a
//! new build-script crate needs a reviewed line written by hand.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use cargo_metadata::semver::Version;

/// One allowlist entry or graph package: `(name, version)`.
pub type Entry = (String, String);

/// Reads the allowlist file (check mode: every entry line must be `name@version`).
pub fn load_allowlist(path: &Path) -> Result<BTreeSet<Entry>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_allowlist(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Splits an entry line into its specification and the trailing comment (from the `#`, or empty).
fn split_comment(line: &str) -> (&str, &str) {
    match line.find('#') {
        Some(at) => (line[..at].trim(), &line[at..]),
        None => (line.trim(), ""),
    }
}

/// True for a blank line or a line holding only a comment.
fn is_plain(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// Parses allowlist text: one `name@version` per line, `#` comments, blank lines ignored. A line without `@`,
/// with an empty name or version, with a version that is not semver, or a duplicate entry is an error that
/// names the line number.
pub fn parse_allowlist(text: &str) -> Result<BTreeSet<Entry>> {
    let mut entries = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        if is_plain(line) {
            continue;
        }
        let number = index + 1;
        let (spec, _) = split_comment(line);
        let Some((name, version)) = spec.split_once('@') else {
            bail!("line {number}: `{spec}` is not `name@version`");
        };
        let (name, version) = (name.trim(), version.trim());
        if name.is_empty() {
            bail!("line {number}: `{spec}` has an empty crate name");
        }
        if version.is_empty() {
            bail!("line {number}: `{spec}` has an empty version");
        }
        if Version::parse(version).is_err() {
            bail!("line {number}: `{version}` of `{name}` is not a semantic version");
        }
        if !entries.insert((name.to_owned(), version.to_owned())) {
            bail!("line {number}: duplicate entry `{name}@{version}`");
        }
    }
    Ok(entries)
}

/// `(name, version)` of every package in the metadata (all platforms) that has a build script.
pub fn packages_with_build_script(metadata: &cargo_metadata::Metadata) -> BTreeSet<Entry> {
    metadata
        .packages
        .iter()
        .filter(|p| {
            p.targets
                .iter()
                .any(|t| t.kind.contains(&cargo_metadata::TargetKind::CustomBuild))
        })
        .map(|p| (p.name.to_string(), p.version.to_string()))
        .collect()
}

/// Returns one violation per package with a build script that is not allowlisted at its version, and one per
/// allowlist entry that no package in the graph matches.
pub fn check(allowlist: &BTreeSet<Entry>, with_build_script: &BTreeSet<Entry>) -> Vec<String> {
    let mut violations = Vec::new();
    for (name, version) in with_build_script.difference(allowlist) {
        let listed: Vec<&str> = allowlist
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .collect();
        let listed = if listed.is_empty() {
            "not listed".to_owned()
        } else {
            format!("listed: {}", listed.join(", "))
        };
        violations.push(format!(
            "package `{name}@{version}` has a build script but is not in ci/build-scripts.allowlist at this version \
             ({listed}); read its new build.rs (no network, no download, writes only under OUT_DIR), then run \
             `cargo xtask check-build-scripts --bless`"
        ));
    }
    for (name, version) in allowlist.difference(with_build_script) {
        violations.push(format!(
            "stale entry `{name}@{version}` in ci/build-scripts.allowlist: no package in the graph has this version"
        ));
    }
    violations
}

/// What [`bless`] changed; printed so that the reviewer reads the re-versioned build scripts.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BlessReport {
    /// `re-versioned NAME: OLD -> NEW` lines: the build scripts to read before committing.
    pub reversioned: Vec<String>,
    /// `removed NAME@OLD` lines: entries with no package in the graph.
    pub removed: Vec<String>,
    /// `unlisted NAME@VERSION (...)` lines: packages that need a reviewed line written by hand.
    pub unlisted: Vec<String>,
}

impl BlessReport {
    /// The report as printed lines.
    pub fn lines(&self) -> impl Iterator<Item = &String> {
        self.reversioned.iter().chain(&self.removed).chain(&self.unlisted)
    }
}

/// Orders versions ascending by semver (the string for a version that does not parse).
fn semver_order(a: &str, b: &str) -> std::cmp::Ordering {
    match (Version::parse(a), Version::parse(b)) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        _ => a.cmp(b),
    }
}

/// Rewrites the allowlist text from the graph: an entry whose version is gone is re-versioned in place (same
/// comment, one line per new version), a legacy name-only line is migrated, an entry whose crate has no build
/// script any more is dropped. A package whose name is on no line is not added. Idempotent.
pub fn bless(text: &str, found: &BTreeSet<Entry>) -> (String, BlessReport) {
    let mut by_name: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, version) in found {
        by_name.entry(name).or_default().push(version);
    }
    for versions in by_name.values_mut() {
        versions.sort_by(|a, b| semver_order(a, b));
    }

    // Versions already carried by a line (and so not available to re-version another one).
    let mut taken: BTreeSet<Entry> = BTreeSet::new();
    let mut names_on_lines: BTreeSet<&str> = BTreeSet::new();
    for line in text.lines().filter(|l| !is_plain(l)) {
        let (spec, _) = split_comment(line);
        let (name, version) = spec.split_once('@').unwrap_or((spec, ""));
        names_on_lines.insert(name.trim());
        if !version.trim().is_empty() {
            taken.insert((name.trim().to_owned(), version.trim().to_owned()));
        }
    }

    let mut report = BlessReport::default();
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if is_plain(line) {
            out.push(line.to_owned());
            continue;
        }
        let (spec, comment) = split_comment(line);
        let (name, version) = match spec.split_once('@') {
            Some((name, version)) => (name.trim(), Some(version.trim())),
            None => (spec, None),
        };
        let listed_at = version.map(|v| (name.to_owned(), v.to_owned()));
        if listed_at.as_ref().is_some_and(|entry| found.contains(entry)) {
            out.push(line.to_owned());
            continue;
        }
        let old = version.map_or_else(|| name.to_owned(), |v| format!("{name}@{v}"));
        // Versions of this crate in the graph that no line carries yet; the first stale line claims them.
        let fresh: Vec<&str> = by_name
            .get(name)
            .map(|versions| {
                versions
                    .iter()
                    .copied()
                    .filter(|v| !taken.contains(&(name.to_owned(), (*v).to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        if fresh.is_empty() {
            report.removed.push(format!("removed {old}"));
            continue;
        }
        for v in &fresh {
            taken.insert((name.to_owned(), (*v).to_owned()));
            let mut new_line = format!("{name}@{v}");
            if !comment.is_empty() {
                new_line.push(' ');
                new_line.push_str(comment);
            }
            out.push(new_line);
        }
        report.reversioned.push(format!(
            "re-versioned {name}: {} -> {}",
            version.unwrap_or("(no version)"),
            fresh.join(", ")
        ));
    }
    for (name, version) in found {
        if !names_on_lines.contains(name.as_str()) {
            report.unlisted.push(format!(
                "unlisted {name}@{version} (needs a reviewed line written by hand)"
            ));
        }
    }
    let mut blessed = out.join("\n");
    blessed.push('\n');
    (blessed, report)
}

/// `check-build-scripts --bless`: rewrites the allowlist from the graph, prints what changed, then runs the
/// normal check. Returns whether the check passed.
pub fn run_bless(root: &Path) -> Result<bool> {
    let path = root.join("ci/build-scripts.allowlist");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let found = packages_with_build_script(&crate::metadata_all_features(root)?);
    let (blessed, report) = bless(&text, &found);
    for line in report.lines() {
        println!("{line}");
    }
    if !report.reversioned.is_empty() {
        println!("read the build.rs of every re-versioned crate above before committing");
    }
    if blessed != text.replace("\r\n", "\n") {
        std::fs::write(&path, &blessed).with_context(|| format!("writing {}", path.display()))?;
        println!("check-build-scripts: wrote ci/build-scripts.allowlist");
    }
    let violations = check(&parse_allowlist(&blessed)?, &found);
    if violations.is_empty() {
        println!("check-build-scripts: ok");
        Ok(true)
    } else {
        eprintln!("check-build-scripts: {} violation(s)", violations.len());
        for v in &violations {
            eprintln!("  - {v}");
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[(&str, &str)]) -> BTreeSet<Entry> {
        items
            .iter()
            .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn parses_name_at_version_and_ignores_comments() {
        let list =
            parse_allowlist("# header\n\nserde@1.0.0  # derive support\n  anyhow@1.2.3+spec-1.1.0\n#gone\n")
                .expect("parses");
        assert_eq!(list, set(&[("serde", "1.0.0"), ("anyhow", "1.2.3+spec-1.1.0")]));
    }

    #[test]
    fn parse_rejects_malformed_lines_with_the_line_number() {
        for (text, needle) in [
            ("# c\nserde\n", "line 2"),
            ("@1.0.0\n", "empty crate name"),
            ("serde@\n", "empty version"),
            ("serde@one\n", "not a semantic version"),
            ("serde@1.0.0\n\nserde@1.0.0 # again\n", "line 3"),
        ] {
            let err = parse_allowlist(text).expect_err(text).to_string();
            assert!(err.contains(needle), "{text:?}: {err}");
        }
    }

    #[test]
    fn version_bump_fails() {
        let allow = set(&[("serde", "1.0.0")]);
        let found = set(&[("serde", "1.0.1")]);
        let v = check(&allow, &found);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(
            v[0].contains("`serde@1.0.1`") && v[0].contains("listed: 1.0.0"),
            "{v:?}"
        );
        assert!(
            v[0].contains("cargo xtask check-build-scripts --bless") && v[0].contains("build.rs"),
            "{v:?}"
        );
        assert!(v[1].contains("stale entry `serde@1.0.0`"), "{v:?}");
    }

    #[test]
    fn unlisted_crate_says_not_listed() {
        let v = check(
            &set(&[("serde", "1.0.0")]),
            &set(&[("serde", "1.0.0"), ("downloader-sys", "0.1.0")]),
        );
        assert_eq!(v.len(), 1);
        assert!(
            v[0].contains("`downloader-sys@0.1.0`") && v[0].contains("not listed"),
            "{v:?}"
        );
    }

    #[test]
    fn exact_match_passes() {
        let both = set(&[("serde", "1.0.0"), ("anyhow", "1.0.2")]);
        assert!(check(&both, &both).is_empty());
    }

    #[test]
    fn stale_entry_fails() {
        let v = check(
            &set(&[("serde", "1.0.0"), ("gone", "0.1.0")]),
            &set(&[("serde", "1.0.0")]),
        );
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("stale entry `gone@0.1.0`"), "{v:?}");
    }

    #[test]
    fn bless_reversions_in_place_keeping_comment_and_position() {
        let text = "# header\nanyhow@1.0.0 # first\nserde@1.0.0 # derive support\n\n# group\nzmij@0.1.0\n";
        let found = set(&[("anyhow", "1.0.0"), ("serde", "1.0.1"), ("zmij", "0.1.0")]);
        let (out, report) = bless(text, &found);
        assert_eq!(
            out,
            "# header\nanyhow@1.0.0 # first\nserde@1.0.1 # derive support\n\n# group\nzmij@0.1.0\n"
        );
        assert_eq!(report.reversioned, ["re-versioned serde: 1.0.0 -> 1.0.1"]);
        assert!(report.removed.is_empty() && report.unlisted.is_empty());
    }

    #[test]
    fn bless_migrates_a_legacy_name_only_line() {
        let (out, report) = bless("# h\nserde # derive\n", &set(&[("serde", "1.0.9")]));
        assert_eq!(out, "# h\nserde@1.0.9 # derive\n");
        assert_eq!(report.reversioned.len(), 1);
    }

    #[test]
    fn bless_writes_two_lines_for_two_versions() {
        let found = set(&[("syn", "2.0.1"), ("syn", "1.0.109"), ("syn", "2.0.10")]);
        let (out, _) = bless("syn # proc macro\n", &found);
        assert_eq!(
            out,
            "syn@1.0.109 # proc macro\nsyn@2.0.1 # proc macro\nsyn@2.0.10 # proc macro\n"
        );
        // A version already listed on another line is not repeated.
        let (out, _) = bless(
            "syn@2.0.1 # kept\nsyn@0.9.0 # old\n",
            &set(&[("syn", "2.0.1"), ("syn", "2.0.2")]),
        );
        assert_eq!(out, "syn@2.0.1 # kept\nsyn@2.0.2 # old\n");
    }

    #[test]
    fn bless_drops_absent_names_and_keeps_comment_lines() {
        let (out, report) = bless(
            "# group\ngone@0.1.0 # old\nserde@1.0.0\n",
            &set(&[("serde", "1.0.0")]),
        );
        assert_eq!(out, "# group\nserde@1.0.0\n");
        assert_eq!(report.removed, ["removed gone@0.1.0"]);
    }

    #[test]
    fn bless_does_not_add_unlisted_names() {
        let (out, report) = bless("serde@1.0.0\n", &set(&[("serde", "1.0.0"), ("fresh", "0.2.0")]));
        assert_eq!(out, "serde@1.0.0\n");
        assert_eq!(
            report.unlisted,
            ["unlisted fresh@0.2.0 (needs a reviewed line written by hand)"]
        );
    }

    #[test]
    fn bless_is_idempotent() {
        let text = "# h\nold # legacy\nserde@1.0.0 # c\nsyn@1.0.0 # s\ngone@0.1.0\n";
        let found = set(&[
            ("old", "3.0.0"),
            ("serde", "1.0.4"),
            ("syn", "1.0.0"),
            ("syn", "2.0.0"),
            ("new", "1.0.0"),
        ]);
        let (once, _) = bless(text, &found);
        let (twice, report) = bless(&once, &found);
        assert_eq!(once, twice);
        assert!(
            report.reversioned.is_empty() && report.removed.is_empty(),
            "{report:?}"
        );
    }
}
