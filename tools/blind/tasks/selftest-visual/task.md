# The visual self-test task

This task is for the harness, not for an agent. A scripted agent (`agent.jsonl`) plays the part of a play-tester and
calls the run's copy of the harness exactly as a real agent would, through `{blind}`, in a visual run: it looks at the
start frame, opens the symbol picker with a click, types a filter and chooses the normally open contact with Enter,
places two contacts with two clicks on page 101, zooms into the frame to read their tags and logs a friction entry
about it, sends a Redo that the product refuses and an Undo and Redo that it does, tries an action line and a text look
(both refused by the harness), hovers for a tooltip, selects the two contacts with a drag, deletes and restores them,
zooms with Ctrl and the wheel, saves with Ctrl+S, fits the page with the short form of `do`, and files the
play-tester's hand-back.

Run `{run}` is a `{kind}` run with the caps {turns} turns, {steps} inputs, {looks} looks and {refusals} harness
refusals. The prelude creates the new project and the page; the epilogue of `stop` describes page 101, saves nothing
(the scripted agent saves its own work) and quits.

`cargo xtask blind selftest-visual` runs it twice, with the session keeper and with `--stateless`, blesses the first run
as the recording `tools/blind/recordings/selftest-visual/` (`ci = "pr"`, the inputs, the dispatched actions and the
frames' hashes, never the PNGs) and replays that recording twice without painting; on Windows it paints the frames once
more and reports how many differ from the recording's hashes (R4, a report). A change of the window that breaks the
recording (a moved button, a new dialog) is repaired by running the visual self-test again in the same change; the
coordinates of `agent.jsonl` are read off its frames.
