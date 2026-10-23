// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Transform and delete on a selection (WP-S2.04 §7.4): with Select active and representations selected, R commits
//! `rep.transform {reps, rotate: 1}`, Shift+R `rotate: 3`, X `rotate: 0, mirror: true`, Y `rotate: 2, mirror: true`;
//! V on exactly one representation commits the `rep.transform` whose composition gives the next variant letter.
//! Delete commits `edit.delete` with every entity of the selection the page tables resolve (§6.6).

use ew_api::AnyCommand;
use ew_api::geometry::orient_then;
use ew_api::output::ew_dl::EntityRef;
use serde_json::json;

use super::{EntityKind, Selection, ToolKey, selection_kinds};
use crate::canvas::CanvasPage;

/// The representations of the selection on `page`, in selection order.
pub fn selected_reps(selection: &Selection, page: &CanvasPage) -> Vec<EntityRef> {
    selection_kinds(selection, &page.tables)
        .into_iter()
        .filter(|(_, k)| *k == EntityKind::Rep)
        .map(|(e, _)| e)
        .collect()
}

/// The rotation and mirror of `rep.transform` that turn orientation `o` into `o + 1` (the next variant letter,
/// wrapping H to A): the first `(rotate, mirror)` in the order `(0..4, false)`, `(0..4, true)` whose composition
/// gives it.
pub fn next_variant_transform(o: u8) -> (u8, bool) {
    let target = (o + 1) % 8;
    for mirror in [false, true] {
        for rotate in 0..4 {
            if orient_then(o, rotate, mirror) == target {
                return (rotate, mirror);
            }
        }
    }
    (0, false)
}

/// The `rep.transform` command of a transform key on `reps`; `None` for a key that is no transform, or for V on
/// another number of representations than one (whose orientation is `orient`).
pub fn transform_command(key: ToolKey, reps: &[EntityRef], orient: Option<u8>) -> Option<Box<AnyCommand>> {
    if reps.is_empty() {
        return None;
    }
    let (rotate, mirror) = match key {
        ToolKey::RotateCw => (1, false),
        ToolKey::RotateCcw => (3, false),
        ToolKey::MirrorX => (0, true),
        ToolKey::MirrorY => (2, true),
        ToolKey::NextVariant if reps.len() == 1 => next_variant_transform(orient?),
        _ => return None,
    };
    let mut params = json!({
        "reps": reps.iter().map(|e| e.0.to_string()).collect::<Vec<_>>(),
        "rotate": rotate,
    });
    if mirror {
        params["mirror"] = json!(true);
    }
    Some(Box::new(AnyCommand {
        name: "rep.transform".into(),
        params,
    }))
}

/// The `edit.delete` command of every entity of the selection the page tables resolve; `None` for none.
pub fn delete_command(selection: &Selection, page: &CanvasPage) -> Option<Box<AnyCommand>> {
    let targets: Vec<_> = selection_kinds(selection, &page.tables)
        .into_iter()
        .map(|(e, k)| json!({"kind": k.kind_name(), "id": e.0.to_string()}))
        .collect();
    if targets.is_empty() {
        return None;
    }
    Some(Box::new(AnyCommand {
        name: "edit.delete".into(),
        params: json!({ "targets": targets }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_variant_is_reached_by_one_transform_from_every_letter() {
        for o in 0..8u8 {
            let (r, m) = next_variant_transform(o);
            assert_eq!(orient_then(o, r, m), (o + 1) % 8, "{o}");
        }
    }
}
