// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The friction log (WP-S6.13 part 4, visual spec §7.1): `friction <run> --severity <s> --category <c> --wanted "…"
//! --expected "…" --saw "…" [--frame <name>] [--region <x>,<y>,<w>,<h>] [--workaround "…"]` appends one entry to the
//! run's `friction.jsonl` (`ew.blindfriction/1`), in a text run as in a visual one. The harness fills in the number, the
//! turn, the step and (in a visual run) the newest frame. The log is append-only; it needs no note and opens no turn.
//!
//! The category `missing-feature` ("the product cannot do this", the owner's rule of 2026-10-29) stands beside the six
//! categories of the window: a checker does not count it against the window.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::run::{Run, append};

/// The `$schema` of the header of `friction.jsonl`.
pub const FRICTION_SCHEMA: &str = "ew.blindfriction/1";
/// The most entries of one run.
pub const FRICTION_CAP: usize = 300;
/// The most characters of one text of an entry.
pub const TEXT_MAX: usize = 400;
/// The severities, from the worst.
pub const SEVERITIES: [&str; 4] = ["blocker", "confusing", "annoying", "cosmetic"];
/// The six categories of the window, then `missing-feature`, which is the product's and not the window's.
pub const CATEGORIES: [&str; 7] = [
    "discoverability",
    "feedback",
    "layout-legibility",
    "wording",
    "flow-dead-end",
    "behaviour-bug",
    "missing-feature",
];

/// One entry of `friction.jsonl`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Entry {
    /// The entry's number, from 1.
    pub n: u32,
    /// The turn it was logged in (0 before the first `do`).
    pub turn: u32,
    /// The steps (a visual run: the inputs) the agent had sent when it logged the entry.
    pub step: u32,
    /// The frame it refers to (default the newest); `null` in a text run.
    pub frame: Option<String>,
    /// A region of that frame in frame pixels, `[x, y, w, h]`.
    pub region: Option<[u32; 4]>,
    /// What the tester wanted to do.
    pub wanted: String,
    /// What it expected.
    pub expected: String,
    /// What it saw or did instead.
    pub saw: String,
    /// `blocker`, `confusing`, `annoying` or `cosmetic`.
    pub severity: String,
    /// One of [`CATEGORIES`].
    pub category: String,
    /// How it got round it, if it did.
    pub workaround: Option<String>,
}

/// What the command line gave.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FrictionArgs {
    /// `--severity`.
    pub severity: Option<String>,
    /// `--category`.
    pub category: Option<String>,
    /// `--wanted`.
    pub wanted: Option<String>,
    /// `--expected`.
    pub expected: Option<String>,
    /// `--saw`.
    pub saw: Option<String>,
    /// `--frame`.
    pub frame: Option<String>,
    /// `--region`.
    pub region: Option<String>,
    /// `--workaround`.
    pub workaround: Option<String>,
}

/// The entries of `friction.jsonl` after its header.
///
/// # Errors
///
/// A line that is not an entry.
pub fn entries(run: &Run) -> Result<Vec<Entry>> {
    let Ok(text) = std::fs::read_to_string(run.path("friction.jsonl")) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line)?;
        if v.get("$schema").is_none() {
            out.push(serde_json::from_value(v)?);
        }
    }
    Ok(out)
}

fn region(text: &str) -> Option<[u32; 4]> {
    let v: Vec<u32> = text.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    let [x, y, w, h] = v[..] else { return None };
    let (fw, fh) = (
        super::visual::FRAME_WIDTH as u32,
        super::visual::FRAME_HEIGHT as u32,
    );
    let inside = |a: u32, b: u32, max: u32| a.checked_add(b).is_some_and(|end| end <= max);
    (text.split(',').count() == 4 && w > 0 && h > 0 && inside(x, w, fw) && inside(y, h, fh))
        .then_some([x, y, w, h])
}

/// `friction`: checks the entry and appends it; returns what the command prints.
///
/// # Errors
///
/// A failure of the run folder.
pub fn friction(run: &Run, a: &FrictionArgs) -> Result<String> {
    let refuse = |t: &str| Ok(format!("{t}\n"));
    let rj = run.load()?;
    if rj.handed_back {
        return refuse("harness: the hand-back is filed");
    }
    let (Some(severity), Some(category), Some(wanted), Some(expected), Some(saw)) =
        (&a.severity, &a.category, &a.wanted, &a.expected, &a.saw)
    else {
        return refuse("harness: friction needs severity, category, wanted, expected and saw");
    };
    if [wanted, expected, saw].iter().any(|t| t.trim().is_empty()) {
        return refuse("harness: friction needs severity, category, wanted, expected and saw");
    }
    if !SEVERITIES.contains(&severity.as_str()) {
        return refuse(&format!("harness: unknown severity {severity}"));
    }
    if !CATEGORIES.contains(&category.as_str()) {
        return refuse(&format!("harness: unknown category {category}"));
    }
    let texts = [Some(wanted), Some(expected), Some(saw), a.workaround.as_ref()];
    if texts.iter().flatten().any(|t| t.chars().count() > TEXT_MAX) {
        return refuse(&format!(
            "harness: a friction text has at most {TEXT_MAX} characters"
        ));
    }
    let so_far = entries(run)?;
    if so_far.len() >= FRICTION_CAP {
        return refuse("harness: friction cap");
    }
    let turns = run.turns()?;
    let turn = super::protocol::derive_state(&turns).turns;
    let visual = rj.visual();
    if !visual && (a.frame.is_some() || a.region.is_some()) {
        return refuse("harness: a text run has no frames; the step is the reference");
    }
    let frame = if visual {
        let known: Vec<&str> = turns.iter().filter_map(|t| t.frame.as_deref()).collect();
        match &a.frame {
            Some(f) => {
                let name = if f.ends_with(".png") {
                    f.clone()
                } else {
                    format!("{f}.png")
                };
                let name = name.trim_start_matches("frames/").to_owned();
                if name != "t0000.png" && !known.contains(&name.as_str()) {
                    return refuse(&format!("harness: no frame {f} in this run"));
                }
                Some(name)
            }
            // The newest turn's frame, never a look's or a zoom's: a region is in the pixels of a normal frame.
            None => Some(
                turns
                    .iter()
                    .filter(|t| t.kind == "do")
                    .filter_map(|t| t.frame.as_deref())
                    .next_back()
                    .map_or_else(|| "t0000.png".to_owned(), str::to_owned),
            ),
        }
    } else {
        None
    };
    let region = match &a.region {
        None => None,
        Some(r) => match region(r) {
            Some(r) => Some(r),
            None => {
                return refuse("harness: --region is x,y,w,h in frame pixels inside the frame");
            }
        },
    };
    let entry = Entry {
        n: u32::try_from(so_far.len() + 1).unwrap_or(u32::MAX),
        turn,
        step: rj.counts.steps,
        frame,
        region,
        wanted: wanted.trim().to_owned(),
        expected: expected.trim().to_owned(),
        saw: saw.trim().to_owned(),
        severity: severity.clone(),
        category: category.clone(),
        workaround: a.workaround.as_ref().map(|w| w.trim().to_owned()),
    };
    let path = run.path("friction.jsonl");
    if !path.is_file() {
        append(&path, &format!("{{\"$schema\":\"{FRICTION_SCHEMA}\"}}\n"))?;
    }
    append(&path, &format!("{}\n", serde_json::to_string(&entry)?))?;
    Ok(format!("friction {} logged\n", entry.n))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::blind::protocol::tests::{fixture, s};
    use crate::blind::run::{Kind, Senses};

    fn args(severity: &str, category: &str) -> FrictionArgs {
        FrictionArgs {
            severity: Some(severity.to_owned()),
            category: Some(category.to_owned()),
            wanted: Some("place a contact".to_owned()),
            expected: Some("a contact on the page".to_owned()),
            saw: Some("nothing happened".to_owned()),
            ..FrictionArgs::default()
        }
    }

    /// BV8: the required members, the two lists, the length and the frame rules are checked with their texts.
    #[test]
    fn bv8_an_entry_is_checked() {
        let f = fixture(Kind::Free);
        let mut a = args("blocker", "discoverability");
        a.saw = None;
        assert_eq!(
            friction(&f.run, &a).unwrap(),
            "harness: friction needs severity, category, wanted, expected and saw\n"
        );
        a.saw = Some("  ".to_owned());
        assert_eq!(
            friction(&f.run, &a).unwrap(),
            "harness: friction needs severity, category, wanted, expected and saw\n"
        );
        assert_eq!(
            friction(&f.run, &args("fatal", "wording")).unwrap(),
            "harness: unknown severity fatal\n"
        );
        assert_eq!(
            friction(&f.run, &args("cosmetic", "colour")).unwrap(),
            "harness: unknown category colour\n"
        );
        let mut long = args("annoying", "wording");
        long.workaround = Some("w".repeat(TEXT_MAX + 1));
        assert_eq!(
            friction(&f.run, &long).unwrap(),
            "harness: a friction text has at most 400 characters\n"
        );
        let mut framed = args("annoying", "wording");
        framed.frame = Some("t0001".to_owned());
        assert_eq!(
            friction(&f.run, &framed).unwrap(),
            "harness: a text run has no frames; the step is the reference\n"
        );
        assert!(
            !f.run.path("friction.jsonl").exists(),
            "nothing refused is logged"
        );
    }

    /// BV8: entries are numbered, carry the turn and the step, are appended behind one header, need no note, and the
    /// category `missing-feature` is accepted; after the hand-back nothing more is logged.
    #[test]
    fn bv8_a_text_run_logs_numbered_entries_behind_a_header() {
        let mut f = fixture(Kind::Free);
        assert_eq!(
            friction(&f.run, &args("blocker", "missing-feature")).unwrap(),
            "friction 1 logged\n"
        );
        f.run_do(&[s("help.about", json!({})), s("help.about", json!({}))]);
        let mut b = args("cosmetic", "layout-legibility");
        b.workaround = Some(" used the menu ".to_owned());
        assert_eq!(friction(&f.run, &b).unwrap(), "friction 2 logged\n");
        let text = std::fs::read_to_string(f.run.path("friction.jsonl")).unwrap();
        assert!(
            text.starts_with("{\"$schema\":\"ew.blindfriction/1\"}\n"),
            "{text}"
        );
        assert_eq!(text.lines().count(), 3);
        let e = entries(&f.run).unwrap();
        assert_eq!((e[0].n, e[0].turn, e[0].step), (1, 0, 0));
        assert_eq!((e[1].n, e[1].turn, e[1].step), (2, 1, 2));
        assert_eq!(e[1].frame, None);
        assert_eq!(e[1].workaround.as_deref(), Some("used the menu"));
        // A friction entry is not a note: the turn's note is still pending.
        assert!(
            f.run_do(&[s("help.about", json!({}))])
                .starts_with("harness: note required")
        );
        f.run.update(|j| j.handed_back = true).unwrap();
        assert_eq!(
            friction(&f.run, &args("blocker", "wording")).unwrap(),
            "harness: the hand-back is filed\n"
        );
        assert_eq!(entries(&f.run).unwrap().len(), 2);
    }

    /// BV8: in a visual run the frame defaults to the newest, a named frame must exist, a region lies inside the
    /// frame, and the cap holds.
    #[test]
    fn bv8_a_visual_run_refers_to_frames() {
        let mut f = fixture(Kind::Free);
        f.run
            .update(|j| {
                j.senses = Some("visual".to_owned());
                j.caps = Kind::Free.caps_for(Senses::Visual);
            })
            .unwrap();
        friction(&f.run, &args("confusing", "feedback")).unwrap();
        let input = json!({"input": "click", "x": 100, "y": 50})
            .as_object()
            .cloned()
            .unwrap();
        crate::blind::visual::do_inputs(&f.run, &mut f.backend, &[input]).unwrap();
        // A zoom is newer, but the default frame is the turn's: a region is in a normal frame's pixels.
        crate::blind::visual::look(&f.run, &mut f.backend, Some([0, 0, 100, 100])).unwrap();
        let mut a = args("confusing", "feedback");
        a.region = Some("10,20,30,40".to_owned());
        friction(&f.run, &a).unwrap();
        let mut named = args("cosmetic", "wording");
        named.frame = Some("frames/t0000.png".to_owned());
        friction(&f.run, &named).unwrap();
        let e = entries(&f.run).unwrap();
        assert_eq!(e[0].frame.as_deref(), Some("t0000.png"));
        assert_eq!(e[1].frame.as_deref(), Some("t0001.png"));
        assert_eq!(e[1].region, Some([10, 20, 30, 40]));
        assert_eq!((e[1].turn, e[1].step), (1, 1));
        assert_eq!(e[2].frame.as_deref(), Some("t0000.png"));
        named.frame = Some("t0009".to_owned());
        assert_eq!(
            friction(&f.run, &named).unwrap(),
            "harness: no frame t0009 in this run\n"
        );
        for bad in ["1500,0,100,10", "0,0,0,10", "1,2,3", "a,b,c,d"] {
            let mut r = args("cosmetic", "wording");
            r.region = Some(bad.to_owned());
            assert_eq!(
                friction(&f.run, &r).unwrap(),
                "harness: --region is x,y,w,h in frame pixels inside the frame\n",
                "{bad}"
            );
        }
        let filler = serde_json::to_string(&Entry::default()).unwrap();
        let mut text = std::fs::read_to_string(f.run.path("friction.jsonl")).unwrap();
        for _ in 3..FRICTION_CAP {
            text.push_str(&filler);
            text.push('\n');
        }
        std::fs::write(f.run.path("friction.jsonl"), text).unwrap();
        assert_eq!(
            friction(&f.run, &args("cosmetic", "wording")).unwrap(),
            "harness: friction cap\n"
        );
    }
}
