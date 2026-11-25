<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.3.5-rc.4

[The release checklist](../../checklist.md), copied and filled for the fourth candidate of 0.3.5. There is no CI
until 1.0 and the release workflow has an early alpha's size, so each item says what was run by hand and what was
not. An item that is not met is on record here for the owner's go; it is not hidden by a passing suite.

The fourth candidate is the third with the fixes of one persona round on it (twelve commits); an item that those
commits do not touch says so and points to [the third candidate's checklist](checklist-rc.3.md).

**Candidate:** `v0.3.5-rc.4` · **Commit:** the commit of `release/0.3` that the tag names · **Draft:** the draft
prerelease `v0.3.5-rc.4` on the repository's releases page · **Filled by:** the release manager ·
**Date:** 2026-11-25

| Item | Check | Signed by | Result and evidence |
|---|---|---|---|
| RC-01 | The threat model reviewed for this release | — | **Not done as a review of its own.** Nothing since the third candidate touches a row of it: no file is read or written in a new way, no `unsafe` block was added, and the release still adds no network code. What the third candidate's item says stands, with the call that has not run on Windows yet (RC-05) |
| RC-02 | No open fuzz crash; at most 10 quarantined tests | The release manager | **Met for what runs by hand.** On the tree of the code, nothing else building: the whole workspace with the deep run and the private corpus (`cargo xtask check --deep`) passed, 7,858 tests and 9 of 9 deep tests, with the persona briefs replayed and the generated guide pages checked; the candidate's commit adds documents to that tree, and the repository rules ran on it. No fuzz campaign runs before 1.0; no test is quarantined. A review of the twelve commits came before the run: one major finding, in the tooling and not in the program (the persona harness read the action list with a copy of the signature grammar that did not know the new type `page`, so every action step of a persona run would have failed), mended at its cause (one grammar in one small crate that the window and the harness both read, and a test that reads the generated list back with the harness's reader), and three small ones in the documents, mended |
| RC-03 | Advisory waivers dated; SQLite's bundled version clean | The release manager | **Met.** `deny.toml` and `.cargo/audit.toml` both have `ignore = []`, so there are no waivers. `cargo deny check advisories licenses bans` on the candidate's tree says "advisories ok, bans ok, licenses ok". No third-party dependency was added or changed since the third candidate; the lock file gained one crate of the workspace itself (`ew-signature`, permissive, depending on nothing). SQLite's own list was not read again for this candidate; the bundled version is the one 0.3.0 shipped |
| RC-04 | `SECURITY.md` names the release and its support end | The release manager | **Met.** Unchanged since the first candidate: "0.3.x — Yes, until the next minor release (0.4.0)", "0.2.x — No", "0.1.x — No" |
| RC-05 | The manual tests (RDP, screen readers, printers, network observation, the desktop installs, the crash offer) | — | **Not run for this candidate.** As for the third: the installers, a printer and a screen reader are the owner's to try on a real desktop; Open PDF, Open in PDF viewer, the privacy file, the log folder and F1 open through one call that has not run on Windows (try a file in a folder whose name has a character outside the system's code page, and that F1 lands on its heading); the exported DXF pages were not opened in a CAD program by anyone. New for that test since the third candidate: Move to page… on the Page tab and in the right-click menu of selected symbols, the interruption point's card, and a toast that leaves after its time and is found under the status bar's bell with its offer |
| RC-06 | The translation status meets NFR-I18N-01; the guide is in every installer | The release manager | **Met for the window and the tutorials.** The rule `translations` of `cargo xtask rules` passes on the candidate's tree: every text key of every crate has its Dutch, the new questions and refusals among them. `cargo xtask book-po --check` passes. The guide beyond its introduction and tutorials is English. The guide's folder in the installers was not looked at by hand |
| RC-07 | The content audit of the bundled assets | The release manager | **Met by its test.** `ew-library`'s content audit passes on the shipped content, which did not change since the third candidate |
| RC-08 | Profiles and format, CLI and API changes listed in the release notes | The release manager | **Met.** `docs/release-notes/0.3.5.md`: each of the fixes has its lines in section 3, and section 5 lists what a script or a binding sees: the window action `edit.move_to_page` takes `page` where it took `folio`, and is on the Page tab and in the right-click menu where it was a submenu of Edit; the signature type `page` is new; an action started in the window without a parameter it needs asks for it or is refused in words; `tool.ip`'s direction is the tool's own state; a navigator row's key is unique in its table. `cargo xtask api-guard` against 0.3.0: "0 breaking, 363 additive"; no stored value was added. The summary has 30 bullets, none over 160 characters; two of them were written again for this candidate (Move to page, the toasts) |
| RC-09 | The engineering-verification disclaimer in About, the guide and the report | The release manager | **Present in About's source and in the release notes' section 1.** Not screenshotted for this candidate |
| RC-10 | LG-1 on file | The owner | **Met.** The owner's statement on file for 0.1.0 stands: no employer is involved |
