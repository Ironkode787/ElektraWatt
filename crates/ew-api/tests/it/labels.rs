// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The label engine on the sample (doc 13 §5.3): the labels list holds a label of each set the sample has, in the order
//! the fitter applies them, each with the stock's marker that marks its set; a cable carries a marker at each end, and
//! a device two labels where the shop sets labels on the device and beside it.

use crate::common;

use ew_api::{AnyCommand, Session};
use ew_derive::{LabelRows, LabelSet};
use serde_json::json;

/// The sets of the sample's labels in their order, each once, and the copies of the first label of `set`.
fn sets(s: &Session) -> Vec<LabelSet> {
    let mut out: Vec<LabelSet> = s.query().get::<LabelRows>(()).iter().map(|l| l.set).collect();
    out.dedup();
    out
}

fn copies(s: &Session, set: LabelSet) -> u8 {
    let q = s.query();
    q.get::<LabelRows>(())
        .iter()
        .find(|l| l.set == set)
        .map(|l| l.copies)
        .expect("a label of the set")
}

#[test]
fn the_labels_follow_the_fitters_order_with_their_marker_parts() {
    let mut s = common::sample();
    assert_eq!(
        sets(&s),
        [
            LabelSet::WireMarkers,
            LabelSet::TerminalMarkers,
            LabelSet::StripMarkers,
            LabelSet::CableMarkers,
            LabelSet::DeviceLabels,
            LabelSet::TubeMarkers,
        ]
    );
    let q = s.query();
    let labels = q.get::<LabelRows>(());
    let first = labels
        .iter()
        .find(|l| l.set == LabelSet::WireMarkers)
        .expect("a wire marker");
    assert_eq!(first.lines, ["-101Q1:1"]);
    let part = first
        .marker_part
        .and_then(|p| q.parts().get(p))
        .expect("the stock's wire marker");
    assert_eq!(part.item.part_number.as_str(), "EW-GEN-MRK-1");
    drop(q);
    assert_eq!(copies(&s, LabelSet::CableMarkers), 2);
    assert_eq!(copies(&s, LabelSet::DeviceLabels), 1);
    let cmd = AnyCommand {
        name: "project.settings.set".into(),
        params: json!({"member": "workshop.device_labels", "value": "on_device_and_beside"}),
    };
    match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .expect("labels beside the devices");
    assert_eq!(copies(&s, LabelSet::DeviceLabels), 2);
}
