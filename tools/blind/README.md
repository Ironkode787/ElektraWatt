# The blind drafter's tools

An agent that never sees a picture draws and edits in ElektraWatt through the shell's headless mode, with `ui.describe`
(`look`) and `page.describe` (`look --page`) as its only senses and the transcript as its evidence. The harness is
`cargo xtask blind` (Rust, in `xtask/src/blind/`); the prompts, personas, task sheets, reference files and recordings
live here. The specification is `docs/plan/specs/WP-S6.13-blind-drafter-harness.md`; the method is doc 18 §3.17.

| Path | What it is |
|---|---|
| `BLIND_DRAFTER.md`, `CHECKER.md` | The drafter's and the checker's prompts (front matter `prompt`, `version`); a change is a reviewed diff with a version bump |
| `personas/qet-veteran.md`, `personas/switcher.md` | The personas (front matter `persona`, `version`): fictional roles, never people |
| `HELP.md` | The protocol as the agent reads it (composed into the prompt, printed by `{blind} help`) |
| `PLAYTESTER.md`, `HELP-VISUAL.md` | A visual run's (`--senses visual`) play-tester prompt and its harness help (front matter `prompt`, `version`): `compose` uses them in place of `BLIND_DRAFTER.md` and `HELP.md`, and `{blind} help` prints `HELP-VISUAL.md` alone (WP-S6.13 part 4) |
| `personas/first-timer.md` | The play-tester persona of part 4: an electrician new to ECAD, who reads every label and opens the guide when lost |
| `tasks/<task>/` | A task: `task.md`, `prelude.uiscript`, and for the self-test `agent.jsonl`; a brief also `prelude.toml` (the class of each prelude step) |
| `tasks/free/` | The free task: no oracle, no score |
| `recordings/<name>/` | A blessed run: the replay inputs and the evidence, never the full transcript |
| `ROUND.md` | The usability rounds' prompt (WP-S2.12 D11, front matter `prompt`, `version`): the hint command and the proxy questionnaire; `compose` adds it after the task sheet of a brief whose `task.toml` names `rounds` |
| `personas/eplan-designer.md`, `personas/eplan-maintainer.md`, `personas/qet-user.md` | The personas of the usability rounds (WP-S2.12 D10): R0 runs `qet-veteran` and `qet-user`, R1 `eplan-designer`, `eplan-maintainer` and `qet-user` (T6 also `qet-veteran`); every persona is senses-independent, the run's prompt says how it acts |
| `tasks/brief-ut-<t>-<name>/` | A usability task of doc 16 §19.4 (WP-S2.12 §5.1) as a brief: `task.toml` adds `task_id`, `rounds` and `start` (`sample`: the run starts on a copy of the sample project, `fixtures/sample/sample.ew`, which the prelude may change first, as T1 and T2 take away the 24 V breaker of folio 102), `acceptance.toml` may decide an item by `actions` done in order, and `hints.toml` holds one to three hints that `{blind} hint` gives one at a time |

`cargo xtask blind check-task <task>` checks a task folder without a product (the sheet and the prelude, and for a brief
that starts on the sample that every identifier of `task.md` is in the sample's script). Run it after any change of a
task folder.

The check run's filter admits the actions of kind `View` and `file.open`, `app.quit`, **except** the canvas actions that can
reach a tool's commit (FT-442): `canvas.press`, `canvas.release`, `canvas.activate` and `canvas.activate_alt` (a press, a
release and a click at the grid cursor), `tool.finish`, and any `canvas.drag*`. It keeps navigation, zoom, pan, tabs, the page
navigator, pointing (`canvas.pointer`, `canvas.dwell`), the grid cursor, `tool.select`, `tool.measure` and the looks.

## The orchestrator's procedure (outside CI, never by an agent)

1. Build: `cargo build -p xtask -p elektrawatt -p ew`, then run `cargo xtask docs-gen` (it writes the action list the harness reads, `target/reference/actions.txt`, never committed), then `mdbook build docs/book`
   (the guide, `target/book`; its reference pages are built, not committed).
2. `cargo xtask blind start --kind brief --task brief-ut-t1-place --persona qet-user --run ut1-r1` starts a run in
   `<temp>/ew-blind/ut1-r1/` (outside the repository; `--out` names another folder) with copies of the
   harness, the product binaries, the guide and the starter content, an empty sample folder, and a keeper (a loopback
   process of the copied harness; `--stateless` for agent shells that block loopback or kill detached processes). It
   prints the path of `prompt.md` and of the agent's command. Without the built guide (`target/book/index.html`) it
   refuses; `--no-book` starts the run with an empty book folder.
3. Start one agent (Sonnet) with `<run>/prompt.md` as its whole instruction and its working directory at `<run>/`.
   Permission rules **allow** only running `<run>/bin/xtask blind ...` and reading `<run>/prompt.md` and `<run>/book/**`,
   and **deny** everything else: the repository, `<run>/.harness/`, `<run>/work/`, `<run>/config/`, other runs, file
   writes, network tools. The harness does not write them; the orchestrator does.
4. `cargo xtask blind stop ut1-r1`: the epilogue (a `page.describe` per folio, a save only if the project is dirty,
   `app.quit`), the exports of the copied `ew` (PDF on 1 and 4 threads, the parts list, `ew check` as text and JSON),
   the score (`out/score.json`: a brief run's acceptance items decided mechanically, a free run's protocol) and
   `out/report.md`.
5. `cargo xtask blind start --kind check --task brief-ut-t1-place --of ut1-r1 --run ut1-r1-check` for the checker (view
   actions only, on a copy of the project; the drafter's check report, parts list and score are copied into its
   `reports/` for `report <run> check|bom|score`, and its folios are the drafter's), then `stop` it.
6. `cargo xtask blind replay ut1-r1 --twice` (R1 and R2 must hold), then `cargo xtask blind bless ut1-r1 --as ut1-1
   --replay check` (`--replay none` keeps a long recording as evidence only).
7. An Opus lead reads `drafter.md`, `handback.md`, `checker.md`, `out/report.md` and the PDF, checks the agents' tool
   logs for any call outside the allowed commands (a violation voids the run, which is not blessed), and writes the
   findings as follow-up tasks.

A lost session (the keeper ended after its idle period, or died): `cargo xtask blind resume <run>` replays the prelude
and every accepted step and goes on when the outcomes repeat; otherwise the run ends `diverged`.

## The self-test

`cargo xtask blind selftest` builds `elektrawatt` and `ew`, plays the scripted agent of `tasks/selftest/agent.jsonl`
through the real keeper and the copied binaries, stops the run, blesses it as `recordings/selftest/` (`replay = "check"`) and
replays it twice in fresh folders; it then plays the same agent with `--stateless` and compares the files, the transcript
and the outcomes. `bin/elektrawatt/tests/it/blind_replay.rs` replays the committed recording in CI. **A change of the
product that breaks the recording is repaired in the same change by running the self-test again; the self-test recording
is never marked stale.** Another recording that no longer replays after an intended change gets `"stale": "<reason>"`
in its `run.json` (a reviewed one-line change) until the next live run replaces it.

## Rules of the files

Everything here is English, carries no customer or personal name, no address, no project or drawing number other than the
sample project's placeholders, no e-mail address and no local path (the confidential-token rule of `cargo xtask rules`). The licences are
in `ci/licence-map.toml`: CC-BY-4.0 for this folder, GPL-3.0-or-later for `reference/`.
