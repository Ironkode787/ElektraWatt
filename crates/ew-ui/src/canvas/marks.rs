// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The marks a canvas paints over the page (the hover and the lines' bands, the conflicts, the locks, the highlights,
//! the wire colours and codes, the arrange moves), the canvas tokens, and the offscreen pictures of a canvas and of
//! the preview.

use super::*;

/// The bands of the hovered line and the selected lines, half as wide as the reach a line answers to the pointer
/// within ([`HOVER_SLOP_PT`]) on each side.
pub(super) fn bands<'a>(
    hover: Option<&'a [(i64, i64)]>,
    selected: &'a [Vec<(i64, i64)>],
) -> overlay::Bands<'a> {
    overlay::Bands {
        hover,
        selected,
        radius_pt: ew_viewmodel::pointer::HOVER_SLOP_PT,
    }
}

/// The overlay colours of `t` (doc 16a §4.6): the canvas tokens; the page bounds in `color.border.default`, the
/// keyboard grid cursor and the rubber band in the accent of the sheet (`canvas.selection`).
pub fn canvas_theme(t: &crate::theme::Tokens) -> Theme {
    let c = |r: crate::theme::Rgba| Color {
        r: r.r,
        g: r.g,
        b: r.b,
        a: r.a,
    };
    Theme {
        surround: c(t.canvas_surround),
        border: c(t.border_default),
        grid_dot: c(t.canvas_grid_dot),
        hover: c(t.canvas_hover),
        selection: c(t.canvas_selection),
        cursor: c(t.canvas_selection),
        paper: c(t.canvas_sheet),
        ghost: c(t.canvas_ghost),
        snap: c(t.canvas_snap),
        measure: c(t.canvas_measure),
        rubber_band: c(t.canvas_selection),
        casing: c(t.canvas_sheet),
        open_cp: c(t.canvas_open_cp),
    }
}

/// The page grid of a canvas as the overlay draws it, when the grid is on.
pub(super) fn grid_spec(shell: &ShellModel, c: &CanvasModel) -> Option<GridSpec> {
    c.page().grid.filter(|_| shell.grid_on()).map(|g| GridSpec {
        num_nm: g.num_nm,
        den: g.den,
        sheet_scale_ppb: g.sheet_scale_ppb,
        sheet_offset_nm: g.sheet_offset_nm,
    })
}

/// One step of the page's connection grid on the sheet (nm), whether the grid is shown or not: the gate of the open
/// markers of the free connection points.
pub(super) fn cp_pitch(c: &CanvasModel) -> Option<f64> {
    c.page().grid.and_then(|g| {
        GridSpec {
            num_nm: g.num_nm,
            den: g.den,
            sheet_scale_ppb: g.sheet_scale_ppb,
            sheet_offset_nm: g.sheet_offset_nm,
        }
        .sheet_pitch_nm()
    })
}

/// `canvas.snapshot` (section 6.6) of group `group`: the pane as the canvas shows it once settled, at
/// `pane × ppp` pixels, and its resolution `round(D)`: `render_region` at that resolution with the view origin on a
/// whole pixel, white paper and the surround of the settings' theme (light for `system`), then the overlays (the
/// grid cursor and the link hover outline included) composited by integer fills. It never uses tiles
/// or a GPU, so equal inputs give equal bytes on every run of one build target. A pane wider or taller than 4096
/// pixels is rendered in regions of at most 4096 pixels and stitched.
///
/// # Errors
///
/// The text of the renderer's error, or of a missing canvas.
pub fn snapshot_image(shell: &ShellModel, group: usize) -> Result<(ew_render_cpu::RgbaImage, u32), String> {
    let Some(canvas) = shell.canvas_of(group) else {
        return Err(strings::text(key!("reason-no-page")).to_owned());
    };
    let selected = shell.selected_rects(group);
    let tool = crate::tool_view::marks_of(shell, group);
    let open = shell.open_cps(group);
    let hover_line = ew_viewmodel::canvas::hover_path(shell, canvas);
    let selected_lines = shell.selected_paths(group);
    let (mut image, dpi) = render_view(
        canvas,
        &theme_of(shell),
        grid_spec(shell, canvas),
        ew_viewmodel::canvas::hover_rect(shell, group, canvas),
        &selected,
        canvas.cursor(),
        &tool,
        OpenCps {
            points: &open,
            pitch_nm: cp_pitch(canvas),
        },
        bands(hover_line.as_deref(), &selected_lines),
    )?;
    let view = view_of(canvas);
    // The wire-colour view, under the conflicts and the highlights as in the window.
    composite_wire_colours(
        &mut image,
        &view.settled_transform(),
        view.ppp,
        &shell.wire_colour_overlay(group),
    );
    // The conflict overlay under the highlights.
    composite_conflicts(
        &mut image,
        &view.settled_transform(),
        view.ppp,
        &shell.conflict_overlay(group),
        conflict_colour(shell),
    );
    // The highlights, composited like the ghost (doc 16a §7.5).
    let items: Vec<OverlayItem> = shell
        .highlight_overlay(group)
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    composite_highlights(&mut image, &view.settled_transform(), view.ppp, &items);
    Ok((image, dpi))
}

/// Composites the wire-colour view into a snapshot by the window's widths: per stroke its casing (the stroke's
/// half width plus `CASING_PX`) in the casing colour, then its pieces in their colours, each pixel whose centre lies
/// within the half width of a piece. The codes are chips, which a snapshot leaves out.
fn composite_wire_colours(
    image: &mut ew_render_cpu::RgbaImage,
    t: &DrawTransform,
    ppp: f64,
    strokes: &[ew_viewmodel::wire_colours::WireStroke],
) {
    use ew_viewmodel::wire_colours::{CASING_PX, CASING_RGB, STROKE_PT};
    let half = STROKE_PT * ppp / 2.0;
    let band =
        |image: &mut ew_render_cpu::RgbaImage, a: (f64, f64), b: (f64, f64), half: f64, rgb: (u8, u8, u8)| {
            let c = Color {
                r: rgb.0,
                g: rgb.1,
                b: rgb.2,
                a: u8::MAX,
            };
            for (x, y) in band_pixels((image.width, image.height), t.to_screen(a), t.to_screen(b), half) {
                overlay::fill(image, PxRect::new(x, y, x + 1, y + 1), c);
            }
        };
    for s in strokes {
        // A laned stroke is drawn on its path moved into its lanes, its corners joined.
        let s = s.in_lanes(ppp / t.px_per_nm());
        if s.cased {
            for (a, b, _, _) in s.segments() {
                band(image, a, b, half + CASING_PX, CASING_RGB);
            }
        }
        for (a, b, c, _) in s.pieces() {
            band(image, a, b, half, s.piece_rgb(c));
        }
    }
}

/// The pen width of the conflict overlay in logical points: the selection outline's 2 points, so it covers the black
/// wire it is drawn over.
pub const CONFLICT_PT: f64 = 2.0;

/// The conflict overlay's colour: `canvas.conflict` of the settings' theme (the light value of `color.error` in every
/// theme: the sheet is white).
fn conflict_colour(shell: &ShellModel) -> crate::theme::Rgba {
    let settings = shell.settings();
    let (light, _, _) = crate::theme::themes_of_setting(settings.theme.as_str());
    crate::theme::tokens(
        light,
        crate::theme::Density::of_setting(settings.density.as_str()),
    )
    .canvas_conflict
}

/// Composites the conflict overlay into a snapshot: every pixel whose centre lies within half of
/// [`CONFLICT_PT`] (in pixels) of a segment of `lines` (sheet coordinates) is painted opaque in `colour`, once.
fn composite_conflicts(
    image: &mut ew_render_cpu::RgbaImage,
    t: &DrawTransform,
    ppp: f64,
    lines: &[Vec<(i64, i64)>],
    colour: crate::theme::Rgba,
) {
    let half = CONFLICT_PT * ppp / 2.0;
    let mut paint: std::collections::BTreeSet<(i64, i64)> = std::collections::BTreeSet::new();
    for line in lines {
        let pts: Vec<(f64, f64)> = line
            .iter()
            .map(|p| t.to_screen((p.0 as f64, p.1 as f64)))
            .collect();
        for s in pts.windows(2) {
            paint.extend(band_pixels((image.width, image.height), s[0], s[1], half));
        }
    }
    let c = Color {
        r: colour.r,
        g: colour.g,
        b: colour.b,
        a: u8::MAX,
    };
    for (x, y) in paint {
        overlay::fill(image, PxRect::new(x, y, x + 1, y + 1), c);
    }
}

/// The pixels of an image of `(width, height)` whose centres lie within `half` of the segment `a`–`b` (pixels of the
/// pane), row by row: the band the snapshot paints a line of the wire-colour view or the conflict overlay in.
fn band_pixels(
    (width, height): (u32, u32),
    a: (f64, f64),
    b: (f64, f64),
    half: f64,
) -> impl Iterator<Item = (i64, i64)> {
    let (w, h) = (i64::from(width), i64::from(height));
    let x0 = ((a.0.min(b.0) - half).floor() as i64).max(0);
    let x1 = ((a.0.max(b.0) + half).ceil() as i64).min(w);
    let y0 = ((a.1.min(b.1) - half).floor() as i64).max(0);
    let y1 = ((a.1.max(b.1) + half).ceil() as i64).min(h);
    (y0..y1)
        .flat_map(move |y| (x0..x1).map(move |x| (x, y)))
        .filter(move |&(x, y)| segment_distance((x as f64 + 0.5, y as f64 + 0.5), a, b) <= half)
}

/// The distance of `p` from the segment `a`–`b`.
fn segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let k = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a.0 + k * dx, a.1 + k * dy);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// Paints the conflict overlay of canvas group `group` in the window: each wire of a potential whose
/// EW-POT-001 finding stands as a [`CONFLICT_PT`] line in `canvas.conflict` over the page, under the highlights.
pub(super) fn paint_conflicts(
    ui: &egui::Ui,
    rect: egui::Rect,
    ppp: f32,
    t: &DrawTransform,
    shell: &ShellModel,
    group: usize,
) {
    let lines = shell.conflict_overlay(group);
    if lines.is_empty() {
        return;
    }
    let painter = ui.painter_at(rect);
    let stroke = egui::Stroke::new(CONFLICT_PT as f32, conflict_colour(shell).color32());
    for line in &lines {
        let pts: Vec<egui::Pos2> = line
            .iter()
            .map(|p| {
                let s = t.to_screen((p.0 as f64, p.1 as f64));
                rect.min + egui::Vec2::new(s.0 as f32, s.1 as f32) / ppp
            })
            .collect();
        painter.add(egui::Shape::line(pts, stroke));
    }
}

/// The hovered link's text underlined, as a link (doc 16a §7.2's hover, P4): a 1-px `canvas.hover` line
/// along the bottom of the link's rectangle on screen, when the link leads somewhere.
pub(super) fn paint_link_underline(
    ui: &egui::Ui,
    rect: Rect,
    ppp: f32,
    t: &DrawTransform,
    canvas: &CanvasModel,
) {
    let Some(link) = canvas.hovered_link().filter(|l| l.label().is_some()) else {
        return;
    };
    let (lo, hi) = (link.rect.min(), link.rect.max());
    let on_screen: Vec<Pos2> = [(lo.x.0, lo.y.0), (hi.x.0, hi.y.0)]
        .iter()
        .map(|&(x, y)| {
            let (sx, sy) = t.to_screen((x as f64, y as f64));
            rect.min + Vec2::new(sx as f32, sy as f32) / ppp
        })
        .collect();
    let left = on_screen[0].x.min(on_screen[1].x);
    let right = on_screen[0].x.max(on_screen[1].x);
    let bottom = on_screen[0].y.max(on_screen[1].y);
    let tokens = crate::theme::current(ui.ctx());
    ui.painter_at(rect).hline(
        left..=right,
        bottom,
        crate::theme::stroke(tokens.border_width, tokens.canvas_hover),
    );
}

/// Paints `IconId::lock` at the top-right corner of each locked placement's footprint on a panel page (doc 12 §12.1):
/// `icon.sm` in `color.text.secondary`, fixed on screen whatever the zoom, standing on the footprint's top
/// edge at its right end, so it covers none of the device's drawing or labels. On a drawn page the same mark is
/// `IconId::pin` on each selected symbol kept in place (doc 06a §2.4), on its body's top edge. An overlay of the window
/// only: the page's output (SVG, PDF) has neither. Returns the icons' frames in points.
pub(super) fn paint_locks(
    ui: &egui::Ui,
    rect: Rect,
    ppp: f32,
    t: &DrawTransform,
    canvas: &CanvasModel,
    shell: &ShellModel,
) -> Vec<Rect> {
    let page = canvas.page();
    // Each mark: its icon and the page points of the outline it stands on.
    let marks: Vec<(crate::icons::IconId, Vec<(i64, i64)>)> = match page.panel.as_deref() {
        Some(panel) => panel
            .placements
            .iter()
            .filter(|p| p.locked)
            .map(|p| {
                let (lo, hi) = (p.footprint.min(), p.footprint.max());
                let corners = [
                    (lo.x.0, lo.y.0),
                    (hi.x.0, lo.y.0),
                    (hi.x.0, hi.y.0),
                    (lo.x.0, hi.y.0),
                ];
                let on_page = corners
                    .iter()
                    .map(|&(u, v)| panel.to_page((i128::from(u), i128::from(v))))
                    .collect();
                (crate::icons::IconId::lock, on_page)
            })
            .collect(),
        None => page
            .tables
            .reps
            .iter()
            .filter(|r| r.locked && shell.selection().contains(ew_dl::EntityRef(r.id)))
            .map(|r| {
                let [x0, y0, x1, y1] = r.body;
                (crate::icons::IconId::pin, vec![(x0, y0), (x1, y1)])
            })
            .collect(),
    };
    let tokens = crate::theme::current(ui.ctx());
    let size = tokens.icon_sm;
    let painter = ui.painter_at(rect);
    let mut out = Vec::new();
    for (id, on_page) in marks {
        let on_screen: Vec<Pos2> = on_page
            .iter()
            .map(|&p| {
                let sheet = page.snapper.page_to_sheet(p);
                let (x, y) = t.to_screen((sheet.0 as f64, sheet.1 as f64));
                rect.min + Vec2::new(x as f32, y as f32) / ppp
            })
            .collect();
        let right = on_screen.iter().map(|q| q.x).fold(f32::NEG_INFINITY, f32::max);
        let top = on_screen.iter().map(|q| q.y).fold(f32::INFINITY, f32::min);
        let icon = Rect::from_min_size(Pos2::new(right - size, top - size), Vec2::splat(size));
        if !icon.intersects(rect) {
            continue;
        }
        crate::icons::paint(
            &painter,
            id,
            icon,
            tokens.text_secondary.color32(),
            tokens.icon_stroke,
        );
        out.push(icon);
    }
    out
}

/// The connection point whose row has the Properties cursor, ringed on the focused page: a `focus_width`
/// ring of `canvas.selection` around the point, fixed on screen whatever the zoom, as the canvas marks a selection.
pub(super) fn paint_cp_mark(
    ui: &egui::Ui,
    rect: Rect,
    ppp: f32,
    t: &DrawTransform,
    canvas: &CanvasModel,
    shell: &ShellModel,
) {
    let Some(at) = shell.properties_cp_point() else {
        return;
    };
    let sheet = canvas.page().snapper.page_to_sheet(at);
    let (x, y) = t.to_screen((sheet.0 as f64, sheet.1 as f64));
    let centre = rect.min + Vec2::new(x as f32, y as f32) / ppp;
    let tokens = crate::theme::current(ui.ctx());
    ui.painter_at(rect).circle_stroke(
        centre,
        tokens.space_3,
        crate::theme::stroke(tokens.focus_width, tokens.canvas_selection),
    );
}

/// The bands of a highlight item on screen (physical pixels of the pane), with its slot.
fn highlight_bands(
    item: &OverlayItem,
    t: &DrawTransform,
    ppp: f64,
) -> Option<(Vec<highlight::HatchPiece>, u8)> {
    let OverlayItem::Highlight { polylines, slot } = item else {
        return None;
    };
    let bands = polylines
        .iter()
        .flat_map(|line| {
            let pts: Vec<(f64, f64)> = line
                .iter()
                .map(|p| t.to_screen((p.0 as f64, p.1 as f64)))
                .collect();
            highlight::pieces(&pts, *slot, ppp)
        })
        .collect();
    Some((bands, *slot))
}

/// Composites the highlights into a snapshot (doc 16a §7.5): per highlight, each pixel whose centre lies in one of its bands is
/// the slot colour at 30 % over what is there, or the full colour on a hatch line. A pixel is painted once per
/// highlight even where two bands overlap, so the result does not depend on the band order.
fn composite_highlights(
    image: &mut ew_render_cpu::RgbaImage,
    t: &DrawTransform,
    ppp: f64,
    items: &[OverlayItem],
) {
    let (w, h) = (i64::from(image.width), i64::from(image.height));
    for item in items {
        let Some((bands, slot)) = highlight_bands(item, t, ppp) else {
            continue;
        };
        let (r, g, b) = highlight::SLOT_COLOURS[usize::from(slot) % highlight::MAX_HIGHLIGHTS];
        // The pixels of this highlight: 1 band, 2 hatch.
        let mut paint: BTreeMap<(i64, i64), u8> = BTreeMap::new();
        for band in &bands {
            let xs = band.band.iter().map(|p| p.0);
            let ys = band.band.iter().map(|p| p.1);
            let x0 = (xs.clone().fold(f64::INFINITY, f64::min).floor() as i64).max(0);
            let x1 = (xs.fold(f64::NEG_INFINITY, f64::max).ceil() as i64).min(w);
            let y0 = (ys.clone().fold(f64::INFINITY, f64::min).floor() as i64).max(0);
            let y1 = (ys.fold(f64::NEG_INFINITY, f64::max).ceil() as i64).min(h);
            for y in y0..y1 {
                for x in x0..x1 {
                    let centre = (x as f64 + 0.5, y as f64 + 0.5);
                    let v = match highlight::paint_at(std::slice::from_ref(band), slot, ppp, centre) {
                        Some(highlight::Paint::Hatch) => 2,
                        Some(highlight::Paint::Band) => 1,
                        None => continue,
                    };
                    let e = paint.entry((x, y)).or_insert(0);
                    *e = (*e).max(v);
                }
            }
        }
        for ((x, y), v) in paint {
            let a = if v == 2 { 255 } else { highlight::BAND_ALPHA };
            overlay::fill(image, PxRect::new(x, y, x + 1, y + 1), Color { r, g, b, a });
        }
    }
}

/// Paints the highlights of canvas group `group` in the window (doc 16a §7.5): the bands as filled quadrilaterals at 30 % and
/// the hatch segments as lines in the full colour, above the overlays.
/// Paints an open Arrange and route card's moves on canvas group `group` (docs/design/06a-drafting.md §7.3): each moved
/// symbol's new place tinted in the accent colour, its old place a faint dashed outline, the two joined by a dotted
/// line; nothing while `as_is` (B held: the page as it is).
pub(super) fn paint_arrange_moves(
    ui: &egui::Ui,
    rect: egui::Rect,
    ppp: f32,
    t: &DrawTransform,
    shell: &ShellModel,
    group: usize,
    as_is: bool,
) {
    if as_is {
        return;
    }
    let moves = shell.arrange_moves(group);
    if moves.is_empty() {
        return;
    }
    let painter = ui.painter_at(rect);
    let theme = crate::theme::current(ui.ctx());
    let to_pt = |p: (i64, i64)| {
        let (x, y) = t.to_screen((p.0 as f64, p.1 as f64));
        rect.min + egui::Vec2::new(x as f32, y as f32) / ppp
    };
    let accent = theme.accent;
    let tint = egui::Color32::from_rgba_unmultiplied(accent.r, accent.g, accent.b, 40);
    let edge = egui::Stroke::new(1.5, accent.color32());
    let faint = egui::Stroke::new(1.0, theme.text_tertiary.color32());
    for (old, new) in moves {
        let (a0, a1) = (to_pt((old[0], old[1])), to_pt((old[2], old[3])));
        let (b0, b1) = (to_pt((new[0], new[1])), to_pt((new[2], new[3])));
        let (was, now) = (egui::Rect::from_two_pos(a0, a1), egui::Rect::from_two_pos(b0, b1));
        painter.rect_filled(now, 0.0, tint);
        painter.rect_stroke(now, 0.0, edge, egui::StrokeKind::Middle);
        for side in [
            [was.left_top(), was.right_top()],
            [was.right_top(), was.right_bottom()],
            [was.right_bottom(), was.left_bottom()],
            [was.left_bottom(), was.left_top()],
        ] {
            painter.extend(egui::Shape::dashed_line(&side, faint, 4.0, 4.0));
        }
        painter.extend(egui::Shape::dotted_line(
            &[was.center(), now.center()],
            faint.color,
            6.0,
            1.0,
        ));
    }
}

pub(super) fn paint_highlights(
    ui: &egui::Ui,
    rect: egui::Rect,
    ppp: f32,
    t: &DrawTransform,
    shell: &ShellModel,
    group: usize,
) {
    let painter = ui.painter_at(rect);
    let to_pt = |p: (f64, f64)| rect.min + egui::Vec2::new(p.0 as f32, p.1 as f32) / ppp;
    for (item, _) in shell.highlight_overlay(group) {
        let Some((bands, slot)) = highlight_bands(&item, t, f64::from(ppp)) else {
            continue;
        };
        let (r, g, b) = highlight::SLOT_COLOURS[usize::from(slot) % highlight::MAX_HIGHLIGHTS];
        // The slot's colour is data of the viewmodel (doc 16 §10.3's hues arrive).
        let fill = crate::theme::rgba(r, g, b, highlight::BAND_ALPHA);
        let stroke = egui::Stroke::new(
            highlight::HATCH_WIDTH_PT as f32,
            crate::theme::rgba(r, g, b, u8::MAX),
        );
        for band in &bands {
            painter.add(egui::Shape::convex_polygon(
                band.band.iter().map(|p| to_pt(*p)).collect(),
                fill,
                egui::Stroke::NONE,
            ));
            for (a, b) in &band.hatches {
                painter.line_segment([to_pt(*a), to_pt(*b)], stroke);
            }
        }
    }
}

/// Paints the wire-colour view's strokes of canvas group `group` ([`wire_colour_shapes`]) and returns them for
/// their codes ([`paint_wire_codes`]).
pub(super) fn paint_wire_colours(
    ui: &egui::Ui,
    rect: Rect,
    ppp: f32,
    t: &DrawTransform,
    shell: &ShellModel,
    group: usize,
) -> Vec<ew_viewmodel::wire_colours::WireStroke> {
    let strokes = shell.wire_colour_overlay(group);
    if !strokes.is_empty() {
        ui.painter_at(rect)
            .extend(wire_colour_shapes(&strokes, rect.min, ppp, t));
    }
    strokes
}

/// Paints the wire-colour view's codes of `strokes`: each code as a canvas chip (`canvas.chip` at 94 % with
/// `canvas.chip_text` in `type.caption`, `radius.sm`, `space.1` padding) over every stroke, placed by doc 16a §7.4's
/// rule from the middle of its stroke in its lane, clear of the codes placed before it, of the chrome
/// `obstacles` and the other chips, of the `pointer`'s box and of the page's printed texts
/// ([`crate::canvas_chips::wire_code_chips`]); a code with no free place is drawn faded. Returns the codes' frames with
/// whether each is faded, and the printed texts' frames.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_wire_codes(
    ui: &egui::Ui,
    rect: Rect,
    ppp: f32,
    t: &DrawTransform,
    canvas: &CanvasModel,
    strokes: &[ew_viewmodel::wire_colours::WireStroke],
    obstacles: &[Rect],
    pointer: Option<Pos2>,
) -> WireCodes {
    if strokes.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let painter = ui.painter_at(rect);
    let tokens = crate::theme::current(ui.ctx());
    let screen = |p: ew_units::Pt2| {
        let (x, y) = t.to_screen((p.x.0 as f64, p.y.0 as f64));
        rect.min + egui::Vec2::new(x as f32, y as f32) / ppp
    };
    let texts: Vec<Rect> = text_rects(&canvas.page().list)
        .into_iter()
        .map(|r| Rect::from_two_pos(screen(r.min()), screen(r.max())))
        .filter(|r| r.intersects(rect))
        .collect();
    let mut obstacles = obstacles.to_vec();
    obstacles.extend(texts.iter().copied());
    let nm_per_pt = f64::from(ppp) / t.px_per_nm();
    let mut codes = Vec::new();
    for s in strokes {
        // On its own stroke, in its lane.
        let Some(mid) = s.in_lanes(nm_per_pt).midpoint() else {
            continue;
        };
        let (x, y) = t.to_screen(mid.at);
        let at = rect.min + egui::Vec2::new(x as f32, y as f32) / ppp;
        let galley = painter.layout_no_wrap(
            s.code.clone(),
            crate::theme::font(&tokens.type_caption),
            tokens.canvas_chip_text.color32(),
        );
        codes.push((at, galley.size() + egui::Vec2::splat(2.0 * tokens.space_1)));
    }
    let fade = |c: egui::Color32, faded: bool| {
        if faded {
            c.gamma_multiply(ew_viewmodel::canvas::chip::FADED_OPACITY)
        } else {
            c
        }
    };
    let placed = crate::canvas_chips::wire_code_chips(rect, &codes, tokens.space_1, &obstacles, pointer);
    let painted = strokes.iter().filter(|s| s.midpoint().is_some());
    for (&(chip, faded), s) in placed.iter().zip(painted) {
        painter.rect_filled(
            chip,
            crate::theme::radius(tokens.radius_sm),
            fade(tokens.canvas_chip.color32(), faded),
        );
        let text = fade(tokens.canvas_chip_text.color32(), faded);
        let galley = painter.layout_no_wrap(s.code.clone(), crate::theme::font(&tokens.type_caption), text);
        painter.galley(chip.center() - galley.size() / 2.0, galley, text);
    }
    (placed, texts)
}

/// The wire-colour view's strokes over a pane whose top-left is `origin`, at `ppp`: per connection a dark casing
/// `CASING_PX` physical pixel wider on each side for a cased one, then its stroke (`STROKE_PT` points) in its colour or
/// its two colours' alternating 2-mm dashes; a laned stroke is drawn on its path moved into its lanes with
/// its corners joined (`WireStroke::in_lanes`).
pub fn wire_colour_shapes(
    strokes: &[ew_viewmodel::wire_colours::WireStroke],
    origin: Pos2,
    ppp: f32,
    t: &DrawTransform,
) -> Vec<egui::Shape> {
    use ew_viewmodel::wire_colours::{CASING_PX, CASING_RGB, STROKE_PT};
    let to_pt = |p: (f64, f64)| {
        let (x, y) = t.to_screen(p);
        origin + egui::Vec2::new(x as f32, y as f32) / ppp
    };
    let rgb = |(r, g, b): (u8, u8, u8)| crate::theme::rgba(r, g, b, u8::MAX);
    let width = STROKE_PT as f32;
    let mut out = Vec::new();
    for s in strokes {
        let s = &s.in_lanes(f64::from(ppp) / t.px_per_nm());
        let path: Vec<Pos2> = s
            .polyline
            .iter()
            .map(|p| to_pt((p.0 as f64, p.1 as f64)))
            .collect();
        let casing = egui::Stroke::new(width + 2.0 * CASING_PX as f32 / ppp, rgb(CASING_RGB));
        if s.cased {
            out.push(egui::Shape::line(path.clone(), casing));
        }
        if s.colours.len() == 1 && !s.unresolved() {
            out.push(egui::Shape::line(
                path,
                egui::Stroke::new(width, rgb(s.piece_rgb(0))),
            ));
        } else {
            for (a, b, c, _) in s.pieces() {
                out.push(egui::Shape::line_segment(
                    [to_pt(a), to_pt(b)],
                    egui::Stroke::new(width, rgb(s.piece_rgb(c))),
                ));
            }
        }
    }
    out
}

/// `canvas.preview_snapshot`: the link preview's pane as the popup shows it, by the same rule as
/// [`snapshot_image`]: the target page's region around the target at the jump view's zoom, the page bounds and a
/// cursor crosshair at the target.
///
/// # Errors
///
/// The text of the renderer's error, or of a missing preview.
pub fn preview_snapshot_image(shell: &ShellModel) -> Result<(ew_render_cpu::RgbaImage, u32), String> {
    let Some(lp) = shell.link_preview() else {
        return Err(strings::text(key!("reason-no-preview")).to_owned());
    };
    render_view(
        &lp.canvas,
        &theme_of(shell),
        None,
        None,
        &[],
        lp.target.at_nm,
        &[],
        OpenCps::default(),
        overlay::Bands::default(),
    )
}

/// The canvas theme of the settings (Light for `system`; the slot the setting pins otherwise), from the tokens.
fn theme_of(shell: &ShellModel) -> Theme {
    let settings = shell.settings();
    let (light, dark, preference) = crate::theme::themes_of_setting(settings.theme.as_str());
    let theme = if preference == egui::ThemePreference::Dark {
        dark
    } else {
        light
    };
    canvas_theme(&crate::theme::tokens(
        theme,
        crate::theme::Density::of_setting(settings.density.as_str()),
    ))
}

/// The settled pane of `canvas` rendered by `render_region` with the overlays composited (see [`snapshot_image`]).
#[allow(clippy::too_many_arguments)]
fn render_view(
    canvas: &CanvasModel,
    theme: &Theme,
    grid: Option<GridSpec>,
    hover: Option<SheetRect>,
    selected: &[SheetRect],
    cursor: Option<(i64, i64)>,
    tool: &[ew_render2d::overlay::ToolMark],
    open_cps: OpenCps<'_>,
    bands: overlay::Bands<'_>,
) -> Result<(ew_render_cpu::RgbaImage, u32), String> {
    let view = view_of(canvas);
    let t = view.settled_transform();
    let dpi = view.settled_dpi();
    let (w, h) = view.pane_px_whole();
    let page = canvas.page();
    let sheet = SheetSize {
        width: Nm(page.sheet.0),
        height: Nm(page.sheet.1),
    };
    let bounds = ew_render_cpu::ItemBounds::of(&page.list, None);
    let (ox, oy) = (t.origin.0 as i64, t.origin.1 as i64);
    let mut image = ew_render_cpu::RgbaImage {
        width: w,
        height: h,
        pixels: vec![0; w as usize * h as usize * 4],
    };
    const CHUNK: u32 = 4096;
    for y0 in (0..h).step_by(CHUNK as usize) {
        for x0 in (0..w).step_by(CHUNK as usize) {
            let (cw, ch) = ((w - x0).min(CHUNK), (h - y0).min(CHUNK));
            let options = ew_render_cpu::RegionOptions {
                dpi,
                x: i64::from(x0) - ox,
                y: i64::from(y0) - oy,
                width: cw,
                height: ch,
                background: Color::WHITE,
                surround: theme.surround,
            };
            let (part, _) =
                ew_render_cpu::render_region(&page.list, Some(&bounds), sheet, &options, &DrawingFonts, None)
                    .map_err(|e| strings::format(key!("error-render"), &[&e.to_string()]))?;
            for row in 0..ch as usize {
                let from = row * cw as usize * 4;
                let to = ((y0 as usize + row) * w as usize + x0 as usize) * 4;
                image.pixels[to..to + cw as usize * 4]
                    .copy_from_slice(&part.pixels[from..from + cw as usize * 4]);
            }
        }
    }
    let o = overlay::build(&OverlayInput {
        transform: t,
        pane_px: (w, h),
        ppp: view.ppp,
        sheet,
        grid,
        hover,
        selected,
        cursor,
        tool,
        open_cps,
        bands,
    });
    overlay::composite(&mut image, &o, theme);
    Ok((image, dpi))
}

/// The overlay marks both backends share: the page bounds, the grid dots, the hover and selection outlines, the lines'
/// bands, the tool's marks and round handles and the grid cursor (last, above everything).
pub(super) fn paint_marks(painter: &egui::Painter, origin: Pos2, ppp: f32, overlay: &Overlay, theme: &Theme) {
    // The snapshot's own list (`overlay::marks`), in its order, in one mesh.
    let mut mesh = Mesh::default();
    for (r, c) in overlay::marks(overlay, theme) {
        mesh.add_colored_rect(to_points(origin, ppp, r), color32(c));
    }
    if !mesh.is_empty() {
        painter.add(mesh);
    }
}

/// The physical-pixel rectangle `r` of a pane at `origin` (points) as an egui rectangle in points.
pub(super) fn to_points(origin: Pos2, ppp: f32, r: PxRect) -> Rect {
    Rect::from_min_max(
        origin + Vec2::new(r.x0 as f32, r.y0 as f32) / ppp,
        origin + Vec2::new(r.x1 as f32, r.y1 as f32) / ppp,
    )
}

pub(super) fn color32(c: Color) -> Color32 {
    crate::theme::rgba(c.r, c.g, c.b, c.a)
}
