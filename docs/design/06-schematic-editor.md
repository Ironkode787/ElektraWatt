# Schematic editor

**Status:** Draft v0.1

**Purpose.** This document specifies how ElektraWatt (EW) edits schematic pages: multi-line, single-line, overview,
bus-topology, fluid and graphic pages, together with their frames, folios and sections. It covers symbol placement,
devices and main/auxiliary functions, black boxes and structure boxes, connections and junctions, interruption
points, cross-reference display, texts and graphics, property editing, clipboard, snapping, tools, shortcuts, undo
and online checks. It refines the accepted [architecture](04-architecture.md) (§5 commands, §6 queries, §8
rendering, §9 UI) and the [domain model](03-domain-model.md) (§6, §7, §9, §10) for the interactive canvas. It ends
with a walkthrough that draws folios 101 and 102 of the reference project.

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Position in the architecture](06-concepts.md#1-position-in-the-architecture) | [position and concepts](06-concepts.md) |
| [§2. Concepts and data](06-concepts.md#2-concepts-and-data) | [position and concepts](06-concepts.md) |
| [§3. Behaviour and rules](06-commands-connecting.md#3-behaviour-and-rules) | [commands, normalisers, placing, moving and connecting](06-commands-connecting.md) |
| [§3.1 Editor commands](06-commands-connecting.md#31-editor-commands) | [commands, normalisers, placing, moving and connecting](06-commands-connecting.md) |
| [§3.2 Normalisers that run behind the editor](06-commands-connecting.md#32-normalisers-that-run-behind-the-editor) | [commands, normalisers, placing, moving and connecting](06-commands-connecting.md) |
| [§3.3 Placing symbols and assigning devices](06-commands-connecting.md#33-placing-symbols-and-assigning-devices) | [commands, normalisers, placing, moving and connecting](06-commands-connecting.md) |
| [§3.4 Move, rotate and delete](06-commands-connecting.md#34-move-rotate-and-delete) | [commands, normalisers, placing, moving and connecting](06-commands-connecting.md) |
| [§3.5 Connections, junctions, interruption points and potentials](06-commands-connecting.md#35-connections-junctions-interruption-points-and-potentials) | [commands, normalisers, placing, moving and connecting](06-commands-connecting.md) |
| [§3.6 Copy, paste and duplicate](06-copy-select-undo.md#36-copy-paste-and-duplicate) | [copy, selection, texts, undo and online checks](06-copy-select-undo.md) |
| [§3.7 Selection, multi-select and bulk edit](06-copy-select-undo.md#37-selection-multi-select-and-bulk-edit) | [copy, selection, texts, undo and online checks](06-copy-select-undo.md) |
| [§3.8 Texts and graphics](06-copy-select-undo.md#38-texts-and-graphics) | [copy, selection, texts, undo and online checks](06-copy-select-undo.md) |
| [§3.9 Undo granularity](06-copy-select-undo.md#39-undo-granularity) | [copy, selection, texts, undo and online checks](06-copy-select-undo.md) |
| [§3.10 Online checks while drawing](06-copy-select-undo.md#310-online-checks-while-drawing) | [copy, selection, texts, undo and online checks](06-copy-select-undo.md) |
| [§4. Editing tools as state machines](06-tools-algorithms.md#4-editing-tools-as-state-machines) | [editing tools and algorithms](06-tools-algorithms.md) |
| [§5. Algorithms](06-tools-algorithms.md#5-algorithms) | [editing tools and algorithms](06-tools-algorithms.md) |
| [§6. UI and UX touchpoints](06-window-edges.md#6-ui-and-ux-touchpoints) | [the window, outputs, edge cases and defects](06-window-edges.md) |
| [§7. Outputs](06-window-edges.md#7-outputs) | [the window, outputs, edge cases and defects](06-window-edges.md) |
| [§8. Edge cases and pitfalls](06-window-edges.md#8-edge-cases-and-pitfalls) | [the window, outputs, edge cases and defects](06-window-edges.md) |
| [§9. Walkthrough: drawing folios 101 and 102](06-window-edges.md#9-walkthrough-drawing-folios-101-and-102) | [the window, outputs, edge cases and defects](06-window-edges.md) |
| [§10. Reference defects this editor prevents](06-window-edges.md#10-reference-defects-this-editor-prevents) | [the window, outputs, edge cases and defects](06-window-edges.md) |
