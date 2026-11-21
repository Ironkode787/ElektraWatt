// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests of the scoring: the protocol of a run without a brief. A brief run's score is tested with `stop`
//! (`stop.rs`), where its run folder is made.

use std::path::PathBuf;

use super::*;
use crate::blind::run::Kind;
use crate::blind::testkit::new_run_json;

fn turn(kind: &str, n: u32, folio: Option<&str>) -> Turn {
    Turn {
        n,
        kind: kind.to_owned(),
        turn: Some(n),
        folio: folio.map(str::to_owned),
        ..Turn::default()
    }
}

/// A free run holds only its protocol: its `do` turns counted per folio (a look is no turn), its end and the
/// observations; the JSON reads back the same, and the Markdown says that there is no oracle.
#[test]
fn a_free_run_holds_only_its_protocol() {
    let mut rj = new_run_json("free-r1", Kind::Free);
    rj.end = Some("handback".to_owned());
    rj.child_exit = Some(0);
    let ev = Evidence {
        rj,
        turns: vec![
            turn("do", 1, Some("101")),
            turn("look", 2, Some("101")),
            turn("do", 3, Some("101")),
            turn("do", 4, Some("102")),
        ],
        work: PathBuf::new(),
        pdfs_identical: Some(true),
        ew_check_exit: Some(1),
    };
    let s = score(&ev);
    assert_eq!(s.protocol.end.as_deref(), Some("handback"));
    let turns: Vec<(&str, u32)> = s
        .protocol
        .turns_by_folio
        .iter()
        .map(|(f, u)| (f.as_str(), u.turns))
        .collect();
    assert_eq!(turns, [("101", 2), ("102", 1)]);
    let o = &s.protocol.observations;
    assert_eq!(
        (o.product_exit, o.pdfs_identical, o.ew_check_exit),
        (Some(0), Some(true), Some(1))
    );
    let md = score_markdown(&s);
    assert!(md.contains("holds no oracle"), "{md}");
    let json = score_json(&s).unwrap();
    assert!(json.contains("\"$schema\": \"ew.blindscore/1\""), "{json}");
    // The round trip.
    let back: Score = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}
