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
//! - No `pull_request_target` trigger (it runs fork code with a privileged token).
//! - Every `actions/checkout` step sets `persist-credentials: false`, so later steps cannot reuse the token.
//! - `${{ secrets.… }}` appears only in a job named `release`.
//! - Every container image reference is pinned by `@sha256:<64 hex>` digest: the value of an `image:` key and of
//!   an environment variable named `*_IMAGE`, and every `docker run`, `docker pull` or `docker create` command
//!   (continuation lines joined), which must name a digest or a `$..._IMAGE` variable (checked at its definition).
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
        let at = format!("{file}:{}", line.number);
        if line.content.contains("pull_request_target") {
            v.push(format!(
                "{at}: `pull_request_target` runs fork code with a privileged token; use `pull_request`"
            ));
        }
        let Some((key, value)) = key_value(line.content) else {
            continue;
        };
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
    check_checkout_credentials(file, &lines, &mut v);
    check_secrets(file, text, &mut v);
    check_images(file, text, &mut v);
    v
}

/// `image@sha256:<64 hex>` somewhere in `text`.
fn digest_pinned(text: &str) -> bool {
    text.match_indices("@sha256:").any(|(at, key)| {
        text.get(at + key.len()..at + key.len() + 64)
            .is_some_and(|d| is_hex(d, 64))
    })
}

/// Whether `command` names a variable `$NAME_IMAGE` or `${NAME_IMAGE}`.
fn names_image_variable(command: &str) -> bool {
    command.match_indices('$').any(|(at, _)| {
        let rest = command[at + 1..].trim_start_matches('{');
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
            .collect();
        name.ends_with("_IMAGE")
    })
}

/// Container images are pinned by digest: `image:` and `*_IMAGE` values must carry `@sha256:<64 hex>` (or be an
/// `${{ env.… }}` expression, whose definition is checked itself), and a `docker run`, `docker pull` or
/// `docker create` command must name a digest or a `$..._IMAGE` variable. Works on the raw lines, because the
/// commands live in `run:` blocks; a command is the line with its `\` continuations joined, and counts only at
/// the start of a shell segment (after `&&`, `||`, `;`), so a message that mentions `docker pull` is no command.
fn check_images(file: &str, text: &str, v: &mut Vec<String>) {
    let raw: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < raw.len() {
        let number = i + 1;
        let trimmed = raw[i].trim();
        i += 1;
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (content, _) = split_comment(trimmed);
        let at = format!("{file}:{number}");
        if let Some((key, value)) = key_value(content)
            && (key == "image" || key.ends_with("_IMAGE"))
            && !value.is_empty()
            && !digest_pinned(value)
            && !value.starts_with("${{")
        {
            v.push(format!(
                "{at}: container image `{value}` ({key}) is not pinned by @sha256 digest"
            ));
        }
        // The logical command line: continuation lines joined.
        let mut command = content.trim_end().to_owned();
        while command.ends_with('\\') && i < raw.len() {
            command.pop();
            command.push(' ');
            command.push_str(split_comment(raw[i].trim()).0.trim_end());
            i += 1;
        }
        let command = command.replace("&&", ";").replace("||", ";");
        for segment in command.split(';') {
            let segment = segment.trim_start_matches(|c: char| c == '-' || c.is_whitespace());
            let segment = segment.strip_prefix("run:").unwrap_or(segment).trim_start();
            let is_container_command = ["docker run ", "docker pull ", "docker create "]
                .iter()
                .any(|c| segment.starts_with(c));
            if is_container_command && !digest_pinned(segment) && !names_image_variable(segment) {
                v.push(format!(
                    "{at}: `{}` names no image pinned by @sha256 digest or `$..._IMAGE` variable",
                    segment.split_whitespace().take(2).collect::<Vec<_>>().join(" ")
                ));
            }
        }
    }
}

/// Index range of the step (list item) that contains `lines[at]`: from the nearest enclosing `- ` line to
/// the line before the next line that is indented no deeper than that item.
fn step_range(lines: &[Line<'_>], at: usize) -> (usize, usize) {
    let start = (0..=at)
        .rev()
        .find(|&i| lines[i].content.starts_with("- ") && lines[i].indent <= lines[at].indent)
        .unwrap_or(at);
    let indent = lines[start].indent;
    let end = (start + 1..lines.len())
        .find(|&i| lines[i].indent <= indent)
        .unwrap_or(lines.len());
    (start, end)
}

/// Every `actions/checkout` step sets `persist-credentials: false`.
fn check_checkout_credentials(file: &str, lines: &[Line<'_>], v: &mut Vec<String>) {
    for (i, line) in lines.iter().enumerate() {
        let Some(("uses", value)) = key_value(line.content) else {
            continue;
        };
        if !value.starts_with("actions/checkout@") {
            continue;
        }
        let (start, end) = step_range(lines, i);
        let credentials_dropped = lines[start..end]
            .iter()
            .any(|l| key_value(l.content) == Some(("persist-credentials", "false")));
        if !credentials_dropped {
            v.push(format!(
                "{file}:{}: `{value}` needs `persist-credentials: false` in its `with:` block",
                line.number
            ));
        }
    }
}

/// `${{ secrets.… }}` only inside a job named `release`. Works on the raw lines (a `run:` block can use a
/// secret too) and skips comment-only lines.
fn check_secrets(file: &str, text: &str, v: &mut Vec<String>) {
    let mut in_jobs = false;
    let mut job_indent: Option<usize> = None;
    let mut job: Option<&str> = None;
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        if indent == 0 {
            in_jobs = trimmed.starts_with("jobs:");
            job_indent = None;
            job = None;
        } else if in_jobs {
            let job_indent = *job_indent.get_or_insert(indent);
            if indent == job_indent {
                job = trimmed.split_once(':').map(|(name, _)| name.trim());
            }
        }
        let squeezed: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        if squeezed.contains("${{secrets.") && job != Some("release") {
            let place = job.map_or_else(|| "outside any job".to_owned(), |j| format!("in job `{j}`"));
            v.push(format!(
                "{file}:{}: `secrets` used {place}; secrets belong only in the `release` job",
                i + 1
            ));
        }
    }
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
            "      - uses: actions/cache@{SHA} # v7.0.1\n      - name: x\n        uses: \"a/b/sub@{SHA}\" # v1\n"
        ));
        assert_eq!(check_text("ci.yml", &text), Vec::<String>::new());
    }

    #[test]
    fn tag_pinned_action_fails() {
        let v = check_text("ci.yml", &workflow("      - uses: actions/cache@v7 # v7\n"));
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(
            v[0].contains("ci.yml:9: action `actions/cache@v7` is not pinned"),
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
            &workflow(&format!("      - uses: actions/cache@{SHA}\n")),
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
    fn pull_request_target_fails_in_every_form() {
        for on in [
            "on: pull_request_target\n",
            "on: [push, pull_request_target]\n",
            "on:\n  pull_request_target:\n    types: [opened]\n",
            "on:\n  - pull_request_target\n",
        ] {
            let text = format!("{on}permissions:\n  contents: read\njobs: {{}}\n");
            let v = check_text("ci.yml", &text);
            assert_eq!(v.len(), 1, "{on:?}: {v:?}");
            assert!(v[0].contains("`pull_request_target`"), "{v:?}");
        }
        // Mentions in comments and run blocks are not triggers.
        let text =
            workflow("      - run: |\n          echo pull_request_target\n      # pull_request_target\n");
        assert_eq!(check_text("ci.yml", &text), Vec::<String>::new());
    }

    #[test]
    fn checkout_without_persist_credentials_false_fails() {
        let bare = workflow(&format!("      - uses: actions/checkout@{SHA} # v7.0.1\n"));
        let v = check_text("ci.yml", &bare);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(
            v[0].contains("ci.yml:9:") && v[0].contains("persist-credentials: false"),
            "{v:?}"
        );

        let wrong = workflow(&format!(
            "      - uses: actions/checkout@{SHA} # v7.0.1\n        with:\n          persist-credentials: true\n"
        ));
        assert_eq!(check_text("ci.yml", &wrong).len(), 1);

        // A neighbouring step's setting does not count; a `name:`-first step is found.
        let neighbour = workflow(&format!(
            "      - uses: actions/checkout@{SHA} # v7.0.1\n      - uses: other/x@{SHA} # v1\n        with:\n          persist-credentials: false\n"
        ));
        assert_eq!(check_text("ci.yml", &neighbour).len(), 1);

        let good = workflow(&format!(
            "      - name: checkout\n        uses: actions/checkout@{SHA} # v7.0.1\n        with:\n          fetch-depth: 0\n          persist-credentials: false\n      - run: ls\n"
        ));
        assert_eq!(check_text("ci.yml", &good), Vec::<String>::new());
    }

    #[test]
    fn secrets_outside_release_job_fail() {
        let text =
            workflow("      - run: deploy\n        env:\n          TOKEN: ${{ secrets.DEPLOY_TOKEN }}\n");
        let v = check_text("ci.yml", &text);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(
            v[0].contains("ci.yml:11:") && v[0].contains("in job `build`"),
            "{v:?}"
        );

        // Inside a run block, without spaces, and the built-in token too.
        let text = workflow("      - run: |\n          curl -H ${{secrets.GITHUB_TOKEN}} x\n");
        assert_eq!(check_text("ci.yml", &text).len(), 1);

        // Workflow-level env is outside any job.
        let text = "on: [push]\npermissions:\n  contents: read\nenv:\n  T: ${{ secrets.T }}\njobs: {}\n";
        let v = check_text("ci.yml", text);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("outside any job"), "{v:?}");

        // Comments are fine.
        let text = workflow("      # never ${{ secrets.X }}\n      - run: ls\n");
        assert_eq!(check_text("ci.yml", &text), Vec::<String>::new());
    }

    #[test]
    fn secrets_in_release_job_pass() {
        let text = "on: [push]\npermissions:\n  contents: read\njobs:\n  build:\n    runs-on: x\n  release:\n    runs-on: x\n    steps:\n      - run: sign\n        env:\n          K: ${{ secrets.SIGNING_KEY }}\n";
        assert_eq!(check_text("ci.yml", text), Vec::<String>::new());
        // The next job after release is not covered.
        let text = format!("{text}  publish:\n    env:\n      K: ${{{{ secrets.K }}}}\n");
        let v = check_text("ci.yml", &text);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("in job `publish`"), "{v:?}");
    }

    #[test]
    fn job_level_write_scope_is_allowed() {
        let text = "on: [push]\npermissions:\n  contents: read\njobs:\n  rel:\n    permissions:\n      id-token: write\n";
        assert_eq!(check_text("ci.yml", text), Vec::<String>::new());
    }

    #[test]
    fn container_images_must_be_pinned_by_digest() {
        let digest = "b".repeat(64);
        // Pinned: an *_IMAGE variable, an `image:` key, and commands that name a digest or an *_IMAGE variable
        // (also across continuation lines and after `&&`); a message that mentions `docker pull` is no command.
        let pinned = format!(
            "name: ci\non: [push]\npermissions:\n  contents: read\nenv:\n  A_IMAGE: ubuntu@sha256:{digest}\n  B_IMAGE: \"x/y@sha256:{digest}\"\njobs:\n  build:\n    runs-on: x\n    container:\n      image: ghcr.io/o/i@sha256:{digest}\n    steps:\n      - run: docker pull \"$A_IMAGE\" && exit 0\n      - run: |\n          docker run --rm \\\n            --volume a:b \\\n            \"${{B_IMAGE}}\" sh x\n          docker create ubuntu@sha256:{digest}\n          echo \"docker pull failed\" ; echo \"docker run x\"\n          # docker run ubuntu:24.04\n"
        );
        assert_eq!(check_text("ci.yml", &pinned), Vec::<String>::new());

        let cases = [
            ("env:\n  A_IMAGE: ubuntu:24.04\n", "A_IMAGE"),
            ("env:\n  A_IMAGE: ubuntu@sha256:abc\n", "A_IMAGE"),
            ("jobs:\n  b:\n    container:\n      image: alpine:3\n", "image"),
            (
                "jobs:\n  b:\n    steps:\n      - run: docker pull ubuntu:24.04\n",
                "docker pull",
            ),
            (
                "jobs:\n  b:\n    steps:\n      - run: |\n          set -e\n          docker run --rm \\\n            --network none \\\n            ubuntu:24.04 true\n",
                "docker run",
            ),
            (
                "jobs:\n  b:\n    steps:\n      - run: make && docker create alpine\n",
                "docker create",
            ),
        ];
        for (body, what) in cases {
            let text = format!("on: [push]\npermissions:\n  contents: read\n{body}");
            let v = check_text("ci.yml", &text);
            assert_eq!(v.len(), 1, "{body:?}: {v:?}");
            assert!(v[0].contains(what), "{body:?}: {v:?}");
        }
        // The message names the line of the command's first line.
        let text = "on: [push]\npermissions:\n  contents: read\njobs:\n  b:\n    steps:\n      - run: |\n          docker run --rm \\\n            alpine true\n";
        let v = check_text("ci.yml", text);
        assert!(v[0].starts_with("ci.yml:8:"), "{v:?}");
    }

    /// The value of the first `key: value` line of `text` (quotes removed).
    fn setting<'a>(text: &'a str, key: &str) -> &'a str {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(key))
            .unwrap_or_else(|| panic!("no `{key}` line"))
            .trim()
            .trim_matches('"')
    }

    /// FT-297, FT-304: the nightly of the cargo-fuzz crate (`fuzz/rust-toolchain.toml`) and the one the deep
    /// nightly installs (`NIGHTLY_TOOLCHAIN`) are the same dated toolchain, and the stable one of the deep
    /// nightly is the workspace's.
    #[test]
    fn the_deep_nightly_pins_the_toolchains_of_the_repository() {
        let root = crate::workspace_root();
        let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
        let workflow = read(".github/workflows/deep-nightly.yml");
        let nightly = setting(&workflow, "NIGHTLY_TOOLCHAIN:");
        assert!(
            nightly.starts_with("nightly-") && nightly.len() == "nightly-2026-10-08".len(),
            "NIGHTLY_TOOLCHAIN is a dated nightly, not `{nightly}`"
        );
        assert_eq!(
            setting(&read("fuzz/rust-toolchain.toml"), "channel ="),
            nightly,
            "fuzz/rust-toolchain.toml and NIGHTLY_TOOLCHAIN of deep-nightly.yml name different nightlies"
        );
        // The jobs that run on the nightly name it for rustup as well (the stable file of the root would otherwise
        // be resolved, and that toolchain is not installed there).
        let rustup: Vec<&str> = workflow
            .lines()
            .filter_map(|l| l.trim().strip_prefix("RUSTUP_TOOLCHAIN:"))
            .map(|v| v.trim().trim_matches('"'))
            .collect();
        assert!(
            rustup.len() >= 2,
            "the nightly jobs set RUSTUP_TOOLCHAIN: {rustup:?}"
        );
        assert!(
            rustup.iter().all(|v| *v == nightly),
            "RUSTUP_TOOLCHAIN of a job differs from NIGHTLY_TOOLCHAIN ({nightly}): {rustup:?}"
        );
        assert_eq!(
            setting(&workflow, "RUST_TOOLCHAIN:"),
            setting(&read("rust-toolchain.toml"), "channel ="),
            "RUST_TOOLCHAIN of deep-nightly.yml is not the workspace's pinned stable"
        );
    }

    /// The committed workflows pass the lint.
    #[test]
    fn real_workflows_pass() {
        let dir = crate::workspace_root().join(".github/workflows");
        assert_eq!(check_dir(&dir).expect("workflows readable"), Vec::<String>::new());
    }
}
