// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The junction sequence editor (WP-S2.04 §8.5, FR-CON-005): a popup of the tool host over one junction of the
//! focused page, showing its style (`dot`, or `bevel` with its orientation), its wiring (`star` or `chain`, stored or
//! the style's default), its sequence as rows named by each member's far endpoint (doc 06 §3.5) and below them the
//! physical wires through it (`Session::junction_wires`). Every change commits `junction.edit` at once (one undo
//! step each, D11): `junction.member` reorders (Alt+Up and Alt+Down on the focused row), `junction.style` sets the
//! style (R on a bevel cycles its orientation), `junction.wiring` the wiring. The popup reads itself again from the
//! next page tables and closes with `ui.dismiss`.

use ew_api::output::ew_dl::EntityRef;
use ew_api::output::{JunctionWire, PageTables, TableEnd};
use serde_json::{Value, json};

/// One member row of the editor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JunctionMember {
    /// The connection.
    pub connection: EntityRef,
    /// The label of its far endpoint (`-102F1:1`, `@C11` for another junction).
    pub label: String,
}

/// The editor's state.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JunctionEditor {
    /// The junction.
    pub junction: EntityRef,
    /// Its name (the frame cell, `#n` when the cell holds several; [`junction_name`]): the transcript's and the
    /// describe text's, no UUID reaches either.
    pub cell: String,
    /// `dot` or `bevel`.
    pub style: String,
    /// The bevel's orientation `0..=7`.
    pub bevel_orient: u8,
    /// The effective wiring, `star` or `chain`.
    pub wiring: String,
    /// Whether the wiring is stored (else the style's default).
    pub explicit_wiring: bool,
    /// The members in wiring order.
    pub members: Vec<JunctionMember>,
    /// The focused row.
    pub row: usize,
    /// The physical wires through the junction.
    pub wires: Vec<JunctionWire>,
    /// The (session, revision) the editor was read at.
    pub rev: (u64, u64),
}

/// The name of junction `j` in the transcript and the describe text: the frame cell it lies in, with `#n` when the
/// cell holds several junctions (`C11#2`), numbered by position (y, then x; ties by ID). Not by ID alone: the IDs one
/// transaction mints are not in creation order (UUIDv7's random bits within one timestamp), so two runs of one script
/// could swap the numbers (ID-474's byte equality). No UUID (`@n` comes with FT-422).
pub fn junction_name(tables: &PageTables, j: EntityRef, cell_of: &dyn Fn((i64, i64)) -> String) -> String {
    let Some(row) = tables.junctions.iter().find(|x| x.id == j.0) else {
        return String::new();
    };
    let cell = cell_of(row.at);
    let mut same: Vec<&ew_api::output::TableJunction> = tables
        .junctions
        .iter()
        .filter(|x| cell_of(x.at) == cell)
        .collect();
    same.sort_by_key(|x| (x.at.1, x.at.0, x.id));
    if same.len() < 2 {
        return cell;
    }
    let n = same.iter().position(|x| x.id == j.0).map_or(0, |i| i + 1);
    format!("{cell}#{n}")
}

/// The label of the far end of connection `c` seen from junction `j`: the CP's label, or `@` and the cell of another
/// junction.
fn far_label(
    tables: &PageTables,
    j: EntityRef,
    end_a: TableEnd,
    end_b: TableEnd,
    cell_of: &dyn Fn((i64, i64)) -> String,
) -> String {
    let far = match (end_a, end_b) {
        (TableEnd::Junction { id }, other) if id == j.0 => other,
        (other, _) => other,
    };
    match far {
        TableEnd::Cp { function, idx } => tables
            .cps
            .iter()
            .find(|c| c.function == function && c.idx == idx)
            .map_or_else(String::new, |c| c.label.clone()),
        TableEnd::Junction { id } => format!("@{}", junction_name(tables, EntityRef(id), cell_of)),
    }
}

impl JunctionEditor {
    /// The editor of junction `j` read from `tables`; `None` when the tables do not hold it. `cell_of` names the frame
    /// cell of a page point; `wires` are the physical wires through it.
    pub fn read(
        tables: &PageTables,
        j: EntityRef,
        cell_of: &dyn Fn((i64, i64)) -> String,
        wires: Vec<JunctionWire>,
        rev: (u64, u64),
        row: usize,
    ) -> Option<JunctionEditor> {
        let row_j = tables.junctions.iter().find(|x| x.id == j.0)?;
        let members: Vec<JunctionMember> = row_j
            .sequence
            .iter()
            .map(|c| {
                let label = tables
                    .connections
                    .iter()
                    .find(|x| x.id == *c)
                    .map_or_else(String::new, |x| far_label(tables, j, x.a, x.b, cell_of));
                JunctionMember {
                    connection: EntityRef(*c),
                    label,
                }
            })
            .collect();
        Some(JunctionEditor {
            junction: j,
            cell: junction_name(tables, j, cell_of),
            style: row_j.style.clone(),
            bevel_orient: row_j.bevel_orient,
            wiring: row_j.wiring.clone(),
            explicit_wiring: row_j.explicit_wiring,
            row: row.min(members.len().saturating_sub(1)),
            members,
            wires,
            rev,
        })
    }

    /// The sequence with member `from` moved to position `to`; `None` for an index out of range or an unchanged
    /// order.
    pub fn reordered(&self, from: usize, to: usize) -> Option<Vec<EntityRef>> {
        if from >= self.members.len() || to >= self.members.len() || from == to {
            return None;
        }
        let mut seq: Vec<EntityRef> = self.members.iter().map(|m| m.connection).collect();
        let c = seq.remove(from);
        seq.insert(to, c);
        Some(seq)
    }

    /// `junction.edit {junction, …}` with `members`.
    pub fn edit(&self, members: Value) -> Value {
        let mut params = json!({ "junction": self.junction.0.to_string() });
        if let (Some(p), Value::Object(m)) = (params.as_object_mut(), members) {
            p.extend(m);
        }
        params
    }

    /// The JSON of a style: `"dot"` or `{"bevel": {"orient": "r90"}}` (orientation A omitted).
    pub fn style_json(style: &str, orient: u8) -> Option<Value> {
        match style {
            "dot" => Some(json!("dot")),
            "bevel" if orient.is_multiple_of(8) => Some(json!({"bevel": {}})),
            "bevel" => Some(json!({"bevel": {"orient": super::ORIENT_NAMES[usize::from(orient % 8)]}})),
            _ => None,
        }
    }

    /// The labels of the members, in wiring order.
    pub fn sequence_labels(&self) -> Vec<String> {
        self.members.iter().map(|m| m.label.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_api::output::TableJunction;

    fn id<T: std::str::FromStr>(n: u128) -> T
    where
        T::Err: std::fmt::Debug,
    {
        format!("{n:032x}").parse().unwrap()
    }

    fn junction(n: u128, at: (i64, i64)) -> TableJunction {
        TableJunction {
            id: id(n),
            at,
            style: "dot".into(),
            bevel_orient: 0,
            wiring: "star".into(),
            explicit_wiring: false,
            sequence: Vec::new(),
        }
    }

    #[test]
    fn two_junctions_in_one_cell_are_numbered_in_id_order() {
        let tables = PageTables {
            folio: "1".into(),
            page: id(1),
            grid: (1, 1),
            reps: Vec::new(),
            cps: Vec::new(),
            connections: Vec::new(),
            junctions: vec![junction(2, (0, 0)), junction(3, (5, 0)), junction(4, (50, 0))],
            items: Vec::new(),
            structure_boxes: Vec::new(),
        };
        let cell = |p: (i64, i64)| {
            if p.0 < 10 {
                "C11".to_owned()
            } else {
                "C12".to_owned()
            }
        };
        assert_eq!(junction_name(&tables, EntityRef(id(2)), &cell), "C11#1");
        assert_eq!(junction_name(&tables, EntityRef(id(3)), &cell), "C11#2");
        assert_eq!(junction_name(&tables, EntityRef(id(4)), &cell), "C12");
    }

    #[test]
    fn junctions_in_one_cell_are_numbered_by_position_not_by_id() {
        // The larger ID lies left of the smaller one, and a third lies above both.
        let tables = PageTables {
            folio: "1".into(),
            page: id(1),
            grid: (1, 1),
            reps: Vec::new(),
            cps: Vec::new(),
            connections: Vec::new(),
            junctions: vec![junction(2, (5, 3)), junction(3, (0, 3)), junction(4, (9, 1))],
            items: Vec::new(),
            structure_boxes: Vec::new(),
        };
        let cell = |_: (i64, i64)| "C11".to_owned();
        assert_eq!(junction_name(&tables, EntityRef(id(4)), &cell), "C11#1");
        assert_eq!(junction_name(&tables, EntityRef(id(3)), &cell), "C11#2");
        assert_eq!(junction_name(&tables, EntityRef(id(2)), &cell), "C11#3");
    }
}
