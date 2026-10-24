# The blind drafter's tools

An agent that never sees a picture draws and edits in ElektraWatt through the shell's headless mode, with `ui.describe`
(`look`) and `page.describe` (`look --page`) as its only senses and the transcript as its evidence. The harness is
`cargo xtask blind` (Rust, in `xtask/src/blind/`); the prompts, personas, task sheets, reference files and recordings
live here. The specification is `docs/plan/specs/WP-S6.13-blind-drafter-harness.md`; the method is doc 18 §3.17.

| Path | What it is |
|---|---|
| `HELP.md` | The protocol as the agent reads it (composed into the prompt, printed by `{blind} help`) |
| `reference/actions.txt`, `reference/vocabulary.txt` | The action registry with the parameter signatures and the describe vocabulary, **generated** by `EW_BLESS=1 cargo nextest run -p ew-viewmodel --test blind_reference`; the harness reads them, and a test fails when the product and they differ |
| `tasks/<task>/` | A task: `task.md`, `prelude.uiscript`, and for the self-test `agent.jsonl` |
| `recordings/<name>/` | A blessed run: the replay inputs and the evidence, never the full transcript |

## The orchestrator's procedure (outside CI, never by an agent)

1. Build: `cargo build -p xtask -p elektrawatt -p ew` and `mdbook build docs/book` (the guide, `target/book`).
2. `cargo xtask blind start --kind gate --task gpp-101-103 --persona qet-veteran --folios 101,102,103 --run gpp-r1`
   starts a run in `<temp>/ew-blind/gpp-r1/` (outside the repository; `--out` names another folder) with copies of the
   harness, the product binaries, the guide and the starter content, an empty sample folder, and a keeper (a loopback
   process of the copied harness; `--stateless` for agent shells that block loopback or kill detached processes). It
   prints the path of `prompt.md` and of the agent's command.
3. Start one agent (Sonnet) with `<run>/prompt.md` as its whole instruction and its working directory at `<run>/`.
   Permission rules **allow** only running `<run>/bin/xtask blind ...` and reading `<run>/prompt.md` and `<run>/book/**`,
   and **deny** everything else: the repository, `<run>/.harness/`, `<run>/work/`, `<run>/config/`, other runs, file
   writes, network tools. The harness does not write them; the orchestrator does.
4. `cargo xtask blind stop gpp-r1`: the epilogue (a `page.describe` per folio, a save only if the project is dirty,
   `app.quit`), the exports of the copied `ew` (PDF on 1 and 4 threads, the parts list, `ew check` as text and JSON),
   the second hash of `golden/`, and `out/report.md`.
5. `cargo xtask blind start --kind check --task gpp-101-103 --of gpp-r1 --run gpp-r1-check` for the checker (view
   actions only, on a copy of the project), then `stop` it.
6. `cargo xtask blind replay gpp-r1 --twice` (R1 and R2 must hold), then `cargo xtask blind bless gpp-r1 --as gpp-1
   --ci nightly`.
7. An Opus lead reads `drafter.md`, `handback.md`, `checker.md`, `out/report.md` and the PDF, checks the agents' tool
   logs for any call outside the allowed commands (a violation voids the run, which is not blessed), and writes the
   findings as follow-up tasks.

A lost session (the keeper ended after its idle period, or died): `cargo xtask blind resume <run>` replays the prelude
and every accepted step and goes on when the outcomes repeat; otherwise the run ends `diverged`.

## The self-test

`cargo xtask blind selftest` builds `elektrawatt` and `ew`, plays the scripted agent of `tasks/selftest/agent.jsonl`
through the real keeper and the copied binaries, stops the run, blesses it as `recordings/selftest/` (`ci = "pr"`) and
replays it twice in fresh folders; it then plays the same agent with `--stateless` and compares the files, the transcript
and the outcomes. `bin/elektrawatt/tests/blind_replay.rs` replays the committed recording in CI. **A change of the
product that breaks the recording is repaired in the same change by running the self-test again; the self-test recording
is never marked stale.** Another recording that no longer replays after an intended change gets `"stale": "<reason>"`
in its `run.json` (a reviewed one-line change) until the next live run replaces it.

## Rules of the files

Everything here is English, carries no customer or personal name, no address, no project or drawing number other than the
golden project's placeholders, no e-mail address and no local path (`cargo xtask check-anonymisation`). The licences are
in `ci/licence-map.toml`: CC-BY-4.0 for this folder, GPL-3.0-or-later for `reference/`.
