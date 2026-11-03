<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# ElektraWatt `ew`, the command-line program

This archive holds `ew` (`ew.exe` on Windows), the command-line program of ElektraWatt, with what it needs offline.
Its folder is named `ew-<version>-<target>`; `<version>` is the version of the build, and `ew --version` prints the
version of the source it was built from, which `<version>` begins with.

| Path | What it is |
|---|---|
| `ew`, `ew.exe` | The program |
| `content/` | The starter content: symbols, function definitions, forms, profiles and the starter library |
| `samples/J-E-2416.ew/` | The sample project, Golden A |
| `LICENSES/`, `NOTICE`, `THIRD-PARTY-NOTICES.txt` | The licence texts, the attributions and the licences of the libraries |
| `PRIVACY.md`, `SECURITY.md` | What the program does with your data, and how to report a security problem |

## Using it

`ew` finds the starter content through `--content` or the environment variable `EW_CONTENT_DIR`, not beside itself.
Set `EW_CONTENT_DIR` to the archive's `content` folder:

- Linux: `export EW_CONTENT_DIR=<folder>/content`
- Windows (PowerShell): `$env:EW_CONTENT_DIR = "<folder>\content"`

where `<folder>` is the folder you unpacked, `ew-<version>-<target>`. `ew --help` lists the commands; `ew export pdf
<folder>/samples/J-E-2416.ew --out sample.pdf` writes the sample project's drawings as a PDF.

## Unsigned

The program is not signed: Windows may warn about an unknown publisher. Verify a download against the checksums
published with its release on the project's repository, <https://github.com/Ironkode787/ElektraWatt/releases>. A
version that ends in `-dev.<commit>` is a development build of that commit from the nightly workflow, not a release:
it is kept as an artefact of the CI run for 90 days and then deleted; do not use it for a project that matters, and
do not redistribute it as a release.

## Licence

The program is free software under the GPL-3.0-or-later (`LICENSES/GPL-3.0-or-later.txt`); its source is the
project's repository at the commit it was built from. The starter content is under CC-BY-SA-4.0, its starter parts
library and the sample project under CC0-1.0, and the program embeds the OFL-1.1 fonts named in `NOTICE`. `THIRD-PARTY-NOTICES.txt` lists the
licences of the libraries it is built from.
