# User experience and user interface: theming, language, help and the rest

A chapter of [User experience and user interface](16-ux-ui.md), whose table says where each section lives.

---

## 11. Theming

### 11.1 Tokens and themes

[16a](16a-design-language.md) §4 gives these tokens their values, adds the alias and component layers and the densities, and maps them onto egui (§4.14).

Themes are sets of semantic tokens: `surface`, `surface_alt`, `text`, `text_secondary`, `accent`, `focus`,
`border`, `canvas_surround`, `sheet`, `grid_dot`, `selection`, `hover`, `highlight_1 … 4`, `error`, `warning`,
`info`, `pending`, `stale_hatch`. Shipped themes: Light, Dark, High contrast dark (MVP), High contrast light
(Should). The default follows the system through egui's theme preference. Tokens map to egui visuals and to a
uniform of the 2D and 3D renderers. A contrast lint checks every text pair (≥ 4.5:1; ≥ 7:1 in high contrast) and
every overlay slot against the sheet (≥ 3:1) per PR (NFR-A11Y-04).

### 11.2 Print-true page colours

The sheet is rendered as it prints in every theme (FR-UI-043): white paper, authored line and text colours, the
frame and title block as in the PDF. Themes change only the chrome and the canvas surround. Display lists never
contain theme colours; highlights live in the session-only highlight layer (06 §7). **Preview as printed** switches
between colour and monochrome so the user sees what `ew print` in monochrome mode produces.

### 11.3 Dark sheet (optional view)

At 1.0 a "dark sheet" view option maps neutral colours (black ↔ near-white, white → #1E1E1E) and adjusts the
lightness of chromatic colours to keep ≥ 3:1 on the dark sheet, keeping their hue. It is a colour transform in the
renderer's final pass, applied to the screen only. Thumbnails, PNG export, PDF and print are always print-true.

---

## 12. UI localisation

- **Stack.** fluent-bundle 0.16.0 with one `.ftl` resource per crate (`ew-ui`, `ew-viewmodel`, `ew-checks` rule
  messages, `elektrawatt` menus), loaded at start and switchable at run time without restart (menus are rebuilt).
  Keys are `area-object-detail` (`cmd-numbering-renumber`, `rule-pot013-title`, `rule-pot013-remedy`). Messages
  take arguments (`{ $tag }`, `{ $count }` with plural selectors) and are never concatenated. Bidi isolation marks
  are disabled (`set_use_isolating(false)`): v1 has no right-to-left UI locale, and the marks would otherwise end up
  in copied messages.
- **Locales.** English is the source; Dutch is complete at MVP and German by 1.0 (FR-UI-032). A locale is listed as
  supported only with ≥ 95 % of strings and 100 % of core-flow strings translated (NFR-I18N-01). Translation runs
  through Weblate, with a terminology glossary per locale.
- **Fallback.** nl-BE → nl → en, per message.
- **CI.** A string-literal lint on `ew-ui` and `ew-checks`; a pseudo-locale `en-XA` (+35 % length, accented
  characters) runs the core kittest flows without truncation or overlap (NFR-I18N-02); a translation-completeness
  gate.
- **Three language axes never mix:**

| Axis | Set by | Affects |
|---|---|---|
| UI locale | user setting | menus, dialogs, messages, number display and key labels in the UI ("Strg+K" in German) |
| Project languages (`MlText`) | project | function texts, page descriptions, comments ([02b](02b-requirements-extended.md) §12) |
| Output display language | export setting | PDF, reports (FR-I18N-005, 1.0) |

  Switching the UI to Dutch translates no function text. Identifiers are never localised (FR-I18N-002). Number
  input accepts both decimal separators where unambiguous (§6.3); outputs follow project settings (FR-I18N-009).

---

## 13. Help system

- **Documentation** is an mdBook in the Diátaxis structure, versioned per release, translated through PO files and
  bundled offline with the application (NFR-DOC-01, NFR-I18N-07).
- **F1** never does nothing and answers for the most specific thing under the hand, in this order (FR-UI-031): what
  the pointer rests on (a command's button or menu entry → its command's section; a property row → its row in the
  properties reference; a list's row → its thing's section, a finding → its rule page, FR-CHK-015, NFR-DOC-02; a
  thing on the page → its kind's section); else the keyboard focus (the cursor row of a list or of Properties); else
  the selection; else the active tool → its tool section; else the screen (an open dialog first, the symbol editor, a
  station's view, the page's kind, the project view, the start page). A thing without a section of its own opens its
  kind's chapter, never the documentation home.
- **Opening.** Bundled pages open in the system browser through a local `file://` URL, as a user action with no
  network traffic (NFR-PRIV-01). Links to the online version are labelled as external. Sandboxed Linux packages
  open them through the OpenURI portal (**verify**). *Exception:* the
  "Privacy (PRIVACY.md)" button of About and of the first-run privacy step opens the repository's web address of
  `PRIVACY.md` instead, because no build bundles the file yet; on failure a banner shows the address, and a build that bundles
  `PRIVACY.md` with the user guide replaces the web address by the local `file://` copy (§3.9).
- **Tooltips** show the label, the shortcut in the active keymap, one line of description and "F1: help"; while the
  pointer rests on something with a topic, the status bar says what F1 opens there ("F1: Wires").
- **Anchor registry.** Every registered tool, dialog and panel has a documentation anchor, and every rule's `docs`
  reference resolves in the bundled documentation; CI fails otherwise (NFR-DOC-01, NFR-DOC-02).
- **Generated reference pages:** command catalogue, keymap sheets per preset, rule catalogue (NFR-DOC-06).
- **Accessible documentation.** Screenshots and screencasts are regenerated from kittest flows, and each carries
  alternative text from the flow step ("Strip editor of -X1031 with terminals 5–10 selected"); the built HTML passes
  an automated WCAG 2.1 AA scan without critical violations (NFR-A11Y-06, 1.0).
- **Tutorials** (Help ▸ Tutorials) open the tutorial page and a fresh copy of its sample project; at MVP tutorials
  1, 2, 4 and 6, at 1.0 also 3 and 5 (NFR-DOC-03).
- **About** shows the version, render backend, privacy state, a button to `PRIVACY.md`, third-party notices, the
  engineering-verification disclaimer and the support status computed offline from the planned support end
  (FR-UI-038; [18](18-quality-security-release.md) §7.5; NFR-COMP-04; specified for 1.0; not built: the dialog shows the
  pre-release support line only). Help ▸ Verify derived data runs
  `ew verify-derived` as a job (ARCH §6.8).

Help ▸ Tutorials
opens the tutorials page and one fresh copy of the sample project for every tutorial (each tutorial's own start is
not built); its items are the tutorial pages of the guide's table of contents `SUMMARY.md`, read
when the program is built; with no installed guide it shows a "user guide is not installed" banner over the copy.
F1 resolves a topic for every `tool.` action, every dialog (an exhaustive match), every pane and navigator, through
the three window reference pages `reference/window/tools.md`, `dialogs.md` and `panels.md`, and a rule's `docs`
reference `reference/rules/<id>`. In a Dutch window F1 opens `nl/<page>` when the Dutch guide holds that page, and the
English page otherwise. In the symbol editor F1 over the outline or the drawing opens the section of what is selected
there (a connection point's, a shape's), and the outline's or the tool's section with nothing selected.

The mapping lives in `ew_viewmodel::help`. Every action a menu or the bar shows has a
topic: its own section, else the section of what it acts on (its family: `cable.` the cables', `view.` the page
view's); only opening, saving, undo and the clipboard open the project view's. A thing on the page is classified by
its record and, for a device, its natures (a terminal, a PLC card or station, a valve terminal, a valve, a bus device,
else a device; an external interruption point is a supply point; a wire marker on cores only is a cable's mark; the
one selected wire under the pointer is its route); its sections are on `reference/window/things.md`, the frame's
where the pointer is on the sheet outside the frame's cells. The window names what is under the pointer each frame
(`ew_ui::help`; of the frame's offers the smallest rectangle wins), hands it to the shell, and F1 asks for that
topic; without one the shell resolves the focus, the selection, the tool and the screen. A row of Properties opens
`reference/properties.md` at the key of the text that explains it (`field-help-<kind>-<path>`, else the shared
`field-help-word-<member>`, else its kind's section); `cargo xtask docs-gen` writes that page from the same texts
(`ew_viewmodel::help::property_sections`). A dialog's field has no anchor of its own: F1 there opens the dialog's
section, and a form the section of its action (Check level…, Cable…).

---

## 14. Performance as the user sees it

| Interaction | Budget | Source | Feedback when the work is longer |
|---|---|---|---|
| Start-up to the start page | ≤ 1.5 s | NFR-PERF-02 | — |
| Open to first interactive page | S ≤ 0.5 s, M ≤ 2 s, L ≤ 5 s, XL ≤ 20 s | NFR-PERF-03 | progress on the start page; then "Warming up" while editing works |
| Frame while placing, moving, wiring, panning | ≤ 16 ms p99 (apply ≤ 2, derived ≤ 5, render ≤ 5) | NFR-PERF-04 | — |
| Commit of a move or wire | p99 ≤ 5 ms (S/M) | NFR-PERF-05 | — |
| Geometric ghost / semantic ghost / xref hover preview | every frame / after 80 ms idle / after 300 ms | 06 §4.1, §6 | stale ghosts are dropped |
| Profile change, visible views updated | S/M ≤ 30 ms | NFR-PERF-07 | — |
| Bulk command | progress ≤ 200 ms; preview S ≤ 0.5 s … XL ≤ 10 s | NFR-PERF-08 | jobs panel, preview tab |
| Full check run | S/M ≤ 2 s, XL ≤ 60 s | NFR-PERF-09 | "as of rev n" |
| Navigator at 100k rows | scroll within the frame; filter ≤ 100 ms per keystroke; find ≤ 1 s at XL | NFR-PERF-15 | previous results stay until new ones land |
| PDF of the sample project | ≤ 3 s | NFR-PERF-11 | jobs panel |
| Pan and zoom | ≥ 60 fps dense A3 on GPU; ≥ 30 fps A4 on CPU | NFR-PERF-06 | backend in the status bar |
| Any UI-thread operation | ≤ 100 ms; cancel ≤ 500 ms | NFR-PERF-19 | watchdog fails tests |

UI techniques: view-models only for visible panels; row virtualisation; `Arc` row tables shared between frames;
lazy tooltips; thumbnails cached by display-list hash (ARCH §8.2); one 3D picking ray per frame; repaint on demand.
`tracing` spans per frame phase are recorded in kittest flows, which assert the 16 ms budget (AC-28).

---

## 15. Algorithms

### 15.1 Palette ranking

```text
score(q, s): best subsequence alignment of q in s (dynamic programming, |q| ≤ 64)
  +32 match at s[0]; +16 match at a word start (after space, '-', '.', '_', digit/letter or case boundary)
  +8 consecutive match; +1 same case; −1 per skipped character (at most −16 per gap); no alignment → excluded
rank(a) = max(score(q, label), 0.9·score(q, alias_i), 0.8·score(q, id))
          + min(20, 5 · uses of a in the last 50 invocations)
          + 10 if a's predicate holds for the current selection
order: rank desc, then label by ICU collation, then id    (deterministic)
```

About 1,000 actions score in well under 1 ms. Object (`@`) results come from the find index on a job and stream in;
they rank exact tag matches first, then by score.

### 15.2 Navigator flattening and filtering

1. A query keyed on (navigator kind, source revisions) builds an `Arc<NodeTable>` with labels, badges and search
   keys: case-folded NFC tokens split at word and digit boundaries, plus the natural-sort key of `ew-mltext`.
2. The visible list `Vec<(depth, NodeId)>` is a depth-first walk over expanded nodes, rebuilt in O(visible rows) on
   a revision change or expand/collapse; the widget paints only the rows in view.
3. A node matches when every query token prefixes one of its tokens or a facet matches. Up to 20,000 nodes the
   filter runs on the UI thread, above that as a job, with the previous result shown until the new one lands.

### 15.3 Selection synchronisation

```text
on selection change (origin O, set S, generation g):
  if g == last_applied: return                         # no feedback loops
  if O == Canvas:
      for N in visible navigators: N.reveal(N.rows_for(S))      # expand ancestors, scroll, mark; keep focus
  else:                                                   # navigator, table, messages, history
      R = representations of S on the active page
      if R ≠ ∅: canvas.select(R); if follow_selection and R off screen: pan to bbox(R), zoom unchanged
      else: canvas.chips("Not on this page", main-first list of page-cells of S's representations)
  property_grid.show(common_fields(S))
  last_applied = g
rows_for: Rep → its functions → device row; Connection → connection row; Cp → terminal or device row;
          Channel → PLC row; Finding → target rows
```

### 15.4 Clipboard table parsing

1. Read `text/plain` through arboard; split rows on CRLF or LF and cells on TAB, honouring Excel quoting (a cell in
   double quotes may contain tabs, newlines and doubled quotes). Drop one trailing empty row. Reject more than
   100,000 cells.
2. If the first row matches at least half of the visible column labels (case-folded, in any shipped UI locale),
   treat it as a header and map columns by name; otherwise map by position from the focused cell.
3. Parse each cell by column type. `Dec3`: a single comma and no dot is a decimal comma; "1,500" and "1.500" are
   ambiguous and marked; a trailing unit equal to the column unit is stripped. Enums: by code or localised name,
   case-insensitive ("dbu" → DBU). Identifiers: NFC and confusable check. `MlText`: into the current edit language.
4. Build the edit intents. If any cell is invalid or ambiguous, or rule §7.5 applies, open the preview grid;
   otherwise compile one `fields.set`.

### 15.5 Keymap conflicts and reachability

```text
contexts form a tree: global > window > { canvas > canvas.page2d > tool.*, canvas.layout3d, panel > panel.table, … }
normalise chord: modifiers in fixed order (Ctrl, Alt, Shift, Cmd); logical key for letters and digits; physical code for punctuation
for bindings b1 ≠ b2 with equal chords:
  ctx(b1) == ctx(b2)              → error "conflict"
  ctx(b1) is an ancestor of ctx(b2) → info "b2 shadows b1 in ctx(b2)"
  otherwise                       → none (never active together)
reserved chords (⌘Q, ⌘H, ⌘M, ⌘Tab, Alt+F4, Alt+Tab, Win+…)              → error
Ctrl+Alt+<key>                                                  → error (AltGr on Windows layouts)
for layout in {QWERTY-US, QWERTZ-DE, AZERTY-FR}:       # reachability: specified; not built
  every action bound in the preset has ≥ 1 binding typable on that layout without AltGr, else error
```

The rules run over the effective bindings in `ew_viewmodel::keymap`: conflicts, shadowing, Ctrl+Alt and
the reserved chords. The reachability rule (a needs-Shift error or info per layout) is specified; not built. By
construction zoom to
100 % binds `Ctrl+phys:Digit0`, so it types on AZERTY-FR, and zoom in's Ctrl+Plus and Ctrl+Equals each cover the other
where one needs Shift.

### 15.6 Colour-vision and contrast test

1. Token lint per theme: text pairs ≥ 4.5:1 (≥ 7:1 in high contrast), overlay slots ≥ 3:1 against `sheet`.
2. Render a schematic page with two potentials highlighted, plus the message panel, and a page with four
   potentials in highlight slots 1–4, through `ew-render-cpu` at 150 dpi; simulate protanopia,
   deuteranopia and tritanopia (Machado et al. 2009, severity 1.0, linear RGB).
3. Assert CIEDE2000 ΔE ≥ 10 between any two highlight slots shown together (proposed threshold, confirmed during
   the spine), and structurally assert distinct pattern IDs in the highlight layer, so highlights stay
   distinguishable even if a colour pair fails (FR-UI-025).

---

## 16. Outputs

| Output | Format | Location | Consumer |
|---|---|---|---|
| Model changes | commands through `Session` | journal, undo stack | model, `ew diff` |
| User settings, keymaps, workspaces | TOML, JSON, versioned | OS configuration directory; export files | EW, other installations |
| Per-project view state | JSON | `.ew/u-<hash>/view.json` inside the project (git-ignored, per user) | EW |
| Basic project | EW project directory | user-chosen | new projects (FR-PRJ-009) |
| Usability reports | Markdown, anonymised | `docs/ux/` (proposed) | team, SC-9 evidence |
| Accessibility Conformance Report | document | documentation, 1.0 | public-sector buyers (NFR-COMP-06) |
| Documentation screenshots and screencasts | PNG and MP4 from kittest flows | docs build | users (G13) |
| Keymap sheets, command catalogue | generated HTML | docs build | users (NFR-DOC-06) |

---

## 17. Edge cases and pitfalls

1. **Selected entity vanishes** through undo or an external change: it is dropped from the selection with a status
   message; an open property grid shows "object no longer exists" with Back.
2. **Edit during an open preview:** the preview gets a staleness badge and recomputes on Accept (§7.6); it never
   commits a result computed on an older revision without the rebase check.
3. **Undo to a page that no longer exists** (removed by an external change): the page navigator opens with the
   affected objects selected.
4. **Long identifiers and German strings** in narrow panels are ellipsised in the middle ("=P01+J-E…-102G1"); the
   full text is in the tooltip and the accessible name. The pseudo-locale catches truncation in CI.
5. **Mixed-kind multi-selection** (terminals and wires) shows only fields common to all kinds, for example function
   text; a field that exists on some of them is not shown as `‹varies›`. The property grid builds its sections by record kind, and a mixed selection shows only the sections
   every selected entity reaches: a representation reaches its representation, its functions and their devices, and a
   function reaches its device. The targets of a section are deduplicated, so two representations of one device edit
   the device once. A representation and a connection share no section, and the grid says so instead of guessing a common field; "Set all" on a `‹varies›` list reads the first target once more.
6. **Focus stealing:** a job finishing, an external change or a toast never takes focus or switches the page.
7. **Mixed-DPI monitors:** moving a viewport from 100 % to 200 % re-lays out on the new scale factor; canvas zoom is
   kept in paper units, so the page keeps its physical size. **The UI scale does not scale the page**: canvas 100 % is one paper millimetre as 96/25.4 physical pixels times the operating system's scale factor,
   so the UI scale of §10.4 enlarges the chrome only, and a user who wants a larger page zooms. The display resolution is
   `zoom × 96 × ppp / ui_scale` (`ppp` is egui's pixels per point), for the GPU and the CPU canvas
   alike.
8. **RDP and the CPU renderer:** animations switch off and repaint is strictly on demand; the 3D tab explains why it
   is unavailable when no adapter exists (NFR-PLAT-03).
9. **Read-only projects** (lock held, released revision, untrusted content blocked): edit actions stay visible but
   disabled with the reason, so users learn why instead of searching for a missing menu item.
10. **Generated pages:** a double-click on a row of the wire list 907 opens the source wire's page (FR-REP-042)
    instead of trying to edit.
11. **Clipboard with confusables:** pasting "-1O2F1" (letter O) or a Cyrillic letter into a tag column marks the
    cell and names the code point before anything commits (FR-I18N-011).
12. **Keymap file from a newer version:** unknown action IDs are kept and listed as unused, not deleted.
13. **Two windows:** shortcuts route to the focused window only; selection and highlights are shared.
14. **Toasts and time limits:** a toast that disappears keeps its action in the notifications log (§8.2).
15. **Pending findings** ([06](06-schematic-editor.md) §3.10): the status bar shows them separately ("+3 pending"),
    so the error count does not jump while drawing; safety findings such as EW-POT-013 are never pending.

---

## 18. The reference project in the UI

### 18.1 Walkthrough

No walkthrough on a reference project is kept: the sample project (`fixtures/sample/`) opens from the start page,
and each behaviour is specified in its own section of this document.

### 18.2 Defects prevented at the interface

| Defect | UI mechanism |
|---|---|
| D-01 stale -4W1 | tags are shown as derived values; no tag text is editable on the canvas; renumbering runs through the preview |
| D-02 T51 vs T55 on the layout | "show all representations"; the unplaced-devices list; layout labels shown as derived |
| D-03 mixed schemes, "+JE2416" | frozen tags carry a lock and the rule value; the structure picker accepts nodes only and resolves aliases |
| D-05, D-06 revision and description drift | title-block cells show their binding; a page description override carries a marker and "reset to derived" |
| D-08 free-text wire rule | wire-property provenance in grid and tables; the cover tables come from the profile shown in the wizard |
| D-10 overview 5 vs 4 valves | generated overview pages are read-only; dropping a valve on 202 is refused with the reason |
| D-11, D-12 terminal designations | the strip editor ghost shows prevent-rule violations before commit; inline errors on designation cells |
| D-13 N switching | EW-POT-013 is an immediate, never-pending error with a canvas icon and a rule page |
| D-15 TBD address | `AddrSpec` editor marks Tbd "open item, blocks release" |
| D-17 core colour "BR" | the colour editor lists only IEC 60757 and profile codes and suggests BN |
| D-18 typos | previewed find and replace; typed `Dec3` editors that cannot store "50hz" |
| D-24 five IP names | the potential navigator shows one canonical name with aliases |
| D-34 author = checker | the release dialog names the rule and refuses; the waiver and revision dialogs show the user name |
| D-39 IP ordering | assisted: Page ▸ Arrange interruption points and the IP navigator context menu sort a page's IP stack (FR-XRF-013, MVP Should) |

---

## 19. Usability test plan

This is the protocol of SC-9 and NFR-UX-01 … NFR-UX-06.

### 19.1 Purpose

The tests measure whether EPLAN and QET users can do the core work without help, and they decide open design
choices: autoconnect defaults on placement and move (ADR-0007), variant letters A–H, pending findings, the
EPLAN-like bindings (FR-UI-014), the glossary (NFR-DOC-05) and the dark-sheet preference.

### 19.2 Rounds

| Round | When | Participants | Build | Tasks |
|---|---|---|---|---|
| R0 formative | as soon as the R0 viewer build and the place and wire tools exist | ≥ 2 (the pilot user and a colleague) | development build | T1, T2, T8; think-aloud only (NFR-UX-09) |
| R1 summative | before M2 exit | ≥ 2 EPLAN users, ≥ 1 QET user (PQ-27) | M2 release candidate | T1–T8; T6 for QET users |
| R2 summative | before 1.0 | ≥ 5: ≥ 2 EPLAN, ≥ 2 QET, ≥ 1 PLC programmer (P3); a wireman (P2) for T11 | 1.0 release candidate | T1–T11 |

**Screening.** At least one year of production work in EPLAN Electric P8 or QET; at least one participant per
summative round on a QWERTZ or AZERTY keyboard (NFR-I18N-06); at R2 one session on a GPU-less RDP host
(NFR-PLAT-03). No health data is collected; colour-vision deficiency may be self-reported and is not stored with
any identity.

### 19.3 Setup

- **Machine.** A BM-class Windows 11 laptop at 1920 × 1080 with an external mouse and the participant's keyboard
  layout ([02c](02c-nonfunctional-requirements.md) §1.4), offline.
- **Fixture GP-UT.** The sample project without the 24 V breaker -102F1 and its two wires (the prelude of the
  persona task `brief-ut-t1-place`), plus a basic project built with the shipped defaults (the pilot's own basic
  project replaces it in pilot sessions, which keeps its logo and notice out of the repository). Each session starts
  from a fresh copy.
- **Session.** 90 minutes: a moderator and a note-taker, think-aloud, screen and audio recording with written
  consent. In-app help (F1, tooltips, glossary) is allowed and counts as unassisted; the moderator intervenes only
  after 2 minutes without progress or on request, and that counts as an assist. T6 runs without documentation
  (NFR-UX-04).
- **Local session log.** The usability build writes the palette queries, commands and errors to a local file,
  with consent. Nothing is sent anywhere (NFR-PRIV-01); the log is deleted with the recordings.

### 19.4 Tasks

| ID | Instruction (paraphrased) | Persona, journey | Success criterion (checked on the saved project) | Limit | Round |
|---|---|---|---|---|---|
| T1 Place | "Add the 16 A miniature circuit breaker A9F78116 to folio 102 between the PSU and the load distribution." | P1, J1 | a device with part A9F78116 on 102, main representation between -102G1 (102-B6) and -102F2 (102-E7), tag derived by the numbering rule: -102F1, because the next free counter fills gaps ([03](03-domain-model.md) §8.6), not frozen | 5 min | R0–R2 |
| T2 Wire | "Connect its input to both + outputs of the PSU and its output to both 24 V inputs of -102F2." | P1, J1 | connections -102G1:+#1 and +#2 → new MCB:1 and new MCB:2 → -102F2:24VDC#1 and #2 exist (the W20, W21, W24, W25 topology); no unconnected-CP finding on them | 6 min | R0–R2 |
| T3 Terminal strip | "Insert a terminal between -X1031:4 and :5, then set the function text of all spare terminals to 'Reserve'." | P1, P2 | terminal inserted with a unique designation and no prevent-rule violation; the function texts of the 12 spare terminals read "Reserve"; the fill is one undo step | 8 min | R1, R2 |
| T4 Renumber | "Folio 106 should become 104. Check what changes before you apply it." | P1, J5 | folio 104 exists; -104W1 and -104W2; the participant opened the Tags, Xrefs or Pages tab before Accept | 5 min | R1, R2 |
| T5 Export PDF | "Export the drawing set and show where 24VDC-1 on folio 102 continues." | P1, P4 | PDF written; the participant follows a link to 103-A1 | 5 min | R1, R2 |
| T6 Timed circuit | QET users: "From the company basic project, draw the folio 102 circuit and export a PDF." | P1, J1 | -102G1, -102F1, -102F2 with parts and wires; PDF within 30 min (NFR-UX-04) | 30 min | R1, R2 |
| T7 Finding | "The check reports a safety problem on folio 101. Find it, explain it and record a waiver." | P4, J5 | jumped to T55; read the rule page or detail; waiver with a reason | 6 min | R1, R2 |
| T8 Navigation | "On folio 101, follow L-1 to its continuation and come back." | P1 | xref jump and Back used | 2 min | R0–R2 |
| T9 Layout | "Place F55 on the DIN rail of the mounting plate." | P1, J3 | EW-LAY-001 no longer lists F55 | 6 min | R2 |
| T10 PLC | "Bind channel 1 of -201U3 to valve -201U3-Q1 with process tag TA05-V-15, then export the TIA tag table." | P3, J4 | binding exists; symbol TA05_V_15 in the export | 8 min | R2 |
| T11 Shop floor | "In the production package, find wire W29 and both of its ends." | P2, J3 | wire found, both ends named ([13](13-manufacturing-outputs.md)) | 5 min | R2 |

The **core tasks** of NFR-UX-01 are T1–T5. With three R1 participants that is 15 attempts, so ≥ 12 unassisted
completions are needed; at R2, ≥ 80 % of the core attempts and of the layout and PLC tasks (T9, T10).

### 19.5 Metrics and thresholds

| Metric | Target | Requirement |
|---|---|---|
| Unassisted completion of core tasks | ≥ 80 % (R1, R2) | NFR-UX-01, SC-9 |
| System Usability Scale after the session | ≥ 68 (R1), ≥ 75 (R2) | NFR-UX-02 |
| T6 time | ≤ 30 min | NFR-UX-04 |
| Palette queries without a result | each one becomes an alias or a glossary entry | NFR-UX-03 |
| First keys pressed for the 15 most used actions (EPLAN users) | input for the EPLAN-like preset; two users confirm it at R2 | FR-UI-014 |
| Time on task, errors, undo count, assists | reported per task | — |

### 19.6 Severity, fixes and reporting

- **Severity** uses Nielsen's 0–4 scale (0 not a problem, 1 cosmetic, 2 minor, 3 major, 4 catastrophe). Severe
  means 3 or 4: the task cannot be completed, an output is wrong, data is lost, or the participant misreads a
  derived value (for example believes a frozen tag was renamed).
- **Fixes.** Severe findings are fixed before the milestone exits (NFR-UX-01). Each fix gets a kittest flow
  reproducing the scenario and is re-tested with at least one participant in the next round.
- **Report.** An anonymised report per round (participant codes U1 …, no names or employers) goes to `docs/ux/`
  (proposed location). Decisions are recorded in ADR-0007 and in the affected design documents.

### 19.7 Pilot log

The pilot user logs hours per project and activity (schematic, reports, checks, export) for three consecutive new
production projects, against the QET baseline timed in QET. The pass conditions: no
hand-maintained report page and no slower than the baseline (NFR-UX-05, SC-3); a reference-size QET migration,
including its migration report, resolved in ≤ 1 hour (NFR-UX-06).

