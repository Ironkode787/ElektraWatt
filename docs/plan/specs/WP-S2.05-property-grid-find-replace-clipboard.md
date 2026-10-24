# WP-S2.05: property grid, provenance, find and replace, clipboard (closed specification)

**Status:** specified in cycle 24 (documents only) for three lead-programmer parts: part 1 (the field catalogue, the
property sheet and the property grid, D4), part 2 (find and replace with `fields.set_many`, D4) and part 3 (the
clipboard: project fragments, `edit.paste`, duplicate and cut, D5); part 1 after WP-S2.03 part 2's delivery, part 2
after part 1, part 3 after part 1 and WP-S2.04 part 1; Opus review of each ·
**Date:** 2026-10-23 (recorded 2026-10-24 by the cycle-24 design pass) · **Work package:**
[WP-S2.05](../02-work-packages.md#wp-s205-property-grid-provenance-find-and-replace-clipboard) · **Decision log:**
ID-591 to ID-600, the ten entries of §15.3 in order (follow-ups FT-393 to FT-403, the eleven of §15.4 in order; item
12 is a note in ID-593); §4.1 rule 2's precondition is met: `ci/dep-rules.toml` and architecture §3 carry the
`ew-api` edges to `ew-units` and `ew-mltext` as normal edges (ID-600); **part 1 delivered in cycle 25**, recorded as ID-626 with its twelve rulings and lane B's last fix, the integration rules ID-628 and the alias grammar ID-630; follow-ups FT-427 to FT-429; **part 2 delivered in cycle 26**, recorded as ID-634 with its twelve rulings, the `run_script` Bulk rule, the regex edge and the window check's focus fix, FT-427 closed, the integration ID-638 and the window checks ID-639; follow-ups FT-431 to FT-433 and FT-435; part 3 (the clipboard) next · **Part of:**
ElektraWatt plan, specifications

**Purpose.** This is the closed specification of the package that lets a user read and change every value of what
WP-S2.04's tools draw: a property grid generated from the record schemas, which shows each value with its origin
(authored, inherited, derived, overridden, frozen, library) and edits selections as single transactions; a project-wide
find and replace; and copy, cut, paste and duplicate through a text project fragment. It is written against **WP-S2.03
part 2's specified contract** and, for part 3, **WP-S2.04 part 1's specified contract** (§2.1), not against their code.
Where this document and a design document differ, this document is the narrower and wins for the lanes; the design
documents stay the target of the whole work package, and the documents named in §15.6 are updated at the design pass
after each part. A choice a lane meets that this document does not settle is escalated, never guessed (`CLAUDE.md`,
rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs](#2-inputs)
3. [The decisions of this specification](#3-the-decisions-of-this-specification)
4. [Crates, boundaries and the public items below the shell](#4-crates-boundaries-and-the-public-items-below-the-shell)
5. [Commands](#5-commands)
6. [Part 1: the field catalogue, the property sheet and the grid](#6-part-1-the-field-catalogue-the-property-sheet-and-the-grid)
7. [Part 2: find and replace](#7-part-2-find-and-replace)
8. [Part 3: the clipboard](#8-part-3-the-clipboard)
9. [Determinism, the headless mode and exact counts](#9-determinism-the-headless-mode-and-exact-counts)
10. [Acceptance checks](#10-acceptance-checks)
11. [D-06 and the gates](#11-d-06-and-the-gates)
12. [Files owned](#12-files-owned)
13. [Out of scope](#13-out-of-scope)
14. [Review checklist for the Opus reviewer](#14-review-checklist-for-the-opus-reviewer)
15. [Size, order, routing and the documents to update](#15-size-order-routing-and-the-documents-to-update)
16. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package a selection on the canvas (or a set of records named in a script) shows its fields
in a docked property grid; every field the registered commands expose is editable with a typed editor, every other
field is shown read-only with its reason; derived values carry their provenance with override, freeze and reset; a
multi-selection shows `‹varies›` and is edited in one command; find and replace searches the project and replaces
through one previewed command; copy, cut, paste and duplicate move records within and across projects and EW instances
as a text project fragment. Every edit is a registered command committed through the WP-S2.03 host path (preview,
then commit), every gesture runs headless through `ew.uiscript/1` and reads back through the transcript and
`ui.describe`, and the keyboard alone can do everything (NFR-A11Y-01). The proofs are AC-28's W29 step, D-06's
property-grid entry, FR-UI-009, FR-UI-010, FR-DEV-031, FR-UI-020, FR-DEV-029, FR-UI-036 and FR-UI-040 on Golden A (§10).

**Non-goals** (each is named with its package in §13):
- **No navigators, no table mode, no fill-down, pattern fill or Excel paste**: WP-S2.06 (ruling, delegated). The
  table-edit rule of FR-UI-044 is built here as the policy that the grid and find use; the tables that also use it
  come with WP-S2.06.
- **No message panel, no bulk preview tab, no history panel, no notifications log**: WP-S2.09 (ruling, delegated). A
  minimal inline preview card (§6.6) and a toast stand in until WP-S2.09 replaces them.
- **No "Why this value?" panel** (FR-UI-011, 1.0) and no multi-format clipboard (FR-UI-051, 1.0); no "Copy as image"
  and no "Paste as representation" (follow-ups, §15.4).
- No undo coalescing (WP-S2.04 D11; FT-372 unchanged): a field edit is one commit (D7).
- No change to any exported byte of Golden A, to AC-15's 609-command log, to the record logs, to the project format or
  to the acceptance result (88 pass); no new memoised family; no new package in `Cargo.lock`; the only new workspace
  edges are those of §4.1 rule 2.

## 2. Inputs

| Input | What the lanes take from it |
|---|---|
| [WP-S2.03's specification](WP-S2.03-viewer-and-tool-framework.md) §6.10, §7.2, §7.3, §7.6, §7.9, §8 | The contract of §2.1: the host's preview and commit path (D9), `Selection`, the transcript and describe rules, the `focus` member, the exact-count counters |
| [WP-S2.04's specification](WP-S2.04-placement-wiring-drawing-tools.md) §4.3, §5.2, §6, §7.3, §7.4, §12 | For part 3: the page tables, `Paths`, `Key` events, the grid-step translation of `rep.move` `steps`, `edit.delete_selection`; D11 (no coalescing); the shared append-only files |
| Code as built at the cycle-23 delivery | `crates/ew-commands/src/commands/{fields.rs,edit.rs,common.rs,library.rs,settings.rs,checks.rs}`, `src/command.rs`, `src/meta.rs`; `crates/ew-schema/src/{schema.rs,kind.rs,record.rs,value.rs}` and `schemas/0.1/*.schema.json`; `crates/ew-derive/src/{tag.rs,structure.rs,wire_props.rs,numbering.rs,xref.rs,title_block.rs,settings.rs}`; `crates/ew-api/src/{lib.rs,cmdlog.rs,output/navigation.rs}`; `crates/ew-checks/src/{scheme.rs,finding.rs}`; `crates/ew-import-tables/src/table.rs` (`COLOUR_ALIASES`); `crates/ew-viewmodel/src/{actions.rs,describe.rs,layout.rs,script.rs,shell.rs,strings.rs}`; `crates/ew-ui/src/{dock.rs,app.rs,keys.rs,headless.rs}` |
| [Architecture](../../design/04-architecture.md) §3 (rule 1), §5.1 to §5.5, §9 | `Session` as the only door; the transaction-read rule (ADR-0008 amendment 4, ID-525); preview as fork; macros as project fragments; the property-grid row |
| [ADR-0008](../../adr/0008-transactions-undo-preview.md) (amendments 2 to 4), [ADR-0004](../../adr/0004-identity-uuidv7-uuidv5.md), [ADR-0005](../../adr/0005-value-types-nm-dec3-mltext.md), [ADR-0011](../../adr/0011-format-versioning-and-migration.md) | Previews are the real command; empty transactions commit nothing; `HumanOnly` members keep their own command (amendment 3); minted IDs and library-copy IDs; integer value types; format versions |
| [UX/UI](../../design/16-ux-ui.md) §2.1 to §2.3, §4.4, §6, §7.2, §7.5, §10.1, §17 | `PropRow`, `EditTarget`, `Provenance`, `Selection`; keys; the grid's layout, rows, chips, typed editors and editing semantics; the table-edit rule; AccessKit rows; edge cases 1, 2, 5, 9, 11 |
| [Schematic editor](../../design/06-schematic-editor.md) §3.1, §3.6, §3.7, §3.9, §6, §8 items 3, 14, 15 | `fields.set`, the paste rules, multi-select, undo granularity, the keymap, the paste edge cases |
| [Domain model](../../design/03-domain-model.md) §1.1, §3, §5, §6, §7, §8, §9.5 | Kinds A, L, D, C, F; the value types; the records the grid edits; tags and frozen tags; the wire-property chain |
| [Extensibility](../../design/17-extensibility.md) §2.1, §2.3, §3.5, §11 | The command registry, exposure classes (`HumanOnly`, FR-EXT-030), the command log and its references, schema stability |
| [Quality](../../design/18-quality-security-release.md) §3.5, §3.11, §3.17 | Determinism, UI tests, what the blind drafter drives |
| [Golden project](../../design/20-golden-project.md) §5, §6, §8, §18.1, §19, AC-28, and `golden/J-E-2416/tables/` | W29, -101F1, -102F1, S51 and the frozen tags of EF-15, -X1031's terminals, the "Spare" valves of 204 to 206, folio 101's description |
| Requirements | The 9 rows of the work package: FR-UI-009, FR-UI-010, FR-UI-020, FR-UI-036, FR-UI-040, FR-UI-044, FR-DEV-029, FR-DEV-031, FR-DEV-032; with FR-UI-007, FR-UI-008, FR-UI-011, FR-UI-022, FR-UI-025, FR-UI-051, FR-CON-017, FR-CON-019, FR-CON-025, FR-REP-036, FR-I18N-001, FR-I18N-011, FR-NUM-016, FR-NUM-028, FR-SYM-016, FR-PRT-014, FR-EXT-030, FR-REV-009, NFR-A11Y-01 to NFR-A11Y-03, NFR-UX-07, NFR-REL-07, NFR-REL-08, NFR-PERF-20 where they touch it; AC-28 (the W29 step), D-06 (the property-grid entry, gated at G-M2, ID-422) |

The WBS lists WP-S2.03 as the only dependency. Part 1 needs only WP-S2.03 part 2; part 3 also needs WP-S2.04 part 1
(§15.2).

### 2.1 What the earlier packages provide (the base contract)

The lanes rely on exactly these specified items and on nothing of their code beyond them. If a delivered item differs,
the lane stops and reports it.

| Item | Where specified | Used here for |
|---|---|---|
| The host's preview and commit path: `Session::preview_with` with the GUI or command-log origin, `PreviewState { base_rev, command, name, result, derived }`, staleness, commit by `Session::commit` (window) or `Session::run_script` (headless), `CmdError::NeedsPreview` handling | S2.03 §7.3 (D9) | Every commit of this package (§6.6) |
| `Selection { items: Vec<EntityRef>, primary, generation }` per window, `EntityRef(Uuid)` | S2.03 §7.6; `ew-dl` | The grid's targets (§6.2) |
| The transcript's `state` members and the `focus` member (`"navigator"`, `"canvas"`, `null`); the describe rules (quoted user text, `VOCABULARY`, `FREE_TEXT_MARKERS`) | S2.03 §6.10, §7.9 | §6.11, §7.6, §8.8 |
| The exact-count counters on `Session` calls (WP-S2.02 A9's pattern) | S2.03 §8 item 3 | §9 item 4 |
| `ui.dismiss` and the Escape chain; registry contexts (`Global`, `Canvas`, `Navigator`) | S2.01, S2.03 §7.2 | §6.10 |
| **Part 3 only:** `Session::page_tables` with `TableRep.body` and `TableItem.bounds`; `OverlayItem::Paths` and `Ghost`; `CanvasEvent::Key`; the grid-step translation rule of `rep.move` `steps` (each grid coordinate moves by whole grid steps, any other coordinate by `round_half_away(steps × num / den)`); `edit.delete_selection` | S2.04 §4.3, §5.2, §6.1, §6.3, §7.4 | The fragment's ghost rectangles, the paste tool, the paste translation, cut |

### 2.2 Gaps found below the shell (why the parts are shaped as they are)

| Gap at the cycle-23 tip | Consequence |
|---|---|
| No query lists a record kind's fields with their value types; `fields.set`'s `SETTABLE` is a list of path strings | The field catalogue (D2, §6.1): types from `ew_schema::kind_schema`, edit routes from `SETTABLE` |
| No query gives a value together with its provenance in UI form; the pieces exist (`TagSource`, `SlotSource`, `PropSource`, the title block) | `Session::property_sheet` with a closed list of derived rows (D9, §6.4) |
| `fields.set` takes one value for all targets; find and replace and "freeze all" need one value per target, across kinds and paths | The new command `fields.set_many` (part 2, §5.1) |
| No find query; `regex` is a dependency of `ew-schema` only | `Session::find` (part 2) and the edge `ew-api → regex` |
| `edit.paste` is planned in doc 06 §3.1 but not built; no fragment format exists (the macro format of WP-S1.19 is not built either) | The fragment v1 text format and `edit.paste` (part 3, D12) |
| The workspace has no property pane; the clipboard is not used | `PaneKind::Properties` and `PaneKind::Find`; egui's text clipboard (`copy_text`, the paste event), already in the tree through `egui-winit` and `arboard 3.7.0` |

## 3. The decisions of this specification

Each is a lead ruling that the owner may reverse, except the four marked "ruling (delegated)", which the owner handed
to the lead for this specification and which are recorded as taken. The design pass allocates the decision-log entries.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Three parts.** Part 1 the field catalogue, the property sheet and the grid (2.3 pw, D4); part 2 find and replace with `fields.set_many` (0.8 pw, D4); part 3 the clipboard with `edit.paste` (1.5 pw, D5). Order: 1 → 2, and 1 + WP-S2.04 part 1 → 3; parts 2 and 3 may run in parallel (§15.2). The parts sum to the PERT of 4.6 pw with **no reserve** (§15.1) | Each part is one lane-cycle; part 3 alone changes no format but defines an interchange format and parses untrusted input, hence D5; the grid carries the gate evidence (AC-28, D-06) and goes first |
| D2 | **Ruling (delegated): the grid is schema-driven from `ew-schema`; there are no hand-written per-kind panels.** One walker turns `ew_schema::kind_schema(kind)` into the field tree of every implemented kind (§6.1); one generic panel draws every kind. What is not in the schemas is data, not panel code: the category of a top-level member, the label words, the closed derived-row table (§6.4) and the closed table of members other commands own (§6.1.3), each pinned by a committed expected file | A new record member appears in the grid without UI work; the committed schemas of `schemas/0.1` are already reviewed and tested, so the grid's types are the format's types |
| D3 | **Ruling (delegated): the grid edits only what commands expose.** A field is editable exactly when `fields.set`'s `SETTABLE` lists it or an ancestor of it, or it is one of the four members of `project.settings.set`; per-target values go through `fields.set_many` (part 2), whose entries obey the same `SETTABLE`. Every other field is read-only with its reason (§6.8). No command gains a settable path in this package | One mutation path; the grid cannot do what a script cannot; widening what is editable is a command change with its own review (follow-ups in §15.4) |
| D4 | **Ruling (delegated): the property grid stays read-only for `HumanOnly` members; the confirmed step is separate.** A field whose only route is a `HumanOnly` command (today `project.checks.set`, which writes the member `settings.checks`, doc 03's `settings.extra.checks`) is shown with the reason and the place of the confirmed step (the company-standard wizard's check-scheme step, `wizard.apply_check_scheme`); the grid never builds a command whose exposure is not `Edit` (`reason-human-only`, tested) | ADR-0008 amendment 3 and FR-EXT-030: a member that decides check outcomes never shares an edit path; the wizard's confirmed step (ID-546) stays the only GUI door |
| D5 | **Ruling (delegated): WP-S2.06 and WP-S2.09 stay out.** No navigator, table, message panel, bulk preview tab, history panel or notifications log. Two small stand-ins live in `ew-viewmodel` until WP-S2.09: the **inline preview card** (§6.6) and the **toast** | Keeps the package inside its PERT and M-PP; WP-S2.09's preview tab replaces the card, its notifications log keeps the toast's action |
| D6 | **One commit path: preview first, then commit or the card.** Every grid, find and replace commit is `Preview(cmd)` through the host (S2.03 §7.3); the edit policy of FR-UI-044 (§6.6) is evaluated on the preview: a direct edit commits the kept preview (window: `Session::commit`; headless: `Session::run_script`), any other shows the card and commits on Apply. Nothing writes the model otherwise | Preview + commit = execute (NFR-REL-08); the policy can use the derived diff (tags changed); the same path headless and in the window |
| D7 | **Undo granularity: one commit per field edit, no coalescing.** Enter, Tab, focus loss, a choice from a list, a checkbox click or a row action ends one edit and commits once; keystrokes in the field change only the edit buffer. FR-UI-009's "keystrokes coalesce until focus leaves" is met by deferring the commit, not by merging history steps | WP-S2.04 D11's ruling and `run_script`'s missing coalescing key (FT-372) stay intact; equal undo stacks in both front ends |
| D8 | **Sections by record kind; a mixed selection shows only the sections every selected entity reaches.** A representation reaches its representation, its functions and their devices; a function its device (§6.2). Targets of a section are deduplicated | Doc 16 §17 item 5 made precise: two representations of one device edit the device once; a representation and a connection share no section, so the grid says so instead of guessing a common field |
| D9 | **Derived rows are a closed list** (§6.4): the device tag, the device's resolved structure and main location, a connection's seven effective wire properties, wire number and potential, a page's title-block description, a representation's cross-reference text. Each pairs with its authored partner (override, freeze, reset). The chip names the source that supplied the value; "derived would be" is shown only where it is a stored value (the project description), not for wire overrides or frozen tags (follow-up) | Uses only queries that exist (`device_tag`, `structure_of`, `rep_anchor_cell`, `wire_props_of`, `wire_numbers`, `potential_partition`, `title_block`, `xref_text`); no change to `ew-derive`'s derivation |
| D10 | **Multi-selection semantics.** Values are compared as canonical JSON; differing values show `‹varies›` with their count; an edit sets every target of the section in one `fields.set`; a leaf below a settable member commits that member's whole new value and is editable only when that member is equal on every target; "freeze" on several devices needs one value per device and is `fields.set_many` (part 2) | FR-DEV-031 as one command; no partial or per-target merging that a script could not reproduce |
| D11 | **Find runs in the `Session`, values are computed by the caller, `fields.set_many` commits them, always previewed.** Library-copy fields and derived tags are found but not replaced (reason shown); regular expressions use the `regex` crate already in the tree | The transaction-read rule (ID-525): the command receives explicit values and never searches; the command log replays exactly without a regex engine |
| D12 | **The clipboard is a text project fragment, `#ew-fragment v1`; `edit.paste` takes the whole fragment as its parameters.** Strict parsing with limits; refusal of another connection grid and of another version of a held library copy; structure nodes rebound, never created; frozen tags, counters and wire numbers cleared so the normalisers number the copies; IDs minted in (kind rank, source ID) order; translation in grid steps (§8) | Replayable from the command log; deterministic; no new crate (egui's clipboard); the refusals are the conservative half of doc 06 §3.6 and §8 item 3, with the rest as follow-ups |
| D13 | **The headless mode and `ui.describe` grow by the members and words of §6.11, §7.6 and §8.8** (ID-474 extended; a reviewed transcript change, as ID-511 and ID-537 were), plus `select.set` (records by ID or by `tag:`, `folio:`, `id:`, `section:` reference) and the headless clipboard | The blind drafter edits and reads back without a screenshot; a script can select what the canvas cannot show (a device, a potential definition) |
| D14 | **No new memoised family, no format change, no golden change.** The sheet, find and copy are plain functions over `Q`; AC-16's memo-sweep line does not change | Keeps the ID-570 re-bless rule out of this package |
| D15 | **The chip names the profile that supplied the value.** W29's cross-section reads `0.75 mm²` with `derived · potential 24VDC-1 · size profile '0.75/1.5' · class control`, its colour `DBU` with `derived · potential 24VDC-1 · colour profile 'DBU/WH 24 V'` (Golden A's `profiles.csv`). AC-28, FR-UI-010, FR-CON-017 and doc 16 §6.2 name the colour profile on the cross-section; the design pass amends their example text | The grid shows what the derivation did (`PropSource::SizeRow` and `ColourRow`), never a summary that names the wrong source |
| D16 | **Golden data that the requirement texts misstate.** FR-UI-020's acceptance ("16 results" on 204 to 206): Golden A has 20 devices with the function text "Spare" on 204 to 206 (-201U5 to -201U8 and -202U2, Q1 to Q4 each), 16 on 204 and 205; the oracle uses both scopes and the design pass corrects the text. FR-DEV-032's example ("Murr Elektronik") replaces a manufacturer, a field of a library part copy (every part is one): found but not replaced here (D11), proposed for the part editor (§15.4). The amended example the design pass writes into FR-DEV-032 replaces an **authored device field**: a function text or an `Ident` tech-data value (FR3, FR4) | Lanes test against the data; a requirement example that the data or the decided rules do not support is a document fix, not a lane guess |

## 4. Crates, boundaries and the public items below the shell

### 4.1 Roles

| Crate | Layer | Part 1 | Part 2 | Part 3 |
|---|---|---|---|---|
| `ew-viewmodel` | GPL | `properties.rs` (the panel model, edit buffer, policy, card, toast), `select.set`, layout pane, actions, transcript, describe | `find.rs` | `clipboard.rs`, `tools/paste.rs`, duplicate, cut |
| `ew-api` | GPL | `output/fields.rs` (catalogue, parse, format, choices), `output/properties.rs` (the sheet), `resolve_reference`, `command_meta` and the `CommandMeta`/`Cost`/`Exposure` re-exports | `output/find.rs` | `output/clipboard.rs` (copy, tag list, parse) |
| `ew-commands` | GPL | — | `fields.set_many` | `edit.paste` (`commands/paste.rs`) |
| `ew-derive` | GPL | `settings::company_codes` (pure, beside `check_member`) | — | — |
| `ew-import-tables` | GPL | `COLOUR_ALIASES` made `pub` (visibility only) | — | — |
| `ew-ui` | GPL | the properties pane, the card, the toast, keys | the find pane | clipboard glue (egui `copy_text`, the paste event) |
| `ew-testkit` | GPL | — | DSL verb and fuzzer arm of `fields.set_many` | same for `edit.paste` |
| `elektrawatt` | GPL | tests only | tests only | tests only |

**Rules the lanes keep** (WP-S2.01's to WP-S2.04's rules stand: through `Session` only, `check-raw-read` green; no
`unsafe`; no unordered output; one clock; `ew-render2d` names no egui type):
1. **Only the public items of §4.2 and the commands of §5** are added below the shell, each first and in its own commit
   with tests. Any other missing item stops the lane.
2. **No new package** in `Cargo.lock` (`cargo tree -p elektrawatt` package count equal before and after, stated in the
   commit message). **New workspace edges, exactly:** part 1 `ew-api → ew-units` and `ew-api → ew-mltext` (path
   crates); part 2 `ew-api → regex` at the version `ew-schema` uses, `default-features = false`, features `std`,
   `unicode-case`, `unicode-perl`. `cargo xtask check-deps` green; nothing else. **Precondition (design pass, before
   part 1 starts):** `ci/dep-rules.toml` allows `ew-api → ew-units` and `ew-api → ew-mltext` today only as dev edges,
   and `ci/**` is read-only for lanes, so the design pass amends the `"ew-api"` entry of `ci/dep-rules.toml` (both as
   normal edges) and architecture §3 (the `ew-api` row and the crate graph) first; a lane that finds them unamended
   stops. *Met by the cycle-24 design pass (ID-600).* The `regex` edge is an external crate already vetted and needs no owner rule; feature unification turns on
   `unicode-case` and `unicode-perl` in `ew-schema`'s `regex` too, which is harmless (more Unicode tables, no
   behaviour change for its patterns). *Amended in cycle 26 (as built; decision log ID-634):* the premise "no new
   package" held for `Cargo.lock` but not for the shipped graph: `ew-schema` uses `regex` as a dev-dependency only,
   so the runtime edge brings `regex`, `regex-automata` and `regex-syntax` into `cargo tree -p elektrawatt` (288 to
   291 packages); accepted, with their cargo-vet criteria raised to `safe-to-deploy` (doc 18 §5.4; FT-435).
3. **One mutation path.** Every state change of the shell is a registry action on `ShellModel::dispatch`; every model
   change is a command committed by the host path of S2.03 §7.3 (D6). The view model compiles each edit into exactly
   one command (doc 16 §2.1); `ew-ui` never builds a command.
4. **No transaction reads `Q`.** `fields.set_many` and `edit.paste` read rows through `TxBuilder` only (ADR-0008
   amendment 4); everything derived (find hits, fragment ghosts, sheet values) is computed outside transactions.
5. **Golden A's outputs do not change**; every new optional member is `skip_serializing_if` absent; the existing golden
   output, acceptance and AC-15 tests pass unmodified; no file under `golden/` changes.
6. **No `ew-model` name in `ew-viewmodel` or `ew-ui`**; value parsing and formatting live in `ew-api` (§4.2).

### 4.2 The public items added below the shell

| Part | Crate | Item | Contract |
|---|---|---|---|
| 1 | `ew-api` | `pub enum FieldType { Text, Ident, MlText, Bool, Int { min: Option<i64>, max: Option<i64> }, Dec3, Quantity, Csa, Nm, Colour, Enum { options: Vec<String> }, Link { kind: String }, Slot, Uuid, Optional(Box<FieldType>), List(Box<FieldType>), Group(Vec<FieldNode>), Json }`; `pub struct FieldNode { pub name: String, pub ty: FieldType }` | The value types the editors know (§6.5). Built by the walker of §6.1.1 only |
| 1 | `ew-api` | `pub enum Category { Identity, Part, Electrical, ConnectionPoints, Texts, Structure, Placement, Terminal, Cable, Plc, Assembly, Other, Extra }` (display order as declared); `pub enum EditRoute { FieldsSet { kind: String, path: String }, SettingsMember { member: String }, ReadOnly(ReadOnlyReason) }`; `pub enum ReadOnlyReason { Identity, Narrowed { why: String }, Command { command: String, action: Option<String> }, HumanOnly { command: String, step: String }, NoCommand }`; `pub struct FieldSpec { pub kind: String, pub path: String, pub ty: FieldType, pub category: Category, pub label_key: String, pub route: EditRoute, pub identifier: bool }`; `pub fn field_catalogue_for(registry: &CommandRegistry, kind: &str) -> Option<std::sync::Arc<[FieldSpec]>>`; `pub fn field_catalogue(kind: &str) -> Option<std::sync::Arc<[FieldSpec]>>` | One entry per node of the kind's field tree (list items as `[]`, so `parts[].qty`), in tree order (§6.1); `None` for a reserved, library or unknown kind. `field_catalogue_for` computes it for the given registry (the `HumanOnly` rule of §6.1.3 reads its `CommandMeta`); `field_catalogue` is it over the built-in registry, once per kind per process (a `OnceLock` table). `identifier` marks the outgoing identifiers of FR-UI-044 rule 2 (§6.6). `label_key` is `field-<kind>-<path>` with `.` and `[]` replaced by `-` |
| 1 | `ew-api` | `pub struct PropertySheet { pub rev: u64, pub header: SheetHeader, pub sections: Vec<SheetSection>, pub missing: usize, pub read_only: Option<String>, pub languages: Vec<String> }`; `pub struct SheetHeader { pub title: String, pub items: usize, pub kinds: Vec<(String, usize)>, pub subtitle: Option<String>, pub location: Option<String>, pub findings: Vec<FindingChip> }`; `pub struct FindingChip { pub rule: String, pub severity: String, pub message: String }`; `pub struct SheetSection { pub kind: String, pub targets: Vec<Uuid>, pub rows: Vec<PropRow> }`; `pub struct PropRow { pub key: String, pub path: String, pub label_key: String, pub category: Category, pub depth: u8, pub ty: FieldType, pub value: RowValue, pub provenance: Provenance, pub actions: Vec<RowAction>, pub route: EditRoute, pub identifier: bool }`; `pub enum RowValue { One { json: serde_json::Value, text: String }, Varies { count: usize, texts: Vec<String> }, Absent }`; `pub enum Provenance { Authored, Inherited { from: String }, Derived { via: String }, Overridden { note: String }, Frozen { note: String }, Library { item: String }, Generated { block: String }, Varies { count: usize } }`; `pub enum RowAction { Edit, Clear, Override, Reset, Freeze, Unfreeze }`; `pub fn Session::property_sheet(&self, ids: &[Uuid]) -> PropertySheet` | §6.2 to §6.4 and §6.8. `ids` in any order; every UUID resolved to its record kind in the current snapshot (IDs that name no record of a sheet kind, unknown IDs and library copies, counted in `missing`). Sections in the order device, function, representation, connection, junction, page, then the other kinds by kind rank; `targets` in label order (as built by FT-439, decision log ID-640: the printed label, then the place, the ID only as the last key); rows in catalogue order within the categories, derived rows first in their category (§6.4). `key` is `<kind>/<row path>` with list indices (`device/parts[0].qty`) or `<kind>/derived.<name>`. `languages` are the project languages, source first (`ProjectLanguages`). Deterministic; reads memoised queries only; no new memo family |
| 1 | `ew-api` | `pub struct ParsedInput { pub value: serde_json::Value, pub notes: Vec<String> }`; `pub enum InputError { Invalid(String), Ambiguous { choices: Vec<(String, serde_json::Value)> } }`; `pub fn Session::parse_field_input(&self, kind: &str, row_path: &str, text: &str) -> Result<ParsedInput, InputError>`; `pub struct FieldChoice { pub text: String, pub value: serde_json::Value, pub note: Option<String> }`; `pub fn Session::field_choices(&self, kind: &str, row_path: &str, filter: &str) -> Vec<FieldChoice>`; `pub fn format_field_value(ty: &FieldType, value: &serde_json::Value) -> String` | The parse and display rules of §6.5, the only place they live; `field_choices` lists the options of an `Enum`, `Colour`, `Link` or `Slot` field matching the case-folded `filter`, in the order of §6.5; empty for other types |
| 1 | `ew-api` | `pub use ew_commands::{CommandMeta, Cost, Exposure}` (re-exports beside the existing `CommandRegistry`); `pub fn Session::command_meta(&self, name: &str) -> Option<CommandMeta>` | The view model reads a command's exposure and cost through these (D4's guard, rule 3 of the policy, §6.6); `None` for an unregistered name |
| 1 | `ew-api` | `pub fn Session::resolve_reference(&self, text: &str) -> Result<Uuid, String>` | Exactly the command-log resolver of `cmdlog.rs` (`id:`, `section:`, `tag:`, `folio:`, same messages), factored into one function that `run_script` and this method share |
| 1 | `ew-derive` | `pub fn settings::company_codes(colour_profile: &serde_json::Value) -> Vec<(String, String)>` | The company codes `(code, base IEC code)` that `check_member` accepts for that `colour_profile` value, in the profile's order; empty for a value it would refuse |
| 1 | `ew-import-tables` | `pub const COLOUR_ALIASES` | Visibility only; the colour editor's suggestion list (§6.5) |
| 2 | `ew-api` | `pub struct FindQuery { pub text: String, pub regex: bool, pub match_case: bool, pub whole_word: bool, pub fields: Vec<FindField>, pub folios: Vec<String>, pub within: Vec<Uuid>, pub replace: Option<String> }`; `pub enum FindField { FunctionText, Tag, Property, Text, PartNumber }`; `pub struct FindHit { pub record: Uuid, pub kind: String, pub row_path: String, pub label_key: String, pub subject: String, pub folio: Option<String>, pub cell: Option<String>, pub before: String, pub ranges: Vec<(usize, usize)>, pub after: Option<String>, pub replaceable: Result<(), String> }`; `pub struct FindResults { pub hits: Vec<FindHit>, pub truncated: bool, pub error: Option<String> }`; `pub fn Session::find(&self, q: &FindQuery) -> FindResults`; `impl FindResults { pub fn replace_command(&self) -> Option<serde_json::Value> }` | §7.1 and §7.4 |
| 3 | `ew-api` | `pub struct CopiedFragment { pub text: String, pub records: usize, pub copies: usize, pub dropped_connections: usize, pub skipped: Vec<(String, usize)> }`; `pub enum CopyError { Empty, NotOnePage, Generated }`; `pub fn Session::copy_fragment(&self, ids: &[Uuid]) -> Result<CopiedFragment, CopyError>`; `pub struct ParsedFragment { pub params: serde_json::Value, pub records: usize, pub grid: (i64, i64), pub ghost: Vec<[i64; 4]> }`; `pub enum FragmentError { NotAFragment, Newer { version: String }, TooLarge(String), Invalid { line: usize, message: String } }`; `pub fn parse_fragment(text: &str) -> Result<ParsedFragment, FragmentError>`; `pub fn Session::tag_list(&self, ids: &[Uuid]) -> String` | §8.1 to §8.3 and §8.6. `params` is the `fragment` member of `edit.paste` |

No other item below the shell is added.

## 5. Commands

### 5.1 New commands

Both are exposure `Edit`, stability `Preview`, undoable as one step, `params_version` 1, `deny_unknown_fields`; each
lane delivers, in its command commit, the `Command` with its doc comment, registration in `commands/mod.rs`, the
committed schema under `crates/ew-commands/schemas/` (regenerated by that crate's `schemas` test), the name in
`tests/params.rs`, the generated reference page and its `SUMMARY.md` line (`cargo xtask docs-gen`, then `--check`
green), a **DSL verb** in `crates/ew-testkit/src/golden/dsl.rs`, a **fuzzer arm** in
`crates/ew-testkit/tests/command_fuzz/arms.rs` (valid and boundary-violating parameters) and unit tests for every
rejection. Each adds one to the registered count at its integration (42 at the cycle-23 tip; WP-S2.04 adds 8 as its
parts land); the lane states the resulting count.

| Command (part) | Parameters | Semantics | Rejected |
|---|---|---|---|
| `fields.set_many` (2; FR-DEV-032, FR-UI-020) | `edits: [{kind, targets: [IdS], path, value}]` (1 to 10,000 entries) | Applies each entry exactly as `fields.set` with those parameters would, in the given order, in one transaction (same `SETTABLE`, the same per-kind checks: closed revisions, aspects, assembly covers, the tag-collision refusal). Cost: `Bulk` when the total number of targets over all entries exceeds 500 (`edit.rs`'s `BULK_ABOVE`), else Interactive. Outcome: none | No entry; an entry rejected as `fields.set` would reject it (the pointer prefixed by `/edits/<i>`); one (target, path) pair in two entries (at `/edits/<j>/targets/<k>` of the later); more than 10,000 entries |
| `edit.paste` (3; FR-DEV-029, FR-UI-036, FR-UI-040) | `page` (IdS), `at` (Pt2: the new position of the fragment's anchor), `fragment` (`Fragment`, §8.1) | §8.4: validation, library copies, structure rebinding, new IDs, translation in grid steps, numbering reset, all in one transaction; the normalisers then number devices and wires. Cost: `Bulk` when the fragment holds more than 500 records (doc 06 §8 item 14), else Interactive. Outcome: the created representations; notices for every cleared link, dropped wire marker and rebound structure | §8.4's list, each at its JSON Pointer |

### 5.2 Commands used as built

`fields.set`, `project.settings.set`, `edit.delete` and `library.embed` (its `check_copy` rules, reused by `edit.paste`
through the shared function) are used as built. No command is amended.

## 6. Part 1: the field catalogue, the property sheet and the grid

### 6.1 The field catalogue

#### 6.1.1 The walker (D2)

`field_catalogue(kind)` walks `ew_schema::kind_schema(kind)` from its `properties`, depth first, members in the order the
schema value lists them. A node's type:

| Schema node | `FieldType` |
|---|---|
| `$ref` to `MlText` | `MlText` |
| `$ref` to `Ident` | `Ident` |
| `$ref` to `Dec3` | `Dec3` |
| `$ref` to `Quantity` | `Quantity` |
| `$ref` to `Csa` | `Csa` |
| `$ref` to `ColourSpec` | `Colour` |
| `$ref` to `Nm` | `Nm` |
| `$ref` to `PartRef` | `Link { kind: "part" }` (the value is the whole object; picking a copy writes `{"copy": id}`) |
| `$ref` to `Slot` | `Slot` |
| `$ref` to `Uuid` | `Uuid` (read-only display) |
| `oneOf` whose branches are all `{"const": <string>}` | `Enum { options }` in branch order |
| `anyOf [X, {"type": "null"}]`, or `"type": [T, "null"]` | `Optional(X)` |
| `"type": "boolean"` | `Bool` |
| `"type": "integer"` | `Int` with `minimum` and `maximum` |
| `"type": "string"` without `$ref` | `Text` |
| `"type": "array"` | `List(items)` |
| `"type": "object"` with `properties` (or a `$ref` to such a definition not named above) | `Group(children)` |
| anything else (mixed `oneOf` such as `NumberSpec`, `TechVal`, maps such as `extra`) | `Json` |

`$ref` is resolved within the schema's `$defs`; a reference cycle is cut at depth 8 as `Json`. **Unlisted members.**
Every JSON object of the format carries its `extra` map flattened (doc 05 §3.1), so a record can hold members its
schema does not list (the project's `settings.numbering`, `settings.checks`, which doc 03 calls
`settings.extra.checks`, and the project-row member `ew.company`). They are not catalogue entries; the sheet adds them
from the values as rows of type `Json`, with the route §6.1.3 gives their path, in byte order of their names after the
listed members of the same object. **Rows that exist even when absent:** the project section always has the rows
`settings.numbering`, `settings.colour_profile`, `settings.size_profile`, `settings.xref_format` (editable, §6.1.3
step 4) and `settings.checks` (read-only, `HumanOnly`), with `RowValue::Absent` when the member is not stored, so a new
wizard project can set its size profile from the grid; the catalogue lists these five as fixed `Json` entries of
`project`. A member name that contains a dot is one path segment, written in brackets in row
paths (`project/[ew.company]`).

#### 6.1.2 Categories and labels

The category of a node is that of its top-level member, by this table (a top-level member the table does not list:
`Other`; an unlisted member whose top-level member the schema does not list either: `Extra`):

| Category | Top-level members |
|---|---|
| Identity | `id`, `$v`, `tag`, `kind`, `state`, `folio`, `number`, `code`, `label`, `name`, `order` |
| Part | `parts`, `tech_data` |
| Electrical | `overrides`, `ptype`, `voltage`, `env`, `circuit_class`, `props`, `core`, `colour`, `csa`, `shield`, `pair` |
| ConnectionPoints | `cp_overrides` |
| Texts | `function_text`, `process_tag`, `title`, `description_override`, `description`, `text_slots`, `aliases`, `caption` |
| Structure | `structure`, `parent`, `position`, `assigns`, `external` |
| Placement | `page`, `pos`, `orient`, `arrangement`, `grid`, `scale`, `form`, `outline`, `style`, `layer`, `at` |
| Terminal | `strip`, `terminal` |
| Cable | `cable` |
| Plc | `plc_station`, `plc_module` |
| Assembly | `assembly` |

The English label is `strings::field_label(kind, path)`: an exact entry of a table in `ew_viewmodel::strings` when one
exists (the lane writes entries only for the rows of §6.12), else the last path segment humanised (underscores to
spaces, first letter capitalised) with the word list `csa` → "cross-section", `seq` → "sequence", `qty` → "quantity",
`id` → "ID", `plc` → "PLC", `ip` → "IP", `cp` → "connection point". Collapse states persist per (kind, category) in the
settings file (`properties.collapsed`).

#### 6.1.3 Edit routes

For a node at path `P` of kind `K`, in this order:
1. `id`, `$v`: `ReadOnly(Identity)`.
2. **The narrowing table** (it may make an editable path read-only, never the reverse): `revision` `author` and
   `checker` → `ReadOnly(Narrowed { why: "decides the four-eyes sign-off (FR-REV-007, D-34)" })`. `fields.set` exposes
   both, but they decide FR-REV-007's author-equals-checker block, so the grid shows them read-only (ruling,
   delegated, on the review's `HumanOnly` audit); moving them to a `HumanOnly` sign-off command is a follow-up (§15.4).
3. **`FieldsSet { kind: K, path: S }`** where `S` is the longest prefix of `P` (by segments, `[]` removed) that
   `SETTABLE` lists for `K`. A node with no such prefix but with a settable descendant is a group header (no editor).
4. For `project`: `settings.numbering`, `settings.colour_profile`, `settings.size_profile`, `settings.xref_format`:
   **`SettingsMember { member }`** (`project.settings.set`, which is `Bulk`: always the card, §6.6).
5. The closed table of members other commands own (paths are JSON member paths, the flattened `extra` included):

| Kind and path | Reason |
|---|---|
| `project` `settings.checks` | `HumanOnly { command: "project.checks.set", step: "the company-standard wizard's check-scheme step" }` (D4) |
| `project` `[ew.company]` (`COMPANY_MEMBER`); `settings.frames`; `languages` and `settings.formatting` | `Command { command: "project.company.set"`, `"project.frames.set"`, `"project.languages.set"` respectively, `action: "the company-standard wizard" }` |
| `project` `settings.drawing` | `Command { command: "project.drawing.set", action: "Project ▸ Drawing" }` (once WP-S2.04 part 2 registers it; before, `NoCommand`) |
| `representation` `pos`, `orient`, `page`; `page_item` and `structure_box` geometry | `Command { command: "rep.move" / "rep.transform", action: "the canvas" }` |
| `connection` `a`, `b`, `route`; `junction` every member | `Command { command: "connection.draw" / "connection.route" / "junction.edit", action: "the canvas" }` |
| `page` `folio` | `Command { command: "page.renumber", action: None }` |
| `device` `parent`, `position` | `Command { command: "assembly.attach", action: None }` |
| `project` `supply.origin.source` | `Command { command: "project.supply.set", action: None }` |
| `representation` `anchor` | `Command { command: "rep.set_anchor_policy", action: None }` |

6. Anything else: `ReadOnly(NoCommand)`.

A route whose command's `CommandMeta` exposure is `HumanOnly` is always `ReadOnly(HumanOnly)`, whatever the table
says (computed from the registry, so a future `HumanOnly` command cannot leak into the grid). The catalogue is
therefore a function of a registry: `field_catalogue_for(registry, kind)` computes it for any `CommandRegistry` (PG1's
test registry), and `field_catalogue(kind)` is that function over the built-in registry, kept in a `OnceLock`.

**`identifier`** is set for `device` `tag.letter`, `tag.counter`, `tag.frozen`, `tag.scheme`, `terminal.designation`,
`terminal.seq`; `connection` `number`; `device` `plc_module.start`, `plc_module.slot` (FR-UI-044 rule 2's outgoing
identifiers; folios are not settable here).

**The expected catalogue.** `crates/ew-api/tests/expected/field_catalogue.txt` holds, for every implemented kind, one
line per node: `<kind> <path> <type> <category> <route>`. A test regenerates it and fails on a difference; the lane
commits it, and the reviewer reads the list of settable paths that fall back to `Json` (expected today:
`connection number`, `device tech_data[].value`, `device assembly`, `device plc_station`, `device plc_module`,
`function cp_overrides`, `representation text_slots`, `report_block` members whose types are mixed, the four project
settings members). A test also asserts that every path of `SETTABLE` maps to a node of the catalogue.

*As built (cycle 25; decision log ID-626).* The list above was a prediction; the expected catalogue file is the record. At the integration tip it has 19 kinds and 406 nodes, 167 editable, and the editable `Json` fallbacks are section `folio_range`, page `scale`, device `cable.length`, `plc_module.slot`, `plc_module.start.input` and `plc_module.start.output`, `tag.scheme` and `tech_data[].value`, representation `text_slots[].pos`, connection `kind` and `number`, report block `empty`, and the four project settings members (`settings.numbering`, `settings.colour_profile`, `settings.size_profile`, `settings.xref_format`). Typed editors for them are FT-429. Since WP-S2.04 part 1 registered `project.drawing.set`, step 5's `settings.drawing` row has the `Command` route (ID-625).

### 6.2 Targets, sections and the empty selection

`Session::property_sheet(ids)` resolves each ID to its record; the view model passes the window's `Selection.items`,
or, when the selection is empty, the focused canvas page's record (a generated page: its report block, all rows
read-only with "generated by block '<title>'"); `props.project` passes the project record's ID.

| Selected record | Sections it reaches |
|---|---|
| Representation | representation; function (each function it draws); device (each device of those functions) |
| Function | function; device (when it has one) |
| Any other record of an implemented project kind (device, connection, junction, page, project, PLC configuration project, import lineage, structure node, section, external reference, conductor, channel, potential definition, bond, device link, report block, revision; page item and structure box once implemented) | its own kind |
| A library copy (part, symbol, FnDef, form) or an unknown ID | none; counted in `missing` |

A section is shown when **every** selected record reaches it (D8); its targets are the union of the records reached,
deduplicated and sorted. When no section is left, the sheet has none and the grid shows "No common fields: the
selection holds <kinds>". A selected ID that no longer exists counts in `missing`, and with nothing else left the grid
shows "Object no longer exists" (doc 16 §17 item 1) with `ui.dismiss` back to the canvas.

**Header.** `title`: for one record its derived tag (device, function and representation sections: the device's
displayed tag on its main page), wire number (connection), folio and title (page), name or code (potential definition,
structure node), else the kind's label; for several `"<n> <kind plural>"` of the first section. `subtitle`: the
device's function text in the source language. `location`: the main representation's page and anchor cell
(`rep_anchor_cell`, `102-B6`). `findings`: the open findings of `ew_checks::scheme("drafting")` in
`CheckMode::Online` (`Session::check`) whose targets include a sheet record or, for a page section, whose page is that
page; in rule order, then by subject.

### 6.3 Rows and values

For each section, every catalogue node of the kind gives a row, with these exceptions: a node below a device aspect
(`strip`, `terminal`, `cable`, `assembly`, `plc_station`, `plc_module`) only when every target has the aspect; list
items (`parts[0]`, `parts[1]`) for each index of the list when the list is equal on every target, none otherwise; a
`Group` that is `Optional` and absent on every target as one row with `RowValue::Absent`.

- **Value.** The member's JSON in each target's record (absent: the schema default, shown as `Absent`). Equal JSON
  (canonical, `serde_json` value equality) on every target: `One { json, text }` with `text =
  format_field_value(ty, json)`; else `Varies { count: distinct values, texts: up to 8 texts in byte order of their
  canonical JSON }` (as built by FT-439: in natural order of the texts, then their bytes, then the JSON, so a link's
  values list records by label, not by UUID).
- **Depth.** Group and list children are rows with `depth` = their nesting level; the panel indents them.
- **Provenance** of a plain row: `Authored`; a part reference: `Library { item: "<library_id>/<item_id> v<version>" }`
  from the embedded copy; rows of a generated page: `Generated`; `Varies` rows: `Varies { count }`. Derived rows: §6.4.
- **Actions.** An editable row: `Edit`, and `Clear` when the member is optional or a list (writes `null`). Derived rows:
  §6.4. A read-only row: none. A `Varies` row of a list or group ("set all", doc 16 §6.2): `Edit` copies the value of
  the first target (section order) into the editor, shows its children for that copy, and commits the result to every
  target.

### 6.4 Derived rows and provenance (D9, D15)

Derived rows come first in their category. "Partner" is the authored path the row's actions write, always through the
row's section targets.

| Section | Row (`key`) | Value | Provenance | Actions (partner) |
|---|---|---|---|---|
| device | `derived.tag` (Identity) | the tag as displayed on the device's main page (`device_tag`, `Tag::displayed`) | `TagSource::Scheme`: `Derived { via: "numbering scheme '<key>'" }`; `Child`: `Derived { via: "child of <parent tag>" }`; `Terminal`: `Derived { via: "terminal of strip <strip tag>" }`; `Frozen`: `Frozen { note: "frozen" }`, `"frozen (sign hidden)"` when `sign_on_pages` is false | `Freeze` (not frozen; one device only in part 1, all in part 2): `tag.frozen` ← `{"text": <product>, "sign": "shown"}`; `Unfreeze` (frozen): `tag.frozen` ← `null` |
| device | `derived.structure.<aspect>` per aspect with a node (Structure) | the node's path codes (`+J-E-2416`) from `structure_of` | `SlotSource::Explicit`: `Authored`; `MainRepPage`: `Inherited { from: "page <folio>" }`; `Parent`: `Inherited { from: "<parent or strip tag>" }` | `Override` (inherited): edits `structure.<aspect>` (Slot editor); `Reset` (explicit): `structure.<aspect>` ← `null` |
| device | `derived.main_location` (Placement) | `<folio>-<cell>` of the main representation | `Derived { via: "main representation" }` | — |
| connection | `derived.csa`, `derived.colour`, `derived.wire_part`, `derived.insulation`, `derived.function`, `derived.shielded`, `derived.twisted` (Electrical) | the effective value of the first physical wire the connection belongs to (`wire_props_of` of its potential, `by_connection`, ascending carrier ID; two wires with different values: `Varies`) | by `PropSource`: `Override(c)`: `Overridden { note: "override (CDP)" }`, or `"override on <wire number of c>"` when `c` is another constituent connection; `Conductor`: `Derived { via: "conductor <cable tag>:<core>" }`; `PotentialDef`: `Derived { via: "potential definition <name>" }`; `ColourRow`: `Derived { via: "potential <name> · colour profile '<profile name>'" }`; `SizeRow`: `Derived { via: "potential <name> · size profile '<profile name>' · class <class>" }`; `ProjectDefault`: `Derived { via: "project default" }`; `BuiltIn`: `Derived { via: "built-in default (<profile>)" }` | `Override` (not level 7): edits `overrides.csa`, `overrides.colour`, `overrides.wire_part`, `overrides.insulation`, `overrides.function`, or `overrides.flags` (the whole flags object, for shielded and twisted); `Reset` (level 7 on this connection): the same path ← `null` |
| connection | `derived.number` (Identity) | the printed wire number (`wire_numbers`; `?` when unassigned) | `NumberSpec::Frozen`: `Frozen { note: "frozen" }`; else `Derived { via: "wire numbering" }` | `Freeze`: `number` ← `{"frozen": <printed>}` (refused for `?`); `Unfreeze`: `number` ← `null` |
| connection | `derived.potential` (Electrical) | the potential's name (`potential_partition`) | `Derived { via: "potential definition at <cp label>" }` when a definition names it, else `Derived { via: "connectivity" }` | — |
| page | `derived.description` (Texts) | the description the title block prints on this page (`title_block`) | `description_override` absent: `Inherited { from: "project description" }`; set: `Overridden { note: "override · project description is '<text>'" }`, and the header's finding chip EW-CORE-023 | `Override`: edits `description_override` (MlText editor, starting from the project description); `Reset`: `description_override` ← `null` |
| representation | `derived.xref` (Texts) | the cross-reference text (`xref_text`), absent when empty | `Derived { via: "cross-reference format" }` | — |

The profile name is the `name` the profile declares (Golden A: `0.75/1.5`, `DBU/WH 24 V`), else its key. Over several
targets a derived row's value and provenance follow §6.3 (`Varies` when they differ); its actions apply to every
target (Freeze on several devices: part 2, §7.5). "Derived would be" for a wire override and "rule gives" for a frozen
tag are not shown (D9; follow-up).

### 6.5 Typed editors

The view model keeps an **edit buffer** (text) per editing row; `ew-ui` draws the editor of the row's type. Parsing is
`Session::parse_field_input`, display `format_field_value`; both are total over their type.

| Type | Editor | Accepted input → JSON | Display |
|---|---|---|---|
| `Text` | one line | any text without control characters | as stored |
| `Ident` | one line | NFC, trimmed; non-empty, one line (as `Ident::from_input`); confusables and mixed scripts become `notes` naming the code points ("Cyrillic А (U+0410)", `ew_mltext::ident_findings`; FR-I18N-011, FR-NUM-016), never a refusal here | as stored |
| `MlText` | one line for the neutral text, or for the first project language when the value holds languages; Shift+Enter opens the multi-language popup: a "Neutral" row and one row per project language, source first | a JSON string (neutral) or a language object; switching between neutral and languages asks first (FR-I18N-001: choices "Replace the neutral text" / "Keep it"), then writes the other form; an empty neutral text writes `null` | the neutral text, else the source language's, with ` [+<n>]` for the other languages |
| `Bool` | checkbox; text `true`, `false`, `yes`, `no`, `1`, `0` | JSON boolean | `yes` / `no` |
| `Int` | one line | decimal integer within `min` and `max` | decimal |
| `Dec3` | one line | `Dec3::parse_lenient` (comma or point); an input matching `^[0-9]{1,3}[.,][0-9]{3}$` is `Ambiguous` with the choices "<n> (decimal)" and "<n> (thousands)" (doc 16 §6.3) | canonical, point separator |
| `Quantity` | one line | a `Dec3` as above, white space, a unit: a persisted spelling of `Unit` or its symbol (`mm²`, `°C`) | `0.75 mm²`, `20 A` |
| `Csa` | one line | a `Dec3` with an optional `mm2` or `mm²`; `AWG <n>` or `<n> AWG`; `<Dec3> kcmil` | `0.75 mm²`, `AWG 18`, `250 kcmil` |
| `Nm` | one line, millimetres | a decimal of at most 6 fraction digits with an optional `mm`, converted exactly to nm | mm with 3 decimals, more when not exact |
| `Colour` | combo plus text | one or two codes separated by space, `/` or nothing (`GNYE`), each an IEC 60757 code or a company code of `company_codes(settings.colour_profile)`; an unknown code is `Invalid` naming the accepted codes, with the suggestion of `COLOUR_ALIASES` when the code is one (`BR` → "BN brown", D-17) | the codes joined (`GNYE`) |
| `Enum` | combo | one of `options` (case-insensitive) | the option |
| `Link { kind: "part" }` | searchable picker | a part copy by part number (unique, case-insensitive), or `id:<uuid>` | `<part number> (<manufacturer>)` |
| `Slot` | node picker per aspect | `none`, empty (inherit: `null`), or a node of the row's aspect by its printed code path or an alias, which adds the note "resolved through alias '<alias>'" (D-03) | the node's code path, `none`, or empty |
| `List` | child rows; `props.list` adds (the item type's default: empty text, `0`, `false`, absent link), removes, moves | the whole list | `<n> items`; a list of `PartUse` as `<part number> × <qty>` joined by `, ` |
| `Group` | child rows only | — | empty |
| `Uuid` | read-only | — | the referenced record's label when it resolves, else the UUID |
| `Json` | multi-line text | strict JSON (`ew_schema::parse_value`: no repeated members) | compact canonical JSON, cut at 120 characters in the row with the whole in the tooltip |

`field_choices` orders: `Enum` in option order; `Colour` the 14 IEC codes in IEC 60757 order, then company codes in
profile order; `Link` part copies by part number (natural order); `Slot` nodes of the aspect by code path (natural
order). A leaf below a settable member is parsed by its own type and composed into the member's whole value by the
view model (§6.6).

### 6.6 The commit path, the edit policy, the card and the toast (D6, D7, D10)

**Compiling an edit.** On `props.commit` (or a choice, a checkbox, a row action, `props.set`): the buffer is parsed;
`Invalid` puts the row in the error state (no command); `Ambiguous` opens the choices; otherwise the view model takes
the row's settable member (the route's path), replaces the edited leaf in its JSON (a JSON pointer set on a copy of the
`One` value) and builds exactly one command: `fields.set { kind, targets: section.targets, path, value }` for
`FieldsSet`, `project.settings.set { member, value }` for `SettingsMember`. A value equal to the current one builds no
command and closes the editor. A row action's command is the partner write of §6.4. The command's exposure is checked
through `Session::command_meta`: not `Edit` → refused `reason-human-only` (D4). The view model keeps, with the open
edit, the **base value**: the settable member's JSON on every target as the sheet showed it when the edit began.

**Preview, then policy.** The command goes to the host as `Preview(cmd)` (S2.03 §7.3, one `Session::preview_with`).
The **edit policy** (`ew_viewmodel::properties::policy`, a pure function, FR-UI-044) decides on the result:

| # | Condition (any one) | Outcome |
|---|---|---|
| 1 | the command has more than 500 targets in total | card, `reason-bulk` |
| 2 | the row is an `identifier` and the command has more than one target, or the preview's derived diff changes more than one tag | card, `reason-identifiers` |
| 3 | the command's cost is `Bulk` (`project.settings.set`) | card, `reason-bulk-command` |
| 4 | the preview failed | the row's error state with the message (no card) |
| — | otherwise | direct: the kept preview is committed at once (window `Session::commit`, headless `Session::run_script`) |

FR-UI-044's rules 3 (an invalid or ambiguous cell) and 4 (from a file) are the parse step and do not occur here.

**The inline preview card** (until WP-S2.09's tab): a strip at the top of the properties pane (and of the find pane in
part 2): `Preview: <command> on <n> <kind plural> · <changes> changes · <tags> tags, <xrefs> cross-reference texts`
with up to 20 lines of changed tags (`old -> new`) from the derived diff, and **Apply** (`preview.apply`, Ctrl+Enter) and
**Cancel** (`preview.cancel`, Escape). A card whose base revision is no longer current is **stale** (badge). **Apply on
a stale card never re-previews the old parameters**, which would overwrite what changed since: it fetches the sheet
again (one `property_sheet` call), and when the edited member's current JSON differs from the base value on any target
it refuses with `reason-changed-meanwhile` ("changed meanwhile: edit again"), closes the card and leaves the row with
the typed text in the error state; otherwise it composes the edit again on the current values, previews the rebuilt
command and shows a fresh card that needs a second Apply (doc 16 §7.6, §17 item 2). Apply on a fresh card commits
through the host path; Cancel drops the preview. One card at a time; a new edit replaces an open card (which is cancelled).

**The toast.** After a direct commit with more than one target: `Set <label> on <n> <kind plural>` with **Undo**
(`toast.undo`, the registry's undo) and a close button (`toast.dismiss`). In the window it disappears after 8 seconds
measured by the window (`ew_render2d::stats::now`, never by the model); headless it stays until dismissed or replaced by
the next toast or the next committing action, so transcripts stay deterministic. It never takes the focus (doc 16 §17
item 6).

**After a commit** the sheet is fetched again on the next frame (one call), the focused row is kept by `key`, and the
editor closes. Undo and redo restore the model; the grid follows the next sheet.

### 6.7 Validation and refusal display

- **Parse errors** (`InputError::Invalid`): under the field, the row in the error state, the typed text kept; Escape
  (`props.cancel`) reverts to the stored value.
- **Command refusals** (a failed preview or a refused commit): `CmdError::InvalidParams` shows its message; a pointer
  `/targets/<i>` is shown with the target's label ("on -101F1: …"); a prevent rule or structural invariant shows its ID
  and message (doc 16 §6.3). The model is unchanged, the buffer kept.
- **Notes** (`ParsedInput.notes`: confusables, alias resolution) show under the field after the commit, without
  blocking it.
- **Choices** (`Ambiguous`, the MlText switch): a list under the field; `props.choose` with the index commits the chosen
  value; Escape returns to the buffer.

### 6.8 Read-only rules

| Condition | Rows | Reason shown |
|---|---|---|
| The project is read-only (lock, read-only open, recovery) | every row; `PropertySheet.read_only` set | the shell's read-only reason (doc 16 §17 item 9) |
| A generated page or report row | every row of that section | "generated by block '<title>'" |
| A released or withdrawn revision | every row of that revision | "released in <label>; edits go to the open revision" (FR-REV-009) |
| `ReadOnly(Identity)` | that row | "identity" |
| `ReadOnly(Narrowed)` | that row | its `why` (revision author and checker: "decides the four-eyes sign-off") |
| `ReadOnly(Command)` | that row | "changed with <action>" or "changed by <command>" |
| `ReadOnly(HumanOnly)` | that row | "human only: decides check outcomes; change it in <step>" (D4) |
| `ReadOnly(NoCommand)` | that row | "no command edits this member yet" |

`props.set`, `props.edit` and the row actions on a read-only row are refused with `reason-read-only` (or
`reason-human-only` for `HumanOnly`), recorded in the transcript.

### 6.9 The pane, the keyboard and accessibility

`PaneKind` gains `Properties` (`"properties"` in `ew.workspace/1`, label "Properties"); the reset layout of the
Schematic workspace puts it to the right of the page view (shares 200 : 800 : 250). A saved workspace without it loads as
saved; View ▸ Properties toggles it. While the pane is not in the layout the sheet is not fetched and the transcript
member is `null`.

**Layout:** the card, the header (title, subtitle, location, finding chips), then each section with a kind line and its
categories (collapsible), each row as label, value, unit text where the type has one, the provenance chip (an icon
glyph and text, never colour alone, FR-UI-025, NFR-A11Y-03), and its action buttons. Long values are ellipsised in the
middle with the full text in the tooltip and the accessible name (doc 16 §17 item 4).

**Keyboard** (context `Properties`): Up, Down, Home, End move the focused row; Left and Right collapse and expand the
focused category; Enter or F2 edits; typing on a focused editable row starts editing with that character; Enter or Tab
commits; Escape reverts, and from a row that is not editing returns the focus to the canvas; Shift+Enter opens the
multi-language popup; Shift+F10 or the Menu key opens the row's actions as a popup list (Up, Down, Enter); Insert,
Delete, Alt+Up, Alt+Down act on list rows; Ctrl+Enter applies the card. E and Alt+Enter on the canvas focus the grid
(doc 06 §6).

**AccessKit:** each row is a group with the label as name and the value, unit, provenance text and error text in its
description; the editor is a text input or combo box labelled by the field; the card and the toast are polite live
regions (doc 16 §10.1). The canvas node's live region announces `<n> selected` as before; the pane announces nothing on
a selection change.

### 6.10 Actions of part 1

Label keys and English texts in `ew_viewmodel::strings`; kinds: `Command` for those that may commit, else `View`.
Context `Properties` is new (registry `Context::Properties`).

| ID | Menu | Shortcut | Context | Params (headless) |
|---|---|---|---|---|
| `props.show` | View ▸ Properties (checked) | — | global | optional `on` |
| `props.focus` | Edit ▸ Properties of selection | E, Alt+Enter | canvas | — |
| `props.project` | Project ▸ Properties | — | global | — |
| `select.set` | hidden | — | global | `ids` (UUID strings) or `refs` (`tag:`, `folio:`, `id:`, `section:` references, resolved by `Session::resolve_reference`); an empty list clears; unresolvable entries refuse the whole action with the resolver's message |
| `props.row` | hidden | Up, Down, Home, End | properties | `step` (±n) or `to` (row key) |
| `props.category` | hidden | Left, Right | properties | `category`, optional `expanded` |
| `props.edit` | hidden | Enter, F2 | properties | optional `row` |
| `props.input` | hidden (the field) | typing | properties | `text` (the whole buffer) |
| `props.commit`, `props.cancel` | hidden | Enter or Tab; Escape | properties | — |
| `props.set` | hidden | — | global | `row` (key), `value` (text as typed), optional `choice` (index, for an ambiguous value) |
| `props.choose` | hidden (the choices) | Up, Down, Enter | properties | `index` |
| `props.row_menu` | hidden | Shift+F10, Menu | properties | — |
| `props.action` | hidden (row buttons, the row menu) | — | properties | `action` (`edit`, `clear`, `override`, `reset`, `freeze`, `unfreeze`), optional `row` |
| `props.list` | hidden (list rows) | Insert, Delete, Alt+Up, Alt+Down | properties | `row`, `op` (`add`, `remove`, `up`, `down`) |
| `props.mltext` | hidden (the popup) | Shift+Enter | properties | optional `lang` (absent: neutral), `text` |
| `preview.apply`, `preview.cancel` | the card's buttons | Ctrl+Enter, Escape | properties | — |
| `toast.undo`, `toast.dismiss` | the toast's buttons | — | global | — |

A `Command` action is disabled with its reason on a read-only project. `clears_banner` gains every action above.
`focus` gains the value `"properties"`.

### 6.11 Transcript and describe of part 1

**Transcript** (`state`, appended after the members of WP-S2.03 and WP-S2.04; a reviewed transcript change, every
expected state in the existing tests gains them):

| Member | Content |
|---|---|
| `properties` | `null` without a project or while the pane is closed; else `{"title": "-101F1", "items": 1, "kinds": {"representation": 1}, "sections": [{"kind": "device", "targets": 1, "rows": 31}, …], "missing": 0, "row": "<key>" or null, "editing": "<key>" or null, "buffer": "<text>" or null, "error": {"row": "<key>", "message": "…"} or null, "choices": n, "popup": null, "mltext" or "actions", "read_only": null or "<reason>"}` |
| `card` | `null` or `{"command": "fields.set", "state": "ready" or "stale" or "failed", "targets": n, "changes": n, "tags": n, "xrefs": n, "origin": "properties"}` |
| `toast` | `null` or `{"text": "Set Function text on 12 devices", "undo": true}` |

No UUID appears in these members (row keys are kind and path; ID-557's rule). *As built (cycle 25, lane B's last fix; ID-626):*
`properties` also carries `highlight`, the highlighted option of an open choice list as an index or `null`, and the describe text's
choices line ends with `selected <i>`; `selected` was a word already, so the 53 words stand.

**`ui.describe`** gains, after the navigator lines (texts in quotes are user text or labels):

```text
properties "-101F1" items 1 kinds representation 1
properties subtitle "<function text>" location "101-B8"
properties finding info "EW-CORE-023" "<message>"
properties read-only "<reason>"                          (only for a read-only sheet)
properties missing 1                                     (only when > 0)
  section device targets 1
  category "Identity" expanded
    row "device/derived.tag" "Tag" "-101F1" derived "numbering scheme 'page'" actions freeze
    row "device/function_text" "Function text" "" authored actions edit clear
    row "device/parts" "Parts" varies 2 "A9F78116 × 1" "A9P52610 × 1" actions edit clear
    row "device/parts[0].part" "Part" "A9P52610 (<manufacturer>)" library "<library>/<item> v<version>" actions edit
    row "connection/derived.csa" "Cross-section" "0.75 mm²" derived "potential 24VDC-1 · size profile '0.75/1.5' · class control" actions override
    row "connection/derived.colour" "Colour" "BU" overridden "override (CDP)" actions reset
    row "device/derived.tag" "Tag" "S51" frozen "frozen (sign hidden)" actions unfreeze
    row "page/derived.description" "Description" "Expansion Control Cabinets" inherited "project description" actions override
    row "project/settings.checks" "Checks" "{…}" read-only "human only: …"
  category "Part" collapsed
  focus row "device/function_text"
  editing "device/function_text" buffer "24 V distribution"
  error "device/tag.counter" "<message>"
  choices "connection/overrides.csa" 2 "1.5 (decimal)" "1500 (thousands)"
  popup mltext "device/function_text"
card ready "fields.set" targets 12 changes 12 tags 12 xrefs 30   (or stale; or failed "fields.set" "<message>")
toast "Set Function text on 12 devices" undo
```

A collapsed category lists no rows; a sheet without sections prints `properties none "<reason>"`. `VOCABULARY` gains
`properties`, `items`, `kinds`, `subtitle`, `location`, `finding`, `info`, `warning`, `error`, `missing`, `targets`,
`category`, `derived`, `authored`, `inherited`, `overridden`, `frozen`, `library`, `varies`, `read-only`, `actions`,
`edit`, `clear`, `override`, `reset`, `freeze`, `unfreeze`, `editing`, `buffer`, `choices`, `popup`, `mltext`, `card`,
`changes`, `toast`, `undo`, and the kind words of every kind a section can have: `device`, `representation`,
`connection`, `project`, `plc_config_project`, `import_lineage`, `potential_def`, `structure_node`, `external_ref`,
`conductor`, `channel`, `bond`, `device_link`, `report_block`, `revision`, `page_item`, `structure_box` (53 words;
FT-314's harness help lists them; a test asserts that every implemented non-library kind's name is in `VOCABULARY`). The lines also use `page`, `section`, `row`, `focus`, `expanded`, `collapsed`,
`generated`, `none`, which exist, `stale`, `failed`, `tags`, `xrefs`, `ready` from WP-S2.03 part 2, and `function` and
`junction` from WP-S2.04 parts 1 and 2; part 1's vocabulary test asserts that every token of these lines is in
`VOCABULARY`, and a word that an earlier part has not added yet is added by this part, which states the new count.

### 6.12 What the pilot edits on folios 101 to 103

The rows the G-PP rehearsal and AC-28 exercise, with their routes (every other row follows the generic rules):

| Record (Golden A) | Row | Route |
|---|---|---|
| -101F1, -102F1 (devices) | Function text, Parts (`parts[0].part`, `qty`), Tech data, State | `fields.set device function_text`, `parts`, `tech_data`, `state` |
| S51, F55, T55, E55, S56, E56, 1X1 (frozen tags, EF-15) | Tag (Frozen), Unfreeze | `fields.set device tag.frozen` ← `null` |
| -102F1 | Tag letter, Tag counter (a collision refused with the next free counter) | `fields.set device tag.letter`, `tag.counter` |
| -X1031:1 to :20 (terminals) | Designation, Parts (FR-DEV-031) | `fields.set device terminal.designation`, `parts` |
| W29 (connection, 102 and 103) | Cross-section, Colour (derived; Override, Reset) | `fields.set connection overrides.csa`, `overrides.colour` |
| W20 (its CDP) | Cross-section (Overridden; Reset) | `fields.set connection overrides.csa` ← `null` |
| Page 101 | Title; Description (Inherited; Override for D-06; Reset) | `fields.set page title`, `description_override` |
| 24VDC-1 (potential definition, by `select.set`) | Name, Voltage | `fields.set potential_def name`, `voltage` |
| Representations | Text slots, Arrangement | `fields.set representation text_slots`, `arrangement` |
| IP functions (L-1, N-1, …) | IP name, direction | read-only (`NoCommand`; follow-up in §15.4) |

## 7. Part 2: find and replace

### 7.1 `Session::find`

**Fields** (FR-UI-020's scopes; FR-DEV-032's list):

| `FindField` | Searched | Replaceable |
|---|---|---|
| `FunctionText` | every string of each device's `function_text` (neutral and each language) | yes: `device function_text` |
| `Tag` | each device's displayed tag on its main page | frozen tags: `device tag.frozen` (its `text`); derived tags: no, "derived: change the numbering or freeze the tag" |
| `Property` | each device's `tech_data` values as text (`key=value`) | `Ident` values: `device tech_data` (the whole list with the item changed); `Quantity` values: no, "a quantity: edit it in the property grid" |
| `Text` | each page's `title` and `description_override` (each string); each device's `process_tag`; each free text's `text` once WP-S2.04 part 4a has implemented `page_item` | `page title`, `page description_override`, `device process_tag`, `page_item kind.free_text.text` |
| `PartNumber` | each device's parts: the part copy's part number and manufacturer | no: "library copy: update from library" (FR-PRT-014; D16) |

**Matching.** Plain text (default): every non-overlapping occurrence, left to right; `match_case` false compares with
Unicode simple case folding (`char::to_lowercase` per character, both sides NFC). `regex`: `regex::RegexBuilder` with
`case_insensitive(!match_case)`, `size_limit(1 << 20)`, `dfa_size_limit(1 << 20)`, `nest_limit(64)`; a pattern longer
than 1,000 characters or one that does not compile gives `error` with the message and no hits. `whole_word`: a match
counts only when the characters before and after it are not alphanumeric or `_`. Empty `text`: no hits, no error.

**Scope.** `fields` (empty: all five); `folios`: only records whose page (device and function texts: the main
representation's page; page fields: the page) has one of these printed folio labels (empty: all; an unknown label
gives `error` "no page <label>"); `within`: only these records and the records they reach (§6.2) when non-empty.

**Hits.** One per (record, field string) with at least one match: `subject` (the device tag, page folio and title, or
wire number), `folio` and `cell` (for the jump), `before` (the whole string), `ranges` (byte ranges of the matches in
`before`), `after` (when `replace` is set and the hit is replaceable: plain text replaces every match by `replace`
literally; regex uses `Regex::replace_all` with `$1`-style expansion), `replaceable`. **Order:** by page order (the
folio's position in `page_order`; records without a page last), then cell (row, column), then subject (natural order),
then row path, then language tag. At most 10,000 hits; beyond, `truncated` and replace is refused.

**`replace_command`**: `None` when no hit has `after`, when `truncated` or `error`; else the `fields.set_many`
parameters: for each replaceable hit with `after`, the new value of the record's settable member (an MlText with that
language string replaced, a frozen tag with the new text, a tech-data list with the item replaced), one entry per
(kind, path, value) holding every record that gets that value, entries ordered by (kind rank, path, first target ID).
Hits on one record and member are merged into one value first.

### 7.2 `fields.set_many`

As §5.1. Its preview's derived diff shows the tag and cross-reference changes; the card shows `changes` = the number of
replaced hits.

### 7.3 The find pane

`PaneKind::Find` (`"find"`, label "Find and replace"), opened by Ctrl+F (find) and Ctrl+H (find and replace; ⌥⌘F on
macOS, doc 16 §4.4) into the bottom dock of the Schematic workspace. Fields: the text, the replacement (find and replace
only), the toggles regex, match case, whole word, the field checkboxes, the folio filter (text, comma-separated
labels), "in selection". `find.run` (Enter in the text) calls `Session::find` once; typing alone does not search (exact
counts). The results list one line per hit: folio and cell, subject, field label, the string with the matches marked
(bold and underline, not colour alone) and, with a replacement, `-> <after>`; non-replaceable hits show their reason.
`find.row` moves the result cursor; `find.jump` (Enter on a result, or double click) selects the record (`select.set`)
and shows its page with the grid cursor on its cell (WP-S2.03's `nav.goto`), keeping the pane open.

### 7.4 Replace

`find.replace_all` (Ctrl+Enter in the pane, or the button) builds `replace_command()` and sends `Preview` through the
host: the card always opens (FR-UI-020, FR-DEV-032: "one previewed command"), listing the hits that change, and Apply
commits one transaction (one undo step). After the commit the results are not re-run automatically; the pane shows
"Replaced <n>; run again to refresh". A hit list made at another revision than the current one is stale (badge), and
`find.replace_all` re-runs the search first. **A stale replace card** is not applied with its old parameters: Apply runs
the search again with the same query (one `find` call), rebuilds `replace_command()` from the new hits and shows a
fresh card that needs a second Apply; the card states how many hits changed ("Preview updated: 2 fewer results").

### 7.5 Freeze on several devices

The `Freeze` action on a device section with several targets builds `fields.set_many` with one entry per device
(`tag.frozen` ← its own displayed product designation, `sign` `shown`), through the same preview and policy (rule 2
opens the card).

### 7.6 Actions, transcript and describe of part 2

| ID | Menu | Shortcut | Context | Params (headless) |
|---|---|---|---|---|
| `find.open`, `find.open_replace` | Edit ▸ Find…, Edit ▸ Find and replace… | Ctrl+F; Ctrl+H (⌥⌘F) | global | — |
| `find.text`, `find.replacement` | hidden (the fields) | — | find | `text` |
| `find.option` | hidden (the toggles) | — | find | `option` (`regex`, `match_case`, `whole_word`, `in_selection`), `on` |
| `find.fields` | hidden (the checkboxes) | — | find | `fields` (list of `function_text`, `tag`, `property`, `text`, `part_number`) |
| `find.folios` | hidden (the filter) | — | find | `folios` (list of labels) |
| `find.run` | hidden | Enter | find | — |
| `find.row`, `find.jump` | hidden | Up, Down; Enter | find | `step` or `to` (1-based); — |
| `find.replace_all` | Edit ▸ Replace all | Ctrl+Enter | find | — |

`focus` gains `"find"`. **Transcript:** `find` after `toast`: `null` while the pane is closed, else `{"text": "Spare",
"replace": "Reserve" or null, "regex": false, "match_case": false, "whole_word": false, "in_selection": false,
"fields": ["function_text"], "folios": ["204", "205", "206"], "results": 20, "truncated": false, "error": null, "row":
1 or null, "stale": false}`; the card's `origin` is `"find"` for a replace.

```text
find "Spare" replace "Reserve" regex off case off word off scope function_text folios "204" "205" "206" results 20
  result 1 folio "204" cell "204-B3" "-201U5-Q1" "Function text" "Spare" -> "Reserve"
  focus result 1
find "Murrelektronik" regex off case off word off scope part_number results 1
  result 1 folio "102" cell "<cell>" "-102F2" "Part number" "MICO 4.10 (Murrelektronik)" read-only "library copy: update from library"
find error "<message>"                                  (or: find truncated)
```

`VOCABULARY` gains `find`, `replace`, `regex`, `case`, `word`, `scope`, `function_text`, `property`, `part_number`,
`folios`, `results`, `result`, `truncated` (13 words); the lines also use `on`, `off`, `folio`, `cell`, `none`,
`focus`, `error`, `read-only`, which exist after part 1, and `->`, `tag` and `text` from WP-S2.04 (added by this part if
absent, with the count stated).

### 7.7 As built (cycle 26; decision log ID-634, ID-639)

Part 2 is built as §7.1 to §7.6 say, with these settlements. **Search:** a row path is `<member>.<lang>` and orders naturally (`tech_data[2]` before `tech_data[10]`); empty `fields` means all five; a search error ends the step done, with the message in the pane and the transcript; "In selection" without a selection is refused. **Replace:** one `fields.set_many` per replace (members merged per record, grouped per (kind, path, value), no-ops dropped); only frozen tags are replaced (their product part); the replaced text is NFC-normalised, and a replacement that would recompose with its neighbours (`=` followed by a combining slash becoming `≠`) is refused; a refusal names its entry "on <subject> (<path>): …". **The card** opens **inside the find pane** ("Preview: replace 20 results on 20 records · 0 tags, 0 cross-reference texts"), counts only the hits that change, survives a selection change (a jump to a result or a canvas click) and is dropped whenever the pane is not shown (`find.close`, which is hidden, a workspace reset or load, a dock layout change). After Apply no toast follows (§6.6 gives the toast to direct commits); the results show `[stale]` and the note "Replaced <n>; run again to refresh"; an automatic re-run was weighed and not adopted (it costs a `find` call that §9 does not count, and the badge says what changed). The card and toast describe lines follow the find lines, so a replace card shows with the grid closed. **Keys:** Ctrl+Enter in the find context replaces, and with a card open (also from a focused field) applies it; **Tab and Shift+Tab move between the pane's fields**: a find field that takes the keyboard focus makes the pane the shell's focus (the window check found Tab in the search field reaching the canvas chord `tool.next_proposal` instead of "Replace with"; fixed by 7019087 and c4b60ce with two kittests). **Place:** the pane sits below the page view in the Schematic workspace, not across doc 16's full-width bottom dock. **macOS:** Ctrl+H on every OS until the per-platform keymap (⌘H hides an application on macOS; FT-431). **Counts:** `VOCABULARY` +14 (the 13 words of §7.6 and `text`, which WP-S2.04 had not added), 12 actions (§7.6's rows counted singly and the hidden `find.close`); the `run_script` Bulk rule of doc 17 §3.5 makes a log line of `fields.set_many` above 500 targets one previewed commit. **Not reached by the find** (FT-432): terminal designations, potential names, wire numbers (§7.1's subject "wire number" has no field), structure descriptions and `page_item` texts. The result list is built for every hit in each frame (FT-433).

## 8. Part 3: the clipboard

### 8.1 The fragment v1 format (D12)

**One format, not two** (ruling, delegated, on the review): `#ew-fragment v1` **is** the project-fragment format of
architecture §5.5. WP-S1.19's window and page macros extend it (new `$fragment` header members, a value set, further
record kinds) instead of defining a second format; this package defines its first version.

A fragment is UTF-8 text, LF line ends, a final LF:
1. Line 1: exactly `#ew-fragment v1`.
2. Line 2: `{"$fragment": {"format": "0.1", "project": "<uuid>", "grid": [num, den], "page_kind": "<page type>",
   "anchor": [x, y], "ghost": [[x0, y0, x1, y1], …]}}`: the project format of the records, the source project's ID, the
   source page's connection grid, its page type, the anchor (the minimum corner of the copied representations' bodies
   and items' bounds, snapped down to the grid: the largest grid point not greater on each axis), and the ghost
   rectangles (each copied representation's `TableRep.body` and each item's bounds, relative to the anchor, in record
   order).
3. Then one line per record, the canonical record line of the project files (`$k` first; doc 05 §3.1), ordered by
   (kind rank, ID).
4. Then one line `{"$lib": <libdoc>}` per library copy (the `lib/` JSON form `library.embed` takes), ordered by
   (kind, copy ID).
5. Then one line `{"$node": {"id": "<uuid>", "aspect": "<aspect>", "path": ["J-E-2416", …], "aliases": […]}}` per
   structure node a record references, ordered by ID.

Equal selections on equal snapshots give byte-equal text. Any other application sees JSON Lines after one comment line
(FR-UI-040). `edit.paste`'s `fragment` parameter is the same content as one JSON object: `{"format", "project",
"grid", "page_kind", "anchor", "records": [<record>…], "copies": [<libdoc>…], "nodes": [<node>…]}` (no ghost); its
schema is committed with the command.

### 8.2 Copy (`Session::copy_fragment`)

From the selected IDs, on one page (else `NotOnePage`; a generated page: `Generated`; nothing copyable: `Empty`):
1. **Records.** Representations, connections, junctions, page items and structure boxes as selected; for each copied
   representation its functions; for each function its device and all of that device's functions (functions the copy
   does not draw are pasted unplaced); for a terminal device its strip device (with only the copied terminals); the
   potential definitions at copied functions' CPs.
2. **Connections:** every connection on the page whose two ends lie in the copied set (a CP end: its function copied;
   a junction end: the junction copied), selected or not; a selected connection with an end outside is dropped and
   counted in `dropped_connections` (doc 06 §3.6 item 5).
3. **Junctions:** selected ones, and every junction at which at least two connections with copied far ends meet;
   steps 2 and 3 repeat until nothing is added. A copied junction's `sequence` keeps only copied connections.
4. **Wire markers** only when every listed connection is copied; other page items as selected.
5. **Library copies:** every symbol, FnDef and part copy the records name (representations' symbols, functions' FnDefs,
   devices' parts, connections' `overrides.wire_part`); **structure nodes:** every node a record's structure
   assignment names (by ID, with its aspect, path codes and aliases).
6. **Not copied** (listed in `skipped` by kind with counts): bonds, cables and conductors, channels, device links
   (`device_link`), PLC and assembly aspects' links outside the copy, revisions, report blocks, project records, and
   image items (part 5) when the
   paste would be cross-project (they are copied, and refused cross-project at paste, §8.4).

`Session::tag_list(ids)`: the devices reached from the IDs (§6.2's expansion), each as its displayed tag on its main
page, deduplicated, in natural order, one per line, LF, final LF; an empty string for none (FR-UI-040's "Copy tag
list").

### 8.3 Parsing (`parse_fragment`)

The text is untrusted input. **Versions.** Within v1, `$fragment` header members this build does not know are ignored
(later minor additions, such as WP-S1.19's); its known members are required as §8.1 lists them. A `format` other than
this build's, or a record whose `$v` differs from this build's schema version of its kind, is refused: newer as
`Newer` ("copied from a newer ElektraWatt, which writes a newer format", doc 06 §8 item 15), older as `Invalid` ("copied
from an older ElektraWatt format; open and save that project with this version first"), until WP-S1.17's migration
chain exists and the fragment reader runs it (doc 06 §8 item 15's upgrade; a follow-up, §15.4). Refused also: not
starting with line 1 (`NotAFragment`); a `v2` or later header (`Newer`);
more than 4 MiB, more than 10,000 record lines, a line over 1 MiB, nesting over 64 (`TooLarge`); a line that is not
strict JSON (`ew_schema::parse_value`), an unknown `$`-line, a record of a kind other than `device`, `function`,
`representation`, `connection`, `junction`, `potential_def`, and `page_item` and `structure_box` once implemented, a
record that `Record::from_value` rejects, a duplicate ID (`Invalid { line, message }`). The result's `params` is the
`fragment` object of §8.1 with the records, copies and nodes as parsed (no semantic check; that is the command's).

### 8.4 `edit.paste`

In one transaction, reading rows through `TxBuilder` only. **Rejected** (at the pointer named):
- `/page`: a missing page, a generated page, a page type that takes no symbols;
- `/fragment/grid`: a page connection grid other than the fragment's ("the copied content uses another grid; paste on a
  page with the grid <n> mm", follow-up for mixed grids, §15.4);
- `/at`: not a grid point of the page;
- `/fragment/records/<i>`: a kind outside §8.3's list; a representation whose rep type the page type does not allow; a
  **mandatory** reference (effect 4) that resolves neither inside the fragment nor through effects 1 and 2; an image
  item whose asset the target project does not hold;
- `/fragment/copies/<i>`: a document that `library.embed`'s `check_copy` rejects; a copy the project holds in another
  version (other `sha256`): "the project holds another version of <item>; update from library first" (the three-way
  choice of doc 06 §3.6 item 4 is a follow-up);
- more than 10,000 records.

**Effects:**
1. **Library copies:** held with the same `sha256`: reused; not held: embedded exactly as `library.embed` does, in this
   transaction (one undo step removes them with the pasted records).
2. **Structure:** each `$node` rebinds to the target node of the same aspect and the same path codes, else to the one
   node whose aliases contain the leaf code; an unmatched node clears the slots that named it (the record inherits),
   with a notice "structure <path> is not in this project: inherits the page". No node is created.
3. **Identity:** every record gets `tx.mint()`, in (kind rank, source ID) order. The references to remap are found
   **mechanically from `ew_model::LINKS`** (every declared link of the record's kind; `NOT_LINKS` members are copied as
   they are): a link whose target is a fragment record takes the new ID; a `LibRef` link takes the held or embedded copy
   ID (effect 1); a structure slot takes the rebound node (effect 2). No hand list of reference members exists in the
   code.
4. **Mandatory and optional by the link's delete policy.** A link that resolves to nothing after step 3 is handled by
   its `Policy` in `LINKS`: **`Unlink`** (an optional reference) is cleared exactly as a delete would clear it, with a
   notice naming the record and the member; **`Forbid`** or **`Cascade`** (a reference the record cannot live without)
   refuses the paste at the record's pointer. An IP function's `external` reference and a structure box's `external`
   are kept only when the fragment's `project` is the target project and the reference exists there, else handled by
   their policy. **The test oracle** (CB3) is the expected outcome per member, derived by hand from doc 03 and checked
   against the mechanical rule: mandatory are a representation's functions and symbol copy, a function's FnDef copy, a
   device's part copies, a connection's ends, a terminal's strip, a potential definition's endpoint; optional are a
   device's `parent`, a connection's `overrides.wire_part`, the `external` references. A difference between the oracle
   and `LINKS` stops the lane for a lead ruling. **A child device whose parent is cleared** also loses its `position`,
   and when its `tag.scheme` is `child` that scheme is cleared too, so the target's default scheme numbers it (effect
   6), with a notice "-<old tag> was a child of a device that was not copied".
5. **Translation in grid steps:** `Δk = index(at) − index(anchor)` per axis on the page grid; every coordinate on the
   grid (positions, route vertices, junctions) becomes the grid point at its index plus `Δk`; any other coordinate
   (graphics, texts, box outlines off the grid) moves by `round_half_away(Δk × num / den)` (WP-S2.04's `rep.move`
   `steps` rule); box CPs and outlines relative to `pos` are unchanged. `page` becomes the target page on every
   representation, junction, page item and structure box.
6. **Numbering reset:** each device's `tag.counter` and `tag.frozen` are cleared (frozen tags are not copied, doc 06
   §3.6 item 2); terminal designations are kept (the strip is new); each connection's `number` becomes `Unassigned`.
   The online-numbering normalisers then number the new devices and wires in their own order (the lane verifies that
   the as-built normaliser orders new devices by anchor cell, FR-NUM-028; if not, it stops and reports).
7. **IP names** are kept; pairings with points already in the target are derived (the preview's derived diff shows
   the new cross-reference texts, doc 06 §3.6 item 6).
8. A wire marker left without connections and a junction left with fewer than two connections are removed by the
   existing clean-up (junction clean-up row 8; the wire-marker delete policy of WP-S2.04 part 4a).

### 8.5 The paste tool, duplicate and cut

**Paste** (`edit.paste_clip`, Ctrl+V on the canvas): the window takes the text of egui's paste event, headless the
headless clipboard (§8.7); `parse_fragment` runs once; an error shows in the banner. A fragment arms the **paste tool**
(`paste`), state `floating` (not sticky): `PointerMove` or `CursorMove` → `Transient`: one `Ghost` rectangle per ghost
entry at the snapped anchor (`snapper.snap(p, alt = true)`'s grid point), a `Label` `paste <n> records`, and the
dropped and skipped counts; `Dwell { ms ≥ 80 }` → `Preview(edit.paste { page, at, fragment })` (the semantic ghost: the
derived diff's tags, "the ghost shows the resulting tags", doc 06 §3.6); `Press` or `Activate` → `Commit` and return to
Select with the pasted representations selected (one `Select` change); `Escape` → `Cancel`. Text that is not a
fragment is refused with `reason-not-a-fragment` (pasting plain text into a canvas does nothing).

**Duplicate** (`edit.duplicate`, Ctrl+D on the canvas): `copy_fragment` of the selection without touching the clipboard,
then the paste tool with the ghost starting two grid steps right and down of the anchor. **Cut** (`edit.cut`, Ctrl+X):
copy, then `edit.delete` of the selection through `edit.delete_selection`'s path (one undo step; the clipboard keeps
the fragment). **Copy** (`edit.copy`, Ctrl+C): the fragment to the clipboard; **Copy tag list** (`edit.copy_tags`,
Edit menu): `tag_list` to the clipboard as text. Copy and cut with nothing copyable are refused with
`reason-nothing-selected`.

### 8.6 Cross-instance and cross-project paste

The fragment is plain text, so a copy in one EW instance pastes in another (FR-UI-040), and a copy in one project pastes
in another project opened later in the same window. Library copies travel with the fragment (§8.4 effect 1); structure
nodes rebind or clear (effect 2); tags follow the target's numbering (effect 6, FR-UI-036).

### 8.7 The headless clipboard

Headless runs have no system clipboard: the shell keeps one text value. `clipboard.text` (hidden; `text`) sets it, as
an external copy would; `edit.copy`, `edit.cut` and `edit.copy_tags` write it; `edit.paste_clip` reads it. In the window
the same actions use egui's `Context::copy_text` and the paste event; `clipboard.text` is headless only.

### 8.8 Actions, transcript and describe of part 3

| ID | Menu | Shortcut | Context | Params (headless) |
|---|---|---|---|---|
| `edit.copy`, `edit.cut`, `edit.paste_clip`, `edit.duplicate` | Edit ▸ Copy, Cut, Paste, Duplicate | Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+D | canvas | — |
| `edit.copy_tags` | Edit ▸ Copy tag list | — | canvas | — |
| `clipboard.text` | hidden | — | global | `text` |

**Transcript:** `clipboard` after `find`: `null` (empty) or `{"kind": "fragment", "records": n, "copies": n}` or
`{"kind": "text", "lines": n}`; the canvas `ghost` gains `{"kind": "paste", "records": n, "at_nm": [x, y], "dropped":
n}`.

```text
clipboard fragment records 7 copies 4                  (or: clipboard text lines 3; clipboard none)
canvas ghost paste records 7 at 254.000 101.600 mm dropped 1
```

`VOCABULARY` gains `clipboard`, `fragment`, `records`, `copies`, `paste`, `dropped` (6 words); `lines`, `none`, `at`,
`mm` exist, `text` and `ghost` come with WP-S2.04.

## 9. Determinism, the headless mode and exact counts

1. **Every flow is a script.** Each acceptance flow of §10 that runs in kittest also runs as an `ew.uiscript/1` script
   through the built binary with byte-equal transcripts on two runs. No timer, clock or hash order reaches the model or
   the transcript; the toast's timeout is window-only (§6.6).
2. **Every commit is reproducible.** The `--record` log of a script, applied with `ew apply` to the starting project,
   gives equal records (a test per part); `fields.set_many` carries explicit values and `edit.paste` the whole
   fragment, so neither searches nor parses the clipboard on replay.
3. **Tie-breaks are named:** section order, row order, `‹varies›` text order, choice order, find hit order and
   `replace_command` entry order, fragment line order, the ID mint order of a paste, the anchor snap. None depends on a
   hash map's order or on time (NFR-REL-07).
4. **Exact counts** (NFR-PERF-20; S2.03 §8 item 3's counters, extended by `property_sheet`, `find`, `copy_fragment`,
   `parse_fragment`): with the pane open, a selection change, a focused-page change with an empty selection, or a new
   revision causes exactly one `property_sheet` call on the next frame, and nothing else does; row moves, category
   toggles, typing into the buffer, opening choices or popups cause 0 `property_sheet` calls and 0 memo
   re-executions; a commit makes exactly one `preview_with` and then one `commit` (window) or one `run_script`
   (headless), or none more until Apply; `find.run` makes exactly one `find` call and typing in the find pane none;
   `edit.paste_clip` makes exactly one `parse_fragment` call, ghost moves 0 `Session` calls, and a dwell that changes
   the anchor or the revision one `preview_with`. With the pane closed, 0 `property_sheet` calls.
5. **No time values, paths, process IDs or UUIDs** in transcripts or describe text of this package. *As built (cycle 26,
   FT-422; ID-630):* every UUID of a transcript line and of the describe text is printed as its alias `@n` (numbered by
   first appearance in the run's output, the table in the shell model); `select.set` and the other UUID-valued action
   parameters take `@n` back. Part 1's mask `‹id›` is gone.

*As built (cycle 25, lane B's last fix; ID-626):* item 4's counts include `field_choices`: each changed buffer text in a choice editor
makes one lookup, counted by `SessionCalls::field_choices`, and moving the highlight makes none.

## 10. Acceptance checks

All tests are ordinary nextest tests in the existing jobs, on temporary copies of Golden A (or of a project made by the
wizard) with `EW_CONFIG_DIR` set to a temporary directory. A golden example that the data does not support stops the
lane for a lead ruling.

### 10.1 Part 1

| # | Check | Where | Proves |
|---|---|---|---|
| PG1 | Catalogue: `field_catalogue` for every implemented kind equals `tests/expected/field_catalogue.txt`; every `SETTABLE` path maps to a node; the `Json` fallbacks are the listed ones; the five project settings rows exist on a new wizard project with `Absent` values and a size profile set from the grid commits `project.settings.set`; revision `author` and `checker` are `ReadOnly(Narrowed)`; a `HumanOnly` route is never editable (`field_catalogue_for` over a test registry in which `fields.set` is registered as `HumanOnly` makes every `FieldsSet` row `ReadOnly(HumanOnly)`) | `ew-api` tests | §6.1, D2, D3, D4 |
| PG2 | Sheet on Golden A: -101F1's representation (three sections; tag `-101F1` derived "numbering scheme '…'"; part row Library); S51 (Frozen, sign hidden); W29 on 103: cross-section `0.75 mm²` with the size-profile chip and colour `DBU` with the colour-profile chip exactly as D15; W20: cross-section Overridden "override (CDP)"; page 101: description Inherited "project description"; a representation plus a connection: no section; a deleted ID: `missing` 1 | `ew-api` tests | §6.2 to §6.4 |
| PG3 | FR-UI-009's acceptance: selecting -101F1 and -102F1 shows `‹varies›` for the parts (2 values); typing a function text on -101F1 commits one `fields.set` and is one undo step (the history grows by one; undo restores); keystrokes alone commit nothing; Escape reverts | `ew-viewmodel` tests | §6.3, §6.6, D7 |
| PG4 | FR-UI-010's acceptance: W29's cross-section shows its profile origin; overriding it to `1.0` shows Overridden; Reset restores the derived value and provenance; each step one undo step | `ew-viewmodel` tests | §6.4 |
| PG5 | AC-28's W29 step (kittest, Golden A, folio 103): select W29 through `select.set` (amended at the integration, ID-628: the canvas picks by geometry since WP-S2.04 part 1, and the centre of a wire's rectangle need not lie on the wire), see the chips of PG2 in the grid, change its colour to `BU` through the colour editor, undo: colour `DBU` derived again | `ew-ui` tests | AC-28 |
| PG6 | FR-DEV-031's acceptance: `select.set` with the 20 terminal devices of -X1031 (`tag:` references in the form `Tag::short` prints, else `id:`); setting their part through the part picker is one `fields.set` with 20 targets, one undo step, and the toast reads "Set Part on 20 devices" (rule 2 does not apply: the part is not an identifier) | `ew-viewmodel` tests | §6.6, D10 |
| PG7 | The policy: a tag-counter edit on two devices opens the card (`reason-identifiers`); Apply commits once; Cancel leaves the revision unchanged; a stale card rebuilds the command from the current values and needs a second Apply, and refuses `reason-changed-meanwhile` when the edited member changed on a target (another edit committed in between); a stale replace card re-runs the search; `project.settings.set` always opens the card; a 501-target edit opens it (`reason-bulk`) | `ew-viewmodel` tests | §6.6 |
| PG8 | Editors and refusals: every row of §6.5's table parses and formats its examples; `1,500` gives two choices; an Ident with a Cyrillic letter commits with the note; `BR` is refused with the suggestion `BN`; a counter collision on -102F1 shows the command's message naming the next free counter; a read-only row refuses `props.set` with `reason-read-only`, the checks member with `reason-human-only` | `ew-api`, `ew-viewmodel` tests | §6.5, §6.7, §6.8 |
| PG9 | D-06's property-grid entry: on page 101, Override the description to "Control Cabinet Expansion": the row shows Overridden "override · project description is 'Expansion Control Cabinets'" and the header shows EW-CORE-023 at info; Reset removes both; each one undo step (kittest and the headless script) | `ew-ui`, `bin/elektrawatt/tests` | §11, D-06 |
| PG10 | Transcript and describe of §6.11 with the 53 words; the scripts of PG3 to PG9 twice byte-identical; their `--record` logs replayed by `ew apply` give equal records | `ew-viewmodel`; `bin/elektrawatt/tests` | §6.11, §9 |
| PG11 | Exact counts of §9 item 4 for the grid | `ew-ui` test | §9 |
| PG12 | Golden unchanged (the existing golden output, acceptance and AC-15 tests pass unmodified); policy and targeted checks for the changed crates (`ew-api`, `ew-derive`, `ew-import-tables`, `ew-viewmodel` with and without `testkit`, `ew-ui`, `elektrawatt`): nextest, clippy `-D warnings`, `cargo fmt --all --check`, `check-deps` (the two new edges), `check-features`, `check-raw-read`, `check-licence-map`, `reuse lint`; no new package | lane | testing policy, §4.1 |

### 10.2 Part 2

| # | Check | Where | Proves |
|---|---|---|---|
| FR1 | `fields.set_many`: every rejection with its pointer; equivalence with a sequence of `fields.set` on the same records; `Bulk` above 500 targets; DSL verb and fuzzer arm | `ew-commands`, `ew-testkit` tests | §5.1 |
| FR2 | `Session::find`: plain, case, whole word and regex matches with ranges; the order rule; the folio and selection scopes; a pattern over 1,000 characters and a non-compiling one give `error`; truncation at 10,000; derived tags and part numbers found but not replaceable with their reasons | `ew-api` tests | §7.1 |
| FR3 | FR-UI-020's acceptance on Golden A: "Spare" in function texts on folios 204, 205, 206 lists 20 results (-201U5 to -201U8 and -202U2, Q1 to Q4), on 204 and 205 16; replacing by "Reserve" on 204 to 206 previews 20 changes in the card and commits one `fields.set_many` (one undo step); undo restores all 20 | `ew-viewmodel` tests; kittest | §7.3, §7.4, D16 |
| FR4 | FR-DEV-032's amended example on authored device fields: the regex `(Spare)` replaced by `$1 (unused)` in the function texts of 204 to 206 gives "Spare (unused)" on the 20 devices, and a replace in an `Ident` tech-data value of a device on a copy (the lane adds one with `fields.set` first), each as one previewed transaction; "Murrelektronik" is found as the manufacturer of MICO 4.10 and not replaceable ("library copy: update from library") | `ew-viewmodel` tests | §7.1, D16 |
| FR5 | Freeze on three devices of folio 102 is one `fields.set_many` through the card; each device's tag then reads Frozen with its own text | `ew-viewmodel` tests | §7.5 |
| FR6 | Transcript, describe, the 13 words (as built 14 with `text`, ID-634); the scripts twice byte-identical; replay; exact counts; golden unchanged; policy and targeted checks (`ew-commands`, `ew-testkit`, `ew-api` with the `regex` edge, `ew-viewmodel`, `ew-ui`, `elektrawatt`; `docs-gen --check`) | lane; tests | §7.6, §9 |

### 10.3 Part 3

| # | Check | Where | Proves |
|---|---|---|---|
| CB1 | Fragment writing: the text of a fixed selection on Golden A is byte-equal on two runs and against a committed expected fragment (`crates/ew-api/tests/expected/fragment_102F1.txt`); the closure rules of §8.2 (a terminal brings its strip; internal connections and junctions; a selected connection with one end outside is dropped and counted) | `ew-api` tests | §8.1, §8.2 |
| CB2 | Parsing: every refusal of §8.3 with hostile inputs (oversized, deep nesting, repeated members, unknown `$`-lines, foreign kinds, malformed records, `v2`, a newer and an older `format`, a record with another `$v`); an unknown `$fragment` header member is ignored; the fuzzer feeds random text to `parse_fragment` without a panic | `ew-api` tests, fuzzer arm | §8.3 |
| CB3 | `edit.paste`: every rejection of §8.4 with its pointer; the mandatory and optional outcome of every link member equal to §8.4 effect 4's oracle (a test walks `ew_model::LINKS` for the copyable kinds); a child device whose parent is not copied (position and child scheme cleared, notice); library copy reuse, embedding and the other-version refusal; structure rebinding by path and alias and the clearing notice; the step translation exact on the QET grid (`7_937_500/3` nm) for three representations with different `k mod 3`; undo removes the records and the embedded copies in one step | `ew-commands` tests | §8.4 |
| CB4 | FR-DEV-029's acceptance: copying -102F1 on folio 102 and pasting it on 102 yields a device whose tag is -102F3 with part A9F78116 and no connection (its wires had an end outside) | `ew-viewmodel` tests | §8.4, FR-DEV-029 |
| CB5 | FR-UI-036's acceptance: copying T55, E55 and E56 of folio 101 into folio 1 of a new wizard project yields three new devices with new IDs, no frozen tag, tags derived by that project's scheme, the golden part copies embedded once | `bin/elektrawatt/tests` | §8.6, FR-UI-036 |
| CB6 | FR-UI-040's acceptance with two shell models in one test (two instances): copying -102G1 and -102F1 on folio 102 in one and pasting into a new wizard project in the other inserts both devices and the connections between them; "Copy tag list" of -101F1, S51 and F55 gives `-101F1\nF55\nS51\n` | `bin/elektrawatt/tests` | §8.6, FR-UI-040 |
| CB7 | The paste tool (ghost, dwell preview with the predicted tags, commit, Escape), duplicate (the clipboard unchanged) and cut (one undo step restores the records; the clipboard keeps the fragment); plain text refused | `ew-viewmodel` tests | §8.5 |
| CB8 | Transcript, describe, the 6 words; the scripts twice byte-identical; replay by `ew apply` (the log carries the fragment); exact counts; golden unchanged; policy and targeted checks (`ew-commands`, `ew-testkit`, `ew-api`, `ew-viewmodel`, `ew-ui`, `elektrawatt`; `docs-gen --check`) | lane; tests | §8.8, §9 |

The workspace suite, the acceptance runner and the performance runs are not run by the lanes (testing policy, ID-512);
the integrating session runs `cargo xtask ci` at the cycle delivery.

## 11. D-06 and the gates

D-06's property-grid entry ("an override is reported as info and shown in the property grid", doc 20 §19) gates at
**G-M2** with AC-18 (GUI) since the G-M1 readiness rulings (ID-422); the acceptance runner reports it `not_applicable`
with that reason (ID-433). This package builds what the entry needs and proves it with PG9 (kittest and a headless
script) at M-PP. **The runner entry is not changed here**: the runner is a headless `ew-api` test that cannot depend on
`ew-viewmodel`, and changing its catalogue would change `golden/J-E-2416/expected/acceptance_headless.md`, which this
package may not touch (§4.1 rule 5). G-M2's evidence for the entry is PG9's test report; the design pass after part 1
records that pointer in gates §8.2 and doc 20 §19, and decides whether the runner's reason text names PG9. AC-28's W29
step (PG5) is likewise M-PP evidence for a G-M2 criterion; its other steps belong to WP-S2.03 and WP-S2.04, and "every
frame ≤ 16 ms" is not measured (owner's rule, ID-512).

## 12. Files owned

**Part 1:** `crates/ew-api/src/output/{mod.rs,fields.rs,properties.rs}` (`fields.rs` and `properties.rs` new),
`crates/ew-api/src/lib.rs` (methods and re-exports only), `crates/ew-api/src/cmdlog.rs` (the resolver factored),
`crates/ew-api/Cargo.toml` (two edges), ew-api tests with `tests/expected/field_catalogue.txt`;
`crates/ew-derive/src/settings.rs` (`company_codes` only); `crates/ew-import-tables/src/table.rs` (`pub` on
`COLOUR_ALIASES` only); `crates/ew-viewmodel/src/**` (new `properties.rs` or `properties/`, `actions`, `strings`,
`describe`, `script`, `shell`, `layout`, `settings` for `properties.collapsed`); `crates/ew-ui/**` (new
`properties.rs`, `dock.rs`, `app.rs`, `keys.rs`, `headless.rs`); `bin/elektrawatt/tests/**`.
**Part 2:** `crates/ew-commands/src/commands/{fields.rs,mod.rs}` (`fields.set_many` beside `fields.set`, sharing its
per-target step), `schemas/**`, `tests/**`; `crates/ew-testkit/src/golden/dsl.rs`, `tests/command_fuzz/**`; a new
`crates/ew-api/src/output/find.rs`, `crates/ew-api/Cargo.toml` (the `regex` edge); new
`crates/ew-viewmodel/src/find.rs` and its actions, transcript and describe; a new `crates/ew-ui/src/find.rs`;
`docs/book/src/reference/commands/**` and the generated lines of `docs/book/src/SUMMARY.md`.
**Part 3:** a new `crates/ew-commands/src/commands/paste.rs`, `commands/{mod.rs,library.rs}` (`check_copy` shared, no
behaviour change), `schemas/**`, `tests/**`; a new `crates/ew-api/src/output/clipboard.rs` with tests and
`tests/expected/fragment_102F1.txt`; new `crates/ew-viewmodel/src/{clipboard.rs,tools/paste.rs}`; the clipboard glue in
`crates/ew-ui/src/app.rs`; `crates/ew-testkit` (verb, arms, a `parse_fragment` fuzz arm); `bin/elektrawatt/tests/**`;
the generated reference pages.

Shared append-only files (registry lists, `VOCABULARY`, transcript members, `params.rs`, fuzzer arms, DSL verbs) may
be touched by every part and by WP-S2.04's lanes; the second to integrate rebases onto the first. Read-only for the
lanes: every other crate, `golden/**`, `ci/**`, `deny.toml`, `supply-chain/**`, `.github/**`, `docs/**` except the
generated reference pages; `ci/dep-rules.toml` is amended by the design pass before part 1 (§4.1 rule 2), never by a lane. Each lane commits on its own branch and does not merge or push.

## 13. Out of scope

| Item | Where it goes |
|---|---|
| Navigators, table mode, fill-down, pattern fill, Excel paste, the column chooser | WP-S2.06 (ruling, delegated) |
| Message panel, bulk preview tab (replacing the card), history panel, notifications log (keeping the toast's action), jobs panel | WP-S2.09 (ruling, delegated) |
| "Why this value?" (FR-UI-011), "derived would be" for wire overrides, "rule gives" for frozen tags | 1.0; the candidate values a follow-up (§15.4) |
| Editing library-copy fields (FR-DEV-032's manufacturer example), "update from library", the part editor | WP-S2.07 (FR-PRT-022) and a follow-up |
| Paste across grids, the three-way choice for another library version, paste as representation, copy as image, the multi-format clipboard (FR-UI-051) | Follow-ups (§15.4); FR-UI-051 is 1.0 |
| New settable paths (IP name and direction, project master data) | Follow-ups (§15.4): command changes with their own review |
| Locale-aware number display (decimal comma) and Fluent label files | WP-S2.08 |
| Undo coalescing (FT-372), the spatial index (FT-359), the per-page marker mode (FT-375) | Their own dues; not needed by this package |
| Performance measurements of the sheet, find and paste | Release phase (ID-512) |

## 14. Review checklist for the Opus reviewer

1. **Boundaries.** Only the items of §4.2 and the commands of §5 below the shell; exactly the new edges of §4.1 rule 2
   and no new package; nothing under `golden/` changed; no new memoised family; no `ew-model` name in `ew-viewmodel` or
   `ew-ui`.
2. **Schema-driven.** One walker over `kind_schema`; no per-kind panel code; the expected catalogue reviewed, its `Json`
   fallbacks as listed; the derived-row and other-command tables exactly §6.4 and §6.1.3.
3. **Only what commands expose.** Editable rows are exactly `SETTABLE` (and the four settings members); no command gains
   a settable path; read-only rows state their reason.
4. **`HumanOnly`.** The checks member and any `HumanOnly`-routed row are read-only; the view model refuses a non-`Edit`
   command (`reason-human-only`), headless and in the window.
5. **One path.** Every change is preview then commit through the host; the policy's four rules as §6.6; the card's
   stale handling; empty edits commit nothing; no coalescing.
6. **Multi-selection.** Sections by D8; `‹varies›` by canonical JSON; edits on all targets in one command; leaves
   composed into the settable member; per-target values only through `fields.set_many`.
7. **The transaction-read rule.** `fields.set_many` and `edit.paste` read no `Q`; find, copy and the sheet run outside
   transactions.
8. **Fragment and paste (part 3).** The format byte-deterministic; the parser's limits and refusals; mandatory versus
   optional references; library reuse, embedding and the version refusal; structure never created; IDs minted in the
   named order; the grid-step translation exact; frozen tags, counters and wire numbers cleared; undo in one step.
9. **Determinism and privacy.** The named tie-breaks; transcripts byte-identical on two runs; replay equality; no time,
   path, process ID or UUID in the new members and lines.
10. **Accessibility.** The grid, find and paste fully keyboard-operable (the kittest flows without pointer events);
    rows as AccessKit groups; chips and matches not by colour alone.
11. **Scope.** Nothing of §13 half-done; the stand-in card and toast stay minimal.

## 15. Size, order, routing and the documents to update

### 15.1 Size

WP-S2.05 keeps **4.6 pw PERT** (O 3 · L 4.5 · P 6.5); the CSV is unchanged. **Part 1, 2.3 pw:** the walker, routes,
categories and the expected catalogue 0.35; `property_sheet` with sections, `‹varies›`, the derived rows, provenance
and findings 0.5; `parse_field_input`, `format_field_value` and `field_choices` 0.3; the view model (rows, buffer,
composition, policy, card, toast, the multi-language popup) 0.5; the pane, AccessKit and keys in `ew-ui` 0.25; actions,
`select.set`, `resolve_reference`, transcript and describe 0.15; tests, scripts and kittest 0.25. **Part 2, 0.8 pw:**
`fields.set_many` with schema, verb, arm and tests 0.2; `Session::find` 0.25; the find pane, jump and replace through
the card 0.2; freeze on several devices, tests and scripts 0.15. **Part 3, 1.5 pw:** copy and tag list 0.25; parsing
0.15; `edit.paste` 0.6; the paste tool, duplicate, cut and the clipboard glue 0.25; tests 0.25. The parts sum to
**4.6 pw: no reserve.** The PERT predates the gaps of §2.2 (a catalogue, two commands, a format), and the cut already
moves "Copy as image", "Paste as representation", the mixed-grid paste and the version dialog to follow-ups. A part
that closes more than 20 % over its size triggers a re-estimate at the next design pass. **De-scope rule:** when a part
is projected (at its lane's midpoint report) or found (at its review) more than 20 % over its size, the next items in
this order leave the package for follow-ups without a further ruling, until the projection fits: "Copy tag list"
(part 3), then duplicate (part 3), then the freeze on several devices (part 2); the lane reports which ones left.

### 15.2 Order and routing

Part 1 starts after WP-S2.03 part 2's delivery and review; it may run beside WP-S2.04's parts (it touches none of their
commands; shared append-only files rebase). Part 2 starts after part 1. Part 3 starts after part 1 and WP-S2.04 part 1
(the page tables, `Paths`, `Key` events, the grid-step rule, `edit.delete_selection`); its page-item and structure-box
rules apply once WP-S2.04 part 4a has implemented those kinds (before, they are not copyable: §8.3's list follows
`KindStatus`). Parts 2 and 3 may run in parallel. Parts 1 and 2 go to a lead-programmer lane at **D4** (`opus`,
`high`): public items below the shell and a new core command. Part 3 goes to a lead-programmer lane at **D5** (`opus`,
`xhigh`): it defines an interchange format that crosses EW versions and instances, parses untrusted input and remaps
identities, where a mistake is expensive and hard to detect. Each part has an Opus review and is planned as one
lane-cycle plus a fix round.

### 15.3 Proposed decision-log entries (for the design pass; IDs after ID-585)

1. WP-S2.05 specified in three parts with sizes, order and routing, no reserve and the de-scope rule on a > 20 %
   overrun (D1, §15.1, §15.2); the review's fix round and its eight items, ruled (delegated) as the reviewer wrote them.
2. The four delegated rulings: schema-driven from `ew-schema` with no per-kind panels (D2); edits only what commands
   expose (D3); read-only for `HumanOnly` members, the confirmed step separate (D4); WP-S2.06 and WP-S2.09 out, with the
   inline card and the toast as stand-ins (D5).
3. The field catalogue and property sheet as public items of `ew-api` (§4.2, §6.1 to §6.4): the walker's type table,
   the categories, the edit routes, the narrowing table (revision `author` and `checker` read-only, ruling (delegated):
   the closed table may narrow what is editable, never widen it), the other-command table, the five project settings
   rows that exist when absent, `field_catalogue_for` over a registry, the `CommandMeta`/`Exposure` re-exports, the
   closed derived-row table, the mixed-kind rule (D8).
4. The commit path: preview first, the edit policy evaluated on the preview (FR-UI-044's rules), a stale card rebuilt
   from the current revision (refused "changed meanwhile" when the edited member moved; find re-runs its search), the
   toast; one commit per field edit and no coalescing (D6, D7); multi-selection semantics (D10).
5. `fields.set_many` and find and replace: values computed by the caller, always previewed; library-copy fields and
   derived tags found but not replaced; the `regex` edge (D11).
6. The fragment v1 format and `edit.paste` (D12): **`#ew-fragment v1` is the project-fragment format of architecture
   §5.5, which WP-S1.19's macros extend** (ruling, delegated); unknown header members ignored within v1, another
   `format` or record `$v` refused until WP-S1.17's migration chain exists; references remapped and mandatory or
   optional by `ew_model::LINKS` and their delete policy, with the hand list as the test oracle; a child whose parent is
   not copied loses its child scheme; the refusals (another grid, another library version), the structure rebinding
   without creation, the numbering reset, the mint order and the grid-step translation.
7. The headless mode, `select.set`, `resolve_reference`, the headless clipboard and the 72 describe words (53, 13, 6;
   D13; ID-474 extended).
8. The departures from the requirement texts: the chips name the supplying profile (D15: AC-28, FR-UI-010, FR-CON-017,
   doc 16 §6.2); FR-UI-020's 20 and 16 results (D16); FR-DEV-032's example amended to an authored device field (a
   function text or an `Ident` tech-data value), its library example to the part editor (D16); doc 16 §17 item 5 made
   precise (D8).
9. D-06's property-grid entry: PG9 is the G-M2 evidence, pointed to from gates §8.2 and doc 20 §19 by the design pass;
   the runner entry and `golden/` unchanged (§11).
10. **Before part 1 starts:** `ci/dep-rules.toml` (the `"ew-api"` entry gains `ew-units` and `ew-mltext` as normal
    edges) and architecture §3 (the `ew-api` row and the crate graph) are amended by the design pass (§4.1 rule 2).

### 15.4 Proposed follow-up tasks (for the design pass; FTs after FT-388)

1. Candidate values for "derived would be" (a wire property without its level-7 override) and "rule gives" (a frozen
   tag's scheme value) from `ew-derive`, the basis of FR-UI-011's trail.
2. Library-copy fields in find and replace and the property grid (FR-DEV-032's manufacturer example): a previewed
   command that edits a project's part copy, with WP-S2.07's part editor.
3. Paste across connection grids (doc 06 §8 item 3), keeping nm geometry and snapping the insertion point.
4. The three-way choice for another version of a held library copy on paste (update, adapt, keep both; doc 06 §3.6
   item 4, FR-SYM-016).
5. Paste as representation (Ctrl+Shift+V, doc 06 §3.6).
6. Copy as image (PNG through egui's `copy_image` and `ew-render-cpu`), FR-UI-040's third flavour.
7. Settable IP members: `fields.set` gains `function` `ip.name`, `ip.direction`, `ip.sort`, `ip.star_source` (renaming an
   interruption point from the grid).
8. **`project.master.set`, before G-PP:** a command for the project's master data following `project.supply.set`'s
   pattern, with one member `master` (`description`, `document_title`, `drawing_id`), so the grid and the wizard can
   edit what the title blocks print; `parties` stay out (personal data, FR-PRJ-024); the grid's `master` rows then get
   that route.
9. Image items in cross-project fragments (asset bytes in the fragment, under ADR-0008 amendment 5's limits).
10. Revision `author` and `checker` moved from `fields.set` to a `HumanOnly` sign-off command (FR-REV-007, D-34), so
    the four-eyes roles have one confirmed path; the grid's narrowing entry then becomes a `HumanOnly` route.
11. Fragments of an older `format` or record `$v` migrated through WP-S1.17's chain instead of refused (doc 06 §8
    item 15).
12. A note for the design pass, not a lane task: `COLOUR_ALIASES` made `pub` in `ew-import-tables` is acceptable (no
    new edge); its natural home is beside `IecColour` in `ew-schema`, where a later change may move it.

### 15.5 Open questions for the owner

The three questions of the first version are answered (owner-confirmed with the review): the parts fill the PERT with
no reserve and the de-scope rule of §15.1 fires on a > 20 % overrun; the readings of D15 and D16 stand and the design
pass amends doc 02a, doc 02b, doc 16 §6.2 and doc 20's AC-28 text; `project.master.set` is scheduled before G-PP
(§15.4 item 8). None is open.

### 15.6 Documents the design passes update

Named here because D2 to D16 and the fix round change or refine accepted text. **Before part 1 starts:** `ci/dep-rules.toml` (the `"ew-api"` entry) and architecture §3 (the `ew-api` row and the crate graph), §4.1 rule 2 (done in cycle 24, ID-600, with the requirement texts of D15 and D16, architecture §5.5 and §9's chip, gates §8.2, doc 20's AC-28 and §19, doc 16 §6.2, §17 item 5 and §18.1, and doc 15 §5.4). Then: doc 16 §2.1 (`PropRow` and `EditTarget` as built: the sheet,
the catalogue, the routes), §2.2 (the provenance variants produced; `Committed` and `Async` not produced yet), §6.1 to
§6.3 (as built, with the chip texts, the sections and the mixed-kind rule of D8, the read-only reasons), §7.5 (the
policy evaluated on the preview; the inline card until WP-S2.09), §17 item 5 (D8); doc 06 §3.1 (`fields.set_many` and
`edit.paste` as built, the counts), §3.6 (the fragment v1 shape and the paste rules of D12), §3.7, §3.9 (one commit per
field edit), §6 (the keys as built), §8 items 3, 14 and 15 (the grid refusal, the Bulk threshold, the version refusal);
doc 17 §2.1 and §2.3 (the two commands, the counts), §3.5 (the references `select.set` shares); doc 02b FR-UI-010 and
FR-UI-020 (D15, D16), doc 02a FR-DEV-032 (D16: the example becomes an authored device field), FR-CON-017 (D15); doc 20 AC-28's text (D15) and §19's D-06 row (PG9 as
the grid evidence, §11); gates §8.2 (the same pointer); doc 18 §3.17 (what the drafter can edit, find and paste, and the
words); architecture §9 (the property-grid row as built) and §5.5 (`#ew-fragment v1` is its project-fragment format, which WP-S1.19's macros extend); doc 03 §18 or doc 15 (revision `author` and `checker` read-only in the grid, the sign-off follow-up); plan 02
(WP-S2.05's status, the follow-ups), the traceability and the glossary (property sheet, field catalogue, edit route,
project fragment).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S2.03, WP-S2.04, WP-S2.05, WP-S2.06, WP-S2.07, WP-S2.08, WP-S2.09,
  WP-S6.13; Appendix B (FT-152, FT-314, FT-359, FT-366, FT-372, FT-375).
- [Decision log](../06-decision-log.md): ID-290, ID-291, ID-422, ID-433, ID-443, ID-474, ID-493, ID-511, ID-512,
  ID-513, ID-525, ID-537, ID-545, ID-546, ID-557, ID-561 to ID-571.
- [ADR-0004](../../adr/0004-identity-uuidv7-uuidv5.md), [ADR-0005](../../adr/0005-value-types-nm-dec3-mltext.md),
  [ADR-0008](../../adr/0008-transactions-undo-preview.md), [ADR-0011](../../adr/0011-format-versioning-and-migration.md);
  [architecture](../../design/04-architecture.md) §3, §5, §9; [domain model](../../design/03-domain-model.md) §1, §3,
  §5 to §9; [schematic editor](../../design/06-schematic-editor.md) §3, §6, §8; [UX/UI](../../design/16-ux-ui.md) §2, §4,
  §6, §7, §10, §17; [extensibility](../../design/17-extensibility.md) §2, §3.5, §11;
  [quality](../../design/18-quality-security-release.md) §3.17; [golden project](../../design/20-golden-project.md) §19,
  AC-28; [gates](../03-gates.md) §8.2.
- Requirements: FR-UI-007 to FR-UI-011, FR-UI-020, FR-UI-022, FR-UI-025, FR-UI-036, FR-UI-040, FR-UI-044, FR-UI-051,
  FR-DEV-029, FR-DEV-031, FR-DEV-032, FR-CON-017, FR-CON-019, FR-CON-025, FR-REP-036, FR-I18N-001, FR-I18N-011,
  FR-NUM-016, FR-NUM-028, FR-SYM-016, FR-PRT-014, FR-PRT-022, FR-EXT-030, FR-REV-009, NFR-A11Y-01 to NFR-A11Y-03,
  NFR-UX-07, NFR-REL-07, NFR-REL-08, NFR-PERF-20; AC-18, AC-28; D-03, D-06, D-17.
