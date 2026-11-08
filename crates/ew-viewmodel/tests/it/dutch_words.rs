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
    // FT-1338 (ID-1392): a function's kind of `KIND_WORDS` in Dutch too.
    let kinds = [
        ("Double-acting cylinder ", "Dubbelwerkende cilinder "),
        ("Two-way process valve ", "Tweewegprocesklep "),
        ("Decoration ", "Decoratie "),
        ("PROFIBUS connector ", "PROFIBUS-connector "),
        ("PROFIBUS port ", "PROFIBUS-poort "),
        ("1P+N MCB ", "1P+N-installatieautomaat "),
        ("1P MCB ", "1P-installatieautomaat "),
        ("Overview ", "Overzicht "),
        ("5/2 valve ", "5/2-ventiel "),
        ("Terminal strip segment ", "Klemmenstrooksegment "),
        ("Protective earth bar ", "PE-rail "),
        ("Enclosure heater ", "Kastverwarming "),
        ("2P manual switch ", "2P-handschakelaar "),
        ("NO position switch ", "NO-positieschakelaar "),
        // FT-1483 (ID-1457): the generic rule's segments, a segment without words of its own as written.
        ("Cabinet lamp ", "Kastlamp "),
        ("NC thermostat ", "NC-thermostaat "),
        ("A1A2 coil ", "A1A2 spoel "),
    ];
    let worded: Vec<String> = english
        .iter()
        .map(|t| {
            let t = kinds
                .iter()
                .find_map(|(en, nl)| t.strip_prefix(en).map(|rest| format!("{nl}{rest}")))
                .unwrap_or_else(|| t.clone());
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
    // FT-1314: each rule group reads its rule's nl-NL title (the row's key is a path, `severity:<word>/rule:<id>`).
    let rule_rows: Vec<(String, String)> = t
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Rule)
        .filter_map(|n| Some((n.key.rsplit_once("rule:")?.1.to_owned(), n.label.clone())))
        .filter(|(rule, _)| rule != "release")
        .collect();
    assert!(!rule_rows.is_empty());
    for (rule, label) in &rule_rows {
        let title = messages::title(rule, "nl-NL").expect("a title");
        assert_eq!(*label, format!("{rule} {title}"));
    }
    for label in ["EW-FL-001 Werkpoort open", "EW-CORE-033 Kabel zonder lengte"] {
        assert_eq!(
            rule_rows.iter().filter(|(_, l)| l == label).count(),
            1,
            "{rule_rows:?}"
        );
    }
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
    // FT-1314: the detail of that row names the rule by its nl-NL title.
    let key = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Finding && n.label == cable)
        .expect("the cable row")
        .key
        .clone();
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": key}),
    );
    assert_eq!(
        shell.messages_detail().expect("the detail").title,
        "Kabel zonder lengte"
    );
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

/// FT-1493 (ID-1457): the property grid's own parse error reads in the window's language, keyed with its argument.
#[test]
fn ft1493_a_grid_parse_error_reads_in_the_windows_language() {
    let tmp = Tmp::new("ft1493");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    let refusal = |shell: &mut ShellModel| match shell
        .dispatch("props.set", &json!({"row": "device/tag.counter", "value": "x1"}))
    {
        ew_viewmodel::shell::Outcome::Failed { message } => message,
        other => panic!("{other:?}"),
    };
    assert_eq!(refusal(&mut shell), "`x1` is not a whole number");
    done(&mut shell, "view.language", json!({ "value": "nl" }));
    assert_eq!(refusal(&mut shell), "`x1` is geen geheel getal");
    done(&mut shell, "view.language", json!({ "value": "en" }));
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

/// C50-p6#7 (WP-S1.20 T25, FT-1258): in nl-NL a fluid line's Line kind choices are the Dutch words, filtered by them:
/// typing "Stuur" finds the pilot line ("Stuurleiding").
#[test]
fn a_fluid_line_kind_is_found_by_its_dutch_word() {
    // Golden A's fluid line F6 on folio 203.
    const F6: &str = "bc567040-5fda-53cd-8063-952499b3c9f3";
    let tmp = Tmp::new("line-kind-nl");
    let mut shell = tmp.shell_in(Mode::Headless, true, "203");
    done(&mut shell, "view.language", json!({"value": "nl"}));
    done(&mut shell, "select.set", json!({"ids": [F6]}));
    // FT-1319 (ID-1357 ruling 2): the sheet is headed by the line's kind in words, never "?".
    let title = |s: &ShellModel| s.properties().sheet().expect("a sheet").header.title.clone();
    assert_eq!(title(&shell), "Werkleiding");
    let sheet = shell.properties().sheet().expect("a sheet");
    let id = sheet.sections[0]
        .rows
        .iter()
        .find(|r| r.key == "connection/id")
        .expect("the ID row");
    assert_eq!(ew_viewmodel::properties::value_text(id), "Werkleiding");
    done(&mut shell, "view.language", json!({"value": "en"}));
    assert_eq!(title(&shell), "Working line");
    done(&mut shell, "view.language", json!({"value": "nl"}));
    done(&mut shell, "props.edit", json!({"row": "connection/line_kind"}));
    done(&mut shell, "props.input", json!({"text": "Stuur"}));
    let options: Vec<(String, serde_json::Value)> = shell
        .properties()
        .choices()
        .expect("the combo's options")
        .items
        .clone();
    assert_eq!(options, [("Stuurleiding".to_owned(), json!("pilot"))]);
    shell.shutdown();
}

/// ID-1389 (W51 finding G; doc 16a §6.8, §8.5): a fluid line's sheet shows its ends A and B as the device's tag and
/// connection point, never JSON, and its route only under Advanced, as frame cells, never nanometres; in English and
/// in Dutch.
#[test]
fn a_fluid_line_sheet_shows_its_ends_in_words_and_its_route_under_advanced() {
    // Golden A's fluid line F6 on folio 203.
    const F6: &str = "bc567040-5fda-53cd-8063-952499b3c9f3";
    let tmp = Tmp::new("fluid-ends");
    let mut shell = tmp.shell_in(Mode::Headless, true, "203");
    done(&mut shell, "select.set", json!({"ids": [F6]}));
    let rows = |s: &ShellModel, advanced: bool| -> Vec<String> {
        let sheet = s.properties().sheet().expect("a sheet");
        ew_viewmodel::properties::visible_rows(sheet, &BTreeSet::new(), advanced, "")
            .iter()
            .map(|v| {
                format!(
                    "{}: {}",
                    ew_viewmodel::properties::row_label(v.section, v.row),
                    ew_viewmodel::properties::value_text(v.row)
                )
            })
            .collect()
    };
    let en = vec![
        "Line kind: Working",
        "Page: 203 Valve Manifold 1",
        "A: -201U3-Q3:4",
        "B: -203Q3:B",
    ];
    let nl = vec![
        "Leidingsoort: Werkleiding",
        "Pagina: 203 Valve Manifold 1",
        "A: -201U3-Q3:4",
        "B: -203Q3:B",
    ];
    let en_advanced = vec![
        "$v: ",
        "ID: Working line",
        "Line kind: Working",
        "Page: 203 Valve Manifold 1",
        "A: -201U3-Q3:4",
        "B: -203Q3:B",
        "Route: 5 items",
        "Route 1: H5",
        "Route 2: H5",
        "Route 3: H5",
        "Route 4: F5",
        "Route 5: F5",
        "Source ref: ",
        "Key: connections|F6",
        "Lineage: import_lineage",
    ];
    let nl_advanced = vec![
        "$v: ",
        "ID: Werkleiding",
        "Leidingsoort: Werkleiding",
        "Pagina: 203 Valve Manifold 1",
        "A: -201U3-Q3:4",
        "B: -203Q3:B",
        "Route: 5 elementen",
        "Route 1: H5",
        "Route 2: H5",
        "Route 3: H5",
        "Route 4: F5",
        "Route 5: F5",
        "Bronverwijzing: ",
        "Sleutel: connections|F6",
        "Herkomst: import_lineage",
    ];
    for (language, main, advanced) in [("en", en, en_advanced), ("nl", nl, nl_advanced)] {
        done(&mut shell, "view.language", json!({ "value": language }));
        assert_eq!(rows(&shell, false), main, "{language}");
        assert_eq!(rows(&shell, true), advanced, "{language}");
    }
    shell.shutdown();
}

/// ID-1389 (W51 findings D, I, J, L) in both languages: the check-scheme question names the scheme's word; a page's
/// grid factor is a length, never a nanometre count; a panel page's Label mode and Surface read in words; a size takes
/// the language's decimal separator.
#[test]
fn the_scheme_grid_factor_surface_view_and_sizes_read_in_words() {
    let tmp = Tmp::new("w51-minor");
    let mut shell = tmp.shell_in(Mode::Headless, true, "902");
    let rows = |s: &ShellModel| -> Vec<String> {
        let sheet = s.properties().sheet().expect("a sheet");
        ew_viewmodel::properties::visible_rows(sheet, &BTreeSet::new(), true, "")
            .iter()
            .filter(|v| ["grid.num", "derived.label_mode", "derived.surface"].contains(&v.row.path.as_str()))
            .map(|v| {
                format!(
                    "{}: {}",
                    ew_viewmodel::properties::row_label(v.section, v.row),
                    ew_viewmodel::properties::value_text(v.row)
                )
            })
            .collect()
    };
    let mut got = Vec::new();
    for language in ["en", "nl"] {
        done(&mut shell, "view.language", json!({ "value": language }));
        done(&mut shell, "select.set", json!({"ids": []}));
        let question = ew_viewmodel::dialogs::spec(
            &ew_viewmodel::shell::Dialog::ConfirmCheckScheme {
                scheme: "pre-delivery".into(),
            },
            &shell,
        )
        .body;
        got.push((rows(&shell), question, strings::decimal("268.4")));
    }
    let words = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
    assert_eq!(
        got,
        [
            (
                words(&[
                    "Grid step multiplier: 7.9375 mm",
                    "Surface: -UC1 plate",
                    "Label mode: Abbreviated"
                ]),
                words(&[
                    "Apply the check scheme \"Pre-delivery\" to the basic project? The check scheme decides which rules \
                     the release gate runs and what the checks report."
                ]),
                "268.4".to_owned()
            ),
            (
                words(&[
                    "Rasterfactor: 7,9375 mm",
                    "Vlak: -UC1 montageplaat",
                    "Labelmodus: Afgekort"
                ]),
                words(&[
                    "Het controleschema \"Vóór oplevering\" op het basisproject toepassen? Het controleschema bepaalt \
                     welke regels de vrijgavepoort uitvoert en wat de controles melden."
                ]),
                "268,4".to_owned()
            )
        ]
    );
    // A label mode typed in Dutch is the mode it names.
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/derived.label_mode", "value": "Volledig"}),
    );
    assert_eq!(rows(&shell)[2], "Labelmodus: Volledig");
    shell.shutdown();
}

/// ID-1389 (ID-1320, W51 finding C): the orchestrator's Dutch word for word, and a re-import's member row quoting its
/// values in both languages.
#[test]
fn the_w51_dutch_texts_read_word_for_word() {
    let tmp = Tmp::new("w51-dutch");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    let member = || {
        strings::format(
            key!("reimport-row-member"),
            &["Title", "Bench", "lamp-circuit.qet", "Lamp and switch"],
        )
    };
    assert_eq!(
        member(),
        "Title: here \"Bench\", in lamp-circuit.qet \"Lamp and switch\""
    );
    done(&mut shell, "view.language", json!({"value": "nl"}));
    let cores = |n: usize| {
        strings::format(
            key!("toast-cable-new"),
            &[
                "-1W1",
                &strings::format_count(count_key!("toast-cable-new-cores"), n),
            ],
        )
    };
    assert_eq!(
        [
            member(),
            strings::format(key!("error-first-folio"), &["9x"]),
            cores(5),
            cores(1),
            strings::text(key!("choice-push")).to_owned(),
            strings::format(key!("refusal-structure-code-used"), &["K1"]),
            strings::format(
                key!("reimport-card-summary"),
                &["a.qet", "0", "1", "0", "0", "0", "0"]
            ),
        ],
        [
            "Title: hier \"Bench\", in lamp-circuit.qet \"Lamp and switch\"",
            "Het eerste folio moet een nummer zijn; \"9x\" is dat niet.",
            "Kabel -1W1 gemaakt met 5 aders.",
            "Kabel -1W1 gemaakt met 1 ader.",
            "Duwen",
            "een knooppunt op hetzelfde niveau met code \"K1\" bestaat al in dit aspect; kies een andere code",
            "Herimport van a.qet: 0 toegevoegd, 1 gewijzigd in de bron, 0 behouden met uw wijzigingen, 0 aan beide \
             kanten gewijzigd en behouden, 0 verwijderd in de bron en behouden, 0 ongewijzigd.",
        ]
    );
    done(&mut shell, "view.language", json!({"value": "en"}));
    shell.shutdown();
}

/// FT-1338 (ID-1357 ruling 1): in nl-NL the navigators' section groups and yes-or-no cells read in Dutch: the report
/// blocks' sections ("Sectie 9 - …"), the cables' Shield column ("nee").
#[test]
fn the_navigators_sections_and_yes_or_no_cells_read_in_dutch() {
    let tmp = Tmp::new("sections-nl");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "view.language", json!({"value": "nl"}));
    done(&mut shell, "navigator.show", json!({"kind": "blocks"}));
    let t = shell.nav(NavKind::Blocks).and_then(|n| n.table()).unwrap();
    let sections: BTreeSet<String> = t
        .nodes
        .iter()
        .filter(|n| n.role == ew_viewmodel::navigators::NavRole::Section)
        .map(|n| n.label.clone())
        .collect();
    assert!(
        !sections.is_empty()
            && sections
                .iter()
                .all(|l| l.starts_with("Sectie ") || l == "Voorwerk"),
        "{sections:?}"
    );
    done(&mut shell, "navigator.show", json!({"kind": "cables"}));
    let t = shell.nav(NavKind::Cables).and_then(|n| n.table()).unwrap();
    let shield = t
        .columns
        .iter()
        .position(|c| c.key == "shield")
        .expect("the Shield column");
    let cells: BTreeSet<String> = t
        .nodes
        .iter()
        .filter_map(|n| n.cells.get(shield).filter(|c| !c.is_empty()).cloned())
        .collect();
    assert!(
        cells.contains("nee") && cells.iter().all(|c| c == "ja" || c == "nee"),
        "{cells:?}"
    );
    shell.shutdown();
}

/// FT-1355 (W50-F4): the Messages pane names its scheme in the window's language, as the wizard's choice does.
#[test]
fn the_messages_scheme_reads_in_dutch() {
    let tmp = Tmp::new("scheme-nl");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "messages.show", json!({"on": true}));
    assert_eq!(shell.messages_scheme(), Some("Drafting"));
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert_eq!(
        (shell.messages_scheme(), shell.messages_project_scheme()),
        (Some("Ontwerp"), Some("Ontwerp"))
    );
    shell.shutdown();
}

/// C50-p7#9 (ID-1320): the Dutch toasts of a new terminal strip each say, as `toast-strip-new` does, that the symbol's
/// strip picker lists the strip.
#[test]
fn the_new_strip_toasts_name_the_strip_picker_in_dutch() {
    let tmp = Tmp::new("strip-new-nl");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert_eq!(
        (
            strings::format(key!("toast-strip-new-tagged"), &["-X5"]),
            strings::text(key!("toast-strip-new-no-example")).to_owned()
        ),
        (
            "Klemmenstrook gemaakt: -X5. Plaats de klemmen met Tekenen ▸ Symbool… en het klemsymbool; de keuzelijst van \
             het symbool toont haar."
                .to_owned(),
            "Klemmenstrook gemaakt. Het kenmerk volgt bij de eerste klem: Tekenen ▸ Symbool… en het klemsymbool; de \
             keuzelijst van het symbool toont haar."
                .to_owned()
        )
    );
    shell.shutdown();
}

/// ID-1439 (C52-p8#5, #6, #7): in Dutch the Messages header's sentence puts the count before "vrijgesteld" and
/// "wachtend", the Preview's counts name items "elementen", and the action after the Preview's colon is in a sentence.
#[test]
fn id1439_the_messages_and_preview_headers_in_dutch() {
    let tmp = Tmp::new("id1439-headers-nl");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert_eq!(
        (
            strings::format(key!("messages-header"), &["Ontwerp", "1", "2", "3", "4", "5"]),
            strings::format_count(count_key!("preview-records"), 1),
            strings::format_count(count_key!("preview-records"), 2),
            strings::format(key!("preview-header"), &["Eigenschap instellen", "7"]),
            strings::format(key!("preview-title"), &["Eigenschap instellen"]),
        ),
        (
            "Ontwerp · F 1 · W 2 · I 3 · 4 vrijgesteld · 5 wachtend".to_owned(),
            "1 element".to_owned(),
            "2 elementen".to_owned(),
            "Voorbeeld: eigenschap instellen · vanaf revisie 7".to_owned(),
            "Voorbeeld: eigenschap instellen".to_owned(),
        )
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

/// FT-1364 (W50-F14): View ▸ Language takes the toasts on show off the screen, so none mixes the languages; the log
/// keeps them, and choosing the language already on show keeps them.
#[test]
fn ft1364_a_language_switch_drops_the_toasts_on_show() {
    let tmp = Tmp::new("ft1364");
    let mut shell = tmp.shell("102");
    done(&mut shell, "messages.check", json!({}));
    assert!(shell.toast().is_some(), "the check toast");
    let logged = shell.notifications().len();
    done(&mut shell, "view.language", json!({"value": "en"}));
    assert!(shell.toast().is_some(), "the same language keeps it");
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert!(shell.toast().is_none());
    assert_eq!(shell.notifications().len(), logged, "the log keeps it");
    done(&mut shell, "view.language", json!({"value": "en"}));
    shell.shutdown();
}

/// FT-1338 (ID-1392): the window words what `ew-api` builds in English (FT-680) in its language: a device's kind, a
/// terminal's state, an interruption point's direction, a function's kind of `KIND_WORDS` and a provenance chip; in
/// en-GB they read as before.
#[test]
fn navigator_cells_kinds_and_chips_read_in_dutch() {
    use ew_viewmodel::navigators::NavRole;
    use ew_viewmodel::properties::{Provenance, chip_text};
    let tmp = Tmp::new("ft1338-rest");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let cell = |shell: &ShellModel, kind: NavKind, col: &str, role: NavRole| -> (String, String) {
        let t = shell
            .nav(kind)
            .and_then(|n| n.table())
            .cloned()
            .expect("the table");
        let i = t.columns.iter().position(|c| c.key == col).expect("the column");
        // The source point 0VDC on 102-C15, whatever the order of the rows.
        let n = t
            .nodes
            .iter()
            .find(|n| n.role == role && (role != NavRole::Ip || n.label == "102-C15"))
            .expect("a row");
        (n.label.clone(), n.cells[i].clone())
    };
    let function = |shell: &ShellModel| -> String {
        let t = shell
            .nav(NavKind::Devices)
            .and_then(|n| n.table())
            .cloned()
            .expect("the table");
        t.nodes
            .iter()
            .filter(|n| n.role == NavRole::Function)
            .map(|n| n.label.clone())
            .find(|l| l.ends_with(" 1-2"))
            .expect("a two-way process valve")
    };
    let chip = Provenance::Derived {
        via: "potential definition at -X1:1 · colour profile 'DBU/WH 24 V'".into(),
    };
    for (locale, kind, state, direction, valve, chip_words) in [
        (
            "en",
            "Device",
            "used",
            "source",
            "Two-way process valve 1-2",
            "f derived · potential definition at -X1:1 · colour profile 'DBU/WH 24 V'",
        ),
        (
            "nl",
            "Apparaat",
            "gebruikt",
            "bron",
            "Tweewegprocesklep 1-2",
            "f afgeleid · potentiaaldefinitie bij -X1:1 · kleurprofiel 'DBU/WH 24 V'",
        ),
    ] {
        done(&mut shell, "view.language", json!({ "value": locale }));
        for k in ["devices", "strips", "ips"] {
            done(&mut shell, "navigator.show", json!({ "kind": k }));
        }
        assert_eq!(cell(&shell, NavKind::Devices, "kind", NavRole::Device).1, kind);
        assert_eq!(cell(&shell, NavKind::Strips, "state", NavRole::Terminal).1, state);
        assert_eq!(cell(&shell, NavKind::Ips, "direction", NavRole::Ip).1, direction);
        assert_eq!(function(&shell), valve);
        assert_eq!(chip_text(&chip), chip_words);
    }
    done(&mut shell, "view.language", json!({ "value": "en" }));
    shell.shutdown();
}

/// FT-1483 (ID-1457, ID-1459): the window words what the navigators' JSON keeps in English: a function's kind by the
/// generic rule ("NC thermostat 5-3"), a library's tier, access and index state, a report block's kind; in en-GB they read as
/// the CLI prints them, but a block's kind, which reads by its words in both.
#[test]
fn ft1483_kinds_tiers_and_block_kinds_read_in_dutch() {
    use ew_viewmodel::navigators::NavRole;
    let tmp = Tmp::new("ft1483");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let table = |shell: &ShellModel, kind: NavKind| shell.nav(kind).and_then(|n| n.table()).cloned().unwrap();
    let col = |t: &ew_viewmodel::ew_api::output::NavTable, key: &str| {
        t.columns.iter().position(|c| c.key == key).expect("the column")
    };
    let mut seen = Vec::new();
    for locale in ["en", "nl"] {
        done(&mut shell, "view.language", json!({ "value": locale }));
        for k in ["devices", "libraries", "blocks"] {
            done(&mut shell, "navigator.show", json!({ "kind": k }));
        }
        let devices = table(&shell, NavKind::Devices);
        let kind = devices
            .nodes
            .iter()
            .filter(|n| n.role == NavRole::Function)
            .map(|n| n.label.clone())
            .find(|l| l.ends_with(" 5-3"))
            .expect("a thermostat");
        let libraries = table(&shell, NavKind::Libraries);
        let tier = libraries
            .nodes
            .iter()
            .find(|n| n.role == NavRole::Tier)
            .map(|n| n.label.clone())
            .expect("a tier");
        let library = libraries
            .nodes
            .iter()
            .find(|n| n.role == NavRole::Library)
            .expect("a library");
        let (tier_cell, access, state) = (
            library.cells[col(&libraries, "tier")].clone(),
            library.cells[col(&libraries, "access")].clone(),
            library.cells[col(&libraries, "state")].clone(),
        );
        let blocks = table(&shell, NavKind::Blocks);
        let kinds: BTreeSet<String> = blocks
            .nodes
            .iter()
            .filter(|n| n.role == NavRole::ReportBlock)
            .map(|n| n.cells[col(&blocks, "kind")].clone())
            .collect();
        seen.push((
            kind,
            tier,
            tier_cell,
            access,
            state,
            kinds.contains(if locale == "en" { "Wire list" } else { "Draadlijst" }),
        ));
    }
    done(&mut shell, "view.language", json!({ "value": "en" }));
    shell.shutdown();
    assert_eq!(
        seen,
        [
            (
                "NC thermostat 5-3".to_owned(),
                "System".to_owned(),
                "System".to_owned(),
                "Read-only".to_owned(),
                "Index rebuilt".to_owned(),
                true
            ),
            (
                "NC-thermostaat 5-3".to_owned(),
                "Systeem".to_owned(),
                "Systeem".to_owned(),
                "Alleen-lezen".to_owned(),
                "Index opnieuw opgebouwd".to_owned(),
                true
            ),
        ]
    );
}

/// C53-p3#5 (ID-1481): the Blocks navigator words a block's kind code in its kind column, and in its title and label
/// only where `ew-api` put the code there for a block without a title; a title that reads as a code stays as typed.
#[test]
fn a_block_title_that_reads_as_a_kind_code_stays_as_typed() {
    let tmp = Tmp::new("c53-p3-5");
    let dir = tmp.golden_copy();
    crate::common::copy_dir(std::path::Path::new(crate::common::GOLDEN_A), &dir);
    let blocks = dir.join("report-blocks.jsonl");
    let text = std::fs::read_to_string(&blocks).unwrap();
    let edited = text.replace("\"title\":\"Table of Contents {n}\",", "").replace(
        "\"title\":\"Revision Overview\"",
        "\"title\":\"revision_overview\"",
    );
    assert!(edited != text);
    std::fs::write(&blocks, edited).unwrap();
    let mut shell = tmp.shell_in(Mode::Headless, false, "101");
    let row = |shell: &ShellModel, id: &str| {
        let table = shell
            .nav(NavKind::Blocks)
            .and_then(|n| n.table())
            .cloned()
            .unwrap();
        let kind = table.columns.iter().position(|c| c.key == "kind").unwrap();
        let node = table
            .nodes
            .iter()
            .find(|n| n.records.first().is_some_and(|r| r.to_string() == id))
            .cloned()
            .expect("the block");
        (
            node.label.clone(),
            node.cells[0].clone(),
            node.cells[kind].clone(),
        )
    };
    let mut seen = Vec::new();
    for locale in ["en", "nl"] {
        done(&mut shell, "view.language", json!({ "value": locale }));
        done(&mut shell, "navigator.show", json!({ "kind": "blocks" }));
        seen.push(row(&shell, "0c94a4fa-8056-575b-88ee-4e5905fbb4c8"));
        seen.push(row(&shell, "0d61e123-bf15-5761-8ca0-edeb5a82734d"));
    }
    done(&mut shell, "view.language", json!({ "value": "en" }));
    shell.shutdown();
    let s = |a: &str, b: &str, c: &str| (a.to_owned(), b.to_owned(), c.to_owned());
    assert_eq!(
        seen,
        [
            s("Table of contents", "Table of contents", "Table of contents"),
            s("revision_overview", "revision_overview", "Revision overview"),
            s("Inhoudsopgave", "Inhoudsopgave", "Inhoudsopgave"),
            s("revision_overview", "revision_overview", "Revisieoverzicht"),
        ]
    );
}
