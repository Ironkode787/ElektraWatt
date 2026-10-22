<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# ElektraWatt `ew`: unsigned development build, not a release

This archive holds the `ew` command-line program built by the nightly workflow `cli-artefact.yml` from the
commit named below. It is an **unsigned development build, not a release**:

- it has no version tag and no signature, and nothing in it is a statement of what a release will contain;
- it is built in the release profile on a hosted CI runner, with no signing step and no notarisation, so
  Windows and macOS may warn about an unknown publisher;
- it is not published anywhere: the CI run keeps it for 90 days and then deletes it;
- it is for people who work on ElektraWatt or test the CLI; do not use it for a project that matters, and do
  not redistribute it as a release.

The program is free software under the GPL-3.0-or-later (`LICENSE.md` in this archive); its source is the
commit named below in the project repository. `ew --version` prints the version of the workspace, which stays
`0.0.0` until the first release.

