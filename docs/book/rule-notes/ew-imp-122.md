## What it checks

An element without a label: the importer gives its device the letter of its kind (the element's prefix, else "A")
and no counter, and the import's commit numbers it like a device you place, with the next free counter of its page.
In tutorial 6 the unlabelled lamp and earth become -1A1 and -1X1.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. A tag the importer made up was not on the QElectroTech drawing, so check that it
is the one you want.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project with an unlabelled element.

## How to fix

Check each new tag. Where you want another, edit the device's tag, or renumber the page. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
