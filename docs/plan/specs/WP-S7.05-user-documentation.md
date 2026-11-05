# WP-S7.05: User documentation, rule pages and the MVP tutorials (closed specification)

**Status:** complete in cycle 51, built by one lane in two sittings and a resume, with its fix round FIX705
(decision log ID-1362, ID-1364, ID-1370, ID-1376); `build-book.sh`'s Linux run is CI's at the delivery; specified in
cycle 50 (documents only), an M2 package built whole by one lane (ID-1281) in the eight steps
of §6, with one hand-back point (§10); about 6.4 pw by its steps, the PERT of 6.7 pw unchanged · **Date:** 2026-11-04 ·
**Work package:**
[WP-S7.05](../02-work-packages.md#wp-s705-user-documentation-rule-pages-and-mvp-tutorials) · **Decision log:**
ID-1281, ID-1320, ID-1322, ID-1327 · **Part of:** ElektraWatt plan, specifications

**Purpose.** The user guide exists as an offline mdBook in the Diátaxis structure, built and link-checked per pull
request and bundled by WP-S6.06, but it does not yet let a QElectroTech user switch without a trainer: it has one
tutorial (the headless run), no window tutorial, a rule reference with one page per *pack* that carries metadata only,
F1 anchors for 16 topics while most tools, dialogs and panels have none, glossaries only as the generated term table,
no release notes in the book and no Dutch. This document closes the package to decisions, a build order and
acceptance, against the code at `e89345a2` (§2.1). The 1.0 chapters stay WP-S7.07's. Its size is an estimate, not a
commitment (ID-1210); every time is not measured (owner's rule). Where this document and a design document differ,
this document is the narrower and wins for the lane; the design documents stay the target and are updated at the
design pass (§13). A choice the lane meets that this document does not settle is escalated, never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [The texts](#5-the-texts)
6. [Build order](#6-build-order)
7. [Acceptance](#7-acceptance)
8. [Neighbours, files owned and what a lane may not decide](#8-neighbours-files-owned-and-what-a-lane-may-not-decide)
9. [Risks](#9-risks)
10. [Size, routing and the hand-back point](#10-size-routing-and-the-hand-back-point)
11. [Follow-ups, proposed](#11-follow-ups-proposed)
12. [Open questions](#12-open-questions)
13. [Documents to update](#13-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package the bundled guide of 0.1.0 carries its version in its title; every shipped rule ID
has a page of its own (what it checks, why, the standard by number and clause, its Golden A example, the fix and the
waiver guidance), each rule declares the `docs` reference of that page, F1 on a finding opens it, and `docs-gen --check`
fails when a rule lacks its page or a page lacks a section; every tool, dialog and panel of the window has an F1 anchor
that a test resolves; two explanation pages relate EPLAN's and QElectroTech's concepts to EW's, kept in step with the
product glossary by a test; tutorials 1 (a first schematic page with potentials), 2 (a terminal strip and a cable), 4
(reports and PDF) and 6 (importing a QElectroTech project and fixing its migration report) exist, each run as a
scripted test that also checks the menu paths and keys the page names; the headless tutorial runs as a test too; the
book shows the release notes of every release and a how-to on upgrading a project; and a Dutch build of the guide,
from PO files through mdbook-i18n-helpers, holds the introduction and the tutorials in Dutch, opens from F1 when the
window speaks Dutch, and is counted by a translation status report that the release attaches.

**Non-goals** (each with its package or follow-up):
- No 1.0 chapters: tutorials 3 (PLC card and addressing) and 5 (2D/3D layout and the wire list), the German guide,
  the Describe page and the WCAG scan of the built HTML (WP-S7.07, NFR-A11Y-06).
- No hosted, versioned online guide (FP-1); the bundled guide is the release's own build.
- No screenshots or screencasts regenerated from kittest flows (doc 16 §13's accessible documentation, 1.0; §12 O6).
- No company-pack rule pages (`docs/<rule>.md` rendered to a temporary file, doc 15 §3.8): no company pack ships at
  0.1.0 (FP-2).
- No Weblate project or account (the owner's, §12 O1); no German PO file.
- No translation of the generated reference pages (D14): the Dutch build shows them in English.
- No change to the rule engine, its findings or their messages; no new rule.
- No release notes content: `docs/release-notes/0.1.0.md` is WP-S6.08's; this package shows it in the book (D13).
- No new Rust dependency in the workspace; no change to `deny.toml`, `supply-chain/` or `Cargo.lock`.

## 2. Inputs and the code as it is

| Input | What the lane takes from it |
|---|---|
| [Quality](../../design/18-quality-security-release.md) §5.5 | The toolchain as built: the offline rule, `docs-gen`, the link check, the pins, the requirement states |
| [UX/UI](../../design/16-ux-ui.md) §13, §20 | F1's resolution order, opening offline, the anchor registry, Help ▸ Tutorials, MVP tutorials 1, 2, 4 and 6 |
| [Checks](../../design/15-checks-revisions-collaboration.md) §3.7, §3.8, §4 | The `docs` reference of a rule, F1 on a finding, the catalogue's Checks, Req., D (golden) and Def. columns |
| [Non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-DOC-01 to -03, -05, -07, NFR-COMP-05, NFR-I18N-01, -07 | §3 |
| [Core requirements](../../design/02a-requirements-core.md) FR-CHK-015; [extended](../../design/02b-requirements-extended.md) FR-UI-031 | The `docs` reference and its pack test; F1 in the strip editor |
| [ADR-0024](../../adr/0024-licence-set-and-permissive-gpl-boundary.md); [governance](../../design/19-open-source-governance.md) §2.1 L9, L10 | Documentation and rule pages CC-BY-4.0; generated pages keep their source's licence (ID-403); PO files under `book/po/` |
| [Gates](../03-gates.md) G-M2-12, G-M2-23 | The documentation and locale criteria of 0.1.0 |
| Decisions ID-1320 (the Dutch is the orchestrator's), ID-1281, ID-1322, ID-1327 | The Dutch wording and its read-through; one lane; the hand-back limits |
| Follow-ups FT-574, FT-613, FT-827 (this package's); FT-181 (rule messages), FT-281 and FT-282 (Weblate, mdbook-i18n-helpers) | D4, D10, D12, D14, D16 |
| Specifications [WP-S3.09](WP-S3.09-importer-contracts-and-lineage.md) (the import front door and the migration report), [WP-S2.08](WP-S2.08-palette-keymaps-accessibility-localisation.md) (the glossary, the keymaps, the completeness gate), [WP-S6.06](WP-S6.06-pilot-installers-and-platform-matrix.md) D7 (the bundled book) | Tutorial 6; D9, D15; the bundle |

### 2.1 What exists at `e89345a2` (the base contract)

The lane relies on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used for |
|---|---|---|
| The book: `book.toml` (`language = "en"`, `create-missing = false`, offline search, no playground), 13 committed source files, one tutorial (the headless run), the tutorials page's "Planned" section, the explanation page's "glossaries … are planned" | `docs/book/book.toml:8-31`; `docs/book/src/SUMMARY.md:5-8`; `docs/book/src/tutorials/index.md:14-19`; `docs/book/src/explanation/index.md:25-26` | D1, D10 |
| The build: `docs-gen`, then the pinned mdBook 0.5.4 (MPL-2.0, `packaging/components.toml`'s `mdbook`), built in an empty network namespace; `docs-book` runs `docs-gen --check`, the build, `doccheck.py` and `check-book-links.py`, and uploads the book that `package.yml` bundles | `ci/build-book.sh:11-56`; `packaging/components.toml:103-109`; `.github/workflows/docs.yml:136-169` | D2, D14 |
| `docs-gen`: rule pages are one page per pack (`reference/rules/<pack>.md`, a section per rule anchored by its ID), holding the catalogue's metadata only; the generated directories and pages are git-ignored | `xtask/src/docs_gen.rs:31-34`, `:61`, `:88`, `:459`, `:1684`, `:1753` | D3, D4 |
| The catalogue snapshot mirrors `id`, `code`, `pack`, `title`, `kind`, `modes`, severities, `scope`, `supersedes`, `release`, `waivable`, `schemes`, `open_item`; no `docs`, no hint; at the base it lists 80 rules in 8 packs | `xtask/src/docs_gen.rs:1611-1643`; `crates/ew-checks/catalogue/rules.json`; `crates/ew-checks/tests/catalogue.rs:60` | D4, D6 |
| `RuleMeta` has no `docs` member | `crates/ew-checks/src/rule.rs:180-204` | D6 |
| Every registered rule has `rule-<id>` with `.title` and `.hint` in en-GB and nl-NL | `crates/ew-checks/src/messages.rs:4-12`; `crates/ew-checks/messages/` | D4 |
| Golden A's expected findings per rule (EF rows: rule, count, subjects, targets, defect) | `crates/ew-api/tests/expected/findings_mvp.csv` | D4 |
| F1: 16 topics; `action_topic` maps eleven actions, the workspace actions and the command bar; `context_topic` maps the wizard, first run, palette and eight dialogs, and gives the project view for every other dialog; `rule_page` gives the pack page and the ID anchor; tests resolve every anchor and every shipped rule's anchor | `crates/ew-viewmodel/src/help.rs:35-115`, `:125`, `:147-183`, `:189-203`, `:277`, `:322` | D3, D8 |
| A help request opens `<docs_dir>/<target>` as a `file://` URL, one language | `crates/ew-viewmodel/src/shell/onboarding.rs:166-176`; `bin/elektrawatt/src/main.rs:439` | D14 |
| Help ▸ Tutorials makes a fresh copy of the sample project in a folder the user names and opens the tutorials page; without the sample it opens the page only | `crates/ew-viewmodel/src/shell/projects.rs:180-223` | D10 |
| The product glossary: one `[[term]]` per EW term with `eplan` and `qet` aliases, labels and definitions in en-GB and nl-NL, starting with NFR-DOC-05's terms; `docs-gen` renders `reference/glossary.md` from it | `crates/ew-viewmodel/glossary/glossary.toml:1-18` | D9 |
| The completeness gate: every shipped UI locale has every en-GB key | `crates/ew-viewmodel/tests/it/locales.rs:73-75` | D15 |
| No PO file, no `po/` folder; FT-281 (Weblate) and FT-282 (mdbook-i18n-helpers) open | `docs/book/README.md:64-69`; FT-281, FT-282 | D14, D16 |
| No test runs the headless tutorial's command lines | `docs/book/src/tutorials/headless-workflow.md` (no test names it) | D11 |
| The only QElectroTech project file in the repository is the network sandbox's fixture | `ci/sandbox/fixture.qet` | D10 |
| Licences: `docs/**` CC-BY-4.0; the generated reference pages GPL-3.0-or-later (the format pages MIT OR Apache-2.0) | `REUSE.toml:43-80`; `ci/licence-map.toml:382-423` | D18 |

**What is missing for NFR-DOC-02** (the brief's "say what is missing"): a page per rule ID (today a section of a pack
page); each rule's declared `docs` reference (FR-CHK-015); on the page, the rule's title and hint (they exist in the
Fluent messages, not in the snapshot), what it checks, why, the standard by number and clause, the Golden A example,
the quick fix and the waiver guidance; a check that fails when any of these is missing; F1 landing on the rule's own
page.

## 3. Requirements traced

| Requirement | State and what this package delivers | Acceptance |
|---|---|---|
| NFR-DOC-01 | **Built in part:** the Diátaxis mdBook, built and link-checked per PR, offline, bundled (WP-S6.06). Here: versioned per release (D2), an F1 anchor for every tool, dialog and panel (D8) | DC-3, DC-4 |
| NFR-DOC-02 | A page per shipped rule ID with D4's sections; CI fails without it (D3 to D5); F1 opens it (D6) | DC-1 |
| NFR-DOC-03 (MVP part) | Tutorials 1, 2, 4 and 6, each a scripted test (D10, D11) | DC-6, DC-7, DC-8 |
| NFR-DOC-05 | The EPLAN ↔ EW and QET ↔ EW concept pages (D9); the review in the usability test is WP-S2.12's | DC-5 |
| NFR-DOC-07 (the guide's half) | The release notes of each release and the upgrade how-to in the book (D13); the notes' content is WP-S6.08's | DC-10 |
| NFR-COMP-05 (the guide's half) | Standards cited by number and clause on the rule pages, with a form check and no quotation (D7); the content audit is WP-S6.08's RC-07 | DC-2 |
| FR-CHK-015 | Every rule declares `docs`; the pack test fails on an unresolved one (D6) | DC-1 |
| FR-UI-031 (the guide's half) | F1 in the strip editor opens its page offline; check messages link their rule page (D6, D8) | DC-3 |
| NFR-I18N-07 (MVP part) | PO files through mdbook-i18n-helpers; Dutch tutorials; English complete (D14); Weblate is the owner's (§12 O1) | DC-11 |
| NFR-I18N-01 (the report) | The translation status report over the UI locales and the guide (D15); the UI translation itself is WP-S2.08's | DC-12 |
| FT-574 | The parts how-to (D12) | DC-9 |
| FT-613 | Tutorial 1 is the beginner drawing tutorial (D10) | DC-6 |
| FT-827 | The guide in the installed tree holds the tutorials (D10 with WP-S6.06's bundle; WP-S6.08's FT-1267 proves it is found) | DC-13 |

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **The book's shape.** The four Diátaxis sections stay. New pages: tutorials 1, 2, 4 and 6 (`tutorials/first-page.md`, `terminal-strip-and-cable.md`, `reports-and-pdf.md`, `import-qet.md`), the tutorials page's list replaced by the six available tutorials with what each needs; how-to `choose-parts.md` (D12) and `upgrade-a-project.md` (D13); reference `window/` with `tools.md`, `dialogs.md`, `panels.md` (D8) and the generated `release-notes/` (D13); explanation `eplan-and-ew.md` and `qet-and-ew.md` (D9). The introduction links each section's new pages | The requirement lists; one place per kind of text |
| D2 | **Versioned per release.** `ci/build-book.sh` builds with `MDBOOK_BOOK__TITLE="ElektraWatt User Guide <version>"`, the version read from `[workspace.package].version` of `Cargo.toml`, so every page of the built book names its release; the installed guide is the release's own build. No hosted online versions (FP-1) | NFR-DOC-01's "versioned per release" for an offline bundle; mdBook's own configuration override, no preprocessor |
| D3 | **A page per rule ID.** `docs-gen` writes `reference/rules/<id in lower case>.md` for every rule of the catalogue snapshot and keeps `reference/rules/<pack>.md` as the pack's index (its rules with title, kind, severity and scope, each linking its page) and `reference/rules/index.md` as now. `SUMMARY.md` nests each rule's page under its pack's page. F1's target becomes `reference/rules/<id>.html` (D6); the headless tutorial's two links are updated | NFR-DOC-02 says "a page"; F1 lands on the rule, not inside a list of thirty |
| D4 | **A rule page's content**, in this order: the title (from `.title`); **What it checks**, **Why**, **Standard** (D7), from the rule's notes (D5); **In the golden project**, generated from `findings_mvp.csv` (each EF row of the rule with its count, subjects and the defect it detects) followed by the notes' example text, or "Golden A has no finding of this rule." when it has none; **How to fix**: the `.hint` line, then the notes' quick fixes; **Waiving**: "This rule's findings can be waived." or "… cannot be waived." from `waivable`, then the notes' guidance; then the metadata table of today. The catalogue snapshot gains `hint` (the en-GB `.hint`, written by `ew-checks`' catalogue test; the mirror struct gains the field). FT-181's page half closes with it | Every item NFR-DOC-02 names; the generated parts are the ones a source already holds, so they cannot drift |
| D5 | **The rule notes.** One hand-written file per shipped rule, `docs/book/rule-notes/<id in lower case>.md` (CC-BY-4.0, L9; outside `src/`, so mdBook never reads it alone), with the headings `## What it checks`, `## Why`, `## Standard`, `## Example`, `## How to fix`, `## Waiving` in this order, each non-empty (`## Example` may say only what to change in a copy of Golden A to see the rule). `docs-gen --check` fails on a shipped rule without its notes (T1), a missing heading (T2) and a notes file for no shipped rule (T3). The text comes from doc 15 §4's row and its Def. document; a rule's behaviour is described as built, never beyond it. A new rule's lane writes its notes (CONTRIBUTING's "Pull request checklist" gains the line; `docs_gen.rs` gains the list's `map:` marker) | One prose source per rule, checked per pull request; the generated part stays generated |
| D6 | **The `docs` reference (FR-CHK-015).** `RuleMeta` gains `docs: &'static str`, `"reference/rules/<id in lower case>"`, declared by every rule; the catalogue snapshot carries it; `ew-checks`' pack test fails on a rule whose `docs` is not that form for its own ID (T5); `docs-gen` fails on a `docs` that names no page it writes. `help::rule_page` reads `meta.docs` (its `map:` marker kept), so F1 on a finding or a rule row opens the rule's page | The requirement's "declare … that resolves", checked at both ends; a convention without a declaration is not what it asks |
| D7 | **Standards by number and clause (NFR-COMP-05).** Each line of `## Standard` is a citation `<body> <number>[:<year>], <clause>` with an optional note in brackets (for example "IEC 60204-1:2016, 13.2.2 (unverified)"), or "No standard: a project-hygiene rule." The notes hold no block quotation, no table copied from a standard and no figure. A rule whose data ships `verified = false` says so in **Standard** (EW-POT-006's PE table). `docs-gen --check` checks the form (T4); the clause itself is read at WP-S6.08's RC-07 audit | The rule the requirement sets, checked where it can be (the form) and read where it cannot (the clause) |
| D8 | **F1 for every tool, dialog and panel.** Three hand-written reference pages, `reference/window/tools.md`, `dialogs.md`, `panels.md`, one heading per item (what it does, how to open it, its keys from the keymap's sheet page, the related tutorial or how-to). `help.rs` gains a topic per item: every action whose ID starts with `tool.`, every `Dialog` variant (an exhaustive `match` with no wildcard, so a new dialog fails to compile until it has its topic), and every pane and navigator kind (the Messages pane, the property grid, the history, the jobs panel, the find pane, each navigator, the strip editor, the table mode); F1 in each resolves its topic. The existing tests resolve every anchor against the book | NFR-DOC-01's acceptance names tools and dialogs; FR-UI-031's names the strip editor; an exhaustive match keeps the list complete without a hand-kept count |
| D9 | **The glossaries (NFR-DOC-05).** Two explanation pages, `explanation/eplan-and-ew.md` (function, device, part, CDP, interruption point, basic project, form, message management) and `explanation/qet-and-ew.md` (element, master/slave, folio report, conductor), one section per concept: the other program's term, the EW term (linked to its row of `reference/glossary.md`), what is the same, what differs, and where to find it in the window (a menu path, checked as D10's lint checks a tutorial's). `docs-gen --check` fails when an `eplan` or `qet` alias of `glossary.toml` has no section in its page, or a section names an EW term that `glossary.toml` lacks (T7). The lane may add a missing term to `glossary.toml` (both languages) | Concept pages a switching user reads, not only an alias table; the test keeps them and the palette's aliases in step |
| D10 | **Tutorials 1, 2, 4 and 6.** Each: a goal, what you need, numbered steps with the menu path or key of each, the visible result after each step, and what you learnt. **1 A first schematic page with potentials** (FT-613): File ▸ New project from the basic project, a page, a supply and two loads placed, wires drawn, the potentials defined (the existing how-to's method), the potentials navigator and the Messages pane read; ends with a saved project and no error. **2 A terminal strip and a cable**: continues tutorial 1's project (or its script's result): a strip with terminals, the field side wired, a cable with its cores, the strip editor and the cable list read. **4 Reports and PDF**: on Help ▸ Tutorials' copy of the sample project: report pages (terminal diagram, cable list, parts list) added, the pages exported as PDF, the print preview read. **6 Importing a QElectroTech project**: a small QElectroTech project written for the tutorial (D18), in the guide's folder `tutorials/files/`, imported through File ▸ Import QElectroTech project… (WP-S3.09's front door and preview), its migration report read in the Messages pane under the `migration` scheme, and its findings fixed until the report shows the expected remainder. Each tutorial has a script `docs/book/tutorial-scripts/<page name>.ewui` (`ew.uiscript/1`), and a test in `bin/elektrawatt/tests/` runs it headless on a fresh configuration folder, asserts every step `done` and the end state named in its page (the findings by rule and count, the pages, the strip's terminals, the PDF's page count), and checks the page: its numbered steps match the script's actions in order, and every menu path and key the page names equals the step's action's en-GB label path and its binding in the default keymap (T6). Help ▸ Tutorials stays as built | NFR-DOC-03's list and its "scripted CI test"; the lint makes the page fail when the window's words or keys change, as the keymap rule asks; tutorial 2 building on 1 is the order a beginner takes |
| D11 | **The headless tutorial is run too.** A test in `bin/ew/tests/` runs each `console` block's `ew` command of `tutorials/headless-workflow.md` on a copy of Golden A and compares the output lines the page shows (whole, where it shows them) | NFR-DOC-03 says every tutorial; the page claims its command lines were run |
| D12 | **FT-574: the parts how-to** `how-to/choose-parts.md`: choose a part on the Parts row of the property grid, place a terminal from its part, read the parts list; the page lint of D10 checks its menu paths | The row as written |
| D13 | **Release and migration notes in the book.** `docs-gen` writes `reference/release-notes/index.md` and one page per `docs/release-notes/<X.Y.Z>.md`, newest first, each relative link rewritten to the repository's address as the other generated pages link the repository; `unreleased.md` and `m1-draft.md` are not shown; with no release the index says "No release yet." The hand-written how-to `upgrade-a-project.md` says, as built: what a release reads and writes (doc 18 §7.1), that an older format migrates in memory and the first save asks and writes the backup beside the project, that a newer or unreleased format opens read-only, `ew upgrade`, and that from 0.1.0 the format changes only additively or with a migration (ADR-0011 amendment 6) | The notes stay one file per release, written by WP-S6.08; an offline user reads them in the guide |
| D14 | **The Dutch guide.** **mdbook-i18n-helpers** (Apache-2.0; its `mdbook-gettext` preprocessor and `mdbook-xgettext` renderer), pinned in `packaging/components.toml` at the newest release that builds and runs with mdBook 0.5.4 (version and SHA-256 recorded by the lane, §12 O2), installed in `ci/build-book.sh` before the network-less build. `docs/book/po/messages.pot` holds the hand-written pages and the table of contents only, made with the generated pages absent (on a temporary copy of the book source), so no generated text enters a PO file and a new command does not stale the template; `docs/book/po/nl.po` translates the introduction, the tutorials page and the tutorials (D10, D11), the rest falls back to English. `ci/build-book.sh` builds the Dutch book into `<dest>/nl/` (`MDBOOK_BOOK__LANGUAGE=nl`); the link check covers it. A help request in a window whose UI language is nl-NL opens `<docs_dir>/nl/<target>` when it exists, else the English page. The template is refreshed by `ci/build-book.sh --pot`; the `docs-book` job warns, not fails, when it is stale; WP-S6.08's RC-06 refreshes it. The Dutch wording is the orchestrator's (ID-1320): the lane writes it, the orchestrator reads it through before the merge. Doc 18 §5.5's offline rule reads "no preprocessor or plugin but the pinned `mdbook-gettext`" | NFR-I18N-07 names the tool; the generated reference changes with every command and rule and is GPL text, so it stays out of translation at 0.1.0 |
| D15 | **The translation status report.** `ci/translation-status.py` (standard library; `--test` its offline unit test) writes a Markdown table per locale: the UI (the share of en-GB keys present in each locale's `.ftl` files, and the core-flow share, which WP-S2.08's gate makes the same set), the rule messages (`ew-checks`' `.ftl`), and the guide (translated and total messages of each PO file, and of the tutorials alone); it exits non-zero when nl-NL's UI is below 95 % or a tutorial message is untranslated. WP-S6.08 attaches its output to the release (RC-06). Weblate's statistics replace it when a Weblate project exists | NFR-I18N-01's and NFR-I18N-07's evidence without an account; one script for both |
| D16 | **Weblate.** The repository layout is Weblate-ready (`docs/book/po/*.po` with the template, the `.ftl` files per locale); creating the project and connecting it are the owner's (§12 O1, FT-281). FT-282 closes with D14 | An account is the owner's; nothing here waits for it |
| D17 | **Tutorial 6 and WP-S3.09.** Tutorial 6 describes the import as WP-S3.09 builds it; when that package is not merged when the lane reaches step 6, the step leaves as a follow-up without a further ruling, and the tutorials page lists tutorial 6 as "follows" (G-M2-23 asks for tutorials 1, 2 and 4 only, tutorial 6 following QET import under lever (b)) | The tutorial describes what ships, never a spike |
| D18 | **Licences.** The pages, the rule notes, the PO files, the scripts and the tutorial's QElectroTech project are CC-BY-4.0 under `docs/**` (L9, the map's existing row, §12 O3); the generated rule and release-notes pages keep the licence of their sources (the rule pages GPL-3.0-or-later, carrying the CC-BY-4.0 notes, which that licence admits; the release-notes pages CC-BY-4.0); `REUSE.toml` and `ci/licence-map.toml` gain the release-notes path | ID-403's rule, each page its source's licence |
| D19 | **Every item closed;** no decision changes an ADR | — |

*As built (2026-11-05, cycle 51; decision log ID-1362, ID-1364, ID-1370).* D10, tutorial 6: the import is
**Project ▸ Import QElectroTech project…**, the window's menu path, not File ▸ Import, which the page names as built
(ID-1364); its QElectroTech file gives the supply a PE pin, since a two-pin black-box supply counts as a load (a row).
D10, tutorials 2 and 4, written on FT-1371's and FT-1372's window actions (ID-1362, ID-1370): tutorial 2 builds the
strip with **Draw ▸ Place ▸ Terminal strip…**, the cable with **Draw ▸ Connect ▸ New cable…** from the sample library's
five-core part and **Connect cores…** on the selected field wires, reads the Cables navigator as its cable list (a
fresh project holds no report block) and freezes the cable's tag, since a cable without a drawn definition line
prints `-?W?` (a row); tutorial 4, on the Help ▸ Tutorials copy, adds the two report kinds the sample lacks (a parts
list and a terminal overview) with **Output ▸ Report block…** and reads the terminal diagrams, cable overview and
parts list it already holds, so D10's "terminal diagram, cable list, parts list added" would have duplicated them;
the tutorial tests set `EW_DATA_DIR`, since the starter set holds no cable part (a row). D14's explicit `<a id>`
anchors are read by `docs/plan/tools/doccheck.py` (ID-1364 named it `ci/doccheck.py`; the file is the plan's tool),
which joined §8.2's files for that one change (ID-1364).

## 5. The texts

Every text is asserted whole. The `docs-gen` and lint texts are English (developer output); the guide's Dutch is the
orchestrator's (ID-1320).

| # | Where | Text |
|---|---|---|
| T1 | `docs-gen --check`, a shipped rule without notes | `docs/book/rule-notes/<id>.md is missing: every shipped rule needs its notes (NFR-DOC-02)` |
| T2 | `docs-gen --check`, a missing or empty heading | `docs/book/rule-notes/<id>.md: the section "## <heading>" is missing or empty` |
| T3 | `docs-gen --check`, a stale notes file | `docs/book/rule-notes/<file> names no shipped rule` |
| T4 | `docs-gen --check`, a citation not in D7's form | `docs/book/rule-notes/<id>.md:<line>: cite a standard by number and clause only, as "<body> <number>[:<year>], <clause>" (NFR-COMP-05)` |
| T5 | `ew-checks` pack test, an unresolved `docs` | `<rule>: docs "<docs>" is not the rule's page reference/rules/<id> (FR-CHK-015)` |
| T6 | Tutorial and how-to lint | `<page>:<line>: "<text>" is not how the window names <action> ("<expected>")` |
| T7 | `docs-gen --check`, the glossaries | `docs/book/src/explanation/<page>: the <EPLAN or QElectroTech> term "<term>" of the product glossary has no section` |
| T8 | The book's index without a release | `No release yet.` |

## 6. Build order

One lane builds the eight steps in this order and commits at the end of each. Steps 1 to 5 and 7 need nothing of
WP-S3.09; step 6 needs it merged (D17).

1. **The toolchain** (D2 to D7): the version in the title; `RuleMeta.docs` for every rule, the snapshot's `docs` and
   `hint`, the pack test (T5); `docs-gen`'s rule pages and pack indexes, the notes reader and its checks (T1 to T4),
   their unit tests on fixture trees; `help::rule_page` from `docs`; the headless tutorial's links; the `map:` marker
   and CONTRIBUTING's line.
2. **The rule notes** (D5, D7): one file per shipped rule, pack by pack (EW-TERM and EW-POT first, the packs with
   safety rules); `docs-gen --check` green.
3. **F1 everywhere** (D8): the three window reference pages; the topics, the exhaustive dialog match and the tests.
4. **The glossaries** (D9): the two pages, any term added to `glossary.toml`, the check (T7).
5. **Tutorials 1, 2 and 4, the parts how-to and the headless test** (D10 to D12): the pages, the scripts, the tests and
   the lint (T6); the tutorials page; FT-613 and FT-574.
6. **Tutorial 6** (D10, D17): the QElectroTech project, the page, the script and its test.
7. **Release notes and upgrading** (D13): the generated pages with their unit test, the how-to, `REUSE.toml` and the
   licence map.
8. **The Dutch guide and the report** (D14 to D16): the pin, the template, `nl.po` with the introduction and the
   tutorials, the Dutch build and its link check, F1 in Dutch, `ci/translation-status.py` with `--test`; the Dutch
   listed for the orchestrator's read-through.

## 7. Acceptance

- **DC-1** (NFR-DOC-02, FR-CHK-015) Every rule of the catalogue snapshot has `reference/rules/<id>.md` with D4's
  sections in order (an `xtask` test over the real snapshot and notes, with no total pinned); fixture trees give T1, T2
  and T3 whole; a rule with a wrong `docs` gives T5 in `ew-checks`' pack test; F1 on an EW-CORE-033 finding resolves
  `reference/rules/ew-core-033.html` (`ew-viewmodel`); `docs-gen --check` passes on the tip.
- **DC-2** (NFR-COMP-05) A fixture note with a quoted sentence or a citation without a clause gives T4; the notes hold
  no block quotation; RC-07's audit (WP-S6.08, manual) reads the **Standard** sections.
- **DC-3** (NFR-DOC-01, FR-UI-031) Every `tool.` action, every `Dialog` variant and every pane and navigator kind has a
  topic whose anchor exists in the book (`ew-viewmodel` tests; the match is exhaustive); F1 with the strip editor
  focused opens its page; `docs-book` builds both languages offline and `check-book-links.py` passes on both.
- **DC-4** (NFR-DOC-01) The built book's `index.html` title carries the workspace version (a check in
  `ci/build-book.sh`, run by `docs-book`).
- **DC-5** (NFR-DOC-05) The two pages have a section for every alias of `glossary.toml`; removing one gives T7 (an
  `xtask` test); the review by users is WP-S2.12's R1 (manual).
- **DC-6** (NFR-DOC-03, FT-613) Tutorials 1, 2 and 4 run as tests: every step `done`, the end state as the page states
  it; a page step whose menu path differs from the registry's label path gives T6 (a fixture page); frames of each
  tutorial's last step at 1280 × 800 and 1920 × 1080 for the window check.
- **DC-7** (NFR-DOC-03) Tutorial 6 runs as a test on WP-S3.09's import: the migration report's counts before and after
  the fixes as the page states them; or, under D17, FT-n opened and the page listed as "follows".
- **DC-8** (NFR-DOC-03) The headless tutorial's commands run and print the lines the page shows.
- **DC-9** (FT-574) The parts how-to passes the lint.
- **DC-10** (NFR-DOC-07) A fixture `docs/release-notes/9.9.9.md` gives its page with rewritten links (an `xtask`
  test); without a release T8; the upgrade how-to passes the lint.
- **DC-11** (NFR-I18N-07) `nl.po` translates every message of the introduction, the tutorials page and the tutorials
  (the report's tutorials share is 100 %); the Dutch book builds offline; with the UI in Dutch and a `docs/nl/` folder,
  F1 opens the Dutch page, and without the folder the English one (`ew-viewmodel` tests); the orchestrator's
  read-through of the Dutch is recorded (named manual check, ID-1320).
- **DC-12** (NFR-I18N-01, NFR-I18N-07) `python -I ci/translation-status.py --test` passes; its run on the tip prints
  en-GB and nl-NL at 100 % for the UI and the tutorials.
- **DC-13** (FT-827) The bundled guide (the `docs-book` artefact that `package.yml` takes) holds the four tutorials and
  `tutorials/files/`; WP-S6.08's IN-15 and FT-1267 prove it on the installed tree.
- **DC-14** The hand-back checks of the lane's package set pass (fmt, settle, clippy, doc, nextest, policy), and
  `python -I docs/plan/tools/doccheck.py` with the built pages present.

## 8. Neighbours, files owned and what a lane may not decide

### 8.1 Neighbours

| Neighbour | It owns | This package |
|---|---|---|
| WP-S3.09 | The import front door, the preview and the migration report | Tutorial 6 describes and scripts them (D17) |
| WP-S7.03 | The basic project, the starter parts and profiles | Tutorials 1 and 2 start from the shipped basic project and parts |
| WP-S2.08 | The glossary file, the keymaps, the completeness gate, the Dutch terminology table | D9 may add terms; D10's lint reads the keymap; D15 reports the gate's result |
| WP-S6.06 | The bundle of the built book in every installer | Gives it the Dutch folder and the tutorial files |
| WP-S6.08 | `docs/release-notes/0.1.0.md`, RC-06, RC-07 | Shows the notes (D13); gives the report (D15) and the notes' citations to the audit |
| WP-S2.12 | R1, which reviews the glossaries (NFR-DOC-05's acceptance) | Gives the pages |
| A lane that adds a rule | Its rule | Writes the rule's notes (D5) |

### 8.2 Files owned

`docs/book/**` (the pages, `rule-notes/`, `tutorial-scripts/`, `po/`, `book.toml`, `README.md`); `xtask/src/docs_gen.rs`
and its tests; `crates/ew-checks/src/rule.rs` and the rule declarations (the `docs` member only),
`crates/ew-checks/tests/catalogue.rs`, `crates/ew-checks/catalogue/rules.json` (regenerated); `crates/ew-viewmodel/src/help.rs`,
`src/shell/onboarding.rs` (the Dutch folder) and their tests; `crates/ew-viewmodel/glossary/glossary.toml` (terms
added only); `bin/elektrawatt/tests/` (the tutorials' test), `bin/ew/tests/` (the headless tutorial's test);
`ci/build-book.sh`, `ci/translation-status.py`, `packaging/components.toml` (the one component),
`.github/workflows/docs.yml` (the `docs-book` steps); `REUSE.toml` and `ci/licence-map.toml` (the release-notes rows);
`CONTRIBUTING.md` (one checklist line). Read-only: every other crate, `golden/**`, `docs/release-notes/**`,
`deny.toml`, `supply-chain/`, `Cargo.lock`, every other document.

### 8.3 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a rule's behaviour as built differs from its doc 15 row or its Def. document in a way the page would have to state
   (the page describes what is built; the difference is a finding);
2. a standard's clause cannot be named from the design documents and the research notes (the citation stays
   "unverified", or the rule is reported);
3. mdbook-i18n-helpers has no release that builds and runs with mdBook 0.5.4, or it needs network at build time (§12
   O2);
4. a tutorial step needs an action, a text or a behaviour the window does not have, or a script needs a hook only tests
   have;
5. a change would reach a rule's findings, a message text, the keymap or a neighbour's files beyond §8.2.

The lane decides freely: the pages' wording within D1 to D13, the headings' order inside a how-to, the tutorials' sample
values, the QElectroTech project's content, the scripts' encoding, the report's layout, and the nl-NL wording (listed
for the orchestrator).

## 9. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The rule notes take longer than sized, or state a clause wrongly | Medium · medium | D7's form check and RC-07's read; "unverified" is allowed; the de-scope rule (§10) does not touch them |
| R2 mdbook-i18n-helpers does not support mdBook 0.5.4 | Medium · medium | §8.3 item 3 and §12 O2's fallback |
| R3 The window changes after the tutorials are written | High · low | The lint (T6) and the scripted tests fail and name the step; the tutorials are written after WP-S2.17's polish |
| R4 WP-S3.09 is not merged at step 6 | Medium · low | D17 |
| R5 A rule lands in a parallel lane without its notes | Medium · low | T1 fails that lane's `docs-gen --check`; D5's checklist line |

## 10. Size, routing and the hand-back point

**Size.** About 6.4 pw: step 1 0.6, step 2 1.3, step 3 0.7, step 4 0.3, step 5 1.4, step 6 0.4, step 7 0.4, step 8
1.3. The PERT of 6.7 pw (O 4.5 · L 6.5 · P 9.5) stays. **De-scope rule:** when the lane is projected more than 20 %
over, these leave as follow-ups without a further ruling, in this order: the headless tutorial's test (D11), the
warning on a stale template (D14), the panels page of D8 (the tools and dialogs stay).

**Routing** (`.claude/orchestration.md` rule 0): one lane, **Opus at `high`** (the rule pages cite standards by clause,
and the toolchain touches `ew-checks`, `ew-viewmodel`, `xtask` and the book build); dispatched after WP-S7.03 and
WP-S3.09 are merged (§12 O4), with WP-S0.06's and WP-S1.16's built parts.

**Hand-back point** (ID-1322). The lane hands back at the end of **step 5**, the tutorials committed, when it then
stands above about 550k tokens; one that nears 800k anywhere commits and hands back. Its hand-off note then holds: the
steps done with their commits; the rule notes written and the packs left; the topics added and any dialog or pane
still without a page; each tutorial's script, test name and asserted end state; whether tutorial 6 waits on WP-S3.09;
the mdbook-i18n-helpers version found or not; the test filters (`-E 'test(/docs_gen/)'` in `xtask`, `ew-checks`'
`catalogue` and pack tests, `ew-viewmodel`'s `help` tests, `elektrawatt`'s tutorials test, `ew`'s headless tutorial
test); and every choice of §8.3's free list already made.

## 11. Follow-ups, proposed

The design pass opens the rows. **FT-574**, **FT-613**, **FT-827** and **FT-282** close with steps 5, 5, 5 and 8;
**FT-181**'s page half with step 1.
- **FP-1** A hosted, versioned online guide (GitHub Pages or a domain), one folder per release.
- **FP-2** Company-pack rule pages rendered to a temporary file (doc 15 §3.8), with the first company pack.
- **FP-3** Screenshots and screencasts regenerated from kittest flows with alternative text (doc 16 §13, NFR-A11Y-06,
  1.0).
- **FP-4** Help ▸ Tutorials offering each tutorial's own start (an empty project for tutorial 1, tutorial 1's result for
  tutorial 2, the QElectroTech file for tutorial 6) instead of the one sample copy.
- **FP-5** The generated reference in the Dutch guide, once its source texts are translatable.

## 12. Open questions

- **O1** (the owner's) The Weblate project for the guide and the UI (FT-281): an account and a project connected to
  the repository. Options: (1) the owner creates a Hosted Weblate project (its free plan for open-source projects) that
  proposes translations as pull requests; (2) no Weblate at 0.1.0: the PO and `.ftl` files are edited in the
  repository, the Dutch by the orchestrator (ID-1320), and G-M2-12's evidence is D15's report; Weblate comes with
  German at 1.0. Recommendation: (2); nothing in 0.1.0 needs outside translators, and NFR-I18N-07's 0.1.0 part then
  reads "PO files in the repository" (a ruling).
- **O2** mdbook-i18n-helpers' version is not verified against mdBook 0.5.4 (this document fetches nothing). Options:
  (1) the lane pins the newest release that builds and runs with 0.5.4, recorded with its SHA-256; (2) if none does,
  the Dutch introduction and tutorials are separate Markdown pages in `docs/book/src/nl/` with the PO route at FP-5's
  time. Recommendation: (1), with (2) ruled only at the stop of §8.3 item 3.
- **O3** The licence of the tutorial pages, their scripts and the sample QElectroTech file. ADR-0024 names tutorials and
  examples CC0-1.0, while `docs/**` is CC-BY-4.0 in the licence map and the existing tutorial is CC-BY-4.0. Options:
  (1) CC-BY-4.0 by the map's row, ADR-0024's row read as the sample projects; (2) CC0-1.0 for `docs/book/src/tutorials/`
  and `tutorial-scripts/`, with two map rows. Recommendation: (1).
- **O4** The dispatch. Options: (1) after WP-S7.03 and WP-S3.09 are merged, so tutorials 1, 2 and 6 describe the shipped
  basic project and import; (2) earlier, with D17's de-scope taking tutorial 6. Recommendation: (1).
- **O5** The size: 6.4 pw by the steps, the PERT of 6.7 pw unchanged. Recommendation: keep it.
- **O6** Screenshots at 0.1.0. Options: (1) none: text with menu paths and keys checked by the lint (FP-3 at 1.0); (2)
  committed frames from the tutorial scripts' visual runs, re-made on every window change. Recommendation: (1).

## 13. Documents to update

At the design pass after the lane: doc 18 §5.5 (the offline rule's one preprocessor; rule pages per ID with their
notes; the version in the title; the Dutch build; the requirement states of NFR-DOC-01, -02, -03, NFR-I18N-07 as
built); doc 16 §13 (Help ▸ Tutorials as built with one sample copy, FP-4; F1 in Dutch); doc 15 §3.7 and §3.8 (the
`docs` reference's form `reference/rules/<id>`, the notes file); `docs/book/README.md` (layout and translation, by the
lane); doc 19 §2.1 L9 and L10 (the notes, the release-notes pages, `book/po/` as `docs/book/po/`); ADR-0024's tutorials
row (§12 O3); 02c NFR-I18N-07 (the 0.1.0 part, §12 O1); doc 03 gates G-M2-12's evidence (D15's report while no
Weblate exists); doc 02's WP-S0.06 status (FT-282 closed).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S7.05, WP-S7.07, WP-S0.06, WP-S1.16, WP-S3.09, WP-S7.03, WP-S2.08,
  WP-S2.12, WP-S6.06, WP-S6.08; FT-181, FT-281, FT-282, FT-574, FT-613, FT-827.
- [Decisions](../decisions.md): ID-1210, ID-1281, ID-1320, ID-1322, ID-1327; [decision log](../06-decision-log.md):
  ID-402 to ID-404.
- [Quality](../../design/18-quality-security-release.md) §5.5, §7.1; [UX/UI](../../design/16-ux-ui.md) §13, §20;
  [checks](../../design/15-checks-revisions-collaboration.md) §3.7, §3.8, §4;
  [non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-DOC-01 to -07, NFR-COMP-05, NFR-I18N-01,
  NFR-I18N-07; [core requirements](../../design/02a-requirements-core.md) FR-CHK-015;
  [extended requirements](../../design/02b-requirements-extended.md) FR-UI-031;
  [governance](../../design/19-open-source-governance.md) §2.1.
- [ADR-0011](../../adr/0011-format-versioning-and-migration.md) amendment 6,
  [ADR-0024](../../adr/0024-licence-set-and-permissive-gpl-boundary.md).
- [Gates](../03-gates.md) G-M2-12, G-M2-23; the specifications of [WP-S3.09](WP-S3.09-importer-contracts-and-lineage.md),
  [WP-S2.08](WP-S2.08-palette-keymaps-accessibility-localisation.md) and
  [WP-S6.06](WP-S6.06-pilot-installers-and-platform-matrix.md).
