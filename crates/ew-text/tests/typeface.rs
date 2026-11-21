// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The font set of a project's typeface (doc 09 §4.8): the user's file prints the Standard role in both weights, the
//! other roles keep their bundled faces. The test's "user's file" is a bundled file of another role (Routed Gothic),
//! so its face is told from Noto Sans by its hash.

use std::sync::Arc;

use ew_text::{Family, TextStyle, Weight, bundled, typeface_file, typeface_set};
use ew_units::Nm;
use sha2::{Digest, Sha256};

const USER_FILE: &[u8] = include_bytes!("../../../assets/fonts/RoutedGothic-Regular.ttf");

fn style(family: Family, weight: Weight) -> TextStyle {
    TextStyle {
        family,
        weight,
        ..TextStyle::regular(Nm(3_175_000))
    }
}

#[test]
fn the_typeface_prints_the_standard_role_and_only_it() {
    let data: Arc<[u8]> = Arc::from(USER_FILE);
    let set = typeface_set(&data).expect("a font file");
    let user: [u8; 32] = Sha256::digest(USER_FILE).into();
    for weight in [Weight::Regular, Weight::Bold] {
        let shaped = set.shape("K1 13", &style(Family::Default, weight), None);
        assert_eq!(shaped.run.font.sha256, user, "{weight:?} prints in the file");
        assert!(shaped.missing.is_empty());
    }
    // Narrow keeps Noto Sans Condensed: the bundled set's face of that role.
    let narrow = style(Family::Narrow, Weight::Regular);
    assert_eq!(set.face_for(&narrow).key, bundled().face_for(&narrow).key);
    // Noto Sans is not in the set any more; the symbol fallback is.
    assert!(set.faces().all(|f| f.family_name != "Noto Sans"));
    assert!(set.faces().any(|f| f.family == Family::Symbols));
    // The same bytes give the same set, and its face's file is found by its key for the writers.
    assert!(std::ptr::eq(set, typeface_set(&data).unwrap()));
    let key = set.face_for(&style(Family::Default, Weight::Regular)).key;
    assert_eq!(typeface_file(key).as_deref(), Some(USER_FILE));
}

#[test]
fn bytes_that_are_no_font_give_no_set() {
    let data: Arc<[u8]> = Arc::from(&b"\0\x01\0\0 not a font at all"[..]);
    assert!(typeface_set(&data).is_err());
    let sha256: [u8; 32] = Sha256::digest(&data).into();
    assert_eq!(
        typeface_file(ew_dl::FontKey {
            sha256,
            face_index: 0
        }),
        None
    );
}
