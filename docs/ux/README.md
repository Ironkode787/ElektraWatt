# Usability rounds

The usability rounds R0 and R1 of [doc 16 §19](../design/16-ux-ui.md#19-usability-test-plan) run as **blind-persona
rounds**, not as sessions with people (ID-1331 (5)): rounds of `cargo xtask blind` runs on doc 16's tasks, in which an
agent plays a persona that mirrors a participant. The specification is
[WP-S2.12](../plan/specs/WP-S2.12-usability-rounds.md); the harness is described in
[`tools/blind/README.md`](../../tools/blind/README.md).

## What the rounds are, and what they are not

**The rounds are a proxy for the usability evidence of people.** Each attempt is a model's attempt in a persona's
role, not a person's. A **completion** is an outcome that the product's exports decide (the task was done or it was
not), so it is counted and set beside doc 16's lines as a reading. A **questionnaire answer** is a model's attitude
with no norm, so its figure gets no threshold. Neither is ever written as NFR-UX-01's completion rate or NFR-UX-02's
SUS score. Every report opens with this statement and lists, under "What the proxy cannot give", the requirements that
measure people and what the round gave instead.

No person takes part: there is no moderator, no consent form and no recording of a person (NFR-PRIV-10 does not
apply). The participants are codes: the persona names.

## The method

- **The tasks.** Each of doc 16 §19.4's tasks T1 to T11, and the ADR-0007 probe A7, is a brief folder
  `tools/blind/tasks/brief-ut-<t>-<name>/`: the task in the user's words, its start project (a copy of Golden A, or of
  GP-UT: Golden A without -102F1 and its four wires), an acceptance list as the oracle, one to three hints and a
  reference solution that `cargo xtask blind check-brief` replays on the build. Each attempt is a fresh run on a fresh
  copy.
- **The personas.** R0 runs `qet-veteran` (the pilot user's role) and `qet-user` (a colleague); R1 runs
  `eplan-designer`, `eplan-maintainer` and `qet-user`, with `qet-veteran` beside `qet-user` on T6.
- **The senses.** The picture-only visual runs are the counted attempts; one text run per task serves the pairing rule
  (friction only in the visual run is a window finding, friction in both a product finding).
- **The decision.** `stop` decides every item of the acceptance list mechanically and calls the attempt
  **unassisted** (every hard item met, no hint), **with help** (met after a hint) or **not completed** (an item unmet,
  or the run ended by a cap or a quit rule). Errors, undo and assists are counts, never times.
- **Hints.** The persona asks for the next hint with `{blind} hint`, when it would ask the person beside it or when
  three turns brought it no closer. Reading the user guide counts as unassisted; T6 runs without the guide.
- **The proxy questionnaire.** Before its hand-back the persona answers ten statements in this project's own wording
  (SUS's alternating form, not SUS's text) and four questions on ADR-0007's defaults. The PQ figure is SUS's
  arithmetic on the ten answers, 0 to 100, reported per attempt, persona and round without a threshold.
- **Severity.** The findings are rated on Nielsen's 0 to 4 scale; severe (3 or 4) findings are fixed with a kittest
  flow by their owning packages (R0) or as WP-S6.08's fix rounds (R1), and re-run.

## The files

| Path | What it holds |
|---|---|
| `docs/ux/README.md` | This page |
| `docs/ux/R0-report.md` | The formative round on the development build (written after R0's runs) |
| `docs/ux/R1-report.md` | The summative round on the first release candidate, with ADR-0007's validation record (written after R1's runs) |
| `tools/blind/rounds/<round>/<run>/` | Each run's evidence: `run.json`, `drafter.md`, `handback.md`, `friction.jsonl`, `questionnaire.json`, `checker.md` when present, `out/score.json` and `out/report.md`, with the run folder's path written `«run»`; no frame, no transcript, no PDF |

A run's ID is `<round>-<task>-<persona>-<v or t>`, for example `r1-t3-eplan-maintainer-v` (R1, task T3, the EPLAN
maintenance persona, visual).
