// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden CSV bootstrap as a session, shared by the report tests (the same loading as
//! `tests/golden_csv.rs`), the sample project opened in place, and a new project in memory.

use std::path::{Path, PathBuf};

use ew_api::library::LibraryTiers;
use ew_api::{Session, SessionOptions};
use ew_import_tables::ew_interop_api::ImportBatch;
use ew_schema::{LibDocument, Record};

/// The repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The imported golden batch (doc 20 §3.2 path (a)): [`crate::golden_common::batch`], the one recipe.
pub fn golden_batch() -> ImportBatch {
    crate::golden_common::batch()
}

/// The project and the other records of a batch.
pub fn split(batch: &ImportBatch) -> (ew_schema::Project, Vec<Record>) {
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    (project.expect("the batch holds the project"), records)
}

/// A session over records (in the given order).
pub fn session_of(project: ew_schema::Project, records: Vec<Record>, lib: Vec<LibDocument>) -> Session {
    Session::from_records(project, records, lib, SessionOptions::default())
        .expect("the golden batch passes the model's integrity check")
}

/// A session over the golden batch.
pub fn golden_session() -> Session {
    let batch = golden_batch();
    let (project, records) = split(&batch);
    session_of(project, records, batch.lib.clone())
}

/// The `{table}|{key}` source key (`report_blocks|901`, `devices|-UC1`) of an imported record.
pub fn source_key(r: &Record) -> Option<&str> {
    let s = match r {
        Record::Representation(x) => x.source_ref.as_ref(),
        Record::Connection(x) => x.source_ref.as_ref(),
        Record::Device(x) => x.source_ref.as_ref(),
        Record::Function(x) => x.source_ref.as_ref(),
        Record::ReportBlock(x) => x.source_ref.as_ref(),
        Record::Page(x) => x.source_ref.as_ref(),
        _ => None,
    }?;
    Some(s.key.as_str())
}

/// A PNG of `w × h` opaque red pixels (synthetic).
pub fn red_png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    writer
        .write_image_data(&[255, 0, 0, 255].repeat((w * h) as usize))
        .unwrap();
    writer.finish().unwrap();
    out
}

/// How often `needle` occurs in `hay`.
pub fn count(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

/// A temporary directory, removed on drop.
pub struct Dir(pub PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The library tiers under `root`, with no company tier.
pub fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

pub use ew_testdir::copy_dir;

/// A new schematic project from the built-in template with the IDs of the seed `seed`.
pub fn new_project(seed: u64) -> Session {
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(seed)),
        ..SessionOptions::default()
    };
    Session::new_project(
        ew_api::ProjectTemplate::Builtin,
        ew_api::NewProjectKind::Schematic,
        options,
    )
    .unwrap()
}

/// The sample project opened read-only in place in the source tree: no copy, no lock, no journal.
// Allowed: no test reaches it yet; the api tests move to it from the golden helpers
// (docs/plan/m3.5-reference-project.md, section 5, lane 4).
#[allow(dead_code)]
pub fn sample() -> Session {
    Session::open_read_only(Path::new(ew_testdir::SAMPLE), "test")
        .expect("the sample opens")
        .0
}

/// The copy ID of the starter part `number` among the library parts.
pub fn library_part(s: &Session, number: &str) -> ew_ids::Uuid {
    s.library_parts()
        .iter()
        .find(|c| c.item.part_number.as_str() == number)
        .unwrap_or_else(|| panic!("{number} among the library parts"))
        .id
        .as_uuid()
}

/// The first page (by ID) of the project.
pub fn first_page(s: &Session) -> ew_ids::Uuid {
    let q = s.query();
    let mut pages: Vec<ew_ids::Uuid> = q.pages().iter().map(|(id, _)| id.as_uuid()).collect();
    pages.sort();
    pages.first().copied().expect("the new project has a page")
}
