## What it checks

A logical net's rate that a port's part does not support. Parts that state no rates are not judged.

## Why

A slave that does not support the bus rate does not answer.

## Standard

No standard: a project-hygiene rule.

## Example

Set a DP net to 12 Mbit/s with a slave whose part supports 1.5 Mbit/s at most.

## How to fix

Choose a rate that every station supports on the logical net. There is no quick fix.

## Waiving

Do not waive it: the slave does not join the bus.
