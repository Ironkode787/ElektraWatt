<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.3.5-rc.2

[The release checklist](../../checklist.md), copied and filled for the second candidate of 0.3.5. There is no CI
until 1.0 and the release workflow has an early alpha's size, so each item says what was run by hand and what was
not. An item that is not met is on record here for the owner's go; it is not hidden by a passing suite.

**Candidate:** `v0.3.5-rc.2` · **Commit:** the commit of `release/0.3` that the tag names · **Draft:** the draft
prerelease `v0.3.5-rc.2` on the repository's releases page · **Filled by:** the release manager ·
**Date:** 2026-11-23

| Item | Check | Signed by | Result and evidence |
|---|---|---|---|
| RC-01 | The threat model reviewed for this release | — | **Not done.** No review entry for 0.3.5. Since the first candidate the release still adds no network code and no reader of a file from outside. Two things touch what leaves the program: an export's toast offers Show in folder, which hands the folder the program wrote to the operating system's file manager; and a script's transcript now writes a path of its working folder relative to it, so that it no longer carries the machine's folder names. Delete and Cut now remove the wires drawn to what they take, each in one undo step, and say how many |
| RC-02 | No open fuzz crash; at most 10 quarantined tests | The release manager | **Met for what runs by hand.** The deep run (`cargo nextest run --workspace --all-features --run-ignored only`) passed on the candidate's tree with the private corpus, 9 of 9. The whole workspace passed there, 7,881 tests. No fuzz campaign runs before 1.0; no test is quarantined. The first exit run of this candidate failed on three things, each mended before the cut and run again: a reference script of the persona harness that wired a card where the retired two-row form had its pins, the nightly comparison of two script runs (the transcript's path, above), and one tutorial sentence without its Dutch. Three window tests that failed on a machine busy with other builds now wait for what they wait on and no longer count frames |
| RC-03 | Advisory waivers dated; SQLite's bundled version clean | The release manager | **Met.** `deny.toml` and `.cargo/audit.toml` both have `ignore = []`, so there are no waivers. `cargo deny check advisories licenses bans` on the candidate's tree says "advisories ok, bans ok, licenses ok". No dependency changed since the first candidate. SQLite's own list was not read again for this candidate; the bundled version is the one 0.3.0 shipped |
| RC-04 | `SECURITY.md` names the release and its support end | The release manager | **Met.** Unchanged since the first candidate: "0.3.x — Yes, until the next minor release (0.4.0)", "0.2.x — No", "0.1.x — No" |
| RC-05 | The manual tests (RDP, screen readers, printers, network observation, the desktop installs, the crash offer) | — | **Not run for this candidate.** The installers, a printer and a screen reader are the owner's to try on a real desktop. The exported DXF pages were not opened in a CAD program by anyone, and no persona run could open the PDF it exported |
| RC-06 | The translation status meets NFR-I18N-01; the guide is in every installer | The release manager | **Met for the window and the tutorials.** `cargo xtask book-po --check` passes on the candidate's tree. `ci/translation-status.py` exits 0: the window's texts are 100.0 % nl-NL (7877 of 7877), the rule messages 100.0 % (582 of 582), the guide's introduction and tutorials 100.0 % (238 of 238); the rest of the guide is English (280 of 1464 messages of the whole guide are translated). The guide's folder in the installers was not looked at by hand |
| RC-07 | The content audit of the bundled assets | The release manager | **Met by its test.** `ew-library`'s content audit passes on the shipped content, with what this candidate adds: the starter part EW-GEN-TRM-3, a two-level feed-through terminal with its Dutch description; the starter parts library is 0.20.0 |
| RC-08 | Profiles and format, CLI and API changes listed in the release notes | The release manager | **Met.** `docs/release-notes/0.3.5.md`, sections 4, 5 and 6. `cargo xtask api-guard` against 0.3.0: "0 breaking, 363 additive". Since the first candidate: one rule added (EW-POT-022), one command added (`rep.move_to_new_page`), `plc.module_move`, `tool.place_terminal`'s `levels`, and behaviour changes of `edit.paste`, `edit.delete` and a script's transcript, each with its line in section 5. The summary has 30 bullets, none over 160 characters |
| RC-09 | The engineering-verification disclaimer in About, the guide and the report | The release manager | **Present in About's source and in the release notes' section 1.** Not screenshotted for this candidate |
| RC-10 | LG-1 on file | The owner | **Met.** The owner's statement on file for 0.1.0 stands: no employer is involved |
