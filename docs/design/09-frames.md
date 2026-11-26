# Reports, forms and page output: the frame and its title block, changed by a person

A chapter of [Reports, forms and page output](09-reports-forms-output.md), whose table says where each section lives.
The form records, the placeholders and the starter frames are in [§2.5 to §2.7](09-reports-forms.md#25-formdef);
the cells and their columns and rows as the page derives them are in [06](06-schematic-editor.md) §2.4 and §5.1.

---

### 2.10 Fields of the project's own

A **field of one's own** is a value a title block prints that the program does not know: an order number, the
customer's own sheet number, a cabinet's position in a plant. It is a user property of the project
([03](03-domain-model.md) §5 `UserPropDef`, FR-PRJ-007) of the kind text, with one of two scopes: **the project**
(one value, the same on every page) or **the page** (a value per page). The other kinds and scopes of FR-PRJ-007 are
not used by a title block.

- **Its definition** is a row of `Project.user_prop_defs`: `{key, label, value_kind: "text", scope: ["project"] or
  ["page"]}`. `label` is its name as the person typed it (a multi-language text, printed in the window's language).
  `key` is made once from the first name: its Latin letters lower-cased, its digits, every other run of characters
  one `_` (`Order number` → `order_number`); a name without a Latin letter or digit gives `field`; a key the project
  already has gets `_2`, `_3` … The key is never shown and stays when the field is renamed.
- **Its values** are rows of `Project.user_props` (scope project) and `Page.user_props` (scope page), each
  `{key, value}`, the value a multi-language text. A field without a value prints nothing.
- **A cell prints it** through `{project.prop.<key>}` or `{page.prop.<key>}` (§2.6). A key the project does not define
  for that scope is an unresolved placeholder (§2.6): it prints nothing and the form finding names the cell. The fields
  are the project's data, not a rule of the derivation, so they resolve under every derivation profile (§3.7).
- **Where it is typed.** Title block fields… (Page ▸ Frame) lists the page's fields with the page's values first,
  then the project's, each with the cell that prints it; Project properties ▸ Fields of your own holds each
  project field's value, and Page properties ▸ Fields of your own each page field's. A row's menu there has Rename…
  and Remove.
- **Made, renamed, removed.** A field is made from the cell that will show it (§3.9), or with New field of your own…
  in Project properties ▸ Fields of your own (`user_prop.define`, which mints the key). Rename… changes its name
  (`user_prop.rename`). Remove (`user_prop.remove`) deletes the definition and every value in one undo step, and is
  refused while a form the project holds shows it, in words that name the pages that print it ("The title block of
  pages 101 to 105 and 201 shows Order; give that cell another value first"). A value is written with `fields.set`
  on the project's or the page's `user_props`, the list whole, as the parties are.
- **A template** passes its fields on with its frames: a project made from it holds the same definitions and the values
  the template holds (§3.9).

---

### 3.9 Changing a frame and its title block

**Who, and when.** A panel builder changes his title block once, when he sets the program up for his company (his
logo, his fields, the cells his drawings have always had), and again when a customer prescribes a block of his own.
He does it on a page of his template, so that every project made from it starts with the result: Templates… ▸ Edit
opens the template as a project, or he changes an ordinary project and saves it with Save as template…. He fills a
form in and moves things; he never draws one. The tenth job does not open it: a new project from the template has the
frame and the fields, and Title block fields… takes the values.

**The journey.**

1. He opens his template (Templates… ▸ Edit) and its page 1. He double-clicks the title block; the
   drawing fades and the card says that a change shows on every page that prints the block.
2. He clicks "End user: …". Properties shows the cell: Text "End user:", Shows "People · End user ▸ Name". He types
   "Customer:" and chooses Customer ▸ Name. The page prints "Customer: Sample Customer" at once.
3. He right-clicks the empty notice cell and chooses Split one above the other. In the upper half he types "Order:",
   opens Shows and chooses New field of your own…: "Order number", one value for the whole project, "0001". The cell
   prints "Order: 0001".
4. The lower half gets "Customer sheet:" and a new field of each page, "Customer sheet". He gives the logo cell
   Company logo and chooses his logo file. A line between two cells he would drag the same way.
5. He clicks the column numbers. Properties shows Columns 17, numbered from 0, and Rows 8, lettered from A. He types
   10 columns from 1 and 6 rows; the page shows them, and the Preview lists the cross-references that print
   differently. Apply.
6. Done. He saves the template. The template now holds his frame, his title block and his two fields.
7. Each job: New project from the template. Page ▸ Title block fields… holds Customer sheet for the page, and Order
   and the program's values for the project; he types them once.
8. The day after, he moves a symbol to another column: its cross-references print the new cell, nothing to update.
   He adds a page: it prints on the project's frame.
9. A customer prescribes his own block: he makes a template for that customer from his own, gives its block the
   customer's cells and logo (A picture…), and makes that customer's projects from it. A project begun before takes
   the new block with Use the frames of a template….
10. A project brought in from QElectroTech keeps its block; its own variables are fields, and he changes it the same
    way.

**Where it starts.** Page ▸ Frame ▸ **Edit frame** (`frame.edit`; Dutch "Kader bewerken"), a double-click on the
title block or on the column numbers or row letters of the page, and Edit frame in the title block's context menu,
beside Title block fields…. The command search finds it by "frame", "title block", "border", "columns", "rows",
"rulers" and "logo". The Frame group of the Page tab holds the two: Edit frame for the block, Title block fields…
for its values. Without an open page Edit frame is greyed with its reason ("Open a page: a frame is changed on a page
that prints it"); a template made with its pages left out gets one with New page….

**What the page shows.** Edit frame is a tool of the page ([04](04-architecture.md) §9.2): the page's drawing fades to
a third, and the frame, its columns and rows and its title block stay as they print. Its card says which frame it is
and where a change shows ("Page 101 prints on the project's frame and title block. Every change shows on each page
that prints them."), with Done; Done and Esc end the tool. The pointer outlines the cell, the line or the ruler it is
over. The HUD offers Split side by side, Split one above the other, Join and Picture…, and says what a click does.
Every change is one undo step and prints at once, on the page in view and on every page of the frame; only a change
of the columns and rows or of the title block's height waits for Apply (below).

**A cell.** A click chooses a cell (Shift adds one, for Join); Properties shows it under its printed text ("Drawing:
SAMPLE-E", "Title block cell · prints on every page"):

- **Text**, its label, in the language the project prints in; the label's other languages under More. A double-click
  on the cell types it on the page. Tab and the arrow keys go to the next cell; Del clears the cell.
- **Shows**, the value it prints, a list in words, never a path: Project (description, long description, reference,
  drawing number, object, document title, created, saved, supply, notice), Page (title, description, number,
  previous, next, `=` function, `+` location, scale, changed in revision, position, number of pages), Revision (label,
  date, state, designer, checker), People (each role ▸ name, street, postcode, city, country), Fields of your own
  (each field, then New field of your own…), Other (Company logo, A picture…, Text only). It writes the cell's
  placeholder (§2.6); a cell of an imported template whose value is text of its own shows "Text only".
- **Text size**, the project's text styles by their names (as Text styles… names them); a new cell takes the size of
  the cell it was split from.
- **Alignment**: Left, Centre, Right; the text stands in the middle of the cell's height.
- A line under the rows says where the value is typed: Title block fields…, Project or Page properties.

A new cell shrinks its text to fit down to a printed capital of 1.2 mm, as the starter cells do (§2.5).

**Lines, splits and joins.** A line between cells is dragged: every cell on either side of it grows or shrinks with
it, a chip names the two widths ("Description 119.8 mm · Drawing 33.1 mm"), it snaps to 0.5 mm (Alt: free), and no
cell gets narrower or lower than 2 mm. The block's outline is the frame's and does not move, but for its top: the line
between the drawing area and the title block makes the block higher or lower (below). The cell's context menu holds
Split side by side and Split one above the other (two halves; the new one empty, the old one keeps its text and
value), Join with the cell to the right and Join with the cell below (greyed, with the reason, where the two do not
make a rectangle), Picture…, Shows…, Clear the cell and Title block fields…. Join keeps the first cell's text and
value; a text that goes is named in the toast, with Undo. The block's lines are written anew from its cells after
each change (§2.5).

**Pictures.** **Company logo** prints the project's company logo (`{project.logo}`, §2.6), the one picture every
block of the company shows; a project without one shows Choose logo… under the row (`project.company.set`). **A
picture…** puts a file into this title block alone, such as a customer's logo: an SVG, PNG or JPEG of at most
512 KiB, an asset of the project, fitted into its cell at its proportions and centred, as an imported QElectroTech
logo is ([14](14-interop.md) §4.9). A drawn logo is not made here.

**A field of one's own.** New field of your own… at the foot of Shows asks the field's name, whether it has one value
for the whole project or one for each page, and its value now ("Order number", the whole project, "0001"); Create
field makes it (`user_prop.define`) and gives the cell its value in one undo step, the cell's text becoming the name
and a colon where it had none. The field then stands in Title block fields…, Project properties and Page properties
(§2.10).

**Columns and rows.** A click on the column numbers or the row letters chooses the rulers. Properties shows Columns,
along the top (How many, Counted with Numbers or Letters, First), Rows, along the left (the same, and Skip I and O),
and Drawn (Yes, No) for both. The paper and the title block keep their size: the columns share the drawing area's
width and the rows its height, each an exact step ([06](06-schematic-editor.md) §5.1). A count of 1 to 100 and a first
number of 0 to 999 are taken, anything else is refused under its row in words. The page in view shows the new columns
at once, and the Preview tab lists what prints differently ([06](06-schematic-editor.md) §2.4): every cross-reference
into the pages of the frame ("103-C1 → 103-B1", with its page), every wire number whose order changes, every tag of a
numbering scheme that prints a column or a row. Apply writes it as one undo step (`frame.set_grid`); Cancel leaves the
frame as it was. A change of the title block's height is previewed the same way, since it changes the rows' height;
it is refused while a connection point, a bend of a wire or a symbol's anchor of a page of the frame lies where the
block would grow, naming the page ("Page 101 has drawing where the title block would grow; move it up first"), and
while a lower block would make a cell lower than 2 mm. The block's cells, pictures and texts scale with its height.

**One frame, several pages.** The card and the header of Properties say where a change shows: a cell of the title block
on every page of every frame that names the block, the columns and rows on the pages of this frame. With nothing
chosen, Properties shows the frame:

- **Paper**, read only;
- **Used for**: the project's default, a page type's (Title page, Section divider, Table of contents), this page's own,
  or the lists'; it is set where it is today, Project properties ▸ Frames, Frame of the lists and a page's Frame row;
- **Title block**: Shared with the other frames, or Its own, a copy of the block for this frame alone (the cover's or
  the dividers' own block); choosing Shared with the other frames again names the shared block, and the copy stays
  unused until purged;
- **Give this page its own frame**: a copy of the frame for this page alone, which its Frame row then names;
- **Columns and rows**, their summary ("17 numbered from 0, 8 lettered from A"); a click chooses them;
- **Use the frames of a template…** (below).

By default one title block serves every frame: the starter frames for drawn pages, for the cover and for the dividers,
contents and lists name the same block, so a first change shows on every page. A page type with a block of its own
(a cover) is one Its own away.

**Where the result lives.** In the project: its frames and title blocks are copies it holds (§2.1), and its fields
are its data (§2.10). A starter form is a copy of the program's library, and its first change makes it the project's
own ([07](07-symbols-parts-macros.md) §3.1, "local edits fork the item"): the changed form, and every starter frame that names a changed title
block, become copies of the project's own; every page, report block, frame and the frame defaults that named the old
copies name the new ones; the program's copies stay unused until purged. The toast of that first change says so
once ("The frame and title block are now this project's own; the program's stay as they were."). A copy of the
project's own, an imported one included, is changed in place. A page whose frame is the built-in one (no form held)
gets the starter frame and title block embedded as the project's own on its first change.

**How the next project gets it.** Through the template: a project made from it holds its frames, title blocks and
fields. A project made before the change takes them with **Use the frames of a template…** (`frames.adopt`, Bulk,
previewed): a list of the templates, then a preview of the pages whose frame changes, the fields of its own added and
the cross-references that print differently. It brings the template's frames and title blocks, its frame defaults,
the frame its lists print on, its fields of its own (their definitions, and their values where this project has none)
and its company logo and notice; a page with a frame of its own keeps it. One undo step; the replaced forms stay
until purged.

**Imports.** A QElectroTech project keeps the title block and the frame of each diagram ([14](14-interop.md) §4.9),
its own variables as fields of its own (§2.10), and is changed in the same way. The EPLAN imports bring symbols, parts
and lists, and no frame or title block: a page made in such a project prints on the project's frame.

**What it does not do.** Draw a title block: a line or a text anywhere but in a cell, and a drawn logo; a frame's own
drawing (the cover's lines and boxes) is kept as it is. A title block beside a drawing area that goes on next to it
(ISO 7200's block in the bottom right corner); columns and rows on all four sides; a frame per section; fields that
are not text; names for a project's own frames; a report form's table (§3.8).
