// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for the project settings.

use super::*;

pub(super) fn project_supply_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(8) {
        // The command refuses: a member that is not settable in v0.
        0 => {
            let path = *rng.pick(&["phases", "u_nom", "earthing", "origin", "origin.ik_max", "bogus"])?;
            Some(Step::cmd("project.supply.set", json!({"path": path, "value": 1})))
        }
        // The command refuses: a value the member does not take.
        1 => Some(Step::cmd(
            "project.supply.set",
            json!({"path": "origin.source", "value": 5}),
        )),
        // Refused: an external reference that does not exist.
        2 => Some(
            Step::cmd(
                "project.supply.set",
                json!({"path": "origin.source", "value": rng.uuid()}),
            )
            .refuse(),
        ),
        _ => {
            if !live.has_supply {
                return None;
            }
            let (value, expect) = if rng.chance(4) {
                (Value::Null, None)
            } else {
                let e = rng.pick(&live.ext_refs)?.clone();
                (e.clone(), Some(e))
            };
            Some(
                Step::cmd(
                    "project.supply.set",
                    json!({"path": "origin.source", "value": value}),
                )
                .check(move |_, after| {
                    let have = after
                        .project()
                        .supply
                        .as_ref()
                        .and_then(|s| s.origin.as_ref())
                        .and_then(|o| o.source)
                        .map(id_json);
                    (have == expect)
                        .then_some(())
                        .ok_or_else(|| format!("origin.source is {have:?}, not {expect:?}"))
                }),
            )
        }
    }
}

pub(super) fn project_settings_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let member = *rng.pick(&["numbering", "colour_profile", "size_profile", "xref_format"])?;
    match rng.below(8) {
        // The command refuses: not a settable member.
        0 => {
            let m = *rng.pick(&["identifier_set", "frames", "wire_default", "bogus"])?;
            Some(Step::cmd(
                "project.settings.set",
                json!({"member": m, "value": {}}),
            ))
        }
        // Removing a member lets the derivation profile's default apply.
        1 | 2 => Some(Step::cmd(
            "project.settings.set",
            json!({"member": member, "value": null}),
        )),
        // Setting a member to the value it has changes nothing.
        _ => {
            let value = live
                .fixture
                .initial_settings
                .get(member)
                .or_else(|| live.settings.get(member))?
                .clone();
            Some(Step::cmd(
                "project.settings.set",
                json!({"member": member, "value": value}),
            ))
        }
    }
}

/// The stored `checks` settings member (`None` when absent).
pub(super) fn checks_member(q: &ew_derive::Q) -> Option<Value> {
    q.project().settings.extra.get("checks").cloned()
}

pub(super) fn project_checks_set(_live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(6) {
        // The command refuses: a scheme that does not ship.
        0 => {
            let member = *rng.pick(&["active_scheme", "release_scheme"])?;
            let name = *rng.pick(&["lenient", "strict", "pre_delivery", "Full"])?;
            Some(Step::cmd("project.checks.set", json!({ member: name })))
        }
        // Refused: an unknown member.
        1 => Some(Step::cmd("project.checks.set", json!({"scheme": "full"})).refuse()),
        _ => {
            let mut params = serde_json::Map::new();
            for member in ["active_scheme", "release_scheme"] {
                if rng.chance(2) {
                    params.insert(
                        member.into(),
                        json!(*rng.pick(ew_commands::commands::SHIPPED_CHECK_SCHEMES)?),
                    );
                }
            }
            let expect = params.clone();
            Some(
                Step::cmd("project.checks.set", Value::Object(params)).check(move |_, after| {
                    let stored = checks_member(after);
                    for member in ["active_scheme", "release_scheme"] {
                        let have = stored.as_ref().and_then(|c| c.get(member));
                        if have != expect.get(member) {
                            return Err(format!(
                                "checks.{member} is {have:?}, not {:?}",
                                expect.get(member)
                            ));
                        }
                    }
                    Ok(())
                }),
            )
        }
    }
}

/// The neutral name of the first panel-builder party, and the stored notice.
pub(super) fn company_of(q: &ew_derive::Q) -> (Option<String>, Option<Value>) {
    let project = q.project();
    let name = project
        .parties
        .iter()
        .find(|p| p.role == ew_schema::PartyRole::PanelBuilder)
        .map(|p| p.name.as_neutral().unwrap_or_default().to_owned());
    let notice = project
        .extra
        .get("ew.company")
        .and_then(|c| c.get("notice"))
        .cloned();
    (name, notice)
}

/// A placeholder logo of `w × h` user units (no company's artwork).
pub(super) fn placeholder_logo(w: u32, h: u32) -> Vec<u8> {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}"/></svg>"#)
        .into_bytes()
}

/// `project.assets.purge`: a non-undoable, `HumanOnly` marker whose file side runs in the session (the
/// fuzzer's session submits as the GUI, which `HumanOnly` accepts). Every purgeable asset
/// (`{}`) commits as a marker when the session is saved (a session without a project directory has nothing to purge)
/// and is refused while it has unsaved changes; the model never changes. A listed hash that is no purgeable asset, and
/// a malformed one, are refused.
pub(super) fn project_assets_purge(_live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    // The checks of an `Either` step run only when the revision moved, which a purge must never do.
    let unchanged = |_: &ew_derive::Q, _: &ew_derive::Q| Err("the purge moved the revision".to_owned());
    match rng.below(4) {
        0 => {
            let hex: String = (0..32).map(|_| format!("{:02x}", rng.below(256))).collect();
            Some(Step::cmd(
                "project.assets.purge",
                json!({"assets": [format!("sha256:{hex}")]}),
            ))
        }
        1 => Some(Step::cmd(
            "project.assets.purge",
            json!({"assets": [rng.word(8)]}),
        )),
        _ => Some(Step::cmd("project.assets.purge", json!({})).check(unchanged)),
    }
}

pub(super) fn project_company_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(11) {
        // Refused: no member at all, or a value of the wrong type.
        0 => Some(Step::cmd("project.company.set", json!({}))),
        1 => Some(Step::cmd("project.company.set", json!({"name": 5})).refuse()),
        // A logo joins the asset store and the company member names it; empty removes it.
        8 | 9 => {
            let (w, h) = (1 + rng.below(400) as u32, 1 + rng.below(400) as u32);
            let bytes = placeholder_logo(w, h);
            let remove = rng.chance(4);
            let text = if remove {
                String::new()
            } else {
                ew_schema::base64_encode(&bytes)
            };
            Some(
                Step::cmd("project.company.set", json!({"logo": text})).check(move |_, after| {
                    let logo = ew_schema::company_logo(&after.project().extra);
                    match (remove, logo) {
                        (true, None) => Ok(()),
                        (false, Some(r)) if r.size == Some((w, h)) => Ok(()),
                        (_, other) => Err(format!("the logo is {other:?} (removed: {remove})")),
                    }
                }),
            )
        }
        // The command refuses: a logo that is no image.
        10 => Some(Step::cmd(
            "project.company.set",
            json!({"logo": ew_schema::base64_encode(rng.word(12).as_bytes())}),
        )),
        // The command refuses: the personal-data flag alone, without a panel-builder party to carry it.
        2 if !live.has_panel_builder => Some(Step::cmd(
            "project.company.set",
            json!({"personal_data": rng.chance(2)}),
        )),
        _ => {
            let name = if rng.chance(4) {
                String::new()
            } else {
                format!("{} Panels", rng.word(6))
            };
            let notice = if rng.chance(3) {
                String::new()
            } else {
                format!("Property of {}.", rng.word(8))
            };
            let mut params = json!({"name": name, "notice": notice});
            if !name.is_empty() && rng.chance(2) {
                params["personal_data"] = json!(rng.chance(2));
            }
            let party_before = live.has_panel_builder;
            Some(Step::cmd("project.company.set", params).check(move |_, after| {
                let (have_name, have_notice) = company_of(after);
                let want_name = (party_before || !name.is_empty()).then(|| name.clone());
                if have_name != want_name {
                    return Err(format!("the panel builder is {have_name:?}, not {want_name:?}"));
                }
                let want_notice = (!notice.is_empty()).then(|| json!(notice));
                if have_notice != want_notice {
                    return Err(format!("the notice is {have_notice:?}, not {want_notice:?}"));
                }
                Ok(())
            }))
        }
    }
}

/// The page types a frame-defaults entry may name (every type but `report`).
pub(super) fn project_drawing_set(_live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(8) {
        // The command refuses: no member given.
        0 => Some(Step::cmd("project.drawing.set", json!({}))),
        // Refused: an unknown value, a value of the wrong type, an unknown member.
        1 => Some(Step::cmd("project.drawing.set", json!({"crossings": "loops"})).refuse()),
        // `marker_mode`, an unknown value refused, a known one or `null` accepted.
        7 if rng.chance(2) => Some(Step::cmd("project.drawing.set", json!({"marker_mode": "some"})).refuse()),
        7 => {
            let mode = [
                json!("all"),
                json!("deviations"),
                json!("explicit"),
                json!("none"),
                Value::Null,
            ][rng.below(5)]
            .clone();
            Some(Step::cmd("project.drawing.set", json!({"marker_mode": mode})))
        }
        2 => Some(Step::cmd("project.drawing.set", json!({"crossings": rng.below(9)})).refuse()),
        3 => Some(
            Step::cmd(
                "project.drawing.set",
                json!({"crossings": "hops", "shadows": true}),
            )
            .refuse(),
        ),
        _ => {
            let value = [json!("plain"), json!("hops"), Value::Null][rng.below(3)].clone();
            let want = value.clone();
            Some(
                Step::cmd("project.drawing.set", json!({"crossings": value})).check(move |_, after| {
                    let drawing = after.project().settings.extra.get("drawing").cloned();
                    let crossings = drawing.as_ref().and_then(|d| d.get("crossings")).cloned();
                    match (&want, &crossings, &drawing) {
                        (Value::Null, None, None) => Ok(()),
                        (Value::Null, None, Some(Value::Object(m))) if !m.is_empty() => Ok(()),
                        (w, Some(c), _) if !w.is_null() && w == c => Ok(()),
                        _ => Err(format!(
                            "the drawing settings are {drawing:?} after setting `crossings` to {want}"
                        )),
                    }
                }),
            )
        }
    }
}

/// The page types of `project.frames.set`'s entries, read from its schema (the command refuses `report`: report
/// pages are generated).
pub(super) fn frame_page_types() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| registry_strings("project.frames.set", &["per_page_type", "page_type"]))
}

pub(super) fn project_frames_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("project.frames.set", p);
    match rng.below(10) {
        // Refused: a form that does not exist.
        0 => Some(cmd(json!({"default": rng.uuid()})).refuse()),
        // The command refuses: an entry for report pages, or the same page type twice.
        1 => {
            let form = rng.pick(&live.frame_forms)?.clone();
            Some(cmd(
                json!({"per_page_type": [{"page_type": "report", "form": form}]}),
            ))
        }
        2 => {
            let form = rng.pick(&live.frame_forms)?.clone();
            let t = *rng.pick(frame_page_types())?;
            Some(cmd(json!({"per_page_type": [
                {"page_type": t, "form": form},
                {"page_type": t, "form": form},
            ]})))
        }
        // The command refuses: a form that is no frame (a title block).
        3 => {
            let form = rng.pick(&live.other_forms)?.clone();
            Some(cmd(json!({"default": form})))
        }
        // Removing the frame defaults lets the built-in frames apply.
        4 => Some(
            cmd(json!({})).check(|_, after| match after.project().settings.extra.get("frames") {
                None => Ok(()),
                Some(v) => Err(format!("the frame defaults are still {v}")),
            }),
        ),
        _ => {
            let default = rng.pick(&live.frame_forms)?.clone();
            let mut entries = Vec::new();
            let mut types: Vec<&str> = frame_page_types().to_vec();
            for _ in 0..rng.below(3) {
                let i = rng.below(types.len());
                let t = types.remove(i);
                entries.push(json!({"page_type": t, "form": rng.pick(&live.frame_forms)?.clone()}));
            }
            let expect_default = default.clone();
            let expect_len = entries.len();
            Some(
                cmd(json!({"default": default, "per_page_type": entries})).check(move |_, after| {
                    let frames = after.get::<ew_derive::FrameSettings>(());
                    let default = frames.default.map(id_json);
                    if default.as_ref() != Some(&expect_default) || frames.per_page_type.len() != expect_len {
                        return Err(format!(
                            "the frame defaults read back as {default:?} with {} entries",
                            frames.per_page_type.len()
                        ));
                    }
                    Ok(())
                }),
            )
        }
    }
}

pub(super) fn project_languages_set(_live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("project.languages.set", p);
    match rng.below(8) {
        // Refused: translations without a source, the source as a translation, an unknown format.
        0 => Some(cmd(json!({"translations": ["de"]}))),
        1 => Some(cmd(json!({"source": "en", "translations": ["en"]}))),
        2 => Some(cmd(json!({"date": "mm/dd/yyyy"})).refuse()),
        _ => {
            let mut params = serde_json::Map::new();
            if !rng.chance(4) {
                params.insert("source".into(), json!(*rng.pick(&["en", "nl", "de"])?));
                if rng.chance(3) {
                    params.insert("translations".into(), json!(["fr"]));
                }
            }
            if !rng.chance(4) {
                params.insert("date".into(), json!(*rng.pick(&["dd_mm_yyyy", "iso8601"])?));
            }
            if !rng.chance(4) {
                params.insert("decimal_separator".into(), json!(*rng.pick(&["point", "comma"])?));
            }
            let expect = params.clone();
            Some(cmd(Value::Object(params)).check(move |_, after| {
                let project = after.project();
                let source = project.languages.as_ref().map(|l| json!(l.source));
                let date = project.settings.formatting.date.as_ref().map(|d| json!(d));
                let separator = project
                    .settings
                    .formatting
                    .decimal_separator
                    .as_ref()
                    .map(|d| json!(d));
                let want = (
                    expect.get("source").cloned(),
                    expect.get("date").cloned(),
                    expect.get("decimal_separator").cloned(),
                );
                if (source.clone(), date.clone(), separator.clone()) != want {
                    return Err(format!(
                        "languages and formats read back as {:?}, not {want:?}",
                        (source, date, separator)
                    ));
                }
                Ok(())
            }))
        }
    }
}
