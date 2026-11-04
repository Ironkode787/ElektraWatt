## What it checks

A record stored in another file than the one its kind and its owner belong in, or a library document stored under
another name than its own. It is read where it is, and moved at the next save. Information only.

## Why

A record in the wrong file still reads, but merges and reviews work file by file, and a record outside its file is
easy to miss.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, move one page's line from its file under `pages/` to another page's file in a copy of Golden A and open
the copy.

## How to fix

Save the project: the save moves the record to its file. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
