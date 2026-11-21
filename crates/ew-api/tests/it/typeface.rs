// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project's typeface on the sample (doc 09 §4.8): without the setting the sample prints with the bundled faces
//! only; with a font file the PDF embeds it in place of Noto Sans, the DXF names its file and stands its texts as high
//! as its capitals, a save writes the file and a reopened project prints with it again; a setting whose file the
//! project does not hold prints with the bundled faces and says so. The user's file here is a bundled font file of
//! the repository (Routed Gothic, OFL-1.1), told from Noto Sans by its hash.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ew_api::output::{ExportOptions, PageSelection, TypefaceState};
use ew_api::{Recovery, Session, SessionOptions, StoreOptions};
use ew_dl::FontKey;
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{ManualClock, Vfs};
use serde_json::json;
use sha2::Digest;

use crate::common;

const FONT: &[u8] = include_bytes!("../../../../assets/fonts/RoutedGothic-Regular.ttf");
const NAME: &str = "RoutedGothic-Regular.ttf";

/// The key of the test's typeface face.
fn typeface_key() -> FontKey {
    FontKey {
        sha256: sha2::Sha256::digest(FONT).into(),
        face_index: 0,
    }
}

/// A writable session over the sample's records.
fn sample() -> Session {
    let (project, records, lib) = common::sample_records();
    common::session_of(project, records, lib)
}

fn set_typeface(s: &mut Session) {
    common::run_cmd(
        s,
        "project.typeface.set",
        json!({"file": [ew_api::base64_encode(FONT)], "name": NAME}),
    );
}

/// The sample's first drawn pages.
fn options() -> ExportOptions {
    ExportOptions {
        pages: PageSelection::parse("101-102").unwrap(),
        ..Default::default()
    }
}

/// The faces of the bundled set.
fn bundled() -> BTreeSet<FontKey> {
    ew_text::bundled().faces().map(|f| f.key).collect()
}

#[test]
fn without_a_typeface_the_sample_prints_with_the_bundled_faces_only() {
    let s = sample();
    assert_eq!(s.typeface(), TypefaceState::Bundled);
    let pdf = s.export_pdf(&options()).unwrap();
    assert!(!pdf.report.fonts.is_empty());
    assert!(
        pdf.report.fonts.iter().all(|k| bundled().contains(k)),
        "{:?}",
        pdf.report.fonts
    );
}

#[test]
fn the_typeface_prints_in_the_pdf_and_is_named_by_the_dxf() {
    let mut s = sample();
    set_typeface(&mut s);
    assert_eq!(
        s.typeface(),
        TypefaceState::Loaded {
            name: NAME.to_owned(),
            family: "Routed Gothic".to_owned(),
            face: typeface_key(),
        }
    );
    let pdf = s.export_pdf(&options()).unwrap();
    assert!(pdf.report.missing_fonts.is_empty());
    assert!(
        pdf.report.fonts.contains(&typeface_key()),
        "{:?}",
        pdf.report.fonts
    );
    let noto = ew_text::bundled()
        .face_for(&ew_text::TextStyle::regular(ew_units::Nm(1)))
        .key;
    assert!(
        !pdf.report.fonts.contains(&noto),
        "the Standard texts left Noto Sans"
    );
    // The DXF's style names the file, and the page's texts are written in it.
    let dxf = s
        .export_dxf(&options(), &ew_export::DxfLayers::default())
        .unwrap();
    let text = &dxf.files[0].text;
    assert!(
        text.contains("  2\nEW-ROUTED-GOTHIC\n") && text.contains(&format!("  3\n{NAME}\n")),
        "the style that names the file"
    );
    assert!(
        text.contains("  7\nEW-ROUTED-GOTHIC\n"),
        "texts in the typeface's style"
    );
}

/// A font of the size of the vendors' text faces (Arial weighs 1.0 MiB as Windows ships it) is taken from a command log
/// as a script or `ew apply` gives it: the file in parts of one parameter string each, one line, one undo step; and the
/// pages print in it.
#[test]
fn a_font_of_the_size_of_arial_is_taken_from_a_command_log_and_prints() {
    let font = ew_testdir::font_of_size(1_040_000);
    assert!(font.len() > 1_000_000);
    let text = ew_api::base64_encode(&font);
    let parts: Vec<&str> = text
        .as_bytes()
        .chunks(ew_api::MAX_PARAM_STRING)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect();
    assert!(parts.len() > 1, "a file over 768 KiB comes in parts");
    let line = json!({"cmd": "project.typeface.set", "params": {"file": parts, "name": "Large.ttf"}});
    let log =
        ew_api::CmdLog::parse(&format!("{{\"$schema\":\"ew.cmdlog/1\"}}\n{line}\n")).expect("a command log");
    let mut s = sample();
    s.run_script(&log, "typeface.ewcmd").expect("the line commits");
    let face = FontKey {
        sha256: sha2::Sha256::digest(&font).into(),
        face_index: 0,
    };
    assert!(
        matches!(s.typeface(), TypefaceState::Loaded { face: f, .. } if f == face),
        "{:?}",
        s.typeface()
    );
    let pdf = s.export_pdf(&options()).unwrap();
    assert!(pdf.report.fonts.contains(&face), "{:?}", pdf.report.fonts);
    s.undo(&ew_api::Origin::Gui).expect("one step");
    assert_eq!(s.typeface(), TypefaceState::Bundled);
}

#[test]
fn a_typeface_whose_file_is_gone_prints_with_the_bundled_faces_and_says_so() {
    let (mut project, records, lib) = common::sample_records();
    project.settings.typeface = Some(ew_schema::TypefaceFile {
        asset: ew_api::Sha256([7; 32]),
        name: "Gone.ttf".to_owned(),
        extra: Default::default(),
    });
    let s = common::session_of(project, records, lib);
    assert_eq!(
        s.typeface(),
        TypefaceState::Missing {
            name: "Gone.ttf".to_owned()
        }
    );
    let pdf = s.export_pdf(&options()).unwrap();
    assert!(pdf.report.fonts.iter().all(|k| bundled().contains(k)));
}

/// A save writes the font file as an asset the manifest lists, and the reopened project prints with it.
#[test]
fn a_saved_typeface_travels_with_the_project() {
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    let store = || {
        let mut o = StoreOptions::new("ElektraWatt test");
        o.vfs = Arc::clone(&vfs) as Arc<dyn Vfs>;
        o.clock = Arc::new(ManualClock::fixed());
        o.state_dir = Some(PathBuf::from("/state"));
        o
    };
    let dir = PathBuf::from("/p");
    let mut s = sample();
    s.create(&dir, store()).expect("the copy is written");
    set_typeface(&mut s);
    s.save().expect("the save writes the font file");
    s.close();
    let (r, report) =
        Session::open(&dir, store(), SessionOptions::default(), Recovery::Keep).expect("it reopens");
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    assert!(
        matches!(r.typeface(), TypefaceState::Loaded { .. }),
        "{:?}",
        r.typeface()
    );
    let pdf = r.export_pdf(&options()).unwrap();
    assert!(pdf.report.fonts.contains(&typeface_key()));
}
