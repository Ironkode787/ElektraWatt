# ADR-0020: Extensibility tiers and security model (CLI, rhai, WASM on wasmtime LTS, MCP, Python)

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the judge panel)

## Context

- **Who needs the core.** Five groups need programmatic access:
  - panel builders automating releases in CI;
  - companies customising tag formats, title-block formulas and rules;
  - third parties writing importers, exporters and machine profiles;
  - integrators scripting from Python;
  - AI agents.
- **Retrofitting is costly.** KiCad shows the cost of retrofitting an API late: its schematic IPC API stayed empty
  for a long time ([oss-landscape](../research/oss-landscape.md) §3).
- **Attack surfaces** (G10):
  - received projects, which can carry scripts, SQL reports and rule packs (the Office-macro lesson);
  - plugins;
  - agents acting on confidential projects;
  - debug endpoints. egui's inspection server listens on `127.0.0.1:5719` when `EGUI_INSPECTION` is set, and on any
    interface if told to.
- **Prior art in QET.** QET suffered a CVE in project serialisation (CVE-2026-15037). Its MCP "live mode" sets the
  ask-first precedent for agent writes.
- **Engine releases.** wasmtime ships monthly majors. Every 12th release is an LTS with 24 months of security fixes,
  and 48.0.0 (2026-08) is such a release.

## Decision

1. **One write path.** Every front-end submits the same serialisable `Command`s
   (`Command → Transaction → Op`, ADR-0008). Read surfaces never write. Every write is previewable, validated,
   undoable, journalled and tagged with its origin (`gui`, `cli`, `script`, `plugin:<id>`, `mcp`).
2. **Tiers:**

   | Tier | Surface | Reads | Writes | Release |
   |---|---|---|---|---|
   | T0 | File format and permissive crates (ADR-0024) | files | files | MVP; crates published at 1.0 |
   | T1 | `ew` CLI: `check`, `query --explain`, `sql`, `render --png`, `export`, `import`, `diff`, `merge-driver`, `fmt`, jobsets | everything | commands | MVP (jobsets 1.0) |
   | T2 | rhai 1.26.1 **formulas**: tag and wire-number formats, title-block formulas, report filters | tracked reads only | nothing | MVP |
   | T3 | Rule and report packs in rhai and SQL | tracked reads, projection | findings, rows | MVP (rhai), 1.0 (SQL) |
   | T4 | rhai **automation** scripts (`query`, `preview`, `submit`) and the macro recorder | Session | commands | 1.0 |
   | T5 | Read-only SQL projection (ADR-0017) | projection | never | MVP |
   | T6 | `ew mcp`: `query`, `sql_query`, `find`, `run_checks`, `preview_command`, `commit_preview` | read tools | ask-first commits | MVP read (Could), 1.0 writes, 1.x GUI live bridge |
   | T7 | WASM component plugins | WIT queries, `sql-query` | `submit` | 1.x |
   | T8 | Python wheel (pyo3 0.30.1) of the headless core | Session API | Session API | 1.x |

3. **rhai inside queries is pure.**
   - It reads only through tracked accessors, so a formula is as incremental as Rust.
   - No I/O function is registered.
   - Operation, call-depth and string-size limits apply, and `unchecked` is banned.
4. **WASM plugins.**
   - **Runtime.** Plugins are WASM components on a wasmtime LTS line (48.x today), with wit-bindgen 0.63.0
     (amendment 2 fixes which LTS line).
   - **What the WIT world `elektrawatt:plugin@1` offers:**
     - typed read queries and `sql-query` against the projection sandbox;
     - `submit(command)` and `preview(command) → diff`;
     - capability-scoped file handles;
     - **declarative dialogs that the host renders**, so plugins never touch the UI toolkit.
   - **What plugins export:** importers, exporters, rules, report columns and machine profiles (amendment 4 adds
     decoders and actions).
   - **Plugin data** lives under namespaced `plugin.<id>.*` keys. It is persisted, merged and undoable, and it
     survives when the plugin is not installed.
5. **MCP.** `ew mcp` (rmcp) never starts implicitly.
   - **Transport.** It listens on stdio, or on loopback only (`127.0.0.1` / `::1`) with a per-session random token
     and an `Origin` check against DNS rebinding. There is no option to bind another interface (NFR-SEC-08,
     FR-EXT-026).
   - **Scope.** One session serves the one project it opened. While the GUI holds the project lock, a headless
     session is read-only (FR-EXT-016).
   - **Writes** are ask-first. `commit_preview` applies a previewed command only after explicit user approval, bound
     to `(preview_id, digest, base_rev)`; a moved revision fails as "stale preview". The result is an ordinary
     journalled, undoable transaction with `origin=mcp`. `HumanOnly` commands cannot be previewed.
   - **Data minimisation.** Fields `ew-schema` marks as personal data (engineer, checker, party persons) and
     confidential project identifiers (project reference, drawing id, party names) are omitted by default, because
     the agent's model may run at a remote provider ([17](../design/17-extensibility.md) §7.4; FR-EXT-033).
6. **Python** is a wheel of the headless core, not an embedded interpreter.
7. **Security model:**

   | Threat | Control |
   |---|---|
   | Malicious project or import file | Limits on every parser (≤ 100,000 archive entries, ≤ 2 GiB expanded, depth); no `..`, absolute or drive paths; NaN and overflow guards; no external XML entities; `cargo-fuzz` per parser; parsers never execute content |
   | Executable content in received projects | Never auto-run, and no event hooks on open, save or export. A per-project trust prompt is remembered by project hash and signer, and trust lapses on external change |
   | Plugins | Capability manifest approved at install; WASI pre-opened directories only; no network unless granted; fuel or epoch deadlines and `StoreLimits` per call; signed manifests (Ed25519); plugin ID shown in the undo history |
   | Agents, plugins and scripts gaming the gate | `HumanOnly` exposure for waivers, schemes, strictness, approval and release |
   | Debug endpoints | egui inspection and egui_mcp compiled only into test builds; a CI assertion on release binaries (ADR-0023) |

## Amendments

Accepted on 2026-10-18 ([17](../design/17-extensibility.md) open questions 1, 3 and 8, §2.3–§2.4). The decision text
above is unchanged; these entries refine it.

1. **JSON at the WIT boundary** (2026-10-18). Queries and records cross the boundary as published JSON Schemas and
   canonical JSONL. Typed guest bindings are generated from the same schemas in `ew-plugin-sdk`, so an additive
   format field needs no WIT change. The WIT package and the guest SDK are permissive crates (`ew-plugin-wit`,
   `ew-plugin-sdk`, FR-EXT-036).
2. **LTS line** (2026-10-18). Plugins run on **the wasmtime LTS line current at plugin launch**, not on a fixed
   48.x, because 48.x support ends about when 1.0 ships. Development continues on 48.x until then.
3. **Trust gating of formulas and profiles** (2026-10-18). Pure formulas (title-block formulas, report filters,
   tag and wire-number formats, value-set formulas) run in untrusted projects, because they read only through
   tracked accessors and have no I/O. Machine-profile `expr` columns in received projects stay trust-gated,
   because machine files leave the building (FR-EXT-031).
4. **Plugin roles and secrets** (2026-10-18). Plugins export importers, exporters, rules, report columns, machine
   profiles, decoders and actions; actions only submit edit commands through preview and commit. A host
   capability `secret-get(name)`, backed by the OS credential store, is granted per plugin at install and is never
   readable by other plugins (1.x, before the first connector plugin).
5. **`HumanOnly` commands** (2026-10-18; FR-EXT-030). They are refused for command-log replays, plugins, scripts,
   Python and MCP. On the CLI only the dedicated `HumanOnly` subcommands run them, and only after an interactive
   terminal confirmation.

## Consequences

**Positive**
- **One API.** The same API serves the GUI, CLI, scripts, plugins, agents and Python, so there is no second mutation
  path to retrofit or secure.
- **Plugins are insulated from the UI stack.** They never touch egui, so they survive the quarterly egui and wgpu
  upgrades (ADR-0014). A future toolkit swap does not break them.
- **Agent edits are safe by construction.** Every agent edit is visible as a diff before commit, and it is undoable
  and journalled.
- **Headless CI.** Panel builders can gate deliveries in CI with `ew check --scheme pre-delivery` from MVP.

**Negative**
- **Late plugins.** Third-party binary extensions arrive only in 1.x. Until then, extension means the CLI, rhai and
  data packs.
- **Young tooling.** Component Model tooling is still young, and WIT versioning becomes a public contract: the host
  keeps `@1` for at least 24 months after `@2`.
- **macOS JIT.** The wasmtime JIT needs an entitlement on macOS, or an interpreter mode
  ([18](../design/18-quality-security-release.md) OQ13).
- **Trust-prompt fatigue.** Prompts can train users to click through. Prompts are therefore per project and
  remembered, and nothing executable runs merely because a project was opened.
- **Plugin dialogs are declarative only.** Rich custom plugin UI is not possible.
- **Licence boundary for plugins.** The licensing of proprietary plugins depends on the plugin exception, which is
  still open (ADR-0025).

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Native dylib plugins** (abi_stable, libloading) | No sandbox; unsafe, per-platform builds; a plugin crash kills the editor; abi_stable is stale |
| **Embedded CPython** | Ships and versions a Python runtime, has ABI issues, and gives no sandbox. A wheel serves the same users without shipping an interpreter |
| **mlua as the primary language** | It needs a vendored C build, and a second language would double the API surface and the documentation. rhai is pure Rust, sandboxed by default (no I/O unless registered) and enough for formulas and rules |
| **MCP on by default** | A standing local endpoint into confidential projects, and the network sandbox could no longer prove "no listening socket" (NFR-SEC-09) |
| **wasmer or extism** | wasmer's Component Model story is weaker (unverified). extism uses byte-buffer interfaces, which are less typed than WIT |
| **Tracking the latest wasmtime** | Monthly majors would add a third upgrade train. The LTS line gives 24 months of security fixes |
| **Scripts with I/O inside the GUI process** | They would break the trust model and the purity that keeps formulas incremental |

## Related

- **ADRs:** [ADR-0008](0008-transactions-undo-preview.md), [ADR-0014](0014-gui-toolkit-egui.md), [ADR-0017](0017-read-only-sql-projection-api-v1.md),
  [ADR-0019](0019-rule-engine-and-rule-packs.md), [ADR-0021](0021-importers-emit-format-level-records.md),
  [ADR-0023](0023-supply-chain-and-release-integrity.md), [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md),
  [ADR-0025](0025-gpl-plugin-exception-wording.md), [ADR-0029](0029-network-telemetry-crash-reporting-policy.md).
- **Design:** [04 architecture](../design/04-architecture.md) §10; [17 extensibility](../design/17-extensibility.md);
  [18 quality, security and release](../design/18-quality-security-release.md) §11;
  [gaps-and-corrections](../research/gaps-and-corrections.md) G10.
- **Requirements:** FR-EXT-001 to FR-EXT-024, FR-EXT-026, FR-EXT-029 to FR-EXT-031, FR-EXT-033 to FR-EXT-037;
  NFR-SEC-01, NFR-SEC-02, NFR-SEC-03, NFR-SEC-05, NFR-SEC-06, NFR-SEC-07,
  NFR-SEC-08, NFR-SEC-09, NFR-REL-12, NFR-PRIV-08, NFR-PLAT-04.
