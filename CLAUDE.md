# ElektraWatt — notes for Claude

Open-source (GPL-3.0-or-later app; MIT OR Apache-2.0 format crates) cross-platform desktop ECAD application in Rust
for electrical control-cabinet design. Currently in the design phase: see `docs/` (start at `docs/README.md`).

## Orchestration

The main session orchestrates: it plans each cycle, briefs Claude subagents, integrates their work and delivers.
There is no external coder (owner, 2026-10-18, 2026-10-27). It is also product owner (delegated): it decides open
product questions, golden changes and design taste, and records them as "project owner (delegated)" in the decision
log. Real-world owner actions stay the owner's: legal, GitHub settings, funding, the pilot (2026-10-18, -27, -28).

### Models and briefs (owner, 2026-10-27)

0. **No Sonnet until the owner lifts it** (2026-10-28, usage). Choose Opus or Haiku. Leads may not delegate to
   Sonnet, and `programmer` and `designer` run with an explicit `model`.
1. **Model per task**, passed explicitly on every Agent or workflow `agent()` call. `.claude/agents/` holds brief
   templates, not a routing.
2. **Effort fits the task**, passed explicitly: `low` for mechanical edits and lookups, `medium` for routine closed
   work, `high` for reviews and hard work, `xhigh`/`max` only where a mistake is very costly (file format, data loss,
   concurrency, security, the licence boundary).
3. **Ambiguity goes up a class, not up in effort:** Opus at `medium` rather than Haiku at `high`. Effort buys depth,
   not judgment.
4. **Do not underestimate Haiku 5.5:** lookups, mechanical edits, fixtures, regeneration, well-specified small code.
   Sonnet 5.5, when allowed, is close to Opus on coding (SWE-bench Pro 81.3 against 89.9). Opus is for open design,
   core crates with subtle invariants, hard debugging and the review.
5. **Briefs match the model** (the orchestrator's brief templates are kept outside the repository). The lower the
   class, the clearer the brief: goal, files owned, exact signatures or text, behaviour with an example per branch,
   error texts, tests and what each asserts, commands, what not to touch, when to stop. A Haiku brief leaves nothing
   open. Delegation is required or forbidden, never "you may". Every UI brief cites
   `docs/design/16a-design-language.md` (2026-10-28).

### The cycle (owner, 2026-10-28, 2026-10-29, 2026-10-31)

6. **Lanes in waves on three slots.** A cycle is about twice the work of a cycle-30 cycle, built as sequential waves
   (2026-10-28: "more wps, not parallel"). At least two of the three builder slots carry work-package parts; polish,
   FT, fix and verify work share the third (2026-10-31). The rule holds at every moment while work-package lanes
   are queued, the end phase included: fixes and the last verify run one after another in the third slot, and only a
   major's fix may take a work-package slot, by delaying the next work-package dispatch (2026-10-31). A finished
   lane's slot takes the next queued lane. Docs-only lanes write the next cycle's specifications during the current
   one (2026-10-31).
7. **A wave ends at a merge point.** Integrate the lanes that are back, regenerate the generated files once, and run
   one merged-tip verify on `cargo xtask affected`'s selection (every package the changes reach, bin crates
   included) plus every policy check (2026-10-29, 2026-10-31); a checkpoint's last verify runs the full suite.
   Then, beside the next wave: the wave's review part, a window check if users see a change, and the wave's
   design-pass part.
8. **The end phase overlaps the next cycle (2026-10-31, ruling C).** After the last wave come the last review part
   and its fixes, the last window check, the visual play-test at a delivery (rule 19), the last design-pass part and
   the last verify. Meanwhile the free slots build the next cycle's first wave on the last wave's verified tip.
   Those lanes are integrated after the end tip and reviewed in the next cycle.
9. **Checkpoints and deliveries alternate (2026-10-31):** cycle 40 is a checkpoint, 41 a delivery, and so on.
   - **Checkpoint:** after the last verify, the design pass and the accepted review, push the working branch. `main`
     does not move and CI does not run.
   - **Delivery:** the gate is CI. Push the working branch, dispatch `ci.yml` and `docs.yml` on it, watch both on the
     full SHA, and fast-forward `main` and push only when both are green. Locally the last verify runs only what CI
     lacks: the blind selftests, `golden_changes.py --check` and the Windows default-features run.
   - **While CI runs,** the next cycle's lanes may be integrated locally but not pushed or reviewed. A CI fix goes onto
     the end tip, and the unpushed commits are rebased onto it.
   - Local Windows and WSL gates are only a fallback for when CI cannot run.
10. **Continuous cycles (2026-10-28).** Never wait for the owner; only an explicit hold stops the work. When a cycle's
    first wave is dispatched, post its table (lane | model | effort | task | wave), and add rows for later lanes.

### Review and fixes (owner, 2026-10-27; amended 2026-10-31)

11. **One independent reviewer per cycle,** a fresh-context Opus subagent at `high`, reviews in parts, one per wave
    (`<previous part's tip>..<wave tip>`, on the verified tip).
    - It reviews against the ADRs, the specs and the design docs: format bytes and data loss first, then determinism,
      panics, spec gaps, tests, CI and supply chain, repository hygiene.
    - It gets the code, not the orchestrator's conclusions.
    - It reports ranked findings with `path:line` and evidence, and a verdict per part.
    - It is continued with SendMessage; a fresh Opus `high` reviewer with the previous findings takes over if it is
      gone.
    - Its cargo runs count as a builder: while all three slots are busy it only reads, and its test runs wait for
      the third slot.
    - The cycle is accepted when every part is.
12. **Triage.** Majors, and minors that touch data, the file format or what users see, are fixed in the cycle. Other
    minors and nits become FT rows for one batch lane in the next cycle's first wave, without re-review. Window-check
    and play-test findings are triaged the same way; after the last wave, only their blockers and majors are
    fixed before the end tip, the rest go to the next cycle's first polish lane (2026-10-31).
13. **Back to the author.** Findings go verbatim, with the orchestrator's rulings, to the authoring lane via
    SendMessage. Its worktree is kept, without `target`, until its part is accepted. A fresh fixer (Opus `medium`) is
    used only when that lane is gone or a fix spans lanes.
14. **No separate re-review rounds.** The reviewer checks earlier fixes first in its next part: each finding closed,
    with the test that shows it. The last part ends with a re-review scoped to the fixes. Only a major re-opens a
    full re-review. A fix that lands after the cycle's review is accepted (a play-test blocker) opens the next
    cycle's first review part, named in its range (cycle-40 retro).
15. **Shift left.** Every lane self-checks its diff against the review checklist and reports the result: whole-text
    assertions; words, not IDs, in user texts; every call site of a changed behaviour; keys from the keymap; bounded
    memory and time on hostile input; all-or-nothing file writes.

### Integration and checks (owner, 2026-10-27, 2026-10-29)

16. **Isolation.** Every editing subagent runs with `isolation: "worktree"`, resets to the tip its brief names, and
    commits on its own branch; it never checks out, resets, stashes or builds in the main checkout. The orchestrator
    merges onto the working branch, regenerates once on the merged tip (`cargo xtask settle` once it exists; the
    main checkout sets `git config merge.regen.driver true` for its `merge=regen` files, 2026-10-31) and verifies
    before calling the tip green. Subagent check results are claims until re-run.
17. **Feasibility read before dispatch** of every brief, window checks included.
    - Haiku `high` checks with a checklist: named items exist, a claimed bug reproduces, files and format touched,
      consumers of a changed behaviour, an item's version already on `main`.
    - Opus `low` reads flagged briefs only: a format or schema, a check rule's meaning, a core crate, a bug that did
      not reproduce.
    - For a brief that touches a core crate, the file format or golden data, the orchestrator writes its rulings
      after the Haiku read, and the Opus `low` read checks those rulings (cycle-40 retro).
18. **Upkeep** (re-bless, regeneration, seed corpus, count bumps) runs through `cargo xtask settle` at each merge
    point once it exists, and lanes run `settle --check`; a Haiku lane takes what settle cannot do (2026-10-31).
    Tests never pin totals of growing sets (CONTRIBUTING.md).
19. **Window checks** run per wave, beside the next wave, and once at the end.
    - They render offscreen only (`--visual`) and never drive the owner's desktop (2026-10-28). If a real window is
      ever unavoidable, check that the app has the focus before every key or click, stop if it does not, and close it
      with WM_CLOSE.
    - Look and hierarchy are judged at 1920 × 1080, truncation at the 1280 × 800 floor. A floor fix scales; it never
      caps (2026-10-31).
    - Reports list every refused or failed step of `log.jsonl` and quote the `strings.rs` key of a text finding. A
      finding from a refused step is a script error.
    - A visual play-test runs at each delivery, on a brief whose areas are frozen, and at the gates of UI-polish
      cycles and milestones (2026-10-29, amended 2026-10-31).

### Golden, downloads, design pass, retro

20. **Golden changes** are decided by the orchestrator (2026-10-27). Lanes re-bless an intended change and list every
    re-blessed file; the orchestrator looks at each before it lands and records it. Never open `golden/J-E-2416.ew`
    in a writing or locking mode.
21. **Downloads** (2026-10-22): fetch crates, tools and images without asking. Keep pins and digests,
    `cargo deny`/`cargo vet`, the licence boundary and the permissive MSRV. The private corpus never enters the
    repository.
22. **The design pass** runs per wave and once at the end: decision-log rows, FT rows, as-built spec text, doc 01's
    cycle section and the next candidates. Report outcomes faithfully, including what was skipped or not proven
    (2026-10-27).
23. **A retro after every cycle,** checkpoint or delivery (2026-10-29, -31), by the orchestrator without agents.
    - Measure the cycle from its own record: spacing, phases, and rework (merge breakages, failed verifies, stopped
      lanes, fix-round size).
    - Name at most three problems with evidence and decide at most three changes.
    - Check the previous retro's changes.
    - Report with the next cycle's table.
    - A rule change enters CLAUDE.md with the next cycle's first commit, never into a verified end tip.

## Testing policy (owner, 2026-10-18; amended 2026-10-22, 2026-10-31)

- **Agents run targeted tests only.** Never the workspace suite, never `cargo xtask ci`. They run:
  - `cargo nextest run -p <crate> --all-features` for the crates they changed and their dependents, once: the
    default-features run is the merged-tip verify's (owner, 2026-10-31);
  - the matching `cargo clippy -p <crate> --all-targets --all-features -- -D warnings`;
  - `cargo fmt --all --check` (per file where a worktree path is too long);
  - the policy checks their change touches (`check-deps`, `check-features`, `check-licence-map`, `reuse lint`).
- **The full suite runs once per delivery** (every second cycle) or milestone delivery, as the CI run on the working
  branch before the fast-forward: `cargo xtask ci`, workspace nextest with and without `--all-features`, doctests,
  rustdoc, `cargo deny`, `cargo vet`. The integrating session runs locally only what CI lacks. CI-only checks (such as
  cargo-about) also run in the merged-tip verify when the lock file changes.
- **No performance tests during development** (2026-10-22: "Assume it passed"): no `#[ignore]`d release-profile
  tests, hostile-input timings, `cargo xtask perf-gate` or G-SP re-test, in any fix round, cycle or milestone
  delivery. Performance is handled at release. Perf claims are marked "not measured (owner's rule)", and the nightly
  `perf.yml` stays report-only.

## Disk and build concurrency (owner, 2026-10-21; amended 2026-10-28, 2026-10-29, 2026-10-31)

- **Environment.** `.claude/settings.json` sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only`, `CARGO_INCREMENTAL=0` and
  `CARGO_BUILD_JOBS=8`. Do not override them in a worktree. Dependencies build without debug info (`Cargo.toml`,
  owner, 2026-10-31).
- **At most three builders at once:** a subagent's cargo run, a verify chain and a gate each count as one.
- **Jobs.** Memory is the limit (24 cores, 32 GB).
  - 8 jobs per builder while up to three run; 16 for a lone verify; 12 for a fallback local gate on either host.
  - After a resource failure, run `cargo clean` before the re-run.
  - The Windows and WSL gates run one after the other.
- **Targets.** Delete a worktree's `target` once its work is integrated and re-verified (`cmd //c "rmdir /s /q
  <worktree>\target"` when `rm -rf` fails). Clean the main checkout's `target` after every last verify and gate.
- **Free space.** At least 150 GB before a gate (owner); the orchestrator's guard is 250 GB before every dispatch and
  verify (cycle-36 retro). Agents delete nothing outside their own worktree; if the disk runs short, they stop and
  report.

## Repository rules

- Commit on the working branch. At a delivery, fast-forward `main` and push both; at a checkpoint, push the working
  branch only (owner's standing instruction, amended 2026-10-31).
- Public repository: never commit customer or end-user names, addresses, project or drawing numbers, personal names,
  e-mail addresses, the reference drawing PDF, or local filesystem paths.
- Design decisions live in `docs/adr/`; requirements in `docs/design/02a/02b/02c-*.md`; the plan in `docs/plan/`.
