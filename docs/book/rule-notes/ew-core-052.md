## What it checks

A group of interruption points in star mode with more than one star source: either more than one point is flagged as
the star source, or none is and two or more points are sources. EW picks one and pairs, and the finding asks you to
decide. An edit that creates a new ambiguity is refused.

## Why

In star mode one source feeds many targets. With two sources the cross-references point at a source chosen by the
program, not by you.

## Standard

- IEC 61082-1:2014, Figure 9 (unverified)

## Example

In a new project, flag two interruption points of one name as star source.

## How to fix

The star-source flag is on the interruption point's row in the property grid. There is no quick fix.

## Waiving

Do not waive it; the choice of source is yours to make.
