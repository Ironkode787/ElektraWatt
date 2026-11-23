<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release dossier: ElektraWatt 0.3.5

The record of the fourth release: its candidates, each candidate's filled checklist and the go. It is kept at an
early alpha's size. The procedure is [the release procedure](../../README.md); the release notes are
[`docs/release-notes/0.3.5.md`](../../../release-notes/0.3.5.md); what the release holds and how each part was
shown is the log of [`docs/plan/m3.5.md`](../../../plan/m3.5.md). People are named by their role only.

0.3.5 is the release of M3.5: a round of hygiene in the repository, then a fix round that worked the backlog down,
the rows of a first real drawing set made with 0.3.0 first. Its gate is the owner's go.

## 1. The owner's items

The items on file for 0.1.0 stand ([its dossier](../0.1.0/README.md) §1): the repository's settings, the audits,
the statement that no employer is involved. No new item is asked for 0.3.5.

Decided by the owner during M3.5 and on record in the plan's log and the documents named: the fix round is part of
this release, and its rows are worked, not moved to a later one; the reference project is retired; the latest
version leads, so a project saved by a newer version need not open in an older one (ADR-0011 amendment 8); until
1.0 a project that exists takes the latest version's derivation rules (ADR-0003 amendment 3); the texts of a symbol
turned a quarter stay level and are placed anew round it; the legend of a panel layout is a list page of its own;
before a candidate is cut, a review of the whole change with a fix lane; the rows still open at the first cut are
worked on after it.

## 2. The candidates

| Candidate | Cut from | Checklist | State |
|---|---|---|---|
| `v0.3.5-rc.1` | `release/0.3`, the commit the tag names | [`checklist-rc.1.md`](checklist-rc.1.md) | Cut on 2026-11-22; its release build passed (run 19 of `release.yml`: verify, the guide, the Linux and the Windows package, the sums and the draft). Not tested by the owner yet; the rows open at the cut are worked on for a second candidate, with a review and a persona round before it (owner) |
| `v0.3.5-rc.2` | `release/0.3`, the commit the tag names | [`checklist-rc.2.md`](checklist-rc.2.md) | Cut on 2026-11-23, with what was done between the candidates (below); its release build passed (run 20 of `release.yml`: verify, the guide, the Linux and the Windows package, the sums and the draft). Not tested by the owner yet. A third candidate follows after the hygiene step that the owner added to the milestone (step 15 of the plan) |

Before the first cut, on the tip that the release branch moves to: two reviews, each with its fixes. The first
read everything from 0.3.0 to the end of the fix round's sweep, in four parts (the format, the model and the
commands; the derivation, the lists and the drawing; the window; the tests, the documents and hygiene): four major
findings, mended or settled by the owner's ruling on versions, and about twenty small ones mended by the reviewers.
The second read
the three bundles that came after it (the window and what symbols print; the placer and the router; the model and
the format): one major finding, a report import that closed a project edited while its file was read, mended with a
test of that case, and eight small ones mended. No persona round was run for this release. After the version was
set, with nothing else building: the whole workspace (`cargo xtask check`, 7,774 of 7,774
tests), the deep run with the private corpus (9 of 9), the format guard and the guide's translation check passed.
The candidate's commit adds to that tree the update of `url`, which the tests of `ew` passed with, and documents
(this dossier, the plan's log, one line of the notes).

Fifteen rows of the fix round are open at the cut, each with what was tried
([backlog](../../../plan/backlog.md), "0.3.5: the fix round"); the notes' section 7 names them in a reader's words.

The release branch `release/0.3` was moved forward to the working branch's tip: everything since 0.3.0 on the
working branch is this release.

**Between the first and the second candidate.** The rows open at the first cut were worked in three bundles, and
a review of those bundles found no major fault and eleven small ones, mended by the reviewers. Then personas (a
model that works the program from pictures of its window, as a person new to it would, on a job sheet) ran nine
times in two rounds: four jobs on the tree after the review (a second installation, parts and terminals of one's
own, a layout and a drawing, a complete cabinet: 7, 6, 6 and 7 of 10, no crash, nothing lost) and five re-tests
after the fixes (8, 6, 5, 6 and 5 of 10). What they met was mended by lanes in between: where a person looks in
the window and what it says, terminals of two levels, a cut and its paste as a move, the form for a part of
one's own, a strip of terminals that keeps its order under Arrange and route, a card laid in one row whichever
way it is placed. The verdicts did not rise as the fixes went in, and the owner's reading of that is on record in
the plan's log: the lanes mended findings and not their causes, the causes are saved for later work, and the
placer is a general aid whose page needs a draughtsman's eye. No fix was made after the last run. On the
candidate's tree, with nothing else building, the whole workspace (7,881 of 7,881 tests), the deep run with the
private corpus (9 of 9), the format guard, the guide's translation check, the advisories and the translation
status pass; the first such run failed on three things, which the checklist's RC-02 names.

Eighteen rows of the fix round are open at the second cut ([backlog](../../../plan/backlog.md), "0.3.5: the fix
round"), and the notes' section 7 names what a reader would meet.

## 3. The release manager's decisions

- The README's pictures were made on the day of the cut from the program as the candidate's tree draws it; no
  picture of the suite changed between them and the cut, so they were not made again. Its download table and its
  "Where it stands" stay at 0.3.0 until 0.3.5 is published.
- One advisory appeared on the day of the cut (RUSTSEC-2026-0592, `url` 2.5.8, in the graph of the tests of `ew`
  only): `url` was updated to 2.5.9 before the cut; no waiver was written.
- The format guard read the unreleased notes only and so failed from a cut until the final tag; it now reads the
  notes of the release being cut as well (`xtask/src/api_guard.rs`).

- For the second candidate the notes' summary was written again for what changed since the first, and the notes'
  known limitations gained what the persona runs showed and no lane mended: a move to another page drops the wires
  to what stays, a strip wired to a card in another order leaves its page as it is, a station is drawn on one page.
- The fixes of the persona rounds were checked lane by lane on the crates each touched; the harness's own tests,
  the deep run and the translation status ran only at the exit, where three things failed and were mended. For
  the third candidate a lane that changes a layout, a message or a tutorial runs those as well.

## 4. The go

Not given yet.
