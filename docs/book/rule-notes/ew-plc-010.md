## What it checks

A channel whose data type (its own, else its part's) is an IEC elementary type of another width than the channel: one bit
for DI and DO, a word for AI and AO. Structs and vendor types are not judged.

## Why

A Bool tag on a 16-bit analogue channel reads one bit of the value.

## Standard

- IEC 61131-3:2013, 6.4.2 (unverified)

## Example

Give an AI channel the data type Bool.

## How to fix

Change the channel's data type, or correct the part's PLC section. There is no quick fix.

## Waiving

Do not waive it: the PLC reads the wrong width.
