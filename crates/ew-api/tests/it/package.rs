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
    assert!(names.contains(&"Lists/SAMPLE-E Wire list.xlsx"), "{names:?}");
    assert!(
        names.iter().any(|n| n.starts_with("Labels/")) && names.iter().any(|n| n.starts_with("Drilling/"))
    );
    let contents = String::from_utf8(entry(&entries, |n| n == "Contents.txt").data.clone()).expect("UTF-8");
    let contents = contents.replace(env!("CARGO_PKG_VERSION"), "<version>");
    let expected =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected/package/Contents.txt");
    ew_testdir::compare_or_bless(&expected, contents.as_bytes());
}

/// Each list of the package is the list's Excel export: the wire list's sheet is the one `ew export report --kind
/// wire_list --format xlsx` writes; every strip of the sample has its rows in the assembly lists; Contents counts the
/// field wires without a length, with their reason.
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
    let member = entry(&entries, |n| n == "Lists/SAMPLE-E Wire list.xlsx");
    assert_eq!(sheet(&member.data), sheet(&wires));
    let assembly = sheet(&entry(&entries, |n| n.starts_with("Lists/") && n.contains("Assembly")).data);
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
    let pages = pdf_texts(&entry(&entries, |n| n.ends_with("drawings.pdf")).data);
    assert!(!pages.is_empty());
    assert!(pages.iter().all(|p| p.contains("Draft V1.0: not released")));
    let release = released(|_, _| {}, true);
    let plan = release.package_plan(&ask());
    assert!(!plan.draft() && !plan.refused(), "{:?}", plan.errors());
    assert!(plan.findings().iter().any(|f| f.rule == "EW-MFG-001"));
    let (_, entries) = package(&release, &ask());
    let pdf = entry(&entries, |n| n.ends_with("drawings.pdf"));
    assert!(pdf.name == "SAMPLE-E V1.0 drawings.pdf");
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
/// were laid, so their ways no longer hold (EW-LAY-011).
#[test]
fn a_release_package_with_an_open_error_is_refused_naming_it() {
    let s = released(
        |probe, records| {
            let x1031 = probe.resolve_reference("tag:-X1031").expect("-X1031");
            for r in records.iter_mut() {
                if let Record::Placement(p) = r
                    && p.device.as_uuid() == x1031
                {
                    p.pose.t[0] = ew_units::Nm(p.pose.t[0].0 + 10_000_000);
                }
            }
        },
        false,
    );
    let plan = s.package_plan(&ask());
    assert!(plan.refused());
    match s.package_job(&ask()) {
        Err(PackageError::Refused(errors)) => {
            assert!(errors.iter().any(|e| e.contains("no longer hold")), "{errors:?}");
        }
        other => panic!("refused: {other:?}"),
    }
}
