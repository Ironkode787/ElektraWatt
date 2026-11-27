// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `part.edit_copy` (`EditPartCopy`): the sheet's command, always previewed.

use std::collections::BTreeSet;

use ew_ids::{Id, v5};
use ew_model::LINKS;
#[cfg(test)]
use ew_model::LinkDecl;
use ew_schema::{
    Connection, Device, Kind, LibCopy, LibDocument, LibItem, Manufacturer, Part, PotentialDef, TechValue,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::commands::library_update::{apply_remap, pin_moves};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;
use ew_format::library_copy_hash;
use ew_l10n::Text;

/// The order units a part takes.
const UNITS: [&str; 3] = ["piece", "m", "set"];

/// `part.edit_copy` (`EditPartCopy`): edits a part copy the project holds, in one undo step. Always previewed
/// (`Bulk`).
///
/// - A **project-local copy** (`library_id` is the project ID, local forks included) is rewritten in place: the same
///   copy ID, item ID and version, the item replaced and `sha256` recomputed.
/// - A **copy from a library** is forked (doc 07 §3.1, "local edits fork the item"; a library item is never edited
///   from inside a project): a new project-local copy with item ID `fork`, copy ID `v5(NS_LIBCOPY, project ‖ fork)`,
///   the original's version and `meta.source` = `{"fork_of": {"library", "item", "version"}}`; every reference to the
///   original copy that the link table (`ew_model::LINKS`) lists as a part-copy link (the part uses, a strip
///   accessory's part, an assembly configuration's part, a connection's `overrides.wire_part`, a potential
///   definition's `props.wire_part`, a carrier's part) names the fork afterwards, and so does a placement's `part` (the
///   part use it realises). The original copy stays, unused, until purged. The
///   caller mints `fork` (the session's `next_fork_item`), so a replay reproduces it.
///
/// `part` is the edited part item (its members, without the copy metadata): any member may differ from the copy's, so
/// a part of the project's own is corrected whole (doc 07 §5.4). The devices that use the copy take the new connection
/// points; a bus port a function carries keeps its value, N5. `copies` carries library documents the edit names that
/// the project lacks (a manufacturer), embedded as `part.assign` embeds its closure.
///
/// Rejected, with the pointer: `/copy` not a part copy the project holds; `/part` not a part item;
/// `/part/manufacturer` missing, or a changed reference to a manufacturer copy
/// the project does not hold and `copies` does not carry; `/part/part_number` empty; `/part/unit` not `piece`, `m` or
/// `set`; `/part/tech_data/<i>` not a technical value, or a key given twice; `/part/accessories/<i>` a part the project does
/// not hold (nor `copies` carries) or a wire, ferrule or marker; `/part/variants/<i>` a variant without its own part
/// number, order number or description; `/fork` absent for a copy from a library,
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

fn bad(at: impl Into<String>, message: Text) -> CmdError {
    CmdError::invalid_params(EditPartCopy::NAME, at, message)
}

/// The edited item of `raw`, with the checks at their pointers; the technical data are put into key order.
fn read_part(raw: &Value) -> Result<Part, CmdError> {
    let Value::Object(map) = raw else {
        return Err(bad("/part", text!("refusal-part-edit-not-object")));
    };
    let mut map = map.clone();
    if map.get("manufacturer").is_none_or(Value::is_null) {
        return Err(bad(
            "/part/manufacturer",
            text!("refusal-part-edit-no-manufacturer"),
        ));
    }
    if map
        .get("part_number")
        .is_none_or(|v| v.as_str().is_some_and(|s| s.trim().is_empty()))
    {
        return Err(bad("/part/part_number", text!("refusal-part-edit-no-number")));
    }
    if let Some(unit) = map.get("unit") {
        if !unit.as_str().is_some_and(|u| UNITS.contains(&u)) {
            return Err(bad("/part/unit", text!("refusal-part-edit-unit")));
        }
    }
    if let Some(list) = map.get("tech_data") {
        let Value::Array(items) = list else {
            return Err(bad("/part/tech_data", text!("refusal-part-edit-tech-data-list")));
        };
        let mut seen = BTreeSet::new();
        let mut values = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            let v: TechValue = serde_json::from_value(item.clone()).map_err(|e| {
                bad(
                    format!("/part/tech_data/{i}"),
                    text!("refusal-part-edit-tech-value", crate::error::serde_error_text(&e)),
                )
            })?;
            if !seen.insert(v.key.as_str().to_owned()) {
                return Err(bad(
                    format!("/part/tech_data/{i}"),
                    text!("refusal-part-edit-tech-twice", v.key.as_str()),
                ));
            }
            values.push(v);
        }
        let sorted = ew_schema::KeyedList::from_vec(values).map_err(|e| {
            bad(
                "/part/tech_data",
                text!("refusal-params-detail", crate::error::schema_error_text(&e)),
            )
        })?;
        if sorted.is_empty() {
            map.remove("tech_data");
        } else {
            map.insert(
                "tech_data".into(),
                serde_json::to_value(&sorted).map_err(|e| {
                    bad(
                        "/part/tech_data",
                        text!("refusal-params-detail", crate::error::serde_error_text(&e)),
                    )
                })?,
            );
        }
    }
    serde_json::from_value(Value::Object(map)).map_err(|e| {
        bad(
            "/part",
            text!("refusal-part-edit-not-part", crate::error::serde_error_text(&e)),
        )
    })
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

/// Re-points the references at the dotted `field` (a link's field) from `from` to `to` on every record of kind `T`.
fn repoint_kind<T>(tx: &mut TxBuilder<'_>, field: &str, from: Id<Part>, to: Id<Part>) -> Result<(), CmdError>
where
    T: ew_model::Entity + Serialize + DeserializeOwned,
{
    let path: Vec<&str> = field.split('.').collect();
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

/// Re-points every reference to the part copy `from` to the copy `to`: each part-copy link of the link table
/// (`ew_model::LINKS`: the part uses with their variants, a strip accessory's part, an assembly configuration's part, a
/// connection's `overrides.wire_part`, a potential definition's `props.wire_part`, a carrier's part) and a placement's
/// `part` (the part use it realises). Only the copy changes: what each use chose stays. A fork (`part.edit_copy`) and
/// Keep in my parts (`part.keep`) move every use with it.
pub(crate) fn repoint_part_copy(
    tx: &mut TxBuilder<'_>,
    from: Id<Part>,
    to: Id<Part>,
) -> Result<(), CmdError> {
    for link in LINKS.iter().filter(|l| l.to == Kind::Part && !l.lib_ref) {
        match link.from {
            Kind::Device => repoint_kind::<Device>(tx, link.field, from, to)?,
            Kind::Connection => repoint_kind::<Connection>(tx, link.field, from, to)?,
            Kind::PotentialDef => repoint_kind::<PotentialDef>(tx, link.field, from, to)?,
            Kind::Carrier => repoint_kind::<ew_schema::Carrier>(tx, link.field, from, to)?,
            Kind::Function => repoint_kind::<ew_schema::Function>(tx, link.field, from, to)?,
            Kind::Junction => repoint_kind::<ew_schema::Junction>(tx, link.field, from, to)?,
            Kind::Medium => repoint_kind::<ew_schema::Medium>(tx, link.field, from, to)?,
            Kind::Project => {
                let path: Vec<&str> = link.field.split('.').collect();
                let mut v =
                    serde_json::to_value(&*tx.project()).map_err(|e| CmdError::internal(e.to_string()))?;
                if repoint_at(&mut v, &path, &from.to_string(), &to.to_string()) {
                    let project = serde_json::from_value(v).map_err(|e| CmdError::internal(e.to_string()))?;
                    tx.set_project(project)?;
                }
            }
            other => {
                return Err(CmdError::internal(format!(
                    "a part copy's uses are not re-pointed along the part-copy link {other}.{}",
                    link.field
                )));
            }
        }
    }
    // A placement names the part use it realises by its copy (not a link); it follows the use, as a replaced part
    // follows in `part.assign`.
    repoint_kind::<ew_schema::Placement>(tx, "part.copy", from, to)
}

/// Whether the command re-points a part-copy link of the link table: `true` for the kinds [`repoint_kind`] is
/// called with in `run`. A link from another kind fails `part_copy_links_are_all_repointed`.
#[cfg(test)]
fn repointed_link(link: &LinkDecl) -> bool {
    matches!(
        link.from,
        Kind::Device
            | Kind::Connection
            | Kind::PotentialDef
            | Kind::Carrier
            | Kind::Function
            | Kind::Junction
            | Kind::Medium
            | Kind::Project
    )
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
                text!("refusal-part-edit-copy-missing", self.copy.to_string()),
            ));
        };
        // A starter part is never changed (doc 07 §5.8): a person makes his own from it.
        if held.library_id.as_uuid() == ew_schema::starter::parts_library() {
            return Err(bad("/copy", text!("refusal-part-edit-starter")));
        }
        let mut part = read_part(&self.part)?;
        let project = tx.project().id.as_uuid();
        let local = held.library_id.as_uuid() == project;
        let fork_copy: Option<Id<Part>> = match (local, self.fork) {
            (true, Some(_)) => {
                return Err(bad("/fork", text!("refusal-part-edit-local-fork")));
            }
            (false, None) => {
                return Err(bad("/fork", text!("refusal-part-edit-needs-fork")));
            }
            (true, None) => None,
            (false, Some(f)) => {
                let id: Id<Part> = v5::lib_copy_id(project, f.as_uuid());
                if tx.exists::<LibCopy<Part>>(id) {
                    return Err(bad("/fork", text!("refusal-part-edit-fork-held", f.to_string())));
                }
                Some(id)
            }
        };
        embed_closure(Self::NAME, tx, &self.copies)?;
        // What a part brings with it is a part the project holds (embedded with it), and never one a wire consumes:
        // the consumables count those (doc 07 §2.5).
        for (i, a) in part.accessories.iter().enumerate() {
            let id: Id<Part> = ew_derive::accessory_copy(a);
            let Some(copy) = tx.get::<LibCopy<Part>>(id) else {
                return Err(bad(
                    format!("/part/accessories/{i}"),
                    text!("refusal-part-edit-accessory-missing", (i + 1).to_string()),
                ));
            };
            if copy.item.kind.is_consumable() {
                return Err(bad(
                    format!("/part/accessories/{i}"),
                    text!("refusal-part-edit-accessory-kind", copy.item.part_number.as_str()),
                ));
            }
        }
        // A variant gives its own part number, order number or description, so that a list can tell it apart.
        for (i, v) in part.variants.iter().enumerate() {
            if !v.has_own_number() {
                return Err(bad(
                    format!("/part/variants/{i}"),
                    text!("refusal-part-edit-variant-number", v.key.as_str()),
                ));
            }
        }
        let mfr = &part.manufacturer;
        let mfr_changed =
            mfr.library != held.item.manufacturer.library || mfr.item != held.item.manufacturer.item;
        if mfr_changed {
            let id: Id<Manufacturer> = v5::lib_copy_id(mfr.library.as_uuid(), mfr.item.as_uuid());
            if !tx.exists::<LibCopy<Manufacturer>>(id) {
                return Err(bad(
                    "/part/manufacturer",
                    text!(
                        "refusal-part-edit-manufacturer-missing",
                        mfr.library.to_string(),
                        mfr.item.to_string()
                    ),
                ));
            }
        }
        if part == held.item {
            return tx.reject(text!("refusal-part-edit-unchanged"));
        }
        // A pin the correction numbers again keeps what is connected to it, the patterns paired by designation as
        // `library.update` pairs them (a wire at L+ stayed at the pin that took L+'s number); a pin it takes
        // away while something is connected to it is refused in words.
        let mut remap = std::collections::BTreeMap::new();
        let mut blocked = Vec::new();
        let corrected = LibDocument::Part(LibCopy {
            item: part.clone(),
            ..(*held).clone()
        });
        pin_moves(
            tx,
            &LibDocument::Part((*held).clone()),
            &corrected,
            &std::collections::BTreeMap::new(),
            &mut remap,
            &mut blocked,
        )?;
        if !blocked.is_empty() {
            return tx.reject(text!("refusal-part-edit-blocked", crate::words::list(blocked)));
        }
        apply_remap(tx, &remap)?;
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
                repoint_part_copy(tx, self.copy, id)?;
                id
            }
        };
        // The drawn boxes of its devices take the pins and sockets the part has now, each kept pin under its new
        // index.
        crate::commands::box_follow::follow_part_boxes(tx, primary, &remap)?;
        Ok(Outcome::primary((Kind::Part, primary.cast())))
    }
}

/// The `sha256` of a part copy's item (EW-CORE-014).
pub(super) fn hash(copy: &LibCopy<Part>) -> Result<ew_schema::Sha256, CmdError> {
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

    /// A technical value that does not deserialise is refused with serde's sentence by key, so the
    /// window words it in its language; its English is serde's own text.
    #[test]
    fn a_bad_technical_value_is_refused_by_key() {
        let raw = json!({"manufacturer": "x", "part_number": "P1", "tech_data": [5]});
        let serde = serde_json::from_value::<TechValue>(json!(5)).unwrap_err();
        let Err(CmdError::InvalidParams { text, .. }) = read_part(&raw) else {
            panic!("a bad technical value is refused")
        };
        assert_eq!(text.to_string(), format!("not a technical value: {serde}"));
        match text.args() {
            [ew_l10n::TextArg::Text(t)] => assert_eq!(t.key(), "refusal-serde-invalid-type"),
            other => panic!("{other:?}"),
        }
    }

    /// Every part-copy link of the link table is one the command re-points.
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
