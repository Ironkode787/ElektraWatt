## What it checks

A schema violation: a record line with a member of the wrong type, a missing member, a float where a decimal is
stored, or a value that does not parse, such as a length with four fraction digits. The line is quarantined
verbatim: kept in its file as it is, and left out of the model.

## Why

The record cannot be read as what its kind says, so every output that would use it would be wrong or incomplete.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, change a page's folio `number` to a text such as `"one"` in a copy of Golden A and open the copy.

## How to fix

Correct the line in a text editor, by the format's reference, or restore it from git. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
