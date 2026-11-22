// Run: node scripts/book-digest/tests/math-theory-module.tests.mjs
// zt-math-theory publishes its own revision after it rewrites the Theory Cards (Astra round 5, finding 4): without
// the publication, `theory/book@1` stays open and Organize waits on it forever. The module is loaded with a stub
// `zmMake`, so the test sees exactly the steps it runs.
import fs from 'node:fs'
import path from 'node:path'
import vm from 'node:vm'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const source = fs.readFileSync(path.join(here, '..', 'workflow', 'modules', 'zt-math-theory.js'), 'utf8')

function load(publishAnswer) {
  const calls = []
  let mod = null
  const M = {
    MATH_WORK: 'D:\\run\\math',
    deepseek: { jobs: 0 },
    Z: {
      bookStep: async (label, step, opts) => { calls.push({ kind: 'book', step, opts }); return publishAnswer },
    },
    step: async (stage, step) => { calls.push({ kind: 'math', step }); return step === 'theory' ? { ok: true, theories: 2, jobs: 1 } : { ok: true, theories: 2, answered: 2 } },
    jobs: async () => { calls.push({ kind: 'jobs' }) },
    must: (what, json) => { if (!json || json.ok !== true) throw new Error(`math ${what}: failed`); return json },
  }
  const sandbox = { defineModule: (m) => { mod = m }, zmMake: () => M, log: () => {}, console }
  vm.runInNewContext(source, sandbox)
  return { mod, calls }
}

let failed = 0
const check = (ok, what) => { console.log(`${ok ? 'ok' : 'FAIL'}: ${what}`); if (!ok) failed++ }

{
  const { mod, calls } = load({ ok: true, layer: 'theory', unit: 'theory/book@2', state: 'done', complete: true })
  const out = await mod.run({}, { args: {}, mode: 'full' })
  const publish = calls.find((c) => c.kind === 'book' && c.step === 'publish')
  check(!!publish && publish.opts.Layer === 'theory', 'the theory stage publishes the theory layer')
  check(publish && publish.opts.Applied === 'D:\\run\\math\\theory\\applied.json', 'the publication binds the applied Theory record')
  check(calls.findIndex((c) => c.step === 'publish') > calls.findIndex((c) => c.step === 'theory-apply'), 'it publishes after the apply')
  check(out.status === 'done' && out.publication && out.publication.unit === 'theory/book@2', 'the stage reports its publication')
}
{
  const { mod } = load({ ok: false, layer: 'theory', error: 'the Theory Card changed after apply' })
  let threw = false
  try { await mod.run({}, { args: {}, mode: 'full' }) } catch { threw = true }
  check(threw, 'a refused publication stops the stage')
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log('all passed')
