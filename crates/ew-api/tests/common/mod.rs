// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sessions of the api tests: an import batch's records as a session, the sample project opened in place, and a
//! new project in memory.

use std::path::{Path, PathBuf};

use ew_api::library::LibraryTiers;
use ew_api::{Session, SessionOptions};
use ew_interop_api::ImportBatch;
use ew_schema::{LibDocument, Record};

/// A wire part copy of a library of its own, as a library document: `csa` mm² in `colour`, numbered `number`, its ID
/// from `n`. The workshop stock tests and the workshop lists' give it to the stock (`workshop.stock`'s `copies`).
pub fn wire_part(
    n: u128,
    number: &str,
    csa: &str,
    colour: &serde_json::Value,
) -> (ew_ids::Id<ew_schema::Part>, serde_json::Value) {
    use serde_json::json;
    let library = ew_ids::Uuid::from_u128(0x5e47_0000_0000_4000_8000_0000_0000_0001);
    let item = ew_ids::Uuid::from_u128(n);
    let id: ew_ids::Id<ew_schema::Part> = ew_ids::v5::lib_copy_id(library, item);
    let mut doc = json!({
        "$k": "part", "id": id.to_string(), "library_id": library.to_string(), "item_id": item.to_string(),
        "version": "1.0.0", "sha256": format!("sha256:{}", "0".repeat(64)),
        "kind": "wire", "part_number": number,
        "manufacturer": {"library": library.to_string(), "item": library.to_string(), "version_req": "*"},
        "sections": {"wire": {"csa": csa, "colour": colour}},
    });
    let parsed = LibDocument::from_value(doc.clone()).expect("a part document");
    doc["sha256"] = json!(ew_schema::Sha256(ew_format::library_copy_hash(&parsed).unwrap()).to_string());
    (id, doc)
}

/// The repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A QET project file in a folder of its own, read as its path; the folder is removed with it.
pub struct QetFolder {
    _dir: ew_testdir::Tmp,
    file: PathBuf,
}

impl std::ops::Deref for QetFolder {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.file
    }
}

/// The QET project `text` as the file `file_name` and, when `map` is not empty, the symbol map beside it, in a folder
/// of their own named for `name`.
pub fn qet_folder(name: &str, file_name: &str, text: &str, map: &str) -> QetFolder {
    let dir = ew_testdir::Tmp::new(name);
    let file = dir.join(file_name);
    std::fs::write(&file, text).expect("the QET file");
    if !map.is_empty() {
        std::fs::write(dir.join("symbol-map.csv"), map).expect("the map");
    }
    QetFolder { _dir: dir, file }
}

/// The import of the QET file `file` with the starter content and the symbol map beside it, its folder removed
/// before the result is read; the import opens.
pub fn import_qet_file(file: &Path) -> ew_api::import::Imported {
    let lib = ew_api::import::StarterLibrary::from_dir(&repo().join("content"))
        .expect("the starter content")
        .with_symbol_map_beside(file)
        .expect("the map reads");
    let name = file.file_name().expect("a file name").to_string_lossy();
    let imported =
        ew_api::import::import_qet(&name, std::fs::read(file).unwrap(), &lib).expect("an importable project");
    std::fs::remove_dir_all(file.parent().unwrap()).expect("the folder goes");
    assert!(imported.session.is_ok(), "{:?}", imported.session.as_ref().err());
    imported
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
        .expect("the records pass the model's integrity check")
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
pub fn sample() -> Session {
    Session::open_read_only(Path::new(ew_testdir::SAMPLE), "test")
        .expect("the sample opens")
        .0
}

/// The sample's project record, its other records and its library copies, as the format reads them from the
/// committed folder: for a test that edits a record before a session is built over them ([`session_of`]), such as one
/// of a kind no command makes.
pub fn sample_records() -> (ew_schema::Project, Vec<Record>, Vec<LibDocument>) {
    use ew_format::{self, Limits, MANIFEST, Manifest};
    let dir = Path::new(ew_testdir::SAMPLE);
    let bytes = std::fs::read(dir.join(MANIFEST)).expect("the sample's manifest");
    let manifest = Manifest::parse(&bytes, &Limits::default()).expect("a manifest");
    let files = manifest
        .files
        .keys()
        .map(|p| {
            let data = std::fs::read(dir.join(p.as_str())).unwrap_or_else(|e| panic!("{p}: {e}"));
            (p.clone(), data)
        })
        .collect();
    let loaded = ew_format::load(Some(&manifest), &files, &Limits::default());
    (
        loaded.project.expect("the sample's project record"),
        loaded.records,
        loaded.lib,
    )
}

/// The sample before Route laid its cabinet's wires: its records without the stored ways (doc 12-3d §8.5), for the
/// tests of what Route does the first time.
pub fn sample_without_ways() -> Session {
    let (project, mut records, lib) = sample_records();
    records.retain(|r| !matches!(r, Record::CommittedRoute(_)));
    session_of(project, records, lib)
}

/// The sample's records as a project made by 0.3.0 holds its layout (doc 12 §5.4): its legend a box beside the plate on
/// 902 (`embedded`, the box the sample's script made then, the plate where that box left it room, the devices numbered),
/// not a list page of its own; for the tests of embedded blocks and of a legend made before.
pub fn sample_records_with_legend_box() -> (ew_schema::Project, Vec<Record>, Vec<LibDocument>) {
    let (project, mut records, lib) = sample_records();
    for r in &mut records {
        match r {
            Record::ReportBlock(b) if b.layout_page.is_some() => {
                let page = b.layout_page.take().expect("the layout page");
                let rect = ew_units::Rect::from_points(
                    ew_units::Pt2::new(ew_units::Nm(203_087_834), ew_units::Nm(10_291_667)),
                    ew_units::Pt2::new(ew_units::Nm(270_166_667), ew_units::Nm(169_625_000)),
                );
                b.embedded = Some(ew_schema::EmbedSpec {
                    page,
                    rect: ew_schema::Bounds(rect),
                    extra: Default::default(),
                });
                b.first_folio = None;
                b.section = None;
                b.title = Default::default();
            }
            Record::PageItem(item) => {
                if let ew_schema::PageItemKind::SurfaceView(v) = &mut item.kind {
                    v.datum_at = ew_units::Pt2::new(ew_units::Nm(79_530_115), ew_units::Nm(154_284_722));
                    v.labels = ew_schema::LayoutLabels::ItemNumbers;
                }
            }
            _ => {}
        }
    }
    (project, records, lib)
}

/// A session over [`sample_records_with_legend_box`].
pub fn sample_with_legend_box() -> Session {
    let (project, records, lib) = sample_records_with_legend_box();
    session_of(project, records, lib)
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

/// `text` with every UUID replaced by `<n>`, numbered in order of first appearance, so that a comparison holds no
/// generated ID (a change of the sample's script above a record renumbers the IDs, never the shape).
pub fn ids_numbered(text: &str) -> String {
    let is_id = |s: &str| {
        s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
    };
    let mut seen: Vec<&str> = Vec::new();
    let mut out = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        match rest.get(..36).filter(|s| is_id(s)) {
            Some(id) => {
                let n = seen.iter().position(|s| *s == id).unwrap_or_else(|| {
                    seen.push(id);
                    seen.len() - 1
                });
                out.push_str(&format!("<{}>", n + 1));
                rest = &rest[36..];
            }
            None => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// Imports the test tables' parts (`fixtures/tables/parts.csv` and its sibling tables) into `s` through
/// `parts.import`, as project-local copies: the end brackets, the modular valve terminal's parts and the supplier parts
/// the sample's starter library does not have.
pub fn import_test_parts(s: &mut Session) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tables/parts.csv");
    let parts = ew_api::import::read_parts_tables(&path, s).expect("the test tables' parts read");
    let cmd = ew_api::AnyCommand {
        name: ew_api::import::PARTS_IMPORT.into(),
        params: parts.params(),
    };
    let preview = s
        .preview_with(&cmd, ew_api::Submit::from(ew_api::Origin::Gui))
        .expect("parts.import previews");
    s.commit(preview).expect("parts.import commits");
}

/// The ID of the part copy whose library key is `key` (the key column of `parts.csv`).
pub fn part_copy(s: &Session, key: &str) -> ew_ids::Uuid {
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key))
        .map(|(id, _)| id.as_uuid())
        .unwrap_or_else(|| panic!("no part copy {key}"))
}

/// Appends a use of the part copy `copy` (`qty` pieces) to the device tagged `tag`, with one `fields.set`.
pub fn add_part_use(s: &mut Session, tag: &str, copy: ew_ids::Uuid, qty: &str) {
    let device = s.resolve_reference(&format!("tag:{tag}")).expect("the device");
    add_part_use_to(s, device, copy, qty);
}

/// [`add_part_use`] by the device's ID.
pub fn add_part_use_to(s: &mut Session, device: ew_ids::Uuid, copy: ew_ids::Uuid, qty: &str) {
    let mut uses = serde_json::to_value(
        &s.query()
            .devices()
            .get(ew_ids::Id::from_uuid(device))
            .expect("the device")
            .parts,
    )
    .expect("the part uses");
    uses.as_array_mut()
        .expect("a list")
        .push(serde_json::json!({"part": {"copy": copy.to_string()}, "qty": qty}));
    s.execute(&ew_api::AnyCommand {
        name: "fields.set".into(),
        params: serde_json::json!({"kind": "device", "targets": [device.to_string()], "path": "parts", "value": uses}),
    })
    .unwrap_or_else(|e| panic!("{device}'s parts: {e}"));
}

/// The sample over its records edited by `edit` (so that it can be saved), with the memo mode `memo`, the test parts
/// imported and the TBD end bracket added to each of its three strips as a part use (a project made before accessories:
/// it wins over the bracket the terminals' parts declare): the brackets the layout derives at the ends of each strip's
/// run on its rail. Each strip leaves off the end plate its terminals' parts bring, so that the strips are as wide as
/// their terminals, which the positions of these set-ups are computed from.
pub fn sample_with_end_brackets_over(edit: impl FnOnce(&mut Vec<Record>), memo: ew_api::MemoMode) -> Session {
    let (project, mut records, lib) = sample_records();
    let plate = lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(c) if c.item.part_number.as_str() == "EW-GEN-EP-1" => Some(c.id),
            _ => None,
        })
        .expect("the sample's end plate");
    for r in &mut records {
        if let Record::Device(d) = r
            && d.aspects.strip.is_some()
        {
            d.accessory_choices.push(ew_schema::AccessoryChoice {
                of: plate,
                part: None,
                extra: Default::default(),
            });
        }
    }
    edit(&mut records);
    let mut s = Session::from_records(
        project,
        records,
        lib,
        SessionOptions {
            memo,
            ..SessionOptions::default()
        },
    )
    .expect("the edited sample is a valid model");
    import_test_parts(&mut s);
    let bracket = part_copy(&s, "TBD end bracket");
    for strip in ["-X1011", "-X1021", "-X1031"] {
        add_part_use(&mut s, strip, bracket, "1");
    }
    s
}

/// Runs a command through preview and commit when it is a bulk command, else directly; panics with the refusal.
pub fn run_cmd(s: &mut Session, name: &str, params: serde_json::Value) -> ew_api::Receipt {
    let cmd = ew_api::AnyCommand {
        name: name.into(),
        params: params.clone(),
    };
    match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .unwrap_or_else(|e| panic!("{name} {params}: {e}"))
}

/// The placement of the device tagged `tag`.
pub fn placement_of(s: &Session, tag: &str) -> ew_ids::Id<ew_schema::Placement> {
    let device = s.resolve_reference(&format!("tag:{tag}")).expect("the device");
    s.query()
        .table::<ew_schema::Placement>()
        .iter()
        .find(|(_, p)| p.device.as_uuid() == device)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("{tag} is not placed"))
}

/// The sample's shield wire on 103, the core SHIELD of -103W1, found by its ends: the terminal -X1031:SH and the motor
/// -103M1.
pub fn sample_shield(s: &Session) -> ew_ids::Id<ew_schema::Connection> {
    let [terminal, motor] =
        ["-X1031:SH", "-103M1"].map(|tag| s.resolve_reference(&format!("tag:{tag}")).expect("the device"));
    let q = s.query();
    let device = |end: &ew_schema::Endpoint| {
        end.function()
            .and_then(|f| q.functions().get(f))
            .and_then(|f| f.device)
            .map(|d| d.as_uuid())
    };
    q.connections()
        .iter()
        .find(|(_, c)| {
            let ends = [device(&c.a), device(&c.b)];
            ends == [Some(terminal), Some(motor)] || ends == [Some(motor), Some(terminal)]
        })
        .map(|(id, _)| id)
        .expect("the shield wire")
}

/// A rail pose whose footprint starts `s10` tenths of a millimetre along the rail (a strip's clip point is its left
/// end).
pub fn rail_at(s10: i64) -> serde_json::Value {
    serde_json::json!({"t": [s10 * 100_000, 0, 0], "q": [1_000_000_000, 0, 0, 0]})
}

/// [`sample_with_end_brackets_over`] with a third rail added to the plate above rail 2 (470 mm, from u = 25 mm at
/// v = 120 mm, where nothing is mounted), and the three strips moved onto it in one run: -X1011 at 30 mm (26 mm
/// wide), -X1021 at 66 mm (41.6 mm), -X1031 at 117.6 mm (26 mm), each right after the bracket before it, so that the
/// run holds four brackets: [20, 30] and [56, 66] of -X1011, [107.6, 117.6] of -X1021 and [143.6, 153.6] of -X1031.
/// The session and the new rail.
pub fn strips_on_a_test_rail() -> (Session, ew_ids::Id<ew_schema::Carrier>) {
    strips_on_a_test_rail_over(|_| {}, ew_api::MemoMode::On)
}

/// [`strips_on_a_test_rail`] over the sample's records edited by `edit`, with the memo mode `memo`.
pub fn strips_on_a_test_rail_over(
    edit: impl FnOnce(&mut Vec<Record>),
    memo: ew_api::MemoMode,
) -> (Session, ew_ids::Id<ew_schema::Carrier>) {
    let mut s = sample_with_end_brackets_over(edit, memo);
    let tables = s.panel_tables("902").expect("902");
    let receipt = run_cmd(
        &mut s,
        "layout.carrier.add",
        serde_json::json!({"surface": tables.surface_id, "kind": "rail",
            "path": [[25_000_000, 120_000_000], [495_000_000, 120_000_000]],
            "params": {"rail": {"profile": "ts35x7_5"}}}),
    );
    let rail: ew_ids::Id<ew_schema::Carrier> =
        ew_ids::Id::from_uuid(receipt.outcome.primary.expect("the new rail").1.as_uuid());
    for (strip, s10) in [("-X1011", 300), ("-X1021", 660), ("-X1031", 1176)] {
        let placement = placement_of(&s, strip);
        run_cmd(
            &mut s,
            "layout.move",
            serde_json::json!({"placement": placement, "carrier": {"carrier": rail}, "pose": rail_at(s10)}),
        );
    }
    (s, rail)
}

/// The ID of the fourth strip [`add_a_fourth_strip`] adds; its terminals' IDs follow it by `0x100 + seq`.
pub const FOURTH_STRIP: u128 = 0x5e44_0000_0000_4000_8000_0000_0000_0033;

/// Adds to the sample's `records` a fourth strip, unplaced: a copy of -X1031 with its five terminals (26 mm), counter
/// 33, with the ID [`FOURTH_STRIP`].
pub fn add_a_fourth_strip(records: &mut Vec<Record>) {
    let x1031 = sample().resolve_reference("tag:-X1031").expect("-X1031");
    let strip: ew_ids::Id<ew_schema::Device> = ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(FOURTH_STRIP));
    let mut extra = Vec::new();
    for r in records.iter() {
        let Record::Device(d) = r else { continue };
        if d.id.as_uuid() == x1031 {
            let mut c = d.clone();
            c.id = strip;
            c.tag.counter = Some(33);
            extra.push(Record::Device(c));
        } else if let Some(t) = d.aspects.terminal.as_ref().filter(|t| t.strip.as_uuid() == x1031) {
            let mut c = d.clone();
            c.id = ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(FOURTH_STRIP + 0x100 + u128::from(t.seq)));
            if let Some(t) = c.aspects.terminal.as_mut() {
                t.strip = strip;
            }
            extra.push(Record::Device(c));
        }
    }
    assert_eq!(extra.len(), 6, "-X1031 and its five terminals");
    records.extend(extra);
}

/// The sample's library documents as copies of a colleague's library that is not installed: each starter part's copy
/// names another library. Such a part is only in this project (doc 07 §5.8), his to change, so an edit forks it, which
/// a starter part never is (its copy ID stays: the records name it).
pub fn as_colleagues(lib: &mut [LibDocument]) {
    let colleague = ew_ids::Uuid::from_u128(0xc011_ea90_0000_4000_8000_0000_0000_0001);
    for doc in lib {
        if let LibDocument::Part(c) = doc
            && c.library_id.as_uuid() == ew_schema::starter::parts_library()
        {
            c.library_id = ew_ids::Id::from_uuid(colleague);
        }
    }
}

/// The sample as [`sample_records`] reads it, its parts copies of a colleague's library ([`as_colleagues`]).
pub fn colleague_sample() -> Session {
    let (project, records, mut lib) = sample_records();
    as_colleagues(&mut lib);
    session_of(project, records, lib)
}
