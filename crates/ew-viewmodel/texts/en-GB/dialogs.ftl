# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in en-GB: dialogs, file prompts, confirmations and the dialogs that make a new thing. One
# message per key; `{ $p0 }`, `{ $p1 }`, ... are the arguments, always strings. The source language and the fallback
# of every other.

prompt-open-image-title = Insert an image
prompt-export-dxf-title = Export DXF: choose a folder for the set
confirm-export-dxf-replace-title = Replace the DXF set in { $p0 }?
confirm-export-dxf-replace-text = { $p0 ->
        [one] The 1 file of the earlier DXF export goes, with every change made to it since; undo does not bring it back
       *[other] The { $p0 } files of the earlier DXF export go, with every change made to them since; undo does not bring them back
    }.
dialog-replace = Replace
dialog-close = Close
dialog-save = Save
dialog-discard = Discard
dialog-cancel = Cancel
dialog-go = Go
confirm-title = Save changes to { $p0 } before closing?
dialog-dont-save = Don't save
confirm-text = The project has unsaved changes.
prompt-open-title = Open a project directory
prompt-save-as-title = Choose a new, empty directory
prompt-export-title = Export PDF
prompt-pdf-name = project.pdf
dialog-apply = Apply
open-failed = The system could not open the file.
dialog-save-workspace = Save layout
save-workspace-title = Save layout as
save-workspace-hint = Workspace name
confirm-journal-title = Discard unsaved changes?
confirm-journal-text = An earlier session of this project ended with unsaved changes. Discarding them deletes that work.
prompt-open-qet-title = Open a QElectroTech project
prompt-open-parts-title = Import parts table
dialog-draw = Draw
prompt-plc-export-title = Export the PLC configuration
prompt-plc-import-title = Import a PLC exchange file
process-tag-scheme-title = Process-tag scheme
prompt-export-table-title = Export table
prompt-table-name = table.xlsx
prompt-open-table-title = Import table
dialog-waive = Waive
new-page-title = New page
new-structure-title = New structure node
new-external-title = New external reference
new-section-title = New section
dialog-create = Create
dialog-export = Export

# The picker's New terminal strip form.
new-strip-title = New terminal strip
dialog-renumber = Renumber
dialog-page-copy = Copy
confirm-purge-title = Purge unused images?
confirm-purge-text = These files are deleted and do not come back by undo.
confirm-purge-more = and { $p0 } more
dialog-purge = Purge
save-macro-title = Save as macro
dialog-save-macro = Save to library
dialog-part-choose-title = Choose a part for { $p0 }

# The lane's own additions beside §9.5 (proposed): the dialogs' targets and buttons, the kind, category and
# value messages, the detail line and the favourite of a project-local part.
dialog-part-choose-button = Choose
confirm-part-remove-title = Remove { $p0 } from your parts library?
confirm-part-remove-text = The next projects no longer offer it, and undo does not bring it back; a project that uses it keeps its copy.
confirm-part-leave-title = Save the changes to { $p0 }?
confirm-part-leave-text = The part has changes that are not saved. Leaving without saving drops them.
confirm-part-card-text = What Save changes in this project waits on the card beside the part. Save applies it before the other part opens; Discard leaves the project as it is.
dialog-stay = Stay
dialog-remove = Remove
prompt-open-keymap-title = Import keymap
prompt-save-keymap-title = Export keymap
prompt-keymap-name = keymap.toml
dialog-approve = Approve
dialog-release = Release
dialog-withdraw = Withdraw
dialog-set-author = Set author
dialog-complete-release = Complete release
dialog-discard-archive = Discard archive
dialog-not-now = Not now
prompt-export-ewz-title = Export .ewz
prompt-ewz-name = project.ewz
prompt-open-ewz-title = Open .ewz
prompt-import-title = Import a QElectroTech project, a drawing, a PDF, EPLAN data or a report, or open a project archive (.ewz)
prompt-import-filter = Projects, archives, drawings, PDFs, EPLAN files and reports
dialog-connect = Connect
prompt-open-symbol-title = Open or import a symbol
prompt-save-symbol-title = Export symbol draft
prompt-symbol-name = symbol.json
prompt-library-folder-title = Choose a symbol library folder
prompt-symbol-filter = Symbol files
prompt-image-filter = Images
prompt-converter-title = Choose the ODA File Converter
dialog-put-station = Put here
prompt-folder-title = Choose the folder the project is made in
new-project-title = New project
new-project-name = Name
new-project-folder = Folder
new-project-browse = Browse…
new-project-made-in = The project is made in { $p0 }
new-project-start-from = Start from
new-project-empty = Empty project
new-project-empty-what = One empty page, with the program's frame and title block and its settings.
new-project-other = Another project…
new-project-other-what = Choose a project to start from: its settings, and its pages only if you tick the box.
new-project-for-new = used for new projects
new-project-copy-pages = { $p0 ->
        [one] Also copy its page and the devices on it
       *[other] Also copy its { $p0 } pages and the devices on them
    }
new-project-pages-note = Without it, the project starts with one empty page.
new-project-default-name = New project
new-project-need-name = Type a name for the project.
new-project-bad-name = A name cannot hold \ / : * ? " < > | and cannot end in a point or a space.
new-project-need-folder = Choose the folder the project is made in: Browse….
new-project-folder-whole = Type the folder's whole path, or choose it with Browse….
new-project-need-other = Choose the project to start from: click Another project… again.
new-project-start-unreadable = That template cannot be read now: choose another start.
new-project-taken = A folder of that name is there already: choose another name.
new-project-other-unreadable = That folder holds no project the program can read: { $p0 }
save-template-title = Save as template
save-template-ok = Save as template
save-template-text = The template keeps this project's settings, frames and title blocks, text styles, sections and structure. Its pages and their devices go in only when you tick the box.
save-template-default-name = { $p0 } template
save-template-failed = The template could not be written: { $p0 }
new-project-other-change = Choose another…
# New cable type… (a cable part's small form) and the cable section of a cable part.
prompt-open-font-title = Choose a font file for the Standard typeface
prompt-open-outline-title = Choose a DXF drawing of the part's front
new-project-folder-new = This folder is not there yet: Create makes it.
new-project-folder-file = A file of that name is in the folder's way: type another folder, or choose one with Browse….
new-project-folder-no-drive = That drive is not there: type a folder on a drive this computer has, or choose one with Browse….
edit-section-title = Edit section
form-field-new-title = New field of your own
dialog-create-field = Create field
form-field-name = Name
form-field-for = One value for
form-field-value = Value now

## Output ▸ Workshop ▸ Rework list… (doc 13 §3.5)

rework-title = Rework list
rework-compare-with = Compare with
rework-archive-file = Archive file…
rework-against = the project as it is now against the archive of { $p0 }
rework-release = { $p0 }, released { $p1 }
rework-export = Export…
rework-work = The work: { $p0 }.
rework-nothing = Nothing to change: the project is as it was released.
rework-not-listed = Not listed: wires that only moved on a page or took another route, and a new wire number.
rework-count-remove-device = { $p0 ->
        [one] 1 device to remove
       *[other] { $p0 } devices to remove
    }
rework-count-remove-terminal = { $p0 ->
        [one] 1 terminal to remove
       *[other] { $p0 } terminals to remove
    }
rework-count-remove-cable = { $p0 ->
        [one] 1 cable to remove
       *[other] { $p0 } cables to remove
    }
rework-count-remark = { $p0 ->
        [one] 1 marker to re-mark
       *[other] { $p0 } markers to re-mark
    }
rework-count-relabel = { $p0 ->
        [one] 1 label to renew
       *[other] { $p0 } labels to renew
    }
rework-count-mount-device = { $p0 ->
        [one] 1 device to mount
       *[other] { $p0 } devices to mount
    }
rework-count-add-terminal = { $p0 ->
        [one] 1 terminal to add
       *[other] { $p0 } terminals to add
    }
rework-count-add-cable = { $p0 ->
        [one] 1 cable to add
       *[other] { $p0 } cables to add
    }
rework-count-pull = { $p0 ->
        [one] 1 wire to pull
       *[other] { $p0 } wires to pull
    }
rework-count-replace = { $p0 ->
        [one] 1 wire to replace
       *[other] { $p0 } wires to replace
    }
rework-count-move-end = { $p0 ->
        [one] 1 end to move
       *[other] { $p0 } ends to move
    }
rework-count-remake-end = { $p0 ->
        [one] 1 end to re-make
       *[other] { $p0 } ends to re-make
    }
rework-count-check-length = { $p0 ->
        [one] 1 length to check
       *[other] { $p0 } lengths to check
    }
rework-count-add = { $p0 ->
        [one] 1 wire to add
       *[other] { $p0 } wires to add
    }
rework-no-release = This project has no released revision yet: the rework list compares with a release. Release it on the Check tab, or choose an archive file.
rework-no-archive = The archive of { $p0 }, { $p1 }, is not in the folder beside the project: choose the archive file.
rework-other-archive = { $p0 } is not the archive of { $p1 }: it is another file under that name. Choose the archive file.
rework-not-opened = The archive does not open: { $p0 }
rework-labels = Labels for these changes…

## Output ▸ Workshop ▸ Labels… (doc 13 §5.4)

labels-title = Labels
labels-which = Labels
labels-all = Every label
labels-since = Changed since { $p0 }
labels-archive = Changed since the archive { $p0 }
labels-profile = Label file
labels-change = Change…
labels-file = File
labels-set = Label set
labels-marker-part = Marker part
labels-count = Labels
labels-misfits = { $p0 ->
        [one] 1 label does not fit its marker part
       *[other] { $p0 } labels do not fit their marker part
    }
labels-export = Export…
labels-show = Show
labels-hide = Hide
labels-misfit = { $p0 } on { $p1 }: { $p2 }
labels-profile-title = Label file
labels-generic = Generic columns
## Output ▸ Workshop ▸ Package… (doc 13-package §8.5)

package-title = Package for the workshop
package-text = One file for the shop: the drawings, the workshop lists, the label files and the drawings for drilling.
package-goes-in = What goes in
package-before = Before it goes to the workshop
package-ready = Ready: nothing stands in the way.
package-stood = { $p0 ->
        [one] 1 thing: { $p1 }.
       *[other] { $p0 } things: { $p1 }.
    }
package-stood-errors = { $p0 ->
        [one] 1 stops a release
       *[other] { $p0 } stop a release
    }
package-stood-warnings = { $p0 ->
        [one] 1 is a warning
       *[other] { $p0 } are warnings
    }
package-stood-infos = { $p0 ->
        [one] 1 is an info
       *[other] { $p0 } are infos
    }
package-check-note = The release check, which the release runs too. An error stops a release package; a warning or an info is shown and stops nothing.
package-stops = stops a release
package-revision = Revision
package-chip-draft = Draft
package-chip-released = Released
package-revision-open = { $p0 } is open, not released.
package-revision-none = The project has no revision.
package-revision-released = { $p0 }, released on { $p1 }.
package-revision-draft-note = The package is a draft: every page of the drawings says "{ $p0 }", and so do its manifest and Contents.
package-revision-released-note = The package is the release's: no page carries a mark, and a second package of { $p0 } is the same file.
package-mark = Draft { $p0 }: not released
package-mark-none = Draft: no revision
package-personal = Personal data
package-personal-check = Without personal data
package-personal-note = Leaves out the name and the address of { $p0 }, marked as personal data. Companies stay.
package-personal-none = No party of this project is marked as personal data (Project properties ▸ Parties)
package-file = File
package-choose = Choose…
package-change = Change…
package-show = Show
package-hide = Hide
package-open-messages = Open Messages
package-write-draft = Write draft package
package-write = Write package
package-replace = Replace
package-exists = { $p0 } exists. Replace it?
package-refused = { $p0 ->
        [one] { $p1 } is released, and 1 error stands now: mend it, which opens a new revision, and write its draft.
       *[other] { $p1 } is released, and { $p0 } errors stand now: mend them, which opens a new revision, and write its draft.
    }
