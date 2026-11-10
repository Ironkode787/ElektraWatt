// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in address profiles as data (SC-6): `content/plc-profiles/*.json` are library documents of
//! kind `plc_profile` in the starter library, canonical (pretty JSON, the schema's member order, their copy IDs and
//! hash), compiled in and parsed once; both render and parse every address Golden A's 56 channels print, and a
//! profile file that does not render is refused with T2's text.
//!
//! `EW_BLESS=1 cargo nextest run -p ew-derive --test it -E 'test(/^plc_profiles::/)'` rewrites the files' IDs and
//! hashes in canonical form.

use std::path::{Path, PathBuf};

use ew_derive::ew_ids::{Id, v5};
use ew_derive::ew_plcaddr::{Area, PlcAddress, Purpose, Size};
use ew_derive::ew_schema::{LibDocument, Sha256};
use ew_derive::{address_profile, builtin_profiles, default_profile, unreadable_profile};

/// The starter library, which the built-in profiles belong to (`ew_interop_api::SYSTEM_LIBRARY`).
const STARTER: u128 = 0x7202fd8b_1024_4bf1_9e99_670772eaa589;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/plc-profiles")
}

// The test boundary: `EW_BLESS=1` asks the test to rewrite the shipped files.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    // The bless switch of a test: it chooses whether the test rewrites the shipped files, never a result.
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// The canonical text of a profile file: the item ID `v5(starter, meta.key)`, the copy ID, the hash of the item.
fn canonical(text: &str) -> String {
    let LibDocument::PlcProfile(mut c) = LibDocument::from_json(text).expect("a library document") else {
        panic!("not a plc_profile document")
    };
    let library = ew_derive::ew_ids::Uuid::from_u128(STARTER);
    let key = c.item.meta.key.as_ref().expect("a key").as_str().to_owned();
    c.library_id = Id::from_uuid(library);
    c.item_id = v5::imported_id(library, &key);
    c.id = v5::lib_copy_id(library, c.item_id.as_uuid());
    let doc = LibDocument::PlcProfile(c.clone());
    c.sha256 = Sha256(ew_commands::commands::library_copy_hash(&doc).expect("a hash"));
    format!(
        "{}\n",
        serde_json::to_string_pretty(&LibDocument::PlcProfile(c)).expect("serialisable")
    )
}

#[test]
fn the_profile_files_are_canonical() {
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    assert!(!names.is_empty());
    for path in names {
        let mut text = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
        let want = canonical(&text);
        if bless() {
            std::fs::write(&path, &want).unwrap();
            text.clone_from(&want);
        }
        assert_eq!(
            text,
            want,
            "{} is not canonical (EW_BLESS=1 rewrites it)",
            path.display()
        );
    }
}

/// SC-6: the two built-ins in shipping order, their pins as before (Golden A's configuration project names
/// `siemens-tia-intl@2027.1`), and every address Golden A's channels print (Q0.0 … Q6.7) round-trips in both.
#[test]
fn both_builtins_parse_and_round_trip_golden_as_addresses() {
    let refs: Vec<String> = builtin_profiles().iter().map(|p| p.reference()).collect();
    assert_eq!(refs, ["siemens-tia-intl@2027.1", "iec-61131-3@2027.1"]);
    assert_eq!(default_profile(), "siemens-tia-intl@2027.1");
    let (tia, iec) = (&builtin_profiles()[0], &builtin_profiles()[1]);
    for byte in 0..7 {
        for bit in 0..8 {
            let a = PlcAddress::bit(Area::Q, byte, bit);
            for p in [tia, iec] {
                for purpose in [Purpose::Drawing, Purpose::Exchange] {
                    assert_eq!(p.parse(&p.render(&a, purpose)), Some(a), "{}", p.reference());
                }
            }
        }
    }
    assert_eq!(
        tia.render(&PlcAddress::bit(Area::Q, 6, 7), Purpose::Drawing),
        "Q6.7"
    );
    assert_eq!(
        iec.render(&PlcAddress::bit(Area::Q, 6, 7), Purpose::Drawing),
        "%QX6.7"
    );
    assert_eq!(
        tia.render(&PlcAddress::unit(Area::I, Size::Word, 64), Purpose::Exchange),
        "%IW64"
    );
}

/// T2: a profile file whose data the engine does not render is refused whole, naming its pin and the reason.
#[test]
fn a_profile_that_does_not_render_says_why() {
    let text = std::fs::read_to_string(dir().join("siemens-tia-intl.json")).unwrap();
    let LibDocument::PlcProfile(c) = LibDocument::from_json(&text).unwrap() else {
        panic!()
    };
    assert_eq!(unreadable_profile(&c.item), None);
    let mut def = c.item.clone();
    def.render.bit = "{area}{byte}".into();
    assert_eq!(
        unreadable_profile(&def).as_deref(),
        Some("the address profile siemens-tia-intl@2027.1 cannot be read: its bit template has no {bit}")
    );
    let mut def = c.item.clone();
    def.unit_bits = 16;
    assert_eq!(
        address_profile(&def).unwrap_err().to_string(),
        "only 8 bits per unit are rendered"
    );
}
