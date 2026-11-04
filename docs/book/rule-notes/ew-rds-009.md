## What it checks

A device identifier (its letter code, frozen tag or a terminal's designation) whose non-Latin characters all look like
Latin letters or digits, such as a Cyrillic "А" in a tag. A genuinely non-Latin identifier is no finding. One finding
per identifier. An edit that creates such an identifier is refused.

## Why

A tag typed on another keyboard looks right but is a different text: search does not find it, and cross-references to
the Latin spelling break.

## Standard

No standard: a project-hygiene rule.

## Example

Paste a tag with a Cyrillic "К" into a frozen tag of a project file.

## How to fix

Type the tag again on a Latin keyboard. There is no quick fix.

## Waiving

Do not waive it: the identifier cannot be found.
