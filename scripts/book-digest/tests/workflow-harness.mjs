// Run: node scripts/book-digest/tests/workflow-harness.mjs
// Runs .claude/workflows/zt-book-digest.js with stand-ins for the Workflow runtime (agent, phase,
// log, parallel, pipeline): every agent returns a canned answer that fits its label or command. It
// checks that each mode runs to its end without a script error, calls its stages in order, and
// builds the commands it should (doc/digest-card-layers-proposal.md). No model and no zt call runs.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const source = fs.readFileSync(path.join(here, '..', '..', '..', '.claude', 'workflows', 'zt-book-digest.js'), 'utf8').replace(/^export /m, '')
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor

let planJobs = 21
function runtime(opts0 = {}) {
  const calls = []
  const managed = []
  const cleanups = []
  const logs = []
  const phases = []
  const commandOf = (prompt) => (String(prompt).match(/pwsh -NoProfile -File[^\n]*/) || [''])[0]
  const answer = (prompt, opts) => {
    const label = opts.label || ''
    const cmd = commandOf(prompt)
    calls.push({ label, phase: opts.phase, model: opts.model, cmd })
    const json = (o) => JSON.stringify(o)
    if (label === 'choose DeepSeek source') return { source: 'official', errors: '' }
    if (label === 'prepare book') return { ok: true, groups: 2, usage_before: { input_cache_miss: 0, cached_read: 0, output: 0, cost: 0, calls: 0 } }
    if (/^(digest|link) manager/.test(label)) {
      const groups = [...String(prompt).matchAll(/Group (\d+):/g)].map(m => Number(m[1]))
      const down = /^digest/.test(label) && opts0.digestFails
      return { reports: groups.map(g => ({ group: g, status: down ? 'failed' : 'done', cards: down ? 0 : 3, models: 0, events: 0, links: 2, topics: [], errors: '无', archived: true })) }
    }
    if (/^find models/.test(label)) return { ok: true, models: [] }
    if (label === 'prepare link pass') return { ok: true, pending: [1, 2], owned_total: 6, unowned: 0 }
    if (label === 'cleanup DeepSeek') return { late_archived: 0, workspace_removed: true, files_removed: 0 }
    if (label === 'show figures') return { ok: true, output: json({ shown: 0, cards: 0, figures: 0, labelled: 0, literature: false }) }
    if (label === 'audit cards') return { ok: true, output: json({ hubs_missing: 0, broken_links: 0, books: [{ key: 'Key', cards: 9, topic_trees: 1, style_faults: 0, orphans: 0, no_source: 0, unkinded: 0, models: 0, not_self_sufficient: 0, figures_missing: 0 }] }) }
    if (label === 'organize: survey') return { ok: true, output: json({ survey: 'x', topics: 1, pairs: 0 }) }
    if (label === 'organize: decisions') return { thesis: '论点', merges: 0, moves: 0, notes: 1, report: '无' }
    if (label === 'organize: apply') return { ok: true, output: json({ merges: 0, moves: 0, refused: 0, renamed: 0 }) }
    if (label === 'hub manager: survey and limits') return { ok: true, survey: '{}', limits: { topic_min: 8, line_min: 10, max_lines: 20, max_groups: 7, min_reason: 8, neighbour_links: 3, field_min: 3, field_max: 8, per_job: 4 }, reasons: '' }
    if (String(prompt).startsWith('You drive DeepSeek Harness')) {
      // A manager of jobs: it reports every job it was given.
      if (/Run every job below/.test(prompt)) {
        const given = JSON.parse(String(prompt).split('\n').find(l => l.startsWith('[{')) || '[]')
        managed.push({ label, jobs: given.map(j => j.id), given })
        return { ok: true, output: '', results: given.map(j => ({ id: j.id, session_id: `s-${j.id}`, status: 'done', answer: '{}' })) }
      }
      if (/^Clean up:/m.test(prompt)) { cleanups.push(label); return { ok: true, results: [], archived: 3 } }
      // A compiler step: the JSON line the script prints, or a crash the test asks for.
      if (opts0.failStep && opts0.failStep.test(cmd)) return { ok: false, error: 'stand-in crash', output: '', results: [] }
      let output = json({ jobs: [] })
      const compact = / -Compact/.test(cmd)
      const head = { dir: 'D:\\x\\Key\\cards', lead: { fix: 'D:\\x\\Key\\cards\\card.md', orphans: 'D:\\x\\Key\\cards\\orphan.md' }, prompts: { fix: 'pf', orphans: 'po', follow: 'pw' } }
      if (compact && /card-compiler\.ps1" -Step plan/.test(cmd)) output = json({ picked: 40, with_faults: 30, faults: 90, review: 1, orphans: 1, ...head, jobs: [...Array.from({ length: planJobs - 1 }, (_, i) => ({ id: `fix-${String(i + 1).padStart(2, '0')}`, kind: 'fix', file: `fix-${String(i + 1).padStart(2, '0')}.md` })), { id: 'orphans-99', kind: 'orphans', file: 'orphans-99.md' }] })
      else if (compact && /card-compiler\.ps1" -Step check/.test(cmd)) output = json({ faults: 1, orphans: 0, ...head, jobs: [{ id: 'fix-01-r2', kind: 'follow', file: 'fix-01-r2.md', continue: 'fix-01' }] })
      else if (/card-compiler\.ps1" -Step plan/.test(cmd)) output = json({ picked: 40, with_faults: 30, faults: 90, review: 1, orphans: 0, jobs: Array.from({ length: planJobs }, (_, i) => ({ id: `fix-${String(i + 1).padStart(2, '0')}`, files: ['a', 'b'], prompt: 'p' })) })
      else if (/card-compiler\.ps1" -Step apply/.test(cmd)) output = json({ applied_jobs: 1, counts: { fix: 2 } })
      else if (/card-compiler\.ps1" -Step check/.test(cmd)) output = json({ jobs: [], faults: 0, orphans: 0 })
      else if (/card-compiler\.ps1" -Step report/.test(cmd)) output = json({ cards: 2, faults_before: 3, faults_after: 0, verdicts: { fix: 2 }, splits: 0, merges: 0, refused: 0, orphan_links: 0, orphans_after: 0 })
      else if (/book-terms\.ps1" -Step plan/.test(cmd)) output = json({ bold: 2, definitions: 1, jobs: [{ id: 'terms', files: ['a', 'b'], prompt: 'p' }] })
      else if (/book-terms\.ps1" -Step apply/.test(cmd)) output = json({ ok: true, terms: 2, with_avoid: 1, card: 'Key/1|a' })
      else if (/book-skeleton\.ps1" -Step plan/.test(cmd)) output = json({ groups: 2, jobs: [{ id: 'skeleton', files: ['a', 'b'], prompt: 'p' }] })
      else if (/book-skeleton\.ps1" -Step apply/.test(cmd)) output = json({ ok: true, thesis: '论点', argument: 2, models: 0, events: 0, topics: 1, written: { structure: 'Key/1|b' } })
      else if (/hub-compiler\.ps1" -Step plan/.test(cmd)) output = json({ tree_count: 1, jobs: [{ id: 'h', files: [], prompt: 'p' }] })
      else if (/hub-compiler\.ps1" -Step check/.test(cmd)) output = json({ hubs: 1, faults: 0, jobs: [] })
      else if (/hub-compiler\.ps1" -Step report/.test(cmd)) output = json({ counts: {}, faults: {}, mirror: 0, missing_count: 0 })
      return { ok: true, output, results: [], archived: 0 }
    }
    return {}
  }
  return {
    calls, logs, phases, managed, cleanups,
    agent: async (prompt, opts = {}) => answer(prompt, opts),
    phase: (p) => phases.push(p),
    log: (m) => logs.push(m),
    parallel: async (fns) => Promise.all(fns.map(fn => fn())),
    pipeline: async (items, ...stages) => Promise.all(items.map(async (item, i) => {
      let value = item
      for (const stage of stages) value = await stage(value, item, i)
      return value
    })),
  }
}

async function run(args, opts0) {
  const rt = runtime(opts0)
  const fn = new AsyncFunction('args', 'agent', 'phase', 'log', 'parallel', 'pipeline', source)
  const result = await fn(args, rt.agent, rt.phase, rt.log, rt.parallel, rt.pipeline)
  return { result, ...rt }
}

let failed = 0
const check = (cond, message) => { console.log(`${cond ? 'ok' : 'FAIL'}: ${message}`); if (!cond) failed++ }
const base = { out: 'D:\\x\\Key', citationKey: 'Key', source: 'S.md', bibtex: '@book{Key}', bookTitle: '书', hubs: false }

// full: Prepare, terms, digest with models and card cycles, link with the link check, organize, audit
{
  const { result, calls, phases, logs } = await run({ ...base, mode: 'full', figures: false })
  const labels = calls.map(c => c.label)
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  check(result && result.mode === 'full', 'full: runs to its end')
  check(labels.indexOf('terms: plan') > labels.indexOf('prepare book') && labels.indexOf('terms: plan') < labels.findIndex(l => /^digest manager/.test(l)), 'full: the term table runs after Prepare and before the digest')
  check(cmds.some(c => /card-compiler\.ps1" -Step plan .* -Book Key -Staging "D:\\x\\Key".* -Groups "1,2" -Sample 0\.1 -NoOrphans/.test(c)), 'full: a card cycle per digest block, with -Groups, the sample, and no orphans')
  check(cmds.some(c => /-Step plan -Work "[^"]*cards-link"/.test(c)) && !cmds.some(c => /cards-link".*-Rules/.test(c)), 'full: the link check runs with the default hard rules (the link pass may add R2, R4, I2 faults)')
  check(cmds.filter(c => /-Rules/.test(c)).every(c => (c.match(/-Rules/g) || []).length === 1), 'full: no command passes -Rules twice')
  check(labels.filter(l => /^link manager/.test(l)).length === 1 && labels.includes('link manager 1: groups 1-2'), 'full: one link manager runs both groups')
  check(labels.includes('organize: survey') && labels.includes('organize: decisions') && labels.includes('organize: apply'), 'full: Organize runs the survey, the decisions, and the apply')
  check(calls.find(c => c.label === 'organize: decisions').model === 'opus', 'full: Opus makes the decisions')
  check(cmds.some(c => /cards-notes"/.test(c)) && cmds.some(c => /-Notes "D:\\x\\Key\\organize\\decisions\.json" -NoOrphans/.test(c)) && !cmds.some(c => /cards-notes".*-Rules/.test(c)), 'full: the notes go to a card cycle with the default hard rules (the script adds N1)')
  check(cmds.some(c => /book-skeleton\.ps1" -Step apply .* -Decisions "D:\\x\\Key\\organize\\decisions\.json" -Apply/.test(c)), 'full: the skeleton applies with the decisions')
  check(labels[labels.length - 1] === 'audit cards', 'full: the audit runs last')
  check(result.terms && result.terms.ok && result.cards && result.cards.blocks.length === 1 && result.cards.link && result.organize && result.organize.skeleton.ok && result.audit.status === 'done', 'full: the result holds terms, cards, organize, and audit')
  check(phases.includes('Cards') || calls.some(c => c.phase === 'Cards'), 'full: the card cycle runs in the Cards phase')
  check(!logs.some(l => /failed(?! none)/.test(l)), `full: no stage logs a failure${logs.filter(l => /failed(?! none)/.test(l)).map(l => ` (${l})`).join('')}`)
}
// legacy Organize keeps the old single Opus job
{
  const { calls } = await run({ ...base, mode: 'full', figures: false, organize: 'legacy', terms: false, cardChecks: false, audit: false })
  const labels = calls.map(c => c.label)
  check(labels.includes('organize card structure') && !labels.includes('organize: survey'), 'legacy: one Opus job, no survey')
  check(!labels.some(l => /^terms:/.test(l)) && !calls.some(c => /card-compiler/.test(c.cmd)), 'legacy: terms: false and cardChecks: false skip their stages')
}
// cards: only the card cycle on the whole book, with orphans
{
  const { result, calls, managed, cleanups } = await run({ out: 'D:\\x\\Key', citationKey: 'Key', mode: 'cards', cardMax: 5 })
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  check(result && result.mode === 'cards' && result.cards.status === 'done', 'cards: runs to its end')
  check(cmds.some(c => /card-compiler\.ps1" -Step plan -Work "D:\\x\\Key\\cards" -Compact -Book Key -Staging "D:\\x\\Key" -MaxCards 5/.test(c)) && !cmds.some(c => /-NoOrphans/.test(c)), 'cards: plans the whole book with orphans and the pilot cap')
  check(!calls.some(c => /^(digest|link) manager|prepare book|organize/.test(c.label)), 'cards: runs no other stage')
  const planManagers = managed.filter(m => /cards whole book: plan/.test(m.label))
  const planJobs = planManagers.flatMap(m => m.jobs)
  check(planManagers.length === 3, `cards: 21 jobs go to 3 Haiku managers, one per 8 jobs (${planManagers.length})`)
  check(Math.max(...planManagers.map(m => m.jobs.length)) <= 8, 'cards: no manager holds more than 8 jobs when 64 or fewer')
  check(planJobs.length === 21 && new Set(planJobs).size === 21, 'cards: every job goes to exactly one manager')
  check(calls.filter(c => /cards whole book: plan/.test(c.label)).every(c => c.model === 'haiku'), 'cards: the step agent and the managers are Haiku')
  check(cleanups.some(l => /cards whole book: report: clean up/.test(l)), 'cards: one cleanup agent ends the conversations after the report')
  check(result.cards.plan && result.cards.plan.jobs.length === 21 && result.cards.plan.jobs.every(j => Array.isArray(j.files)), 'cards: the plan reaches the stage intact, its jobs expanded')
  const given = planManagers.flatMap(m => m.given)
  const fix = given.find(j => j.id === 'fix-01')
  const orphan = given.find(j => j.id === 'orphans-99')
  check(fix && fix.files[0] === 'D:\\x\\Key\\cards\\card.md' && fix.files[1] === 'D:\\x\\Key\\cards\\fix-01.md' && fix.prompt === 'pf', `cards: a compact fix job expands to its instruction file, job file, and prompt (${JSON.stringify(fix)})`)
  check(orphan && orphan.files[0] === 'D:\\x\\Key\\cards\\orphan.md' && orphan.prompt === 'po', 'cards: a compact orphan job takes the orphan instructions')
  const follow = managed.filter(m => /check 1/.test(m.label)).flatMap(m => m.given).find(j => j.id === 'fix-01-r2')
  check(follow && follow.continue === 'fix-01' && follow.files.length === 1 && follow.fallback[0] === 'D:\\x\\Key\\cards\\card.md' && follow.prompt === 'pw', `cards: a compact follow-up continues its job, with the instructions as fallback (${JSON.stringify(follow)})`)
  check(calls.filter(c => /card-compiler\.ps1/.test(c.cmd)).every(c => / -Compact/.test(c.cmd)), 'cards: every card-compiler step gets -Compact')
}
// the managers: one per 8 jobs, at most 8
for (const [n, want] of [[5, 1], [16, 2], [17, 3], [70, 8]]) {
  planJobs = n
  const { managed } = await run({ out: 'D:\\x\\Key', citationKey: 'Key', mode: 'cards' })
  const plan = managed.filter(m => /cards whole book: plan/.test(m.label))
  const all = plan.flatMap(m => m.jobs)
  check(plan.length === want && all.length === n && new Set(all).size === n, `managers: ${n} jobs go to ${want} managers (${plan.length}), each job once`)
}
planJobs = 21
// audit: one script, no DeepSeek source probe
{
  const { result, calls } = await run({ out: 'D:\\x\\audit', mode: 'audit' })
  check(result && result.mode === 'audit' && result.audit.status === 'done', 'audit: runs to its end without a citation key')
  check(calls.length === 1 && calls[0].label === 'audit cards', 'audit: one call, no source probe')
}
// link: the link check runs after the link pass unless cardChecks is false
{
  const { calls } = await run({ out: 'D:\\x\\Key', citationKey: 'Key', mode: 'link', groups: 2, hubs: false })
  check(calls.some(c => /cards-link"/.test(c.cmd)), 'link: the link check runs after the link pass')
  check(calls.some(c => /cards-link".*-Rules "L1,L2"/.test(c.cmd)), 'link: on a book without card cycles the link check keeps rules L1 and L2')
}
// full: when every digest group fails (zt or DeepSeek Harness down), the run stops before the link pass
{
  const { result, calls } = await run({ ...base, mode: 'full', figures: false }, { digestFails: true })
  const labels = calls.map(c => c.label)
  check(result && result.stage === 'digest' && /every digest group failed/.test(result.error || ''), `full: a digest with no group done stops with an error (${JSON.stringify(result && { stage: result.stage, error: result.error })})`)
  check(!labels.some(l => /^link manager|^organize|^hub|audit cards/.test(l)), 'full: no link, Organize, hub, or audit stage runs after it')
}
// full: a crashed apply step makes its stage "failed", not "done"
{
  const { result } = await run({ ...base, mode: 'full', figures: false, hubs: true }, { failStep: /(hub|card)-compiler\.ps1" -Step apply/ })
  check(result && result.hubs && result.hubs.status === 'failed' && /apply/.test(result.hubs.error || ''), `full: a hub apply crash fails the hub stage (${JSON.stringify(result && result.hubs && { status: result.hubs.status, error: result.hubs.error })})`)
  check(result && result.cards && result.cards.blocks.every(b => b.status === 'failed'), 'full: a card apply crash fails its card cycle')
}
console.log(`failed: ${failed}`)
process.exit(failed ? 1 : 0)
