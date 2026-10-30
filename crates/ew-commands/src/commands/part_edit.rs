// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `part.edit_copy` (`EditPartCopy`, WP-S2.07 D20, D21; FT-394): the part editor's command, always previewed.

use std::collections::BTreeSet;

use ew_ids::{Id, v5};
use ew_model::{LINKS, LinkDecl};
use ew_schema::{
    Connection, Device, Kind, LibCopy, LibDocument, LibItem, Manufacturer, Part, PotentialDef, TechValue,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::commands::library::library_copy_hash;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// The members of a part that `part.edit_copy` edits (D20); every other member must equal the copy's.
pub const PART_EDITABLE: [&str; 10] = [
    "manufacturer",
    "part_number",
    "type_designation",
    "order_number",
    "gtin",
    "description",
    "kind",
    "category",
    "unit",
    "tech_data",
];

/// The order units a part takes (D21).
const UNITS: [&str; 3] = ["piece", "m", "set"];

/// `part.edit_copy` (`EditPartCopy`, WP-S2.07 D20, D21; FT-394): edits the identity, description, category, unit
/// and technical data of a part copy the project holds, in one undo step. Always previewed (`Bulk`).
///
/// - A **project-local copy** (`library_id` is the project ID, local forks included) is rewritten in place: the same
///   copy ID, item ID and version, the item replaced and `sha256` recomputed.
/// - A **copy from a library** is forked (doc 07 §3.1, "local edits fork the item"; a library item is never edited
///   from inside a project): a new project-local copy with item ID `fork`, copy ID `v5(NS_LIBCOPY, project ‖ fork)`,
///   the original's version and `meta.source` = `{"fork_of": {"library", "item", "version"}}`; every reference to the
///   original copy that the link table (`ew_model::LINKS`) lists as a part-copy link (the part uses, a strip
///   accessory's part, an assembly configuration's part, a connection's `overrides.wire_part`, a potential
///   definition's `props.wire_part`) names the fork afterwards. The original copy stays, unused, until purged. The
///   caller mints `fork` (the session's `next_fork_item`), so a replay reproduces it.
///
/// `part` is the edited part item (its members, without the copy metadata). Only `manufacturer`, `part_number`,
/// `type_designation`, `order_number`, `gtin`, `description`, `kind`, `category`, `unit` and `tech_data` may differ
/// from the copy's; so no edit changes a connection point, the function template, the contacts, the sections or the
/// mechanical data. `copies` carries library documents the edit names that the project lacks (a manufacturer),
/// embedded as `part.assign` embeds its closure.
///
/// Rejected, with the pointer: `/copy` not a part copy the project holds; `/part` not a part item, or one whose
/// other members differ from the copy's; `/part/manufacturer` missing, or a changed reference to a manufacturer copy
/// the project does not hold and `copies` does not carry; `/part/part_number` empty; `/part/unit` not `piece`, `m` or
/// `set`; `/part/tech_data/<i>` not a technical value, or a key given twice; `/fork` absent for a copy from a library,
/// given for a project-local copy, or naming an item whose copy the project holds; `/copies/<i>` as `library.embed`
/// refuses, or another version of a held copy. The whole command is refused when the item equals the copy's ("the
/// part is unchanged"). Bulk. Outcome: the edited copy (the fork's copy for a fork).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditPartCopy {
    /// The part copy to edit.
    #[schemars(with = "UuidS")]
    pub copy: Id<Part>,
    /// The edited part item: the part's members, without the copy metadata.
    pub part: Value,
    /// The item ID of the fork, for a copy from a library only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub fork: Option<Id<LibItem>>,
    /// Library documents to embed (`lib/` JSON form): a manufacturer the project lacks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

fn bad(at: impl Into<String>, message: impl Into<String>) -> CmdError {
    CmdError::invalid_params(EditPartCopy::NAME, at, message)
}

/// The part members without the editable ones (D20), for the "other members unchanged" check.
fn fixed_members(part: &Part) -> Map<String, Value> {
    let mut m = match serde_json::to_value(part) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    };
    for k in PART_EDITABLE {
        m.remove(k);
    }
    m
}

/// The edited item of `raw`, with D21's checks at their pointers; the technical data are put into key order.
fn read_part(raw: &Value) -> Result<Part, CmdError> {
    let Value::Object(map) = raw else {
        return Err(bad(
            "/part",
            "not a part item: give the part's members as an object",
        ));
    };
    let mut map = map.clone();
    if map.get("manufacturer").is_none_or(Value::is_null) {
        return Err(bad("/part/manufacturer", "a part needs a manufacturer"));
    }
    if map
        .get("part_number")
        .is_none_or(|v| v.as_str().is_some_and(|s| s.trim().is_empty()))
    {
        return Err(bad("/part/part_number", "a part needs a part number"));
    }
    if let Some(unit) = map.get("unit") {
        if !unit.as_str().is_some_and(|u| UNITS.contains(&u)) {
            return Err(bad("/part/unit", "the unit is piece, m or set"));
        }
    }
    if let Some(list) = map.get("tech_data") {
        let Value::Array(items) = list else {
            return Err(bad("/part/tech_data", "the technical data are a list"));
        };
        let mut seen = BTreeSet::new();
        let mut values = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            let v: TechValue = serde_json::from_value(item.clone()).map_err(|e| {
                bad(
                    format!("/part/tech_data/{i}"),
                    format!("not a technical value: {e}"),
                )
            })?;
            if !seen.insert(v.key.as_str().to_owned()) {
                return Err(bad(
                    format!("/part/tech_data/{i}"),
                    format!("{} is given twice", v.key.as_str()),
                ));
            }
            values.push(v);
        }
        let sorted =
            ew_schema::KeyedList::from_vec(values).map_err(|e| bad("/part/tech_data", e.to_string()))?;
        if sorted.is_empty() {
            map.remove("tech_data");
        } else {
            map.insert(
                "tech_data".into(),
                serde_json::to_value(&sorted).map_err(|e| bad("/part/tech_data", e.to_string()))?,
            );
        }
    }
    serde_json::from_value(Value::Object(map)).map_err(|e| bad("/part", format!("not a part item: {e}")))
}

/// Replaces the string `from` by `to` at the dotted `path` of `v` (`parts[].part.copy`: `[]` walks every element of
/// an array member); whether anything changed.
fn repoint_at(v: &mut Value, path: &[&str], from: &str, to: &str) -> bool {
    let Some((head, rest)) = path.split_first() else {
        if v.as_str() == Some(from) {
            *v = Value::String(to.to_owned());
            return true;
        }
        return false;
    };
    let (name, each) = match head.strip_suffix("[]") {
        Some(n) => (n, true),
        None => (*head, false),
    };
    let Some(member) = v.get_mut(name) else {
        return false;
    };
    if each {
        let Some(items) = member.as_array_mut() else {
            return false;
        };
        let mut changed = false;
        for item in items {
            changed |= repoint_at(item, rest, from, to);
        }
        changed
    } else {
        repoint_at(member, rest, from, to)
    }
}

/// Re-points one link's references from `from` to `to` on every record of the link's kind `T`.
fn repoint_kind<T>(
    tx: &mut TxBuilder<'_>,
    link: &LinkDecl,
    from: Id<Part>,
    to: Id<Part>,
) -> Result<(), CmdError>
where
    T: ew_model::Entity + Serialize + DeserializeOwned,
{
    let path: Vec<&str> = link.field.split('.').collect();
    let (from, to) = (from.to_string(), to.to_string());
    for row in tx.iter::<T>() {
        let mut v = serde_json::to_value(&*row).map_err(|e| CmdError::internal(e.to_string()))?;
        if repoint_at(&mut v, &path, &from, &to) {
            let record: T = serde_json::from_value(v).map_err(|e| CmdError::internal(e.to_string()))?;
            tx.put(record);
        }
    }
    Ok(())
}

/// Whether the command re-points a part-copy link of the link table (D20): `true` for the kinds [`repoint_kind`] is
/// called with in `run`. A link from another kind fails `part_copy_links_are_all_repointed` (R7).
#[cfg(test)]
fn repointed_link(link: &LinkDecl) -> bool {
    matches!(link.from, Kind::Device | Kind::Connection | Kind::PotentialDef)
}

impl Command for EditPartCopy {
    const NAME: &'static str = "part.edit_copy";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(held) = tx.get::<LibCopy<Part>>(self.copy) else {
            return Err(bad(
                "/copy",
                format!("the project holds no part copy {}", self.copy),
            ));
        };
        let mut part = read_part(&self.part)?;
        if fixed_members(&part) != fixed_members(&held.item) {
            return Err(bad(
                "/part",
                "only the identity, the description, the category, the unit and the technical data can be edited",
            ));
        }
        let project = tx.project().id.as_uuid();
        let local = held.library_id.as_uuid() == project;
        let fork_copy: Option<Id<Part>> = match (local, self.fork) {
            (true, Some(_)) => {
                return Err(bad(
                    "/fork",
                    "a project-local copy is edited in place; give no `fork`",
                ));
            }
            (false, None) => {
                return Err(bad(
                    "/fork",
                    "a copy from a library is forked: give `fork`, the item ID of the project's copy",
                ));
            }
            (true, None) => None,
            (false, Some(f)) => {
                let id: Id<Part> = v5::lib_copy_id(project, f.as_uuid());
                if tx.exists::<LibCopy<Part>>(id) {
                    return Err(bad(
                        "/fork",
                        format!("the project holds item {f} already; mint another fork item"),
                    ));
                }
                Some(id)
            }
        };
        embed_closure(Self::NAME, tx, &self.copies)?;
        let mfr = &part.manufacturer;
        let mfr_changed =
            mfr.library != held.item.manufacturer.library || mfr.item != held.item.manufacturer.item;
        if mfr_changed {
            let id: Id<Manufacturer> = v5::lib_copy_id(mfr.library.as_uuid(), mfr.item.as_uuid());
            if !tx.exists::<LibCopy<Manufacturer>>(id) {
                return Err(bad(
                    "/part/manufacturer",
                    format!(
                        "the project holds no manufacturer {}/{} and `copies` does not carry it",
                        mfr.library, mfr.item
                    ),
                ));
            }
        }
        if part == held.item {
            return tx.reject("the part is unchanged");
        }
        let primary = match fork_copy {
            None => {
                let mut copy = (*held).clone();
                copy.item = part;
                copy.sha256 = hash(&copy)?;
                tx.put(copy);
                self.copy
            }
            Some(id) => {
                let fork = self.fork.expect("checked above");
                part.meta.extra.insert(
                    "source".into(),
                    json!({"fork_of": {
                        "library": held.library_id.as_uuid().to_string(),
                        "item": held.item_id.as_uuid().to_string(),
                        "version": held.version,
                    }}),
                );
                let mut copy = LibCopy {
                    id,
                    v: held.v,
                    library_id: Id::from_uuid(project),
                    item_id: fork,
                    version: held.version.clone(),
                    sha256: held.sha256,
                    item: part,
                };
                copy.sha256 = hash(&copy)?;
                tx.insert(copy)?;
                for link in LINKS.iter().filter(|l| l.to == Kind::Part && !l.lib_ref) {
                    match link.from {
                        Kind::Device => repoint_kind::<Device>(tx, link, self.copy, id)?,
                        Kind::Connection => repoint_kind::<Connection>(tx, link, self.copy, id)?,
                        Kind::PotentialDef => repoint_kind::<PotentialDef>(tx, link, self.copy, id)?,
                        other => {
                            return Err(CmdError::internal(format!(
                                "part.edit_copy does not re-point the part-copy link {other}.{}",
                                link.field
                            )));
                        }
                    }
                }
                id
            }
        };
        Ok(Outcome::primary((Kind::Part, primary.cast())))
    }
}

/// The `sha256` of a part copy's item (EW-CORE-014).
fn hash(copy: &LibCopy<Part>) -> Result<ew_schema::Sha256, CmdError> {
    library_copy_hash(&LibDocument::Part(copy.clone()))
        .map(ew_schema::Sha256)
        .ok_or_else(|| CmdError::internal("a part copy has a hash"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repoint_walks_lists_and_members() {
        let mut v = json!({"parts": [{"part": {"copy": "a"}}, {"part": {"copy": "b"}}], "x": "a"});
        assert!(repoint_at(&mut v, &["parts[]", "part", "copy"], "a", "z"));
        assert_eq!(
            v,
            json!({"parts": [{"part": {"copy": "z"}}, {"part": {"copy": "b"}}], "x": "a"})
        );
        assert!(!repoint_at(&mut v, &["overrides", "wire_part", "copy"], "a", "z"));
    }

    /// R7: every part-copy link of the link table is one the command re-points.
    #[test]
    fn part_copy_links_are_all_repointed() {
        let links: Vec<String> = LINKS
            .iter()
            .filter(|l| l.to == Kind::Part && !l.lib_ref)
            .map(LinkDecl::path)
            .collect();
        assert!(links.iter().any(|l| l == "device.parts[].part.copy"), "{links:?}");
        for l in LINKS.iter().filter(|l| l.to == Kind::Part && !l.lib_ref) {
            assert!(repointed_link(l), "{} is not re-pointed by a fork", l.path());
        }
    }
}
