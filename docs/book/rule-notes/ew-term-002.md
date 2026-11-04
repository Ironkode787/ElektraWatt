## What it checks

Two or more terminals of one strip that carry the same designation, compared under the project's case setting. One
finding per duplicated designation, naming every terminal that carries it. A strip whose own designation is still
unnumbered (it contains "?", the importer's fallback strip for unlabelled terminals) is skipped. An edit that creates
a duplicate is refused; a duplicate that a file or an import already holds is reported.

## Why

A designation identifies one terminal on the terminal diagram, the labels and the wire list. Two terminals with one
designation make the connection list ambiguous, and the wrong terminal gets wired.

## Standard

- IEC 61666:2010, 6 (unverified)

## Example

In the window the edit is refused, with the next free number proposed. To see the finding, import a QElectroTech
project in which two terminals of one strip carry the same label.

## How to fix

Swapping two designations is one edit and needs no temporary name. There is no quick fix.

## Waiving

Waive it only for a deliberate double designation that the terminal markers show apart, which is rare; a renumber is
almost always the better fix.
