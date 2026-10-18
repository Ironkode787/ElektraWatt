// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-workflows`: the workflow lint of NFR-SEC-19 over `.github/workflows/*.yml`.
//!
//! - Every `uses:` of a remote action is pinned to a full 40-hex commit SHA and carries a trailing version
//!   comment such as `# v4.2.2`, so a reader (and Renovate) can see what the SHA stands for. Local actions
//!   (`./…`) are exempt; `docker://` actions must be pinned by `@sha256:` digest.
//! - The workflow declares a top-level `permissions:` block whose `contents` is `read` and which grants no
//!   `write` scope; jobs that need more (release jobs) widen it per job.
//! - No `permissions: write-all` anywhere.
//!
//! The parser is deliberately line-based (no YAML dependency): it understands block mappings, `- uses:` list
//! items, comments and block scalars (`run: |`), which is all a workflow file needs here.

use std::path::Path;

use anyhow::{Context, Result};

/// Checks every `*.yml` and `*.yaml` file in `dir`.
pub fn check_dir(dir: &Path) -> Result<Vec<String>> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "yml" || ext == "yaml"))
        .collect();
    files.sort();
    let mut violations = Vec::new();
    if files.is_empty() {
        violations.push(format!("no workflow files in {}", dir.display()));
    }
    for path in files {
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        violations.extend(check_text(&name, &text));
    }
    Ok(violations)
}

/// One meaningful line of a workflow: its number, indentation and content without the trailing comment.
struct Line<'a> {
    number: usize,
    indent: usize,
    content: &'a str,
    comment: Option<&'a str>,
}

/// Splits a line into content and comment. A `#` starts a comment at the line start or after whitespace and
/// outside quotes (enough for workflow files).
fn split_comment(line: &str) -> (&str, Option<&str>) {
    let bytes = line.as_bytes();
    let (mut single, mut double) = (false, false);
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'\'' if !double => single = !single,
            b'"' if !single => double = !double,
            b'#' if !single && !double && (i == 0 || bytes[i - 1].is_ascii_whitespace()) => {
                return (&line[..i], Some(line[i + 1..].trim()));
            }
            _ => {}
        }
    }
    (line, None)
}

/// The meaningful lines of a workflow, skipping blank lines, comment-only lines and block-scalar content.
fn lines(text: &str) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut scalar_indent: Option<usize> = None;
    for (i, raw) in text.lines().enumerate() {
        let indent = raw.len() - raw.trim_start().len();
        if let Some(block) = scalar_indent {
            if raw.trim().is_empty() || indent > block {
                continue;
            }
            scalar_indent = None;
        }
        let (content, comment) = split_comment(raw);
        let content = content.trim_end();
        if content.trim().is_empty() {
            continue;
        }
        let value = content.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
        if matches!(value, "|" | "|-" | "|+" | ">" | ">-" | ">+") {
            scalar_indent = Some(indent);
        }
        out.push(Line {
            number: i + 1,
            indent,
            content: content.trim_start(),
            comment,
        });
    }
    out
}

/// `key: value` of a line (after an optional `- ` list marker), with the value unquoted.
fn key_value(content: &str) -> Option<(&str, &str)> {
    let content = content.strip_prefix("- ").unwrap_or(content).trim_start();
    let (key, value) = content.split_once(':')?;
    Some((key.trim(), value.trim().trim_matches(|c| c == '"' || c == '\'')))
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A version comment starts with a version number, optionally prefixed by `v`: `v4.2.2`, `v1`, `2.9.2`.
fn is_version_comment(comment: Option<&str>) -> bool {
    comment
        .map(|c| c.strip_prefix('v').unwrap_or(c))
        .is_some_and(|c| c.starts_with(|ch: char| ch.is_ascii_digit()))
}

/// Checks one workflow file's text; `file` names it in the messages.
pub fn check_text(file: &str, text: &str) -> Vec<String> {
    let lines = lines(text);
    let mut v = Vec::new();

    for line in &lines {
        let Some((key, value)) = key_value(line.content) else {
            continue;
        };
        let at = format!("{file}:{}", line.number);
        match key {
            "uses" => check_uses(&at, value, line.comment, &mut v),
            "permissions" if value == "write-all" => {
                v.push(format!(
                    "{at}: `permissions: write-all` grants every scope; list the needed ones"
                ));
            }
            _ => {}
        }
    }

    check_top_level_permissions(file, &lines, &mut v);
    v
}

fn check_uses(at: &str, value: &str, comment: Option<&str>, v: &mut Vec<String>) {
    if value.starts_with("./") {
        return;
    }
    if let Some(image) = value.strip_prefix("docker://") {
        let pinned = image
            .split_once("@sha256:")
            .is_some_and(|(_, digest)| is_hex(digest, 64));
        if !pinned {
            v.push(format!(
                "{at}: docker action `{value}` is not pinned by @sha256 digest"
            ));
        }
        return;
    }
    match value.split_once('@') {
        Some((_, reference)) if is_hex(reference, 40) => {
            if !is_version_comment(comment) {
                v.push(format!(
                    "{at}: `{value}` needs a trailing version comment such as `# v1.2.3`"
                ));
            }
        }
        _ => v.push(format!(
            "{at}: action `{value}` is not pinned to a full 40-hex commit SHA"
        )),
    }
}

fn check_top_level_permissions(file: &str, lines: &[Line<'_>], v: &mut Vec<String>) {
    let Some(start) = lines
        .iter()
        .position(|l| l.indent == 0 && key_value(l.content).is_some_and(|(k, _)| k == "permissions"))
    else {
        v.push(format!(
            "{file}: no top-level `permissions:` block (the default token must be `contents: read`)"
        ));
        return;
    };
    let head = &lines[start];
    let (_, inline) = key_value(head.content).unwrap_or(("", ""));
    if !inline.is_empty() {
        v.push(format!(
            "{file}:{}: top-level `permissions: {inline}`; use a block with `contents: read`",
            head.number
        ));
        return;
    }
    let mut contents_read = false;
    for line in lines[start + 1..].iter().take_while(|l| l.indent > 0) {
        let Some((scope, access)) = key_value(line.content) else {
            continue;
        };
        if scope == "contents" && access == "read" {
            contents_read = true;
        }
        if access == "write" {
            v.push(format!(
                "{file}:{}: top-level permissions grant `{scope}: write`; grant it per job instead",
                line.number
            ));
        }
    }
    if !contents_read {
        v.push(format!(
            "{file}:{}: top-level permissions lack `contents: read`",
            head.number
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "3d3c42e5aac5ba805825da76410c181273ba90b1";

    fn workflow(steps: &str) -> String {
        format!(
            "name: ci\non: [push]\npermissions:\n  contents: read\njobs:\n  build:\n    runs-on: ubuntu-24.04\n    steps:\n{steps}"
        )
    }

    #[test]
    fn pinned_action_with_version_comment_passes() {
        let text = workflow(&format!(
            "      - uses: actions/checkout@{SHA} # v7.0.1\n      - name: x\n        uses: \"a/b/sub@{SHA}\" # v1\n"
        ));
        assert_eq!(check_text("ci.yml", &text), Vec::<String>::new());
    }

    #[test]
    fn tag_pinned_action_fails() {
        let v = check_text("ci.yml", &workflow("      - uses: actions/checkout@v7 # v7\n"));
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(
            v[0].contains("ci.yml:9: action `actions/checkout@v7` is not pinned"),
            "{v:?}"
        );
    }

    #[test]
    fn short_sha_and_unpinned_actions_fail() {
        let v = check_text(
            "ci.yml",
            &workflow("      - uses: a/b@3d3c42e # v1\n      - uses: a/c\n"),
        );
        assert_eq!(v.len(), 2, "{v:?}");
    }

    #[test]
    fn missing_version_comment_fails() {
        let v = check_text(
            "ci.yml",
            &workflow(&format!("      - uses: actions/checkout@{SHA}\n")),
        );
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("trailing version comment"), "{v:?}");
    }

    #[test]
    fn local_actions_are_exempt_and_docker_needs_digest() {
        let digest = "a".repeat(64);
        let text = workflow(&format!(
            "      - uses: ./.github/actions/setup\n      - uses: docker://alpine@sha256:{digest}\n      - uses: docker://alpine:3\n"
        ));
        let v = check_text("ci.yml", &text);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("docker://alpine:3"), "{v:?}");
    }

    #[test]
    fn uses_inside_run_block_is_ignored() {
        let text = workflow("      - run: |\n          echo uses: a/b@main\n          # uses: c/d@v1\n");
        assert_eq!(check_text("ci.yml", &text), Vec::<String>::new());
    }

    #[test]
    fn missing_top_level_permissions_fails() {
        let text = "on: [push]\njobs:\n  a:\n    permissions:\n      contents: read\n    runs-on: x\n";
        let v = check_text("ci.yml", text);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("no top-level `permissions:`"), "{v:?}");
    }

    #[test]
    fn write_permissions_fail() {
        let text = "on: [push]\npermissions:\n  contents: write\njobs: {}\n";
        let v = check_text("ci.yml", text);
        assert!(v.iter().any(|m| m.contains("grant `contents: write`")), "{v:?}");
        assert!(v.iter().any(|m| m.contains("lack `contents: read`")), "{v:?}");

        let text = "on: [push]\npermissions: write-all\njobs: {}\n";
        let v = check_text("ci.yml", text);
        assert!(v.iter().any(|m| m.contains("write-all")), "{v:?}");

        let text = "on: [push]\npermissions:\n  contents: read\njobs:\n  a:\n    permissions: write-all\n";
        let v = check_text("ci.yml", text);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("ci.yml:6: `permissions: write-all`"), "{v:?}");
    }

    #[test]
    fn job_level_write_scope_is_allowed() {
        let text = "on: [push]\npermissions:\n  contents: read\njobs:\n  rel:\n    permissions:\n      id-token: write\n";
        assert_eq!(check_text("ci.yml", text), Vec::<String>::new());
    }

    /// The committed workflows pass the lint.
    #[test]
    fn real_workflows_pass() {
        let dir = crate::workspace_root().join(".github/workflows");
        assert_eq!(check_dir(&dir).expect("workflows readable"), Vec::<String>::new());
    }
}
