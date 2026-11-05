# WP-S2.12: the usability rounds R0 and R1 as blind-persona rounds (closed specification)

**Status:** cycle 52 (decision log ID-1399, ID-1436): FT-1375 (R0-F1) fixed by FT52a and proven by R0's kittest flow on GP-UT; R1 waits for 0.1.0's first candidate on `release/0.1` (ID-1412), with FT-1486 ruled first · the first half built in cycle 51 (§7 steps 1 to 5 and the R0 run, decision log ID-1361, ID-1365); R1
waits for WP-S6.08's first candidate, with FT-1375 (R0-F1) fixed before it; specified in cycle 50 (documents only), an
M2 package in two halves: the lane's five build steps of §7
(the harness additions, GP-UT, the personas and the round prompt, the task sheets, then R0 with its report and its
fix list routed), built whole by one lane (ID-1281), about 2.05 pw; and R1 on the first release candidate inside
WP-S6.08's release cycle, with its report and re-tests (§7 steps 6 and 7), about 0.65 pw; the orchestrator's runs
about 0.3 pw beside them; the PERT of 3.1 pw unchanged · **Date:** 2026-11-04 · **Work package:**
[WP-S2.12](../02-work-packages.md#wp-s212-usability-rounds-r0-and-r1) · **Decision log:** ID-777, ID-785, ID-1281,
ID-1322, ID-1330, ID-1331, ID-1333 · **Part of:** ElektraWatt plan, specifications

**Purpose.** Doc 16 §19 plans R0 and R1 as moderated sessions with people: the pilot user and a colleague for R0, two
EPLAN users and one QET user for R1, a moderator, consent forms, think-aloud and a SUS questionnaire. The owner ruled
that R0 and R1 run as **blind-persona runs**, the method accepted for G-PP (ID-777), not as sessions with people
(ID-1331 (5)). This document translates doc 16 §19 into rounds of `cargo xtask blind` runs on doc 16's tasks: the
personas that mirror the participants, the task sheets with their oracles, the GP-UT fixture, text runs and
picture-only visual runs, the scoring, a proxy questionnaire, the ADR-0007 validation record, the analysis with its
fix list, and the reports that G-M2-20 to G-M2-22 read. **The rounds are a proxy for the usability evidence of
people.** Where a requirement measures people, this document says so, says what the proxy gives instead, and never
reports a proxy figure as the requirement's figure (§3, D2). It is written against the code at `b022248d` (§2.1). Its
size is an estimate, not a commitment (ID-1210); every time is not measured (owner's rule). Where this document and a
design document differ, this document is the narrower and wins for the lane; the design documents stay the target
and are updated at the design pass (§13). A choice the lane meets that this document does not settle is escalated,
never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [The rounds](#5-the-rounds)
6. [The texts](#6-the-texts)
7. [Build order](#7-build-order)
8. [Acceptance](#8-acceptance)
9. [Neighbours, files owned, what a lane may not decide, and risks](#9-neighbours-files-owned-what-a-lane-may-not-decide-and-risks)
10. [Size, routing and the hand-back point](#10-size-routing-and-the-hand-back-point)
11. [Follow-ups, proposed](#11-follow-ups-proposed)
12. [Open questions](#12-open-questions)
13. [Documents to update](#13-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package:
- **The materials.** `tools/blind/tasks/` holds one task folder for each of doc 16 §19.4's tasks T1 to T11 and one
  ADR-0007 probe, A7, each with its acceptance list as the oracle where the task has a checkable result, its hints and
  its reference solution; T1 to T8 and A7 pass `check-brief` on the build. GP-UT exists as
  `golden/J-E-2416/variants/gp-ut.ewcmd`. Three new personas mirror R1's participants. A round prompt gives every
  persona the hint command and the proxy questionnaire.
- **The harness** starts a task on a temporary copy of Golden A or of GP-UT, records hints as assists, takes the
  questionnaire, and decides each acceptance item of a run mechanically at `stop`.
- **R0** has run on the development build with two QET personas (the pilot user's role and a colleague) on T1, T2, T8
  and A7, visual and text; its report `docs/ux/R0-report.md` exists, and its severe fixes are routed to their owning
  packages and closed before R1 (G-M2-22).
- **R1** has run on the first release candidate with two EPLAN personas and one QET persona on T1 to T5, T7, T8 and
  A7, T6 for the QET personas; its report `docs/ux/R1-report.md` holds the proxy completion, the proxy questionnaire's
  figure, the findings by Nielsen severity and ADR-0007's validation record; its severe findings went to WP-S6.08's
  D16 and passed their re-runs on the next candidate (G-M2-20, G-M2-21).

**Non-goals** (each with its package):
- **No session with people**, no moderator, no recruitment, no consent form, no recording of a person (ID-1331).
- **No fix of a finding.** Severe findings go to their owning packages (R0) and to WP-S6.08's D16 (R1); the rest
  become follow-up rows through the design pass.
- **No R2.** R2 is WP-S2.16's at 1.0, and its method is not ruled (ID-1331 rules R0 and R1 only). The task sheets of
  T9 to T11 are written here because R2 runs R1's tasks; they are not run here.
- **No ruling on doc 16 §19.1's other open choices** (variant letters, pending findings, the dark sheet): the reports
  note what the runs show about them; their documents decide.
- **No change to an existing prompt, persona, help file, task, recording or self-test** beyond §9.2's list; no change
  to the gate file, ADR-0007 or doc 16 (the design pass writes them, §13).
- **No new crate, no new dependency**, no change to `deny.toml`, `supply-chain/` or `Cargo.lock`; no change to Golden
  A's committed project (`golden/J-E-2416.ew`).
- **No timing** of any kind: no minutes, no clock read as evidence (owner's rule).

## 2. Inputs and the code as it is

| Input | What the lane takes from it |
|---|---|
| [UX/UI](../../design/16-ux-ui.md) §19 (`:1858-1943`), `:93`, open questions 5, 7, 9 (`:2066`, `:2072`, `:2077`) | The rounds, screening, setup, GP-UT, tasks T1 to T11 with their success criteria and limits, the metrics, Nielsen's scale, fixes and reports |
| [Non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-UX-01, -02, -04, -06, -09 (`:355-363`), NFR-PRIV-10 (`:234`) | §3 |
| [Gates](../03-gates.md) CP-05, CP-08 (`:337`, `:340`), GP-10 (`:491`), G-M2-20 to G-M2-22 (`:951-953`), the blind-drafter note (`:958`) | The evidence the gates read; §13's wording |
| [ADR-0007](../../adr/0007-explicit-connections-and-autoconnect.md) "Validation before M2" (`:83-95`), amendment 2 (`:55-62`) | The questions the validation record answers |
| [Golden project](../../design/20-golden-project.md) §2 (`:93-97`), §3.1 (`:236-239`) | GP-UT's definition and its place |
| [Governance](../../design/19-open-source-governance.md) §3.7 (`:481-496`) | The consent template, which no round of this package uses |
| [WP-S6.13](WP-S6.13-blind-drafter-harness.md) §1 (`:77`), D3, D4, D13, D16 (`:176-189`), §5.11 (`:635-662`), §8 (`:716-735`); [visual runs](WP-S6.13-visual-playtest-runs.md) §4.2, §6.1, §6.2, §7.1 to §7.5, §9 | The method, the procedure, the caps, the brief kind, the friction log, the pairing rule, the gate rule |
| [WP-S6.08](WP-S6.08-release-stabilisation.md) D16 (`:136`), §7 (`:174-216`), §10.4 R2 (`:314`), §13 O8 (`:375-377`) | Where R1 runs and where its severe fixes go |
| [WP-S2.11](WP-S2.11-gui-test-suite.md) §12 FP-2, §13 O2, as ruled by ID-1332 | FT-713's friction 3 (the first zoom at the floor) for R0 |
| Decisions ID-341, ID-612, ID-700, ID-768, ID-776, ID-777, ID-785, ID-810 ([decision log](../06-decision-log.md)); ID-1281, ID-1322, ID-1330, ID-1331, ID-1332, ID-1333 ([decisions](../decisions.md)) | The method, the models, the run costs seen so far, the rulings |

### 2.1 What exists at `b022248d` (the base contract)

The lane relies on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used for |
|---|---|---|
| WP-S2.12's entry: R0 and R1 with people, consent forms, GP-UT, the analysis and fix list; depends on WP-S2.04 and WP-S0.08; NFR-UX-01, -02, -04, -06, -09, NFR-PRIV-10 | `docs/plan/02-work-packages.md:1884-1897` | §3, §13 |
| WP-S6.08 depends on WP-S2.12 | `docs/plan/02-work-packages.md:3003` | D1 |
| The brief kind: a folder `brief-<name>/` with `task.md`, `task.toml` (`kind`, `grade`, `version`, `turn_budget`, `senses`, `requires`, `areas`; unknown members refused), `acceptance.toml` (`id`, `kind`, `what`, `how`, `query`, `expect`, `infeasible`), a setup-only prelude and `solution.uiscript`; `check-brief` replays the solution in a scratch folder and decides each item by its query or its output; `start --kind brief` refuses a brief that is not runnable and a brief folder without the prefix | `xtask/src/blind/brief.rs:4-18`, `:33-42`, `:47-67`, `:70-90`, `:246`, `:430-621`; `xtask/src/blind/start.rs:162-165` | D4, D7 |
| A brief run's epilogue saves and describes every authored page; nothing decides the acceptance items at `stop` (the checker does) | `xtask/src/blind/stop.rs:286-300` | D7 |
| A check run's project is copied into `work/project` and put back after `work/` is deleted (`resume`, a stateless call); its prelude opens `project` | `xtask/src/blind/start.rs:111-133`; `xtask/src/blind/stop.rs:280-285` | D5 |
| The golden project is read only through temporary copies, its digest taken at `start` and compared at `stop`; `EW_SAMPLE_DIR` points at an empty folder | WP-S6.13 D16 (`:189`); `xtask/src/blind/start.rs:82`; `xtask/src/blind/oracle.rs:31-32`, `:173-181` | D5 |
| `compose`: the drafter's (`BLIND_DRAFTER.md`) or play-tester's (`PLAYTESTER.md`) prompt, the persona, the task sheet and the help (`HELP.md` or `HELP-VISUAL.md`), each file's digest recorded | `xtask/src/blind/compose.rs:104-142` | D11 |
| The caps: a visual brief 250 turns, 2,000 inputs, 300 looks, 50 refusals; a text brief 300 turns, 800 steps, 400 looks, 50 refusals | `xtask/src/blind/run.rs:244-275` | §5.3 |
| `start --no-book` starts a run with an empty book folder | `xtask/src/blind/mod.rs:68`, `:248` | T6 |
| The harness's commands: no hint, no questionnaire, no `report inputs`, no `friction-report` | `xtask/src/blind/mod.rs:46-81` | D8, D11, D14 |
| WP-S6.13 part 5 is not built: no `PLAYCHECKER.md`, no `keyboard-expert` or `reviewer` persona, no friction ledger | `tools/blind/` (the personas `first-timer`, `qet-veteran`, `switcher`) | D14 |
| The personas `qet-veteran` and `switcher` say "you have no mouse here", written for text runs | `tools/blind/personas/qet-veteran.md:21`; `tools/blind/personas/switcher.md:25` | D10 |
| The hand-backs: the drafter's seven headings (heading 5 asks for a QET veteran's time for the set); the play-tester's eight (heading 5 asks for the time in the persona's usual tool) | `tools/blind/BLIND_DRAFTER.md`; `tools/blind/PLAYTESTER.md` | D9 |
| The run procedure is the orchestrator's, "never by an agent"; a lane's tools are Read, Edit, Write, Grep, Glob and Bash, so a lane starts no agent | `tools/blind/README.md:31`; `.claude/agents/lane.md:5` | D3 |
| No `variants/gp-ut.ewcmd` and no `basic-project/` folder; the variant tests of Golden B replay a log on a copy | `golden/J-E-2416/variants/`; `crates/ew-api/tests/acceptance/checks.rs:1610-1627`; `bin/ew/tests/cli.rs:1115` | D6 |
| The actions the tasks reach: `file.export_pdf`, `pages.renumber`, `nav.back`, `nav.follow`, `messages.waive`, `tool.next_proposal`, `tool.connect_on_move` (`C`), `strip.insert` | `tools/blind/reference/actions.txt:13`, `:37`, `:43`, `:45`, `:78`, `:127`, `:142`, `:165` | §5.1 |
| No `docs/ux/` folder; `docs/**` is CC-BY-4.0 and `tools/blind/**` CC-BY-4.0 | `ci/licence-map.toml:383`, `:432` | D16 |
| The run costs seen: gpp-3 (text gate) about 169k tokens for 55 turns; the calibration round's text run about 115k for 16 turns, its visual run about 124k for 21 turns, a checker about 70k; all on Opus at `low` | `docs/plan/06-decision-log.md:1056`, `:1057` | §5.4 |

## 3. Requirements traced

Each row is **met**, **met by proxy** (the proxy gives what the requirement asks, from personas instead of people),
**not met by a proxy** (the requirement measures people; the row says what the proxy gives instead) or **not
applicable**.

| Requirement | State, and what this package delivers | Acceptance |
|---|---|---|
| NFR-UX-01, SC-9 (completion) | **Not met by a proxy.** "≥ 80 % unassisted completion" is a rate of people. The proxy gives each persona's attempt of T1 to T5 as completed unassisted, completed with hints or not completed (D8), counted over 15 visual attempts as doc 16 §19.4 counts people's, and reported beside doc 16's 12-of-15 line as a reading, never as NFR-UX-01's rate (D2) | UR-12 |
| NFR-UX-01 ("severe findings are fixed before milestone exit") | **Met** for the findings the rounds make: each severe finding is fixed with a kittest flow and re-run (D15) | UR-11, UR-14 |
| NFR-UX-02 | **Not met by a proxy.** SUS is a questionnaire of people, normed on people. The proxy gives the proxy questionnaire's figure per persona, a model's answers on SUS's scale, reported without NFR-UX-02's threshold and never called a SUS score (D11) | UR-12 |
| NFR-UX-04 | **Met by proxy** in its task: the QET personas draw the folio 102 circuit from the company basic project without the guide (`--no-book`) and export the PDF (T6). **Not met by a proxy** in its time: "30 minutes" is a person's time. The proxy gives the turns and inputs of the attempt and the persona's own estimate of its time in its usual tool (D9) | UR-12 |
| NFR-UX-06 | **Not met by a proxy.** It needs the pilot user's own QET project, which is the private corpus, and the pilot's hour. A persona run on Golden C would give a migration report resolved by a model, on the pilot's drawing: not the pilot's project, not the pilot's time, and it puts private customer data in front of an agent, which ID-341 excludes ("placeholder data only, never customer data"); it is not run (§12 O5). The requirement stays with the pilot log; in the plan's lean variant its criterion closes at G-M3 (02c `:360`) | — |
| NFR-UX-09 | **Met by proxy:** a formative round with two personas (the pilot user's role and a colleague) on T1, T2 and T8, its report before R1 and its findings triaged before R1. It runs after M-PP rather than as soon as the tools existed: the report says so | UR-10, UR-11 |
| NFR-PRIV-10 | **Not applicable.** No person takes part, so no consent is asked and nothing is recorded of a person; the runs hold an agent's text and offscreen frames. The evidence committed holds no personal data (UR-15). Doc 19 §3.7's template stays for any later round with people | UR-15 |
| ADR-0007 "Validation before M2" and amendment 2 | **Met by proxy, with stated limits:** the record answers the three questions and the junction defaults from what the personas did and answered (D17); a model playing an EPLAN user answers from its knowledge of EPLAN, not from EPLAN users' practice | UR-13 |
| G-M2-20, G-M2-21, G-M2-22 | Their evidence, read under the wording §13 proposes ("blind-persona rounds (ID-1331)") | UR-10 to UR-14 |
| FT-713 friction 3 (ID-1332 O2) | R0's visual runs at the floor show whether texts are readable at the first zoom; the R0 report rules it a finding with its severity, or no finding | UR-10 |

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Two halves.** The **lane** builds §7 steps 1 to 4, hands back for R0's runs, is resumed for step 5 (R0's report and routing) and is reviewed once, whole, then merged; WP-S2.12 counts as merged for WP-S6.08's Depends from then. **R1** (steps 6 and 7) runs on WP-S6.08's first candidate, between its §7 steps 3 and 5: the orchestrator's runs, then a lane of this brief (fresh, with the hand-off note, or resumed while under about 750k, ID-1322) writes the R1 report; G-M2's gate review reads it as WP-S6.08's D12 row | WP-S6.08 depends on WP-S2.12 and R1 runs on WP-S6.08's candidate (doc 16 `:1874`), so one lane end to end would wait months; the build is still one brief and one review (ID-1281), the report the gate's evidence |
| D2 | **The proxy statement.** Every report opens with it: the rounds are a model's attempts in a persona's role, not people's. A **completion** is an outcome the product's exports decide (the task was done or not), so it is counted and set beside doc 16's line as a reading; a **questionnaire answer** is a model's attitude with no norm, so its figure gets no threshold. Neither is ever written as NFR-UX-01's rate or NFR-UX-02's score. Each report lists, under "What the proxy cannot give", the rows of §3 marked "not met by a proxy" and what it gave instead | The owner's ruling asks for plain words where the proxy falls short (ID-1333); an outcome and an attitude are different kinds of evidence |
| D3 | **Who runs the rounds: the orchestrator, at the end phase.** It builds the binaries and the guide (WP-S6.13 §5.11 step 1), starts each run (`cargo xtask blind start`), starts each persona and checker agent with the permission rules of §5.11 step 3, stops, and audits each agent's tool log (§5.11 step 7; ID-678's precedent: a benign call is recorded, a violation voids the run, which is run again). The lane writes the materials, reads the runs, writes the reports and routes the findings. R0 runs at the end phase of the cycle the lane hands back in, on the lane's tip; R1 on the candidate | Only the orchestrator starts agents (§2.1); the runs are a deliberate orchestrator act, as every blind run so far |
| D4 | **The tasks as brief-kind folders, one run per persona, task and sense.** Each of doc 16's tasks is a folder `tools/blind/tasks/brief-ut-<t>-<name>/` (§5.1; the prefix the brief kind requires), its `task.md` doc 16's instruction in the user's words with the placeholders of doc 20, `task.toml` with `task_id`, `rounds` and `start` (D5) and `turn_budget` taken from doc 16's limit column (reported, never enforced). Each attempt is a fresh run on a fresh copy, as doc 16 §19.3's "each session starts from a fresh copy"; a later task's prelude supplies an earlier task's result as `setup` lines (T2 starts with T1's reference steps). **Deviation from doc 16:** a person's 90-minute session is cut into one run per task, so the persona does not carry what it learnt from T1 into T2 | Per-task runs give independent attempts that the oracle decides one by one, and the harness composes one task per run; the learning effect across a session is lost, which the reports state |
| D5 | **Start projects.** `task.toml`'s `start` is `gp-ut` or `golden-a` (or absent for T6, which starts from the wizard's basic project). `start`, `replay`, `check-brief` and a stateless call make `work/project` from a temporary copy of `golden/J-E-2416.ew` (with `gp-ut`, `ew apply` of `variants/gp-ut.ewcmd` on the copy), as a check run's copy is made and put back (§2.1); the prelude opens `project`. WP-S6.13's D16 holds in its guard (the digest at `start` and `stop`, the empty sample folder, no path to the fixture) and **is amended in one point:** in a usability task the agent works on a copy of Golden A, as doc 16 §19.3 has the participant do | Doc 16's tasks are edits of the golden project; the copy is made the way the guard already allows, and the agent still cannot reach the committed fixture or the sample |
| D6 | **GP-UT.** `golden/J-E-2416/variants/gp-ut.ewcmd` (`ew.cmdlog/1`) removes -102F1 and the wires W20, W21, W24 and W25 from Golden A and nothing else (doc 20 §2, `:93-97`); a test beside the Golden B variant tests replays it on a temporary copy and checks the removed records and that every other record is unchanged. The R2 form ("also without the binding of channel 1 of -201U3") is T10's prelude line, so the file holds R0's and R1's form only. The basic project of GP-UT sessions is the wizard's company standard with its defaults in the prelude (as `gpp-101-103` makes it); no `basic-project/` folder is written (WP-S7.03 O6, ID-1332) | Doc 20 names the file; one form keeps the file what R0 and R1 use; the commit carries the `Golden:` trailer of the brief's ID (a new file under `golden/`, Golden A's files unchanged) |
| D7 | **The oracle.** Each task's `acceptance.toml` turns doc 16's success criterion into items: a **hard** item for each part an output decides (a `query` over the `api_v1_*` views with its `expect`, or the existing outputs); a new member **`actions`**, an ordered list of action IDs (alternatives joined by ` or `) that must be done in that order, decided from `outcomes.jsonl` (text) or its `dispatched` lines (visual), for the parts doc 16 states as behaviour ("opened the Tags, Xrefs or Pages tab before Accept", "xref jump and Back used"); a part no output decides becomes a **soft** item the checker decides, named in the report. `check-brief` decides every item on the reference solution, `actions` included. **`stop` decides every item of a brief run mechanically** with the same code (its result in `out/score.json` and `out/report.md`); a run's attempt is **completed** when every hard item is met | The completion count stays an outcome of the product's exports, not a reader's judgement; the existing decision code is reused (brief.rs `:556-621`) |
| D8 | **Assists: hints on request.** Each task has `hints.toml`, one to three hints graded from where to look to what to do, in the user's words (no action ID, the brief's word scan). `{blind} hint <run>` prints the next hint (K1) and records it in `turns.jsonl`; the round prompt tells the persona to ask when it would ask the person at its side, or when three turns brought it no closer (doc 16's "two minutes without progress", in turns). An attempt is **unassisted** (completed, no hint), **with help** (completed after a hint) or **not completed** (an item unmet, or the run ended by quit or cap). Reading the run's user guide counts as unassisted, as doc 16 §19.3 counts in-app help; T6 runs without the guide | The moderator's intervention as a scripted, counted act; on request only, because a harness that judged "no progress" during a run would need the oracle live |
| D9 | **Errors and time, by counts.** Per attempt the score records turns, steps or inputs, refused and failed steps (text) or refused dispatches (visual), `edit.undo` done, hints and friction entries by severity: doc 16 §19.5's "time on task, errors, undo count, assists" as counts. No minutes: T6's time is the persona's own estimate in hand-back heading 5 beside its counts | The owner's rule; a model's turn is not a person's minute |
| D10 | **Personas.** Three new files, senses-independent (no "you have no mouse"; the prompt of the run says how it acts), each with the sections of `first-timer.md` and doc 16's screening ("at least one year of production work"): **`eplan-designer`** (a design engineer from a large EPLAN shop: master data, navigators, macros, the mouse), **`eplan-maintainer`** (a maintenance and revision engineer who works in EPLAN daily: shortcuts, typed values, quick edits of existing projects) and **`qet-user`** (a panel-shop draughtsman with a few years of QET, mouse first, from the pilot's trade as a role). R0's participants are `qet-veteran` (the pilot user's role, as at G-PP) and `qet-user` (a colleague); R1's are `eplan-designer`, `eplan-maintainer` and `qet-user`, with `qet-veteran` on T6 beside `qet-user`. Existing personas are unchanged. **Not mirrored:** the QWERTZ or AZERTY participant (the driver sends named keys), the 1920 × 1080 laptop (D12) | Two EPLAN personas with different habits and one QET persona mirror R1's participants (PQ-27); fresh agents carry nothing from R0 to R1, so reusing `qet-user` biases nothing |
| D11 | **The round prompt and the proxy questionnaire.** `tools/blind/ROUND.md` (front matter `prompt: usability-round`, version 1) is composed after the task sheet for a task whose `rounds` is not empty: the hint command, the questionnaire and its form, and the rule "file the questionnaire, then the hand-back". `{blind} questionnaire <run> -` takes ten lines `Q<n> <1-5>` (§5.5's statements, 1 "strongly disagree" to 5 "strongly agree") and four lines `B<n> <choice> <one sentence>`, writes `questionnaire.json`, and `handback` refuses until it is filed (K4 to K7). The **PQ figure** is SUS's arithmetic on the ten answers (odd items score minus 1, even items 5 minus score, the sum times 2.5, 0 to 100); the reports give it per attempt, per persona and per round, without a threshold. The ten statements are this project's own wording in SUS's alternating form, not SUS's text (§12 O6). `BLIND_DRAFTER.md`, `PLAYTESTER.md`, `HELP.md` and `HELP-VISUAL.md` are unchanged, so no self-test recording changes | A composed file reaches every round run and no other run; the questionnaire is answered in the run, by the agent that worked, before it hands back |
| D12 | **Senses.** The **visual** runs (`--senses visual`, ID-785) are the counted attempts: a person sees the window. The **text** runs serve the pairing rule (visual spec §7.4): friction only in the visual run is a window finding, in both a product finding. Visual runs use the default frame (1280 × 800 at 1.2, 1536 × 994 pixels), the frame the reading probe calibrated (20 of 20, ID-810), not doc 16 §19.3's 1920 × 1080 (§12 O4). The caps are the brief kind's (§2.1) | People use the window; the text run separates "the window hides it" from "the product cannot do it" |
| D13 | **Run counts** as §5.3: R0 12 drafter runs, R1 35, R1's re-tests one per severe finding and persona that hit it. Each run ID is `<round>-<task>-<persona>-<v or t>` (`r1-t3-eplan-maintainer-v`) | Doc 16's arithmetic for the counted attempts (three personas, five core tasks, 15 attempts); one text run per task is enough for the pairing rule |
| D14 | **Checkers and triage.** A checker (`CHECKER.md`, Opus at `low`, a check run) audits each run with a hard item unmet or a soft item open, and every T6 run; elsewhere the mechanical decision stands. The friction ledger and the play-test checker of WP-S6.13 part 5 are not built, so the lane clusters the friction entries, notes and checker audits of a round itself into the report's findings table (`R0-F<n>`, `R1-F<n>`) | The mechanical decision removes most of a checker's acceptance work; part 5 is not a dependency |
| D15 | **Severity and routing.** The lane rates each finding on Nielsen's 0 to 4 (doc 16 §19.6) with its evidence: **4** a task attempt not completed even after every hint, or work lost; **3** a wrong output, a misread derived value that U4 (visual spec §7.2) does not explain by the agent's sight, or a hard item met only with help by two or more personas; **2** completed but slowed (a dead end of three or more turns, a `confusing` entry); **1** cosmetic or `annoying`; **0** by design, or the agent's own misreading. A core task that no persona completes even with hints is severe by definition. **R0:** each severe finding goes to its owning package, or to a fix lane by ID-1281's rule when that package is complete, is fixed with a kittest flow reproducing it before R1 starts (G-M2-22), and is re-run in R1 by its task's runs. **R1:** each severe finding is a fix round of WP-S6.08's D16, fixed with a kittest flow before the next candidate, then re-run on it by the persona or personas that hit it. **Nielsen 1 and 2** become proposed follow-up rows in the report, each with its owner, which the design pass opens (parked while its area is in feature work, visual spec §9.1 item 5); **0** is noted only | Doc 16 §19.6's scale and rule, read from what a run records; the fix stays with its code's owner |
| D16 | **The reports and the evidence.** `docs/ux/README.md` (the method, the proxy statement, the file layout), `docs/ux/R0-report.md` and `docs/ux/R1-report.md` (CC-BY-4.0), in §5.6's sections; persona names are the participant codes. Each run's evidence is committed under `tools/blind/rounds/<round>/<run>/`: `run.json`, `drafter.md`, `handback.md`, `friction.jsonl`, `questionnaire.json`, `checker.md` when present, `out/score.json` and `out/report.md`, with the run folder's absolute path written `«run»` as `bless` writes it; no frame, no transcript, no PDF. Runs are not blessed as recordings and have no CI replay | The report is GP-10's evidence; the files let the gate reader audit any number; frames and transcripts stay out of a public repository's size and away from local paths |
| D17 | **ADR-0007's validation record** is a section of the R1 report, with R0's early evidence (PD-10): for each of the ADR's three questions and amendment 2's junction defaults, the behaviour seen (the proposals kept, cycled with Tab or refused with Ctrl+Enter in T1 and T2; whether A7's personas used Connect while moving or expected the move to connect; where T5's personas looked for 24VDC-1's continuation), the questionnaire's part B by persona, what the proxy cannot answer, and the proposal: **accept as is** or **amend the defaults** (on placement, on move, the junction wiring). The orchestrator rules it (PD-10's maintainers, as product owner delegated) and records it; the wiring-tool defaults are frozen from that ruling; the design pass amends ADR-0007's status line. A finding that asks for autoconnect as model semantics is a stop (the ADR's own rule: a new ADR) | G-M2-21 asks for the record and the frozen defaults; the ADR names the questions |
| D18 | **Models and effort** (`.claude/orchestration.md` rule 0): the **drafter** (text runs), the **play-tester** (visual runs) and the **checker** run on **Opus at `low`**, as G-PP's runs, the calibration round and every play-test since (ID-700, ID-768, ID-810), so the rounds compare with them; no role runs on Haiku, because a persona's judgement is the measurement and only Opus at `low` was calibrated (BV13); no Sonnet. The lane is Opus at `high`; the review Opus at `high` | Comparable, calibrated runs; cost in §5.4 |
| D19 | **What leaves the scope.** The consent forms of the scope line (no person); the moderator and the note-taker (the harness and the lead); the local session log (the run folder is it, and no person is in it) | ID-1331 |
| D20 | **Every item closed;** no decision changes an ADR (ADR-0007's amendment is proposed by D17 and written by the design pass) | — |

## 5. The rounds

### 5.1 The tasks

| Task | Folder (`brief-ut-…`) | Instruction (doc 16 §19.4) | Start (D5) | Hard items: what decides them (D7) | Round |
|---|---|---|---|---|---|
| T1 Place | `t1-place` | `:1902` | `gp-ut` | A device with T1's part on folio 102, its main representation between -102G1 and -102F2 by cell, tag -102F1 by the numbering rule and not frozen (queries) | R0, R1 |
| T2 Wire | `t2-wire` | `:1903` | `gp-ut` and T1's reference steps | The W20, W21, W24, W25 topology through the new device, no unconnected-CP finding on it (queries, `check`) | R0, R1 |
| T3 Terminal strip | `t3-strip` | `:1904` | `golden-a` | A terminal between -X1031:4 and :5 with a unique designation and no prevent-rule finding; the 12 spare terminals' function texts read "Reserve" (queries); "one undo step" soft unless a query decides it | R1 |
| T4 Renumber | `t4-renumber` | `:1905` | `golden-a` | Folio 104 with -104W1 and -104W2 (queries); a preview tab opened before the accept (`actions`) | R1 |
| T5 Export PDF | `t5-export` | `:1906` | `golden-a` | The persona's own PDF (`pdf`); a followed cross-reference from 24VDC-1 on folio 102 to folio 103 (`actions`), the proxy of the PDF's link, which no persona can click | R1 |
| T6 Timed circuit | `t6-timed-circuit` | `:1907` | the wizard's basic project; `--no-book` | -102G1, -102F1 and -102F2 with their parts and wires (queries); the PDF (`pdf`); the parts from `ew-sample-parts` once WP-S7.03 ships it (ID-1332 O2), until then three golden part copies embedded in the prelude as `gpp-101-103`'s are | R1, QET personas |
| T7 Finding | `t7-finding` | `:1908` | `golden-a` | A waiver with a reason on T55's safety finding (query); the finding opened and the rule page or detail read (`actions`) | R1 |
| T8 Navigation | `t8-navigation` | `:1909` | `golden-a` | A followed cross-reference from L-1 on folio 101, then Back (`actions`) | R0, R1 |
| A7 Move | `a7-move` | ADR-0007 `:88-90` | `golden-a` with one device moved off its open wire ends in the prelude (`setup`) | The device back in place with its connection points connected to those ends (queries); how it was connected is evidence for D17, not an item | R0, R1 |
| T9 Layout | `t9-layout` | `:1910` | `golden-a` | EW-LAY-001 no longer lists F55 | R2 (not run) |
| T10 PLC | `t10-plc` | `:1911` | `gp-ut` and the line that removes channel 1's binding of -201U3 | The binding and the symbol in the TIA export | R2 (not run) |
| T11 Shop floor | `t11-shop-floor` | `:1912` | `golden-a` | W29 found with both ends named | R2 (not run) |

T1 to T8 and A7 pass `check-brief`. T9 to T11 are committed with `check-brief`'s verdict in their `task.md` history:
runnable, or not runnable with its problems, as `brief-relays-valves` was committed (visual spec §6.3).

*As built (2026-11-05, cycle 51; decision log ID-1355, ID-1361).* T1 to T6, T8 and A7 are committed and pass
`check-brief`. T7 is not committed (§9.3 item 1): its waiver step is human-only (`check.waive`, ADR-0008) and
`check-brief` replays headless, so the item cannot be proven; R0 does not need it, and before R1 a row settles whether
`check-brief` proves a human-only step or T7 stays out of R1 (R0 report row P-1). T9 to T11 are not committed: they
leave under §10's de-scope rule as WP-S2.16's rows, their drafts in the lane's hand-off note, which replaces the
paragraph above's "committed with `check-brief`'s verdict" for them (ID-1355's ruling on them amended by ID-1361).

### 5.2 Who takes part

| Round | Personas (D10) | Mirrors |
|---|---|---|
| R0 formative | `qet-veteran`, `qet-user` | The pilot user and a colleague (doc 16 `:1873`) |
| R1 summative | `eplan-designer`, `eplan-maintainer`, `qet-user`; T6 also `qet-veteran` | ≥ 2 EPLAN users, ≥ 1 QET user (doc 16 `:1874`) |

*Amended (2026-11-05, cycle 51; decision log ID-1365, R0 report row P-3).* For R1, T8's second hard item (A2) accepts
a return to the start page by the arriving cross-reference as well as by Back: the actions `nav.follow`, then
`nav.back` or `nav.follow`, with a new item that folio 101 shows again at the run's last step, which needs the page
shown in the decision (a harness addition), in the sheet's version 2. In R0 a persona returned by double-clicking the
back reference, as QET users do, and failed the narrower item; the missing visible Back stays R0-F6. §5.1's row for T8
reads in this light.

### 5.3 Run counts

| Round | Runs | Count |
|---|---|---|
| R0 | T1, T2, T8, A7 visual × 2 personas | 8 |
| R0 | T1, T2, T8, A7 text × `qet-veteran` | 4 |
| R1 | T1 to T5, T7, T8, A7 visual × 3 personas (T1 to T5: the 15 counted attempts) | 24 |
| R1 | the same eight tasks text, one persona each in turn | 8 |
| R1 | T6 visual × `qet-user` and `qet-veteran`, text × `qet-user` | 3 |
| R1 re-test | each severe R1 finding's task, visual, by each persona that hit it, on the next candidate | as found |
| Checkers | by D14 | about 4 (R0), 12 (R1) |

### 5.4 The token budget per round (an estimate, not measured)

From §2.1's costs: a visual run of a small task about 150k to 200k tokens, a text run about 100k to 150k, T6 visual
about 500k (a whole circuit, like a brief run of 60 to 120 turns at about 2,000 image tokens a frame), a checker about
80k. **R0** about 2.2M tokens (8 visual, 4 text, 4 checkers); **R1** about 7.5M (24 visual, 8 text, 3 T6 runs, 12
checkers); each re-test about 200k. The lane's own reading is about 6k to 8k tokens a run (§10).

### 5.5 The proxy questionnaire

Part A, answered 1 (strongly disagree) to 5 (strongly agree):

| # | Statement |
|---|---|
| Q1 | I would use this program for my daily drawing work. |
| Q2 | The job took more effort in this program than the job itself needs. |
| Q3 | I could find my next step without hunting for it. |
| Q4 | To do this again I would want someone beside me who knows the program. |
| Q5 | The parts of the program fit together into one way of working. |
| Q6 | The program behaved differently from one place to the next. |
| Q7 | A colleague from my trade would pick this program up quickly. |
| Q8 | Getting the job done felt clumsy. |
| Q9 | I trusted what the program showed me about my drawing. |
| Q10 | I had to learn a lot before I could get going. |

Part B, ADR-0007's questions (D17), one choice and one sentence each:

| # | Question | Choices |
|---|---|---|
| B1 | When you moved a device so that its connection points lined up with others, what did you expect? | `connect` (by itself), `ask` (only when I ask), `never`, `not-seen` |
| B2 | When the program proposed wires as you placed a device, what did you want? | `keep`, `choose` (keep some), `off`, `not-seen` |
| B3 | Where a potential or signal continued on another page or behind a terminal, did the program show it where you expected? | `yes`, `partly`, `no`, `not-seen` |
| B4 | Where three or more wires met at one point, did the order of the wires match how you would wire it? | `yes`, `partly`, `no`, `not-seen` |

### 5.6 The reports' sections

**R0** (`docs/ux/R0-report.md`): 1 What this round is (D2's statement); 2 The build and the runs (the commit, the
models, every run with its validity by the audit and its counts); 3 Per task (attempts unassisted, with help, not
completed; errors, undo, hints; what the personas said, quoted briefly from their notes, which are the think-aloud);
4 Findings (ID, title, Nielsen severity, personas, runs, evidence as `<run>#<entry>` with frame and region, the
pairing reading, owner, route); 5 The fix list (the severe rows with their owner, kittest flow and state); 6
ADR-0007's early evidence; 7 FT-713's friction 3; 8 The PQ figures; 9 What the proxy cannot give; 10 The proposed
follow-up rows; 11 Token use as the orchestrator records it.
**R1** (`docs/ux/R1-report.md`): the same sections, with 3 adding the 15 counted attempts beside the 12-of-15 line,
a section for T6 (the counts and the personas' own time estimates), ADR-0007's validation record (D17) in place of
6, a reading table of G-M2-20 to G-M2-22 against §13's wording, and an addendum with the re-tests on the next
candidate.

## 6. The texts

Every text is asserted whole. These are harness outputs (en-GB only); no product text changes.

| # | Where | Text |
|---|---|---|
| K1 | `hint`, the next hint | `hint <n> of <m>: <text>` |
| K2 | `hint`, none left | `harness: no more hints for this task` |
| K3 | `hint` in a run whose task has no hints, or in a check run | `harness: this task has no hints` |
| K4 | `questionnaire`, filed | `questionnaire filed` |
| K5 | `questionnaire`, a form not filled | `harness: the questionnaire has the answers Q1 to Q10, each 1 to 5, and B1 to B4, each one of its choices with a sentence` |
| K6 | `questionnaire`, a second filing | `harness: the questionnaire is filed` |
| K7 | `handback` before the questionnaire, in a round task | `harness: file the questionnaire before the hand-back ({blind} questionnaire {run} -)` |
| K8 | `check-brief` and `start`, a start project that cannot be made | `the task's start project <start> could not be made: <reason>` |
| K9 | `check-brief`, a `task.toml` with another start | ``task.toml: `start` is `gp-ut` or `golden-a` `` |

## 7. Build order

One lane builds steps 1 to 5 and commits at the end of each; steps 6 and 7 follow in WP-S6.08's release cycle (D1).

1. **The harness additions** (D5, D7, D8, D11): `start` in `task.toml` with the copies of `start`, `replay`,
   `check-brief`, `resume` and a stateless call; `task_id` and `rounds`; `actions` items; `stop`'s mechanical
   decision and the counts of D9 in `score.json` and `report.md`; `hint` with `hints.toml`; `questionnaire`,
   `questionnaire.json` and the hand-back rule; `ROUND.md` in `compose`; the texts K1 to K9; `xtask` tests with a fake
   product; the self-tests run unchanged (README's rule).
2. **GP-UT** (D6): `variants/gp-ut.ewcmd`, its test, the variants line of `golden/J-E-2416/README.md`; the commit with
   the `Golden:` trailer the brief gives.
3. **The personas and the round prompt** (D10, D11): the three persona files, `ROUND.md` with §5.5, `docs/ux/README.md`,
   `tools/blind/README.md`'s rows.
4. **The task sheets** (§5.1): the twelve folders with `task.md`, `task.toml`, `acceptance.toml`, `prelude.uiscript`,
   `prelude.toml`, `solution.uiscript` and `hints.toml`; `check-brief` on each, its output kept for the hand-off note;
   T9 to T11's verdicts in their history. **Hand back** (the R0-ready point, §10).
5. **R0** (after the orchestrator's runs, D3): the evidence folders, the analysis, `docs/ux/R0-report.md`, the fix list
   with each severe finding's owner and the kittest flow it needs, the proposed follow-up rows. **Hand back** for the
   review.
6. **R1** (after the orchestrator's runs on the first candidate): the evidence folders, `docs/ux/R1-report.md` with
   ADR-0007's validation record, the D16 fix rounds listed for the orchestrator.
7. **The re-tests** on the next candidate: the R1 report's addendum.

## 8. Acceptance

- **UR-1** (D5) A brief task with `start = "gp-ut"` or `"golden-a"` starts on a copy in `work/project`, from a temporary
  copy of the fixture; `resume`, a stateless call and `replay` make it again; the golden digest is equal at `start` and
  `stop`; another value fails with K9, a copy that cannot be made with K8 (`xtask` tests with a fake product).
- **UR-2** (D7) An `actions` item is met when its actions are done in order and not met when one is missing, refused or
  out of order, read from a text run's outcomes and a visual run's `dispatched` lines; `check-brief` decides it on the
  solution (`xtask` tests).
- **UR-3** (D7, D9) `stop` of a brief run writes each item's decision, the attempt's state (unassisted, with help, not
  completed) and D9's counts into `out/score.json` and `out/report.md`, equal to `check-brief`'s decision on the same
  project (`xtask` tests on fixtures).
- **UR-4** (D8) `hint` prints K1 in order, K2 after the last, K3 for a task without hints and in a check run, and is
  recorded in `turns.jsonl`; it is allowed in a visual run (`xtask` tests).
- **UR-5** (D11) `questionnaire` accepts §5.5's form and writes `questionnaire.json` with the PQ figure (a fixture of
  known answers gives its known figure), refuses with K5 and K6 whole, and `handback` refuses with K7 in a round task
  and not elsewhere; `compose` adds `ROUND.md` for a round task only, byte-deterministic, its digest recorded (`xtask`
  tests).
- **UR-6** (D6) The GP-UT test passes: -102F1 and W20, W21, W24, W25 are gone and every other record is unchanged;
  `golden/J-E-2416.ew` is unchanged; `check-trailers` finds the `Golden:` line.
- **UR-7** (D10) The three persona files have front matter and the sections of `first-timer.md`;
  `check-anonymisation` passes on `tools/blind/`.
- **UR-8** (§5.1) `check-brief` passes for T1 to T8 and A7 with every hard item met by the reference solution; T9 to
  T11 carry their verdict; each folder has one to three hints and no action ID in its texts.
- **UR-9** The self-test recordings replay unchanged (`blind_replay`), and `cargo xtask blind selftest` and
  `selftest-visual` pass.
- **UR-10** (NFR-UX-09, G-M2-22, FT-713) R0's twelve runs are valid by the orchestrator's audit (a void run re-run);
  `docs/ux/R0-report.md` has §5.6's sections, every finding its severity, evidence and owner, and FT-713's friction 3
  its ruling (the review reads it).
- **UR-11** (NFR-UX-01's fix clause, G-M2-22) Every severe R0 finding is closed before R1 starts, each by its owning
  package's commit with a kittest flow named in the R0 report's fix list; R1's runs of its task are its re-run
  (the R1 report).
- **UR-12** (NFR-UX-01, -02, -04 by proxy) R1's 35 runs are valid by the audit; the R1 report gives the 15 counted
  attempts as unassisted, with help or not completed, the PQ figures without a threshold, T6's counts and time
  estimates, and "What the proxy cannot give" (the gate review reads it).
- **UR-13** (G-M2-21) The R1 report's validation record answers ADR-0007's three questions and amendment 2, states the
  limits, and proposes accept or amend; the orchestrator's ruling is in the decision log.
- **UR-14** (G-M2-20) Every severe R1 finding is a D16 fix round with a kittest flow, and its re-run on the next
  candidate completes its task; the addendum records each.
- **UR-15** (NFR-PRIV-10, public repository) The evidence folders hold only D16's files, no absolute path, no frame, no
  personal name; `check-anonymisation` and `doccheck.py` pass on `tools/blind/rounds/` and `docs/ux/`.
- **UR-16** The hand-back checks of the lane's package set pass (fmt, settle, clippy, doc, nextest, policy, trailers
  with `Golden:`).

## 9. Neighbours, files owned, what a lane may not decide, and risks

### 9.1 Neighbours

| Neighbour | It owns | This package |
|---|---|---|
| WP-S6.13 (complete) | The harness, the prompts, the personas, the brief kind, the self-tests | Adds start projects, `actions`, the decision at `stop`, hints, the questionnaire and `ROUND.md` (§7 step 1) |
| WP-S6.08 | The candidates, D16's fix rounds, the acceptance run | R1 runs on its first candidate; its severe findings are D16 rounds; G-M2-20 to G-M2-22 are its D12 rows |
| WP-S2.11 | The kittest suite | The severe fixes add flows to it, written by the fix lanes |
| WP-S7.03 | The wizard's basic project, `ew-sample-parts` | T6's start and parts |
| WP-S7.05 | The user guide | The runs read it; T6 runs without it |
| WP-S2.16 | R2 | Takes T9 to T11's sheets and this protocol |
| The packages that own a finding's code | Their fixes | Receive R0's severe findings and R1's through D16 |
| WP-S6.13 visual spec §9.1 item 3 | G-M2's milestone visual round on the brief library | Runs apart from R1, on briefs, not on doc 16's tasks |

### 9.2 Files owned

`xtask/src/blind/**` (the additions of §7 step 1; a new `round.rs` for hints and the questionnaire) and its tests;
`tools/blind/ROUND.md`, `tools/blind/personas/{eplan-designer,eplan-maintainer,qet-user}.md`,
`tools/blind/tasks/brief-ut-*/**` (new); `tools/blind/README.md` (the layout rows); `tools/blind/rounds/**` (new);
`golden/J-E-2416/variants/gp-ut.ewcmd` (new) and one line of `golden/J-E-2416/README.md`; the GP-UT test beside the
variant tests; `docs/ux/**` (new). Read-only: `golden/J-E-2416.ew`, the existing personas, `BLIND_DRAFTER.md`,
`CHECKER.md`, `PLAYTESTER.md`, `HELP.md`, `HELP-VISUAL.md`, the existing tasks and recordings, `deny.toml`,
`supply-chain/`, `Cargo.lock`, every other document. A fix of a finding belongs to its owner, never to this package.

### 9.3 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a hard item from doc 16's criterion is not met by the reference solution on the build (the task is then not
   runnable; the item is never dropped or made soft to pass);
2. GP-UT's log changes a record beyond -102F1 and its four wires, or T1's re-placed device does not take the tag -102F1;
3. a change reaches an existing prompt, persona, help file, task or recording, or the self-tests need a re-bless;
4. a finding's evidence points at autoconnect as model semantics (D17), or a severity between 2 and 3 rests on one
   persona's reading alone (listed for the orchestrator with both readings);
5. a text would differ from §6, a new dependency seems needed, or a change reaches a neighbour's files;
6. an evidence file would hold an absolute path, a frame or anything personal that normalising does not remove.

The lane decides freely: the wording of the task sheets within doc 16's instructions and the brief kind's rules, the
hints, the reference solutions, the queries and their `expect`, A7's device and position, the persona texts within
D10, the clustering of findings, the reports' layout within §5.6, the GP-UT test's home, and how the evidence is
copied and normalised.

### 9.4 Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 One attempt per persona and task is a noisy sample of a model | High · medium | A not-completed core task is severe by D15 and read at once; §12 O3 offers a second sample |
| R2 The proxy over-reads usability (a model reads every label and never tires) or under-reads it (sight misreads small text) | Medium · medium | The pairing rule (D12), U4's separation of the agent's sight (D15), the calibrated frame; D2's statement in every report |
| R3 A persona's EPLAN expectations are a model's picture of EPLAN | Medium · medium | D17 states the limit; ADR-0007's record proposes, the orchestrator rules |
| R4 R1's severe findings add candidates to WP-S6.08's cycle | Medium · medium | R1 runs first on rc.1, before the checklist; D16 routes each fix; re-tests are single runs |
| R5 The token cost exceeds §5.4 | Medium · low | The counts are the orchestrator's to lower (§12 O3); the calibrated costs of §2.1 |
| R6 The harness additions break a self-test recording | Low · medium | §9.3 item 3; the additions act only on tasks that use them |
| R7 An agent reaches outside the harness | Low · medium | The audit of D3; a void run is run again |

## 10. Size, routing and the hand-back point

**Size.** The lane about 2.7 pw: step 1 0.7, step 2 0.1, step 3 0.15, step 4 0.75 (the twelve folders, T1 to T8 and
A7 proven runnable), step 5 0.35; step 6 0.55; step 7 0.1. The orchestrator's acts of the rounds (builds, runs,
audits) about 0.3 pw. The PERT of 3.1 pw (O 2 · L 3 · P 4.5) stays (§12 O7). **De-scope rule:** when the lane is
projected more than 20 % over at step 4, T9 to T11's sheets leave as a follow-up for WP-S2.16 without a further
ruling.

**Routing** (`.claude/orchestration.md` rule 0): one lane, **Opus at `high`** (harness code with tests, a golden
fixture, and the judgement of severity and of the proxy's limits); the persona, play-tester and checker agents Opus at
`low` (D18); the review Opus at `high`. The brief gives the `Golden:` ID of step 2.

**Hand-back points** (ID-1322). The lane hands back at the end of **step 4** in any case, because R0's runs are the
orchestrator's (D3); it is resumed for step 5 while its context is under about 750k, and otherwise a fresh lane
continues with the hand-off note. It hands back at the end of **step 5** for the review. Steps 6 and 7 are a fresh lane
of this brief, or the same lane resumed while under about 750k. A lane that nears 800k anywhere commits and hands
back. Its reading is about 6k to 8k tokens a run (the hand-back, the friction log, the score, the questionnaire, a
checker's audit), so R0 adds about 100k and R1 about 280k to the lane's context.

**The hand-off note** at step 4 holds: the commits; the harness additions with their texts, file formats and test
filters (`-p xtask -E 'test(/blind::(start|brief|stop|round|compose|replay)/)'`, the GP-UT test by name); GP-UT's
commands; per task its start, prelude lines, items (hard, soft, `actions`), hints and `check-brief` output; T9 to
T11's verdicts; the run-ID scheme; and every choice of §9.3's free list made. At step 5, also: the R0 run table with
validity, the findings with their severity, evidence and route, the PQ figures, ADR-0007's early evidence, FT-713's
ruling, and what R1 must re-run.

## 11. Follow-ups, proposed

The design pass opens the rows.
- **FP-1** `qet-veteran` and `switcher` made senses-independent (their "you have no mouse here" lines contradict the
  play-tester's prompt in visual runs), with a version bump and the recordings' digests noted.
- **FP-2** A reading probe at 1920 × 1080 with `--ppp 1`, before any round at doc 16 §19.3's resolution (§12 O4).
- **FP-3** An expert-time estimate from a visual run's inputs (fixed times per input kind), reported beside T6's
  counts, if the owner wants a time-shaped proxy for NFR-UX-04.
- **FP-4** The friction ledger of WP-S6.13 part 5 used by the rounds once it is built.
- **FP-5** R2's method (people, or blind-persona rounds as here), a ruling for WP-S2.16 before its specification.
- **FP-6** `tools/blind/README.md` names the play-tester's prompt, the visual help and the `first-timer` persona,
  which it lacks (step 3 writes the rows this package adds).

## 12. Open questions

- **O1** The package's shape (D1). Options: (1) as D1: the lane through R0, reviewed and merged, R1 in WP-S6.08's
  release cycle with its report read by the gate review; (2) the lane stays open through R1 and is reviewed once after
  it, its materials merged before review; (3) R1 on the merged tip before WP-S6.08's lane, not on a candidate.
  Recommendation: (1); WP-S6.08 depends on this package, and doc 16 runs R1 on the candidate.
- **O2** Doc 16's 12-of-15 line under the proxy. Options: (1) a reading: reported, never binding; what binds is that
  every severe finding (a core task not completed even with hints among them) is fixed and re-run; (2) binding: fewer
  than 12 unassisted visual attempts fail G-M2-20. Recommendation: (1); a model's rate is not NFR-UX-01's, and the
  severe rule already stops the release on any task the product does not let a persona finish.
- **O3** Run counts (§5.3). Options: (1) one visual attempt per persona and task; (2) two, by independent agents, for
  the counted tasks (about 2.6M tokens more in R1). Recommendation: (1), with a second sample run only for a task
  whose attempts disagree across personas.
- **O4** The frame. Options: (1) the default 1280 × 800 at 1.2, which the reading probe calibrated; (2) doc 16 §19.3's
  1920 × 1080 at 1.0 (1920 × 1108 frames), which model vision downscales; (3) one EPLAN persona at 1920. Recommendation:
  (1), with FP-2 before any change.
- **O5** NFR-UX-06 and Golden C. Options: (1) no persona run on Golden C; the row stays with the pilot log (G-M3 in the
  lean variant); (2) a text run on the private `.qet` outside the repository, nothing committed but its counts.
  Recommendation: (1); ID-341 keeps customer data away from the agents, and the private corpus never enters the
  repository.
- **O6** The questionnaire's wording. Options: (1) this project's own ten statements in SUS's form (§5.5), scored by
  SUS's arithmetic; (2) SUS's published statements with their attribution. Recommendation: (1); the figure is never
  SUS's anyway, and own wording leaves no licence question in a public repository.
- **O7** The size: the lane about 2.7 pw and the orchestrator's acts about 0.3 pw, the PERT of 3.1 pw unchanged.
  Recommendation: keep it.
- **O8** Checkers (D14). Options: (1) a checker only where an item is unmet or open, and on T6; (2) a checker on every
  run (about 1.8M tokens more in R1). Recommendation: (1); the mechanical decision is the oracle, and a checker reads
  what it cannot decide.

## 13. Documents to update

At the design pass after the lane:
- **Gates** ([03](../03-gates.md)), the three rows of `:951-953` to read:
  - **G-M2-20** | H | **R1 passed as a blind-persona round** (ID-1331; SC-9, NFR-UX-01 and NFR-UX-02 by proxy): two
    EPLAN personas and one QET persona attempt the core tasks T1 … T5 in picture-only runs on the release candidate;
    each attempt is reported as unassisted, with hints or not completed, beside doc 16's 12-of-15 line as a reading,
    never as NFR-UX-01's rate; the proxy questionnaire's figure is reported per persona, never as a SUS score; every
    severe finding (Nielsen 3 or 4; a core task that no persona completes even with hints is one) is fixed with a
    kittest flow and its persona's re-run on the next candidate completes the task; T6 (the QET personas, without the
    guide) is reported | R1 report (GP-10)
  - **G-M2-21** | H | **ADR-0007 validated by the blind-persona R1** (ID-1331; PD-10): the R1 report's validation
    record answers the ADR's three questions and amendment 2's junction defaults from the runs and the questionnaire,
    states what a proxy cannot answer, and proposes accept or amend; the ruling is recorded and the wiring-tool
    defaults are frozen | ADR status; R1 report
  - **G-M2-22** | M | The R0 report of the blind-persona formative round (ID-1331, CP-05) exists and its severe fixes
    are closed, each with a kittest flow, before R1 (NFR-UX-09) | R0 report
- Gates, also: CP-05 and CP-08 (`:337`, `:340`: personas, the development build and the first candidate); GP-10
  (`:491`: persona names as codes); the blind-drafter note (`:958`: R1 is now the blind-persona round).
- **Doc 16** §19: a paragraph on the proxy (ID-1331); §19.2's participants as personas; §19.3 as the harness's setup;
  §19.5's metrics as D9's counts and the PQ figure; §19.6's report location as built (`docs/ux/`); `:93`'s "usability
  evidence comes only from moderated sessions and pilot logs" with the rounds of personas.
- **02c** NFR-UX-01, NFR-UX-02, NFR-UX-04 and NFR-UX-09's acceptance as §3 reads them; NFR-PRIV-10 not applicable to
  R0 and R1. **Vision** SC-9 (a note: R1 by proxy).
- **ADR-0007**: the status line and "Validation before M2" with the record of D17 and the ruling.
- **Doc 20** §2 and §3.1: GP-UT as built, the basic project from the wizard's defaults, the R2 form in T10's prelude.
- **Doc 18** §3.17: the round form of the harness (start projects, hints, the questionnaire, `ROUND.md`).
- **Plan 01**: the R0 and R1 rows (`:104-105`, `:2033-2035`, `:2051`), the R1 participants' cost (`:2160`: none for
  R1), PD-10 (`:2192`).
- **Plan 02**: WP-S2.12's objective, scope (no consent forms), deliverables (the reports and the evidence) and owner
  role; the glossary's R0 and R1 row.
- **WP-S6.08**'s specification: §10.4 R2 and §13 O8 as ruled (R1 delivered by WP-S2.12 as blind-persona runs);
  **WP-S6.13**'s visual specification §9.2 (R1 on the first candidate).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S2.12, WP-S2.04, WP-S0.08, WP-S2.11, WP-S2.16, WP-S6.08, WP-S6.13,
  WP-S7.03, WP-S7.05; FT-713.
- [Decisions](../decisions.md): ID-1210, ID-1281, ID-1322, ID-1327, ID-1330, ID-1331, ID-1332, ID-1333;
  [decision log](../06-decision-log.md): ID-341, ID-612, ID-678, ID-700, ID-768, ID-776, ID-777, ID-785, ID-809,
  ID-810.
- [UX/UI](../../design/16-ux-ui.md) §19 and open questions 5, 7, 9;
  [non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-UX-01, NFR-UX-02, NFR-UX-04,
  NFR-UX-06, NFR-UX-09, NFR-PRIV-10; [vision](../../design/00-vision-and-scope.md) SC-9;
  [golden project](../../design/20-golden-project.md) §2, §3.1;
  [governance](../../design/19-open-source-governance.md) §3.7;
  [quality](../../design/18-quality-security-release.md) §3.17.
- [ADR-0007](../../adr/0007-explicit-connections-and-autoconnect.md).
- [Gates](../03-gates.md) CP-05, CP-08, GP-10, G-M2-20 to G-M2-22; the specifications of
  [WP-S6.13](WP-S6.13-blind-drafter-harness.md), its [visual runs](WP-S6.13-visual-playtest-runs.md),
  [WP-S6.08](WP-S6.08-release-stabilisation.md) and [WP-S2.11](WP-S2.11-gui-test-suite.md).
- `tools/blind/` (the prompts, personas, tasks and README); `xtask/src/blind/`.
