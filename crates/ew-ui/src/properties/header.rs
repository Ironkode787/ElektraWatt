// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Properties pane's header: the title of what is shown, its lines, and the chips of its findings by severity.

use super::*;

impl PropertiesPane {
    /// The header: title, subtitle, location, finding chips (doc 16a §6.8: the title in `type.title`, the
    /// subtitle and the location in `type.small`, secondary; one severity chip per finding at the right of the
    /// title line, its icon and word, the rule and message as the tooltip and in its node's name).
    pub(super) fn header(
        ui: &mut Ui,
        sheet: &PropertySheet,
        editing: Option<&str>,
        link: Option<ew_viewmodel::actions::ActionItem>,
        style: Option<ew_viewmodel::actions::ActionItem>,
        events: &mut Vec<Event>,
    ) {
        let t = crate::theme::current(ui.ctx());
        // The chips are laid out first, at the right, and the title takes what is left,
        // ellipsised; the findings collapse into one count chip per severity ("3 Warnings", every rule and message in
        // its tooltip), so the header keeps its width whatever the count (doc 16a §6.8: "the findings badge").
        // Always one chip per severity, also for one or two findings ("2 Warnings", not
        // "Warning Warning").
        // The chips never take the title's room. When the title at its full width and the chips
        // with their words do not fit the line, each severity becomes one glyph chip (its icon, and the count when
        // more than one), the words in its tooltip and node name; the title is then ellipsised by the panel only.
        let findings = &sheet.header.findings;
        let groups = severity_groups(findings, editing);
        let compact = {
            let measure = |text: String, font: &crate::theme::TypeToken| {
                ui.painter()
                    .layout_no_wrap(text, crate::theme::font(font), t.text_primary.color32())
                    .size()
                    .x
            };
            let chip = |word: String| {
                t.space_2
                    + t.type_caption.size
                    + t.space_1
                    + t.space_1
                    + measure(word, &t.type_caption)
                    + t.space_2
            };
            let chips: f32 = groups
                .iter()
                .map(|(severity, of)| chip(crate::messages::count_words(*severity, of.len())))
                .sum();
            // The chip that says where every value is from stands in the same line, with its gap.
            let shared = ew_viewmodel::properties::shared_provenance(sheet).map_or(0.0, |p| {
                crate::components::caption_chip_layout(ui, &t, chip_word(p)).1.x + t.space_1
            });
            let gaps = t.space_1 * groups.len().saturating_sub(1) as f32 + shared;
            // The title elides first (whole in its tooltip), down to its first three
            // characters and the ellipsis; only past that do the chips lose their words.
            let shortest: String = sheet
                .header
                .title
                .chars()
                .take(3)
                .chain(std::iter::once('\u{2026}'))
                .collect();
            let title = measure(shortest, &t.type_title);
            !findings.is_empty() && title + t.space_2 + chips + gaps > ui.available_width() - 2.0 * t.space_4
        };
        // The header keeps the pane's `space.4` gutter on both sides, as the filter field and the row labels, so
        // a chip at the right end is never clipped by the pane's edge.
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(crate::theme::margin_px(t.space_4), 0))
            .show(ui, |ui| {
                Self::header_lines(
                    ui,
                    sheet,
                    &groups,
                    compact,
                    [link.as_ref(), style.as_ref()],
                    events,
                )
            });
        ui.separator();
    }

    /// The header's lines inside its gutter: the title line with its chips, what the device is, the subtitle, the
    /// location, the read-only reason ([`Self::header`]).
    pub(super) fn header_lines(
        ui: &mut Ui,
        sheet: &PropertySheet,
        groups: &[(crate::messages::Severity, Vec<(&str, &str)>)],
        compact: bool,
        links: [Option<&ew_viewmodel::actions::ActionItem>; 2],
        events: &mut Vec<Event>,
    ) {
        let t = crate::theme::current(ui.ctx());
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = t.space_1;
                // Right to left: info, warning, error, so the most severe reads first. A chip is a button
                // that opens Messages filtered to the record (the existing `messages.show` and `navigator.filter`).
                for (severity, of) in groups.iter().rev() {
                    let chip = if compact {
                        glyph_chip(ui, *severity, of)
                    } else {
                        crate::messages::severity_count_chip(ui, *severity, of)
                    };
                    if chip.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        events.extend(findings_events(&sheet.header));
                    }
                }
                // Where every value shares its source (a report page's, generated), the header says it once.
                if let Some(p) = ew_viewmodel::properties::shared_provenance(sheet) {
                    crate::components::caption_chip(ui, chip_word(p)).tip(chip_text(p));
                }
                ui.add_space(t.space_2);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&sheet.header.title).font(crate::theme::font(&t.type_title)),
                        )
                        .truncate(),
                    );
                });
            });
        });
        let small = |s: &str| crate::components::small_secondary(&t, s);
        // What the device is ("Compressed-air supply point · pneumatic").
        if let Some(what) = &sheet.header.what {
            ui.label(small(&ew_viewmodel::properties::what_words(what)));
        }
        if let Some(s) = &sheet.header.subtitle {
            ui.label(small(s));
        }
        if let Some(l) = &sheet.header.location {
            ui.label(small(l));
        }
        // One page's sheet links its title block's fields, where a person looks for them.
        if sheet.read_only.is_none() && sheet.header.kinds.as_slice() == [("page".to_owned(), 1)] {
            let link = egui::Link::new(crate::components::styled(
                strings::text(key!("action-page-title-block")),
                &t.type_small,
                t.accent,
            ));
            if ui.add(link).clicked() {
                events.push(Event::Action("page.title_block", json!({})));
            }
        }
        // An interruption point's sheet links Supply comes in here…, or Remove the supply mark; a text's
        // and a device's sheet name their text style and open Text styles at it.
        for item in links
            .into_iter()
            .flatten()
            .filter(|i| sheet.read_only.is_none() && i.enabled)
        {
            let link = egui::Link::new(crate::components::styled(&item.label, &t.type_small, t.accent));
            if ui.add(link).clicked() {
                events.push(Event::Action(item.id, item.params.clone()));
            }
        }
        if let Some(r) = &sheet.read_only {
            ui.label(RichText::new(r).italics());
        }
    }
}

/// The header's findings by severity, most severe first (`error`, `warning`, then every other severity as `info`),
/// then `pending`, each with its `(rule, message)` pairs; severities without a finding are left out. A finding
/// pending while `editing` (the editing page's folio) is the editing page counts as pending, not under its severity,
/// as the Messages header counts it.
pub(super) fn severity_groups<'a>(
    findings: &'a [ew_viewmodel::ew_api::output::FindingChip],
    editing: Option<&str>,
) -> Vec<(crate::messages::Severity, Vec<(&'a str, &'a str)>)> {
    let pending = |f: &ew_viewmodel::ew_api::output::FindingChip| {
        ew_viewmodel::navigators::messages::chip_pending(f, editing)
    };
    use crate::messages::Severity;
    [
        Severity::Error,
        Severity::Warning,
        Severity::Info,
        Severity::Pending,
    ]
    .into_iter()
    .map(|severity| {
        let of: Vec<(&str, &str)> = findings
            .iter()
            .filter(|f| match severity {
                Severity::Pending => pending(f),
                _ if pending(f) => false,
                Severity::Error | Severity::Warning => Severity::of(f.severity) == severity,
                Severity::Info => !matches!(Severity::of(f.severity), Severity::Error | Severity::Warning),
            })
            .map(|f| (f.rule.as_str(), f.message.as_str()))
            .collect();
        (severity, of)
    })
    .filter(|(_, of)| !of.is_empty())
    .collect()
}

/// A severity's glyph chip: the severity's subtle fill, its icon, and the count in `type.caption` when more
/// than one; named like the count chip ("2 Warnings: <rule>: <message>; …"), every finding in its tooltip.
/// The actions of a click on a Properties finding chip: Messages opens, filtered to the record
/// ([`ew_viewmodel::properties::findings_filter`]), at its first row, so the most severe findings are in view (the list kept its
/// cursor on an info below them, and the error the chip counted stood above the pane's top).
pub(super) fn findings_events(header: &ew_viewmodel::ew_api::output::SheetHeader) -> [Event; 3] {
    [
        Event::Action("messages.show", json!({"on": true})),
        Event::Action(
            "navigator.filter",
            json!({"kind": "messages", "text": ew_viewmodel::properties::findings_filter(header)}),
        ),
        Event::Action("navigator.row", json!({"kind": "messages", "to": "first"})),
    ]
}

pub(super) fn glyph_chip(
    ui: &mut Ui,
    severity: crate::messages::Severity,
    findings: &[(&str, &str)],
) -> egui::Response {
    let t = crate::theme::current(ui.ctx());
    let (fill, text) = severity.count_colours(&t);
    // The glyph of a pending chip is in its text colour, not the info's.
    let icon_colour = match severity {
        crate::messages::Severity::Pending => t.text_secondary,
        s => s.icon_colour(&t),
    };
    let count = (findings.len() > 1).then(|| {
        ui.painter().layout_no_wrap(
            findings.len().to_string(),
            crate::theme::font(&t.type_caption),
            text.color32(),
        )
    });
    let icon = t.type_caption.size + t.space_1;
    let width = t.space_1 + icon + count.as_ref().map_or(0.0, |g| t.space_1 + g.size().x) + t.space_1;
    // A button that opens Messages filtered to the record.
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, t.size_badge), Sense::click());
    let lines: Vec<String> = findings
        .iter()
        .map(|(rule, message)| strings::format(key!("props-finding-tip"), &[rule, message]))
        .collect();
    let name = strings::format(
        key!("props-findings-chip"),
        &[
            &crate::messages::count_words(severity, findings.len()),
            &lines.join("; "),
        ],
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    let painter = ui.painter();
    painter.rect_filled(rect, crate::theme::radius(t.radius_sm), fill.color32());
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + t.space_1 + icon / 2.0, rect.center().y),
        Vec2::splat(icon),
    );
    crate::icons::paint(
        painter,
        severity.icon(),
        icon_rect,
        icon_colour.color32(),
        t.icon_stroke,
    );
    if let Some(g) = count {
        painter.galley(
            egui::pos2(icon_rect.right() + t.space_1, rect.center().y - g.size().y / 2.0),
            g,
            text.color32(),
        );
    }
    response.tip(crate::messages::chip_tip(severity, &lines))
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;
    use ew_viewmodel::ew_api::output::{FindingChip, SheetHeader};
    use ew_viewmodel::properties::{Category, EditRoute, FieldReadOnly, FieldType};

    use super::*;

    /// A generated page's sheet: its one row's source is the header's chip "generated", and it has one warning.
    fn generated_sheet() -> PropertySheet {
        let row = PropRow {
            key: "page/title".to_owned(),
            path: "title".to_owned(),
            category: Category::Identity,
            depth: 0,
            ty: FieldType::Text,
            value: RowValue::One {
                json: serde_json::Value::String("Wire list".to_owned()),
                text: "Wire list".to_owned(),
            },
            provenance: Provenance::Generated {
                block: "Wire list".to_owned(),
            },
            actions: Vec::new(),
            route: EditRoute::ReadOnly(FieldReadOnly::Identity),
            identifier: false,
            source: None,
            named_reference: false,
            label: None,
            link: None,
            note: None,
            words: Vec::new(),
        };
        PropertySheet {
            rev: 0,
            header: SheetHeader {
                title: "907 Wire list".to_owned(),
                items: 1,
                kinds: vec![("page".to_owned(), 1)],
                findings: vec![FindingChip {
                    rule: "EW-CORE-001".to_owned(),
                    severity: ew_viewmodel::ew_api::checks::Severity::Warning,
                    message: "a warning".to_owned(),
                    pending_on: None,
                }],
                ..Default::default()
            },
            sections: vec![SheetSection {
                kind: "page".to_owned(),
                targets: Vec::new(),
                rows: vec![row],
                port: None,
            }],
            missing: 0,
            read_only: None,
            languages: Vec::new(),
        }
    }

    /// The header drawn `width` wide: its warning chip's width and its word's, the source chip's width and the title's
    /// shortest form's.
    fn header_at(width: f32) -> (f32, f32, f32, f32) {
        let t = crate::theme::tokens(crate::theme::Theme::Light, crate::theme::Density::Comfortable);
        let sheet = generated_sheet();
        let mut ready = false;
        let mut h = Harness::builder()
            .with_size(egui::vec2(width + 200.0, 200.0))
            .build_ui(move |ui| {
                // The fonts apply from the next pass: the first draws nothing.
                if !ready {
                    crate::fonts::install(ui.ctx());
                    crate::theme::install(ui.ctx(), &t);
                    ready = true;
                    return;
                }
                ui.allocate_ui(egui::vec2(width, 100.0), |ui| {
                    PropertiesPane::header(ui, &sheet, None, None, None, &mut Vec::new());
                });
            });
        h.run();
        let chip = h.get_by_label_contains("a warning").rect().width();
        let source = h.get_by_label("generated").rect().width();
        let t = crate::theme::current(&h.ctx);
        let measure = |text: String, font: &crate::theme::TypeToken| {
            h.ctx.fonts_mut(|f| {
                f.layout_no_wrap(text, crate::theme::font(font), egui::Color32::WHITE)
                    .size()
                    .x
            })
        };
        let word = measure(
            crate::messages::count_words(crate::messages::Severity::Warning, 1),
            &t.type_caption,
        );
        // The title's shortest form: its first three characters and the ellipsis.
        let title = measure("907\u{2026}".to_owned(), &t.type_title);
        (chip, word, source, title)
    }

    /// The width that decides whether the header goes compact counts the chip that says where every value is from: a
    /// pane where the title's shortest form and the worded chips fit without that chip, and not with it, shows the
    /// warning as its glyph rather than cutting the title.
    #[test]
    fn the_compact_header_counts_the_source_chip() {
        let (chip, word, source, title) = header_at(1200.0);
        assert!(chip > word, "wide, the chip keeps its word");
        let t = crate::theme::tokens(crate::theme::Theme::Light, crate::theme::Density::Comfortable);
        // Wider than the title and the worded chip need, narrower than that with the source chip beside them.
        let width = 2.0 * t.space_4 + title + t.space_2 + chip + source / 2.0;
        let (chip, word, _, _) = header_at(width);
        assert!(
            chip < word,
            "at {width} the warning chip keeps its word and the title is cut"
        );
    }
}
