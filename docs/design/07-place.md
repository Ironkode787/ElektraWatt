# Symbols, parts and macros: Parts, the place for parts and symbols, and the other windows of master data

A chapter of [Symbols, parts and macros](07-symbols-parts-macros.md), whose table says where each section lives.

---

## 5. UI and UX touchpoints

| Touchpoint | What it is | Where it is said |
|---|---|---|
| **Parts** | the one place where a part or a symbol is looked up, seen whole, changed and made: a screen of its own with a list, the open part's sheet, its pictures and its drawing | §5.1 to §5.10 |
| **Choosing a part for a device** (the device browser: Insert ▸ Device from part, `device.place` with `target = FromPart`; the part dialog: Properties ▸ Part ▸ Choose…) | a list to pick from, with what choosing a row does and how the part fits; it changes no part, and opens Parts for that | §5.11 |
| **Symbol browser** (`Insert ▸ Symbol`, palette `device.place`, alias `symbol.place`) | search by name, category, standard number and keyword; previews of the eight turned and mirrored forms; filtered by what the page allows | [16a](16a-layout.md) §5.6 |
| **Macro browser, Save as macro, the library manager** | the Macros and the Libraries navigator | §5.12 |
| **Company-standard wizard** | steps: logo, title-block form, colour and size profile with its stated basis, identifier set, numbering rules, structure nodes, sections, revision format, languages, report blocks; writes a basic project and offers it as the default template | [16](16-onboarding-access.md) |

### 5.1 Parts, the one place

**A part is met in one place.** Looking a part up, seeing everything the program knows of it, changing it and making
a new one are the same surface in three states, and a symbol is drawn there too. The place is called **Parts**. It
is a screen of its own (`Screen::Parts`), built as the main window is, and it shows with or without a project open.
Nothing else in the program edits a part or a symbol: no dialog, no second editor, no form of a tool.

The screen has three columns under its command bar:

- **left, the list** (360 px at 1920 × 1080): what there is, and the way to find one among thousands (§5.3);
- **middle, the sheet** of the open part: its groups one under another on one sheet, with a contents list at the
  sheet's left edge (§5.4). One sheet and not one section at a time, because viewing must show the whole part
  without a click per group, and because it reads as the data sheet a person types from;
- **right, the pictures** (500 px): how the part is drawn on a schematic and how it stands in the cabinet, drawn by
  the same code that draws a page, and under them **Ready for**, what the program's features still lack of this part
  (§5.7). The pictures stand beside the data and not inside a group, so that a number typed shows at once what it
  does.

At 2560 × 1440 the list is 420 px and shows more rows, the sheet stands in two columns of groups so that a part is
in view whole, and the pictures are larger; nothing is stretched.

**The command bar** has two tabs. **Part**: Back; New part, Like this, New symbol; Import, Export; Make my own, Keep
in my parts; Remove; and at the right end Discard and the next step, Save. **Drawing**: To the part; Select, Line,
Rectangle, Circle, More shapes, Text; Pin, Text field, Other marks; the drawing grid and Snap; Test on a page; and
the next step: Done, or for a drawing that was opened from a page, Save and place on that page (§5.6). Drawing is greyed with its reason while nothing is being drawn. No button is hidden or moves by
state: one that cannot run stands greyed and says why.

**The three states.**

- *Viewing.* A part that is the person's own shows each value in its field, without a frame until the pointer or the
  focus is on it. A part that is not his (§5.8) shows its values as text and one line under its heading that says
  why and offers the way on.
- *Changing.* There is no mode to enter. A value typed makes the part "Changed, not saved" in its heading; Save and
  Discard are then enabled. Undo and Redo step the place's own history of the open part. A field's text is read and
  checked as it is typed, and what is wrong is said under the field in words (§5.4).
- *Making.* New part… asks one question first (§5.5); the sheet then is a part's sheet like any other, "New, not
  saved".

One session model holds the open part whatever opened it (`PartSheet` of `ew-api`: the part as a value, the texts as
typed, the messages), and one model of the shell holds the place (where the person came from, the list's query, the
open sheet, the drawing).

### 5.2 Getting there, and back

Every place where a part is met opens Parts on that part, and one action does it, `parts.open` with the part, and
where a field is meant, the field:

| From | Entry |
|---|---|
| anywhere | Project ▸ Parts and Draw ▸ Place ▸ Parts on the command bar; the app menu; the start page, also with no project open; F9; the command search under Parts, My parts, New part, and under the names of what Parts replaced (Symbol editor, Edit stock, New cable type) |
| a page, when a symbol is needed that nobody has | **New symbol** on Draw ▸ Place, beside Symbol (Ctrl+I), which opens Parts on a new drawing at once; New symbol… at the foot of the Insert symbol picker, where a search that found nothing ends; Draw a symbol like this one… in a placed symbol's context menu; the command search under New symbol, Draw a symbol and Symbol editor (§5.6) |
| a device on a page | Open part in its context menu; the Parts row of Properties (the row's link, and Open part in its menu beside Choose part…) |
| the part dialog and the device browser | Open in Parts on the active row's detail; **New part…** at the list's foot |
| a finding, a refusal, a toast | **Edit part…**, offered by every outcome that comes from a part's missing datum (§5.7) |
| a list's row, a navigator's row (Parts in use, Libraries, Devices) | Open part in the row's context menu, and Enter |
| a form that needs a part that is not there (a cable's type, a bus cable, a plug, a station's card) | New part… in the form, which opens Parts with the kind given |

**A strip over the sheet says where the person came from** whenever he came with a purpose, and holds the way back:
"You are choosing a part for -102Q1" with Back without choosing; "Opened from a finding on page 904 Door" with Back
to Messages. Back on the bar returns to where he was, with the page, the selection and an open dialog as he left
them. Coming from a chooser, Save reads **Save and use for -102Q1**: the part is saved, given to the device, and he
is back on the page with the device selected; from the part dialog without saving, Back returns to the dialog with
the opened part under the cursor. Coming from a page for a new symbol, the next step reads **Save and place on
102** and Back is Back without it (§5.6). Back with unsaved changes asks: Save, Discard or stay.

### 5.3 Finding a part

The list's head is a switch **Parts | Symbols**, the search field, three scopes and the filters.

- **Search** takes what a person knows: a part number in any spelling, a maker, a word of the description in the
  window's language, what it is for. The hits come best first by the ranking of §4.3, the words that matched marked;
  parts close to the query follow under Close to it. The search reads the libraries' index and the project's own
  copies as one list.
- **Scopes**: All, In this project (the parts the project holds, used or not), My parts. A project's workshop stock
  and the set an import has just brought are scopes of the same list while they apply (§5.9, §5.10).
- **Filters**: Kind, Maker, each a menu with the count of hits per choice, and **Lacks something**, the parts that
  lack a datum a feature reads.
- **A row** is two lines: the part number and the maker, then the description; at its right where the part lives
  (mine, starter, a library's name, only this project); a dot where it lacks something. With nothing searched the
  list stands grouped by kind with counts, under Lately opened.
- Enter or a click opens the row's part in the sheet. Several rows are selected with Shift and Ctrl (§5.9).

The Symbols shelf lists symbols the same way: by name, what it is, its standard's number and keywords, with the
scopes All, In this project and My symbols.

### 5.4 The sheet

**The heading** is the part number, the description and the maker, and at its right what a person must know of the
part at a glance: whether it is saved; where it lives; which devices of the open project use it (a click lists them).

**The groups** are those the part's kind has, in this order, and no other (§2.5 says which fields each kind has):
Identity; On the schematic; In the cabinet; Connection points; the kind's own (Enclosure, Mounting plate, Rail, Duct,
Cable, Wire, Ferrule, Marker, Gland, Valve, Tube, PLC, Network ports); Technical data; Goes with it; About. A cabinet
has no pins and is asked for none; a wire has no size in the cabinet.

**A field** is a label and a value: a length in millimetres, a cross-section in mm², a count, a choice of words, a
colour, a yes or no, a part. The declaration of §2.5 gives each field its word, its kind of value and its check, and
one renderer per kind of value draws it. Under a field stands, in the tertiary colour, what the datum is for where
that is not plain ("The strip length the wire list prints"), and in the warning colour what follows while it is
empty (§5.7): on one line that a field he can change keeps whether or not it has something to say, the whole sentence
in its tooltip and under Ready for, so that the fields and buttons below never move as a warning comes and goes. A text that cannot be read is refused under the field as it is typed, in words that name what was
typed and what is expected ("“six” is not a length: type millimetres, such as 6.2"); the field keeps the text.

**A table** is a list member with a column per field (Connection points, Doors, Holes, Comes with, Also sold as): a row is
added by the link under the table and removed by its row's button; a cell is a field. Rows that hold the same values
stand as one row naming their pins ("1, 3, 5"), which opens to change one of them.

**On the schematic** asks how the part is drawn, in a person's words, and holds what the function template and the
contact data are in the record:

- *Drawn as pieces.* A table **Drawn as**: What it is, Pins, Symbol. What it is offers what a panel builder calls
  the piece (a coil, a normally open contact, a main contact, a protective device, a lamp, a pushbutton, a motor),
  a long list that what is typed at its top narrows; Pins are the pin names of that piece, typed; Symbol shows the symbol that draws it and takes another (§5.6). The
  first row is the piece the device's tag stands at. From a row the place writes the part's pins, the template
  entry and, for a contact, its contact data: no one types a function, a key or a template.
- *Drawn as a box.* The pins (Name, Carries, and for a box the Edge and the distance from the corner) and, where it
  has them, the network ports (their bus symbol pictured "On a bus page", with Make my own from it…) and the PLC
  channels. The picture shows the box; Arrange… opens it as a drawing (§5.6).
- *A terminal* is its levels and their clamps: What it is (feed-through, protective earth), Levels (1, 2 or 3), and
  a grid with a row per level and the clamps on its left and right side by name, a second clamp on a side added by
  a link. Each level passes through on its own.
- *A cable* is drawn as its cores, which stand in its own group: how many, how they are named (numbers, colours, his
  own names), a cross-section for all or per core, the protective earth core, the shield.
- A kind that is never drawn (a wire, a ferrule, a marker, a rail, a duct, an enclosure, a plate, an accessory) has
  no such group, and a line under the pictures says where it is chosen instead; Ready for reads its pins, where it
  has them (a plug, a bridge), for its clamps alone and says that the schematic does not draw it.

**In the cabinet** holds the size, how it mounts, what is kept free round it and in front of it, its front (a
rectangle, or round where the width equals the height, or an outline brought from a drawing, shown as "an outline
of 4 lines" with Import outline… and Remove) and its holes (X and Y from the front's bottom-left corner, Kind and
Size as two cells over one hole; a hole off the front is refused in words). **Connection points** holds, per pin, the kind
of clamp, the cross-sections it takes, how many wires, whether a twin ferrule, and where a wire reaches it.
**Technical data** holds the values the program computes with as fields of the part's kind, by their names (§2.5),
and any other value as a free row of a name and a value. **Goes with it** holds what the part brings (Comes with)
and the forms it is also sold in (Also sold as), §2.5. **About** shows where the part came from, when it was last
changed, where it is listed, and Also stored: what a file brought that the program does not ask for.

**Save** is refused only for what makes no part: no maker, no part number, or a part of that maker and number that
exists, each said at its field with the way on ("Example CON-18-24 is in My parts already. Open it"). Everything
else may be missing: Ready for says what follows. Where the open project holds the part (its devices use it, or
its workshop stock names it), Save shows first what changes there (the pins that go or change their names, the
boxes that are drawn anew and why, the lines of the parts list that change), and Save and update makes the change
in one undo step: until then the project's copy is as it was. The card names each field that changes by its word on
the sheet, with its values as the sheet shows them ("Mounts on: TS 35 × 7.5", Comes with by the part's number), and
never an identifier, a version requirement or a member's path. It names the parts kept in My parts with the part
(§3.1), and its Apply writes My parts; its Cancel writes nothing. A part of kind wire, ferrule or
marker is refused as something another part comes with: the consumables count it (§2.5).

**Remove…** takes a part out of My parts, or out of the project when it is only there. It stands greyed while
devices use a part that is only in the project, saying how many. A part something else names (another part that
comes with it, a device's choice, the workshop stock) is refused in words that name the first that does: who names a
copy is the one question of the link table, which `edit.delete` and the store's check read too. A project that
holds a copy of a part removed from My parts keeps it, and the answer names its devices.

### 5.5 Making a part

**New part…** asks first: **What is it?** The sheet shows cards in four groups (Devices; Terminals, cables and
wiring; Cabinet; Pneumatics), each a kind in a person's words with a line of what it will be asked for: Device with
symbols, Device drawn as a box, PLC card or station, Pushbutton or lamp in a door; Terminal, Cable, Wire, Ferrule or
lug, Marker, Cable gland; Enclosure, Mounting plate, Rail, Wiring duct, End bracket or accessory; Valve, Tube, Other
pneumatic device. A card is a part kind with what it presets (a pushbutton in a door is a device that mounts in a
cut-out); the cards are declared once, as the starts of the place, and a form that opens Parts with a kind names its
start (a bus cable, a plug, a station's card).

The answer decides what the sheet asks for, and nothing else is asked. The kind may be changed later while no device
uses the part; what the new kind has no field for is named before Save and goes with it.

Two more ways start a part, and both end on the same sheet:

- **A part like it.** Like this on an open part, or the card A part like it… and a row of the list: the new part
  opens with everything the other has but its number, "New, not saved".
- **What the maker gives.** A table, an EPLAN parts file, an outline drawing for the open part (§3.6, §5.9).

A new part needs a maker and a part number to be saved and nothing more. It is saved to My parts (§3.1); the open
project takes it when a device uses it.

A part of another part is made in passing: where a field takes a part that is not there yet (a cabinet's mounting
plate, a terminal's end plate), New… beside the field opens that part's sheet over the first, and Save returns to
the field with it chosen: New plate… under a cabinet's fields, New part… in each Comes with row.

### 5.6 The pictures, the drawing and the symbol

**The pictures** show the part as a page draws it: its pieces with their symbols and pins (a terminal once per level),
or its box, and its front on its rail with its size. They follow each value as it is typed; while a text does not
read or a table row is not filled in yet, Ready for keeps what it said.

**A drawing opens in the middle of the same screen** when a person draws: Draw… on the schematic picture, Arrange…
on a box, the Symbol cell of a piece, a symbol of the Symbols shelf. The bar goes to its Drawing tab, the list
gives its place to what is being drawn (the part's Drawn as table, or its pins and ports, and the outline of the
symbol: its pins, texts and shapes, each row a click to the thing, and To do, what keeps it from being saved, or
Ready to save) in a pane of 416 px (476 px at 2560 × 1440) where a Drawn as row reads whole, and Properties on the
right shows the rows of what is selected. A strip over the drawing says in a sentence what is being drawn and for
what. Done returns to the sheet; nothing is saved until the part is.

- **Arranging a box.** The box stands as a device of the part is drawn. A pin is dragged along its edge or onto
  another one; an edge or a corner is dragged to size the box, the pins on the edge that moves going with it, the
  edge stopping one step before the nearest pin. There is one list of pins, the sheet's: a pin dragged changes its
  row, a row typed moves the pin. A place typed between two steps of 4 mm, or on a corner, goes to the nearest step
  clear of the corners, and a note says where it stands. What a pin takes from a PLC channel (its name, what it
  carries, its address) is changed in that group and is shown here; its place on the box is the box's. A network
  port's sockets are no pins of the schematic box: a bus page draws the device with its bus symbol
  ([10](10-plc-and-bus.md) §2.7), pictured on the sheet as "On a bus page"; Make my own from it… opens that symbol as
  a drawing of his own. Done on a box arranged writes
  its layout into the sheet, so a device of it is drawn with one click; a box nobody arranged keeps none (as 0.3.5
  left every one), is pictured laid out as Arrange… would and drawn on a page by its two corners, and its part's Save
  draws no placed box anew. The Save of a part whose box was arranged draws the part's placed boxes anew in the same
  step (`library.update` and `part.edit_copy` carry them as `redraw`, each a `box.reshape`), and its card names each
  box drawn anew.
- **A symbol for a piece.** The Symbol cell of a Drawn as row offers the symbols that draw what the piece is with
  that many pins, the starter's first, and two more: **Change it for this part…** opens a copy of the piece's symbol,
  and **Draw a new symbol…** an empty drawing on which the piece's pins stand already, named as the piece names
  them. Either is the person's own symbol, kept with his parts, and it draws that piece of that part
  (`TemplateFn.symbol`, §2.5); the symbol it came from stays as it is. What the symbol is, where it is listed and
  which function it stands for follow from the piece and are not asked. Done saves the symbol with his parts at once
  and gives the piece its address; the part that names it waits for its own Save. Use it for every part with this
  piece… on the strip makes it the symbol of that kind of piece for his parts.
- **A symbol that belongs to no part** (a symbol of a standard, a symbol placed before a part is chosen) is opened
  from the Symbols shelf, or made with New symbol. It has a short sheet (Name, What it is, Listed under, its pins)
  and the drawing. What it is asks in a panel builder's words and sets what the program needs under it. Its sheet
  lists Parts drawn with it and offers New part drawn with it… once it is saved. A symbol that is not
  his is not changed: Make my own from it. Save answers with the first thing To do lists, above the drawing.
- **A new symbol, in the middle of a page.** Drawing a symbol of one's own is a thing a person does often, so it is
  one click from a page and its way back finishes the errand. New symbol (the Draw tab, Ctrl+I, the picker's foot)
  opens Parts on a new drawing at once, with no question first; the strip says "A new symbol for page 102 Control
  24 V DC" and holds Back without it. The drawing opens ready to draw on:
  - on **the page's connection grid**, so that what he draws fits the page he came from;
  - with **What it is** as he chose it for the symbol he drew last, the discipline of the page (a fluid page gives a
    pneumatic symbol), and Listed under following from them;
  - with **the texts its kind needs** already placed where the standard puts them ([07a](07a-symbol-design-standard.md):
    the device tag, and the cross-reference of a coil or a contact), so that To do lists only what is his to give:
    a name, and the pins;
  - with **his drawing grid, Snap and the tool** as he left them the last time.

  New symbol's menu offers two more starts: **Like the last one**, a copy of the symbol he saved last, and **Like
  the selected one** when a symbol is selected on the page (the same as Draw a symbol like this one… in its context menu). The
  Symbols shelf leads with Lately drawn and keeps the scope he used last.

  The next step is **Save and place on 102** (Ctrl+Enter): the symbol is saved with his parts, the window is back on
  the page, and the Place tool is armed with the symbol under the pointer, as if he had picked it in the symbol
  picker, which lists it first under Recent from then on. One click places it. For a drawing started as Like the
  selected one the next step is **Save and use for -102S1**: the selected symbol is exchanged for the new one, its
  wires kept by the exchange's rule (§3.2 rule 7). Back without it returns to the page as it was; the drawing is
  kept as the draft in hand and New symbol offers to go on with it. Where he changed the symbol's connection grid
  so that it differs from the page's, the strip says so before he saves, naming both grids as the status bar does
  ("This symbol is on 4 mm; page 102 is on QET 10 px: it is placed as a copy drawn on the page's grid"), and the
  placing follows the picker's rule for a symbol of another grid (§5.12).
- **What a person who draws weekly repeats, and what the program keeps.** He gives each symbol its name, its pins
  and its shapes: those are the symbol. He gives nothing else twice: the grid is the page's; what it is, the
  drawing grid, Snap and the tool are his last; the texts of its kind stand ready; a symbol like the last one or
  like the one on the page is one entry; nothing asks where to keep it.
- **A symbol from a file** (a QElectroTech element, an EPLAN symbol file, a DXF drawing) comes in by Import… as a
  drawing with its import report; one brought from EPLAN is saved only once its review is acknowledged.

**What a person learned on a page works on the drawing.** The selection is the view-model's,
so the outline, the drawing and Properties show the same things: a click selects, Shift adds, Ctrl toggles, a
rectangle dragged from empty paper takes what lies inside it (to the right) or what it touches (to the left), and
Ctrl+A takes everything. A shape that is not filled is met by its line (a click within a few points of it, a
rectangle that crosses it), a filled one, a text or a picture by its face: inside an empty box is empty paper. The page's own keys act on it whenever no field is typed into, through the keymap
(its context `symbol_editor` takes, of the canvas's keys, those of its tools and of what acts on its selection:
`keymap::context_takes`): the arrows move it one grid step (the connection grid's with a connection point selected), Delete
removes it (a connection point with its number's text and its pin), Ctrl+C, Ctrl+X, Ctrl+V and Ctrl+D copy, cut, paste
and duplicate, Escape ends a gesture, then returns to Select, then clears the selection, and Ctrl+Z and Ctrl+Y step the
drawing's own history. A drag on a thing moves the selection with a ghost of where it lands; the one selected shape's
handles drag its ends and corners (`symbol.grip`); Line up puts several things on one edge (`symbol.align`). The
highlight and the handles are the page's (doc 16a §7.1). The tools draw as on a page: a shape by a drag or two clicks,
a text typed where it was clicked, a connection point on the connection grid with the next number and the text that
writes that number: inside the body one millimetre from the point, turned a quarter on an upper or lower edge
and level on a left or right one, where a device box prints a pin's name (§2.5; one rule,
`ew_scene::page::pin_text_place`), so a point on the lower edge has its number above it inside the box, not across
it; a point moved alone takes that text with it, and a point given another edge in Properties has it placed again by
the same rule; and the drawing shows the text the page prints (before, the drawing showed the point's name as an editor mark that
no page printed, and a number text clicked onto a lower point hung across it); a tool's options
stand on a card at the drawing's top left. The live A–H strip is drawn through the placement's own lowering. To do is
the drawing's validation, shared with `ew symbol validate`, not the library lint. "Test place" is Test on a page
(`symbol.scratch`): a new page on the symbol's grid with an embedded test copy, one undo step for the page and one for
the placement.

### 5.7 What a part lacks

Every field names what reads it and what that feature does without it (§2.5, §3.4), and the place shows the same
list three times, so that it cannot be missed and is never said in other words:

- **at the field**, while it is empty: "This cabinet cannot be chosen for a layout until it has a height"; "Without a
  clamp range the check that a wire fits its clamp skips these pins";
- **in the contents list**, a dot at each group that lacks something;
- **under Ready for**, one row per feature the part's kind is read by: ready, with what the feature will make of the
  part ("Footprint 45 × 85 mm on a rail 35 mm"); lacking, with the same sentence as at the field; or not needed, with
  why ("It mounts on a rail: it needs no holes"). A click on a lacking row goes to its field.

The other way round, **a finding, a refusal or a toast that comes from a part's missing datum offers Edit part…**,
which opens Parts on that part with the field in view, ringed, and the sentence under it. The outcome carries the
part and the field as facts; the shell makes the offer for every outcome that carries them, in one place.

### 5.8 Where a part lives, in the window

A person sees three things and no more (§3.1 has the rules):

- **Mine**: a part of My parts. He changes it freely; it is there in his next project.
- **Not mine**: a starter part, or a part of any other library, one he can write as well (a company library, a
  colleague's): only My parts is his, and a library is named where its part is shown. One line under the heading
  says so with the reason ("A starter part. It comes with the program and is the same on every PC, so it cannot be changed here") and
  offers **Make my own from it**: the same sheet as a new part with everything the other has, the maker and number
  his to give. Where devices of the open project are the other part, Save offers to use his for them, names them,
  and says what does not carry over.
- **Only in this project**: a part the project holds that no library he can read has, such as one that came with a
  colleague's project. He changes it freely, for this project; **Keep in my parts** puts it among his with the parts
  only in this project that it names, and the toast names them ("EX-TB-1 is kept in My parts with Example EX-EP-1").

A part is one row wherever it is listed, in Parts, the part dialog, the device browser, the layout's offers (Create
enclosure, the rail and duct tools, a cable's type) and Properties' Parts row: a part of My parts the project holds is
one row, Mine, and the project's copy it was kept from is not listed beside it; after a Keep taken back by Undo the
part is one row, "Only in this project · also in My parts", with Keep in my parts as the way (§3.1). A part that names
a part no one holds says so over Ready for, by the field's word, with the way.

A part of My parts that was changed since the open project took it says so on its sheet and in the list ("Changed in
My parts since this project took it. See changes…"); nothing changes in the project until he says so (§3.1).

### 5.9 Many at once

- **A table mapped once.** Import… with a table (CSV, Excel) shows as the sheet which column is what: each column
  of the file with its first value and the field it goes to, chosen from the fields by their words and groups; Not
  imported for a column no field keeps. A column whose header is a field's word or its name is mapped already. The map
  is saved under the source's name when the set is saved, and read again for the next table of that source. Width,
  Height and Depth go to the field of that name the row's kind has. A changed map reads the set again at once.
- **The set.** What a table or a parts file brings stands in the list as a scope of its own, This import, each row
  new, changed, the same or refused with its reason, and a dot where it lacks something; the sheet's What was read
  says each refusal and each row that is the same in a sentence. A row shows beside it as it will be: its kind, its
  fields and Ready for. A row is laid over the part My parts holds of its maker and number; a row of a part he can read that is not his (a starter part, another library's, one only in this project) is the same where it repeats it, else refused with the way in (Make my own from it, Keep in my parts); only a row that names no part he can read is new. A changed row names the fields it changes. and its texts are read as
  the part's sheet reads them, so a text that does not read refuses its row in the sheet's words. Nothing is saved
  until **Save n to My parts**, which reads the file again against My parts as it is then and writes the new and the
  changed parts in one write; Discard drops the set. An EPLAN parts file's set also says how many of its parts the
  open project holds a copy of and how many of those would change, with **Update them…**, the previewed update of
  those copies.
- **Several parts selected** in the list (Ctrl takes a row in or out, Shift the rows up to it) show the fields they
  share on one sheet: a value they all have, or "mixed". A value typed goes to all of them, and "mixed" is never
  written; Save saves each as its next version in My parts, with one card for the devices of the open project that
  names how many parts and which fields before Apply, one undo step. A starter part among them, or one of a library he
  cannot write, locks the sheet in a line that names it, with Make my own from it for each. Where parts only in this
  project are among them, Save reads **Keep in my parts and save**: they are kept in My parts with the values typed,
  in the same Save as the others (§3.1: the card names what is kept, Apply writes it, Cancel writes nothing). The Connection points table is shared too where the selected parts have the same
  pins, a cell each by its pin and column; the other tables, the forms and the drawings are a single part's.
- **A spreadsheet for a family.** Export as a table… writes the selected parts, or the open one, with the declared
  columns in the window's words (CSV or Excel, by the file's extension). Fifty terminals of one family are made there by copying rows; the import reads them back, new rows as new
  parts, changed rows as changes. A column mapped to **Like** names a part by its number: a new row takes from it
  everything its own columns do not give (its pins and what it is drawn as, its clamps, what it comes with), so a
  family is one part made on its sheet and a table of the others. A table exported and imported unchanged brings
  every row as the same.

### 5.10 The workshop stock

The stock of the open project (§2.5) is a view of the list: In this project, narrowed to Workshop stock (the scope
`stock` of `parts.find`, a chip beside the scopes), in the shop's order of preference under a heading per kind that
says the rule ("Ferrules · the first that fits"), each row numbered among its kind. A row is dragged to another place
among its kind, and a click on it opens its part; the chip's hint says where the stock is changed. Add from my parts…
(My parts: a part opened there is added from its sheet) and Add the generic stock add to it. The sheet of a wire, a
ferrule, a marker or a gland says whether it is in the stock and where, with Up, Down and Take out, or Add to the stock,
which puts it first of its kind: a shop adds a part because it prefers it (`parts.stock {op}`; each change one
`workshop.stock`, one undo step). Project ▸ Edit
stock…, the project's Properties and the package dialog's Workshop stock… open the view (`parts.stock`). **Use this
stock for new projects** keeps the stock, in its order, as the window's setting `shop_stock`, which a new project from
the built-in basic project starts with (an item no longer found is left out; none found: the generic stock). Cut to is
the row `workshop.cut_increment` of Project properties ▸ Workshop.

### 5.11 Choosing a part for a device

One row in both the device browser and the part dialog: part number, maker, description in the reader's language,
then what choosing it does, in words (below); the dialog adds the fit of §4.3; search with the ranking of §4.3;
favourites, recently used, used in this project; the active row's detail: what it is drawn as, its pins with each
clamp's range, what it comes with and the forms it is sold in as a choice, and Open in Parts. New part… stands at
the foot of both lists.

**What a part's row says it does.** A part is placed by the first of its functions that has a symbol on the page,
its main function first, and its row says which in words: "Places the circuit breaker", "Places the coil 14" (a
valve on an electrical page). A part whose function is a black box says "Drawn as a device box" and arms the box
tool, as its row said; a part none of whose functions has a symbol on the page stands greyed with why ("No symbol
draws a valve on a schematic page"). A row that opens to its functions says what it holds ("3 functions: coil, 2
contacts"). A part with variants places the variant chosen in its detail, the part's own numbers first. The browser
files a part under its trade by its category's row of the category table (`trade` beside the letter codes,
`crates/ew-rds/data/identifier-sets.toml`), which also names the part groups that are no function category (bus
cables, tubes): one table, no list of categories in the window.

**Accessories and variants of a device.** Properties of a device show, under Parts, a Variant row (the part's own
numbers, then each variant by name and number). Properties of a strip show a row per accessory its terminals' parts
bring (End plate, End brackets): the part counted, with the parts it comes with; typing another part's number takes
that part, emptying the row leaves it off (both the strip's choice, `Device.accessory_choices`, through `fields.set`).
The strip editor lists what the terminals' parts bring at their places in the mounting order (an end bracket before
the first terminal and after the last, the end plate after the last terminal whose part names it), each read-only with
"with <part>", and a left-off one as "left off this strip"; Delete on such a row leaves it off for that strip, the
toast says so. The Part cell of a terminal whose part has variants chooses the variant (by its number, its name or
the part's own number): no column of its own, so that the side columns stay on show at 1920 × 1080. The strip editor
opens with the cursor on the strip's first terminal. The place browser's detail of a part lists its variants as a
choice and what it brings ("Comes with": kind, part, quantity and per what); the choice (`place.variant`, by a
variant's key, name or number) is the one the Place tool it arms takes, `device.place`'s `part_variant`, kept across
placements and named in the tool's label. A greyed row of a part no symbol draws on the page says what it is ("No
symbol draws a cylinder double-acting on this page").

### 5.12 Macros and libraries

The **Macro browser** is the Macros navigator (`NavKind::Macros`): the macros of every tier, then the project's own macro copies under "In this project"; its detail shows name, version and library, description, the devices and the variables, with "No preview" where the thumbnail should be. Enter on a row, its drop on a canvas or Insert macro… (Draw ▸ Place) arms the macro tool; its click opens the `macro` card in the Preview tab with the value set and the conflicts, and Apply is one undo step; a held macro without an installed source is not inserted. **Save as macro** (Edit menu) is a window-only dialog of name, key, description and library, which is the confirmation of a write that cannot be undone; kind, insertion point and the value-set table are not built, and "make variable" is the hidden action `props.make_variable`. The **Library manager** is the Libraries navigator (`NavKind::Libraries`): the libraries by tier with version, licence, access and index state, then "In this project" with each copy's state in words (Current, Update available, Only in this project, Deprecated, Local fork), each with a hover text of what it means, and the devices that use it; Update… names how many items it updates, is disabled with its reason when they have none, and opens `library.update`'s bulk card; "purge unused" is not in it. Its rows show IDs rather than names.

In the Insert symbol picker a symbol of another connection grid says so, naming both grids as the
status bar does ("4 mm", "QET 10 px"), and its Place puts it on the page in front: the project gets the
symbol's copy drawn on the page's grid (`SymbolDraft::on_grid`: every length scaled by the ratio of the grid steps,
every connection point on the page grid's point of its own index, as the starter library's QET edition), embedded
once (`library.embed`), and the place tool is armed with it. Test on a page on such a page is refused
in the same words and offers a new page with the symbol's grid: **Place on a new page with its grid** places the
published symbol there at 40/40 mm and selects it, from the drawing while its symbol is saved and
unchanged (`symbol.new_page`) and as the picker's second choice (`picker.new_page`); for any other drawing Parts
offers Test on a new page. Each is one undo
step for the page and one for the placement. New page (`page.new`) has a Connection grid, "4 mm" or "QET 10 px",
the current page's by default; on a page whose grid is neither (an imported one) the field is empty and the new page
still takes the current page's grid. A page's grid changes only while the page is empty.

### 5.13 Accessibility

**NFR-A11Y-02.** Previews expose an accessible name built from the item name, its representation type and its pin
names, for example "MCB 1P+N, multi-line, connection points 1, 2, N, N". What a part lacks is shown as text with an
icon, never as colour alone (FR-UI-025). The list, the contents list, the sheet and the pictures are reached with
Tab and F6 in that order.
