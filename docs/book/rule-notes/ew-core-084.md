## What it checks

A rule pack, or the part of it that needs a data table, that did not run. For the shipped packs the cause is a data
table that does not load. One finding per pack and table, naming the rules left out.

## Why

A pack that did not run reports nothing, which looks like a clean result. The finding makes the gap visible.

## Standard

No standard: a project-hygiene rule.

## Example

No project shows it with a correct installation: it appears when a pack's data file is missing or damaged.

## How to fix

Reinstalling EW restores the shipped data files. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
