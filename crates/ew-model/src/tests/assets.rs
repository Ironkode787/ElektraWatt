// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The stamp of the asset store (cycle-29 review 4): a tracked read of whether an asset is held depends on it, so it
//! changes exactly when an insert grows the set.

use ew_schema::{AssetFormat, AssetRef, Sha256};

use super::fixture::mini_golden;
use crate::{Asset, Rev};

/// An asset whose hash is `n` repeated (the model does not hash; the caller computed the reference).
fn asset(n: u8) -> Asset {
    Asset::new(
        AssetRef {
            sha256: Sha256([n; 32]),
            format: AssetFormat::Svg,
            size: None,
        },
        vec![n; 4],
    )
}

#[test]
fn the_assets_stamp_grows_with_every_insert_and_only_then() {
    let mut m = mini_golden();
    assert_eq!(m.assets_stamp(), Rev(0));
    let snapshot = m.clone();
    m.put_asset(asset(1));
    assert_eq!(m.assets_stamp(), Rev(1), "an insert grows the set");
    m.put_asset(asset(1));
    assert_eq!(
        m.assets_stamp(),
        Rev(1),
        "an asset that is held already changes nothing"
    );
    m.put_asset(asset(2));
    assert_eq!(m.assets_stamp(), Rev(2));
    assert_eq!(m.rev(), snapshot.rev(), "putting an asset is no revision");
    assert_eq!(
        snapshot.assets_stamp(),
        Rev(0),
        "a snapshot taken before keeps its stamp"
    );
}
