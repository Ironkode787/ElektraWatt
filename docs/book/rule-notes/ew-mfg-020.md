## What it checks

A terminal strip whose terminals have parts and after whose last terminal no end plate stands: none of its terminals'
parts names one, the end plate was left off in the strip editor, or none was added. One finding per strip.

## Why

The last terminal of a feed-through strip is open on one side: without an end plate its clamps and its live parts are
open to touch, and the assembly list tells the fitter nothing to close it with.

## Standard

No standard: a project-hygiene rule.

## Example

Leave the end plate off -X1031 in the strip editor (Delete on its grey End plate row): -X1031 has no end plate after
its last terminal SH.

## How to fix

Add an end plate after the last terminal in the strip editor (Add accessory, kind End plate), or name the end plate in
the terminal's part, under Accessories, so every strip gets it. Where a terminal's part names an end plate, the quick
fix Add the end plate after the last terminal adds it as the strip's own. A strip whose terminals have no part asks for
nothing: the part decides whether one is needed.

## Waiving

Waive it for a strip whose last terminal closes its side itself, or that ends against another strip's end plate.
