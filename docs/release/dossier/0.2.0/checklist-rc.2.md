<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.2.0-rc.2

[The release checklist](../../checklist.md), copied and filled for the second candidate of 0.2.0. There is no CI
until 1.0 and the release workflow has an early alpha's size, so each item says what was run by hand and what was
not. An item that is not met is on record here for the owner's go; it is not hidden by a passing suite.

**Candidate:** `v0.2.0-rc.2` · **Commit:** the commit of `release/0.2` that the tag names · **Draft:** the draft
prerelease `v0.2.0-rc.2` on the repository's releases page · **Filled by:** the release manager ·
**Date:** 2026-11-12

What the candidate adds to the first one is the log of [`docs/plan/m2.5-fix.md`](../../../plan/m2.5-fix.md), its
entries "The second candidate": the fixes for the owner's 28 findings on the first candidate, and what two
play-testers' runs on those fixes found.

| Item | Check | Signed by | Result and evidence |
|---|---|---|---|
| RC-01 | The threat model reviewed for this release | — | **Not done.** No review entry for 0.2.0. The release adds no network code and no new file kind. Since the first candidate the window opens an exported file by its path with the system's viewer (the same opener the print window uses), `elektrawatt.exe` carries an icon resource made by its build script from a file of the repository, and File ▸ Import… takes a `.qet` or an `.ewz` through the readers that were there |
| RC-02 | No open fuzz crash; at most 10 quarantined tests | The release manager | **Met for what runs by hand.** The deep run (`cargo nextest run --workspace --all-features --run-ignored only`) passed on the release branch: it holds the long command fuzz, the oracle on every read, the acceptance gate and the long recordings. The hostile-corpus test passes in the whole-workspace check. Two recorded fuzz inputs were remade, since a new command moved what their first word draws; each still asserts the step of its finding. No test is quarantined: every ignored test is a test of the deep run |
| RC-03 | Advisory waivers dated; SQLite's bundled version clean | The release manager | **Partly met.** `deny.toml` and `.cargo/audit.toml` both have `ignore = []`, so there are no waivers. No dependency changed since 0.1.0 (`Cargo.lock` is the first candidate's). SQLite's own vulnerability list was not checked |
| RC-04 | `SECURITY.md` names the release and its support end | The release manager | **Met.** The supported-versions rows: "0.2.x — Yes, until the next minor release (0.3.0)", "0.1.x — No" |
| RC-05 | The manual tests (RDP, screen readers, printers, network observation, the desktop installs, the crash offer) | — | **Not run.** The manual test of this release is the owner's own hands-on test of the candidate on Windows, which is its gate. Three things of this candidate show only there: the program's icon in Explorer and the task bar, that no console window opens, and that Open on an export's toast opens the file in the system's viewer |
| RC-06 | The translation status meets NFR-I18N-01; the guide is in every installer | The release manager | **Met for the window and the tutorials.** `ci/translation-status.py` exits 0: the window's texts are 100.0 % nl-NL (5457 of 5457), the guide's introduction and tutorials 100.0 % (138 of 138; the command-line walk-through left the tutorials for the how-tos); the rest of the guide is English (184 of 761 messages of the whole guide are translated). The guide's folder in the installed tree was not checked on this candidate |
| RC-07 | The content audit of the bundled assets | The release manager | **Met by its test.** `ew-library`'s content audit passes on the shipped content (licences, sources, no vendor file), with this candidate's changed coil symbols and the starter cable's cores. The program's icon is drawn from the repository's own mark. `NOTICE` and the third-party notices ship in every package; the notices of the AppImage's runtime are unchanged |
| RC-08 | Profiles and format, CLI and API changes listed in the release notes | The release manager | **Met.** `docs/release-notes/0.2.0.md`, sections 4 and 5. `cargo xtask api-guard` against 0.1.0: "0 breaking, 0 additive", so the published schemas are unchanged and nothing is migrated. One member joins the open drawing settings that have no schema (`settings.extra.drawing.wire_numbers`), additive under ADR-0011 decision 6; section 4 says so |
| RC-09 | The engineering-verification disclaimer in About, the guide and the release report | The release manager | **Present in About's source and in the release notes' section 1.** The owner removed it from the rest of the window on 2026-11-12; a test holds that only About names it. Not screenshotted for this candidate |
| RC-10 | LG-1 on file | The owner | **Met.** The owner's statement on file for 0.1.0 stands: no employer is involved |
