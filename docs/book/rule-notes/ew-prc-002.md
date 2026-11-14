## What it checks

A row of an applied instrument list that has no channel, because no spare channel of its signal type was free, or
whose placeholder device is gone. One finding per row.

## Why

An instrument without a channel is not wired to the PLC and not in its program. The list of open tags is what is
still to be engineered.

## Standard

No standard: a completeness rule.

## Example

Apply an instrument list with more digital inputs than the stations have spare input channels.

## How to fix

Add a module, or free a channel of the tag's signal type, and apply the instrument list again: a row that has its
channel keeps it. There is no quick fix.

## Waiving

Waive it for an instrument that another party connects, with the reason.
