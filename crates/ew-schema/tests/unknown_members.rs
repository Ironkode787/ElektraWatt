// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The generic unknown-member test (doc 05 §3.1: every JSON object of the format keeps members it does not
//! know, written last in byte order): `"zz_unknown":1` is injected into every object of every example line
//! and document, one object at a time. A result that parses must write back byte for byte; one that does
//! not may only refuse at the injected object, and only objects that are not extensible may refuse.

mod common;

use std::collections::BTreeMap;

use common::fill;
use common::lines::{DOC05_LINES, DOC05_PART, GOLDEN, GOLDEN_LIB};
use ew_schema::{LibDocument, Record};

/// One JSON object found in a text.
struct Obj {
    /// Byte offset of its closing `}`.
    close: usize,
    /// Whether it has no members.
    empty: bool,
    /// Its JSON Pointer, relative to the text.
    pointer: String,
    /// The member name it is the value of (`None` for the root and for array elements).
    parent_key: Option<String>,
}

/// Scans `text` (one JSON value) and returns every object in it, in document order.
fn objects(text: &str) -> Vec<Obj> {
    struct Scanner<'a> {
        text: &'a [u8],
        pos: usize,
        out: Vec<Obj>,
    }
    impl Scanner<'_> {
        fn skip_ws(&mut self) {
            while self.text[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
        }

        /// Reads a string starting at the opening quote; returns its unescaped content.
        fn string(&mut self) -> String {
            let start = self.pos;
            assert_eq!(self.text[self.pos], b'"');
            self.pos += 1;
            let mut escaped = false;
            loop {
                let b = self.text[self.pos];
                self.pos += 1;
                match (escaped, b) {
                    (true, _) => escaped = false,
                    (false, b'\\') => escaped = true,
                    (false, b'"') => break,
                    _ => {}
                }
            }
            serde_json::from_slice(&self.text[start..self.pos]).unwrap()
        }

        fn value(&mut self, pointer: &str, parent_key: Option<&str>) {
            self.skip_ws();
            match self.text[self.pos] {
                b'{' => {
                    self.pos += 1;
                    let slot = self.out.len();
                    self.out.push(Obj {
                        close: 0,
                        empty: true,
                        pointer: pointer.to_owned(),
                        parent_key: parent_key.map(str::to_owned),
                    });
                    loop {
                        self.skip_ws();
                        if self.text[self.pos] == b'}' {
                            self.out[slot].close = self.pos;
                            self.pos += 1;
                            return;
                        }
                        if self.text[self.pos] == b',' {
                            self.pos += 1;
                            self.skip_ws();
                        }
                        self.out[slot].empty = false;
                        let key = self.string();
                        self.skip_ws();
                        assert_eq!(self.text[self.pos], b':');
                        self.pos += 1;
                        let child = format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1"));
                        self.value(&child, Some(&key));
                    }
                }
                b'[' => {
                    self.pos += 1;
                    let mut index = 0;
                    loop {
                        self.skip_ws();
                        if self.text[self.pos] == b']' {
                            self.pos += 1;
                            return;
                        }
                        if self.text[self.pos] == b',' {
                            self.pos += 1;
                        }
                        self.value(&format!("{pointer}/{index}"), None);
                        index += 1;
                    }
                }
                b'"' => {
                    self.string();
                }
                _ => {
                    while !matches!(self.text[self.pos], b',' | b']' | b'}')
                        && !self.text[self.pos].is_ascii_whitespace()
                    {
                        self.pos += 1;
                    }
                }
            }
        }
    }
    let mut scanner = Scanner {
        text: text.as_bytes(),
        pos: 0,
        out: Vec::new(),
    };
    scanner.value("", None);
    assert_eq!(scanner.pos, text.len(), "trailing text after the value");
    scanner.out
}

/// `line` with `"zz_unknown":1` as the last member of `object`.
fn inject(line: &str, object: &Obj) -> String {
    let member = if object.empty {
        r#""zz_unknown":1"#
    } else {
        r#","zz_unknown":1"#
    };
    format!("{}{member}{}", &line[..object.close], &line[object.close..])
}

#[derive(Default)]
struct Tally {
    survived: usize,
    rejected: usize,
    /// Member names (of the parent) of objects that took the member, with counts.
    survivors_by_key: BTreeMap<String, usize>,
    /// Member names of objects that refused it.
    rejecters_by_key: BTreeMap<String, usize>,
}

fn run(texts: &[(String, bool)], tally: &mut Tally) {
    for (text, library) in texts {
        let found = objects(text);
        assert!(!found.is_empty());
        for object in &found {
            let modified = inject(text, object);
            let result: Result<String, ew_schema::SchemaError> = if *library {
                LibDocument::from_json(&modified).map(|d| serde_json::to_string(&d).unwrap())
            } else {
                Record::from_json(&modified).map(|r| serde_json::to_string(&r).unwrap())
            };
            let key = object
                .parent_key
                .clone()
                .unwrap_or_else(|| "<root or element>".into());
            match result {
                Ok(written) => {
                    assert_eq!(
                        written, modified,
                        "an unknown member in the object at \"{}\" was lost or moved",
                        object.pointer
                    );
                    tally.survived += 1;
                    *tally.survivors_by_key.entry(key).or_default() += 1;
                }
                Err(err) => {
                    let path = err.path().unwrap_or_else(|| {
                        panic!(
                            "error without a path for the object at \"{}\": {err}\n{modified}",
                            object.pointer
                        )
                    });
                    let injected = format!("{}/zz_unknown", object.pointer);
                    // An object inside the payload of a known variant (`{"one_page": {"en-GB": …}}`, or a
                    // non-extensible enum value such as `pen` below `rect`) that cannot take the member makes
                    // the variant malformed; the error is reported at the enum value that wraps the payload.
                    let payload_of_variant = matches!(
                        &err,
                        ew_schema::SchemaError::MalformedVariant(m)
                            if object.pointer.starts_with(&format!("{}/{}", m.path, m.variant))
                    );
                    assert!(
                        path == object.pointer || path == injected || payload_of_variant,
                        "the error for the object at \"{}\" points at \"{path}\": {err}\n{modified}",
                        object.pointer
                    );
                    tally.rejected += 1;
                    *tally.rejecters_by_key.entry(key).or_default() += 1;
                }
            }
        }
    }
}

/// The counts observed when the test was written: 259 objects in the examples, 184 of them extensible. The
/// PLC lines added five refusing enum wrappers and payloads (`address`, `output`, `slot`, `binding`,
/// `comment`); the assembly line two multi-language `label`s, which are no extensible objects.
const MIN_SURVIVORS: usize = 184;
const MAX_REJECTED: usize = 82;

fn examples() -> Vec<(String, bool)> {
    let mut all: Vec<(String, bool)> = DOC05_LINES.iter().map(|l| (fill(l), false)).collect();
    all.extend(GOLDEN.iter().map(|l| (fill(l), false)));
    all.extend(GOLDEN_LIB.iter().map(|l| (fill(l), true)));
    all.push((fill(DOC05_PART), true));
    all
}

#[test]
fn the_scanner_finds_every_object_with_its_pointer() {
    let text = r#"{"a":{"b":[{"c":{}},1,"x{}",{"d\"":[]}]},"e/f":{}}"#;
    let found = objects(text);
    let got: Vec<(&str, bool)> = found.iter().map(|o| (o.pointer.as_str(), o.empty)).collect();
    assert_eq!(
        got,
        [
            ("", false),
            ("/a", false),
            ("/a/b/0", false),
            ("/a/b/0/c", true),
            ("/a/b/3", false),
            ("/e~1f", true)
        ]
    );
    let injected = inject(text, &found[3]);
    assert!(injected.contains(r#""c":{"zz_unknown":1}"#), "{injected}");
    assert!(serde_json::from_str::<serde_json::Value>(&inject(text, &found[0])).is_ok());
}

#[test]
fn an_unknown_member_survives_in_every_extensible_object() {
    let mut tally = Tally::default();
    run(&examples(), &mut tally);
    println!(
        "objects that keep the unknown member: {}, objects that refuse it: {}",
        tally.survived, tally.rejected
    );
    println!("refusing parents: {:?}", tally.rejecters_by_key);

    // A regression that turns survivals into rejections lowers the first count.
    assert!(
        tally.survived >= MIN_SURVIVORS,
        "only {} objects keep an unknown member (expected at least {MIN_SURVIVORS})",
        tally.survived
    );
    assert!(
        tally.rejected <= MAX_REJECTED,
        "{} objects refuse it",
        tally.rejected
    );

    // The objects whose members used to be dropped are among the survivors, all of them.
    for key in ["assigned", "bevel", "pass_through", "shrink_to_fit", "deprecated"] {
        assert!(
            tally.survivors_by_key.get(key).copied().unwrap_or(0) > 0,
            "no `{key}` object kept the unknown member"
        );
        assert!(
            !tally.rejecters_by_key.contains_key(key),
            "a `{key}` object refused the unknown member"
        );
    }
}
