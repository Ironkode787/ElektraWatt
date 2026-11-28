// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The package for the workshop on the sample (doc 13-package §8): the same bytes twice, a manifest that names every
//! other entry with its hash, the lists as their Excel exports, Contents with the wires without a length, the draft
//! mark on a draft's pages and none on a release's, Without personal data, and a release package refused while an
//! error of the release check stands.

use crate::common;

use std::sync::Arc;

use ew_api::output::{PdfDate, RowExport, RowsFormat, RowsOutput, row_output, workbook_dump};
use ew_api::{PackageAsk, PackageError, PackageMember, Session};
use ew_schema::{Date, Record, RevisionState};
use ew_zip::{Entry, ZipLimits};

/// The package's date in these tests.
const DATE: PdfDate = PdfDate::ymd(2026, 11, 27);

fn ask() -> PackageAsk {
    PackageAsk {
        date: Some(DATE),
        ..PackageAsk::default()
    }
}

/// The package of `s` as asked, its entries read back.
fn package(s: &Session, ask: &PackageAsk) -> (Vec<u8>, Vec<Entry>) {
    let bytes = s
        .package_job(ask)
        .expect("a package")
        .run(None)
        .expect("written")
        .bytes;
    let entries = ew_zip::read(&bytes, &ZipLimits::default()).expect("a ZIP");
    (bytes, entries)
}

fn entry(entries: &[Entry], kind_path: impl Fn(&str) -> bool) -> &Entry {
    entries.iter().find(|e| kind_path(&e.name)).expect("the entry")
}

/// The entries the manifest names of the member kind `kind` (`wire_list`, `pages`), in byte order of their paths.
fn of_kind<'a>(entries: &'a [Entry], kind: &str) -> Vec<&'a Entry> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&entry(entries, |n| n == "manifest.json").data).expect("JSON");
    let paths: Vec<&str> = manifest["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter(|m| m["kind"] == kind)
        .filter_map(|m| m["path"].as_str())
        .collect();
    entries
        .iter()
        .filter(|e| paths.contains(&e.name.as_str()))
        .collect()
}

/// The one entry of the member kind `kind`.
fn one_of_kind<'a>(entries: &'a [Entry], kind: &str) -> &'a Entry {
    match of_kind(entries, kind).as_slice() {
        [e] => e,
        other => panic!("{} entries of {kind}", other.len()),
    }
}

/// The text of every page of the PDF `bytes`.
fn pdf_texts(bytes: &[u8]) -> Vec<String> {
    let bytes: Arc<[u8]> = Arc::from(bytes);
    let pages = ew_scene::pdf_text::pdf_page_labels(Arc::clone(&bytes))
        .expect("a PDF")
        .len();
    (1..=u32::try_from(pages).expect("pages"))
        .map(|p| {
            ew_scene::pdf_text::pdf_page_text(Arc::clone(&bytes), p)
                .expect("the page")
                .runs
                .iter()
                .map(|r| r.text.clone())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

/// The sample with its revision V1.0 released, the records edited by `edit` first.
fn released(edit: impl FnOnce(&Session, &mut Vec<Record>), without_ways: bool) -> Session {
    let probe = common::sample();
    let (project, mut records, lib) = common::sample_records();
    if without_ways {
        records.retain(|r| !matches!(r, Record::CommittedRoute(_)));
    }
    for r in &mut records {
        if let Record::Revision(rev) = r {
            rev.state = RevisionState::Released;
            rev.date = Some(Date::new(2026, 11, 28).expect("a date"));
        }
    }
    edit(&probe, &mut records);
    common::session_of(project, records, lib)
}

/// -X1031 10 mm further along its rail, after its wires were laid: their ways no longer hold (EW-LAY-011, an error).
fn move_x1031(probe: &Session, records: &mut [Record]) {
    let x1031 = probe.resolve_reference("tag:-X1031").expect("-X1031");
    for r in records.iter_mut() {
        if let Record::Placement(p) = r
            && p.device.as_uuid() == x1031
        {
            p.pose.t[0] = ew_units::Nm(p.pose.t[0].0 + 10_000_000);
        }
    }
}

/// Two packages of the sample are the same bytes; the manifest names every other entry once, with its kind, its size
/// and its SHA-256, in byte order of the paths; the drawings, each workshop list, the label files and the drawings for
/// drilling are there, in their folders; the sample's Contents is as blessed.
#[test]
fn the_package_is_the_same_bytes_twice_and_its_manifest_names_each_entry() {
    let s = common::sample();
    let plan = s.package_plan(&ask());
    assert!(plan.draft() && plan.revision.as_deref() == Some("V1.0"));
    assert_eq!(plan.file_name, "SAMPLE-E V1.0 2026-11-27 draft package.zip");
    let (one, entries) = package(&s, &ask());
    let (two, _) = package(&s, &ask());
    assert!(one == two, "the same bytes");
    // No member holds the clock, which two writes in one second would not show: given a day far from today, every
    // workbook's core properties and every PDF's creation date are that day at midnight.
    let dated = PackageAsk {
        date: Some(PdfDate::ymd(2001, 2, 3)),
        ..PackageAsk::default()
    };
    let (_, old) = package(&s, &dated);
    let (mut books, mut pdfs) = (0, 0);
    for e in &old {
        if e.name.ends_with(".xlsx") {
            let parts = ew_zip::read(&e.data, &ZipLimits::default()).expect("a workbook");
            let core =
                String::from_utf8(entry(&parts, |n| n == "docProps/core.xml").data.clone()).expect("UTF-8");
            for tag in ["created", "modified"] {
                let at = format!(
                    "<dcterms:{tag} xsi:type=\"dcterms:W3CDTF\">2001-02-03T00:00:00Z</dcterms:{tag}>"
                );
                assert!(core.contains(&at), "{}: {core}", e.name);
            }
            books += 1;
        } else if e.name.ends_with(".pdf") {
            let date = b"D:20010203000000";
            assert!(e.data.windows(date.len()).any(|w| w == date), "{}", e.name);
            pdfs += 1;
        }
    }
    assert!(books > 0 && pdfs > 0, "{books} workbooks, {pdfs} PDFs");
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "in byte order of the paths");
    let manifest: serde_json::Value =
        serde_json::from_slice(&entry(&entries, |n| n == "manifest.json").data).expect("JSON");
    assert_eq!(manifest["draft"], true);
    assert_eq!(manifest["revision"]["state"], "open");
    assert_eq!(manifest["date"], "2026-11-27");
    let members = manifest["members"].as_array().expect("members");
    let others: Vec<&Entry> = entries.iter().filter(|e| e.name != "manifest.json").collect();
    assert_eq!(members.len(), others.len());
    for e in others {
        let m = members
            .iter()
            .find(|m| m["path"] == e.name.as_str())
            .unwrap_or_else(|| panic!("{} in the manifest", e.name));
        assert_eq!(m["bytes"], e.data.len());
        let hex: String = <sha2::Sha256 as sha2::Digest>::digest(&e.data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(m["sha256"], hex);
    }
    let kinds: Vec<&str> = members.iter().filter_map(|m| m["kind"].as_str()).collect();
    for m in PackageMember::all() {
        assert!(kinds.contains(&m.key()), "{} in the package", m.key());
    }
    // Their names and folders are the blessed Contents'.
    let contents = String::from_utf8(entry(&entries, |n| n == "Contents.txt").data.clone()).expect("UTF-8");
    let contents = contents.replace(env!("CARGO_PKG_VERSION"), "<version>");
    let expected =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected/package/Contents.txt");
    ew_testdir::compare_or_bless(&expected, contents.as_bytes());
}

/// Each list of the package is the list's Excel export: the wire list's file is the one `ew export report --kind
/// wire_list --format xlsx` writes, byte for byte, its hidden sheet `_ew` with the session's revision and store
/// generation too (the package is made on a copy of the snapshot); every strip of the sample has its rows in the
/// assembly lists; Contents counts the field wires without a length, with their reason.
#[test]
fn the_lists_are_the_lists_exports_and_contents_counts_the_wires_without_a_length() {
    let s = common::sample();
    let (_, entries) = package(&s, &ask());
    let sheet = |bytes: &[u8]| {
        let dump = workbook_dump(bytes).expect("a workbook");
        dump.split("\n[_ew").next().unwrap_or_default().to_owned()
    };
    let q = s.query();
    let RowsOutput::Xlsx(wires) = row_output(
        &q,
        &RowExport::Kind("wire_list".to_owned()),
        None,
        &RowsFormat::Xlsx(s.workbook_meta(Some(DATE))),
    )
    .expect("the wire list")
    .output
    else {
        panic!("a workbook")
    };
    drop(q);
    let member = one_of_kind(&entries, "wire_list");
    assert!(
        member.data == wires,
        "the wire list's file:\n{}\n---\n{}",
        workbook_dump(&member.data).unwrap_or_default(),
        workbook_dump(&wires).unwrap_or_default()
    );
    let assembly = sheet(&one_of_kind(&entries, "assembly_list").data);
    for strip in ["-X1011", "-X1021", "-X1031"] {
        assert!(assembly.contains(strip), "{strip}");
    }
    let contents = String::from_utf8(entry(&entries, |n| n == "Contents.txt").data.clone()).expect("UTF-8");
    assert!(contents.starts_with('\u{feff}') && contents.contains("\r\n"));
    let site = s
        .package_plan(&ask())
        .without_length
        .into_iter()
        .find(|n| n.why == ew_api::NoLengthWhy::Site)
        .expect("field wires without a length");
    assert!(
        contents.contains(&format!(
            "{} wires have no length: one end of each is made on site ({})",
            site.count,
            site.numbers.join(", ")
        )),
        "{contents}"
    );
}

/// A draft's pages carry the draft mark; the package of a release carries none, and one whose only finding is EW-MFG-001
/// (wires without a length: an info) is written.
#[test]
fn a_drafts_pages_carry_the_mark_and_a_releases_none() {
    let draft = common::sample();
    let (_, entries) = package(&draft, &ask());
    let pages = pdf_texts(&one_of_kind(&entries, "pages").data);
    assert!(!pages.is_empty());
    assert!(pages.iter().all(|p| p.contains("Draft V1.0: not released")));
    let release = released(|_, _| {}, true);
    let plan = release.package_plan(&ask());
    assert!(!plan.draft() && !plan.refused(), "{:?}", plan.errors());
    assert!(plan.findings().iter().any(|f| f.rule == "EW-MFG-001"));
    let (_, entries) = package(&release, &ask());
    let pdf = one_of_kind(&entries, "pages");
    assert!(pdf_texts(&pdf.data).iter().all(|p| !p.contains("Draft")));
    let manifest: serde_json::Value =
        serde_json::from_slice(&entry(&entries, |n| n == "manifest.json").data).expect("JSON");
    assert_eq!(manifest["revision"]["state"], "released");
    assert_eq!(manifest["date"], "2026-11-28", "the release's date");
}

/// Without personal data no entry names a party marked as personal data (the sample's panel builder, and its end user
/// marked so here, whom the title blocks print); with them kept, the drawings name the end user; the session keeps both.
#[test]
fn without_personal_data_no_entry_names_the_personal_parties() {
    let mut s = common::sample();
    let mut parties = serde_json::to_value(&s.query().project().parties).expect("the parties");
    for p in parties.as_array_mut().expect("a list") {
        if p["role"] == "end_user" {
            p["personal_data"] = serde_json::Value::Bool(true);
        }
    }
    let project = s.query().project().id.to_string();
    common::run_cmd(
        &mut s,
        "fields.set",
        serde_json::json!({"kind": "project", "targets": [project], "path": "parties", "value": parties}),
    );
    let named = |entries: &[Entry], party: &str| {
        entries.iter().any(|e| {
            if e.name.ends_with(".pdf") {
                pdf_texts(&e.data).iter().any(|p| p.contains(party))
            } else if e.name.ends_with(".xlsx") {
                workbook_dump(&e.data).is_ok_and(|d| d.contains(party))
            } else {
                String::from_utf8_lossy(&e.data).contains(party)
            }
        })
    };
    let (_, kept) = package(&s, &ask());
    assert!(
        named(&kept, "Sample End User"),
        "the kept drawings name the end user"
    );
    let ask = PackageAsk {
        omit_personal: true,
        ..ask()
    };
    let (_, left_out) = package(&s, &ask);
    for party in ["Sample End User", "Sample Panel Builder"] {
        assert!(!named(&left_out, party), "{party}");
    }
    assert!(named(&left_out, "Sample Customer"), "a party not marked stays");
    let manifest: serde_json::Value =
        serde_json::from_slice(&entry(&left_out, |n| n == "manifest.json").data).expect("JSON");
    assert_eq!(manifest["personal_data"], "left_out");
    let parties = serde_json::to_string(&s.query().project().parties).expect("JSON");
    assert!(parties.contains("Sample End User"), "the session keeps it");
}

/// A release package is refused while an error of the release check stands, naming it: -X1031 moved after its wires
/// were laid, so their ways no longer hold (EW-LAY-011). The refusal carries the findings, so a Dutch window says it in
/// Dutch and the command line in English.
#[test]
fn a_release_package_with_an_open_error_is_refused_naming_it() {
    let s = released(|probe, records| move_x1031(probe, records), false);
    let plan = s.package_plan(&ask());
    assert!(plan.refused());
    match s.package_job(&ask()) {
        Err(e @ PackageError::Refused(_)) => {
            let english = e.to_string();
            assert!(english.contains("no longer hold"), "{english}");
            assert!(
                english.contains("stop a release package") || english.contains("stops a release package")
            );
            let dutch = e.words("nl-NL");
            assert!(
                dutch.contains("houd") && dutch.contains("vrijgavepakket tegen: "),
                "{dutch}"
            );
            assert!(!dutch.contains("no longer hold"), "{dutch}");
        }
        other => panic!("refused: {other:?}"),
    }
}

/// A marker of one's own (owner, 2026-11-26): the sample's stock has no marker that marks cables, so the label files'
/// row says so; a cable marker made here and added to the stock prints the cable markers, the line goes, and the
/// package's cable markers' file names the part.
#[test]
fn a_cable_marker_of_ones_own_prints_the_cable_markers() {
    let mut s = common::sample();
    let lacks = |s: &Session| {
        s.package_plan(&ask())
            .members
            .into_iter()
            .find(|m| m.member == PackageMember::Labels)
            .expect("the label files")
            .lacks
    };
    assert_eq!(lacks(&s).len(), 1, "the cable markers lack a marker part");
    let (part, mut doc) = common::wire_part(0x7301, "MY-CM-16", "1.5", &serde_json::json!(["BK"]));
    doc["kind"] = serde_json::json!("marker");
    doc["sections"] = serde_json::json!({"marker": {"marks": "cable", "lines": 1, "chars": 16}});
    let parsed = ew_schema::LibDocument::from_value(doc.clone()).expect("a part document");
    doc["sha256"] = serde_json::json!(
        ew_schema::Sha256(ew_format::library_copy_hash(&parsed).expect("a hash")).to_string()
    );
    let mut stock = serde_json::to_value(&s.query().project().settings.workshop.stock).expect("the stock");
    stock
        .as_array_mut()
        .expect("a list")
        .push(serde_json::json!({"copy": part.to_string()}));
    common::run_cmd(
        &mut s,
        "workshop.stock",
        serde_json::json!({"stock": stock, "copies": [doc]}),
    );
    assert!(lacks(&s).is_empty());
    let (_, entries) = package(&s, &ask());
    let labels = of_kind(&entries, "labels");
    assert!(
        labels.iter().any(|e| e.name.contains("MY-CM-16")),
        "{:?}",
        labels.iter().map(|e| &e.name).collect::<Vec<_>>()
    );
}

/// A project in Dutch names its package's folders and files in Dutch: `Inhoud.txt`, `Lijsten/` and each list by its
/// kind's Dutch name, as Report block… names it; its Contents says what stood in Dutch, the severity too (a draft whose
/// -X1031 moved after its wires were laid: EW-LAY-011, an error, "fout").
#[test]
fn a_dutch_project_names_its_files_in_dutch() {
    let probe = common::sample();
    let (project, mut records, lib) = common::sample_records();
    move_x1031(&probe, &mut records);
    let mut s = common::session_of(project, records, lib);
    common::run_cmd(
        &mut s,
        "project.settings.set",
        serde_json::json!({"member": "languages", "value": {"source": "nl-NL"}}),
    );
    let (_, entries) = package(&s, &ask());
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Inhoud.txt"), "{names:?}");
    assert!(names.contains(&"Lijsten/SAMPLE-E Draadlijst.xlsx"), "{names:?}");
    assert!(
        names.iter().any(|n| n.starts_with("Boortekeningen/")),
        "{names:?}"
    );
    let contents = String::from_utf8(entry(&entries, |n| n == "Inhoud.txt").data.clone()).expect("UTF-8");
    assert!(contents.contains("  EW-LAY-011 fout: "), "{contents}");
}

/// Without personal data on a released revision changes nothing but the parties left out (doc 13-package §8.7): the
/// copy is no edit of the project, so no revision opens in it. Every page and every drawing for drilling read the
/// released V1.0 and none a draft or a V2.0; the manifest and Contents say V1.0 released; and every member is the same
/// bytes as the package with personal data (the sample prints no personal party), the manifest aside.
#[test]
fn without_personal_data_a_release_stays_the_release() {
    let s = released(|_, _| {}, false);
    let kept = package(&s, &ask()).1;
    let omit = PackageAsk {
        omit_personal: true,
        ..ask()
    };
    let (_, left_out) = package(&s, &omit);
    for e in &left_out {
        if e.name.ends_with(".pdf") {
            for page in pdf_texts(&e.data) {
                assert!(
                    !page.contains("V2.0") && !page.contains("Draft"),
                    "{}: {page}",
                    e.name
                );
            }
        }
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&entry(&left_out, |n| n == "manifest.json").data).expect("JSON");
    assert_eq!(manifest["revision"]["state"], "released");
    assert_eq!(manifest["revision"]["label"], "V1.0");
    let contents = String::from_utf8(entry(&left_out, |n| n == "Contents.txt").data.clone()).expect("UTF-8");
    assert!(
        contents.contains("revision V1.0, released on 2026-11-28"),
        "{contents}"
    );
    assert_eq!(kept.len(), left_out.len());
    for (a, b) in kept.iter().zip(&left_out) {
        assert_eq!(a.name, b.name);
        if a.name != "manifest.json" {
            assert!(a.data == b.data, "{} differs", a.name);
        }
    }
}

/// A project without a panel layout: Route lays none of its wires, so Contents counts every wire without a length under
/// that reason, and the release check names none of them (no EW-MFG-001).
#[test]
fn a_project_without_a_layout_counts_its_wires_without_a_length() {
    let (project, mut records, lib) = common::sample_records();
    // The layout's records go, and what shows them on a page (a surface's view, a legend).
    records.retain(|r| {
        r.kind()
            .is_none_or(|k| ew_schema::Kind::file_rule(k) != ew_schema::FileRule::LayoutFile)
            && !matches!(r, Record::PageItem(i) if format!("{:?}", i.kind).starts_with("SurfaceView"))
            && !matches!(r, Record::ReportBlock(b) if b.layout_page.is_some())
    });
    let s = common::session_of(project, records, lib);
    let plan = s.package_plan(&ask());
    let none = plan
        .without_length
        .iter()
        .find(|n| n.why == ew_api::NoLengthWhy::NoLayout)
        .expect("the wires of a project without a layout");
    assert!(!none.in_a_finding());
    assert!(plan.findings().iter().all(|f| f.rule != "EW-MFG-001"));
    let (_, entries) = package(&s, &ask());
    let contents = String::from_utf8(entry(&entries, |n| n == "Contents.txt").data.clone()).expect("UTF-8");
    assert!(
        contents.contains(&format!(
            "{} wires have no length: this project has no panel layout",
            none.count
        )),
        "{contents}"
    );
}

/// What the wire list and the label files need of the workshop stock and do not find is said under their rows and in
/// Contents, in the words of the field's reader, with parts made here (owner, 2026-11-26): a ferrule without a sleeve
/// length, a wire marker without its characters per line, the sample's ring lug end (101.4, set by hand) that no stock
/// part takes, and the wires that no wire part of the stock matches (the sample's stock holds none). A line about one
/// part's missing field carries the part and the field, which Edit part… opens; a line about the stock carries none.
#[test]
fn what_the_lists_lack_of_the_stock_is_said() {
    let mut s = common::sample();
    let own = |n: u128, number: &str, kind: &str, sections: serde_json::Value| {
        let (part, mut doc) = common::wire_part(n, number, "1.5", &serde_json::json!(["BK"]));
        doc["kind"] = serde_json::json!(kind);
        doc["sections"] = sections;
        let parsed = ew_schema::LibDocument::from_value(doc.clone()).expect("a part document");
        doc["sha256"] = serde_json::json!(
            ew_schema::Sha256(ew_format::library_copy_hash(&parsed).expect("a hash")).to_string()
        );
        (part, doc)
    };
    let (ferrule, ferrule_doc) = own(
        0x7401,
        "MY-FER-075",
        "ferrule",
        serde_json::json!({"ferrule": {"csa_min": "0.75", "csa_max": "0.75"}}),
    );
    let (marker, marker_doc) = own(
        0x7402,
        "MY-WM",
        "marker",
        serde_json::json!({"marker": {"marks": "wire", "lines": 1}}),
    );
    let mut stock = vec![
        serde_json::json!({"copy": ferrule.to_string()}),
        serde_json::json!({"copy": marker.to_string()}),
    ];
    stock.extend(
        serde_json::to_value(&s.query().project().settings.workshop.stock)
            .expect("the stock")
            .as_array()
            .expect("a list")
            .iter()
            .cloned(),
    );
    common::run_cmd(
        &mut s,
        "workshop.stock",
        serde_json::json!({"stock": stock, "copies": [ferrule_doc, marker_doc]}),
    );
    let plan = s.package_plan(&ask());
    let member = |m: PackageMember| plan.members.iter().find(|p| p.member == m).expect("the member");
    let lacks_of =
        |m: PackageMember| -> Vec<String> { member(m).lacks.iter().map(|l| l.text.to_string()).collect() };
    let wires = lacks_of(PackageMember::List("wire_list"));
    let part_of = |said: &str| {
        member(PackageMember::List("wire_list"))
            .lacks
            .iter()
            .find(|l| l.text.to_string() == said)
            .map(|l| l.part)
    };
    assert_eq!(
        part_of("MY-FER-075: Without a sleeve length the wire list prints no strip length."),
        Some(Some(ew_derive::part_fields::PartLack {
            copy: ferrule,
            field: "sections.ferrule.sleeve"
        })),
        "the ferrule's line is about its sleeve length"
    );
    assert_eq!(
        part_of("No ring lug of the workshop stock takes 2.5 mm²: those ends are listed without a part."),
        Some(None),
        "a line about the stock names no part"
    );
    for said in [
        "MY-FER-075: Without a sleeve length the wire list prints no strip length.",
        "No ring lug of the workshop stock takes 2.5 mm²: those ends are listed without a part.",
    ] {
        assert!(wires.iter().any(|l| l == said), "{said}: {wires:#?}");
    }
    assert!(
        wires
            .iter()
            .any(|l| l.starts_with("No wire part of the workshop stock matches ")),
        "{wires:#?}"
    );
    let labels = lacks_of(PackageMember::Labels);
    assert!(
        labels
            .iter()
            .any(|l| l.starts_with("MY-WM: Without ") && l.contains("characters")),
        "{labels:#?}"
    );
    let (_, entries) = package(&s, &ask());
    let contents = String::from_utf8(entry(&entries, |n| n == "Contents.txt").data.clone()).expect("UTF-8");
    for said in wires.iter().chain(&labels) {
        assert!(contents.contains(&format!("  {said}")), "{said}: {contents}");
    }
}

/// A draft of a project that carries no date (no edited or created date, none given): Contents says so, where it
/// said "made on 1980-01-01"; the members keep the fixed date, so two writes are the same bytes.
#[test]
fn a_project_without_a_date_says_so_in_contents() {
    let (mut project, records, lib) = common::sample_records();
    project.master.edited = None;
    project.master.created = None;
    let s = common::session_of(project, records, lib);
    let undated = PackageAsk::default();
    assert!(s.package_plan(&undated).undated);
    let (bytes, entries) = package(&s, &undated);
    let contents = String::from_utf8(entry(&entries, |n| n == "Contents.txt").data.clone()).expect("UTF-8");
    assert!(
        contents.contains("Package for the workshop, revision V1.0; the project carries no date\r\n"),
        "{contents}"
    );
    assert!(!contents.contains("1980"), "{contents}");
    assert_eq!(package(&s, &undated).0, bytes, "the same bytes");
    // A date given by the caller is named as before.
    assert!(!s.package_plan(&ask()).undated);
}
