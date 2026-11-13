## What it checks

A logical net without a master: it names none, and none of its ports is a master or a controller. An external partner
named as the master counts.

## Why

A bus without a master never starts.

## Standard

No standard: a completeness rule.

## Example

Create a logical net and add only slave ports to it.

## How to fix

Name the master on the logical net's row. There is no quick fix.

## Waiving

Do not waive it: name the external partner instead.
