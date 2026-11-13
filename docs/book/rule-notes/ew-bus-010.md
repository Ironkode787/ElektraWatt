## What it checks

Ports joined by bus connections that belong to different logical nets.

## Why

Ports on one line are on one bus; two logical nets on one line confuse the addresses and the master.

## Standard

No standard: a project-hygiene rule.

## Example

Move -201U1:DP to a second logical net while -3W1 joins it to the master.

## How to fix

Put every port of the segment in one logical net. There is no quick fix.

## Waiving

Do not waive it: the line is one bus.
