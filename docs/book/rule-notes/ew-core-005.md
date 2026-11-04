## What it checks

A JSON syntax error: a line of a record file that is not one JSON value or not UTF-8, named by its file and line,
or a library document that is not JSON. The line is kept as found, and the project opens read-only.

## Why

EW cannot read the line's record, so it cannot tell what a save would lose.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, delete the closing brace of one line of `project.jsonl` in a copy of Golden A and open the copy.

## How to fix

`ew fmt --repair` discards the broken lines, after packing the files as found beside the project folder as
`<project folder>.before-repair-g<generation>.ewz`. It refuses a library document that is not UTF-8, and refuses
while another finding keeps the project read-only. You can also correct the line in a text editor. There is no
quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
