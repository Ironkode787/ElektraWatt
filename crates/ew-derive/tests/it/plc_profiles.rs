// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in address profiles as data (SC-6): `content/plc-profiles/*.json` are library documents of
//! kind `plc_profile` in the starter library, canonical (pretty JSON, the schema's member order, their copy IDs and
//! hash), compiled in and parsed once; both render and parse every address of seven 8-channel output bytes, and a
//! profile file that does not render is refused with T2's text. That the files are canonical is `ew-commands`'
//! `tests/it/plc_profile_files.rs` (the hash is the command layer's).

use std::path::{Path, PathBuf};

use ew_derive::ew_plcaddr::{Area, PlcAddress, Purpose, Size};
use ew_derive::ew_schema::LibDocument;
use ew_derive::{address_profile, builtin_profiles, default_profile, unreadable_profile};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/plc-profiles")
}

/// SC-6: the two built-ins in shipping order, their pins as before (a configuration project names
/// `siemens-tia-intl@2027.1`), and every address of seven output bytes (Q0.0 … Q6.7) round-trips in both.
#[test]
fn both_builtins_parse_and_round_trip_seven_output_bytes() {
    let refs: Vec<String> = builtin_profiles().iter().map(|p| p.reference()).collect();
    assert_eq!(
        refs,
        [
            "siemens-tia-intl@2027.1",
            "iec-61131-3@2027.1",
            "rockwell-logix@2027.1"
        ]
    );
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

/// FR-PLC-009: the tag-based built-in renders from the station and the slot, reads no address text, and names its
/// form in a refusal's example.
#[test]
fn the_rockwell_builtin_is_tag_based() {
    let rockwell = &builtin_profiles()[2];
    assert!(!rockwell.absolute);
    let at = ew_derive::ew_plcaddr::TagPlace {
        station: "Local",
        slot: 3,
        channel: 1,
    };
    assert_eq!(
        rockwell.render_tag(&at, Area::Q, Some(1), Purpose::Drawing),
        "Local:3:O.Data.1"
    );
    assert_eq!(
        rockwell.render_tag(&at, Area::I, None, Purpose::Drawing),
        "Local:3:I.Ch1Data"
    );
    assert_eq!(rockwell.parse("Local:3:O.Data.1"), None);
    assert_eq!(rockwell.example(), "Local:1:O.Data.1");
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
