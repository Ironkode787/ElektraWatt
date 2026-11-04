## What it checks

Two pages of the page order that print the same folio label: drawn pages, section dividers and the generated pages of
report blocks, sub-pages such as 901.1 included. One finding per colliding label. An edit that creates a collision
among drawn pages and dividers is refused; a collision a file already holds, and one with generated pages, is
reported.

## Why

A folio identifies one page in every cross-reference and in the table of contents. Two pages with one folio make the
references ambiguous. EW keeps both pages in the output, so nothing is lost, but someone has to decide.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of Golden A, add a report block whose generated pages start at the folio of an existing drawn page.

## How to fix

Change the folio in the page's or the report block's properties. There is no quick fix.

## Waiving

Do not waive it: two pages never share a folio.
