# Project format and persistence

**Status:** Draft v0.3 · **Part of:** ElektraWatt design document set

> **Anonymization note:** dates and the manufacturing year of the reference project in this document are synthetic placeholders, not the original values.

**Purpose.** This document specifies how an ElektraWatt (EW) project lives on disk and travels between machines:
- the directory layout and the canonical JSON Lines encoding;
- the manifest and its generations, and the save protocol;
- the lock, the crash journal and recovery, and external-change handling;
- `.ewz` transport, format versioning and git integration;
- library embedding and the as-built archive.

It elaborates [architecture](04-architecture.md) §7 and ADR-0010 … ADR-0013 without changing their decisions.
Its refinements of those ADRs are recorded as their amendments and are marked where they apply. The crates
concerned are `ew-format` (MIT OR Apache-2.0), `ew-store` and `ew-library` (GPL-3.0-or-later).

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Principles and locations](05-layout-encoding.md#1-principles-and-locations) | [principles, directory layout and canonical encoding](05-layout-encoding.md) |
| [§2. Directory layout and record ownership](05-layout-encoding.md#2-directory-layout-and-record-ownership) | [principles, directory layout and canonical encoding](05-layout-encoding.md) |
| [§3. Canonical encoding](05-layout-encoding.md#3-canonical-encoding) | [principles, directory layout and canonical encoding](05-layout-encoding.md) |
| [§4. Manifest and generations](05-save-open-recover.md#4-manifest-and-generations) | [manifest, save, open, lock, journal and external changes](05-save-open-recover.md) |
| [§5. Save protocol](05-save-open-recover.md#5-save-protocol) | [manifest, save, open, lock, journal and external changes](05-save-open-recover.md) |
| [§6. Open, verification and integrity](05-save-open-recover.md#6-open-verification-and-integrity) | [manifest, save, open, lock, journal and external changes](05-save-open-recover.md) |
| [§7. Lock file](05-save-open-recover.md#7-lock-file) | [manifest, save, open, lock, journal and external changes](05-save-open-recover.md) |
| [§8. Crash journal and recovery](05-save-open-recover.md#8-crash-journal-and-recovery) | [manifest, save, open, lock, journal and external changes](05-save-open-recover.md) |
| [§9. External changes](05-save-open-recover.md#9-external-changes) | [manifest, save, open, lock, journal and external changes](05-save-open-recover.md) |
| [§10. Format versioning and migration](05-versions-git-ewz.md#10-format-versioning-and-migration) | [versions and migration, git and .ewz transport](05-versions-git-ewz.md) |
| [§11. Git integration](05-versions-git-ewz.md#11-git-integration) | [versions and migration, git and .ewz transport](05-versions-git-ewz.md) |
| [§12. `.ewz` transport](05-versions-git-ewz.md#12-ewz-transport) | [versions and migration, git and .ewz transport](05-versions-git-ewz.md) |
| [§13. Libraries, catalogues and embedding](05-libraries-archive.md#13-libraries-catalogues-and-embedding) | [libraries, revisions and the archive, platforms and the sample project](05-libraries-archive.md) |
| [§14. Revisions and the as-built archive](05-libraries-archive.md#14-revisions-and-the-as-built-archive) | [libraries, revisions and the archive, platforms and the sample project](05-libraries-archive.md) |
| [§15. Platform, share and cloud-sync pitfalls](05-libraries-archive.md#15-platform-share-and-cloud-sync-pitfalls) | [libraries, revisions and the archive, platforms and the sample project](05-libraries-archive.md) |
| [§16. UI and UX touchpoints](05-libraries-archive.md#16-ui-and-ux-touchpoints) | [libraries, revisions and the archive, platforms and the sample project](05-libraries-archive.md) |
| [§17. The sample project on disk](05-libraries-archive.md#17-the-sample-project-on-disk) | [libraries, revisions and the archive, platforms and the sample project](05-libraries-archive.md) |
