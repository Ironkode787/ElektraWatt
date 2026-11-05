<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Manual tests: ElektraWatt 0.1.0

The tests a pipeline cannot run (RC-05; WP-S6.08 D15), run once on the last candidate's own files by the tester the
owner names, and recorded here: WP-S6.06's four desktop installs (IN-15), WP-S6.05's PV-13 on Windows, and RC-05's
remote desktop without a GPU, screen readers, printers and Windows network observation. Each record names the build it
ran on (the candidate and the file's SHA-256 from `SHA256SUMS`), the system (its edition and version, never a machine
name or serial number), the steps and the result: **passed**, or **failed** with what was seen (a failure is a
blocker, [the procedure](../../README.md#8-a-blocker) §8).

**Candidate:** none yet · **Tester:** the tester the owner names · **Date:** —

## IN-15: the four desktop installs

Each install on a clean desktop, as a standard user without network: the setup or the AppImage installs; the window
starts; the sample project opens read-only with its 36 pages; Help ▸ Documentation opens the installed user guide.

| System | File and SHA-256 | Steps | Result |
|---|---|---|---|
| Windows 10 22H2 | `ElektraWatt-0.1.0-x64-user-setup.exe` | The setup with its defaults; the window from the Start menu; the sample; the guide; the uninstall | Open |
| Windows 11 | `ElektraWatt-0.1.0-x64-user-setup.exe` | As on Windows 10 | Open |
| Ubuntu 22.04 desktop | `ElektraWatt-0.1.0-x86_64.AppImage` | Made executable and run through FUSE; the sample; the guide | Open |
| Debian 12 desktop | `ElektraWatt-0.1.0-x86_64.AppImage` | As on Ubuntu | Open |

## PV-13: the crash offer and the state folder on Windows

| Test | Build | Steps | Result |
|---|---|---|---|
| A dirty project after the window is ended | The setup | Open a copy of the sample (Save as), change it without saving, end `elektrawatt.exe` in Task Manager, start it again: the start-up offer names the project and Recover restores the change | Open |
| The state folder's ACL | The setup | `icacls` on the state folder shows access for the user alone (with the system and the administrators, as Windows grants) | Open |

## RC-05's other tests

| Test | Build | Steps | Result |
|---|---|---|---|
| Remote desktop without a GPU | The setup | Over RDP on a host without a hardware adapter: the window starts on the CPU renderer, names it in the status bar, and the five core tasks complete | Open |
| NVDA (Windows) | The setup | The five core tasks' widgets read their name, role and state | Open |
| Orca (Linux) | The AppImage | As with NVDA | Open |
| Two printers | The setup or the AppImage | Output ▸ Print… of the sample to two printers of different makes: the pages come out whole, scaled when asked | Open |
| Windows network observation | The setup | A session of the core tasks under a network monitor: no connection is opened | Open |
