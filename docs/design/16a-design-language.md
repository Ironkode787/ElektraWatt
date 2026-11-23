# Design language

**Status:** v1.3, accepted 2026-10-28 · **Date:** 2026-10-28 · **Decision:** [ADR-0031](../adr/0031-design-language-and-task-ordered-command-bar.md)
(Accepted) · **Visual reference:** [the mockup](assets/design-language-mockup.html) (approved by the owner) · **Work package:**
WP-S2.17 · **Part of:** ElektraWatt design document set

**Purpose.** This document is the visual and verbal language of ElektraWatt (EW): the pillars every screen follows, the
design tokens (colour, type, spacing, sizes, radii, borders, elevation, icons, motion, density) in light, dark and
high-contrast themes, the layout of the main window and its command surface, the component inventory with anatomy and
states, the canvas overlays, the wording rules, the order in which commands are offered, and the map from today's
window to the target. [16 UX and UI](16-ux-ui.md) says what the window does; this document says how it looks and
reads. Where the two differ on appearance, this one wins (ADR-0031 decision 1); behaviour stays doc 16's. The
owner-approved mockup is the visual reference: every value here is the mockup's, and where this text and the mockup
differ the mockup wins, except for the two corrections that requirements force (§11).

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Why a design language](16a-pillars-tokens.md#1-why-a-design-language) | [the user, the pillars and the tokens](16a-pillars-tokens.md) |
| [§2. The user and the job](16a-pillars-tokens.md#2-the-user-and-the-job) | [the user, the pillars and the tokens](16a-pillars-tokens.md) |
| [§3. Pillars](16a-pillars-tokens.md#3-pillars) | [the user, the pillars and the tokens](16a-pillars-tokens.md) |
| [§4. Tokens](16a-pillars-tokens.md#4-tokens) | [the user, the pillars and the tokens](16a-pillars-tokens.md) |
| [§5. Layout architecture](16a-layout.md#5-layout-architecture) | [the layout](16a-layout.md) |
| [§6. Components](16a-components.md#6-components) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.1 States](16a-components.md#61-states) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.2 Buttons](16a-components.md#62-buttons) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.3 Icon buttons and command buttons](16a-components.md#63-icon-buttons-and-command-buttons) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.4 Tabs](16a-components.md#64-tabs) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.5 Fields](16a-components.md#65-fields) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.6 Tree rows](16a-components.md#66-tree-rows) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.7 Table and grid rows](16a-components.md#67-table-and-grid-rows) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.8 Property rows and the Properties header](16a-components.md#68-property-rows-and-the-properties-header) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.9 Cards](16a-components.md#69-cards) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.10 Toasts and the notifications log](16a-components.md#610-toasts-and-the-notifications-log) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.11 Banners](16a-components.md#611-banners) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.12 Dialogs](16a-components.md#612-dialogs) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.13 Menus and context menus](16a-components.md#613-menus-and-context-menus) | [components: states, buttons, fields, rows, cards, toasts, dialogs and menus](16a-components.md) |
| [§6.14 The command palette](16a-components-more.md#614-the-command-palette) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.15 The symbol and part palettes](16a-components-more.md#615-the-symbol-and-part-palettes) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.16 Chips and badges](16a-components-more.md#616-chips-and-badges) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.17 Empty states](16a-components-more.md#617-empty-states) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.18 Tooltips](16a-components-more.md#618-tooltips) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.19 Progress](16a-components-more.md#619-progress) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.20 Segmented control](16a-components-more.md#620-segmented-control) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.21 Data grid (a navigator's table mode)](16a-components-more.md#621-data-grid-a-navigators-table-mode) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.22 Chooser list](16a-components-more.md#622-chooser-list) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.23 Preview card, the Preview tab and the card strip](16a-components-more.md#623-preview-card-the-preview-tab-and-the-card-strip) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.24 Finding rows, History rows and the chip line](16a-components-more.md#624-finding-rows-history-rows-and-the-chip-line) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.25 The built widgets mapped](16a-components-more.md#625-the-built-widgets-mapped) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§6.26 The print pre-dialog](16a-components-more.md#626-the-print-pre-dialog) | [components: palettes, chips, grids, choosers, preview cards and finding rows](16a-components-more.md) |
| [§7. Canvas overlays](16a-overlays-wording.md#7-canvas-overlays) | [canvas overlays, wording, command progression and sources](16a-overlays-wording.md) |
| [§8. Wording and capitalisation](16a-overlays-wording.md#8-wording-and-capitalisation) | [canvas overlays, wording, command progression and sources](16a-overlays-wording.md) |
| [§9. Command progression](16a-overlays-wording.md#9-command-progression) | [canvas overlays, wording, command progression and sources](16a-overlays-wording.md) |
| [§10. Migration map from the current window](16a-overlays-wording.md#10-migration-map-from-the-current-window) | [canvas overlays, wording, command progression and sources](16a-overlays-wording.md) |
| [§11. The mockup, the visual reference](16a-overlays-wording.md#11-the-mockup-the-visual-reference) | [canvas overlays, wording, command progression and sources](16a-overlays-wording.md) |
| [§12. Sources](16a-overlays-wording.md#12-sources) | [canvas overlays, wording, command progression and sources](16a-overlays-wording.md) |
