// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Property tests: the value encodings of doc 05 §3.2 round-trip, and their text is stable.

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{ColourCode, ColourSpec, Csa, Date, Endpoint, IecColour, Quantity, Sha256, Unit};
use ew_units::{Dec3, MAX_SAFE_INTEGER};
use proptest::prelude::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// value → text → value is the identity, and text → value → text is stable.
fn round_trips<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
    let text = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    assert_eq!(&back, value);
    assert_eq!(serde_json::to_string(&back).unwrap(), text);
}

fn dec3() -> impl Strategy<Value = Dec3> {
    (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).prop_map(Dec3)
}

fn unit() -> impl Strategy<Value = Unit> {
    prop_oneof![
        Just(Unit::Mm2),
        Just(Unit::A),
        Just(Unit::V),
        Just(Unit::Hz),
        Just(Unit::Bar),
        Just(Unit::DegC),
        Just(Unit::Kg),
        Just(Unit::W),
        Just(Unit::M),
        Just(Unit::Piece),
        Just(Unit::Set),
    ]
}

fn ident() -> impl Strategy<Value = Ident> {
    "[A-Z][A-Z0-9]{0,4}".prop_map(|s| s.parse().unwrap())
}

fn colour_code() -> impl Strategy<Value = ColourCode> {
    prop_oneof![
        proptest::sample::select(IecColour::ALL.to_vec()).prop_map(ColourCode::Iec),
        // A company code is any identifier that is not an IEC code.
        ident()
            .prop_filter("not an IEC code", |i| i.as_str().parse::<IecColour>().is_err())
            .prop_map(ColourCode::Company),
        ident().prop_map(ColourCode::Foreign),
    ]
}

proptest! {
    #[test]
    fn quantity(value in dec3(), unit in unit()) {
        round_trips(&Quantity::new(value, unit));
    }

    #[test]
    fn csa(mm2 in dec3(), awg in any::<i8>(), kcmil in dec3(), pick in 0..3u8) {
        let csa = match pick {
            0 => Csa::Mm2(mm2),
            1 => Csa::Awg(awg),
            _ => Csa::Kcmil(kcmil),
        };
        round_trips(&csa);
    }

    #[test]
    fn colour_spec(codes in proptest::collection::vec(colour_code(), 1..=2)) {
        round_trips(&ColourSpec::new(codes).unwrap());
    }

    #[test]
    fn endpoint(bits in any::<u128>(), idx in any::<u16>(), junction in any::<bool>()) {
        let uuid = ew_ids::Uuid::from_u128(bits);
        let e = if junction {
            Endpoint::Junction(Id::from_uuid(uuid))
        } else {
            Endpoint::Cp(Id::from_uuid(uuid), idx)
        };
        round_trips(&e);
    }

    #[test]
    fn date(year in 1u16..=9999, month in 1u8..=12, day in 1u8..=31) {
        match Date::new(year, month, day) {
            Ok(d) => {
                round_trips(&d);
                prop_assert_eq!(d.to_string().parse::<Date>().unwrap(), d);
            }
            Err(_) => prop_assert!(day > 28),
        }
    }

    #[test]
    fn sha256(bytes in any::<[u8; 32]>()) {
        let h = Sha256(bytes);
        round_trips(&h);
        prop_assert_eq!(h.to_string().len(), 7 + 64);
    }
}
