## What it checks

A text style the project prints with whose cap height, the nominal size of the printed letters, is below the minimum
of 1.2 mm. The height comes from the project's own text styles. One finding per text style, not per text; a style no
text uses is not reported.

## Why

Text below the minimum height becomes unreadable once a drawing is printed or copied at a smaller size.

## Standard

- ISO 3098-1:2015, 4 (unverified)

## Example

On Golden A, the built-in styles cp.small and marker print at 0.94 mm. A new project overrides both to 1.32 mm, so it
passes.

## How to fix

A text style applies to every text that uses it. There is no quick fix.

## Waiving

Waive it for a style that only prints on documents that are never reduced, and say which.
