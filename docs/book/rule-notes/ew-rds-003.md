## What it checks

A device whose letter code does not match the function category of its main function in the project's identifier set;
the finding suggests the set's letter. Left out: terminals, devices without a main function, black boxes and overview
definitions, and categories the set does not map. The shipped identifier sets are not verified against the standard,
and the message says so. One finding per device.

## Why

The letter code says what a device does: a temperature switch is a sensor (B, or BT under the 2019 set), not a
transformer (T). A wrong letter misleads every reader of the drawing.

## Standard

- IEC 81346-2:2019, Table 2 (unverified; the identifier sets ship unverified)

## Example

On Golden A, the temperature switch T55 carries the letter T; the suggestion is B, BT under the 2019 set.

## How to fix

The suggested letter is in the finding's message. There is no quick fix.

## Waiving

Waive it when the customer's standard prescribes the letter, and name that standard in the reason.
