# ElektraWatt — notes for Claude

Open-source (GPL-3.0-or-later app; MIT OR Apache-2.0 format crates) cross-platform desktop ECAD application in Rust
for electrical control-cabinet design. Currently in the design phase: see `docs/` (start at `docs/README.md`).

## Model routing policy (quota)

Use the cheapest model that can do the task well. Three tiers, each with project subagents in `.claude/agents/`:

| Tier | Model | Subagents | Use for |
|---|---|---|---|
| Lead | Opus | `lead-designer`, `lead-programmer` | Open decisions: architecture, ADRs, domain-model and file-format changes, cross-document conflicts, estimation/planning, judging alternatives; core/critical code (ew-model, ew-commands, ew-memo, ew-derive, ew-format/ew-store, concurrency, FFI/STEP worker, public & plugin APIs, perf-critical rendering); hard debugging; final review |
| Worker | Sonnet | `designer`, `programmer` | Well-defined work with a closed spec: write/update a doc section to an outline, apply decided resolutions, write an ADR for a made decision; implement a specified command/query/importer step/report/UI panel/CLI subcommand, tests, bug fixes with a known repro; research and literature sweeps |
| Mechanical | Haiku | `boilerplate` | No judgment needed: scaffolding from a template, Cargo.toml edits, licence/REUSE headers, renames and ID-map find-and-replace, fmt/trivial lint fixes, fixtures, CHANGELOG/README, link and Mermaid fixes, simple lookups |

Rules:
1. **The lead decides, workers execute.** Opus turns open questions into closed specs (inputs, outputs, files owned,
   acceptance checks); Sonnet/Haiku execute them. Do not give an open-ended design question to a worker tier.
2. **Escalate, don't guess.** A worker that hits an undecided choice stops and reports it; Haiku escalates to Sonnet
   when a task needs judgment or fails verification twice; Sonnet escalates to Opus for cross-cutting or
   architectural impact.
3. **Workflows:** set `model` on every `agent()` call. Fan-out stages (per-file fixes, per-section writing to a fixed
   outline, per-item verification, sweeps) → `sonnet`; mechanical stages → `haiku` with `effort: 'low'`;
   judge/synthesis/triage/critic stages → `opus`. Keep Opus to one or a few agents per workflow.
4. **Effort:** `low` for mechanical work, `medium` for routine worker tasks, default/`high` only for lead work.
5. **Ad-hoc subagents:** pass `model` to the Agent tool (or use the role subagents above). Codebase searches via
   `Explore` should use `haiku` or `sonnet`.
6. **Verification is cheap, rework is not:** always end worker tasks with a mechanical check (build, tests, lint,
   link/Mermaid check, ID-existence grep) so failures surface before an Opus review.

## Repository rules

- Commit on the working branch, then merge into `main` and push both (owner's standing instruction).
- Public repository: never commit customer or end-user names, addresses, project or drawing numbers, personal names,
  e-mail addresses, the reference drawing PDF, or local filesystem paths.
- Design decisions live in `docs/adr/`; requirements in `docs/design/02a/02b/02c-*.md`; the plan in `docs/plan/`.
