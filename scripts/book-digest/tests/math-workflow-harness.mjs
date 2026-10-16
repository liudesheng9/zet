// Run: node scripts/book-digest/tests/math-workflow-harness.mjs
// Runs .claude/workflows/zt-math-digest.js with stand-ins for the Workflow runtime, as
// workflow-harness.mjs does for zt-book-digest.js: every agent returns a canned answer that fits
// its label or command. It checks that the math workflow is generated from the current book digest
// (make-math-workflow.mjs --check), that its math stages run in order, that the shared stages
// build the same commands as the book digest (the same Card, Link, hub, and Organize standards),
// and that every DeepSeek manager follows the DeepSeek Subagent Managed Protocol.
import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..', '..', '..')
const read = (name) => fs.readFileSync(path.join(root, '.claude', 'workflows', name), 'utf8').replace(/^export /m, '')
const sources = { math: read('zt-math-digest.js'), book: read('zt-book-digest.js') }
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor

function runtime() {
  const calls = []
  const managed = []
  const cleanups = []
  const logs = []
  const phases = []
  const pool = { alive: 0, peak: 0, prompts: [] }
  const commandOf = (prompt) => (String(prompt).match(/pwsh -NoProfile -File[^\n]*/) || [''])[0]
  const json = (o) => JSON.stringify(o)
  const job = (id) => ({ id, files: ['a', 'b'], prompt: 'p' })
  const answer = (prompt, opts) => {
    const label = opts.label || ''
    const cmd = commandOf(prompt)
    calls.push({ label, phase: opts.phase, model: opts.model, cmd, prompt: String(prompt) })
    if (label === 'choose DeepSeek source') return { source: 'official', errors: '' }
    if (label === 'prepare book') return { ok: true, groups: 2, usage_before: { input_cache_miss: 0, cached_read: 0, output: 0, cost: 0, calls: 0 } }
    if (label === 'math: index') return { ok: true, output: json({ ok: true, units: 5, forms: { theorem: 1, lemma: 1 }, examples: 1, exercises: 1 }) }
    if (label === 'math: docs') return { ok: true, output: json({ ok: true, files: [] }) }
    if (label === 'math: skeleton') return { ok: true, output: json({ ok: true, units: 5, created: 5, theories: 2 }) }
    if (label === 'math: check') return { ok: true, output: json({ ok: true, units: 5, theories: 2, faults: 0, by_rule: {} }) }
    if (/^(digest|link) manager/.test(label)) {
      const groups = [...String(prompt).matchAll(/Group (\d+):/g)].map(m => Number(m[1]))
      return { reports: groups.map(g => ({ group: g, status: 'done', cards: 3, models: 1, events: 0, links: 2, topics: [], errors: '无', deleted: true })) }
    }
    if (/^find models/.test(label)) return { ok: true, models: [{ address: 'Key/2|e', title: '理论', form: 'theory', level: null, parts: 0 }, { address: 'Key/2|c', title: '介值定理', form: 'theorem', level: 'formal', parts: 0 }] }
    if (/^plan: /.test(label)) return { plans: [{ model: 'Key/2|c', status: 'planned', level: 'formal', parts: 4, derivations: 1 }] }
    if (label === 'prepare link pass') return { ok: true, pending: [1, 2], owned_total: 6, unowned: 0 }
    if (label === 'cleanup DeepSeek') return { late_deleted: 0, workspace_removed: true, files_removed: 0 }
    if (label === 'cleanup model managers') return { late_deleted: 0, folders: 1 }
    if (label === 'show figures') return { ok: true, output: json({ shown: 0, cards: 0, figures: 0, labelled: 0, literature: false }) }
    if (label === 'audit cards') return { ok: true, output: json({ hubs_missing: 0, broken_links: 0, books: [] }) }
    if (label === 'organize: survey') return { ok: true, output: json({ survey: 'x', topics: 1, pairs: 0 }) }
    if (label === 'organize: decisions') return { thesis: '论点', merges: 0, moves: 0, notes: 1, report: '无' }
    if (label === 'organize: apply') return { ok: true, output: json({ merges: 0, moves: 0, refused: 0, renamed: 0 }) }
    if (label === 'hub manager: survey and limits') return { ok: true, survey: '{}', limits: { topic_min: 8, line_min: 10, max_lines: 20, max_groups: 7, min_reason: 8, neighbour_links: 3, field_min: 3, field_max: 8, per_job: 4 }, reasons: '' }
    if (String(prompt).startsWith('You drive DeepSeek Harness')) {
      if (/Run every job below/.test(prompt)) {
        const given = JSON.parse(String(prompt).split('\n').find(l => l.startsWith('[{')) || '[]')
        managed.push({ label, jobs: given.map(j => j.id), given })
        return { ok: true, output: '', results: given.map(j => ({ id: j.id, session_id: `s-${j.id}`, status: 'done', answer: '{}' })) }
      }
      if (/^Clean up:/m.test(prompt)) { cleanups.push(label); return { ok: true, results: [], deleted: 2 } }
      let output = json({ jobs: [] })
      if (/model-compiler\.ps1" -Step brief/.test(cmd)) output = json({ jobs: [job('brief')], skipped: [] })
      else if (/model-compiler\.ps1" -Step report/.test(cmd)) output = json({ models: [{ model: 'Key/2|c', status: 'done', level: 'formal', parts: [], faults: {} }] })
      else if (/card-compiler\.ps1" -Step plan/.test(cmd)) output = json({ picked: 4, with_faults: 1, faults: 1, review: 0, orphans: 0, jobs: [job('fix-01')] })
      else if (/card-compiler\.ps1" -Step apply/.test(cmd)) output = json({ applied_jobs: 1, pending_jobs: 0, counts: {} })
      else if (/card-compiler\.ps1" -Step check/.test(cmd)) output = json({ jobs: [], faults: 0, orphans: 0 })
      else if (/card-compiler\.ps1" -Step report/.test(cmd)) output = json({ cards: 2, faults_before: 1, faults_after: 0, verdicts: {}, splits: 0, merges: 0, refused: 0, orphan_links: 0, orphans_after: 0 })
      else if (/book-terms\.ps1" -Step plan/.test(cmd)) output = json({ bold: 2, definitions: 1, jobs: [job('terms')] })
      else if (/book-terms\.ps1" -Step apply/.test(cmd)) output = json({ ok: true, terms: 2, with_avoid: 1, card: 'Key/1|a' })
      else if (/book-skeleton\.ps1" -Step plan/.test(cmd)) output = json({ groups: 2, jobs: [job('skeleton')] })
      else if (/book-skeleton\.ps1" -Step apply/.test(cmd)) output = json({ ok: true, thesis: '论点', argument: 2, models: 0, events: 0, topics: 1, written: { structure: 'Key/1|b' } })
      else if (/hub-compiler\.ps1" -Step plan/.test(cmd)) output = json({ tree_count: 1, jobs: [job('h')] })
      else if (/hub-compiler\.ps1" -Step check/.test(cmd)) output = json({ hubs: 1, faults: 0, jobs: [] })
      else if (/hub-compiler\.ps1" -Step report/.test(cmd)) output = json({ counts: {}, faults: {}, mirror: 0, missing_count: 0 })
      else if (/math-compiler\.ps1" -Step notation /.test(cmd + ' ')) output = json({ candidates: 3, jobs: [job('notation')] })
      else if (/math-compiler\.ps1" -Step notation-apply/.test(cmd)) output = json({ ok: true, symbols: 3, card: 'Key/1|c' })
      else if (/math-compiler\.ps1" -Step boundary /.test(cmd + ' ')) output = json({ units: 2, jobs: [job('b-01')] })
      else if (/math-compiler\.ps1" -Step boundary-apply/.test(cmd)) output = json({ created: 3, skipped_supplied: 0, jobs: [job('v-b-01')] })
      else if (/math-compiler\.ps1" -Step boundary-settle/.test(cmd)) output = json({ kept: 1, deleted: 0, parts: [] })
      else if (/math-compiler\.ps1" -Step proof /.test(cmd + ' ')) output = json({ units: 2, jobs: [job('p-01')] })
      else if (/math-compiler\.ps1" -Step proof-apply/.test(cmd)) output = json({ ok: 3, fixed: 1, refused: 0, parts: [] })
      else if (/math-compiler\.ps1" -Step theory /.test(cmd + ' ')) output = json({ theories: 2, jobs: [job('t-01')] })
      else if (/math-compiler\.ps1" -Step theory-apply/.test(cmd)) output = json({ ok: true, theories: 2, answered: 2 })
      return { ok: true, output, results: [], deleted: 0 }
    }
    return {}
  }
  return {
    calls, logs, phases, managed, cleanups, pool,
    agent: async (prompt, opts = {}) => {
      const manager = /^You manage \d+ DeepSeek/.test(prompt) || /Run every job below/.test(prompt)
      if (!manager) return answer(prompt, opts)
      pool.prompts.push(String(prompt))
      pool.alive++
      pool.peak = Math.max(pool.peak, pool.alive)
      await new Promise(resolve => setTimeout(resolve, 3))
      try { return answer(prompt, opts) } finally { pool.alive-- }
    },
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
async function run(which, args) {
  const rt = runtime()
  const fn = new AsyncFunction('args', 'agent', 'phase', 'log', 'parallel', 'pipeline', sources[which])
  const result = await fn(args, rt.agent, rt.phase, rt.log, rt.parallel, rt.pipeline)
  return { result, ...rt }
}

let failed = 0
const check = (cond, message) => { console.log(`${cond ? 'ok' : 'FAIL'}: ${message}`); if (!cond) failed++ }
const base = { out: 'D:\\x\\Key', citationKey: 'Key', source: 'S.md', bibtex: '@book{Key}', bookTitle: '书', hubs: true, figures: false }

// The math workflow is the book digest plus the math stages, generated from the current book digest.
try {
  execFileSync(process.execPath, [path.join(root, 'scripts', 'book-digest', 'make-math-workflow.mjs'), '--check'], { stdio: 'pipe' })
  check(true, 'generator: zt-math-digest.js is generated from the current zt-book-digest.js')
} catch (error) {
  check(false, `generator: zt-math-digest.js is out of date (${String(error.stdout || error.message).trim()})`)
}

// full: the math stages in order, around the shared stages.
{
  const { result, calls, logs, cleanups, pool, managed } = await run('math', { ...base, mode: 'full' })
  const labels = calls.map(c => c.label)
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  const at = (pattern) => labels.findIndex(l => pattern.test(l))
  check(result && result.mode === 'full' && result.math, 'full: runs to its end and reports the math stages')
  check(at(/^prepare book$/) < at(/^math: index$/) && at(/^math: index$/) < at(/^math: docs$/) && at(/^math: docs$/) < at(/^math: skeleton$/), 'full: index, docs, and skeleton follow Prepare')
  check(at(/^math: skeleton$/) < at(/^notation: plan/) && at(/^notation: apply/) < at(/^terms: plan/) && at(/^terms: plan/) < at(/^digest manager/), 'full: the notation table, then the term table, before the digest')
  const digestPrompt = calls.find(c => /^digest manager/.test(c.label)).prompt
  check(/math-prefix\.md/.test(digestPrompt) && !/shared-prefix\.md/.test(digestPrompt), 'full: the digest runs read the math prefix (the shared prefix with the math supplement)')
  check(/math-link-prefix\.md/.test(calls.find(c => /^link manager/.test(c.label)).prompt), 'full: the link runs read the math link prefix')
  const compiler = cmds.filter(c => /model-compiler\.ps1/.test(c))
  check(compiler.length > 0 && compiler.every(c => /-BriefDoc "D:\\x\\Key\\math\\brief\.zh\.md" -FillDoc "D:\\x\\Key\\math\\fill\.zh\.md" -ReviewDoc "D:\\x\\Key\\math\\review\.zh\.md"/.test(c)), 'full: every model-compiler step takes the math instruction files')
  check(!compiler.some(c => /Key\/2\|e/.test(c)), 'full: the model compiler never builds a Theory Card')
  check(/model-architect-math\.md/.test(calls.find(c => /^plan: /.test(c.label)).prompt), 'full: the architect reads the math supplement')
  check(at(/^boundary: write/) > at(/^find models: sweep/) && at(/^boundary: settle/) < at(/^proof: review/) && at(/^proof: apply/) < at(/^link manager/), 'full: Boundary and Proof run after the models and before the link pass')
  check(at(/^theory: write/) > at(/^cards link check: report/) && at(/^theory: apply/) < at(/^cleanup DeepSeek$/), 'full: Theory runs after the link check and before the cleanup')
  check(at(/^math: check$/) > at(/^audit cards$/), 'full: the math check runs after the audit')
  check(['notation: apply', 'boundary: settle', 'proof: apply', 'theory: apply'].every(s => cleanups.some(l => l.startsWith(`${s}: clean up`))), 'full: each math stage\'s last step deletes its DeepSeek conversations')
  check(pool.prompts.every(p => /deepseek_delete/.test(p) && !/deepseek_archive/.test(p)), `protocol: every DeepSeek manager deletes, none archives (${pool.prompts.length} managers)`)
  check(pool.peak <= 8 && managed.every(m => m.jobs.length <= 8), `protocol: at most 8 managers at once and 8 jobs per manager (peak ${pool.peak})`)
  check(calls.filter(c => /^(boundary|proof|theory|notation):/.test(c.label)).every(c => c.model === 'haiku'), 'protocol: Haiku runs every math step and manager')
  check(!logs.some(l => /failed(?! none)/.test(l)), `full: no stage logs a failure${logs.filter(l => /failed(?! none)/.test(l)).map(l => ` (${l})`).join('')}`)
}

// The shared standards: the card cycles, the link check, Organize, hubs, and the audit build the
// same commands as in the book digest.
{
  const shared = /card-compiler\.ps1|hub-compiler\.ps1|book-skeleton\.ps1|organize-(survey|apply)\.ps1|digest-audit\.ps1|book-terms\.ps1|prepare-link-pass\.ps1|prepare-book-digest\.ps1/
  const math = (await run('math', { ...base, mode: 'full' })).calls.map(c => c.cmd).filter(c => shared.test(c))
  const book = (await run('book', { ...base, mode: 'full' })).calls.map(c => c.cmd).filter(c => shared.test(c))
  const sort = (list) => [...list].sort()
  check(math.length > 10 && JSON.stringify(sort(math)) === JSON.stringify(sort(book)), `standards: the shared stages build the same ${book.length} commands in both workflows`)
}

// math: only the math stages on an existing digest.
{
  const { result, calls } = await run('math', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'math' })
  const labels = calls.map(c => c.label)
  check(result && result.mode === 'math' && result.math.boundary && result.math.proof && result.math.theory && result.math.check, 'math: runs Boundary, Proof, Theory, and the check')
  check(!labels.some(l => /^(digest|link) manager|^prepare book|^organize|^hub manager/.test(l)), 'math: runs no other stage')
}
// supply none and the stage switches.
{
  const { calls } = await run('math', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'math', supply: 'none', proof: false })
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  check(cmds.some(c => /-Step boundary -Staging "D:\\x\\Key" -UnitsPerJob 4 -Supply none/.test(c)) && cmds.some(c => /-Step boundary-apply .* -Supply none/.test(c)), 'math: supply none reaches the boundary steps')
  check(!cmds.some(c => /-Step proof/.test(c)), 'math: proof false skips the proof review')
}
console.log(`failed: ${failed}`)
process.exit(failed ? 1 : 0)
