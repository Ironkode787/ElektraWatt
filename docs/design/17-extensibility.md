# Extensibility: CLI, scripting, SQL, plugins and agents

**Status:** Draft v0.4 · **Date:** 2026-10-25 · **Part of:** ElektraWatt design document set

**Purpose.** This document specifies every way to drive, extend or query ElektraWatt (EW) other than the GUI: the
`ew` command-line tool, the headless `Session` API behind it, rhai formulas, rules and automation scripts, the
read-only SQL projection, the MCP server for AI agents, WASM component plugins, the Python wheel, and the
importer/exporter plugin contracts. It fixes the security model of each surface, the API stability and versioning
policy, and the boundary of the GPL plugin exception (ADR-0025). It elaborates the EXT rows of
[02b](02b-requirements-extended.md) §10 and the security rows NFR-SEC-05 … NFR-SEC-08 of
[02c](02c-nonfunctional-requirements.md), within the accepted [architecture](04-architecture.md) §10 and ADR-0017,
ADR-0020, ADR-0021 and ADR-0024.

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and principles](17-commands-cli.md#1-scope-and-principles) | [commands, the ew command line and the session API](17-commands-cli.md) |
| [§2. Concepts and data](17-commands-cli.md#2-concepts-and-data) | [commands, the ew command line and the session API](17-commands-cli.md) |
| <a id="3-the-ew-cli"></a><a id="32-exit-codes"></a>[§3. The `ew` CLI](17-commands-cli.md#3-the-ew-cli) | [commands, the ew command line and the session API](17-commands-cli.md) |
| [§4. Headless Session API](17-commands-cli.md#4-headless-session-api) | [commands, the ew command line and the session API](17-commands-cli.md) |
| [§5. rhai scripting](17-scripting-sql-plugins.md#5-rhai-scripting) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§6. Read-only SQL projection](17-scripting-sql-plugins.md#6-read-only-sql-projection) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§7. MCP server](17-scripting-sql-plugins.md#7-mcp-server) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§8. WASM component plugins (1.x)](17-scripting-sql-plugins.md#8-wasm-component-plugins-1x) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§9. Python wheel (1.x)](17-scripting-sql-plugins.md#9-python-wheel-1x) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§10. Importer and exporter plugin contracts](17-scripting-sql-plugins.md#10-importer-and-exporter-plugin-contracts) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| <a id="11-api-stability-and-versioning"></a>[§11. API stability and versioning](17-scripting-sql-plugins.md#11-api-stability-and-versioning) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§12. The GPL plugin-exception boundary](17-scripting-sql-plugins.md#12-the-gpl-plugin-exception-boundary) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§13. Security model by surface](17-scripting-sql-plugins.md#13-security-model-by-surface) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§14. UI/UX touchpoints](17-scripting-sql-plugins.md#14-uiux-touchpoints) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§15. Outputs](17-scripting-sql-plugins.md#15-outputs) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§16. Edge cases and pitfalls](17-scripting-sql-plugins.md#16-edge-cases-and-pitfalls) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
| [§17. The reference project](17-scripting-sql-plugins.md#17-the-reference-project) | [scripting, SQL, MCP, plugins and stability](17-scripting-sql-plugins.md) |
