# Completeness review: gaps, contradictions and corrections across the ElektraWatt research notes

> Critic's note for the ElektraWatt design team. Date: 2026-10-17. Inputs: the shared brief and all eight
> research notes (`eplan-features`, `standards`, `rust-ecosystem`, `interop-formats`, `oss-landscape`,
> `reference-drawing`, `panel-manufacturing`, `plc-bus-fluid`), read in full. This note does three jobs:
> - it lists topics that no note covers adequately, and researches each one;
> - it resolves contradictions between notes, with a verdict where the evidence allows one;
> - it checks claims that looked wrong or outdated.

---

## 0. Method and evidence quality

- **The shared WebSearch budget was used up before this agent started.** Every search call was refused.
  WebFetch could not resolve most non-GitHub hosts (`eur-lex.europa.eu`, `cra.orcwg.org` and others failed with
  `ENOTFOUND`).
- **What was reachable:** GitHub (raw files, shallow and sparse clones), GitLab (KiCad source) and the crates.io
  API. I therefore verified facts against **primary artefacts hosted there**:
  - the Eclipse ORC WG's CRA FAQ sources ([orcwg/cra-hub](https://github.com/orcwg/cra-hub));
  - Microsoft's own documentation repositories ([MicrosoftDocs/win32](https://github.com/MicrosoftDocs/win32),
    [MicrosoftDocs/azure-docs](https://github.com/MicrosoftDocs/azure-docs));
  - the Rust, wgpu, wasmtime, egui, krilla, AccessKit, Slint, KiCad (10.0.7) and QElectroTech source trees;
  - a GitHub mirror of the EPLAN 2026 API reference
    ([binfen1/Eplan_2026_IA_MCP_scripts](https://github.com/binfen1/Eplan_2026_IA_MCP_scripts));
  - crates.io metadata, queried today.
- **Claims marked (unverified)** come from my own knowledge, or from a source I could name but not open in this
  session. Treat them as hypotheses until checked.
- "Defect n" refers to the brief's defect list. "D-nn" and "REF-nnn" refer to `reference-drawing.md`.

---

## 1. Executive summary

1. **Two of the brief's nine defects are wrong.** I checked the PDF text layer:
   - Folio 902's empty "Next:" is correct, because 902 is the last page (defect 7).
   - Only folio 101 has the deviating description, not 101 and 106 (defect 6).

   `reference-drawing.md` had already found both. `eplan-features.md`, `standards.md`, `oss-landscape.md` and
   `panel-manufacturing.md` still cite defect 7 as "Next empty". Design docs must use the corrected list (§3.1).
2. **The EU Cyber Resilience Act (CRA) is missing from every note.**
   - The CRA's reporting obligations have applied since **11 September 2026**. All other obligations apply from
     **11 December 2027**.
   - A non-monetised open-source project is out of scope. If the pilot panel builder monetises ElektraWatt beyond actual costs
     (for example paid support for profit), it becomes a *manufacturer*, and SBOMs, vulnerability handling and CE
     conformity follow
     ([ORC WG CRA FAQ](https://github.com/orcwg/cra-hub/blob/main/faq/cra-itself/timeline.md)).
   - This affects the funding and commercial model in `oss-landscape.md` (§11).
3. **No note designs crash safety, saving or locking for the recommended multi-file project directory.**
   - Saving a directory of files is not atomic as a set.
   - Windows still applies `MAX_PATH` = 260 unless the user opts in on two levels, and it reserves names such as
     `CON` and `AUX`.
   - Projects live on SMB shares and in OneDrive folders.

   §4 (G1) gives a concrete protocol: manifest-last commit, a recovery journal, KiCad-style lock files and a git
   merge driver.
4. **Undo/redo, format migration, performance targets, project-data i18n, accessibility, telemetry, plugin and
   supply-chain security, and onboarding are covered only in passing, or not at all.** Each is researched below
   (G2–G13).
5. **Several inter-note contradictions are resolved with primary evidence:**
   - **EDZ is 7-Zip, not ZIP.**
   - **TIA Portal V21 writes CAEX 2.15 with AR APC 1.4.0, and its CAx export does contain tags.**
   - **krilla has no optional content (layers).**
   - **Contributor Covenant 3.0 exists** (published July 2025).
   - **Azure Trusted Signing is now "Artifact Signing", and EU organisations are eligible but EU individuals are
     not.**
   - **x86_64 macOS is now a Rust Tier 2 target.**
   - The rust-ecosystem note's own "fewer than 10k downloads = watch-list" rule rules out several crates that the
     other notes say to "use directly" (cadrum, gsd-parser, plcopen, l5x, ioddengine).
6. **One model decision is implicit in the notes but never stated:** the connection model.
   - EPLAN *autoconnects* aligned connection points and generates connections from them.
   - QET and KiCad store *explicit* wire objects.
   - The reference needs T-junction sequences (REF-034).

   §4 (G5) recommends explicit logical connections, with autoconnect as an editing aid.
7. **A coordinate-unit inconsistency hides a QET import trap.**
   - `rust-ecosystem.md` says "integer µm"; `panel-manufacturing.md` says "f64 mm, quantised to 0.1 mm".
   - QET's 10 px grid is 2.6458333… mm, which no decimal integer unit represents exactly.

   §4 (G4) proposes i64 nanometres plus rational grid definitions. This matches KiCad, whose PCB unit is 1 nm and
   whose schematic unit is 100 nm
   ([base_units.h](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/include/base_units.h)).

---

## 2. Coverage matrix (before this note)

● adequate · ◐ partial / in passing · ○ absent

| Topic | eplan-features | standards | rust-eco | interop | oss | reference | panel | plc |
|---|---|---|---|---|---|---|---|---|
| UX, interaction model, navigators | ◐ (navigator list) | ○ | ◐ (toolkits) | ○ | ◐ | ○ | ◐ (snapping) | ◐ (PLC navigator) |
| Undo/redo, transactions | ○ | ◐ (RD-08) | ◐ | ○ | ◐ (command bus) | ○ | ○ | ◐ (diff preview) |
| Project file layout, VCS | ◐ (EPLAN files) | ○ | ◐ | ○ | ● (KiCad lessons) | ○ | ◐ (`.ewpp`) | ○ |
| Atomic save, autosave, recovery, locking | ○ | ○ | ○ | ○ | ○ | ○ | ○ | ○ |
| Format versioning, migration, archiving | ○ | ○ | ○ | ○ | ● (LibrePCB/KiCad policy) | ○ | ○ | ○ |
| Performance targets, project sizes | ○ | ○ | ◐ (tentative) | ○ | ◐ (QET load issues) | ○ | ◐ (collision budget) | ○ |
| Supported OS/hardware, RDP/VDI | ○ | ○ | ◐ (open question) | ○ | ○ | ○ | ○ | ○ |
| Multilingual project data | ● (EPLAN feature) | ○ | ◐ ("keep separate") | ◐ (EDZ strings) | ○ | ○ | ○ | ◐ (AML comments) |
| UI translation | ○ | ○ | ◐ (Fluent) | ○ | ◐ (Weblate) | ○ | ○ | ○ |
| Printing | ○ | ○ | ● | ○ | ○ | ○ | ○ | ○ |
| Plugin and script security | ○ | ○ | ◐ | ◐ (untrusted input) | ◐ (SECURITY.md) | ○ | ○ | ○ |
| Supply chain, SBOM, signing | ○ | ○ | ◐ (cargo-deny) | ○ | ◐ | ○ | ○ | ○ |
| EU regulation (CRA, PLD, EAA) | ○ | ◐ (Machinery Reg.) | ○ | ◐ (database right) | ○ | ○ | ◐ (DoC templates) | ○ |
| Accessibility | ○ | ○ | ◐ | ○ | ○ | ○ | ○ | ○ |
| Onboarding, docs, training | ○ | ○ | ○ | ○ | ◐ | ○ | ○ | ○ |
| Telemetry, privacy | ○ | ○ | ◐ (Sentry opt-in) | ○ | ○ | ○ | ○ | ○ |
| Fonts and lettering | ○ | ○ | ◐ | ◐ (SHX) | ○ | ◐ (Tahoma) | ○ | ○ |
| Coordinates, units, grid | ○ | ◐ (frames) | ◐ (µm) | ◐ (QET px) | ○ | ◐ | ◐ (f64 mm) | ○ |

---

## 3. Contradictions and corrections

### 3.1 Corrections to the brief (verified against `ref/reference.txt`)

| Brief item | Finding | Evidence | Notes that still carry the error |
|---|---|---|---|
| Defect 6: description differs on "pages 101, 106" | Only **101** reads "Control Cabinet Expansion". 106 reads "Expansion Control Cabinets". 106 differs only in *revision* (V2.0) | Text layer: `Description:` per page (line 242 vs 460) | — |
| Defect 7: "Cabinet layout 'Next:' empty" | **Not a defect.** 902 is the last folio (`Next:` empty at line 961; 901 → `Next: 902`). The real navigation gaps are the missing PDF outline, link annotations and folio page labels (`reference-drawing.md` C2, D-07) | Text layer | eplan-features §2.5 and Implications 1/5; standards §3 software rules and Implications 6; oss §2.5 and Implications 3; panel §2 table and §6.10 |
| Folio 902 "DIN rails" | No rails are drawn: 5 ducts, device rectangles and 4 end brackets only | `reference-drawing.md` C4; `panel-manufacturing.md` §2 | — |
| Ports "2/4/3/5(15)" on 203 | Reads "3 1 5" (exhaust 3, supply 1, exhaust 5) | `reference-drawing.md` C3 | — |

**Action.** The design documents should cite the *revised* defect list (D-01 … D-42 in `reference-drawing.md`),
not the brief's nine. In particular, replace defect 7 with "PDF has no bookmarks, no hyperlinks, physical page
labels".

### 3.2 Contradictions between notes

| # | Topic | Positions | Verdict | Evidence |
|---|---|---|---|---|
| C-1 | **EDZ container** | rust-eco §6.4: "zip for EDZ … both zip-based"; eplan-features OQ1: "ZIP vs 7z"; interop §2.2: 7z, some ZIP | **7-Zip.** QET's importer supports only 7-Zip-format EDZ and calls ZIP-format packages unsupported. Use `sevenz-rust2` 0.24.0 (Apache-2.0, MSRV 1.93) plus `zip` for the ZIP minority | [QET EDZ README](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/import/edz/README.md) ("A `.edz` is a 7-Zip archive …"); crates.io |
| C-2 | **TIA CAx and tags** | interop §4.2: "Program and PLC tags: not part of CAx"; plc §3.1: tags are present | **plc-bus-fluid is right.** A real TIA V21 export (`WriterVersion` 2100) contains `LogicalAddress` tag attributes and `InternalLink`s from channel to tag | [TIA_Export.aml](https://github.com/JBuschkind/Interoperabilit-tsprojekt/blob/HEAD/Testing-Files/TIA_Export.aml): `SchemaVersion="2.15"`, `DocumentIdentifier="AR APC" Version="1.4.0"`, 2 × `InternalLink` |
| C-3 | **CAEX version to write** | interop §4 / Impl. 6: "typed CAEX 3.0 model (and 2.15 for older tools)"; plc: "CAEX 2.15 first" | **Write 2.15 for TIA; read both.** TIA V21 still writes 2.15 | as C-2 |
| C-4 | **DWG route** | oss Impl. 15: LibreDWG (GPL-3) or external converter; rust-eco and interop: `acadrust` (MPL-2.0) behind a flag, LibreDWG only if GPL | **acadrust plus the user-installed ODA File Converter.** The brief allows C/C++ FFI "only where justified, e.g. a B-rep/STEP kernel". A pure-Rust DWG crate exists, so LibreDWG FFI is not justified. acadrust: 0.6.4, MPL-2.0, 33k downloads in 90 days; it still needs the corpus test the notes ask for | brief; crates.io |
| C-5 | **STEP binding** | rust-eco Impl. 5: start with **cadrum** (prebuilt static OCCT 8.0.1); interop Impl. 9: opencascade-rs, *dynamically linked*; panel: either | **Internal inconsistency.** rust-eco's own rule says 2026-vintage crates under 10k downloads are watch-list only. cadrum has 6,872 downloads in 90 days and no MSRV. cadrum also **downloads prebuilt binaries at build time**, a supply-chain and SBOM liability under the CRA (G11). **Recommendation:** `occt-sys` / `opencascade` (108k / 1.2k downloads in 90 days, LGPL-2.1) or a system OCCT in the out-of-process worker. Keep cadrum as an evaluated option | crates.io |
| C-6 | **OCCT licence exception** | rust-eco: "LGPL-2.1 + Open CASCADE exception (exception wording unverified)" | **The exception does not relax static linking.** It only lets object code that incorporates *header-file material* (inline code, CDL generics) be distributed "under terms of your choice", with a prominent notice. LGPL §6 relinking obligations still apply to the library. With a GPL-3 app this is moot; with a permissive app, ship the worker dynamically linked or with relinkable objects | [OCCT_LGPL_EXCEPTION.txt](https://github.com/Open-Cascade-SAS/OCCT/blob/master/OCCT_LGPL_EXCEPTION.txt) |
| C-7 | **Symbol-library licence** | standards §4 / Impl. 15: CC-BY-4.0 *or* CC0 with a design exception; oss §7.3: **CC-BY-SA-4.0 + KiCad-style exception**; plc Impl. 9: "CC-BY/CC0" | **Decision needed; recommend oss's choice**, because KiCad's precedent is proven at scale (exception in [kicad-symbols LICENSE](https://gitlab.com/kicad/libraries/kicad-symbols/-/blob/master/LICENSE.md)). Whatever is chosen, every note and the CONTRIBUTING file must use the same SPDX expression | — |
| C-8 | **Plugin exception vs porting QET code** | oss §7.3 recommends GPL-3.0-or-later with a §7 plugin exception, *and* (§2.6, Impl. 8) porting QET's GPL-2.0-or-later parser code | **These conflict.** An additional permission can only be granted by the copyright holder. QET-derived files would carry no plugin exception. Either clean-room the QET importer (interop Impl. 3), or keep ported code in a component that plugins never link against (a separate process or crate behind the WIT boundary) | GPL-3.0 §7 (general knowledge) |
| C-9 | **Crate-adoption threshold** | rust-eco §0 / Impl. 15: under 10k downloads → watch-list; plc §16: `gsd-parser` "use directly"; Impl. 7 relies on `taktora-ethercat-esi`, `ioddengine` | **Apply the rule consistently.** Downloads in the last 90 days: gsd-parser 58, plcopen 83, l5x 100, taktora-ethercat-esi 261, ioddengine 4. Use them in tests, or vendor or fork them with a named owner. Do not take them as unreviewed runtime dependencies | crates.io, 2026-10-17 |
| C-10 | **Default PDF/A level** | rust-eco Impl. 6: "targeting **PDF/A-3b**"; standards, interop, reference: **2b** default, 3b/4f when embedding | **2b default, 3b/4f only with attachments.** krilla supports embedded files ([embed.rs](https://github.com/LaurenzV/krilla/blob/main/crates/krilla/src/interchange/embed.rs)). The title block's confidentiality notice argues against embedding the native project by default | — |
| C-11 | **Optional content (layers) in PDF** | interop §8.1: "OCG not in krilla's feature list (unverified gap)"; rust-eco silent | **Confirmed: krilla does not support optional content.** Its PDF/A-4 conformance notes say so. A patch upstream or a `pdf-writer` post-step is needed if layers are wanted | [PDF_A4.md §6.10](https://github.com/LaurenzV/krilla/blob/main/crates/krilla/src/configure/PDF_A4.md); krilla MSRV 1.92 confirmed ([README](https://github.com/LaurenzV/krilla/blob/main/README.md)) |
| C-12 | **Coordinates** | rust-eco §7: "integers (µm) or fixed decimals"; panel §4.1: "f64 mm … quantise to 0.1 mm on save"; interop: QET px at 96 dpi | **Unify on i64 nm in files** (G4). 0.1 mm quantisation is too coarse for symbol geometry, and µm cannot represent QET px exactly either | KiCad: PCB 1 nm, schematic 100 nm ([base_units.h](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/include/base_units.h)) |
| C-13 | **Code of conduct version** | oss §8.2: "Contributor Covenant 2.1 (latest tagged version)" | **Outdated.** **Contributor Covenant 3.0** was published on the project's release branch in July 2025, but it has no git tag (tags stop at 2.1) | [content/version/3/0/code_of_conduct.md](https://github.com/EthicalSource/contributor_covenant/blob/release/content/version/3/0/code_of_conduct.md), git log 2025-07-21 … 08-04 |
| C-14 | **QET example corpus size** | interop: 24 example projects; oss: "23 files, 22 distinct" | **24 `.qet` files on master.** Two are near-duplicates (`cablage-…` / `câblage-…`) | `qelectrotech-source-mirror/examples` listing |
| C-15 | **QET cross-reference variable** | standards §2.2: QET default `%f-%l%c`; reference §5.2: format `%F-%l%c` | **Both are right, and the difference matters.** `%f` is the folio **position** (`folio_index+1`); `%F` is the folio **label**. The default is `%f-%l%c`, so the user's project must have been set to `%F-…`: "102-A1" would otherwise print "6-A1". The importer must read each project's `<xrefs>` and `<report>` formulas and must not assume the default | [assignvariables.cpp L405-L410](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/autoNum/assignvariables.cpp); [xrefproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/properties/xrefproperties.cpp) |

### 3.3 Claims checked for staleness

| Claim (note) | Status | Evidence |
|---|---|---|
| "Azure Artifact (Trusted) Signing … eligibility unverified" (rust-eco §10.1) | **Renamed to "Artifact Signing".** Public-trust certificates go to *organisations* in the US, Canada, **EU**, UK and others. *Individual* developers must be in the US or Canada. The pilot panel builder or a Dutch stichting qualifies; an individual Dutch maintainer does not | [quickstart.md](https://github.com/MicrosoftDocs/azure-docs/blob/main/articles/artifact-signing/quickstart.md); [overview.md](https://github.com/MicrosoftDocs/azure-docs/blob/main/articles/artifact-signing/overview.md) |
| wasmtime "48.0.0 is the next LTS by that rule (inferred)" (rust-eco §9) | **Confirmed rule:** releases whose number is a multiple of 12 are LTS, supported 24 months; others 2 months. Security fixes go to all supported releases | [stability-release.md](https://github.com/bytecodealliance/wasmtime/blob/main/docs/stability-release.md) |
| bincode unmaintained (rust-eco §7) | **Confirmed:** RUSTSEC-2025-0141 (2025-12-16), "informational = unmaintained". Suggests wincode, postcard, bitcode, rkyv | [advisory](https://github.com/rustsec/advisory-db/blob/main/crates/bincode/RUSTSEC-2025-0141.md) |
| Slint Royalty-free 2.0 terms (rust-eco, oss) | **Confirmed:** AboutSlint widget *or* public badge; no embedded systems; "does not permit the distribution of Application that exposes the APIs" | [LicenseRef-Slint-Royalty-free-2.0.md](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md) |
| CI runner labels "windows-2025, macos-15, ubuntu-24.04 (unverified)" (rust-eco §10.3) | **Valid.** Newer images also exist: `ubuntu-26.04(-arm)`, `macos-26` (arm64; `macos-26-intel`), `windows-11-arm`. macOS 14 images are deprecated | [runner-images README](https://github.com/actions/runner-images/blob/main/README.md) |
| Rust platform baseline (not stated in any note) | **Tier 1 with host tools:** `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` (Windows 10+, Server 2016+), `aarch64-apple-darwin` (macOS 11+), `x86_64-unknown-linux-gnu` (glibc 2.17+). **`x86_64-apple-darwin` is now Tier 2** (macOS 10.12+), so Intel Macs get weaker guarantees | [platform-support.md](https://github.com/rust-lang/rust/blob/main/src/doc/rustc/src/platform-support.md) |
| Rust stable "about 1.99 (computed)" (rust-eco §0) | **Consistent** with the 6-week train (1.99 ≈ 2026-10-01; 1.100 ≈ 2026-11-12) | computed |
| QET has "MCP server", "SignPath", CVE-2026-15037 (oss, interop) | **Confirmed** in QET's release notes. They also show **three rotating crash-recovery snapshots** (#1167), relevant to G1 | [QET_release_notes_0.100_to_master.md](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/QET_release_notes_0.100_to_master.md) |
| egui_kittest fails on unnamed inputs; egui_mcp (rust-eco) | **Confirmed**, with a **security caveat:** the `inspection` feature listens on `127.0.0.1:5719` when `EGUI_INSPECTION=1`, and `EGUI_INSPECTION=0.0.0.0:5719` exposes it across the network. It must never be compiled into release builds (G10) | [egui CHANGELOG 0.35/0.37](https://github.com/emilk/egui/blob/main/CHANGELOG.md); [egui_inspection lib.rs](https://github.com/emilk/egui/blob/main/crates/egui_inspection/src/lib.rs) |
| Crate versions in rust-eco tables | **Spot-checked, all correct:** egui/eframe 0.37.0 (MSRV 1.95), bevy 0.20.0 (1.97.1), wasmtime 49.0.3 (1.96.0), krilla 0.8.2 (1.92), iced 0.14.0, slint 1.18.1, salsa 0.28.5, rust_xlsxwriter 0.100.0, zip 9.0.4 (MSRV 1.88), quick-xml 0.42.0. Note `imbl` is **MPL-2.0+** and `notify` is **CC0-1.0** | crates.io API |

---

## 4. Gap research

### G1. Project storage: atomic saves, recovery, locking and VCS hygiene

**Why it is a gap.** rust-eco §7 and oss §3.2 recommend a git-friendly text project made of many files. Neither
says how to save it safely, recover from crashes, prevent two users editing the same project, or survive Windows,
SMB and cloud-sync behaviour. For a CAD tool, data loss is the defect users forgive least.

**Facts.**
- **POSIX `rename` is atomic for one file. A set of files is never atomic.** A crash mid-save of a multi-file
  project leaves a mixed generation.
- **Rust std behaviour:**
  - `File::sync_all` uses `fcntl(F_FULLFSYNC)` on Apple platforms, so it really flushes the drive cache
    ([std unix fs](https://github.com/rust-lang/rust/blob/main/library/std/src/sys/fs/unix.rs)).
  - `std::fs::rename` on Windows replaces the target with `FILE_RENAME_FLAG_POSIX_SEMANTICS`, falling back to
    `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`
    ([std windows fs](https://github.com/rust-lang/rust/blob/main/library/std/src/sys/fs/windows.rs)).
- **Windows path limits:**
  - `MAX_PATH` is 260 characters.
  - Since Windows 10 1607, long paths work only if **both** the `LongPathsEnabled` registry value is 1 **and** the
    app manifest declares `longPathAware`
    ([Microsoft](https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/FileIO/maximum-file-path-limitation.md)).
  - Relative paths are always limited to `MAX_PATH`.
- **Reserved Windows names:** `CON, PRN, AUX, NUL, COM1–9, LPT1–9` (also with extensions, and with superscript
  digits) cannot be file names
  ([naming-a-file](https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/FileIO/naming-a-file.md)). A
  "file per device tag" layout would break on a device or page named `AUX`, and case-insensitive file systems
  collide `-K1` with `-k1`.
- **KiCad 10's lock file** (`include/lockfile.h`):
  - created with exclusive open;
  - stores JSON `{username, hostname}`;
  - is named with a prefix plus a lock extension next to the file
    ([lockfile.h](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/include/lockfile.h)).
- **KiCad 10's autosave and backup** use **git-based local history by default** (`BACKUP_FORMAT::INCREMENTAL`),
  with ZIP snapshots as the alternative. The location is the project directory or the user directory, with
  retention limits (`limit_total_files`, `limit_daily_files`, `min_interval`, `limit_total_size`)
  ([common_settings.h](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/include/settings/common_settings.h)).
- **QET** has three rotating crash-recovery snapshots (QET release notes #1167).
- **EPLAN** treats project open/close/restore as non-undoable file-system operations, and API code uses explicit
  `LockingStep` scopes ([LockingStep](https://github.com/binfen1/Eplan_2026_IA_MCP_scripts/blob/HEAD/Eplan_2026_RAG_mcp/Eplan_DOCS/Api/Asm/LockingStep.md)).
- **Git hooks for semantic tools.** Git supports a custom merge driver `merge.<name>.driver = cmd %O %A %B %L %P`
  (ancestor, ours, theirs, marker size, path) and `diff.<name>.textconv`
  ([gitattributes](https://github.com/git/git/blob/master/Documentation/gitattributes.adoc)).

**Recommended design.**

| Concern | Rule |
|---|---|
| File names | Derived only from UUIDs (`pages/018f…json`), never from tags or titles. ASCII lower-case hex. Keep the depth shallow so the full path stays under ~120 characters. Set `longPathAware` in the Windows manifest anyway |
| Layout | `project.json`, `structure.json`, one file per **page**, **sharded** domain tables (e.g. `devices/<uuid[0..2]>.json`: 256 shards), `parts-used.json`, `revisions.json`. This bounds file count (Windows Defender and OneDrive scan per file) and keeps merges local. A pure file-per-object layout (LibrePCB style) reaches 10⁴–10⁵ files on L/XL projects (§G6) |
| Commit protocol | (1) Write every changed file to `name.tmp-<gen>`, `sync_all`. (2) Rename over the target. (3) Write `manifest.json` **last**, holding `format`, `generator`, `generation`, and per-file SHA-256. (4) `sync_all` the directory where supported. On open, verify the hashes. A mismatch means an interrupted save or a hand/merge edit: offer recovery from the journal or the last good generation |
| Crash recovery | An append-only **operation journal** (JSON Lines of committed transactions) under the OS state directory (`etcetera`/`directories` crates), *not* in the project, so it never reaches git or sync. Snapshot every N minutes. On restart, offer replay. Optionally a **git local-history** backup like KiCad's, via `gix` 0.89.0 (MIT OR Apache-2.0, pure Rust) |
| Locking | `.~ew.lock` next to `manifest.json`, created with `create_new`. JSON `{user, host, pid, app_version, started_at}`. Open read-only if the lock is held. Allow a "take over" once the lock is stale (host matches and the pid is dead, or older than a configurable age). Do not rely on OS byte-range locks over SMB |
| External changes | `notify` 8.2.0 (CC0-1.0) watches the directory. Ignore the app's own writes by generation number. On a foreign change (git pull, colleague), offer reload or a 3-way merge |
| Cloud-sync folders | Detect projects inside OneDrive, Dropbox or iCloud paths (e.g. the `%OneDrive%` environment variables on Windows, **unverified**). Warn that partial sync can interleave generations. Detect conflict copies, i.e. unexpected files not in the manifest |
| Git integration | Ship `.gitattributes` (`*.json merge=elektrawatt diff=elektrawatt`, 3D binaries `binary` or LFS) and `.gitignore` (caches, per-user view state like KiCad's `.kicad_prl`). Provide `ew merge-driver %O %A %B %P` (semantic, entity-level) and `ew diff --textconv` |
| Transport | `.ewz` = ZIP of the directory (rust-eco). Opening an `.ewz` extracts it to a working directory, or opens read-only |

### G2. File-format versioning, migration and long-term archiving

**Gap.** oss-landscape documents KiCad's date-token versions and LibrePCB's "format changes only in majors"
policy. Nobody specifies *how* the Rust code evolves the schema, what happens when an old build meets a new file,
or how long old files must remain readable.

**Facts.**
- KiCad bumps a date-stamped version token and upgrades on read: about 25 schematic bumps between 2023-11 and
  2026-03 (`oss-landscape.md` §3.2).
- LibrePCB ties the file format to the major version and gives no forward compatibility (oss §4).
- `schemars` 1.2.3 (MIT) generates JSON Schema from serde types. `serde_path_to_error` 0.1.20 reports the exact
  failing path. `semver` 1.0.28 handles version parsing (crates.io).
- EPLAN's undo and project mechanisms treat "creating user-defined property definitions" as a schema-level
  change outside the undo protocol
  ([UndoStep remarks](https://github.com/binfen1/Eplan_2026_IA_MCP_scripts/blob/HEAD/Eplan_2026_RAG_mcp/Eplan_DOCS/Api/Asm/UndoStep.md)).
  User-defined properties are schema data, and they need their own versioning.
- **Retention (unverified):** the Machinery Regulation (EU) 2023/1230 requires manufacturers to keep technical
  documentation for at least 10 years. Panel builders' customers will expect EW projects from 2027 to open in 2037.

**Recommended rules.**
1. **Two-part format version** `ew-format = "1.3"`:
   - a **minor** bump is additive. Older readers ignore and **preserve** unknown fields (via
     `#[serde(flatten)] extra: BTreeMap<String, Value>` per entity) and warn;
   - a **major** bump breaks compatibility. Older readers refuse, or open read-only.
2. **Upgrade chain on `serde_json::Value`.** One pure function per step, `v1_2 → v1_3`, run before typed
   deserialisation. Never delete old steps. The CI corpus holds one fixture per released version (golden
   J-E-2416 + QET examples), opened by every new build.
3. **Generator stamp** (`"generator": "ElektraWatt 0.7.2"`, as KiCad does), plus a `min-reader` field (QETZ
   uses `<qetz format="4" min-reader="4">`; `interop-formats.md` §1.11).
4. **Embedded library copies.** Every symbol, part, form and plot frame a project uses is copied in, with
   `{library_id, item_id, version, sha256}`. "Update from library" shows a diff, so a project stays openable
   without its libraries (QET and KiCad behave this way).
5. **Publish JSON Schemas** for every file kind in the MIT/Apache `ew-format` crate (oss §7.3), and validate in
   CI.
6. **An archive profile ("as-built")** bundles project + embedded libraries + PDF/A-2b + production package
   (`.ewpp`) + checksums. It is the 10-year deliverable.

### G3. Undo/redo and transactions

**Gap.** rust-eco §8 lists the `undo` crate, `imbl` snapshots and a command pattern, and oss Impl. 6 says "the
API is the command bus". Nobody defines granularity, coalescing, scope, or the interplay with derived data,
saving, imports and external reloads.

**Facts.**
- **EPLAN's data-model undo** is project-wide. Each `UndoStep` groups changes. File-system and registry
  operations are not undoable. Any API change made *without* an `UndoStep` **clears the undo history**
  ([UndoStep](https://github.com/binfen1/Eplan_2026_IA_MCP_scripts/blob/HEAD/Eplan_2026_RAG_mcp/Eplan_DOCS/Api/Asm/UndoStep.md),
  [UndoManager](https://github.com/binfen1/Eplan_2026_IA_MCP_scripts/blob/HEAD/Eplan_2026_RAG_mcp/Eplan_DOCS/Api/Asm/UndoManager.md)).
- **KiCad's `COMMIT`** stages items as `CHT_ADD / CHT_REMOVE / CHT_MODIFY` (with a `CHT_DONE` flag for changes
  already applied), then `Push(message)` creates one undo point or `Revert()` rolls back
  ([commit.h](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/include/commit.h)).
- **Crate options:** `undo` 0.52.0 (MIT OR Apache-2.0) has only 8.7k downloads in 90 days. `imbl` 7.0.2
  (MPL-2.0+) provides structural sharing.

**Recommended design.**

| Aspect | Rule |
|---|---|
| Unit | One **transaction per user gesture or command**: place, move, renumber, import, apply quick-fix. The command layer (GUI, CLI, scripts, MCP) is the only writer, as oss recommends |
| Content | Each transaction stores invertible **field-level diffs** keyed by entity UUID (before/after values), not whole snapshots. `imbl` snapshots remain available for background jobs and previews |
| Derived data | Never part of undo. Tags, cross-references, potentials, BOM and routing are recomputed (salsa or a hand-rolled cache) after apply or revert. This keeps undo small and correct |
| Coalescing | Consecutive drags of the same selection merge. Typing in one property field merges until focus leaves. Bulk edits in a navigator table form one transaction |
| Scope | A single **project-wide** stack, like EPLAN. Each entry records the page and viewport, so undo navigates to the affected page and restores the selection |
| Limits | Count- and memory-bounded (e.g. 500 steps or 256 MB, configurable). Show the oldest available step |
| Save | Saving does not clear undo. A "saved" marker in the stack drives the dirty flag |
| Non-undoable | Library writes outside the project, exports, file renames: flag them in the command metadata and ask for confirmation, as EPLAN does |
| External reload | A git pull or another user's change invalidates the stack. Keep it only if the reload was an EW-generated merge with an explicit diff transaction |
| Journal | The same transaction records feed the crash-recovery journal (G1), the macro recorder, `ew diff` and the revision change log, giving one serialisation of "what changed" |
| Long operations | Renumbering 10k tags or importing a QET project runs as one transaction with a **preview diff** (as plc §2.9 describes for PLC readdressing) and is cancellable before commit |

### G4. Coordinates, units and grids

**Gap and conflict:** C-12. Neither note handles the QET grid.

**Facts.**
- QET scene units are 1 px = 25.4/96 mm = **0.264583… mm**, with a 10 px wiring grid (2.6458 mm) and 60 × 80 px
  frame cells (`interop-formats.md` §1.2; `reference-drawing.md` §5.2). Neither µm nor nm represents these values
  exactly.
- KiCad uses **1 nm** (PCB) and **100 nm** (schematic) integer units
  ([base_units.h](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/include/base_units.h)). A signed 32-bit nm
  range is ±2.147 m, too small for bayed cabinets. EW needs i64.
- EPLAN's schematic grids are in mm (sizes A–E, unverified values 1/2/4/8/16 mm), and IEC symbols are commonly
  built on a 4 mm connection grid (unverified).

**Recommendation.**
- **Storage:** i64 **nanometres** for every 2D and 3D linear coordinate in files and in the 2D model. Use f64 mm
  (glam `DVec3`) only for 3D maths and GPU staging. Store 2D orientation as an enum (0/90/180/270 + mirror) and 3D
  rotation as a quaternion rounded to 1e-9.
- **Grids are rational:** `Grid { num: i64, den: i64 }` in nm. A QET-compatible 10 px grid is 254/96 mm =
  `num = 7_937_500, den = 3` nm (exact). Snapping uses grid indices, not float equality, and stored coordinates
  round to the nearest nm (error ≤ 0.5 nm). Imported QET geometry then stays on its grid, so "102-A1"-style cell
  lookups remain exact.
- **Symbol library grid:** decide once whether connection points sit on a 2.5 mm or 4 mm grid. Never use the QET
  grid for new symbols. QET-imported symbols live in a per-project "QET import" library on their native grid
  (`interop-formats.md` §1.12).
- **Display units:** mm by default, inch optional. Report rounding comes from the profile, never from the stored
  value.

### G5. UX, interaction model and navigator design

**Gap.** eplan-features lists 16 navigators and rust-eco compares widget kits. Nobody designs the interaction
model, or how EPLAN and QET users will find their way.

**A decision nobody stated: the connection model.**

| Model | Used by | Pros | Cons |
|---|---|---|---|
| **Autoconnect**: connection lines generated from aligned connection points and connection symbols (angles, T-nodes with target direction) | EPLAN (`eplan-features.md` §3.4) | Fast drawing of regular ladder diagrams; wiring order via T-node direction | Moving a symbol can silently re-route logic; logic depends on geometry; complex to reimplement faithfully |
| **Explicit connections**: wire objects between terminals with auto-routed or edited paths | QET (`interop-formats.md` §1.4), KiCad | Robust to moves; direct QET import; simple model for wire IDs | Slower to draw ladders without assistance |

**Recommendation.**
- Make **explicit logical connections** the model: an edge between two connection points with a graphical route.
- Add **junction objects with a connection sequence** (REF-034, D-20) to represent daisy-chains and T-node order.
- Offer **autoconnect as an editing aid**: when points align on placement, *create* explicit connections and show
  them before commit.
- Wire numbers, CDPs (connection definition points) and ferrules then attach to stable connection IDs (panel
  §4.1).

**Interaction blueprint** (EPLAN feature names from `eplan-features.md`; concrete key bindings are proposals):

| Element | Recommendation | Rationale and evidence |
|---|---|---|
| Layout | Canvas centre. Left dock: page/device/terminal/cable/PLC/potential navigators. Right dock: property grid. Bottom dock: messages, search results, table editors (wire list, I/O list). Saved **workspaces** per task: Schematic, PLC, Panel 3D, Manufacturing | EPLAN's docked-navigator workflow; rust-eco's docking crates; multi-monitor via egui viewports |
| Command access | **Command palette** (fuzzy search over all commands, with their shortcuts), menus, compact toolbars. No ribbon in v1 | QET 2026 added "command search" and a shortcut bar (oss §2.4); the ribbon is costly and non-native in egui |
| Keymaps | Configurable, with presets **"EW default"**, **"KiCad-like"** (M move, R rotate, W wire, E properties) and **"EPLAN-like"** (bindings to be collected from EPLAN users; **unverified**) | Users arrive from EPLAN, QET and KiCad |
| Placement | While placing: rotate and mirror, cycle **symbol variants** (EPLAN A–H) and **representation** (multi-line / overview), and show a ghost preview of autoconnects. Placing a *part* creates the device's functions as **unplaced** entries in the device navigator (EPLAN function templates) | `eplan-features.md` §3.1, §3.2 |
| Selection ↔ navigator sync | Bidirectional. Selecting in a navigator highlights on the canvas, and the reverse. "Show all representations of this device" lists main, aux, overview, layout and 3D | Fixes defect 2 / D-02 at the UX level |
| Property grid | Schema-driven. Multi-selection shows `‹varies›`. **Inherited and derived values are shown greyed with their origin** (e.g. "0.75 mm² — from potential 24VDC-1 via profile the pilot user"), with an override marker and "reset to inherited" | Makes defect 8 / D-08 rules visible and auditable |
| Table editing | Every navigator has a table mode (100k-row virtualised; egui_table 0.10.0) with fill-down, pattern fill, paste from Excel, filter bar and a column chooser | PLC grid (plc §2.8), wire list, terminal editor |
| Find & replace | Project-wide, scoped (text, DT, property, function text, part number). Results list with jump. Replace with preview diff as one transaction (G3) | Not covered by any note; standard in EPLAN (unverified details) |
| Cross-reference navigation | Click a cross-reference (or interruption-point text) to jump. **Back/forward history** like a browser. Hover preview of the target region | Reference set: 22 interruption-point pairs, 9 main/aux sets |
| Checks | The message panel groups by severity and rule, with jump, **quick-fix** actions (e.g. "rename +J-E-2416 → canonical") and waivers with reason | `eplan-features.md` §9; reference "open items" (REF-094) |
| Canvas aids | Snap indicators; grid; inline dimension readout; **net/potential highlight** with a pattern as well as colour (G8); rubber-band connections while dragging | — |
| First-run | A company-standard wizard (G13) | Defects 3, 5, 6 come from absent standards |

### G6. Performance targets, hardware and OS baseline

**Gap.** rust-eco offers tentative targets (60 fps; 500 pages of PDF in under 10 s) and an open question on
Citrix/RDP. No note sizes projects or names a minimum platform.

**Facts.**
- **Platforms:** Rust Tier 1 covers Windows 10+ (x64, ARM64), macOS 11+ (arm64) and Linux glibc 2.17+. Intel
  macOS is Tier 2 (§3.3).
- **wgpu backends:** Vulkan, DX12 and Metal are first class. OpenGL 3.3 / GLES 3.0 is best effort. wgpu's own
  MSRV is 1.87 ([README](https://github.com/gfx-rs/wgpu/blob/trunk/README.md)).
- **Software rendering:** `RequestAdapterOptions::force_fallback_adapter` requests a software adapter
  ([adapter.rs](https://github.com/gfx-rs/wgpu/blob/trunk/wgpu-types/src/adapter.rs)). The DX12 backend
  recognises and works around **WARP** ("Microsoft Basic Render Driver")
  ([dx12/adapter.rs](https://github.com/gfx-rs/wgpu/blob/trunk/wgpu-hal/src/dx12/adapter.rs)). That is the path
  for GPU-less RDP and VDI on Windows. Linux would use llvmpipe or lavapipe (unverified in wgpu CI).
- **Project sizes in the market:** EPLAN P8 Basic is capped at 40 pages (`eplan-features.md` §1.1). The
  reference project has 18 pages and about 44 wires (`reference-drawing.md` §4.7).
- **Profiling crates:** `puffin` 0.20.0 (MIT OR Apache-2.0) and `tracy-client` 0.19.0.

**Proposed sizing classes (assumptions to validate with the pilot user and peers):**

| Class | Pages | Devices | Connections | 3D placements | Example |
|---|---|---|---|---|---|
| S | ≤ 50 | ≤ 300 | ≤ 1,500 | ≤ 200 | J-E-2416 (18 pages) |
| M | ≤ 300 | ≤ 2,500 | ≤ 15,000 | ≤ 800 | typical machine panel |
| L | ≤ 1,500 | ≤ 12,000 | ≤ 70,000 | ≤ 3,000 | multi-cabinet line |
| XL | ≤ 5,000 | ≤ 40,000 | ≤ 250,000 | ≤ 10,000 | plant / OEM project (stretch) |

**Proposed budgets** on the baseline machine: 4-core 2018+ x64 or Apple M1, 8 GB RAM (16 GB recommended),
integrated GPU with DX12, Vulkan or Metal, 1920×1080.

| Operation | S/M | L | XL | Notes |
|---|---|---|---|---|
| Cold start to usable window | ≤ 1.5 s | — | — | No network on start |
| Open project | ≤ 0.5 s / ≤ 2 s | ≤ 5 s | ≤ 20 s | Lazy page geometry; indices built in the background |
| Interactive edit (place/move/connect) | frame ≤ 16 ms | ≤ 16 ms | ≤ 16 ms | Derived updates incremental: ≤ 50 ms (M), ≤ 200 ms (L), then background |
| Pan/zoom dense A3 page (≈ 5k primitives) | 60 fps | 60 fps | 60 fps | ≥ 30 fps on WARP or llvmpipe for A4 |
| Full check run | ≤ 2 s | ≤ 10 s | ≤ 60 s | Parallel (rayon) |
| Regenerate all reports | ≤ 3 s | ≤ 15 s | ≤ 60 s | — |
| PDF export | 500 pages ≤ 10 s (rust-eco) | — | — | Per page ≤ 50 ms |
| Save (incremental) | ≤ 200 ms | ≤ 500 ms | ≤ 2 s | G1 protocol |
| Peak memory | ≤ 500 MB | ≤ 1.5 GB | ≤ 4 GB | — |
| Re-route a cabinet (2k wires) | ≤ 5 s | — | — | Single wire ≤ 50 ms (panel §6.7) |
| STEP import (20 MB) in worker | ≤ 30 s | — | — | Cached reopen ≤ 1 s |

**Enforce the budgets in CI:**
- synthetic projects generated by replicating the golden J-E-2416 model to the S, M, L and XL classes;
- `criterion` benchmarks with stored baselines;
- a nightly perf job that fails on more than 15 % regression.

**Supported platforms for v1:**
- Windows 10 22H2+ / 11 x64 (ARM64 best effort);
- macOS on Apple silicon (Intel best effort, since it is Rust Tier 2);
- Linux x64 on Ubuntu 22.04+ / Debian 12+, with Wayland and X11 via winit.

### G7. Multilingual project data

**Gap.** eplan-features §11 describes EPLAN's translation feature, and rust-eco says "keep project
multilinguality separate from UI i18n". Neither specifies the data model, exchange formats, identifier hygiene or
locale-aware output.

**Facts.**
- **EPLAN `MultiLangString` semantics:** a value is either *language-neutral* or a set of language strings.
  Adding a language string removes the neutral string; adding a neutral string removes all languages
  ([MultiLangString](https://github.com/binfen1/Eplan_2026_IA_MCP_scripts/blob/HEAD/Eplan_2026_RAG_mcp/Eplan_DOCS/Api/Asm/MultiLangString.md)).
  Languages are managed via `ISOCode`
  ([ISOCode](https://github.com/binfen1/Eplan_2026_IA_MCP_scripts/blob/HEAD/Eplan_2026_RAG_mcp/Eplan_DOCS/Api/Asm/ISOCode.md)).
  The serialised form in EDZ is `de_DE@text;en_US@text;` (`interop-formats.md` §2.2).
- **AML comments** carry `aml-lang` sub-attributes (TIA V21; `plc-bus-fluid.md` §2.6).
- **Rust crates:**
  - `unicode-normalization` 0.1.26 ((MIT OR Apache-2.0) AND Unicode-3.0);
  - `unicode-security` 0.1.2 (UTS #39 confusables and mixed-script detection);
  - ICU4X `icu_collator` and `icu_locale` 2.3.x (Unicode-3.0);
  - `fluent-bundle` 0.16.0 (for UI strings).

**Recommended model and rules.**

| Topic | Rule |
|---|---|
| Type | `MlText = Neutral(String) | Langs(BTreeMap<LangTag, String>)`, with EPLAN's mutual-exclusion semantics, so EPLAN import and export stay lossless |
| Tags | BCP 47 (`nl-NL`, `de-DE`, `en-GB`). The EPLAN importer maps `de_DE` → `de-DE` |
| Project languages | A source language, translation languages, and **display languages** per output. Function texts can print stacked as "NL / EN" in one field. A "PDF per language" export runs the same pages with a different display language |
| What is translatable | Function texts, page descriptions, comments, report headings, colour names, door labels. **Never** DTs, part numbers, wire numbers, PLC addresses or process tags: these are identifiers |
| Dictionary | Company and project dictionaries with entry-, sentence- and word-based matching (EPLAN modes, §11). **Exchange via XLIFF 2.1** (OASIS standard, 2018) for translation agencies and CAT tools. **TBX** (ISO 30042) for terminology. CSV for simple cases. A report lists untranslated texts per language. (Both standard citations are from knowledge, **unverified** in this session.) |
| Machine translation | Plugin only, with the user's own API key (DeepL, Azure and so on) or a local engine. Show translated-by-MT status per string. No network call without user action (G12) |
| Identifier hygiene | NFC-normalise every identifier on input. DT letter codes are restricted to A–Z (IEC 81346 letter codes). Flag **confusables and mixed scripts** in tags (Cyrillic "А" vs Latin "A") with `unicode-security`. Define DT comparison as case-sensitive or not **once**, project-wide, and check it in RD-04 |
| Sorting | Natural sort for tags and terminal designations (X2 < X10) via ICU4X collation with numeric ordering. The reference BOM is sorted as ASCII ("+" < "-" < digits), which is a usability defect |
| Formatting | Number, date and unit formatting per *report language*: decimal comma for nl and de; the date format is a project setting (REF-078); SI spacing "0.75 mm²". A space before the superscript, as in the reference, is a typo class (D-18) |
| Layout | Text expansion: German runs typically +20–35 % longer than English (**unverified rule of thumb**). Title-block cells and report columns need overflow checks (shrink-to-fit with a minimum height, or a warning). Use Noto fonts for CJK. parley handles bidi |

### G8. Accessibility

**Gap.** rust-eco covers AccessKit progress in egui. No note says what accessibility ElektraWatt must deliver, or
how wire colours work for colour-blind users.

**Facts.**
- AccessKit has released adapters for **Windows (UI Automation)**, **macOS (NSAccessibility)**,
  **Unix (AT-SPI via zbus)**, Android and iOS. The adapters are "at rough feature parity" and do not yet support
  rich text or hypertext ([README](https://github.com/AccessKit/accesskit/blob/main/README.md)).
- egui_kittest 0.37 fails tests when input widgets lack an accessible name
  ([CHANGELOG](https://github.com/emilk/egui/blob/main/CHANGELOG.md)).
- krilla can emit **tagged PDF / PDF/UA-1** ([README](https://github.com/LaurenzV/krilla/blob/main/README.md)).
- **Legal scope (unverified; EUR-Lex unreachable):** the European Accessibility Act (Directive (EU) 2019/882,
  applicable since 2025-06-28) covers listed *consumer* products and services. It does not cover B2B engineering
  software. Public-sector buyers may still require EN 301 549 conformance in tenders.
- **Prevalence (unverified):** colour-vision deficiency affects about 8 % of men of Northern-European descent and
  about 0.5 % of women. Wire-colour data (DBU vs BU, GNYE, BK vs BN) is exactly where this hurts.

**Recommendations (v1).**
1. **Never encode meaning in colour alone:**
   - wire colours are always shown with their IEC 60757 letter code;
   - net and potential highlights use dash or hatch patterns plus colour;
   - check severities use icons plus colour;
   - GNYE renders as a striped stroke.
2. **CVD-safe UI palette** for highlights (e.g. the Okabe–Ito palette), plus a CI test that renders key views
   through protanopia, deuteranopia and tritanopia simulation (vello_cpu PNG + colour matrix) and asserts minimum
   contrast.
3. **Full keyboard operation:**
   - every command is in the keymap and the palette;
   - canvas cursor movement by grid step;
   - "next connection point" and "next unconnected point" navigation;
   - the property grid and navigators are fully keyboard-driven.
4. **Screen readers:**
   - accessible names everywhere (enforced by kittest);
   - the canvas exposes the *selected object's* description (DT, function, connections) as a live region;
   - navigators and table editors are the accessible representation of the schematic.
5. **Display:** UI scale 100–300 %, per-monitor DPI, system dark/light (egui `ThemePreference`), a high-contrast
   theme, configurable minimum font size.
6. **Outputs:** tagged PDF (PDF/UA-1 option via krilla), and reports also exported as XLSX and CSV, which are
   accessible tables.

### G9. Printing (small additions to rust-eco §10.2)

rust-eco's "PDF is the print master" plan is sound and the only well-covered gap topic. Add:
- **scale-to-fit with a stamped scale** (A3 originals on A4 printers, "not to scale" note);
- **page-range and section printing** (folio ranges such as 100–199, using the section model of REF-010);
- **monochrome mode** (colour lines to black, while keeping wire-colour text codes, G8);
- **print-preview from the same PDF**;
- **batch printing from the CLI** (`ew print --pages 101-106 --printer …`).

Large-format plotters receive the PDF through the OS driver. No HPGL is needed (unverified that no the pilot user
customer needs it).

### G10. Security: project files, plugins, scripts, MCP and supply chain

**Gap.** interop and oss cover fuzzing and a SECURITY.md. rust-eco names wasmtime and rhai sandboxes. No note has
a threat model, plugin capability rules, a policy for project-embedded scripts, AI-agent (MCP) guardrails, or
supply-chain controls.

**Facts.**
- **wasmtime resource controls** exist on `Config`: `consume_fuel`, `epoch_interruption`, `max_wasm_stack`,
  `memory_reservation`
  ([config.rs](https://github.com/bytecodealliance/wasmtime/blob/main/crates/wasmtime/src/config.rs)). Per-store
  limits (`StoreLimitsBuilder::memory_size`, `table_elements`, `instances`, `memories`) live in
  [limits.rs](https://github.com/bytecodealliance/wasmtime/blob/main/crates/wasmtime/src/runtime/limits.rs).
  Defence-in-depth guard regions are documented in
  [security.md](https://github.com/bytecodealliance/wasmtime/blob/main/docs/security.md).
- **Supply-chain and attestation tools:**
  - [actions/attest-build-provenance](https://github.com/actions/attest-build-provenance) produces **SLSA build
    provenance** signed through Sigstore. It is free for public repositories; private repositories need GitHub
    Enterprise Cloud;
  - [cargo-cyclonedx](https://github.com/CycloneDX/cyclonedx-rust-cargo) 0.5.9 (Apache-2.0) produces CycloneDX
    SBOMs;
  - `cargo-vet` 0.10.2 and `cargo-audit` 0.22.2 audit dependencies (crates.io).
- **egui's inspection server** can expose UI control over the network (§3.3).
- **QET** hit CVE-2026-15037 in project serialisation (release notes), and offers MCP with an ask-first "live
  mode".

**Threat model and controls.**

| Threat | Control |
|---|---|
| Malicious project, library or import file (XML bombs, zip bombs, path traversal, NaN coordinates, huge allocations) | Size, entry and depth limits on every parser (QETZ-style ≤ 100k entries / ≤ 2 GiB). Reject `..`, absolute and drive paths in archives. `cargo-fuzz` per importer. Parsers never execute content (interop §10) |
| Scripts embedded in a *received* project (the Office-macro lesson) | **Never auto-run.** Per-project "trust" prompt, remembered by project hash and signer. rhai with `set_max_operations`, call-depth and string-size limits and no I/O functions registered |
| Plugins (WASM components) | A manifest declares capabilities (read model, submit operations, read or write *selected* files, network: none by default). The host grants WASI pre-opened directories only. Fuel or epoch deadlines and `StoreLimits` per call. Plugins are signed. The user approves capabilities on install, and the UI shows which plugin made a change in the undo history (G3) |
| AI agents via MCP | Off by default. stdio or localhost only. **Ask-first** for writes (QET live-mode precedent). Every MCP write is a normal transaction, so it is undoable and journalled. Read scopes exclude unrelated files |
| Debug and inspection endpoints | The `egui/inspection` feature is compiled only into test builds. CI asserts that release binaries do not contain the `EGUI_INSPECTION` code path |
| Dependency compromise | `Cargo.lock` committed. `cargo-deny` (licences, advisories, bans, source allowlist). `cargo-audit` in CI. `cargo-vet` audits for new crates. No build scripts that download binaries (see C-5, cadrum). Pinned toolchain |
| Release integrity | Authenticode via **Artifact Signing** (organisation identity, §3.3); Apple notarisation (`rcodesign`); **SLSA provenance attestations**; SHA-256 sums; updater verifies signatures before applying (velopack's mechanism **unverified**) |
| Vulnerability handling | `SECURITY.md` with private reporting (GitHub security advisories), a coordinated-disclosure window, a CVE request process, and supported-versions table. Doubles as CRA evidence (G11) |
| SBOM | CycloneDX SBOM per release artefact, published with the release and embedded via `cargo-auditable` (rust-eco) |

### G11. EU regulation: CRA, product liability, accessibility, machinery documentation, reverse engineering

**Gap.** None of the notes mentions the CRA or the revised Product Liability Directive, and both bear directly on
an open-source project run by a Dutch private company.

**Cyber Resilience Act (Regulation (EU) 2024/2847), verified via the
[ORC WG CRA FAQ](https://github.com/orcwg/cra-hub/tree/main/faq):**
- **Timeline:**
  - in force since 2024-12-11;
  - conformity-assessment bodies from 2026-06-11;
  - **reporting of actively exploited vulnerabilities and severe incidents (Art. 14) from 2026-09-11**;
  - all other obligations from **2027-12-11**
  ([timeline](https://github.com/orcwg/cra-hub/blob/main/faq/cra-itself/timeline.md)).
- **Scope:** software products are in scope ([scope](https://github.com/orcwg/cra-hub/blob/main/faq/cra-itself/scope.md)).
  Free and open-source software that is **not monetised** is not a commercial activity (Recitals 15 and 18;
  [no-monetisation](https://github.com/orcwg/cra-hub/blob/main/faq/maintainers/no-monetization.md)).
- **Monetisation indicators** (Recital 15): monetising related services; requiring personal-data processing as a
  condition of use for reasons other than security, compatibility or interoperability; charging for support
  beyond actual costs; donations beyond actual costs. Recovering *actual costs*, including reasonable salaries, is
  not monetisation
  ([monetisation](https://github.com/orcwg/cra-hub/blob/main/faq/cra-itself/monetisation.md);
  [actual costs](https://github.com/orcwg/cra-hub/blob/main/faq/cra-itself/actual-costs.md)).
- **Manufacturer** (Art. 3(13)): anyone who markets a product with digital elements under its name or trademark,
  "whether for payment, monetisation or free of charge"
  ([definition](https://github.com/orcwg/cra-hub/blob/main/faq/manufacturers/what-is-a-manufacturer.md)).
  **Merely funding** an open-source project does not make the funder a manufacturer
  ([funding](https://github.com/orcwg/cra-hub/blob/main/faq/manufacturers/funding-foss-dev.md)).
- **Open-source software steward** (Art. 3(14)): a *legal person* that systematically supports FOSS intended for
  commercial activities. It has light-touch duties (Art. 24): a cybersecurity policy, vulnerability handling,
  cooperation with market-surveillance authorities, and notification of actively exploited vulnerabilities
  ([definition](https://github.com/orcwg/cra-hub/blob/main/faq/stewards/what-is-a-steward.md);
  [obligations](https://github.com/orcwg/cra-hub/blob/main/faq/stewards/obligations.md)).
  One organisation can be a manufacturer for its commercial edition **and** a steward for the community edition
  ([dual role](https://github.com/orcwg/cra-hub/blob/main/faq/manufacturers/both-manufacturer-and-steward.md)).
- **Manufacturer duties** (from knowledge of Art. 13 and Annex I; text not fetched, **verify**):
  - security by design;
  - vulnerability handling with an **SBOM** covering at least top-level dependencies;
  - a support period normally ≥ 5 years;
  - security updates;
  - technical documentation;
  - EU declaration of conformity and CE marking (self-assessment for "default" category products, which an ECAD
    tool most likely is).

**Consequences for the commercial model** (oss §7.6 "services company"):
- A Stichting or the pilot user acting as **steward** of the community edition is feasible with the G10 controls.
- **Selling support for profit or a paid "LTS build" under the ElektraWatt name makes the seller a manufacturer of
  that build** from 2027-12-11.
- Telemetry that makes personal-data processing a condition of use would also count as monetisation (G12).

**Product Liability Directive (EU) 2024/2853 (unverified; from knowledge):**
- software is explicitly a "product";
- it applies to products placed on the market after **2026-12-09**;
- FOSS developed or supplied outside a commercial activity is excluded;
- property used exclusively for professional purposes is excluded from compensable damage, but personal injury
  is not.

ElektraWatt's checks (PE sizing, I_b ≤ I_n ≤ I_z, N-switching) are safety-relevant engineering aids. Licence
texts, documentation and UI must state that they do not replace engineering verification, and commercial
editions need liability review.

**European Accessibility Act:** see G8. Most likely not applicable (B2B).

**Machinery Regulation (EU) 2023/1230 (from 2027-01-20, cited in standards.md):** instructions may be supplied
digitally, and technical documentation must be retained for years (**unverified article numbers**). This
supports the G2 archive profile and the PDF/A + QR (ePOCKET-like) outputs in panel §7.5.

**Reverse engineering for interoperability (EPLAN `.epj`/`.ema`; interop §2.1).** The EU Software Directive
2009/24/EC:
- permits observing, studying and testing a program to determine its underlying ideas (Art. 5(3));
- permits decompilation where indispensable for interoperability (Art. 6);
- voids contract clauses that override these rights (Art. 8).

(Well-established law, but EUR-Lex was not reachable; **verify**.) This supports interop's "black-box, outputs
only" plan and the provenance log. Database rights (96/9/EC) still bar shipping Data Portal content.

### G12. Telemetry, crash reporting and privacy

**Gap.** rust-eco lists `sentry` "opt-in only (GDPR)". There is no policy.

**Facts.**
- **KiCad** compiles Sentry support optionally (`KICAD_USE_SENTRY`), keeps an explicit opt-in flag
  (`IsOptedIn` / `SetSentryOptIn`) and a resettable anonymous ID, and shows a first-run data-collection prompt
  (`data_collection_prompt`)
  ([app_monitor.h](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/include/app_monitor.h),
  [common_settings.h](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/include/settings/common_settings.h)).
- **Audacity's 2021 telemetry PR** (Google Analytics + Yandex Metrica, client UUID, IP recorded) drew about 3,300
  thumbs-down votes against about 250 up. It was closed and the thread locked within a week, even after the
  maintainers promised "strictly optional and disabled by default"
  ([PR #835](https://github.com/audacity/audacity/pull/835)).
- **Legal frame (unverified; sources not reachable):**
  - GDPR applies to install IDs and IP addresses;
  - ePrivacy Art. 5(3) requires consent for reading or storing information on terminal equipment unless strictly
    necessary, and EDPB Guidelines 2/2023 (final 2024) read it to cover app telemetry;
  - the CRA's monetisation indicators include mandatory personal-data processing (G11).
- **Engineering projects are confidential.** The title block reads "This document and the copyright therein are
  confidential …". Projects also contain personal data: the engineer's name and the customer's address on
  folio 1.

**Policy proposal ("offline-first, zero-by-default"):**
1. **No network traffic without an explicit user action.** No update ping, no telemetry, no font or symbol
   downloads on start.
2. **Update checks:** opt-in at first run, off by default in enterprise MSI installs (policy key). The check sends
   only the version and platform.
3. **Crash reports:** written locally (minidump + log). The user can **inspect, copy, or opt-in to upload** to a
   project-run endpoint (self-hosted GlitchTip or Sentry; the Sentry server licence is **unverified**). Uploads
   never include project content, paths, titles or user names (scrubbed in the SDK `before_send`).
4. **Usage statistics:** none in v1. If ever added: opt-in, aggregated, a documented schema, a public dashboard,
   and a toggle in the About dialog.
5. **Exports:** an option to strip personal metadata (PDF XMP author, title-block names) when exporting for
   third parties. Show a confidentiality reminder before embedding the native project in a PDF (C-10).
6. Document all of this in `PRIVACY.md`, and test it: a CI job runs the app under a network sandbox and fails on
   any unexpected connection.

### G13. Onboarding, documentation and training

**Gap.** oss mentions an mdBook and migration guides in one bullet. Adoption by EPLAN and QET users depends on
far more.

**Facts.**
- **mdBook** is at the 0.5.x line ([CHANGELOG](https://github.com/rust-lang/mdBook/blob/master/CHANGELOG.md)).
- **`mdbook-i18n-helpers`** (Google) adds gettext (PO) translation for mdBook. It is used by *Comprehensive Rust*
  and *Rust By Example*, and it produces translation-status reports
  ([README](https://github.com/google/mdbook-i18n-helpers/blob/main/README.md)). PO files are Weblate-native.
- The Diátaxis framework (tutorials / how-to / reference / explanation) is the de facto structure for technical
  docs (diataxis.fr, **unverified URL access**).

**Recommendations.**

| Item | Content |
|---|---|
| **Company-standard wizard** (first run) | Company name and logo (SVG), title-block template, confidentiality text, colour and size profile (e.g. "the pilot user DBU/WH", REF-006/007), identifier set and numbering scheme (page-based `-101F1`), structure identifiers, revision format, project languages, ferrule colour scheme (DIN/French). Output: a **basic project** (EPLAN `.zw9` equivalent) that every new project inherits. This attacks defects 3, 5, 6 and 8 at their source |
| Sample projects | The golden **J-E-2416** rebuilt defect-free (CC0, oss §7.3), plus "minimal relay panel" and "PLC + valve terminal" tutorials. Each sample is also a CI fixture |
| Tutorials (Diátaxis) | (1) First schematic page with potentials. (2) Terminal strip and cable. (3) PLC card and addressing. (4) Reports and PDF. (5) 2D/3D layout and wire list. (6) Import a QET project and fix the migration report |
| Concept mapping | EPLAN ↔ EW glossary (function, device, part, CDP, interruption point, basic project, form/plot frame, message management) and QET ↔ EW (element, master/slave, folio report, conductor) |
| In-app help | F1 context links to the docs anchor of the current tool or dialog. Tooltips show the shortcut and a "learn more" link. The check messages link to rule pages (each rule ID has a page: why, standard reference, fixes) |
| Docs as code | mdBook in `docs/`, built and link-checked in CI, versioned per release, translated with mdbook-i18n-helpers + Weblate (EN, NL, DE first, as oss §8.5 proposes). rustdoc for the core crate. WIT docs for plugins |
| Videos | Short screencasts per tutorial. Recordings from egui_kittest UI tests (MP4 recording in 0.37) keep them regenerable |

### G14. Fonts and lettering

**Gap.** reference §5.3 notes that the set uses Tahoma, which may not be redistributed. rust-eco says "bundled
fonts". No note picks fonts, considers ISO 3098, or plans substitution for imports.

**Facts.**
- **osifont** is an ISO 3098-style CAD font under **GPL-3 with font exception, GPL-2 with font exception, or
  LGPL-3 with font exception**. The font exception keeps documents that embed it free of copyleft
  ([osifont README](https://github.com/hikikomori82/osifont/blob/master/README.md)).
- **Wine** ships a Tahoma substitute (`FontName: WineTahoma`), derived from Bitstream Vera Sans
  ([tahoma.sfd](https://github.com/wine-mirror/wine/blob/master/fonts/tahoma.sfd)). Metric compatibility with
  Microsoft Tahoma is **unverified**.
- DejaVu Sans is Vera-derived (Bitstream Vera licence). Noto Sans is OFL-1.1 (general knowledge).
- Reference text sizes go down to 3.70 pt (≈ 0.95 mm cap height; D-31).

**Recommendations.**
1. Bundle **one OFL sans family (Noto Sans or DejaVu Sans) as the default** drawing font, and **osifont as an
   optional ISO 3098 style**. Embed subsets in PDFs via krilla.
2. **Never bundle Tahoma.** On QET import, map Tahoma to the bundled font, warn that line breaks may change, and
   record the substitution in the PDF metadata (reference Impl. 7).
3. Define text and line styles from ISO 3098 / ISO 128 series (text heights 1.8, 2.5, 3.5, 5, 7 mm; line widths
   0.13–2.0 mm; series values **unverified**).
4. Implement the minimum-text-height check REF-088 against the *printed* scale.

### G15. Smaller gaps worth a line in the design docs

| Gap | Why | Proposal |
|---|---|---|
| **Power budget per source/potential** | plc PLC-14 checks DO load only. The reference has a 20 A PSU, a 6 A MICO channel feeding all four CPX supplies (D-25) and a "Consumption 6A" project field | Load summation per potential and protective device (part data `i_nominal`, simultaneity factor); check against PSU and channel ratings; report on the cover page |
| **Voltage drop on long 24 V field cables** | J-E-206 links between cabinets | Cable length × section × current, with a threshold per profile (limit values **unverified**; IEC 60364-5-52 informative) |
| **Review loop with customers** | eplan-features notes eVIEW redlining and PDF comment import (unverified) | Import PDF annotations (`lopdf` 0.45 reads annotations) as EW comments anchored by page and position; comment navigator with status |
| **Visual revision diff** | Customers ask "what changed in V2.0" (D-05) | Render both revisions with vello_cpu, overlay the difference, and generate revision clouds from the transaction log (G3) |
| **Personal data in projects** | Engineer and customer addresses on folio 1 | Mark person fields; strip on export (G12) |
| **Cross-project links** | External connections to J-E-206 (REF-018) | A typed `ExternalRef {project_id, location, device, connection}`; resolve if the other project is open; otherwise show as stored text with a "stale" check |

---

## Implications for ElektraWatt

1. **Use the corrected defect list.** Replace the brief's defect 7 with "PDF lacks outline, link annotations and
   folio page labels", and defect 6 with "folio 101 only". Adopt D-01…D-42 from `reference-drawing.md` as the
   acceptance list.
2. **Fix the storage contract before writing code (G1, G2, G4):**
   - a UUID-named, sharded JSON directory;
   - a manifest-last commit with SHA-256s;
   - a recovery journal outside the project;
   - `.~ew.lock` lock files (KiCad style);
   - a git merge driver and textconv;
   - i64 nm coordinates with rational grids;
   - a two-part format version with preserve-unknown-fields and a never-pruned upgrade chain;
   - published JSON Schemas;
   - an "as-built" archive profile.
3. **Specify undo as a property of the command layer (G3):**
   - one transaction per gesture, holding field diffs by UUID;
   - derived data excluded from undo;
   - a project-wide stack with page and viewport restore;
   - coalescing;
   - a memory bound;
   - preview-and-commit for bulk operations;
   - one record format shared by undo, journal, macros, diff and revision log.
4. **Decide the connection model explicitly (G5):** explicit logical connections with junction sequence objects,
   and autoconnect as an assistant. Document the decision in an ADR, because it affects QET import, wire
   numbering and routing.
5. **Write a UX blueprint** with workspaces, command palette, keymap presets, navigator ↔ canvas sync, a property
   grid that shows inherited values and their origin, find & replace, cross-reference back/forward navigation and
   quick-fixes. Validate it with two EPLAN users and one QET user before M1.
6. **Adopt the performance classes and budgets (G6)** as non-functional requirements, with synthetic S–XL
   fixtures from the golden project and a CI performance gate. Declare the v1 platform matrix:
   - Windows 10 22H2+/11 x64;
   - macOS on Apple silicon (Intel best effort);
   - Ubuntu 22.04+;
   - DX12 WARP as the RDP/VDI fallback, tested in CI.
7. **Implement `MlText` with EPLAN semantics from day one (G7).** Use BCP 47 tags, XLIFF 2.1 export and import,
   NFC and confusable checks on identifiers, natural-sort collation, per-language number formats and text
   overflow checks.
8. **Make accessibility a v1 quality bar (G8):**
   - no colour-only meaning (wire colour codes always as text, pattern highlights);
   - a CVD-safe palette with simulation tests;
   - full keyboard operation;
   - AccessKit names enforced by kittest;
   - tagged PDF as an option.
9. **Write the threat model into the architecture (G10):**
   - parsers with limits and fuzzing;
   - no auto-run of project scripts;
   - capability-based, signed WASM plugins with fuel and memory limits;
   - MCP off by default and ask-first;
   - egui inspection excluded from releases;
   - cargo-deny, audit and vet;
   - no build-time binary downloads;
   - SLSA attestations, CycloneDX SBOMs, Artifact Signing under an organisation identity.
10. **Plan for the CRA now (G11).** Choose the legal holder (the pilot user or a stichting) and decide whether any paid
    offering will be sold under the ElektraWatt name. If so, budget manufacturer obligations from 2027-12-11
    (SBOM, vulnerability handling, ≥ 5-year support period, CE self-assessment). Either way, run steward-grade
    security processes. Revisit oss §7.6 with this in mind.
11. **Publish an offline-first privacy policy (G12):** zero network by default, opt-in update checks, local crash
    dumps with optional scrubbed upload, no usage analytics in v1, a metadata-stripping export, and a CI
    network-sandbox test.
12. **Treat onboarding as a feature (G13).** The company-standard wizard produces a basic project. Ship
    J-E-2416 as tutorial and fixture, an EPLAN/QET concept glossary, F1 help and rule pages per check ID, and
    mdBook + mdbook-i18n-helpers + Weblate for docs in EN/NL/DE.
13. **Fonts (G14):** bundle Noto or DejaVu Sans plus osifont; never Tahoma; substitution on import; ISO 3098
    text-height styles; a minimum-height check.
14. **Resolve the licence inconsistencies in one ADR (C-7, C-8):**
    - the SPDX expressions for code, format crates, symbols, part data and docs;
    - whether QET code is ported (which forbids a plugin exception on those files) or clean-roomed;
    - Contributor Covenant 3.0 (C-13).
15. **Apply the crate-adoption rule consistently (C-5, C-9).** Low-adoption crates (cadrum, gsd-parser, plcopen,
    l5x, ioddengine, taktora-ethercat-esi) are test-only, or vendored with an owner. Prefer `occt-sys`/system OCCT
    in the STEP worker. Use acadrust and the ODA converter for DWG, not LibreDWG FFI.
16. **Correct the interop facts in the design docs:**
    - EDZ = 7-Zip (sevenz-rust2);
    - TIA CAx = CAEX 2.15 + AR APC 1.4.0 with tags and InternalLinks;
    - krilla has no OCG;
    - the QET `%f` vs `%F` semantics.

## Open questions

1. **Project sizes.** What are the page, device and wire counts of the pilot user's largest projects of the last three
   years? Do customers deliver EPLAN projects of L/XL size that must be imported? This calibrates G6.
2. **Workstations.** Do the pilot user engineers work on local PCs, over RDP/Citrix, or on VDI without a GPU? Which
   Windows versions (10 LTSC?) are in use on the shop floor (guided wiring)?
3. **Project storage.** Do projects live on a file server (SMB), in SharePoint/OneDrive, or in git today?
   Should EW support simultaneous users through locking only, or is real-time co-editing a future must?
4. **Connection model.** Do the pilot user and its EPLAN-trained hires expect EPLAN-style autoconnect? Or is
   explicit-wire drawing with autoconnect assistance acceptable (G5)?
5. **Commercial intent and CRA.** Will the pilot user sell support, LTS builds or hosted services under the ElektraWatt
   name? Who will be the legal steward (the pilot panel builder, a stichting, or an umbrella foundation)? Legal advice on CRA
   manufacturer/steward roles and on PLD exposure is needed before the first paid offering.
6. **Languages.** Which languages must project data support in v1 (NL, EN, DE; FR for Belgium)? Do customers
   require stacked bilingual drawings or separate PDFs per language? Is there a company dictionary to import?
7. **Accessibility.** Does any the pilot user customer (public sector, utilities) require EN 301 549 conformance or
   accessible PDFs in tenders?
8. **Telemetry.** Is the zero-by-default policy acceptable to the team, given that it limits product analytics?
   Which crash-report endpoint will be operated, and by whom?
9. **Symbol grid.** Should the EW symbol library use a 2.5 mm or 4 mm connection grid? Must QET-imported pages
   stay on the 2.6458 mm grid permanently, or be converted?
10. **Fonts.** Does the pilot user want to keep the Tahoma look (users would install Tahoma themselves) or switch to an
    ISO 3098 style (osifont) for new projects?
11. **Unverified facts to check once the web is reachable:**
    - CRA Art. 13/Annex I details (SBOM depth, support period) on EUR-Lex;
    - PLD 2024/2853 dates and exclusions;
    - EAA scope;
    - EDPB Guidelines 2/2023 status;
    - Machinery Regulation documentation-retention article;
    - Software Directive articles;
    - XLIFF 2.1 / TBX editions;
    - ISO 3098 / ISO 128 series values;
    - EPLAN grid sizes and keyboard bindings;
    - wgpu CI coverage of lavapipe/llvmpipe;
    - velopack signature verification;
    - Sentry server licence;
    - NLnet/NGI0 call status for 2026–2027 (oss §8.4 depends on it).
