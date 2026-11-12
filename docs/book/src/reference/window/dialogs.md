# Dialogs

A dialog asks for a decision or a few values and closes when you answer. Help ▸ Documentation in an open dialog opens
its section, here or in [Use the application window](../../how-to/application-window.md), which describes these
dialogs:

- About ElektraWatt and the third-party notices: [About](../../how-to/application-window.md#about);
- the keyboard shortcuts: [Keyboard shortcuts](../../how-to/application-window.md#keyboard-shortcuts);
- Save, Discard or Cancel before a change would be lost, and the recovery of unsaved changes:
  [Unsaved changes and recovery](../../how-to/application-window.md#unsaved-changes-and-recovery);
- the check scheme the company standard wizard applies:
  [Company standard wizard](../../how-to/application-window.md#company-standard-wizard);
- the name of a saved layout: [Workspaces](../../how-to/application-window.md#workspaces);
- the QElectroTech import: [Import a QElectroTech project](../../how-to/application-window.md#import-a-qelectrotech-project).

The keys of a dialog's buttons are on the [keymap](../keymaps.md) page.

## Go to page or cell

Opens a page by its folio, or a cell of a page by its folio and grid reference, such as 102-B4. Open it with Page ▸
Go ▸ Go to page or cell….

## Form dialogs

The fields of an action that needs values before it runs, such as Page ▸ New page…, Project ▸ Setup ▸ Structure
node… and Project ▸ Setup ▸ External reference…. The primary button runs the action with the values; Cancel
changes nothing.

## Purge unused images

Lists the image files that no page uses any more and deletes them on Purge. Undo does not bring them back. Open it
with Project ▸ Purge unused images….

## Choose part

The part selection: search the library, filter the results and choose the part for the selected devices. Open it with
Edit ▸ Choose part…. See [Choose parts](../../how-to/choose-parts.md).

## Edit part

The part editor: change the members of the project's copy of a part and save them. Open it with Edit ▸ Edit part….

## Print

The print options beside a preview of the print set; Print… hands the set to the printer, Open in PDF viewer opens it
there. Open it with Output ▸ Print…. See [Reports and PDF](../../tutorials/reports-and-pdf.md).

## Changes on disk

The project changed on disk while you have unsaved edits to the same fields: each colliding field with its base, your
value and the value on disk. Choose a side per field and Apply, take all of one side, or decide later. It opens by
itself; File ▸ Check for changes on disk looks again.

## Upgrade the format

The first save of a project in an older format: Upgrade and save writes a backup beside the project, then saves in the
current format; Cancel saves nothing. It opens by itself. See [Upgrade a project](../../how-to/upgrade-a-project.md).

## Re-import a matched file

A QElectroTech file that matches an earlier import of this project by its items: Re-import compares it with that
import; Cancel stops. Open it with Project ▸ Re-import QElectroTech project….

## Re-import QElectroTech project

The re-import card: the records the source added, changed, changed on both sides and removed, in words. Apply takes
them over as one undo step; Cancel changes nothing. The migration report then lists what needs a decision.

## Open .ewz

An `.ewz` archive of a project: Extract and open unpacks it into a new folder beside it, Open read-only opens it
without unpacking. Open it with File ▸ Import…, which takes a QElectroTech project as well.

## Set up merge driver

Sets up the repository's git configuration so that git merges the project's files record by record. It asks first
and writes nothing on Cancel. Open it with Project ▸ Set up merge driver.
