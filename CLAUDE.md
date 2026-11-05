# ElektraWatt — notes for Claude

ElektraWatt is a cross-platform desktop ECAD application in Rust, for electrical control-cabinet design. It is open
source: GPL-3.0-or-later for the app, MIT OR Apache-2.0 for the format crates. The documentation starts at
`docs/README.md`.

## Who reads what

- **The orchestrating (main) session** follows `.claude/orchestration.md`. Read it at the start of every session.
- **Subagents** follow their agent definition in `.claude/agents/` (`lane`, `reviewer`, `checker`, `scout`) and their
  brief. From this file they need only the rules below.

## Repository rules

- **Branches.** Commit on the working branch. At a delivery, fast-forward `main` and push both; at a checkpoint, push
  the working branch only (owner's standing instruction, amended 2026-10-31).
  - A release has its own branch, `release/<X.Y>`, cut at a delivery. Its candidates are tagged on it, and only fixes
    for them land there, each also on the working branch (owner, 2026-11-05, ID-1412).
- **Public repository.** Never commit any of these:
  - customer or end-user names, addresses, project or drawing numbers;
  - personal names or e-mail addresses;
  - the reference drawing PDF;
  - local filesystem paths or agent IDs.
- **Golden A.** Never open `golden/J-E-2416.ew` in a writing or locking mode.
- **Where things live.** Design decisions are in `docs/adr/`, the requirements in `docs/design/02a/02b/02c-*.md`, and
  the plan in `docs/plan/`.
- **The licence boundary.** Permissive crates (`MIT OR Apache-2.0`) never depend on GPL crates. `cargo deny`,
  `cargo vet` and the licence map guard it.
- **Downloads.** Fetch crates, tools and images without asking (owner, 2026-10-22). Keep pins and digests,
  `cargo deny`/`cargo vet`, the licence boundary and the permissive MSRV. The private corpus never enters the
  repository.
- **Commits.** No AI `Signed-off-by`. Commit messages end with `Co-Authored-By: Claude Opus 5.5
  <noreply@anthropic.com>`.

## Testing policy (owner, 2026-10-18; amended 2026-10-22, 2026-10-31, 2026-11-01)

- **Agents run targeted tests only.** Never the workspace suite, never `cargo xtask ci`. The agent definitions give
  the edit loop (one package set, `--all-features`, the nextest `lane` profile) and the hand-back checks.
- **The full suite runs once per delivery,** as the CI run on the working branch.
- **No performance tests during development** (2026-10-22: "Assume it passed"):
  - no `#[ignore]`d release-profile tests, no timing assertions, no `cargo xtask perf-gate`;
  - perf claims are marked "not measured (owner's rule)".
- **Tests never pin the total of a growing set** (CONTRIBUTING.md).

## Build and disk (owner, 2026-10-21; amended 2026-10-28, -29, -31, 2026-11-01)

- **Environment.** `.claude/settings.json` sets the build environment: no debug info (ID-1440) and 8 jobs. Local
  builds are incremental. Do not override these settings in a worktree.
- **Builders.** At most three at once. A subagent's cargo run, a verify and a gate each count as one.
- **Disk.** Agents delete nothing outside their own worktree. If the disk runs short, they stop and report.
