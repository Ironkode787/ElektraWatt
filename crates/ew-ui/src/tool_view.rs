// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window paints of the tool framework besides the shared marks (WP-S2.03 §7.4, §7.3, §7.8): the tool
//! overlay's marks in the renderer's form, its text labels as chips in the chrome font, the semantic ghost of a
//! kept preview as chips (the derived diff carries no positions, so the chips stand in the canvas's top-left corner,
//! with the `stale` badge while the preview is stale) and the frame of the link preview popup.
//!
//! Labels and chips are never composited into `canvas.snapshot`; `ui.describe` lists them.

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Ui, Vec2};
use ew_render2d::overlay::{Dash, MarkerKind, ToolMark, ToolToken};
use ew_render2d::view::DrawTransform;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::{self, Anchor, Overlay, OverlayItem, PreviewStatus};

/// The renderer's token of a tool token.
fn token(t: tools::Token) -> ToolToken {
    match t {
        tools::Token::Ghost => ToolToken::Ghost,
        tools::Token::Snap => ToolToken::Snap,
        tools::Token::Measure => ToolToken::Measure,
        tools::Token::RubberBand => ToolToken::RubberBand,
    }
}

fn dash(d: tools::Dash) -> Dash {
    match d {
        tools::Dash::Solid => Dash::Solid,
        tools::Dash::Dashed => Dash::Dashed,
    }
}

/// The overlay's marks for `ew-render2d` (labels left out: the window draws them as chips); `snapper` is the page's,
/// which maps a floating outline (`Paths`, page nm) to the sheet (WP-S2.04 §6.3).
pub fn marks(overlay: &Overlay, snapper: &tools::Snapper) -> Vec<ToolMark> {
    overlay
        .items
        .iter()
        .filter_map(|item| {
            Some(match item {
                OverlayItem::Line { a, b, style } => ToolMark::Line {
                    a: *a,
                    b: *b,
                    dash: dash(style.dash),
                    token: token(style.token),
                },
                OverlayItem::Polyline { points, style } => ToolMark::Polyline {
                    points: points.clone(),
                    dash: dash(style.dash),
                    token: token(style.token),
                },
                OverlayItem::Rect { r, style } => ToolMark::Rect {
                    r: *r,
                    dash: dash(style.dash),
                    token: token(style.token),
                },
                OverlayItem::Ghost { rects, offset } => ToolMark::Ghost {
                    rects: rects.clone(),
                    offset: *offset,
                },
                OverlayItem::Marker { at, kind } => ToolMark::Marker {
                    at: *at,
                    kind: match kind {
                        tools::MarkerKind::Grid => MarkerKind::Grid,
                        tools::MarkerKind::Cp => MarkerKind::Cp,
                        tools::MarkerKind::Junction => MarkerKind::Junction,
                    },
                },
                OverlayItem::Paths { paths, transform } => ToolMark::Paths {
                    paths: tools::paths_on_sheet(paths, *transform, snapper),
                },
                OverlayItem::Label { .. } => return None,
            })
        })
        .collect()
}

/// The marks the canvas of `group` shows: the active tool's overlay on the focused group only.
pub fn marks_of(shell: &ShellModel, group: usize) -> Vec<ToolMark> {
    if shell.area().map(|a| a.focused) == Some(group)
        && let Some(c) = shell.canvas_of(group)
    {
        marks(shell.tool_overlay(), &c.page().snapper)
    } else {
        Vec::new()
    }
}

/// The tool strip's text (WP-S2.04 §6.4 item 4): the active tool's label and what it floats or drags, as the
/// transcript's `ghost` member says it; `None` for Select at rest.
pub fn tool_strip_text(shell: &ShellModel) -> Option<String> {
    let tool = shell.tool();
    let label = tools::tool_label(tool.name());
    let detail = match shell.tool_ghost() {
        None if tool.name() == "select" => return None,
        None => strings::text(tool.status_hint()).to_owned(),
        Some(tools::Ghost::Place {
            symbol,
            variant,
            target,
            letter,
            strip,
            tag,
            ..
        }) => {
            let mut parts = vec![symbol, variant.to_string(), target.to_owned()];
            parts.extend(letter);
            parts.extend(strip);
            parts.extend(tag);
            parts.join(" · ")
        }
        Some(tools::Ghost::Move { readout, .. }) => readout,
        Some(tools::Ghost::Exchange { symbol, unmapped }) => {
            let mut parts = vec![symbol];
            parts.extend(unmapped);
            parts.join(" · ")
        }
    };
    Some(format!("{label}: {detail}"))
}

/// A text chip: the text on the panel colour with a thin frame, its anchor point at `at` (points).
fn chip(ui: &Ui, clip: Rect, at: Pos2, align: Align2, text: &str) {
    let painter = ui.painter_at(clip);
    let visuals = ui.visuals();
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(12.0), visuals.text_color());
    let rect = align.anchor_size(at, galley.size());
    painter.rect_filled(rect.expand(3.0), 2.0, visuals.extreme_bg_color);
    painter.rect_stroke(
        rect.expand(3.0),
        2.0,
        Stroke::new(1.0, visuals.weak_text_color()),
        egui::StrokeKind::Outside,
    );
    painter.galley(rect.min, galley, visuals.text_color());
}

/// Paints the overlay's labels of the focused canvas: each a chip at its point (`transform` maps sheet nm to the
/// pane's physical pixels, `ppp` pixels per point, `origin` the pane's top-left corner in points).
pub fn paint_labels(
    ui: &Ui,
    rect: Rect,
    origin: Pos2,
    ppp: f32,
    transform: &DrawTransform,
    overlay: &Overlay,
) {
    for item in &overlay.items {
        let OverlayItem::Label { at, text, anchor } = item else {
            continue;
        };
        let (x, y) = transform.to_screen((at.0 as f64, at.1 as f64));
        let p = origin + Vec2::new(x as f32, y as f32) / ppp;
        let (pos, align) = match anchor {
            Anchor::Above => (p - Vec2::new(0.0, 8.0), Align2::CENTER_BOTTOM),
            Anchor::Below => (p + Vec2::new(0.0, 8.0), Align2::CENTER_TOP),
            Anchor::Left => (p - Vec2::new(8.0, 0.0), Align2::RIGHT_CENTER),
            Anchor::Right => (p + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER),
        };
        chip(ui, rect, pos, align, text);
    }
}

/// Paints the semantic ghost of the kept preview (§7.3): its command and status, then one chip per changed tag
/// and cross-reference text (`before → after`), in the canvas's top-left corner.
pub fn paint_preview(ui: &Ui, rect: Rect, shell: &ShellModel) {
    let (Some(p), Some(status)) = (shell.preview_state(), shell.preview_status()) else {
        return;
    };
    let mut lines = vec![match (&p.result, status) {
        (Err(message), _) => strings::format("preview-chip-failed", &[&p.name, message]),
        (_, PreviewStatus::Stale) => strings::format("preview-chip-stale", &[&p.name]),
        _ => strings::format("preview-chip", &[&p.name]),
    }];
    for c in &p.derived.tags {
        lines.push(strings::format("preview-chip-change", &[&c.before, &c.after]));
    }
    for c in &p.derived.xrefs {
        lines.push(strings::format("preview-chip-change", &[&c.before, &c.after]));
    }
    let mut at = rect.min + Vec2::new(12.0, 12.0);
    for line in lines.iter().take(12) {
        chip(ui, rect, at, Align2::LEFT_TOP, line);
        at.y += 22.0;
    }
}

/// The frame of the link preview popup (§7.8): a shadowed border around `rect` and the target's folio above it.
pub fn paint_popup_frame(ui: &Ui, clip: Rect, rect: Rect, folio: &str) {
    let painter = ui.painter_at(clip);
    painter.rect_filled(
        rect.translate(Vec2::new(3.0, 3.0)),
        2.0,
        Color32::from_black_alpha(60),
    );
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, ui.visuals().strong_text_color()),
        egui::StrokeKind::Outside,
    );
    chip(
        ui,
        clip,
        rect.left_top() - Vec2::new(0.0, 4.0),
        Align2::LEFT_BOTTOM,
        &strings::format("link-preview-title", &[folio]),
    );
}
