## What it checks

A part use that names a variant its part does not hold: the device asks for the blue terminal, and its part sells no
blue one. One finding per device and variant.

## Why

A variant is the part under its own number. The lists cannot print a number the part does not have, so they print the
part's own, and the order would bring the part, not the variant that was asked for.

## Standard

No standard: a project-hygiene rule.

## Example

Give the terminal -X1011:N, of part EW-GEN-TRM-1, the variant RD by a script: EW-GEN-TRM-1 holds the grey terminal and
the blue variant BU only.

## How to fix

Choose a variant the part holds in Properties ▸ Parts ▸ Variant, or the part's own number. The quick fix Use the
part's own number takes the part's own number.

## Waiving

Waive it for a variant the maker sells that the library part does not list yet; the lists print the part until the
library holds it.
