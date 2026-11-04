## What it checks

A file in the project folder that the manifest does not list: a sync client's conflict copy, a file added by hand,
or a file that a save moved aside to an `.unlisted-<generation>` name instead of overwriting it. EW never reads
such a file while a manifest exists, and never deletes it. A warning: it does not block release.

## Why

The file may hold work that is not in the project, such as the other side of a sync conflict. EW cannot know, so it
tells you.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, put a copy of `project.jsonl` named `project (conflict).jsonl` in a copy of Golden A and open the copy.

## How to fix

Compare the file with the project. Merge what you need as an external change, then delete the file by hand. There
is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
