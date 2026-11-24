// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool options card over the page view (doc 16a §5.5): the fields of the tools that have them.

use super::*;

impl App {
    /// The tool options card at the page view's top left: the fields of the IP tool,
    /// the Text tool's binding, the Wire marker's fields and the Structure box's style and fields,
    /// which the removed tool strip held, and the layout place options. Shown only while one of those
    /// tools runs (or Select drags a placement); returns the card's frame (the canvas chips keep off it).
    pub(super) fn tool_options(
        &mut self,
        ctx: &egui::Context,
        pane: egui::Rect,
        events: &mut Vec<Event>,
    ) -> Option<(egui::Rect, bool)> {
        let tool = self.shell.tool().name();
        let (ip, text, marker, structure_box) = (
            tool == "ip",
            tool == "text",
            tool == "marker",
            tool == "structure_box",
        );
        // The name is the first thing the interruption point tool takes when it is chosen from the bar or a menu:
        // the pointer is off the sheet then, and what is typed next is the point's name. (It would go to the canvas,
        // where the letters were lost and Enter placed a point at the grid cursor.)
        // Chosen by its key with the pointer on the sheet (or with no pointer in the window), the keys stay with the
        // tool, so D turns the direction.
        // Noted every frame (noted only while the tool ran, so the tool chosen a second time kept the keys
        // from the name).
        let ip_focus = crate::components::opened(ctx, egui::Id::unique("ip-fields-opened"), ip.then_some(()))
            && ctx.pointer_latest_pos().is_some_and(|p| !pane.contains(p));
        // The layout place tool, or a placement dragged by Select.
        let layout = self
            .shell
            .layout_options_shown()
            .then(|| self.shell.layout_card_options());
        // Draw rail's and Draw duct's part, grid step and duct size.
        let draw = self
            .shell
            .draw_card()
            .map(|card| (card, self.shell.layout_options().grid_mm));
        if !(ip || text || marker || structure_box || layout.is_some() || draw.is_some()) {
            self.options_rect = None;
            self.options_band = None;
            return None;
        }
        let t = crate::theme::current(ctx);
        let externals = if ip || structure_box {
            self.shell.external_choices()
        } else {
            Vec::new()
        };
        let text_bind = if text { self.shell.text_bind() } else { None };
        // The IP name field shows the typed name, else the ghost's proposal as its hint.
        let ip_typed = self.shell.ip_typed_name().map(str::to_owned);
        let (ip_proposal, ip_external, ip_direction) = match self.shell.tool_ghost() {
            Some(ew_viewmodel::tools::Ghost::Ip {
                name,
                external,
                direction,
                star,
                ..
            }) if ip => {
                // An external point is always a target and has no choice; a star source is a source.
                let choice = external
                    .is_none()
                    .then_some(if star { "source" } else { direction });
                (Some(name).filter(|n| !n.is_empty()), external, choice)
            }
            _ => (None, None, None),
        };
        // See below. The card stays whole while it is being used: the pointer on its controls, or a field of it
        // typed into (FT-1646: the ghost follows the pointer, so going to the name field to type put the ghost
        // under the card and the card at 40 %, which read as a disabled field).
        let in_use = ctx.text_edit_focused()
            || self
                .options_band
                .zip(ctx.pointer_latest_pos())
                .is_some_and(|(band, p)| band.contains(p));
        let faded = !in_use
            && self
                .options_rect
                .zip(self.ghost_screen_point())
                .is_some_and(|(rect, ghost)| rect.contains(ghost));
        // Right under the tab bar, one row high: a fitted page's margin and its frame's band of column numbers hold
        // it, so it covers none of the drawing (FT-929, FT-981: with its title and `space.4` padding it lay over
        // the sheet's top left, where terminals are clicked).
        let at = egui::pos2(
            pane.left() + t.space_4,
            pane.top() + t.size_dock_tab_bar + t.space_1,
        );
        let App {
            ip_name,
            marker_csa,
            marker_colours,
            structure_caption,
            structure_location,
            ..
        } = self;
        // Only the controls take the pointer. The card is two layers: its frame, which lets the pointer through to
        // the canvas beneath (a click on it reaches the sheet), and, over it, a band holding the controls, which are
        // the card's only interactive part; the fields carry their own labels, so the card has no title. While the
        // tool's ghost is under the card (by the rect the last frame drew, so the card never flips) it is drawn at
        // 40 % and is no chip obstacle.
        let opacity = if faded { OPTIONS_FADED_OPACITY } else { 1.0 };
        let frame = crate::components::card_frame(&t)
            .inner_margin(egui::Margin::same(crate::theme::margin_px(t.space_2)))
            .shadow(crate::theme::shadow(&t.elevation_1));
        let margin = frame.total_margin();
        let band_at = at + egui::vec2(margin.left, margin.top);
        // The areas are named per tool, so a tool's card sizes itself on a sizing pass rather than inheriting
        // the last tool's size as its bound.
        let id = egui::Id::unique(("tool-options", tool));
        // The band takes the pointer only on its controls. A container's hit rectangle is its clip
        // rectangle cut to its extent, so the band's area (constrained to nothing, and not moved by it) and its row
        // leave a clip of nothing behind them as they end: a click in a gap between the controls reaches the sheet.
        let screen = ctx.content_rect();
        let band = egui::Area::new(id.with("controls"))
            .fixed_pos(band_at)
            .order(egui::Order::Middle)
            .constrain_to(egui::Rect::NOTHING)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(screen);
                ui.set_opacity(opacity);
                ui.horizontal(|ui| {
                    // The IP tool's name field and external-reference field.
                    if ip {
                        crate::tool_view::ip_fields(
                            ui,
                            ip_name,
                            &crate::tool_view::IpFields {
                                typed: ip_typed.as_deref(),
                                proposal: ip_proposal.as_deref(),
                                external: ip_external.as_deref(),
                                externals: &externals,
                                focus: ip_focus,
                                direction: ip_direction,
                            },
                            events,
                        );
                    }
                    // The Text tool's binding.
                    if text {
                        crate::tool_view::text_fields(ui, text_bind.as_deref(), events);
                    }
                    // The Wire marker's fields and the Structure box's style and fields.
                    if marker {
                        crate::tool_view::marker_fields(ui, marker_csa, marker_colours, events);
                    }
                    if structure_box {
                        crate::tool_view::structure_box_fields(
                            ui,
                            &externals,
                            structure_caption,
                            structure_location,
                            events,
                        );
                    }
                    if let Some(options) = layout {
                        crate::tool_view::layout_fields(ui, options, events);
                    }
                    if let Some((card, grid_mm)) = &draw {
                        crate::tool_view::draw_fields(ui, card, *grid_mm, events);
                    }
                    ui.set_clip_rect(egui::Rect::NOTHING);
                });
                ui.set_clip_rect(egui::Rect::NOTHING);
            });
        let band_size = band.response.rect.size();
        let card = egui::Area::new(id)
            .interactable(false)
            .fixed_pos(at)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                // A card of doc 16a §6.9 (panel fill, a 1-px border, `radius.md`) with `elevation.1`, as it floats
                // over the canvas, and `space.2` padding around its one row.
                frame.show(ui, |ui| {
                    ui.allocate_space(band_size);
                });
            });
        // The controls stand over the frame, which was shown after them.
        ctx.move_to_top(band.response.layer_id);
        // The keys go back to the tool when the pointer goes back to the sheet: with a name typed and the pointer
        // moved to where the point goes, D turns the direction and is not typed into the name (FT-1657).
        let to_sheet = ctx.input(|i| {
            i.pointer.delta() != egui::Vec2::ZERO
                && i.pointer
                    .hover_pos()
                    .is_some_and(|p| pane.contains(p) && !card.response.rect.contains(p))
        });
        if to_sheet
            && ctx.text_edit_focused()
            && let Some(focused) = ctx.memory(|m| m.focused())
        {
            ctx.memory_mut(|m| m.surrender_focus(focused));
        }
        self.options_band = Some(egui::Rect::from_min_size(band_at, band_size));
        self.options_rect = Some(card.response.rect);
        Some((card.response.rect, faded))
    }

    /// The screen point of the tool's ghost on the focused canvas as its last frame drew it: its insertion point, else
    /// the pointer's; `None` without a ghost.
    fn ghost_screen_point(&self) -> Option<egui::Pos2> {
        use ew_viewmodel::tools::Ghost;
        let ghost = self.shell.tool_ghost()?;
        let canvas = self.shell.canvas()?;
        let at = match ghost {
            Ghost::Place { at_nm, .. }
            | Ghost::Paste { at_nm, .. }
            | Ghost::Ip { at_nm, .. }
            | Ghost::Text { at_nm, .. } => at_nm.map(|p| canvas.page().snapper.page_to_sheet(p)),
            _ => None,
        }
        .or_else(|| canvas.pointer())?;
        let group = self.shell.area().map_or(0, |a| a.focused);
        self.canvas.drawn_screen_of(group, at)
    }
}

/// The opacity of the tool options card while the tool's ghost is under it (doc 16a §5.5).
const OPTIONS_FADED_OPACITY: f32 = 0.4;
