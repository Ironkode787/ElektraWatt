## What it checks

A save that was interrupted and left the project's files from two generations, in a way that EW cannot complete
from the save's journal. The project opens read-only.

## Why

A project made of two saves holds records that never existed together. Saving it would write that mix as if it
were one project.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, copy one record file of an older save of a copy of Golden A over the current one, and open the copy.

## How to fix

Restore the project's files from git, the sync client or a backup, so that they come from one save. `ew recover`
replays a crashed session's journal. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
