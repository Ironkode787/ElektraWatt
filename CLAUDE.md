# ElektraWatt — notes for Claude

Open-source (GPL-3.0-or-later app; MIT OR Apache-2.0 format crates) cross-platform desktop ECAD application in Rust
for electrical control-cabinet design. Currently in the design phase: see `docs/` (start at `docs/README.md`).

## Model routing policy (quota)

Use the cheapest model that can do the task well. Three Claude tiers, each with project subagents in
`.claude/agents/`, and an external DeepSeek coder that does not draw on the Claude quota. Since the owner's ruling of
2026-10-24 the Claude leads design and review; the DeepSeek coder does the research, the code and the small tasks:

| Tier | Model | Subagents | Use for |
|---|---|---|---|
| Coder (external) | DeepSeek Flash at max effort in the DeepSeek harness (Flash only: Pro is not used, owner's ruling 2026-10-24) | — (bridge `.claude/deepseek/dsh-task.py`) | Everything with a closed spec, at every D-level, in its own worktree: implementation (D4–D5 included once the lead's spec is closed), tests, fixtures, DSL verbs, schema/reference regeneration, fuzz arms, kittests, docs sections and as-built text to a decided outline, mechanical edits, and research (repository surveys, literature sweeps) in read-only rounds; Claude reviews, commits and merges (see "DeepSeek coder (dsh)") |
| Lead | Opus | `lead-designer`, `lead-programmer` | Design and review only (owner's rule, 2026-10-24): architecture, ADRs, domain-model and file-format decisions, cross-document conflicts, estimation/planning, judging alternatives; the closed specs the coder works from (the core crates — ew-model, ew-commands, ew-memo, ew-derive, ew-format/ew-store, concurrency, FFI, public & plugin APIs — get the most exact specs); the rulings; the review of every DeepSeek round (report → diff → acceptance checks re-run); hard debugging. A lead writes code or doc text itself only when a task failed on DeepSeek Flash twice (a second round with a sharpened brief; Pro is not used) |
| Worker | Sonnet | `designer`, `programmer` | Fallback coder/writer when dsh is unavailable or a task failed on Flash twice (Pro is not used); window checks and other verification that needs reading screenshots |
| Mechanical | Haiku | `boilerplate`, `scout` (read-only search) | `scout` for quick read-only lookups the orchestrator or a lead needs inside a turn; `boilerplate` only as the fallback for mechanical edits (headers, renames, count bumps) when dsh is unavailable |

**Defaults.** `.claude/settings.json` sets `CLAUDE_CODE_SUBAGENT_MODEL=sonnet`: any subagent or workflow agent that is
not given a model another way runs on Sonnet. Precedence (Claude Code docs): per-call `model` parameter > the agent
file's `model:` frontmatter > `CLAUDE_CODE_SUBAGENT_MODEL` > main-session model. So the Opus lead roles stay on Opus,
and Opus is otherwise used only when asked for explicitly. The built-in `Explore`, `Plan` and `general-purpose`
agents follow the main-session model, so prefer `scout` (Haiku) for searches, or pass `model` explicitly. The main
session itself is the lead (run it on Opus; `opusplan` = Opus while planning, Sonnet while executing, is an option).

Rules:
1. **The lead decides, the coder executes.** Opus turns open questions into closed specs (inputs, outputs, files
   owned, types/signatures, acceptance checks); the DeepSeek coder executes them, Sonnet/Haiku only as the fallback.
   Do not give an open-ended design question to the coder or to a worker tier.
2. **Escalate, don't guess.** A worker that hits an undecided choice stops and reports it; Haiku escalates to Sonnet
   when a task needs judgment or fails verification twice; Sonnet escalates to Opus for cross-cutting or
   architectural impact.
3. **Workflows:** agents default to Sonnet. Set `model: 'opus'` explicitly on judge/synthesis/triage/critic stages
   (keep Opus to one or a few agents per workflow) and `model: 'haiku', effort: 'low'` on mechanical stages; fan-out
   stages (per-file fixes, per-section writing to a fixed outline, per-item verification, sweeps) stay on Sonnet with
   `effort: 'medium'`.
4. **Effort:** `low` for mechanical work, `medium` for routine worker tasks, default/`high` only for lead work. The
   role subagents carry these levels in their frontmatter.
5. **Ad-hoc subagents:** use the role subagents above, or pass `model` to the Agent tool. New or changed files in
   `.claude/agents/` are hot-reloaded; only a brand-new `agents` directory needs a session restart.
6. **Verification is cheap, rework is not:** always end worker tasks with a mechanical check (build, tests, lint,
   link/Mermaid check, ID-existence grep) so failures surface before an Opus review.
7. **Leads design and review (owner's rule, 2026-10-24; replaces the Sonnet/Haiku delegation rule of 2026-10-25).**
   A lead lane (Opus) keeps for itself only the judgment — the specs, the rulings, the review of what came back — and
   hands every closed sub-task to the DeepSeek coder as a dsh round (see "DeepSeek coder (dsh)"): the implementation
   itself, tests to a given list, fixtures, DSL verbs, schema and reference regeneration, fuzz arms, kittests, docs
   notes and as-built text, mechanical edits, renames, count bumps, licence headers, re-blessing, and the research
   the spec needs. The orchestrator writes this into every lead brief; the lead's report lists its rounds (run name,
   model, number of rounds, outcome) and what it kept for itself and why. The lead reviews every DeepSeek round
   itself, at every D-level, and writes the fixes file itself (rule 9); Sonnet keeps the window checks.
8. **Specs leave no choice open (owner's rule, 2026-10-24).** A brief for the coder names the files owned, the exact
   types, signatures or schema, each behaviour with at least one example per branch, the error texts, the tests to
   write (names and what each asserts), the commands that must pass, and what not to touch. A choice the lead has
   not made is made before dispatch, never left to the coder: the coder brief tells it to stop and ask, and every
   open point costs a round.
9. **Fix feedback is accurate and well scoped (owner's rule, 2026-10-24 evening: "let the lead review the flash
   rounds, and give accurate, well scoped feedback for the fixes").** The lead reviews every DeepSeek round itself
   and writes the fixes file for `resume` itself. A fixes file lists numbered findings; each names the file and
   line, states what the round did against what the brief required with the evidence (the failing command or
   test, the observed and the expected text or value), gives the exact required change or behaviour and the test
   that proves it, and says what must not change. No finding without evidence, no "improve" or "clean up", no
   scope the brief did not have — a new wish is a new round with a new brief. The coder answers by number.

## DeepSeek coder (dsh)

The DeepSeek harness (`dsh`, npm `@deepseek-ai/dsh`, developer preview) codes; Claude Code orchestrates. dsh bills the
owner's DeepSeek key (`~/.dsh/.env`), not the Claude quota. Since the owner's ruling of 2026-10-24 the coder does all
the work that has a closed spec; the Claude tiers design, review and integrate. The bridge `.claude/deepseek/dsh-task.py` runs
`dsh --profile headless` in a worktree of its own; the coder reads `.claude/deepseek/AGENTS.md` (scope, git rules,
testing policy, public-repo rules, report format) instead of this file.

- **Routing.** Everything with a closed spec goes to DeepSeek, at every D-level — code (D4–D5 included once the
  lead's spec is closed), tests, regeneration, docs text to a decided outline, mechanical edits, and research in
  read-only rounds — on DeepSeek Flash at max reasoning effort (the bridge's default, owner's choice 2026-10-24); a
  round that fails on Flash is re-run once on Flash with a sharpened brief (what failed, the exact expected result);
  DeepSeek Pro is NOT used (owner's ruling, 2026-10-24: the current Flash outperforms Pro on the benchmarks). Open design questions,
  the rulings and the design pass stay with the leads; `programmer`/`designer`/`boilerplate` take over only when dsh
  is unavailable or a task still fails on the second Flash round.
- **Run a round** from the lead lane or the main session with Bash `run_in_background` (a round takes minutes to
  tens of minutes; the notification arrives on exit — a subagent too is re-invoked when its own background command
  exits, verified 2026-10-24). Write the brief to a file (goal, files owned, references, acceptance checks), then
  `python .claude/deepseek/dsh-task.py start <name> --task-file <brief> [--base <ref>] [--model flash|pro]`. It
  creates `.claude/worktrees/ds-<name>` on branch `ds/<name>` and prints the outcome, tokens, the changes against the
  base and the coder's `## Report`. `show <name>` re-prints a summary (also mid-round); `list` lists the runs.
- **Research rounds** are read-only: the brief names the questions and the findings file (`RESEARCH-<name>.md` at
  the worktree root, untracked; every claim cites `path:line` or a URL); the lead reads it, and `remove` deletes it
  with the worktree.
- **Review**: for D3 and above first a DeepSeek review round in a FRESH run (`start <name>-review --base ds/<name>`,
  read-only, findings to `REVIEW-<name>.md`), then the lead's own review: the report, then the diff (`git -C
  <worktree> diff <base>` and the untracked files listed), then the brief's acceptance checks re-run by the reviewer in a NORMAL worktree, never inside the `ds-*`
  worktree: apply the round's diff to the lead's or reviewer's own worktree (`git -C <ds worktree> diff --binary
  <base> | git apply --index`, untracked files copied; or commit on `ds/<name>` and merge) and run cargo there — the
  coder's check results are claims, not proof. The lead reviews every round itself (rules 7 and 9). For revisions
  write the findings to a fixes file in the form of rule 9 and run `resume <name> --task-file <fixes>` (same dsh session); after three rounds on
  Flash hand the worktree to `programmer` or `lead-programmer` (never `--model pro`: Pro is not used).
- **Integrate**: commit in the worktree in the project's message style with the line `Coded by DeepSeek <model> via
  dsh (run <name>).` above the trailers, merge `ds/<name>` into the working branch, then `remove <name>` (deletes
  the worktree with its `target` and the branch).
- **Limits.** The coder never commits, pushes or leaves its worktree. A round that builds counts as one of the two
  builders, and a lane runs one building round at a time. `--sandbox workspace` (default) confines its file writes to the worktree (dsh's Windows ACL sandbox;
  reads and network stay open; cargo builds and tests work under it); `--sandbox full` lifts it. A resumed session
  keeps the sandbox it started with, and approval prompts are always refused (headless has nobody to ask). Workflow
  agents cannot wait out a round (the 10-minute foreground limit), so rounds are started from the main session.
- **Low integrity (lesson 2026-10-24).** dsh's workspace sandbox labels the whole `ds-*` worktree Low Mandatory
  Level with inheritance: every file the coder writes or builds there — sources, test exes, `elektrawatt.exe` — is
  low-integrity. Launched from outside the sandbox, such an exe runs low-integrity: it cannot write to normal folders
  (the app opens projects read-only, tests fail on the temp dir) and Windows raises "publisher could not be verified"
  dialogs on the owner's screen. Therefore nobody runs cargo, nextest, clippy, rustdoc or a built binary inside a
  `ds-*` worktree from outside the sandbox; reviews, acceptance checks, window checks and gates run on a normal
  worktree that received the diff (the files copied or applied there take normal labels). Inside its own round the
  coder is unaffected.

## Effort by difficulty (dynamic routing)

Choose **model and effort per task from its expected difficulty**, not per role. When delegating with the Agent
tool or a workflow `agent()` call, always set both `model` and `effort` from this ladder (this instruction is the
explicit request that permits setting `effort` on delegated work):

| Level | Difficulty | Signals | Model / effort |
|---|---|---|---|
| D1 | Trivial | Mechanical, one file or pattern, fully specified, verifiable by a command | `haiku` / `low` |
| D2 | Routine | Closed spec, follows an existing pattern, ≤ 3 files, tests or checks exist | `sonnet` / `medium` |
| D3 | Moderate | Several files or docs, judgment inside a decided design, synthesis of many sources | `sonnet` / `high` |
| D4 | Hard | Open design choice, core crates, cross-cutting change, debugging without a repro | `opus` / `high` |
| D5 | Critical | File format, data-loss, safety, concurrency, security, licence boundary, irreversible | `opus` / `xhigh` |

Score with five questions; each "yes" adds a level above D1 (cap D5): Is the spec open (a choice is not yet
decided)? Does it touch more than 3 files, or a public API/format? Is there no existing pattern to copy? Would a mistake
be expensive or hard to detect (data, safety, legal)? Is verification weak (no tests/checks)? The level scores the
task; the model column applies to the work the Claude tiers do themselves (design, reviews, fallbacks). Code, docs
text and research with a closed spec go to the DeepSeek coder at every level (rule 7); the level still sets how exact
the spec must be and who reviews.

- **Escalate on failure:** a DeepSeek round that comes back blocked, fails its verification, or is rejected in review
  is re-run once on Flash with a sharpened brief, then handed to the Claude tier of the ladder's level (with the failed rounds' notes; Pro is not used); a Claude task
  that fails is re-run one level up. Two escalations without success → hand to `lead-designer` / `lead-programmer`.
- **De-escalate on evidence:** when a kind of task succeeds first time at a level three times in a row, start the
  next one a level lower.
- **Batch triage:** for fan-outs, score all items in one Haiku/`low` call, then route each item; use
  `.claude/workflows/routed-tasks.js` (run with Workflow `scriptPath`, `args: {tasks: [{id, prompt, difficulty?}]}`).
- The main session's own effort is chosen by the user; this ladder governs delegated work.
- **Changing models mid-run:** do not edit-and-resume a workflow whose stages run concurrently — replay follows call
  order, so finished agents can be re-run live. Stop it after the last expensive agent finishes and launch a fresh
  workflow containing only the remaining agents.

## Testing policy (owner's rule, 2026-10-18)

- **Agents run targeted tests only**: the crates they changed (`cargo nextest run -p <crate> …`), the matching
  `cargo clippy -p <crate> --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, and the policy
  checks their change touches (`cargo xtask check-deps`, `check-features`, `check-licence-map`, `reuse lint`).
  Never the workspace-wide suite, never `cargo xtask ci`.
- **The full suite** (`cargo xtask ci`, workspace nextest with and without `--all-features`, doctests, rustdoc,
  `cargo deny`, `cargo vet`) runs once per **cycle delivery** or **milestone delivery**, by the integrating session,
  right before the fast-forward into `main`.
- **Performance tests** (the `#[ignore]`d release-profile tests, hostile-input timings, `cargo xtask perf-gate`
  and the G-SP re-test) do **not run during development at all** (owner's rule, 2026-10-22): not in a fix round,
  not in a cycle gate, not at a milestone delivery. Performance criteria are assumed to pass and are addressed in
  the release phase. Perf claims in reports are marked "not measured (owner's rule)". The nightly `perf.yml` on the
  hosted runner stays report-only.

## Disk and build concurrency (owner's rule, 2026-10-21)

- `.claude/settings.json` sets, for every Claude session in this project, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`
  (panics keep file and line; the PDBs lose most of their size), `CARGO_INCREMENTAL=0` and `CARGO_BUILD_JOBS=4`. Do
  not override them in a lane; a full-debuginfo build is a deliberate, one-off choice.
- A lane's worktree `target` is deleted the moment its work is integrated and re-verified (`cmd //c "rmdir /s /q
  <worktree>	arget"` when `rm -rf` fails on stale incremental entries); the main checkout's `target` is cleaned
  after every gate. The orchestrator keeps at least 150 GB free before a gate and lets at most THREE builders run at once
  (owner, 2026-10-24, after the DeepSeek coder freed Claude quota: "We can add another lane"; two were the rule from
  the lowered-limits ruling of 2026-10-25 until then). A builder is a lane's building dsh round or its own cargo run,
  a gate, or a verify chain in the main checkout; a lane runs one building round at a time.
- Agents never delete anything outside their own worktree; if the disk runs short they stop and report.

## Repository rules

- Commit on the working branch, then merge into `main` and push both (owner's standing instruction).
- Public repository: never commit customer or end-user names, addresses, project or drawing numbers, personal names,
  e-mail addresses, the reference drawing PDF, or local filesystem paths.
- Design decisions live in `docs/adr/`; requirements in `docs/design/02a/02b/02c-*.md`; the plan in `docs/plan/`.
