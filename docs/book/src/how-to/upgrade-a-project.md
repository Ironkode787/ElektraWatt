# Upgrade a project

This guide takes a project made with an earlier release of ElektraWatt to the release you have installed. Installing
the new release itself is in [Install ElektraWatt](install.md#upgrade).

## What a release reads and writes

The version of ElektraWatt and the version of the project format are numbered separately. Each release writes one
format, the current one, and reads that format and every older one it has a migration for. The
[release notes](../reference/release-notes/index.md) of each release name the format it writes.

- **An older format** opens migrated in memory: you see and edit the project as the current format holds it, and
  nothing on disk changes until you save.
- **A newer or an unreleased format**, one that a later release or a development build wrote, opens read-only. The
  banner says so: "This project is open read-only: a file format this version does not write." Open it with the
  release that wrote it.

## Upgrade in the window

1. Open the project with **File ▸ Open…** (`Ctrl+O`).
   The banner says "This project is in an older format; saving converts it."
2. Choose **File ▸ Save** (`Ctrl+S`).
   The dialog "Convert this project to the current format?" names the older format and the new one. Older versions
   of ElektraWatt cannot open the project once it is converted.
3. Choose **Upgrade and save**.
   ElektraWatt first writes a backup of the project as it was, `<project folder>.pre-<old format>.ewz`, beside the
   project folder, then saves the project in the current format. If a file of that name already holds something
   else, the backup takes the next free name (`.pre-<old format>-2.ewz` and so on).

To go back, keep the backup: it is the whole project in one file. **File ▸ Open .ewz…** opens it read-only, or
extracts it into a new folder beside it, which the earlier release opens.

## Upgrade from the terminal

`ew upgrade` does the same without the window:

```console
$ ew upgrade --project my-project.ew --check
$ ew upgrade --project my-project.ew
```

- `--check` only reports: it exits 0 when the project needs no upgrade and 1 when it is in an older format, and it
  writes nothing.
- Without `--check`, `ew upgrade` asks you to type `upgrade` to confirm, writes the backup beside the project folder
  and saves the project. `--yes` confirms without asking, for a script; without a terminal and without `--yes` it
  changes nothing and exits 2.

The [CLI reference](../reference/cli/upgrade.md) lists every flag and exit code.

## What changes in the format from 0.1.0

Until the release 0.1.0 the format changed from build to build without migrations. From 0.1.0 on, the format that
0.1.0 writes is fixed: a later release changes it only by adding to it, so that a project of 0.1.0 opens and saves
unchanged, or by a new format version that comes with a migration from the released one, and with the first save's
backup described above. The rule is the project's
[format decision](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/adr/0011-format-versioning-and-migration.md),
amendment 6.
