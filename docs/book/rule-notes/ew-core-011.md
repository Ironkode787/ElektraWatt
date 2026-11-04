## What it checks

Conflict records that a merge left in the project. One finding per conflict record: a record-level conflict names the
record in words with the members both sides changed differently, or the side that deleted it; a file-level conflict
names its file and the side whose lines it holds. An error in release mode, so the release gate blocks while one is
left.

## Why

A merge that both sides changed differently cannot be settled by a program. Until someone decides, the project holds
two truths for one record.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, merge two branches of a copy of Golden A in git that both changed the same device's description.

## How to fix

`ew merge resolve` takes ours or theirs for each conflict from the terminal. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
