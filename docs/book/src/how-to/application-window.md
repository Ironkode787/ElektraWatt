# Use the application window

This guide describes the desktop application `elektrawatt`: what is on the start page, how to make or open a project,
how to arrange the window, and where F1 leads. Every action of the window is an action of one registry, so the same
names work in the menu, in the command palette (`Ctrl+K`) and in a headless UI script
(`elektrawatt --headless --script`).

## Start page

With no project open the window shows the start page. It has no news feed and opens no connection.

- **New project** makes a project in a new, empty directory from your company basic project, or from the built-in basic
  project when you have none.
- **Open...** opens a project directory; the recent projects are listed below it.
- **Open sample project** opens the CC0 sample project read-only; use **Save as...** to make your own copy.
- **Import QElectroTech project** reads a `.qet` file into a new project directory.
- **Create company standard** starts the [company-standard wizard](#company-standard-wizard).
- **Tutorials** opens the tutorials and a fresh copy of the sample project.
- The quick switches set the language and the keymap preset. The privacy line says whether the update check is on; it
  is off until you switch it on in the [first-run setup](#first-run-setup).

## Project view

With a project open the window shows the page list, the page view and the status bar. The page view shows the current
page read-only. `Page Down` and `Page Up` move between pages, `Ctrl+G` goes to a folio, `Ctrl+Z` and `Ctrl+Shift+Z`
undo and redo, `Ctrl+S` saves and `Ctrl+Shift+S` saves under a new name.

## New project

**File, New project** (`Ctrl+N`) asks for a new, empty directory and writes the project there at once. The project
starts from the default basic project (a company basic project made by the wizard, or the built-in one) with new
identities for every record.

## Sample project

**File, Open sample project** opens the sample project read-only, so that nothing is written into the installation.
**Save as...** writes a copy into a directory of your own and continues in the copy.

## Import a QElectroTech project

**File, Import QElectroTech project...** asks for the `.qet` file and then for a new, empty directory. The importer
reads the file into a new project and writes it to the directory; a banner counts the pages and the findings of the
migration report.

## Unsaved changes and recovery

Closing, quitting or opening another project with unsaved changes asks whether to save, discard or cancel. If the
application ends while a project has unsaved changes, the next open finds the journal of that session and opens the
project read-only with a banner. **File, Discard unsaved changes of the earlier session** deletes the journal after a
confirmation and opens the project for editing.

## Command palette

`Ctrl+K` (or `Ctrl+Shift+P`) opens the palette: type a part of a name and press `Enter` to run the selected entry.
Disabled entries are listed last with the reason. `Escape` closes it.

## Keyboard shortcuts

**Help, Keyboard shortcuts** lists the bindings of the EW preset. `F1` opens this guide at the page of what has the
focus: an open dialog, the palette, the wizard, the project view or the start page. Over a menu item with a shortcut
the tooltip ends with "F1: learn more"; `F1` while the pointer is on the item opens the page of that item.

## Workspaces

The project view is a dock: drag the border between two panes to resize them, and drag a pane's title onto another
pane to rearrange or tab it. The built-in workspace is **Schematic**. **View, Workspace** lists the saved workspaces,
saves the current layout under a name and resets the layout. A workspace is a file `workspaces/<name>.json` in the
configuration directory; the layout is saved a second after its last change.

## First-run setup

On the first start the application asks for the language, what you come from (EPLAN, QElectroTech, KiCad or other),
the privacy choice (the update check, off by default) and the appearance (theme and scale), and then what to do first.
Every step can be skipped. **Help, First-run setup** runs it again. Nothing in it makes a network connection.

## Company-standard wizard

The wizard collects the conventions of your company and writes a **basic project**, from which new projects start. It
has steps for the company, the frame and title block, the colour profile, the size profile, identifiers and numbering,
structure and sections, revisions, languages and formats, and reports and checks, and a review. A preview of the cover
and of a sample page shows the current choices. The draft is kept in the configuration directory until you finish, and
the basic project is written to a directory you choose and becomes the default for **New project**.

## About

**Help, About ElektraWatt** shows the version, the page-view renderer, the privacy state, the licence, the
engineering-verification disclaimer, the support status computed from the planned support end, and the third-party
notices.
