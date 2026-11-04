## What it checks

The project's manifest, the file `manifest.json` that lists the project's files and their hashes, is missing,
cannot be parsed, or holds git conflict markers. It is found when the project is opened and by
`ew check --integrity`. A warning: it does not block release.

## Why

The manifest says which files belong to the last save and what each held. Without it EW cannot tell a file of the
last save from a stray copy, or notice a file that changed outside EW.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, delete `manifest.json` from a copy of Golden A and open the copy.

## How to fix

Save the project: the save writes a new manifest. Where the manifest holds conflict markers, you can also take
either side's version from git. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
