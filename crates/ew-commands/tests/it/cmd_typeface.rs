// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.typeface.set` (doc 09 §4.8): a font file of the user's becomes the project's typeface, held as an asset;
//! what is no usable font, or a font whose embedding permission forbids what the PDF does, is refused in words. The
//! user's file in these tests is a bundled font file of the repository (Routed Gothic, OFL-1.1).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::{AnyCommand, CmdError, CommandRegistry, Cost, DynCommand, Engine, Exposure, Origin};
use serde_json::json;
use sha2::Digest;

const FONT: &[u8] = include_bytes!("../../../../assets/fonts/RoutedGothic-Regular.ttf");

fn set(file: &[u8], name: &str) -> AnyCommand {
    AnyCommand {
        name: "project.typeface.set".to_owned(),
        params: json!({"file": [ew_schema::base64_encode(file)], "name": name}),
    }
}

fn commit(e: &mut Engine, cmd: &dyn DynCommand) {
    let p = e.preview(cmd).unwrap_or_else(|err| panic!("{err:?}"));
    e.commit(p).unwrap();
}

/// The pointer and the words of a refusal.
fn refused(e: &mut Engine, cmd: &dyn DynCommand) -> (String, String) {
    match e.preview(cmd).unwrap_err() {
        CmdError::InvalidParams { path, message, .. } => (path, message),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

/// `FONT` with the `OS/2` table's embedding permission (`fsType`, at offset 8 of the table) set to `fs_type`.
fn with_fs_type(fs_type: u16) -> Vec<u8> {
    let mut bytes = FONT.to_vec();
    let tables = usize::from(u16::from_be_bytes([bytes[4], bytes[5]]));
    let record = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&r| &bytes[r..r + 4] == b"OS/2")
        .expect("the font has an OS/2 table");
    let offset = u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
    bytes[offset + 8..offset + 10].copy_from_slice(&fs_type.to_be_bytes());
    bytes
}

#[test]
fn the_typeface_command_is_a_bulk_edit_and_undoable() {
    let meta = CommandRegistry::builtin()
        .get("project.typeface.set")
        .expect("registered")
        .meta();
    assert_eq!((meta.cost, meta.exposure), (Cost::Bulk, Exposure::Edit));
    assert!(meta.undoable);
}

/// The file joins the asset store and the setting names it with its name; undo removes the setting and keeps the
/// bytes; an empty file removes the setting.
#[test]
fn a_font_file_becomes_the_typeface_and_undoes() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    commit(&mut e, &set(FONT, "RoutedGothic-Regular.ttf"));
    let typeface = e
        .model()
        .project()
        .settings
        .typeface
        .clone()
        .expect("the setting");
    assert_eq!(typeface.asset.0, <[u8; 32]>::from(sha2::Sha256::digest(FONT)));
    assert_eq!(typeface.name, "RoutedGothic-Regular.ttf");
    let asset = e.model().asset(&typeface.asset).expect("the file is an asset");
    assert_eq!(asset.reference().format, ew_schema::AssetFormat::Ttf);
    assert_eq!(&**asset.bytes(), FONT);
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
    assert!(e.model().asset(&typeface.asset).is_some(), "undo keeps the bytes");
    e.redo(&Origin::Gui).unwrap();
    let removal = AnyCommand {
        name: "project.typeface.set".to_owned(),
        params: json!({"file": []}),
    };
    commit(&mut e, &removal);
    assert_eq!(e.model().project().settings.typeface, None);
}

/// Every refusal names its parameter and says why in words; the project is unchanged.
#[test]
fn what_is_no_usable_typeface_is_refused_in_words() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    let cases: Vec<(AnyCommand, &str, &str)> = vec![
        (
            set(b"%PDF-1.7 not a font", "a.ttf"),
            "/file",
            "no font of one typeface",
        ),
        (
            set(&[b"ttcf", &FONT[4..]].concat(), "a.ttc"),
            "/file",
            "collection",
        ),
        (set(&FONT[..64], "a.ttf"), "/file", "cannot be read"),
        (set(&with_fs_type(0x0002), "a.ttf"), "/file", "forbids embedding"),
        (set(&with_fs_type(0x0200), "a.ttf"), "/file", "bitmaps only"),
        (set(&with_fs_type(0x0100), "a.ttf"), "/file", "only whole"),
        (set(FONT, ""), "/name", "name"),
        (set(FONT, "C:/fonts/a.ttf"), "/name", "without its folder"),
        (
            AnyCommand {
                name: "project.typeface.set".to_owned(),
                params: json!({"file": ["not base64!"], "name": "a.ttf"}),
            },
            "/file",
            "base64",
        ),
    ];
    for (cmd, path, words) in cases {
        let (at, message) = refused(&mut e, &cmd);
        assert_eq!(at, path, "{message}");
        assert!(message.contains(words), "{path}: {message}");
    }
    // Over the ceiling, in parts within the limits of command parameters: refused in words before it is decoded.
    let text = ew_schema::base64_encode(&[FONT, &vec![0; 2_600 * 1024]].concat());
    let large = AnyCommand {
        name: "project.typeface.set".to_owned(),
        params: json!({"file": [&text[..1 << 20], &text[1 << 20..2 << 20], &text[2 << 20..3 << 20], &text[3 << 20..]],
            "name": "a.ttf"}),
    };
    let (at, message) = refused(&mut e, &large);
    assert_eq!(
        (at.as_str(), message.contains("2560 KiB")),
        ("/file", true),
        "{message}"
    );
    assert_eq!(lines(e.model()), before);
}

/// An installable, editable or preview-and-print font is taken, and a permission field with more than one of those
/// bits is read as the least restrictive (the OpenType specification).
#[test]
fn a_font_that_may_be_embedded_is_taken() {
    for fs_type in [0x0000, 0x0004, 0x0008, 0x000C] {
        let mut e = engine(mini_model());
        commit(&mut e, &set(&with_fs_type(fs_type), "a.ttf"));
        assert!(e.model().project().settings.typeface.is_some(), "{fs_type:#06x}");
    }
}
