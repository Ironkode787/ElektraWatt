## What it checks

A terminal of a strip whose designation is empty, or holds only white space. One finding per terminal.

## Why

A terminal without a designation cannot be found on the terminal diagram, the labels or the wire list, so the wiring
on site cannot be checked against the documentation.

## Standard

- IEC 61666:2010, 6 (unverified)

## Example

The window and the commands refuse an empty designation, so a project you draw in EW never holds one. Only a file
written by another program, or an import, can carry such a terminal.

## How to fix

Select the terminal in the strip editor or the property grid and type a designation, or renumber the strip.

## Waiving

Do not waive it: a terminal without a designation cannot be labelled. Give it a designation instead.
