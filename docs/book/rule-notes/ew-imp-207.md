## What it checks

Terminals of an EPLAN terminal diagram that follow one another with one designation and rising levels. The import
folded them into one multi-level terminal.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out.

## Why

EPLAN lists each level of a multi-level terminal as a row of its own; ElektraWatt keeps one terminal with its levels.

## Standard

No standard: a project-hygiene rule.

## Example

Rows `X1 / 5 / level 1` and `X1 / 5 / level 2` in a terminal diagram.

## How to fix

Check the terminal's levels against the report. There is no quick fix.

## Waiving

Waive the finding once you have checked the levels; the waiver records the review.
