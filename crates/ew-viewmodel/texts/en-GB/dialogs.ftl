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
prompt-new-project-title = Create the project in a new directory
prompt-basic-project-title = Write the template to a new folder
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
dialog-part-edit-title = Edit part { $p0 }

# The lane's own additions beside §9.5 (proposed): the dialogs' targets and buttons, the kind, category and
# value messages, the detail line and the favourite of a project-local part.
dialog-part-choose-button = Choose
dialog-part-new-title = New part
confirm-part-remove-title = Remove { $p0 } from your parts library?
confirm-part-remove-text = The next projects no longer offer it, and undo does not bring it back; a project that uses it keeps its copy.
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
dialog-cable-type-new-title = New cable type
prompt-open-font-title = Choose a font file for the Standard typeface
new-project-folder-new = This folder is not there yet: Create makes it.
new-project-folder-file = A file of that name is in the folder's way: type another folder, or choose one with Browse….
new-project-folder-no-drive = That drive is not there: type a folder on a drive this computer has, or choose one with Browse….
edit-section-title = Edit section
dialog-bus-cable-new-title = New bus cable
dialog-bus-plug-new-title = New bus plug
