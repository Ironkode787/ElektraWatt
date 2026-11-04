## What it checks

Imported text styles that are not bound to the kinds of text slot EW uses. The live rule is EW-CORE-063.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. Texts keep their imported sizes instead of the project's styles.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project with its own font sizes.

## How to fix

The text style is on the text's row in the property grid. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
