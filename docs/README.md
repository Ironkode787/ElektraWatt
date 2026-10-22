# ElektraWatt documentation map

**Status:** Draft v0.1 · **Date:** 2026-10-18 · **Part of:** ElektraWatt document set

**Purpose.** This page is the entry point to the ElektraWatt (EW) documents. It gives reading orders for three kinds of
reader, a one-line purpose for every document, the identifier schemes in one table and the rules for maintaining the
set. The project has no code yet; the documents are the product of the design phase.

## 1. Reading orders

### 1.1 Decision maker

1. [Vision and scope](design/00-vision-and-scope.md): why EW exists, who it is for, release targets per capability.
2. [Global plan](plan/01-global-plan.md): the headline (§1), the plan of record, the AI-assisted internal target, phases.
3. [Decision log](plan/06-decision-log.md): what is decided (Part A) and what is still open (Part B).
4. [Risk register](plan/04-risk-register.md) and [gates](plan/03-gates.md): what can go wrong and where the plan is re-forecast.
5. [Open-source governance](design/19-open-source-governance.md): licences, legal gates, roles.

### 1.2 Developer

1. [Architecture](design/04-architecture.md) and the [domain model](design/03-domain-model.md), with the [glossary](design/glossary.md) at hand.
2. The [ADR index](adr/README.md), starting with ADR-0001 to ADR-0013 (model, identity, transactions, storage).
3. The feature documents [05](design/05-project-format-and-persistence.md) to [18](design/18-quality-security-release.md) for the area you work on.
4. [Work packages](plan/02-work-packages.md) and [traceability](plan/05-traceability.md): which WP delivers which requirement.
5. [CLAUDE.md](../CLAUDE.md): house rules and model routing for AI-assisted work.

### 1.3 Domain expert or pilot

1. [Golden project](design/20-golden-project.md): the executable acceptance specification built from the reference drawing set.
2. [Requirements](design/02a-requirements-core.md) ([02b](design/02b-requirements-extended.md), [02c](design/02c-nonfunctional-requirements.md)): what EW must do, with IDs you can comment on.
3. [Brainstorm](design/01-brainstorm.md): ideas, defects of the reference set, open questions.
4. [Vision and scope](design/00-vision-and-scope.md), then [schematic editor](design/06-schematic-editor.md) and [UX](design/16-ux-ui.md) to see how work will feel.
5. The Proposed ADRs ([0025](adr/0025-gpl-plugin-exception-wording.md), [0027](adr/0027-native-symbol-grid-and-qet-grid-policy.md), [0028](adr/0028-default-drawing-fonts.md), [0029](adr/0029-network-telemetry-crash-reporting-policy.md), [0030](adr/0030-legal-steward-cra-role-support-signing.md)) where pilot input is wanted.

## 2. Document table

### 2.1 design/

| Document | Purpose |
|---|---|
| [00 Vision and scope](design/00-vision-and-scope.md) | Why EW exists, users, goals, non-goals, release targets per capability area |
| [01 Brainstorm](design/01-brainstorm.md) | Lessons from the reference set, 109 candidate ideas, big bets, open questions |
| [02a Requirements, core](design/02a-requirements-core.md) | Functional requirements of the core ECAD areas (FR-PRJ ... FR-REV) |
| [02b Requirements, extended](design/02b-requirements-extended.md) | Functional requirements for PLC, fluid, layout, manufacturing, interop, UI, extensibility |
| [02c Non-functional requirements](design/02c-nonfunctional-requirements.md) | Measurable quality requirements (NFR-*) |
| [03 Domain model](design/03-domain-model.md) | Entities, attributes, relationships, identity and invariants |
| [04 Architecture](design/04-architecture.md) | Crate structure, model, derived data, concurrency, rendering, format, extensibility |
| [05 Project format](design/05-project-format-and-persistence.md) | Directory layout, JSON Lines encoding, manifest, save protocol, migration |
| [06 Schematic editor](design/06-schematic-editor.md) | Editing of schematic pages, symbols, devices, connections, frames |
| [07 Symbols, parts, macros](design/07-symbols-parts-macros.md) | Master data: FnDefs, parts database, macros, templates, libraries |
| [08 Connections, terminals, cables](design/08-connections-terminals-cables.md) | Connectivity levels, potentials, wire properties, terminals, cables |
| [09 Reports, forms, output](design/09-reports-forms-output.md) | Report kinds, forms engine, derived report pages, display-list pipeline |
| [10 PLC and bus](design/10-plc-and-bus.md) | PLC configuration, channels, addresses, fieldbus, exchange with TIA Portal and CODESYS |
| [11 Fluid power](design/11-fluid-power.md) | Pneumatics (v1) and hydraulics (later) on the shared connection graph |
| [12 Panel layout](design/12-panel-layout.md) | Enclosures, rails, ducts, placements, 2D panel pages, 3D scene, STEP |
| [13 Manufacturing outputs](design/13-manufacturing-outputs.md) | Wire records, ferrules, marking, machine profiles, production package |
| [14 Interop](design/14-interop.md) | Importers and exporters, QET migration, EPLAN data, DXF/DWG, spreadsheets |
| [15 Checks, revisions, collaboration](design/15-checks-revisions-collaboration.md) | Rule catalogue, release workflow, revisions, working without a server |
| [16 UX and UI](design/16-ux-ui.md) | Workspace, navigators, property grid, command palette, wizards |
| [17 Extensibility](design/17-extensibility.md) | CLI, Session API, rhai, SQL projection, MCP, WASM plugins, Python |
| [18 Quality, security, release](design/18-quality-security-release.md) | Test strategy, performance, security, build, signing, support |
| [19 Open-source governance](design/19-open-source-governance.md) | Licences per layer, DCO, legal gates, roles, decision process |
| [20 Golden project](design/20-golden-project.md) | Reference project as acceptance specification (AC, D, EF, MF) |
| [23 Symbol coverage of the pilot corpus](design/23-symbol-coverage-pilot-corpus.md) | The pilot's 81 QET element types mapped to the starter symbol set; missing families and importer mappings |
| [Glossary](design/glossary.md) | Terms, identifier schemes, EPLAN/QET/EW translation table |

### 2.2 plan/

| Document | Purpose |
|---|---|
| [README](plan/README.md) | What each plan file is, generators, headline dates |
| [01 Global plan](plan/01-global-plan.md) | Capacity model, scenarios, milestones, phases, pilot preview, release train |
| [02 Work packages](plan/02-work-packages.md) | The work breakdown: 124 WPs with estimates, dependencies, acceptance |
| [03 Gates](plan/03-gates.md) | Gate system G-M0 ... G-M6, G-PP, G-SP: criteria, checkpoints, kill triggers |
| [04 Risk register](plan/04-risk-register.md) | Scored risks with owners, mitigations, contingencies, review gates |
| [05 Traceability](plan/05-traceability.md) | Generated report: requirements, ACs, defects to WPs; gaps |
| [06 Decision log](plan/06-decision-log.md) | Decided items (user decisions, Accepted ADRs) and open questions |
| [Gate records](plan/gate-records/G-M1-2026-10-readiness.md) | Dated readiness runs, pack drafts and gate records; first: the G-M1 readiness run of 2026-10-22 |

### 2.3 adr/

The [ADR index](adr/README.md) lists all 30 records with status and the one-line decision.

### 2.4 book/ (the user guide)

| Document | Purpose |
|---|---|
| [Book README](book/README.md) | The user guide as an mdBook in the Diátaxis structure (tutorials, how-to, reference, explanation): layout, offline build, generated pages, writing rules |
| [Introduction](book/src/introduction.md) | The guide's entry page; the tutorial [a first headless run on the golden project](book/src/tutorials/headless-workflow.md) |
| [Reference](book/src/reference/index.md) | The command and rule reference, **generated** by `cargo xtask docs-gen` from the command schemas and the rule-catalogue snapshot; never edited by hand |

### 2.5 research/

| Document | Purpose |
|---|---|
| [reference-drawing](research/reference-drawing.md) | Analysis of the reference drawing set: element types, defects, requirement candidates |
| [eplan-features](research/eplan-features.md) | Feature inventory of the EPLAN platform with a suggested EW priority |
| [standards](research/standards.md) | Standards that EW must implement or respect and the rules they imply |
| [rust-ecosystem](research/rust-ecosystem.md) | Candidate crates and their state, per architectural concern |
| [interop-formats](research/interop-formats.md) | Foreign file formats and exchange paths |
| [oss-landscape](research/oss-landscape.md) | Open-source ECAD and related tools |
| [panel-manufacturing](research/panel-manufacturing.md) | Panel-shop processes, machines and data |
| [plc-bus-fluid](research/plc-bus-fluid.md) | PLC engineering, fieldbus and fluid power notes |
| [gaps-and-corrections](research/gaps-and-corrections.md) | Research gaps (G1 ... G13+) and corrections (C-n) to the first brief |
| [architecture-proposals](research/architecture-proposals/) | The four competing architecture proposals (A incremental, B ECS, C relational, D retained) |

## 3. Identifier schemes

The full list is in [glossary §3](design/glossary.md#3-identifier-schemes). The schemes that are easiest to confuse:

| Scheme | Form | Meaning |
|---|---|---|
| Gate | `G-M0` ... `G-M6`, `G-PP`, `G-SP` | Plan gates; `G-PP` belongs to milestone M-PP (pilot preview), `G-SP` is the optional performance-spike gate. Bare `G1` ... `G13` are research gap topics |
| Milestone | `M0` ... `M6`, `M-PP` | Plan milestones |
| Kill trigger | `KT-n` | Conditions in the gates that stop or re-scope the project |
| Checkpoint | `CP-nn` | Gate checkpoints |
| Gate-pack item | `GP-nn` | Items of a gate pack. Personas keep `P1` ... `P6` |
| Constraint | `K-1` ... `K-7` | Vision constraints. Research corrections keep `C-n` |
| Planning assumption | `PA-n` | WBS planning assumptions; the decision log's own former inputs are `PI-n` |
| Work package | `WP-...` | Rows of [work-packages.csv](plan/work-packages.csv) |
| ADR | `ADR-nnnn` | Architecture decision records |

## 4. Maintenance

- **Decisions** go into ADRs ([how to add one](adr/README.md#how-to-add-an-adr)); open questions stay in the "Open
  questions" sections and in [decision log](plan/06-decision-log.md) Part B. After the first binary release, larger
  changes go through RFCs ([governance](design/19-open-source-governance.md) §3.4).
- **Generated book pages** (`book/src/reference/commands/`, `book/src/reference/rules/` and their `SUMMARY.md`
  entries) are written by `cargo xtask docs-gen` and checked with `--check` in `cargo xtask ci` and the `docs-book`
  job ([18](design/18-quality-security-release.md) §5.5).
- **Generated plan files** are written by scripts and checked in CI with `--check`; never edit text between
  `<!-- generated:NAME -->` markers by hand. See [plan/README](plan/README.md).
- **Mechanical checks:** `python3 -I docs/plan/tools/doccheck.py` verifies links and anchors, table columns, WP IDs and gate IDs.
- **House rules** (public repository, no customer data, house style) are in [CLAUDE.md](../CLAUDE.md).
