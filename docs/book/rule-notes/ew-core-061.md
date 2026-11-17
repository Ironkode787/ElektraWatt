## What it checks

A text style the project prints with whose capitals print lower than 1.2 mm on paper. The message names the style as
Project ▸ Text styles… lists it (Wire marker, Connection point, …), not by an inner name. The height comes from the
project's own text styles. One finding per text style, not per text; a style no text uses is not reported.

## Why

Text below the minimum height becomes unreadable once a drawing is printed or copied at a smaller size.

## Standard

- ISO 3098-1:2015, 4 (unverified)

## Example

On Golden A, the built-in styles Connection point and Wire marker print at 0.94 mm. A new project starts with both at
1.32 mm, so it passes.

## How to fix

Open the message (double-click it, or Enter): Text styles opens at the style. Make it at least 1.2 mm high there, or
press Standard sizes to give every style the height a new project has. The quick fix Set to 1.2 mm sets the style to
the minimum in one step. A text style applies to every text that uses it, and the change is one undo step.

## Waiving

Waive it for a style that only prints on documents that are never reduced, and say which.
