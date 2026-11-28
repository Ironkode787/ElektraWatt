# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of `ew-api` in en-GB: what the session refuses or names: the edits of a table, the part editor, the find
# pane's fields, the symbol importers, the libraries' states. One message per key; `{ $p0 }`, `{ $p1 }`, ... are the
# arguments, always strings. The source language and the fallback of every other.

error-part-length = "{ $p0 }" is not a length: give millimetres, as 45 or 22.5.
error-part-hole-incomplete = Hole { $p0 } needs its X, its Y, its kind and its size.
error-part-cell-needed = Row { $p0 } needs a value here: only a value that may stay empty can be left out.
error-parts-written-partly = { $p0 } saved; { $p1 } not saved, as their library could not be written: { $p2 }
error-part-gland-incomplete = A gland needs its thread and the smallest and largest cable it seals.
error-part-gland-range = The largest cable a gland seals cannot be smaller than the smallest.
error-part-hole-off = Hole { $p0 } at { $p1 }, { $p2 } mm lies off the front, which is { $p3 } × { $p4 } mm: its centre is measured from the front's bottom-left corner.
error-part-front-round = A round front needs the same Width and Height: give both in Size and mounting, or choose Rectangle.
error-part-outline-locked = A part from a library keeps its front view: Import outline… changes a project's own part or a new one.
error-part-outline-unreadable = { $p0 } cannot be read: { $p1 }
error-part-accessory = No part is numbered { $p0 }
error-stock-kind = { $p0 } is no wire, ferrule, marker or cable gland: the stock keeps only those
error-stock-gone = The part is not in its library any more: it cannot join the stock
error-part-accessory-kind = { $p0 } is no accessory: a wire, ferrule or marker is counted from the wires. Add it to the workshop stock instead, on its sheet in Parts.
reason-no-terminal-def = The project has no terminal definition to insert
reason-no-accessory-part = The project has no part for this accessory; embed one first
refusal-strip-nothing = nothing to edit
refusal-strip-no-accessory = the strip has no accessory { $p0 }
refusal-strip-accessory-part = an accessory needs a part
refusal-strip-accessory-kind = `{ $p0 }` is not one of { $p1 }
refusal-strip-gesture-alone = an insert or a move is a gesture of its own
refusal-strip-no-part = the project has no part `{ $p0 }` of kind { $p1 }
refusal-strip-no-part-either = the project has no part `{ $p0 }` of kind { $p1 } or { $p2 }
refusal-strip-not-strip = the device is not a terminal strip
refusal-strip-not-terminal = the row is not a terminal
refusal-strip-terminal-strip = the terminal's strip is not a strip
refusal-strip-insert-count = insert at least one terminal
refusal-strip-designation-one = a designation names one terminal
table-accessory-row = accessory row
table-pos-move = move rows with Alt+Up and Alt+Down
table-no-bridge = the terminal has no bridge
table-bridge-how = mark the rows of two or more terminals of the strip and press F2 in this column, or choose Bridge terminals in a terminal's menu on the page
reason-swap-two-cores = Mark two cores of one cable
reason-swap-cores-unwired = Neither core has a connection
# A damaged copy written from its library, with the version written.
refusal-words-copy-at-version = { $p0 } at version { $p1 }
refusal-words-part-kind = { $p0 }
refusal-words-cp-other-page = a connection point of another page
# The property grid's own parse errors: a typed value that is not one of its field's type.
refusal-input-scale = `{ $p0 }` is not a scale; type 1:n with n a whole number from 1 to 4294967295
refusal-input-decimal = `{ $p0 }` is not a decimal number
refusal-input-awg = `{ $p0 }` is not an AWG size
refusal-input-mm = `{ $p0 }` is not a length: type millimetres (at most 6 decimals), or the length with its unit, such as 2 m or 35 cm
refusal-input-rect = `{ $p0 }` is not a rectangle: type its place and its size in millimetres, such as 20, 15 mm · 60 × 40 mm
refusal-input-colour-alias = `{ $p0 }` is not a colour code; did you mean { $p1 } { $p2 }? Accepted: { $p3 }
refusal-input-colour = `{ $p0 }` is not a colour code; accepted: { $p1 }
refusal-input-colour-none = give one or two colour codes
refusal-input-colour-count = a colour has one or two codes
refusal-input-text-control = a text has no line breaks or control characters
refusal-input-yes-no = `{ $p0 }` is not yes or no
refusal-input-whole = `{ $p0 }` is not a whole number
refusal-input-bus-address = `{ $p0 }` is no bus address: type a station number, a PROFINET name with its IP address, pos and alias (EtherCAT), port and number (IO-Link), or TBD
refusal-input-range = { $p0 } is outside { $p1 }..{ $p2 }
refusal-input-no-unit = `{ $p0 }` has no unit; units: { $p1 }
refusal-input-ac-dc = `{ $p0 }`: type the number and its unit alone, such as 24 V; DC or AC is a potential's type, set at its definition
refusal-input-no-node = no structure node `{ $p0 }` of this aspect
refusal-input-form = no form of the project is called "{ $p0 }"; its forms: { $p1 }
library-form-frame = Frame
library-form-title-block = Title block
library-form-cover = Cover
library-form-divider = Section divider
library-form-report = Report page
library-form-label = Label
library-form-other = Form
library-form-named = { $p0 } { $p1 }
library-form-own-landscape = { $p0 } { $p1 } landscape, on { $p2 }
library-form-own-portrait = { $p0 } { $p1 } portrait, on { $p2 }
library-form-on-no-page = no page
library-form-numbered = { $p0 } ({ $p1 })
refusal-input-shown = this field is shown, not edited
refusal-input-json = not strict JSON: { $p0 }
refusal-input-no-part-copy = the project has no part copy `{ $p0 }`
refusal-input-quantity = `{ $p0 }` is not a quantity above 0
refusal-input-not-a-field = `{ $p0 }` is not a field of { $p1 }
refusal-input-plc-address = give an address (for example Q0.1), auto, excluded or tbd
refusal-input-plc-target = `{ $p0 }` names no device with a function to target
refusal-ref-not-uuid = `{ $p0 }`: not a UUID
refusal-ref-no-record = `{ $p0 }`: the project has no such record
refusal-ref-section-number = `{ $p0 }`: a section number is a whole number
refusal-ref-no-section = The project has no section { $p1 } yet: make it with New section… (the folder button over the Pages list, or Page ▸ New section…), then choose it here.
refusal-ref-no-folio = `{ $p0 }`: no drawn page has this folio label
refusal-ref-folio-many = `{ $p0 }`: { $p1 } matches (more than one drawn page has this folio label); use `id:<uuid>`
refusal-ref-no-tag = `{ $p0 }`: no device prints this tag
refusal-ref-no-tag-forms = `{ $p0 }`: no device prints this tag; a reference is a tag (`-101Q1`, a terminal as `-X1031:5`), `tag:<tag>`, `section:<number>`, `folio:<label>` or `id:<uuid>`
refusal-ref-ambiguous = `{ $p0 }` is ambiguous: { $p1 } devices print this tag ({ $p2 }); name one by its full designation or its `id:<uuid>`
refusal-ref-candidate = { $p0 } as `id:{ $p1 }`
refusal-ref-not-text = `ref` is a text `<kind>:<value>`
refusal-input-plc-slot = `{ $p0 }` is not a slot; give a number (3), <rack>.<slot> (1.3) or auto
refusal-input-metres = `{ $p0 }` is not a length in metres (for example 12.5 m)
# A clipboard fragment that does not read: its kind of failure and each line's reason.
refusal-fragment-not = the clipboard holds no ElektraWatt fragment
refusal-fragment-newer = copied from a newer ElektraWatt, which writes a newer format ({ $p0 })
refusal-fragment-too-large = the fragment is too large: { $p0 }
refusal-fragment-line = fragment line { $p0 }: { $p1 }
refusal-fragment-bytes = { $p0 } bytes, at most { $p1 }
refusal-fragment-line-bytes = line { $p0 } has { $p1 } bytes, at most { $p2 }
refusal-fragment-line-depth = line { $p0 } nests deeper than { $p1 }
refusal-fragment-records = more than { $p0 } records
refusal-fragment-object = a line of a fragment is a JSON object
refusal-fragment-header-line = line 2 is the `$fragment` header
refusal-fragment-older = copied from an older ElektraWatt format; open and save that project with this version first
refusal-fragment-kind = a record of kind { $p0 } is not pasted (only devices, functions, representations, connections, junctions, potential definitions, page items and structure boxes)
refusal-fragment-id-twice = the record's ID is listed twice
refusal-fragment-unknown-line = an unknown `$` line
refusal-fragment-no-kind = a line without `$k` is no record
refusal-fragment-no-header = the `$fragment` header is missing
refusal-fragment-header-object = the `$fragment` header is an object
refusal-fragment-header-member = the header has no `{ $p0 }`
refusal-fragment-format-text = `format` is a text
refusal-fragment-format-version = `format` { $p0 } is no version
refusal-fragment-project = `project` is a UUID
refusal-fragment-grid = `grid` is [num, den] of positive integers
refusal-fragment-page-kind = `page_kind` is a text
refusal-fragment-anchor = `anchor` is [x, y] in nm
refusal-fragment-ghost = `ghost` is a list of [x0, y0, x1, y1]
refusal-fragment-free = `free` is a list of indices of `ghost`'s rectangles
refusal-fragment-node-object = `$node` is an object
refusal-fragment-node-members = `$node` has the members id, aspect, path and aliases
refusal-fragment-node-needs = `$node` needs an ID, an aspect and a path
refusal-fragment-node-aliases = `aliases` is a list of texts
refusal-page-copy-page-twice = the page is named twice
refusal-page-copy-grids = the copied pages use different connection grids; copy the pages of each grid on their own
refusal-page-copy-code = { $p0 } is not a structure code
refusal-page-copy-ip-join = the copy's interruption points { $p0 } would join the points they were copied from; give them other names under Rename (for example { $p1 } = { $p1 }.2)
refusal-macro-value-set-none = no value set `{ $p0 }`: the macro has none
refusal-macro-value-set-unknown = no value set `{ $p0 }`: the macro has { $p1 }
# revision.approve, revision.release and project.assets.purge: the refusals of ew-api's own.
refusal-revision-approve-direct = the approval is recorded only by Approve revision… on the saved project, which runs the release gate first
refusal-revision-release-direct = the release is written only by Release revision…, after the release gate, the as-built archive and the git tag
refusal-purge-read-only = the project is open read-only; a purge needs a project open for editing
refusal-purge-quarantined = the project has quarantined lines in { $p0 }; repair them before a purge
refusal-purge-unsaved = the project has unsaved changes; save before a purge
refusal-purge-recovery = a crashed session left unsaved changes in its journal; recover or discard them before a purge
refusal-purge-incomplete = the last save is incomplete; it completes first
refusal-purge-external = the project changed on disk; merge the change first
refusal-purge-stale = { $p0 } of the listed images can no longer be purged; preview again; nothing was removed
# The form for one image.
refusal-purge-stale-one = 1 of the listed images can no longer be purged; preview again; nothing was removed
refusal-purge-unreadable = a file of the project could not be read; nothing was removed
refusal-purge-save = the save that removes the images failed: { $p0 }
refusal-purge-human-only = the purge is human-only: confirm it in ElektraWatt or with ew project assets purge on a terminal; nothing was removed
# Update from library of the damaged library copies: the refusals of ew-api's own.
refusal-update-copies-no-library = no library on this computer holds a readable version of { $p0 }; nothing was written
refusal-update-copies-misfit = the libraries' versions of the damaged copies do not fit the project, which would then not open; nothing was written
refusal-update-copies-locked = another session holds the project; nothing was written
refusal-update-copies-read-only = the project can only be opened read-only, for a reason other than its damaged library copies; nothing was written
refusal-update-copies-findings = the project has other integrity findings, which keep it read-only; nothing was written
refusal-update-copies-save = the update could not be written: { $p0 }
saveerr-read-only = the project is read-only
saveerr-lock-lost = the lock was taken over
saveerr-external = the project changed on disk
saveerr-failed = a file could not be written; nothing changed on disk
saveerr-incomplete = the save is incomplete and completes on the next open
saveerr-pending = an earlier save is still incomplete
saveerr-generation-limit = this project has been saved the greatest number of times its format allows; save it to a new folder (Save as…), which starts the count again
saveerr-other = the project could not be written
props-position-number = Type a valve position, such as 2.
props-position-no-terminal = The valve sits on no valve terminal: place it inside the valve terminal's box, or use Add valve on the valve terminal's menu.
props-supply-no-mark = The point has no supply mark: right-click it and choose Supply comes in here… first.
refusal-layout-length = Type a length in millimetres, such as 158.5.
refusal-variant-no-part = The device has no part with variants.
refusal-variant-unknown = { $p0 } has no variant { $p1 }; choose one of the list.
refusal-end-no-holder = This wire has no connection of its own to hold its end: every connection of it is shared with another wire.
refusal-end-treatment-unknown = { $p0 } is no end treatment; choose ferrule, twin ferrule, ring lug or bare.
refusal-end-part-unknown = The project holds no ferrule or ring lug part { $p0 }; choose one of the list.
refusal-gland-part-unknown = The project holds no gland part { $p0 }: choose a gland with Choose part….
refusal-layout-length-comma = Type a length in millimetres, such as 158,5.
refusal-layout-lengths-two = Type two lengths in millimetres, such as 40, 740.
refusal-layout-hole-row = That row names no hole of the pattern.
refusal-layout-lengths-two-comma = Type two lengths in millimetres, such as 40; 740.
refusal-layout-lengths-three = Type three lengths in millimetres, such as 10, 20, 0.
refusal-layout-lengths-three-comma = Type three lengths in millimetres, such as 10; 20; 0.
refusal-layout-length-above-zero = Type a length above 0 mm.
refusal-layout-tube-no-port = The tube ends at no port to define its medium at.
refusal-layout-device-no-fluid-port = The device has no fluid port to define its medium at.
refusal-layout-medium-choose = Choose one of the project's media.
refusal-layout-placement-gone = The placement is no longer in the project.
refusal-layout-carrier-gone = The carrier is no longer in the project.
refusal-layout-page-gone = The page is no longer in the project.
refusal-layout-choose-flip = Choose Normal or Flipped.
refusal-layout-choose-angle = Choose 0°, 90°, 180° or 270°.
refusal-layout-choose-yes-no = Choose yes or no.
refusal-layout-choose-hinge = Choose Left or Right.
refusal-layout-choose-corner = Choose the corner the holes are measured from, or the project's.
surface-alike = { $p0 } ({ $p1 })
surface-datum-project = The project's: { $p0 }
refusal-layout-choose-labels = Choose Tags, Item numbers, Both or Tags or numbers.
refusal-layout-choose-label-mode = Choose Abbreviated or Full.
refusal-layout-row-not-here = This row cannot be changed here.
refusal-layout-placement-no-part = { $p0 } uses no part { $p1 }; choose one of its parts.
refusal-layout-no-rail-part = The project has no rail part { $p0 }.
refusal-layout-no-duct-part = The project has no duct part { $p0 }.
refusal-layout-surface-gone = That surface no longer exists; choose one of the list.
refusal-layout-no-surface = The project has no surface { $p0 }; choose one of the list.
refusal-layout-surface-many = More than one surface reads { $p0 }; choose one of the list.
refusal-layout-surface-first = Choose the surface first.
find-field-function-text = Function text
find-field-tag = Tag
find-field-property = Property
find-field-title = Title
find-field-description = Description
find-field-process-tag = Process tag
find-field-part-number = Part number
# Why a search cannot run, in words, never the parser's own text.
find-error-regex = The regular expression cannot be used: check its brackets, repeats and escapes, or make it simpler
find-error-too-long = The regular expression is longer than { $p0 } characters
find-error-no-page = No page has the folio { $p0 }

# A cell whose row has no record of the column's kind (a CP-only potential's definition cells).
table-no-record = no record of this kind
table-define-alone = a potential without a definition is defined on its own row alone

# Type or Voltage typed on a potential known only at its CPs, without a name.
table-define-name-first = Name the potential first

# The other read-only reasons of a table cell (`cell_reason`, §8.2's precedence).
table-reason-derived = derived
table-reason-generated = generated
table-reason-no-command = not editable in this table
table-reason-end-outside = made outside the project
table-reason-end-shared = shares every connection with another wire: none of its own holds its end
table-reason-property-grid = edit in the property grid

# A strip-editor or a potential's defining cell in a table round trip.
table-reason-window = edit in the window

# The texts of table mode's pure rule, which `ew-api` hands as keys (`output::table_edit`).
table-same-record = same record as row { $p0 }
table-same-bridge = same bridge as row { $p0 }
table-gesture-edit = edit
table-gesture-fill-down = fill down
table-gesture-pattern-fill = pattern fill
table-gesture-paste = paste
table-gesture-clear = clear

# The word of a navigator's table rows, singular and plural (the Messages pane's rows are findings).
table-row-pages = page
table-rows-pages = pages
table-row-devices = device
table-rows-devices = devices
table-row-connections = connection
table-rows-connections = connections
table-row-strips = terminal
table-rows-strips = terminals
table-row-cables = conductor
table-rows-cables = conductors
table-row-plc = channel
table-rows-plc = channels
table-row-bus = port
table-rows-bus = ports
table-row-tubes = tube
table-rows-tubes = tubes
table-row-media = medium
table-rows-media = media
table-row-potentials = potential
table-rows-potentials = potentials
table-row-ips = interruption point
table-rows-ips = interruption points
table-row-structure = structure node
table-rows-structure = structure nodes
table-row-parts = part
table-rows-parts = parts
table-row-blocks = report block
table-rows-blocks = report blocks
table-row-unplaced = unplaced device
table-rows-unplaced = unplaced devices
table-row-macros = macro
table-rows-macros = macros
table-row-libraries = library item
table-rows-libraries = library items
table-row-messages = finding
table-rows-messages = findings
hint-address-profibus = PROFIBUS DP: the station address, 0 to 126, such as 3; TBD while it is open
hint-address-profinet = PROFINET: the device name, a space and the IP address, such as plc1 192.168.0.1; TBD while it is open
hint-address-ethercat = EtherCAT: pos and the position after the coupler, or alias and the alias, such as pos 2; TBD while it is open
hint-address-io-link = IO-Link: port and the master's port, such as port 3; TBD while it is open
hint-address-other = The address as its bus writes it; TBD while it is open, empty for none
badge-source-missing = Only in this project
badge-deprecated = Deprecated
badge-local-fork = Local fork
reason-part-edit-target = Select a device with a part, or a part in the Libraries navigator
reason-part-no-manufacturer = A part needs a manufacturer
reason-part-no-number = A part needs a part number
reason-part-unit = Choose the unit: piece, m or set
reason-part-unchanged = Nothing to save: the part is unchanged
error-part-tech-value = { $p0 }: { $p1 }
error-part-tech-twice = { $p0 } is given twice
error-part-manufacturer = No manufacturer is called “{ $p0 }”; choose one from the list
error-part-kind = “{ $p0 }” is not a part kind
error-part-category = A category has three levels: category/group/definition
error-part-value = { $p0 }
error-part-field = A part's sheet has no field { $p0 }
# A new part and its bus interfaces, made in the part editor (Edit ▸ New part…).
error-part-bus-interface = An interface needs a name, such as X1 or DP
error-part-bus-system = “{ $p0 }” is no bus system; choose one from the list
error-part-bus-role = “{ $p0 }” is no port role; choose one from the list
error-part-bus-ports = “{ $p0 }” is no number of ports: type a whole number from 1 to { $p1 }, such as 8
error-part-bus-termination = “{ $p0 }” is no state of a termination switch: choose on, off or none
error-part-bus-twice = Interface { $p0 } is listed twice; give each row its own interface or port label
error-part-new-id = No new part could be made: the system gave no random number; try again
error-part-exists = { $p1 } { $p0 } is in My parts or in this project already: open it from the list, or give the new part a part number of its own
error-part-library = { $p0 } could not be written: { $p1 }
error-part-no-my-parts = With no project open a new part is saved in My parts, and My parts cannot be made here: open a project to keep the part in it
reason-part-symbol-unsaved = Save the symbol to a library first; the part then names it
reason-part-not-own = This part is not in your parts library: only a part of your own is taken out of it
# The PLC section of a new part.
error-part-plc-role = “{ $p0 }” is no PLC role; choose station or module
error-part-plc-kind = “{ $p0 }” is no channel kind; choose one from the list
error-part-plc-kind-missing = Choose the kind of the channels
error-part-plc-count = “{ $p0 }” is no number of channels: type a whole number from 1 to { $p1 }, such as 8
error-part-plc-count-missing = Type how many channels the part has, such as 8
error-part-plc-supply = A supply has an L+ pin and an M pin; give both, or neither
error-part-plc-supply-pairs = { $p0 } L+ pins, { $p1 } M pins: give one M pin, or one for each L+ pin
error-part-plc-pin = “{ $p0 }” gives no pin for channel { $p1 }: write the pin of channel n with braces, such as {"{"}n+1{"}"} or X2:{"{"}n+1{"}"}
error-part-plc-pin-twice = { $p0 } would be the pin of two channels; each channel needs its own pin
error-part-plc-slots = “{ $p0 }” is no list of slots: type numbers and ranges, such as 1-8, 10
error-part-plc-max = “{ $p0 }” is no number of modules: type a whole number, such as 32
error-part-plc-kind2-way = The second kind is another than the first: inputs or outputs beside its channels, each of DI, DO, AI, AO or counter
# An image over the pixel limit that an earlier build took in is kept at open, not drawn, and named.
banner-image-not-drawn = The image on folio { $p0 } is { $p1 } × { $p2 } pixels, more than a drawing can show, so it is kept but not drawn; scale it down and add it again.
banner-image-not-drawn-folios = The image on folios { $p0 } is { $p1 } × { $p2 } pixels, more than a drawing can show, so it is kept but not drawn; scale it down and add it again.
banner-logo-not-drawn = The company logo is { $p0 } × { $p1 } pixels, more than a drawing can show, so it is kept but not drawn; scale it down and add it again.
saveerr-external-collisions = the change on disk collides with your edits; choose yours or theirs first
saveerr-external-unreadable = the project on disk changed and cannot be read; your edits are kept
reason-cores-no-wires = Select the wires that become the cable's cores
reason-cores-no-cable = The project has no cable yet; create one first
error-cores-too-few = { $p0 } has { $p1 } free cores for { $p2 } wires: select fewer wires, or choose another cable.
error-cable-type = "{ $p0 }" is not a cable type. Write it as 3G1.5 or 4x0.75, or give a cable part's number.
error-cable-core-unknown = The cable has no core { $p0 }: its cores are { $p1 }.
error-cable-core-twice = Core { $p0 } is given to two wires: give each wire its own core.
symbol-input-limit = The symbol file exceeds the supported size or item limit.
symbol-input-version = This symbol authoring format is not supported.
symbol-invalid-key = Use a key with letters, numbers, dots, hyphens or underscores: { $p0 }.
symbol-key-required = Give the symbol a key.
symbol-unknown-member = The symbol has a field this version does not know: { $p0 }. Remove or rename it before saving.
symbol-export-exists = A file with this name already exists. Choose another name.
symbol-invalid-version = Use a version as major.minor.patch: { $p0 }.
symbol-name-required = Give the symbol a name: { $p0 }.
symbol-provenance-required = Add a licence and source for { $p0 }.
symbol-grid-unsupported = Choose the native 4 mm or QET 10 px grid for { $p0 }.
symbol-cp-off-grid = Connection point { $p0 } is off the connection grid. Move it onto the grid before saving.
symbol-cp-mapping = Connection point { $p0 } needs a unique, matching function connection point.
symbol-cp-direction = Choose an outward direction for connection point { $p0 }.
symbol-slot-pin = The designation text refers to a missing connection point: { $p0 }.
symbol-anchor-slot = The anchor refers to a missing text slot in { $p0 }.
symbol-graphic-invalid = Graphic { $p0 } is empty or uses an unsupported drawing type.
symbol-geometry-limit = The symbol geometry exceeds the supported coordinate or complexity limit.
symbol-slot-arrangement = Text arrangement { $p0 } refers to a missing slot.
symbol-function-changed = The assigned function definition changed. Assign it again before saving.
symbol-function-required = Assign a function definition to this symbol.
symbol-input-object = The symbol document is not valid.
symbol-import-review = Review the imported geometry, connections and source terms before saving.
symbol-graphics-required = Draw at least one graphic before saving.
symbol-link-duplicate = Link point { $p0 } needs a unique name.
symbol-extra-variants = Stored alternative drawings require separate review; fork the required drawing first.
symbol-qet-review = The QElectroTech reader reported conversion limitations. Compare the draft with the source before use.
eplan-input-limit = The EPLAN input exceeds the supported size or complexity.
eplan-archive-path = The EPLAN archive has unsafe or duplicate member names.
eplan-archive-invalid = The EPLAN archive is damaged, encrypted or unsupported.
eplan-archive-method = This archive compression method is unsupported.
eplan-binary-unsupported = This binary EPLAN library or macro is unsupported. Export a PXF XML macro or parts XML from EPLAN.
eplan-xml-invalid = The EPLAN XML is malformed or contains forbidden declarations.
eplan-geometry-invalid = The EPLAN drawing contains invalid coordinates.
eplan-pxf-version = This EPLAN PXF version is unsupported; the reader supports revision 1.23.
eplan-no-symbols = No supported symbol drawing or connection template was found.
eplan-format-unsupported = This EPLAN document type is unsupported.
symbol-import-stopped = The importer stopped on an internal error; nothing was imported.
symbol-function-connections = The function contains an invalid internal connection. Check its pin indices and link kind.
symbol-function-semantics = Unsupported electrical function data: { $p0 }. Choose a supported function or correct its connection data before saving.

eplan-slk-invalid = The symbol-library database is damaged or has an unsupported record layout.
eplan-slk-version = This EPLAN symbol-library database version is not supported.
eplan-slk-encoding = This older SLK file uses an unsupported text encoding. UTF-16 and UTF-8 descriptors are supported.
eplan-slk-companion = Keep the SLK file beside its complete .sdb folder, then open it again. A required drawing table is missing.
eplan-slk-record-unsupported = The symbol-library database contains an unsupported record encoding.
dxf-input-limit = The DXF file exceeds the supported size or complexity.
dxf-input-invalid = The file is no DXF drawing this reader understands.
dxf-no-unit = The drawing names no unit. Choose the unit it is drawn in.
dxf-no-blocks = The drawing has no named block to make a symbol of.
dxf-no-page = The project has no drawn page { $p0 }.
dxf-outline-empty = { $p0 } holds no line to make an outline of.

# A date typed in the project's date format.
refusal-input-date = `{ $p0 }` is not a date: type it as { $p1 }

# The Libraries pane's states and its Update button in words.
library-state-current = Current
library-state-update = Update available ({ $p0 })
error-part-pin-unknown = The part has no pin { $p0 }
error-part-pin-name = Give the pin a name as the device prints it, such as 13 or L+
error-part-pin-role = { $p0 } is no potential a pin can carry
rack-channel-no-device = Nothing is wired to this channel yet: its function text will be that of the device wired to it. Until then the card prints the channel's symbol name in its cell: give it one.
error-cable-section-cores = “{ $p0 }” is no number of cores: type a whole number from 1 to { $p1 }, such as 3
error-cable-section-pe = A cable of one core has no protective conductor beside it: give it two cores or more, or untick the protective conductor
error-cable-section-csa = “{ $p0 }” is no cross-section: type the square millimetres, such as 1.5
error-cable-section-core = Core { $p0 } needs a name of its own (“{ $p1 }”), such as 1 or BN
error-cable-section-colour = Core { $p0 }: “{ $p1 }” is no colour code; type one such as BK, BN or GNYE, or leave it empty
part-misfit-cores = { $p0 } cores, too few for its { $p1 } wires
part-misfit-pe = no protective conductor for its PE wire

# The navigators' group rows (output::navigators): the bus navigator's nets and ports, the PLC navigator's modules without a station and racks, the sections, the fluid groups.
nav-bus-logical = Logical nets
nav-bus-physical = Physical nets
nav-bus-no-net = Ports without a net
nav-bus-undrawn = Ports not drawn yet
nav-bus-segment = Segment { $p0 }
nav-plc-no-cpu = Modules without a station
nav-bridges = Bridges
nav-none = (none)
nav-front-matter = Front matter
nav-unplaced-glands = Glands
nav-unplaced-gland-mount = gland { $p0 }
nav-section = Section { $p0 } - { $p1 }
nav-rack = Rack { $p0 }
nav-fluid-terminals = Valve terminals
nav-fluid-devices = Fluid devices
nav-fluid-media = Media
# An interruption point's direction (output::navigators::ips; the context menu's choice too).
choice-source = Source
choice-target = Target
choice-undirected = Undirected
# The Messages pane's severity groups (output::navigators::messages).
messages-group-error = Errors
messages-group-warning = Warnings
messages-group-info = Infos
# A channel's state in the PLC navigator (output::navigators::plc; the field's choice too).
choice-used = Used
choice-spare = Spare
choice-reserved = Reserved
choice-deactivated = Deactivated
# The words of a choice that a navigator's column also heads by (output::navigators).
choice-bridge-part = Bridge part
choice-bus = Bus
choice-channel = Channel
choice-designation = Designation
choice-device = Device
choice-devices = Devices
choice-folio = Folio
choice-function-text = Function text
choice-library = Library
choice-module = Module
choice-name = Name
choice-pages = Pages
choice-potential = Potential
choice-section = Section
choice-shield = Shield
choice-station = Station
choice-type = Type

# A library's index, tier and access in the Libraries navigator (output::navigators::libraries; the status bar's Read-only too).
lib-index-fresh = Index up to date
lib-index-rebuilt = Index rebuilt
lib-index-failed = Index failed: { $p0 }
lib-notes-between = { "; " }
lib-tier-system = System
lib-tier-community = Community
lib-tier-company = Company
lib-tier-user = User
lib-access-writable = Writable
status-bar-read-only = Read-only

# The names of function kinds by their FnDef category (output::navigators::function_kind), and the spelt-out last entry of a changeover's category.
kind-words-decoration = Decoration
kind-words-overview = Overview
kind-words-black-box = Black box
kind-words-interruption-point = Interruption point
kind-words-external-reference = External reference
kind-words-earth-connection = Earth connection
kind-words-functional-earth-connection = Functional earth connection
kind-words-profibus-connector = PROFIBUS connector
kind-words-profibus-port = PROFIBUS port
kind-words-plug-pin = Plug pin
kind-words-terminal-strip-segment = Terminal strip segment
kind-words-feed-through-terminal = Feed-through terminal
kind-words-pe-terminal = PE terminal
kind-words-1p-mcb = 1P MCB
kind-words-1p-n-mcb = 1P+N MCB
kind-words-1p-manual-switch = 1P manual switch
kind-words-2p-manual-switch = 2P manual switch
kind-words-no-position-switch = NO position switch
kind-words-no-selector-switch = NO selector switch
kind-words-enclosure-heater = Enclosure heater
kind-words-double-acting-cylinder = Double-acting cylinder
kind-words-two-way-process-valve = Two-way process valve
kind-words-5-2-valve = 5/2 valve
kind-segment-changeover = changeover
# A function's kind by the generic rule (output::navigators::function_kind): its two category segments joined
# by kind-generic, each by its kind-segment-<segment> text (a Dutch segment carries its own joint).
kind-generic = { $p0 } { $p1 }
kind-segment-no = NO
kind-segment-nc = NC
kind-segment-1p = 1P
kind-segment-3p = 3P
kind-segment-cabinet = cabinet
kind-segment-fixed = fixed
kind-segment-contact = contact
kind-segment-coil = coil
kind-segment-fuse = fuse
kind-segment-lamp = lamp
kind-segment-motor = motor
kind-segment-pushbutton = pushbutton
kind-segment-resistor = resistor
kind-segment-thermostat = thermostat
kind-segment-transformer = transformer
kind-segment-2p = 2P
kind-segment-a1a2 = A1A2
kind-segment-acdc = AcDc
kind-segment-adjustable = Adjustable
kind-segment-and = And
kind-segment-autodrain = AutoDrain
kind-segment-automatic = Automatic
kind-segment-closedcentre = ClosedCentre
kind-segment-cushioned = Cushioned
kind-segment-cylinder = Cylinder
kind-segment-doubleacting = DoubleActing
kind-segment-exhaustedcentre = ExhaustedCentre
kind-segment-fluid = Fluid
kind-segment-frl = Frl
kind-segment-frldetailed = FrlDetailed
kind-segment-generic = Generic
kind-segment-lever = Lever
kind-segment-leverdetent = LeverDetent
kind-segment-leverspring = LeverSpring
kind-segment-magnetic = Magnetic
kind-segment-manifold = Manifold
kind-segment-manualdrain = ManualDrain
kind-segment-open = Open
kind-segment-or = Or
kind-segment-pnp = PNP
kind-segment-parallel = Parallel
kind-segment-pilotspring = PilotSpring
kind-segment-pressure = Pressure
kind-segment-pressurisedcentre = PressurisedCentre
kind-segment-profinet = Profinet
kind-segment-pushbuttonspring = PushbuttonSpring
kind-segment-quick = Quick
kind-segment-relieving = Relieving
kind-segment-rollerspring = RollerSpring
kind-segment-sensing = Sensing
kind-segment-solenoidsolenoid = SolenoidSolenoid
kind-segment-solenoidspring = SolenoidSpring
kind-segment-solenoidspringno = SolenoidSpringNo
kind-segment-standard = Standard
kind-segment-supply = Supply
kind-segment-bulkhead = bulkhead
kind-segment-busport = busport
kind-segment-check = check
kind-segment-coupling = coupling
kind-segment-distributor = distributor
kind-segment-drain = drain
kind-segment-ejector = ejector
kind-segment-emergencystop = emergencystop
kind-segment-exhaust = exhaust
kind-segment-filter = filter
kind-segment-gauge = gauge
kind-segment-gripper = gripper
kind-segment-ip = ip
kind-segment-lubricator = lubricator
kind-segment-manualswitch = manualswitch
kind-segment-mcb = mcb
kind-segment-oneway = oneway
kind-segment-onoffvalve = onoffvalve
kind-segment-plug = plug
kind-segment-powersupply = powersupply
kind-segment-pressureswitch = pressureswitch
kind-segment-proximitysensor = proximitysensor
kind-segment-quickexhaust = quickexhaust
kind-segment-rcd = rcd
kind-segment-regulator = regulator
kind-segment-reservoir = reservoir
kind-segment-rodless = rodless
kind-segment-semirotary = semirotary
kind-segment-serviceunit = serviceunit
kind-segment-shuttle = shuttle
kind-segment-silencer = silencer
kind-segment-singleacting = singleacting
kind-segment-softstart = softstart
kind-segment-source = source
kind-segment-suctioncup = suctioncup
kind-segment-supplyplate = supplyplate
kind-segment-switch = switch
kind-segment-throttle = throttle
kind-segment-twopressure = twopressure
kind-segment-valve22 = valve22
kind-segment-valve2x32 = valve2x32
kind-segment-valve32 = valve32
kind-segment-valve52 = valve52
kind-segment-valve53 = valve53

# The stamp a print scaled to its paper carries, in the project's language (output::print).
print-stamp-scaled = scaled { $p0 } %

# The bus navigator's column labels (output::navigators::bus).
column-interface = Interface
column-address = Address
column-termination = Termination
column-listed-in = Listed in
column-role = Role
column-logical-net = Logical net
column-physical-net = Physical net
# The browser's ratings column and its connection points, and the tree's function categories, one per first
# level of an FnDef category (the identifier sets' categories, docs/design/07-symbols-parts-macros.md §2.4).
column-rating = Ratings
# The navigators' own column headers (a column of a list kind takes the list's), their fixed badge words, the Messages pane's groups and the accessory kinds.
column-title = Title
column-frame = Frame
column-generated = Generated
column-changed-in = Changed in
column-kind = Kind
column-part = Part
column-colour = Colour
column-strip = Strip
column-pos = Pos
strips-with = with { $p0 }
strips-left-off = left off this strip
strips-accessory = Accessory
table-derived-accessory = it comes with its terminals' part: Delete leaves it off this strip
column-state = State
column-pair = Pair
column-target = Target
column-symbol = Symbol
column-process-tag = Process tag
column-placed-on = Placed on
column-slot = Slot
column-rack = Rack
column-data-type = Data type
column-signal-range = Signal range
column-safety = Safety
column-comment = Comment
column-page-cell = Page cell
column-direction = Direction
column-partner = Partner
column-external = External
column-usage = Usage
column-part-number = Part number
column-first-folio = First folio
column-size = W × H (mm)
column-mount = Mount
column-version = Version
column-variables = Variables
column-tier = Tier
column-licence = Licence
column-access = Access
column-used-by = Used by
column-severity = Severity
column-rule = Rule
column-message = Message
column-object = Object
column-fix = Fix
column-connections-part = Wire part
# Why a part does not fit the device it is chosen for (doc 07 §4.3).
misfit-of = Does not fit { $p0 } of { $p1 }; { $p2 }: { $p3 }
misfit-function = A part of another kind: its function is not the device's
misfit-function-kinds = A part of another kind: { $p0 }, not { $p1 }
misfit-pin = It has no pin { $p0 }; { $p1 }:{ $p0 } is wired
misfit-clamp = Its clamp { $p0 } takes { DECIMAL($p1) } to { DECIMAL($p2) } mm²; wire { $p3 } is { DECIMAL($p4) } mm²
misfit-clamp-unnumbered = Its clamp { $p0 } takes { DECIMAL($p1) } to { DECIMAL($p2) } mm²; a wire on it is { DECIMAL($p3) } mm²

## How a cable's gland and its hole are derived (doc 12 §3.9): a provenance chip's words
via-gland-proposed = proposed
via-gland-proposed-for = proposed for { $p0 } mm
via-gland-no-diameter = the cable gives no outer diameter
via-gland-none-seals = no gland of the stock seals { $p0 } mm
via-gland-hole-on-page = on page { $p0 }
via-gland-hole-placed = placed
via-gland-hole-not-placed = not placed

## What made a hole of a surface, its Made row (doc 12-3d §10.6), and the row's link
hole-made-by-hand = By hand
hole-made-for-cable = For the gland of { $p0 }
hole-made-for-gone-cable = For a cable's gland; the cable is gone
hole-cable-select = Select the cable

## A wire's way as a preview names it (Route's card, the history)
preview-route-way = { $p0 }: a way through { $p1 }, { $p2 }
preview-route-way-plain = { $p0 }: a way of { $p1 }
preview-route-unfinished = { $p0 }: its way is removed, the wire is unfinished
preview-route-replaced = { $p0 }: its old way gives way to its new one

## How a value beside its derived row is had (doc 16 §2.2): the provenance chip's words
via-joined = { $p0 } · { $p1 }
via-connectivity = connectivity
via-potential-definition-at = potential definition at { $p0 }
via-override = override
via-override-cdp = override (CDP)
via-override-on = override on { $p0 }
via-override-on-another = override on another connection
via-no-supply-upstream = no supply found upstream of { $p0 }, so its rating is not used
via-no-supply = no supply found upstream, so no protective device sizes it
via-no-protection = no protective device upstream sizes it
via-frozen = frozen
via-frozen-sign-hidden = frozen (sign hidden)
via-wire-numbering = wire numbering
via-numbering = numbering
via-numbering-scheme = numbering scheme '{ $p0 }'
via-child-of = child of { $p0 }
via-terminal-of-strip = terminal of strip { $p0 }
via-earth-connection = an earth symbol: a connection to earth, no device, so no tag
via-main-representation = main representation
via-strip-groups = strip groups
via-function-definition = function definition
via-shared-stretch = several wires share this stretch: set a wire's own values in the Connections table
via-set-by-hand = set by hand
via-workshop-marking = the project's marking
via-workshop-stock = workshop stock
via-made-outside = made outside the project
via-rule-unverified = rule (unverified)
via-rule-assumed-class = rule (unverified): fine-stranded wire
via-rule-assumed-clamp = rule (unverified): clamp kind not known
via-rule-assumed-both = rule (unverified): fine-stranded wire, clamp kind not known
via-rule-verified = rule
via-rule-verified-assumed-class = rule: fine-stranded wire
via-rule-verified-assumed-clamp = rule: clamp kind not known
via-rule-verified-assumed-both = rule: fine-stranded wire, clamp kind not known
via-project-default = project default
via-project-description = project description
via-project-description-is = project description is '{ $p0 }'
via-cross-reference-format = cross-reference format
via-page = page { $p0 }
via-structure-box-on-page = structure box on page { $p0 }
via-no-terminal = on no valve terminal
via-valve-positions = its valve positions
via-with = with { $p0 }
via-plc-station = its PLC station
via-plc-output = PLC output { $p0 }
via-no-plc-output = no PLC output yet
via-plug = its plug
via-valve-part = the valve's part
via-medium-definition-at = medium defined at { $p0 }
via-through-valves = carried through the valves
via-no-medium = no medium defined
via-no-medium-yet = no medium yet: choose one here
via-port-no-medium = no medium: choose one in the Medium row of its source, the air supply
via-project-default-tube = the project's default tube
via-medium = medium { $p0 }
via-tube-part = tube part { $p0 }
via-no-tube-spec = no default tube, medium tube or tube part
via-primary-part = primary part
via-part = part
bus-symbol-derived-note = Drawn from { $p0 }'s network ports; it follows the part when its ports change.
bus-symbol-own-note = { $p0 }: the program never redraws it; a socket it lacks is named on the part's sheet and by the check.
bus-symbol-program = the program's
bus-symbol-own = your own
bus-symbol-make-own = Make my own from it…
bus-symbol-open = Open the bus symbol
via-layout-label = layout label
via-rail-length = rail length less placements and end brackets
via-path = path
via-stored-ways = stored ways
via-hole-table = hole table
via-hole = H{ $p0 } · { $p1 }
via-hole-tap = { $p0 } · tap M{ $p1 }
via-hole-moved = moved · { $p0 }
via-hole-left-out = left out

## Keep in my parts (doc 07 §5.8)
reason-part-keep = Only a part that is only in this project is kept in My parts.
reason-part-edit-not-yours = This part is not yours to change: make a part of your own from it (Make my own in Parts)
reason-part-edit-mine = This part is in My parts: change it in Parts, where Save gives its next version

## New part…'s first question, What is it? (doc 07 §5.5): its groups, and each start's card with the line of what it
## will be asked for
part-starts-devices = Devices
part-starts-wiring = Terminals, cables and wiring
part-starts-cabinet = Cabinet
part-starts-fluid = Pneumatics
part-start-device = Device with symbols
part-start-device-line = A contactor, a breaker, a relay, a switch, a lamp: drawn as its pieces.
part-start-box = Device drawn as a box
part-start-box-line = A power supply, a drive, a bus device: a box with its pins and ports.
part-start-module = PLC card or station
part-start-module-line = Channels with addresses, on a rack or a bus.
part-start-door = Pushbutton or lamp in a door
part-start-door-line = Drawn with a symbol, mounted in a cut-out.
part-start-terminal = Terminal
part-start-terminal-line = Its levels and clamps, its width.
part-start-cable = Cable
part-start-cable-line = Its cores and its outer diameter.
part-start-wire = Wire
part-start-wire-line = A cross-section and a colour, for the stock.
part-start-ferrule = Ferrule or lug
part-start-ferrule-line = The cross-sections it takes.
part-start-marker = Marker
part-start-marker-line = What it marks and how much text it holds.
part-start-gland = Cable gland
part-start-gland-line = Its thread and the cables it clamps.
part-start-enclosure = Enclosure
part-start-enclosure-line = Its size, its mounting plate, its doors.
part-start-plate = Mounting plate
part-start-plate-line = Its size.
part-start-rail = Rail
part-start-rail-line = Its profile.
part-start-duct = Wiring duct
part-start-duct-line = Its outer and inner size.
part-start-accessory = End bracket or accessory
part-start-accessory-line = An end plate, a cover, a bridge: counted with what it comes with.
part-start-valve = Valve
part-start-valve-line = Its ports and its coil.
part-start-tube = Tube
part-start-tube-line = Its diameters and its pressure.
part-start-fluid = Other pneumatic device
part-start-fluid-line = A cylinder, an air service unit: drawn with a symbol.
# The label file's export profile in a line (doc 13-package §7.3): its form, then its headings.
label-file-summary = { $p0 } · { $p1 }
label-file-xlsx = Excel
label-file-csv-comma = CSV with commas
label-file-csv-semicolon = CSV with semicolons
label-file-csv-other = CSV with { $p0 }
label-file-change = Change…

## The package for the workshop (doc 13-package §8). A `$count` is `one` or `many`, `$n` its number; `$count2` and `$m`
## a second count.
package-member-contents = Contents
package-member-pages = Drawings
package-member-labels = Label files
package-member-drilling = Drawings for drilling
package-form-text = Text
package-form-pdf = PDF
package-form-drilling = PDF, DXF
package-folder-lists = Lists
package-folder-labels = Labels
drilling-folder = Drilling
package-refused = { $count ->
        [one] 1 error stops a release package
       *[many] { $n } errors stop a release package
    }
package-file-name = { $p0 } { $p1 } package.zip
package-file-name-draft = { $p0 } { $p1 } draft package.zip
package-holds-pages = every page in page order, as Export PDF writes them
package-no-holes = No surface of this project has holes yet
package-no-rows = Nothing to list yet
package-no-labels = The project has no labels yet
package-lacks-marker = { $p0 }: no marker part in the stock marks { $p1 }; the label program asks which to print on.
package-lacks-part = { $p0 }: { $p1 }
package-holds-labels = { $count ->
        [one] 1 file
       *[many] { $n } files
    }, { $count2 ->
        [one] 1 label
       *[many] { $m } labels
    }
package-holds-drilling = { $count ->
        [one] 1 surface · 1 template on A4 and 1 DXF
       *[many] { $n } surfaces · { $n } templates on A4 and { $n } DXF
    }
package-holds-wire-list = { $count ->
        [one] 1 wire
       *[many] { $n } wires
    }, { $count2 ->
        [one] 1 bridge
       *[many] { $m } bridges
    }
package-holds-assembly-list = { $count ->
        [one] 1 strip
       *[many] { $n } strips
    }
package-holds-hole-table = { $count ->
        [one] 1 surface
       *[many] { $n } surfaces
    }, { $count2 ->
        [one] 1 hole
       *[many] { $m } holes
    }
package-holds-cut-list = rails, ducts, cables, wires and tubes
package-holds-consumables = wire, ferrules and markers to order
package-holds-rows = { $count ->
        [one] 1 row
       *[many] { $n } rows
    }
package-marks-wires = wire ends
package-marks-terminals = terminals
package-marks-strips = strips
package-marks-cables = cables
package-marks-devices = devices
package-marks-tubes = tubes
package-marks-plates = the plates of the devices in the door
package-draft-mark = Draft { $p0 }: not released
package-draft-mark-none = Draft: no revision
package-drawings-file = { $p0 } drawings.pdf
package-about-pages = Every page of the drawings in page order, the list pages among them.
package-about-labels = { $count ->
        [one] 1 label.
       *[many] { $n } labels.
    }
package-about-labels-no-part = { $count ->
        [one] 1 label
       *[many] { $n } labels
    }; no marker part: choose the marker in the label program.
package-about-dxf = { $count ->
        [one] 1 hole
       *[many] { $n } holes
    }, for a machine; 0,0 at the surface's corner its holes are measured from.
package-about-template = { $count ->
        [one] 1 hole
       *[many] { $n } holes
    } on { $count2 ->
        [one] 1 sheet
       *[many] { $m } sheets
    } of A4, to print at 100 %.
package-contents-file = Contents.txt
package-contents-released-on = Package for the workshop, revision { $p0 }, released on { $p1 }
package-contents-made-on = Package for the workshop, revision { $p0 }, made on { $p1 }
package-contents-made-on-none = Package for the workshop, made on { $p0 }
package-contents-undated = Package for the workshop, revision { $p0 }; the project carries no date
package-contents-undated-none = Package for the workshop; the project carries no date
package-contents-draft = DRAFT: revision { $p0 } is open, not released. Every page of the drawings says "{ $p1 }".
package-contents-draft-none = DRAFT: the project has no revision. Every page of the drawings says "{ $p0 }".
package-contents-release = The package of the release: its pages carry no mark.
package-contents-ready = Before it went to the workshop the project was checked: nothing stood in the way.
package-contents-stood = Before it went to the workshop the project was checked: { $count ->
        [one] 1 thing stood
       *[many] { $n } things stood
    }. An error stops a release; a warning or an info stops nothing.
package-no-length-site = { $count ->
        [one] 1 wire has no length: one end of it is made on site
       *[many] { $n } wires have no length: one end of each is made on site
    }
package-no-length-not-laid-in = { $count ->
        [one] 1 wire of { $cabinet } has no length yet: Route gives it one, or type it
       *[many] { $n } wires of { $cabinet } have no length yet: Route gives them one, or type it
    }
package-no-length-not-laid = { $count ->
        [one] 1 wire has no length yet: type it
       *[many] { $n } wires have no length yet: type them
    }
package-no-length-no-layout = { $count ->
        [one] 1 wire has no length: this project has no panel layout; type it, or lay out the cabinet and Route
       *[many] { $n } wires have no length: this project has no panel layout; type them, or lay out the cabinet and Route
    }
package-no-length-stale = { $count ->
        [one] 1 wire has a length along a way that no longer holds, printed after "≈": Route gives it a new one
       *[many] { $n } wires have a length along a way that no longer holds, printed after "≈": Route gives them new ones
    }
package-contents-no-length = { $p0 } ({ $p1 }).
package-no-length-line = { $p0 }.
package-contents-every-length = Every wire has its length in the wire list; a cable core has its cable's.
package-contents-other-lengths = Every other wire has its length in the wire list; a cable core has its cable's.
package-contents-lacks = What the files lack of the workshop stock and of its parts' data:
package-contents-what = What is in it
package-about-labels-folder = One file per kind of label and marker, for the label program: { $count ->
        [one] 1 label.
       *[many] { $n } labels.
    }
package-about-drilling-folder = Per surface with holes: a template to print at 100 % and lay on it, and a DXF for a machine.
package-about-lists-folder = The workshop lists, each as the list's Excel export.
package-about-manifest = The same list for a program, with a SHA-256 of each file.
package-contents-made-by = Made by ElektraWatt { $p0 }.
# Route's card and `ew route`: the stored ways a Route removes (doc 12-3d §8.5); `$count` is `one` or `many`, `$n`
# its number.
route-ways-removed = { $count ->
        [one] 1 way of a wire { $cabinet } no longer lays is removed: { $wires }
       *[many] { $n } ways of wires { $cabinet } no longer lays are removed: { $wires }
    }
