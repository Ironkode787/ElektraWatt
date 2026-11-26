// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Unplaced devices navigator (doc 16 §5.2): each layout space with the devices it holds that the layout does
//! not place yet.
//!
//! Not here: the Devices tree (`devices.rs`); the panel's own unplaced list (`output/panel.rs`).

use ew_derive::Q;

use super::{Builder, NavRole};
use super::{plain, plains};
use crate::output::layout_edit::mm_text;

/// The Unplaced devices tree (doc 16 §5.2's row): one location node per layout space (its
/// full designation, `+J-E-2416`), in space order, and under it the space's unplaced devices in `unplaced` order
/// (natural full tag), each labelled by its tag as a sentence names it, with the badges `strip`, `assembly`
/// and `unknown size` and the cells Device (the tag again, the key column of table mode), Part (the primary part's
/// number), W × H (mm) (`18 × 90`, empty when unknown; its decimals as the project prints them, with a point in the
/// command line's table, [`Builder::point`]) and Mount (the mount kind in words, `mount_label`). A space whose devices
/// are all placed keeps its location node, so that the empty state can name it.
pub(super) fn unplaced_table(q: &Q, b: &mut Builder) {
    let point = b.point;
    let mm = |nm: i128| {
        if point {
            ew_derive::mm_exact(nm)
        } else {
            mm_text(q, nm)
        }
    };
    use ew_derive::layout::{Unplaced, primary_part, rail_extent};
    let devices = q.devices();
    let parts = q.table::<ew_schema::LibCopy<ew_schema::Part>>();
    for (sid, space) in q.table::<ew_schema::LayoutSpace>().iter() {
        let path = q.get::<ew_derive::StructurePath>(space.location);
        let location = if path.codes.is_empty() {
            String::new()
        } else {
            format!("+{}", path.codes.join("+"))
        };
        let lnode = b.push(
            None,
            NavRole::Location,
            &format!("space:{}", sid.as_uuid()),
            plain(location),
            Vec::new(),
            vec![space.location.as_uuid()],
            Vec::new(),
            None,
            vec![("kind".to_owned(), "location".to_owned())],
        );
        for (d, use_part) in q.get::<Unplaced>(sid).iter() {
            let d = *d;
            let Some(dev) = devices.get(d) else {
                continue;
            };
            let primary = primary_part(q, dev);
            let part = use_part.clone().or_else(|| primary.clone());
            // A part use beside the primary one (a door coupling handle) is a row of its own, keyed by its part.
            let second = use_part
                .as_ref()
                .filter(|u| primary.as_ref().is_none_or(|p| p.copy != u.copy));
            let copy = part.as_ref().and_then(|p| parts.get(p.copy));
            let mech = copy.and_then(|c| c.item.mech.clone());
            let size = mech
                .as_ref()
                .and_then(|m| Some((i128::from(m.width?.0), i128::from(m.height?.0))));
            let strip = rail_extent(q, dev, part.as_ref(), false).strip;
            let assembly = q.idx().device_children(d).any(|c| c != d) || dev.parent.is_some();
            let mut badges = Vec::new();
            if strip {
                badges.push("strip".to_owned());
            }
            if assembly {
                badges.push("assembly".to_owned());
            }
            if size.is_none() && !strip {
                badges.push("unknown size".to_owned());
            }
            let label = q
                .try_get::<ew_derive::DeviceTag>(d)
                .ok()
                .and_then(|t| t.as_ref().as_ref().map(ew_derive::Tag::short))
                .unwrap_or_default();
            let mut cells = plains(vec![
                label.clone(),
                part.as_ref()
                    .map(|p| ew_derive::printed_number(q, p))
                    .unwrap_or_default(),
                size.map(|(w, h)| format!("{} × {}", mm(w), mm(h)))
                    .unwrap_or_default(),
            ]);
            cells.push(
                mech.as_ref()
                    .and_then(|m| m.mount_kind.as_ref())
                    .map_or_else(|| plain(String::new()), ew_derive::text::mount_kind),
            );
            let (key, words, records) = match second {
                Some(u) => (
                    format!("device:{}:{}", d.as_uuid(), u.copy.as_uuid()),
                    format!("{label} \u{b7} {}", ew_derive::printed_number(q, u)),
                    vec![d.as_uuid(), u.copy.as_uuid()],
                ),
                None => (format!("device:{}", d.as_uuid()), label, vec![d.as_uuid()]),
            };
            b.push(
                Some(lnode),
                NavRole::Device,
                &key,
                plain(words),
                badges,
                records,
                Vec::new(),
                Some(cells),
                vec![("kind".to_owned(), "device".to_owned())],
            );
        }
    }
}
