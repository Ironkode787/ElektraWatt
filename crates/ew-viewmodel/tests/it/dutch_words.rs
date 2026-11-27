// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Dutch UI: the palette form's labels and the finding detail's kind from keys in
//! both locales, the count texts and a new terminal strip in words.

use std::collections::BTreeSet;

use crate::common::{Tmp, done};
use ew_api::checks::RuleKind;
use ew_viewmodel::actions;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::navigators::messages::FindingDetail;
use ew_viewmodel::palette::{FormPlan, PaletteField, form_plan};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::{count_key, key, strings};
use serde_json::json;

/// The keys of the shipped Fluent files of `locale`.
fn keys(locale: &str) -> BTreeSet<String> {
    strings::sources(locale)
        .flat_map(str::lines)
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

/// A parameter's label as it was derived before the labels came from keys: the name with `_` as spaces, its first letter upper case.
fn derived(name: &str) -> String {
    let text = name.replace('_', " ");
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Every function category of the shipped identifier sets (the first level of an FnDef category) has its
/// words in en-GB and nl-NL, so the browser's tree names it rather than listing its devices under Other.
#[test]
fn every_function_category_has_words_in_both_locales() {
    let sets = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ew-rds/data/identifier-sets.toml"),
    )
    .unwrap();
    // The `key` line after each `[[category]]` header.
    let wanted: BTreeSet<String> = sets
        .split("[[category]]")
        .skip(1)
        .filter_map(|c| {
            c.lines()
                .find_map(|l| l.strip_prefix("key = \"")?.strip_suffix('"'))
        })
        .map(|k| format!("function-category-{k}"))
        .collect();
    assert!(!wanted.is_empty());
    assert_eq!(missing(&wanted), Vec::<String>::new());
}

/// Ruling 1: every parameter of every action the palette opens a form for has its `param-<name>` label in
/// en-GB and nl-NL; the en-GB label is the one derived before, the Dutch one its own.
#[test]
fn every_palette_form_parameter_has_a_label_in_both_locales() {
    let mut wanted = BTreeSet::new();
    let mut fields = Vec::new();
    for meta in actions::ACTIONS {
        // Define potential… opens its form itself, with the action's parameters.
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
                    pages: Vec::new(),
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

/// Ruling 3: every rule kind has its `finding-kind-<kind>` in en-GB and nl-NL, and the detail's line of a
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

/// Ruling 6: a count of one takes its own text in both languages.
#[test]
fn a_count_of_one_reads_in_the_singular() {
    let cases = [
        (count_key!("find-results"), "1 result", "1 resultaat", "2 results"),
        (count_key!("label-paste-wires"), "1 wire", "1 draad", "2 wires"),
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
        // The paste ghost's dropped connections, the open banner's findings, the approval's waivers.
        (
            count_key!("label-paste-dropped"),
            "; 1 wire left out: its other end was not copied",
            "; 1 draad weggelaten: het andere eind was niet gekopieerd",
            "; 2 wires left out: their other end was not copied",
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

/// Ruling 7: right after Terminal strip…, the Terminal strips tree names the new strip in words, not by its
/// underived tag, and the table, whose rows are terminals, says there is none yet and how to place one.
#[test]
fn a_new_strip_without_a_terminal_reads_in_words() {
    let tmp = Tmp::new("l49");
    let mut shell = tmp.empty_shell(Mode::Headless);
    crate::common::make_project(&mut shell, &tmp.0.join("new.ew"), &[]);
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
            "Place a terminal with Draw ▸ Terminal…; it goes into the strip you choose there.".to_owned()
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

/// Dutch puts the action's name mid-sentence through the Fluent function `IN_SENTENCE`;
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
fn table(shell: &ShellModel, kind: NavKind) -> std::sync::Arc<ew_api::output::NavTable> {
    shell
        .nav(kind)
        .and_then(|n| n.table())
        .cloned()
        .expect("the navigator's table")
}

/// Every navigator's column heading has its words in both locales, the en-GB ones the label `ew-api` gives; in nl-NL
/// the window's tables show the Dutch headers and the Devices tree its badges, its group without a function ("(geen)") and its kinds in Dutch, the
/// generic rule's kinds by their segments' words ("3P-handschakelaar", "Geluiddemper").
#[test]
fn every_navigator_column_and_badge_reads_in_both_locales() {
    let (session, _) =
        ew_api::Session::open_read_only(std::path::Path::new(crate::common::SAMPLE), "test").unwrap();
    let mut wrong = Vec::new();
    for kind in NavKind::ALL {
        let english = session.navigator_table(kind).columns;
        for (c, shown) in ew_api::output::navigator_columns(kind).iter().zip(english) {
            let words = strings::shipped_renders(&c.label);
            if words.len() != 2 || words[0] != shown.label {
                wrong.push(format!("{kind:?} {}: {words:?} for {:?}", c.key, shown.label));
            }
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
    let tmp = Tmp::new("ft1264");
    let mut shell = tmp.sample_shell("102");
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
    // A function's kind of `KIND_WORDS` in Dutch too, and the generic rule's by its segments' words ("Black box"
    // keeps its English).
    let kinds = [
        ("1P MCB ", "1P-installatieautomaat "),
        ("2P mcb ", "2P-installatieautomaat "),
        ("3P manualswitch ", "3P-handschakelaar "),
        ("3P mcb ", "3P-installatieautomaat "),
        ("3P motor ", "3P-motor "),
        ("5/2 valve ", "5/2-ventiel "),
        ("A1A2 coil ", "A1A2-spoel "),
        ("AcDc powersupply ", "AC/DC-voeding "),
        ("Cabinet lamp ", "Kastlamp "),
        ("Coil ", "Spoel "),
        ("Double-acting cylinder ", "Dubbelwerkende cilinder "),
        ("Generic supplyplate ", "Algemene aansluitplaat "),
        ("NC pushbutton ", "NC-drukknop "),
        ("NO contact ", "NO-contact "),
        ("NO position switch ", "NO-positieschakelaar "),
        ("NO pushbutton ", "NO-drukknop "),
        ("Overview ", "Overzicht "),
        ("PROFIBUS port ", "PROFIBUS-poort "),
        ("Profinet busport ", "PROFINET-buspoort "),
        ("Standard silencer ", "Geluiddemper "),
        ("Supply source ", "Luchtbron "),
        ("(none)", "(geen)"),
    ];
    let worded: Vec<String> = english
        .iter()
        .map(|t| {
            let t = kinds
                .iter()
                .find_map(|(en, nl)| Some(format!("{nl}{}", t.strip_prefix(en)?)))
                .unwrap_or_else(|| t.clone());
            t.replace(" [main]", " [hoofdfunctie]")
                .replace(" [aux]", " [hulpfunctie]")
                .replace(" [spare]", " [reserve]")
                .replace(" [not drawn]", " [niet getekend]")
                .replace(" [frozen]", " [vastgezet]")
                .replace(" [earth connection]", " [aardverbinding]")
        })
        .collect();
    assert!(english.iter().any(|t| t.ends_with(" [main]")), "{english:?}");
    // The same rows, siblings in the order of the words they show ("NO-contact" before "Spoel").
    let (mut dutch, mut worded) = (dutch, worded);
    dutch.sort();
    worded.sort();
    assert_eq!(dutch, worded);
    shell.shutdown();
}

/// In nl-NL the Messages pane's group headers, its rule groups and its
/// finding rows read in Dutch, the messages as the detail renders them, and so does the Properties pane's finding chip.
#[test]
fn the_messages_rows_and_the_chip_read_in_dutch() {
    use ew_api::checks::{CheckMode, messages};
    use ew_api::output::NavRole;
    let tmp = Tmp::new("ft1162");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    // A finding the sample lacks: the supply cable -101W1 without its length.
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101W1"}],
            "path": "cable.length", "value": "missing"}}),
    );
    done(&mut shell, "file.save", json!({}));
    let (session, _) = ew_api::Session::open_read_only(&tmp.sample_copy(), "test").unwrap();
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
        (ew_api::checks::Severity::Error, "Fouten"),
        (ew_api::checks::Severity::Warning, "Waarschuwingen"),
        (ew_api::checks::Severity::Info, "Info"),
    ] {
        if report.findings.iter().any(|f| f.severity == severity) {
            expected_groups.push(word);
        }
    }
    assert_eq!(groups, expected_groups);
    // Each rule group reads its rule's nl-NL title (the row's key is a path, `severity:<word>/rule:<id>`).
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
    // The sample's tubes from the valve terminal in +CAB to the cylinders in +FLD, and the cable without its length.
    for label in [
        "EW-FL-014 Slang verlaat een locatie",
        "EW-CORE-033 Kabel zonder lengte",
    ] {
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
        // The subject once: before the message unless the message names it.
        let subject = f.subject_in("nl-NL");
        let leads = ew_api::output::names_subject(&subject, &text);
        let shown = if subject.is_empty() || leads {
            text
        } else {
            format!("{subject}: {text}")
        };
        rows.push(n.label.clone());
        expected.push(shown);
    }
    assert!(!rows.is_empty());
    assert_eq!(rows, expected);
    // One row as the Dutch reader sees it.
    let cable = "-101W1 heeft nog geen lengte: typ die in de eigenschappen, of routeer de kabel in de layout";
    assert_eq!(rows.iter().filter(|r| r.ends_with(cable)).count(), 1, "{rows:?}");
    // The detail of that row names the rule by its nl-NL title.
    let key = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Finding && n.label.ends_with(cable))
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
            .any(|f| messages::render(f, "nl-NL") != f.message()),
        "a Dutch message to show"
    );
    // The chip of a record with an open finding.
    let f = report
        .findings
        .iter()
        .find(|f| {
            f.state == ew_api::checks::FindingState::Open
                && !f.targets.is_empty()
                && messages::render(f, "nl-NL") != f.message()
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
                && g.state == ew_api::checks::FindingState::Open
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

/// A generated page's Properties show its Kind and its row source in words, in en-GB and
/// nl-NL, never the JSON; the JSON values stay.
#[test]
fn a_generated_page_shows_its_kind_and_source_in_words() {
    let tmp = Tmp::new("ft1287");
    let mut shell = tmp.sample_shell("2");
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

/// The property grid's own parse error reads in the window's language, keyed with its argument.
#[test]
fn a_grid_parse_error_reads_in_the_windows_language() {
    let tmp = Tmp::new("ft1493");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
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

/// A command whose reference names no record (the command log's path of `command.run`) and a
/// selection by an unknown tag are refused in the window's language.
#[test]
fn an_unresolved_reference_reads_in_the_windows_language() {
    let tmp = Tmp::new("ft1553");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    let failed = |shell: &mut ShellModel, action: &str, params: serde_json::Value| match shell
        .dispatch(action, &params)
    {
        ew_viewmodel::shell::Outcome::Failed { message } => message,
        other => panic!("{other:?}"),
    };
    let run = json!({"cmd": "fields.set", "params": {
        "kind": "device", "targets": [{"ref": "tag:-Q99"}], "path": "function_text", "value": {"en": "x"}
    }});
    let select = json!({"refs": ["-Q99"]});
    let en = (
        failed(&mut shell, "command.run", run.clone()),
        failed(&mut shell, "select.set", select.clone()),
    );
    done(&mut shell, "view.language", json!({ "value": "nl" }));
    let nl = (
        failed(&mut shell, "command.run", run),
        failed(&mut shell, "select.set", select),
    );
    done(&mut shell, "view.language", json!({ "value": "en" }));
    assert!(en.0.contains("`tag:-Q99`: no device prints this tag"), "{en:?}");
    assert!(
        en.1.starts_with("`-Q99`: no device prints this tag; a reference is a tag"),
        "{en:?}"
    );
    assert!(
        nl.0.contains("`tag:-Q99`: geen apparaat drukt dit kenmerk af"),
        "{nl:?}"
    );
    assert!(
        nl.1.starts_with("`-Q99`: geen apparaat drukt dit kenmerk af; een verwijzing is"),
        "{nl:?}"
    );
    shell.shutdown();
}

/// An enumerated row shows its value and its choices in the UI language's words, takes
/// a word typed in them, and refuses another word naming the choices in words, in en-GB and nl-NL.
#[test]
fn an_enumerated_row_reads_and_takes_its_words() {
    let tmp = Tmp::new("ft1258");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
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

/// The sample's tube from the valve -201K1-Q1 to the cylinder -201M1 on folio 201, as the page text names it.
const TUBE: &str = "wire:-201K1-Q1:4 -> -201M1:A";

/// In nl-NL a fluid line's Line kind choices are the Dutch words, filtered by them:
/// typing "Stuur" finds the pilot line ("Stuurleiding").
#[test]
fn a_fluid_line_kind_is_found_by_its_dutch_word() {
    let tmp = Tmp::new("line-kind-nl");
    let mut shell = tmp.sample_in(Mode::Headless, true, "201");
    done(&mut shell, "view.language", json!({"value": "nl"}));
    done(&mut shell, "select.set", json!({"refs": [TUBE]}));
    // The sheet is headed by the line's kind in words, never "?".
    let title = |s: &ShellModel| s.properties().sheet().expect("a sheet").header.title.clone();
    assert_eq!(title(&shell), "Slang (Werkleiding)");
    let sheet = shell.properties().sheet().expect("a sheet");
    let id = sheet.sections[0]
        .rows
        .iter()
        .find(|r| r.key == "connection/id")
        .expect("the ID row");
    assert_eq!(ew_viewmodel::properties::value_text(id), "Slang (Werkleiding)");
    done(&mut shell, "view.language", json!({"value": "en"}));
    assert_eq!(title(&shell), "Tube (Working)");
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

/// Doc 16a §6.8, §8.5: a fluid line's sheet shows its ends A and B as the device's tag and
/// connection point, never JSON, and its route only under Advanced, as frame cells, never nanometres; in English and
/// in Dutch.
#[test]
fn a_fluid_line_sheet_shows_its_ends_in_words_and_its_route_under_advanced() {
    let tmp = Tmp::new("fluid-ends");
    let mut shell = tmp.sample_in(Mode::Headless, true, "201");
    done(&mut shell, "select.set", json!({"refs": [TUBE]}));
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
    for (language, kind, page, route) in [
        ("en", "Line kind: Working", "Page: 201 Pneumatics", "Route: "),
        (
            "nl",
            "Leidingsoort: Werkleiding",
            "Pagina: 201 Pneumatics",
            "Route: ",
        ),
    ] {
        done(&mut shell, "view.language", json!({ "value": language }));
        let main = rows(&shell, false);
        for row in [kind, page, "A: -201K1-Q1:4", "B: -201M1:A"] {
            assert!(main.iter().any(|r| r == row), "{language}: {row} in {main:?}");
        }
        assert!(!main.iter().any(|r| r.starts_with(route)), "{language}: {main:?}");
        let advanced = rows(&shell, true);
        let points: Vec<&String> = advanced.iter().filter(|r| r.starts_with("Route ")).collect();
        assert!(!points.is_empty(), "{language}: {advanced:?}");
        // Every point of the route is a frame cell (a letter and a number), never nanometres.
        for p in points {
            let cell = p.split_once(": ").map(|(_, c)| c).unwrap_or_default();
            let (row, column) = cell.split_at(cell.len().min(1));
            assert!(
                row.chars().all(|c| c.is_ascii_uppercase()) && column.parse::<u32>().is_ok(),
                "{language}: {p}"
            );
        }
        assert!(
            advanced.iter().any(|r| r.starts_with(route)),
            "{language}: the route's count row: {advanced:?}"
        );
    }
    shell.shutdown();
}

/// In both languages: a check level reads as its word; a page's
/// grid factor is a length, never a nanometre count; a panel page's Label mode and Surface read in words; a size takes
/// the language's decimal separator.
#[test]
fn the_scheme_grid_factor_surface_view_and_sizes_read_in_words() {
    let tmp = Tmp::new("w51-minor");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
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
        let question = vec![ew_viewmodel::shell::level_words("pre-delivery")];
        got.push((rows(&shell), question, strings::decimal("268.4")));
    }
    let words = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
    assert_eq!(
        got,
        [
            (
                words(&[
                    "Grid step multiplier: 7.9375 mm",
                    "Surface: -UC1 mounting plate",
                    "Label mode: Abbreviated"
                ]),
                words(&["Pre-delivery"]),
                "268.4".to_owned()
            ),
            (
                words(&[
                    "Rasterfactor: 7,9375 mm",
                    "Vlak: -UC1 montageplaat",
                    "Labelmodus: Afgekort"
                ]),
                words(&["Vóór oplevering"]),
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

/// The Dutch word for word, and a re-import's member row quoting its
/// values in both languages.
#[test]
fn the_w51_dutch_texts_read_word_for_word() {
    let tmp = Tmp::new("w51-dutch");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
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

/// In nl-NL the navigators' section groups and yes-or-no cells read in Dutch: the report
/// blocks' sections ("Sectie 9 - …"), the cables' Shield column ("nee").
#[test]
fn the_navigators_sections_and_yes_or_no_cells_read_in_dutch() {
    let tmp = Tmp::new("sections-nl");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
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

/// The Messages pane names its scheme in the window's language, as the wizard's choice does.
#[test]
fn the_messages_scheme_reads_in_dutch() {
    let tmp = Tmp::new("scheme-nl");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    done(&mut shell, "messages.show", json!({"on": true}));
    assert_eq!(shell.messages_scheme(), Some("Drafting"));
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert_eq!(
        (shell.messages_scheme(), shell.messages_project_scheme()),
        (Some("Ontwerp"), Some("Ontwerp"))
    );
    shell.shutdown();
}

/// The Dutch toasts of a new terminal strip each say, as `toast-strip-new` does, that the symbol's
/// strip picker lists the strip.
#[test]
fn the_new_strip_toasts_name_the_strip_picker_in_dutch() {
    let tmp = Tmp::new("strip-new-nl");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert_eq!(
        (
            strings::format(key!("toast-strip-new-tagged"), &["-X5"]),
            strings::text(key!("toast-strip-new-no-example")).to_owned()
        ),
        (
            "Nieuwe klemmenstrook -X5: Tekenen ▸ Klem… plaatst haar klemmen.".to_owned(),
            "Nieuwe klemmenstrook; de eerste klem geeft haar kenmerk: Tekenen ▸ Klem… plaatst haar klemmen."
                .to_owned()
        )
    );
    shell.shutdown();
}

/// In Dutch the Messages header's sentence puts the count before "vrijgesteld" and
/// "wachtend", the Preview's counts name items "elementen", and the action after the Preview's colon is in a sentence.
#[test]
fn the_messages_and_preview_headers_in_dutch() {
    let tmp = Tmp::new("id1439-headers-nl");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
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

/// Every report kind of the format has its name in en-GB and nl-NL (`ew-reports`' `report-kind-<kind>`), so no
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
    for k in &kinds {
        let tags: Vec<&str> = ew_api::report_kind_names(k).iter().map(|(tag, _)| *tag).collect();
        assert_eq!(tags, ["en-GB", "nl-NL"], "the name of {k}");
    }
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    // What gives a block of a page kind its rows, said when it makes no page yet.
    wanted.extend(
        ew_api::output::page_report_kinds()
            .iter()
            .map(|k| format!("report-rows-{}", k.replace('_', "-"))),
    );
    assert_eq!(missing(&wanted), Vec::<String>::new());
}

/// View ▸ Language takes the toasts on show off the screen, so none mixes the languages; the log
/// keeps them, and choosing the language already on show keeps them.
#[test]
fn a_language_switch_drops_the_toasts_on_show() {
    let tmp = Tmp::new("ft1364");
    let mut shell = tmp.sample_shell("102");
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

/// The window words what `ew-api` builds in English in its language: a device's kind, a
/// terminal's state, an interruption point's direction, a function's kind of `KIND_WORDS` and a provenance chip; in
/// en-GB they read as before.
#[test]
fn navigator_cells_kinds_and_chips_read_in_dutch() {
    use ew_viewmodel::navigators::NavRole;
    use ew_viewmodel::properties::{Provenance, chip_text};
    let tmp = Tmp::new("ft1338-rest");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    let cell = |shell: &ShellModel, kind: NavKind, col: &str, role: NavRole| -> (String, String) {
        let t = shell
            .nav(kind)
            .and_then(|n| n.table())
            .cloned()
            .expect("the table");
        let i = t.columns.iter().position(|c| c.key == col).expect("the column");
        // An interruption point that is a source (the sample's are on 101), whatever the order of the rows.
        let n = t
            .nodes
            .iter()
            .find(|n| {
                n.role == role && (role != NavRole::Ip || ["source", "bron"].contains(&n.cells[i].as_str()))
            })
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
            .find(|l| l.starts_with("1P") && l.ends_with(" 1-2"))
            .expect("the 1P breaker -102F1")
    };
    // A chip of two phrases, each in the window's language.
    let chip = Provenance::Derived {
        via: ew_api::text!(
            "via-joined",
            ew_api::text!("via-potential-definition-at", "-X1:1"),
            ew_api::text!("via-workshop-stock")
        ),
    };
    for (locale, kind, state, direction, valve, chip_words) in [
        (
            "en",
            "Device",
            "used",
            "source",
            "1P MCB 1-2",
            "f derived · potential definition at -X1:1 · workshop stock",
        ),
        (
            "nl",
            "Apparaat",
            "gebruikt",
            "bron",
            "1P-installatieautomaat 1-2",
            "f afgeleid · potentiaaldefinitie bij -X1:1 · werkplaatsvoorraad",
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

/// The window words what the navigators' JSON keeps in English: a function's kind by the
/// generic rule ("Cabinet lamp X1-X2"), a library's tier, access and index state, a report block's kind; in en-GB they read as
/// the CLI prints them, but a block's kind, which reads by its words in both.
#[test]
fn kinds_tiers_and_block_kinds_read_in_dutch() {
    use ew_viewmodel::navigators::NavRole;
    let tmp = Tmp::new("ft1483");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    let table = |shell: &ShellModel, kind: NavKind| shell.nav(kind).and_then(|n| n.table()).cloned().unwrap();
    let col = |t: &ew_api::output::NavTable, key: &str| {
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
            .find(|l| l.ends_with(" X1-X2"))
            .expect("a lamp");
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
                "Cabinet lamp X1-X2".to_owned(),
                "System".to_owned(),
                "System".to_owned(),
                "Read-only".to_owned(),
                "Index rebuilt".to_owned(),
                true
            ),
            (
                "Kastlamp X1-X2".to_owned(),
                "Systeem".to_owned(),
                "Systeem".to_owned(),
                "Alleen-lezen".to_owned(),
                "Index opnieuw opgebouwd".to_owned(),
                true
            ),
        ]
    );
}

/// The live description of a selected device and the Libraries kind column read in Dutch: a function's kind by
/// the generic rule, a library copy's kind by its record kind's words.
#[test]
fn the_live_description_and_the_library_kinds_read_in_dutch() {
    use ew_viewmodel::navigators::NavRole;
    let tmp = Tmp::new("ft1554");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    done(&mut shell, "select.set", json!({"refs": ["-102E1"]}));
    done(&mut shell, "navigator.show", json!({ "kind": "libraries" }));
    let mut seen = Vec::new();
    for locale in ["en", "nl"] {
        done(&mut shell, "view.language", json!({ "value": locale }));
        let live = ew_viewmodel::status::selection_text(&shell);
        let t = shell
            .nav(NavKind::Libraries)
            .and_then(|n| n.table())
            .cloned()
            .unwrap();
        let kind = t
            .columns
            .iter()
            .position(|c| c.key == "kind")
            .expect("the kind column");
        let kinds: BTreeSet<String> = t
            .nodes
            .iter()
            .filter(|n| n.role == NavRole::LibCopy)
            .map(|n| n.cells[kind].clone())
            .collect();
        seen.push((live, kinds));
    }
    done(&mut shell, "view.language", json!({ "value": "en" }));
    shell.shutdown();
    assert!(seen[0].0.contains("Cabinet lamp X1-X2"), "{:?}", seen[0]);
    assert!(seen[1].0.contains("Kastlamp X1-X2"), "{:?}", seen[1]);
    assert!(seen[0].1.contains("Function definition"), "{:?}", seen[0].1);
    assert!(seen[1].1.contains("Functiedefinitie"), "{:?}", seen[1].1);
    for english in [
        "Part",
        "Symbol",
        "Function definition",
        "Form",
        "Manufacturer",
        "PLC profile",
    ] {
        assert!(!seen[1].1.contains(english), "{english} in {:?}", seen[1].1);
    }
}

/// A function placed from a part's template shows its Part function row in the window's language.
#[test]
fn the_part_function_row_reads_in_dutch() {
    let tmp = Tmp::new("ft1554-part");
    let mut s = crate::common::new_project(&tmp);
    let before = crate::common::tables(&s).reps.clone();
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-MCB-2", "template": 0}),
    );
    let p = crate::common::grid_point(&s, (40, 20));
    crate::common::click_at(&mut s, p);
    done(&mut s, "tool.select", json!({}));
    let tables = crate::common::tables(&s);
    let rep = tables
        .reps
        .iter()
        .find(|r| !before.iter().any(|b| b.id == r.id))
        .expect("the placed breaker");
    let function = rep.functions[0].to_string();
    done(&mut s, "select.set", json!({ "ids": [function] }));
    let mut seen = Vec::new();
    for locale in ["en", "nl"] {
        done(&mut s, "view.language", json!({ "value": locale }));
        seen.push(row_value(&s, "function/template_entry").0);
    }
    done(&mut s, "view.language", json!({ "value": "en" }));
    s.shutdown();
    assert_ne!(seen[0], seen[1], "{seen:?}");
}

/// The Blocks navigator words a block's kind code in its kind column, and in its title and label
/// only where `ew-api` put the code there for a block without a title; a title that reads as a code stays as typed.
#[test]
fn a_block_title_that_reads_as_a_kind_code_stays_as_typed() {
    let tmp = Tmp::new("c53-p3-5");
    let dir = tmp.sample_copy();
    crate::common::copy_dir(std::path::Path::new(crate::common::SAMPLE), &dir);
    let blocks = dir.join("report-blocks.jsonl");
    let text = std::fs::read_to_string(&blocks).unwrap();
    // The standard layout's titles are in English and Dutch (doc 09 §3.8).
    let edited = text
        .replace(
            ",\"title\":{\"en-GB\":\"Table of Contents {n}\",\"nl-NL\":\"Inhoudsopgave {n}\"}",
            "",
        )
        .replace(
            "\"title\":{\"en-GB\":\"Revision Overview\",\"nl-NL\":\"Revisieoverzicht\"}",
            "\"title\":\"revision_overview\"",
        );
    assert!(edited != text);
    std::fs::write(&blocks, edited).unwrap();
    // The blocks of the table of contents and of the revision overview, by their kind.
    let block = |kind: &str| {
        text.lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|b| b["source"]["kind"] == kind)
            .and_then(|b| b["id"].as_str().map(str::to_owned))
            .unwrap_or_else(|| panic!("the sample's {kind} block"))
    };
    let (toc, revisions) = (block("toc"), block("revision_overview"));
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": true}),
    );
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
        seen.push(row(&shell, &toc));
        seen.push(row(&shell, &revisions));
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

/// A size in a sentence of the part dialog is formatted at the edge in the window's language: the fit hands out
/// "0.5" and the Dutch window writes "0,5", as the rule messages do (`DECIMAL`).
#[test]
fn a_misfit_sentence_writes_its_sizes_in_the_windows_language() {
    let args = ["1", "0.5", "1.5", "7", "2.5"]
        .into_iter()
        .map(|a| ew_api::TextArg::Plain(a.to_owned()))
        .collect();
    let text = ew_api::Text::checked_by_macro("misfit-clamp", "", args);
    assert_eq!(
        strings::shipped_renders(&text),
        [
            "Its clamp 1 takes 0.5 to 1.5 mm²; wire 7 is 2.5 mm²",
            "Zijn klem 1 neemt 0,5 tot 1,5 mm²; draad 7 is 2,5 mm²",
        ]
    );
}

/// One thing, one word: a strip's accessory row in Properties is named by the accessory kind's own word in both
/// languages (the marker carrier is no "strip marker"), and the end brackets' row, which counts the pair, by its plural.
#[test]
fn a_strips_accessory_rows_take_the_kinds_word() {
    for (locale, marker, brackets) in [
        ("en", "Marker carrier", "End brackets"),
        ("nl", "Markeringsdrager", "Eindsteunen"),
    ] {
        assert!(strings::set_locale(locale));
        assert_eq!(
            strings::render(&ew_api::output::accessory_kind_word("marker")),
            marker
        );
        assert_eq!(strings::field_label("device", "derived.accessory.marker"), marker);
        assert_eq!(
            strings::field_label("device", "derived.accessory.end_clamp"),
            brackets
        );
        assert!(strings::field_help("device", "derived.accessory.marker").is_some());
    }
    assert!(strings::set_locale("en"));
}

/// A carrier in a Dutch window reads in Dutch from the one home of the layout's words (`Session::layout_words`): a
/// selected duct is headed "Kabelgoot 1", and a placement's Carrier row names its rail and, on the plate itself, the
/// plate in words, never "Plate".
#[test]
fn a_carrier_reads_in_the_windows_language() {
    let tmp = Tmp::new("carrier-nl");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    done(&mut shell, "view.language", json!({"value": "nl"}));
    let duct = shell
        .panel_tables()
        .expect("902's tables")
        .carriers
        .iter()
        .find(|c| c.axis.is_none())
        .map(|c| c.id)
        .expect("the duct");
    done(&mut shell, "select.set", json!({"ids": [duct.to_string()]}));
    let title = |s: &ShellModel| s.properties().sheet().expect("a sheet").header.title.clone();
    assert_eq!(title(&shell), "Kabelgoot 1");
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    let placement = shell
        .panel_tables()
        .expect("902's tables")
        .placements
        .iter()
        .find(|p| p.tag == "-102F1")
        .map(|p| p.id)
        .expect("-102F1 on 902");
    done(&mut shell, "select.set", json!({"ids": [placement.to_string()]}));
    let (carrier, _) = row_value(&shell, "placement/derived.carrier");
    assert!(carrier.starts_with("Rail "), "{carrier}");
    done(&mut shell, "view.language", json!({"value": "en"}));
    done(&mut shell, "select.set", json!({"ids": [duct.to_string()]}));
    assert_eq!(title(&shell), "Duct 1");
    shell.shutdown();
}

/// Every phrase of a Properties chip reads in the window's language: in Dutch the cross-section of 102's wire from the
/// breaker -102F1 names the breaker that protects it in Dutch, where its "protected by …, rated 16 A" stood in English
/// among the Dutch phrases.
#[test]
fn every_phrase_of_a_properties_chip_reads_in_dutch() {
    let tmp = Tmp::new("ft2334");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["wire:-102F1:2 -> -X1021:1/a"]}),
    );
    done(&mut shell, "view.language", json!({ "value": "nl" }));
    let chip = {
        let sheet = shell.properties().sheet().expect("a sheet");
        let (_, row) = ew_viewmodel::properties::find_row(sheet, "connection/derived.csa").expect("the row");
        ew_viewmodel::properties::chip_text(&row.provenance)
    };
    assert!(
        chip.contains("beveiligd door -102F1") && !chip.contains("protected by"),
        "{chip}"
    );
    done(&mut shell, "view.language", json!({ "value": "en" }));
    shell.shutdown();
}
