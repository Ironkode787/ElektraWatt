<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.2.0-rc.3

The third candidate of 0.2.0 is the second with one line of the Windows setup's script changed. The release build
of `v0.2.0-rc.2` failed in its step "compile the per-user setup", so that candidate has no downloads: the script
named the application icon by `${__FILEDIR__}`, which is the script's folder as the command line names it, and the
workflow starts the compiler at the repository's root with a relative script path, so the icon was looked up from
inside the script's folder a second time. The path is now relative to the script's folder.

**Candidate:** `v0.2.0-rc.3` · **Commit:** the commit of `release/0.2` that the tag names · **Draft:** the draft
prerelease `v0.2.0-rc.3` on the repository's releases page · **Filled by:** the release manager ·
**Date:** 2026-11-12

Every row of [the second candidate's checklist](checklist-rc.2.md) stands for this candidate: no file that a test
reads changed. What was run for this candidate:

| What | Result |
|---|---|
| The setup compiled with the pinned NSIS (`packaging/components.toml`) on a small tree, started at the repository's root as the workflow starts it, in the script's folder, and elsewhere by an absolute path | Compiles in all three |
| `cargo xtask rules` | Passes |
| The whole workspace, the deep run and the format guard | Not run again: they passed on the second candidate's commit, and the one changed file is the setup's script |

Not covered by anything that runs by hand: the setup itself was not installed. The release workflow's Windows job
compiles it from the real tree; the owner's hands-on test installs it.
