// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The revision label format (doc 15 §5.2, FR-REV-002; WP-S1.18 D2, RL-2): the three forms' order and next label,
//! and the `revision_label` project setting (absent means `v_major_minor`).

use ew_schema::{LabelFormat, ProjectSettings};
use proptest::prelude::*;
use serde_json::{Value, json};

fn next(format: &LabelFormat, label: &str) -> Option<String> {
    format.next(label).map(|l| l.as_str().to_owned())
}

/// RL-2: after V2.0 is V3.0, after 02 is 03, after H is J (I and O are skipped), with the carries of each form.
#[test]
fn the_next_label_bumps_the_major_part() {
    let v = LabelFormat::VMajorMinor;
    assert_eq!(next(&v, "V2.0").as_deref(), Some("V3.0"));
    assert_eq!(next(&v, "V2.3").as_deref(), Some("V3.0"));
    assert_eq!(next(&v, "V9.0").as_deref(), Some("V10.0"));
    let n = LabelFormat::TwoDigit;
    assert_eq!(next(&n, "02").as_deref(), Some("03"));
    assert_eq!(next(&n, "09").as_deref(), Some("10"));
    assert_eq!(next(&n, "99").as_deref(), Some("100"));
    let a = LabelFormat::Letter;
    assert_eq!(next(&a, "H").as_deref(), Some("J"));
    assert_eq!(next(&a, "N").as_deref(), Some("P"));
    assert_eq!(next(&a, "Z").as_deref(), Some("AA"));
    assert_eq!(next(&a, "AZ").as_deref(), Some("BA"));
    assert_eq!(next(&a, "ZZ").as_deref(), Some("AAA"));
}

/// Labels outside the format have no order and no next label: "2" under `V{major}.{minor}` (doc 15 §5.2), leading
/// zeros, a lower-case letter, I and O, a one-digit number under `{nn}`.
#[test]
fn labels_out_of_the_format_have_no_order() {
    for (format, label) in [
        (LabelFormat::VMajorMinor, "2"),
        (LabelFormat::VMajorMinor, "V2"),
        (LabelFormat::VMajorMinor, "v2.0"),
        (LabelFormat::VMajorMinor, "V02.0"),
        (LabelFormat::VMajorMinor, "V2.00"),
        (LabelFormat::VMajorMinor, "V2.0.1"),
        (LabelFormat::VMajorMinor, "V.0"),
        (LabelFormat::TwoDigit, "2"),
        (LabelFormat::TwoDigit, "002"),
        (LabelFormat::TwoDigit, "0x"),
        (LabelFormat::Letter, ""),
        (LabelFormat::Letter, "I"),
        (LabelFormat::Letter, "O"),
        (LabelFormat::Letter, "a"),
        (LabelFormat::Letter, "A1"),
        (LabelFormat::Unknown(json!("roman")), "IV"),
    ] {
        assert!(format.order_key(label).is_none(), "{format:?} ordered {label:?}");
        assert!(
            format.next(label).is_none(),
            "{format:?} proposed after {label:?}"
        );
    }
}

/// The natural order of each form: numeric parts compare as numbers, and longer letter labels follow shorter ones.
#[test]
fn the_order_is_natural_within_the_format() {
    let key = |f: &LabelFormat, l: &str| f.order_key(l).expect(l);
    let v = LabelFormat::VMajorMinor;
    assert!(key(&v, "V2.0") < key(&v, "V2.1"));
    assert!(key(&v, "V2.9") < key(&v, "V10.0"));
    let n = LabelFormat::TwoDigit;
    assert!(key(&n, "09") < key(&n, "10"));
    assert!(key(&n, "99") < key(&n, "100"));
    let a = LabelFormat::Letter;
    assert!(key(&a, "H") < key(&a, "J"));
    assert!(key(&a, "Z") < key(&a, "AA"));
}

/// The next label after a set of labels: after the greatest label in the format, labels out of it passed over; the
/// first label when none is in the format; nothing for a format of a newer build.
#[test]
fn the_next_label_after_existing_labels() {
    let after = |f: &LabelFormat, labels: &[&str]| {
        f.next_after(labels.iter().copied())
            .map(|l| l.as_str().to_owned())
    };
    let v = LabelFormat::VMajorMinor;
    assert_eq!(after(&v, &["V1.0", "V2.0", "2", "V1.5"]).as_deref(), Some("V3.0"));
    assert_eq!(after(&v, &["V10.0", "V9.0"]).as_deref(), Some("V11.0"));
    assert_eq!(after(&v, &[]).as_deref(), Some("V1.0"));
    assert_eq!(after(&v, &["Rev 7"]).as_deref(), Some("V1.0"));
    assert_eq!(after(&LabelFormat::TwoDigit, &[]).as_deref(), Some("01"));
    assert_eq!(after(&LabelFormat::Letter, &["Y", "Z"]).as_deref(), Some("AA"));
    assert_eq!(after(&LabelFormat::Unknown(json!("roman")), &["V1.0"]), None);
}

/// The setting: absent means `v_major_minor`, so a project without it (Golden A) reads as before; the three forms
/// round-trip; a form of a newer build is kept verbatim.
#[test]
fn the_label_format_setting_round_trips() {
    let empty: ProjectSettings = serde_json::from_str("{}").unwrap();
    assert_eq!(empty.revision_label, None);
    assert_eq!(empty.revision_label_format(), LabelFormat::VMajorMinor);
    assert_eq!(serde_json::to_string(&empty).unwrap(), "{}");
    for (text, format) in [
        ("v_major_minor", LabelFormat::VMajorMinor),
        ("two_digit", LabelFormat::TwoDigit),
        ("letter", LabelFormat::Letter),
        ("roman", LabelFormat::Unknown(Value::String("roman".into()))),
    ] {
        let line = format!(r#"{{"revision_label":"{text}"}}"#);
        let settings: ProjectSettings = serde_json::from_str(&line).unwrap();
        assert_eq!(settings.revision_label_format(), format);
        assert_eq!(serde_json::to_string(&settings).unwrap(), line);
    }
}

/// The `n`-th label of a form, built independently of the reader: `V{n}.0`, `{n:02}`, and the letters without I and
/// O counted like spreadsheet columns.
fn nth(format: &LabelFormat, n: u64) -> String {
    match format {
        LabelFormat::VMajorMinor => format!("V{n}.0"),
        LabelFormat::TwoDigit => format!("{n:02}"),
        _ => {
            let letters: Vec<char> = ('A'..='Z').filter(|c| *c != 'I' && *c != 'O').collect();
            let mut out = String::new();
            let mut rest = n;
            while rest > 0 {
                out.insert(0, letters[usize::try_from((rest - 1) % 24).unwrap()]);
                rest = (rest - 1) / 24;
            }
            out
        }
    }
}

fn formats() -> impl Strategy<Value = LabelFormat> {
    prop_oneof![
        Just(LabelFormat::VMajorMinor),
        Just(LabelFormat::TwoDigit),
        Just(LabelFormat::Letter),
    ]
}

proptest! {
    /// The proposal is the next label of the form and always follows its predecessor in the format's order.
    #[test]
    fn the_proposal_is_the_successor(format in formats(), n in 1u64..1_000_000) {
        let label = nth(&format, n);
        let proposed = next(&format, &label).expect("a proposal");
        prop_assert_eq!(&proposed, &nth(&format, n + 1));
        prop_assert!(format.order_key(&proposed) > format.order_key(&label));
    }

    /// Order keys agree with the numbers the labels count, so sorting by key sorts labels naturally.
    #[test]
    fn order_keys_follow_the_count(format in formats(), a in 1u64..100_000, b in 1u64..100_000) {
        let (ka, kb) = (format.order_key(&nth(&format, a)), format.order_key(&nth(&format, b)));
        prop_assert_eq!(ka.cmp(&kb), a.cmp(&b));
    }
}
