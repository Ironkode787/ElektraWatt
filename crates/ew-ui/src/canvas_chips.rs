// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The chips of a canvas (WP-S2.17 part 5; doc 16a §7.3 to §7.5): the tool overlay's labels (the Place ghost's one
//! label with the tag it will get, FT-507; the measurement readout), the highlights' name chips and the kept
//! preview's semantic ghost (its header with the command and status, one chip per changed tag or cross-reference
//! text, FT-389), each placed by `ew-viewmodel`'s one rule, [`place_chip`] (D13), in that order, so that a later chip
//! keeps off the earlier ones.
//!
//! **Obstacles** (FT-508): the outside of the canvas, the frame-statistics panel, the chrome the caller reports
//! over the canvas (the tool HUD and the zoom control, [`crate::canvas::CanvasHost::set_chrome_obstacles`]), the
//! pointer's 16-point box, and every chip placed before. **Targets**: a label stands by its point; a highlight's
//! name chip by the first vertex of its first drawn wire; a tag change by the printed tag text of the device's first
//! representation on the page (`TableRep::tag_at`, FT-502), else the top-left corner of its body, else the Place
//! tool's floating insertion point; a cross-reference change by its representation's body; the header right above
//! the first placed change (its target is a point 8 points left of that chip's top-left corner, so the rule's
//! preferred anchor is straight above it); changes the page does not draw, and the header without a placed change,
//! stack from the canvas's top-left corner.
//!
//! **Look** (doc 16a §7.4, §4.6): a canvas chip is `canvas.chip` (the inverse surface at 94 %) with
//! `canvas.chip_text` in `type.small`, `radius.sm`, `space.2` by `space.1` padding and `elevation.1`; a highlight
//! name chip is filled with its slot's hue with a black `type.caption` SemiBold label, `radius.sm`, 20 points high
//! (the mockup). A chip that found no free place is drawn at 80 % opacity.
//!
//! [`plan`] is the layout (deterministic for equal input and fonts), [`paint`] draws it; the window keeps the plan of
//! the last frame for tests. Chips are never composited into `canvas.snapshot`; `ui.describe` lists the labels and
//! the preview.

use egui::{Pos2, Rect, Ui, Vec2};
use ew_render2d::view::DrawTransform;
use ew_viewmodel::canvas::CanvasModel;
use ew_viewmodel::canvas::chip::{self as vm, FADED_OPACITY, place_chip};
use ew_viewmodel::highlight::{MAX_HIGHLIGHTS, SLOT_COLOURS};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::{self, OverlayItem, PreviewStatus};

use crate::theme::{Rgba, Tokens, TypeToken};

/// What a chip shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChipKind {
    /// A tool overlay label (the Place ghost's label, a measurement readout).
    Label,
    /// A highlight's name chip, with its slot.
    Highlight(u8),
    /// The kept preview's header (command and status).
    PreviewHeader,
    /// One change of the kept preview (`before → after`).
    PreviewChange,
}

/// One chip of a canvas as [`plan`] places it.
#[derive(Clone, PartialEq, Debug)]
pub struct CanvasChip {
    /// What it shows.
    pub kind: ChipKind,
    /// Its text.
    pub text: String,
    /// Its frame in points (the fill's rectangle).
    pub rect: Rect,
    /// No candidate place was free: drawn at the preferred anchor at 80 % opacity.
    pub faded: bool,
    /// The fill (before the fade).
    pub fill: Rgba,
    /// The text colour (before the fade).
    pub text_colour: Rgba,
    /// The corner radius.
    pub radius: f32,
}

/// The height of a highlight name chip (doc 16a §4.6, the mockup's 20 px).
pub const HIGHLIGHT_CHIP_HEIGHT: f32 = 20.0;

/// The chip rule's offset (8 points) in egui's type.
const OFFSET: f32 = vm::CHIP_OFFSET;

/// What a canvas's chips are placed from.
pub struct ChipInput<'a> {
    /// The canvas pane (points); its top-left corner is the pane's origin.
    pub rect: Rect,
    /// Physical pixels per point.
    pub ppp: f32,
    /// Sheet nm to the pane's physical pixels.
    pub transform: &'a DrawTransform,
    /// The group of the canvas.
    pub group: usize,
    /// Whether the group has the focus: only the focused canvas shows the tool's labels and the preview.
    pub focused: bool,
    /// The statistics panel and the chrome over the canvas (points).
    pub obstacles: &'a [Rect],
    /// The pointer, when it is over the canvas.
    pub pointer: Option<Pos2>,
}

fn to_vm(r: Rect) -> vm::Rect {
    vm::Rect::from_min_max(vm::Pos2::new(r.min.x, r.min.y), vm::Pos2::new(r.max.x, r.max.y))
}

fn from_vm(r: vm::Rect) -> Rect {
    Rect::from_min_max(Pos2::new(r.min.x, r.min.y), Pos2::new(r.max.x, r.max.y))
}

fn point(p: Pos2) -> vm::Rect {
    vm::Rect::point(vm::Pos2::new(p.x, p.y))
}

/// The font of a highlight name chip: `type.caption` SemiBold.
fn caption_semibold(t: &Tokens) -> TypeToken {
    TypeToken {
        semibold: true,
        ..t.type_caption
    }
}

/// The places taken so far and the pointer.
struct Placer<'u> {
    ui: &'u Ui,
    tokens: &'u Tokens,
    obstacles: Vec<vm::Rect>,
    pointer: Option<vm::Pos2>,
    out: Vec<CanvasChip>,
}

impl Placer<'_> {
    /// The size of a chip of `kind` showing `text`.
    fn size(&self, kind: ChipKind, text: &str) -> Vec2 {
        let t = self.tokens;
        let (font, pad) = match kind {
            ChipKind::Highlight(_) => (caption_semibold(t), Vec2::new(t.space_3, 0.0)),
            _ => (t.type_small, Vec2::new(t.space_2, t.space_1)),
        };
        let galley = self.ui.painter().layout_no_wrap(
            text.to_owned(),
            crate::theme::font(&font),
            crate::theme::NO_TINT,
        );
        let s = galley.size() + 2.0 * pad;
        match kind {
            ChipKind::Highlight(_) => Vec2::new(s.x, HIGHLIGHT_CHIP_HEIGHT),
            _ => s,
        }
    }

    /// Places a chip of `kind` showing `text` by `target` and returns its frame.
    fn put(&mut self, kind: ChipKind, text: String, target: vm::Rect) -> Rect {
        let t = self.tokens;
        let size = self.size(kind, &text);
        let p = place_chip(
            target,
            vm::Vec2::new(size.x, size.y),
            &self.obstacles,
            self.pointer,
        );
        self.obstacles.push(p.rect);
        let (fill, text_colour) = match kind {
            ChipKind::Highlight(slot) => {
                let (r, g, b) = SLOT_COLOURS[usize::from(slot) % MAX_HIGHLIGHTS];
                (Rgba { r, g, b, a: u8::MAX }, t.canvas_highlight_label)
            }
            _ => (t.canvas_chip, t.canvas_chip_text),
        };
        let rect = from_vm(p.rect);
        self.out.push(CanvasChip {
            kind,
            text,
            rect,
            faded: p.faded,
            fill,
            text_colour,
            radius: t.radius_sm,
        });
        rect
    }
}

/// The chips of the canvas of `input.group` (see the module documentation), in paint order.
pub fn plan(ui: &Ui, input: &ChipInput<'_>, canvas: &CanvasModel, shell: &ShellModel) -> Vec<CanvasChip> {
    let tokens = crate::theme::current(ui.ctx());
    let mut obstacles: Vec<vm::Rect> = vm::outside(to_vm(input.rect)).to_vec();
    obstacles.extend(input.obstacles.iter().copied().map(to_vm));
    let mut placer = Placer {
        ui,
        tokens: &tokens,
        obstacles,
        pointer: input.pointer.map(|p| vm::Pos2::new(p.x, p.y)),
        out: Vec::new(),
    };
    let screen = |at: (i64, i64)| {
        let (x, y) = input.transform.to_screen((at.0 as f64, at.1 as f64));
        input.rect.min + Vec2::new(x as f32, y as f32) / input.ppp
    };
    if input.focused {
        for item in &shell.tool_overlay().items {
            if let OverlayItem::Label { at, text, .. } = item {
                placer.put(ChipKind::Label, text.clone(), point(screen(*at)));
            }
        }
    }
    for (item, _) in shell.highlight_overlay(input.group) {
        let OverlayItem::Highlight { polylines, slot } = item else {
            continue;
        };
        let Some(first) = polylines.first().and_then(|l| l.first()) else {
            continue;
        };
        let name = shell
            .highlights()
            .items()
            .iter()
            .find(|h| h.slot == slot)
            .map(|h| h.name.clone())
            .unwrap_or_default();
        placer.put(ChipKind::Highlight(slot), name, point(screen(*first)));
    }
    if input.focused {
        preview(&mut placer, input, canvas, shell, &screen);
    }
    placer.out
}

/// The kept preview's chips (FT-389 by the rule of D13).
fn preview(
    placer: &mut Placer<'_>,
    input: &ChipInput<'_>,
    canvas: &CanvasModel,
    shell: &ShellModel,
    screen: &dyn Fn((i64, i64)) -> Pos2,
) {
    let (Some(p), Some(status)) = (shell.preview_state(), shell.preview_status()) else {
        return;
    };
    // H40 item 6: a refusal the HUD line and the toast already say gets no "Preview of … failed" chip.
    if p.result.is_err() && shell.preview_refusal_in_hud() {
        return;
    }
    // A ready preview that changes no tag and no cross-reference text says nothing a chip should stand for (a route
    // edit, a move): the tool's readout and the HUD carry it (WP-S2.04 part 2, window check).
    if status == PreviewStatus::Ready
        && p.result.is_ok()
        && p.derived.tags.is_empty()
        && p.derived.xrefs.is_empty()
    {
        return;
    }
    let title = strings::command_title(&p.name);
    let header = match (&p.result, status) {
        (Err(message), _) => strings::format("preview-chip-failed", &[title, message]),
        (_, PreviewStatus::Stale) => strings::format("preview-chip-stale", &[title]),
        _ => strings::format("preview-chip", &[title]),
    };
    let page = canvas.page();
    let tables = &page.tables;
    let corner_of = |body: [i64; 4]| {
        screen(
            page.snapper
                .page_to_sheet((body[0].min(body[2]), body[1].min(body[3]))),
        )
    };
    // A device the page does not draw yet (the Place tool's new device) stands by the floating insertion point.
    let floating = match shell.tool_ghost() {
        Some(tools::Ghost::Place { at_nm: Some(at), .. }) => Some([at.0, at.1, at.0, at.1]),
        _ => None,
    };
    let mut texts: Vec<(String, Option<Pos2>)> = Vec::new();
    for c in &p.derived.tags {
        let id = c.key.as_uuid();
        let at = tables
            .reps
            .iter()
            .find(|r| r.device == Some(id))
            // FT-502: by the printed tag text where the page tables give it, else by the body.
            .map(|r| r.tag_at.map_or(r.body, |t| [t.0, t.1, t.0, t.1]))
            .or(floating)
            .map(corner_of);
        texts.push((strings::format("preview-chip-change", &[&c.before, &c.after]), at));
    }
    for c in &p.derived.xrefs {
        let id = c.key.as_uuid();
        let at = tables.reps.iter().find(|r| r.id == id).map(|r| corner_of(r.body));
        texts.push((strings::format("preview-chip-change", &[&c.before, &c.after]), at));
    }
    texts.truncate(11);
    let start = placer.out.len();
    // The placed changes first, by their targets.
    let mut frames: Vec<Option<Rect>> = vec![None; texts.len()];
    for (k, (text, at)) in texts.iter().enumerate() {
        if let Some(at) = at {
            frames[k] = Some(placer.put(ChipKind::PreviewChange, text.clone(), point(*at)));
        }
    }
    // The header straight above the first placed change, else in the corner; the unplaced changes below it.
    let t = placer.tokens;
    let corner = point(input.rect.min + Vec2::splat(t.space_4 - OFFSET));
    let head_target = frames
        .iter()
        .flatten()
        .next()
        .map_or(corner, |first| point(first.left_top() - Vec2::new(OFFSET, 0.0)));
    let head = placer.put(ChipKind::PreviewHeader, header, head_target);
    let mut chain = if frames.iter().flatten().next().is_some() {
        corner
    } else {
        point(head.left_bottom() - Vec2::new(OFFSET, 0.0))
    };
    for (k, (text, at)) in texts.iter().enumerate() {
        if at.is_none() {
            let r = placer.put(ChipKind::PreviewChange, text.clone(), chain);
            chain = point(r.left_bottom() - Vec2::new(OFFSET, 0.0));
            frames[k] = Some(r);
        }
    }
    // Paint and report order: the header, then the changes in the preview's order.
    let mut chips: Vec<CanvasChip> = placer.out.drain(start..).collect();
    let head_index = chips
        .iter()
        .position(|c| c.kind == ChipKind::PreviewHeader)
        .unwrap_or(0);
    let head_chip = chips.remove(head_index);
    let mut ordered = vec![head_chip];
    for frame in frames.into_iter().flatten() {
        if let Some(i) = chips.iter().position(|c| c.rect == frame) {
            ordered.push(chips.remove(i));
        }
    }
    ordered.extend(chips);
    placer.out.extend(ordered);
}

/// The colour `c` at the chip's opacity.
fn faded(c: Rgba, faded: bool) -> egui::Color32 {
    let c = c.color32();
    if faded { c.gamma_multiply(FADED_OPACITY) } else { c }
}

/// Paints `chips` in `clip`.
pub fn paint(ui: &Ui, clip: Rect, chips: &[CanvasChip]) {
    let t = crate::theme::current(ui.ctx());
    let painter = ui.painter_at(clip);
    for chip in chips {
        let font = match chip.kind {
            ChipKind::Highlight(_) => caption_semibold(&t),
            _ => t.type_small,
        };
        if !matches!(chip.kind, ChipKind::Highlight(_)) {
            let shadow = crate::theme::shadow(&t.elevation_1);
            painter.add(shadow.as_shape(chip.rect, crate::theme::radius(chip.radius)));
        }
        painter.rect_filled(
            chip.rect,
            crate::theme::radius(chip.radius),
            faded(chip.fill, chip.faded),
        );
        let colour = faded(chip.text_colour, chip.faded);
        let galley = painter.layout_no_wrap(chip.text.clone(), crate::theme::font(&font), colour);
        let at = chip.rect.center() - galley.size() / 2.0;
        painter.galley(at, galley, colour);
    }
}

/// The frame of the link preview popup (WP-S2.03 §7.8): a popover (doc 16a §6.13's surface: `elevation.1`, a 1-px
/// `color.border.default` outline) around `rect` and the target's folio as a canvas chip above it.
pub fn paint_popup_frame(ui: &Ui, clip: Rect, rect: Rect, folio: &str) {
    let t = crate::theme::current(ui.ctx());
    let painter = ui.painter_at(clip);
    painter.add(crate::theme::shadow(&t.elevation_1).as_shape(rect, crate::theme::SQUARE));
    painter.rect_stroke(
        rect,
        crate::theme::SQUARE,
        crate::theme::stroke(t.border_width, t.border_default),
        egui::StrokeKind::Outside,
    );
    let text = strings::format("link-preview-title", &[folio]);
    let galley = painter.layout_no_wrap(
        text,
        crate::theme::font(&t.type_small),
        t.canvas_chip_text.color32(),
    );
    let pad = Vec2::new(t.space_2, t.space_1);
    let frame = Rect::from_min_size(
        rect.left_top() - Vec2::new(0.0, t.space_2 + galley.size().y + 2.0 * pad.y),
        galley.size() + 2.0 * pad,
    );
    painter.rect_filled(frame, crate::theme::radius(t.radius_sm), t.canvas_chip.color32());
    painter.galley(frame.min + pad, galley, t.canvas_chip_text.color32());
}
