## What it checks

After a merge, a value that must be unique and now appears twice: a device tag, a terminal designation, a wire number,
an interruption point's star source, a PLC address or a PLC symbol. It reports only while the project carries evidence
of a merge (its conflict records, a merge in progress or `ew check --integrity --after-merge`). One finding per group;
a group of device tags carries the fix "Renumber the newer". The rule runs in the window's online check and in release
mode.

## Why

Each side of a merge was valid on its own; together they hand out one number twice. The prevent rules that normally
refuse a duplicate could not see the other side.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, add a device on two branches of a copy of Golden A that both take the next free tag, then merge.

## How to fix

For device tags the quick fix "Renumber the newer" gives the newer devices free counters; the keeper is the record
whose tag is frozen, else the oldest. The other values have no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
