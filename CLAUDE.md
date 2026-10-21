# ElektraWatt — notes for Claude

Open-source (GPL-3.0-or-later app; MIT OR Apache-2.0 format crates) cross-platform desktop ECAD application in Rust
for electrical control-cabinet design. Currently in the design phase: see `docs/` (start at `docs/README.md`).

## Model routing policy (quota)

Use the cheapest model that can do the task well. Three tiers, each with project subagents in `.claude/agents/`:

| Tier | Model | Subagents | Use for |
|---|---|---|---|
| Lead | Opus | `lead-designer`, `lead-programmer` | Open decisions: architecture, ADRs, domain-model and file-format changes, cross-document conflicts, estimation/planning, judging alternatives; core/critical code (ew-model, ew-commands, ew-memo, ew-derive, ew-format/ew-store, concurrency, FFI/STEP worker, public & plugin APIs, perf-critical rendering); hard debugging; final review |
| Worker | Sonnet | `designer`, `programmer` | Well-defined work with a closed spec: write/update a doc section to an outline, apply decided resolutions, write an ADR for a made decision; implement a specified command/query/importer step/report/UI panel/CLI subcommand, tests, bug fixes with a known repro; research and literature sweeps |
| Mechanical | Haiku | `boilerplate`, `scout` (read-only search) | No judgment needed: scaffolding from a template, Cargo.toml edits, licence/REUSE headers, renames and ID-map find-and-replace, fmt/trivial lint fixes, fixtures, CHANGELOG/README, link and Mermaid fixes; repository searches and lookups (`scout`) |

**Defaults.** `.claude/settings.json` sets `CLAUDE_CODE_SUBAGENT_MODEL=sonnet`: any subagent or workflow agent that is
not given a model another way runs on Sonnet. Precedence (Claude Code docs): per-call `model` parameter > the agent
file's `model:` frontmatter > `CLAUDE_CODE_SUBAGENT_MODEL` > main-session model. So the Opus lead roles stay on Opus,
and Opus is otherwise used only when asked for explicitly. The built-in `Explore`, `Plan` and `general-purpose`
agents follow the main-session model, so prefer `scout` (Haiku) for searches, or pass `model` explicitly. The main
session itself is the lead (run it on Opus; `opusplan` = Opus while planning, Sonnet while executing, is an option).

Rules:
1. **The lead decides, workers execute.** Opus turns open questions into closed specs (inputs, outputs, files owned,
   acceptance checks); Sonnet/Haiku execute them. Do not give an open-ended design question to a worker tier.
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
be expensive or hard to detect (data, safety, legal)? Is verification weak (no tests/checks)?

- **Escalate on failure:** a task that comes back blocked, fails its verification, or is rejected in review is re-run
  one level up (with the failed attempt's notes). Two escalations without success → hand to `lead-designer` /
  `lead-programmer`.
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
- **Performance tests** (the `#[ignore]`d release-profile tests and hostile-input timings) run only at
  **milestone deliveries**, never in a fix round or a cycle gate. Perf claims in reports are marked
  "not measured (milestone rule)" unless a number came from a run that was needed to develop the fix.

## Disk and build concurrency (owner's rule, 2026-10-21)

- `.claude/settings.json` sets, for every Claude session in this project, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`
  (panics keep file and line; the PDBs lose most of their size), `CARGO_INCREMENTAL=0` and `CARGO_BUILD_JOBS=4`. Do
  not override them in a lane; a full-debuginfo build is a deliberate, one-off choice.
- A lane's worktree `target` is deleted the moment its work is integrated and re-verified (`cmd //c "rmdir /s /q
  <worktree>	arget"` when `rm -rf` fails on stale incremental entries); the main checkout's `target` is cleaned
  after every gate. The orchestrator keeps at least 150 GB free before a gate and never lets a third lane build while
  two are building.
- Agents never delete anything outside their own worktree; if the disk runs short they stop and report.

## Repository rules

- Commit on the working branch, then merge into `main` and push both (owner's standing instruction).
- Public repository: never commit customer or end-user names, addresses, project or drawing numbers, personal names,
  e-mail addresses, the reference drawing PDF, or local filesystem paths.
- Design decisions live in `docs/adr/`; requirements in `docs/design/02a/02b/02c-*.md`; the plan in `docs/plan/`.
