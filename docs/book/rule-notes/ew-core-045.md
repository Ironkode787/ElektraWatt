## What it checks

A device whose contact functions (normally open, normally closed, change-over) exceed, or differ in kind from, the
contacts of its parts. Every contact function counts, the main one included; a change-over contact serves a normally
open or closed one; the contacts of every part of the device add up. A device whose parts carry no contact data is not
judged. One finding per device.

## Why

A contactor has a fixed set of contacts. A drawing that uses more contacts, or a normally closed one where the part
has only normally open ones, cannot be built.

## Standard

- IEC 61082-1:2014, Figure 9 (unverified)

## Example

In a new project, place three normally open contacts of a contactor whose part has two.

## How to fix

An auxiliary contact block is a further part on the device's Parts row. There is no quick fix.

## Waiving

Do not waive it: the device cannot be built as drawn.
