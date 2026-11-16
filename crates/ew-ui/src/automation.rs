// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dialog contents of the automation workspaces (WP-S2.13), drawn inside the one dialog frame
//! ([`crate::dialog`]) from the shell's views. A station's rack is no dialog: it is a view in a canvas tab
//! ([`crate::station_view`]).

use egui::{RichText, ScrollArea, Ui};
use ew_viewmodel::key;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::theme;

/// The PLC import's preview (doc 10 §6): one tab per list with its count (Changed, Conflicts, Removed, Parking), then
/// the shown tab's rows: a change's object and field with the project's and the file's value and what the merge makes
/// of it; a conflict's choice between keeping the project's value and taking the file's; a removed object; a parked
/// object, with Create where it can be created unplaced.
pub(crate) fn plc_import(
    ui: &mut Ui,
    card: &ew_viewmodel::shell::automation::PlcImportCard,
    events: &mut Vec<Event>,
) {
    use ew_viewmodel::shell::automation::ImportTab;
    let t = theme::current(ui.ctx());
    let small = |text: String, colour: theme::Rgba| {
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(colour.color32())
    };
    // "The file is complete" (doc 10 §4.4) makes the preview again: what the file lacks is then removed.
    let mut complete = card.complete;
    if ui
        .checkbox(&mut complete, strings::text(key!("plc-import-complete")))
        .changed()
    {
        events.push(Event::Action(
            "plc.import",
            json!({ "path": card.path, "complete": complete }),
        ));
    }
    if let Some(why) = &card.refusal {
        ui.label(small(why.clone(), t.error));
    }
    ui.horizontal(|ui| {
        for tab in ImportTab::ALL {
            let label = strings::format(
                ew_viewmodel::strings::Key::run_time(&format!("plc-import-tab-{}", tab.word())),
                &[&card.count(tab).to_string()],
            );
            let on = card.tab == tab;
            let r = ui.selectable_label(on, label.clone());
            if r.clicked() {
                events.push(Event::Action("plc.import_tab", json!({ "tab": tab.word() })));
            }
        }
    });
    ScrollArea::vertical()
        .max_height(ui.ctx().content_rect().height() * 0.45)
        .auto_shrink([false, true])
        .show(ui, |ui| match card.tab {
            tab if card.count(tab) == 0 => {
                ui.label(small(
                    strings::text(key!("plc-import-empty")).to_owned(),
                    t.text_secondary,
                ));
            }
            ImportTab::Changed | ImportTab::Conflicts => {
                let rows = if card.tab == ImportTab::Changed {
                    &card.changes
                } else {
                    &card.conflicts
                };
                for c in rows {
                    ui.label(small(c.text.clone(), t.text_primary));
                    ui.horizontal_wrapped(|ui| {
                        // Which value is the project's and which the file's, in words.
                        let none = || strings::text(key!("plc-import-no-value")).to_owned();
                        let side = |v: &str| if v.is_empty() { none() } else { v.to_owned() };
                        let sides =
                            strings::format(key!("plc-import-sides"), &[&side(&c.ours), &side(&c.theirs)]);
                        ui.label(small(sides, t.text_primary));
                        let state = strings::text(ew_viewmodel::strings::Key::run_time(&format!(
                            "plc-import-state-{}",
                            c.state
                        )));
                        ui.label(small(format!("· {state}"), t.text_tertiary));
                    });
                    if card.tab == ImportTab::Changed
                        && let Some(why) = &c.refused
                    {
                        ui.label(small(why.clone(), t.error));
                    }
                    if card.tab == ImportTab::Conflicts {
                        // The side Apply takes: the one chosen, else the merge's default (FT-1756: the file's value
                        // only where it can be taken, else the reason why not).
                        let chosen = card.choices.get(&c.id).map_or(c.default, String::as_str);
                        ui.horizontal_wrapped(|ui| {
                            for (word, key) in [
                                ("ours", key!("plc-import-keep-ours")),
                                ("theirs", key!("plc-import-take-theirs")),
                            ] {
                                if word == "theirs"
                                    && let Some(why) = &c.refused
                                {
                                    ui.label(small(why.clone(), t.text_secondary));
                                    continue;
                                }
                                if ui.radio(chosen == word, strings::text(key)).clicked() {
                                    events.push(Event::Action(
                                        "plc.import_choose",
                                        json!({ "id": c.id, "choice": word }),
                                    ));
                                }
                            }
                        });
                    }
                    ui.add_space(t.space_2);
                }
            }
            ImportTab::Removed => {
                for r in &card.removed {
                    ui.label(small(r.clone(), t.text_primary));
                }
            }
            ImportTab::Parking => {
                for p in &card.parking {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(small(p.text.clone(), t.text_primary));
                        let why = if p.ambiguous {
                            key!("plc-import-ambiguous")
                        } else {
                            key!("plc-import-unmatched")
                        };
                        ui.label(small(format!("· {}", strings::text(why)), t.text_tertiary));
                        if p.creatable {
                            let mut create = card.choices.get(&p.id).is_some_and(|c| c == "create");
                            if ui
                                .checkbox(&mut create, strings::text(key!("plc-import-create")))
                                .changed()
                            {
                                let mut params = json!({ "id": p.id });
                                if create {
                                    params["choice"] = json!("create");
                                }
                                events.push(Event::Action("plc.import_choose", params));
                            }
                        }
                    });
                }
            }
        });
}
