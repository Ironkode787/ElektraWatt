# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in en-GB: toasts, banners, announcements, notes, jobs, recovery and history. One message per
# key; `{ $p0 }`, `{ $p1 }`, ... are the arguments, always strings. The source language and the fallback of every
# other.

announce-place = Place { $p0 }: { $p1 }, variant { $p2 }, at { $p3 }
announce-place-no-cell = Place { $p0 }: { $p1 }, variant { $p2 }
banner-host-commit = { $p0 } failed: { $p1 }
announce-wire-routing = Wire: routing from { $p0 }. Click a connection point; { $p1 } removes the last bend; { $p2 } cancels
toast-tube-no-size = This project has no default tube: Fluid ▸ Fluid settings… sets its size, and each tube's Tube row changes it.
# Show wire numbers on the pages and Number wires by potential: what the switch now is, for the whole project.
toast-wire-numbers-on = Wire numbers now print on every page of the project, in the PDF and in print.
toast-wire-numbers-off = Wire numbers no longer print on any page of the project.
toast-wire-numbering-potential = Every wire of a potential now gets the same number, its potential's, in the whole project.
toast-wire-numbering-own = Every wire now gets its own number again, in the whole project.
toast-wire-numbers-hidden = Switch on Show wire numbers on the pages to see them.
toast-box-pin-part = The pins of a part's box are the part's: they are arranged in Parts, opened now.
toast-box-pin-not-yours = The pins of this box are its part's, and the part is not yours: Make my own from it, then Arrange… to change them.
toast-box-pin-side-full = This side of the box has no free point for another pin; make the box larger or add the pin on another side.
toast-drawn-as-box = { $p0 } has no symbol of its own and is drawn as a box, by the Black box tool: click its first corner, then its opposite corner.
toast-accessory-left-off = Left { $p0 } off { $p1 }. Its terminals' parts still name it for the other strips.
notice-ip-not-placed = Not placed: { $p0 }

# FIXPT41: the drop on a connection point the IP tool cannot honour (an external point on a pin connects).
notice-ip-on-ip = Not placed: the interruption point { $p0 } is already here; place it on a pin or a free grid point
notice-ip-too-close = Not placed: too close to the interruption point { $p0 }, their names and references would print over each other. The two must stand { $p1 } grid steps ({ $p2 } mm) apart, one above the other, or hold Alt to place it anyway
# A point placed free beside a pin without a wire, or on it with Alt, does not join it.
toast-ip-beside-pin = { $p0 } stands beside { $p1 } without joining it: place it on the pin, or draw a wire to it.

# A drop whose connection point lies on a junction or inside a wire, which it would not join.
notice-ip-on-wire = Not placed: the point would sit on a wire without joining it; place it on a pin or a free grid point

# An arrow of EW's two forms on a drawn box's left or right side that would point into the box.
notice-ip-into-box-source = Not placed: a source's wire leaves its arrow at the tail, on the left, so on { $p0 } the arrow would lie in the box. Make it a target, or hold Alt to place it beside the box and draw its wire
notice-ip-into-box-target = Not placed: a target's wire leaves its arrow at the tip, on the right, so on { $p0 } the arrow would lie in the box. Make it a source, or hold Alt to place it beside the box and draw its wire

# H40 item 6: EW-CORE-052 in words: what is in the way (the existing source's page) and the way out.
notice-ip-second-source = Not placed: { $p0 } already has a source on page { $p1 }; press { $p2 } for a target, or mark one source as the star

# Cycle-40 review: the same, when `tool.ip_direction` has no key: the action by its title.
notice-ip-second-source-unbound = Not placed: { $p0 } already has a source on page { $p1 }; use { $p2 } for a target, or mark one source as the star
toast-strip-inserted = { $p0 ->
        [one] Inserted 1 terminal before { $p1 }
       *[other] Inserted { $p0 } terminals before { $p1 }
    }.
toast-strip-appended = { $p0 ->
        [one] Inserted 1 terminal at the end of { $p1 }
       *[other] Inserted { $p0 } terminals at the end of { $p1 }
    }.
toast-strip-moved = { $p0 ->
        [one] Moved { $p1 }
       *[other] Moved { $p0 } terminals of { $p1 }
    }.
toast-accessory-added = Added { $p0 } after { $p1 }.
toast-accessory-added-first = Added { $p0 } at the start of { $p1 }.
toast-accessory-added-before = Added { $p0 } before { $p1 }.
toast-accessories-removed = { $p0 ->
        [one] Removed 1 accessory
       *[other] Removed { $p0 } accessories
    }.
toast-unbridged = { $p0 ->
        [one] Removed 1 bridge
       *[other] Removed { $p0 } bridges
    }.
toast-cores-swapped = Swapped cores { $p0 } and { $p1 } of { $p2 }.
announce-tool = { $p0 }: { $p1 }
announce-selected = { $p0 } selected
# One selected record's live description (tag, kind, folio; a wire's number, potential and ends; a page
# item's kind and text).
announce-device = { $p0 }, { $p1 }, folio { $p2 }
announce-device-unplaced = { $p0 }, { $p1 }
announce-device-connected = { $p0 }, { $p1 }, { $p2 }
announce-device-kind = Device
announce-cps = { $p0 ->
        [one] 1 connection point
       *[other] { $p0 } connection points
    }
announce-wires = { $p0 ->
        [one] 1 wire
       *[other] { $p0 } wires
    }
announce-wire = { $p0 }, { $p1 }, { $p2 } to { $p3 }
announce-wire-no-potential = { $p0 }, { $p1 } to { $p2 }
announce-wire-unnumbered = Unnumbered wire
announce-end-junction = a junction
announce-item = { $p0 }, { $p1 }
announce-item-graphic = Graphic
announce-item-free_text = Text
announce-item-wire_marker = Wire annotation
announce-item-cable_mark = Mark of the cable
announce-item-image = Image
announce-item-surface_view = Surface view
announce-item-unknown = Page item
# A junction, an interruption point and a structure box, and a wire end that is no junction.
announce-junction = Junction, folio { $p0 }, { $p1 }
announce-junction-unplaced = Junction, { $p0 }
announce-ip = Interruption point { $p0 }, folio { $p1 }
announce-ip-unplaced = Interruption point { $p0 }
announce-structure-box = Structure box { $p0 }
announce-structure-box-plain = Structure box
announce-on-folio = { $p0 }, folio { $p1 }
announce-end-point = a connection point
announce-end-unknown = an end of a newer format
job-kind-dxf = DXF export
toast-export-dxf = Exported the DXF set, one file per page, to { $p0 }: { $p1 }, { $p2 } KiB.
toast-export-dxf-again = Exported the DXF set, one file per page, to { $p0 } again: { $p1 }, { $p2 } KiB.
banner-gpu-init = The GPU canvas could not start ({ $p0 }); the canvas draws on the CPU.
banner-gpu-lost = The GPU canvas stopped ({ $p0 }); the canvas draws on the CPU for the rest of this session.
banner-gpu-no-software-adapter = --no-gpu: no software adapter was found, so the window uses { $p0 }; the canvas draws on the CPU.

# Banners and messages.
banner-recovery = This project has unsaved changes from a session that ended unexpectedly. It is open read-only. File, Discard unsaved changes of the earlier session deletes them and opens the project for editing; to recover them, reopen the project and choose Recover in the recovery dialog.
banner-read-only = This project is open read-only: { $p0 }.
# A read-only project whose damaged library copy a device uses.
banner-damaged-copy = This project is open read-only: the device { $p0 } uses the library copy of part { $p1 } from { $p2 }, which is damaged. Update the copy from its library first.
banner-damaged-copy-unnamed = This project is open read-only: the device { $p0 } uses the library copy of its part, which is damaged. Update the copy from its library first.
banner-damaged-copy-record = This project is open read-only: { $p0 } is damaged. Update the copy from its library first.
banner-damaged-copy-left-out = This project is open read-only: { $p0 } is damaged. Update the copy from its library, or use Save as for a copy of the project without it.
# Two or more damaged copies that a Save As leaves out.
banner-damaged-copies-left-out = This project is open read-only: { $p0 } are damaged. Update the copies from their libraries, or use Save as for a copy of the project without them.
# The same banners when no library on this computer holds a damaged copy: no update is offered.
banner-damaged-copy-unnamed-no-library = This project is open read-only: the device { $p0 } uses the library copy of its part, which is damaged. No library on this computer holds it, so the copy cannot be updated from one.
banner-damaged-copy-record-no-library = This project is open read-only: { $p0 } is damaged. No library on this computer holds it, so the copy cannot be updated from one.
banner-damaged-copy-left-out-no-library = This project is open read-only: { $p0 } is damaged. No library on this computer holds it; use Save as for a copy of the project without it.
banner-damaged-copies-left-out-no-library = This project is open read-only: { $p0 } are damaged. No library on this computer holds them; use Save as for a copy of the project without them.
banner-findings = { $p0 ->
        [one] 1 integrity finding was found when the project was opened
       *[other] { $p0 } integrity findings were found when the project was opened
    }.
banner-settings-bad = The settings file could not be read; defaults are used and the old file was kept as { $p0 }.
banner-settings-write = The settings could not be written.
banner-recent-removed = { $p0 } could not be opened and was removed from the recent projects.
banner-dismiss = Dismiss
toast-save-as-left-out = Saved as { $p0 } without { $p1 }, which is damaged.
toast-save-as-left-out-many = Saved as { $p0 } without { $p1 }, which are damaged.
toast-damaged-copies-updated = Updated from library: { $p0 }.
toast-damaged-copies-not-updated = Updated from library: { $p0 }. Not updated, as no library on this computer holds a readable version: { $p1 }.
toast-saved-unreleased-format = Saved in a format that is not released yet. Released versions of ElektraWatt open this project read-only.
banner-no-file-dialog = The file dialog is not available: it needs the xdg-desktop-portal service on the session bus. Start the              portal for your desktop (for example xdg-desktop-portal-gtk or -kde) or open the project as an argument.

# Part 2: banners and errors.
banner-sample = This is the sample project. It is open read-only; Save as makes a copy of your own.
banner-tutorial-copy = A fresh copy of the sample project was made for the tutorials.
banner-imported = Imported { $p0 }: { $p1 }, { $p2 }, { $p3 } in the migration report.
banner-import-no-content = The starter library was not found; symbols the project refers to may be missing.
banner-import-map = { $p0 } beside the file was read ({ $p1 }); the migration report says what each row bound.
banner-journal-discarded = The unsaved changes of the earlier session were discarded. The project is open for editing.
banner-workspace-bad = The workspace file { $p0 } could not be read; the built-in layout is used and the file was left as it is.
banner-workspace-write = The workspace file could not be written.
banner-wizard-done = The template was written to { $p0 }. New projects start from it.
toast-set = Set { $p0 } on { $p1 } { $p2 }.
toast-tag-proposal = Tag { $p0 }: online numbering gave counter { $p1 }.
toast-joined = Joined { $p0 } ({ $p1 }) at { $p2 }.
toast-joined-dropped = { $p0 }; { $p1 }.

# The strip takes its tag with its first terminal; the example is the tag it would take on the page on
# show (`-no-example` off a schematic page).
toast-strip-new = New terminal strip; its first terminal tags it, such as { $p0 }: Draw ▸ Terminal… places its terminals.

# A tag given in the New terminal strip form is the strip's from the start.
toast-strip-new-tagged = New terminal strip { $p0 }: Draw ▸ Terminal… places its terminals.
toast-strip-new-no-example = New terminal strip; its first terminal tags it: Draw ▸ Terminal… places its terminals.
# A terminal chosen in the device browser of a project without strips goes on a new strip.
toast-strip-new-for-terminal = The terminal goes on a new terminal strip: click where it is to stand.
toast-export-pdf = Exported { $p0 }: { $p1 }, { $p2 } KiB, in { $p3 }.

# P-2: a second export of the same file in this session says so, so the toast visibly changes.
toast-export-pdf-again = Exported { $p0 } again: { $p1 }, { $p2 } KiB, in { $p3 }.
toast-structure-new = Created the { $p0 } node { $p1 }.
toast-external-ref-new = Created the external reference { $p0 }; the Interruption point tool links an external point to it by this label.
toast-ip-deleted-wire = Deleted the interruption point { $p0 } and the wire connected to it.
toast-ip-deleted-wires = Deleted the interruption point { $p0 } and the { $p1 } wires connected to it.
toast-ips-deleted-wire = Deleted the interruption points { $p0 } and the wire connected to them.
toast-ips-deleted-wires = Deleted the interruption points { $p0 } and the { $p1 } wires connected to them.
# A pasted interruption point keeps its name, so it continues the points of that name.
toast-paste-ip-continues = The pasted interruption point { $p0 } keeps its name, so it continues { $p0 } at { $p1 }.
toast-paste-fitted = { $p0 ->
        [one] The paste was fitted to this page's { $p1 } grid: one item moved onto it, and the wires were drawn anew. Undo takes it all back in one step
       *[other] The paste was fitted to this page's { $p1 } grid: { $p0 } items moved onto it, and their wires were drawn anew. Undo takes it all back in one step
    }.
toast-page-grid-set = Page { $p0 } now has the { $p1 } grid.
toast-page-grid-fitted-none = Page { $p0 } now has the { $p1 } grid: every item stood on it already, and its wires were drawn anew. Undo takes it all back in one step.
toast-page-grid-fitted = Page { $p0 } now has the { $p1 } grid: { $p2 } items moved onto it and its wires were drawn anew. Undo takes it all back in one step.
toast-wires-unrouted = { $p0 ->
        [one] One wire found no way and is drawn as a thin dashed line: move the items apart and choose Route
       *[other] { $p0 } wires found no way and are drawn as thin dashed lines: move the items apart and choose Route
    }.

# A delete that leaves a device without a symbol keeps the device; the toast says where it stays.
toast-device-unplaced = { $p0 } has no symbol on a page now; it stays in the Devices navigator and the parts list.
toast-device-unplaced-no-part = { $p0 } has no symbol on a page now; it stays in the Devices navigator.
toast-deleted-wires = { $p0 ->
        [one] 1 wire that ended at the deleted symbols was deleted with them
       *[other] { $p0 } wires that ended at the deleted symbols were deleted with them
    }.
toast-cut-terminals = { $p0 ->
        [one] Cut 1 terminal
       *[other] Cut { $p0 } terminals
    }.
toast-cut-symbols = { $p0 ->
        [one] Cut 1 symbol
       *[other] Cut { $p0 } symbols
    }.
toast-cut-wires = { $p0 ->
        [one] 1 wire that ended at them is removed with them; a paste keeps the wires between the cut items only
       *[other] { $p0 } wires that ended at them are removed with them; a paste keeps the wires between the cut items only
    }.
toast-terminals-undrawn = { $p0 ->
        [one] 1 terminal is no longer drawn; it stays in the project and its lists
       *[other] { $p0 } terminals are no longer drawn; they stay in the project and its lists
    }.
toast-devices-unplaced = { $p0 } have no symbol on a page now; they stay in the Devices navigator and the parts list.
toast-devices-unplaced-no-part = { $p0 } have no symbol on a page now; they stay in the Devices navigator.
toast-device-retagged = { $p0 } (now { $p1 })
toast-placed-connected = { $p0 ->
        [one] { $p1 } and connected { $p3 } to { $p4 }
       *[other] { $p1 } and connected { $p2 }: { $p3 } to { $p4 } and { $p5 } more
    }.
toast-place-without-wires = Alt+Click or { $p0 } places without connecting.
toast-placed = Placed { $p0 } at { $p1 }.
toast-placed-no-cell = Placed { $p0 }.
toast-one-symbol = { $p0 } is drawn as one symbol.
toast-one-symbol-many = { $p0 } devices are drawn as one symbol each.
toast-retagged = { $p0 }: a device takes its tag from the page that draws its own box, not from a bus page, which shows its interfaces.
toast-paste-retagged = { $p0 }: a tag follows the page its device is drawn on, and the paste put the cut devices back on this one.
toast-paste-notices = The paste: { $p0 }.
toast-paste-moved-terminals = Moved { $p0 } terminals of { $p1 } to { $p2 }.
toast-paste-moved-terminal = Moved 1 terminal of { $p1 } to { $p2 }.
toast-paste-moved = Moved { $p0 } symbols to { $p2 }.
toast-paste-moved-symbol = Moved 1 symbol to { $p2 }.
toast-placed-in-wire = { $p0 }, connected into the wire it was dropped on.
# The end of the toast of a terminal placed where a terminal of its strip stands ({0} the strip, {1} the grid steps).
toast-placed-moved = { $p1 } grid steps right: { $p0 } has a terminal on the click point.
# A placement's result ({0}) and where and why a terminal left the click point ({1}), each without its full stop.
toast-placed-and-moved = { $p0 }, { $p1 }.

# FIXPT41: an interruption point dropped on a pin ({3} the key of `canvas.activate_alt`).
toast-ip-connected = Placed { $p0 } on { $p1 }. Alt+Click or { $p2 } places it free.
toast-ip-external-connected = Placed the external point { $p0 } on { $p1 }. Alt+Click or { $p2 } places it free.
toast-ip-placed-source = { $p0 } at { $p1 } goes on at { $p2 }.
toast-ip-placed-source-no-cell = { $p0 } goes on at { $p2 }.
toast-ip-placed-source-lone = { $p0 } at { $p1 } has no target yet: place one where the potential arrives.
toast-ip-placed-source-lone-no-cell = { $p0 } has no target yet: place one.
toast-ip-placed-target = { $p0 } at { $p1 } continues from { $p2 }.
toast-ip-placed-target-no-cell = { $p0 } continues from { $p2 }.
toast-ip-placed-target-lone = { $p0 } at { $p1 } has no source yet: place one where the potential leaves, or right-click it (Supply comes in here…).
toast-ip-placed-target-lone-no-cell = { $p0 } has no source yet: place one, or right-click it (Supply comes in here…).
toast-undo = Undo
toast-close = Close
# A toast that says again what the visible toast of its key says, with the times in a row (it ran again).
note-said-again = { $p0 } ({ $p1 }×)

# The jobs, the notifications and the recovery dialog.
toast-open = Open
toast-export-cancelled = Export cancelled; no file written.
toast-export-failed = Export failed: { $p0 }; check that the folder is writable and the file is not open in another program.
toast-export-internal = Export failed: an internal error.
toast-recovered = { $p0 ->
        [one] Recovered 1 change; save to keep it
       *[other] Recovered { $p0 } changes; save to keep them
    }.

# The counted outcomes.
toast-bridged = { $p0 ->
        [one] Added 1 bridge between { $p1 } terminals
       *[other] Added { $p0 } bridges between { $p1 } terminals
    }.
toast-check-door-closes = The door of { $p0 } closes over what stands behind it.
toast-check = Check: { $p0 }.
toast-marker-count = { $p0 ->
        [one] 1 wire carries an annotation
       *[other] { $p0 } wires carry an annotation
    }.
job-kind-pdf = PDF export
job-kind-print = Print set
toast-print-printed = Sent { $p0 } to the printer.
toast-print-opened = Opened { $p0 } in your PDF viewer.
toast-print-fallback-printer = Printing through the print dialog failed, so { $p0 } went to the default printer.
toast-print-fallback-viewer = Printing could not start, so { $p0 } opened in your PDF viewer to print from there.
toast-print-failed = Printing failed: { $p0 }. Export a PDF and print it from your PDF viewer.
toast-print-cancel-late-printer = The printer already had { $p0 } when you cancelled; cancel it in the system's print queue.
toast-print-cancel-late-viewer = { $p0 } was already open in your PDF viewer when you cancelled; close it there.
job-cause-in-use = the file is in use
job-cause-no-folder = the folder does not exist
job-cause-no-file = the file does not exist
job-cause-denied = access to the folder or the file was denied
job-cause-full = the disk is full
job-cause-file-name = a file's name, made from the project's codes, holds a / or a \ or another character a file name cannot hold
job-cause-internal = an internal error
job-phase-building = building pages
job-phase-drawing = drawing pages
job-phase-writing = writing the file
job-as-of = as of rev { $p0 }
job-progress = page { $p0 } of { $p1 }
job-elapsed = { $p0 } s
job-cancel = Cancel
job-done = { $p0 ->
        [one] 1 page, { $p1 } KiB
       *[other] { $p0 } pages, { $p1 } KiB
    }
job-now = exported rev { $p0 }; the model is now at rev { $p1 }
job-cancelled = cancelled; no file written
job-failed = failed: { $p0 }
notifications-title = Notifications
notifications-empty = No notifications
recovery-title = Recover unsaved changes?
recovery-text = { $p0 } has unsaved changes from a session that ended unexpectedly.
recovery-count = Unsaved changes: { $p0 }
recovery-last-change = Last change: { $p0 }
recovery-last-change-utc = { $p0 } UTC
elapsed-just-now = just now
elapsed-minutes = { $p0 ->
        [one] 1 minute ago
       *[other] { $p0 } minutes ago
    }
elapsed-hours = { $p0 ->
        [one] 1 hour ago
       *[other] { $p0 } hours ago
    }
elapsed-days = { $p0 ->
        [one] 1 day ago
       *[other] { $p0 } days ago
    }
recovery-moved = The project changed on disk since.
recovery-discard = Discard…
recovery-failed-moved = the project changed on disk in the same places; recovery needs the merge panel
recovery-failed-replay = the journal could not be replayed
recovery-failed = Recover failed: { $p0 }
toast-dismiss = Dismiss
toast-name = Notification
announce-copied = Copied { $p0 } records
toast-copied = { $p0 ->
        [one] Copied 1 item
       *[other] Copied { $p0 } items
    }. { $p1 } hangs the copy on the pointer; a click puts it down.
banner-paste-refused = Not pasted: { $p0 }
notices-title = Third-party notices
notices-generated-missing = The notices of the Rust crates are generated for every release (THIRD-PARTY-NOTICES.txt next to the application); this build has none.
toast-valve-added = Added { $p0 } at position { $p1 } of { $p2 }; click where its symbol goes.
toast-seat-valve = { $p0 } is the valve at position { $p1 } of { $p2 } now.
toast-seat-plate = { $p0 } is a supply plate of { $p1 } now.
toast-seat-full = The valve terminal has no free valve position: { $p0 } stays a valve of its own.
toast-seat-own = { $p0 } is no valve for a valve terminal (it has a coil of its own to wire, or is no valve): it stays a device of its own.
toast-position-held = Position { $p0 } of { $p1 } holds { $p2 }: choose Swap to swap the two.
toast-solenoid-draw = Click where solenoid { $p0 } of { $p1 } goes on page { $p2 }.
toast-zoom-way-back = Zoomed far in: { $p0 }, or Fit page at the page's lower right, shows the whole page again.
toast-plug-draw = Click where the plug of { $p0 } goes on page { $p1 }.
toast-ports-plugged = Plugged the open working ports of { $p0 }.

# The words say what to do.
toast-bind-off-card = { $p0 } is not on the module { $p1 }: wire { $p0 } to a pin of { $p1 } first; the channel binds the module's pin.
toast-bind-off-card-untagged = The connection point is not on the module { $p0 }: a channel binds a pin of its module or the coil of a valve on it.
toast-bind-wired-pins = { $p0 } is wired to { $p2 } pins of { $p1 } ({ $p3 }): drop the channel on the module's pin you mean.
toast-bind-no-position = { $p0 } cannot go on the valve terminal { $p1 }: no valve position is free, or { $p0 } sits on another assembly.
toast-bind-done = Bound { $p0 } to { $p1 }.
toast-bind-wired = Bound { $p0 } to { $p1 }, where { $p2 } is wired.
toast-bind-attached = Attached { $p0 } to { $p1 } at position { $p2 } and bound { $p3 } to { $p4 }.
toast-cards-attached = Attached { $p0 } to { $p1 }.
toast-card-waits = { $p0 } waits for a station: drag it onto its station in the PLC panel, or right-click it there under Modules without a station and choose Attach to.
toast-card-waits-cpu = { $p0 } waits for a station: make one with New station…, and the module goes into its rack.
toast-plc-export = Exported { $p0 }: { $p1 }, { $p2 }, { $p3 }, { $p4 }.
toast-plc-import = Imported { $p0 }.
toast-medium-new = Added the medium { $p0 }.
toast-line-medium = The tube carries { $p0 } now.
toast-fluid-settings = The default tube is { $p0 } now: every tube without one of its own takes it, the tubes drawn already too.
toast-fluid-settings-no-material = The default tube is { $p0 } now, without a material: every tube without one of its own takes it, the tubes drawn already too; give it a material to complete their specification.
toast-fluid-settings-no-size = The project has no default tube size now: each tube takes its medium's or its own.

# A channel the bind unbound (FR-PLC-005), appended to the bind's toast.
toast-bind-displaced = ; { $p0 } was bound there and is now unbound
job-kind-table = Table export
job-kind-import = Drawing import
job-kind-read = Import read
job-done-table = { $p0 } KiB
job-kind-package = Package
job-done-package = { $p0 ->
        [one] 1 file
       *[other] { $p0 } files
    }
toast-package-draft = { $p0 ->
        [one] Wrote the draft package { $p1 }: 1 file
       *[other] Wrote the draft package { $p1 }: { $p0 } files
    }.
toast-package = { $p0 ->
        [one] Wrote the package { $p1 }: 1 file
       *[other] Wrote the package { $p1 }: { $p0 } files
    }.
toast-export-table = Exported { $p0 }: { $p1 } KiB, in { $p2 }.
toast-export-table-again = Exported { $p0 } again: { $p1 } KiB, in { $p2 }.
toast-table-imported = { $p0 ->
        [one] Imported 1 changed cell from { $p1 }
       *[other] Imported { $p0 } changed cells from { $p1 }
    }.
# The toast of one cell, the refusal with several tables shown and none focused, and the link preview's line
# when `nav.follow` has no key (the command by its menu path, as the menu table holds it).
toast-check-qualified = Check · { $p1 }: { $p0 }.
toast-section-new = Section { $p0 } { $p1 } made, folios { $p2 } to { $p3 }. A new page with a folio in that range joins it.
toast-page-new-section = Page { $p0 } is made: { $p1 }.
history-header = { $p0 ->
        [one] { $p0 } step · cursor at { $p1 }
       *[other] { $p0 } steps · cursor at { $p1 }
    }

# One step in the singular.
history-empty = Nothing to undo yet
history-not-undoable = not undoable
history-changes-title = Changes of step { $p0 }
history-redo = undone
# The text protocol's answer to a step that changed the project: the step's title and its pages.
history-changed-on = { $p0 } on { $p1 }
# What an Undo or a Redo took back or did again, and where.
history-undone = Undone: { $p0 }
history-redone = Redone: { $p0 }
note-action-go-to-page = Go to page
note-action-show-messages = Show messages
note-action-wheel-scrolls = Wheel scrolls the page
note-action-route-his = Route these too…
history-redo-to = Redo to here
# An entry's origin in words.
history-origin-gui = by hand
history-origin-from = from { $p0 }
history-origin-external = changed on disk
history-origin-import = import
history-origin-merge = merge
history-origin-fix = quick fix { $p0 }
toast-pages-copied = { $p0 ->
        [one] Copied 1 page: { $p1 }
       *[other] Copied { $p0 } pages: { $p1 }
    }.
toast-pages-copied-bus = The copies of { $p0 } join no bus yet: draw their bus line on a bus page and give each its address.
history-header-kept = oldest kept: step { $p0 } · { $p1 } MB of { $p2 } MB
note-macro-saved = Saved { $p0 } { $p1 } into { $p2 }
announce-variable = { $p0 } is a variable of the next macro
toast-part-new = Part { $p0 } is in the project.
toast-part-new-library = Part { $p0 } is in the project and in { $p1 }.
toast-part-new-library-with = Part { $p0 } is in the project and in { $p1 }, kept with { $p2 }.
toast-part-kept = { $p0 } is kept in My parts.
toast-part-kept-with = { $p0 } is kept in My parts with { $p1 }.
banner-keymap-refused = The keymap file was refused: { $p0 }. The { $p1 } preset applies; the file was kept as { $p2 }.
banner-keymap-write = The keymap could not be written.
banner-keymap-unread = The keymap file could not be read ({ $p0 }); the { $p1 } preset applies and the file is left as it is.
banner-keymap-refused-kept = The keymap file was refused: { $p0 }. The { $p1 } preset applies; the file is left as it is because it could not be copied aside.
banner-keymap-kept = The keymap applies for this session but was not written: keymap.toml is left as it is.
toast-revision-approved = Approved revision { $p0 }; saved.
toast-revision-released = Released revision { $p0 } as { $p1 }.
toast-revision-withdrawn = Withdrew revision { $p0 }.
toast-revision-author = The { $p1 } is now the author of { $p0 }.
toast-revision-discarded = Discarded the incomplete release of { $p0 }.
toast-release-troubles = The release left housekeeping warnings ({ $p0 }): see Messages.
toast-bus-line-no-cable = Bus line drawn: it has no cable yet.
note-action-choose-cable = Choose cable…
toast-locate-devices = Gave { $p0 } the location { $p1 }; Unplaced devices now lists what to drag onto the plate.
toast-legend-tags = Legend added. The devices show their tags, not the numbers it lists.
toast-devices-location = { $p0 ->
        [one] Gave the device the location { $p1 }
       *[other] Gave { $p0 } devices the location { $p1 }
    }.
toast-auto-fill = Placed { $p0 } on { $p1 }.
toast-auto-fill-left = Placed { $p0 } on { $p1 }; { $p2 }.
# Project ▸ Enclosure…'s toast: the part's words and the location (+K1).
toast-enclosure-new = Created the enclosure { $p0 } at { $p1 }.
toast-enclosure-new-shown = Created the enclosure { $p0 } at { $p1 }; the page shows its mounting plate.
toast-enclosure-drawn = Drew { $p0 } of { $p1 } on page { $p2 }.
toast-enclosure-new-page = Created the enclosure { $p0 } at { $p1 } and drew it on the new panel layout page { $p2 }.
toast-convert = Convert to connection
toast-show-block = Show report block
note-action-select-unlisted = Select them
note-action-connect = Connect
note-action-remove-undrawn = { $p0 ->
        [one] Remove it from the project
       *[other] Remove them from the project
    }
note-action-keep-tag = Keep { $p0 }
note-action-keep-tags = Keep the old tags
toast-connect-offer = { $p0 } faces { $p1 }, further off or across other wires, so it is not connected: Connect draws the wire.
toast-show-changes = Show changes
toast-external = External change applied: { $p0 }.
toast-external-folios = External change applied: { $p0 } on folios { $p1 }.
toast-external-folio = External change applied: { $p0 } on folio { $p1 }.
toast-external-git = {" "}(git { $p0 })
banner-external-collisions = The project changed on disk, and the change collides with your edits. Saving waits until you choose yours or theirs for each field.
banner-external-unreadable = The project on disk changed and cannot be read; your edits are kept. Save a copy, or wait until the disk is complete.
banner-external-choose = Choose…
toast-external-kept = Your version was kept: { $p0 }.
toast-external-same = The change on disk equals your version: nothing to take.
banner-older-format = This project is in an older format; saving converts it.
banner-cloud = This project is in a cloud-synced folder ({ $p0 }). Two people can open it within the sync delay; use git or a network share for teams.
banner-cloud-other = This project is in a cloud-synced folder. Two people can open it within the sync delay; use git or a network share for teams.
toast-ewz = Exported { $p0 }.
toast-ewz-unsaved = Exported { $p0 } with the saved state; the unsaved changes are not in it.
toast-git-setup = Merge driver set up.
banner-reimported = Re-imported { $p0 }.

# The banner while the import job runs.
banner-importing = Importing { $p0 }…
toast-cable-new = Created cable { $p0 } with { $p1 }.
toast-cable-new-cores = { $p0 ->
        [one] 1 core
       *[other] { $p0 } cores
    }
toast-cores-connected = Wires connected to the cores of { $p1 }: { $p0 }.
toast-cable-wires = Cable { $p0 } made from { $p1 }.
toast-cable-wires-changed = Cable { $p0 } now carries { $p1 }.
toast-bus-cable = Cable { $p0 } carries { $p1 }.
toast-report-block-new = Added the report block { $p0 } from folio { $p1 }.
toast-report-block-empty = Added the report block { $p0 } from folio { $p1 }; it has nothing to list yet, so it makes no page. { $p2 }.
toast-report-block-had = Added the report block { $p0 } from folio { $p1 }. The set already had one on page { $p2 }: go to it, or undo this one.
toast-parts-unlisted = { $p0 ->
        [one] 1 device has no part and is not in the parts list. Select it, then Edit ▸ Choose part… gives it one, or Type not known yet… lists it by a description until it is known
       *[other] { $p0 } devices have no part and are not in the parts list. Select them, then Edit ▸ Choose part… gives them one, or Type not known yet… lists them by a description until it is known
    }.
toast-parts-list-opened = Opened the set's parts list on page { $p0 }; it makes no second one. Report… on the Output tab adds another list.
toast-list-opened = Opened the { $p0 } on page { $p1 }.
toast-rework-written = Wrote the rework list to { $p0 }.
toast-revision-opened = This change opens revision { $p0 }: { $p1 } stays as released.
toast-list-written = Wrote the list to { $p0 }.
toast-labels-written = { $p0 ->
        [one] Wrote 1 label file to { $p1 }
       *[other] Wrote { $p0 } label files to { $p1 }
    }.
toast-label-columns = The label files take the columns and the form you chose.
toast-back-to-editor = Back to the drawing
toast-route-done = { $p0 ->
        [one] Routed 1 wire: { $p1 }
       *[other] Routed { $p0 } wires: { $p1 }
    }.
toast-route-applied = { $p0 ->
        [one] Routed 1 wire on { $p1 }
       *[other] Routed { $p0 } wires on { $p1 }
    }.
toast-route-nothing = Nothing to route: no wire would run better.
toast-wire-error = The wire makes an error: { $p0 }.
toast-wire-to-give = Still to be given: { $p0 }.
toast-route-done-selected = { $p0 ->
        [one] Routed 1 selected wire: { $p1 }
       *[other] Routed { $p0 } selected wires: { $p1 }
    }.
toast-route-nothing-selected = { $p0 ->
        [one] Nothing to route in the 1 selected item: no wire would run better
       *[other] Nothing to route among the { $p0 } selected items: no wire would run better
    }.
toast-route-kept = { $p0 ->
        [one] Nothing changed: 1 wire keeps the route it has
       *[other] Nothing changed: { $p0 } wires keep the routes they have
    }.
toast-lists-frame = Every list of the project, the table of contents among them, now prints on { $p0 }; the frames of the drawn pages and the section dividers stay as they were.
toast-drawing-imported = Imported { $p0 } as page { $p1 }.
toast-underlay-placed = Placed { $p0 } under page { $p1 }.
toast-parts-set-saved = { $p0 } saved to My parts: { $p1 } new, { $p2 } changed.
toast-parts-set-saved-map-not = { $p0 } saved to My parts: { $p1 } new, { $p2 } changed. Its map of the columns was not kept ({ $p3 }), so the next table of this source is mapped anew.
toast-eplan-report-imported = Imported { $p0 } as a new project with { $p1 } devices.
# The project changed while a report was read, and Cancel kept it.
toast-report-read-dropped = The report was not imported: the project changed while it was read and stays open; import the report again to make its project.
toast-plc-freeze = Froze { $p0 } of { $p1 }; spare channels keep their automatic addresses.
toast-plc-readdress = Re-addressed: { $p0 } moved.
toast-placed-positions-empty = { $p0 }: { $p1 } positions, all empty.
toast-placed-positions = { $p0 }: { $p1 } positions, { $p2 } of them empty.
toast-terminal-undrawn = The project's valve terminal { $p0 } is drawn on no fluid page yet: Fluid ▸ Valve terminal draws it here.
toast-terminal-draw = Click where { $p0 } goes on page { $p1 }: it is drawn as its positions, its valves in their cells.
toast-terminal-valves-drawn = The valves of { $p0 } stand in its cells, its supply ports 1, 3 and 5 at its left end.
toast-fluid-page-made = Fluid page { $p0 } made for it.
toast-card-joined = { $p0 } joined { $p1 } in slot { $p2 }.
toast-station-on-bus-page = { $p0 } is drawn on this bus page; Rack… fills its slots.
toast-show-folder = Show in folder
toast-tutorial-here = The tutorial's practice project is under Tutorials in your projects folder.
toast-tutorial-second-window = The tutorial's practice project is under Tutorials in your projects folder; it opens in a second window, and your project stays open here.
banner-second-window-failed = The second window did not start: open the practice project with File ▸ Open recent.
toast-layout-overruns = At 1:{ $p0 } the layout runs over the frame's drawing area. Fit the layout to the sheet draws it at 1:{ $p1 }, centred.
toast-legend-own-page = The legend does not fit beside the layout whole, so it has a page of its own: { $p0 }.
toast-legend-moved = The legend is on a page of its own now: { $p0 }.
toast-kept-in-place = { $p0 ->
        [one] Kept in place: { $p1 }. Arrange and route arranges the other symbols around it; you can still move it yourself
       *[other] Kept in place: { $p1 }. Arrange and route arranges the other symbols around them; you can still move them yourself
    }.
toast-let-go = No longer kept in place: { $p0 }.
toast-plc-joined = { $p0 } belongs to { $p1 } now.
toast-guide-page = The guide's page: { $p0 }.
toast-fix-all = { $p0 } findings fixed.
toast-fix-all-fix-mark-cp-unused = { $p0 } pins marked not used.
toast-fix-all-left = { $p0 } fixes are left out: what they change has changed since the check.
toast-card-redrawn = { $p0 } is drawn in one row: Arrange and route draws its wires to the new pins.
toast-card-terminals-faced = The terminals wired to { $p0 } take its wire on their pin that faces the card: Arrange and route draws the wires.
toast-rack-own-pages = Each card is drawn on a page of its own: { $p0 }.
toast-module-moved-new-page = { $p0 } stands on the new page { $p1 }.
banner-template-saved = The template "{ $p0 }" is made and in the list of templates.
banner-template-saved-used = The template "{ $p0 }" is made. New projects start from it.
banner-wizard-done-listed = The template was written to { $p0 } and is in the list of templates.
banner-check-level-set = The check level is now "{ $p0 }".
toast-arrange-applied = Arranged { $p0 }: { $p1 }, { $p2 }.
toast-arrange-nothing = Nothing to arrange: the page is already arranged as well as the rules allow.
toast-arrange-none-left = Nothing is left to apply: every change of this preview is unticked.
toast-list-columns = The list { $p0 } prints its columns as chosen.
toast-legend-columns = The legend on { $p0 } prints its columns as chosen.
toast-list-rows = { $p0 ->
        [one] The list { $p1 } holds 1 row of your own
       *[other] The list { $p1 } holds { $p0 } rows of your own
    }.
toast-list-rows-none = The list { $p0 } holds no rows of your own.
toast-drilling-written = { $p0 ->
        [one] Wrote 1 file to { $p1 }: { $p2 } for { $p3 }
       *[other] Wrote { $p0 } files to { $p1 }: { $p2 } for { $p3 }
    }.
toast-cabinet-routed = { $p0 ->
        [one] Routed 1 wire of { $p1 }
       *[other] Routed { $p0 } wires of { $p1 }
    }.
toast-cabinet-gone = { $p0 ->
        [one] Removed 1 way of a wire { $p1 } no longer lays
       *[other] Removed { $p0 } ways of wires { $p1 } no longer lays
    }.
toast-cabinet-all-hold = Every wire of { $p0 } has its way.
toast-layout-turned = Turned { $p0 }.
toast-layout-moved = Moved { $p0 }.
toast-ways-stale = { $p0 ->
        [one] The way of 1 wire no longer holds
       *[other] The ways of { $p0 } wires no longer hold
    }.
