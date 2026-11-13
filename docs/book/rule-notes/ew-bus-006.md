## What it checks

Over the segments of a PROFIBUS physical net: more than 32 stations, a longer segment than the rate allows (the
summed cable lengths), and a branch above 1.5 Mbit/s. The limits are unverified data, so the rule reports warnings.
It runs in the offline and release modes.

## Why

A segment beyond its limits loses telegrams.

## Standard

No standard: a project-hygiene rule.

## Example

Lay a 300 m segment at 1.5 Mbit/s.

## How to fix

Shorten the segment, lower the rate, or split it with a repeater. There is no quick fix.

## Waiving

Waive it when the installation was measured, with the measurement.
