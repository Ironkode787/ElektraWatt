## What it checks

A channel symbol that is not an IEC 61131-3 identifier while the configuration project's symbol policy is strict (the
default): only letters, digits and single underscores, not starting with a digit, not ending with an underscore. It runs
in the export and release modes.

## Why

An export to TIA Portal or CODESYS refuses a tag whose name is not an identifier.

## Standard

- IEC 61131-3:2013, 6.1.2 (unverified)

## Example

Give a channel the manual symbol TA05-V-15; TA05_V_15 passes.

## How to fix

Change the symbol on the channel's row, or set the symbol policy to lenient for TIA quoting. There is no quick fix.

## Waiving

Do not waive it for an export: the PLC software cannot read the tag.
