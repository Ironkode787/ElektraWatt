# ADR-0031: Design language and the task-ordered command bar

**Status:** Accepted · amended 2026-10-30 (no macOS menu bar, decision log ID-880) · **Date:** 2026-10-28 · **Deciders:** lead designer; the project owner approved the look on
[the mockup](../design/assets/design-language-mockup.html) (2026-10-28), and the orchestrator decided the remaining taste choices under the owner's delegation

## Context

- **The window is a mess.** The cycle-30 window checks show egui's default dark theme with no hierarchy: a menu bar
  of eight menus and no toolbar, a hint strip under the menu that repeats the status bar, pane titles that repeat
  their tabs ("Pages" over a tab "Pages", "Page view" over the canvas tabs), a property grid whose every row carries
  "= authored" and an "edit" button and that lists internal fields (`$v`, keys, UUIDs), navigator rows without icons
  and with "[QET grid]" in the text, a left tab strip that hides navigators behind an overflow arrow (FT-509),
  dialogs with grey centred titles and buttons of equal weight, chips that collide on the canvas (FT-507, FT-508) and
  highlight colours below the sheet's contrast floor (FT-506). The owner calls it "a great big mess" and asks for a
  design language: sleek, clear, minimalist, the eye drawn to what matters, a logical menu progression
  ([16a](../design/16a-design-language.md) §1).
- **Who uses it.** Panel builders who know EPLAN (a ribbon since EPLAN 2022, which replaced its menus and toolbars),
  QElectroTech users (menus and toolbars) and new users. The usability gates (NFR-UX-01, NFR-UX-04, R-069) are
  passed by people who find commands without help.
- **What is already fixed.** egui/eframe behind `ew-ui` (ADR-0014); one action registry from which menus, palette,
  keymaps and tooltips are built ([16](../design/16-ux-ui.md) §2.4); docking with egui_tiles; Noto Sans as the bundled
  face (ADR-0028, ID-37); print-true sheet colours (FR-UI-043); the accessibility NFRs (NFR-A11Y-01 to -04); the
  semantic token list of doc 16 §11.1, which was never given values.
- **Doc 16 §1 and ARCH §16 say "no ribbon".** The reasons recorded then (`docs/research/gaps-and-corrections.md`):
  a ribbon is costly and non-native in egui, and QET only added command search. Neither holds for a single-row
  command bar: in egui it is a horizontal layout of buttons over the registry, and EPLAN, the tool our users come
  from, has had a ribbon for four releases.

## Decision

1. **A design language document governs every pixel of the chrome:**
   [16a-design-language.md](../design/16a-design-language.md). It defines ten pillars with do and don't rules, the
   token set (colour, type, spacing, size, radius, border, elevation, icon, motion, density) in light, dark and two
   high-contrast themes, the layout architecture, the component inventory with anatomy and states, the wording rules,
   the command progression and the migration map. Doc 16 keeps the behaviour; 16a owns the look. Where they differ on
   appearance, 16a wins. **The owner-approved mockup** (`docs/design/assets/design-language-mockup.html`) is the
   visual reference: 16a's values are read from it, it wins over 16a's text on look, and WP-S2.17's acceptance compares
   each part with it (token values exactly, geometry within stated tolerances). Only requirements override it (16a
   §11 lists the two corrections they forced).
2. **One token source.** `ew-ui` holds the tokens as one Rust table (global ramps → alias tokens → component
   tokens), maps them onto egui `Style`, `Visuals` and `Spacing` and onto the canvas overlay colours, and dumps them
   to a blessed text file. No colour, size or radius literal appears in widget code outside the token module.
3. **The command surface is a task-ordered command bar** (a single-row ribbon in Microsoft's terms): an app-menu
   button (the product mark and name) that opens the full menu tree, a quick-access group (Save, Undo, Redo), **six
   tabs in the order of work — Project, Page, Draw, Check, Output, View —** with one row of labelled command groups
   each and, on Draw and Check, the tab's next step as the one primary button at the row's end ("Check project",
   "Export PDF"), and a command search field (the palette, Ctrl+K) at the right end of the tab row. Draw is the home tab: select, place, connect, annotate and arrange
   are its groups, left to right, so the core loop never changes tabs. No contextual tabs: the Properties panel is the
   selection's surface. The command bar is generated from the action registry (`ActionMeta` gains its tab, group,
   size and icon), so menus, bar, palette and keys cannot disagree.
4. **The menu bar becomes the app menu** on Windows and Linux (one button, the same tree as today, opened by its
   button, Alt or F10). The macOS global menu bar (muda, FT-310) is not built: macOS is not a supported platform
   (2026-10-30, decision log ID-880).
   Nothing is reachable only from the bar or only from the menu.
5. **The window layout:** command bar on top; on the left an icon rail of navigators and one navigator panel with
   Properties docked below it; the canvas in the centre with its tabs and a floating tool HUD; a right rail with the
   Symbols and Parts palettes, closed by default; a bottom dock (Messages, Find, Jobs, History, Preview) closed by
   default and opened by its status-bar field or key; a quiet status bar; on the canvas a tool HUD and a zoom control.
   The canvas takes at least 65 % of a 1280 × 800 window (72 % in the default layout).
6. **One accent colour, Volt blue #0B63CE** (dark theme #5AA2FF), used for the primary action, focus, the selected
   state in every list and the selection on the canvas; everything else is a cool neutral ramp. Semantic colours
   (error, warning, success) appear only where there is a finding or an outcome.
7. **Typography on the bundled Noto Sans** (ADR-0028), with **Noto Sans SemiBold** (OFL-1.1, the same family)
   added for emphasis; a five-step scale (11, 12, 13, 15, 20 px at 100 %); Noto Sans's digits are tabular by default
   (all ten advance widths are 572 units), so coordinates do not jitter.
8. **Icons from Tabler Icons** (MIT, 6,257 icons, 24-px grid, stroke-only paths), vendored as SVG files at a pinned
   release, converted by an `xtask` generator into a Rust table of paths and drawn with the token stroke width (1.5 px
   at 20 px) through epaint, with no icon crate and no icon font; the dozen EDA glyphs Tabler lacks are drawn in its
   grid by the project.
9. **Two densities:** Comfortable (default; rows 28 px, large command buttons with labels under the icon) and Compact
   (rows 24 px, every command with its label beside the icon), switched in View. **The default theme follows the
   system**; light is the reference of the mockup and the snapshots.

## Alternatives considered

| Option | For | Against | Verdict |
|---|---|---|---|
| **A. Menu bar plus one compact toolbar** (doc 16 §3.3, KiCad, QET) | Familiar to QET users; cheapest; what doc 16 planned | Eight menus of indirect commands are what the owner calls a mess; a toolbar without labels teaches nothing; EPLAN users lose their ribbon habit (R-069) | Rejected |
| **B. Full Office ribbon** (EPLAN 2022+): several rows, large and small buttons, group captions, contextual tabs, KeyTips | Closest to EPLAN; strong discoverability | Three rows of small buttons plus group captions take about a sixth of an 800-px laptop screen; contextual tabs move commands under the user; Microsoft's own guidance warns that a ribbon pays only when users stay on one Home tab, and that porting menus into tabs "misses most of the value" | Rejected as specified; its good parts are kept |
| **C. Task-ordered command bar (chosen):** one row per tab, labelled large buttons for the frequent commands, small ones for the rest, a home tab for the core loop, an app menu, quick access and search | Direct one-click commands with labels; the task order is visible; 85 px in Comfortable and 69 px in Compact (16a §5.1); buildable in egui as rows of buttons from the registry | Not identical to EPLAN; tab design must be right the first time (frequent tab switching kills it) | **Chosen** |
| **D. Command palette only, minimal chrome** (Linear, Zed) | The calmest look | Fails discoverability for users who do not yet know the command names (NFR-UX-04 is "without documentation") | Rejected; the palette stays as the keyboard path |

**Properties on the right** (doc 16 §2.6, Figma, Altium) was weighed against **Properties under the navigator**
(KiCad 8 and 9, the owner's reference). Two side columns leave the canvas about 50 % of a 1280-px window; stacking
navigator and Properties on the left leaves it about 70 %, and the selection's details sit under the structure the
user picked it from. Docking lets anyone move the panel; the default is the stack.

**Icon sets** weighed: Lucide (ISC, with some MIT icons from Feather; about 1,600 icons, no electrical symbols),
Phosphor (MIT; about 1,240 icons in six weights; the egui wrapper crate tracks egui 0.32, not 0.37), Tabler (MIT;
6,257 icons with nineteen `circuit-*` electrical glyphs: switch, resistor, capacitor, ground, motor, bulb, diode and
more). Tabler covers the most of our domain under the simplest licence; vendoring SVGs avoids a wrapper crate that
lags egui.

## Consequences

- **Documents.** [16a](../design/16a-design-language.md) is new. [16](../design/16-ux-ui.md) §1 (non-goals: the "no
  ribbon" line), §3.1 (the wireframe is superseded by 16a §5.1), §3.2 (the menu bar becomes the app menu), §3.3 (the
  toolbar becomes the command bar), §3.6 (status bar fields), §6.2 (provenance chips only for non-authored values),
  §10.3 (the selection colour and the four highlight slots), §11.1 (the tokens have values in 16a §4) are amended by
  notes that point here; its §1 non-goal narrows to "no multi-row Office ribbon and no contextual tabs", and
  [ARCH](../design/04-architecture.md) §16 says the same. The plan gains WP-S2.17
  ([specification](../plan/specs/WP-S2.17-design-language-and-ui-polish.md)); FT-338, FT-348, FT-390, FT-391,
  FT-425, FT-506, FT-507, FT-508, FT-509, FT-510 and FT-512's tooltip half are absorbed by it. Its 5.7 pw come from
  WP-S2.08 (4.0 pw: the theming, colour-vision and contrast half and the look of the palette, tooltips and menus) and
  WP-S2.16 (1.7 pw), so the MVP gate stays at month 41 (the specification's §15.5). *Amended 2026-10-28:* 16a v1.1
  maps the widgets built alongside it (§6.25) and WP-S2.17 grows to 6.0 pw with 0.3 pw more from WP-S2.08
  (the specification's §15.1).
- **Requirements.** No FR or NFR text changes. The language serves FR-UI-001, FR-UI-018, FR-UI-025, FR-UI-027,
  FR-UI-034, FR-UI-043 and NFR-A11Y-01 to -04; the token contrast test is NFR-A11Y-04's "contrast lint over theme
  tokens".
- **Code.** `ew-ui` gains a token module, a theme applier, an icon table and the command bar; `ew-viewmodel`'s
  `ActionMeta` gains placement metadata (tab, group, size, icon ID), which is toolkit-neutral data. The headless
  `ui.describe` gains a command-bar section and loses nothing; every expected transcript whose `layout:` or status
  line changes is re-blessed once in a reviewed commit.
- **Assets and dependencies.** Two asset additions, accepted on 2026-10-28, and no new crate in the product: `NotoSans-SemiBold.ttf` (OFL-1.1,
  the family already bundled) and the vendored Tabler SVGs (MIT) with their REUSE annotations; the generator uses
  `kurbo`, already in the lock file. The snapshot tests need `egui_kittest`'s `snapshot` and `wgpu` features (MIT OR
  Apache-2.0), a dev-dependency decision recorded in WP-S2.17 D7.
- **Risk.** A command bar that makes users switch tabs often would be worse than menus. The Draw tab must hold the
  whole core loop, and the R0 usability round (WP-S2.12) measures tab switches per task; more than one switch per
  core task reopens this ADR.

## Sources

- Microsoft, Windows 7 ribbon guidelines (Home tab, tab order, contextual tabs, "misses most of the value"):
  https://learn.microsoft.com/en-us/windows/win32/uxguide/cmd-ribbons
- Microsoft, the Simplified Ribbon (a single-line ribbon): https://support.microsoft.com/en-us/outlook/use-the-simplified-ribbon
- EPLAN Platform API, the ribbon since version 2022: https://eplan.help/en-us/infoportal/content/api/2023/TheRibbon.html
- Autodesk Fusion API, workspaces, toolbar tabs and panels: https://help.autodesk.com/cloudhelp/ENU/Fusion-360-API/files/UserInterface_UM.htm
- KiCad 8 schematic editor manual (hierarchy navigator and Properties at left): https://docs.kicad.org/8.0/es/eeschema/eeschema.html
- Tabler Icons (MIT, 6,257 icons, 24 grid): https://github.com/tabler/tabler-icons; npm `@tabler/icons` 3.50.0
- Lucide licence (ISC, Feather icons MIT): https://lucide.dev/license
- Phosphor core (MIT): https://github.com/phosphor-icons/core; egui-phosphor (egui 0.32): https://docs.rs/egui-phosphor
- Linear, "Behind the latest design refresh": https://linear.app/now/behind-the-latest-design-refresh
- NN/g, visual hierarchy: https://www.nngroup.com/articles/visual-hierarchy-ux-definition/
