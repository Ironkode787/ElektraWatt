# WP-S6.06: The pilot release's installers and the platform CI matrix (closed specification)

**Status:** specified in cycle 48 (documents only), an M2 package at the size of the owner's 0.1.0 scope (ID-1187), in
three parts: part 1 the installed tree, the release builds and the CLI archives (0.8 pw, D3 to D8, D18), part 2 the
per-user Windows setup, the AppImage and the install tests (0.9 pw, D2, D9 to D13), part 3 the cross-platform
digests, the best-effort artefacts and the upgrade train (0.5 pw, D14 to D17); 0.3 pw reserve; part 1 first, then
parts 2 and 3 in any order · **Date:** 2026-11-03 · **Work package:**
[WP-S6.06](../02-work-packages.md#wp-s606-the-pilot-releases-installers-and-the-platform-ci-matrix) · **Decision
log:** ID-1187, ID-1188, ID-1210; part 1 ruled by ID-1251, built in cycle 49's first lane I49 and folded into cycle
48's delivery (as built ID-1265, the review's part 6 ID-1266; the as-built notes in §5) · **Part of:** ElektraWatt plan, specifications

**Purpose.** ElektraWatt has a nightly `ew` archive and a report-only release check of the window's binary, and no
installer of any kind. 0.1.0, the first binary release, ships a per-user Windows setup, a Linux AppImage and the CLI
archives, each offline and complete, with Golden A and the offline user guide inside (ID-1187, NFR-PLAT-06, AC-40).
This document traces what the platform matrix already proves, closes the rest to items, decisions and acceptance, and
settles the boundaries with WP-S6.07 (sums, provenance, SBOM, publishing), WP-S6.05 (privacy and the crash log) and
WP-S6.08 (the release itself). It is written against the code at `af780b80` (§2.1). Its size is an estimate, not a
commitment (ID-1210); every build and job time is not measured (owner's rule). Where this document and a design
document differ, this document is the narrower and wins for the lanes; the design documents stay the target and are
updated at the design pass after each part (§13). A choice a lane meets that this document does not settle is
escalated, never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: the installed tree, the release builds and the CLI archives](#5-part-1-the-installed-tree-the-release-builds-and-the-cli-archives)
6. [Part 2: the setup, the AppImage and the install tests](#6-part-2-the-setup-the-appimage-and-the-install-tests)
7. [Part 3: digests, best-effort artefacts and the upgrade train](#7-part-3-digests-best-effort-artefacts-and-the-upgrade-train)
8. [Neighbours and who owns what](#8-neighbours-and-who-owns-what)
9. [Files owned and what a lane may not decide](#9-files-owned-and-what-a-lane-may-not-decide)
10. [Risks](#10-risks)
11. [Size, order and routing](#11-size-order-and-routing)
12. [The gate items and what WP-S6.08 keeps](#12-the-gate-items-and-what-wp-s608-keeps)
13. [Open questions, follow-ups and the documents to update](#13-open-questions-follow-ups-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package one workflow builds, from one commit, on Windows and Linux:
- `ElektraWatt-<v>-x64-user-setup.exe`, a per-user setup that needs no administrator rights;
- `ElektraWatt-<v>-x86_64.AppImage`;
- `ew-<v>-x86_64-pc-windows-msvc.zip` and `ew-<v>-x86_64-unknown-linux-gnu.tar.gz`, the CLI archives;
- best effort, `ew-<v>-aarch64-pc-windows-msvc.zip` and `ew-<v>-aarch64-unknown-linux-gnu.tar.gz`.

Each installer holds the window and the `ew` CLI, the starter content (`content/`: symbols, function definitions,
forms, profiles, the starter library; the fonts are compiled in), Golden A as `samples/J-E-2416.ew`, the offline user
guide, `PRIVACY.md`, `SECURITY.md`, the licence texts and the third-party notices. On a machine with no network a
test installs each one, opens the sample from a read-only tree, writes AC-40's outputs equal to the committed ones,
upgrades and uninstalls. A nightly job compares Golden A's outputs from Windows and Linux byte for byte (AC-14), and a
weekly job reports how far the UI stack lags (NFR-MAINT-04). WP-S6.07's release workflow calls the same workflow from
a tag.

**Non-goals** (each with its package or follow-up):
- No per-machine MSI, no policy keys, no `.deb`, no Flathub (WP-S6.10, from 1.0, ID-1187); the setup does not look
  for a per-machine installation, which cannot exist before 1.0.
- No signing, no `SHA256SUMS`, no SBOM, no provenance, no `cargo auditable`, no publishing (WP-S6.07; signing
  WP-S6.10). No update check (ADR-0029, 1.0).
- No macOS (ID-880). No ARM installer: the best-effort rows get CLI archives only (D16, FP-2).
- No `ew-step-worker`, no `ew-wiring-viewer` (1.0).
- No new WARP or CPU-renderer job: the per-pull-request jobs already run both paths (D15).
- No reproducible build (NFR-SEC-20, WP-S6.10). No published split debug symbols (FP-3).
- No change to the PATH, no file association, no executable icon resource (FP-4), no installer pages beyond the
  folder, progress and finish pages.
- No change to how `ew` finds its starter content (FP-1) and no change to the window's resource lookup.
- No performance number: build times and sizes are reported, never gated (owner's rule).

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Quality](../../design/18-quality-security-release.md) §5.2 (CI-N-04, CI-N-05, CI-N-12, CI-R-05, CI-R-10), §5.3, §5.4, §6.3, §7.4, §8, §16 items 5, 11 and 12, §18, open questions 1, 3, 10 and 11 | The jobs, the runners, the upgrade train, the packaging table and its 0.1.0 scope |
| [Non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-PLAT-01 to NFR-PLAT-04, NFR-PLAT-06, NFR-PLAT-07, NFR-MAINT-04, NFR-MAINT-09 | §3 |
| [Golden project](../../design/20-golden-project.md) AC-14, AC-40 | Part 3's digests, part 2's installed acceptance |
| ADR-0022 (the platform tiers; amendment 1, no macOS), ADR-0023 (amendment 7, the pilot release), ADR-0028 (bundled fonts), ADR-0029 (no update check in 0.1.0) | The rows the artefacts cover; what an installer holds |
| [Gates](../03-gates.md) G-M2-01, G-M2-04, G-M2-08, G-M2-13 | §12 |
| Decision log ID-476, ID-515, ID-516, ID-723 (the distribution hold and the audits), ID-880, ID-1082, ID-1187, ID-1188, ID-1203, ID-1210 | D8, §13 |
| Follow-ups FT-234 (cross-platform digests), FT-331 (the hold's condition), FT-386 (the bundled guide and `PRIVACY.md`) | Parts 1 and 3, §13 |
| [WP-S6.07](WP-S6.07-release-pipeline.md) | The release workflow that calls this package's workflow |

### 2.1 What exists at `af780b80` (the base contract)

The lanes rely on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| `locate_resources`: each resource from its environment variable, then beside the executable, then in `../share/elektrawatt/` one level up, then in the source tree; the sample `samples/J-E-2416.ew`, the content `content`, the guide `docs` (`target/book` in the source tree); `THIRD-PARTY-NOTICES.txt` beside the executable only; a missing one disables only its feature; no test | `bin/elektrawatt/src/main.rs:20-23`, `:373-404` | D3's layout; IN-2 |
| The guide is opened by a `file://` URL through the system's opener, no shell | `bin/elektrawatt/src/main.rs:347-367` | The offline guide |
| The sample opens read-only (`readonly-sample`), is never a recent project, and is copied by Save as | `crates/ew-viewmodel/src/shell/projects.rs:156-178` | The read-only tree (NFR-PLAT-07) |
| `elektrawatt --smoke <dir>`: opens read-only, draws page 1 at 36 dpi, exits; tested on a copy of Golden A; no workflow runs it | `bin/elektrawatt/src/main.rs:11`, `:105`; `bin/elektrawatt/tests/headless.rs:509-525` | The install tests |
| `elektrawatt --headless --script <file> --config-dir <dir>`, the UI script `ew.uiscript/1` and its transcript | `bin/elektrawatt/src/main.rs:7-9`; `crates/ew-viewmodel/src/script.rs:1-60` | D13's resource proof |
| `--no-gpu` draws the canvas on the CPU and asks for a software adapter; the `canvas_backend` setting (`auto`, `cpu`) | `bin/elektrawatt/src/main.rs:18-19`, `:106`; `crates/ew-ui/src/gpu.rs:42` | D15 |
| kittest has no wgpu render state, so every kittest flow draws on the CPU canvas, asserted by `b9_kittest_draws_on_the_cpu_canvas` | `crates/ew-ui/tests/it/switch.rs:4-5`, `:52-59` | D15 |
| The GPU canvas tests on lavapipe (Linux) and WARP (Windows) with `EW_REQUIRE_GPU_ADAPTER=1` in every test job | `.github/workflows/ci.yml:160-187`, `:266-293`, `:377-384`, `:438-445`, `:498-505` | D15 |
| `ew` finds the starter content through `--content` or `EW_CONTENT_DIR` only, not beside its executable | `bin/ew/src/cli.rs:224`, `:3248-3251` | D3, FP-1 |
| The fonts compiled in (`bundled-fonts`); `content/` holds 146 files (`fndefs`, `forms`, `libraries/ew-starter-parts.ewlib`, `profiles`, `symbols`); Golden A `golden/J-E-2416.ew` holds 181 files | `crates/ew-text/src/fonts.rs:497-506`; `git ls-files` | The tree's content |
| `cli-artefact.yml`: `cli-artefact` builds `ew` in release on ubuntu-24.04 and windows-2025, smoke `--version` and `--help`, stages the licence texts, `NOTICE`, `THIRD-PARTY-NOTICES.txt` and a README, uploads `ew-<os>-<arch>-unsigned-dev-<sha>` for 90 days; `release-check` builds `elektrawatt` in release on ubuntu-24.04 with `PKG_CONFIG=false` and the path remaps, runs `ci/release-check.sh env` and `scan`, uploads its report only (ID-476) | `.github/workflows/cli-artefact.yml:44-157`, `:159-209`; `ci/release-check.sh:5-38` | D6, D8 |
| `docs-book` builds the book with mdBook 0.5.4 (pinned by version and SHA-256) in a network namespace and uploads `user-guide` (14 days) | `.github/workflows/docs.yml:134-191` | D7 |
| The third-party notices generated by cargo-about 0.9.2 and not shipped | `.github/workflows/ci.yml:1018-1020` | D3 |
| The `headless` job: `ew export pdf` and `ew render` on Golden A in a digest-pinned `ubuntu:24.04` container with `--network none`, compared with the committed digests | `.github/workflows/ci.yml:576-644` | D11's container pattern |
| AC-14's entries compare on the platform of the run (gate platform Linux; across Windows and Linux at the MVP); the tests compare the PDF with the committed digest on every OS that runs them | `golden/J-E-2416/expected/acceptance_headless.md:49-50`; `crates/ew-api/tests/acceptance/checks.rs:3711` | D14 |
| `* text=auto eol=lf`: a committed expected file has the same bytes on both OSes | `.gitattributes:4` | D14 |
| `cargo xtask check-workflows`: actions pinned by SHA with a version comment, top-level `contents: read`, no `write-all`, no `pull_request_target`, `persist-credentials: false`, secrets only in a job named `release`, images pinned by digest | `xtask/src/workflows.rs:4-17` | Every workflow of this package |
| The toolchain 1.99.0 with the targets `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`; no release profile in the workspace; local Windows builds link with rust-lld, which an explicit `RUSTFLAGS` replaces | `rust-toolchain.toml:5-8`; `Cargo.toml`; `.cargo/config.toml` | D5, D6 |
| Renovate updates actions only | `.github/renovate.json` | D17 |
| No packaging tool, NSIS script, AppImage tool, `xtask dist`, application icon, tag or release trigger anywhere | — | Parts 1 and 2 |
| ID-476's hold: no binary that links the UI graph is distributed until FT-331 has landed and the release check is green on `main` (ID-515); FT-331 is due with WP-S4.03, an M4 package | `docs/plan/06-decision-log.md` ID-476, ID-515; FT-331's row in doc 02 | D8, §13 O1 |

### 2.2 Gaps found

| Gap at `af780b80` | Consequence |
|---|---|
| No installer, no AppImage, no versioned CLI archive | Parts 1 and 2 |
| The notices file is looked for beside the executable only, and `ew` does not look beside itself at all | D3: one flat tree, every resource beside the executables; the CLI archive's README names `EW_CONTENT_DIR` (FP-1) |
| The hosted Windows images carry the Visual C++ runtime, so a binary that needs it passes CI and fails on a clean PC | D5: the static CRT, and a check of the imports (IN-4) |
| A Linux binary built on ubuntu-24.04 needs glibc 2.39 and fails on Ubuntu 22.04 (doc 18 §16 item 5) | D6: the Linux build in an Ubuntu 22.04 container |
| No test runs the programs from a read-only installation directory (NFR-PLAT-07) | IN-10 |
| No job compares outputs across the two OSes (FT-234) | D14 |
| ID-476's hold forbids distributing a binary that links the UI graph, and its FT-331 condition falls at M4, after M2 | D8 keeps every such binary inside its job; §13 O1 |
| No application icon exists; an AppImage's desktop file names one | §13 O5 |
| No upgrade-train issue and no lag report (NFR-MAINT-04) | D17 |

## 3. Requirements traced

| Requirement | State and what this package delivers | Part | Acceptance |
|---|---|---|---|
| NFR-PLAT-01 | **PR half built:** the full suite runs on ubuntu-24.04 and windows-2025 (`test-linux`, `test-linux-default`, `test-windows-1` to `-3`). **R half here:** an artefact per Supported row (the setup, the AppImage, the two x86-64 CLI archives) and per Best-effort row (the two aarch64 CLI archives, D16) | 1, 2, 3 | IN-5, IN-8, IN-14, IN-18 |
| NFR-PLAT-02 | **Built:** the GPU canvas tests per backend on the runners that have one, Vulkan on lavapipe and DX12 on WARP, in every test job | — | Traced (§2.1) |
| NFR-PLAT-03 (MVP part) | **Built:** every kittest flow runs on the CPU canvas on both OSes per pull request, and the GPU canvas's parity tests run on WARP per pull request (D15). The RDP check is RC-05's (WP-S6.08); the fps sample is not measured (owner's rule) | — | Traced; D15 |
| NFR-PLAT-04 | **Built:** the `headless` container job (CI-PR-05) | — | Traced (§2.1) |
| NFR-PLAT-06 (0.1.0 part) | The per-user setup, the AppImage and the CLI archives, with the content, the sample and the guide; installed without network on clean machines: CI on the hosted runners and containers, the desktop versions by hand (D11, §13 O3) | 1, 2 | IN-5, IN-8 to IN-15 |
| NFR-PLAT-07 | The installed tree is never written at run time, proven with write access denied; the setup needs no administrator rights | 2 | IN-8, IN-10 |
| NFR-MAINT-04 | The quarterly upgrade issue and the weekly lag report of the UI stack | 3 | IN-19, IN-20 |
| NFR-MAINT-09 | The application builds with Rust and a C compiler only: a per-pull-request check and every Linux release build run in a container with no C++ compiler, asserted | 1 | IN-3 |

**AC-14 (Windows and Linux)** gets its cross-platform entry in part 3 (D14, FT-234). **AC-40** is proven on every
installed artefact in part 2 (D12); it has no runner entry, and its evidence is the install test (CI-R-10).

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **The size.** Parts 1 to 3 at 2.2 pw and 0.3 pw reserve, 2.5 pw against the PERT of 2.6 pw (O 1.5 · L 2.5 · P 4): the PERT stays | ID-1187 already re-estimated the package; the per-pull-request matrix exists (§3), so the work is the artefacts and their tests |
| D2 | **No cargo-packager: the formats are driven directly.** The setup is an NSIS 3 script (`makensis`, zlib licence), the AppImage is assembled from the AppDir with `mksquashfs` and AppImage's static type-2 runtime (MIT, §13 O4), the CLI archives by the staging script (D4). Pins: `makensis` from the windows-2025 image, its version asserted against `packaging/components.toml` (an image that moves it fails the job and is fixed in its own commit, doc 18 §5.3's rule for images); if the image has none (**verify** at the lane's base), the NSIS 3 release archive pinned there by version and SHA-256. The type-2 runtime pinned there by URL and SHA-256 (a dated release if the project publishes one, else its `continuous` asset, which fails closed when upstream replaces it). `squashfs-tools` from the digest-pinned Ubuntu 22.04 container's signed archive, its version written to the job's report. No `appimagetool`, no linuxdeploy | Doc 18's open question 1 keeps "the configuration thin enough that NSIS and appimagetool can be driven directly" as its fallback. cargo-packager 0.11.8 (its last release 2025-11-27), like the bundler it derives from, fetches NSIS and the AppImage tools at run time when they are missing (**verify**), and whether it pins them by digest is unverified; driving the two tools directly puts every download under the workflow's pins (doc 18 §5.4: pinned versions with checksum verification) and the layout under D3's control without a code change. The cost: an NSIS script and an AppDir script to keep (about 250 lines), and the MSI of 1.0 chooses its own tool (WP-S6.10) |
| D3 | **One installed tree, flat: every resource beside the executables.** `<root>/elektrawatt[.exe]`, `<root>/ew[.exe]`, on Windows `<root>/elektrawatt.pdb` and `<root>/ew.pdb` (D5), `<root>/content/` (the repository's `content/` as is), `<root>/samples/J-E-2416.ew/` (a copy of `golden/J-E-2416.ew`, read only, never opened for writing), `<root>/docs/` (the built book), `<root>/THIRD-PARTY-NOTICES.txt`, `<root>/NOTICE`, `<root>/LICENSES/` (the texts of the licences the tree's files carry, from `ci/licence-map.toml`: at least GPL-3.0-or-later, CC-BY-SA-4.0, CC0-1.0, OFL-1.1, MIT and CC-BY-4.0), `<root>/PRIVACY.md`, `<root>/SECURITY.md`, `<root>/README.txt` (what the tree is, its version, "unsigned" and where to verify it). The setup's root is `%LOCALAPPDATA%\Programs\ElektraWatt`; the AppImage's is `usr/lib/elektrawatt/` in the AppDir; a CLI archive's root folder `ew-<v>-<target>/` holds `ew[.exe]`, `content/`, `samples/`, `THIRD-PARTY-NOTICES.txt`, `NOTICE`, `LICENSES/`, `PRIVACY.md`, `SECURITY.md` and its README (the existing `ci/cli-artefact-README.md`, rewritten for a release, which names `EW_CONTENT_DIR=<root>/content`) | `locate_resources` finds all four resources beside the executable on both OSes with no code change, and the notices only there; one layout, one test (IN-2, IN-11). `share/elektrawatt/` stays for the `.deb` of 1.0. `PRIVACY.md` sits in the tree for FT-386; opening it locally is WP-S6.05's (§8) |
| D4 | **The staging script** `packaging/stage.py` (Python standard library, as the plan's and `ci/`'s scripts): `stage.py tree --bin <dir> --book <dir> --version <v> --out <root>` assembles D3's tree from the built binaries, the repository and the built book; `stage.py archive --tree <root> --kind zip\|tar.gz --out <file>` writes a CLI archive with its entries in byte order under one root folder, Unix modes 0755 for executables and 0644 for the rest; `--test` is its offline unit test over a fixture repository in a temporary directory. A missing input is refused with its path ("missing input: <path>"), never skipped | One script on both OSes (Python is on both hosted images); the trees of the setup, the AppImage and the archives cannot drift apart |
| D5 | **The Windows build.** `cargo build --release --locked -p elektrawatt -p ew` on windows-2025 with `RUSTFLAGS="-D warnings -C target-feature=+crt-static"` plus the three path remaps of the release check (the cargo home, the rustup home and the checkout), so link.exe links and the C runtime is static. The two `.pdb` files the linker writes beside the executables (public symbols only, since the release profile has no debug info) ship in the tree | The static CRT is doc 18 §8's rule for Windows (no redistributable); the hosted image hides its absence (IN-4). Without the `.pdb` beside the executable, the local crash log's backtrace (NFR-REL-13's 0.1.0 part, WP-S6.05) has no function names on Windows |
| D6 | **The Linux build in an Ubuntu 22.04 container** (`ubuntu:22.04` pinned by digest, a `*_IMAGE` variable): `gcc` and `libc6-dev` installed, no C++ compiler (the job asserts that `g++`, `c++` and `clang++` are absent: NFR-MAINT-09); the toolchain that the host installed (`dtolnay/rust-toolchain`, as every job) mounted in, since rustc's own binaries need only glibc 2.17; any tool that runs inside the container is built or installed inside it. `cargo fetch --locked` with network, then `cargo build --release --locked --offline -p elektrawatt -p ew` in `docker run --network none` with `PKG_CONFIG=false` and the remaps, `ci/release-check.sh env` before it and `scan` on both binaries after it, inside the container. The highest `GLIBC_` symbol version of both binaries is at most 2.35. The same container, with the same assertion, runs a per-pull-request `cargo check --locked -p elektrawatt -p ew` (the job `c-only`), so every build script's C compilation is proven without a C++ compiler on each pull request (NFR-MAINT-09's "PR") | Doc 18 §5.3 and §16 item 5: glibc 2.35 is the oldest Supported Linux, so the binaries run on Ubuntu 22.04 and Debian 12; the network-less build is CI-R-03's hermetic half (doc 18 §10: no build-time download), and running the release check on the shipped binary instead of a second build removes a build |
| D7 | **The book** is built by one script, `ci/build-book.sh <dest>` (the mdBook pin, its checksum, `cargo xtask docs-gen` and the build in a network namespace move out of `docs.yml` into it), which `docs.yml`'s `docs-book` and this package's workflow call | One mdBook pin; the installers take the same offline book the `docs-book` job checks |
| D8 | **One workflow, `package.yml`, which replaces `cli-artefact.yml`.** Triggers: a nightly schedule, `workflow_dispatch`, and `workflow_call` with the inputs `version` (default: the workspace version with `-dev.<short sha>`) and `upload` (default `false`). One job per OS builds, stages, packages and runs part 2's install tests, so no binary that links the UI graph leaves its job while ID-476's hold stands; with `upload: false` a job uploads only its CLI archive (the nightly artefact `ew-<os>-<arch>-unsigned-dev-<sha>`, 90 days, as before: G-M1-22), its digests (D14) and its reports. Only WP-S6.07's release workflow passes `upload: true`, which uploads every artefact for one day, and only after the hold lifts (§13 O1). A `book` job builds the guide once (D7) and hands it to the OS jobs as an artifact (HTML, no binary). `cli-artefact.yml` and its two jobs are removed; the release check runs in D6's Linux job | The hold is a decision of record (ID-476, ID-515), and a CI artifact of a public repository is a distribution; building and testing inside one job keeps the hold whatever its date, and the nightly proves every format before any tag (doc 18's open question 1) |
| D9 | **The per-user setup** (`packaging/windows/elektrawatt.nsi`): `RequestExecutionLevel user`; Modern UI pages: folder, progress, finish (with "Start ElektraWatt"); English and Dutch by the system locale (NSIS's own translations; the one product text, the shortcut's name, is "ElektraWatt"); default folder `$LOCALAPPDATA\Programs\ElektraWatt`; the tree of D3; a Start-menu shortcut for the current user; the uninstall entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\ElektraWatt` (`DisplayName`, `DisplayVersion` = `<v>`, `Publisher` "The ElektraWatt contributors", `InstallLocation`, `UninstallString`, `QuietUninstallString`, `NoModify`, `NoRepair`, `EstimatedSize`); `/S` silent, `/D=` the folder. **Upgrade:** a setup that finds the entry runs the old uninstaller silently and waits for it, then installs, so no file of an older version remains. **Uninstall** removes the folder, the shortcut and the entry, and nothing else: never the user's configuration, data or cache directories | Doc 18 §8's per-user row and §7.4's CI-R-10 (an upgrade keeps settings and keymaps; uninstall leaves them); the uninstall-first upgrade is the simplest rule that leaves no stale file |
| D10 | **The AppImage** (`packaging/linux/`): the AppDir holds `AppRun` (a POSIX shell script: `"$APPDIR/usr/lib/elektrawatt/ew" "$@"` when the first argument is `ew`, with that argument dropped, else `"$APPDIR/usr/lib/elektrawatt/elektrawatt" "$@"`), `elektrawatt.desktop` (`Type=Application`, `Name=ElektraWatt`, `Exec=elektrawatt %F`, `Icon=elektrawatt`, `Categories=Engineering;`, `Terminal=false`), the icon `elektrawatt.png` (§13 O5) and `usr/lib/elektrawatt/` with D3's tree; `mksquashfs AppDir <img> -root-owned -noappend` with a compression the pinned runtime reads (the lane names it), the runtime and the image concatenated and made executable; the file name `ElektraWatt-<v>-x86_64.AppImage`. The static runtime needs no libfuse2; where FUSE is missing the user runs `--appimage-extract-and-run` (the guide's install page says so) | Doc 18 §16 item 11 asks for the static type-2 runtime; the `ew` dispatch puts the CLI that every installer holds (doc 18 §8) within reach of a single-file AppImage |
| D11 | **The install tests (CI-R-10, 0.1.0's form).** In D8's jobs, on the artefacts just built. **Windows** (windows-2025, a Server image): outbound firewall rules block the setup, `elektrawatt.exe` and `ew.exe` by program for the whole test, so they run without network while the runner keeps its own; install with `/S`, then the checks of IN-8 to IN-13, then uninstall. **Linux:** the AppImage in `ubuntu:22.04` and `debian:12` containers (both pinned by digest) with `--network none`, through `--appimage-extract-and-run` (a container has no FUSE), then IN-10 to IN-12 and IN-14. **By hand** (WP-S6.08, RC-05): the setup on clean Windows 10 22H2 and Windows 11 desktops as a standard user without network, and the AppImage through FUSE on Ubuntu 22.04 and Debian 12 desktops, each with the window, the sample and the guide; the record goes into the release dossier (§13 O3) | The hosted runners are neither clean nor desktop editions, and a container has no FUSE; the CI half proves offline, read-only and complete on every run, the manual half proves the four named systems once per release |
| D12 | **AC-40's installed acceptance**, `ci/installed-acceptance.sh <ew> <sample-dir> <expected-dir> <out-dir>` (bash, on both OSes): with no `EW_CONTENT_DIR` and no `--content` ("without libraries"), the installed `ew` writes `ew export pdf` equal to `expected/pdf/sha256.txt`, `ew export report --kind <kind> --format csv` equal to `expected/csv/<kind>.csv` for every kind the lane finds there (the list printed in the report), and `ew fmt --check` lists no file. "Without system fonts" is proven in the Linux containers, which have none; on Windows the PDF's digest, whose fonts are the bundled faces, is the same proof | The CSVs and the PDF are what AC-40 names; `ew fmt --check` proves that the shipped writer reproduces the sample's files |
| D13 | **The installed resources** are proven from the installed executable: `elektrawatt --headless --script packaging/tests/installed.uiscript --config-dir <fresh>` opens the sample (`file.open_sample`: read-only, 36 pages), and three further steps whose outcome differs when the content, the guide or the notices are missing (the lane chooses the steps among the existing actions and names them); each step is run once more against a copy of the tree with that one resource removed, and its outcome must change. `locate_resources` gains a unit test over the flat layout and the `share/elektrawatt` layout (`bin/elektrawatt/src/main.rs`'s test module) | A green smoke run does not show that the four resources were found; this proves each one, in the installed layout, with no new code path |
| D14 | **Cross-platform digests (CI-N-04, 0.1.0's form; FT-234).** `ci/golden-digests.py <ew> <project> <out>` runs the release `ew` of D5 and D6 on Golden A and writes `digests.txt` (`<sha256>  <name>`, sorted): the PDF on 1 and 4 threads, the 36 SVG files serial and on 4 threads, each report CSV of D12, the wire, terminal, cable and BOM CSVs of `ew export wires`, `terminals`, `cables` and `bom`, and the line `fmt-check clean`. `--compare <a> <b> --expected <dir>` fails on any line that differs, naming the files, and on a PDF digest other than `expected/pdf/sha256.txt`. Each OS job of D8 uploads its `digests-<os>.txt`; a `digests` job compares them. It runs nightly and in the release | AC-14 at the MVP is byte identity across the two OSes; the release binaries are what ships, and the tests already compare the debug builds with the committed files per OS |
| D15 | **No new WARP or CPU-renderer job.** The kittest flows run on the CPU canvas on both OSes in every pull request (§2.1), which is the CPU-renderer job of NFR-PLAT-03 and ADR-0022 decision 6, and the GPU canvas tests run on WARP in every Windows test job; CI-N-05's "fps sample" is a performance number, not measured (owner's rule); the lavapipe 3D smoke is M4's | The per-pull-request jobs are stronger than a nightly job; a new job would repeat them |
| D16 | **Best-effort artefacts:** the two aarch64 CLI archives, built nightly and per release on the hosted `windows-11-arm` and `ubuntu-24.04-arm` images (the Linux one in an arm64 `ubuntu:22.04` container, as D6), `continue-on-error: true`, smoke `ew --version` and D12's script; no ARM installer | NFR-PLAT-01's R asks for an artefact per Best-effort row; the CLI needs no GPU stack, and doc 18's open question 10 keeps them out of the release gate |
| D17 | **The upgrade train (NFR-MAINT-04).** An issue form `.github/ISSUE_TEMPLATE/upgrade-train.yml` holds doc 18 §6.3's seven steps as check boxes with their pass conditions, the abort rule and the crates to bump together. `ci/ui-stack-lag.py` reads `Cargo.lock` and the crates.io sparse index (standard library only) and reports, for egui, eframe, egui-wgpu, egui_tiles, egui_table, egui_ltreeview, egui_kittest, wgpu and winit, the locked and the newest non-yanked release; it fails when egui is more than two minors behind or wgpu more than one major behind; `--test` is its offline unit test. A weekly workflow `weekly.yml` (CI-W-03's lag half) runs it and writes the table to the job summary | The requirement's two pieces of evidence ("A: a quarterly upgrade issue; N: a dependency-age report"), with the lag limits as code; Renovate stays on actions only |
| D18 | **`packaging/components.toml`**, a hand-kept list of every non-Rust component the packaging downloads or takes from an image (the type-2 runtime, NSIS, `squashfs-tools`'s version as recorded): name, version, licence, URL and SHA-256; the workflow reads its pins from it and WP-S6.07's SBOM reads its components from it. Above its entries stands the marker of a hand-kept list (`cargo xtask map --check`) `# map: packaging components: a new downloaded or image-provided packaging tool or runtime goes here with its version, licence, URL and SHA-256.` | One place for the pins, which the SBOM needs too |
| D19 | **Every part closed;** each part is re-read at dispatch against the delivered earlier parts without reopening D1 to D18 | — |

No decision changes an ADR or an architecture edge. D2 settles doc 18's open questions 1 and 3 differently from
their recommendations (§13).

## 5. Part 1: the installed tree, the release builds and the CLI archives

**Scope.** D3 to D8, D18. **0.8 pw.**

**Build list.**
1. `packaging/stage.py` (new; D4) with its `--test`; `packaging/components.toml` (new; D18).
2. `.github/workflows/package.yml` (new; D8): the `book` job, the Windows job (D5) and the Linux job (D6) up to the
   staged tree and the CLI archives, their smoke steps (`ew --version`, `elektrawatt --version`, `elektrawatt --smoke`
   on the staged sample), the nightly CLI upload and the job summaries; `cli-artefact.yml` removed.
3. `ci/build-book.sh` (new; D7) and `docs.yml`'s `docs-book` job calling it.
4. A per-pull-request job `c-only` in `ci.yml` (NFR-MAINT-09): `cargo check --locked -p elektrawatt -p ew` in the
   digest-pinned `ubuntu:22.04` container with `gcc` and `libc6-dev` and no C++ compiler (asserted), the host's
   toolchain mounted, so every build script's C compilation runs there.
5. `bin/elektrawatt/src/main.rs`: the unit test of `locate_resources` (D13), no other change.
6. `ci/cli-artefact-README.md` rewritten as the CLI archive's README (D3); `ci/README.md`'s sections on the removed
   workflow rewritten for `package.yml`; `ci/licence-map.toml` and `REUSE.toml` rows for `packaging/`.

**Acceptance.**
- **IN-1** `python -I packaging/stage.py --test` passes: the tree of a fixture repository equals D3's listing; a
  missing input is refused with "missing input: <path>"; an archive lists its entries in byte order under one root
  folder with the modes of D4; two runs give identical archives.
- **IN-2** The unit test of `locate_resources` passes: in the flat layout all four resources are found beside the
  executable; in the `share/elektrawatt` layout the three directories are found and the notices are not (as built);
  an environment variable wins over both; an absent resource is `None`.
- **IN-3** (CI run) The Linux job builds inside the container with `--network none` and `--offline`; the absence of
  `g++`, `c++` and `clang++` is asserted before the build; `release-check.sh env` and `scan` pass on both binaries; the
  highest `GLIBC_` version of both is at most 2.35. The `c-only` job is green on the pull request that adds it.
- **IN-4** (CI run) `dumpbin /dependents` of `elektrawatt.exe` and `ew.exe` lists no `VCRUNTIME*`, `MSVCP*`,
  `ucrtbase` or `api-ms-win-crt-*` library.
- **IN-5** (CI run) The nightly writes both x86-64 CLI archives; extracted, `ew --version` prints `<v>` and `ew export
  pdf` on the archive's sample equals `expected/pdf/sha256.txt`; the archives are uploaded as the nightly CLI artefact.
- **IN-6** The staged tree's `elektrawatt --smoke samples/J-E-2416.ew` exits 0 on both OSes, and `elektrawatt
  --version` and `ew --version` print `<v>` (CI run).
- **IN-7** `cargo xtask check-workflows` passes with `package.yml` and without `cli-artefact.yml`; `docs-book` stays
  green through `ci/build-book.sh`; targeted checks for `elektrawatt` and `xtask`; `doccheck.py` for `ci/README.md`.

*As built (part 1, lane I49, folded into cycle 48's delivery; decision log ID-1251, ID-1265, ID-1266):*
`packaging/stage.py` (standard library; `tree`, `archive`, and the helpers `pin`, which prints a component's version
from `components.toml`, and `version`, which prints the workspace version or D8's default; `--test` asserts whole
lists) and `packaging/components.toml` (`gcc` and `libc6-dev` at exact versions, cargo-about 0.9.2 and the runner's
Python); `.github/workflows/package.yml` (the `book` job, the Windows and Linux jobs to the staged tree and the CLI
archives, their smoke steps, the nightly CLI upload under the old artefact name) with `cli-artefact.yml` removed;
`ci/build-book.sh`, which `docs.yml`'s `docs-book` calls; `ci/linux-container.sh`, which the Linux job and `c-only` run
inside the container (not in the build list; the code stands on it); the job `c-only` in `ci.yml`, which runs on every
push and pull request (making it a required check is the owner's); and `locate_resources`' test in
`bin/elektrawatt`, which reaches the environment variable through a child process. IN-1, IN-2 and IN-7 are met by
tests; IN-3 to IN-6 by the delivery's dispatch of `package.yml`, recorded by cycle 49's first design pass. The spec
gives way on four points (the review's part 6, ID-1266): D18's marker is `packaging-components`, since the map check
refuses a name with a space; §5 item 6's `REUSE.toml` row is not written, since every file of `packaging/` carries its
own SPDX header; "the image's Python" (ID-1251) reads as the runner's, since `stage.py` runs on the host after the
container's build; and stamping `<v>` into the binaries is WP-S6.07's (ID-1251): the programs print the workspace
version, and the jobs assert that `<v>` begins with it. Part 2's rows from the review are FT-1222 to FT-1230, the
stamping FT-1231.

## 6. Part 2: the setup, the AppImage and the install tests

**Scope.** D2, D9 to D13. **0.9 pw.**

**Build list.**
1. `packaging/windows/elektrawatt.nsi` (new; D9) and the Windows job's packaging and install-test steps.
2. `packaging/linux/AppRun`, `packaging/linux/elektrawatt.desktop`, the icon of §13 O5, and the Linux job's AppImage
   and container steps (D10, D11).
3. `ci/installed-acceptance.sh` (new; D12); `packaging/tests/installed.uiscript` and its runner step (D13).
4. A how-to page of the user guide, `docs/book/src/how-to/install.md`: the setup (with Windows' warning for an
   unsigned program and how to go on), the AppImage (`chmod +x`, FUSE and `--appimage-extract-and-run`, the `ew`
   dispatch), the CLI archives (`EW_CONTENT_DIR`), upgrading and uninstalling; linked from the how-to index.

**Acceptance.**
- **IN-8** (CI run, Windows) `/S` installs into `%LOCALAPPDATA%\Programs\ElektraWatt`; the setup's manifest asks for
  `asInvoker`; nothing is written under `HKLM` or `%ProgramFiles%` (a before-and-after listing of both); the installed
  tree equals D3's listing; the shortcut and the uninstall entry exist, its `DisplayVersion` equal to `<v>`.
- **IN-9** (CI run, Windows) The firewall rules of D11 are in force for the whole test, which passes with them.
- **IN-10** (CI run, both OSes; NFR-PLAT-07) With write access to the installed tree denied to the running user
  (Windows: an `icacls` deny of write and delete; Linux: `chmod -R a-w` on the extracted tree), `elektrawatt --smoke
  <root>/samples/J-E-2416.ew` exits 0, and IN-11 and IN-12 pass; afterwards the tree's listing and every file's
  SHA-256 are unchanged.
- **IN-11** (CI run, both OSes; D13) The installed-resources script passes from the installed executable, and each of
  its resource steps changes its outcome against a tree without that resource.
- **IN-12** (CI run, both OSes; AC-40) `ci/installed-acceptance.sh` passes with the installed `ew` (on Linux through
  the AppImage's `ew` dispatch) on the installed sample.
- **IN-13** (CI run, Windows) A settings file and a keymap file written into the user's configuration directory
  (where the shell reads them) are byte for byte the same after the setup is run a second time over the installation
  and after `/S` uninstall; the uninstall removes the folder, the shortcut and the entry.
- **IN-14** (CI run, Linux) In `ubuntu:22.04` and `debian:12` with `--network none`, `ElektraWatt-<v>-x86_64.AppImage
  --appimage-extract-and-run --version` and `... ew --version` print `<v>`; the extracted `usr/lib/elektrawatt/`
  equals D3's listing; the desktop file and the icon sit at the AppDir's root.
- **IN-15** (manual, WP-S6.08's RC-05) The four desktop installs of D11, each with the window, the sample and the
  guide, recorded in the release dossier.
- **IN-16** `cargo xtask check-workflows`; `docs-gen --check` and the book's link check with the new page.

## 7. Part 3: digests, best-effort artefacts and the upgrade train

**Scope.** D14 to D17 (D15 builds nothing). **0.5 pw.**

**Build list.** `ci/golden-digests.py` (new) and its steps and `digests` job in `package.yml`; the two aarch64 jobs of
D16 in `package.yml`; `ci/ui-stack-lag.py` (new), `.github/workflows/weekly.yml` (new),
`.github/ISSUE_TEMPLATE/upgrade-train.yml` (new); `ci/README.md`'s rows for each.

**Acceptance.**
- **IN-17** `python -I ci/golden-digests.py --test` passes: equal manifests pass; a manifest that differs in one SVG
  fails naming that file; a PDF digest other than the expected one fails with both digests.
- **IN-18** (CI run) The nightly's `digests` job is green with the manifests of both OSes, and its summary lists the
  number of files compared; the two aarch64 jobs run and a failure of either leaves the run green with the failure
  in the summary.
- **IN-19** `python -I ci/ui-stack-lag.py --test` passes: a fixture lock and index give the expected table; egui three
  minors behind and wgpu two majors behind fail, two minors and one major pass; a yanked newest release is skipped.
- **IN-20** (CI run) `weekly.yml` runs on dispatch and writes the table; `cargo xtask check-workflows` passes; the
  issue form lists the seven steps of doc 18 §6.3 with their pass conditions.

## 8. Neighbours and who owns what

| Neighbour | It owns | This package |
|---|---|---|
| WP-S6.07 | The release workflow from a tag, `cargo auditable`, `SHA256SUMS`, the SBOM and notices, provenance, the advisory gate, the canary assertions, publishing, the checklist | Provides `package.yml` with `version` and `upload`, the artefacts, D18's list and the install tests (CI-R-05, CI-R-10); WP-S6.07 adds its per-artefact steps to `package.yml`'s OS jobs |
| WP-S6.05 | The local crash log, the network sandbox (CI-N-06), `PRIVACY.md`, opening it locally (FT-386's window half) | Places `PRIVACY.md` and the `.pdb` files in the tree (D3, D5) |
| WP-S6.04 | `SECURITY.md`, the threat model | Places `SECURITY.md` in the tree |
| WP-S6.08 | The version, the release notes, cutting the candidate, RC-05's manual runs, the dossier | Gives IN-15's procedure |
| WP-S6.10 | The MSI, its policy keys, the `.deb`, Flathub, signing, reproducible builds | Leaves `share/elektrawatt/` for the `.deb` |
| WP-S1.22 | The CLI surface | FP-1 is its row |
| WP-S6.02 | Per-platform determinism tests | D14 compares across the platforms |
| WP-S0.06 | The user guide and `docs-book` | D7 moves the build into a script; part 2 adds a page |

## 9. Files owned and what a lane may not decide

### 9.1 Files owned

**Part 1:** `packaging/stage.py`, `packaging/components.toml`, `.github/workflows/package.yml`, the removal of
`.github/workflows/cli-artefact.yml`, `ci/build-book.sh`, `.github/workflows/docs.yml` (the `docs-book` job's steps),
`.github/workflows/ci.yml` (the `c-only` job), `bin/elektrawatt/src/main.rs` (the test module only),
`ci/cli-artefact-README.md`, `ci/README.md`, `ci/licence-map.toml` and `REUSE.toml` (rows). **Part 2:**
`packaging/windows/**`, `packaging/linux/**`, `packaging/tests/**`, `ci/installed-acceptance.sh`, `package.yml`,
`docs/book/src/how-to/install.md` and the how-to index, the icon file (§13 O5). **Part 3:** `ci/golden-digests.py`,
`ci/ui-stack-lag.py`, `.github/workflows/weekly.yml`, `.github/ISSUE_TEMPLATE/upgrade-train.yml`, `package.yml`,
`ci/README.md`.

Read-only for every lane: every crate's source except the test module above, `golden/**`, `content/**`, `deny.toml`,
`supply-chain/**`, `Cargo.toml` and `Cargo.lock`, `docs/**` except the book page and the generated pages.

### 9.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a change needs product code (a crate's or a binary's source beyond D13's test), a new Rust dependency, or a change
   to `deny.toml` or `supply-chain/`;
2. a download cannot be pinned by version and SHA-256, or a component's licence is outside the NFR-LIC-04 allowlist
   (beyond §13 O4);
3. a program writes into the installed tree, or opening the installed sample needs write access to it (IN-10);
4. a binary that links the UI graph would leave its job without `upload: true` (D8);
5. a committed expected digest or CSV differs from what the installed or release `ew` writes (IN-12, D14), or the two
   OSes differ: this is a finding, reported with the files, never re-blessed by this package;
6. the work of a neighbour (§8) is reached.

The lane decides freely: the job and step names, the NSIS script's structure, the squashfs compression the runtime
reads, the steps of the installed-resources script, test fixtures, and the wording of the README and the guide's page.

## 10. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The workflows are proven only by CI, which runs at a delivery (the lanes run no CI) | Certain · medium | Every step calls a script with an offline `--test`; the YAML stays thin; part 1 lands in a delivery cycle so part 2 starts on a proven base |
| R2 ID-476's hold is not lifted before the first candidate | Medium · high | D8 builds and tests without distributing; §13 O1 asks for the ruling now |
| R3 The static type-2 runtime's licence needs an owner's ruling (§13 O4) | Medium · medium | Part 2 builds it; the release waits on the ruling, not the lane |
| R4 A reader of `ew` or the window writes into the project or the installation (a lock, a cache) | Low · high | IN-10 denies write access and compares every file's digest |
| R5 The hosted Windows image drops NSIS or changes its version | Medium · low | D2's version assertion and the pinned release archive |
| R6 A cross-platform difference surfaces (float formatting, path separators) | Low · high | §9.2 item 5: a finding with its files, outside this package |

## 11. Size, order and routing

**Size.** 2.2 pw plus 0.3 pw reserve: part 1 **0.8** (the staging script and its test 0.2, the two builds and the
container 0.25, `package.yml` and the book script 0.2, the `c-only` job and `locate_resources`'s test 0.15), part 2
**0.9** (the NSIS script 0.25, the AppImage 0.2, the install tests and AC-40's script 0.3, the resources script 0.1,
the guide's page 0.05), part 3 **0.5** (the digests 0.2, the aarch64 jobs 0.1, the lag script and workflow 0.15, the
issue form 0.05). **De-scope rule:** when a part is projected or found more than 20 % over its size, these leave as
follow-ups without a further ruling, in this order: for part 2 IN-13's second run over an installation (the uninstall
check stays), then D13's removed-resource runs (the positive script stays); for part 3 the aarch64 jobs, then the
weekly workflow (the script and the issue form stay).

**Order and routing** (`.claude/orchestration.md` rule 0). Part 1 **Opus at `medium`** (new CI structure, the
container build, the hold), first; part 2 **Opus at `medium`** (installer scripting with upgrade and uninstall
semantics, the read-only proof), after part 1 and before WP-S6.07 part 1; part 3 **Sonnet at `medium`** (decided jobs
and scripts with offline tests), after part 1, in parallel with part 2. Part 1 should land in a delivery cycle, since
only CI proves it (R1).

## 12. The gate items and what WP-S6.08 keeps

- **G-M2-13** (CI-R-01 to CI-R-11 in 0.1.0's form): this package delivers CI-R-05 (packaging) and CI-R-10's CI half
  (D11); WP-S6.07 the rest.
- **G-M2-01** (AC-14 on Windows and Linux): D14's `digests` job is its CI-N-04 evidence.
- **G-M2-04**'s AC-40 item: IN-12 on every installed artefact (CI-R-10).
- **G-M2-08**: the WARP and CPU-canvas runs exist per pull request (D15); its ≥ 30 fps figure and RDP check are not
  this package's.
- **WP-S6.08 keeps:** the version bump to 0.1.0, the release and migration notes, cutting the candidate, IN-15's four
  manual installs with RC-05's other manual tests, the release dossier (GP-11) and the go.

## 13. Open questions, follow-ups and the documents to update

**Open questions for the orchestrator.**
- **O1** ID-476's distribution hold lifts when FT-331 has landed and the release check is green on `main` (ID-515);
  FT-331 is due with WP-S4.03, an M4 package, so as recorded no 0.1.0 artefact could be distributed at M2. The
  zbus descriptors FT-331 names reach any child the window starts while one is open: at 0.1.0 these are `lp`,
  `xdg-open` and `rundll32` (printing and the guide), not only the STEP worker. Options: (1) a ruling that FT-331's
  condition binds the STEP worker's spawn, so the hold lifts for 0.1.0 when D6's release check is green on `main`,
  with a new row to check the descriptors at the 0.1.0 spawns before the first candidate; (2) FT-331's fix (close
  inherited descriptors in every spawn of `elektrawatt`) re-homed to WP-S6.07 before the first candidate; (3) the
  hold stands and 0.1.0 waits for WP-S4.03. Recommendation: (1); the 0.1.0 children are the system's own programs run
  as the same user.
- **O2** The PERT stays at 2.6 pw (D1). Options: (1) keep it; (2) enter 2.5. Recommendation: (1), the reserve absorbs
  the difference.
- **O3** G-M2-13 and NFR-PLAT-06 name clean-VM installs on Windows 10 22H2, Windows 11, Ubuntu 22.04 and Debian 12.
  Hosted runners are Server images and containers have no FUSE. Options: (1) D11 as written: the CI half on every run,
  the four desktop installs by hand once per release, recorded in the dossier; (2) self-hosted clean VMs as runners
  (the owner's hardware; doc 18 §5.4's rules for self-hosted runners apply). Recommendation: (1).
- **O4** (the owner's: a licence matter) The static type-2 runtime that doc 18 §16 item 11 asks for links libfuse 3
  (its library under LGPL-2.1, **verify** against the runtime's own licence file) and squashfuse statically; NFR-LIC-04
  admits LGPL-2.1 only for `ew-step-worker`, and whether that list governs a packaging runtime is the owner's to say.
  Options: (1) ship the
  static runtime, a separate program aggregated with the payload, with its licences in `LICENSES/`, its notices and
  a pointer to its source in `NOTICE` and the release notes; (2) the classic runtime, which loads the system's libfuse2
  (no LGPL code shipped; Ubuntu 22.04 and later lack libfuse2 by default, so users run `--appimage-extract-and-run`);
  (3) no AppImage at 0.1.0 (against ID-1187). Recommendation: (1), before the first public release; part 2 builds it
  either way.
- **O5** ElektraWatt has no application icon, and the AppImage's desktop file names one. Options: (1) cycle 48's design
  pass draws it (`assets/icons/app/elektrawatt.svg` with a 256-pixel PNG) before part 2; (2) part 2 ships a plain
  wordmark placeholder. Recommendation: (1); the executable's icon resource needs a build-script crate and is FP-4.

**Follow-ups, proposed** (the design pass opens the rows). **FT-234** closes with part 3; **FT-386**'s bundling half
closes with part 1 (its window half stays WP-S6.05's).
- **FP-1** `ew` finds the starter content beside its executable, as the window does (WP-S1.22).
- **FP-2** ARM installers (the setup on arm64, the AppImage on aarch64), from 1.0 (WP-S6.10).
- **FP-3** Published split debug symbols (DWARF and PDB archives) with the minidump of 1.0 (WP-S6.10).
- **FP-4** The executable's icon and version resource on Windows (a build-script dependency), from 1.0.
- With O1 (1): the descriptor check at the 0.1.0 spawns, due before the first candidate.
- A de-scoped item of §11 becomes a new FT at the design pass.

**Documents to update** (at the design pass after each part). **After part 1:** doc 18 §5.2 (CI-N-12 and CI-R-03's
pre-release row: `package.yml` replaces `cli-artefact.yml`; CI-PR-02's "Linux container (C compiler only)" as built
in the `c-only` job), §5.3 (the release builds in the 22.04 container as built), §8 (the tool choice of D2, the tree
of D3, the artefact names), open questions 1 and 3 (resolved by D2); 02c NFR-MAINT-09 (its evidence as built);
`ci/README.md`. **After part 2:** doc 18 §7.4 CI-R-05 and CI-R-10 (0.1.0's form of D11), §16 items 11 and 12 (the
static runtime as built; no per-machine detection before 1.0); doc 20 §21 (AC-40's evidence is the install test);
`NOTICE` (the runtime, with O4). **After part 3:** doc 18 §5.2 CI-N-04 (as built), CI-N-05 (D15: covered per pull
request, the fps sample not measured), CI-W-03 (the lag half as built), §6.3 (the issue form), open question 10
(the best-effort CLI archives).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S6.06, WP-S6.07, WP-S6.08, WP-S6.10, WP-S6.05, WP-S6.04, WP-S6.02,
  WP-S4.03; FT-234, FT-331, FT-386.
- [Decision log](../06-decision-log.md): ID-476, ID-515, ID-516, ID-723, ID-880; [decisions](../decisions.md):
  ID-1082, ID-1187, ID-1188, ID-1203, ID-1210.
- [Quality](../../design/18-quality-security-release.md) §5, §6.3, §7.4, §8, §16, §18, open questions;
  [non-functional requirements](../../design/02c-nonfunctional-requirements.md);
  [golden project](../../design/20-golden-project.md) AC-14, AC-40; [gates](../03-gates.md) G-M2.
- [ADR-0022](../../adr/0022-platform-matrix-and-performance-budgets.md),
  [ADR-0023](../../adr/0023-supply-chain-and-release-integrity.md),
  [ADR-0028](../../adr/0028-default-drawing-fonts.md),
  [ADR-0029](../../adr/0029-network-telemetry-crash-reporting-policy.md).
- The specification of [WP-S6.07](WP-S6.07-release-pipeline.md).
