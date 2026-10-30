# WP-S1.12: per-project text styles (closed specification, one lane)

**Status:** specified in cycle 36 (documents only), one part of 0.4 pw for FT-662 under
[ADR-0032](../../adr/0032-per-project-text-styles.md); **built in cycle 37** (lane B37, decision log ID-855; FT-662 closed; Golden A and C unchanged) · **Date:** 2026-10-30 · **Work package:**
[WP-S1.12](../02-work-packages.md#wp-s112-project-structure-and-page-model-breadth) · **Decision log:** ID-837 ·
**Part of:** ElektraWatt plan, specifications

**Purpose.** This is the lane brief's base for ADR-0032: per-project text styles stored as overrides of the built-in
table, the 7 px defaults for `cp.small` and `marker` in new projects, one effective table read by the scene, the
exports and EW-CORE-061. It lives here and not in a WP-S1.12 specification because WP-S1.12 has none and needs none
for its other scope; FT-662 names WP-S1.12 as its owner, and FR-SYM-013 stays traced to WP-S3.01. Written against
the code at `adb0df1a`. A choice the lane meets that this document and the ADR do not settle is escalated, never
guessed (`CLAUDE.md`, rule 2).

## 1. What exists at `adb0df1a`

| Item | Where |
|---|---|
| The fourteen built-in rows (`StyleDef {key, qet_px, weight}`), `builtin()`, `TextStyleTable { rows: &'static [StyleDef] }`, `resolve`, `get` | `crates/ew-text/src/style.rs` |
| `TEXT_STYLES` (the copy), `em_nm`, EW-CORE-061 and EW-CORE-063, the style choice of drawn texts | `crates/ew-checks/src/rules/page.rs` |
| The agreement test `text_styles_of_the_checks_match_ew_text` | `crates/ew-api/tests/checks.rs` |
| `builtin()` callers: page, forms, symbol, table and embedded scenes; `ew-api`'s editing output; export and renderer tests | `crates/ew-scene/src/{page.rs,forms/mod.rs,symbol.rs,table.rs,embedded.rs}`, `crates/ew-api/src/output/editing.rs`, tests |
| `ProjectSettings {identifier_set, ip_pairing, formatting, extra}` | `crates/ew-schema/src/project.rs` |
| The built-in basic project | `crates/ew-api/src/new_project.rs` (`builtin`) |
| `project.settings.set` with its whitelist | `crates/ew-commands/src/commands/settings.rs` |

## 2. Build list

1. **`ew-schema`** (`text_style.rs`, new module, re-exported): `TextStyleSpec { size: Nm, weight: Option<TextWeight>,
   extra }`, `TextWeight` (`regular`, `bold`, open enum), `ProjectSettings.text_styles: BTreeMap<Ident, TextStyleSpec>`
   (default empty, skipped when empty); `BUILTIN_TEXT_STYLES: &[(&str, u16, TextWeight)]` (the fourteen rows moved
   from `ew-text`, same order); `fn effective_text_styles(&ProjectSettings) -> Vec<EffectiveStyle { key: Ident, em:
   Nm, weight: TextWeight }>` (base order, then extending keys in key order); `NEW_PROJECT_TEXT_STYLES` (the two
   overrides at 1,852,083 nm). `schemas/0.1` re-blessed.
2. **`ew-derive`**: the memoised query `TextStyles` over the project record, returning `Arc<[EffectiveStyle]>`.
3. **`ew-checks`**: EW-CORE-061 and the style-choice helpers read `TextStyles`; `TEXT_STYLES` and `em_nm` deleted;
   the minimum stays `MIN_CAP_HEIGHT_NM`.
4. **`ew-text`**: `TextStyleTable` holds owned rows (`from_rows(impl IntoIterator<Item = (key, em, weight)>)`);
   `builtin()` stays for project-less callers.
5. **`ew-scene`**: the scene inputs carry the effective table; every `builtin()` call in `src/` is replaced; a test
   asserts `ew_text::builtin()`'s rows equal `ew_schema::BUILTIN_TEXT_STYLES` (replacing the `ew-api` test).
6. **`ew-api`**: builds the table from `TextStyles` for the scene and the export; `new_project.rs`'s `builtin` writes
   `NEW_PROJECT_TEXT_STYLES`.
7. **`ew-commands`**: `project.settings.set` whitelists `text_styles`; refusals at the member path for a key that is not
   an identifier, a size outside 500,000 … 50,000,000 nm and an unknown weight.

## 3. Acceptance

- **TS-1** Golden A and Golden C: every exported byte and the acceptance result unchanged; `project.jsonl` has no
  `text_styles`; EF-12's two EW-CORE-061 findings stay.
- **TS-2** A new project's `project.jsonl` holds exactly the two overrides; placing a wire marker and an MCB with pin
  numbers prints those texts at em 1,852,083 nm in the page scene and the PDF; EW-CORE-061 reports nothing.
- **TS-3** `project.settings.set text_styles` with `tag.main` at 4,000,000 nm changes every device tag's size on every
  page (FR-SYM-013's acceptance), one undo step; undo restores; a size of 100 nm is refused at
  `/text_styles/tag.main/size`.
- **TS-4** An extending key `note.big` named by a free text's style resolves to its size; an unknown key still falls
  back to `text.normal` with EW-CORE-063's existing behaviour.
- **TS-5** A file with an unknown member inside an entry and one with an unknown weight value load and save
  byte-identically.
- **TS-6** A company basic project without the member makes a project without it; `save_as_basic` of a new project
  keeps the overrides.
- **TS-7** No `ew_text::builtin()` call remains in `crates/ew-scene/src` or `crates/ew-api/src` (a test over the source
  text, as the repository's other source scans).
- **TS-8** Expected outputs that start from the built-in basic project and print a marker or a small pin number are
  re-blessed in one commit, each file listed in the lane's report.
- **TS-9** `cargo nextest run -p ew-schema -p ew-derive -p ew-checks -p ew-text -p ew-scene -p ew-api -p ew-commands`,
  the matching clippy, `cargo fmt --all --check`, `cargo xtask check-deps`.

## 4. Stop rules, size and routing

The lane stops and reports when a change needs a new dependency edge, when Golden A's or Golden C's bytes change,
when a style member beyond `size` and `weight` would be typed, or when a renderer needs the project's styles where no
caller can pass them. **Size 0.4 pw** (schema and query 0.1, checks and scene plumbing 0.15, new project and command
0.05, tests and re-bless 0.1), follow-up work inside WP-S1.12's scope; the CSV is unchanged. **Routing:** Opus at `high`
(a format member and a cross-crate change; ID-700). Follow-up: FT-669 (the table editable in the window). Documents
after the lane: doc 03 §5.1, doc 05 §2.2, doc 09 §2.8, doc 20 §17.5, doc 07 §2.3 (ADR-0032's consequences).

## Sources

- [ADR-0032](../../adr/0032-per-project-text-styles.md); decision log ID-825, ID-833, ID-837; FT-648, FT-662, FT-669.
- [Golden project](../../design/20-golden-project.md) §17.5, §18.1; [reports and forms](../../design/09-reports-forms-output.md)
  §2.8; FR-SYM-013.
