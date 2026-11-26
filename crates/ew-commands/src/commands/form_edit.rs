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
    Counting, FormDef, FormKind, ImagePrim, Kind, LibCopy, LibDocument, LibItem, LinePrim, Page, Pen,
    Primitive, ReportBlock, Stroke, UserPropScope,
};
use ew_units::{Grid, Nm, Pt2, Rect};
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
    let renamed = change_forms(tx, command, vec![(copy, item)], forks)?;
    Ok(renamed.get(&copy).copied().unwrap_or(copy))
}

/// The copies a change of `edited` forks, in the order their forks' item IDs are given: each edited copy that is not
/// the project's own, in the order of `edited`, then every other frame that names an edited title block and is not the
/// project's own, in copy ID order. The one rule of `form.edit`, `frame.set_grid` and the window's fork IDs.
pub fn forked_copies(
    copies: &[(Id<FormDef>, &LibCopy<FormDef>)],
    edited: &[Id<FormDef>],
    project: ew_ids::Uuid,
) -> Vec<Id<FormDef>> {
    let local = |c: &LibCopy<FormDef>| c.library_id.as_uuid() == project;
    let held = |id: Id<FormDef>| copies.iter().find(|(i, _)| *i == id).map(|(_, c)| *c);
    let mut out: Vec<Id<FormDef>> = edited
        .iter()
        .copied()
        .filter(|id| held(*id).is_some_and(|c| !local(c)))
        .collect();
    let mut frames: Vec<Id<FormDef>> = copies
        .iter()
        .filter(|(id, c)| {
            !local(c)
                && !out.contains(id)
                && c.item.title_block.is_some_and(|b| {
                    edited.contains(&b) && held(b).is_some_and(|b| b.item.kind == FormKind::TitleBlock)
                })
        })
        .map(|(id, _)| *id)
        .collect();
    frames.sort();
    frames.dedup();
    out.extend(frames);
    out
}

/// Changes each copy of `edits` to its item: a copy of the project's own in place; the others, and every other frame
/// that names an edited title block and is not the project's own, forked under `forks` in [`forked_copies`] order, with
/// every reference re-pointed. The old copy → the copy the project names afterwards, for each forked one.
pub(crate) fn change_forms(
    tx: &mut TxBuilder<'_>,
    command: &str,
    edits: Vec<(Id<FormDef>, FormDef)>,
    forks: &[Id<LibItem>],
) -> Result<BTreeMap<Id<FormDef>, Id<FormDef>>, CmdError> {
    let project = tx.project().id.as_uuid();
    let held = tx.iter::<LibCopy<FormDef>>();
    let copies: Vec<(Id<FormDef>, &LibCopy<FormDef>)> = held.iter().map(|c| (c.id, &**c)).collect();
    let edited: Vec<Id<FormDef>> = edits.iter().map(|(id, _)| *id).collect();
    let to_fork = forked_copies(&copies, &edited, project);
    if to_fork.is_empty() && !forks.is_empty() {
        return Err(bad(command, "/forks", text!("refusal-form-edit-local-fork")));
    }
    if forks.len() < to_fork.len() {
        return Err(bad(command, "/forks", text!("refusal-form-edit-needs-forks")));
    }
    let mut renamed: BTreeMap<Id<FormDef>, Id<FormDef>> = BTreeMap::new();
    for (old, fork) in to_fork.iter().zip(forks) {
        let id: Id<FormDef> = v5::lib_copy_id(project, fork.as_uuid());
        if tx.exists::<LibCopy<FormDef>>(id) || renamed.values().any(|v| *v == id) {
            return Err(bad(command, "/forks", text!("refusal-form-edit-fork-held")));
        }
        renamed.insert(*old, id);
    }
    // The edited copies of the project's own, in place.
    for (id, item) in &edits {
        if renamed.contains_key(id) {
            continue;
        }
        let mut changed = (*tx.require::<LibCopy<FormDef>>(*id)?).clone();
        changed.item = item.clone();
        if let Some(block) = changed.item.title_block.and_then(|b| renamed.get(&b)) {
            changed.item.title_block = Some(*block);
        }
        changed.sha256 = hash(&changed)?;
        tx.put(changed);
    }
    for (old, fork) in to_fork.iter().zip(forks) {
        let original = tx.require::<LibCopy<FormDef>>(*old)?;
        let mut item = edits
            .iter()
            .find(|(id, _)| id == old)
            .map_or_else(|| original.item.clone(), |(_, item)| item.clone());
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
            id: renamed[old],
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
    Ok(renamed)
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

/// One axis of `frame.set_grid`: what changes of the columns or the rows; an absent member stays.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GridAxis {
    /// How many, 1 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u16>,
    /// Counted with numbers or letters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counting: Option<Counting>,
    /// The index of the first label, 0 to 999 (the number itself, or the letter at it: 0 is A).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<u16>,
    /// Letters skip I and O.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_io: Option<bool>,
}

/// The largest count of columns or rows.
pub const MAX_AXIS: u16 = 100;

/// The largest index of a first label.
pub const MAX_FIRST: u16 = 999;

/// `frame.set_grid` (`SetFrameGrid`): the columns and rows of a frame (doc 09 §3.9, doc 06 §2.4): how many, counted
/// with numbers or letters, from which first label, letters skipping I and O; whether the rulers are drawn; and the
/// height of its title block. The paper and the drawing area's outer edges stay: the columns share the drawing area's
/// width and the rows its height, each an exact step; a taller title block takes its height from the rows (its cells'
/// heights scaled with it, its lines written anew). Bulk (previewed): what prints differently is listed before it is
/// applied. A frame or title block that is not the project's own is forked as `form.edit` forks it (`forks`, in
/// `forked_copies` order).
///
/// Rejected, with the pointer: `/frame` not a frame with a grid the project holds; a count outside 1 to 100; a first
/// label outside 0 to 999; a title-block height below 2 mm or one that leaves the rows no height; a title block that
/// grows over a connection point, a bend of a wire or a symbol's anchor of a page of the frame, naming the page;
/// nothing changed. Outcome: the frame (its fork for a fork).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetFrameGrid {
    /// The frame.
    #[schemars(with = "UuidS")]
    pub frame: Id<FormDef>,
    /// The columns, along the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<GridAxis>,
    /// The rows, along the left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<GridAxis>,
    /// Whether the rulers are drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawn: Option<bool>,
    /// The title block's height, nm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub title_block_height: Option<Nm>,
    /// The item IDs of the forks, for a frame or title block that is not the project's own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub forks: Vec<Id<LibItem>>,
}

/// The step of `new` cells over the length of `old` cells of `step` (`None` on overflow or a step under 1 nm).
fn shared_step(step: Grid, old: u16, new: u16) -> Option<Grid> {
    let num = step.num().checked_mul(u64::from(old))?;
    let den = step.den().checked_mul(u64::from(new))?;
    Grid::new(num, den).ok()
}

/// One axis changed as `a` says; refused at `at` for a count, a first label or a counting the frame cannot take.
/// The members of one axis of a frame grid that `frame.set_grid` changes.
struct AxisMut<'a> {
    count: &'a mut u16,
    step: &'a mut Grid,
    labels: &'a mut Vec<ew_mltext::Ident>,
    counting: &'a mut Counting,
    first: &'a mut u16,
    skip_io: &'a mut bool,
}

fn change_axis(at: &str, a: &GridAxis, axis: AxisMut<'_>) -> Result<(), CmdError> {
    let AxisMut {
        count,
        step,
        labels,
        counting,
        first,
        skip_io,
    } = axis;
    let name = SetFrameGrid::NAME;
    if a.count.is_some_and(|c| !(1..=MAX_AXIS).contains(&c)) {
        return Err(bad(
            name,
            format!("{at}/count"),
            text!("refusal-frame-grid-count"),
        ));
    }
    if a.first.is_some_and(|f| f > MAX_FIRST) {
        return Err(bad(
            name,
            format!("{at}/first"),
            text!("refusal-frame-grid-first"),
        ));
    }
    if matches!(a.counting, Some(Counting::Unknown(_))) {
        return Err(bad(
            name,
            format!("{at}/counting"),
            text!("refusal-frame-grid-counting"),
        ));
    }
    if let Some(n) = a.count.filter(|n| n != count) {
        *step = shared_step(*step, *count, n)
            .ok_or_else(|| bad(name, format!("{at}/count"), text!("refusal-frame-grid-too-fine")))?;
        *count = n;
        labels.clear();
    }
    if let Some(k) = &a.counting {
        *counting = k.clone();
    }
    if let Some(f) = a.first {
        *first = f;
    }
    if let Some(s) = a.skip_io {
        *skip_io = s;
    }
    Ok(())
}

impl Command for SetFrameGrid {
    const NAME: &'static str = "frame.set_grid";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let held = held_frame_form(tx, Self::NAME, self.frame)?;
        let mut frame = held.item.clone();
        if frame.kind != FormKind::Frame || frame.grid.is_none() {
            return Err(bad(Self::NAME, "/frame", text!("refusal-frame-grid-no-grid")));
        }
        {
            let grid = frame.grid.as_mut().expect("checked above");
            if let Some(a) = &self.columns {
                let c = &mut grid.columns;
                change_axis(
                    "/columns",
                    a,
                    AxisMut {
                        count: &mut c.count,
                        step: &mut c.width,
                        labels: &mut c.labels,
                        counting: &mut c.counting,
                        first: &mut c.start_index,
                        skip_io: &mut c.skip_io,
                    },
                )?;
            }
            if let Some(a) = &self.rows {
                let r = &mut grid.rows;
                change_axis(
                    "/rows",
                    a,
                    AxisMut {
                        count: &mut r.count,
                        step: &mut r.height,
                        labels: &mut r.labels,
                        counting: &mut r.counting,
                        first: &mut r.start_index,
                        skip_io: &mut r.skip_io,
                    },
                )?;
            }
            match self.drawn {
                Some(true) => {
                    grid.extra.remove(ew_schema::EXTRA_SHOW_GRID);
                }
                Some(false) => {
                    grid.extra
                        .insert(ew_schema::EXTRA_SHOW_GRID.into(), Value::Bool(false));
                }
                None => {}
            }
        }
        let mut edits: Vec<(Id<FormDef>, FormDef)> = Vec::new();
        // A taller or lower title block takes its height from the rows: one exact step per row over the rest.
        if let Some(h) = self.title_block_height {
            let Some(block_id) = frame.title_block else {
                return Err(bad(
                    Self::NAME,
                    "/title_block_height",
                    text!("refusal-frame-grid-no-block"),
                ));
            };
            let mut block = tx.require::<LibCopy<FormDef>>(block_id)?.item.clone();
            let old = block.sheet.height;
            if h != old {
                if h < MIN_CELL {
                    return Err(bad(
                        Self::NAME,
                        "/title_block_height",
                        text!("refusal-frame-grid-block-low"),
                    ));
                }
                let high = || {
                    bad(
                        Self::NAME,
                        "/title_block_height",
                        text!("refusal-frame-grid-block-high"),
                    )
                };
                let r = &mut frame.grid.as_mut().expect("checked above").rows;
                // (num·count − d·den) / (den·count): the rows' length less the growth `d`.
                let d = i128::from(h.0) - i128::from(old.0);
                let total = i128::from(r.height.num()) * i128::from(r.count) - d * i128::from(r.height.den());
                let den = r.height.den().checked_mul(u64::from(r.count)).ok_or_else(high)?;
                let total = u64::try_from(total).ok().filter(|t| *t > 0).ok_or_else(high)?;
                r.height = Grid::new(total, den).map_err(|_| high())?;
                let scale = |y: Nm| {
                    Nm(
                        i64::try_from(i128::from(y.0) * i128::from(h.0) / i128::from(old.0.max(1)))
                            .unwrap_or(0),
                    )
                };
                let scaled = |b: ew_schema::Bounds| {
                    let (min, max) = (b.0.min(), b.0.max());
                    ew_schema::Bounds(Rect::from_points(
                        Pt2::new(min.x, scale(min.y)),
                        Pt2::new(max.x, scale(max.y)),
                    ))
                };
                for c in &mut block.cells {
                    c.rect = scaled(c.rect);
                }
                for g in &mut block.graphics {
                    if let Primitive::Image(img) = g {
                        img.rect = scaled(img.rect);
                    }
                }
                block.sheet.height = h;
                lines_from_cells(&mut block);
                if d > 0 {
                    refuse_drawing_under(tx, self.frame, &frame)?;
                }
                edits.push((block_id, block));
            }
        }
        if frame == held.item && edits.is_empty() {
            return tx.reject(text!("refusal-frame-grid-unchanged"));
        }
        edits.insert(0, (self.frame, frame));
        let renamed = change_forms(tx, Self::NAME, edits, &self.forks)?;
        let id = renamed.get(&self.frame).copied().unwrap_or(self.frame);
        Ok(Outcome::primary((Kind::Form, id.cast())))
    }
}

/// Refuses a frame whose drawing area (`frame`'s new grid) leaves out a connection point, a bend of a wire or a
/// symbol's anchor of a page that prints the frame `id`, naming the first such page.
fn refuse_drawing_under(tx: &TxBuilder<'_>, id: Id<FormDef>, frame: &FormDef) -> Result<(), CmdError> {
    let Some(grid) = &frame.grid else {
        return Ok(());
    };
    let mut pages: Vec<_> = tx
        .iter::<Page>()
        .into_iter()
        .filter(|p| crate::numbering::frame_form(tx, p) == Some(id))
        .collect();
    pages.sort_by_key(|p| (p.folio.number, p.id));
    for page in pages {
        let outside = |p: &Pt2| ew_derive::cell_in_grid(grid, page.grid, *p).is_none();
        let mut points: Vec<Pt2> = Vec::new();
        for rep in tx
            .reps_on_page(page.id)
            .into_iter()
            .filter_map(|r| tx.get::<ew_schema::Representation>(r))
        {
            points.extend(crate::geometry::rep_cps(tx, &rep).iter().map(|c| c.pos));
            points.extend(
                tx.get::<LibCopy<ew_schema::SymbolDef>>(rep.symbol)
                    .and_then(|s| ew_derive::anchor_point_of(&s.item, &rep)),
            );
        }
        for c in tx.connections_on_page(page.id) {
            if let Some(c) = tx.get::<ew_schema::Connection>(c) {
                points.extend(c.route.iter().copied());
            }
        }
        if points.iter().any(outside) {
            return tx.reject(text!(
                "refusal-frame-grid-drawing-under",
                crate::words::page(tx, page.id)
            ));
        }
    }
    Ok(())
}

/// `form.make_own` (`MakeFormOwn`): a frame of a page's own, or a title block of a frame's own (doc 09 §3.9: Give this
/// page its own frame; Title block ▸ Its own), one undo step.
///
/// - `page`: a copy of the frame the page prints on, the project's own with item ID `forks[0]`, which the page's frame
///   (`Page.form`) names; the other pages keep theirs.
/// - `frame`: a copy of the frame's title block, the project's own with item ID `forks[0]`, which the frame names; a
///   frame that is not the project's own is forked first (`forks[1]`), as `form.edit` forks it. Every other frame keeps
///   the shared block.
///
/// Exactly one of `page` and `frame`. Rejected: neither or both (at the root); `/page` a page that prints on no frame
/// the project holds; `/frame` not a frame the project holds, or one without a title block; `/forks` too few or naming a
/// copy the project holds. Outcome: the new copy.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MakeFormOwn {
    /// The page that gets a frame of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub page: Option<Id<Page>>,
    /// The frame that gets a title block of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub frame: Option<Id<FormDef>>,
    /// The item IDs: the new copy's, then the frame's fork for a frame that is not the project's own.
    #[schemars(with = "Vec<UuidS>")]
    pub forks: Vec<Id<LibItem>>,
}

/// A copy of `original` as the project's own under the item ID `item`, saying what it is a copy of.
fn own_copy(
    tx: &TxBuilder<'_>,
    command: &str,
    original: &LibCopy<FormDef>,
    item: Option<&Id<LibItem>>,
) -> Result<LibCopy<FormDef>, CmdError> {
    let project = tx.project().id.as_uuid();
    let item = *item.ok_or_else(|| bad(command, "/forks", text!("refusal-form-edit-needs-forks")))?;
    let id: Id<FormDef> = v5::lib_copy_id(project, item.as_uuid());
    if tx.exists::<LibCopy<FormDef>>(id) {
        return Err(bad(command, "/forks", text!("refusal-form-edit-fork-held")));
    }
    let mut form = original.item.clone();
    form.meta.extra.insert(
        "source".into(),
        json!({"fork_of": {
            "library": original.library_id.as_uuid().to_string(),
            "item": original.item_id.as_uuid().to_string(),
            "version": original.version,
        }}),
    );
    let mut copy = LibCopy {
        id,
        v: original.v,
        library_id: Id::from_uuid(project),
        item_id: item,
        version: original.version.clone(),
        sha256: original.sha256,
        item: form,
    };
    copy.sha256 = hash(&copy)?;
    Ok(copy)
}

impl Command for MakeFormOwn {
    const NAME: &'static str = "form.make_own";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        match (self.page, self.frame) {
            (Some(page), None) => {
                let record = tx.require::<Page>(page)?;
                let Some(frame) = crate::numbering::frame_form(tx, &record) else {
                    return Err(bad(Self::NAME, "/page", text!("refusal-form-own-no-frame")));
                };
                let original = tx.require::<LibCopy<FormDef>>(frame)?;
                let copy = own_copy(tx, Self::NAME, &original, self.forks.first())?;
                let id = copy.id;
                tx.insert(copy)?;
                tx.update(page, |p: &mut Page| p.form = Some(id))?;
                Ok(Outcome::primary((Kind::Form, id.cast())))
            }
            (None, Some(frame)) => {
                let held = held_frame_form(tx, Self::NAME, frame)?;
                let Some(block) = held
                    .item
                    .title_block
                    .filter(|_| held.item.kind == FormKind::Frame)
                else {
                    return Err(bad(Self::NAME, "/frame", text!("refusal-form-own-no-block")));
                };
                let original = tx.require::<LibCopy<FormDef>>(block)?;
                let copy = own_copy(tx, Self::NAME, &original, self.forks.first())?;
                let id = copy.id;
                tx.insert(copy)?;
                let mut item = held.item.clone();
                item.title_block = Some(id);
                change_form(
                    tx,
                    Self::NAME,
                    frame,
                    item,
                    self.forks.get(1..).unwrap_or_default(),
                )?;
                Ok(Outcome::primary((Kind::Form, id.cast())))
            }
            _ => Err(bad(Self::NAME, "", text!("refusal-form-own-one"))),
        }
    }
}
