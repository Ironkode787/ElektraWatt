## What it checks

Git conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) in a record file, left by a merge that was committed or
stopped before its conflicts were resolved. The project opens read-only.

## Why

Between the markers the file holds both sides of a merge, and neither is the project. A save would have to choose
for you.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, merge two branches of a copy of Golden A in git that both changed the same device's description, and
open the project before resolving the conflict.

## How to fix

Resolve the conflict in git, or turn the markers into conflict records with EW's merge driver and settle them with
`ew merge resolve`. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
