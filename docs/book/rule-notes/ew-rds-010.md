## What it checks

Two or more structure nodes of one aspect whose codes differ only in separators or case, such as `+JE2416` and
`+J-E-2416`. One finding per group. The merge quick fix is not offered yet.

## Why

Two spellings of one place or function split its devices in two, so reports and designations treat one location as
two.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project that writes one location both as +JE2416 and as +J-E-2416.

## How to fix

Merge the structure nodes into one: move the devices and pages of one node to the other and delete the empty node.

## Waiving

Waive it when the two codes really name two nodes.
