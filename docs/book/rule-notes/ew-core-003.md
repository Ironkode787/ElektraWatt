## What it checks

A file that the manifest lists is missing, or is not a regular file: EW does not follow links. The project opens
read-only.

## Why

The records of a missing file are not in the model. Saving would drop them for good.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, delete one file under `pages/` from a copy of Golden A and open the copy.

## How to fix

Restore the file from git, the sync client or a backup. A cloud folder may still be downloading it: wait, and open
the project again. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
