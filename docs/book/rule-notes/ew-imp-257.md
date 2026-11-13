## What it checks

A DWG drawing that the ODA File Converter, which you installed and configured, turned into a DXF before the import
read it. ElektraWatt does not include or download the converter.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out.

## Why

The import read the converter's DXF, not the DWG itself, so what the page shows depends on that conversion as well.

## Standard

No standard: a project-hygiene rule.

## Example

Import a DWG drawing with the converter configured.

## How to fix

Compare the page with the DWG in the program that made it. There is no quick fix.

## Waiving

Waive the finding once you have compared the page; the waiver records the review.
