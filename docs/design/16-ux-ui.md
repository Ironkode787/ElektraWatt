# User experience and user interface

**Status:** Draft v0.1

**Purpose.** This document specifies how people work with ElektraWatt (EW): the main window and its docking
workspace, navigators, canvas tabs, the property grid with provenance, the command palette and keymaps, table
editing with bulk previews, history, notifications for background jobs, the first-run company-standard wizard,
aids for users coming from EPLAN and QElectroTech (QET), accessibility, theming, UI localisation, help, the
performance budgets as the user perceives them, and the usability test that gates M2 and 1.0. It elaborates
FR-UI-001 … FR-UI-051 of [02b](02b-requirements-extended.md) and the NFR-A11Y and NFR-UX rows of
[02c](02c-nonfunctional-requirements.md) on the egui shell behind `ew-ui` and `ew-viewmodel` fixed by the
[architecture](04-architecture.md) §9 and ADR-0014. Canvas tools are specified in [06](06-schematic-editor.md) and
domain panels in their area documents; this document owns the shell they live in and the rules they share.

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Position in the architecture](16-shell.md#1-position-in-the-architecture) | [the shell and the main window](16-shell.md) |
| [§2. Concepts and data](16-shell.md#2-concepts-and-data) | [the shell and the main window](16-shell.md) |
| [§3. Main window](16-shell.md#3-main-window) | [the shell and the main window](16-shell.md) |
| [§4. Interaction model](16-interaction.md#4-interaction-model) | [the interaction model](16-interaction.md) |
| [§5. Navigators](16-panels.md#5-navigators) | [navigators, properties, tables and history](16-panels.md) |
| [§6. Property grid with provenance](16-panels.md#6-property-grid-with-provenance) | [navigators, properties, tables and history](16-panels.md) |
| [§7. Tables, bulk edits and previews](16-panels.md#7-tables-bulk-edits-and-previews) | [navigators, properties, tables and history](16-panels.md) |
| [§8. History, notifications and background jobs](16-panels.md#8-history-notifications-and-background-jobs) | [navigators, properties, tables and history](16-panels.md) |
| [§9. Onboarding and migration aids](16-onboarding-access.md#9-onboarding-and-migration-aids) | [onboarding and accessibility](16-onboarding-access.md) |
| [§10. Accessibility](16-onboarding-access.md#10-accessibility) | [onboarding and accessibility](16-onboarding-access.md) |
| [§11. Theming](16-theming-help.md#11-theming) | [theming, language, help and the rest](16-theming-help.md) |
| [§12. UI localisation](16-theming-help.md#12-ui-localisation) | [theming, language, help and the rest](16-theming-help.md) |
| [§13. Help system](16-theming-help.md#13-help-system) | [theming, language, help and the rest](16-theming-help.md) |
| [§14. Performance as the user sees it](16-theming-help.md#14-performance-as-the-user-sees-it) | [theming, language, help and the rest](16-theming-help.md) |
| [§15. Algorithms](16-theming-help.md#15-algorithms) | [theming, language, help and the rest](16-theming-help.md) |
| [§16. Outputs](16-theming-help.md#16-outputs) | [theming, language, help and the rest](16-theming-help.md) |
| [§17. Edge cases and pitfalls](16-theming-help.md#17-edge-cases-and-pitfalls) | [theming, language, help and the rest](16-theming-help.md) |
| [§18. Defects prevented at the interface](16-theming-help.md#18-defects-prevented-at-the-interface) | [theming, language, help and the rest](16-theming-help.md) |
| [§19. Usability test plan](16-theming-help.md#19-usability-test-plan) | [theming, language, help and the rest](16-theming-help.md) |
