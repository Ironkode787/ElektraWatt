## What it checks

A page whose title block overrides a bound project value; at present the one such override is the page's own
description. One finding per page, with the quick fix "drop override", which removes it.

## Why

A title-block value is bound to the project so that it is the same on every page. An override on one page is sometimes
right (a page with its own title), and sometimes a leftover that now shows stale text.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, give one page its own description in the page's properties.

## How to fix

The quick fix "drop override" removes the page's own description in one step.

## Waiving

Waive it when the page's own description is intended; the finding is an info and does not block release.
