<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Privacy

ElektraWatt is a desktop application that runs on your computer. This document lists everything it reads and writes
outside the project you are working on, every data flow that can leave your computer, and how to remove what it
stores. Changing the list of network flows requires an architecture decision record (ADR-0029, NFR-PRIV-07).

## In short

- **No telemetry.** There is no analytics, no usage statistics, no install ID, no account and no licence check.
- **No network connection of its own.** Start-up opens no connection. This version has no update check, no crash
  upload and no plugin network; nothing is downloaded (fonts, symbols and parts ship with the application).
- **Your project data stays in your project files.** The application never copies a project, its drawings, its
  part data, its logos or its file names anywhere except to the places listed below, all on your computer.
- **Crash reports stay on your computer.** This version writes none to upload.
- **Contact.** Open an issue in the repository's issue tracker (never put project data or personal data in one).
  Vulnerabilities are reported privately as described in [SECURITY.md](SECURITY.md).

## What the application stores on your computer

| What | Purpose | Location | Retention | How to delete |
|---|---|---|---|---|
| Settings (`settings.toml`): language, theme, keymap, UI scale, the first-run answers, the update-check choice (off), the recent-projects list (paths as text) | Remember your choices and offer recent projects | The configuration directory: `%APPDATA%\ElektraWatt` on Windows, `~/Library/Application Support/ElektraWatt` on macOS, `$XDG_CONFIG_HOME/elektrawatt` or `~/.config/elektrawatt` on Linux (`EW_CONFIG_DIR` overrides it) | Until you delete it; the list keeps the ten latest projects | Delete the file, or the directory |
| Workspace layouts (`workspaces/<name>.json`) | Save and restore window layouts you name | `workspaces/` in the configuration directory | Until you delete them | Delete the file (View ▸ Workspace ▸ Reset layout restores the default layout but deletes no file) |
| Crash journal (`journal/`) | Recover unsaved changes after a crash or power loss; holds the field-level changes of each commit and the bytes of assets (logos) the changes name, so it can contain project content | The state directory: `%LOCALAPPDATA%\ElektraWatt\state` on Windows, `~/Library/Application Support/ElektraWatt/state` on macOS, `$XDG_STATE_HOME/elektrawatt` or `~/.local/state/elektrawatt` on Linux | Deleted on a clean close and after a successful save; a leftover journal (one per project, under `journal/<project>-<hash>`) is pruned after 30 days, when that project is next opened | Discard it when the application offers it at the next open, or delete the `journal` directory |
| Lock file (`.~ew.lock`) | Tell another opener that the project is in use | The project directory, while the project is open | Removed on close; a stale one is replaced on confirmed takeover (renamed `.~ew.lock.stale-<session>`) | Delete it once no session is open. It records the operating-system user name, the host name, the process ID, the program version and the start time, so leave it out of anything you share |
| The project (`.ew` directory: records, `assets/`, generated outputs) | Your work | Where you save it | Yours to keep | Delete the directory. Logos and other opaque files sit in `assets/` under their content hash |

Project files are plain text and contain what you enter: company, project and drawing identifiers, the names you
type, the logo you pick. Check a project before you share it (for example in an issue or a repository); the
application does not remove such content for you. Exported files (PDF, reports) are written where you choose.

## Data flows that can leave your computer

| Flow | Trigger | Content | Endpoint | Default | In this version |
|---|---|---|---|---|---|
| Update check | Opt-in, at most once a day | `GET` of a signed manifest; the product version, the operating system and the architecture | Project site or a mirror you configure | Off; off in managed installations | Not implemented; the setting is stored and nothing is sent |
| Crash upload | Per report, after you preview the payload | A reduced stack trace without project text | None decided | Off; the button is hidden while no endpoint exists | Not implemented |
| Help and community links | You click one | A URL opened in your system browser; the application itself sends nothing. The About dialog and the first-run privacy step link to this document in the repository | The documentation site and the repository's pages | Offline user guide shipped with the application | The browser is started by you; the browser then talks to the site |
| Plugin network | Capability granted at install (1.x) | Plugin-defined | Plugin-defined | Denied | Not implemented |
| `ew mcp` | You start it | MCP over standard input and output, or `127.0.0.1` with a token | Local | Off | Not implemented |
| ODA converter | You configure it (1.0) | A local process | None | Off | Not implemented |

There are no other flows: no telemetry, no install ID, no font, symbol or part downloads, no licence check and no
AI or translation service.

## The network sandbox

The design (doc 18, section 12.2) is that continuous integration runs the application and the command line in a
network namespace with only loopback up and fails the build on any socket, DNS lookup or listening port outside the
list above. With the update check opted in, a second run against a local mock server asserts the exact request.
Windows and macOS are observed by hand for each minor release with outbound firewall logging. The sandbox job
is being built; until it runs on every change, "no network connection of its own" is a design rule that the code
follows (the code has no network client of its own), not yet a proven one.

## Changes

A change to the flows above is a decision that is recorded and reviewed before it ships. The history of this file
is the history of the promise: read it with `git log -p PRIVACY.md`.
