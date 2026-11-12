<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Security policy

## Reporting a vulnerability

Report vulnerabilities privately through GitHub Security Advisories: use "Report a vulnerability" on the
repository's Security tab (private vulnerability reporting). Never report a vulnerability in a public issue,
discussion or pull request.

Please include:

- the affected version or commit;
- steps to reproduce;
- the impact as you see it.

Do not attach customer files or personal data.

This channel also receives Code of Conduct reports until the conduct contacts are published (see
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)).

## What happens next

- We acknowledge your report within 7 days.
- We triage it within 14 days: a CVSS 4.0 score, the affected versions, and whether it is exploited.
- For critical issues, we aim to ship a fix or mitigation within 30 days.
- Disclosure is coordinated, with an embargo of at most 90 days: the GitHub advisory is published with the fix, or
  earlier by agreement with the reporter.
- A CVE is requested through GitHub for each fixed vulnerability.
- Reporters are credited if they wish.

## Supported versions

| Version | Supported |
|---|---|
| 0.2.x | Yes, until the next minor release (0.3.0) |
| 0.1.x | No |
| Earlier development builds | No |

Before 1.0 only the latest 0.x release is supported. From 1.0, the latest minor gets all fixes, and each minor gets
security and data-loss fixes for 6 months after its successor ships. There is no long-term-support line.

## Verifying a download

No release before 1.0 is signed. Each release carries `SHA256SUMS`, the SHA-256 checksum of each of its files. The
commands to check a file against it are in the install guide's section
[Verify a download](docs/book/src/how-to/install.md#verify-a-download).

## Scope

In scope:

- the desktop application `elektrawatt` and the command-line tool `ew`;
- the format crates;
- the release files and the release workflow that builds them.

Out of scope:

- vulnerabilities of third-party crates: report them to their maintainers, with a note to this project so that we can
  update;
- the content of users' own projects.

## The threat model

What each release does against each threat, with its controls and evidence, is in the
[threat model](docs/security/threat-model.md).

## What follows at 1.0

The notification process of the EU Cyber Resilience Act and the disclosure exercise follow by 1.0 (WP-S6.10).
