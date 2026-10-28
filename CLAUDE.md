# ElektraWatt — notes for Claude

Open-source (GPL-3.0-or-later app; MIT OR Apache-2.0 format crates) cross-platform desktop ECAD application in Rust
for electrical control-cabinet design. Currently in the design phase: see `docs/` (start at `docs/README.md`).

## Orchestration (owner's rules, 2026-10-27)

The main session orchestrates: it plans each cycle, briefs the subagents, integrates their work and delivers. All
work is done by Claude subagents; there is no external coder.

0. **Sonnet is excluded until the owner lifts it (owner, 2026-10-28: Sonnet drove the excessive usage).** Choose
   only Opus or Haiku; wherever a rule below names Sonnet, use Opus at a lower effort for work that needs judgment
   and Haiku for closed, mechanical work. Leads may not delegate to Sonnet (`programmer`/`designer` run with an
   explicit `model`).
1. **Model per task.** For every task, work package or bundle in a cycle the orchestrator chooses the subagent's
   model — Opus, Sonnet or Haiku — and passes it explicitly (`model` on the Agent tool or a workflow `agent()`
   call). The role agents in `.claude/agents/` are templates for the brief, not a fixed routing.
2. **Effort fits the task.** Pass `effort` explicitly on every delegated task, sized to the work: `low` for
   mechanical edits and lookups, `medium` for routine closed work, `high` for reviews and hard work, `xhigh`/`max`
   only where a mistake is very costly (file format, data loss, concurrency, security, licence boundary).
3. **Ambiguity goes up a class, not up in effort.** A task with open questions or judgment calls goes to a higher
   model class at a lower effort (Opus at `medium` rather than Sonnet at `max`; Sonnet at `low`/`medium` rather
   than Haiku at `high`). Effort buys depth on a well-posed problem; it does not buy judgment.
4. **Do not underestimate Sonnet and Haiku.** Sonnet 5.5 is close to Opus 5.5 on coding benchmarks (SWE-bench Pro
   81.3 against 89.9, both at max effort, per Anthropic's system card) and handles most closed implementation,
   tests, reviews of contained changes and docs to a decided outline. Haiku 5.5 is at or above the previous
   Sonnet on several coding benchmarks and handles lookups, mechanical edits, fixtures, regeneration and well-
   specified small code changes. Reserve Opus for open design, core-crate changes with subtle invariants, hard
   debugging and the cycle review.
5. **Briefs match the model.** The lower the class, the clearer the brief: name the goal, the files owned, the exact
   types/signatures or text, the behaviour with an example per branch, the error texts, the tests to write and what
   each asserts, the commands that must pass, what not to touch, and when to stop and report instead of choosing.
   An Opus brief may state the goal and constraints and leave the design to it; a Haiku brief leaves nothing open.
6. **One independent review per cycle.** After the cycle's work is integrated on the working branch and before the
   gate, one fresh-context Opus subagent at `high` effort reviews the cycle's total diff (`git diff <last delivered
   main>..<tip>`) against the ADRs, the specs and the design docs: format bytes and data loss first, then
   determinism, panics, spec gaps, tests, CI/supply chain and repository hygiene. It did not write the briefs and is
   given the code, not the orchestrator's conclusions. It reports ranked findings with `path:line` and evidence,
   and a verdict (accepted / not accepted). The findings go to a fix subagent on Sonnet at `xhigh` effort (while rule 0 holds: Opus at
   `medium`) (in its
   own worktree, the findings verbatim as its brief); then the same reviewer (continued with SendMessage; a fresh
   Opus `high` reviewer with the previous findings if it is gone) re-reviews the fixes and the remaining diff.
   Fix → re-review repeats until the reviewer accepts; only then does the cycle go to the gate.
7. **Isolation and integration.** Every subagent that edits files runs with `isolation: "worktree"` and commits on
   its own branch; the orchestrator picks or merges onto the working branch, regenerates generated files once on
   the merged tip, and verifies before claiming the tip is green. Subagent check results are claims until re-run.
8. **Cycle rhythm.** Plan and dispatch → integrate → window checks of user-visible changes (a model that reads
   screenshots, on a normal build) → design pass (decision-log rows, FT rows, as-built spec text, doc 01 cycle
   section, next candidates) → the cycle review (rule 6), fix and re-review until accepted → one full gate → fast-forward `main`, push,
   watch CI on the full SHA. Report outcomes faithfully, including what was skipped or not proven.

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
  not override them in a worktree; a full-debuginfo build is a deliberate, one-off choice.
- A worktree's `target` is deleted the moment its work is integrated and re-verified (`cmd //c "rmdir /s /q
  <worktree>\target"` when `rm -rf` fails on stale incremental entries); the main checkout's `target` is cleaned
  after every gate. Keep at least 150 GB free before a gate and let at most THREE builders run at once (a
  subagent's cargo run, a gate, or a verify chain in the main checkout each count as one).
- Agents never delete anything outside their own worktree; if the disk runs short they stop and report.

## Repository rules

- Commit on the working branch, then merge into `main` and push both (owner's standing instruction).
- Public repository: never commit customer or end-user names, addresses, project or drawing numbers, personal names,
  e-mail addresses, the reference drawing PDF, or local filesystem paths.
- Design decisions live in `docs/adr/`; requirements in `docs/design/02a/02b/02c-*.md`; the plan in `docs/plan/`.
