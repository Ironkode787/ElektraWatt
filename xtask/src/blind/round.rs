// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The usability rounds' additions to a run: hints on request and the proxy questionnaire.
//!
//! **Hints.** A task's `hints.toml` holds one to three hints, graded from where to look to what to do, in the user's
//! words. `start` copies it into `.harness/hints.toml`; `hint <run>` prints the next one and records it in
//! `turns.jsonl` as a turn line of kind `hint`, which the score counts as an assist. A hint opens no turn and needs no
//! note; it is allowed in a visual run.
//!
//! **The questionnaire.** `questionnaire <run> -` takes the ten answers `Q<n> <1-5>` of part A and the four lines
//! `B<n> <choice> <one sentence>` of part B on standard input and writes `questionnaire.json` with the PQ figure: SUS's
//! arithmetic on part A (an odd item scores its answer minus 1, an even item 5 minus its answer, the sum times 2.5, so
//! 0 to 100). The figure is reported without a threshold and is never a SUS score (D2). In a round task (`rounds` in
//! `task.toml`) the hand-back waits for it.

use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::run::{Run, Turn, write_atomic};

/// One hint of `hints.toml`.
#[derive(Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Hint {
    /// The hint, in the user's words.
    pub text: String,
}

/// `hints.toml`.
#[derive(Deserialize, Clone, PartialEq, Eq, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Hints {
    /// The hints in order, from where to look to what to do.
    #[serde(default)]
    pub hint: Vec<Hint>,
}

/// K2: every hint was given.
pub const NO_MORE_HINTS: &str = "harness: no more hints for this task";
/// K3: a task without hints, or a check run.
pub const NO_HINTS: &str = "harness: this task has no hints";
/// K4.
pub const FILED: &str = "questionnaire filed";
/// K5.
pub const FORM: &str = "harness: the questionnaire has the answers Q1 to Q10, each 1 to 5, and B1 to B4, each one of its choices with a sentence";
/// K6.
pub const FILED_ALREADY: &str = "harness: the questionnaire is filed";

/// The choices of part B's questions B1 to B4 (§5.5).
pub const CHOICES: [&[&str]; 4] = [
    &["connect", "ask", "never", "not-seen"],
    &["keep", "choose", "off", "not-seen"],
    &["yes", "partly", "no", "not-seen"],
    &["yes", "partly", "no", "not-seen"],
];

/// The `$schema` of `questionnaire.json`.
pub const QUESTIONNAIRE_SCHEMA: &str = "ew.blindquestionnaire/1";

/// K7: a round task's hand-back before the questionnaire.
pub fn handback_first(run: &Run, id: &str) -> String {
    format!(
        "harness: file the questionnaire before the hand-back ({} questionnaire {id} -)",
        run.blind_command()
    )
}

/// `hint <run>`: the next hint (K1), or why there is none (K2, K3); a hint given is a turn line of kind `hint`.
///
/// # Errors
///
/// A failure of the run folder, or a `hints.toml` that does not parse.
pub fn hint(run: &Run) -> Result<String> {
    let rj = run.load()?;
    if let Some(end) = &rj.end {
        return Ok(super::protocol::ended(end));
    }
    let path = run.path(".harness/hints.toml");
    let hints: Hints = if rj.kind == "check" || !path.is_file() {
        Hints::default()
    } else {
        super::score::read_toml(&path)?
    };
    if hints.hint.is_empty() {
        return Ok(format!("{NO_HINTS}\n"));
    }
    let given = run.turns()?.iter().filter(|t| t.kind == "hint").count();
    let Some(next) = hints.hint.get(given) else {
        return Ok(format!("{NO_MORE_HINTS}\n"));
    };
    let n = u32::try_from(given + 1).unwrap_or(u32::MAX);
    run.push_turn(Turn {
        kind: "hint".to_owned(),
        hint: Some(n),
        ..Turn::default()
    })?;
    Ok(format!(
        "hint {n} of {}: {}\n",
        hints.hint.len(),
        next.text.trim()
    ))
}

/// One answer of part B.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Choice {
    /// One of the question's [`CHOICES`].
    pub choice: String,
    /// The persona's sentence.
    pub sentence: String,
}

/// `questionnaire.json`, format `ew.blindquestionnaire/1`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Questionnaire {
    /// `ew.blindquestionnaire/1`.
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The run.
    pub run: String,
    /// Part A: `Q1` … `Q10` to the answer, 1 (strongly disagree) to 5 (strongly agree).
    pub a: BTreeMap<String, u8>,
    /// Part B: `B1` … `B4` to the choice and its sentence.
    pub b: BTreeMap<String, Choice>,
    /// The PQ figure, 0 to 100 (SUS's arithmetic on part A; not a SUS score).
    pub pq: f64,
}

/// The PQ figure of part A's ten answers in order (`answers[0]` is Q1): SUS's arithmetic, 0 to 100.
pub fn pq_figure(answers: &[u8; 10]) -> f64 {
    let sum: u32 = answers
        .iter()
        .enumerate()
        .map(|(i, &a)| {
            let a = u32::from(a);
            if i % 2 == 0 { a - 1 } else { 5 - a }
        })
        .sum();
    f64::from(sum) * 2.5
}

/// The questionnaire in `text`, or `None` when the form is not filled: every one of Q1 to Q10 once with 1 to 5, every
/// one of B1 to B4 once with one of its choices and a sentence, and no other line (blank lines aside).
pub fn parse(text: &str) -> Option<([u8; 10], BTreeMap<String, Choice>)> {
    let mut a: [Option<u8>; 10] = [None; 10];
    let mut b: BTreeMap<String, Choice> = BTreeMap::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (key, rest) = line.split_once(char::is_whitespace)?;
        let rest = rest.trim();
        if let Some(n) = key.strip_prefix('Q') {
            let n: usize = n.parse().ok().filter(|n| (1..=10).contains(n))?;
            let v: u8 = rest.parse().ok().filter(|v| (1..=5).contains(v))?;
            if a[n - 1].replace(v).is_some() {
                return None;
            }
        } else {
            let n = key.strip_prefix('B')?;
            let n: usize = n.parse().ok().filter(|n| (1..=4).contains(n))?;
            let (choice, sentence) = rest.split_once(char::is_whitespace)?;
            let sentence = sentence.trim();
            if !CHOICES[n - 1].contains(&choice) || sentence.is_empty() {
                return None;
            }
            let answer = Choice {
                choice: choice.to_owned(),
                sentence: sentence.to_owned(),
            };
            if b.insert(format!("B{n}"), answer).is_some() {
                return None;
            }
        }
    }
    let mut answers = [0u8; 10];
    for (slot, v) in answers.iter_mut().zip(a) {
        *slot = v?;
    }
    (b.len() == 4).then_some((answers, b))
}

/// `questionnaire <run> -`: files the questionnaire once (K4), refuses a form not filled (K5) and a second filing (K6).
///
/// # Errors
///
/// A failure of the run folder.
pub fn questionnaire(run: &Run, text: &str) -> Result<String> {
    let rj = run.load()?;
    let path = run.path("questionnaire.json");
    if path.is_file() {
        return Ok(format!("{FILED_ALREADY}\n"));
    }
    let Some((answers, b)) = parse(text) else {
        return Ok(format!("{FORM}\n"));
    };
    let q = Questionnaire {
        schema: QUESTIONNAIRE_SCHEMA.to_owned(),
        run: rj.run,
        a: answers
            .iter()
            .enumerate()
            .map(|(i, &v)| (format!("Q{}", i + 1), v))
            .collect(),
        b,
        pq: pq_figure(&answers),
    };
    let mut json = serde_json::to_string_pretty(&q)?;
    json.push('\n');
    write_atomic(&path, &json)?;
    Ok(format!("{FILED}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::run::Kind;
    use crate::blind::testkit::{Tmp, test_run};

    fn hints_file(run: &Run, n: usize) {
        let body: String = (1..=n)
            .map(|i| format!("[[hint]]\ntext = \"Look {i}.\"\n\n"))
            .collect();
        std::fs::write(run.path(".harness/hints.toml"), body).unwrap();
    }

    /// UR-4: `hint` prints the hints in order (K1), then K2; each hint given is a turn line; K3 without hints and in a
    /// check run; a visual run is served like a text run.
    #[test]
    fn hints_come_in_order_and_are_recorded() {
        let tmp = Tmp::new("round-hint");
        let run = test_run(&tmp);
        hints_file(&run, 2);
        assert_eq!(hint(&run).unwrap(), "hint 1 of 2: Look 1.\n");
        assert_eq!(hint(&run).unwrap(), "hint 2 of 2: Look 2.\n");
        assert_eq!(hint(&run).unwrap(), "harness: no more hints for this task\n");
        let turns = run.turns().unwrap();
        let hints: Vec<Option<u32>> = turns
            .iter()
            .filter(|t| t.kind == "hint")
            .map(|t| t.hint)
            .collect();
        assert_eq!(hints, [Some(1), Some(2)]);
        // A hint is no `do` turn: no note is owed.
        assert_eq!(crate::blind::protocol::derive_state(&turns).pending_note, None);
        // A visual run is served the same way.
        let vtmp = Tmp::new("round-hint-visual");
        let visual = test_run(&vtmp);
        visual.update(|j| j.senses = Some("visual".to_owned())).unwrap();
        hints_file(&visual, 1);
        assert_eq!(hint(&visual).unwrap(), "hint 1 of 1: Look 1.\n");
        // No hints file, an empty one, and a check run: K3, and nothing recorded.
        let ntmp = Tmp::new("round-hint-none");
        let none = test_run(&ntmp);
        assert_eq!(hint(&none).unwrap(), "harness: this task has no hints\n");
        std::fs::write(none.path(".harness/hints.toml"), "").unwrap();
        assert_eq!(hint(&none).unwrap(), "harness: this task has no hints\n");
        let ctmp = Tmp::new("round-hint-check");
        let check = test_run(&ctmp);
        check
            .save(&crate::blind::testkit::new_run_json("c1", Kind::Check))
            .unwrap();
        hints_file(&check, 2);
        assert_eq!(hint(&check).unwrap(), "harness: this task has no hints\n");
        assert!(check.turns().unwrap().is_empty() && none.turns().unwrap().is_empty());
        // An ended run gives no hint.
        run.update(|j| j.end = Some("handback".to_owned())).unwrap();
        assert_eq!(hint(&run).unwrap(), "harness: the run has ended (handback)\n");
    }

    const FILLED: &str = "Q1 4\nQ2 2\nQ3 4\nQ4 2\nQ5 4\nQ6 2\nQ7 4\nQ8 2\nQ9 4\nQ10 2\n\
        B1 ask I wanted to choose when it connects.\nB2 keep The proposals were right.\n\
        B3 partly The arrow showed the page but not the cell.\nB4 not-seen No three wires met.\n";

    /// UR-5: the figure of known answers is the known figure: all 4 and 2 give 75, the best 100, the worst 0.
    #[test]
    fn the_pq_figure_is_sus_arithmetic() {
        assert_eq!(pq_figure(&[4, 2, 4, 2, 4, 2, 4, 2, 4, 2]), 75.0);
        assert_eq!(pq_figure(&[5, 1, 5, 1, 5, 1, 5, 1, 5, 1]), 100.0);
        assert_eq!(pq_figure(&[1, 5, 1, 5, 1, 5, 1, 5, 1, 5]), 0.0);
        assert_eq!(pq_figure(&[3; 10]), 50.0);
    }

    /// The example of `ROUND.md` is a filled form, so the prompt and the parser agree; its figure is 67.5.
    #[test]
    fn the_round_prompts_example_is_a_filled_form() {
        let text = std::fs::read_to_string(crate::workspace_root().join("tools/blind/ROUND.md"))
            .unwrap()
            .replace("\r\n", "\n");
        let (_, rest) = text.split_once("For example:\n\n```\n").unwrap();
        let (example, _) = rest.split_once("```").unwrap();
        let (answers, b) = parse(example).expect("the example parses");
        assert_eq!(pq_figure(&answers), 67.5);
        assert_eq!(b["B4"].choice, "not-seen");
    }

    /// UR-5: the form is filed once (K4) into `questionnaire.json` with its figure; a form not filled is refused with
    /// K5, a second filing with K6.
    #[test]
    fn the_questionnaire_is_filed_once_and_whole() {
        let tmp = Tmp::new("round-q");
        let run = test_run(&tmp);
        let k5 = "harness: the questionnaire has the answers Q1 to Q10, each 1 to 5, and B1 to B4, each one of its \
                  choices with a sentence\n";
        for bad in [
            "",
            &FILLED.replace("Q10 2\n", ""),
            &FILLED.replace("Q3 4", "Q3 6"),
            &FILLED.replace("Q3 4", "Q3 four"),
            &FILLED.replace("Q10 2", "Q1 2"),
            &FILLED.replace("B1 ask", "B1 maybe"),
            &FILLED.replace("B4 not-seen No three wires met.", "B4 no"),
            &format!("{FILLED}B5 yes Extra.\n"),
            &format!("{FILLED}And a remark.\n"),
        ] {
            assert_eq!(questionnaire(&run, bad).unwrap(), k5, "{bad:?}");
        }
        assert!(!run.path("questionnaire.json").exists());
        assert_eq!(questionnaire(&run, FILLED).unwrap(), "questionnaire filed\n");
        let q: Questionnaire =
            serde_json::from_str(&std::fs::read_to_string(run.path("questionnaire.json")).unwrap()).unwrap();
        assert_eq!(q.schema, "ew.blindquestionnaire/1");
        assert_eq!(q.run, "r1");
        assert_eq!(q.pq, 75.0);
        assert_eq!(q.a["Q10"], 2);
        assert_eq!(
            q.b["B3"],
            Choice {
                choice: "partly".to_owned(),
                sentence: "The arrow showed the page but not the cell.".to_owned()
            }
        );
        assert_eq!(
            questionnaire(&run, FILLED).unwrap(),
            "harness: the questionnaire is filed\n"
        );
    }
}
