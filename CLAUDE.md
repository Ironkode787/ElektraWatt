# ElektraWatt — notes for Claude

ElektraWatt is a desktop ECAD application in Rust for electrical control-cabinet design. It is open source:
GPL-3.0-or-later for the app, MIT OR Apache-2.0 for the format crates. It is an early alpha.

## Read first

- `docs/plan/README.md`: the milestone that is running and the open work.
- `CONTRIBUTING.md`, "The development loop": how a change is checked.
- `docs/README.md`: the map of the documents. Read a design document only for the area you change.
- To find code: `cargo xtask where <name>` (an action, a command, a type or a function, by its name) and
  `cargo xtask outline <file>` answer with the place and its span. `docs/map/crates.md` is where to look up one
  crate; do not read it or `docs/map/actions.md` whole (generated: `cargo xtask gen-maps`).
- `.claude/orchestration.md`: only when you hand work to subagents.

## The loop

```sh
cargo xtask check -p <crate> [-p <crate>]...   # format, lint, test what changed, repository rules
cargo xtask check -p <crate> -- <filter>        # the same, the tests narrowed to a nextest filter (no doctests)
cargo xtask check --deep                        # the whole workspace with the slow tests of the deep run
cargo xtask gen                                 # regenerate generated files after changing a command, rule, text, icon, guide page
cargo xtask rules                               # the repository rules alone
```

- Run `check` on the crates you changed and the crates that show the change. The whole workspace (`check` without
  `-p`, about seven minutes) runs at the start and the end of a milestone and before a release, not per change.
  The deep run (`check --deep`: the slow tests as well, about five minutes more; CONTRIBUTING.md, "Tests") runs at
  the same moments.
- `check` also replays the persona briefs on the built programs when the change touches `tools/blind/`, `content/`
  or `xtask/`, and always for the whole workspace. A check that passes writes down its tree; on the same tree it
  says so and runs nothing.
- There is no CI until 1.0 (owner, 2026-11-08). `check` is the check.
- No performance tests and no timing assertions during development (owner, 2026-10-22); a performance claim is
  marked "not measured".
- A test never pins the total of a growing set, a generated name or a path.
- One place for every item: no list kept in step by hand, no committed copy of something the code can produce.

## Proportion

This is an alpha of a desktop tool with one developer and one pilot (owner, 2026-11-08 and 2026-11-09: "lean,
efficient, and streamlined"). Before you add a check, a test, a rule, a document or an abstraction, say what it
catches or who reads it. Delete before you fix, fix before you add. The owner's hands-on test is the gate that
counts; a passing suite is not proof that the program works.

## Repository rules

- **Branches.** Commit on the working branch, `claude/m0-foundations`. At a delivery, fast-forward `main` and push
  both; at a checkpoint, push the working branch only. A release is cut from its own branch `release/<X.Y>`; only
  fixes for its candidates land there, each also on the working branch.
- **Public repository.** Never commit customer or end-user names, addresses, project or drawing numbers, personal
  names or e-mail addresses, the reference drawing PDF, local filesystem paths or agent IDs.
- **The licence boundary.** Permissive crates (`MIT OR Apache-2.0`) never depend on GPL crates, and they build on
  the Rust version their manifest names. `cargo xtask rules` and `cargo deny` guard it.
- **Downloads.** Fetch crates, tools and images without asking (owner, 2026-10-22). Keep versions pinned. The
  private corpus never enters the repository.
- **Decisions.** A decision that changes how something works goes into the document that describes it. Open work is
  a row of `docs/plan/backlog.md`; a finished row is deleted.
- **Commits.** Conventional subjects; the message says what changed and why. No AI `Signed-off-by`. Messages end
  with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Build and disk

- `.claude/settings.json` sets the build environment (no debug info, 8 jobs). Do not override it in a worktree.
- At most three builds at once, counting every subagent's cargo run.
- A subagent deletes nothing outside its own worktree. If the disk runs short, stop and report.
