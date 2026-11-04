## What it checks

A font the source uses that EW does not have, replaced by a substitute. Recorded once per font family.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. Texts may print wider or narrower than in the source.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project whose texts use a font that is not installed with EW.

## How to fix

A text style applies to every text that uses it. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
