<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Starter symbol library

The IEC 60617 and ISO 1219 style symbols that the golden project needs (WP-S7.02), with their function definitions
(`../fndefs/`) and the generated QET 10 px edition (`qet10/`). Design background: `docs/design/07-symbols-parts-macros.md`
§2.3, §2.4, §3.2 and §9.1; grid policy: ADR-0027.

**Every drawing in this directory is original work of the project.** No symbol, geometry or text was copied or
converted from the QElectroTech element collection or from any other library (AI-use policy point 3, LR-05,
NFR-LIC-07). The documents carry no IEC description text: `iec60617_ref` and `iso1219_ref` are left empty until the
identifiers have been checked against the standards (they are metadata only, FR-SYM-019). All items have status
`draft`: the proportions are the project's own reading of the IEC 60617 and ISO 1219 style and still need the visual
review of the pilot users before the set is frozen (FR-SYM-030).

## Layout

| Path | Content |
|---|---|
| `content/symbols/<key>.json` | the native edition, connection grid 4 mm |
| `content/symbols/qet10/<key>@qet10.json` | the generated edition, connection grid QET 10 px (never edited by hand) |
| `content/fndefs/<key>.json` | the function definitions; one FnDef can serve several symbols |

Every file is one library document (`LibDocument`, doc 05 §13.2) as pretty JSON: 2-space indent, LF, member order of
the schema, one trailing newline. `crates/ew-scene/tests/starter_symbols.rs` fails when a file is not in exactly
that canonical form.

## Keys

Symbol keys follow doc 07 §9.1: `ew.iec.*` (IEC 60617 style electrical symbols), `ew.iso.*` (ISO 1219 fluid power),
`ew.gen.*` (built-in generic items: interruption point, black box, junction, markers, strip segment), `ew.bus.*`
(bus ports) and `ew.ovw.*` (assembly overview). FnDef keys are `ew.fn.<name>`. The edition suffix `@qet10` is
appended to the symbol key only; both editions reference the same FnDef.

The built-in interruption point FnDef is `ew.fn.ip`, not `ew.gen.ip` (that key names the symbol; a symbol and a FnDef
must not share an item id). A reference by key can be re-pointed (`LibRef.extra["key"]`).

## Identity

- `library_id` is the provisional system library `7202fd8b-1024-4bf1-9e99-670772eaa589` (`BUILTIN_LIBRARY` of
  `ew-import-qet`), until the system library gets its own identity (WP-S1.19a).
- `item_id = v5(BUILTIN_LIBRARY, meta.key)`: UUIDv5 over the key's UTF-8 bytes (the formula of
  `ew_ids::v5::imported_id`). The `@qet10` edition derives its ids from the suffixed key.
- `id = v5(NS_LIBCOPY, library_id ‖ U+001F ‖ item_id)` (`ew_ids::v5::lib_copy_id`).
- `version` is `0.1.0`; a symbol's `fndef` reference has `version_req` `^0.1` and carries the FnDef key in `key`.
- `sha256` is the SHA-256 of the compact JSON of the item's own members (every member except `$k`, `id`, `library_id`,
  `item_id`, `version` and `sha256`), in file order, as `sha256:` plus 64 hexadecimal digits (EW-CORE-014).
- All of this is written by `cargo xtask symbols-qet10`; never edit ids or hashes by hand.

## Grid rules

- **Connection points** lie on the native connection grid of 4 mm (`Grid::MM4`); the symbol origin is a CP or the
  graphic centre (contacts: the upper CP; terminals: the terminal centre; interruption point: the arrow tip).
- **Graphics** (lines, polylines, rectangles, circles, polygons, static texts, `r`, `rx`, `ry`) use the 0.5 mm grid.
- **Text slots** use the 1 mm grid. Their text height is style-driven (doc 20 §17.5), never part of the symbol.
- **Anchors** are explicit `named` points on the connection grid, `graphic_centre` (snapped to the grid when
  resolved), or the origin when the `anchor` member is absent (the default of the schema).
- **Pens** are the house pens: `p026` for outlines and conductors, `p013` for thin lines and internal linkages with
  `dash42`, `p052` is reserved for the valve double bar.
- **Orientation variants** A to H are never stored; they are derived (`ew_scene::symbol::lower`).
- The CP direction is the direction a wire leaves the CP.
- The linkage between the poles of the switch and the MCB, and between actuator and process valve, is derived from the
  `link_points` (FR-SYM-014); only the actuator handle and the internal release marks are drawn.

## The `@qet10` edition

`cargo xtask symbols-qet10` writes `qet10/<key>@qet10.json` from each native symbol (doc 07 §3.2 rule 2). Every
length `v` is scaled to `round(v · 7 937 500 / 12 000 000)` nm (ties toward zero): a CP that is `k` steps of 4 mm from
the origin lies `k` steps of the QET 10 px grid (`7 937 500 / 3` nm) from it; graphics and text slots scale by the same
ratio (0.66146) and are rounded to the nm; text heights are untouched. Because 0.5 mm becomes 1.25 px, graphics of the
edition are not whole pixels; only the CPs are on the grid. The edition is linted like the native one and
`cargo xtask symbols-qet10 --check` fails when a committed file differs from the generation.

## How to add or change a symbol

1. Draw the native symbol in `content/symbols/<key>.json` (and its FnDef in `content/fndefs/` unless it shares an
   existing one). The sealed files show the member order; ids and hash may be placeholders at this point.
2. Canonicalise the files: `EW_BLESS_CONTENT=1 cargo nextest run -p ew-scene --test starter_symbols bless_native_documents`
   (PowerShell: `$env:EW_BLESS_CONTENT='1'; cargo nextest ...`).
3. Seal and generate: `cargo xtask symbols-qet10` (ids, hashes, the `@qet10` edition).
4. Run the tests: `cargo nextest run -p ew-scene --locked --test starter_symbols` and `cargo xtask symbols-qet10 --check`.
5. A new key must be added to the expected list in `starter_symbols.rs`.

A change that moves a CP or the anchor, or removes a CP, is a minor or major change of the item (doc 07 §3.1); bump
`version` by hand in that case.

## Licence

CC-BY-SA-4.0 (`meta.licence` of every item; licence map rule L7). The design exception of ADR-0024 (KiCad-style, doc 07
§3.8) is added to the expression when it is adopted (LG-2); it is pending.
