## What it checks

A printed cross-reference whose target cell lies outside the frame, or holds no graphics of the target. The target's
anchor cell comes from its anchor policy; a target with no graphics is not judged. One finding per cross-reference,
with the quick fix that sets the anchor policy to the graphics' centre.

## Why

A cross-reference names a grid cell so that the reader finds the target there. A cell outside the frame, or one the
symbol does not reach, sends the reader to the wrong place.

## Standard

- IEC 61082-1:2014, Figure 8 and Figure 9 (unverified)

## Example

In a copy of Golden A, set a contact's anchor policy to a corner that lies in an empty cell.

## How to fix

The quick fix sets the target's anchor policy to the centre of its graphics.

## Waiving

Waive it when the reference is right and the target's graphics only touch the cell's border.
