<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# ElektraWatt `ew`: unsigned development build, not a release

This archive holds the `ew` command-line program built by the nightly workflow `cli-artefact.yml` from the
commit named below. It is an **unsigned development build, not a release**:

- it has no version tag and no signature, and nothing in it is a statement of what a release will contain;
- it is built in the release profile on a hosted CI runner, with no signing step, so Windows may warn about
  an unknown publisher;
- it is kept as an artefact of the CI run for 90 days and then deleted; it is not a release;
- it is for people who work on ElektraWatt or test the CLI; do not use it for a project that matters, and do
  not redistribute it as a release.

The program is free software under the GPL-3.0-or-later (`LICENSES/GPL-3.0-or-later.txt`). The archive also
holds `LICENSES/CC-BY-SA-4.0.txt` and `LICENSES/OFL-1.1.txt` (the symbols and the fonts that the program embeds),
`NOTICE` (attributions, including the Noto trademark line and the symbol attribution) and
`THIRD-PARTY-NOTICES.txt` (the licences of the libraries). Its source is the commit named below in the project
repository. `ew --version` prints the version of the workspace, which stays
`0.0.0` until the first release.

