## What it checks

A file that a DXF drawing refers to but does not hold: an image (`IMAGE`) or an external reference block. The import
never opens a file beside the drawing, so it draws a frame with the image's file name in its place, and nothing for an
external reference.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out.

## Why

An import reads only the file you chose, never another file of your computer. What the referenced file showed is
therefore missing from the page.

## Standard

No standard: a project-hygiene rule.

## Example

Import a DXF drawing with a company logo placed as an image.

## How to fix

If the page needs the picture, place it yourself as an image on the page. There is no quick fix.

## Waiving

Waive the finding once you have checked the page; the waiver records the review.
