// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A connection list whose map names a level column per end lands each connection on the level it names, not every
//! one on level 1 (doc 14 §5.4 "Levels in a connection list", FT-1985).

use ew_api::catalog::ImportInput;
use ew_api::eplan;
use ew_api::{Session, SessionOptions};
use ew_derive::FunctionCps;
use ew_model::EndpointKey;
use ew_schema::{Endpoint, Record};

#[test]
fn a_connection_list_lands_each_end_on_the_level_its_column_names() {
    let mut map = eplan::report_presets()
        .remove("connections-en")
        .expect("a shipped preset");
    map.columns.insert("from_level".into(), "Source level".into());
    map.columns.insert("to_level".into(), "Target level".into());
    // -X1:5 at levels 1 and 2, each to its own terminal of -X2; the second row names no level for its target.
    let text = "Source;Source level;Target;Target level\n-X1:5;1;-X2:1;\n-X1:5;2;-X2:2;\n";
    let input = ImportInput::new("wires.csv", text.as_bytes().to_vec()).unwrap();
    let batch = eplan::report_batch(&input, &map, "connections-en").unwrap();
    let levels: Vec<Vec<u8>> = batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Device(d) => d.aspects.terminal.as_ref(),
            _ => None,
        })
        .filter(|t| t.designation.as_str() == "5")
        .map(|t| t.levels.iter().map(|l| l.level).collect())
        .collect();
    assert_eq!(levels, [vec![1, 2]], "-X1:5 is one terminal of two levels");
    let ends: Vec<Endpoint> = batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Connection(c) => Some(c.a.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(ends.len(), 2, "{ends:?}");
    let session: Session = ew_api::import::session_from_batch(&batch, SessionOptions::default()).unwrap();
    let q = session.query();
    // Each connection's end at -X1:5 is a connection point of the level its row names.
    let level_of = |e: &Endpoint| {
        let Endpoint::Cp(f, idx) = e else { panic!("a CP: {e:?}") };
        q.get::<FunctionCps>(*f)
            .iter()
            .find(|c| c.idx == *idx)
            .and_then(|c| c.level)
    };
    let mut found: Vec<Option<u8>> = ends.iter().map(level_of).collect();
    found.sort_unstable();
    assert_eq!(found, [Some(1), Some(2)]);
    // The levels stay apart: the two connections are on two potentials.
    let partition = q.get::<ew_derive::PotentialPartition>(());
    let potentials: Vec<_> = ends
        .iter()
        .map(|e| {
            let Endpoint::Cp(f, idx) = e else { unreachable!() };
            partition.potential_at(EndpointKey::Cp(*f, *idx)).map(|p| p.id)
        })
        .collect();
    assert_ne!(potentials[0], potentials[1]);
}
