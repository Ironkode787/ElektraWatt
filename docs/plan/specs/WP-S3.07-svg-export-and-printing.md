# WP-S3.07: SVG export and printing (closed specification)

**Status:** specified in cycle 42 (documents only), an M2 package, in four parts: part 1 the SVG export completed
(embedded font subsets, 0.6 pw, D2 to D4), part 2 the print set and `ew print` with the process adapters (0.9 pw, D5 to
D9), part 3 printing from the window with the print preview and the Linux portal (0.8 pw, D10, D11), part 4 the
native Windows adapter (0.7 pw, D12); 0.3 pw reserve; part 1 at any time, part 2 before parts 3 and 4, parts 3 and 4
in any order · **Date:** 2026-10-31 · **Work package:**
[WP-S3.07](../02-work-packages.md#wp-s307-svg-export-and-printing) · **Decision log:** ID-1017; O3 answered by ID-1030 · **Part of:** ElektraWatt plan, specifications

**Purpose.** The SVG writer was built ahead of this package (cycle 14, `ew_export::svg`; `ew export svg` since cycle 15):
text as text, stable order, links, one file per page, byte-identical serial and parallel output (AC-14). Its fonts are
named, not embedded (FR-OUT-013 partial, FT-233), so a page renders with whatever font the viewer has. Printing does not
exist: `ew-print` is an empty crate, there is no `ew print` and no Print action. This document traces the built SVG with
its evidence, closes the rest to items, decisions and acceptance, and settles the boundaries with WP-S3.04, WP-S1.22,
WP-S2.01, WP-S6.06 and WP-S3.01. It is written against the code at `453d252d` (§2.1). Where this document and a design
document differ, this document is the narrower and wins for the lanes; the design documents stay the target and are
updated at the design pass after each part (§13). A choice a lane meets that this document does not settle is
escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: the SVG export completed](#5-part-1-the-svg-export-completed)
6. [Part 2: the print set and ew print](#6-part-2-the-print-set-and-ew-print)
7. [Part 3: printing from the window](#7-part-3-printing-from-the-window)
8. [Part 4: the Windows adapter](#8-part-4-the-windows-adapter)
9. [Neighbours and who owns what](#9-neighbours-and-who-owns-what)
10. [Files owned and what a lane may not decide](#10-files-owned-and-what-a-lane-may-not-decide)
11. [Risks](#11-risks)
12. [Size, order and routing](#12-size-order-and-routing)
13. [Open questions, follow-ups and the documents to update](#13-open-questions-follow-ups-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package every SVG page embeds the subsets of the faces it uses, so folio 103's SVG renders
in a browser without the fonts installed, as the PDF page does (FR-OUT-013, AC-37's MVP half). `ew print --sections 1
--paper A5 --fit --out p.pdf` writes the print set of section 1 fitted to A5, each page carrying "scaled 71 %" in its
margin; `ew print --pages 101-106` sends folios 101, 102, 103 and 106 to the default printer through the OS
(FR-EXT-009). In the window, Output ▸ Print… (Ctrl+P) opens the print pre-dialog with ranges, paper, scale-to-fit and
colour mode and a preview of the print set, then the native dialog: the desktop portal on Linux, the Windows print
dialog on Windows (FR-OUT-017). Print, PDF, SVG and PNG show one geometry because every print path draws the export's
display lists under one transform (FR-OUT-018's print share).

**Non-goals** (each with its package or follow-up):
- No DXF (WP-S3.19), no tagged or PDF/A-3b output (WP-S3.14), no per-language print sets (FT-250, 1.0).
- No macOS adapter: macOS is not a supported platform (ID-880).
- No raster parity of PDF against PNG (FT-235, WP-S3.04 and WP-S6.01), no cross-platform digest matrix (FT-234,
  WP-S6.06); no automated browser rendering of the SVG (D4).
- No print scheme in the export schemes of doc 09 §2.9 and no saved print settings (FP-1); no duplex, tray or copies in
  the pre-dialog (the native dialog has them).
- No change to the PDF export, the PNG export or any drawn page.

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Reports](../../design/09-reports-forms-output.md) §1.3, §4.2, §4.3, §4.4, §4.5, §5 | `ew-print`'s place, the PDF, determinism, the SVG target, the print adapters, the Print touchpoint |
| [UX](../../design/16-ux-ui.md) (the File and Output commands, Ctrl+P); [design language](../../design/16a-design-language.md) | Part 3's action, dialog and preview |
| [Quality](../../design/18-quality-security-release.md) §11.2; 02c NFR-SEC-04 | The `unsafe` allowlist part 4 needs |
| [Extensibility](../../design/17-extensibility.md) §3.1, §3.3 | `ew print`'s row, CLI conventions |
| [Golden project](../../design/20-golden-project.md) §5 (pages and sections), §21 (AC-14, AC-37) | Section 1 is folios 100 to 199; every Golden A page is an A4 landscape sheet |
| ADR-0015 (PDF is the print master; amendment 1, `ew-print` is GPL with per-OS features) | D5 |
| Specifications | [WP-S1.22](WP-S1.22-cli-surface-and-api-guard.md) D2, D4, D5, D13, D14; [WP-S2.09](WP-S2.09-messages-bulk-previews-history-jobs-and-recovery-ui.md) part 1 (the export job); [WP-S2.01](WP-S2.01-application-shell.md) (printing named as this package's) |
| Follow-ups | FT-233 (moves here, D2); FT-234, FT-235 (stay) |

### 2.1 What exists at `453d252d` (the base contract)

The lanes rely on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| `svg(pages: &[PdfPage], &SvgOptions { monochrome, faces: BTreeMap<FontKey, SvgFace { family, weight, italic }> }, geometry, parallelism) -> SvgOutput { files, report }`; text as `<text>` with per-character `x`; `id`s for link targets, `<a href>` sources; numbers from integer nm; fonts **named, not embedded** (module note "As built (cycle 14)") | `crates/ew-export/src/svg.rs:1-200` | Part 1 adds the embedded subsets |
| The SVG faces of an export from `ew_text::bundled()` | `crates/ew-api/src/output/mod.rs:366-395` | Part 1 hands the font bytes too |
| Bundled fonts: Noto Sans Regular, Bold and Noto Sans Math Regular, all TrueType (`glyf`, no `CFF `, no `fvar`), compiled in with `include_bytes!` (`assets/fonts/` also holds Noto Sans SemiBold, which is not bundled; corrected in cycle 45) | `assets/fonts/`; `crates/ew-text/src/fonts.rs:497-506` | D3's subsetter handles `glyf` only |
| `ew export svg [<project>] --out <dir> [--pages <folios>] [--threads <n>] [--json]`; the directory replaced as a whole, SVG files only | `bin/ew/src/cli.rs:43, 1047-1095`; doc 09 §4.4 (ID-370) | Unchanged surface |
| AC-14's SVG entry pins the SHA-256 of the 36 SVG files (`fa2d79d4…`), 3935 glyph runs as `<text>`, 92 links; no runner entry for AC-37. *At cycle 44's end* (after WP-S4.02 part 5 and WP-S3.06 part 3, decision log ID-1089, ID-1091) the entry reads 3938 glyph runs, 246 links (64 cross-reference items, 28 PLC overview rows, 154 report row links) and the SHA-256 `2bc2304c…`: a lane re-reads it at its base. *After part 1* (cycle 45, ID-1105): the same runs and links, the SHA-256 `d6515093…`, and AC-37's SVG runner entry | `golden/J-E-2416/expected/acceptance_headless.md:49-50` | Part 1's golden change |
| `PageSelection::{All, Folios(Vec<String>)}` (a range `<first>-<last>` is every page between them in page order); `ExportOptions { pages, parallelism, monochrome, date }` | `crates/ew-api/src/output/mod.rs:210-286` | Part 2 adds sections and the print options |
| `ew_export::pdf(pages, options, fonts, geometry)` (krilla 0.8.2, PDF/A-2b); `PdfPage { label, width, height, list, … }` | `crates/ew-export/src/pdf.rs` | The print set is written by the same writer |
| The export job on a snapshot (`Session::export_job`, `ExportJob::run(ctl)`, `ew-jobs`) | `crates/ew-api/src/output/job.rs` | Part 3's print job |
| `ew-print`: an empty GPL crate (`#![forbid(unsafe_code)]`), dependencies `ew-export`, `ew-render-cpu`; already a dependency of `ew` and `elektrawatt`; `ci/dep-rules.toml` allows exactly those edges | `crates/ew-print/`; `ci/dep-rules.toml:83, 113-114` | Filled by parts 2 to 4 (D5) |
| The workspace lint `unsafe_code = "forbid"`; no product crate uses `unsafe`; no `unsafe-allowlist.toml` and no allowlist lint yet (doc 18 §11.2 plans both and names the print adapters) | `Cargo.toml:39-40`; doc 18 §11.2 | Part 4 (D12) |
| In the graph already: `windows 0.62.2` (core, collections, future, numerics features), `windows-sys` 0.52/0.59/0.61, `zbus 5.13.2` and `async-io 2.6.0` on Linux (through accesskit and atspi), `rfd 0.17.2` (`xdg-portal`) in `elektrawatt` | `Cargo.lock` | D9 and D12's dependency choices |
| Output menu: `file.export_pdf` (Ctrl+Shift+E) in `Menu::Output`; no print action; doc 16 binds Print to Ctrl+P | `crates/ew-viewmodel/src/actions.rs:1110-1127`; doc 16 (the shortcut table) | Part 3's `file.print` |

### 2.2 Gaps found

| Gap at `453d252d` | Consequence |
|---|---|
| SVG fonts named only; the bundled subsetter (typst `subsetter` 0.2.6, krilla's) drops `cmap`, which `<text>` needs (FT-233) | D3: an own subsetter that keeps glyph IDs and `cmap` |
| No section selection, no paper, no fit | D6 |
| Golden A has A4 sheets only, so fitting to A4 is 100 % | D8: the golden fits section 1 to A5 (210/297 = 70.7 %, "scaled 71 %"); an A3 fixture gives the requirement's literal case |
| Windows printing needs FFI (`PrintDlgExW`, a printer DC, `StretchDIBits`): the first `unsafe` of the product | D12, §13 O3 |

## 3. Requirements traced

| Requirement | State and what this package delivers | Part | Acceptance |
|---|---|---|---|
| FR-OUT-013 | **Built** except the fonts (cycle 14, ID-344, ID-345; AC-14 passes): text as text, stable element order, links, mm `viewBox`. Here the embedded subsets (D2 to D4, FT-233) | 1 | SV-1 to SV-4 |
| FR-OUT-017 | The print set: page, section and folio ranges, scale-to-fit stamped on the print set only, colour mode, preview, through the OS (D5 to D11) | 2, 3, 4 | PR-1 to PR-5, PW-1 to PW-4, WN-1 to WN-3 |
| FR-OUT-018 | Screen, PDF and PNG geometry are WP-S3.01's and WP-S3.04's (raster parity FT-235); SVG draws the PDF's display lists (built). Here the print share: every print path draws the export's display lists under one affine transform (D7), proven per page | 2, 4 | PR-3, WN-2 |
| FR-EXT-009 | `ew print` with the adapters (D9, D12); the manual per-OS test of its acceptance | 2, 4 | PR-4, WN-3 |

**AC-37** (SVG, MVP half) gets a runner entry in part 1: every page's SVG keeps its text as `<text>` and embeds the
faces it names (SV-3). Its DXF half is WP-S3.19's.

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Built rows traced and sized at nil.** Remaining: parts 1 to 4 (3.0 pw) and 0.3 pw reserve; proposed PERT O 2.3 · L 3.2 · P 4.8 · **PERT 3.3** (σ 0.4), against 3.2: the SVG writer is built (about 0.8 pw of the row), the Windows adapter and the preview were not priced in full | Re-sizing the SVG writer would count WP-S3.04's cycle-14 work twice |
| D2 | **FT-233 moves to this package.** It is FR-OUT-013's only gap and FR-OUT-013 is this package's row; WP-S3.04 is an M1 package closed in practice | One owner per requirement |
| D3 | **An own subsetter that keeps glyph IDs**, in `ew-export` (`font_subset.rs`), for TrueType (`glyf`) faces: the used glyphs and the components of composite glyphs (closure, depth at most 16) keep their outlines, every other glyph becomes empty in `glyf` and `loca` (long offsets), glyph IDs do not change; the tables `cmap`, `head` (checksum adjustment recomputed), `hhea`, `hmtx`, `maxp`, `OS/2`, `name` and `post` (rewritten as version 3.0, no glyph names) are kept, and the hinting tables `cvt `, `fpgm` and `prep` byte for byte when the face has them (amended cycle 46, FT-1045: a hinted face's instructions refer to them); `GSUB`, `GPOS`, `GDEF`, `kern`, `MATH` and every other table are dropped (the text is shaped already and placed per character); tables in tag order, 4-byte aligned, checksums per table. A face that is not `glyf` (a project font with `CFF `) is named, not embedded, and counted in `SvgReport::named_fonts`. The subset is written as `@font-face { font-family: "<family>"; font-weight: <w>; font-style: <s>; src: url(data:font/ttf;base64,…) format("truetype"); }` in a `<style>` at the top of each page that uses the face, base64 by an own encoder (RFC 4648, no line breaks). No new crate | `subsetter` 0.2.6 drops `cmap` (it serves PDF CID fonts), and a retain-GID subset needs no remapping of the runs; the alternatives are a new subsetting crate whose `read-fonts` and `write-fonts` versions would add a third copy to the graph, or whole fonts per page (about 350 KB each). The cost: a page carries the `cmap`, `hmtx` and `loca` of the whole face; the per-page size is not measured (owner's rule) and the lane reports Golden A's total |
| D4 | **SVG acceptance without a browser in CI.** The structural proof is automated: every `<text>` names a family that the page's `<style>` declares, every declared subset parses (the test reads it back with `skrifa`, a dev-dependency already in the graph) and maps every character of the page's texts to a glyph with an outline. "Renders identically in a browser" is a manual check: folio 103's SVG opened in a browser without Noto Sans installed beside the PDF page, an item of the G-M2 owner checklist (§13 O4) | No browser runs in CI; the structural test proves what makes a browser render the text with the right face |
| D5 | **`ew-print` holds the adapters; the print set is built in `ew-api`.** `ew-api` builds the print set (`Session::print_set(&PrintOptions) -> PrintSet { pages: Vec<PdfPage>, pdf: Vec<u8>, stamp: Option<String> }`): selection, paper, fit, stamp, monochrome, written by `ew_export::pdf` with the export's options and fonts. `ew-print` takes a `PrintSet`'s bytes or pages and a `PrintTarget` and hands them to the OS through one `trait PrintAdapter { fn name(&self) -> &'static str; fn print(&self, job: &PrintJob) -> Result<PrintOutcome, PrintError>; }`. `ew` and `elektrawatt` link both (edges exist). No new workspace edge | ADR-0015: PDF is the print master; the stamp needs `ew-text`, which `ew-print` does not reach; the adapters are platform code that only the binaries need |
| D6 | **The print options.** `PrintOptions { pages: PageSelection, paper: Paper, fit: Fit, monochrome: bool, date }`. `PageSelection` gains `Sections(Vec<String>)`: the pages of each named section (by its number, `1`) in page order, **its divider included**, each page once. `Paper` is `Sheet` (each page on its own sheet size, the default) or a named size `A5`, `A4`, `A3`, `A2`, `A1`, `A0`, `Letter`, `Legal`, `Tabloid`, oriented like each page. `Fit` is `Actual` (default; a page larger than the paper is clipped from its top-left and the print set's `notices` say which pages) or `Fit` (each page scaled uniformly to the whole paper, `min(paper width / sheet width, paper height / sheet height)`, and centred; never enlarged), so an ISO sheet on the next smaller ISO paper is 70.7 %. With `Fit` and a factor below 100 %, the stamp "scaled <n> %" (`n` rounded half up to a whole percent) is drawn with its baseline 4 mm above the paper's bottom edge and its end 5 mm from the right edge, 2 mm cap height, the house font, black, over the scaled sheet's border outside its frame; the lane checks on Golden A's frames that it covers no drawn item and stops if it does; in the print set only. Refusals, exact: "unknown paper `<name>` (sheet, A5, A4, A3, A2, A1, A0, Letter, Legal, Tabloid)"; "no section `<n>` (the sections are <numbers>)"; "give `--pages` or `--sections`, not both" | FR-OUT-017's ranges and stamp; a divider belongs to its section in the outline, so a section prints with it (FR-OUT-017's acceptance "101–106 only" read as "no page outside 100–199"; §13 O2). The stamp's position and size are rules, not fits (the stop above guards the one fixture they touch); fitting to the whole paper gives FR-OUT-017's "scaled 71 %" for A3 on A4, which a margin would turn into 67 % |
| D7 | **One geometry.** A print page is the export page's sheet display list inside one `Group` whose transform is the fit's scale and translation (identity for `Actual` on a matching sheet), plus the stamp's glyph run outside the group; links and destinations stay. A test asserts, per page, that the inner list equals the export page's list item for item. The Windows adapter (D12) renders these lists with `ew-render-cpu` | FR-OUT-018: print draws nothing of its own besides the stamp |
| D8 | **Golden examples add files.** `golden/J-E-2416/expected/print/sha256.txt` (the SHA-256 of `ew print golden/J-E-2416.ew --sections 1 --paper A5 --fit --out <file>`) and `expected/print/structure.json` (its page labels, page sizes in mm and the stamp texts, written by the PDF structure test of `expected/pdf/`). **Part 1 is a golden change** (§5); parts 2 to 4 change no existing byte | The print set is a new output beside the PDF |
| D9 | **The process adapters (part 2), no FFI.** `lp` (Linux, CUPS): `lp [-d <printer>] -t <title> <file>` spawned with arguments (no shell), the print set written to a temporary file in the system temporary directory and removed after `lp` returns; `lp` missing gives "printing needs the CUPS command `lp`; install cups-client or print from the window". `viewer` (all platforms): the print set written to `<temp>/<project>-print.pdf` and opened with the default PDF application (`xdg-open` on Linux, `explorer.exe` with the file on Windows), so the person prints from it. `ew print [<project>] [--pages <folios> \| --sections <n,…>] [--paper <size>] [--fit] [--mono] [--printer <name>] [--via lp\|viewer\|windows] [--out <file.pdf>] [--json]`: `--out` writes the print set and prints nothing; the default `--via` is `lp` on Linux and `windows` on Windows when part 4 is in, else `viewer`; `--printer` with `viewer` is refused ("`--printer` needs `--via lp` or `--via windows`; the viewer asks"). Result `ew.print-result/1` (adapter, printer or empty, pages, paper, scale, the file with `--out`). Exit 0, or 2 for a refusal or an adapter failure (its text on stderr) | The CUPS command and the default viewer are the OS's own print paths, need no library and work headless on Linux |
| D10 | **The window (part 3; every text and layout by [doc 16a](../../design/16a-design-language.md)).** `file.print` in `Menu::Output`, Ctrl+P, enabled with pages. It opens the **print pre-dialog**: Range (All, Current page, Sections with a list, Pages with a text field), Paper (Sheet or a size), Scale to fit, Monochrome, and the **preview**: the print set's current page rendered by `ew-render-cpu` at the dialog's scale with Previous and Next and "<n> of <m>", the stamp visible. The print set is built as a job on the snapshot (`ew-jobs`, progress, cancel). **Print…** hands it to the adapter of D11; **Open in PDF viewer** to `viewer`. The dialog never blocks editing; the set prints the revision it was built at. Strings in `strings.rs` (`action-file-print`, `print-*`) | Doc 09 §5 keeps an EW pre-dialog for fit and monochrome; the preview of FR-OUT-017 is the print set itself, so what is seen is what prints |
| D11 | **The Linux portal adapter (part 3).** `portal`: xdg-desktop-portal Print through **ashpd 0.13.13** (MIT; MSRV 1.87, within `ew-print`'s 1.92; GPL crate, so the permissive MSRV does not apply): `PreparePrint` with the window as parent, then `Print` with the print set's file descriptor; works in Flatpak. Dependency `ashpd = { version = "=0.13.13", default-features = false, features = [<the async-io executor feature>] }` under `cfg(target_os = "linux")` only; it reuses the graph's `zbus` 5. **Stop** if ashpd needs tokio, a second `zbus`, a licence outside `deny.toml`'s allow list, or more than eight new packages. A portal that is missing or fails falls back to `lp`, then `viewer`, with one notice | The portal is the sandbox-safe Linux print path; zbus is already linked through accessibility |
| D12 | **The Windows adapter (part 4): the first `unsafe` of the product.** `windows`: `PrintDlgExW` (the native dialog; `--printer <name>` opens the named printer's DC without a dialog, `CreateDCW`), then `StartDocW`, and per page `StartPage`, the page's display lists (D7) rendered by `ew-render-cpu` at the printer's DPI in bands of 256 device rows (RGBA, about 10 MB per band at 600 dpi on A3), each band sent with `StretchDIBits`, `EndPage`, then `EndDoc`; colour mode from D6; cancel between bands. Dependency `windows 0.62.2` (in the graph) with the features `Win32_Foundation`, `Win32_Graphics_Gdi`, `Win32_Storage_Xps`, `Win32_UI_Controls_Dialogs` under `cfg(windows)` only. **`unsafe` is confined** to `crates/ew-print/src/windows.rs`: `ew-print`'s `Cargo.toml` replaces `[lints] workspace = true` by the workspace set with `unsafe_code = "deny"`, the module carries `#![allow(unsafe_code)]`, every block a `// SAFETY:` comment; the part adds `unsafe-allowlist.toml` (one entry: `ew-print`, `src/windows.rs`) and doc 18 §11.2's lint (`cargo xtask check-unsafe`: a crate root without `forbid(unsafe_code)` must be listed, and `unsafe` may occur only in its listed files) | Doc 09 §4.5 and doc 18 §11.2 plan exactly this; printing the same display lists keeps print and PNG one geometry. The cost is FFI that only a manual test proves (a print to "Microsoft Print to PDF"), and NFR-SEC-04's second-maintainer review (§13 O3). Without part 4 Windows prints through `viewer` |
| D13 | **Packaging needs (WP-S6.06 owns the manifests).** Flatpak: the portal only (no `lp` in the sandbox), no extra permission beyond the portal's default access. Debian package and AppImage: `cups-client` recommended for `lp`. Windows: no runtime beyond the system's GDI and print spooler; `viewer` needs a default PDF application | The adapters' platform dependencies stated once for the packager |
| D14 | **Every part closed;** each part is re-read at dispatch against the delivered earlier parts without reopening D1 to D13 | — |

No decision changes an ADR or an architecture edge. D12 adds the `unsafe` allowlist file and its lint (doc 18 §11.2 as
planned).

## 5. Part 1: the SVG export completed

**Scope.** D2 to D4; FT-233. **0.6 pw.**

**Build list.**
1. `crates/ew-export/src/font_subset.rs` (new): `subset_glyf(font: &[u8], glyphs: &BTreeSet<u16>) -> Result<Vec<u8>,
   SubsetError>` (D3), bounded: a font over 16 MiB, a table outside the file or a composite deeper than 16 is an error,
   never a panic; the base64 encoder.
2. `crates/ew-export/src/svg.rs`: `SvgOptions.faces` entries gain the font bytes (`SvgFace.data: Option<Arc<[u8]>>`);
   per page the used glyph IDs per face, the `<style>` with one `@font-face` per used face; `SvgReport.named_fonts`.
3. `crates/ew-api/src/output/mod.rs`: the faces with `ew_text::bundled()`'s bytes.
4. The acceptance runner: an AC-37 (SVG) entry; the module note of `svg.rs` and its "as built" text.

**Golden change** (decided by the orchestrator under the owner's standing delegation of 2026-10-27, re-blessed and
recorded): `golden/J-E-2416/expected/acceptance_headless.md` (AC-14's SVG SHA-256 and the new AC-37 entry); any test
that pins the SVG digest. The PDF, PNG, text, rows and CSV files do not change; if one does, the lane stops.

**Acceptance.**
- **SV-1** `ew export svg golden/J-E-2416.ew --out d` writes 36 files; each page's `<style>` declares exactly the faces
  its `<text>` elements name (folio 103: Noto Sans Regular and the faces its texts use, the lane lists them); every
  `<text>`'s characters map through the embedded subset's `cmap` to glyphs with outlines (D4's test with `skrifa`).
- **SV-2** A subset keeps the face's glyph IDs: a glyph used on the page has the same outline bytes as in the bundled
  font; an unused glyph is empty; `head`'s checksum adjustment is valid; the dropped tables are absent. A composite glyph
  brings its components. A hostile font (a `loca` past `glyf`, a composite cycle) is refused with an error.
- **SV-3** AC-37's runner entry passes: every page keeps its text as `<text>` (3938 runs as in AC-14 at cycle 45,
  ID-1105) and embeds what it names. AC-14's SVG entry passes with the new digest; serial and parallel exports stay
  byte-identical.
- **SV-4** No PDF, PNG or CSV byte changes; twice-run determinism; targeted checks for `ew-export`, `ew-api`, `ew`.
  Manual (G-M2 checklist): folio 103's SVG in a browser without Noto Sans installed looks like the PDF page.

*As built (cycle 45, B45; decision log ID-1122):* a page's subset of a face holds the glyphs of its runs and the cmap
glyphs of the characters written; a face the subsetter refuses (CFF or any other refusal), a face of a collection or a
run glyph ID above 65535 is named only and counted in `named_fonts`; `.notdef` stays empty (D3). AC-14's SVG digest is
d6515093…; Golden A's 36 SVG files carry 45 subsets in 3.48 MB, and folio 103 embeds Noto Sans 400 only (a 35,540-byte
subset in an 80,480-byte page). SV-4's browser check stays a manual G-M2 item. Open: the hinting tables (FT-1045), two
hostile-case tests (FT-1046) and the characters collected as written (FT-1047).

## 6. Part 2: the print set and ew print

**Scope.** D5 to D9, D13; FR-EXT-009, FR-OUT-017's headless half. **0.9 pw.**

**Build list.**
1. `crates/ew-api/src/output/mod.rs`: `PageSelection::Sections`; `crates/ew-api/src/output/print.rs` (new):
   `PrintOptions`, `Paper`, `Fit`, `Session::print_set`, D7's page composition and the stamp.
2. `crates/ew-print/src/lib.rs`: `PrintAdapter`, `PrintJob { title, pdf, pages, printer, monochrome }`, `PrintOutcome`,
   `PrintError`; `src/lp.rs`, `src/viewer.rs`.
3. `bin/ew`: `ew print`, its catalogue entry, `schemas/ew.print-result-1.json` (the naming of the other schemas), one
   line in `docs/release-notes/unreleased.md`; `--sections` also on `ew export pdf|svg|png` (the same selection).
4. Golden and tests: D8's files.

**Acceptance.**
- **PR-1** `ew print golden/J-E-2416.ew --sections 1 --paper A5 --fit --out p.pdf` writes the pages 100, 101, 102,
  103, 106 on A5 landscape sheets, each stamped "scaled 71 %"; its digest and structure equal D8's files; `--paper A4
  --fit` gives no stamp (100 %); `--pages 101-106` the four folios 101, 102, 103 and 106.
- **PR-2** A fixture with an A3 page printed `--paper A4 --fit` carries "scaled 71 %"; `--paper A4` without `--fit`
  clips it and the result's notices name the page; the three refusals of D6 and those of D9, whole-text.
- **PR-3** (FR-OUT-018) For every page of PR-1 the inner display list equals the export page's, item for item, and the
  only other item is the stamp; `--mono` draws as the monochrome PDF does.
- **PR-4** `--via lp` runs `lp` with the arguments of D9: the adapter takes the program to run as a parameter, so the test runs a stand-in program that records its arguments, asserts the list and that the temporary file is gone afterwards; `--via viewer` writes the file and spawns the opener (the same parameter). Manual (FR-EXT-009, per OS):
  `ew print --pages 101-106` prints the four folios on the default printer on Linux.
- **PR-5** No existing golden byte changes; twice-run determinism of the print set; targeted checks for `ew-api`,
  `ew-print`, `ew`; `check-deps` with no new edge.

## 7. Part 3: printing from the window

**Scope.** D10, D11; FR-OUT-017's window half. **0.8 pw.** Every brief of this part cites
[doc 16a](../../design/16a-design-language.md).

**Build list.** `crates/ew-viewmodel/src/actions.rs` (`file.print`, Ctrl+P), the shell's print dialog state and job,
`crates/ew-ui` (the pre-dialog and preview), `strings.rs`; `crates/ew-print/src/portal.rs` with D11's dependency;
`bin/elektrawatt` wiring the adapters.

**Acceptance.**
- **PW-1** Headless: `file.print` with Range Sections 1, Paper A5, Scale to fit builds the set of PR-1 (the same
  digest); the preview's page 1 of 5 shows the stamp; Escape closes the dialog and prints nothing.
- **PW-2** The set is built as a job: an edit made while it builds is applied at once, the set holds the revision it
  started from, and cancel leaves no file.
- **PW-3** On Linux the portal adapter is called with the set (a test double of the adapter trait records the call);
  a failing portal falls back to `lp` with the notice of D11.
- **PW-4** Window check (offscreen, `--visual`): the pre-dialog with its preview at 1920 × 1080 and at the 1280 × 800
  floor, no truncated text; each text finding quotes its `strings.rs` key. Manual: a print through the portal on a
  Linux desktop.
- **PW-5** Targeted checks for `ew-viewmodel`, `ew-ui`, `ew-print`, `elektrawatt`; the snapshot gate; `cargo deny check`
  with ashpd's packages listed in the lane's report.

## 8. Part 4: the Windows adapter

**Scope.** D12; FR-OUT-017 and FR-EXT-009 on Windows. **0.7 pw.**

**Build list.** `crates/ew-print/src/windows.rs`, `crates/ew-print/Cargo.toml` (the lints table and the `windows`
dependency), `unsafe-allowlist.toml` (new), `xtask/src/` (the `check-unsafe` lint), the CI job line that runs it, doc 18
§11.2's as-built note at the design pass.

**Acceptance.**
- **WN-1** `cargo xtask check-unsafe` passes with the one entry and fails on a fixture crate that uses `unsafe` outside
  the list or drops `forbid(unsafe_code)` without an entry; every `unsafe` block in `windows.rs` has a `SAFETY:`
  comment (clippy's `undocumented_unsafe_blocks`).
- **WN-2** The band renderer is pure and tested without a printer: the bands of a page at 300 dpi stacked equal
  `ew-render-cpu`'s PNG of the page at 300 dpi pixel for pixel; band memory is bounded by 256 rows.
- **WN-3** Manual (FR-EXT-009 and FR-OUT-017 on Windows): `ew print --pages 101-106 --printer "Microsoft Print to PDF"`
  writes four pages; the window's Print… opens the Windows dialog and prints section 1; an A3 fixture on A4 shows
  "scaled 71 %".
- **WN-4** Targeted checks for `ew-print` on Windows, `xtask`; the Linux build unchanged (`cfg`).

## 9. Neighbours and who owns what

| Neighbour | It owns | This package |
|---|---|---|
| WP-S3.04 (M1) | The PDF, PNG and SVG writers as built, FT-234, FT-235 | Takes FT-233 (D2); FT-234 and FT-235 stay |
| WP-S1.22 (part 5 in this cycle) | CLI conventions, the CLI table, schemas, the API guard | `ew print` and `--sections` under them (its D2 names `ew print` here); after part 5 the CLI snapshot is re-blessed and a release-note line added |
| WP-S2.01 (complete) | The shell and its menus | Part 3 adds `file.print` |
| WP-S2.09 (complete) | `ew-jobs` | The print job |
| WP-S6.06 | Packaging, FT-234 | D13's needs into its manifests |
| WP-S3.01 | Screen geometry | FR-OUT-018's screen share stays there |

## 10. Files owned and what a lane may not decide

### 10.1 Files owned

**Part 1:** `crates/ew-export/src/{font_subset.rs,svg.rs,lib.rs}`, `crates/ew-export/tests/**`, `crates/ew-export/Cargo.toml`
(the `skrifa` dev-dependency only), `crates/ew-api/src/output/mod.rs` (the faces only), the acceptance runner's AC-37
entry, the golden files of §5. **Part 2:** `crates/ew-api/src/output/{mod.rs,print.rs}`, `crates/ew-print/src/{lib.rs,
lp.rs,viewer.rs}`, `bin/ew/**`, `docs/release-notes/unreleased.md` (lines), `golden/J-E-2416/expected/print/**`.
**Part 3:** `crates/ew-viewmodel/src/{actions.rs,shell.rs,shell/**,strings.rs}` for the print action and dialog,
`crates/ew-ui/src/**` for the dialog, `crates/ew-print/src/portal.rs`, `crates/ew-print/Cargo.toml` (ashpd),
`bin/elektrawatt/src/**` (the adapter wiring). **Part 4:** `crates/ew-print/src/windows.rs`, `crates/ew-print/Cargo.toml`,
`unsafe-allowlist.toml`, `xtask/src/` (the lint), `.github/workflows/ci.yml` (the lint's step).

Shared append-only files (the CLI table, the release notes, `Cargo.lock`, `REUSE.toml`) may be touched by other lanes;
the second to integrate rebases. Read-only for every lane: other crates, `golden/**` except as named, `ci/**`, `docs/**`
except the release notes and generated reference pages.

### 10.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a change needs a new workspace package, a workspace edge, or an external crate other than ashpd (part 3, within
   D11's stop) and the `windows` features of D12 (part 4);
2. an existing exported byte of Golden A changes outside part 1's list;
3. `unsafe` would appear outside `crates/ew-print/src/windows.rs`;
4. a print page would draw anything besides the export page's list and the stamp;
5. a golden fact here (section 1's five pages, "scaled 71 %", 36 SVG files, 3938 runs) is contradicted by the data;
6. the work of a neighbour (§9) is reached.

The lane decides freely: private names, module layout inside the owned files, the stamp's position and size of D6
within its stop (reported), the dialog's layout within doc 16a, test names and fixtures.

## 11. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 Large-format printing exceeds memory or spool size on Windows (R-037) | Medium · medium | 256-row bands (D12); the viewer adapter as the fallback; not measured (owner's rule) |
| R2 A browser renders the embedded subset differently (hinting, missing table) | Low · medium | The kept tables are those browsers require for TrueType; D4's manual check before G-M2 |
| R3 ashpd brings a heavy or conflicting graph | Medium · low | D11's stop; the `lp` and viewer adapters work without it |
| R4 The Windows FFI cannot be proven in CI | Certain · medium | WN-2 proves the pixels without a printer; WN-3 is manual; the viewer adapter remains |
| R5 Part 1's SVG re-bless collides with another lane's SVG change | Low · low | The golden change lists its files; the second lane rebases and re-blesses |

## 12. Size, order and routing

**Size.** 3.0 pw plus 0.3 pw reserve: part 1 **0.6** (the subsetter 0.3, the SVG wiring 0.1, tests and the re-bless
0.2), part 2 **0.9** (sections and print options 0.15, the print set and stamp 0.3, the adapters `lp` and `viewer` 0.15,
`ew print` 0.15, goldens and tests 0.15), part 3 **0.8** (the action, dialog and preview 0.45, the portal 0.2, window
check and tests 0.15), part 4 **0.7** (the GDI adapter 0.35, the band renderer and its test 0.15, the `unsafe` lint 0.2).
**De-scope rule:** when a part is projected or found more than 20 % over its size, these leave as follow-ups without a
further ruling, in this order: for part 2 `--sections` on `export pdf|svg|png` (kept on `ew print`); for part 3 the
portal (Linux prints through `lp` and the viewer), then the preview's page navigation (page 1 only); **part 4 as a
whole** (Windows then prints through the viewer adapter at the MVP, and the part becomes a follow-up).

**Order and routing** (Opus or Haiku only, ID-700). Part 1 **Opus at `high`** (a font file format and a golden SVG
change; feasibility read flagged for Opus `low`), at any time; part 2 **Opus at `medium`** (a new output on existing
writers, process spawning), before parts 3 and 4; part 3 **Opus at `medium`** (UI on existing patterns, one new
dependency); part 4 **Opus at `xhigh`** (the product's first `unsafe` FFI; a mistake is memory safety); the owner
approved it (§13 O3, ID-1030), and the cycle's reviewer reviews each `unsafe` block. No Haiku lane.

## 13. Open questions, follow-ups and the documents to update

**Open questions for the orchestrator.**
- **O1** The PERT of D1 (O 2.3 · L 3.2 · P 4.8 · PERT 3.3). Options: (1) enter it; (2) keep 3.2 and plan part 4 as the
  de-scope candidate from the start. Recommendation: (1).
- **O2** FR-OUT-017's acceptance says section 1 prints "101–106 only"; section 1 holds the divider 100. Options: (1) the
  divider prints with its section (D6), and 02a's acceptance gets an as-built note "100 to 106, no page outside the
  section"; (2) a section print leaves dividers out. Recommendation: (1), the outline and the PDF already group the
  divider with its section.
- **O3** Part 4 adds the product's first `unsafe` (doc 18 §11.2's planned exception) and NFR-SEC-04 asks for a second
  maintainer's review of every `unsafe` block (CODEOWNERS, a GitHub setting). Options: (1) approve part 4, with the
  cycle's independent reviewer as the second review and the CODEOWNERS entry left to the owner; (2) defer part 4 past
  the MVP, Windows printing through the viewer. Recommendation: (1) if the pilot prints on Windows, which the plan
  assumes; it needs the owner's word, not a delegated ruling.
- **O4** A G-M2 owner checklist with two manual items: folio 103's SVG in a browser without the fonts (D4) and a print
  per OS (FR-EXT-009). Recommendation: add it with WP-S3.08's O4.

**Rulings** (decision log ID-1017, ID-1030). **O1:** option (1), the PERT 3.3 pw entered (project owner (delegated)).
**O2:** option (1): section 1 prints with its divider 100; FR-OUT-017's acceptance gets the as-built note "section 1
prints 100 to 106: the divider belongs to its section, and no page outside the section prints" after part 2.
**O3:** answered by the owner (2026-10-31, ID-1030): approve. Part 4, the Windows GDI print adapter, is in scope; the
`unsafe` review NFR-SEC-04 asks for is done by the cycle's independent reviewer, whose brief names each `unsafe` block
of `crates/ew-print/src/windows.rs`; the CODEOWNERS entry is not needed for it. The delegated ruling that first
de-scoped part 4 is superseded, and §12's de-scope rule for part 4 applies only to size. **O4:** option (1), one G-M2
owner checklist with folio 103's SVG in a browser without the fonts and a print per OS, beside WP-S3.08's O4.

**FT annotations, proposed** (the design pass edits the rows): **FT-233** moves to WP-S3.07 part 1 and closes with it.
WP-S3.07 has no other FT row. Proposed (not opened):
- **FP-1** A print scheme beside the export schemes (doc 09 §2.9) and remembered print settings.
- **FP-2** Embedded subsets for `CFF ` project fonts in SVG.
- A de-scoped item of §12 becomes a new FT at the design pass.

**Documents.** **After part 1:** doc 09 §4.4 (the SVG fonts as built: retain-GID subsets, dropped tables); 02a
FR-OUT-013 (the partial note removed); doc 20 §21 (AC-37's runner entry). **After part 2:** doc 09 §4.5 (the print set,
`Sections`, the margin and stamp, the `lp` and viewer adapters); doc 17 §3.3 (`ew print` as built, `--sections`); doc 20
§3.1 (`expected/print/`); 02a FR-OUT-017 (O2's note). **After part 3:** doc 09 §4.5 and §5 (the pre-dialog and preview
as built, the portal through ashpd); doc 16 (File and Output commands). **After part 4:** doc 09 §4.5 (the Windows
adapter as built); doc 18 §11.2 (the allowlist file and lint as built); arch §3's `ew-print` row (its dependencies).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S3.07, WP-S3.04, WP-S3.01, WP-S1.22, WP-S2.01, WP-S2.09, WP-S6.06;
  Appendix B (FT-233, FT-234, FT-235).
- [Decision log](../06-decision-log.md): ID-344, ID-345, ID-370, ID-700, ID-880.
- [Reports](../../design/09-reports-forms-output.md) §1.3, §4.4, §4.5, §5; [UX](../../design/16-ux-ui.md);
  [design language](../../design/16a-design-language.md); [quality](../../design/18-quality-security-release.md) §11.2;
  [extensibility](../../design/17-extensibility.md) §3; [golden project](../../design/20-golden-project.md) §21;
  [architecture](../../design/04-architecture.md) §3.
- [ADR-0015](../../adr/0015-display-list-renderers-pdf.md); [risk register](../04-risk-register.md) R-037.
