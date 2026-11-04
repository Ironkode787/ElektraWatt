// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The hostile corpus test (WP-S6.04 D10, SE-7; NFR-SEC-02): every file of `crates/ew-testkit/hostile/` (its index is
//! the folder's `README.md`) through the public entry point of its parser, the importers through WP-S3.09's front door,
//! with its verdict asserted whole (a refusal's text, or the findings a reader reports) and at most 256 MiB of extra
//! peak allocation each, counted by this binary's own global allocator ([`alloc`]). No time is asserted (owner's rule).
//! The SQL files are `ew-sqlview`'s (`the_hostile_sql_corpus_is_refused_file_by_file`): SQLite allocates through its
//! own allocator, not this one, and its limits are the sandbox's (`SQLITE_LIMIT_*`, the deadline).

mod alloc;
mod generate;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ew_api::import::{StarterLibrary, front_door, import_qet};
use ew_api::{MemoMode, Session, StoreOptions};

#[global_allocator]
static COUNTING: alloc::Counting = alloc::Counting;

/// NFR-SEC-02's bound: the extra peak allocation of one hostile file (doc 14 §11's 256 MB, §12 O3).
const BOUND: usize = 256 << 20;

/// The program name the opens give the store.
const APP: &str = "hostile-corpus";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn hostile() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("hostile")
}

// Allowed: the program boundary of a test, read once; the value never reaches a verdict.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// The files of the class `class`: the committed ones (the generated binaries compared with the generator, which
/// `EW_BLESS=1` writes) and the ones made at each run, by name.
fn files(class: &str) -> BTreeMap<String, Vec<u8>> {
    let prefix = format!("{class}/");
    for (path, bytes) in generate::committed() {
        if path.starts_with(&prefix) {
            let on_disk = hostile().join(path);
            if bless() {
                std::fs::create_dir_all(on_disk.parent().unwrap()).unwrap();
                std::fs::write(&on_disk, &bytes).unwrap();
            }
            let held =
                std::fs::read(&on_disk).unwrap_or_else(|e| panic!("{path}: {e} (EW_BLESS=1 writes it)"));
            assert!(
                held == bytes,
                "{path} is not what the generator makes (EW_BLESS=1 writes it)"
            );
        }
    }
    let mut out: BTreeMap<String, Vec<u8>> = std::fs::read_dir(hostile().join(class))
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    for (path, bytes) in generate::large() {
        if let Some(name) = path.strip_prefix(&prefix) {
            assert!(
                out.insert(name.to_owned(), bytes).is_none(),
                "{path} is generated, not committed"
            );
        }
    }
    out
}

/// Runs every file of `class` through `verdict` and asserts the verdicts whole, in file order, and each file's extra
/// peak allocation within [`BOUND`].
fn class(class: &str, expected: &[(&str, &str)], verdict: impl Fn(&str, &[u8]) -> String) {
    let files = files(class);
    let mut got = Vec::new();
    for (name, bytes) in &files {
        let (text, peak) = alloc::extra_peak(|| verdict(name, bytes));
        assert!(
            peak <= BOUND,
            "{class}/{name}: {peak} bytes of extra peak allocation, over {BOUND}"
        );
        got.push((name.clone(), text));
    }
    let want: Vec<(String, String)> = expected
        .iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
        .collect();
    assert_eq!(got, want, "the verdicts of {class}/");
}

/// The starter content the importers resolve built-in keys in, read once before any measurement.
fn library() -> &'static StarterLibrary {
    static LIB: OnceLock<StarterLibrary> = OnceLock::new();
    LIB.get_or_init(|| StarterLibrary::from_dir(&repo().join("content")).expect("the starter content"))
}

/// The import's report entries, one `<code> <severity> <message>` each.
fn report(entries: &[ew_api::import::ReportEntry]) -> String {
    let lines: Vec<String> = entries
        .iter()
        .map(|e| format!("{} {:?} {}", e.code, e.severity, e.message))
        .collect();
    format!("imported; report: {}", lines.join(" | "))
}

/// A QElectroTech project through the front door into a new project (`import.apply`).
#[test]
fn qet_projects() {
    let lib = library();
    class(
        "qet",
        &[
            (
                "attributes-over-the-limit.qet",
                "input exceeds the XML attributes per element limit of 8192",
            ),
            (
                "billion-laughs.qet",
                "malformed input at line 1: a DOCTYPE is not accepted",
            ),
            (
                "depth-over-the-limit.qet",
                "input exceeds the XML depth limit of 256",
            ),
            (
                "external-entity.qet",
                "malformed input at line 1: a DOCTYPE is not accepted",
            ),
            (
                "infinite-coordinate.qet",
                "imported; report: EW-IMP-002 Error element skipped: bad x/y | EW-IMP-004 Info border ?×? / ?×? \
                 has no shipped frame; the default frame applies",
            ),
            (
                "nan-coordinate.qet",
                "imported; report: EW-IMP-002 Error element skipped: bad x/y | EW-IMP-004 Info border ?×? / ?×? \
                 has no shipped frame; the default frame applies",
            ),
        ],
        |name, bytes| match import_qet(name, bytes.to_vec(), lib) {
            Ok(imported) => report(&imported.batch.report.entries),
            Err(r) => r.to_string(),
        },
    );
}

/// A QElectroTech element definition as the window's Import and `ew import qet` take one: through the front door
/// (`import_qet`). The standalone element importer has no caller outside the project importer at 0.1.0; it is the fuzz
/// target `qet_element`'s, and `ew-import-qet` is confined to `ew-api` (ADR-0024 decision 4).
#[test]
fn qet_elements() {
    let lib = library();
    class(
        "elmt",
        &[
            (
                "billion-laughs.elmt",
                "malformed input at line 1: a DOCTYPE is not accepted",
            ),
            (
                "external-entity.elmt",
                "malformed input at line 1: a DOCTYPE is not accepted",
            ),
        ],
        |name, bytes| match import_qet(name, bytes.to_vec(), lib) {
            Ok(imported) => report(&imported.batch.report.entries),
            Err(r) => r.to_string(),
        },
    );
}

/// A supplier's parts workbook through the front door (`parse_parts`, as `ew import parts` and the window read one).
#[test]
fn xlsx_parts_tables() {
    use ew_api::catalog::{CatalogTarget, ColumnMap, ImportInput, parse_parts};
    let map = ColumnMap::new(
        None,
        [("manufacturer", "Manufacturer"), ("part_number", "Part number")],
    )
    .expect("a valid map");
    class(
        "xlsx",
        &[
            (
                "billion-laughs.xlsx",
                "malformed input: not a readable XLSX workbook: a part declares a DOCTYPE, and DTDs are refused",
            ),
            (
                "entry-absolute.xlsx",
                "malformed input: not a readable XLSX workbook: the archive entry name is an absolute path: `/x`",
            ),
            (
                "entry-drive.xlsx",
                "malformed input: not a readable XLSX workbook: the archive entry name contains ':' (a drive \
                 path): `C:x`",
            ),
            (
                "entry-parent.xlsx",
                "malformed input: not a readable XLSX workbook: the archive entry name leads out of the archive \
                 (`..`): `../../x`",
            ),
            (
                "external-entity.xlsx",
                "malformed input: not a readable XLSX workbook: a part declares a DOCTYPE, and DTDs are refused",
            ),
            (
                "ratio-over-200.xlsx",
                "input exceeds the archive entry expansion ratio limit of 200",
            ),
            (
                "zip-bomb.xlsx",
                "input exceeds the unpacked size of _rels/.rels limit of 4096",
            ),
        ],
        |name, bytes| {
            let input = ImportInput::new(name, bytes.to_vec()).expect("a file name");
            let target =
                CatalogTarget::new(ew_ids::Uuid::from_u128(0x0190_d8a4_6d3e_7c41_8f00_0000_0000_0001));
            let sizes = [bytes.len() as u64];
            match front_door(name, &sizes, &target.limits, None, || {
                parse_parts(&input, &map, &[], &target)
            }) {
                Ok(p) => format!("imported {} parts", p.parts.len()),
                Err(r) => r.to_string(),
            }
        },
    );
}

/// An `.ewz` opened as every reader opens one (`Session::open_ewz`).
#[test]
fn ewz_archives() {
    let dir = ew_testdir::fresh_dir("ew-hostile-ewz");
    class(
        "ewz",
        &[
            (
                "65536-entries.ewz",
                "the .ewz is refused: not an archive of the expected form: bytes between the central directory \
                 and the end record",
            ),
            (
                "parent-entry.ewz",
                "the .ewz is refused: the entry name `../x` is not allowed: an empty, `.` or `..` segment",
            ),
        ],
        |name, bytes| {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            match Session::open_ewz(&path, APP) {
                Ok(_) => "opened".to_owned(),
                Err(e) => e.to_string(),
            }
        },
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Golden A's files, read once.
fn golden_files() -> &'static BTreeMap<String, Vec<u8>> {
    static FILES: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    FILES.get_or_init(|| {
        fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, base, out);
                } else {
                    let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                    out.insert(rel, std::fs::read(&p).unwrap());
                }
            }
        }
        let dir = repo().join("golden/J-E-2416.ew");
        let mut out = BTreeMap::new();
        walk(&dir, &dir, &mut out);
        out
    })
}

/// Golden A's first page file.
fn first_page() -> String {
    golden_files()
        .keys()
        .find(|p| p.starts_with("pages/"))
        .expect("Golden A has page files")
        .clone()
}

/// What an open reports about `page`: the access, then each finding on the file.
fn open_verdict(report: &ew_api::OpenReport, page: &str) -> String {
    let findings: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.file.as_ref().is_some_and(|p| p.as_str() == page))
        .map(ToString::to_string)
        .collect();
    format!("{:?}; {}", report.access, findings.join(" | "))
}

/// A JSONL line in place of Golden A's first page file, opened from memory as `ew diff`'s revision side opens one.
#[test]
fn jsonl_lines() {
    let page = first_page();
    golden_files();
    class(
        "jsonl",
        &[
            (
                "line-over-16-mib.jsonl",
                "ReadOnly(Requested); pages/1d6aab0f-e47b-5da2-9d75-d3eb428e8c48.jsonl: changed outside \
                 ElektraWatt (the hash differs from the manifest) | EW-CORE-019 \
                 pages/1d6aab0f-e47b-5da2-9d75-d3eb428e8c48.jsonl:1: line of 16777217 bytes, more than the \
                 limit of 16777216",
            ),
            (
                "nested-past-depth-64.jsonl",
                "ReadOnly(Requested); pages/1d6aab0f-e47b-5da2-9d75-d3eb428e8c48.jsonl: changed outside \
                 ElektraWatt (the hash differs from the manifest) | EW-CORE-019 \
                 pages/1d6aab0f-e47b-5da2-9d75-d3eb428e8c48.jsonl:1: nesting deeper than 64",
            ),
        ],
        |_, bytes| {
            let mut files = golden_files().clone();
            files.insert(page.clone(), bytes.to_vec());
            match Session::open_in_memory(&files, StoreOptions::new(APP), "open", MemoMode::default()) {
                Ok((_, report)) => open_verdict(&report, &page),
                Err(e) => e.to_string(),
            }
        },
    );
}

/// SVG assets as the window and `item.place` take a picked or dropped file (`asset_ref_of`, `sniff`).
#[test]
fn svg_assets() {
    class(
        "svg",
        &[
            (
                "external-href.svg",
                "the SVG file is refused: it references something outside the document; remove linked or \
                 embedded images and fonts (convert text to outlines) and export it again as a plain SVG; add a \
                 raster picture as its own PNG or JPEG file",
            ),
            (
                "internal-subset.svg",
                "the SVG file is refused: it declares an entity; this is the editing data Illustrator writes \
                 with \"Preserve Illustrator Editing Capabilities\": save the SVG again with that option off \
                 (File > Save As, SVG Options), or save it from Inkscape as \"Plain SVG\"",
            ),
            (
                "script.svg",
                "the SVG file is refused: it has a script; a drawing's image never runs code: remove the \
                 scripts and interactivity, then export it again as a plain SVG",
            ),
        ],
        |_, bytes| match ew_api::asset_ref_of(bytes) {
            Ok(r) => format!("accepted as {:?}", r.format),
            Err(e) => e.to_string(),
        },
    );
}

/// Raster assets: taken as an asset (`asset_ref_of`), then decoded as every renderer and writer decodes one.
#[test]
fn raster_assets() {
    use ew_render_cpu::ew_dl::ImageFormat;
    class(
        "raster",
        &[
            (
                "jpeg-without-end-marker.jpg",
                "taken as Jpeg of Some((16, 8)); not decoded",
            ),
            (
                "png-2-31-pixels.png",
                "taken as Png of Some((65536, 32768)); not decoded",
            ),
        ],
        |_, bytes| match ew_api::asset_ref_of(bytes) {
            Ok(r) => {
                let format = match r.format {
                    ew_schema::AssetFormat::Png => ImageFormat::Png,
                    ew_schema::AssetFormat::Jpeg => ImageFormat::Jpeg,
                    ew_schema::AssetFormat::Svg => ImageFormat::Svg,
                };
                let decoded = ew_render_cpu::decode_image(format, bytes);
                format!(
                    "taken as {:?} of {:?}; {}",
                    r.format,
                    r.size,
                    if decoded.is_some() {
                        "decoded"
                    } else {
                        "not decoded"
                    }
                )
            }
            Err(e) => e.to_string(),
        },
    );
}

/// NFR-SEC-16: a project copy whose page file is a symbolic link opens read-only with the page missing; the link is
/// never followed (Unix only: making a link needs no privilege there).
#[cfg(unix)]
#[test]
fn a_page_file_that_is_a_symbolic_link_is_not_followed() {
    let root = ew_testdir::fresh_dir("ew-hostile-link");
    let dir = root.join("J-E-2416.ew");
    for (rel, bytes) in golden_files() {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let page = first_page();
    let target = root.join("elsewhere.jsonl");
    std::fs::write(&target, &golden_files()[&page]).unwrap();
    std::fs::remove_file(dir.join(&page)).unwrap();
    std::os::unix::fs::symlink(&target, dir.join(&page)).unwrap();
    let mut store = StoreOptions::new(APP);
    store.state_dir = Some(root.join("state"));
    let (verdict, peak) = alloc::extra_peak(|| {
        match Session::open(
            &dir,
            store,
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        ) {
            Ok((_, report)) => open_verdict(&report, &page),
            Err(e) => e.to_string(),
        }
    });
    assert!(
        peak <= BOUND,
        "{peak} bytes of extra peak allocation, over {BOUND}"
    );
    assert_eq!(
        verdict,
        format!(
            "ReadOnly(Integrity); EW-CORE-003 {page}: listed in the manifest but is not a regular file; links are not \
             followed (NFR-SEC-16)"
        )
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The class folders a test of this binary runs (`sql/` is `ew-sqlview`'s).
const CLASSES: &[&str] = &["elmt", "ewz", "jsonl", "qet", "raster", "sql", "svg", "xlsx"];

/// Every committed file of the corpus stands in the index, and every class folder is one a test runs.
#[test]
fn the_index_names_every_file() {
    let index = std::fs::read_to_string(hostile().join("README.md")).unwrap();
    for e in std::fs::read_dir(hostile()).unwrap() {
        let e = e.unwrap();
        if !e.path().is_dir() {
            continue;
        }
        let class = e.file_name().to_string_lossy().into_owned();
        assert!(
            index.contains(&format!("## `{class}/`")),
            "the index has no section for {class}/"
        );
        for f in std::fs::read_dir(e.path()).unwrap() {
            let name = f.unwrap().file_name().to_string_lossy().into_owned();
            assert!(
                index.contains(&format!("| `{name}` |")),
                "the index does not name {class}/{name}"
            );
        }
        assert!(
            CLASSES.contains(&class.as_str()),
            "no test runs the class {class}/"
        );
    }
    for (path, _) in generate::large() {
        let name = path.rsplit('/').next().unwrap();
        assert!(
            index.contains(&format!("| `{name}` |")),
            "the index does not name {path}"
        );
    }
}
