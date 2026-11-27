# User experience and user interface: navigators, properties, tables and history

A chapter of [User experience and user interface](16-ux-ui.md), whose table says where each section lives.

---

## 5. Navigators

### 5.1 Shared behaviour

- **Modes:** tree, list and table (FR-UI-008), switchable per instance; the page navigator adds a thumbnail mode
  from the cached page thumbnails (FR-UI-005, §14).
- **Scale:** virtualised to 100,000 rows; scrolling stays inside the frame budget (FR-UI-007, NFR-PERF-15).
- **Sorting:** natural, by the specified order of `ew-mltext` (X2 before X10; FR-I18N-010); folios in page order.
- **Badges** are text plus icon: unplaced, frozen, generated, Tbd, spare, finding count ("E 2").
- **Keyboard:** arrows; Left and Right collapse and expand; Enter opens the main representation; Space toggles
  selection; F2 edits an authored label; Delete runs the domain delete command, with a preview when it cascades;
  typing jumps to the next row starting with the typed prefix (reset after 1 s).
- **Accessible names:** `"<label>, <kind>, <badges>, level n, m of k"`, for example "-X1031, terminal strip, 12 spare, level 1, 2 of 3". In the Pages navigator (§5.2) the level, position and set size are AccessKit properties of the tree item, not words of its name.

### 5.2 Catalogue

| Navigator | Tree levels | Table-mode columns, badges | Drag source to | Release | Domain doc |
|---|---|---|---|---|---|
| Pages | structure (=, +) → section → page; flat and section views | folio, title, type, frame, change status, generated badge | reorder (renumber preview) | MVP | [06](06-schematic-editor.md), [09](09-reports-forms-output.md) |
| Devices | structure → device → functions (main or auxiliary, placed or unplaced) → representations; parts | DT, kind, function text, part, main location, findings | canvas | MVP | 06, [07](07-symbols-parts-macros.md) |
| Connections | potential → connection, or flat | wire ID, from, to, potential, csa, colour code, source, number, page | — | MVP (FR-UI-005) | [08](08-connections-terminals-cables.md) |
| Terminal strips | strip → terminal → clamps | designation, function text, level, part, potential, sides, bridge, state | strip editor (FR-TRM-013) | MVP | 08 |
| Cables | cable → conductors | colour code, csa, assignment, potential | edit-cable table (FR-CBL-016) | MVP | 08 |
| PLC | configuration project → station → rack → module → channel; bus ports | the PLC grid columns (FR-PLC-021) | coil or channel symbol | MVP (FR-PLC-020) | [10](10-plc-and-bus.md) |
| Potentials | potential → signals → nets → CPs | canonical name, aliases, type, voltage, class, colour, size, pages, members | — | MVP | 08 |
| Interruption points | name → points with direction and star source → partner cell | name, page-cell, direction, partner, state | — | MVP | 06 |
| Structure identifiers | aspect → node | code, description, aliases, usage count | — | MVP | [03](03-domain-model.md) §5 |
| Messages | severity → rule → finding | §8.4 | — | MVP | [15](15-checks-revisions-collaboration.md) |
| Parts in use | manufacturer → part → devices | quantity, library status (current, update available, local fork) | — | MVP Should | 07 |
| Unplaced devices | location → device | part, dimensions | rail or surface | MVP | [12](12-panel-layout.md) |
| Report blocks | section → block | kind, folio, filter, pages | — | MVP | 09 |
| Bus networks, fluid, layout, comments | per FR-UI-006 | — | — | 1.0 | 10, [11](11-fluid-power.md), 12, 15 |

**The Pages navigator.** The `pages` pane is a navigator, built from `Session::page_rows` once per revision. **Views:** flat (one level), by section (the default: a node per section in section order, front
matter first, then its pages) or by structure (function nodes, location nodes inside them, section nodes inside those; a
page without a function or location sits under a `(none)` node). **Sort:** by folio (page order, the default) or by
type (the page type's declaration order, then page order). **Filter:** a text field; a row matches when the typed text,
case-folded, is a substring of its folio or its title, and the parents of a match stay shown and expand; the facets of
§5.3 are not built. **Rows:** a page row reads `<folio> <title>` with the badges `generated`, `QET grid` and `current`
(the focused tab's page); every node starts expanded. **Node keys** are path strings (`s:<n>`, `s:front`, `f:<text>`,
`f:<text>/l:<text>/s:<n>`, `f:-`, `l:-`, and `<parent key>/p:<page uuid>` for a page) because a location or a section
repeats under different parents, so a key is unique in every view. **Keys:** Up and Down move the row cursor, Left
collapses (or moves to the parent), Right expands, Home and End go to the first and last row, Enter opens the page in
the focused tab as a jump, Ctrl+Enter in a new tab; a click opens, a middle click opens in a new tab. A page opened
from the list takes the keyboard focus, so a tool's key works on it at once. Rows are drawn
with row virtualisation. **Accessibility** (§10.1): the list is a `Tree`; a page row is a `TreeItem` named `<folio>
<title>, page, <badges>` and a node a `TreeItem` named `<label>, <section|function|location>`; the level, the position
in the set, the set size and the expanded state are AccessKit's own properties, which a screen reader speaks in its own
way (they carry the "level n, m of k" of §5.1 instead of words in the name). Every view lists the page records and the
generated pages (FR-PAG-024's viewing part). **Not built:** multi-selection and drag to reorder with the renumber
preview, type-ahead, the thumbnail mode and the facet filter. The row cursor stays where the user put it after a jump
made elsewhere (§4.2), and the visible rows are rebuilt from the page rows on every call instead of being cached per
revision and navigator state.

**Eleven navigators on one framework.**
Pages, Devices, Connections, Terminal strips, Cables, PLC, Potentials, Interruption points, Structure
identifiers, Parts in use and Report blocks are shown from View ▸ Navigators and read their rows from
`Session::navigator_texts`, the last nine through `ew_reports::project_rows`, so each equals its stored report block. Every heading, label and cell leaves the session as a text, which the window renders once in its language (a column of a list kind under the list's heading, a list's own words in the list's words); a table sorts and filters by the words it shows, the command line and the round-trip export by its English. **Pages** keeps its actions, keys and lines; a click marks and a double click or Enter opens. **Devices:** function `=` › location `+` › devices › child devices by position, then
tag › functions, labelled by their FnDef key (a function has no text of its own) with `main` or `aux` ›
representations `<folio>-<cell>`; -201U3 sits at level 4, under -201U1. **Connections:** the wire list's rows, under
potentials, with the bridges under a group "Bridges"; a connection's drawable is the cell of its first end that is a
CP drawn on the page; an unnumbered wire reads `<from> <cp> → <to> <cp>`; every wire and core connection appears at
least once, shared only by junction arms. **Structure identifiers:** aspect groups in the Function and Location roles;
aliases are filter tokens. **PLC:** a channel's label is its address, else its number; it opens its representation's
page with nothing selected. **Report blocks:** an embedded block shows its host page as first folio, a page count of 0
and no open target; a block's section is that of its first page. **Not built:** Unplaced devices, the library status of parts, finding badges and `has:finding`,
thumbnails, persisted navigator state, Delete and F2 in trees.

Symbol, part and macro **browsers** are library views, specified in [07](07-symbols-parts-macros.md) §5; they
dock like navigators.

### 5.3 Filter syntax

The filter bar matches free text against row tokens (case-folded, NFC, prefix match) and accepts facets with
autocompletion: `page:103`, `potential:24VDC-1`, `state:spare`, `kind:terminal`, `part:2904602`, `has:finding`,
`unplaced`, `frozen`. Matches keep their ancestors, which expand automatically. A filter answers within 100 ms
per keystroke at XL (NFR-PERF-15; §15.2). Filters are saved per navigator instance in the per-user view state
`.ew/u-<hash>/view.json` (§2.5).

A row matches when every query token is a prefix
of one of its tokens, in any order (so `-12k4` also matches -49K12), or when the whole case-folded query is a substring
of its label; a typed number with points, a wire's number such as `1.1`, must stand in one cell as typed, not right
after a letter or a digit; the facets are the closed list `page:`, `potential:`, `state:`, `kind:`, `part:` and the
flags `unplaced` and `frozen` (`has:finding` is not built); always on the UI thread. Filters are not saved.

### 5.4 Selection synchronisation

Selection is synchronised both ways (FR-UI-007; algorithm in §15.3):
- **Canvas → navigators.** Visible navigators reveal and mark the corresponding rows; they do not take focus.
- **Navigator → canvas.** Representations on the active page are selected there. If none is on the active page,
  the page is **not** switched. For -201U1 selected while folio 102 is shown, the canvas shows a chip line "Not on
  this page: 201-E1, 3-B2, 106-G3, 902", and Enter opens the page of the main representation (lowest page order,
  row, column, ID; [03](03-domain-model.md) §8.2). A device drawn by no symbol of its own is drawn where its members
  are: a cable by its cores, a strip by its terminals, so a Messages row of a cable opens the page of its cores.

Marking rows selects the drawables on the shown
page, else the records, and never switches the page; the chip line above the canvas area lists at most 12 places (main
first) as buttons that read what they do, `Go to page <folio>, <cell>`, with Put it here beside them on a bus-topology
page that does not draw the device yet, and a selection or page change clears it. Every open navigator follows a
foreign selection once (the deepest nodes per record, each row that lists it where it stands in several, as a wire
under its potential and again under a pin; ancestors expanded, the cursor on the first), guarded by a
selection generation; a page switch puts the Pages cursor on the current page.

---

## 6. Property grid with provenance

### 6.1 Layout

- **Header:** kind icon, derived tag (full designation in its tooltip), function text, main location as page-cell
  ("102-B6" for -102G1), finding chips.
- **Categories:** Identity, Part, Electrical, Connection points, Texts, Structure, Placement, PLC, Manufacturing,
  user properties, Extra (unknown fields, read-only). They come from `ew-schema` metadata (type, unit, enum options,
  Fluent label key; FR-UI-009). Collapse states persist per entity kind. A record's own bookkeeping, the members
  whose name starts with `$` (its kind `$k`, its schema version `$v`), is no row.

The **Properties** pane (View ▸ Properties; Edit ▸ Properties of selection moves the focus to it) shows the selection, or the page on show when nothing is selected or when the selection was made on another page (the selection survives page switches, §4.6; the pane shows it again on returning to its page). The header carries the title (a record's tag, wire number, folio and title or name; for several records `<n> <kinds>`), the device's function text, the main location as folio and cell, and the finding chips of the drafting scheme on the shown records (rule, severity, message). Below it, one **section per record kind** (a representation reaches its representation, its functions and their devices), each with collapsible categories by the specification's table: Identity, Part, Electrical, Connection points, Texts, Structure, Placement, Terminal, Cable, PLC, Assembly, Other and Extra; Manufacturing and user properties have no members. Labels are in the UI language (`ew_viewmodel::strings::field_label`: the field's own `field-<kind>-<path>` text, else its member's `field-word-<member>` text, else the words of the choice or the column of that name, one word under one key, else the member humanised). Collapse states persist per kind and category. Rows are named AccessKit groups (label, then value, chip, read-only reason and error as the description), and a long value is ellipsised in the middle with the whole text in its tooltip.

A click on a category's header folds that section's category, whichever row has the cursor; a focused header folds on Enter and Space. The tooltip of a row's label explains the row (what it is, what changing it does, where a derived value comes from): one text per field, `field-help-<kind>-<path>`, else the member's `field-help-word-<member>`, in both languages, and a test walks every row the sample's records show. A wire and an interruption point reach the definitions of their potential, so their sheet shows a Potential definition section (name, type, voltage, supply, circuit class, wire properties), edited in place with undo; an interruption point's function shows its Potential row as a wire does.

Non-settable page-item/structure-box rows give specific reasons: changing a variant or marker group requires delete/place; geometry uses the canvas routes. The PageItem/StructureBox lookup kinds exist, so a selected text, graphic, box or marker shows populated narrowed Properties.

### 6.2 Rows and provenance

A row shows its provenance chip (one word: "part", "page", "derived" …) only when the value is not authored, and has no edit button (the value is the editor); internal fields move to Advanced ([16a](16a-design-language.md) §6.8).

A row shows its label, value, unit, a provenance chip and its actions:

```text
Cross-section  0.75 mm²   f derived · potential 24VDC-1 · size profile '0.75/1.5' · class control  [Override] [Why?]
Colour         DBU        f derived · potential 24VDC-1 · colour profile 'DBU/WH 24 V'
Cross-section  1.5 mm²    <> override (CDP) · derived would be 0.75 mm²                            [Reset to derived]
Tag            S51        lock frozen (sign hidden) · rule gives -101S1                            [Unfreeze…]
Address        Q0.1       f derived · module -201U3 start Q0 (auto, slot 1) + channel 1 (533361: channel n → bit n)
Length         810 mm     pin committed rev 212 · stale: re-route pending                          [Re-route…]
Part           2904602    book library 'ew-parts' v3 · update available                            [Update…]
```

| Provenance | Value style | Chip text | Actions | Domain kind |
|---|---|---|---|---|
| Authored | normal | none | edit, clear | A |
| Inherited | italic, secondary text | "from colour profile 'DBU/WH 24 V'", "from company scheme" | override, go to source | A (scheme) |
| Derived | italic, secondary text, `f` icon | `derived · <rule or source>` | override or freeze where allowed, Why? | D |
| Overridden | normal, override marker | "override · derived would be …" | reset to derived; "move override to profile" when equal | A over D |
| Frozen | normal, lock icon | "frozen · rule gives …" | unfreeze (previewed) | F |
| Committed | normal, pin icon | "routed rev 212"; "stale" with hatched background when the input hash changed | re-route (previewed) | C |
| Library | normal, book icon | "library … vN · update available" | update from library (previewed) | L |
| Async | normal, clock icon | "as of rev n" | re-run | Class II |
| Generated | read-only | "generated by block 'Wire list'" | go to block, go to source row (FR-REP-042) | D |
| Varies | `‹varies›` | "3 values" (listed on hover) | set all | — |

The chip names the profile that supplied the value: the cross-section from the size profile, the colour from the colour
profile.

The chips are a glyph and words, never colour alone. The `Committed` and `Async` rows and the "Why?" action are not built (§2.2); a row's actions are the ones its route allows (edit, clear, override, reset, freeze, unfreeze, set all), and a read-only row has none.

A derived wire property's chip names the level of the chain it comes from in one word: "profile" (a colour or size profile), "potential" (a potential definition), "cable" (a core), "default" (the project's or the built-in default, which says that the project has no profile that sets it). A row whose source is a row of another record or of the project offers **Go to source** in its menu: the potential definition's row in the same sheet, the cable core's or the overriding connection's sheet, or the project's colour profile, size profile, wire default or description, with the cursor on that row. The Potential row of a potential that no definition names offers **Define potential…** at the wire's or the interruption point's point, in its menu and on Enter, F2 or a click on its value. A device's connection point row names its point and offers **Mark not used** or **Mark used** (`cp.unused`, the point's `cp_overrides[].unused`, one undo step; its EW-POT-009 finding drops to info); the page's right-click menu over a connection point offers the same, naming the point ("Mark -102F2:13 not used"). The page draws no mark for an unused point (doc 06 defines none).

Every state is distinguished by icon and text, not by colour (FR-UI-025).

### 6.3 Editing semantics

- **Commit and cancel.** Enter or focus loss commits one `fields.set`; keystrokes in one field coalesce into one
  undo step (FR-UI-009). Escape reverts the field.
- **Validation.** A refused commit (prevent rule, structural invariant) leaves the model unchanged, keeps the typed
  text in an error state and shows the reason and rule ID under the field.
- **Multi-selection.** The grid shows the fields common to the selection, `‹varies›` where values differ; an edit
  sets all of them in one command (FR-DEV-031).
- **Override or freeze.** Derived properties (csa, colour, function text from a path) are overridden; derived
  identifiers (tag, wire number, address) are frozen. Both are explicit commands with a visible marker; typing into
  a derived tag offers "Freeze as 'S51'" or "Change numbering rule…" and never edits silently.
- **Read-only reasons** are stated in place: "generated", "defined at -102F2:Out1 (potential definition) · Go to",
  `project locked by <user>@<host>`, "released in V1.0; edits go to the open revision".

**Commit:** Enter, Tab or leaving the field commits; typing commits nothing, Escape reverts, and Enter without a change writes nothing, so a display form that does not parse (a part's "number (manufacturer)") is never written back. One edit is one command and one undo step (no coalescing is needed). **Routes:** a row is edited through `fields.set` on its settable path or through one `project.settings.set` member; every other row is read-only and says why: the record's identity; another command ("set on the canvas", the wizard, a named command); a `HumanOnly` command (`settings.checks`: the wizard's check-scheme step); "no command yet"; or the narrowing table (revision `author` and `checker`, which decide the four-eyes sign-off, [15](15-checks-revisions-collaboration.md) §5.4). A read-only project offers no action on any row. **Override and freeze** as above; Enter on a derived row is Override. **Multi-language text:** the field edits the source language (the project's first language, else the object's first, which is the alphabetically first key when the project names no languages); committing writes that language inside the stored object and keeps the others, and an emptied text removes only that language; Shift+Enter opens the per-language editor. **Typed editors:** enums, colours, part references and structure slots list their options (`Session::field_choices`) in a list box under the field, filtered by what is typed (Up and Down move the highlight while the field keeps the focus; Enter or a click chooses, and the field's focus loss commits nothing while the pointer is on the list); a boolean is a named checkbox; an ambiguous number ("1,500") offers its two readings. The editable values the schema types only as JSON (the catalogue file lists them) are edited as JSON text; a scale reads and is typed as `1:5`, a cable's length as `12.5 m` (or as the lists print it in a shipped language: `12,5 m`, `uit artikel`, `lengte ontbreekt`), a length anywhere in millimetres or with its unit (`35 cm`, `2 m`), with a point
or a comma, by the one reader `ew_api::output::parse_mm`, and it reads, set or unset, as the shortest millimetres with
its unit and the project's separator (`250 mm`, `7,5 mm`), and a rectangle (a legend's area) as its place and size in millimetres, `20.000, 15.000 mm · 60.000 × 40.000 mm`. **Refusals** show the command's message under the field with the typed text kept; a refusal at a target names it by its label, and pages by folio.

**Typed editors.**

| Type | Editor | Rules |
|---|---|---|
| `Dec3` with unit | text with unit suffix | accepts "0.75", "0,75", "0.75 mm2", "0,75mm²"; "1,500" and "1.500" are ambiguous and prompt; shown in the UI locale |
| `ColourCode` | combo of IEC 60757 codes plus profile company codes | each entry shows code, name and a swatch (GNYE striped); typing "BR" offers "BN brown" (D-17) |
| `Ident` (tags, designations, part numbers) | text | NFC on input; confusables named with the code point, "Cyrillic А (U+0410)" (FR-I18N-011) |
| `MlText` | single-line field in the current edit language; "…" or Shift+Enter opens a multi-language editor with one row per project language, source language first, and a multi-line area per row | neutral and language strings exclude each other, with the FR-I18N-001 prompt; the dedicated editor compensates for egui's weak multi-line editing (ADR-0014) |
| `StructureAssignment` | node picker per aspect | no free text: typing "+JE2416" resolves through the alias to +J-E-2416 and says so (D-03) |
| `AddrSpec` | Auto, Manual, Excluded, Tbd | Tbd shows "open item, blocks release" (D-15) |
| References | searchable picker | shows derived tags, stores IDs |

### 6.4 "Why this value?"

At MVP the chip shows the one-line `via` chain. At 1.0 the **Why?** action (FR-UI-011) opens a panel with the
full trail, the GUI face of `ew query --explain` (FR-EXT-005):
- for W29's cross-section, the seven levels of [03](03-domain-model.md) §9.5, the empty ones greyed and the
  winning one (profile rule "24 VDC × control") marked;
- for "102-A1" at L-1 on folio 101, the partner interruption point L-1 on folio 102, its anchor cell A1 from the
  symbol's anchor policy and the `XrefFormat` `{page}-{row}{column}` ([03](03-domain-model.md) §10).

Each line links to its source object or setting.

---

## 7. Tables, bulk edits and previews

### 7.1 Table mode

Tables are egui_table 0.10.0 views with fixed row height (for virtualisation), a frozen key column (tag, wire ID
or designation), a column chooser, natural sort and the filter bar of §5.3. A table opens with every column, unless
its kind names the columns it opens with (`opening_columns`): Pages its title and Changed in, Connections the ends,
the colour, the cross-section, the length and the treatment of each end ([13](13-manufacturing-outputs.md) §3.1). Derived columns are italic and
show their provenance on hover. Cells use the editors of §6.3. The strip editor, edit-cable table, PLC grid, wire list
and tube table are table models with domain commands for insert, delete and move (08, 10, 11).

### 7.2 Edit → command mapping

| Gesture | Command | Undo steps | Preview |
|---|---|---|---|
| Edit one cell | `fields.set` on one entity | 1; keystrokes coalesce until focus leaves | rule §7.5 |
| Edit with several rows selected, Ctrl+Enter applies to all | `fields.set` on n entities | 1 | rule §7.5 |
| Fill down (Ctrl+D) | `fields.set` with one value | 1 | rule §7.5 |
| Pattern fill (Ctrl+Shift+D) | `fields.set` with per-row values | 1 | rule §7.5 |
| Paste from Excel (Ctrl+V) | `fields.set`, plus inserts where the table allows them | 1 | rule §7.5 |
| Clear cells (Delete) | `fields.set` to empty, which removes overrides | 1 | no |
| Insert, delete, move rows | the table's domain commands (strip editor: `terminal.insert`, `edit.delete` of the terminal devices, `terminal.move`, the Terminal cell for re-designation, and `fields.set` of `strip.accessories` for accessory rows) | 1 each | ghost of prevent-rule violations (08 §9); every row gesture commits directly with a toast and an undo step (Delete of whole rows, §4.4 of doc 16) |
| Sort, filter, resize, reorder columns | none | 0 | — |
| Excel round-trip import | one `fields.set_many` ([14](14-interop.md) §8.2) | 1 | always |

### 7.3 Pattern fill

A small deterministic template language, not rhai: `{n}` row index from 1, `{n:02}` zero-padded, `{n+100}`
offset, `{n*2}` step, `{col:<column>}` another column of the row, `{prev}` the previous row's value. The dialog
previews the first five values. Examples: `Spare_{n:02}` as the symbols of the 20 spare channels in the PLC grid
(Spare_01 … Spare_20; FR-PLC-021); `{col:function_text}` to copy each valve's process tag into its comment; the
constant "Reserve" as the function text of the 12 spare terminals of -X1031.

### 7.4 Paste from Excel

Clipboard text is parsed as Excel TSV with quoting, header detection, locale-aware numbers and enum matching by
code or name (§15.4). Invalid or ambiguous cells open the preview grid with those cells marked; otherwise the paste
is one command (FR-UI-036).

### 7.5 When a table edit opens a preview

An edit commits directly, with a toast "Set function text on 12 terminals · Undo", unless one of these holds; then
the bulk preview opens (FR-UI-044):
1. it touches more than 500 entities (`Cost::Bulk`, as for large pastes in 06 §8);
2. it changes an identifier that leaves the building (tag, terminal designation, wire number, PLC address, folio)
   on more than one entity;
3. a cell failed validation or was ambiguous;
4. it comes from an external file (Excel round trip, FR-EXP-004).

For the property grid the policy is evaluated on the command's preview (one `preview_with` per commit): rule 1 by the preview's target count (`reason-bulk`, more than 500) and by the command's cost (`project.settings.set` is `Bulk` and always shows the card, `reason-bulk-command`), rule 2 by the route's identifier flag on more than one target, including a leaf below an identifier member (`reason-identifiers`); rule 3 applies to tables and rule 4 to imports. The preview is an **inline card** in the pane (Apply with Ctrl+Enter, Cancel, up to 20 changed tags, a stale badge); a stale card rebuilds the command from the current values and needs a second Apply, and refuses with "Changed meanwhile: edit again" when the edited member changed on a target. A direct commit on several records shows the toast ("Set Part on 20 devices · Undo"), which closes after 8 s measured on the window's clock (never in the transcript).

### 7.6 Bulk preview

The preview is a dockable tab, not a modal dialog: editing continues while it is open (NFR-UX-07). It serves
renumbering (FR-NUM-008), re-addressing (FR-PLC-014), imports, "update parts from library", `RouteWires`, find and
replace (FR-UI-020), multi-target quick fixes, structure rename and merge (FR-PRJ-018), derivation-profile upgrades
(FR-PRJ-011) and Excel round trips.

```text
+- Preview: Renumber pages (section 1, start 101, step 1) - as of rev 214 ---------- [Accept] [Cancel] -+
| Summary: 1 page record, 2 derived tags, 14 cross-reference texts, TOC, outline, Prev/Next of 103, 200 |
| [Summary] [Entities 1] [Tags 2] [Xrefs 14] [BOM 1] [Wire list 12] [Pages] [Findings +0 -0]             |
| Tags   -106W1 -> -104W1    -106W2 -> -104W2                                                             |
| Xrefs  folio 103: 12 texts "106-..." -> "104-..."   folio 201: "106-G3" -> "104-G3"   202: "106-G9" ...  |
| Pages  before | after of every page whose display list changed (2, 103, 104, 200, 201, 202, reports) |
+-------------------------------------------------------------------------------------------------------+
```

In the Pages list a page whose folio the command changes is named by both
folios, "106 → 104"; the others by their folio. The lists open and close in place: the dock is sized for every list
open, and past its room the lists scroll. Renumber pages' dialog says "Section dividers keep their numbers." under
Start.

Only the page record changes; the tags and xref texts are derived from it ([03](03-domain-model.md) §8.2), which is
why the preview shows them as query diffs rather than entity changes. The BOM delta is one row (pos 13, tags
-106W1, -106W2 and folio 106), the wire-list delta the 12 rows W33 … W44 whose page references or cable tag change.

- **Content** comes from `diff_model` and `diff_query` on the fork (ARCH §5.4): entity and field changes, tags,
  xref texts, BOM rows, wire-list rows, PLC addresses, affected pages, and the findings delta (new and resolved).
  The Pages tab renders candidate pages with `ew-render-cpu` on the job; the compare view overlays changed regions
  with a hatch pattern.
- **Accept** (Ctrl+Enter) commits the prepared transaction after the rebase check. If the model moved and the
  re-run changes the result, the tab shows "Preview updated: 1 more tag" and needs a second Accept.
- **Cancel** (Escape in the tab) drops the fork. Large diffs are virtualised and exportable as CSV.
- **Example.** A page renumbered (folio 106 → 104) changes its tags and cross-reference texts, and one undo restores
  the previous state (AC-18).

A find and replace does not open the dockable tab: like the property grid's (§7.5), its preview is the inline card, shown **inside the find pane** ("Preview: replace 20 results on 20 records · 0 tags, 0 cross-reference texts"); Ctrl+Enter or Apply commits the one `fields.set_many`, one undo step, and the card counts only the hits that change. No toast follows, because the toast belongs to direct commits: the results are marked `[stale]` and the pane notes "Replaced <n>; run again to refresh". A card made at an older revision is not applied: Apply runs the search again and shows a fresh card ("Preview updated: 1 fewer results") that needs a second Apply, as the Accept rule above.

### 7.7 Excel round trip

The file format and the merge are those of [14](14-interop.md) §8: the export writes the visible columns, a hidden
`id` column, a hidden sheet `_ew` (format `ew-table/1`, project ID, table key, export revision, locale, column
schema) and a hidden sheet `_ew_base` with the editable cells as exported. On re-import EW compares, per cell, the
base value, the current model value and the file value, so it detects edits made in EW since the export even in a
later session. The UI side is the preview tab of §7.6 with a per-cell diff grid: cells changed on both sides are
conflicts, defaulting to "keep EW" with "take file" per cell or per column; edits in derived columns (tag, xref,
effective colour) are refused with the hint of [14](14-interop.md) §8.2; unknown and missing rows are listed, never
deleted. Applying is one `table.apply_changes` command (FR-EXP-004).

Import table…
reads the file and opens the import card in the Preview tab (`CardOrigin::TableImport`): the counts (changed, kept,
conflicts, refused, unknown, missing) and the per-cell list. Accept applies through `preview.apply` as one
`fields.set_many`, one undo step; a stale import is refused in words and the card stays; Escape commits nothing. A
conflict keeps EW's value, as the card says beside the count: the switch "Take the file's values" and the per-cell and per-column choice are not built.

---

## 8. History, notifications and background jobs

### 8.1 History panel

```text
History                                        oldest kept: step 1 · 12 MB of 256 MB
  redo   Set colour BN on W39                       GUI        106
> now    Renumber pages (section 1)                 GUI        2, 103, 104, 201, 202
         Rebind -201U3-Q1 coil 14 to channel 0      quick fix EW-PLC-012   203
  ------ saved · generation 214 ---------------------------------------------
         External change (git a1b2c3d, 12 records)  external   103, 106
         Set function text "Reserve" on 12 terminals of -X1031   GUI   103
         Exported PDF to <file>                     not undoable
```

- **Entries** show the command label (a Fluent message with arguments), the origin and the affected pages
  (FR-UI-023). Origins: GUI, script `<name>`, MCP (1.0), plugin `<id>` (1.x), import `<file>`, quick fix
  `<rule>`, external change `<generation or commit>`, journal replay. A CLI cannot write while the GUI holds the
  lock, so CLI edits arrive as external changes.
- **Undo to here** runs the needed undos as one user action and restores the page, viewport and selection of the
  target entry. **Show changes** lists the entry's entity changes with derived tags.
- **Undo and Redo show the page the step changed**: the page, viewport and selection the step was made with when
  that page is one it changed, or no page it changed is left; else the page on show when the step
  changed it, else the first page it changed, as a jump (a quick fix or a Properties edit of page 1 made while page 5
  was on show went to page 5). The step's pages are every page it changed, a panel layout page for its
  plate, rails, ducts and placements too. A step without a kept context (an external change) moves nothing;
  where the page on show is not one the step changed, its toast offers Go to page.
- **Linear history.** A new commit after undo discards the redo entries. Edit ▸ Undo and Redo carry the entry's
  label ("Undo Renumber pages (section 1)"), and so do their tooltips and accessible names.
- **Bounds.** 500 steps or 256 MB (ARCH §5.3); the header shows the oldest step kept.
- **Non-undoable operations** appear as marker lines. EW asks for confirmation only when such an operation
  destroys or overwrites data: library writes outside the project, overwriting an existing file, purging unused
  library copies, and the import checkpoint into an empty project, as FR-UI-023, ARCH §5.3 and ADR-0008 state.

### 8.2 Notification classes

| Class | Used for | Lifetime | Example | Accessibility |
|---|---|---|---|---|
| Inline | field and cell validation, refused commits | until fixed | "Designation 3 already used on -X1031" | announced with the field |
| Toast | the result of a user or external action with a follow-up | 8 s on screen, then kept in the notifications log | "External change applied: 12 records on folios 103 and 106 (git a1b2c3d) · Undo · Show changes" (05 §16) | polite live region |
| Banner | persistent project state | while the state holds | `Read-only: locked by <user>@<host> since 09:12 · Notify when free`; "Untrusted project: 2 SQL reports and 1 rule pack are blocked · Review…"; "1 file differs from the manifest · Details" | announced once |
| Jobs panel and status bar | background work | while running, then a result entry | "PDF export 40 % · as of rev 214 · Cancel" | progress role |
| Dialog | a decision without which work cannot continue | modal | trust prompt, recovery at start-up (05 §8.3), destructive confirmation | focus moves into it |
| OS notification | long job finished while the window is unfocused | — | "PDF export finished" | OS | 

- Toasts never hold the only path to an action. The **notifications log** (status-bar entry) keeps the last 100
  notifications of the session with their actions while those are valid (FR-UI-042; NFR-A11Y-04).
- At most three toasts are visible; repeated toasts of one kind coalesce ("3 external changes applied"). EW makes
  no sounds. OS notifications are a 1.x Could, opt-in.

`notify_with(text, key, undo)` raises a
toast and logs it; at most three toasts show, a toast of a visible key replaces it, each lasts 8 s on the window's
clock, and the log holds the session's last 100 notifications, newest first, opened from the `Notifications <n>` field
(§3.6) and View ▸ Notifications. Undo stays usable while the step it undoes is the next undo step of the same project
session; closing the project removes the visible toasts and expires their Undo; Open stays usable while the exported
file was written in this session. A committing action does not remove a toast;
the toasts (strip, structure node, external reference, autoconnect, PDF export) share the key `edit`.

### 8.3 Jobs and staleness

The jobs panel lists kind, scope, base revision, progress (determinate when the job knows its total of pages or
records, otherwise a phase text), elapsed time and Cancel. A finished job leaves a result entry ("Open PDF",
"Show 3 findings") or a failure with cause and remedy.

| State | Display | Example |
|---|---|---|
| Fresh | plain value | "810 mm" |
| Computing | clock icon and phase text | "routing 3 of 5 wires" |
| Behind (Rev < current) | hatched background, "as of rev n", re-run action | "Offline check as of rev 212 · 3 edits since · Re-run" |
| Stale committed value | "≈" before the value, the source word "stale" | "≈ 735 mm · stale" (EW-LAY-011, [12](12-panel-layout.md) §8.5) |
| Failed | error icon, cause, remedy | "STEP import failed: file has no solids · Replace model" |
| Blocking release | the same, with "blocks release" | stale lengths at release (FR-REV-007) |

**Timing.** A `Cost::Bulk` command shows progress within 200 ms (NFR-PERF-08). Jobs that finish within 200 ms show
no indicator, so nothing flickers. Cancel takes effect within 500 ms (NFR-PERF-19), and a cancelled export leaves no
partial file (FR-UI-024). After opening a project the status bar shows "Warming up" while editing is already
possible (NFR-PERF-03). An export reports the revision it ran on ("exported rev 214; the model is now at rev
216"), as information, not as an error.

The PDF export is a job: one named
`std` thread of `ew-jobs` over a snapshot, with a checkpoint per page and per drawn item. The Jobs pane is a group named
"Jobs" whose running entry is a progress bar labelled `PDF export <name>` with Cancel; a finished entry shows its
result ("cancelled; no file written", a failure with its cause, or Done with Open). The file is written to a hidden
sibling `.<name>.ew-part`, synced and renamed into place, and removed on cancel, failure or panic, so no partial file
remains. Replacing a file is confirmed once, by the native save dialog's prompt or `replace: true` in a script; a
second export while one runs is refused. Headless, the export waits for its job unless `wait: false`.

*Imports read as jobs.* A drawing for File ▸ Import…, an EDZ or a parts XML, a PLC exchange file
and a report spreadsheet are read off the window's thread: what needs no session (the file read and parsed; the PLC
file merged against the configuration project's image, taken from the session first) runs as a job, listed in the Jobs
pane with Cancel, and its end runs the rest on the window's thread: the dialog, the preview card, or the new project.
The window answers meanwhile, so a report's new project replaces the open one only as that project was asked about:
where it changed while the file was read, the unsaved-changes question comes again at the end, and its Cancel keeps the
project and drops what was read, which a toast says. A cancel takes effect when the parsing ends and is said in words ("The import of … was cancelled; nothing was
imported."); a job of a project that closed meanwhile imports nothing and says so, and a read that fails says why in
the banner. A script waits for the read. Add to my parts library
takes the user's library's part of the registry first (its manufacturers, the parts it holds), parses the file against
it in the job and writes at the end, into the library as it is then: a part another write changed meanwhile takes its
version from the copy held at the write. A user library that does not exist yet is made at the end and the
file read again for it, so an EDZ that stops at the terms notice makes none.

### 8.4 Message panel

The message panel (FR-UI-021, FR-CHK-008) groups findings by severity and rule. Columns: severity (icon and word),
rule ID, message (cause), object (derived tag), page-cell, quick fixes, waiver state. The detail pane shows the
remedy text, which every rule must provide (NFR-UX-07). Enter or a double-click jumps to the object; F8 and
Shift+F8 walk the findings; F1 opens the rule page (FR-CHK-015). A waiver's reason and author are optional, of any
length (the owner, 2026-11-12); the prompt offers the user's display name as the author (FR-CHK-009; Open
question 14). Pending findings ([06](06-schematic-editor.md) §3.10) are listed in a muted style with their own
counter. The panel has no disclaimer footer (the owner, 2026-11-12); About keeps the one line (NFR-COMP-04). The check report exports as PDF and XLSX (FR-CHK-014).

