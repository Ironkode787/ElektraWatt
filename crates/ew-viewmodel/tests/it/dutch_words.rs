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
use ew_viewmodel::{count_key, key, strings};
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
        // C49-p6#2: the paste ghost's dropped connections, the open banner's findings, the approval's waivers.
        (
            count_key!("label-paste-dropped"),
            ", 1 connection dropped",
            ", 1 verbinding weggelaten",
            ", 2 connections dropped",
        ),
        (
            count_key!("banner-findings"),
            "1 integrity finding was found when the project was opened.",
            "Bij het openen van het project is 1 integriteitsbevinding gevonden.",
            "2 integrity findings were found when the project was opened.",
        ),
        (
            count_key!("revision-waivers"),
            "Approving also acknowledges 1 safety waiver in force.",
            "Goedkeuren erkent ook 1 geldende veiligheidsvrijstelling.",
            "Approving also acknowledges 2 safety waivers in force.",
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

/// C49-p6#4 (ID-1278 ruling 9): Dutch puts the action's name mid-sentence through the Fluent function `IN_SENTENCE`;
/// en-GB keeps the title as it is.
#[test]
fn the_preview_chip_puts_the_action_mid_sentence_in_dutch() {
    let chip = || strings::format(key!("preview-chip"), &[strings::command_title("device.place")]);
    assert_eq!(chip(), "Preview of Place device");
    assert!(strings::set_locale("nl"));
    assert_eq!(chip(), "Voorbeeld: apparaat plaatsen");
    assert_eq!(
        strings::format(
            key!("preview-chip-failed"),
            &[strings::command_title("device.place"), "x"]
        ),
        "Voorbeeld: apparaat plaatsen mislukt: x"
    );
    assert!(strings::set_locale("en"));
}

/// The navigator `kind`'s table of the window.
fn table(shell: &ShellModel, kind: NavKind) -> std::sync::Arc<ew_viewmodel::ew_api::output::NavTable> {
    shell
        .nav(kind)
        .and_then(|n| n.table())
        .cloned()
        .expect("the navigator's table")
}

/// FT-1264 and FT-1289 (ID-1315 ruling 1): every navigator's column header has its text in both locales, the en-GB one
/// the label `ew-api` gives (`column-<navigator>-<key>`, else `column-<key>`); in nl-NL the window's tables show the
/// Dutch headers and the Devices tree its badges in Dutch.
#[test]
fn ft1264_every_navigator_column_and_badge_reads_in_both_locales() {
    let (session, _) =
        ew_viewmodel::ew_api::Session::open_read_only(std::path::Path::new(crate::common::GOLDEN_A), "test")
            .unwrap();
    let mut wanted = BTreeSet::new();
    let mut wrong = Vec::new();
    for kind in NavKind::ALL {
        for c in session.navigator_table(kind).columns {
            let key = c.key.replace('_', "-");
            let own = format!("column-{}-{key}", kind.word());
            let key = if strings::has_key(&own) {
                own
            } else {
                format!("column-{key}")
            };
            if strings::lookup(&key) != Some(c.label.as_str()) {
                wrong.push(format!("{key}: {:?} is not {:?}", strings::lookup(&key), c.label));
            }
            wanted.insert(key);
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
    assert_eq!(missing(&wanted), Vec::<String>::new());
    let tmp = Tmp::new("ft1264");
    let mut shell = tmp.shell("102");
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    let english: Vec<String> = shell
        .shown_rows(NavKind::Devices)
        .iter()
        .map(|r| r.text.clone())
        .collect();
    done(&mut shell, "view.language", json!({"value": "nl"}));
    let labels: Vec<String> = table(&shell, NavKind::Devices)
        .columns
        .iter()
        .map(|c| c.label.clone())
        .collect();
    assert_eq!(
        labels,
        ["Aanduiding", "Soort", "Functietekst", "Artikel", "Locatie (+)"]
    );
    let dutch: Vec<String> = shell
        .shown_rows(NavKind::Devices)
        .iter()
        .map(|r| r.text.clone())
        .collect();
    let worded: Vec<String> = english
        .iter()
        .map(|t| {
            t.replace(" [main]", " [hoofdfunctie]")
                .replace(" [aux]", " [hulpfunctie]")
                .replace(" [spare]", " [reserve]")
                .replace(" [unplaced]", " [niet geplaatst]")
                .replace(" [frozen]", " [vastgezet]")
        })
        .collect();
    assert!(
        english.iter().any(|t| t.ends_with(" [main]")) && english.iter().any(|t| t.ends_with(" [spare]")),
        "{english:?}"
    );
    assert_eq!(dutch, worded);
    shell.shutdown();
}

/// FT-1162 (ID-1219 ruling 6, ID-1315 ruling 1): in nl-NL the Messages pane's group headers, its rule groups and its
/// finding rows read in Dutch, the messages as the detail renders them, and so does the Properties pane's finding chip.
#[test]
fn ft1162_the_messages_rows_and_the_chip_read_in_dutch() {
    use ew_viewmodel::ew_api::checks::{CheckMode, messages};
    use ew_viewmodel::ew_api::output::NavRole;
    let tmp = Tmp::new("ft1162");
    let mut shell = tmp.shell("102");
    let (session, _) = ew_viewmodel::ew_api::Session::open_read_only(&tmp.golden_copy(), "test").unwrap();
    let report = session.check(session.online_scheme(), CheckMode::Online);
    done(&mut shell, "view.language", json!({"value": "nl"}));
    done(&mut shell, "messages.show", json!({}));
    let t = table(&shell, NavKind::Messages);
    let groups: Vec<&str> = t
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Severity && n.key.starts_with("severity:"))
        .map(|n| n.label.as_str())
        .collect();
    let mut expected_groups = Vec::new();
    for (severity, word) in [
        (ew_viewmodel::ew_api::checks::Severity::Error, "Fouten"),
        (ew_viewmodel::ew_api::checks::Severity::Warning, "Waarschuwingen"),
        (ew_viewmodel::ew_api::checks::Severity::Info, "Info"),
    ] {
        if report.findings.iter().any(|f| f.severity == severity) {
            expected_groups.push(word);
        }
    }
    assert_eq!(groups, expected_groups);
    let mut rows = Vec::new();
    let mut expected = Vec::new();
    for n in t.nodes.iter().filter(|n| n.role == NavRole::Finding) {
        let fp = n.key.rsplit_once("finding:").expect("a finding key").1;
        let Some(f) = report.findings.iter().find(|f| f.fingerprint.to_string() == fp) else {
            continue;
        };
        let text = messages::render(f, "nl-NL");
        let leads = text
            .strip_prefix(f.subject.as_str())
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(':') || rest.starts_with(' '));
        let shown = if f.subject.is_empty() || leads {
            text
        } else {
            format!("{}: {text}", f.subject)
        };
        rows.push(n.label.clone());
        expected.push(shown);
    }
    assert!(!rows.is_empty());
    assert_eq!(rows, expected);
    // C50-p5#7: one row as the Dutch reader sees it.
    let cable = "=P01+J-E-2416-3W1 heeft geen gerouteerde, handmatige of artikellengte";
    assert_eq!(rows.iter().filter(|r| *r == cable).count(), 1, "{rows:?}");
    assert!(
        report
            .findings
            .iter()
            .any(|f| messages::render(f, "nl-NL") != f.message),
        "a Dutch message to show"
    );
    // The chip of a record with an open finding.
    let f = report
        .findings
        .iter()
        .find(|f| {
            f.state == ew_viewmodel::ew_api::checks::FindingState::Open
                && !f.targets.is_empty()
                && messages::render(f, "nl-NL") != f.message
        })
        .expect("an open finding with a Dutch message on a record");
    let target = f.targets[0].id;
    done(
        &mut shell,
        "select.set",
        json!({ "ids": [target.as_uuid().to_string()] }),
    );
    let chips: BTreeSet<String> = shell
        .properties()
        .sheet()
        .expect("its sheet")
        .header
        .findings
        .iter()
        .filter(|c| c.rule == f.rule)
        .map(|c| c.message.clone())
        .collect();
    let dutch: BTreeSet<String> = report
        .findings
        .iter()
        .filter(|g| {
            g.rule == f.rule
                && g.state == ew_viewmodel::ew_api::checks::FindingState::Open
                && g.targets.iter().any(|t| t.id == target)
        })
        .map(|g| messages::render(g, "nl-NL"))
        .collect();
    assert_eq!(chips, dutch);
    shell.shutdown();
}

/// The value text and the JSON of row `key` of the window's property sheet.
fn row_value(shell: &ShellModel, key: &str) -> (String, serde_json::Value) {
    let sheet = shell.properties().sheet().expect("a sheet");
    let row = sheet
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no row {key}"));
    let json = match &row.value {
        ew_viewmodel::properties::RowValue::One { json, .. } => json.clone(),
        other => panic!("{key}: {other:?}"),
    };
    (ew_viewmodel::properties::value_text(row), json)
}

/// FT-1287 (ID-1315 ruling 3): a generated page's Properties show its Kind and its row source in words, in en-GB and
/// nl-NL, never the JSON; the JSON values stay.
#[test]
fn ft1287_a_generated_page_shows_its_kind_and_source_in_words() {
    let tmp = Tmp::new("ft1287");
    let mut shell = tmp.shell("2");
    let mut said = Vec::new();
    for lang in ["en", "nl"] {
        done(&mut shell, "view.language", json!({ "value": lang }));
        let (kind, kind_json) = row_value(&shell, "report_block/derived.kind");
        let (source, source_json) = row_value(&shell, "report_block/source");
        assert_eq!(
            (kind_json, source_json),
            (json!("report"), json!({"kind": "toc"}))
        );
        let sheet = shell.properties().sheet().expect("a sheet");
        let labels: Vec<String> = sheet
            .sections
            .iter()
            .flat_map(|s| s.rows.iter().map(move |r| (s, r)))
            .filter(|(_, r)| r.key == "report_block/derived.kind" || r.key == "report_block/source")
            .map(|(s, r)| ew_viewmodel::properties::row_label(s, r))
            .collect();
        said.push((labels, kind, source));
    }
    assert_eq!(
        said,
        [
            (
                vec!["Kind".to_owned(), "Source".to_owned()],
                "Report".to_owned(),
                "Table of contents".to_owned()
            ),
            (
                vec!["Soort".to_owned(), "Bron".to_owned()],
                "Rapport".to_owned(),
                "Inhoudsopgave".to_owned()
            ),
        ]
    );
    shell.shutdown();
}

/// FT-1258 and FT-1240 (ID-1315): an enumerated row shows its value and its choices in the UI language's words, takes
/// a word typed in them, and refuses another word naming the choices in words, in en-GB and nl-NL.
#[test]
fn ft1258_an_enumerated_row_reads_and_takes_its_words() {
    let tmp = Tmp::new("ft1258");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/state", "value": "Spare"}),
    );
    assert_eq!(
        row_value(&shell, "device/state"),
        ("Spare".to_owned(), json!("spare"))
    );
    let refusal = |shell: &mut ShellModel| match shell
        .dispatch("props.set", &json!({"row": "device/state", "value": "xx"}))
    {
        ew_viewmodel::shell::Outcome::Failed { message } => message,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        refusal(&mut shell),
        "\"xx\" is not one of the choices: Used, Spare, Reserved, Not fitted."
    );
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert_eq!(
        row_value(&shell, "device/state"),
        ("Reserve".to_owned(), json!("spare"))
    );
    done(&mut shell, "props.edit", json!({"row": "device/state"}));
    let options: Vec<String> = shell
        .properties()
        .choices()
        .expect("the combo's options")
        .items
        .iter()
        .map(|(t, _)| t.clone())
        .collect();
    assert_eq!(options, ["Gebruikt", "Reserve", "Gereserveerd", "Niet geplaatst"]);
    done(&mut shell, "props.cancel", json!({}));
    assert_eq!(
        refusal(&mut shell),
        "\"xx\" is geen van de keuzes: Gebruikt, Reserve, Gereserveerd, Niet geplaatst."
    );
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/state", "value": "gereserveerd"}),
    );
    assert_eq!(
        row_value(&shell, "device/state"),
        ("Gereserveerd".to_owned(), json!("reserved"))
    );
    shell.shutdown();
}

/// C50-p5#7 (FT-1287): every report kind of the format has its `report-kind-<kind>` text in en-GB and nl-NL, so no
/// generated page's Source shows JSON.
#[test]
fn every_report_kind_has_its_words_in_both_locales() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/0.1/report_block.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let kinds: Vec<String> = schema["$defs"]["ReportKind"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|k| k["const"].as_str().map(str::to_owned))
        .collect();
    assert!(kinds.iter().any(|k| k == "wire_list"), "{kinds:?}");
    let wanted: BTreeSet<String> = kinds
        .iter()
        .map(|k| format!("report-kind-{}", k.replace('_', "-")))
        .collect();
    assert_eq!(missing(&wanted), Vec::<String>::new());
}
