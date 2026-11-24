// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The arrival mark: the ring around a record the view arrived at, and the texts it runs under.

use super::*;

impl CanvasHost {
    /// The opacity group `group`'s jump arrival mark was painted at last, `None` when none showed (tests).
    pub fn arrival_mark(&self, group: usize) -> Option<f32> {
        self.arrivals
            .get(group)
            .copied()
            .flatten()
            .map(|(_, _, a)| a)
            .filter(|a| *a > 0.0)
    }

    /// Paints group `group`'s jump arrival mark: a ring in `color.accent` around the point the jump
    /// arrived at, fading out over [`ARRIVAL_SECONDS`] of frame time from the first frame that shows it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_arrival(
        &mut self,
        ui: &Ui,
        rect: Rect,
        ppp: f32,
        t: &DrawTransform,
        canvas: &CanvasModel,
        group: usize,
        reduce_motion: bool,
    ) {
        let Some((at, serial)) = canvas.arrival() else {
            self.arrivals[group] = None;
            return;
        };
        let time = ui.input(|i| i.time);
        let start = match self.arrivals[group] {
            Some((s, start, _)) if s == serial => start,
            _ => time,
        };
        let opacity = arrival_opacity(time - start, reduce_motion).unwrap_or(0.0);
        self.arrivals[group] = Some((serial, start, opacity));
        if opacity <= 0.0 {
            return;
        }
        let tokens = crate::theme::current(ui.ctx());
        let (x, y) = t.to_screen((at.0 as f64, at.1 as f64));
        let centre = rect.min + egui::Vec2::new(x as f32, y as f32) / ppp;
        let mut colour = tokens.accent;
        colour.a = (f32::from(colour.a) * opacity).round() as u8;
        // Doc 16a §5.5: the ring runs under the page's texts: it is left out where it crosses one,
        // so it never covers a text.
        let radius = tokens.size_control_inline;
        let stroke = crate::theme::stroke(2.0 * tokens.focus_width, colour);
        let ring = Rect::from_center_size(centre, Vec2::splat(2.0 * radius + stroke.width));
        let texts: Vec<Rect> = text_rects(&canvas.page().list)
            .into_iter()
            .map(|r| {
                let a = t.to_screen((r.min().x.0 as f64, r.min().y.0 as f64));
                let b = t.to_screen((r.max().x.0 as f64, r.max().y.0 as f64));
                Rect::from_two_pos(
                    rect.min + Vec2::new(a.0 as f32, a.1 as f32) / ppp,
                    rect.min + Vec2::new(b.0 as f32, b.1 as f32) / ppp,
                )
                .expand(stroke.width / 2.0)
            })
            .filter(|r| r.intersects(ring))
            .collect();
        let painter = ui.painter_at(rect);
        for run in ring_runs(centre, radius, &texts) {
            painter.add(egui::Shape::line(run, stroke));
        }
        if reduce_motion {
            // A still mark needs one more frame only, when its moment is over (a visual run's next frames show it until
            // that much frame time has passed).
            let left = ARRIVAL_SECONDS - (time - start);
            ui.ctx()
                .request_repaint_after(Duration::from_secs_f64(left.max(0.0)));
        } else {
            ui.ctx().request_repaint();
        }
    }
}

/// How long a jump's arrival stays marked, in seconds of frame time: its ring fades out over it.
pub(crate) const ARRIVAL_SECONDS: f64 = 1.5;

/// The opacity of a jump's arrival mark `elapsed` seconds after it started: from 1 down to 0 over
/// [`ARRIVAL_SECONDS`], or 1 throughout with reduced motion; `None` once it is over.
pub(crate) fn arrival_opacity(elapsed: f64, reduce_motion: bool) -> Option<f32> {
    if !(0.0..ARRIVAL_SECONDS).contains(&elapsed) {
        return None;
    }
    Some(if reduce_motion {
        1.0
    } else {
        (1.0 - elapsed / ARRIVAL_SECONDS) as f32
    })
}

/// The arrival ring of `radius` around `centre` as the polylines it is drawn with: the circle in 96 steps,
/// each step whose middle lies in one of `texts` left out, so the ring runs under the texts.
pub(crate) fn ring_runs(centre: Pos2, radius: f32, texts: &[Rect]) -> Vec<Vec<Pos2>> {
    const STEPS: usize = 96;
    let on_ring = |i: usize| centre + radius * Vec2::angled(i as f32 * std::f32::consts::TAU / STEPS as f32);
    let mut runs = Vec::new();
    let mut run: Vec<Pos2> = Vec::new();
    for i in 0..STEPS {
        let (a, b) = (on_ring(i), on_ring(i + 1));
        if texts.iter().any(|r| r.contains(a + (b - a) / 2.0)) {
            if run.len() > 1 {
                runs.push(std::mem::take(&mut run));
            }
            run.clear();
            continue;
        }
        if run.is_empty() {
            run.push(a);
        }
        run.push(b);
    }
    if run.len() > 1 {
        runs.push(run);
    }
    runs
}

/// The frames, in sheet nm, of the printed texts of the page's display list `list` (a device's texts, the title
/// block's): each glyph run at any depth, its layout box through the transforms of the groups that hold it. Symbol
/// instances are not resolved. The wire codes and the arrival ring keep clear of them.
pub(crate) fn text_rects(list: &ew_dl::DisplayList) -> Vec<SheetRect> {
    use ew_dl::ItemKind;
    list.placed()
        .filter_map(|placed| {
            let ItemKind::Glyphs(run) = &placed.item.kind else {
                return None;
            };
            let [a, b, c, d] = run.layout_quad().map(|q| placed.to_list(q));
            Some(SheetRect::from_points(a, b).union(SheetRect::from_points(c, d)))
        })
        .collect()
}

#[cfg(test)]
mod arrival_tests {
    use super::*;
    use ew_dl::{Brush, DisplayList, FontKey, Glyph, Item, ShapedRun, Transform, ew_units::Rect as R};
    use ew_units::Pt2;

    /// The ring runs under a text: no step of it lies in the text's rectangle, and the rest of the
    /// circle is drawn; without a text it is one closed run.
    #[test]
    fn the_arrival_ring_runs_under_the_texts() {
        let centre = egui::pos2(100.0, 100.0);
        let whole = ring_runs(centre, 24.0, &[]);
        assert_eq!(whole.len(), 1);
        assert_eq!(whole[0].len(), 97);
        let text = Rect::from_min_max(egui::pos2(110.0, 90.0), egui::pos2(200.0, 110.0));
        let runs = ring_runs(centre, 24.0, &[text]);
        assert!(!runs.is_empty());
        for run in &runs {
            for w in run.windows(2) {
                assert!(!text.contains(w[0] + (w[1] - w[0]) / 2.0), "{w:?} in the text");
            }
        }
        let drawn: usize = runs.iter().map(|r| r.len() - 1).sum();
        assert!(drawn > 48 && drawn < 96, "{drawn} of 96 steps");
    }

    /// The texts of a list are its glyph runs' rectangles, through the groups' transforms; paths are not
    /// texts.
    #[test]
    fn the_texts_of_a_list_are_its_glyph_runs() {
        let p = |x, y| Pt2::new(Nm(x), Nm(y));
        let run = ShapedRun {
            font: FontKey {
                sha256: [1; 32],
                face_index: 0,
            },
            size: Nm(2_000),
            origin: p(1_000, 1_000),
            transform: Transform::IDENTITY,
            glyphs: vec![Glyph {
                id: 5,
                x: Nm(0),
                y: Nm(0),
            }],
            text: "X".into(),
            brush: Brush::BLACK,
            layout_box: R::from_points(p(0, -1_600), p(1_200, 400)),
        };
        let list = DisplayList {
            items: vec![
                Item::path(ew_dl::Path::line(p(0, 0), p(9, 9)), None, None),
                Item::group(
                    Some(Transform::from_translation(p(10_000, 0))),
                    None,
                    vec![Item::glyphs(Arc::new(run))],
                ),
            ],
        };
        assert_eq!(
            text_rects(&list),
            [R::from_points(p(11_000, -600), p(12_200, 1_400))]
        );
    }
}
