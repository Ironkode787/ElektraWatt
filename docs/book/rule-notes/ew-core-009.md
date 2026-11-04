## What it checks

A record ID that occurs twice for one kind of record, in one file or across files, records of unknown or reserved
kinds included. The first occurrence is kept, and the others are quarantined verbatim.

## Why

Two records under one ID cannot both be the record that others refer to. Keeping one silently would lose the
other's data.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, copy one line of `project.jsonl` in a copy of Golden A to the end of the file and open the copy.

## How to fix

Delete the extra line in a text editor, or give its record a new ID where both are wanted. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
