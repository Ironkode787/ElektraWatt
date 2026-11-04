## What it checks

A reader limit exceeded: a line, a file, the number of files or the nesting of JSON is beyond what the loader
reads. The file or the line is not read, and the project opens read-only.

## Why

The limits protect EW from files made to exhaust it. A project that exceeds one is damaged or not what it claims
to be.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, nest one value of a line of `project.jsonl` in a copy of Golden A more than 64 arrays deep and open the
copy.

## How to fix

Split or repair the file outside EW, and check where the project came from. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
