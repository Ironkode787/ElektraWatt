// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Golden C (doc 20 §2.3, §20): a private QElectroTech project of the same control cabinet as Golden A,
//! imported by `ew_api::import::import_qet`, repaired by the command log
//! `golden/J-E-2416/variants/c-fix.ewcmd` and compared with Golden A (the CSV golden model).
//!
//! The source is private and never part of the repository. `EW_PRIVATE_CORPUS` names the directory that
//! holds it; the test picks the `.qet` file whose SHA-256 is [`SOURCE_SHA256`] (the record IDs of
//! `c-fix.ewcmd` refer to that exact file). Both tests are `#[ignore]`d and skip when the variable is unset.
//! They print counts, Golden A's own (public) strings and imported strings that pass the sanitiser
//! ([`San`]): a string prints as itself only when Golden A uses it too or when it is short and technical,
//! otherwise as `<Nch>`. Run them with
//! `EW_PRIVATE_CORPUS=<dir> cargo nextest run -p ew-api --test golden_c --run-ignored all --no-capture`.
//!
//! 1. `the_migration_report_has_the_expected_counts_per_code`: the findings of the import per code (the
//!    migration report groups and the lenient checks) against `expected/c-migration-report.csv`.
//! 2. `the_fixed_import_compared_with_golden_a`: replays `c-fix.ewcmd` on the import and prints one table of
//!    named comparisons with Golden A. [`HOLDS`] lists those that hold today; the test asserts them, so a
//!    regression of a repaired aspect fails, and a newly holding comparison is added to the list.

mod golden_common;
#[path = "page_output/golden_files.rs"]
#[allow(dead_code)]
mod golden_files;
#[path = "page_output/view.rs"]
#[allow(dead_code)]
mod view;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use ew_api::import::{Imported, StarterLibrary, import_qet};
use ew_api::{CmdLog, Session};
use ew_derive::ew_ids::Id;
use ew_derive::{DeviceTag, FunctionCps, PageOrder, PageRef, PotentialPartition, TagSource};
use ew_reports::BlockRows;
use ew_schema::{ConnectionKind, Device};
use sha2::{Digest, Sha256};

/// The SHA-256 of the Golden C source file.
const SOURCE_SHA256: &str = "94235575d350d98df2491f49b8474f769fd5c64912b560f41f30d8b555c2fafc";

/// The comparisons of `the_fixed_import_compared_with_golden_a` that hold today, with the number of
/// common keys each compares (titles of drawn folios, device tags, terminal tags). A comparison over keys
/// both sides have holds trivially when the common key set shrinks, so the count is asserted too.
const HOLDS: &[(&str, usize)] = &[
    ("pages.titles", 13),
    ("devices.tag_source", 35),
    ("terminals.function_text", 28),
    ("terminals.state", 28),
    ("devices.no_duplicate_common_tags", 0),
];

// ---------------------------------------------------------------------------------------------------------
// Source and import

/// The Golden C import, or `None` (and a message) when `EW_PRIVATE_CORPUS` is unset.
fn import_golden_c() -> Option<Imported> {
    let Some(dir) = std::env::var_os("EW_PRIVATE_CORPUS").map(PathBuf::from) else {
        eprintln!("EW_PRIVATE_CORPUS is not set; skipped");
        return None;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the corpus directory is readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("qet")))
        .collect();
    files.sort();
    let bytes = files
        .iter()
        .filter_map(|p| std::fs::read(p).ok())
        .find(|b| format!("{:x}", Sha256::digest(b)) == SOURCE_SHA256)
        .expect("the Golden C source changed: the record IDs of c-fix.ewcmd no longer apply");
    let library = StarterLibrary::from_dir(&view::repo().join("content")).expect("the starter content loads");
    Some(import_qet("golden-c.qet", bytes, &library).expect("the Golden C import is accepted"))
}

// ---------------------------------------------------------------------------------------------------------
// Sanitiser

/// Decides which imported strings may be printed.
struct San {
    allow: BTreeSet<String>,
}

impl San {
    /// The allow-list of Golden A: device short tags with and without the leading `-`, folio labels,
    /// cross-reference texts, terminal designations and function texts, page titles.
    fn from_golden_a(a: &Session) -> San {
        let mut allow = BTreeSet::new();
        for d in devices(a) {
            allow.insert(d.tag.trim_start_matches('-').to_owned());
            allow.insert(d.tag);
            if let Some(t) = d.terminal {
                allow.insert(t.designation);
                allow.insert(t.function_text);
            }
        }
        allow.extend(golden_common::folios(a));
        allow.extend(golden_common::xrefs(a).into_values());
        let q = a.query();
        for (_, p) in q.pages().iter() {
            allow.insert(p.title.as_neutral().unwrap_or("").to_owned());
        }
        allow.remove("");
        San { allow }
    }

    /// The string itself when it is allowed or technical, else `<Nch>`.
    fn tok(&self, s: &str) -> String {
        if self.allow.contains(s) {
            return s.to_owned();
        }
        let (mut run, mut max_run) = (0, 0);
        for c in s.chars() {
            if c.is_ascii_digit() {
                run += 1;
                max_run = max_run.max(run);
            } else {
                run = 0;
            }
        }
        let technical = s.len() <= 14
            && s.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || "-+:.?/ ".contains(c));
        if technical && max_run < 4 {
            s.to_owned()
        } else {
            format!("<{}ch>", s.chars().count())
        }
    }

    /// A multiset difference as `item` or `item x n`, sorted.
    fn list(&self, m: &BTreeMap<String, usize>) -> String {
        m.iter()
            .map(|(k, n)| {
                if *n > 1 {
                    format!("{} x{n}", self.tok(k))
                } else {
                    self.tok(k)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

// ---------------------------------------------------------------------------------------------------------
// What the tests read from a session

struct TerminalInfo {
    designation: String,
    function_text: String,
    state: String,
}

struct DeviceInfo {
    id: Id<Device>,
    tag: String,
    /// The class of the tag source (`frozen`, `child`, `terminal`, `scheme:<format>`).
    source: String,
    terminal: Option<TerminalInfo>,
}

fn devices(s: &Session) -> Vec<DeviceInfo> {
    let q = s.query();
    let mut out = Vec::new();
    for (id, d) in q.devices().iter() {
        let t = q.get::<DeviceTag>(id);
        let Some(t) = t.as_ref() else { continue };
        let source = match &t.source {
            TagSource::Frozen => "frozen".to_owned(),
            TagSource::Child => "child".to_owned(),
            TagSource::Terminal => "terminal".to_owned(),
            TagSource::Scheme { format, .. } => format!("scheme:{format}"),
            #[allow(unreachable_patterns)]
            _ => "other".to_owned(),
        };
        out.push(DeviceInfo {
            id,
            tag: t.short(),
            source,
            terminal: d.aspects.terminal.as_ref().map(|a| TerminalInfo {
                designation: a.designation.as_str().to_owned(),
                function_text: d.function_text.as_neutral().unwrap_or("").to_owned(),
                state: format!("{:?}", d.state),
            }),
        });
    }
    out
}

fn counts(items: impl IntoIterator<Item = String>) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for i in items {
        *m.entry(i).or_default() += 1;
    }
    m
}

/// The multiset difference `a - b`.
fn minus(a: &BTreeMap<String, usize>, b: &BTreeMap<String, usize>) -> BTreeMap<String, usize> {
    a.iter()
        .filter_map(|(k, n)| {
            let m = n.saturating_sub(b.get(k).copied().unwrap_or(0));
            (m > 0).then(|| (k.clone(), m))
        })
        .collect()
}

fn common(a: &BTreeMap<String, usize>, b: &BTreeMap<String, usize>) -> usize {
    a.iter()
        .map(|(k, n)| (*n).min(b.get(k).copied().unwrap_or(0)))
        .sum()
}

fn folio_kinds(s: &Session) -> BTreeMap<String, String> {
    let q = s.query();
    q.get::<PageOrder>(())
        .entries
        .iter()
        .map(|e| (e.folio.clone(), format!("{:?}", e.kind)))
        .collect()
}

/// The potentials as sets of `<device tag>:<cp designation>` restricted to the devices whose tag is in
/// `keep`; potentials that end up empty are dropped.
fn potential_sets(s: &Session, keep: &BTreeSet<String>) -> BTreeSet<BTreeSet<String>> {
    let q = s.query();
    let tags: BTreeMap<Id<Device>, String> = devices(s).into_iter().map(|d| (d.id, d.tag)).collect();
    let part = q.get::<PotentialPartition>(());
    let mut out = BTreeSet::new();
    for p in part.potentials() {
        let mut set = BTreeSet::new();
        for cp in &p.cps {
            let Some(dev) = q.functions().get(cp.function).and_then(|f| f.device) else {
                continue;
            };
            let Some(tag) = tags.get(&dev).filter(|t| keep.contains(*t)) else {
                continue;
            };
            let cps = q.get::<FunctionCps>(cp.function);
            let designation = cps
                .iter()
                .find(|c| c.idx == cp.idx)
                .map_or_else(|| "?".to_owned(), |c| c.designation.clone());
            set.insert(format!("{tag}:{designation}"));
        }
        if !set.is_empty() {
            out.insert(set);
        }
    }
    out
}

fn connection_kinds(s: &Session) -> BTreeMap<String, usize> {
    let q = s.query();
    counts(q.connections().iter().map(|(_, c)| {
        match &c.kind {
            ConnectionKind::Wire => "wire",
            ConnectionKind::Core(_) => "core",
            ConnectionKind::Bridge(_) => "bridge",
            ConnectionKind::Bus(_) => "bus",
            ConnectionKind::Overview => "overview",
            ConnectionKind::Internal => "internal",
            _ => "unknown",
        }
        .to_owned()
    }))
}

/// The records of a CSV text (RFC 4180 as the golden files write it: fields may be quoted with doubled
/// quotes and may hold line breaks).
fn csv_records(text: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => row.push(std::mem::take(&mut field)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                out.push(std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        out.push(row);
    }
    out
}

/// The `text` column of a text CSV.
fn text_column(csv: &str) -> Vec<String> {
    let records = csv_records(csv);
    let col = records[0]
        .iter()
        .position(|h| h == "text")
        .expect("a text column");
    records[1..].iter().map(|r| r[col].clone()).collect()
}

// ---------------------------------------------------------------------------------------------------------
// Test 1: the migration report

/// The findings of the import by code: the migration report groups and the lenient checks.
fn report_map(imported: &Imported) -> BTreeMap<String, u32> {
    let mut map: BTreeMap<String, u32> = BTreeMap::new();
    for g in &imported.migration_report().groups {
        *map.entry(g.code.to_string()).or_default() += g.count;
    }
    for g in &imported.live {
        *map.entry(g.rule.clone()).or_default() += g.count;
    }
    map
}

#[test]
#[ignore = "needs the private corpus (EW_PRIVATE_CORPUS)"]
fn the_migration_report_has_the_expected_counts_per_code() {
    let Some(imported) = import_golden_c() else { return };
    let actual = report_map(&imported);
    let text = std::fs::read_to_string(view::repo().join("golden/J-E-2416/expected/c-migration-report.csv"))
        .expect("the expected migration report is committed");
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("code,mf,doc20,as_built,status,note"));
    let mut expected: BTreeMap<String, u32> = BTreeMap::new();
    println!("code | mf | doc20 | as_built | actual | status");
    let mut listed = BTreeSet::new();
    for line in lines.filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split(',').collect();
        let as_built: u32 = f[3].parse().expect("a count");
        if as_built > 0 {
            expected.insert(f[0].to_owned(), as_built);
        }
        listed.insert(f[0].to_owned());
        println!(
            "{} | {} | {} | {as_built} | {} | {}",
            f[0],
            f[1],
            f[2],
            actual.get(f[0]).copied().unwrap_or(0),
            f[4]
        );
    }
    for (code, n) in &actual {
        if !listed.contains(code) {
            println!("{code} | - | - | - | {n} | not in the CSV");
        }
    }
    let actual_nonzero: BTreeMap<String, u32> = actual.into_iter().filter(|(_, n)| *n > 0).collect();
    assert_eq!(
        actual_nonzero, expected,
        "the findings per code differ from c-migration-report.csv"
    );
}

// ---------------------------------------------------------------------------------------------------------
// Test 2: the fixed import against Golden A

struct Cmp {
    name: String,
    a: usize,
    c: usize,
    common: usize,
    holds: bool,
}

/// Compares two multisets; `show` lists the one-sided items (A's as they are, C's sanitised).
fn multiset(
    name: &str,
    a: &BTreeMap<String, usize>,
    c: &BTreeMap<String, usize>,
    san: &San,
    show: bool,
    details: &mut Vec<String>,
) -> Cmp {
    let (a_only, c_only) = (minus(a, c), minus(c, a));
    if show {
        details.push(format!(
            "{name}: A only [{}]",
            a_only
                .iter()
                .map(|(k, n)| if *n > 1 { format!("{k} x{n}") } else { k.clone() })
                .collect::<Vec<_>>()
                .join(" ")
        ));
        details.push(format!("{name}: C only [{}]", san.list(&c_only)));
    }
    Cmp {
        name: name.to_owned(),
        a: a.values().sum(),
        c: c.values().sum(),
        common: common(a, c),
        holds: a_only.is_empty() && c_only.is_empty(),
    }
}

/// Counts of equal values over the keys both maps have.
fn equal_over_common(a: &BTreeMap<String, String>, c: &BTreeMap<String, String>) -> (usize, usize) {
    let both: Vec<_> = a.keys().filter(|k| c.contains_key(*k)).collect();
    let equal = both.iter().filter(|k| a[**k] == c[**k]).count();
    (both.len(), equal)
}

fn compare(a: &Session, c: &Session, san: &San, details: &mut Vec<String>) -> Vec<Cmp> {
    let mut out = Vec::new();

    // page_order.folios, page_order.kinds
    let (fa, fc) = (golden_common::folios(a), golden_common::folios(c));
    let mut cmp = multiset(
        "page_order.folios",
        &counts(fa.clone()),
        &counts(fc.clone()),
        san,
        true,
        details,
    );
    cmp.holds = fa == fc;
    out.push(cmp);
    let (ka, kc) = (folio_kinds(a), folio_kinds(c));
    let (both, equal) = equal_over_common(&ka, &kc);
    details.push(format!("page_order.kinds: {both} folios in both"));
    out.push(Cmp {
        name: "page_order.kinds".into(),
        a: ka.len(),
        c: kc.len(),
        common: equal,
        holds: equal == both,
    });

    // pages.titles: drawn pages in both, by folio label
    let titles = |s: &Session| -> BTreeMap<String, ew_mltext::MlText> {
        let q = s.query();
        let order = q.get::<PageOrder>(());
        order
            .entries
            .iter()
            .filter_map(|e| match e.page {
                PageRef::Drawn(p) => Some((e.folio.clone(), q.pages().get(p)?.title.clone())),
                _ => None,
            })
            .collect()
    };
    let (ta, tc) = (titles(a), titles(c));
    let both = ta.keys().filter(|k| tc.contains_key(*k)).count();
    let equal = ta.iter().filter(|(k, t)| tc.get(*k) == Some(t)).count();
    details.push(format!("pages.titles: {both} drawn folios in both"));
    out.push(Cmp {
        name: "pages.titles".into(),
        a: ta.len(),
        c: tc.len(),
        common: equal,
        holds: equal == both,
    });

    // devices.*, terminals.*
    let (da, dc) = (devices(a), devices(c));
    // The keyed comparisons below map tag → value, so a tag that C prints twice would collapse: count the
    // C tags that Golden A has and that C prints more than once.
    let a_tags: BTreeSet<&str> = da.iter().map(|d| d.tag.as_str()).collect();
    let duplicates: BTreeMap<String, usize> = counts(
        dc.iter()
            .filter(|d| a_tags.contains(d.tag.as_str()))
            .map(|d| d.tag.clone()),
    )
    .into_iter()
    .filter(|(_, n)| *n > 1)
    .collect();
    details.push(format!(
        "devices.no_duplicate_common_tags: [{}]",
        san.list(&duplicates)
    ));
    out.push(Cmp {
        name: "devices.no_duplicate_common_tags".into(),
        a: 0,
        c: duplicates.len(),
        common: duplicates.len(),
        holds: duplicates.is_empty(),
    });
    let split = |d: &[DeviceInfo], terminal: bool| -> Vec<String> {
        d.iter()
            .filter(|x| x.tag.contains(':') == terminal)
            .map(|x| x.tag.clone())
            .collect()
    };
    out.push(multiset(
        "devices.tags",
        &counts(split(&da, false)),
        &counts(split(&dc, false)),
        san,
        true,
        details,
    ));
    let class = |d: &[DeviceInfo], terminal: bool| -> BTreeMap<String, String> {
        d.iter()
            .filter(|x| x.tag.contains(':') == terminal)
            .map(|x| (x.tag.clone(), x.source.clone()))
            .collect()
    };
    let (ca, cc) = (class(&da, false), class(&dc, false));
    let (both, equal) = equal_over_common(&ca, &cc);
    details.push(format!("devices.tag_source: {both} tags in both"));
    out.push(Cmp {
        name: "devices.tag_source".into(),
        a: ca.len(),
        c: cc.len(),
        common: equal,
        holds: equal == both,
    });
    out.push(multiset(
        "terminals.tags",
        &counts(split(&da, true)),
        &counts(split(&dc, true)),
        san,
        true,
        details,
    ));
    let term = |d: &[DeviceInfo], f: fn(&TerminalInfo) -> String| -> BTreeMap<String, String> {
        d.iter()
            .filter_map(|x| x.terminal.as_ref().map(|t| (x.tag.clone(), f(t))))
            .collect()
    };
    for (name, f) in [
        (
            "terminals.function_text",
            (|t: &TerminalInfo| t.function_text.clone()) as fn(&TerminalInfo) -> String,
        ),
        ("terminals.state", |t: &TerminalInfo| t.state.clone()),
    ] {
        let (xa, xc) = (term(&da, f), term(&dc, f));
        let (both, equal) = equal_over_common(&xa, &xc);
        details.push(format!("{name}: {both} terminal tags in both"));
        out.push(Cmp {
            name: name.into(),
            a: xa.len(),
            c: xc.len(),
            common: equal,
            holds: equal == both,
        });
    }

    // wires.count
    let (wa, wc) = (connection_kinds(a), connection_kinds(c));
    details.push(format!("wires.count by kind: A {wa:?} C {wc:?}"));
    out.push(multiset("wires.count", &wa, &wc, san, false, details));

    // potentials.count, potentials.members
    let n_potentials = |s: &Session| s.query().get::<PotentialPartition>(()).potentials().count();
    let (pa, pc) = (n_potentials(a), n_potentials(c));
    out.push(Cmp {
        name: "potentials.count".into(),
        a: pa,
        c: pc,
        common: pa.min(pc),
        holds: pa == pc,
    });
    // The reference file stores no bridge (no `<terminal_strips>`, no bridge element): Golden A without its
    // bridge connections shows how much of the potential count that source limit explains.
    let ab = golden_common::batch();
    let unbridged = golden_common::session_with(&ab, |_, records| {
        records.retain(
            |r| !matches!(r, ew_schema::Record::Connection(x) if matches!(x.kind, ConnectionKind::Bridge(_))),
        );
    });
    let pu = n_potentials(&unbridged);
    out.push(Cmp {
        name: "potentials.count_a_without_bridges".into(),
        a: pu,
        c: pc,
        common: pu.min(pc),
        holds: pu == pc,
    });
    let keep: BTreeSet<String> = da
        .iter()
        .map(|d| d.tag.clone())
        .collect::<BTreeSet<_>>()
        .intersection(&dc.iter().map(|d| d.tag.clone()).collect())
        .cloned()
        .collect();
    let (sa, sc) = (potential_sets(a, &keep), potential_sets(c, &keep));
    out.push(Cmp {
        name: "potentials.members".into(),
        a: sa.len(),
        c: sc.len(),
        common: sa.intersection(&sc).count(),
        holds: sa == sc,
    });

    // xrefs
    let xr = |s: &Session| counts(golden_common::xrefs(s).into_values());
    out.push(multiset("xrefs", &xr(a), &xr(c), san, true, details));

    // text.<folio>
    let qc = c.query();
    let pages = view::golden_pages(&qc);
    let text_dir = golden_files::expected_dir("text");
    let mut expected_files: Vec<PathBuf> = std::fs::read_dir(&text_dir)
        .expect("the expected text files")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "csv"))
        .collect();
    expected_files.sort();
    let mut absent = Vec::new();
    for path in expected_files {
        let folio = path.file_stem().unwrap().to_string_lossy().into_owned();
        let Some(page) = pages.iter().find(|g| g.entry.folio == folio) else {
            absent.push(folio);
            continue;
        };
        let scene = ew_scene::page::page_scene(&page.view, ew_text::bundled());
        let c_texts = counts(text_column(&golden_files::text_csv(&scene)));
        let a_texts = counts(text_column(&std::fs::read_to_string(&path).unwrap()));
        out.push(multiset(
            &format!("text.{folio}"),
            &a_texts,
            &c_texts,
            san,
            false,
            details,
        ));
    }
    details.push(format!(
        "text: folios of Golden A that C does not have [{}]",
        absent.join(" ")
    ));

    // rows.<n>
    let rows_dir = view::repo().join("golden/J-E-2416/expected/rows");
    let mut row_files: Vec<PathBuf> = std::fs::read_dir(&rows_dir)
        .expect("the expected rows files")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "csv"))
        .collect();
    row_files.sort();
    let order = qc.get::<PageOrder>(());
    for path in row_files {
        let n = path.file_stem().unwrap().to_string_lossy().into_owned();
        let name = format!("rows.{n}");
        let a_lines = counts(std::fs::read_to_string(&path).unwrap().lines().map(str::to_owned));
        let block = order.entries.iter().find_map(|e| match e.page {
            PageRef::Generated { block, index: 0 } if e.folio == n => Some(block),
            _ => None,
        });
        match block {
            Some(block) => {
                let set = qc.get::<BlockRows>(block);
                let c_lines = counts(golden_common::rows_csv(&qc, &set).lines().map(str::to_owned));
                out.push(multiset(&name, &a_lines, &c_lines, san, false, details));
            }
            None => {
                details.push(format!("{name}: absent"));
                out.push(Cmp {
                    name,
                    a: a_lines.values().sum(),
                    c: 0,
                    common: 0,
                    holds: false,
                });
            }
        }
    }
    out
}

#[test]
#[ignore = "needs the private corpus (EW_PRIVATE_CORPUS)"]
fn the_fixed_import_compared_with_golden_a() {
    let Some(imported) = import_golden_c() else { return };
    let mut c = imported.session.expect("the model accepts the Golden C batch");
    let log_text = std::fs::read_to_string(golden_common::variant("c-fix.ewcmd")).unwrap_or_else(|e| {
        panic!(
            "golden/J-E-2416/variants/c-fix.ewcmd is missing or unreadable ({e}): Golden C has no fix log yet"
        )
    });
    let commands = CmdLog::parse(&log_text).expect("c-fix.ewcmd parses").len();
    let committed = golden_common::run(&mut c, "c-fix.ewcmd");
    assert_eq!(committed, commands, "every line of c-fix.ewcmd commits");

    let ab = golden_common::batch();
    let a = golden_common::session(&ab);
    let san = San::from_golden_a(&a);
    let mut details = Vec::new();
    let table = compare(&a, &c, &san, &mut details);

    println!("name | a_count | c_count | common | holds");
    for r in &table {
        println!("{} | {} | {} | {} | {}", r.name, r.a, r.c, r.common, r.holds);
    }
    for d in &details {
        println!("{d}");
    }
    let held: Vec<&str> = table
        .iter()
        .filter(|r| r.holds)
        .map(|r| r.name.as_str())
        .collect();
    println!("holding today: {held:?}");
    for (name, common) in HOLDS {
        let row = table
            .iter()
            .find(|r| r.name == *name)
            .unwrap_or_else(|| panic!("`{name}` is listed in HOLDS but not computed"));
        assert!(row.holds, "`{name}` held before and no longer does");
        assert_eq!(row.common, *common, "`{name}`: the number of common keys changed");
    }
}
