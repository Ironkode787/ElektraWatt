# WP-S7.03: Starter parts, profiles, address maps and the basic project (closed specification)

**Status:** complete in cycle 51, built whole by one lane and its fix round FIX51p3 (decision log ID-1354,
ID-1375); SC-9 is CI's (`package.yml`) at the delivery; specified in cycle 50 (documents only), an M2 package built
whole by one lane (ID-1281) in the six steps
of §6, with one hand-back point (§11); about 2.5 pw by its steps, the PERT of 2.6 pw unchanged · **Date:** 2026-11-04
· **Work package:**
[WP-S7.03](../02-work-packages.md#wp-s703-starter-parts-profiles-address-maps-and-basic-project) · **Decision log:**
ID-812, ID-1281, ID-1322, ID-1325, ID-1327 · **Part of:** ElektraWatt plan, specifications

**Purpose.** A new project should be correct by default, but half of the data that makes it so is code, not content.
The generic starter parts ship (WP-S2.18), and so do two of the three colour profiles and the four QET-compatible forms.
The sample project's colour profile "DBU/WH 24 V" and its size profile "0.75/1.5", however, are JSON constants in
`ew-viewmodel`. The two address profiles are Rust statics in `ew-plcaddr`, and the kind `plc_profile` is reserved. The
parts of the sample project exist only as Golden A's project-local copies, and those copies carry no licence. No test
shows that Golden A reproduces from the shipped defaults and a basic project made with the wizard: the built-in basic
project writes the larger new-project text styles of ADR-0032, which Golden A does not have. This document closes the
package to decisions, a build order and acceptance, against the code at `e89345a2` (§2.1). Its size is an estimate,
not a commitment (ID-1210); every time is not measured (owner's rule). Where this document and a design document
differ, this document is the narrower and wins for the lane; the design documents stay the target and are updated at
the design pass (§14). A choice the lane meets that this document does not settle is escalated, never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [The texts](#5-the-texts)
6. [Build order](#6-build-order)
7. [Acceptance](#7-acceptance)
8. [Golden A's expected changes](#8-golden-as-expected-changes)
9. [Neighbours, files owned and what a lane may not decide](#9-neighbours-files-owned-and-what-a-lane-may-not-decide)
10. [Risks](#10-risks)
11. [Size, routing and the hand-back point](#11-size-routing-and-the-hand-back-point)
12. [Follow-ups, proposed](#12-follow-ups-proposed)
13. [Open questions](#13-open-questions)
14. [Documents to update](#14-documents-to-update)
15. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package:
- **Licences.** Every shipped symbol, function definition, part, manufacturer, form and macro record carries an SPDX
  licence and a source. This covers the records of `content/` and the copies in the sample project. A per-PR test
  refuses a record without them.
- **The sample parts.** The catalogue parts of the sample project are a CC0 system library, `ew-sample-parts`. Each
  part has the dimensions Golden A gives it and, where Golden A's functions use it, a function template and a CP
  pattern. The PLC and
  bus sections, the part address maps, are copied with the confidence of their source. Every map expression and
  designation pattern of a shipped part parses.
- **Colour and size profiles.** "DBU/WH 24 V" and "0.75/1.5" are content files beside the two shipped colour profiles.
- **Address profiles.** The TIA and IEC address profiles are versioned library documents of kind `plc_profile`. The
  built-in two are compiled in from `content/`. A profile file put into a library is chosen by `plc.set_profile`
  without a code change, and the project then holds its copy.
- **NFR-UX-08.** A test runs the company-standard wizard with the pilot's choices, makes a project from the basic
  project it writes, and finds Golden A's effective conventions in it, one by one.

**Non-goals** (each with its package):
- No manufacturer packs, no vendor catalogue data, no vendor file of any kind (WP-S7.08; NFR-LIC-06).
- No new symbols and no change to the generic starter parts or to `ew-starter-parts` 0.3.0 (WP-S7.04; WP-S2.18).
- No forms editor, no title-block cell mapping (FT-365), no report forms with data areas (§13 O1).
- No colour profile that selects by circuit class, and no verification of the shipped standard profiles against
  licensed copies (FT-388).
- No window chooser for an address profile (§12 FP-2), no tag-based profiles (Rockwell, 1.0).
- No part linter at import (FT-351 keeps its import half; this package checks the shipped content only).
- No new Rust dependency; no change to `deny.toml`, `supply-chain/` or `Cargo.lock`.

## 2. Inputs and the code as it is

| Input | What the lane takes from it |
|---|---|
| [Symbols, parts, macros](../../design/07-symbols-parts-macros.md) §2.8, §3.8, §9.1, §9.2 | The basic project's contents; the licence table; the starter set and the golden part data |
| [PLC and bus](../../design/10-plc-and-bus.md) §2.5, §2.6, §3.1 | The profile's members, the IEC profile's differences, the part PLC sections and the map language |
| [Connections](../../design/08-connections-terminals-cables.md) §4.2, §4.3 | The profile rules, the shipped colour columns, the size profiles' state |
| [Golden project](../../design/20-golden-project.md) §4.4, §7.1, §7.2, §11.2, §13, §17, AC-40 | Golden A's settings, the two company profiles, the station's maps, the test dimensions (▲U-26), the forms |
| [Reports and forms](../../design/09-reports-forms-output.md) §2.5, §5 | `FormDef`; the shipped forms |
| [UX/UI](../../design/16-ux-ui.md) §9.2 | The wizard's steps and defaults; NFR-UX-08's wording there |
| [Open-source governance](../../design/19-open-source-governance.md) §3.1 | LR-06, LR-07, LR-10: what ships and under which licence |
| [Requirements](../../design/02a-requirements-core.md), [02b](../../design/02b-requirements-extended.md), [02c](../../design/02c-nonfunctional-requirements.md) | FR-CON-020, FR-CON-021, FR-PLC-008, NFR-LIC-06, NFR-LIC-07, NFR-UX-08, NFR-COMP-05 |
| ADR-0032 decision 3; ADR-0024; ADR-0011 | New projects' text styles; the content licences; the kind registry |
| Decisions ID-812 (the generic parts moved to WP-S2.18), ID-577 (the wizard's default profile), ID-150 (built-in defaults), ID-1325 (the format changes freely until 0.1.0) | The scope; the defaults that stay |
| FT-592, FT-805 (both closed: the briefs' parts and the layout parts), FT-351, FT-365, FT-388 | What is done; what stays open beside this package |

### 2.1 What exists at `e89345a2` (the base contract)

The lane relies on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used for |
|---|---|---|
| The generic starter parts: 23 part records and the manufacturer "Generic", all `CC0-1.0`, library 0.3.0, generated by a test and compiled in | `content/libraries/ew-starter-parts.ewlib/library.json:5`; `crates/ew-library/tests/it/starter_parts.rs:1-16`, `:36`, `:750`; `crates/ew-library/src/starter_parts.rs:1-6`; `crates/ew-library/src/lib.rs:48-56` | D3 (untouched) |
| The system tier: `content/libraries/*.ewlib` of the installation, read by the window and by `ew lib` through `EW_CONTENT_DIR`; the release ships `content/` as it is | `bin/elektrawatt/src/main.rs:438`, `:467-478`; `bin/ew/src/cli.rs:3355-3364`; `packaging/stage.py:26-27`; `crates/ew-library/src/registry.rs:488` | D3, D7 |
| A library's record directories: `fndefs`, `forms`, `macros`, `parts`, `symbols` | `crates/ew-library/src/record.rs:19-25` | D7 |
| Golden A's 29 part copies and 8 manufacturer copies carry `meta.sources` (`golden.table:parts\|<key>`) and no `meta.licence`; its symbol, function-definition and form copies carry both. The tables emitter writes the part meta | `golden/J-E-2416.ew/lib/parts/`, `lib/manufacturers/`; `crates/ew-import-tables/src/emit/parts.rs:400-410` | D1 |
| Golden A's part tables: 29 parts (4 `TBD` test parts, 2 configured order codes), CP patterns for 5 parts, templates for 5, one PLC row (533361); the dimensions are test inputs | `golden/J-E-2416/tables/parts.csv`, `part_cps.csv`, `part_templates.csv`, `part_plc.csv`; doc 20 §13 (▲U-26) | D3, D4 |
| Two shipped colour profiles, `verified: false`; the company profiles "DBU/WH 24 V" and "0.75/1.5" are constants in code | `content/profiles/README.md:12-13`; `crates/ew-viewmodel/src/wizard_data.rs:15`, `:18`, `:42-51` | D5, D6 |
| The wizard's choices: colour `sample`, `en60204-machine`, `iec60445-installation`, `default`; size `sample`, `default` with the two cross-section fields | `crates/ew-viewmodel/src/wizard.rs:236`, `:244-262`, `:705-737` | D5, D6, D11 |
| The four QET-compatible forms (CC-BY-SA-4.0, with sources), compiled into the wizard; a divider takes the frame defaults' `section_divider` entry, else the no-grid frame (ID-144) | `content/forms/`; `crates/ew-viewmodel/src/wizard_data.rs:24-29`; `crates/ew-api/src/output/view.rs:29-33`, `:258-260` | D9 |
| The address profiles: a struct of `&'static str` members, the statics `SIEMENS_TIA_INTL` and `IEC_61131_3` (both `2027.1`), `builtins()` and `builtin()` | `crates/ew-plcaddr/src/profile.rs:19-52`, `:330`, `:351`, `:371-379` | D7 |
| Every reader of a profile | `crates/ew-derive/src/plc.rs:97-105`, `:112`, `:152`, `:186-197`; `crates/ew-checks/src/rules/plc.rs:185`, `:254`, `:352`, `:709`; `crates/ew-commands/src/commands/plc.rs:329`, `:406-415`; `crates/ew-commands/src/commands/fields.rs:632`; `crates/ew-commands/src/normalise/plc_aspects.rs:149` | D7, D8 |
| The kind `plc_profile` is reserved ("1.0"); `lib/plc-profiles` is a library directory of the format; `LibDocument` has no variant for it | `crates/ew-schema/src/kind.rs:120-121`, `:315`; `crates/ew-format/src/path.rs:35-43`; `crates/ew-schema/src/record.rs:520-535`; `schemas/0.1/index.json:404-410` | D7 |
| The map language and the PLC section | `crates/ew-plcaddr/src/map.rs`; doc 10 §2.6 | D4 |
| New projects: the built-in basic project (the identifier set, `new_project_text_styles`, one page), the copy of a basic-project directory, `save_as_basic` | `crates/ew-api/src/new_project.rs:8-21`, `:142`, `:266-311`; `crates/ew-schema/src/text_style.rs:117` | D10, D11 |
| NFR-UX-08 in miniature: the wizard's default draft leaves the sample's folio 102 unchanged | `crates/ew-viewmodel/tests/part2.rs:806-848` | D10 |
| Golden A's settings hold `colour_profile`, `formatting`, `identifier_set`, `numbering` and `size_profile`; no `text_styles` and no frame defaults (its pages name their forms) | `golden/J-E-2416.ew/project.jsonl:1` | D10 |
| The licence rows: `content/**` CC-BY-SA-4.0, `ew-starter-parts.ewlib/**` CC0-1.0 | `REUSE.toml:116-127`; `ci/licence-map.toml:350-356` | D1, D3 |
| AC-40's installed acceptance (WP-S6.06 D12) and RC-07, the content audit | `ci/installed-acceptance.sh:5-17`; `.github/workflows/package.yml:379`; `docs/release/checklist.md:24` | D2 |

## 3. Requirements traced

| Requirement | State and what this package delivers | Acceptance |
|---|---|---|
| FR-PLC-008 | Built as code (TIA and IEC, WP-S1.21); here as versioned data, a profile in a library chosen without a code change (D7, D8) | SC-6, SC-7 |
| FR-CON-020 | Built (three profiles, the wizard); here "DBU/WH 24 V" as content (D5) | SC-4 |
| FR-CON-021 | Built as a constant; here "0.75/1.5" as content (D6) | SC-4 |
| NFR-LIC-06 | The sample parts are the project's own facts, no vendor file, each record's provenance; the content test and RC-07's inventory (D1 to D3) | SC-1, SC-2 |
| NFR-LIC-07 | Every shipped record with an SPDX licence and a source, refused per PR otherwise (D1) | SC-1 |
| NFR-UX-08 | Golden A's conventions from the shipped defaults and a wizard-made basic project, member by member (D10, D11) | SC-8 |
| AC-40 | Built (WP-S6.06's installed acceptance); here the sample's copies carry their licence (D1) and the run stays green | SC-9 |

## 4. Decisions

Each is a lead ruling that the owner may reverse. The format changes freely until the 0.1.0 release (ID-1325): a
changed schema is re-blessed, with no migration.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **A licence and a source on every shipped record.** A test of `ew-library`, `content_audit`, reads every library document under `content/` and every copy under `golden/J-E-2416.ew/lib/` as bytes (never through `Store::open`: Golden A is only read). It fails, naming the file and the item, on a missing or empty `meta.licence`, a licence other than its kind's (doc 07 §3.8: CC0-1.0 for parts and manufacturers, CC-BY-SA-4.0 for symbols, function definitions, forms and macros; the record's licence, not its file's, so Golden A's CC0 tree holds CC-BY-SA symbol copies), or an empty `meta.sources` (§5 T4). Under `content/`, a file's row in `ci/licence-map.toml` must also allow its records' licence. The tables emitter writes `meta.licence: "CC0-1.0"` on the part and manufacturer copies of the golden bootstrap only. A user's `ew import parts` keeps writing none: its licence is the user's to state. Path (b), the command DSL, gives the same copies (AC-15) | NFR-LIC-07's PR test, over the two places that ship records; Golden A is shipped as the sample (AC-40), so its copies count |
| D2 | **The content inventory for RC-07.** `content/README.md` (new) lists each shipped content set: folder, kind, licence, source, `verified` where the kind has it, and the package that made it. The audit test keeps the list complete: a folder under `content/` without a row fails. RC-07's evidence is the list, `NOTICE` and the test's run on the release commit | The release manager signs RC-07 from one page instead of a tree walk; the test keeps the page in step |
| D3 | **The sample parts library.** A new system library `content/libraries/ew-sample-parts.ewlib`: key `ew-sample-parts`, name "Sample project parts", version 0.1.0, CC0-1.0, ID `v5(SYSTEM_LIBRARY, "ew-sample-parts")`. It is not compiled in: the installation's system tier reads it, like any `.ewlib` there. A generator test, `sample_parts`, beside `starter_parts`, writes it with the library writer from Golden A's part and manufacturer copies, read as bytes. It takes Golden A's catalogue parts: the 29 less the four `TBD` test parts and the two configured order codes, which are one machine's configurations, not catalogue items. It takes their manufacturers and three things of its own: a function template and a CP pattern for each part whose device in Golden A has functions on starter function definitions, from those functions and the CP designations they print (for example 1, 2, N, N on A9P52610, doc 07 §9.2); the PLC and bus sections as Golden A holds them (D4); and per-section confidence (D4). Golden A keeps its project-local copies unchanged | The window's picker and Parts row then offer the sample's parts in a new project; reading Golden A's own copies keeps one source; a project-local copy is not re-pointed, so Golden A does not churn |
| D4 | **The part address maps and their confidence.** The PLC sections (533361: `bit_of`, the solenoid-position map H1, `unbound`), the bus ports (195740) and the supply groups and earth pin (550208) go into the sample parts as Golden A holds them. Each part's `meta.confidence` names, per section, `test` where doc 20 gives the value as a test input (the dimensions, ▲U-26; H1, ▲U-23; the FE pin, ▲U-18) and `unverified` for the rest; nothing is `verified`, because no source was checked against the maker's data. The audit test (D1) parses every `bit_of` and `channel` with `ew_plcaddr::MapExpr` and every designation pattern with `ew_derive::expand_pattern` over every shipped part, the generic ones included, and fails with the reason of `PatternError` | The address maps are the package's "part address maps"; the confidence marks every finding from them as based on unchecked data (doc 10 §2.6); the check is FT-351's content half |
| D5 | **"DBU/WH 24 V" as content.** `content/profiles/colour/company.dbu-wh-24v.json` holds the value of `COLOUR_PROFILE_SAMPLE`, byte for byte as the setting stores it; the constant becomes `include_str!` of the file. `content/profiles/README.md` gains its row (basis: company convention, `verified: true`, the profile of the sample project). The wizard's choices are unchanged | FR-CON-020's company profile becomes shipped data like the other two; the existing test against Golden A's `project.jsonl` still holds it |
| D6 | **"0.75/1.5" as content.** `content/profiles/size/company.0.75-1.5.json` holds `SIZE_PROFILE_SAMPLE` the same way, and the README gains a "size" row. No standard-based size profile ships at 0.1.0 (§13 O4): its cross-sections would need a licensed copy of the standard to choose and to mark verified, and the built-in defaults (ID-150) stay code | FR-CON-021's profile becomes data; a profile that cites a standard by number and clause with values no one checked would add a choice and no knowledge |
| D7 | **Address profiles as library documents** (§13 O5's option 1). The kind `plc_profile` is implemented: a typed `PlcProfileDef` in `ew-schema` with doc 10 §2.5's members in JSON (`profile` = `{id, version}`, `label`, `absolute`, `base`, `unit_bits`, `word_align`, `mnemonics`, `render`, `parse`, `allocation`) and the item meta; `LibDocument::PlcProfile`; `plc-profiles` among a library's record directories; the schema in `schemas/0.1` re-blessed. `ew-plcaddr`'s `AddressProfile` takes owned members and one validating constructor; its statics go. The built-in two are `content/plc-profiles/siemens-tia-intl.json` and `iec-61131-3.json` (CC-BY-SA-4.0 with `content/**`), compiled into `ew-derive` and parsed once. The pin stays `id@version` (`siemens-tia-intl@2027.1`), so Golden A's configuration project is unchanged. A profile's item version is the library's semver; its `profile.version` is the pin, and a change in what it renders is a new `profile.version`, never an edit of a published one | FR-PLC-008's "versioned data" and "a new profile file … selectable without a code change"; one parse path for the built-ins and a user's file; doc 10 §2.5's library copy, so a project renders without the installation that made it (NFR-REL-11) |
| D8 | **Choosing a profile.** `profile_of` resolves a pin from the project's `plc_profile` copies first, then the built-ins. `plc.set_profile` accepts a built-in or a profile the project holds; the front ends (`ew apply`, the palette) embed a library's profile through `library.embed` before the command, as `part.assign` takes its closure. `shipped_address_profiles` becomes "the profiles at hand": the built-ins, then the project's copies. A profile file that does not validate (D7's constructor) is refused by `library.embed` and left out of the profiles at hand, with §5 T2, and never reaches a project | The command stays pure over the model; the embed path exists; a broken file is stopped at the door, not at render |
| D9 | **The forms.** The four QET-compatible forms stay as they are (published content, unchanged versions); the audit test covers them. A divider form and report forms with data areas are §13 O1: recommended for after 0.1.0 | The forms that Golden A uses ship and are licensed; the rest changes Golden A's output and doc 20 for one page kind |
| D10 | **NFR-UX-08's proof.** A test of `ew-viewmodel`, `golden_from_the_basic_project`, runs `tools.company_standard` with the pilot's choices (the defaults, and D11's choice "As in the sample project"), finishes into a fresh directory, makes a project from it with `file.new`, and compares its **effective** conventions with Golden A's: identifier set; numbering schemes and the strip rule; cross-reference format; colour and size profiles (whole values); the effective text-style table; date and number formatting; the frame of each page type (Golden A: the form its pages of that type name; the basic project: its frame defaults); sections (number, folio range, divider); structure aspect signs; revision label format; check scheme; derivation profile; report blocks by kind, first folio, form and title. The test names its exclusions in one table, each with its reason: parties and master data (the project's own); the per-strip and per-device report blocks 903 to 905 (their selection names project records); the title-block cell mapping (FT-365). A member that differs and is a value the wizard sets is fixed in the wizard's default. A member with no wizard field and no command is a stop | Golden A built "from the defaults and the basic project only" (02c), as the effective settings that print, not as JSON members; the two may differ in form (frame defaults against per-page forms) and agree in output |
| D11 | **The text-size choice** (§13 O3's option 1). Step 2 of the wizard (Frame and title block) gains the field "Pin numbers and markers" with two choices: "Readable (new projects)" (the default: the built-in basic project's `text_styles`, ADR-0032 decision 3) and "As in the sample project", which writes no `text_styles` member (the base table that Golden A prints). It is written through the existing `project.settings.set` (`text_styles`); no command is added | Without it no basic project from the wizard reproduces Golden A, and NFR-UX-08 fails; the default keeps ADR-0032's legibility for every new user |
| D12 | **Every item closed;** no decision changes an ADR. D7 implements a kind that ADR-0011's registry already holds; doc 05 is updated at the design pass (§14) | — |

*Amended (2026-11-05, cycle 51; decision log ID-1375).* D8 named two orders: `profile_of` the project's copies
first, "the profiles at hand" the built-ins first. One order holds everywhere: the project's own copy of a pin first,
then the built-in, `profiles_at_hand` following `profile_of` (the copy takes the built-in's place, the list's order
kept), so a project whose copy carries a built-in's pin prints, sets and parses addresses in one notation. D10's
exclusions also hold report block 908's title and the location nodes, which are project data (ID-1354).

## 5. The texts

Every text is asserted whole (en-GB); the nl-NL texts are the orchestrator's (ID-1320), written by the lane and listed
for the orchestrator's read.

| # | Where | Text |
|---|---|---|
| T1 | `plc.set_profile`, a profile neither built in nor held by the project | `` `<text>` is not an address profile of this build or of the project; the profiles at hand are <list> `` |
| T2 | A library's `plc_profile` that does not validate (the library listing and `ew lib`) | `the address profile <key> cannot be read: <reason>`; reasons: `its bit template has no {bit}`, `its unit template has no {byte}`, `an area mnemonic is empty`, `only base 10 is rendered`, `only 8 bits per unit are rendered` |
| T3 | The wizard's step 2 (the `wizard-field-*` and choice keys of `strings.rs`) | Field `Pin numbers and markers`; choices `Readable (new projects)` and `As in the sample project` |
| T4 | The audit test's failures (test output, asserted whole in its own unit test) | `<path>: <item key>: no licence`, `<path>: <item key>: the licence <spdx> is not allowed here (<allowed>)`, `<path>: <item key>: no source`, `<path>: <item key>: <member> = "<expr>" is no map expression: <reason>`, `content/<folder>: not listed in content/README.md` |

## 6. Build order

One lane builds the six steps in this order and commits at the end of each.

1. **The licences** (D1, D2): the audit test, the emitter's `meta.licence` for the bootstrap, Golden A's copies
   re-blessed (§8), `content/README.md`.
2. **The company profiles** (D5, D6): the two files, `include_str!`, the README rows.
3. **The sample parts** (D3, D4): the generator, the library, its manufacturers, templates, CP patterns, sections and
   confidence; the map check over every shipped part; the REUSE and licence-map rows; the system tier finds it.
4. **The address profiles** (D7, D8): the typed kind and its schema, the library directory, the two content files,
   `AddressProfile`'s owned form, every reader of §2.1, `plc.set_profile` and T1, T2.
5. **The basic project** (D10, D11): the wizard's field with its frames at 1280 × 800 and 1920 × 1080, the test.
6. **The release notes:** the entries of `docs/release-notes/unreleased.md` (the shipped profiles, the library, the
   kind `plc_profile`, the wizard's field).

## 7. Acceptance

- **SC-1** (NFR-LIC-07) `content_audit` passes on the repository; given a fixture library whose part lacks
  `meta.licence`, and one whose form has `CC0-1.0`, it fails with T4's texts whole; Golden A's 29 part and 8
  manufacturer copies carry `CC0-1.0`.
- **SC-2** (NFR-LIC-06, RC-07) `content/README.md` lists every folder of `content/`, and the audit fails on a fixture
  folder without a row; no shipped record names a vendor file in its sources.
- **SC-3** (D3, D4) `sample_parts` regenerates `ew-sample-parts.ewlib` byte for byte (a second bless changes nothing);
  the library holds 23 parts and their manufacturers, none of them a `TBD` part or an order code; each part with a
  template places on its starter symbol in a new project (a placement test per such part); 533361's PLC section
  derives Q0.0 … Q0.7 on a module in a new project; every section's confidence is `test` or `unverified`; the map
  check fails on a fixture part with `bit_of = "n-1"` with `PatternError`'s reason.
- **SC-4** (FR-CON-020, FR-CON-021) the two profile files equal the values in Golden A's `project.jsonl`, and the
  wizard's `sample` choices write them unchanged.
- **SC-5** (D3) in the window, a new project's Parts row and the place tool's part picker list a sample part (frames at
  1280 × 800 and 1920 × 1080); the Libraries navigator lists "Sample project parts" in the system tier.
- **SC-6** (FR-PLC-008) both built-in profiles parse and round-trip every address of Golden A's 56 channels (AC-29
  stays green); Golden A's files and every output are byte-identical.
- **SC-7** (FR-PLC-008) a fixture library with a third absolute profile (`%Q0.1`-style, its own `profile.id`) is
  embedded and chosen by `plc.set_profile` with no code change; the configuration project's channels then render in
  it; the project, reopened without the library, still renders them; T1 and T2 are asserted whole.
- **SC-8** (NFR-UX-08) `golden_from_the_basic_project` passes with its exclusion table as D10 names it; the wizard's
  step 2 shows T3 (frames at both sizes); with "Readable (new projects)" the project carries the built-in text styles.
- **SC-9** (AC-40) the delivery's `package.yml` run is green: the installed sample exports the committed PDF digest and
  CSVs.
- **SC-10** The hand-back checks of the lane's package set pass (fmt, settle, clippy, doc, nextest, policy, trailers).

## 8. Golden A's expected changes

- **Step 1:** Golden A's 29 files under `lib/parts/` and 8 under `lib/manufacturers/` gain `"licence": "CC0-1.0"` in
  their `meta`, so their `sha256` members and `manifest.json` change. Every exported output stays byte-equal: the
  PDF's digest in `expected/pdf/sha256.txt`, the PNGs, the text and report CSVs, the acceptance report. Expected files
  that quote a part copy line (`crates/ew-api/tests/expected/fragment_102F1.txt` and its siblings) are re-blessed in
  the same commit. The commit carries the `Golden:` trailer of the brief's ID.
- **Steps 2 to 6:** no Golden A change. A change there is a stop.

## 9. Neighbours, files owned and what a lane may not decide

### 9.1 Neighbours

| Neighbour | It owns | This package |
|---|---|---|
| WP-S2.18 | The generic starter parts and `ew-starter-parts` 0.3.0 | Reads them (the map check); changes nothing |
| WP-S1.19 | The library tiers, the `.ewlib` reader, `library.embed`, the wizard's backend | Adds a record directory and a kind; uses `library.embed` |
| WP-S1.21 | The PLC model, `plc.set_profile`, the derivation | Moves the profiles to data behind the same functions |
| WP-S7.01 | The golden tables and fixture | The emitter's licence member, re-blessed |
| WP-S6.06 / WP-S6.08 | The staged tree and the release checklist | `content/` ships as it is; RC-07 reads D2's page |
| WP-S7.08 | Manufacturer packs | Nothing of a maker's data ships here |

### 9.2 Files owned

`content/README.md` (new); `content/libraries/ew-sample-parts.ewlib/` (new); `content/plc-profiles/` (new);
`content/profiles/colour/company.dbu-wh-24v.json` and `content/profiles/size/` (new); `content/profiles/README.md`;
`crates/ew-library/` (`record.rs`, the `content_audit` and `sample_parts` tests); `crates/ew-import-tables/src/emit/`
(the bootstrap's licence); `crates/ew-schema/` (`PlcProfileDef`, `LibDocument`, `kind.rs`) and `schemas/0.1/`;
`crates/ew-format/` (the kind's read and write); `crates/ew-plcaddr/src/profile.rs`; `crates/ew-derive/src/plc.rs`;
`crates/ew-checks/src/rules/plc.rs` and, in `crates/ew-commands/src/`, `commands/plc.rs`, `commands/fields.rs` and
`normalise/plc_aspects.rs` (the readers); `crates/ew-viewmodel/src/{wizard.rs,wizard_data.rs}`, `strings/`,
`locales/` and its tests; `crates/ew-ui/` for the field's frames; `golden/J-E-2416.ew/lib/` (re-blessed by
`ew-api`'s `golden_fixture` test only, never opened as a project for writing); the expected files of §8;
`REUSE.toml` and `ci/licence-map.toml` (the CC0 row of the new library); `docs/release-notes/unreleased.md`.
Read-only: `golden/J-E-2416/tables/`, `content/libraries/ew-starter-parts.ewlib/`, `content/forms/`, the two shipped
colour profiles, `deny.toml`, `supply-chain/`, `Cargo.lock`, every other document.

### 9.3 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a published library item or content file would have to change (`git show main:<path>` differs from what the step
   needs);
2. a part's value would come from anything but Golden A's copies and doc 20, or a value would be marked `verified`;
3. Golden A changes beyond §8;
4. a convention of D10 differs and neither a wizard default nor D11's field can carry it;
5. a text would differ from §5, or a new dependency seems needed.

The lane decides freely: the generator's code and the item IDs (`v5` of the library and the key, as WP-S2.18 does);
which Golden A function gives a part's template when several do (the first by tag); the JSON layout of
`PlcProfileDef` within D7's members; the test names beyond those given; and the nl-NL wording (listed).

## 10. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The templates derived from Golden A give a part a pin list that differs from the drawing in another project | Medium · low | Confidence `unverified`; the unverified-data marker on findings; FP-1 replaces them with checked data |
| R2 Typing `plc_profile` reaches more readers than §2.1 lists | Low · medium | The grep of §2.1 is the brief's reader list; a further reader is updated in step 4 and named in the report |
| R3 The effective-convention comparison finds a difference that only a new command can fix | Medium · medium | §9.3 item 4: a stop, with the member and its value |
| R4 Shipping real part numbers with test dimensions misleads a layout | Medium · low | `test` confidence (D4), the layout's unverified marker, §13 O2 |

## 11. Size, routing and the hand-back point

**Size.** About 2.5 pw: step 1 0.3, step 2 0.15, step 3 0.6, step 4 0.9, step 5 0.45, step 6 0.1. The PERT of 2.6 pw
(O 1.6 · L 2.5 · P 3.8) stays. **De-scope rule:** when the lane is projected more than 20 % over, these leave as
follow-ups without a further ruling, in this order: SC-5's Libraries-navigator line, SC-7's reopen-without-library
case (the embed stays), the placement test per sample part (one part per symbol kind stays).

**Routing** (`.claude/orchestration.md` rule 0): one lane, **Opus at `high`** (a format kind, a permissive crate's
public type, a golden re-bless, a wizard field). WP-S1.19 and WP-S1.21 are complete.

**Hand-back point** (ID-1322). The lane hands back at the end of **step 4** when it then stands above about 550k
tokens; one that nears 800k anywhere commits and hands back. Its hand-off note then holds: the steps done with their
commits; Golden A's re-blessed files and the trailer used; the sample library's part list and each template's source
function; the `PlcProfileDef` members as written and the schema diff; every profile reader changed and any left (with
why); the texts T1 and T2 as built; what step 5 needs (the wizard field's key names, the exclusion table so far); the
test filters (`-p ew-library --test it -E 'test(/^(content_audit|sample_parts|starter_parts)::/)'`, `ew-derive`'s
and `ew-commands`' PLC tests, `ew-api`'s `golden_fixture` and `acceptance`, `ew-viewmodel`'s wizard tests); and every
choice of §9.3's free list already made.

## 12. Follow-ups, proposed

The design pass opens the rows.
- **FP-1** The pilot's further parts through the pilot's company library (the parts import, WP-S1.19 part 4), and
  checked data for the sample parts with the first manufacturer pack (WP-S7.08).
- **FP-2** A window chooser for a configuration project's address profile (Properties), listing the profiles at hand.
- **FP-3** Standard-based size profiles once the cross-sections are checked against licensed copies (with FT-388).
- **FP-4** The divider form and report forms with data areas (if §13 O1 keeps option 2).
- **FP-5** A ready-made sample basic project offered by the new-project flow (if §13 O6 keeps option 2).

## 13. Open questions

- **O1** The divider and report forms of doc 09 §5. Options: (1) ship `ew.frame.qet-a4-landscape-divider` after doc 20
  §17.3 (rules, the section's designation and subtitle, the logo cell) and name it in Golden A's frame defaults: a
  Golden A change on folios 100, 200 and 900 (PDF digest, three PNGs and text files, `frame.json`) and a doc 20 §4.4
  row through the tables script, about 0.3 pw more; (2) keep dividers on the no-grid frame (ID-144) and the house
  report table in code for 0.1.0 (FP-4). Recommendation: (2); the change is cosmetic, wide in Golden A, and late.
- **O2** The pilot-essential parts beyond Golden A. Options: (1) 0.1.0 ships Golden A's catalogue parts only (D3), and
  the pilot brings its own parts through its company library (FP-1); (2) the orchestrator supplies a list of part
  numbers from the private corpus and the lane adds them with data the project writes itself. Recommendation: (1);
  option 2 needs data no public source holds, and the corpus never enters the repository.
- **O3** The text sizes and NFR-UX-08. Options: (1) D11's wizard field, its default the readable styles of ADR-0032;
  (2) no field: text styles become NFR-UX-08's one named exclusion, and 02c's row gains a note. Recommendation: (1);
  it meets a Must row with one field and changes no default.
- **O4** A standard-based size profile. Options: (1) ship "EN 60204-1 machine" sizes, the project's own values citing
  clause 12.4 by number, `verified: false`; (2) none at 0.1.0 (D6, FP-3). Recommendation: (2).
- **O5** User address profiles at MVP. Doc 05 §2.2 dates `plc_profile` copies at 1.0, while FR-PLC-008 (MVP) asks
  that a new profile file be selectable without a code change. Options: (1) D7 and D8 as written: the kind implemented
  now, and doc 05 updated; (2) the built-ins as data only, the user-file half moved to 1.0 with an amendment of
  FR-PLC-008's criterion (about 0.4 pw less). Recommendation: (1); the requirement row is the binding text, and the
  format is free until 0.1.0.
- **O6** A ready-made basic project in the release. Options: (1) ship the wizard's output as
  `content/basic-projects/sample-company.ew` and offer it in the new-project flow (a visible item); (2) none: the
  wizard with its defaults makes it, and D10 proves it. Recommendation: (2) at 0.1.0 (FP-5).
- **O7** The size: about 2.5 pw against the PERT's 2.6. Recommendation: keep the PERT.

## 14. Documents to update

At the design pass after the lane: doc 05 §2.2 and line 135 (`plc_profile` implemented at MVP, the built-ins compiled
from `content/plc-profiles/`); doc 10 §2.5 (the profile as a JSON library document, the pin and the item version,
built-ins and user copies as built); doc 08 §4.3 (the company profiles shipped as content; no standard size profile at
0.1.0); doc 07 §3.8 and §9.2 (the sample parts library, its confidence, the excluded parts); doc 16 §9.2 step 2 (the
text-size field; NFR-UX-08 as proven); doc 20 §4.4 (text styles as the sample's base table; a note on the effective
conventions); 02c NFR-UX-08 (the proof's form, the named exclusions); doc 19 §3.1 LR-06 (the inventory page as RC-07's
evidence); ADR-0032's consequences (the wizard's choice).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S7.03, WP-S2.18, WP-S1.19, WP-S1.21, WP-S7.01, WP-S7.08; FT-592,
  FT-805, FT-351, FT-365, FT-388.
- [Decisions](../decisions.md): ID-1210, ID-1281, ID-1320, ID-1322, ID-1325, ID-1327;
  [decision log](../06-decision-log.md): ID-150, ID-577, ID-812.
- [Symbols, parts, macros](../../design/07-symbols-parts-macros.md) §2.8, §3.8, §9;
  [PLC and bus](../../design/10-plc-and-bus.md) §2.5, §2.6, §3.1;
  [connections](../../design/08-connections-terminals-cables.md) §4.2 to §4.4;
  [golden project](../../design/20-golden-project.md) §4.4, §7, §11.2, §13, §17, §21;
  [reports and forms](../../design/09-reports-forms-output.md) §2.5, §5; [UX/UI](../../design/16-ux-ui.md) §9.2;
  [open-source governance](../../design/19-open-source-governance.md) §3.1;
  [project format](../../design/05-project-format-and-persistence.md) §2.2, §13.
- [ADR-0011](../../adr/0011-format-versioning-and-migration.md),
  [ADR-0024](../../adr/0024-licence-set-and-permissive-gpl-boundary.md),
  [ADR-0032](../../adr/0032-per-project-text-styles.md).
- `docs/release/checklist.md` (RC-07); `ci/installed-acceptance.sh`.
