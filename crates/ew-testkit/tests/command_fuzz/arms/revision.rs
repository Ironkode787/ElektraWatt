// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for revisions.

use super::*;

pub(super) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The greatest major number among the labels of the form `V<major>.<minor>`.
pub(super) fn max_major(live: &Live) -> u32 {
    live.revisions
        .iter()
        .filter_map(|r| r.label.strip_prefix('V')?.split('.').next()?.parse().ok())
        .max()
        .unwrap_or(0)
}

/// A label no revision has, above every major number (EW-REL-004: an open revision's label follows every label).
pub(super) fn fresh_label(live: &Live, rng: &mut SplitMix64) -> String {
    format!(
        "V{}.{}",
        max_major(live) + 1 + u32::try_from(rng.below(3)).unwrap_or(0),
        rng.below(5)
    )
}

pub(super) fn role(rng: &mut SplitMix64) -> &'static str {
    [
        "customer",
        "end_user",
        "panel_builder",
        "author",
        "checker",
        "engineer",
    ][rng.below(6)]
}

/// The oracle of lane A's R5: the title block's `revision.current.label` follows the records. With an open
/// revision, the open one is the current one (it wins over released ones), so its label is the printed one.
/// The title block is read cold over the snapshot; a profile that does not bind the path prints nothing, which
/// is no violation.
pub(super) fn title_block_follows(after: &ew_derive::Q) -> Result<(), String> {
    let open: Vec<_> = after
        .table::<ew_schema::Revision>()
        .iter()
        .filter(|(_, r)| r.is_open())
        .map(|(_, r)| r.label.as_str().to_owned())
        .collect();
    let Some(page) = after.pages().ids().next() else {
        return Ok(());
    };
    let tb = after.get::<ew_derive::TitleBlockValues>(ew_derive::PageRef::Drawn(page));
    let printed = match tb.get("revision.current.label") {
        Some(ew_derive::TbValue::Text(t)) => Some(t.clone()),
        _ => None,
    };
    match (open.as_slice(), printed) {
        ([label], Some(printed)) if *label != printed => Err(format!(
            "the title block prints the revision `{printed}`, the open revision is `{label}`"
        )),
        _ => Ok(()),
    }
}

/// A `V0.<minor>` label no revision has, below the open revision's label (EW-REL-004: a historic released revision
/// precedes the open one); `None` when the open label is not above `V0.*` or the pick is taken.
pub(super) fn history_label(live: &Live, rng: &mut SplitMix64) -> Option<String> {
    let open = live.revisions.iter().find(|r| r.state == "open")?;
    let major: u32 = open.label.strip_prefix('V')?.split('.').next()?.parse().ok()?;
    let label = format!("V0.{}", 1 + rng.below(500));
    (major >= 1 && live.revisions.iter().all(|r| r.label != label)).then_some(label)
}

pub(super) fn revision_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let label = fresh_label(live, rng);
    let open = live.revisions.iter().any(|r| r.state == "open");
    let create = |p: Value| Step::cmd("revision.create", p);
    match rng.below(15) {
        // Prevented (EW-REL-004): a label out of the `V{major}.{minor}` format, in either state.
        12 => {
            let state = *rng.pick(&["open", "released"])?;
            let bad = *rng.pick(&["2", "V2", "v3.0", "V01.0", "Rev 4"])?;
            Some(create(json!({"label": bad, "state": state})))
        }
        // Prevented (EW-REL-004): a historic released revision above the open one.
        13 if open && !live.any_release => Some(create(json!({"label": label, "state": "released"}))),
        // The command refuses (D4, FT-402): a checker on an open revision.
        14 if !open => Some(create(json!({"label": label, "checker": "checker"}))),
        // The command refuses: a label another revision has (EW-INV-017).
        0 => {
            let r = rng.pick(&live.revisions)?;
            Some(create(json!({"label": r.label, "state": "released"})))
        }
        // The command refuses: the checker is the author's party (EW-INV-018).
        1 => Some(create(
            json!({"label": label, "state": "released", "author": "engineer", "checker": "engineer"}),
        )),
        // The command refuses: a second open revision (EW-INV-016).
        2 if open => Some(create(json!({"label": label}))),
        // The command refuses: a historic released revision once a release record exists.
        3 if live.any_release => Some(create(json!({"label": label, "state": "released"}))),
        // The command refuses: two notes on one page (EW-INV-019).
        4 => {
            let page = rng.pick(&live.pages)?;
            Some(create(
                json!({"label": label, "state": "released", "page_notes": [{"page": page}, {"page": page}]}),
            ))
        }
        // Refused: a note on a page that does not exist.
        5 => Some(
            create(json!({"label": label, "state": "released", "page_notes": [{"page": rng.uuid()}]}))
                .refuse(),
        ),
        _ => {
            // The open revision when there is none; else a historic released one below it, unless a release record
            // exists.
            let (state, label) = if !open {
                ("open", label)
            } else if !live.any_release {
                ("released", history_label(live, rng)?)
            } else {
                return None;
            };
            let mut p = json!({"label": label, "state": state, "description": rng.word(10)});
            if rng.chance(2) {
                p["date"] = json!(format!(
                    "20{}-0{}-1{}",
                    20 + rng.below(6),
                    1 + rng.below(9),
                    rng.below(9)
                ));
            }
            if rng.chance(2) {
                // The checker of an open revision is approval's (D4): a historic released revision only.
                let (a, b) = (role(rng), role(rng));
                p["author"] = json!(a);
                if a != b && state == "released" {
                    p["checker"] = json!(b);
                }
            }
            if rng.chance(2) {
                let n = 1 + rng.below(2);
                let mut pages: Vec<Value> = (0..n).filter_map(|_| rng.pick(&live.pages).cloned()).collect();
                pages.sort_by_key(Value::to_string);
                pages.dedup();
                p["page_notes"] = Value::Array(pages.iter().map(|g| json!({"page": g})).collect());
            }
            Some(
                create(p)
                    .counts(&[(Table::Revisions, 1)])
                    .check(|_, after| title_block_follows(after)),
            )
        }
    }
}

pub fn revision_fields_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let one = |target: &Value, path: &str, value: Value| {
        Step::cmd(
            "fields.set",
            json!({"kind": "revision", "targets": [target], "path": path, "value": value}),
        )
    };
    // A released or withdrawn revision is immutable (FR-REV-009, EW-INV-020).
    if rng.chance(6) {
        let closed: Vec<_> = live.revisions.iter().filter(|r| r.state != "open").collect();
        let r = *rng.pick(&closed)?;
        let path = *rng.pick(&["label", "date", "description", "page_notes", "author"])?;
        return Some(one(&r.id, path, json!("X")));
    }
    let r = live.revisions.iter().find(|r| r.state == "open")?;
    let id = r.id.clone();
    let record = move |q: &ew_derive::Q| {
        typed::<ew_schema::Revision>(&id).and_then(|i| q.table::<ew_schema::Revision>().get(i).cloned())
    };
    match rng.below(14) {
        // The command refuses: the state is not settable.
        0 => Some(one(&r.id, "state", json!("released"))),
        // The command refuses: two notes on one page (EW-INV-019).
        1 => {
            let p = rng.pick(&live.pages)?;
            Some(one(&r.id, "page_notes", json!([{"page": p}, {"page": p}])))
        }
        // The command refuses: a note takes no member besides `page` and `description`.
        2 => {
            let p = rng.pick(&live.pages)?;
            Some(one(&r.id, "page_notes", json!([{"page": p, "bogus": 1}])))
        }
        // Refused: a note on a page that does not exist.
        3 => Some(one(&r.id, "page_notes", json!([{"page": rng.uuid()}])).refuse()),
        // The command refuses: a label another revision has (EW-INV-017).
        4 => {
            let other = live
                .revisions
                .iter()
                .find(|o| o.id != r.id && o.state != "withdrawn")?;
            Some(one(&r.id, "label", json!(other.label)))
        }
        // The command refuses: the author's party as the checker (EW-INV-018).
        5 => {
            let author = r.author.clone()?;
            Some(one(&r.id, "checker", json!(author)))
        }
        6 => {
            let date = format!("20{}-0{}-2{}", 20 + rng.below(6), 1 + rng.below(9), rng.below(8));
            let want = json!(date);
            Some(one(&r.id, "date", want.clone()).check(move |_, after| {
                let have = record(after).and_then(|r| serde_json::to_value(r.date).ok());
                (have == Some(want.clone()))
                    .then_some(())
                    .ok_or_else(|| format!("date is {have:?}, expected {want}"))
            }))
        }
        7 => Some(one(&r.id, "description", json!(rng.word(12)))),
        8 => Some(one(&r.id, "date", Value::Null).check(move |_, after| {
            record(after)
                .is_some_and(|r| r.date.is_none())
                .then_some(())
                .ok_or_else(|| "the date is still set after null".to_owned())
        })),
        9 | 10 => {
            let n = rng.below(3);
            let mut pages: Vec<Value> = (0..n).filter_map(|_| rng.pick(&live.pages).cloned()).collect();
            pages.sort_by_key(Value::to_string);
            pages.dedup();
            let notes = Value::Array(
                pages
                    .iter()
                    .map(|g| json!({"page": g, "description": rng.word(6)}))
                    .collect(),
            );
            Some(one(&r.id, "page_notes", notes).check(move |_, after| {
                let have: Vec<Value> = record(after)
                    .map(|r| r.page_notes.iter().map(|n| id_json(n.page)).collect())
                    .unwrap_or_default();
                (have == pages)
                    .then_some(())
                    .ok_or_else(|| format!("the notes are on {have:?}, expected {pages:?}"))
            }))
        }
        // The command refuses (D4, FT-402): the roles are set by `revision.set_author` and by approval, never by fields.set.
        11 => Some(one(&r.id, "author", json!(role(rng)))),
        12 => Some(one(&r.id, "checker", json!(role(rng)))),
        // A new label above every other (the order of labels is the release gate's rule); the title block
        // prints it.
        _ => {
            let label = fresh_label(live, rng);
            let want = label.clone();
            Some(one(&r.id, "label", json!(label)).check(move |_, after| {
                let have = record(after).map(|r| r.label.as_str().to_owned());
                if have.as_deref() != Some(want.as_str()) {
                    return Err(format!("label is {have:?}, expected {want}"));
                }
                title_block_follows(after)
            }))
        }
    }
}

/// `revision.set_author` (WP-S1.18 D4; `HumanOnly`, the fuzz session's origin is the GUI): the open revision's
/// author, never the checker's party; a released or withdrawn revision and a missing one are refused.
pub(super) fn revision_set_author(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let step = |revision: &Value, role: &str| {
        Step::cmd("revision.set_author", json!({"revision": revision, "role": role}))
    };
    match rng.below(6) {
        0 => {
            let closed: Vec<_> = live.revisions.iter().filter(|r| r.state != "open").collect();
            let r = *rng.pick(&closed)?;
            Some(step(&r.id, role(rng)))
        }
        1 => Some(step(&json!(rng.uuid()), role(rng)).refuse()),
        _ => {
            let r = live.revisions.iter().find(|r| r.state == "open")?;
            let new = role(rng);
            if r.checker.as_deref() == Some(new) {
                return Some(step(&r.id, new));
            }
            let id = r.id.clone();
            let want = json!(new);
            Some(step(&r.id, new).check(move |_, after| {
                let have = typed::<ew_schema::Revision>(&id)
                    .and_then(|i| after.table::<ew_schema::Revision>().get(i).cloned())
                    .and_then(|r| serde_json::to_value(&r.author).ok());
                (have.as_ref() == Some(&want))
                    .then_some(())
                    .ok_or_else(|| format!("the author is {have:?}, expected {want}"))
            }))
        }
    }
}

/// `revision.approve` (WP-S1.18 part 3, D9; `HumanOnly`): the session refuses it on every submit path, from every
/// origin, whatever its parameters; the sign-off is recorded only by `Session::approve`, which runs the release gate on
/// the saved generation first (a fuzz session is never saved).
pub(super) fn revision_approve(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let revision = match rng.below(3) {
        0 => json!(rng.uuid()),
        _ => rng.pick(&live.revisions)?.id.clone(),
    };
    Some(
        Step::cmd(
            "revision.approve",
            json!({"revision": revision, "checker": role(rng), "date": "2025-06-02", "scheme": "pre_delivery",
                "content_digest": "0".repeat(64)}),
        )
        .refuse(),
    )
}

/// `revision.release` (WP-S1.18 part 4, D13; `HumanOnly`, not undoable): the session refuses it on every submit path,
/// from every origin, whatever its parameters; the stamp is written only by `Session::release`, after the gate on the
/// saved generation, the archive and the tag (a fuzz session is never saved).
pub(super) fn revision_release(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let revision = match rng.below(3) {
        0 => json!(rng.uuid()),
        _ => rng.pick(&live.revisions)?.id.clone(),
    };
    let sha = format!("sha256:{}", "0".repeat(64));
    Some(
        Step::cmd(
            "revision.release",
            json!({"revision": revision, "released_on": "2025-06-09", "generation": 2, "content_digest": sha,
                "derivation_profile": "2027.1", "archive": {"name": "V9.9.ewarchive", "sha256": sha}}),
        )
        .refuse(),
    )
}

/// `revision.withdraw` (WP-S1.18 D5; `HumanOnly`): a released revision gets one withdrawal record and stays as it
/// was; an open revision, a withdrawal record, a revision withdrawn already and a blank reason are refused.
pub(super) fn revision_withdraw(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let step = |revision: &Value, reason: &str| {
        Step::cmd(
            "revision.withdraw",
            json!({"revision": revision, "reason": reason}),
        )
    };
    let withdrawn = |id: &Value| live.revisions.iter().any(|w| w.withdraws.as_ref() == Some(id));
    match rng.below(6) {
        0 => {
            let others: Vec<_> = live.revisions.iter().filter(|r| r.state != "released").collect();
            let r = *rng.pick(&others)?;
            Some(step(&r.id, "not a released revision"))
        }
        1 => {
            let r = rng.pick(&live.revisions)?;
            Some(step(&r.id, "   "))
        }
        _ => {
            let released: Vec<_> = live.revisions.iter().filter(|r| r.state == "released").collect();
            let r = *rng.pick(&released)?;
            if withdrawn(&r.id) {
                return Some(step(&r.id, "withdrawn again"));
            }
            let id = r.id.clone();
            let before = typed::<ew_schema::Revision>(&id);
            Some(
                step(&r.id, &format!("withdrawn: {}", rng.word(8)))
                    .counts(&[(Table::Revisions, 1)])
                    .check(move |q_before, after| {
                        let get = |q: &ew_derive::Q| {
                            before.and_then(|i| q.table::<ew_schema::Revision>().get(i).cloned())
                        };
                        (get(q_before) == get(after))
                            .then_some(())
                            .ok_or_else(|| "the withdrawn revision changed".to_owned())
                    }),
            )
        }
    }
}
