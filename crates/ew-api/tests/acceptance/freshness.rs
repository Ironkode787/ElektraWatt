// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! AC-16 (doc 20 §21; doc 18 §3.4): freshness. The command log `variants/ac16-freshness.ewcmd` makes four
//! edits (rename T55, move -102F2 by one cell, renumber folio 103 to 104, change the 24 V DC colour). Each runs
//! as its own log of one line, on one session with the memo oracle on, and must
//!
//! - commit exactly one transaction (one undo step),
//! - change exactly the outputs it affects: the printed tags, the cross-reference texts, the BOM and cable
//!   exports, the report rows and the text of every page, compared before and after (each snapshot is read
//!   through a cold `Q`, so the comparison does not depend on the memo store), and equal, read through the
//!   session's memoising `Q`, to the cold snapshot byte for byte, and
//! - leave `ew verify-derived` (`ew_api::verify::verify_derived`) without a difference.
//!
//! Afterwards four undos restore the records the session started with (`undo ∘ apply = id`).
//!
//! The run is computed once (about 45 seconds in a debug build, most of it the oracle) and read by seven report
//! entries.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::str::FromStr;

use ew_api::{BomMode, Export, Session};
use ew_derive::{DeviceTag, Oracle, PageOrder, PropSource, Q, RepAnchorCell, XrefTextOf};
use ew_ids::Id;
use ew_reports::export::export_csv;
use ew_schema::Representation;

use crate::golden_common::{self, all_props, repo, rows_csv, rows_of};
use crate::golden_files::text_csv;

use super::checks::runs;
use super::runner::Ctx;

/// The representation of -102F2 on folio 102 (the key `representations|-102F2`).
const REP_102F2: &str = "37a8c75f-bf41-529c-9953-a094db94e9ac";

/// The memoised reads the oracle re-runs cold: one in this many (the debug-build rate of doc 18 §3.4).
const ORACLE_ONE_IN: u32 = 8;

/// The outcome of one edit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    /// What the edit does, for the report.
    pub name: &'static str,
    /// What differs from the expectation; empty when the step holds.
    pub problems: Vec<String>,
    /// What the step changed, one line, for a passing entry.
    pub note: String,
}

/// What `verify_derived` found after the edits.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Verified {
    /// The keys read per pass and the families.
    pub keys: usize,
    /// The families.
    pub families: usize,
    /// Differences and oracle mismatches, over every edit.
    pub problems: Vec<String>,
}

/// The whole run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Run {
    /// The script, its commits and the undo of all of them.
    pub script: Result<String, String>,
    /// The four edits in script order.
    pub steps: Vec<Step>,
    /// The `verify_derived` runs.
    pub verified: Verified,
}

/// Everything an edit can change that the entry compares.
#[derive(PartialEq)]
struct Snap {
    tags: BTreeMap<String, String>,
    xrefs: BTreeMap<Id<Representation>, String>,
    bom: String,
    cables: String,
    wires: String,
    potentials: String,
    texts: BTreeMap<String, String>,
    labels: Vec<String>,
}

/// The printed outputs of the snapshot `q` reads: through a cold `Q` what the edit really produced, through the
/// session's memoising `Q` what the exports and the GUI would print.
fn snap(c: &Ctx, q: &Q) -> Result<Snap, String> {
    let mut tags = BTreeMap::new();
    for id in q.devices().ids() {
        let tag = q.get::<DeviceTag>(id);
        tags.insert(
            id.to_string(),
            tag.as_ref().as_ref().map(|t| t.short()).unwrap_or_default(),
        );
    }
    let mut xrefs = BTreeMap::new();
    for id in q.reps().ids() {
        let t = q.get::<XrefTextOf>(id);
        if !t.text.is_empty() {
            xrefs.insert(id, t.text.clone());
        }
    }
    let export = |what| export_csv(q, what).map_err(|e| e.to_string());
    Ok(Snap {
        tags,
        xrefs,
        bom: export(Export::Bom(BomMode::Component))?,
        cables: export(Export::Cables)?,
        wires: rows_csv(q, &rows_of(q, &c.batch, "907")),
        potentials: rows_csv(q, &rows_of(q, &c.batch, "909")),
        // The shipped export job's page scenes: the memoised layers of the session after the edit.
        texts: q
            .get::<PageOrder>(())
            .entries
            .iter()
            .map(|e| (e.folio.clone(), text_csv(&ew_api::output::page_scene(q, e))))
            .collect(),
        labels: q
            .get::<PageOrder>(())
            .entries
            .iter()
            .map(|e| e.folio.clone())
            .collect(),
    })
}

/// The outputs of `a` that differ from `b`.
fn snap_differences(a: &Snap, b: &Snap) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (name, differs) in [
        ("tags", a.tags != b.tags),
        ("cross-reference texts", a.xrefs != b.xrefs),
        ("the BOM export", a.bom != b.bom),
        ("the cable export", a.cables != b.cables),
        ("the wire rows", a.wires != b.wires),
        ("the potential rows", a.potentials != b.potentials),
        ("page texts", a.texts != b.texts),
        ("page labels", a.labels != b.labels),
    ] {
        if differs {
            out.push(name);
        }
    }
    out
}

/// Whether a text CSV prints the folio 103 or a tag of its strips: "-X1031", "103-B14", "Next: 103", "Page: 103".
fn mentions_103(csv: &str) -> bool {
    runs(csv).iter().any(|(_, text, _)| {
        text.contains("X103")
            || text
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| word == "103")
    })
}

/// The folios whose text differs, in folio order of `after`, then the folios that are gone.
fn changed_folios(before: &Snap, after: &Snap) -> Vec<String> {
    let mut out: Vec<String> = after
        .texts
        .iter()
        .filter(|(f, t)| before.texts.get(*f) != Some(*t))
        .map(|(f, _)| f.clone())
        .collect();
    out.extend(
        before
            .texts
            .keys()
            .filter(|f| !after.texts.contains_key(*f))
            .cloned(),
    );
    out
}

/// The folios whose text contains `needle`.
fn folios_with(snap: &Snap, needle: &str) -> BTreeSet<String> {
    snap.texts
        .iter()
        .filter(|(_, t)| t.contains(needle))
        .map(|(f, _)| f.clone())
        .collect()
}

fn tag_changes(before: &Snap, after: &Snap) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = before
        .tags
        .iter()
        .filter(|(id, old)| after.tags.get(*id) != Some(*old))
        .map(|(id, old)| (old.clone(), after.tags.get(id).cloned().unwrap_or_default()))
        .collect();
    v.sort();
    v
}

fn xref_changes(before: &Snap, after: &Snap) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = before
        .xrefs
        .iter()
        .filter(|(id, old)| after.xrefs.get(*id) != Some(*old))
        .map(|(id, old)| (old.clone(), after.xrefs.get(id).cloned().unwrap_or_default()))
        .collect();
    v.sort();
    v
}

fn list(items: &BTreeSet<String>) -> String {
    items.iter().cloned().collect::<Vec<_>>().join(", ")
}

/// The command log of one line of the script: its header and the line.
fn single(header: &str, line: &str) -> String {
    format!("{header}\n{line}\n")
}

fn rename(before: &Snap, after: &Snap) -> Step {
    let mut problems = Vec::new();
    let changes = tag_changes(before, after);
    if changes != [("T55".to_owned(), "T57".to_owned())] {
        problems.push(format!("tags changed: {changes:?}"));
    }
    if !xref_changes(before, after).is_empty() {
        problems.push("a cross-reference text changed".to_owned());
    }
    // Every output that printed T55 prints T57 and nothing else of it changes.
    let printed = folios_with(before, "T55");
    let now = folios_with(after, "T57");
    let changed: BTreeSet<String> = changed_folios(before, after).into_iter().collect();
    if printed.is_empty() {
        problems.push("no folio printed T55 before the edit".to_owned());
    }
    if printed != changed {
        problems.push(format!(
            "folios that printed T55: {}; folios that changed: {}",
            list(&printed),
            list(&changed)
        ));
    }
    if printed != now {
        problems.push(format!(
            "T57 is printed on {}, T55 was on {}",
            list(&now),
            list(&printed)
        ));
    }
    if !folios_with(after, "T55").is_empty() {
        problems.push(format!(
            "T55 is still printed on {}",
            list(&folios_with(after, "T55"))
        ));
    }
    let want_bom = before.bom.replace(",T55,", ",T57,");
    if after.bom != want_bom {
        problems.push("the BOM export differs from the old one in anything but the tag".to_owned());
    }
    if after.cables != before.cables {
        problems.push("the cable export changed".to_owned());
    }
    Step {
        name: "rename T55 to T57",
        note: format!(
            "T55 becomes T57 and nothing else changes: the tag, the BOM row and folios {} (the folios that printed it); no folio prints T55",
            list(&printed)
        ),
        problems,
    }
}

fn cell_of(s: &Session, rep: Id<Representation>) -> AnchorCell {
    s.query()
        .get::<RepAnchorCell>(rep)
        .as_ref()
        .as_ref()
        .map(|c| (c.column, c.row))
}

/// An anchor cell `(column, row)`, when the representation has one.
type AnchorCell = Option<(u32, u32)>;

fn moved(before: &Snap, after: &Snap, cells: (AnchorCell, AnchorCell)) -> Step {
    let mut problems = Vec::new();
    match cells {
        (Some((c0, r0)), Some((c1, r1))) if c1 == c0 + 1 && r1 == r0 => {}
        other => problems.push(format!(
            "the anchor cell goes from {:?} to {:?}, not one column right",
            other.0, other.1
        )),
    }
    let changed = changed_folios(before, after);
    if changed != ["102"] {
        problems.push(format!("folios that changed: {changed:?}, expected only 102"));
    }
    if !tag_changes(before, after).is_empty() {
        problems.push("a tag changed".to_owned());
    }
    if !xref_changes(before, after).is_empty() {
        problems.push("a cross-reference text changed".to_owned());
    }
    if after.labels != before.labels {
        problems.push("the page order changed".to_owned());
    }
    Step {
        name: "move -102F2 by one cell",
        note: format!(
            "-102F2 moves from cell {:?} to {:?}; only folio 102 changes; no tag, cross-reference text or page label changes",
            cells.0.unwrap_or_default(),
            cells.1.unwrap_or_default()
        ),
        problems,
    }
}

fn renumber(before: &Snap, after: &Snap) -> Step {
    let mut problems = Vec::new();
    let changes = tag_changes(before, after);
    // The strips of folio 103 (-X1031, -X1032 and their terminals) take the page into their tags.
    let ok = !changes.is_empty()
        && changes
            .iter()
            .all(|(old, new)| old.contains("X103") && *new == old.replacen("X103", "X104", 1));
    if !ok {
        problems.push(format!(
            "tags changed: {:?}",
            changes.iter().take(3).collect::<Vec<_>>()
        ));
    }
    let xs = xref_changes(before, after);
    if xs.is_empty() || !xs.iter().all(|(old, new)| *new == old.replace("103-", "104-")) {
        problems.push(format!(
            "cross-reference changes: {:?}",
            xs.iter().take(3).collect::<Vec<_>>()
        ));
    }
    let want: Vec<String> = before
        .labels
        .iter()
        .map(|l| if l == "103" { "104".to_owned() } else { l.clone() })
        .collect();
    if after.labels != want {
        problems.push("the page order is not the old one with 103 labelled 104".to_owned());
    }
    // Nothing prints the old folio or the old strip tags any more, the titles of the terminal diagrams included
    // ("Terminal Diagram {strip}", resolved at derivation).
    for stale in ["-X103", "103-"] {
        let on = folios_with(after, stale);
        if !on.is_empty() {
            problems.push(format!("`{stale}` is still printed on {}", list(&on)));
        }
    }
    // The folios that change are exactly the ones that printed folio 103 or a tag of its strips, with 103 itself
    // replaced by 104.
    let mut want: BTreeSet<String> = before
        .texts
        .iter()
        .filter(|(_, csv)| mentions_103(csv))
        .map(|(f, _)| f.clone())
        .collect();
    want.insert("104".to_owned());
    want.insert("103".to_owned());
    let got: BTreeSet<String> = changed_folios(before, after).into_iter().collect();
    if got != want {
        problems.push(format!(
            "folios that changed: {}; folios that printed 103 or its tags: {}",
            list(&got),
            list(&want)
        ));
    }
    if !after.texts.contains_key("104") || after.texts.contains_key("103") {
        problems.push("folio 104 is not drawn in place of 103".to_owned());
    }
    if !after.texts.get("104").is_some_and(|t| t.contains("Page: 104")) {
        problems.push("the title block of folio 104 does not print \"Page: 104\"".to_owned());
    }
    // The BOM export follows the tags and folios.
    let want_bom = before
        .bom
        .replace("X103", "X104")
        .replace(",103\n", ",104\n")
        .replace("\"101, 103\"", "\"101, 104\"");
    if after.bom != want_bom {
        problems.push("the BOM export is not the old one with the strips and folio renamed".to_owned());
    }
    if !after.cables.contains("-X1041") || after.cables.contains("-X1031") {
        problems.push("the cable export does not print the renamed strips".to_owned());
    }
    let changed = changed_folios(before, after);
    Step {
        name: "renumber folio 103 to 104",
        note: format!(
            "folio 103 becomes 104 in the page order; {} tags (-X103n to -X104n) and {} cross-reference texts follow; the BOM and cable exports and folios {} change; nothing prints the old folio",
            changes.len(),
            xs.len(),
            changed.join(", ")
        ),
        problems,
    }
}

fn colour(
    before: &Snap,
    after: &Snap,
    wires: (
        &BTreeMap<String, golden_common::Props>,
        &BTreeMap<String, golden_common::Props>,
    ),
) -> Step {
    let mut problems = Vec::new();
    let mut taken = 0;
    for (w, b) in wires.0 {
        let a = &wires.1[w];
        if matches!(&b.1, PropSource::ColourRow { row: 0, .. }) {
            if a.0 == "RD" {
                taken += 1;
            } else {
                problems.push(format!("wire {w} kept {}", a.0));
            }
        } else if a != b {
            problems.push(format!("wire {w} changed without taking row 0"));
        }
    }
    if taken == 0 {
        problems.push("no wire takes the colour of row 0".to_owned());
    }
    let cover = |s: &Snap| s.texts.get("1").map_or(0, |t| t.matches("Dark Blue").count());
    let red = |s: &Snap| s.texts.get("1").map_or(0, |t| t.matches(",Red,").count());
    if cover(after) + 1 != cover(before) || red(after) != red(before) + 1 {
        problems.push("folio 1 does not print the new colour row in place of the old one".to_owned());
    }
    if after.wires == before.wires {
        problems.push("the wire rows of folio 907 are the same".to_owned());
    }
    if after.potentials == before.potentials {
        problems.push("the potential rows of folio 909 are the same".to_owned());
    }
    let changed = changed_folios(before, after);
    let want = BTreeSet::from(["1", "907", "907.1", "909"].map(str::to_owned));
    let got: BTreeSet<String> = changed.iter().cloned().collect();
    if got != want {
        problems.push(format!(
            "folios that changed: {}, expected {}",
            list(&got),
            list(&want)
        ));
    }
    if !tag_changes(before, after).is_empty() || !xref_changes(before, after).is_empty() {
        problems.push("a tag or cross-reference text changed".to_owned());
    }
    if after.bom != before.bom || after.cables != before.cables || after.labels != before.labels {
        problems.push("the BOM, the cable export or the page order changed".to_owned());
    }
    Step {
        name: "change the 24 V DC colour",
        note: format!(
            "{taken} wires take RD and nothing else changes; folios {} change (folio 1 prints the new row of the colour table); no tag, text, export or page label changes",
            list(&got)
        ),
        problems,
    }
}

fn compute(c: &Ctx) -> Result<Run, String> {
    let path = repo().join("golden/J-E-2416/variants/ac16-freshness.ewcmd");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("variants/ac16-freshness.ewcmd: {e}"))?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or("variants/ac16-freshness.ewcmd is empty")?
        .to_owned();
    let edits: Vec<&str> = lines.filter(|l| !l.trim().is_empty()).collect();
    if edits.len() != 4 {
        return Err(format!("the script has {} edits, AC-16 names 4", edits.len()));
    }
    // The golden model with its profiles and the oracle on: one memoised read in `ORACLE_ONE_IN` is also
    // computed cold and compared, at every commit and every read.
    let mut s = golden_common::session_oracle_with(&c.batch, |p, _| golden_common::with_profiles(p));
    if let Some(memo) = s.memo() {
        memo.set_oracle(Oracle::OneIn(
            NonZeroU32::new(ORACLE_ONE_IN).expect("n is not zero"),
        ));
    }
    let records_before = ew_commands::rows::model_lines(&s.world().model).map_err(|e| format!("{e:?}"))?;
    let mut steps = Vec::new();
    let mut verify = Verified {
        keys: 0,
        families: 0,
        problems: Vec::new(),
    };
    let mut script_problems = Vec::new();
    let mut before = snap(c, &s.query_over(&s.world().model))?;
    let mut wires_before = all_props(&golden_common::plain(&s));
    for (i, line) in edits.iter().enumerate() {
        let history = s.history().entries().len();
        let cell_before = cell_of(&s, id_102f2());
        let committed = golden_common::run_log(&mut s, &single(&header, line), "ac16-freshness.ewcmd")
            .map_err(|e| format!("edit {}: {e}", i + 1))?;
        if committed != 1 || s.history().entries().len() != history + 1 {
            script_problems.push(format!(
                "edit {} committed {committed} lines and {} undo steps",
                i + 1,
                s.history().entries().len() - history
            ));
        }
        let model = s.world().model.clone();
        let after = snap(c, &s.query_over(&model))?;
        // The same outputs through the session's memoising `Q` (what the exports and the GUI print) equal the
        // cold ones byte for byte. The oracle is paused: it would only slow the reads, `verify_derived` runs it.
        if let Some(memo) = s.memo() {
            memo.set_oracle(Oracle::Off);
        }
        let memoised = snap(c, &s.query())?;
        if let Some(memo) = s.memo() {
            memo.set_oracle(Oracle::OneIn(
                NonZeroU32::new(ORACLE_ONE_IN).expect("n is not zero"),
            ));
        }
        let differing = snap_differences(&memoised, &after);
        if !differing.is_empty() {
            verify.problems.push(format!(
                "after edit {}: the memoised outputs differ from the cold ones in {}",
                i + 1,
                differing.join(", ")
            ));
        }
        let wires_after = all_props(&golden_common::plain(&s));
        steps.push(match i {
            0 => rename(&before, &after),
            1 => moved(&before, &after, (cell_before, cell_of(&s, id_102f2()))),
            2 => renumber(&before, &after),
            _ => colour(&before, &after, (&wires_before, &wires_after)),
        });
        let report = ew_api::verify::verify_derived(&s).map_err(|e| e.to_string())?;
        verify.keys = report.total_keys();
        verify.families = report.keys.len();
        for d in &report.differences {
            verify.problems.push(format!("after edit {}: {d}", i + 1));
        }
        for m in &report.oracle {
            verify
                .problems
                .push(format!("after edit {}: oracle {m:?}", i + 1));
        }
        before = after;
        wires_before = wires_after;
    }
    let mut undone = 0;
    while s.undo().is_ok() {
        undone += 1;
    }
    let restored =
        ew_commands::rows::model_lines(&s.world().model).map_err(|e| format!("{e:?}"))? == records_before;
    if undone != 4 || !restored {
        script_problems.push(format!(
            "{undone} undos restore {}the records",
            if restored { "" } else { "not " }
        ));
    }
    let mismatches = s.take_oracle_mismatches().len();
    if mismatches > 0 {
        verify
            .problems
            .push(format!("{mismatches} oracle mismatches while undoing"));
    }
    let script = if script_problems.is_empty() {
        Ok(format!(
            "4 edits, 4 commits (one undo step each), 4 undos restore the records byte for byte; the session ran with the oracle on (one memoised read in {ORACLE_ONE_IN} re-run cold)"
        ))
    } else {
        Err(script_problems.join("; "))
    };
    Ok(Run {
        script,
        steps,
        verified: verify,
    })
}

fn id_102f2() -> Id<Representation> {
    Id::from_str(REP_102F2).expect("the representation ID of -102F2")
}

/// The run, computed once per [`Ctx`].
pub fn run(c: &Ctx) -> &Result<Run, String> {
    c.ac16.get_or_init(|| compute(c))
}
