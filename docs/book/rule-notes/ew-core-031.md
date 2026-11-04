## What it checks

A cable core whose colour holds a code that is neither an IEC 60757 colour code nor a company code that the project's
colour profile declares. A foreign code, such as "BR" kept by a lenient import, and an undeclared company code are
reported; a code of a newer format is not judged. One finding per core. An edit that gives a core such a colour is
refused; a core that already holds one is reported.

## Why

A colour code on a drawing must mean the same to everyone. A code outside the standard and the project's own profile
cannot be checked against the cable's real cores, and machines and labels may not print it.

## Standard

- IEC 60757:2021, 4 (unverified)

## Example

Import a QElectroTech project whose cable cores use the code "BR" for brown.

## How to fix

Change the core's colour on the cable's core row. The import finding EW-IMP-008 may offer the mapping as its quick
fix.

## Waiving

Waive it only for a code that the cable's manufacturer prints and the project cannot declare.
