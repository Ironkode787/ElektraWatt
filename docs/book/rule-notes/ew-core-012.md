## What it checks

A file-level change that breaks a structural invariant of the model (ADR-0009), such as a record that a hand edit
or a merge left in a state no command can make. No reader of this build reports it yet: a reference to a missing
record is EW-CORE-008, and a value taken twice after a merge is EW-CORE-013.

## Why

An invariant is what every command relies on. A project that breaks one can make the next command fail or do the
wrong thing.

## Standard

No standard: a project-hygiene rule.

## Example

No change to a copy of Golden A shows it in this build.

## How to fix

The next change that touches the record must restore the invariant. Where you cannot, restore the file from git.
There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
