## What it checks

A text that EW derives, such as a tag or a cross-reference, that differs from the text the source printed.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. The printed drawing of the source and EW's drawing disagree.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project whose stored cross-reference texts are out of date.

## How to fix

The migration report shows both texts. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
