## What it checks

A structure code (of a function, location or other node) that contains a prefix or separator character, such as
`+J-E-2416`. EW keeps a code as one token and never re-parses it, so the finding is about exports. One finding per
node; online only, so it runs in the drafting and full schemes but not in pre-delivery.

## Why

A third-party program that reads the full designation `+J-E-2416-F1` sees location J, then products E, 2416 and F1.
Exports such as AML, EPLAN XML and CSV then carry a designation that other tools misread.

## Standard

- IEC 81346-1:2022, 6 (unverified)

## Example

Golden A's location code `J-E-2416` holds hyphens.

## How to fix

The code is on the structure node's row in the property grid. There is no quick fix.

## Waiving

Waive it when the code is fixed by the customer and the exports do not leave EW; the finding is an info.
