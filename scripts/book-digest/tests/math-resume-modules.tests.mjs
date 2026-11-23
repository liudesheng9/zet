// Run: node scripts/book-digest/tests/math-resume-modules.tests.mjs
// zt-math-boundary and zt-math-proof start from the ledger (thompson1914calculus pilot, 2026-11-22): a resume after
// the boundary settle failed planned the stage again, its new plan differed from the registered one, and the ledger
// refused to rebind the open unit. A stage whose obligations are all done is reused; a stage whose compiler steps
// finished (its evidence file exists) and whose obligations are open runs the ledger settle alone; only a stage the
// ledger does not hold yet runs the whole chain.
import fs from 'node:fs'
import path from 'node:path'
import vm from 'node:vm'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const modules = path.join(here, '..', 'workflow', 'modules')

function load(file, state) {
  const steps = []
  let mod = null
  const M = {
    MATH_WORK: 'D:\\run\\math',
    deepseek: {},
    Z: { bookStep: async () => ({ ok: true }) },
    step: async (stage, name, label, extra) => {
      steps.push({ name, extra: extra || '' })
      if (name === 'layer-state') return { ok: true, ...state }
      if (name === 'settle') return { ok: true, settled: state.open || 0 }
      return { ok: true, units: 1, jobs: 1, created: 1, kept: 1, steps_ok: 1 }
    },
    jobs: async () => { steps.push({ name: 'jobs' }) },
    must: (what, json) => { if (!json || json.ok !== true) throw new Error(`math ${what}`); return json },
  }
  vm.runInNewContext(fs.readFileSync(path.join(modules, file), 'utf8'), { defineModule: (m) => { mod = m }, zmMake: () => M, log: () => {}, console })
  return { run: () => mod.run({}, { args: {}, mode: 'full' }), steps }
}

let failed = 0
const check = (ok, what) => { console.log(`${ok ? 'ok' : 'FAIL'}: ${what}`); if (!ok) failed++ }

for (const [file, layer, chain] of [['zt-math-boundary.js', 'boundary', 'boundary-settle'], ['zt-math-proof.js', 'proof', 'proof-apply']]) {
  {
    const t = load(file, { layer, total: 13, done: 13, open: 0 })
    const out = await t.run()
    const names = t.steps.map((s) => s.name)
    check(names[0] === 'layer-state' && t.steps[0].extra.includes(`-Layer ${layer}`), `${layer}: the stage first reads its ledger state`)
    check(out.status === 'done' && out.reused === true && names.length === 1, `${layer}: every obligation done -> reused, nothing runs (${names})`)
  }
  {
    const t = load(file, { layer, total: 13, done: 12, open: 1, evidence: true })
    const out = await t.run()
    const names = t.steps.map((s) => s.name)
    check(names.join(',') === 'layer-state,settle' && t.steps[1].extra.includes(`-Layer ${layer}`), `${layer}: open obligations with evidence -> the settle alone (${names})`)
    check(out.status === 'done' && out.resumed === 'settle', `${layer}: the stage reports it resumed at the settle`)
  }
  {
    const t = load(file, { layer })
    await t.run()
    const names = t.steps.map((s) => s.name)
    check(names.includes(layer) && names.includes(chain), `${layer}: a stage the ledger does not hold runs the whole chain (${names})`)
  }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log('all passed')
