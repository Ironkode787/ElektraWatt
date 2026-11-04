# ElektraWatt orchestration rules

The orchestrating (main) session reads this file at the start of every session. Subagents never read it: their agent
definitions in `.claude/agents/` and their briefs carry what they need.
- Rules keep their numbers, and every amendment names its date.
- The owner adopted the workflow redesign on 2026-11-01 ("Implement all changes"). The removal of hand-maintained
  lists was moved forward ahead of product work.

## Roles

The main session orchestrates. It plans each cycle, briefs Claude subagents, integrates their work and delivers.
- There is no external coder (owner, 2026-10-18, 2026-10-27).
- It is also the product owner (delegated). It decides open product questions, golden changes and design taste, and
  records them as "project owner (delegated)" in the decision log.
- Real-world owner actions stay the owner's: legal matters, GitHub settings, funding, the pilot (2026-10-18, -27, -28).

## Models, agents and briefs (owner, 2026-10-27; amended 2026-11-01, 2026-11-02, 2026-11-03)

0. **Two classes, no Sonnet** (owner, 2026-11-03: "Let's scrap sonnet from the lineup again."; this withdraws the
   2026-11-02 reintroduction). Choose Opus or Haiku.
   - **Opus:** open design and new work-package parts, the file format, saves and data loss, core crates with subtle
     invariants, hard debugging, decided work with tests to run (FT batches, polish lanes, fix rounds, docs and
     design-pass lanes), rulings reads, the review and window checks.
   - **Haiku:** the facts and other lookups, the feasibility read, mechanical edits, fixtures, regeneration,
     well-specified small code.
1. **Agent types** (2026-11-01). Pass the model and the effort on every Agent call. Each definition carries its
   standing rules, so a brief never repeats them.

   | Type | Tools | Used for |
   |---|---|---|
   | `lane` | Read, Edit, Write, Grep, Glob, Bash | Every editing task: work-package parts, fixes, FT batches, docs and design-pass lanes, upkeep (Opus or Haiku) |
   | `reviewer` | Read, Grep, Glob, Bash, Write (the ledger) | Review parts (Opus `high`) |
   | `checker` | Read, Grep, Glob, Bash, Write (its folder) | Window checks |
   | `scout` | Read, Grep, Glob, Bash | Lookups and feasibility reads (Haiku), rulings reads (Opus `low`) |

   - A narrow tool set keeps a call's base context near 14k tokens instead of 55k.
   - `general-purpose` is only for a task that needs a tool outside these sets.
2. **Effort fits the task.**
   - `low`: mechanical edits and lookups.
   - `medium`: routine closed work.
   - `high`: reviews and hard work.
   - `xhigh`/`max`: only where a mistake is very costly (file format, data loss, concurrency, security, the licence
     boundary).
   - Before the first release the format may change freely (owner, 2026-10-31): no migrations or compatibility
     readers. Re-bless Golden A and the fixtures. Format work routes at `high`; data loss inside a session still at
     `xhigh`.
3. **Ambiguity goes up a class, not up in effort:** Opus at `medium` rather than Haiku at `high`. Effort buys depth,
   not judgment.
4. **Do not underestimate Haiku 5.5** (rule 0 says what it takes): lookups, mechanical edits, fixtures, regeneration,
   well-specified small code. Opus takes everything that needs judgment, at the effort rule 2 gives.
5. **Briefs** (amended 2026-11-01; the facts, 2026-11-01, cycle-43 retro): at most 800 words, written from the brief
   template in the orchestrator's private notes.
   - **Sections:**
     - the goal and the base tip;
     - the spec anchors (rulings go into the spec first; a brief never carries them);
     - the facts: gathered by a `scout` at the lane's base hash, never copied from an older inventory. Each item the
       brief names has its `path:line` there, and each changed item lists every reader, found by grep (rule 17);
     - the lane's package set and its expected files;
     - the golden expectations. A lane that re-blesses golden or snapshot files is given the exact test targets it
       runs and blesses first, by name; its one lane-check runs after them (cycle-46 retro);
     - the trailers, as the exact lines the lane's commits carry (cycle-44 retro);
     - the stops;
     - the hand-off note and code map to read;
     - any extra report fields.
   - **Every expectation cites its source.** Each expected behaviour, count or text cites a spec line, a doc line or
     the fixture query that produced it (cycle-42 retro). An expectation with no source is left to the lane, with a
     stop.
   - **A ruling about existing behaviour quotes the old code line it rests on,** at the base (cycle-43 retro).
   - **A fix that rewrites an algorithm gets an oracle over the input classes** (cycle-44 retro; amended 2026-11-03).
     Its ruling names the input classes the code accepts (for text: kerned pairs, ligatures, combining marks, CJK,
     right-to-left runs, line breaks), and the test compares the old and the new results over them, or the ruling
     says which results change and why.
   - **Banned:** restating the spec, describing the code as it is, registry lists, copies of standing rules.
   - **Precision matches the model.** The lower the class, the more exact the brief. A Haiku brief gives exact
     signatures, error texts, tests and commands, and leaves nothing open.
   - **Delegation is required or forbidden,** never "you may".
   - **Every UI brief cites `docs/design/16a-design-language.md`.**

## The cycle (owner, 2026-10-28, -29, -31; amended 2026-11-01)

6. **Lanes in waves on three slots.**
   - A cycle is about twice the work of a cycle-30 cycle, built as sequential waves.
   - At least two of the three builder slots carry work packages. Polish, FT, fix and verify work share the third.
     The rule holds at every moment while work-package lanes are queued, the end phase included: fixes and the last
     verify run one after another in the third slot. Only a major's fix may take a work-package slot, by delaying the
     next work-package dispatch.
   - A finished lane's slot takes the next queued lane.
   - **No slot stays idle** (owner, 2026-11-04: "our goal should be to always fill up all possible build spaces";
     ID-1326). When the current cycle has no lane ready for a free slot, its end phase included, the slot takes the
     next cycle's first-wave lane, on the newest picked tip. That lane is integrated after the end tip and reviewed in
     the next cycle (rule 8). The next cycle's briefs are therefore kept ready ahead of need.
   - Docs-only lanes write the next cycle's specifications during the current one.
   - The next cycle's first-wave briefs are written, feasibility-read and ruled by the time the last wave is
     dispatched.
7. **A wave ends at a merge point.**
   - Integrate the lanes that are back and run `cargo xtask settle --merge` once.
   - **The orchestrator's own merge work is tested like a lane's** (cycle-50 retro: a conflict resolution and a
     re-bless of mine were both wrong, and review caught them).
     - After a hand resolution, run the touched crates' tests before the commit.
     - Re-bless an image only by comparing the old and new images side by side, never the diff image alone. A
       conflicting re-bless goes back to the lane that owns the newer change when it can.
     - A shared snapshot re-blessed on Windows leaves its Linux override (`snapshots/linux/`) stale. Re-bless the
       Linux overrides in WSL on the merged tip, compared side by side (ID-1354).
     - After rewriting a file, `git diff --quiet` it, and restore it if it shows no change.
   - Run one merged-tip verify:
     - `cargo xtask affected`'s selection (every package the changes reach, bin crates included), with and without
       `--all-features`;
     - every policy check;
     - the checks lanes no longer run (2026-11-01), which `settle --merge` runs by their triggers: the blind
       selftests and `check-brief` when settle's trigger fires, and the MSRV check when `Cargo.lock`, a
       `rust-version`, or a permissive crate's source or manifest changes (amended 2026-11-01, ID-1072).

     A checkpoint's last verify runs the full suite.
   - Then, beside the next wave: the wave's review part, and a window check only if the wave changed what users see.
     At the verify, mark the wave UI or not: a UI wave has a lane whose report lists window items. A UI wave's window
     check is dispatched with its review part, on the verified tip's binaries (cycle-46 retro).
8. **The end phase overlaps the next cycle** (2026-10-31, ruling C).
   - After the last wave come:
     - the last review part and its fixes;
     - the last window check;
     - the visual play-test at a delivery (rule 19);
     - the cycle's one design pass (rule 22);
     - the last verify.
   - Meanwhile the free slots build the next cycle's first wave on the last wave's verified tip. Those lanes are
     integrated after the end tip and reviewed in the next cycle.
9. **Checkpoints and deliveries alternate** (2026-10-31).
   - **Checkpoint:** after the last verify, the design pass and the accepted review, push the working branch. `main`
     does not move and CI does not run.
   - **Delivery:** the gate is CI.
     - Push the working branch, dispatch `ci.yml` and `docs.yml` on it, and watch both on the full SHA.
     - Fast-forward `main` and push only when both are green.
     - Locally, the last verify runs only what CI lacks: the blind selftests, `golden_changes.py --check` and the
       Windows default-features run.
   - **While CI runs,** the next cycle's lanes may be integrated locally but not pushed or reviewed. A CI fix goes onto
     the end tip, and the unpushed commits are rebased onto it.
   - Local Windows and WSL gates are only a fallback for when CI cannot run.
10. **Continuous cycles** (2026-10-28). Never wait for the owner; only an explicit hold stops the work. When a cycle's
    first wave is dispatched, post its table (lane | model | effort | task | wave), and add rows for later lanes.

## Review and fixes (owner, 2026-10-27; amended 2026-10-31, 2026-11-01)

11. **One independent review per cycle, in parts:** one part per work package, built whole, and one per wave for the
    FT, polish and fix lanes (`<previous part's tip>..<tip>`, on the verified tip; amended 2026-11-04, ID-1281).
    - Each part is a fresh `reviewer` (Opus `high`) with the cycle's findings ledger: a file in the orchestrator's
      notes, its path in the brief. Reviewers resumed across parts compacted in two of the last five cycles
      (2026-11-01).
    - It reviews against the ADRs, the specs and the design docs: format bytes and data loss first, then determinism,
      panics, spec gaps, tests, CI and the supply chain, repository hygiene.
    - It gets the code, not the orchestrator's conclusions. It reports ranked findings with `path:line` and evidence,
      and a verdict per part.
    - Its cargo runs count as a builder: while all three slots are busy it only reads.
    - The independent reviewer is the second maintainer wherever a requirement or ADR asks for one (owner,
      2026-10-31). Its brief names each `unsafe` block in the range.
    - The cycle is accepted when every part is.
12. **Triage.**
    - Fixed in the cycle: majors, and minors that touch data, the file format or what users see.
    - Nits in files that a fix round already touches are fixed by the author in that round (cycle-42 retro).
    - Other minors and nits become FT rows for one batch lane in the next cycle's first wave, without re-review.
    - Window-check and play-test findings are triaged the same way.
    - **After the last wave's review part** (its scoped re-review included) and the last window check, only majors,
      blockers and minors that touch data or the file format get a fix round before the end tip. The rest go to the
      next cycle's first review part, FT batch or polish lane, named in the ledger (cycle-49 retro: six fix rounds and
      four re-review parts chained after the last wave).
13. **Back to the author.**
    - Findings go verbatim, with the orchestrator's rulings, to the authoring lane.
    - Resume that lane with SendMessage while its context is under about 750k tokens (ID-1322, the 1M window).
      Otherwise use a fresh `lane` fixer (Opus `medium`) with the findings and the lane's hand-off note (2026-11-01;
      amended 2026-11-04).
    - The lane's worktree is kept until its part is accepted.
    - A lane resumed after it has handed back may not be able to hand back again. Its message asks it to write its
      report to a file in the orchestrator's notes and to end with one line naming that file (cycle-46 retro).
14. **No separate re-review rounds.**
    - The next part's reviewer first checks the earlier fixes in the ledger: each finding closed, with the test that
      shows it.
    - The last part ends with a re-review scoped to the fixes. Only a major re-opens a full re-review. That
      re-review's own findings follow rule 12's last bullet; no further re-review runs in the cycle (cycle-49 retro).
    - A fix that lands after the cycle's review is accepted (a play-test blocker) opens the next cycle's first review
      part, named in its range.
15. **Shift left.** Every lane runs the hand-back checks and the self-check of its agent definition, and reports them.
    - One full lane-check per hand-back; after a failure, only the failed step (`--only <step>`). A second full run
      needs a reason in the report (cycle-49 retro; LC50 found that cycle 49's repeated runs followed real changes,
      which the measure did not see, ID-1314).
    - **Scripts are parse-checked before CI runs them** (cycle-49 retro: four of eight package runs red on scripts no
      local step had run). Before any CI dispatch, the orchestrator parses every changed `.ps1`/`.psm1` with
      PowerShell's parser (`[System.Management.Automation.Language.Parser]::ParseFile`, parse only, never run), runs
      `bash -n` on every changed `.sh`, and runs `cargo xtask check-workflows`.

## Integration and checks (owner, 2026-10-27, 2026-10-29; amended 2026-11-01)

16. **Isolation.**
    - Every editing subagent runs with `isolation: "worktree"`, resets to the tip its brief names, and commits on its
      own branch. It never checks out, resets, stashes or builds in the main checkout. It commits work in progress at
      each milestone.
    - The orchestrator:
      - merges onto the working branch;
      - runs `cargo xtask settle --merge` once on the merged tip and commits its rewrites (the main checkout sets
        `git config merge.regen.driver true` for its `merge=regen` files);
      - verifies, `settle --check` included, before calling the tip green;
      - copies the verify script for each run and never edits a running one, and stages named paths in an
        integration commit, never `git add -A` (2026-11-01, cycle-43 retro);
      - squashes a lane's work-in-progress commits that fail alone into the commit that completes them when it
        merges them (ID-1113, from cycle 46).
    - Subagent check results are claims until re-run.
    - Lanes keep their `target` for a fix round; the orchestrator deletes it when the part is accepted (2026-11-01).
17. **Facts and the feasibility read before dispatch** (amended 2026-11-02; the facts, 2026-11-01, cycle-43 retro;
    fix rulings, cycle-45 retro).
    - **The facts, for every brief:** a `scout` (Haiku) gathers them at the lane's base hash with `git show` and
      `git grep`: the `path:line` of each named item, every reader of a changed item, and where a new item goes by its
      `// map:` marker. The brief's facts section is written from that report.
    - **The feasibility read, only for briefs that touch:** the file format or a schema, a core crate, golden data, a
      check rule's meaning, or a bug that must reproduce. Every other brief goes out after the orchestrator's own
      check of its cited sources.
    - **Every fix ruling gets the read too** (cycle-45 retro), for a resumed author as for a fresh lane, sent by
      message or by brief: its premises at the lane's base (the items exist, the edges and callers it assumes, the
      behaviour it changes), beside the Opus `low` read of the ruling.
    - **The read:** a `scout` (Haiku `high`) checks with a checklist: the named items exist, a claimed bug reproduces,
      the files and the format touched, the consumers of a changed behaviour, an item's version already on `main`,
      each expectation against its cited source.
    - **Rulings:** the orchestrator writes its rulings into the spec, and an Opus `low` read checks them.
    - **Fits:** a spec or brief that fixes a fit names the fixture data it was tried against, or leaves the number to
      the lane with a stop (cycle-41 retro).
18. **Upkeep** (re-bless, regeneration, seed corpus, count bumps) runs through `cargo xtask settle` at each merge
    point. Lanes run it in write mode as `lane-check`'s settle step (amended 2026-11-02, ID-1103). Tests never pin
    totals of growing sets (CONTRIBUTING.md).
19. **Window checks** run after a wave that changed what users see, beside the next wave, and once at the end
    (amended 2026-11-01). They use `checker` agents.
    - They render offscreen only (`--visual`) and never drive the owner's desktop (2026-10-28). If a real window is
      ever unavoidable, check that the app has the focus before every key or click, stop if it does not, and close it
      with WM_CLOSE.
    - Look and hierarchy are judged at 1920 × 1080, truncation at the 1280 × 800 floor. A floor fix scales; it never
      caps (2026-10-31).
    - A visual play-test runs at each delivery, on a brief whose areas are frozen, and at the gates of UI-polish
      cycles and milestones (2026-10-29, amended 2026-10-31).

## Golden, downloads, design pass, retro

20. **Golden changes** are decided by the orchestrator (2026-10-27). Lanes re-bless an intended change and list every
    re-blessed file; the orchestrator looks at each before it lands and records it. Never open `golden/J-E-2416.ew`
    in a writing or locking mode.
21. **Downloads** (2026-10-22): fetch crates, tools and images without asking. Keep pins and digests,
    `cargo deny`/`cargo vet`, the licence boundary and the permissive MSRV. The private corpus never enters the
    repository.
22. **The design pass** runs once per cycle, in the end phase, as one `lane` (amended 2026-11-01; ID-1072).
    - It writes only what `cargo xtask records` cannot produce: decision lines in `docs/plan/decisions.md`, FT lines
      in `docs/plan/follow-ups.md`, the as-built spec text, doc 01's cycle section and the next candidates, with
      trailers.
    - It never edits `records.md` or doc 01 §3.8's status block: `settle --merge` regenerates them from those lines
      and the commit trailers.
    - Report outcomes faithfully, including what was skipped or not proven (2026-10-27).
23. **A retro after every cycle,** checkpoint or delivery (2026-10-29, -31), by the orchestrator without agents.
    - **Measure** the cycle with the metrics script in the private notes (2026-11-01):
      - runs and cargo minutes per lane, and the share of runs with no change before them;
      - build variants per lane;
      - base context and context at the first edit, and compactions;
      - hand-kept files per commit;
      - merge conflicts and settle drifts;
      - CI rounds;
      - wrong brief expectations.
    - Name at most three problems with evidence and decide at most three changes.
    - Check the previous retro's changes.
    - Report with the next cycle's table.
    - A rule change enters this file with the next cycle's first commit, never into a verified end tip.

## Lanes across parts (owner, 2026-11-01)

24. **A work package is one lane** (owner, 2026-11-03: "do not split wps from now on, please", since reviewing
    partial work costs time and tokens; ID-1281, ID-1282, ID-1322; amended 2026-11-04).
    - One brief, one lane, one review part per work package.
    - A lane is sized to finish under about 700k tokens (the 1M auto-compact window, the owner's maximum, ID-1322).
      Near 800k it commits, writes its hand-off note (at most 1,000 words, kept in the orchestrator's notes) and
      hands back; a fresh `lane` continues from that note and the code map (`cargo xtask map --crates <set>`),
      within the same brief and review part.
    - An agent is resumed for fix rounds, rulings and questions while its context is under about 750k tokens.
    - The lane definition states these limits, so a brief names a smaller one only where its package needs it.

## Testing policy (owner, 2026-10-18; amended 2026-10-22, 2026-10-31, 2026-11-01)

- **Agents run targeted tests only,** as their agent definitions say: one package set with `--all-features` in the
  edit loop, and the hand-back checks once. Never the workspace suite, never `cargo xtask ci`.
- **The merge point** runs the default-features and all-features verify of the affected set, plus the checks moved
  out of lanes (rule 7).
- **The full suite runs once per delivery,** or at a milestone delivery, as the CI run on the working branch before
  the fast-forward:
  - `cargo xtask ci`;
  - workspace nextest, with and without `--all-features`;
  - doctests and rustdoc;
  - `cargo deny` and `cargo vet`.

  CI-only checks (such as cargo-about) also run in the merged-tip verify when the lock file changes.
- **No performance tests during development** (2026-10-22: "Assume it passed"):
  - no `#[ignore]`d release-profile tests, no hostile-input timings, no `cargo xtask perf-gate`, no G-SP re-test;
  - performance is handled at release;
  - perf claims are marked "not measured (owner's rule)", and the nightly `perf.yml` stays report-only.

## Build environment and disk (owner, 2026-10-21; amended 2026-10-28, -29, -31, 2026-11-01)

- **Environment.**
  - `.claude/settings.json` sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only` and `CARGO_BUILD_JOBS=8`.
  - Local builds are incremental since 2026-11-01: −46 to −53% per rebuild, measured. CI stays non-incremental: its
    workflows set `CARGO_INCREMENTAL=0`.
  - Dependencies build without debug info. Integration tests build as one test binary per crate (`tests/it/`),
    except the binaries `xtask/src/test_layout.rs` names.
- **At most three builders at once.** A subagent's cargo run, a verify chain and a gate each count as one.
- **Jobs.** Memory is the limit (24 cores, 32 GB).
  - 8 jobs per builder while up to three run; 16 for a lone verify; 12 for a fallback local gate.
  - After a resource failure, run `cargo clean` before the re-run.
  - The Windows and WSL gates run one after the other.
- **Targets.**
  - Delete a lane's `target` when its part is accepted, and at once when its last fix round is picked: "accepted with
    fixes" counts as accepted once the fixes are in (cycle 50: a dozen 50-60 GB targets of picked lanes filled the
    disk and stopped three builders). If `rm -rf` fails, use `cmd //c "rmdir /s /q <path>"`.
  - Clean the main checkout's `target` after every last verify and gate.
  - Incremental builds make targets larger: watch them at each merge point.
- **Free space.** At least 150 GB before a gate. The orchestrator's guard is 250 GB before every dispatch and verify:
  run `python ../ew-private/notes/tools/gate.py verify` (or `df`) in the same message as each builder's dispatch, a
  resumed author's included.
  Agents delete nothing outside their own worktree; if the disk runs short, they stop and report.

## Notes outside the repository

- **The orchestrator's private notes** hold the briefs, the brief templates, the cycle log, the review ledgers, the
  hand-off notes and the tools (dispatch, CI watch, metrics).
- **The handoff file** at the repository root (untracked, never committed) holds the current state for a new
  session.
- Neither may be copied into the repository.
