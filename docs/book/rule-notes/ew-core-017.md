## What it checks

A malformed value of a known variant: a value whose tag names a variant this build knows but whose content does
not parse, such as `{"core": 5}`. It is kept verbatim, and derivations treat it as unknown.

## Why

The value means something EW cannot read, so a tag, a cross-reference or a report that derives from it would be
wrong.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, change a connection's end `{"cp": ["<id>", 3]}` in a copy of Golden A to `{"cp": 3}` and open the
copy.

## How to fix

Enter the value again in the property grid, which shows the raw value. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
