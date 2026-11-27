// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Surface… (`layout.surface_page`, doc 12 §3.8): the page of one face of a cabinet. It goes to the page that shows
//! the face; where none does it makes, in one undo step, the face's record if it has none (`layout.surface.add`), a
//! panel layout page after the cabinet's plate page (the first free folio after it, as New page proposes) in the plate
//! page's location, its view fitted to the sheet and its legend page. Without a face it asks which, in a chooser of the cabinet's faces with
//! their sizes and pages (`Session::surface_offers`).

use ew_api::output::SurfaceOffer;
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed};
use crate::key;
use crate::strings;

/// The face of the chooser's choice.
const FACE: &str = "surface";

impl ShellModel {
    /// The cabinet Surface… works on: the enclosure of the panel page shown, else the project's only one; `None` with
    /// several and no panel page (the chooser then lists every cabinet's faces by their tags).
    fn surface_cabinet(&self) -> Option<ew_api::output::Uuid> {
        let session = &self.project.as_ref()?.session;
        if let Some(e) = self.focused_page_id().and_then(|p| session.page_enclosure(p)) {
            return Some(e);
        }
        let all = session.surface_offers(None);
        let first = all.first()?.enclosure;
        all.iter().all(|o| o.enclosure == first).then_some(first)
    }

    /// The faces Surface… offers.
    fn surface_offers(&self) -> Vec<SurfaceOffer> {
        self.project
            .as_ref()
            .map(|p| p.session.surface_offers(self.surface_cabinet()))
            .unwrap_or_default()
    }

    /// The face `text` names: a value of [`SurfaceOffer::value`] (a record's ID, `<enclosure>/door`), or a face's
    /// stored word of the cabinet shown (`door`, `door:1`, `bottom`), or of the cabinet named before it, made or not
    /// (`<enclosure>/bottom`, as EW-LAY-018's Place on the bottom names it).
    fn surface_named(&self, text: &str) -> Option<SurfaceOffer> {
        let offers = self.surface_offers();
        let word = |o: &SurfaceOffer| o.value.rsplit('/').next().map(str::to_owned);
        offers
            .iter()
            .find(|o| o.value == text)
            .or_else(|| offers.iter().find(|o| word(o).as_deref() == Some(text)))
            .or_else(|| {
                // A made face by its stored word: its record's kind, of the cabinet named before it.
                let session = &self.project.as_ref()?.session;
                let (cabinet, word) = match text.split_once('/') {
                    Some((e, w)) => (Some(e), w),
                    None => (None, text),
                };
                offers.iter().find(|o| {
                    cabinet.is_none_or(|e| o.enclosure.to_string() == e)
                        && o.record
                            .and_then(|r| session.surface_kind_word(r))
                            .is_some_and(|w| w == word)
                })
            })
            .cloned()
    }

    /// `layout.surface_page {surface?}` (Draw ▸ Enclosure ▾ ▸ Surface…, the panel page's context menu, the HUD's Door,
    /// a refused drop's Open the door): see the module documentation. Without `surface` in the window the chooser
    /// opens; headless it is a missing parameter.
    pub(crate) fn act_layout_surface_page(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "layout.surface_page";
        let given = p
            .get(FACE)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(given) = given else {
            return match self.mode {
                super::Mode::Headless => failed(key!("error-missing-param"), &[FACE]),
                super::Mode::Gui => {
                    let form = self.surface_form();
                    self.open_dialog(super::Dialog::Form(form))
                }
            };
        };
        let Some(offer) = self.surface_named(given) else {
            return self.form_failed(ACTION, strings::format(key!("error-no-surface"), &[given]));
        };
        if let Some(page) = offer.page {
            self.show_page_by_id(&page.to_string());
            return self.layout_form_done(ACTION, Outcome::Done);
        }
        let Some(plate_page) = self
            .surface_offers()
            .into_iter()
            .find(|o| o.plate && o.enclosure == offer.enclosure)
            .and_then(|o| o.page)
        else {
            return self.form_failed(
                ACTION,
                strings::text(key!("reason-surface-no-plate-page")).to_owned(),
            );
        };
        let words = strings::render(&offer.words);
        // The record, the page, its view and its legend: one undo step.
        let mut steps = 0usize;
        let surface = match offer.record {
            Some(r) => r,
            None => {
                let (enclosure, face) = offer.value.split_once('/').unwrap_or_default();
                let (kind, door) = match face.split_once(':') {
                    Some((k, d)) => (k, d.parse::<u16>().ok()),
                    None => (face, None),
                };
                let mut params = json!({"enclosure": enclosure, "kind": kind});
                if let Some(d) = door {
                    params["door"] = json!(d);
                }
                let before: Vec<_> = self
                    .project
                    .as_ref()
                    .map(|p| {
                        p.session
                            .surface_choices()
                            .into_iter()
                            .map(|(id, _)| id)
                            .collect()
                    })
                    .unwrap_or_default();
                let out = self.host_create("layout.surface.add", params);
                if out != Outcome::Done {
                    return self.layout_form_done(ACTION, out);
                }
                steps += 1;
                let made = self.project.as_ref().and_then(|p| {
                    p.session
                        .surface_choices()
                        .into_iter()
                        .map(|(id, _)| id)
                        .find(|id| !before.contains(id))
                });
                let Some(made) = made else {
                    return self.layout_form_done(ACTION, Outcome::refused(key!("reason-no-project")));
                };
                made
            }
        };
        let (folio, _) = self.new_page_proposal(Some(&format!("p:{plate_page}")), Some("panel_layout"));
        let mut page = Map::new();
        page.insert("folio".into(), Value::String(folio));
        page.insert("type".into(), Value::String("panel_layout".into()));
        page.insert("title".into(), Value::String(words.clone()));
        let out = self.act_page_new(&page);
        if out != Outcome::Done {
            self.take_back(steps);
            return self.layout_form_done(ACTION, out);
        }
        steps += 1;
        // The page is the cabinet's, as its plate's page is: it takes that page's structure.
        let structure = self.focused_page_id().and_then(|page| {
            self.project
                .as_ref()?
                .session
                .page_structure_from(plate_page, page)
        });
        if let Some(cmd) = structure {
            let out = self.host_create(&cmd.name, cmd.params);
            if out != Outcome::Done {
                self.take_back(steps);
                return self.layout_form_done(ACTION, out);
            }
            steps += 1;
        }
        let shown = self
            .focused_page_id()
            .is_some_and(|page| self.show_surface(page, surface));
        if !shown {
            self.take_back(steps);
            return self.layout_form_done(ACTION, Outcome::refused(key!("reason-no-panel-page")));
        }
        steps += 1;
        // The steps fold into the first; the legend folds itself into the view's.
        if let Some(p) = &mut self.project {
            for _ in 1..steps {
                p.session.fold_last_step();
            }
        }
        self.add_view_legend();
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.notify(
            strings::format(key!("toast-surface-drawn"), &[&words, &folio]),
            true,
        );
        self.layout_form_done(ACTION, Outcome::Done)
    }

    /// Undoes the last `steps` steps of an act that could not finish (all or nothing).
    fn take_back(&mut self, steps: usize) {
        for _ in 0..steps {
            let _ = self.act_undo();
        }
    }

    /// The chooser of Surface…: the cabinet's faces, each "Door · 600 × 800 mm · hinged left · new page", the face
    /// of the page shown first chosen, else the first face without a page.
    fn surface_form(&self) -> super::FormDialog {
        let offers = self.surface_offers();
        let page = self.focused_page_id();
        let shown = offers.iter().find(|o| o.page.is_some() && o.page == page);
        let value = offers
            .iter()
            .find(|o| o.page.is_none())
            .or(shown)
            .or(offers.first())
            .map(|o| o.value.clone())
            .unwrap_or_default();
        let options = offers
            .iter()
            .map(|o| (o.value.clone(), self.offer_words(o)))
            .collect();
        super::FormDialog {
            action: "layout.surface_page",
            title_key: key!("surface-chooser-title"),
            ok_key: key!("surface-chooser-new-page"),
            fields: vec![super::FormField {
                label: None,
                name: FACE.into(),
                label_key: key!("surface-chooser-field"),
                choices: &super::choices::NONE,
                options,
                value,
            }],
            error: None,
            table: None,
        }
    }

    /// A face in the chooser's words: its name, size, hinge and page ("Door · 600 × 800 mm · hinged left · new
    /// page").
    fn offer_words(&self, o: &SurfaceOffer) -> String {
        let mut parts = vec![strings::render(&o.words)];
        parts.extend(o.size.clone());
        if let Some(h) = &o.hinge {
            parts.push(strings::format(
                key!("surface-chooser-hinged"),
                &[&strings::render(h).to_lowercase()],
            ));
        }
        parts.push(match o.page.and_then(|p| self.folio_of(p)) {
            Some(folio) => strings::format(key!("surface-chooser-page"), &[&folio]),
            None => strings::text(key!("surface-chooser-new")).to_owned(),
        });
        parts.join(" \u{b7} ")
    }

    /// The printed folio of the page `id`.
    fn folio_of(&self, id: ew_api::output::Uuid) -> Option<String> {
        self.page_rows()
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.folio.clone())
    }

    /// The Primary of the Surface… chooser for the face chosen: "Go to page 902" for a face a page shows, else "Draw
    /// on a new page" (the form's own); and the line under the field.
    pub fn surface_choice(&self) -> Option<(Option<String>, String)> {
        let Some(super::Dialog::Form(f)) = &self.dialog else {
            return None;
        };
        if f.action != "layout.surface_page" {
            return None;
        }
        let value = &f.fields.iter().find(|x| x.name == FACE)?.value;
        let offer = self.surface_offers().into_iter().find(|o| &o.value == value)?;
        Some(match offer.page.and_then(|p| self.folio_of(p)) {
            Some(folio) => (
                Some(strings::format(key!("surface-chooser-go"), &[&folio])),
                strings::format(key!("surface-chooser-shown"), &[&folio]),
            ),
            None => (None, strings::text(key!("surface-chooser-line")).to_owned()),
        })
    }

    /// The chooser's title: "Which surface of -UC1?", or "Which surface?" over several cabinets.
    pub fn surface_chooser_title(&self) -> String {
        let tag = self
            .surface_cabinet()
            .and_then(|e| self.project.as_ref()?.session.enclosure_tag(e))
            .filter(|t| !t.is_empty());
        match tag {
            Some(t) => strings::format(key!("surface-chooser-title-of"), &[&t]),
            None => strings::text(key!("surface-chooser-title")).to_owned(),
        }
    }

    /// The panel page's context menu's surface entries: one per face of the cabinet shown, each `layout.surface_page`
    /// with its face, named "Surface: Door".
    pub fn surface_menu(&self) -> Vec<(String, Value)> {
        if self.focused_page_id().is_none_or(|p| {
            self.project
                .as_ref()
                .is_none_or(|pr| pr.session.page_enclosure(p).is_none())
        }) {
            return Vec::new();
        }
        self.surface_offers()
            .into_iter()
            .map(|o| {
                (
                    strings::format(key!("canvas-surface-entry"), &[&strings::render(&o.words)]),
                    json!({ FACE: o.value }),
                )
            })
            .collect()
    }
}
