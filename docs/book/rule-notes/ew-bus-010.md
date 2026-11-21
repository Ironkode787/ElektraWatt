## What it checks

Ports joined by bus connections that belong to different logical nets.

## Why

Ports on one line are on one bus; two logical nets on one line confuse the addresses and the master.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of the sample project, move the PROFIBUS port of the valve terminal -201K1 to a second logical net while the
bus cable -105W1 still joins it to the CPU -104A1.

## How to fix

Put every port of the segment in one logical net. There is no quick fix.

## Waiving

Do not waive it: the line is one bus.
