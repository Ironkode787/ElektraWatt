export const meta = {
  name: 'routed-tasks',
  description: 'Run independent tasks with model/effort chosen per task from estimated difficulty; escalate one level on failure',
  whenToUse: 'Fan-out of independent, self-contained tasks (doc fixes, small features, migrations) where difficulty varies per item',
  phases: [
    { title: 'Triage', detail: 'one Haiku call scores difficulty D1–D5 for unscored tasks', model: 'haiku' },
    { title: 'Execute', detail: 'each task runs at its level; blocked/failed tasks retry one level up' },
  ],
}

// args: { tasks: [{ id: string, prompt: string, difficulty?: 1..5 }], maxEscalations?: number }
// Each task prompt must be self-contained: goal, files owned, acceptance checks to run.
// Ladder and rubric: this file; model and effort rules: CLAUDE.md "Orchestration".
const LADDER = [
  null,
  { model: 'haiku', effort: 'low' },     // D1 trivial / mechanical
  { model: 'sonnet', effort: 'medium' }, // D2 routine, closed spec
  { model: 'sonnet', effort: 'high' },   // D3 moderate: several files, judgment within a decided design
  { model: 'opus', effort: 'high' },     // D4 hard: open choice, core code, cross-cutting
  { model: 'opus', effort: 'xhigh' },    // D5 critical: format, data loss, safety, security, irreversible
]
const tasks = (args && args.tasks) || []
const maxEsc = (args && Number.isInteger(args.maxEscalations)) ? args.maxEscalations : 2
if (!tasks.length) return { error: 'args.tasks is empty' }

const RUBRIC = `Score each task D1..D5. Start at D1 and add one level for each "yes" (cap at D5):
(1) Is the spec open — a design choice is not yet decided? (2) Does it touch more than 3 files, or a public API or
file format? (3) Is there no existing pattern to copy? (4) Would a mistake be expensive or hard to detect (data loss,
safety, legal)? (5) Is verification weak (no tests or checks to run)?`

phase('Triage')
const unscored = tasks.filter(t => !(t.difficulty >= 1 && t.difficulty <= 5))
const scores = {}
if (unscored.length) {
  const tri = await agent(`${RUBRIC}

Tasks (score each; do not perform them):
${unscored.map(t => `--- id: ${t.id}\n${t.prompt.slice(0, 1500)}`).join('\n')}`, {
    label: 'triage:difficulty', phase: 'Triage', model: 'haiku', effort: 'low',
    schema: {
      type: 'object',
      properties: { scores: { type: 'array', items: { type: 'object',
        properties: { id: { type: 'string' }, level: { type: 'integer', minimum: 1, maximum: 5 }, reason: { type: 'string' } },
        required: ['id', 'level', 'reason'] } } },
      required: ['scores'],
    },
  })
  for (const s of (tri && tri.scores) || []) scores[s.id] = s
}

const RESULT = {
  type: 'object',
  properties: {
    status: { type: 'string', enum: ['done', 'blocked', 'failed'] },
    summary: { type: 'string' },
    verification: { type: 'string', description: 'commands/checks run and their outcome' },
    notes_for_retry: { type: 'string', description: 'if not done: what went wrong / what decision is needed' },
  },
  required: ['status', 'summary', 'verification', 'notes_for_retry'],
}

phase('Execute')
const results = await pipeline(tasks, async t => {
  let level = (t.difficulty >= 1 && t.difficulty <= 5) ? t.difficulty : ((scores[t.id] && scores[t.id].level) || 2)
  const start = level
  let attempt = 0, notes = '', res = null
  while (true) {
    const { model, effort } = LADDER[level]
    res = await agent(`${t.prompt}

Finish by running the task's acceptance checks. Report status "done" only if they pass; "blocked" if a decision outside
your brief is needed; "failed" otherwise.${notes ? `\n\nA previous attempt at a lower level did not succeed:\n${notes}` : ''}`,
      { label: `${t.id}@D${level}`, phase: 'Execute', model, effort, schema: RESULT })
    if (res && res.status === 'done') break
    if (attempt >= maxEsc || level >= 5) break
    notes = res ? `${res.status}: ${res.summary}\n${res.notes_for_retry}` : 'agent returned no result'
    attempt++; level++
    log(`${t.id}: escalating to D${level} (${LADDER[level].model}/${LADDER[level].effort})`)
  }
  return { id: t.id, startLevel: start, finalLevel: level, reason: scores[t.id] ? scores[t.id].reason : 'preset', result: res }
})

const done = results.filter(r => r && r.result && r.result.status === 'done')
log(`${done.length}/${tasks.length} done; escalated: ${results.filter(r => r && r.finalLevel > r.startLevel).length}`)
return { results }
