// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The generated JSON Schemas accept the example lines and reject what the readers reject.
//!
//! Every example line (doc 05 §3.5, the golden lines, the library documents) validates against the schema
//! of its kind and, for record lines, against `record.schema.json` with its sibling `$ref`s; negative
//! controls prove the validation bites; and each proxy schema of `src/proxy.rs` agrees with the serde
//! implementation of the type it stands for on accepted and rejected samples.
//!
//! The validator is a small structural one (below) rather than a crate: the candidates `jsonschema` and
//! `boon` bring 52 to 120 new crates into the dependency graph, far beyond what a dev-dependency of a
//! permissive crate should cost. It supports exactly the keywords the generated schemas use and panics on
//! any other, so a new keyword cannot be silently ignored. `pattern` uses the `regex` crate, whose syntax
//! agrees with ECMA-262 on the subset the proxies use (no look-around, no `\p{..}`; `\uXXXX` ranges).

mod common;

use std::cell::RefCell;
// FT-271: the compiled-pattern cache below is keyed lookup only; it is never iterated.
#[allow(clippy::disallowed_types)]
use std::collections::{BTreeMap, BTreeSet, HashMap};

use common::fill;
use common::lines::{DOC05_LINES, DOC05_PART, GOLDEN, GOLDEN_LIB};
use ew_schema::{Kind, KindStatus, kind_schema, schema_files};
use regex::Regex;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------------------------
// A minimal JSON Schema 2020-12 validator

/// Keywords that annotate or organise a schema without constraining an instance.
const ANNOTATIONS: &[&str] = &[
    "$schema",
    "$defs",
    "$comment",
    "title",
    "description",
    "default",
    "examples",
    "deprecated",
    "format",
];

struct Validator {
    /// The parsed schema files by file name, for `$ref`s between files.
    files: BTreeMap<String, Value>,
    // FT-271: pattern text to compiled regex, lookup only.
    #[allow(clippy::disallowed_types)]
    patterns: RefCell<HashMap<String, Regex>>,
}

impl Validator {
    fn new() -> Validator {
        let files = schema_files()
            .into_iter()
            .map(|(name, text)| (name, serde_json::from_str(&text).expect("schema file is JSON")))
            .collect();
        Validator {
            files,
            patterns: RefCell::default(),
        }
    }

    /// The problems of `instance` against `schema`, a document that is its own root for `#/…` references.
    fn errors(&self, schema: &Value, instance: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        self.check(schema, schema, instance, "", &mut errors);
        errors
    }

    fn is_valid(&self, schema: &Value, instance: &Value) -> bool {
        self.errors(schema, instance).is_empty()
    }

    fn matches(&self, pattern: &str, text: &str) -> bool {
        let mut cache = self.patterns.borrow_mut();
        let regex = cache
            .entry(pattern.to_owned())
            .or_insert_with(|| Regex::new(pattern).unwrap_or_else(|e| panic!("bad pattern {pattern}: {e}")));
        regex.is_match(text)
    }

    /// The root document and the schema that `reference` names: `#/$defs/X` inside `root`, or a sibling
    /// file such as `page.schema.json`.
    fn resolve<'a>(&'a self, root: &'a Value, reference: &str) -> (&'a Value, &'a Value) {
        if let Some(pointer) = reference.strip_prefix('#') {
            let target = root
                .pointer(pointer)
                .unwrap_or_else(|| panic!("unresolved reference {reference}"));
            (root, target)
        } else {
            let file = self
                .files
                .get(reference)
                .unwrap_or_else(|| panic!("unknown schema file {reference}"));
            (file, file)
        }
    }

    fn check(&self, root: &Value, schema: &Value, instance: &Value, at: &str, errors: &mut Vec<String>) {
        let map = match schema {
            Value::Bool(true) => return,
            Value::Bool(false) => {
                errors.push(format!("{at}: not allowed here"));
                return;
            }
            Value::Object(map) => map,
            other => panic!("schema is neither an object nor a boolean: {other}"),
        };
        let sub = |schema: &Value, instance: &Value, at: &str| {
            let mut found = Vec::new();
            self.check(root, schema, instance, at, &mut found);
            found
        };
        for (keyword, value) in map {
            match keyword.as_str() {
                "$ref" => {
                    let (new_root, target) = self.resolve(root, value.as_str().expect("$ref is a string"));
                    self.check(new_root, target, instance, at, errors);
                }
                "type" => {
                    let names: Vec<&str> = match value {
                        Value::String(name) => vec![name],
                        Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
                        other => panic!("bad type {other}"),
                    };
                    if !names.iter().any(|name| type_matches(name, instance)) {
                        errors.push(format!("{at}: expected {names:?}, found {instance}"));
                    }
                }
                "const" => {
                    if instance != value {
                        errors.push(format!("{at}: expected the constant {value}, found {instance}"));
                    }
                }
                "enum" => {
                    if !value.as_array().expect("enum is an array").contains(instance) {
                        errors.push(format!("{at}: {instance} is not one of {value}"));
                    }
                }
                "required" => {
                    if let Value::Object(members) = instance {
                        for name in value.as_array().expect("required is an array") {
                            let name = name.as_str().expect("required names are strings");
                            if !members.contains_key(name) {
                                errors.push(format!("{at}: missing required member {name}"));
                            }
                        }
                    }
                }
                "properties" => {
                    if let Value::Object(members) = instance {
                        for (name, schema) in value.as_object().expect("properties is an object") {
                            if let Some(member) = members.get(name) {
                                errors.extend(sub(schema, member, &format!("{at}/{name}")));
                            }
                        }
                    }
                }
                "additionalProperties" => {
                    if let Value::Object(members) = instance {
                        let declared = map.get("properties").and_then(Value::as_object);
                        for (name, member) in members {
                            if declared.is_none_or(|d| !d.contains_key(name)) {
                                errors.extend(sub(value, member, &format!("{at}/{name}")));
                            }
                        }
                    }
                }
                "propertyNames" => {
                    if let Value::Object(members) = instance {
                        for name in members.keys() {
                            errors.extend(sub(
                                value,
                                &Value::String(name.clone()),
                                &format!("{at}/<{name}>"),
                            ));
                        }
                    }
                }
                "prefixItems" => {
                    if let Value::Array(items) = instance {
                        for (i, schema) in value
                            .as_array()
                            .expect("prefixItems is an array")
                            .iter()
                            .enumerate()
                        {
                            if let Some(item) = items.get(i) {
                                errors.extend(sub(schema, item, &format!("{at}/{i}")));
                            }
                        }
                    }
                }
                "items" => {
                    if let Value::Array(items) = instance {
                        let prefix = map
                            .get("prefixItems")
                            .and_then(Value::as_array)
                            .map_or(0, Vec::len);
                        for (i, item) in items.iter().enumerate().skip(prefix) {
                            errors.extend(sub(value, item, &format!("{at}/{i}")));
                        }
                    }
                }
                "minItems" | "maxItems" => {
                    if let Value::Array(items) = instance {
                        check_count(keyword, value, items.len(), at, errors);
                    }
                }
                "minProperties" => {
                    if let Value::Object(members) = instance {
                        check_count(keyword, value, members.len(), at, errors);
                    }
                }
                "minLength" | "maxLength" => {
                    if let Value::String(text) = instance {
                        check_count(keyword, value, text.chars().count(), at, errors);
                    }
                }
                "minimum" | "maximum" => {
                    if instance.is_number() && !within(keyword == "minimum", value, instance) {
                        errors.push(format!("{at}: {instance} violates {keyword} {value}"));
                    }
                }
                "uniqueItems" => {
                    if let (Value::Array(items), Value::Bool(true)) = (instance, value) {
                        let duplicate = (0..items.len()).any(|i| items[..i].contains(&items[i]));
                        if duplicate {
                            errors.push(format!("{at}: the items are not unique"));
                        }
                    }
                }
                "pattern" => {
                    if let Value::String(text) = instance {
                        let pattern = value.as_str().expect("pattern is a string");
                        if !self.matches(pattern, text) {
                            errors.push(format!("{at}: {text:?} does not match {pattern}"));
                        }
                    }
                }
                "oneOf" | "anyOf" | "allOf" => {
                    let schemas = value.as_array().expect("combinator is an array");
                    let passing = schemas.iter().filter(|s| sub(s, instance, at).is_empty()).count();
                    let ok = match keyword.as_str() {
                        "oneOf" => passing == 1,
                        "anyOf" => passing >= 1,
                        _ => passing == schemas.len(),
                    };
                    if !ok {
                        errors.push(format!(
                            "{at}: {passing} of {} alternatives of {keyword} match",
                            schemas.len()
                        ));
                    }
                }
                other if ANNOTATIONS.contains(&other) => {}
                other => panic!("the test validator does not support the keyword {other}"),
            }
        }
    }
}

/// Whether `instance` has JSON Schema type `name`; `integer` is exactly a JSON number without fraction or
/// exponent, as the strict readers require (so `1.0` is not an integer here).
fn type_matches(name: &str, instance: &Value) -> bool {
    match name {
        "null" => instance.is_null(),
        "boolean" => instance.is_boolean(),
        "string" => instance.is_string(),
        "array" => instance.is_array(),
        "object" => instance.is_object(),
        "number" => instance.is_number(),
        "integer" => instance.as_i64().is_some() || instance.as_u64().is_some(),
        other => panic!("unknown type {other}"),
    }
}

fn check_count(keyword: &str, limit: &Value, count: usize, at: &str, errors: &mut Vec<String>) {
    let limit = limit.as_u64().expect("limit is an unsigned integer");
    let count = count as u64;
    let ok = if keyword.starts_with("max") {
        count <= limit
    } else {
        count >= limit
    };
    if !ok {
        errors.push(format!("{at}: {count} violates {keyword} {limit}"));
    }
}

/// `instance >= bound` (`lower`) or `instance <= bound`, exact for integers.
fn within(lower: bool, bound: &Value, instance: &Value) -> bool {
    let as_int = |v: &Value| v.as_i64().map(i128::from).or_else(|| v.as_u64().map(i128::from));
    if let (Some(b), Some(i)) = (as_int(bound), as_int(instance)) {
        return if lower { i >= b } else { i <= b };
    }
    let (b, i) = (
        bound.as_f64().unwrap_or(f64::NAN),
        instance.as_f64().unwrap_or(f64::NAN),
    );
    if lower { i >= b } else { i <= b }
}

// ---------------------------------------------------------------------------------------------------------
// The example lines

fn parse(line: &str) -> Value {
    serde_json::from_str(line).unwrap_or_else(|e| panic!("not JSON: {e}\n{line}"))
}

fn kind_of(line: &Value) -> Kind {
    let name = line["$k"].as_str().expect("line has $k");
    Kind::ALL
        .into_iter()
        .find(|k| k.name() == name)
        .unwrap_or_else(|| panic!("unknown kind {name}"))
}

/// Record lines (JSON Lines kinds) of the shared examples.
fn record_lines() -> Vec<Value> {
    DOC05_LINES
        .iter()
        .chain(GOLDEN)
        .map(|l| parse(&fill(l)))
        .collect()
}

/// Library documents of the shared examples.
fn library_lines() -> Vec<Value> {
    std::iter::once(&DOC05_PART)
        .chain(GOLDEN_LIB)
        .map(|l| parse(&fill(l)))
        .collect()
}

fn assert_valid(validator: &Validator, schema: &Value, line: &Value) {
    let errors = validator.errors(schema, line);
    assert!(errors.is_empty(), "{line}\n{}", errors.join("\n"));
}

#[test]
fn every_example_line_validates_against_the_schema_of_its_kind() {
    let validator = Validator::new();
    let mut covered = BTreeSet::new();
    for line in record_lines().iter().chain(&library_lines()) {
        let kind = kind_of(line);
        let schema = kind_schema(kind).unwrap_or_else(|| panic!("no schema for {kind:?}"));
        assert_valid(&validator, &schema, line);
        covered.insert(kind);
    }
    let implemented: BTreeSet<Kind> = Kind::ALL
        .into_iter()
        .filter(|k| k.status() == KindStatus::Implemented)
        .collect();
    assert_eq!(covered, implemented, "every implemented kind has an example line");
}

#[test]
fn record_lines_validate_against_record_schema_through_its_sibling_refs() {
    let validator = Validator::new();
    let record = validator.files["record.schema.json"].clone();
    let lines = record_lines();
    assert!(lines.len() >= 20);
    for line in &lines {
        assert_valid(&validator, &record, line);
    }
    // The `$ref`s are followed: a record with a bad member fails inside its kind's schema.
    let mut broken = lines[0].clone();
    broken["id"] = json!("not-a-uuid");
    assert!(!validator.is_valid(&record, &broken));
}

// ---------------------------------------------------------------------------------------------------------
// Negative controls

/// The first record line of `kind` from the shared examples.
fn sample(kind: Kind) -> Value {
    record_lines()
        .into_iter()
        .find(|l| kind_of(l) == kind)
        .unwrap_or_else(|| panic!("no example of {kind:?}"))
}

/// Asserts that `line` is valid, applies `mutate`, and asserts that the result is not.
fn assert_mutation_rejected(kind: Kind, what: &str, mutate: impl FnOnce(&mut Value)) {
    let validator = Validator::new();
    let schema = kind_schema(kind).expect("schema");
    let mut line = sample(kind);
    assert_valid(&validator, &schema, &line);
    mutate(&mut line);
    let errors = validator.errors(&schema, &line);
    assert!(!errors.is_empty(), "{what} must fail schema validation: {line}");
}

#[test]
fn a_float_where_an_integer_is_required_fails() {
    assert_mutation_rejected(Kind::StructureNode, "a float order", |l| l["order"] = json!(1.5));
    assert_mutation_rejected(Kind::StructureNode, "an integral float order", |l| {
        l["order"] = serde_json::from_str("1.0").unwrap();
    });
    assert_mutation_rejected(Kind::Representation, "a float coordinate", |l| {
        l["pos"][0] = json!(1.5)
    });
}

#[test]
fn a_missing_required_member_fails() {
    assert_mutation_rejected(Kind::StructureNode, "a missing code", |l| {
        l.as_object_mut().unwrap().remove("code");
    });
    assert_mutation_rejected(Kind::StructureNode, "a missing $k", |l| {
        l.as_object_mut().unwrap().remove("$k");
    });
}

#[test]
fn a_malformed_uuid_fails() {
    assert_mutation_rejected(Kind::StructureNode, "an upper-case UUID", |l| {
        l["id"] = json!(l["id"].as_str().unwrap().to_uppercase());
    });
    assert_mutation_rejected(Kind::StructureNode, "an unhyphenated UUID", |l| {
        l["id"] = json!(l["id"].as_str().unwrap().replace('-', ""));
    });
    assert_mutation_rejected(Kind::Representation, "a UUID with a trailing character", |l| {
        l["page"] = json!(format!("{}0", l["page"].as_str().unwrap()));
    });
}

#[test]
fn a_dec3_of_minus_zero_or_another_non_canonical_form_fails() {
    for bad in [
        "-0", "-0.0", "0.750", "+1", "01", "1e3", ".5", "5.", " 1", "1.2345",
    ] {
        assert_mutation_rejected(Kind::Device, bad, |l| l["parts"][0]["qty"] = json!(bad));
    }
}

#[test]
fn an_ident_with_leading_white_space_fails() {
    for bad in [
        " J-E-2416",
        "J-E-2416 ",
        "\tJ",
        "\u{00A0}J",
        "J\u{3000}",
        "J\nK",
        "J\u{0085}K",
        "",
    ] {
        assert_mutation_rejected(Kind::StructureNode, bad, |l| l["code"] = json!(bad));
    }
}

// ---------------------------------------------------------------------------------------------------------
// The proxy schemas against the serde implementations

/// The schema `$defs` entry the proxy `name` produced, taken from the generated kind schemas.
fn proxy_schema(name: &str) -> Value {
    Kind::ALL
        .into_iter()
        .filter_map(kind_schema)
        .find_map(|schema| schema["$defs"].get(name).cloned())
        .unwrap_or_else(|| panic!("no schema defines {name}"))
}

/// `serde_json::from_str::<T>` and the schema agree on every sample (each is JSON text).
fn assert_agreement<T: DeserializeOwned>(
    validator: &Validator,
    name: &str,
    accepted: &[&str],
    rejected: &[&str],
) {
    let schema = proxy_schema(name);
    for (samples, expected) in [(accepted, true), (rejected, false)] {
        for &text in samples {
            let instance = parse(text);
            assert_eq!(
                serde_json::from_str::<T>(text).is_ok(),
                expected,
                "{name}: serde on {text}"
            );
            assert_eq!(
                validator.is_valid(&schema, &instance),
                expected,
                "{name}: schema on {text}: {:?}",
                validator.errors(&schema, &instance)
            );
        }
    }
}

/// Samples the schema cannot reject although serde does (named in each proxy's description).
fn assert_documented_gap<T: DeserializeOwned>(validator: &Validator, name: &str, samples: &[&str]) {
    let schema = proxy_schema(name);
    for &text in samples {
        assert!(
            serde_json::from_str::<T>(text).is_err(),
            "{name}: serde accepts {text}"
        );
        assert!(
            validator.is_valid(&schema, &parse(text)),
            "{name}: the gap closed for {text}; update the description"
        );
    }
}

/// A JSON string literal of `text`.
fn lit(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}

#[test]
fn dec3_schema_agrees_with_the_reader() {
    let v = Validator::new();
    let accepted = [
        "0",
        "1",
        "-1",
        "40",
        "10",
        "100",
        "0.5",
        "-0.5",
        "0.75",
        "0.001",
        "-0.001",
        "2.5",
        "10.25",
        "1234.567",
        "9223372036854775.807",
        "-9223372036854775.808",
        "-9007199254740992",
    ];
    let rejected = [
        "-0", "-0.0", "-0.00", "0.0", "0.750", "0.50", "1.0", "00", "01", "-01", "007", "+1", "+0.5", "1e3",
        "1E3", ".5", "-.5", "5.", "0.", "1.2345", "0.0001", "1,5", "1 ", " 1", "", "-", "--1", "1.2.3", "٣",
        "1_000", "0x10", "NaN",
    ];
    let quote = |list: &[&str]| list.iter().map(|s| lit(s)).collect::<Vec<_>>();
    let (accepted, rejected) = (quote(&accepted), quote(&rejected));
    let (a, r): (Vec<&str>, Vec<&str>) = (
        accepted.iter().map(String::as_str).collect(),
        rejected.iter().map(String::as_str).collect(),
    );
    assert_agreement::<ew_units::Dec3>(&v, "Dec3", &a, &r);
    // JSON numbers are not Dec3 strings, in the reader or the schema.
    assert_agreement::<ew_units::Dec3>(&v, "Dec3", &[], &["1", "0.5", "null", "true", "[]"]);
    // The exact i64 bound is the reader's.
    assert_documented_gap::<ew_units::Dec3>(
        &v,
        "Dec3",
        &[
            "\"9223372036854775.808\"",
            "\"9999999999999999\"",
            "\"-9999999999999999.999\"",
        ],
    );
}

#[test]
fn ident_schema_agrees_with_the_reader() {
    let v = Validator::new();
    let accepted = [
        "-101F1",
        "J-E-2416",
        "PT 2.5-TWIN",
        "a",
        "é",
        "\u{0410}",
        "日本語",
        "😀",
        "a\u{2028}b",
        "a b",
        "\u{FEFF}x",
        "x\u{200B}",
        "\u{200B}x",
        "<project ref>",
    ];
    let rejected = [
        "",
        " ",
        " x",
        "x ",
        "\tx",
        "x\t",
        "x\n",
        "\nx",
        "\r",
        "a\tb",
        "a\nb",
        "a\u{0000}b",
        "a\u{001F}b",
        "a\u{007F}b",
        "a\u{0085}b",
        "a\u{009F}b",
        "\u{00A0}x",
        "x\u{00A0}",
        "\u{1680}x",
        "x\u{2000}",
        "x\u{200A}",
        "\u{2028}x",
        "x\u{2029}",
        "x\u{202F}",
        "\u{205F}x",
        "\u{3000}x",
        "x\u{3000}",
        "\u{000B}x",
        "x\u{000C}",
    ];
    let to_json = |list: &[&str]| list.iter().map(|s| lit(s)).collect::<Vec<_>>();
    let (accepted, rejected) = (to_json(&accepted), to_json(&rejected));
    let (a, r): (Vec<&str>, Vec<&str>) = (
        accepted.iter().map(String::as_str).collect(),
        rejected.iter().map(String::as_str).collect(),
    );
    assert_agreement::<ew_mltext::Ident>(&v, "Ident", &a, &r);
    assert_agreement::<ew_mltext::Ident>(&v, "Ident", &[], &["1", "null", "[]"]);
    // NFC is the reader's: "e" and a combining acute accent is not NFC.
    assert_documented_gap::<ew_mltext::Ident>(&v, "Ident", &[&lit("e\u{0301}")]);
}

#[test]
fn lang_tag_and_ml_text_schemas_agree_with_the_readers() {
    let v = Validator::new();
    let accepted_tags = [
        "en",
        "nl",
        "en-GB",
        "nl-NL",
        "de-DE",
        "sr-Latn-RS",
        "zh-cmn-Hans-CN",
        "es-419",
        "de-DE-1996",
        "en-US-x-private",
        "en-a-bbb-x-ccc",
        "qq-QQ",
        "haw",
        "abcd",
        "en-abc",
    ];
    let rejected_tags = [
        "",
        "e",
        "EN",
        "en-gb",
        "nl-nl",
        "en_GB",
        "en-GB-",
        "-en",
        "x-foo",
        "i-klingon",
        "en-GB-oed",
        "nl-NL1",
        "en-x",
        "en-a",
        "abcd-efg",
        "en-Latn-latn",
        "en GB",
        "en-GB-ab",
        "toolonglang",
    ];
    let to_json = |list: &[&str]| list.iter().map(|s| lit(s)).collect::<Vec<_>>();
    let (accepted, rejected) = (to_json(&accepted_tags), to_json(&rejected_tags));
    let (a, r): (Vec<&str>, Vec<&str>) = (
        accepted.iter().map(String::as_str).collect(),
        rejected.iter().map(String::as_str).collect(),
    );
    assert_agreement::<ew_mltext::LangTag>(&v, "LangTag", &a, &r);
    // Duplicate variants and extensions are the reader's to reject.
    assert_documented_gap::<ew_mltext::LangTag>(
        &v,
        "LangTag",
        &["\"en-GB-abcde-abcde\"", "\"en-a-bb-a-cc\""],
    );

    assert_agreement::<ew_mltext::MlText>(
        &v,
        "MlText",
        &[
            r#""1+""#,
            r#""""#,
            r#""Power supply""#,
            r#"{"en-GB":"Power"}"#,
            r#"{"en-GB":"Power","nl-NL":"Voeding"}"#,
            r#"{"sr-Latn-RS":"x"}"#,
        ],
        &[
            "{}",
            "1",
            "null",
            "[]",
            r#"{"en-GB":""}"#,
            r#"{"en-GB":1}"#,
            r#"{"en-gb":"x"}"#,
            r#"{"EN":"x"}"#,
            r#"{"en_GB":"x"}"#,
            r#"{"x-foo":"x"}"#,
            r#"{"":"x"}"#,
            r#"{"en-GB":"Power","nl-NL":""}"#,
        ],
    );
    // Key order and NFC are the reader's.
    assert_documented_gap::<ew_mltext::MlText>(
        &v,
        "MlText",
        &[
            r#"{"nl-NL":"a","en-GB":"b"}"#,
            "\"e\\u0301\"",
            "{\"en-GB\":\"e\\u0301\"}",
        ],
    );
}

#[test]
fn uuid_schema_agrees_with_the_reader() {
    let v = Validator::new();
    assert_agreement::<ew_ids::Id<()>>(
        &v,
        "Uuid",
        &[
            "\"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2\"",
            "\"00000000-0000-0000-0000-000000000000\"",
        ],
        &[
            "\"C7A90E35-42D8-5F61-8B03-9D2E6F1A47C2\"",
            "\"c7a90e35-42d8-5f61-8b03-9D2e6f1a47c2\"",
            "\"c7a90e3542d85f618b039d2e6f1a47c2\"",
            "\"{c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2}\"",
            "\"urn:uuid:c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2\"",
            "\" c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2\"",
            "\"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2\\n\"",
            "\"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c\"",
            "\"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2a\"",
            "\"c7a90e35_42d8-5f61-8b03-9d2e6f1a47c2\"",
            "\"g7a90e35-42d8-5f61-8b03-9d2e6f1a47c2\"",
            "\"\"",
            "1",
            "null",
        ],
    );
}

#[test]
fn integer_proxies_agree_with_the_readers() {
    let v = Validator::new();
    let max = (1_i64 << 53) - 1;
    let (over, under) = (max + 1, -max - 1);
    let (max, min) = (max.to_string(), (-(1_i64 << 53) + 1).to_string());
    let (over, under) = (over.to_string(), under.to_string());

    let accepted_nm = ["0", "-1", "137583333", max.as_str(), min.as_str()];
    let rejected_nm = [
        over.as_str(),
        under.as_str(),
        "1.5",
        "1.0",
        "1e3",
        "\"1\"",
        "null",
        "9223372036854775807",
    ];
    assert_agreement::<ew_units::Nm>(&v, "Nm", &accepted_nm, &rejected_nm);

    let accepted_pt = ["[0,0]", "[137583333,31750000]", "[-1,1]"];
    let over_pt = format!("[0,{over}]");
    let rejected_pt = [
        "[]",
        "[1]",
        "[1,2,3]",
        "[1.5,2]",
        "[\"1\",2]",
        "{}",
        "null",
        over_pt.as_str(),
    ];
    assert_agreement::<ew_units::Pt2>(&v, "Pt2", &accepted_pt, &rejected_pt);

    assert_agreement::<ew_units::Angle2>(
        &v,
        "Angle2",
        &["0", "90000", "-90000", "360000", "2147483647", "-2147483648"],
        &[
            "2147483648",
            "-2147483649",
            "1.5",
            "\"90\"",
            "null",
            "9007199254740991",
        ],
    );
}

#[test]
fn grid_orientation_and_aspect_schemas_agree_with_the_readers() {
    let v = Validator::new();
    assert_agreement::<ew_units::Grid>(
        &v,
        "Grid",
        &[
            r#"{"num":7937500,"den":3}"#,
            r#"{"num":4000000,"den":1}"#,
            r#"{"num":1,"den":1}"#,
            r#"{"num":9007199254740991,"den":1}"#,
        ],
        &[
            r#"{"num":0,"den":1}"#,
            r#"{"num":1,"den":0}"#,
            r#"{"num":-1,"den":1}"#,
            r#"{"num":1}"#,
            r#"{"den":1}"#,
            r#"{}"#,
            r#"{"num":1.5,"den":1}"#,
            r#"{"num":9007199254740992,"den":1}"#,
            r#"{"num":1,"den":1,"extra":1}"#,
            r#"[1,1]"#,
            r#""1/1""#,
        ],
    );
    // Reducedness and the 1 nm lower bound are the reader's.
    assert_documented_gap::<ew_units::Grid>(
        &v,
        "Grid",
        &[
            r#"{"num":2,"den":4}"#,
            r#"{"num":6,"den":3}"#,
            r#"{"num":1,"den":3}"#,
        ],
    );

    assert_agreement::<ew_units::Orient2>(
        &v,
        "Orient2",
        &[
            "\"r0\"", "\"r90\"", "\"r180\"", "\"r270\"", "\"m0\"", "\"m90\"", "\"m180\"", "\"m270\"",
        ],
        &["\"R0\"", "\"r45\"", "\"m\"", "\"\"", "0", "null"],
    );

    // The product aspect is the device tree, never a structure node, so the proxy omits it although
    // `Aspect` itself has it.
    assert_agreement::<ew_rds::Aspect>(
        &v,
        "Aspect",
        &[
            "\"functional_assignment\"",
            "\"function\"",
            "\"installation_site\"",
            "\"location\"",
            "\"document_kind\"",
            "\"user_defined\"",
            "\"type\"",
        ],
        &["\"Function\"", "\"\"", "\"aspect\"", "1", "null"],
    );
}
