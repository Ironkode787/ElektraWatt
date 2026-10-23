# WP-S6.13: the blind-drafter harness, personas and prompts (closed specification)

**Status:** specified in cycle 25 (documents only) for three parts: part 1 (the harness `cargo xtask blind`, the
session keeper, the three product additions it needs and the replay in CI, D3), part 2 (the prompts, the personas, the
G-PP task sheet with its prelude and oracle, and a dry run on folio 101 after WP-S2.04 part 1, D3), part 3 (the full
G-PP run with the checker, the committed recording and the triage, D4); part 1 after WP-S2.04 part 1's delivery, part 2
after part 1, part 3 after WP-S2.04 part 4b and WP-S2.05 part 1; Opus review of part 1, Opus reading of every run ·
**Date:** 2026-10-24 · **Work package:**
[WP-S6.13](../02-work-packages.md#wp-s613-blind-drafter-gate-agent-harness-personas-and-prompt) · **Decision log:**
ID-610 to ID-619, the ten entries of §12.3 in order (follow-ups FT-408 to FT-416, the nine of §12.4 in order) ·
**Part of:** ElektraWatt plan, specifications

**Purpose.** This is the closed specification of the blind drafter's harness: the tool through which an agent that
never sees pixels draws and edits in EW alone, as a user would, through the shell's headless mode
(`elektrawatt --headless --script -`), with `ui.describe` and a page description as its only senses and the transcript as
its evidence (decision log ID-341, ID-443, ID-474; FT-314). It turns the owner's method of doc 18 §3.17 into a harness,
a protocol, versioned prompts and personas, a scored gate task (golden folios 101 to 103 from doc 20's description, the
pilot preview's task) and a recording that CI replays. It is written against **the shell as built at the cycle-24 tip**
(§2.1) and **the vocabulary WP-S2.04 and WP-S2.05 specify** (§2.2), not against code that later parts write. Where this
document and a design document differ, this document is the narrower and wins for the lanes; doc 18 §3.17 stays the
target, and the documents named in §12.6 are updated at the design pass after each part. A choice a lane meets that this
document does not settle is escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs](#2-inputs)
3. [The decisions of this specification](#3-the-decisions-of-this-specification)
4. [Roles and public items](#4-roles-and-public-items)
5. [The design](#5-the-design)
6. [Determinism, the headless mode and exact counts](#6-determinism-the-headless-mode-and-exact-counts)
7. [Acceptance checks](#7-acceptance-checks)
8. [The gate evidence for G-PP](#8-the-gate-evidence-for-g-pp)
9. [Files owned](#9-files-owned)
10. [Out of scope](#10-out-of-scope)
11. [Review checklist for the Opus reviewer](#11-review-checklist-for-the-opus-reviewer)
12. [Size, order, routing and the documents to update](#12-size-order-routing-and-the-documents-to-update)
13. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package the orchestrator can start a blind-drafter run with one command, hand the composed
prompt to an agent of any vendor that has a shell, and get back, without any human step in between, a run folder that
holds the agent's live notes, its fixed hand-back, every UI step it sent with its transcript line, the saved project,
the exported PDF, the check report, the parts list and a score against the oracle of the task. The agent's only senses
are `ui.describe` (through the harness's `look`) and the new `page.describe` (§5.5.3); it never receives an image, never
reaches the model except through registered UI actions, and never reads the design documents, the code or the tests
(ID-341). A second agent, the blind checker, audits the saved result through the same senses and the CLI reports. A
committed recording of a run is replayed in CI by an ordinary test, so a product change that breaks what an agent could
do shows up as a failing replay rather than as a surprise at the next gate. The proofs are the acceptance checks BD1 to
BD18 (§7) and, for G-PP, the committed recording of the gate run with its score, hand-back and audit (§8).

**The gate this serves.** G-PP's one criterion G-PP-01 is the pilot's: the pilot draws golden folios 101 to 103 and the
parts list 901 in a new project interactively, without developer help, and exports the PDF ([gates](../03-gates.md)
§5.4). The blind drafter's gate run is the **rehearsal of that task by an agent that never sees pixels**; it is recorded
evidence read with the preview issue list and **never a criterion and never blocking** (ID-341; gates §5.4 "Not
criteria, but recorded"). Its score says whether the product, through its text interface alone, lets the task be done,
and names each place where it does not.

**Non-goals** (each is named with its package in §10):
- **No new GUI, no pixels, no screenshots, no OCR.** The harness refuses the snapshot actions from the agent (§5.3), and
  the page description is text from the model's tables, not from a raster.
- **No model call in the harness and no model in CI.** The harness is model-agnostic; the orchestrator runs the agent
  outside CI; CI replays a recording (D13, D15). No API key, token or account appears anywhere in the repository.
- **No fix of what a run finds.** Findings become follow-up tasks of their owning work packages through the next design
  pass (ID-341).
- **No moderated usability round** (WP-S2.12) and no proof of NFR-UX-01: a model's judgement stands in for a user's only
  until those rounds (ID-341's trade-off).
- **No change to the golden fixture, to any exported byte of Golden A, to AC-15's 609-command log, to the project format
  or to the acceptance result** (94 entries, 88 pass); no new memoised family; no new package in `Cargo.lock`; no new
  workspace edge (the harness drives the built binaries as processes, D2).

## 2. Inputs

| Input | What the lanes take from it |
|---|---|
| [Decision log](../06-decision-log.md) ID-341, ID-342 | The method, the run protocol (live note, patience and flow, quit rules, broken after two tries, caps), the fixed hand-back, the personas, the harness commands `start`, `do`, `note`, `stop`, `check`, the paths `tools/blind/BLIND_DRAFTER.md` and `tools/blind/personas/`, the rulings (never blocking, never in the headless acceptance run, task sheets and placeholder data only, drafters on Sonnet with an Opus lead reading) |
| ID-443, ID-474, ID-475, ID-493, ID-511, ID-537, ID-540, ID-546, ID-557, ID-559, ID-586 to ID-589 | The headless mode, its script and transcript, the exit codes, the configuration isolation, the vocabulary rule (`VOCABULARY`, `FREE_TEXT_MARKERS`, a change is a reviewed transcript change), the `HumanOnly` refusal, the UUID-free navigator row, the no-timing rule for tests |
| [Quality](../../design/18-quality-security-release.md) §3.10, §3.11, §3.14, §3.17, §12 | The acceptance runner's statuses, the UI test jobs, flaky-test rules, the blind drafter's section, privacy |
| [WP-S2.03's specification](WP-S2.03-viewer-and-tool-framework.md) §6.10, §7.9, §8 | The transcript members and describe lines of the viewer and the tool framework |
| [WP-S2.04's specification](WP-S2.04-placement-wiring-drawing-tools.md) §4.3, §7.6, §7.7, §8.7, §9.6, §10.7, §11, §12, §13.6, §17.5 | `Session::page_tables`; the actions, transcript members and describe words of parts 1 to 5; the folio scripts, the exact counts per folio, the oracle rules, the G-PP rehearsal and the folio-103 demo variant |
| [WP-S2.05's specification](WP-S2.05-property-grid-find-replace-clipboard.md) §6.10 to §6.12, §7.6, §8.8, §9 | The property grid's, find's and the clipboard's actions, members and words; `select.set` and `resolve_reference`; what the pilot edits on 101 to 103 |
| Code as built at the cycle-24 tip (6afe6f2) | `crates/ew-viewmodel/src/{actions.rs,describe.rs,script.rs,shell.rs}`, `crates/ew-ui/src/headless.rs`, `bin/elektrawatt/src/main.rs`, `bin/ew/src/cli.rs` (`ew check`, `ew export bom`, `ew export pdf`, `ew sql`), `crates/ew-api/src/cmdlog.rs`, `xtask/src/main.rs` and `xtask/Cargo.toml`, `ci/licence-map.toml`, `REUSE.toml` |
| [Golden project](../../design/20-golden-project.md) §4.3, §5, §6.1, §7.3, §8.2, §8.3, §9, §14.1, §15, §18.1, AC-02, AC-05, AC-06, AC-07, AC-14, AC-21 | The description of folios 101 to 103 (the task sheet's only source, D4), the oracle counts and rows, the parts list, the checks; its J-E-2416 placeholders are the only identifiers the task sheet may carry |
| `golden/J-E-2416.ew`, `golden/J-E-2416/expected/parts_list_14_1.csv`, `golden/J-E-2416/tables/` | The oracle's source, read only through temporary copies (D16) |
| [Gates](../03-gates.md) §5.4, §5.5 | G-PP-01, the place of the blind drafter at G-PP and G-M2 |
| Requirements | NFR-UX-04 (the WP's one row); with NFR-UX-05, SC-9, PR-13, FR-UI-046, FR-EXT-030, NFR-A11Y-02, NFR-REL-07, NFR-PRIV-09 where they touch it; G-PP-01 |

The WBS lists WP-S2.04 and WP-S6.01 as dependencies. Part 1 needs WP-S2.04 part 1 (`Session::page_tables`); part 3
needs WP-S2.04 part 4b and WP-S2.05 part 1 at least, and scores what is still missing as gaps (§5.9).

### 2.1 The base contract as built (cycle-24 tip)

The lanes rely on exactly these items. If a delivered item differs, the lane stops and reports it.

| Item | As built | Consequence for the harness |
|---|---|---|
| Invocation | `elektrawatt --headless --script <file\|-> [--record <file.ewcmd>] [--config-dir <dir>]`; with `-` the script is read line by line from stdin and every transcript line is flushed (ID-474) | A live session is one child process fed through stdin (§5.4) |
| Script | `ew.uiscript/1`: a header `{"$schema":"ew.uiscript/1","label":…}`, then one `{"action":…,"params":{…}}` per line; an unknown member or action ID is a usage error | The harness builds every line itself with a JSON serializer and checks action IDs against the reference before sending (§5.3) |
| Transcript | `ew.uitranscript/1`: a header with the version, then per step `step`, `action`, `outcome` (`done`, `refused`, `failed`), `reason`, `message`, `state` (15 top-level members: `title`, `project`, `access`, `dirty`, `page`, `page_index`, `page_count`, `can_undo`, `can_redo`, `banner`, `status`, `canvas`, `tabs`, `focus`, `navigator`), and `describe` for `ui.describe` | The evidence; the `do` output is a compact rendering of it (§5.2.3) |
| End of a run | A refused or failed step **ends the run** (exit 1); a malformed line or unknown action in stream mode ends it with exit 2; unsaved changes at the end give exit 1 (ID-475) | A live agent's first mistake would end its session: `--keep-going` (D7, §5.5.1) |
| Registry | **107 actions** in `ACTIONS` (`ActionMeta`: `id`, `kind` View or Command, `label_key`, `category`, `shortcut`, `enabled`, `menu`, `headless_only`, `needs_parameters`, `hidden`, `context`); **no parameter metadata** | The agent cannot learn an action's parameters from any user-facing source: parameter signatures (D9, §5.5.2) |
| Vocabulary | **111 words** in `VOCABULARY` (53 of WP-S2.01, 11 canvas words of ID-511, 3 renderer words of ID-540, +27/−1 of WP-S2.03 part 1, 18 of part 2) and **8 markers** in `FREE_TEXT_MARKERS` (`title:`, `banner:`, `error:`, `help:`, `view:`, `bar:`, `disabled:`, `menu`), checked by the tests of `ew-viewmodel` | The words are the harness help's vocabulary; a pinned copy keeps the help equal to the code (D9) |
| Back doors and pixels | `command.run` (any model command with JSON parameters, headless only), `view.snapshot`, `canvas.snapshot`, `canvas.preview_snapshot` (PNG files) | Refused from the agent (D4, §5.3); `command.run` only in the harness's prelude (D11) |
| Identities | `canvas hover` and `canvas selection` print UUIDs only; a new project mints random UUIDv7 (the seeded source is behind the `testkit` feature of `ew-ids`) | A recording of a new-project run differs in UUIDs between runs: UUID normalisation and UUID-free steps (D14); entity labels in those lines are a gap (FT-412) |
| Page content | `ui.describe` lists no page content: no device, no connection, no connection point except the one under the grid cursor | Without a content listing the agent cannot see what it drew: `page.describe` (D8, §5.5.3) |
| Command log | `--record` writes the committed `command.run` and tool-host steps as `ew.cmdlog/1`; references of later lines to records minted earlier are raw UUIDs | `ew apply` cannot replay a new project's recorded log on a fresh project (the minted IDs differ): the UI script is the replay artefact (D14); FT-416 |
| xtask | `xtask` has no edge to any product crate (`ci/dep-rules.toml`: `"xtask" = []`); it is MIT OR Apache-2.0 | The harness drives the built `elektrawatt` and `ew` binaries as processes; no new edge (D2) |
| Licence map | No row for `tools/**` | Precondition P1 (§12.2, FT-415) |

### 2.2 What the M-PP chain adds (specified, not built)

The harness speaks the vocabulary of these parts as each lands; each part's own test pins its words, and the reference
files of D9 follow by a bless (§5.5.4).

| Package and part | Actions the drafter uses | New words (spec) |
|---|---|---|
| WP-S2.04 part 1 (this cycle, lane A) | `tool.place`, `tool.place_part`, `place.pick`, `picker.*`, `strip.new`, the transforms, `tool.move`, `tool.finish`, `text.input`, `edit.delete_selection`, `rep.exchange` | 28 (§7.7) |
| WP-S2.04 part 2 | `tool.wire`, `tool.next_proposal`, `tool.connect_on_move`, `tool.route_edit`, `junction.*`, `project.crossings` | 22 (§8.7) |
| WP-S2.04 part 3 | `tool.ip`, `tool.ip_direction`, `tool.star_source`, `ip.*`, `tool.black_box`, `box.*` | 10 (§9.6) |
| WP-S2.04 part 4b | `tool.text`, `tool.graphic`, `text.*`, `tool.marker`, `marker.fields`, `tool.structure_box`, `structure_box.fields`, `project.marker_mode` | 13 (§10.7) |
| WP-S2.04 part 5 | `image`, `image.pick` (not needed on 101 to 103) | 1 (§11) |
| WP-S2.05 part 1 | the property grid's actions, `select.set` | 53 (§6.11) |
| WP-S2.05 part 2 | `find.*` | 13 (§7.6) |
| WP-S2.05 part 3 | `edit.copy`, `edit.cut`, `edit.paste_clip`, `edit.duplicate`, `edit.copy_tags`, `clipboard.text` | 6 (§8.8) |

With this package's 7 words (§5.5.3) the vocabulary reaches about 265 words at the end of the chain (111 + 74 + 72 + 7,
less the words a later part finds already added; each part's test states its exact count).

### 2.3 What the vocabulary still lacks for the pilot's task

Checked against G-PP-01's task (folios 101 to 103 and 901 in a new project) and against WP-S2.04 §13.6's list of steps
that its folio scripts still issue through `command.run`. A gap that is not closed when part 3 runs is scored `gap`, not
`miss` (§5.9), and named in the score with its follow-up.

| Gap (by action or describe word) | Why the drafter needs it | Closed by |
|---|---|---|
| A refused step ends the headless run | The agent's first refusal would end its session | This package, part 1: `--keep-going` (§5.5.1) |
| No parameter signatures in the registry | The agent cannot learn `canvas.press`'s `x_mm`, `y_mm` or `place.pick`'s `symbol` from user-facing material | This package, part 1: `ActionMeta.params` and `tools/blind/reference/actions.txt` (§5.5.2); closes FT-314 |
| No page content in any describe text | The agent cannot see what it drew | This package, part 1: `page.describe` (§5.5.3) |
| No `page.new` (Page ▸ New page, doc 16 §3.2's menu table; doc 06 §9.2 step 1) | Folios 101 to 103 are created in the new project | FT-408 (WP-S2.06), before G-PP |
| No action creates a structure node or an external reference (`+J-E-206`, `JE-N1000-03` of folio 101's external points) | Folio 101's external arrows print them (doc 20 §3.4 item 9) | FT-409 (WP-S2.06), before G-PP |
| No action brings part copies into a new project (WP-S1.19a's CSV import has no UI action; WP-S2.07's part selection is M2) | Every placed device on 101 to 103 carries a golden part | FT-410 (WP-S1.19a, WP-S2.09), before G-PP; until then the parts are prelude setup (D11) |
| No action creates a strip bridge (BR1 to BR6 of doc 20 §8.3; the strip editor is WP-S2.07, M2) | Folio 103's terminals are bridged; the bridges carry the potentials 24VDC-1 to 24VDC-4 and 0V across the strip | FT-411, before G-PP; **also a G-PP-01 gap for the pilot** (§12.5 Q3) |
| UUID-only parameters (`place.pick` `function`, `ip.external`, the junction editor's target; `canvas hover` and `canvas selection` print UUIDs only) | A recorded UI script must not depend on minted IDs (D14), and the agent cannot tell what it hovers or selects | FT-412 (WP-S2.04 parts 2 and 3, WP-S2.05 part 1's resolver), before G-PP |
| Title-block master data and IP names | Edited by the pilot on 101 | FT-400 and FT-399 (existing, before G-PP) |

Not gaps (checked): page titles (`fields.set page title` through the grid, WP-S2.05 §6.12), frozen tags (S2.05 §6.12),
terminals one by one (`new_terminal`, ID-571), the PDF (`file.export_pdf`), save (`file.save`), the parts list 901 (the
wizard's report blocks, FT-341; part 2's dry run confirms that a new wizard project generates 901), the check scheme
(`HumanOnly`, never by the drafter; `ew check` runs `pre-delivery` by default, ID-602).

## 3. The decisions of this specification

Each is a lead ruling that the owner may reverse; D3 and P1 carry owner questions (§12.5). The design pass allocates the
decision-log entries (§12.3).

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Three parts.** Part 1 the harness, the keeper, the three product additions and the replay in CI (0.9 pw, D3); part 2 the prompts, personas, G-PP task sheet, prelude and oracle with a dry run on folio 101 (0.4 pw, D3); part 3 the full G-PP run with the checker, the committed recording and the triage (0.3 pw, D4). Order 1 → 2 → 3. The parts sum to the PERT of 1.6 pw with **no reserve**; a de-scope order applies (§12.1) | Part 1 is the only code; part 2 is writing plus one agent run; part 3 is a run and its reading, which ID-341 puts on an Opus lead |
| D2 | **Home and language: Rust in `xtask/src/blind/`, run as `cargo xtask blind`; the prompts, personas, tasks, references and recordings under `tools/blind/`.** The harness drives the built `elektrawatt` and `ew` binaries as child processes and adds no workspace edge, no crate and no runtime dependency to the application. Python under `docs/plan/tools/` is rejected | The owner fixed the command and the paths (ID-341). The repository's precedent is Python for plan tooling and Rust for whatever runs the product; the harness runs the product on three OSes, holds a child process and a loopback socket, and its replay must also be an ordinary nextest test. Std alone covers processes and loopback TCP, so `xtask`'s dependency list is unchanged |
| D3 | **Two run kinds.** `free`: the agent chooses its own task, as ID-341 rules (the default after a large UI change). `gate`: a fixed task sheet with an oracle; the first is `gpp-101-103`, the G-PP rehearsal. A gate run is scored; a free run is read only | G-PP-01 is a fixed task, and oracle counts mean something only on it; the owner's free run keeps its place. **Owner question Q1** (§12.5): confirm the gate kind beside the free run |
| D4 | **The agent's senses and limits.** It sees only `ui.describe` (through `look`) and `page.describe`; it acts only through registered UI actions; the harness refuses `command.run`, the three snapshot actions, any path parameter that is not a bare relative name, and any parameter that holds a UUID (§5.3). It reads only the composed prompt, `cargo xtask blind help`, `elektrawatt --help`, `ew help` and the user guide. **Doc 20 reaches it only through the task sheet**, a hand-written extract that carries doc 20's J-E-2416 placeholders and nothing else, so ID-341's "never the design documents" holds | The run measures what the text interface allows, not what a back door allows; the task sheet plays the part of the drawing the pilot gets on paper |
| D5 | **The protocol.** A turn is one `do` call with 1 to 20 steps, followed by exactly one note; the harness forwards the steps in order and stops at the first step that is not done; `look` calls are free within a cap; caps are counts (turns, steps, looks), never wall time; a step that fails twice in a row is marked broken and refused thereafter; quit rules on the patience and flow scores end the run (§5.2) | ID-341's live note, scores, broken rule and caps, made mechanical so that a run is reproducible and the rules do not depend on the agent's honesty |
| D6 | **A loopback session keeper.** `start` spawns `cargo xtask blind serve`, which holds `elektrawatt --headless --script - --keep-going` as a child and serves `do` and `look` over TCP on 127.0.0.1 with a per-run token; a lost keeper is recovered by `resume`, which replays the run's accepted steps and checks that the outcomes repeat (§5.4) | An agent calls the harness once per turn from its own shell, so the session must outlive each call; std's loopback TCP works the same on the three OSes, where named pipes and FIFOs do not; replay-on-resume costs time only after a loss and doubles as a determinism check |
| D7 | **`--keep-going` in the product.** With it, a refused or failed step is written to the transcript as now and the run goes on; usage errors are unchanged; the exit code is 1 when any step was not done or the script ends with unsaved changes, else 0 (§5.5.1) | The only change to the run semantics the agent needs; without it every refusal would end the session. A flag keeps every existing script and test byte-identical |
| D8 | **`page.describe`: FR-UI-046's text list pulled forward as a headless-only v0.** One action, optional `folio`, whose transcript line carries the page's content as text in reading order: representations with tag, cell, symbol, variant, position and connection points (label, position, wire count, potential), drawn connections by their end labels, junctions by reading-order number with style, wiring and sequence, and (from WP-S2.04 part 4) page items and structure boxes. No UUID in the text. 7 new words (§5.5.3) | The drafter cannot work without seeing what it drew, and the page tables of WP-S2.04 part 1 already hold every row; FR-UI-046 (1.0) gets its data path early, and the GUI command stays 1.0 |
| D9 | **Parameter signatures in the registry and pinned reference files.** `ActionMeta` gains `params: &'static str`, a signature in a fixed grammar (§5.5.2); an `ew-viewmodel` test writes and compares `tools/blind/reference/actions.txt` (the registry with labels, menus, shortcuts, contexts, flags and signatures) and `tools/blind/reference/vocabulary.txt` (`VOCABULARY` and `FREE_TEXT_MARKERS`) under `EW_BLESS=1` | FT-314's "the harness help documents the vocabulary" becomes a failing test whenever a lane adds a word or an action without updating the help; one source of truth, the registry, and no help text written by hand that drifts |
| D10 | **Prompts and personas as versioned files.** `tools/blind/BLIND_DRAFTER.md` (the drafter's prompt), `tools/blind/CHECKER.md` (the checker's), `tools/blind/HELP.md` (the protocol the agent reads), `tools/blind/personas/qet-veteran.md` and `switcher.md` (the apprentice later, as ID-341 says); each starts with a front-matter block holding `prompt` and `version`; `compose` concatenates them deterministically and the run manifest records each file's SHA-256 (§5.6) | A run is reproducible from the repository state; a prompt change is a reviewed diff with a version bump; personas are fictional roles, never people |
| D11 | **Setup through a listed prelude only.** A gate task may hold a prelude (`prelude.uiscript`), run by the harness before the first turn and recorded apart from the agent's steps; only the prelude may use `command.run`; every prelude line is classified `setup` (what a company basic project holds: the first-run flow, the wizard with its defaults, part and symbol copies) or `drawing` (anything a user draws on 101 to 103, pages included). A `drawing` line is allowed only while its gap is open and is scored as that gap (§5.7, §5.9) | The dry run on folio 101 must work before the chain closes; the classification keeps the score honest: G-PP's run counts only what the agent drew through UI actions |
| D12 | **Scoring: hard checks, soft checks, statuses `ok`, `miss`, `gap`.** Hard: the exact counts per folio of WP-S2.04 §13.6 (folio 103 as the demo variant, ID-571), the parts list equal to doc 20 §14.1 restricted to the devices on 101 to 103 (AC-07's rule on the new project), the PDF byte-identical on 1 and 4 threads (AC-14's rule), `ew check` run (exit 0 or 1), no prelude `drawing` line. Soft: anchor cells (AC-02's rule), endpoint labels, junction sequences, cross-reference texts (AC-05's rule), tags (AC-06's rule), page titles, marker texts, the finding set (AC-21's rule), each restricted to 101 to 103. Outcome `pass`, `pass-with-gaps`, `fail` or `incomplete` (§5.9). **Never a gate criterion** | The hard checks are G-PP-01's own content in machine form; a check blocked by a listed gap is not the agent's miss; the gate stays the pilot's (gates §5.4, ID-341) |
| D13 | **The model runs outside the repository's automation.** The harness never calls a model, has no network code beyond loopback, reads no key and writes none; the orchestrator composes the prompt, starts the agent (the drafter and the checker on Sonnet, ID-341), stops the run and has an Opus lead read it (§5.11). CI runs only the replay | Model-agnostic and key-free by construction; the cost of a run is a deliberate orchestrator act, never a CI side effect |
| D14 | **The UI script is the replay artefact; transcripts are compared after UUID normalisation.** A run's `steps.uiscript` (prelude, agent steps, epilogue) replayed through the built binary reproduces the run's outcomes and its `page.describe` epilogue; every UUID in a transcript is replaced by `«n»` in order of first appearance before any comparison; the agent's steps carry no UUID (D4). The `.ewcmd` of `--record` is kept as evidence only. **ID-341's "replayable `.ewcmd`" is amended** | A new project mints random IDs in the shipped binary, and a recorded command log references them, so `ew apply` cannot replay it on a fresh project (FT-416 keeps that open for the command-log format); the UI script replays the stronger path, the UI itself |
| D15 | **CI replays committed recordings by an ordinary test.** `bin/elektrawatt/tests/blind_replay.rs` replays every recording whose manifest says `ci = "pr"` (part 1's self-test recording, at most 100 steps) in the existing test jobs; the G-PP recording has `ci = "nightly"` and is replayed by an `#[ignore]`d test that the deep nightly runs (FT-414). A recording whose replay diverges after an intended product change is marked `stale` in its manifest by the lane, with the reason, as a reviewed one-line change, until the next live run replaces it | The PR jobs stay inside G-M0-04's wall-time budget; a lane is never blocked by a recording it cannot re-make (a live run is an orchestrator act), and the drift stays visible |
| D16 | **The golden fixture is read only through temporary copies.** Every harness step that opens Golden A (`bless-oracle`) copies `golden/J-E-2416.ew` into the run's temporary directory first and opens the copy; a check compares the SHA-256 of every file under `golden/` before and after | Opening a project takes a lock and may write a journal; the committed fixture must never see either (doc 20 §3.1, the acceptance runner's rule) |

## 4. Roles and public items

### 4.1 Roles

| Role | What it is | Model and who runs it |
|---|---|---|
| **Harness** | `cargo xtask blind`: run folders, the protocol, the filter, the keeper, compose, stop, score, bless, replay | Code in `xtask` (part 1) |
| **Blind drafter** | The agent that draws, persona-parametrised (`qet-veteran` for the gate run; `switcher` for free runs) | Sonnet (ID-341), started by the orchestrator |
| **Blind checker** | A second agent that audits a stopped gate run through `look`, `page.describe`, view-only actions and the CLI reports, without reading the drafter's notes | Sonnet, started by the orchestrator after `stop` |
| **Reader** | The lead who reads the notes, the hand-back, the audit and the score and turns findings into follow-up tasks | Opus (ID-341); not a persona |
| **Orchestrator** | Builds the binaries, starts and stops runs, launches the agents, blesses recordings, commits | The main session (§5.11) |

No third persona: the reader is ID-341's Opus lead, and the apprentice persona comes later (ID-341); a prompt file for it
is written when its first run is scheduled.

### 4.2 The public items

| Item | Contract |
|---|---|
| `cargo xtask blind <subcommand>` | `help`, `start`, `do`, `look`, `note`, `handback`, `stop`, `report`, `resume`, `compose`, `bless-oracle`, `bless`, `replay`, `selftest`, and the internal `serve` (§5) |
| `elektrawatt --headless … --keep-going` | §5.5.1 |
| `ActionMeta.params: &'static str` | §5.5.2; `act()` sets `""`; the const wrapper `with_params(sig, a)` sets it |
| The action `page.describe` | Headless-only, `View`, context global, params `folio?:text`; the transcript line carries `describe` with the page text (§5.5.3) |
| `tools/blind/reference/actions.txt`, `vocabulary.txt` | Generated by `crates/ew-viewmodel/tests/blind_reference.rs` under `EW_BLESS=1` (§5.5.4) |
| Formats `ew.blindrun/1` (`run.json`), `ew.blindturns/1` (`turns.jsonl`), `ew.blindscore/1` (`score.json`) | §5.1, §5.9; versioned, additive within `/1` |
| The task folder layout `tools/blind/tasks/<task>/` | §5.7 |

No other item is added to any product crate. `ew-api`, `ew-commands`, the format, the command catalogue (43 commands) and
the golden files are unchanged.

## 5. The design

### 5.1 Runs, kinds and the run folder

A run is a folder, by default `target/blind/<run-id>/` (`--out` may name another directory; the harness refuses a folder
inside `golden/`, inside `tools/` or inside any tracked directory other than `target/`). The run ID is given by the
orchestrator (`gpp-r1`, `free-2026-11-a`); the harness writes no time stamp, host name, user name or absolute path into
any file of the folder.

| File | Content |
|---|---|
| `run.json` | `ew.blindrun/1`: `run`, `kind` (`free`, `gate`, `check`), `task`, `persona`, `of` (for a check run: the drafter run's ID), `caps`, `files` (path relative to the repository root and SHA-256 of every composed prompt file, the task files and the reference files), `app_version` (`elektrawatt --version`), `harness_version`, `counts` (turns, steps, looks, harness refusals, refused, failed, broken), `end` (`handback`, `cap`, `quit`, `orchestrator`, `diverged`, `lost`), `ci` (`none`, `pr`, `nightly`; set by `bless`), `stale` (`null` or a reason) |
| `prompt.md` | The composed prompt (§5.6) |
| `prelude.uiscript` | The task's prelude as run (`ew.uiscript/1`) |
| `steps.uiscript` | Every step sent to the product after the prelude, in order: the agent's steps, the harness's `ui.describe` and `page.describe` steps for `look`, and the epilogue (§5.8) |
| `turns.jsonl` | `ew.blindturns/1`: one line per `do`, `look`, `note` or harness refusal: `n`, `kind`, the step range in `steps.uiscript`, the folio shown, the refusal text |
| `transcript.jsonl` | The product's transcript of prelude and steps, as produced |
| `drafter.md` | The notes, one section per turn (`## Turn n`), with the parsed scores |
| `handback.md` | The fixed hand-back (§5.6) |
| `record.ewcmd` | `--record`'s command log (evidence, D14) |
| `work/` | The keeper's working directory: the project (`work/project/`), exports |
| `config/` | The `--config-dir` of the run (configuration isolation, ID-474) |
| `out/` | After `stop`: `project.pdf`, `project-t4.pdf`, `bom.csv`, `check.txt`, `check.json`, `score.json`, `report.md` |
| `checker.md` | The checker's audit (a check run writes it into the drafter run's folder) |

### 5.2 The protocol

#### 5.2.1 Commands the agent uses

| Command | Effect |
|---|---|
| `cargo xtask blind help` | Prints `tools/blind/HELP.md`, then `reference/vocabulary.txt` and `reference/actions.txt` |
| `cargo xtask blind do <run> <action> [key=value …]` | One step. A value is a number, `true`, `false`, `null`, or JSON when it starts with `[`, `{` or `"`; anything else is a string |
| `cargo xtask blind do <run> -` | A batch: 1 to 20 steps as JSON Lines (`{"action":…,"params":{…}}`) on stdin |
| `cargo xtask blind look <run> [section …] [--diff] [--menus]` | Sends `ui.describe` and prints its lines, grouped by section (a section is a top-level line's first token with its indented lines: `title:`, `banner:`, `page`, `navigator`, `canvas`, `properties`, `card`, `toast`, `find`, `clipboard`, `status`, `palette`, `dialog`, `wizard`, `first`, `start`, `workspace`, `help:`, `notices:`); menu lines only with `--menus`; with `--diff` only the lines added or removed since the previous `look` of the same sections, prefixed `+` or `-` (a longest-common-subsequence diff) |
| `cargo xtask blind look <run> page [<folio>]` | Sends `page.describe` (§5.5.3) and prints its text |
| `cargo xtask blind note <run> "<text>"` | The note of the last turn: it must start with `patience <0-10> flow <0-10>`, followed by free text of at most 1,200 characters |
| `cargo xtask blind handback <run> -` | The hand-back on stdin, with the seven headings of §5.6; accepted once, at any time; it ends the run (`end: handback`) |

#### 5.2.2 A turn

1. A turn is **one `do` call**. Before it, the previous turn must have its note; otherwise `do` prints
   `harness: note required for turn <n>` and sends nothing.
2. The harness checks every step of the batch (§5.3). A step it refuses is not sent; the turn ends there with a harness
   refusal, which is recorded in `turns.jsonl` and counts toward the cap of harness refusals.
3. The harness sends the steps in order and **stops at the first step whose outcome is not `done`**; the remaining steps
   are reported `skipped` and are not sent (so the agent reacts to the first surprise). The product runs with
   `--keep-going`, so the session stays alive.
4. **Broken.** When a step is not done and the previous turn's first not-done step had the same action and the same
   parameters with the same outcome and reason, the harness appends `broken: <action> <params> <outcome> <reason>` to
   `drafter.md` and refuses that exact step (action and parameters) for the rest of the run with
   `harness: marked broken, move on`. The same action with other parameters stays allowed.
5. `look` calls do not open a turn and need no note; at most 3 looks between two `do` calls (`harness: look cap per
   turn`).
6. **Quit rules.** The scores start at 10. Three consecutive notes with patience ≤ 2, or five consecutive notes with flow
   ≤ 2, end the run (`end: quit`); the agent then files its hand-back, which `handback` still accepts.
7. **Caps.** Per kind, counted, never timed:

| Kind | Turns | Steps (agent) | Looks | Harness refusals |
|---|---|---|---|---|
| `gate` (`gpp-101-103`) | 900 | 3,000 | 1,500 | 100 |
| `free` | 300 | 800 | 400 | 50 |
| `check` | 200 (looks and view steps) | 600 | 600 | 50 |

   A reached cap ends the run (`end: cap`). Per-folio budgets of the gate task (101: 250 turns, 102: 250, 103: 400) are
   reported in the score from the `page` member of the transcript, never enforced.
8. A run also ends by `cargo xtask blind stop <run> --reason orchestrator`.

#### 5.2.3 What `do` prints

Per step one line `<n> <action> <outcome>[ <reason>][ "<message>"]`, then, for each top-level member of `state` and each
member of `state.canvas` that differs from the previous step's, one line `  <member>: <compact JSON>` with UUIDs
normalised (§6 item 2); `skipped` steps print `<n> <action> skipped`. A harness refusal prints
`harness: <text>`. The full lines stay in `transcript.jsonl`.

### 5.3 The harness filter

Every step the agent sends is checked before it reaches the product. A refused step is never sent.

| Rule | Refusal text |
|---|---|
| The action ID is in `reference/actions.txt` | `harness: unknown action <id>` |
| The action is not `command.run`, `view.snapshot`, `canvas.snapshot` or `canvas.preview_snapshot` | `harness: <id> is not available to the drafter` |
| A `gate` or `free` run: any action. A `check` run: only actions of kind `View` and `file.open`, `app.quit` | `harness: the checker does not change the project` |
| Every parameter named `path`, `dir` or `file` is a bare relative name (no `/`, `\`, `:`, no `..`, not empty) | `harness: paths are names inside the run's work folder` |
| No parameter value (at any depth) contains a UUID (8-4-4-4-12 hexadecimal) | `harness: use a label or a position, not an ID` |
| The parameters are a JSON object of at most 64 KiB | `harness: parameters too large` |
| The step is not marked broken (§5.2.2 item 4) | `harness: marked broken, move on` |

The product runs with its working directory at `work/`, so bare names resolve inside the run folder; `--config-dir`
points at `config/`.

### 5.4 The keeper and `resume`

- `cargo xtask blind start --kind <k> --task <t> [--persona <p>] [--of <run>] [--folios <list>] --run <id>
  [--out <dir>] [--bin <dir>]` creates the folder, composes `prompt.md`, and spawns `cargo xtask blind serve <run>`
  detached. `--bin` names the directory of the built `elektrawatt` and `ew` (default `target/debug`); the harness
  records `elektrawatt --version`. `--folios` limits a gate task to some of its folios (the task sheet's other folios are
  left out of the composed prompt, and their checks are scored `gap` with the reason "not in this run").
- `serve` spawns `elektrawatt --headless --script - --keep-going --record <run>/record.ewcmd --config-dir <run>/config`
  with the working directory `<run>/work`, binds a TCP listener on `127.0.0.1:0`, writes `<run>/session.json` (`port`,
  `token`: 32 random hexadecimal digits from the OS, never logged elsewhere), runs the prelude, and then serves requests:
  one line `{"token": …, "steps": [ … ]}` in, the transcript lines of the sent steps out. It accepts loopback
  connections only and closes a connection whose first line lacks the token. It ends when the run ends (after the
  epilogue of `stop`) or after an idle period without a request (default 30 minutes, `--idle-minutes`), terminating the
  child; an idle end leaves `end: lost` until `resume`.
- **A check run** (`--kind check --of <run>`) needs a stopped drafter run; its prelude is the harness's own: it copies the
  drafter run's `work/project` into the check run's `work/project` (the drafter's folder stays untouched evidence) and
  sends the first-run defaults and `file.open` with `dir: project`; its `checker.md` is copied into the drafter run's
  folder at `stop`.
- `do` and `look` read `session.json`, connect, send, and print. A failed connection prints
  `harness: session lost, run cargo xtask blind resume <run>`.
- `resume <run>` stops a keeper that still answers, deletes `<run>/work` and `<run>/config` (inside the run folder only),
  spawns a fresh keeper, replays `prelude.uiscript` and `steps.uiscript`, and compares each replayed step's action,
  outcome and reason with `transcript.jsonl`. Equal: the run goes on. Different: `end: diverged`, with the first
  differing step in `run.json`.
- The keeper's idle timer reads a clock behind a trait; tests drive a manual clock (no test sleeps, ID-559).

### 5.5 The product additions (part 1)

#### 5.5.1 `--keep-going`

`bin/elektrawatt/src/main.rs` accepts `--keep-going` with `--headless` only (else a usage error, exit 2) and passes it to
`ew_ui::headless::run` (`HeadlessArgs.keep_going: bool`). `ew_viewmodel::script::run` and `run_stream` take a
`keep_going: bool`: when true, a step that is not done is written as now and the loop continues; `app.quit` still ends
the run. `RunEnd` gains `Completed { quit, not_done: usize }` (`not_done` counts the refused and failed steps); the exit
code is 0 when `not_done` is 0 and the project is clean, else 1; usage errors stay exit 2 and still end the run (the
harness never sends a malformed line). Without the flag every byte of every transcript and every exit code is unchanged
(the existing headless tests pass unmodified). The usage text gains the flag.

#### 5.5.2 Parameter signatures

`ActionMeta` gains `params: &'static str`. The grammar (checked by a unit test for every entry):

```text
signature := "" | param ("," " " param)*
param     := name ["?"] ":" type
name      := [a-z][a-z0-9_]*
type      := "mm" | "int" | "uint" | "bool" | "text" | "path" | "json" | "ref"
           | "enum(" word ("|" word)* ")" | "list(" type ")"
```

`?` marks an optional parameter; `ref` is a reference such as `tag:-101F1` or `folio:101` (WP-S2.05's
`resolve_reference`; FT-412 extends where it is accepted). Every action that reads parameters has a signature; a test
asserts that every `needs_parameters`, `headless_only` and `hidden` action either has a non-empty signature or is in the
test's explicit list of parameterless actions. Examples: `canvas.press` `x_mm:mm, y_mm:mm, button?:enum(primary|middle),
shift?:bool, ctrl?:bool, alt?:bool, group?:uint`; `nav.goto` `folio?:text, cell?:text`; `page.describe` `folio?:text`.
Each later part that adds an action gives its signature (its spec table's "Params (headless)" column), as a shared
append-only change.

#### 5.5.3 `page.describe`

A headless-only `View` action (`headless(…)`, context global, params `folio?:text`, default the focused tab's page);
refused with `reason-no-project` or `reason-no-page`, failed with `error-unknown-folio` for an unknown folio. Its
transcript line carries the member `describe` (as `ui.describe`'s does) with the text below, built from
`Session::page_tables` (WP-S2.04 §4.3), the device tags the scene prints and the page's cell map (`PageCells::cell_at`).
Texts in quotes are user text or labels; positions are page millimetres with three decimals:

```text
page "101" "Power Supply 1 - Main Current" reps 17 connections 21 junctions 2
  rep "S51" cell "101-B2" symbol "<symbol key>" variant "A" at 50.800 38.100 mm
    cp "S51:1" at 50.800 31.750 mm wires 1 potential "L-S"
    cp "S51:2" at 50.800 44.450 mm wires 0
  rep none cell "101-D5" symbol "<symbol key>" variant "A" at 76.200 63.500 mm
  rep ip "L-1" direction source cell "101-H15" at 381.000 177.800 mm
    cp "L-1" at 381.000 177.800 mm wires 1 potential "L-1"
  wire "-101F1:2" -> "L-1" route stored bends 2
  wire "F55:2" -> junction 1 route provisional bends 1
  junction 1 cell "101-C12" at 304.800 76.200 mm style dot wiring star sequence "F55:2" "E55:L" "E56:X2"
```

- **Header:** the folio, the title, and the exact counts of WP-S2.04 §13.6: representations (`reps`), drawn connections
  (`connections`), junctions (`junctions`); from WP-S2.04 part 4a on, `items <n> boxes <n>` follow.
- **Representations** in reading order: anchor cell row A to H, then column 0 to 16, then position y, then x, then symbol
  key. The label is the device's short tag as the page prints it, `ip "<name>" direction <word>` for an interruption
  point (WP-S2.04 part 3's words), or `none`. Connection points in index order with their printed label, position, the
  number of drawn connections ending there (`wires`) and the derived potential name when there is one.
- **Connections** sorted by their end labels: `wire <end> -> <end> route stored|provisional bends <n>`; an end is a CP
  label in quotes or `junction <n>`.
- **Junctions** numbered 1, 2, … in reading order of their position; `sequence` names each member by its far-end CP label
  (WP-S2.04 part 2's rule for `junction_wires`).
- **Items and boxes** (once WP-S2.04 part 4a's fields exist): `item <graphic|text|marker|image|unknown> "<cell>"` with
  the text of a free text or marker in quotes, and `box structure "<cell>" "<caption>"`. If WP-S2.04 part 4a has
  integrated when part 1 lands, part 1 prints them; otherwise WP-S2.04 part 4b's lane adds the two lines and their words
  (shared append-only files; the second to integrate rebases).
- **No UUID** appears in the text. **Words:** `reps`, `junctions`, `wires`, `stored`, `provisional` (5, part 1) and
  `boxes`, `unknown` (2, with the items); the lines also use `page`, `cell`, `cp`, `at`, `mm`, `none`, `junction`,
  `item`, `box`, `structure` (existing), `rep`, `symbol`, `variant` (WP-S2.04 part 1), `wire`, `->`, `route`, `bends`,
  `style`, `dot`, `bevel`, `wiring`, `star`, `chain`, `sequence` (part 2), `ip`, `direction`, `source`, `undirected`,
  `potential` (part 3), `connections`, `text`, `graphic`, `marker` (part 4b), `image` (part 5), `items` (WP-S2.05 part
  1). A word that an earlier part has not added yet is added by this part, which states the new count; the vocabulary
  test asserts every token of these lines (ID-474's rule; a reviewed transcript change).

#### 5.5.4 The reference files

`crates/ew-viewmodel/tests/blind_reference.rs` renders `tools/blind/reference/actions.txt` (one line per action in
registry order: `<id> <View|Command> "<English label>" menu "<menu>[ ▸ <submenu>]"|none shortcut "<first>"|none context
<global|canvas|navigator>[ headless][ hidden][ needs-parameters] params "<signature>"`) and
`tools/blind/reference/vocabulary.txt` (a header line, then the words of `VOCABULARY` in order, one per line, then
`markers:` and `FREE_TEXT_MARKERS`), and compares them with the committed files; under `EW_BLESS=1` it writes them. The
failure message names the bless command. FT-314 is closed by part 1 with this test.

### 5.6 Prompts, personas and composition (part 2)

- **`tools/blind/BLIND_DRAFTER.md`** (prompt `blind-drafter`, version 1): the role (draw as a user would, with EW alone);
  what may be read (the composed prompt, `cargo xtask blind help`, `elektrawatt --help`, `ew help`, the user guide under
  `docs/book/`; never the design documents, the code, the tests or other runs); the protocol of §5.2 in plain words; the
  live note after every turn with the two scores and what the agent expected, saw and felt; the broken and quit rules;
  the caps; that there are no pixels and why; the fixed **hand-back** with exactly these seven headings, in this order:
  `1. Keep or drop` (a score 0 to 10 and one sentence), `2. Where it lost me`, `3. What the tool is`, `4. Broken`,
  `5. The drawing set against the design language` (with the time a QET veteran would need for the same set),
  `6. Flow chart` (a Mermaid flowchart of the run as experienced), `7. Good` (ID-341).
- **`tools/blind/personas/qet-veteran.md`** (persona `qet-veteran`, version 1): a fictional panel-building engineer with
  many years of QElectroTech drawings of control cabinets; habits (folios and cells, strips like `X1`, wire numbers on
  the drawing, drag habits translated to keyboard and pointer steps, impatience with dialogs) and vocabulary; no
  knowledge of EW. Modelled on the pilot's engineers as a role, **never named, never a real person** (ID-341).
- **`tools/blind/personas/switcher.md`** (persona `switcher`, version 1): a fictional engineer switching from EPLAN or
  E3, with their terms (device tag, master data, navigator, macro) and expectations; no knowledge of EW.
- **`tools/blind/CHECKER.md`** (prompt `blind-checker`, version 1): the reviewing engineer's role; reads the task sheet,
  `look`, `look page`, view-only actions and `cargo xtask blind report <run> check|bom|score`; never `drafter.md` or
  `handback.md`; writes `checker.md` with five headings: `1. Verdict per folio`, `2. Misses against the task sheet`,
  `3. Findings worth fixing` (from the check report), `4. What a reviewer would mark up`, `5. Confidence`.
- **`tools/blind/HELP.md`**: the protocol as the agent needs it (commands, the `key=value` form, the batch form, `look`
  sections, the note format, the hand-back headings, the refusal texts of §5.3, the outcome words, how a describe line is
  read: structural words versus quoted user text, ID-474), and nothing about EW's design.
- **`compose`** writes `prompt.md` as the concatenation, in this order, of `BLIND_DRAFTER.md` (or `CHECKER.md`), the
  persona (none for the checker), the task sheet, and `HELP.md`, each preceded by a line `<!-- file: <path> sha256 <hex>
  -->`, with `{run}`, `{kind}` and the cap values substituted. Equal inputs give equal bytes.
- Every file is English, carries no customer or personal name, no address, no project or drawing number other than
  doc 20's J-E-2416 placeholders, no e-mail address and no local path; `cargo xtask check-anonymisation` covers them.

### 5.7 Tasks, the prelude and the oracle (part 2)

`tools/blind/tasks/<task>/` holds:

| File | Content |
|---|---|
| `task.md` | The task sheet the agent reads |
| `prelude.uiscript` | The setup steps (D11), an `ew.uiscript/1` script without a header label beyond the task name |
| `prelude.toml` | One entry per prelude step, in order: `class = "setup"` or `"drawing"`, and for `drawing` the follow-up that removes it (`gap = "FT-408"`); a check asserts one entry per step |
| `gaps.toml` | Per hard or soft check that an open gap blocks: the check ID, the follow-up, a one-line reason |
| `oracle/counts.toml` | The exact counts per folio with their doc 20 or WP-S2.04 §13.6 source: 101 `reps 17 connections 21 junctions 2`; 102 `reps 12 connections 19 junctions 4`; 103 (demo variant, ID-571) `reps 35 connections 20 junctions 0 terminals 15 strips 2` |
| `oracle/bom.csv` | Doc 20 §14.1 restricted to the devices on 101 to 103, quantities counted over those devices, with WP-S2.04 §13.6's demo rule for terminals and bridges; if `bin/elektrawatt/tests/gpp_demo.rs` (WP-S2.04 part 4b) commits its restricted list, this file equals it (a check) |
| `oracle/rows.sql` | One read-only query per soft check over the `api_v1_*` views (`api_v1_function`: folio, device tag, cell, cross-reference text; `api_v1_wire`: end tags and CPs; `api_v1_page`: titles), restricted to folios 101 to 103 and without IDs |
| `oracle/rows/*.csv` | The expected results, produced by `bless-oracle` from a temporary copy of Golden A (D16) and reviewed; folio 103's terminal rows follow the demo variant's labels (`-X1031:11/a`, WP-S2.04 §13.6) |

**The G-PP task `gpp-101-103`.** `task.md` is written by part 2 from doc 20 §4.3 (the structure identifiers), §5 (the
three page rows), §6.1 (the devices on 101 to 103 with function text, part, connection points and anchor cell), §7.3
(the potentials on these folios), §8.2 (the wires with their ends), §8.3 (the junctions J1 to J6 and the bridges), §9
(the terminals of -X1031 and -X1032 that 103 uses) and §15 (the cross-references among 101 to 103). It reads like an
engineer's job sheet ("Draw folio 101, Power Supply 1 - Main Current: the supply from the upstream cabinet +J-E-206
arrives on strip 1X1 …"), followed by one table per folio (device, function text, part, cell, connection points), one
connection list per folio and the junction table; folio 103 asks for terminals placed one by one (ID-571). A check
(`cargo xtask blind check-task <task>`) asserts that every identifier-shaped token of `task.md` (a tag, a folio, a cell,
a part number, a potential name, a structure identifier) occurs in doc 20, and that the sheet holds no other proper
name.

**The prelude of `gpp-101-103`.** The first-run flow's defaults and the company-standard wizard with its defaults
(WP-S2.01's flow, as WP-S2.04 §13.6 starts), `file.new` into `project`, the golden part copies by `command.run`
`library.embed` from the copies `golden/J-E-2416.ew/lib/` holds (their parameters written into the prelude as data, so
the run never opens the fixture), all `setup`; and, while their gaps are open, `page.create` for 101, 102 and 103
(`drawing`, FT-408) and the structure node and external reference (`drawing`, FT-409). Part 2 writes it; each design
pass that closes a gap removes the lines and records the change in the task's history section.

**The free task `free`.** `task.md` asks the agent to choose and draw a small control-cabinet circuit of its own with
placeholder data, to save it and to export the PDF; no prelude beyond the first-run defaults; no oracle.

**The self-test task `selftest`** (part 1): a scripted fake agent `tools/blind/tasks/selftest/agent.jsonl` (turns of
`do`, `look` and `note` as data), run by `cargo xtask blind selftest`, with a prelude (`file.new`, the wizard's
defaults, one page by `command.run`), two placements with WP-S2.04 part 1's Place tool, one deliberately refused step
(`edit.redo` with nothing to redo), two looks and a `page.describe`; about 60 steps.

### 5.8 `stop`, the epilogue and the reports

`cargo xtask blind stop <run> [--reason <r>]` requires a hand-back unless the reason is `orchestrator`, then sends the
**epilogue** (part of `steps.uiscript`): `file.save` (if the project is dirty), `page.describe` for every folio of the
task's oracle, `app.quit`; it waits for the child's exit and records the exit code. Then it runs, outside the
transcript, `ew export pdf work/project --out out/project.pdf --threads 1` and `--threads 4` into
`out/project-t4.pdf`, `ew export bom work/project --mode parts --out out/bom.csv`, `ew check work/project --format text
--out out/check.txt` and `--format json --out out/check.json`, then scores (§5.9) and writes `out/score.json` and
`out/report.md` (the score as a table, the protocol counts, the prelude lines, the broken steps, the harness refusals).
`cargo xtask blind report <run> check|bom|score` prints those files for the checker. No PNG is written: the reader
reads the PDF (ID-341's "PNGs" are replaced by the PDF, D14's amendment).

### 5.9 Scoring

`score.json` (`ew.blindscore/1`): `run`, `task`, `outcome`, `hard` and `soft` lists of `{id, what, want, got, status,
misses}` with `status` one of `ok`, `miss`, `gap` (with `gap: "FT-…"`), and `protocol` (the counts of `run.json`, the
per-folio turn use, the prelude lines by class).

| ID | Check | Source of `got` | Oracle |
|---|---|---|---|
| H1, H2, H3 | Exact counts on 101, 102, 103 (folio 103 also `terminals` and `strips`) | The epilogue's `page.describe` headers (and the terminal count from `ew sql` over `api_v1_terminal`) | `oracle/counts.toml` |
| H4 | The parts list equals §14.1 restricted to 101 to 103 (AC-07's rule on the new project) | `out/bom.csv` | `oracle/bom.csv` |
| H5 | The PDF exists and is byte-identical on 1 and 4 threads (AC-14's rule) | `out/project.pdf`, `out/project-t4.pdf` | SHA-256 equality |
| H6 | `ew check` ran (exit 0 or 1) | `out/check.json` | — |
| H7 | No prelude line of class `drawing` | `prelude.toml` | zero |
| S1 | Anchor cells (AC-02's rule) | `ew sql` with `oracle/rows.sql` over `work/project` | `oracle/rows/cells.csv` |
| S2 | Endpoint labels of drawn connections, up to orientation | the same | `rows/ends.csv` |
| S3 | Junction sequences by far-end labels | the epilogue's `page.describe` | `rows/junctions.csv` |
| S4 | Cross-reference texts among 101 to 103 (AC-05's rule) | `ew sql` | `rows/xrefs.csv` |
| S5 | Tags, frozen tags included (AC-06's rule) | `ew sql` | `rows/tags.csv` |
| S6 | Page titles | `ew sql` | `rows/pages.csv` |
| S7 | Marker texts and `ip_structure` (after WP-S2.04 part 4b) | `ew sql` | `rows/markers.csv` |
| S8 | Findings on 101 to 103 against §18.1 restricted (AC-21's comparison: rule ID, sorted target tags, severity) | `out/check.json` | `rows/findings.csv` |

A check is `gap` when it is not `ok` and `gaps.toml` lists it with an open follow-up (`gap = "FT-…"`) or a package part
not yet delivered (`gap = "WP-S2.04 part 2"`), or when `--folios` left its folio out (`gap = "not in this run"`); a gap
closed in the plan but still listed is a review finding. Soft misses list each differing row (at most 50 per check, then a count).

**Outcome.** `pass`: every hard check `ok`. `pass-with-gaps`: every hard check `ok` or `gap`, at least one `gap`.
`fail`: any hard check `miss`. `incomplete`: the run ended by `cap`, `quit`, `orchestrator`, `lost` or `diverged`
before the epilogue, or a folio was not started. Soft misses never change the outcome; they are listed. The score of a
`free` run holds only `protocol`.

### 5.10 Recording, bless and replay

- **Normalisation:** every UUID in a transcript line (state and describe text) is replaced by `«n»`, numbered from 1 in
  order of first appearance over the whole run; the transcript header's version is replaced by `«version»`.
- **`bless <run> --as <name> [--ci pr|nightly|none]`** copies into `tools/blind/recordings/<name>/`: `run.json` (with
  `ci`), `prompt.md`, `prelude.uiscript`, `steps.uiscript`, `turns.jsonl`, `outcomes.jsonl` (per step: `step`,
  `action`, `outcome`, `reason`), `epilogue.txt` (the `page.describe` texts of the epilogue), `transcript.jsonl`
  normalised, `drafter.md`, `handback.md`, `checker.md` when present, `out/score.json`, `out/report.md`,
  `out/check.txt`, `out/bom.csv` and `out/project.pdf.sha256`. Not the project, not the PDF, not `record.ewcmd`'s
  UUIDs: `record.ewcmd` is committed normalised as `record.ewcmd.txt` (evidence, not replayable, D14). A normalised
  transcript above 8 MiB keeps its describe members as `{"sha256": …}` and the replay compares digests for those.
  `bless` refuses a run whose `end` is `lost` or `diverged`.
- **`replay <recording|run> [--bin <dir>]`** runs `prelude.uiscript` and `steps.uiscript` through
  `elektrawatt --headless --script <file> --keep-going` in a temporary work folder with a temporary config folder,
  normalises, and compares: (R1) every step's `action`, `outcome` and `reason` with `outcomes.jsonl`; (R2) the epilogue's
  `page.describe` texts with `epilogue.txt`, byte for byte; (R3) the normalised transcript with the committed one, as a
  reported count of differing lines (not a failure, because a reviewed vocabulary change alters describe text without
  changing what the agent could do). R1 and R2 decide.
- **CI** (D15): `bin/elektrawatt/tests/blind_replay.rs` implements R1 and R2 against the built binary
  (`CARGO_BIN_EXE_elektrawatt`) for every recording with `ci = "pr"` and `stale = null`, in a temporary directory with
  `EW_CONFIG_DIR` unset and `--config-dir` set; `blind_replay_long` (`#[ignore]`) does the same for `ci = "nightly"`. A
  test needs no UUID normaliser: outcomes and `page.describe` carry no UUID.
- **A stale recording:** when an intended product change makes R1 or R2 fail, the lane sets `stale` in the recording's
  `run.json` to a one-line reason naming its change (a reviewed diff); the test then skips the recording and prints the
  reason. The next live run of the task replaces it, and the design pass lists stale recordings in its plan view.

### 5.11 How a model is invoked (outside CI)

The orchestrator, never CI:

1. `cargo build -p elektrawatt -p ew` (the debug profile suffices).
2. `cargo xtask blind start --kind gate --task gpp-101-103 --persona qet-veteran --run gpp-r1` (the folder
   `target/blind/gpp-r1/`).
3. Starts one agent on Sonnet (ID-341) with `target/blind/gpp-r1/prompt.md` as its whole instruction and a shell as its
   only tool, and lets it work until it files its hand-back or a cap or quit rule ends the run.
4. `cargo xtask blind stop gpp-r1`.
5. `cargo xtask blind start --kind check --task gpp-101-103 --of gpp-r1 --run gpp-r1-check`, a second Sonnet agent with
   that prompt, then `stop`.
6. `cargo xtask blind replay gpp-r1` (R1 and R2 must pass), then `cargo xtask blind bless gpp-r1 --as gpp-<n> --ci
   nightly`.
7. An Opus lead reads `drafter.md`, `handback.md`, `checker.md`, `report.md` and the PDF and writes the findings as
   follow-up tasks; the design pass records them.

The agent's vendor and model are the orchestrator's choice; the protocol is text over a shell, so any agent with a shell
can drive it. No key, token or account is written into the repository or the run folder; the harness reads no
environment variable of a model vendor.

## 6. Determinism, the headless mode and exact counts

1. **The headless mode is synchronous.** One step in, one transcript line out; the harness never sleeps, polls or waits
   on time. The only timer is the keeper's idle period, behind a clock trait that tests drive by hand (WP-S2.02's hold
   pattern, ID-542; no timing assertion anywhere, ID-559).
2. **UUID normalisation** (§5.10) is the only rewriting of product output; it is applied identically to live runs,
   replays and committed recordings.
3. **Reproducible runs.** Equal repository state, equal binaries and equal agent steps give equal `steps.uiscript`,
   equal outcomes and an equal `page.describe` epilogue; `compose` is byte-deterministic; the harness writes no time,
   host, user, process ID or absolute path into a run folder or a recording (the anonymisation guard and a test check
   recordings for absolute paths).
4. **Exact counts** (NFR-PERF-20; WP-S2.04 §12 item 4's counters): `page.describe` makes exactly one `page_tables` call
   when the (revision, folio) is not in the page-content cache and 0 when it is, and causes 0 memo re-executions beyond
   those of that call; `ui.describe` and `page.describe` steps change no state member except `status` where an existing
   rule says so (checked on the self-test recording: the state before and after each look is equal).
5. **Named tie-breaks** for every ordering of `page.describe` (§5.5.3) and of the score's miss lists (by check row key);
   none depends on a hash map's order or on an ID (NFR-REL-07).
6. **`--keep-going` changes nothing without the flag** (§5.5.1): the existing headless tests and transcripts are
   unmodified.
7. **No pixels reach the agent:** the filter of §5.3 is the only path to the product, and no harness command prints or
   writes an image.

## 7. Acceptance checks

Part 1's checks are ordinary nextest tests in the existing jobs (`xtask`, `ew-viewmodel`, `ew-ui`, `elektrawatt`), plus
`cargo xtask blind selftest` run by the lane; parts 2 and 3 are checked by the lane or the orchestrator as stated.

| # | Check | Where | Proves |
|---|---|---|---|
| BD1 | `--keep-going`: a script with a refused step in the middle runs to its end, writes every step's line, exits 1; the same script without the flag stops at the refusal, byte-identical to today's transcript; `--keep-going` without `--headless` exits 2; the usage text lists it | `ew-viewmodel` (`run`, `run_stream`), `elektrawatt` tests | §5.5.1, D7 |
| BD2 | Signatures: every entry of `ACTIONS` parses with the grammar; every `needs_parameters`, `headless_only` and `hidden` action has a signature or is in the parameterless list; `canvas.press`, `nav.goto`, `page.describe` carry the signatures of §5.5.2 | `ew-viewmodel` tests | §5.5.2 |
| BD3 | Reference files: `actions.txt` and `vocabulary.txt` equal the registry and the two lists; a changed word fails with the bless hint; `EW_BLESS=1` rewrites them | `ew-viewmodel` test `blind_reference` | §5.5.4, D9 |
| BD4 | `page.describe` on a temporary copy of Golden A: folios 101, 102, 103 print the header counts 17/21/2, 12/19/4, 23/20/0 (folio 103 with its three strip segments) and the rows of §5.5.3 (an expected text per folio, `tests/expected/page_describe_10{1,2,3}.txt`); no UUID in the text; the vocabulary test passes with the new words and states the count; refusals for no project, no page, an unknown folio | `ew-viewmodel` tests | §5.5.3, D8 |
| BD5 | Exact counts of §6 item 4 for `page.describe` (cached and uncached) | `ew-ui` test | §6 |
| BD6 | Filter: each refusal of §5.3 with its text (a UUID nested in a list, a path with `..`, `command.run`, `canvas.snapshot`, an unknown action, a `Command` action in a check run, 64 KiB + 1); a refused step is not sent | `xtask` unit tests with a fake product | §5.3, D4 |
| BD7 | Protocol: note required; stop at the first not-done step with the rest `skipped`; broken after two equal failures and the exact step refused thereafter while other parameters pass; the quit rules at the third and fifth low note; each cap ends the run; looks per turn capped; the `key=value` parser's typing | `xtask` unit tests with a fake product | §5.2, D5 |
| BD8 | Keeper: token required, loopback only; `do` against a stopped keeper prints the lost message; `resume` replays and continues when outcomes repeat and ends `diverged` when a fake product changes one outcome; the idle end with a manual clock | `xtask` unit tests (fake product, manual clock); `selftest` | §5.4, D6 |
| BD9 | `compose` is byte-deterministic and records each file's SHA-256; `run.json`, `turns.jsonl`, `score.json` round-trip; no absolute path, time stamp or host name in any written file (a scan of the self-test run folder) | `xtask` unit tests; `selftest` | §5.1, §5.6, §6 |
| BD10 | Scoring on fixtures: a project folder fixture and canned CLI outputs give each status (`ok`, `miss`, `gap`) and each outcome; a gap listed for a check that is `ok` is `ok` | `xtask` unit tests | §5.9, D12 |
| BD11 | Golden untouched: `bless-oracle` on the self-test task's oracle copies the fixture to a temporary directory; the SHA-256 of every file under `golden/` is equal before and after | `xtask` unit test with a fake copy; lane run | D16 |
| BD12 | `selftest`: the scripted agent runs through the real keeper and binary; the run ends `handback`; the refused step is recorded and the session continues; `bless --ci pr` writes `tools/blind/recordings/selftest/`; `replay` passes R1 and R2 | lane (`cargo xtask blind selftest`) | §5.7, §5.10 |
| BD13 | CI replay: `blind_replay` passes on the committed self-test recording; a recording with `stale` set is skipped with its reason; a corrupted `outcomes.jsonl` line fails the test | `elektrawatt` test | §5.10, D15 |
| BD14 | Targeted checks for the changed crates (`xtask`, `ew-viewmodel`, `ew-ui`, `elektrawatt`): nextest, clippy `-D warnings`, `cargo fmt --all --check`, `check-deps` (no new edge), `check-licence-map` and `reuse lint` (after P1), `check-anonymisation`; no new package in `Cargo.lock`; golden unchanged | lane | testing policy |
| BD15 | Part 2: the prompt, persona, checker and help files exist with front matter; `check-task gpp-101-103` passes; `oracle/counts.toml` equals doc 20 and WP-S2.04 §13.6 (the `page.describe` headers of a Golden A copy for 101 and 102, the demo counts for 103); `bless-oracle` produced the rows on a copy; `check-anonymisation` passes on `tools/blind/` | lane | §5.6, §5.7 |
| BD16 | Part 2's dry run on folio 101 (after WP-S2.04 part 1): a drafter run of task `gpp-101-103` limited to folio 101 (`--folios 101`), placement only; the score's H1 representation count is `ok` or a listed miss, every connection check `gap` (WP-S2.04 part 2 not built); the reader's notes become prompt revisions (version 2) or follow-ups; the run is blessed as `gpp-dry-101` with `ci = "none"` | orchestrator, lead | §5.11 |
| BD17 | Part 3's G-PP run: drafter and checker runs of `gpp-101-103` after WP-S2.04 part 4b and WP-S2.05 part 1; `replay` passes R1 and R2; blessed with `ci = "nightly"`; the score, hand-back, audit and the reader's findings exist | orchestrator, lead | §8 |
| BD18 | Every follow-up the reader writes names its owning package and a due point; every `gap` of the score names an open follow-up, an undelivered package part or "not in this run" | design pass | §5.9 |

The workspace suite, the acceptance runner and the performance runs are not run by the lanes (testing policy, ID-512);
the integrating session runs `cargo xtask ci` at the cycle delivery.

## 8. The gate evidence for G-PP

The G-PP record's "not criteria, but recorded" part ([gates](../03-gates.md) §5.4) gains, from part 3:

| Evidence | Where |
|---|---|
| The committed recording of the gate run | `tools/blind/recordings/gpp-<n>/` (§5.10) |
| The score with its outcome (`pass`, `pass-with-gaps`, `fail`, `incomplete`), every hard and soft check and every gap with its follow-up | `out/score.json`, `out/report.md` in the recording |
| The drafter's hand-back (keep/drop score, where it lost its way, broken, the QET-veteran time estimate) and its notes | `handback.md`, `drafter.md` |
| The checker's audit | `checker.md` |
| The replay result at the review (R1 and R2) | the nightly's `blind_replay_long` or a `cargo xtask blind replay` by the chair |
| The reader's findings as follow-up tasks | plan 02 Appendix B, through the design pass |

**Reading.** The chair reads the run with the pilot's issue list. A `pass` says the text interface alone lets the task
be done; `pass-with-gaps` names what the product still lacks for it; `fail` names an agent or product miss. None of
them changes G-PP-01's outcome, which is the pilot's (ID-341: never blocking). G-M2's run (gates §5.5) reuses the harness
with a new task when the owner schedules it.

## 9. Files owned

**Part 1:** `xtask/src/main.rs` (the `blind` dispatch and its usage lines only), a new `xtask/src/blind/` (`mod.rs`,
`run.rs`, `protocol.rs`, `filter.rs`, `keeper.rs`, `compose.rs`, `score.rs`, `replay.rs`, `normalise.rs`, and their unit
tests); `crates/ew-viewmodel/src/actions.rs` (the `params` field, `with_params`, the signatures, the `page.describe`
entry), `crates/ew-viewmodel/src/describe.rs` (`page.describe`'s text and the words), `crates/ew-viewmodel/src/script.rs`
(`keep_going`, `RunEnd`, the `describe` member for `page.describe`), `crates/ew-viewmodel/src/shell.rs` (the
`page.describe` dispatch only), `crates/ew-viewmodel/src/strings.rs` (its label and error keys), a new
`crates/ew-viewmodel/tests/blind_reference.rs` and `tests/expected/page_describe_10{1,2,3}.txt`;
`crates/ew-ui/src/headless.rs` (`keep_going`); `bin/elektrawatt/src/main.rs` (the flag and usage), a new
`bin/elektrawatt/tests/blind_replay.rs`; new `tools/blind/reference/{actions.txt,vocabulary.txt}`,
`tools/blind/tasks/selftest/**`, `tools/blind/recordings/selftest/**`, `tools/blind/HELP.md` (first version) and
`tools/blind/README.md` (the orchestrator procedure of §5.11).
**Part 2:** `tools/blind/{BLIND_DRAFTER.md,CHECKER.md,HELP.md}`, `tools/blind/personas/{qet-veteran.md,switcher.md}`,
`tools/blind/tasks/{gpp-101-103,free}/**`, `tools/blind/recordings/gpp-dry-101/**`; the `check-task` subcommand in
`xtask/src/blind/` if part 1 did not ship it.
**Part 3:** `tools/blind/recordings/gpp-<n>/**`; prompt or task revisions with version bumps; nothing else.

Shared append-only files (`ACTIONS`, `VOCABULARY`, the transcript members, `strings.rs`) may be touched by every part and
by the lanes of WP-S2.04 and WP-S2.05; the second to integrate rebases onto the first and re-blesses the reference files.
Read-only for the lanes: every other crate, `golden/**`, `ci/**`, `deny.toml`, `supply-chain/**`, `.github/**`,
`docs/**`, `REUSE.toml`. `ci/licence-map.toml`, `REUSE.toml`, doc 19 §2.1 and `LICENSE.md` are amended by the design
pass before part 1 (P1), never by a lane. Each lane commits on its own branch and does not merge or push.

## 10. Out of scope

| Item | Where it goes |
|---|---|
| A GUI for the harness, screenshots, image diffing, OCR, any pixel path to the agent | Never (ID-341's method; D4) |
| The live runs after other large UI changes (the M2 run of gates §5.5) | The 20 % reserve, when scheduled (WBS scope out) |
| Fixing what a run finds | The owning packages, through follow-up tasks |
| The GUI "Describe page" command and its full FR-UI-046 text (function texts, parts, connections by potential) | 1.0 (FR-UI-046); `page.describe` is its headless v0 |
| `page.new`, structure and external-reference creation, part import, bridges, UUID-free references and labelled hover and selection | FT-408 to FT-412 |
| A replayable command log of a new-project session (pinned minted IDs or a seeded ID source) | FT-416 (lead, M2) |
| The apprentice persona | Later (ID-341), when its first run is scheduled |
| The moderated usability rounds and NFR-UX-01 | WP-S2.12, WP-S2.16 |
| Performance of the harness, the keeper or `page.describe` | Release phase (ID-512); not measured |

## 11. Review checklist for the Opus reviewer

1. **Boundaries.** No new workspace edge or package; `xtask` drives binaries only; the product changes are exactly
   `--keep-going`, `ActionMeta.params`, `page.describe` and the reference test; nothing under `golden/` changed.
2. **The agent's senses.** The filter is the only path to the product; `command.run` and the three snapshot actions are
   refused; paths stay inside the run folder; UUID parameters refused; no harness command emits an image.
3. **`--keep-going`.** Without the flag, byte-identical transcripts and unchanged exit codes; with it, the exit-code
   rule of §5.5.1.
4. **`page.describe`.** Exactly the lines and order of §5.5.3; no UUID; the header counts equal the page tables'; the
   words added with the stated count; exact counts.
5. **The protocol.** Note gating, stop-at-first-failure, broken, quit, caps and look limits as §5.2; all counted, none
   timed.
6. **The keeper.** Loopback only, token checked, no secret logged, idle end via the clock trait, `resume` deletes only
   inside the run folder and detects divergence.
7. **Determinism and privacy.** No time, host, user, process ID or absolute path in run folders or recordings; UUID
   normalisation as §5.10; replay R1 and R2 decide; the self-test recording replays in CI.
8. **Golden safety.** `bless-oracle` works on a copy and the before-and-after digest check exists.
9. **Licence and anonymisation.** P1 done before any file under `tools/` lands; `reuse lint`, `check-licence-map` and
   `check-anonymisation` green; personas are roles, never people.
10. **Scope.** Nothing of §10 half-done; the vocabulary gaps are follow-ups, not quietly built.

## 12. Size, order, routing and the documents to update

### 12.1 Size

WP-S6.13 keeps **1.6 pw PERT** (O 1 · L 1.5 · P 2.5); the CSV is unchanged. **Part 1, 0.9 pw:** the run folder,
filter, protocol, notes, hand-back and compose 0.3; the keeper and `resume` 0.15; stop, scoring, `bless-oracle`, `bless`
and `replay` 0.15; `--keep-going` 0.05; signatures and the reference test 0.1; `page.describe` with its expected texts
0.1; the CI replay test and the self-test recording 0.05. **Part 2, 0.4 pw:** the prompt, the personas, the checker and
the help 0.1; the task sheet, prelude, gaps and oracle with `check-task` 0.15; the dry run on folio 101, its reading and
the prompt revision 0.15. **Part 3, 0.3 pw:** the G-PP run, the checker run, replay and bless, the reading and the
follow-ups. The parts sum to **1.6 pw: no reserve.** The PERT predates the gaps of §2.1 (the keep-going semantics, the
missing parameter metadata and page content), which part 1 absorbs. **De-scope rule:** when a part is projected (at its
lane's midpoint report) or found (at its review) more than 20 % over its size, these items leave the package for
follow-ups in this order without a further ruling: `resume` (a lost session then restarts the run), `look --diff`, the
checker run of part 3 (the reader audits instead), the `switcher` persona; the lane reports which ones left.

### 12.2 Order and routing

**Precondition P1, before part 1:** the design pass adds `tools/**` to `ci/licence-map.toml` (code and scripts
MIT OR Apache-2.0 as L12, prose `tools/**.md` CC-BY-4.0 as L9, recordings, references and task data
`tools/blind/recordings/**`, `tools/blind/reference/**`, `tools/blind/tasks/**` except Markdown CC0-1.0 as L8), the
matching `REUSE.toml` annotations, doc 19 §2.1 and `LICENSE.md`, by the licence route of doc 19 §3.4 (FT-415; owner
question Q2). A lane that finds P1 undone stops.

Part 1 starts after WP-S2.04 part 1's delivery and review (it needs `Session::page_tables`); it may run beside WP-S2.04
part 2 and WP-S2.05's parts (shared append-only files rebase). It goes to a **programmer lane at D3** (`sonnet`, `high`):
the specification is closed and the product changes follow existing patterns, but it touches more than three files and
a public text format, and the keeper has no pattern to copy; Opus review. Part 2 starts after part 1: a **designer at
D3** (`sonnet`, `high`) writes the files; the orchestrator runs the dry run (the drafter on Sonnet, ID-341) and an Opus
lead reads it. Part 3 starts after WP-S2.04 part 4b and WP-S2.05 part 1, ideally after the chain and FT-408 to FT-412,
and before the G-PP review: the orchestrator runs it (drafter and checker on Sonnet) and an **Opus lead at D4** reads and
triages it. Each part is one lane-cycle; part 3 may be repeated when the chain closes more gaps.

### 12.3 Decision-log entries (recorded as ID-610 to ID-619)

1. **ID-610** WP-S6.13 specified in three parts with sizes, order, routing, no reserve and the de-scope rule (D1, §12.1,
   §12.2).
2. **ID-611** Home and language: Rust in `xtask/src/blind/` as `cargo xtask blind`, the files under `tools/blind/`, no
   edge and no runtime dependency; Python rejected; precondition P1 (D2).
3. **ID-612** Two run kinds, `free` as ruled and `gate` with the task `gpp-101-103`; the gate run is G-PP's recorded
   rehearsal, never a criterion (D3, D12's last clause; owner question Q1).
4. **ID-613** The agent's senses and limits, the filter, and doc 20 reaching the agent only through the task sheet
   (D4).
5. **ID-614** The protocol (turn, note, stop at first failure, broken, quit, caps, looks) and the loopback keeper with
   `resume` (D5, D6).
6. **ID-615** The product additions: `--keep-going`, `ActionMeta.params` with the pinned reference files (FT-314 closed
   by part 1), `page.describe` as FR-UI-046's headless v0 with 7 words (ID-474 extended; a reviewed transcript change)
   (D7 to D9).
7. **ID-616** Prompts, personas and the checker as versioned files; compose; the apprentice later (D10).
8. **ID-617** The prelude with its classes, the task folder, and the scoring with hard and soft checks, the statuses and
   the outcomes (D11, D12).
9. **ID-618** The model outside CI; the UI script as the replay artefact with UUID normalisation; ID-341 amended (the
   `.ewcmd` kept as evidence, the PDF instead of PNGs); CI replay with `pr`, `nightly` and `stale`; golden only through
   copies (D13 to D16).
10. **ID-619** The vocabulary gaps of §2.3 and their follow-ups FT-408 to FT-416; the plan view's chain text.

### 12.4 Follow-up tasks (recorded as FT-408 to FT-416)

1. **FT-408** `page.new` (Page ▸ New page and the pages navigator's New page; params `folio`, `type`, `title`,
   `section`), committing `page.create` through the host; WP-S2.06; before G-PP.
2. **FT-409** Structure nodes and external references created from the UI (the location `+J-E-206` and the external
   target `JE-N1000-03` of folio 101's external points); WP-S2.06, WP-S1.12; before G-PP.
3. **FT-410** Part copies into a new project from the UI: File ▸ Import ▸ Parts table… driving WP-S1.19a's CSV import
   with its column mapping and a previewed commit; WP-S1.19a, WP-S2.09; before G-PP.
4. **FT-411** Strip bridges from the UI before the strip editor: "Bridge terminals" on a selection of terminals of one
   strip (one command, previewed); WP-S2.04 (part 3) or WP-S1.14; before G-PP; a G-PP-01 gap for the pilot as well
   (Q3).
5. **FT-412** UUID-free references for every UUID-valued UI parameter (`place.pick` `function`, `ip.external`, the
   junction editor's target; signature type `ref`), resolved through WP-S2.05's `resolve_reference` plus a connection
   point by its printed label; and the `canvas hover` and `canvas selection` describe lines naming each entity's kind and
   label after its UUID (doc 16 §4.6's announcement data; FT-359's rich announcement stays there); WP-S2.04 parts 2 and
   3, WP-S2.05 part 1; before G-PP.
6. **FT-413** The `switcher` free run after WP-S2.05 part 1, as the first free run, read by an Opus lead; WP-S6.13;
   after part 2 (in the reserve, as a run).
7. **FT-414** The deep nightly runs `blind_replay_long` (`cargo nextest run -p elektrawatt --run-ignored only -E
   'test(blind_replay_long)'`) on Linux; the orchestrator's change to `deep-nightly.yml`; with part 3.
8. **FT-415** Precondition P1: `tools/**` in `ci/licence-map.toml` and `REUSE.toml`, doc 19 §2.1 and `LICENSE.md`; the
   design pass, by the licence route; before WP-S6.13 part 1.
9. **FT-416** A command log of a new-project session that `ew apply` can replay on a fresh project: either the recorder
   pins the minted IDs (an advisory member of `ew.cmdlog/1` lines consumed by a replay ID source) or the headless mode
   offers a seeded ID source for test projects (ADR-0004's uniqueness question); a lead decision (D5); M2.

### 12.5 Open questions for the owner

- **Q1.** The `gate` run kind (a fixed task sheet with an oracle) beside the owner's free run (ID-341: the agent chooses
  its own task). The lead recommends both; the free run stays the default after large UI changes.
- **Q2.** The licence rows for `tools/` (P1, FT-415): code L12 (MIT OR Apache-2.0), prose L9 (CC-BY-4.0), data L8
  (CC0-1.0), by the licence route of doc 19 §3.4.
- **Q3.** Bridges at G-PP: G-PP-01's folio 103 holds the bridges BR1 to BR6 and no UI gesture creates one before
  WP-S2.07 (M2). Schedule FT-411 before G-PP (the lead's recommendation, about 0.3 pw), or accept folio 103 without
  bridges at G-PP together with ID-571's folio-103 confirmation.
- **Q4.** The committed size of a G-PP recording: the full normalised transcript (estimated 3 to 10 MB per run,
  describe members above 8 MiB kept as digests) or only outcomes, epilogue, score and notes with the transcript kept out
  of git. The lead recommends committing it, as the task's evidence.

### 12.6 Documents the design passes update

**Now (this design pass):** plan 01 §3.8 (the chain text: WP-S6.13 specified) and the M-PP row; decision log ID-610 to
ID-619; plan 02 (WP-S6.13's status, FT-408 to FT-416, FT-314's note); doc 18 §3.17 (what is specified for the drafter:
the harness, the gate kind, `page.describe` and its words, `--keep-going`, the reference files that pin the harness
help). **Before part 1:** P1 (`ci/licence-map.toml`, `REUSE.toml`, doc 19 §2.1, `LICENSE.md`). **After part 1:** doc 18
§3.17 and §3.11 as built (the replay test), doc 16 §3.9 (the headless mode's `--keep-going`, `page.describe`), doc 02b
FR-UI-046's note (the headless v0), WP-S2.04's specification §7.7 note (part 4b adds `page.describe`'s item and box
lines if part 1 integrated first), plan 02 (FT-314 closed). **After part 2:** doc 18 §3.17 (the prompts and personas as
built). **After part 3:** gates §5.4 (the evidence pointer of §8), plan 01 §3.8, the follow-ups of the reading.

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S1.12, WP-S1.14, WP-S1.19a, WP-S2.01, WP-S2.03 to WP-S2.07, WP-S2.09,
  WP-S2.12, WP-S2.16, WP-S6.01, WP-S6.13; Appendix B (FT-314, FT-341, FT-359, FT-399, FT-400).
- [Decision log](../06-decision-log.md): ID-341, ID-342, ID-443, ID-449, ID-474, ID-475, ID-493, ID-511, ID-512,
  ID-513, ID-537, ID-540, ID-542, ID-546, ID-557, ID-559, ID-571, ID-586 to ID-589, ID-597, ID-602, ID-608.
- [Quality](../../design/18-quality-security-release.md) §3.10, §3.11, §3.14, §3.17, §12;
  [UX/UI](../../design/16-ux-ui.md) §3.9, §4.6, §10; [golden project](../../design/20-golden-project.md) §3.1, §3.4, §4.3,
  §5, §6.1, §7.3, §8.2, §8.3, §9, §14.1, §15, §18.1, §21; [gates](../03-gates.md) §5.4, §5.5;
  [WP-S2.03](WP-S2.03-viewer-and-tool-framework.md), [WP-S2.04](WP-S2.04-placement-wiring-drawing-tools.md),
  [WP-S2.05](WP-S2.05-property-grid-find-replace-clipboard.md).
- Requirements: NFR-UX-04, NFR-UX-05, NFR-A11Y-02, NFR-REL-07, NFR-PERF-20, NFR-PRIV-09, FR-UI-046, FR-EXT-030; SC-9,
  PR-13; AC-02, AC-05, AC-06, AC-07, AC-14, AC-21; G-PP-01.
