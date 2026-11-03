// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Dutch UI's fix round (cycle 49, ID-1278): the palette form's labels and the finding detail's kind from keys in
//! both locales (rulings 1 and 3), the count texts (ruling 6) and a new terminal strip in words (ruling 7).

use std::collections::BTreeSet;

use crate::common::{Tmp, done};
use ew_viewmodel::actions;
use ew_viewmodel::ew_api::checks::RuleKind;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::navigators::messages::FindingDetail;
use ew_viewmodel::palette::{FormPlan, PaletteField, form_plan};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use ew_viewmodel::{count_key, strings};
use serde_json::json;

/// The keys of the shipped Fluent file of `locale`.
fn keys(locale: &str) -> BTreeSet<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("locales")
        .join(locale)
        .join("shell.ftl");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.to_owned()))
        .collect()
}

/// The keys of `wanted` that en-GB or nl-NL lacks, each as `<locale>: <key>`.
fn missing(wanted: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for locale in ["en-GB", "nl-NL"] {
        let have = keys(locale);
        out.extend(
            wanted
                .iter()
                .filter(|k| !have.contains(*k))
                .map(|k| format!("{locale}: {k}")),
        );
    }
    out
}

/// A parameter's label as it was derived before ruling 1: the name with `_` as spaces, its first letter upper case.
fn derived(name: &str) -> String {
    let text = name.replace('_', " ");
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Ruling 1 (W49.1 C): every parameter of every action the palette opens a form for has its `param-<name>` label in
/// en-GB and nl-NL; the en-GB label is the one derived before, the Dutch one its own.
#[test]
fn every_palette_form_parameter_has_a_label_in_both_locales() {
    let mut wanted = BTreeSet::new();
    let mut fields = Vec::new();
    for meta in actions::ACTIONS {
        // Define potential… opens its form itself, with the action's parameters (FT-1202).
        let specs = match form_plan(meta) {
            FormPlan::Form(specs) => Some(specs),
            _ if meta.id == "potential.draw" => actions::parse_signature(meta.params).ok(),
            _ => None,
        };
        if let Some(specs) = specs {
            for p in specs {
                wanted.insert(format!("param-{}", p.name.replace('_', "-")));
                fields.push(PaletteField {
                    name: p.name,
                    ty: p.ty,
                    optional: p.optional,
                    value: String::new(),
                });
            }
        }
    }
    assert_eq!(missing(&wanted), Vec::<String>::new());
    for f in &fields {
        assert_eq!(f.label(), derived(&f.name), "{}", f.name);
    }
    let voltage = fields.iter().find(|f| f.name == "voltage").unwrap();
    assert!(strings::set_locale("nl"));
    assert_eq!(voltage.label(), "Spanning");
    assert!(strings::set_locale("en"));
}

/// The kinds of rule, listed by an exhaustive match: a new kind fails to compile here until it has its key.
fn kinds() -> [RuleKind; 5] {
    let all = [
        RuleKind::Safety,
        RuleKind::Conflict,
        RuleKind::Completeness,
        RuleKind::Hygiene,
        RuleKind::Integrity,
    ];
    for k in all {
        match k {
            RuleKind::Safety
            | RuleKind::Conflict
            | RuleKind::Completeness
            | RuleKind::Hygiene
            | RuleKind::Integrity => {}
        }
    }
    all
}

/// Ruling 3 (W49.1 F): every rule kind has its `finding-kind-<kind>` in en-GB and nl-NL, and the detail's line of a
/// completeness error reads in the UI language with both words mid-sentence; in en-GB as before.
#[test]
fn the_finding_detail_names_kind_and_severity_in_words() {
    let wanted = kinds()
        .iter()
        .map(|k| format!("finding-kind-{}", k.name()))
        .collect();
    assert_eq!(missing(&wanted), Vec::<String>::new());
    let detail = FindingDetail {
        rule: "EW-CORE-041".to_owned(),
        title: String::new(),
        kind: "completeness".to_owned(),
        severity: "error".to_owned(),
        message: String::new(),
        fix: None,
        fingerprint: String::new(),
        waived: false,
        waiver: None,
    };
    assert_eq!(detail.kind_line(), "Kind: completeness · severity: error");
    assert!(strings::set_locale("nl"));
    assert_eq!(detail.kind_line(), "Soort: volledigheid · ernst: fout");
    assert!(strings::set_locale("en"));
}

/// Ruling 6 (W49.1 K): a count of one takes its own text in both languages.
#[test]
fn a_count_of_one_reads_in_the_singular() {
    let cases = [
        (count_key!("find-results"), "1 result", "1 resultaat", "2 results"),
        (
            count_key!("label-paste"),
            "paste 1 record",
            "1 record plakken",
            "paste 2 records",
        ),
        (
            count_key!("find-updated-fewer"),
            "Preview updated: 1 fewer result",
            "Voorbeeld bijgewerkt: 1 resultaat minder",
            "Preview updated: 2 fewer results",
        ),
        (
            count_key!("find-updated-more"),
            "Preview updated: 1 more result",
            "Voorbeeld bijgewerkt: 1 resultaat meer",
            "Preview updated: 2 more results",
        ),
    ];
    for (key, one, een, two) in cases {
        assert_eq!(strings::format_count(key, 1), one);
        assert_eq!(strings::format_count(key, 2), two);
        assert!(strings::set_locale("nl"));
        assert_eq!(strings::format_count(key, 1), een);
        assert!(strings::set_locale("en"));
    }
    assert_eq!(
        ew_viewmodel::find::updated_note(3, 2),
        "Preview updated: 1 fewer result"
    );
}

/// Ruling 7 (W49.1 L): right after Terminal strip…, the Terminal strips tree names the new strip in words, not by its
/// underived tag, and the table, whose rows are terminals, says there is none yet and how to place one.
#[test]
fn a_new_strip_without_a_terminal_reads_in_words() {
    let tmp = Tmp::new("l49");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut shell,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut shell,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Control"}),
    );
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    done(&mut shell, "strip.new", json!({}));
    done(&mut shell, "navigator.show", json!({"kind": "strips"}));
    let tree: Vec<(String, String)> = shell
        .shown_rows(NavKind::Strips)
        .into_iter()
        .map(|r| (r.label, r.name))
        .collect();
    assert_eq!(
        tree,
        [(
            "New terminal strip · no tag yet".to_owned(),
            "New terminal strip · no tag yet, strip".to_owned()
        )]
    );
    assert_eq!(shell.strips_empty(), None, "the tree shows the strip");
    done(&mut shell, "navigator.table", json!({"kind": "strips"}));
    assert_eq!(shell.shown_rows(NavKind::Strips).len(), 0);
    assert_eq!(
        shell.strips_empty(),
        Some((
            "No terminals yet".to_owned(),
            "Place a terminal with Draw ▸ Symbol… and the terminal symbol; it goes into the strip you choose there."
                .to_owned()
        ))
    );
    assert!(strings::set_locale("nl"));
    assert_eq!(
        shell.strips_empty().map(|(line, _)| line).as_deref(),
        Some("Nog geen klemmen")
    );
    assert!(strings::set_locale("en"));
    // The strip picker's row (`ew-ui`) names it by the same words.
    assert_eq!(
        ew_viewmodel::navigators::strip_label("-X??"),
        "New terminal strip · no tag yet"
    );
    assert_eq!(ew_viewmodel::navigators::strip_label("-X1"), "-X1");
    shell.shutdown();
}
