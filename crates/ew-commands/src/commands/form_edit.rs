// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `form.edit` (`EditForm`): a frame or title block changed on the page (doc 09 §3.9), and the one way a form copy
//! changes: in place when it is the project's own, else forked with every form and reference that names it.

use std::collections::BTreeMap;

use ew_format::library_copy_hash;
use ew_ids::{Id, v5};
use ew_l10n::Text;
use ew_mltext::MlText;
use ew_schema::{
    FormDef, FormKind, ImagePrim, Kind, LibCopy, LibDocument, LibItem, LinePrim, Page, Pen, Primitive,
    ReportBlock, Stroke, UserPropScope,
};
use ew_units::{Nm, Pt2, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::command::{Command, Outcome};
use crate::commands::frames::FRAMES_MEMBER;
use crate::commands::user_prop::DefineUserProp;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{MlTextS, UuidS};
use crate::tx::TxBuilder;

/// The smallest width and height of a title block's cell: 2 mm (doc 09 §3.9).
pub const MIN_CELL: Nm = Nm(2_000_000);

/// A field of the project's own made from a cell (New field of your own… under Shows), in the same undo step.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldFromCell {
    /// The index of the cell, in the changed form's `cells`, that prints the field.
    pub cell: usize,
    /// The field's name.
    #[schemars(with = "MlTextS")]
    pub label: MlText,
    /// One value for the whole project (`project`) or one for each page (`page`).
    pub scope: UserPropScope,
    /// Its value now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<MlTextS>")]
    pub value: Option<MlText>,
    /// The page whose value `value` is, for a field of each page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub page: Option<Id<Page>>,
}

/// A picture put into a cell of the title block alone (A picture…, doc 09 §3.9).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PictureInCell {
    /// The index of the cell, in the changed form's `cells`.
    pub cell: usize,
    /// The image file (SVG, PNG or JPEG, at most 512 KiB) in base64.
    pub image: String,
}

/// `form.edit` (`EditForm`): replaces the item of a frame or title-block copy the project holds with `form`, in one
/// undo step (doc 09 §3.9).
///
/// - A title block's lines are its cells' edges (`FormDef::cell_edges`): its line graphics are written anew from the
///   cells, in the stroke of its first line (the 0.26 mm pen without one); its other graphics stay.
/// - `field` makes a field of the project's own as `user_prop.define` does and gives the cell `field.cell` its
///   placeholder (`{project.prop.<key>}` or `{page.prop.<key>}`), the cell's text becoming the name and a colon where
///   it had none. `picture` puts an image into the title block, fitted into the cell `picture.cell` at its proportions
///   and centred, and an asset of the project.
/// - A copy that is not the project's own (a starter form, a library's) is **forked**, as `part.edit_copy` forks a
///   part: a new copy of the project's own with item ID `forks[0]` and `meta.source.fork_of`; for a title block, every
///   frame the project holds that names it and is not the project's own is forked too, in copy ID order, with item IDs
///   `forks[1..]`, naming the new block. Every page, report block, form and the frame defaults that named an old copy
///   name its fork; the old copies stay unused until purged. The caller mints the item IDs (the session's
///   `next_fork_item`). A copy of the project's own is changed in place.
///
/// Rejected, with the pointer: `/copy` not a frame or title block the project holds; `/forks` too few for the copies
/// to fork, or naming a copy the project holds; `/form` not a form of the copy's kind, a cell that overlaps another or
/// leaves the block, or one narrower or lower than 2 mm, a picture whose asset the project does not hold; `/field`
/// and `/picture` as `user_prop.define` and `project.company.set`'s logo refuse them, and a cell index that is no
/// cell. The whole command is refused when nothing changes. Outcome: the changed copy (the fork for a fork).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditForm {
    /// The frame or title-block copy to change.
    #[schemars(with = "UuidS")]
    pub copy: Id<FormDef>,
    /// The changed form: the item's members, without the copy metadata.
    pub form: Value,
    /// The item IDs of the forks, for a copy that is not the project's own: the form's, then each frame's that names it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub forks: Vec<Id<LibItem>>,
    /// A field of the project's own made from a cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<FieldFromCell>,
    /// A picture put into a cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<PictureInCell>,
}

fn bad(command: &str, at: impl Into<String>, message: Text) -> CmdError {
    CmdError::invalid_params(command, at, message)
}

/// The `sha256` of a form copy's item (EW-CORE-014).
fn hash(copy: &LibCopy<FormDef>) -> Result<ew_schema::Sha256, CmdError> {
    library_copy_hash(&LibDocument::Form(copy.clone()))
        .map(ew_schema::Sha256)
        .ok_or_else(|| CmdError::internal("a form copy has a hash"))
}

/// The frame or title-block copy `copy` the project holds, else refused at `/copy`.
pub(crate) fn held_frame_form(
    tx: &TxBuilder<'_>,
    command: &str,
    copy: Id<FormDef>,
) -> Result<std::sync::Arc<LibCopy<FormDef>>, CmdError> {
    match tx.get::<LibCopy<FormDef>>(copy) {
        Some(c) if matches!(c.item.kind, FormKind::Frame | FormKind::TitleBlock) => Ok(c),
        _ => Err(bad(command, "/copy", text!("refusal-form-edit-not-frame"))),
    }
}

/// A table's cells checked (doc 09 §2.5): inside the block, at least 2 mm each way, none overlapping another;
/// refused at `/form/cells/<i>`.
fn check_cells(command: &str, form: &FormDef) -> Result<(), CmdError> {
    let block = Rect::from_points(Pt2::ORIGIN, Pt2::new(form.sheet.width, form.sheet.height));
    let rects: Vec<Rect> = form.cells.iter().map(|c| c.rect.0).collect();
    for (i, r) in rects.iter().enumerate() {
        let at = format!("/form/cells/{i}");
        if !block.holds(*r) {
            return Err(bad(command, at, text!("refusal-form-edit-cell-outside")));
        }
        if r.width() < MIN_CELL || r.height() < MIN_CELL {
            return Err(bad(command, at, text!("refusal-form-edit-cell-small")));
        }
        let overlaps = rects[..i].iter().any(|o| {
            r.min().x < o.max().x && o.min().x < r.max().x && r.min().y < o.max().y && o.min().y < r.max().y
        });
        if overlaps {
            return Err(bad(command, at, text!("refusal-form-edit-cell-overlap")));
        }
    }
    Ok(())
}

/// A title block's line graphics written anew from its cells (doc 09 §2.5): the lines go, its other graphics stay,
/// and its cells' edges are its lines, in the stroke of its first line.
fn lines_from_cells(form: &mut FormDef) {
    let stroke = form
        .graphics
        .iter()
        .find_map(|g| match g {
            Primitive::Line(l) => l.stroke.clone(),
            _ => None,
        })
        .unwrap_or_else(|| Stroke {
            pen: Pen::P026,
            dash: Default::default(),
            colour: Default::default(),
            extra: Default::default(),
        });
    form.graphics.retain(|g| !matches!(g, Primitive::Line(_)));
    form.graphics
        .extend(form.cell_edges().into_iter().map(|(from, to)| {
            Primitive::Line(LinePrim {
                from,
                to,
                stroke: Some(stroke.clone()),
                extra: Default::default(),
            })
        }));
}

/// Changes the form copy `copy` to `item` (see [`EditForm`]): in place when it is the project's own, else forked with
/// the frames that name it (a title block) and every reference re-pointed. `forks` are the item IDs of the forks, in
/// order. The copy the project names afterwards.
pub(crate) fn change_form(
    tx: &mut TxBuilder<'_>,
    command: &str,
    copy: Id<FormDef>,
    item: FormDef,
    forks: &[Id<LibItem>],
) -> Result<Id<FormDef>, CmdError> {
    let held = tx.require::<LibCopy<FormDef>>(copy)?;
    let project = tx.project().id.as_uuid();
    if held.library_id.as_uuid() == project {
        if !forks.is_empty() {
            return Err(bad(command, "/forks", text!("refusal-form-edit-local-fork")));
        }
        let mut changed = (*held).clone();
        changed.item = item;
        changed.sha256 = hash(&changed)?;
        tx.put(changed);
        return Ok(copy);
    }
    // The copies to fork: the form, then (a title block's) every frame that names it and is not the project's own.
    let mut to_fork: Vec<(Id<FormDef>, FormDef)> = vec![(copy, item)];
    if held.item.kind == FormKind::TitleBlock {
        let mut frames: Vec<_> = tx
            .iter::<LibCopy<FormDef>>()
            .into_iter()
            .filter(|f| f.item.title_block == Some(copy) && f.library_id.as_uuid() != project)
            .collect();
        frames.sort_by_key(|f| f.id);
        to_fork.extend(frames.into_iter().map(|f| (f.id, f.item.clone())));
    }
    if forks.len() < to_fork.len() {
        return Err(bad(command, "/forks", text!("refusal-form-edit-needs-forks")));
    }
    let mut renamed: BTreeMap<Id<FormDef>, Id<FormDef>> = BTreeMap::new();
    for ((old, _), fork) in to_fork.iter().zip(forks) {
        let id: Id<FormDef> = v5::lib_copy_id(project, fork.as_uuid());
        if tx.exists::<LibCopy<FormDef>>(id) {
            return Err(bad(command, "/forks", text!("refusal-form-edit-fork-held")));
        }
        renamed.insert(*old, id);
    }
    for ((old, mut item), fork) in to_fork.into_iter().zip(forks) {
        let original = tx.require::<LibCopy<FormDef>>(old)?;
        item.meta.extra.insert(
            "source".into(),
            json!({"fork_of": {
                "library": original.library_id.as_uuid().to_string(),
                "item": original.item_id.as_uuid().to_string(),
                "version": original.version,
            }}),
        );
        if let Some(block) = item.title_block.and_then(|b| renamed.get(&b)) {
            item.title_block = Some(*block);
        }
        let mut forked = LibCopy {
            id: renamed[&old],
            v: original.v,
            library_id: Id::from_uuid(project),
            item_id: *fork,
            version: original.version.clone(),
            sha256: original.sha256,
            item,
        };
        forked.sha256 = hash(&forked)?;
        tx.insert(forked)?;
    }
    repoint_forms(tx, &renamed)?;
    Ok(renamed[&copy])
}

/// Every reference to a renamed form copy names its new copy: a page's frame, a report block's form, a form's title
/// block and continuation (the links of `ew_model::LINKS` to forms) and the frame defaults of the project settings.
pub(crate) fn repoint_forms(
    tx: &mut TxBuilder<'_>,
    renamed: &BTreeMap<Id<FormDef>, Id<FormDef>>,
) -> Result<(), CmdError> {
    let new = |id: Id<FormDef>| renamed.get(&id).copied();
    for page in tx.iter::<Page>() {
        if let Some(to) = page.form.and_then(new) {
            tx.update(page.id, |p: &mut Page| p.form = Some(to))?;
        }
    }
    for block in tx.iter::<ReportBlock>() {
        if let Some(to) = new(block.form) {
            tx.update(block.id, |b: &mut ReportBlock| b.form = to)?;
        }
    }
    // The old copies stay as they were, unused until purged.
    for form in tx
        .iter::<LibCopy<FormDef>>()
        .into_iter()
        .filter(|f| !renamed.contains_key(&f.id))
    {
        let block = form.item.title_block.and_then(new);
        let next = form.item.continuation.and_then(new);
        if block.is_some() || next.is_some() {
            let mut changed = (*form).clone();
            changed.item.title_block = block.or(changed.item.title_block);
            changed.item.continuation = next.or(changed.item.continuation);
            changed.sha256 = hash(&changed)?;
            tx.put(changed);
        }
    }
    // The frame defaults (`ProjectSettings.extra.frames`): its `default` and each entry's `form`.
    let mut project = (*tx.project()).clone();
    let mut changed = false;
    if let Some(Value::Object(frames)) = project.settings.extra.get_mut(FRAMES_MEMBER) {
        let mut swap = |v: &mut Value| {
            let to = v
                .as_str()
                .and_then(|s| s.parse::<ew_ids::Uuid>().ok())
                .and_then(|u| new(Id::from_uuid(u)));
            if let Some(to) = to {
                *v = Value::String(to.to_string());
                changed = true;
            }
        };
        if let Some(d) = frames.get_mut("default") {
            swap(d);
        }
        if let Some(Value::Array(entries)) = frames.get_mut("per_page_type") {
            for e in entries {
                if let Some(f) = e.get_mut("form") {
                    swap(f);
                }
            }
        }
    }
    if changed {
        tx.set_project(project)?;
    }
    Ok(())
}

/// The rectangle of an image of intrinsic size `size` fitted into `cell` at its proportions and centred; the whole
/// cell for an unknown size.
fn fitted(cell: Rect, size: Option<(u32, u32)>) -> Rect {
    let Some((w, h)) = size.filter(|(w, h)| *w > 0 && *h > 0) else {
        return cell;
    };
    let (cw, ch) = (i128::from(cell.width().0), i128::from(cell.height().0));
    let (w, h) = (i128::from(w), i128::from(h));
    // The larger of the two scales that fits: width-bound when the image is wider than the cell.
    let (fw, fh) = if cw * h <= ch * w {
        (cw, cw * h / w)
    } else {
        (ch * w / h, ch)
    };
    let x = i128::from(cell.min().x.0) + (cw - fw) / 2;
    let y = i128::from(cell.min().y.0) + (ch - fh) / 2;
    let nm = |v: i128| Nm(i64::try_from(v).unwrap_or(0));
    Rect::from_points(Pt2::new(nm(x), nm(y)), Pt2::new(nm(x + fw), nm(y + fh)))
}

impl Command for EditForm {
    const NAME: &'static str = "form.edit";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let held = held_frame_form(tx, Self::NAME, self.copy)?;
        let mut form: FormDef = serde_json::from_value(self.form.clone()).map_err(|e| {
            bad(
                Self::NAME,
                "/form",
                text!("refusal-params-detail", crate::error::serde_error_text(&e)),
            )
        })?;
        if form.kind != held.item.kind {
            return Err(bad(Self::NAME, "/form/kind", text!("refusal-form-edit-kind")));
        }
        if form == held.item && self.field.is_none() && self.picture.is_none() {
            return tx.reject(text!("refusal-form-edit-unchanged"));
        }
        let cells = form.cells.len();
        let cell_of = |at: &str, i: usize| {
            if i < cells {
                Ok(i)
            } else {
                Err(bad(Self::NAME, at, text!("refusal-form-edit-no-cell")))
            }
        };
        if let Some(f) = &self.field {
            let i = cell_of("/field/cell", f.cell)?;
            let define = DefineUserProp {
                label: f.label.clone(),
                scope: f.scope.clone(),
                value: f.value.clone(),
                page: f.page,
            };
            let key = define.define(tx, Self::NAME)?;
            let path = crate::commands::user_prop::user_prop_path(&f.scope, key.as_str())
                .ok_or_else(|| bad(Self::NAME, "/field/scope", text!("refusal-user-prop-scope")))?;
            let cell = &mut form.cells[i];
            cell.value = Some(format!("{{{path}}}"));
            let text = cell
                .label
                .as_ref()
                .map(|l| ew_derive::text_in(l, None).trim().to_owned())
                .unwrap_or_default();
            if text.is_empty() {
                let name = ew_derive::text_in(&f.label, None).trim().to_owned();
                cell.label = Some(MlText::neutral(&format!("{name}:")));
            }
        }
        if let Some(p) = &self.picture {
            let i = cell_of("/picture/cell", p.cell)?;
            let reference = crate::commands::company::store_image(
                tx,
                Self::NAME,
                "/picture/image",
                (
                    text!("refusal-form-edit-the-picture"),
                    text!("refusal-form-edit-a-picture"),
                ),
                &p.image,
            )?;
            let rect = fitted(form.cells[i].rect.0, reference.size);
            form.graphics.push(Primitive::Image(ImagePrim {
                rect: ew_schema::Bounds(rect),
                asset: reference.sha256,
                extra: Default::default(),
            }));
        }
        if form.kind == FormKind::TitleBlock {
            check_cells(Self::NAME, &form)?;
            lines_from_cells(&mut form);
        }
        for (i, g) in form.graphics.iter().enumerate() {
            if let Primitive::Image(img) = g
                && tx.asset(&img.asset).is_none()
            {
                return Err(bad(
                    Self::NAME,
                    format!("/form/graphics/{i}"),
                    text!("refusal-form-edit-picture-missing"),
                ));
            }
        }
        let id = change_form(tx, Self::NAME, self.copy, form, &self.forks)?;
        Ok(Outcome::primary((Kind::Form, id.cast())))
    }
}
