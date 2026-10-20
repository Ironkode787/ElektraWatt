// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Smoke run of the QET project importer over a private corpus (Golden C spike, doc 20 §2.3): every `.qet`
//! of the directory named by `EW_PRIVATE_CORPUS` is imported in lenient mode, opened as a session, derived
//! (page order, device tags, potential partition) and exported to PDF. The test prints counts only, never a
//! name or a text of the corpus, and asserts that nothing panics, the model accepts every batch and the PDF
//! has one page per entry of the page order.
//!
//! It is `#[ignore]`d and skips when the variable is unset: the corpus is private and never part of the
//! repository. Run it with
//! `EW_PRIVATE_CORPUS=<dir> cargo nextest run -p ew-api --test qet_private_corpus --run-ignored all --no-capture`.

#[path = "page_output/view.rs"]
#[allow(dead_code)]
mod view;

use std::collections::BTreeMap;
use std::path::PathBuf;

use ew_api::import::{StarterLibrary, import_qet};
use ew_derive::{DeviceTag, PageOrder, PotentialPartition};
use ew_export::{PdfDate, PdfOptions, PdfPage, pdf};
use ew_scene::page::page_scene;
use ew_schema::Record;

#[test]
#[ignore = "needs the private corpus (EW_PRIVATE_CORPUS)"]
fn the_private_corpus_imports_derives_and_exports() {
    let Some(dir) = std::env::var_os("EW_PRIVATE_CORPUS").map(PathBuf::from) else {
        eprintln!("EW_PRIVATE_CORPUS is not set; skipped");
        return;
    };
    let library = StarterLibrary::from_dir(&view::repo().join("content")).expect("the starter content loads");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the corpus directory is readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("qet")))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .qet file in EW_PRIVATE_CORPUS");
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(&path).unwrap();
        let imported =
            import_qet(&name, bytes, &library).unwrap_or_else(|e| panic!("{name}: import refused: {e}"));
        let c = imported.counts();
        let mut by_code: BTreeMap<String, u32> = BTreeMap::new();
        for g in &imported.migration_report().groups {
            by_code.insert(g.code.to_string(), g.count);
        }
        let ips = imported
            .batch
            .records
            .iter()
            .filter(|r| matches!(r, Record::Function(f) if f.device.is_none()))
            .count();
        println!(
            "{name}: pages {} devices {} (strips {}, terminals {}) functions {} (interruption points {ips}) representations {} connections {} junctions {} library {} lost {}",
            c.pages,
            c.devices,
            c.strips,
            c.terminals,
            c.functions,
            c.representations,
            c.connections,
            c.junctions,
            c.library,
            imported.batch.report.stats.lost_connections
        );
        println!("{name}: findings by code {by_code:?}");
        let session = match &imported.session {
            Ok(s) => s,
            Err(e) => panic!("{name}: the model refused the batch: {e}"),
        };
        let q = session.query();
        let order = q.get::<PageOrder>(());
        let partition = q.get::<PotentialPartition>(());
        let devices: Vec<_> = q.devices().iter().map(|(_, d)| d.id).collect();
        let tagged = devices
            .iter()
            .filter(|d| q.get::<DeviceTag>(**d).is_some())
            .count();
        println!(
            "{name}: derived page order {} entries, potentials {}, devices with a derived tag {tagged} of {}",
            order.entries.len(),
            partition.potentials().count(),
            devices.len()
        );
        let pages: Vec<PdfPage> = view::golden_pages(&q)
            .into_iter()
            .map(|g| {
                let scene = page_scene(&g.view, ew_text::bundled());
                PdfPage {
                    destination: view::destination(g.entry.page),
                    label: g.entry.folio.clone(),
                    title: g.outline_title.clone(),
                    section: g.section_title.clone(),
                    width: scene.sheet_width,
                    height: scene.sheet_height,
                    content: scene.sheet_list(),
                }
            })
            .collect();
        let out = pdf(
            &pages,
            &PdfOptions::new(PdfDate::ymd(2026, 1, 1)),
            &view::fonts(),
            None,
        )
        .unwrap_or_else(|e| panic!("{name}: PDF export failed: {e:?}"));
        let pdf_pages = out.bytes.windows(b"/Type /Page\n".len()).count();
        let _ = pdf_pages;
        println!("{name}: PDF {} pages, {} bytes", pages.len(), out.bytes.len());
        assert_eq!(
            pages.len(),
            order.entries.len(),
            "{name}: one PDF page per page-order entry"
        );
    }
}
