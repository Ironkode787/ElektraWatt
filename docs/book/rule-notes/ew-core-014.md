## What it checks

A library copy embedded in the project whose content does not match its `sha256`: the copy was changed after it
was embedded. A warning: it does not block release.

## Why

The hash says which published library version the copy is. A changed copy is no longer that version, so its parts
lists can differ from the library's, and updating it from the library would lose the change.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, change the description of one copy under `lib/` in a copy of Golden A in a text editor and open the
copy.

## How to fix

Embed the copy again from the library to take its published version, or keep the change on purpose. There is no
quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
