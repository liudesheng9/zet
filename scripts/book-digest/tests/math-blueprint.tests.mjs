// Run: node scripts/book-digest/tests/math-blueprint.tests.mjs
// The math blueprint's stage predicates (docs/math-digest-redesign-proposal.md, Bug H): which mathematics
// stages run in each mode, evaluated with the operators the framework's `when` lists use.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const blueprint = JSON.parse(fs.readFileSync(path.join(here, '..', 'workflow', 'zt-math-digest.blueprint.json'), 'utf8'))

const get = (ctx, p) => p.split('.').reduce((v, k) => (v == null ? undefined : v[k]), ctx)
function holds(cond, ctx) {
  switch (cond.op) {
    case 'and': return cond.value.every((c) => holds(c, ctx))
    case 'or': return cond.value.some((c) => holds(c, ctx))
    case '==': return get(ctx, cond.path) === cond.value
    case '!=': return get(ctx, cond.path) !== cond.value
    case 'matches': return new RegExp(cond.value).test(String(get(ctx, cond.path) ?? ''))
    case 'nonempty': { const v = get(ctx, cond.path); return v != null && String(v).trim() !== '' }
    default: throw new Error(`unknown op ${cond.op}`)
  }
}
const stages = Object.values(blueprint.fill).flat()
const stage = (id) => stages.find((s) => s.id === id) || (() => { throw new Error(`no stage ${id}`) })()
const runs = (id, ctx) => (stage(id).when || []).every((c) => holds(c, ctx))

let failed = 0
function check(ok, what) { console.log(`${ok ? 'ok' : 'FAIL'}: ${what}`); if (!ok) failed++ }

const prepared = { stages: { prepare: { output: { citation_key: 'thompson1914calculus' } } } }
const ctx = (mode, args = {}) => ({ mode, args, ...prepared })

// Every stage mode `math` lists runs in mode math: the repair mode can rebuild missing units.
for (const id of blueprint.modes.math.filter((id) => stages.some((s) => s.id === id))) {
  check(runs(id, ctx('math')), `mode math runs ${id}`)
}
// The math check keys on the prepared Citation key, not the raw argument (a resume carries none).
check(runs('math-check', ctx('full', { citationKey: '' })), 'math-check runs in mode full from the prepared key')
check(!runs('math-check', { mode: 'full', args: { citationKey: 'x' }, stages: { prepare: { output: {} } } }), 'math-check without a prepared key does not run')
check(!runs('math-check', ctx('full', { audit: false })), 'args.audit false still turns the check off')
// The unit pipeline (models) runs in a full run by default.
check(runs('models', ctx('full')), 'mode full runs the model layer')

if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log('all passed')
