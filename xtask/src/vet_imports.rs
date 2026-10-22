// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-vet-imports`: the committed `supply-chain/imports.lock` carries no auditor identity (ADR-0023
//! amendment 5). `cargo vet` writes the imported audit sets into that file together with the auditors' names
//! (`who`, `user-name`), free-text `notes` and a `user-id = N # Name (login)` comment, and the public-repository
//! rule forbids personal names and e-mail addresses in the repository. The check parses the file as TOML and
//! reports every `who`, `notes` and `user-name` key with its path, every line with an e-mail-like token and every
//! `user-id` comment that holds a name. `--bless` scrubs the file, runs `cargo vet fmt` and checks again.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The lock file, relative to the workspace root.
const LOCK: &str = "supply-chain/imports.lock";

/// Keys that carry an auditor identity or free text.
const FORBIDDEN_KEYS: [&str; 3] = ["who", "notes", "user-name"];

const BLESS_HINT: &str = "run `cargo xtask check-vet-imports --bless`";

/// Reads the lock file and returns its violations.
pub fn violations(root: &Path) -> Result<Vec<String>> {
    let path = root.join(LOCK);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(check(&text))
}

/// One violation per forbidden key (with its dotted path), per e-mail-like line and per `user-id` comment that
/// holds a name.
pub fn check(text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    match text.parse::<toml::Table>() {
        Ok(table) => walk_table(&table, "", &mut violations),
        Err(err) => violations.push(format!("{LOCK} does not parse as TOML: {err}")),
    }
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        if has_email(line) {
            violations.push(format!(
                "{LOCK} line {number}: contains an e-mail address; {BLESS_HINT}"
            ));
        }
        if user_id_comment_names_someone(line) {
            violations.push(format!(
                "{LOCK} line {number}: the user-id comment holds a name, not only a login; {BLESS_HINT}"
            ));
        }
    }
    violations
}

fn walk_table(table: &toml::Table, path: &str, violations: &mut Vec<String>) {
    for (key, value) in table {
        let here = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
        if FORBIDDEN_KEYS.contains(&key.as_str()) {
            violations.push(format!(
                "{LOCK}: forbidden key `{here}` (auditor identity or notes); {BLESS_HINT}"
            ));
        }
        walk_value(value, &here, violations);
    }
}

fn walk_value(value: &toml::Value, path: &str, violations: &mut Vec<String>) {
    match value {
        toml::Value::Table(table) => walk_table(table, path, violations),
        toml::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                walk_value(item, &format!("{path}[{index}]"), violations);
            }
        }
        _ => {}
    }
}

/// A whitespace-separated token (surrounding `<>"',` stripped) with an `@` whose part after it has a `.`.
fn has_email(line: &str) -> bool {
    line.split_whitespace().any(|token| {
        let token = token.trim_matches(|c| matches!(c, '<' | '>' | '"' | '\'' | ','));
        token
            .split_once('@')
            .is_some_and(|(_, domain)| domain.contains('.'))
    })
}

/// Splits `user-id = <digits> # <comment>` into the part up to the comment and the comment text.
fn split_user_id(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("user-id = ")?;
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let comment = rest[digits..].trim_start().strip_prefix('#')?;
    let end = line.len() - trimmed.len() + "user-id = ".len() + digits;
    Some((&line[..end], comment.trim()))
}

fn user_id_comment_names_someone(line: &str) -> bool {
    split_user_id(line).is_some_and(|(_, comment)| comment.contains(' ') || comment.contains('('))
}

/// How a forbidden key's value continues after its first line.
fn value_end(value: &str) -> ValueEnd {
    for delimiter in ["\"\"\"", "'''"] {
        if let Some(after) = value.strip_prefix(delimiter) {
            return if after.contains(delimiter) {
                ValueEnd::SameLine
            } else {
                ValueEnd::Delimiter(delimiter)
            };
        }
    }
    if value.starts_with('[') && !value.trim_end().ends_with(']') {
        return ValueEnd::ArrayClose;
    }
    ValueEnd::SameLine
}

enum ValueEnd {
    SameLine,
    /// Until the line that ends with this delimiter.
    Delimiter(&'static str),
    /// Until the line that is `]`.
    ArrayClose,
}

/// Removes the identity keys (multi-line values included) and rewrites `user-id = N # Name (login)` to
/// `user-id = N # login`. Output uses `\n` line endings. Idempotent.
pub fn scrub(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if let Some((key, value)) = trimmed.split_once('=') {
            if FORBIDDEN_KEYS.contains(&key.trim()) {
                match value_end(value.trim()) {
                    ValueEnd::SameLine => {}
                    ValueEnd::Delimiter(delimiter) => {
                        for next in lines.by_ref() {
                            if next.trim_end().ends_with(delimiter) {
                                break;
                            }
                        }
                    }
                    ValueEnd::ArrayClose => {
                        for next in lines.by_ref() {
                            if next.trim() == "]" {
                                break;
                            }
                        }
                    }
                }
                continue;
            }
        }
        if let Some((head, comment)) = split_user_id(line) {
            if comment.contains(' ') || comment.contains('(') {
                let login = comment.rfind('(').and_then(|open| {
                    let inner = &comment[open + 1..];
                    inner.find(')').map(|close| inner[..close].trim())
                });
                match login {
                    Some(login) if !login.is_empty() => out.push(format!("{head} # {login}")),
                    _ => out.push(head.to_owned()),
                }
                continue;
            }
        }
        out.push(line.to_owned());
    }
    let mut scrubbed = out.join("\n");
    scrubbed.push('\n');
    scrubbed
}

/// `check-vet-imports --bless`: scrubs the lock file, runs `cargo vet fmt` and checks. Returns whether the check
/// passed.
pub fn run(root: &Path) -> Result<bool> {
    let path = root.join(LOCK);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let scrubbed = scrub(&text);
    if scrubbed != text {
        std::fs::write(&path, &scrubbed).with_context(|| format!("writing {}", path.display()))?;
        println!("check-vet-imports: scrubbed {LOCK}");
    }
    let status = Command::new(crate::cargo())
        .args(["vet", "fmt"])
        .current_dir(root)
        .status()
        .context("starting `cargo vet fmt` (is cargo-vet installed? `cargo install cargo-vet --locked --version 0.10.2`)")?;
    if !status.success() {
        bail!(
            "`cargo vet fmt` failed ({status}); the scrubbed {LOCK} is written, fix the error and run again"
        );
    }
    let violations = violations(root)?;
    if violations.is_empty() {
        println!("check-vet-imports: ok");
        Ok(true)
    } else {
        eprintln!("check-vet-imports: {} violation(s)", violations.len());
        for v in &violations {
            eprintln!("  - {v}");
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lock file with every identity form and every key that must survive.
    const DIRTY: &str = r#"# cargo-vet imports lock

[[audits.example.audits.adler2]]
who = "Some Person <some.person@example.com>"
criteria = "safe-to-deploy"
version = "2.0.0"
notes = "single line note"

[[audits.example.audits.bytes]]
who = [
    "A Person <a@example.com>",
    "B Person <b@example.com>",
]
criteria = "safe-to-run"
delta = "1.0.0 -> 1.1.0"
notes = """
first line
second line with who = inside
"""
aggregated-from = "https://example.com/audits.toml"

[[audits.example.audits.cc]]
who = 'Quoted'
criteria = "safe-to-deploy"
version = "1.0.0"
notes = '''
literal block'''

[[publisher.bumpalo]]
version = "3.20.3"
when = "2026-05-22"
user-id = 696 # Some Person (fitzgen)
user-login = "fitzgen"
user-name = "Some Person"

[[publisher.euclid]]
version = "0.22.14"
when = "2026-03-18"
user-id = 1281 # nical
user-login = "nical"
start = "2020-01-01"
end = "2027-01-01"
"#;

    #[test]
    fn scrub_removes_every_form_and_rewrites_the_comment() {
        let out = scrub(DIRTY);
        for gone in [
            "who",
            "notes",
            "user-name",
            "second line",
            "literal block",
            "Person",
        ] {
            assert!(!out.contains(gone), "{gone} left in:\n{out}");
        }
        assert!(out.contains("user-id = 696 # fitzgen\n"), "{out}");
        assert!(out.contains("user-id = 1281 # nical\n"), "{out}");
        assert!(check(&out).is_empty(), "{:?}", check(&out));
    }

    #[test]
    fn scrub_leaves_everything_else_untouched() {
        let out = scrub(DIRTY);
        for kept in [
            "# cargo-vet imports lock",
            "[[audits.example.audits.adler2]]",
            "[[publisher.bumpalo]]",
            "criteria = \"safe-to-deploy\"",
            "criteria = \"safe-to-run\"",
            "version = \"2.0.0\"",
            "delta = \"1.0.0 -> 1.1.0\"",
            "user-login = \"fitzgen\"",
            "when = \"2026-05-22\"",
            "start = \"2020-01-01\"",
            "end = \"2027-01-01\"",
            "aggregated-from = \"https://example.com/audits.toml\"",
            "user-id = 696 # fitzgen",
        ] {
            assert!(out.contains(kept), "{kept} missing in:\n{out}");
        }
        // Nothing but identity lines was removed: the section headers and key counts match.
        assert_eq!(out.matches("[[").count(), DIRTY.matches("[[").count());
    }

    #[test]
    fn scrub_is_idempotent_and_uses_lf() {
        let once = scrub(DIRTY);
        assert_eq!(scrub(&once), once);
        let crlf = scrub("who = \"X\"\r\nversion = \"1\"\r\n");
        assert_eq!(crlf, "version = \"1\"\n");
    }

    #[test]
    fn check_reports_each_forbidden_key_with_its_path() {
        let v = check(DIRTY);
        for path in [
            "audits.example.audits.adler2[0].who",
            "audits.example.audits.adler2[0].notes",
            "audits.example.audits.bytes[0].who",
            "audits.example.audits.bytes[0].notes",
            "audits.example.audits.cc[0].who",
            "audits.example.audits.cc[0].notes",
            "publisher.bumpalo[0].user-name",
        ] {
            assert!(
                v.iter().any(|m| m.contains(&format!("`{path}`"))),
                "no violation for {path} in {v:#?}"
            );
        }
        assert!(
            v.iter().all(|m| m.contains("check-vet-imports --bless")),
            "{v:#?}"
        );
    }

    #[test]
    fn check_reports_an_email_and_a_named_user_id_comment() {
        let v = check(
            "[[publisher.x]]\nuser-id = 1 # Some Person (login)\nversion = \"1\" # <a.b@example.org>\n",
        );
        assert!(
            v.iter().any(|m| m.contains("line 2") && m.contains("user-id")),
            "{v:#?}"
        );
        assert!(
            v.iter().any(|m| m.contains("line 3") && m.contains("e-mail")),
            "{v:#?}"
        );
        // A login-only comment and an at-sign without a dotted domain are fine.
        assert!(check("[[publisher.x]]\nuser-id = 1 # login\nversion = \"1\" # @handle\n").is_empty());
    }

    #[test]
    fn check_reports_text_that_is_not_toml() {
        let v = check("[[publisher.x\n");
        assert!(v.iter().any(|m| m.contains("does not parse as TOML")), "{v:#?}");
    }

    /// The committed lock file carries no auditor identity.
    #[test]
    fn real_imports_lock_is_scrubbed() {
        let root = crate::workspace_root();
        assert_eq!(violations(&root).expect("lock file reads"), Vec::<String>::new());
    }
}
