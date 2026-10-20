// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hostile input (doc 05 §3.6, NFR-SEC-02): every limit of the loader, without recursing into a hostile
//! line.
//!
//! The functional tests bound the work structurally: a line over the depth limit is reported by the
//! linear pre-scan (exactly one `LimitExceeded` finding, EW-CORE-019, whose message names the nesting
//! limit) and never reaches the recursive JSON parser, whose own failure would be a syntax finding
//! instead. The wall-clock budget is a performance test (`CLAUDE.md` testing policy):
//! `hostile_inputs_load_within_the_budget` is `#[ignore]`d and runs at milestone deliveries only.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ew_format::{FilePath, FindingKind, Kept, Limits, LineKey, Loaded, jsonl_file, load};

const DEVICE: &str = r#"{"$k":"device","id":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","tag":{"letter":"G","counter":1},"kind":"psu","parts":[{"part":{"copy":"e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025"},"qty":"1"}],"function_text":{"en-GB":"Power supply 1 AC / 24 V DC"},"tech_data":[{"key":"i_out","value":{"value":"20","unit":"A"}}]}"#;
const FUNCTION: &str = r#"{"$k":"function","id":"1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19","device":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","def":"5b9e2d70-3c41-5f8a-b6d2-19e4c7a03f58","kind":"black_box"}"#;

const SHARD: &str = "devices/c2.jsonl";

fn p(s: &str) -> FilePath {
    FilePath::new(s).unwrap()
}

fn load_bytes(path: &str, bytes: &[u8], limits: &Limits) -> Loaded {
    let files: BTreeMap<FilePath, Vec<u8>> = [(p(path), bytes.to_vec())].into();
    load(None, &files, limits)
}

fn count(loaded: &Loaded, kind: FindingKind) -> usize {
    loaded.findings.iter().filter(|f| f.kind == kind).count()
}

/// The single finding of a line rejected by the depth pre-scan, not by the parser.
fn assert_rejected_by_the_pre_scan(loaded: &Loaded, limits: &Limits) {
    assert_eq!(loaded.findings.len(), 1, "{:?}", loaded.findings);
    let f = &loaded.findings[0];
    assert_eq!(f.kind, FindingKind::LimitExceeded);
    assert_eq!(f.code(), Some("EW-CORE-019"));
    assert_eq!(
        f.message,
        format!("nesting deeper than {}", limits.max_depth),
        "the pre-scan, not the parser, rejected the line"
    );
}

#[test]
fn a_hundred_thousand_open_brackets_exceed_the_depth_limit() {
    for open in ["[", "{\"a\":"] {
        let line = open.repeat(100_000);
        let limits = Limits::default();
        let loaded = load_bytes(SHARD, format!("{line}\n").as_bytes(), &limits);
        assert_rejected_by_the_pre_scan(&loaded, &limits);
        assert_eq!(loaded.findings[0].line, Some(1));
        assert!(loaded.read_only);
        assert_eq!(loaded.kept.len(), 1);
        assert_eq!(loaded.kept[0].kept, Kept::Quarantined);
        assert_eq!(loaded.kept[0].text, line.as_bytes());
        assert!(loaded.records.is_empty());
    }
}

#[test]
fn depth_exactly_the_limit_passes_the_scan() {
    let limits = Limits::default();
    let at_limit = format!("{}{}", "[".repeat(limits.max_depth), "]".repeat(limits.max_depth));
    let loaded = load_bytes(SHARD, format!("{at_limit}\n").as_bytes(), &limits);
    assert_eq!(
        count(&loaded, FindingKind::LimitExceeded),
        0,
        "{:?}",
        loaded.findings
    );
    // Not a record: an array has no `$k` (a schema violation, kept verbatim).
    assert_eq!(count(&loaded, FindingKind::SchemaViolation), 1);
    assert_eq!(loaded.kept[0].text, at_limit.as_bytes());

    let one_more = format!(
        "{}{}",
        "[".repeat(limits.max_depth + 1),
        "]".repeat(limits.max_depth + 1)
    );
    let loaded = load_bytes(SHARD, format!("{one_more}\n").as_bytes(), &limits);
    assert_rejected_by_the_pre_scan(&loaded, &limits);
}

#[test]
fn brackets_inside_strings_do_not_count() {
    let brackets = "[".repeat(1000);
    let line = format!(
        r#"{{"$k":"frobnicator","id":"3b9c1d72-5e48-7a06-8f13-2d6c0e9a4b51","s":"\"{brackets}\\","t":"{brackets}"}}"#
    );
    let loaded = load_bytes(
        "structure.jsonl",
        format!("{line}\n").as_bytes(),
        &Limits::default(),
    );
    assert_eq!(
        count(&loaded, FindingKind::LimitExceeded),
        0,
        "{:?}",
        loaded.findings
    );
    assert_eq!(loaded.records.len(), 1);
    assert_eq!(loaded.kept[0].text, line.as_bytes());
}

#[test]
fn a_line_over_the_limit_is_quarantined() {
    let limits = Limits {
        max_line_bytes: 200,
        ..Limits::default()
    };
    let long = format!(r#"{{"$k":"frobnicator","id":"x","s":"{}"}}"#, "a".repeat(300));
    let text = format!("{FUNCTION}\n{long}\n");
    let loaded = load_bytes(SHARD, text.as_bytes(), &limits);
    let found: Vec<_> = loaded
        .findings
        .iter()
        .filter(|f| f.kind == FindingKind::LimitExceeded)
        .collect();
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].line, Some(2));
    assert_eq!(found[0].file, Some(p(SHARD)));
    assert!(loaded.read_only);
    assert_eq!(loaded.kept.len(), 1);
    assert_eq!(loaded.kept[0].text, long.as_bytes());
    assert_eq!(loaded.kept[0].kept, Kept::Quarantined);
    // The record before it is read.
    assert_eq!(loaded.records.len(), 1);
}

#[test]
fn a_file_over_the_limit_is_opaque_and_not_parsed() {
    let limits = Limits {
        max_file_bytes: 100,
        ..Limits::default()
    };
    let text = format!("{DEVICE}\n{FUNCTION}\n");
    let loaded = load_bytes(SHARD, text.as_bytes(), &limits);
    assert_eq!(loaded.findings.len(), 1, "{:?}", loaded.findings);
    assert_eq!(loaded.findings[0].kind, FindingKind::LimitExceeded);
    assert_eq!(loaded.findings[0].file, Some(p(SHARD)));
    assert_eq!(loaded.findings[0].line, None);
    assert!(loaded.read_only);
    assert!(loaded.records.is_empty() && loaded.kept.is_empty());
    assert_eq!(loaded.opaque[&p(SHARD)], ew_format::sha256(text.as_bytes()));
    assert_eq!(loaded.hashes[&p(SHARD)], ew_format::sha256(text.as_bytes()));
    assert!(loaded.rewrite.is_empty());
}

#[test]
fn too_many_lines_stop_the_file() {
    let limits = Limits {
        max_lines_per_file: 2,
        ..Limits::default()
    };
    let text = format!("{DEVICE}\n{FUNCTION}\n{FUNCTION}\n{FUNCTION}\n");
    let loaded = load_bytes(SHARD, text.as_bytes(), &limits);
    assert_eq!(count(&loaded, FindingKind::LimitExceeded), 1);
    assert!(loaded.read_only);
    // The lines read before the limit stay; the file is never scheduled for a rewrite.
    assert_eq!(loaded.records.len(), 2);
    assert!(loaded.rewrite.is_empty());
    assert_eq!(count(&loaded, FindingKind::NonCanonical), 0);
    // Exactly at the limit is fine.
    let text = format!("{DEVICE}\n{FUNCTION}\n");
    let loaded = load_bytes(SHARD, text.as_bytes(), &limits);
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
}

#[test]
fn invalid_utf8_is_a_syntax_error_kept_byte_for_byte() {
    let mut bytes = FUNCTION.as_bytes().to_vec();
    bytes.push(b'\n');
    bytes.extend_from_slice(b"\xff\xfe\n");
    let loaded = load_bytes(SHARD, &bytes, &Limits::default());
    let found: Vec<_> = loaded
        .findings
        .iter()
        .filter(|f| f.kind == FindingKind::JsonSyntax)
        .collect();
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].line, Some(2));
    assert_eq!(found[0].code(), Some("EW-CORE-005"));
    assert!(loaded.read_only);
    assert_eq!(loaded.kept.len(), 1);
    assert_eq!(loaded.kept[0].kept, Kept::Quarantined);
    // Kept as its bytes, not as U+FFFD: the writer puts the same bytes back (NFR-REL-04).
    assert_eq!(loaded.kept[0].text, b"\xff\xfe");
    assert_eq!(loaded.kept[0].text_lossy(), "\u{fffd}\u{fffd}");
    assert_eq!(loaded.records.len(), 1);
    let function = LineKey::of(&loaded.records[0]).expect("a keyed record");
    let rewritten = jsonl_file(vec![
        (function, FUNCTION.as_bytes().to_vec()),
        (loaded.kept[0].key.clone(), loaded.kept[0].text.clone()),
    ]);
    assert_eq!(
        rewritten, bytes,
        "the file written from the record and the kept line is the file read"
    );
}

#[test]
fn a_two_mebibyte_string_member_parses() {
    let big = "a".repeat(2 << 20);
    let line = format!(r#"{{"$k":"frobnicator","id":"3b9c1d72-5e48-7a06-8f13-2d6c0e9a4b51","s":"{big}"}}"#);
    let loaded = load_bytes(
        "structure.jsonl",
        format!("{line}\n").as_bytes(),
        &Limits::default(),
    );
    assert_eq!(loaded.records.len(), 1);
    assert_eq!(loaded.kept[0].text.len(), line.len());
    assert!(!loaded.read_only);
    // Only the newer-kind note.
    assert_eq!(loaded.findings.len(), 1);
    assert_eq!(loaded.findings[0].kind, FindingKind::NewerData);
}

#[test]
fn a_deep_library_document_is_opaque() {
    let id = "5d1ab4de-706a-5b85-b463-8b15311c4f39";
    let path = format!("lib/fndefs/{id}.json");
    let deep = "[".repeat(100_000);
    let limits = Limits::default();
    let loaded = load_bytes(&path, deep.as_bytes(), &limits);
    assert_rejected_by_the_pre_scan(&loaded, &limits);
    assert!(loaded.read_only);
    assert!(loaded.opaque.contains_key(&p(&path)));
    assert!(loaded.lib.is_empty());
}

/// The loader's time on the hostile inputs above, against a 2 s budget (debug or release). A performance
/// test: run it at milestone deliveries with
/// `cargo nextest run -p ew-format --test hostile --run-ignored only`.
#[test]
#[ignore = "perf: wall-clock timing, milestone deliveries only"]
fn hostile_inputs_load_within_the_budget() {
    const BUDGET: Duration = Duration::from_secs(2);
    let limits = Limits::default();
    let inputs: Vec<(&str, Vec<u8>)> = vec![
        (SHARD, format!("{}\n", "[".repeat(100_000)).into_bytes()),
        (SHARD, format!("{}\n", "{\"a\":".repeat(100_000)).into_bytes()),
        (
            "structure.jsonl",
            format!(
                r#"{{"$k":"frobnicator","id":"3b9c1d72-5e48-7a06-8f13-2d6c0e9a4b51","s":"{}"}}"#,
                "a".repeat(2 << 20)
            )
            .into_bytes(),
        ),
        (
            "lib/fndefs/5d1ab4de-706a-5b85-b463-8b15311c4f39.json",
            "[".repeat(100_000).into_bytes(),
        ),
    ];
    for (path, bytes) in inputs {
        let start = Instant::now();
        let _ = load_bytes(path, &bytes, &limits);
        assert!(start.elapsed() < BUDGET, "{path}: took {:?}", start.elapsed());
    }
}
