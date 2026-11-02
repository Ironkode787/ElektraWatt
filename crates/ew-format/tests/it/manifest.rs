// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `manifest.json` (doc 05 §4): the exact text, the round trip and every rejection.

use ew_format::{FileHash, FilePath, Limits, Manifest, ManifestError, hash_text, sha256};
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;
use serde_json::json;

const PROJECT: &str = "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37";

fn path(s: &str) -> FilePath {
    FilePath::new(s).unwrap()
}

fn base() -> Manifest {
    Manifest::new("0.1", "ElektraWatt 0.0.0", PROJECT.parse().unwrap(), 1)
}

fn two_files() -> Manifest {
    let mut m = base();
    m.files
        .insert(path("project.jsonl"), FileHash::Sha256(sha256(b"project")));
    m.files
        .insert(path("devices/c2.jsonl"), FileHash::Sha256(sha256(b"devices")));
    m
}

/// A manifest text with the given `files` body and extra members, for the rejection tests.
fn text_with(generation: &str, files: &str, rest: &str) -> String {
    format!(
        "{{\"format\":\"0.1\",\"min_reader\":\"0.1\",\"generator\":\"g\",\"project\":\"{PROJECT}\",\
         \"generation\":{generation},\"files\":{files}{rest}}}"
    )
}

fn parse(text: &str) -> Result<Manifest, ManifestError> {
    Manifest::parse(text.as_bytes(), &Limits::default())
}

fn invalid_member(result: Result<Manifest, ManifestError>) -> String {
    match result {
        Err(ManifestError::Invalid { member, .. }) => member,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn to_text_is_exact() {
    let m = two_files();
    let expected = format!(
        "{{\n  \"format\": \"0.1\",\n  \"min_reader\": \"0.1\",\n  \"generator\": \"ElektraWatt 0.0.0\",\n  \
         \"project\": \"{PROJECT}\",\n  \"generation\": 1,\n  \"files\": {{\n    \
         \"devices/c2.jsonl\": \"{}\",\n    \"project.jsonl\": \"{}\"\n  }}\n}}\n",
        hash_text(&sha256(b"devices")),
        hash_text(&sha256(b"project")),
    );
    assert_eq!(m.to_text(), expected);
}

#[test]
fn to_text_merged_unstable_extra_and_empty_files() {
    let mut m = base();
    m.merged = true;
    m.unstable = true;
    m.extra.insert("zz".into(), json!([1, "x"]));
    m.extra.insert("aa".into(), json!({"b": 2}));
    let expected = format!(
        "{{\n  \"format\": \"0.1\",\n  \"min_reader\": \"0.1\",\n  \"generator\": \"ElektraWatt 0.0.0\",\n  \
         \"project\": \"{PROJECT}\",\n  \"generation\": 1,\n  \"files\": {{}},\n  \"merged\": true,\n  \
         \"unstable\": true,\n  \"aa\": {{\"b\":2}},\n  \"zz\": [1,\"x\"]\n}}\n"
    );
    // The value of an extra member is serde_json's compact text.
    assert_eq!(m.to_text(), expected);
    assert_eq!(parse(&m.to_text()), Ok(m));
}

#[test]
fn round_trip_with_every_member() {
    let mut m = two_files();
    m.generation = (1 << 53) - 1;
    m.merged = true;
    m.unstable = true;
    m.files.insert(path("pages/x.jsonl"), FileHash::Pending);
    m.extra.insert("zz".into(), json!({"zz": [1, "x"]}));
    m.extra.insert("future\u{e9}".into(), json!(null));
    let text = m.to_text();
    assert!(text.ends_with("}\n"));
    assert_eq!(parse(&text), Ok(m));
}

#[test]
fn rejects_conflict_markers() {
    for marker in ["<<<<<<< HEAD", "=======", ">>>>>>> theirs"] {
        let text = format!("{{\n  \"generation\": 1,\n{marker}\n  \"generation\": 2\n}}\n");
        assert_eq!(parse(&text), Err(ManifestError::ConflictMarkers), "{marker}");
    }
}

fn conflicted(text: &str) -> Option<u64> {
    Manifest::conflicted_generation(text.as_bytes(), &Limits::default())
}

#[test]
fn a_conflicted_manifest_gives_the_greatest_generation_of_its_sides() {
    // FT-97 (WP-S1.17 D6): git's two-way and diff3 hunks on the `generation` line; the next save writes one more.
    let two_way = format!(
        "{{\n  \"format\": \"0.1\",\n  \"project\": \"{PROJECT}\",\n<<<<<<< ours\n  \"generation\": 7,\n=======\n  \
         \"generation\": 9,\n>>>>>>> theirs\n  \"files\": {{}}\n}}\n"
    );
    assert_eq!(parse(&two_way), Err(ManifestError::ConflictMarkers));
    assert_eq!(conflicted(&two_way), Some(9));
    let diff3 = "{\n<<<<<<< ours\n  \"generation\": 12,\n||||||| base\n  \"generation\": 4,\n=======\n  \
                 \"generation\": 5,\n>>>>>>> theirs\n}\n";
    assert_eq!(conflicted(diff3), Some(12));
    // CRLF line ends (a checkout with `core.autocrlf`), and the last member without its comma.
    let crlf = "{\r\n<<<<<<< ours\r\n  \"generation\": 3\r\n=======\r\n  \"generation\" : 30\r\n>>>>>>> theirs\r\n}\r\n";
    assert_eq!(conflicted(crlf), Some(30));
    // The conflict is elsewhere: the shared generation line is both sides' generation.
    let elsewhere = "{\n  \"generation\": 6,\n  \"files\": {\n<<<<<<< ours\n    \"a.jsonl\": \"x\"\n=======\n    \
                     \"b.jsonl\": \"y\"\n>>>>>>> theirs\n  }\n}\n";
    assert_eq!(conflicted(elsewhere), Some(6));
}

#[test]
fn a_conflicted_manifest_without_a_usable_generation_line_gives_none() {
    // No marker: not a conflicted manifest (a broken one restarts at 0, as before).
    assert_eq!(conflicted("{\n  \"generation\": 9,\n  \"files\": {}\n}\n"), None);
    assert_eq!(conflicted("{\n  \"generation\": 9,\n"), None);
    // Markers without a generation line, or with numbers that are not generations.
    assert_eq!(
        conflicted("{\n<<<<<<< ours\n  \"files\": {}\n=======\n>>>>>>> theirs\n}\n"),
        None
    );
    for bad in [
        "-1",
        "1.5",
        "07",
        "\"9\"",
        "9007199254740992",
        "99999999999999999999",
        "",
        "9 9",
    ] {
        let text = format!("<<<<<<< ours\n  \"generation\": {bad},\n=======\n>>>>>>> theirs\n");
        assert_eq!(conflicted(&text), None, "{bad}");
    }
    // A generation inside another member's value is not a member line.
    let nested =
        "<<<<<<< ours\n  \"x\": {\"generation\": 40},\n=======\n  \"generation\": 2,\n>>>>>>> theirs\n";
    assert_eq!(conflicted(nested), Some(2));
    assert_eq!(
        conflicted("<<<<<<< ours\n  \"generation\": 9007199254740991\n>>>>>>> theirs\n"),
        Some(9_007_199_254_740_991),
        "2^53 - 1 is the largest generation"
    );
    // Not UTF-8, or larger than the manifest limit: nothing is read.
    let mut bytes = b"<<<<<<< ours\n  \"generation\": 5,\n\xff\n".to_vec();
    assert_eq!(Manifest::conflicted_generation(&bytes, &Limits::default()), None);
    bytes.truncate(bytes.len() - 2);
    let small = Limits {
        max_manifest_bytes: 8,
        ..Limits::default()
    };
    assert_eq!(Manifest::conflicted_generation(&bytes, &small), None);
    assert_eq!(
        Manifest::conflicted_generation(&bytes, &Limits::default()),
        Some(5)
    );
}

#[test]
fn a_new_manifest_needs_the_major_floor_of_its_format() {
    // FT-95 (WP-S1.17 D3): a manifest without files needs no more than its format's major floor; before 1.0 every step
    // is a major, so the floor of 0.1 is 0.1.
    assert_eq!(base().min_reader, "0.1");
    let id = PROJECT.parse().unwrap();
    assert_eq!(Manifest::new("1.3", "g", id, 1).min_reader, "1.0");
    assert_eq!(Manifest::new("0.2", "g", id, 1).min_reader, "0.2");
    assert_eq!(Manifest::new("x", "g", id, 1).min_reader, "x");
}

#[test]
fn rejects_duplicate_member_and_non_utf8_and_non_object() {
    let dup = text_with("1", "{}", ",\"generation\":2");
    assert!(matches!(parse(&dup), Err(ManifestError::Json { .. })));
    assert!(matches!(
        Manifest::parse(b"\xff\xfe{}", &Limits::default()),
        Err(ManifestError::Json { .. })
    ));
    assert!(matches!(parse("{"), Err(ManifestError::Json { .. })));
    assert_eq!(invalid_member(parse("[]")), "");
}

#[test]
fn rejects_bad_paths_and_hashes() {
    let good = hash_text(&sha256(b"x"));
    assert_eq!(
        invalid_member(parse(&text_with("1", &format!("{{\"../x\":\"{good}\"}}"), ""))),
        "files/../x"
    );
    let upper = good.to_uppercase().replace("SHA256", "sha256");
    assert_eq!(
        invalid_member(parse(&text_with(
            "1",
            &format!("{{\"a.jsonl\":\"{upper}\"}}"),
            ""
        ))),
        "files/a.jsonl"
    );
    let short = &good[..good.len() - 1];
    assert_eq!(
        invalid_member(parse(&text_with(
            "1",
            &format!("{{\"a.jsonl\":\"{short}\"}}"),
            ""
        ))),
        "files/a.jsonl"
    );
    assert_eq!(
        invalid_member(parse(&text_with("1", "{\"a.jsonl\":7}", ""))),
        "files/a.jsonl"
    );
    assert_eq!(
        invalid_member(parse(&text_with("1", "{\"a.jsonl\":\"md5:abc\"}", ""))),
        "files/a.jsonl"
    );
}

#[test]
fn pending_needs_merged() {
    let files = "{\"a.jsonl\":\"sha256:pending\"}";
    assert_eq!(invalid_member(parse(&text_with("1", files, ""))), "files/a.jsonl");
    assert_eq!(
        invalid_member(parse(&text_with("1", files, ",\"merged\":false"))),
        "files/a.jsonl"
    );
    let ok = parse(&text_with("1", files, ",\"merged\":true")).unwrap();
    assert_eq!(ok.files[&path("a.jsonl")], FileHash::Pending);
    assert!(ok.merged);
}

#[test]
fn rejects_bad_generation() {
    for generation in ["1.5", "-1", "9007199254740992", "1e3", "\"1\"", "null", "1.0"] {
        assert_eq!(
            invalid_member(parse(&text_with(generation, "{}", ""))),
            "generation",
            "{generation}"
        );
    }
    assert_eq!(
        parse(&text_with("9007199254740991", "{}", ""))
            .unwrap()
            .generation,
        (1 << 53) - 1
    );
    assert_eq!(parse(&text_with("0", "{}", "")).unwrap().generation, 0);
}

#[test]
fn rejects_missing_and_mistyped_members() {
    let full = text_with("1", "{}", "");
    let value: serde_json::Value = serde_json::from_str(&full).unwrap();
    for name in [
        "format",
        "min_reader",
        "generator",
        "project",
        "generation",
        "files",
    ] {
        let mut v = value.clone();
        v.as_object_mut().unwrap().remove(name);
        assert_eq!(invalid_member(parse(&v.to_string())), name, "missing {name}");
        let mut v = value.clone();
        v.as_object_mut().unwrap().insert(name.into(), json!([]));
        assert_eq!(invalid_member(parse(&v.to_string())), name, "mistyped {name}");
    }
    assert_eq!(
        invalid_member(parse(&text_with("1", "{}", ",\"merged\":\"yes\""))),
        "merged"
    );
    assert_eq!(
        invalid_member(parse(&text_with("1", "{}", ",\"unstable\":1"))),
        "unstable"
    );
    // A project ID that is not canonical (upper case).
    let text = full.replace(PROJECT, &PROJECT.to_uppercase());
    assert_eq!(invalid_member(parse(&text)), "project");
    // An extra member holding a float.
    assert_eq!(invalid_member(parse(&text_with("1", "{}", ",\"x\":1.5"))), "x");
}

#[test]
fn rejects_the_manifest_as_a_key() {
    let good = hash_text(&sha256(b"x"));
    assert_eq!(
        invalid_member(parse(&text_with(
            "1",
            &format!("{{\"manifest.json\":\"{good}\"}}"),
            ""
        ))),
        "files/manifest.json"
    );
}

#[test]
fn too_large_and_too_many_files() {
    let text = two_files().to_text();
    let limits = Limits {
        max_manifest_bytes: 50,
        ..Limits::default()
    };
    assert_eq!(
        Manifest::parse(text.as_bytes(), &limits),
        Err(ManifestError::TooLarge {
            len: text.len() as u64
        })
    );
    let limits = Limits {
        max_files: 1,
        ..Limits::default()
    };
    assert_eq!(
        Manifest::parse(text.as_bytes(), &limits),
        Err(ManifestError::TooManyFiles { count: 2 })
    );
    let limits = Limits {
        max_files: 2,
        ..Limits::default()
    };
    assert!(Manifest::parse(text.as_bytes(), &limits).is_ok());
}

fn arb_extra_value() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(json!(null)),
        any::<bool>().prop_map(|b| json!(b)),
        (-1_000_000i64..1_000_000).prop_map(|n| json!(n)),
        "[a-z\"\\\\ \u{e9}]{0,6}".prop_map(|s| json!(s)),
    ];
    leaf.prop_recursive(2, 8, 3, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..3).prop_map(serde_json::Value::Array),
            proptest::collection::btree_map("[a-z]{1,3}", inner, 0..3)
                .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
        ]
    })
}

fn arb_manifest() -> impl Strategy<Value = Manifest> {
    (
        "[0-9]\\.[0-9]",
        "[ -~]{0,12}",
        any::<[u8; 16]>(),
        0u64..(1 << 53),
        proptest::collection::btree_map("[a-z]{1,4}(/[a-z]{1,4}){0,2}\\.jsonl", any::<[u8; 32]>(), 0..5),
        any::<bool>(),
        any::<bool>(),
        proptest::collection::btree_map("x[a-z]{0,4}", arb_extra_value(), 0..3),
    )
        .prop_map(
            |(format, generator, id, generation, files, merged, unstable, extra)| {
                let mut m = Manifest::new(&format, &generator, uuid_from(id), generation);
                m.merged = merged;
                m.unstable = unstable;
                m.files = files
                    .into_iter()
                    .enumerate()
                    .map(|(i, (p, h))| {
                        let hash = if merged && i == 0 {
                            FileHash::Pending
                        } else {
                            FileHash::Sha256(h)
                        };
                        (path(&p), hash)
                    })
                    .collect();
                m.extra = extra;
                m
            },
        )
}

fn uuid_from(bytes: [u8; 16]) -> ew_format::ew_schema::AnyId {
    // The canonical lower-case hyphenated form of the 16 bytes.
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let text = format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    );
    text.parse().unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn parse_of_to_text_is_the_manifest(m in arb_manifest()) {
        let text = m.to_text();
        prop_assert!(text.ends_with('\n'));
        prop_assert_eq!(Manifest::parse(text.as_bytes(), &Limits::default()), Ok(m));
    }
}
