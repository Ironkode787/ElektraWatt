// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The frame statistics panel of a canvas (View, Frame statistics; doc 16a §7.7).

use super::*;

impl CanvasHost {
    /// The statistics overlay: fps, the p99 of the frame and of each phase, the pending tiles and the backend.
    /// The frame the statistics panel takes in `rect` (doc 16a §7.7: `space.4` from the canvas's top and right
    /// edges, `space.2` padding round its `type.caption` text): the canvas chips keep out of it.
    pub(super) fn statistics_frame(&self, ui: &Ui, rect: Rect, shell: &ShellModel) -> Rect {
        let size = statistics_size(ui.ctx(), &self.statistics_values(shell));
        let t = crate::theme::current(ui.ctx());
        Rect::from_min_size(rect.right_top() + Vec2::new(-t.space_4 - size.x, t.space_4), size)
    }

    /// The statistics shown now.
    fn statistics_values(&self, shell: &ShellModel) -> StatValues {
        StatValues {
            fps: self.stats.fps(),
            frame: self.stats.frame().p99,
            phases: [Phase::Input, Phase::Scene, Phase::Render, Phase::Paint]
                .map(|phase| self.stats.phase(phase).p99),
            pending: self.pool.as_ref().map_or(0, TilePool::pending) + self.arrived.len(),
            backend: shell.canvas_backend().label().to_string(),
        }
    }

    pub(super) fn paint_statistics(&self, ui: &Ui, rect: Rect, shell: &ShellModel) {
        // The statistics panel, a diagnostic (doc 16a §7.7): the canvas chip's look (`canvas.chip`, `radius.sm`,
        // `elevation.1`) with `type.caption` text, at the canvas's top right.
        let t = crate::theme::current(ui.ctx());
        let painter = ui.painter_at(rect);
        let back = self.statistics_frame(ui, rect, shell);
        let galley = painter.layout_no_wrap(
            self.statistics_values(shell).text(),
            crate::theme::font(&t.type_caption),
            t.canvas_chip_text.color32(),
        );
        let radius = crate::theme::radius(t.radius_sm);
        painter.add(crate::theme::shadow(&t.elevation_1).as_shape(back, radius));
        painter.rect_filled(back, radius, t.canvas_chip.color32());
        painter.galley(
            back.min + Vec2::splat(t.space_2),
            galley,
            t.canvas_chip_text.color32(),
        );
    }
}

/// The values the statistics panel shows.
#[derive(Clone, Debug, PartialEq)]
struct StatValues {
    fps: f64,
    frame: f64,
    phases: [f64; 4],
    pending: usize,
    backend: String,
}

impl StatValues {
    /// The panel's text.
    fn text(&self) -> String {
        let ms = |v: f64| format!("{v:.2}");
        let mut lines = vec![
            strings::format(key!("canvas-stats-fps"), &[&format!("{:.0}", self.fps)]),
            strings::format(key!("canvas-stats-frame"), &[&ms(self.frame)]),
        ];
        let phases = [Phase::Input, Phase::Scene, Phase::Render, Phase::Paint];
        for (phase, p99) in phases.iter().zip(self.phases) {
            lines.push(strings::format(
                key!("canvas-stats-phase"),
                &[phase.name(), &ms(p99)],
            ));
        }
        lines.push(strings::format(
            key!("canvas-stats-tiles"),
            &[&self.pending.to_string()],
        ));
        lines.push(strings::format(key!("canvas-stats-backend"), &[&self.backend]));
        lines.join("\n")
    }

    /// The same panel with every number at its widest expected width (four integer digits), so that the panel's
    /// frame does not follow the live digits.
    fn widest(&self) -> Self {
        Self {
            fps: 9999.0,
            frame: 9999.99,
            phases: [9999.99; 4],
            pending: 9999,
            backend: self.backend.clone(),
        }
    }
}

/// The size of the statistics panel's frame: its text laid out at the widest expected values (and at the live ones
/// when those are wider still), plus `space.2` padding (the frame stays put while the numbers change).
fn statistics_size(ctx: &egui::Context, v: &StatValues) -> Vec2 {
    let t = crate::theme::current(ctx);
    let font = crate::theme::font(&t.type_caption);
    let measure =
        |text: String| ctx.fonts_mut(|f| f.layout_no_wrap(text, font.clone(), crate::theme::NO_TINT).size());
    measure(v.widest().text()).max(measure(v.text())) + Vec2::splat(2.0 * t.space_2)
}

#[cfg(test)]
mod statistics_tests {
    use super::{StatValues, statistics_size};

    fn values(ms: f64) -> StatValues {
        StatValues {
            fps: 1000.0 / ms,
            frame: ms,
            phases: [ms / 4.0; 4],
            pending: ms as usize,
            backend: "CPU".to_string(),
        }
    }

    #[test]
    fn the_statistics_frame_does_not_follow_the_numbers() {
        let ctx = egui::Context::default();
        let mut sizes = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx();
            sizes.push(statistics_size(ctx, &values(9.0)));
            sizes.push(statistics_size(ctx, &values(123.0)));
        });
        out.textures_delta.clear();
        assert_eq!(sizes[0], sizes[1]);
    }
}
