# The self-test task

This task is for the harness, not for an agent. A scripted agent (`agent.jsonl`) plays the part of a drafter and
calls the run's copy of the harness exactly as a real agent would, through `{blind}`: it looks at the interface and at
the page, places two contactors on page 101 of a new project with the Place tool, makes one step that the product
refuses (Redo with nothing to redo) and one that the harness refuses, changes some views, saves, exports the PDF and
files its hand-back.

Run `{run}` is a `{kind}` run with the caps {turns} turns, {steps} steps, {looks} looks and {refusals} harness refusals.
The prelude creates the new project and the page; the epilogue of `stop` describes page 101, saves nothing (the scripted
agent saves its own work) and quits.

`cargo xtask blind selftest` runs it twice, with the session keeper and with `--stateless`, blesses the first run as
the recording `tools/blind/recordings/selftest/` and replays that recording twice. A change of the product that breaks
the recording is repaired by running the self-test again in the same change.
