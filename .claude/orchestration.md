# Working with subagents

Read this only when you hand work to subagents. The main session plans, decides and integrates; it may also do the
work itself, which is the default for anything small or anything that needs judgment across the code.

## Roles

- The main session is the product owner by delegation: it decides open product questions, golden changes, design
  taste and Dutch wording, and writes each decision into the document it changes.
- Real-world actions stay the owner's: legal matters, GitHub settings, funding, the pilot, the go for a release.
- The owner's hands-on test decides whether the program works. Automated checks and persona runs inform it; they do
  not replace it.

## One work item

1. **Brief.** The goal, the files or area, what done looks like, and what not to touch. A few lines; point at the
   documents instead of repeating them. A brief that cannot be written clearly is not ready.
2. **Build.** One `lane` agent in its own worktree, on the tip the brief names. A work item is not split across
   agents.
3. **Check.** The agent runs `cargo xtask check -p <its crates>` and reports the summary. Its result is a claim:
   read the diff, and run the check again on the merged tip when the change is large.
4. **Review.** A change to the file format, to saving and loading, to a core crate or to what a release ships gets
   one independent `reviewer`. The reviewer fixes its own minor findings; a major goes back to the author. A fix
   is not reviewed again.
5. **Merge** onto the working branch and delete the agent's worktree with its `target`.

## Choosing the agent

| Type | For |
|---|---|
| `scout` | Finding things and gathering facts. Read-only |
| `lane` | Building one brief in a worktree |
| `reviewer` | One independent review, then the fixes of its own minor findings |
| `checker` | Judging what users see, on offscreen renders at 1280 × 800 and 1920 × 1080 |

Pass the model on every call. Haiku and Sonnet for lookups, surveys and mechanical edits; Opus where a mistake is
costly or the work is open design. A survey's report is a lead, not a finding: check its numbers before you build on
them.

## What users see

- Never drive the owner's desktop. Render offscreen (`elektrawatt --headless --script <s> --visual --frames <dir>`).
- Judge look and hierarchy at 1920 × 1080 and truncation at the 1280 × 800 floor. A floor fix scales; it never caps.
- A persona round is three or four runs, and it runs before any expensive step, never beside it.
- Compare an old and a new image side by side before you accept a changed snapshot.

## Machine

- At most three builds at once (8 jobs each); a lone build may use 16.
- Keep 150 GB free. Delete a worktree's `target` when its work is merged.
- The private notes folder beside the repository holds briefs, reports and archives. Nothing from it enters the
  repository.
