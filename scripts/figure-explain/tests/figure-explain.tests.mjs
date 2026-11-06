// Harness for the zt-figure-explain workflow and module: builds the blueprint with the Workflow Framework assembler and
// runs it in the framework's stand-in runtime (no model runs).
//   node scripts/figure-explain/tests/figure-explain.tests.mjs
import path from 'node:path'
import os from 'node:os'
import { fileURLToPath, pathToFileURL } from 'node:url'
import assert from 'node:assert/strict'

const here = path.dirname(fileURLToPath(import.meta.url))
const FW = path.join(os.homedir(), '.claude', 'skills', 'workflow-framework-protocol', 'tools')
const { loadBlueprint, loadModules, build, validate } = await import(pathToFileURL(path.join(FW, 'lib.mjs')).href)
const { simulate } = await import(pathToFileURL(path.join(FW, 'sim.mjs')).href)

const { bp, dirs } = loadBlueprint(path.join(here, '..', 'workflow', 'zt-figure-explain.blueprint.json'))
const mods = loadModules(dirs)
assert.deepEqual(validate(bp, mods).errors, [], 'the blueprint validates')
const built = build(bp, mods).script
// A copy that records the manager prompts and the step commands, for checks on their text.
const script = built
  .replace('ctx.agent(manager(label, file),', 'ctx.agent((globalThis.__readers.push(manager(label, file)), manager(label, file)),')
  .replace('const step = (label, command) => ztDrive(', 'const step = (label, command) => (globalThis.__commands.push(command), 0) || ztDrive(')
assert.notEqual(script, built)

const line = (value) => ({ answer: JSON.stringify(value), session_id: 'd', deleted: true })
const FILE = 'D:\\zt-digest\\_figure-explain\\95-1283\\jobs.json'
const started = (o = {}) => line({ ok: true, figures: 9, settled: 0, jobs: 9, chunks: 2, file: FILE, ...o })
const run = (answers, args = { fileIds: [95, 1283] }) => {
  globalThis.__readers = []
  globalThis.__commands = []
  return simulate(script, { sim: { answers }, args })
}

let passed = 0
const test = async (name, fn) => {
  try { await fn(); passed++; console.log(`ok   ${name}`) } catch (e) { console.log(`FAIL ${name}\n     ${e.message}`); process.exitCode = 1 }
}
const label = (c) => c.label || ''
const managers = (r) => r.calls.filter((c) => /: read \d+ of/.test(label(c)))

await test('one start, one queue of managers, one finish; each reader gets its images', async () => {
  const r = await run([{ label: 'start 1', ...{ answer: started() } }, { label: 'finish 1', answer: line({ ok: true, saved: 9, open: 0, refused: 0 }) }])
  assert.equal(r.error, undefined, String(r.error))
  const labels = r.calls.map(label)
  assert.equal(labels.filter((l) => /read \d+ of 2/.test(l)).length, 2, labels.join(' | '))
  assert.equal(labels.filter((l) => /start 1/.test(l)).length, 1)
  assert.equal(labels.filter((l) => /finish 1/.test(l)).length, 1)
  assert.ok(!labels.some((l) => /start 2/.test(l)), 'nothing open, no second round')
  const reader = globalThis.__readers[0]
  assert.match(reader, /images = the entry's images/, 'the manager hands the images to deepseek_run')
  assert.match(reader, /jobs-001\.json/, 'each manager has its own job file')
  assert.match(reader, /model "deepseek-official\/deepseek-flash", reasoning_effort "high"/)
  assert.match(reader, /zt-digest figures -Step explain-check/, 'the SELF-CHECK is the one command it approves')
  assert.match(reader, /never archive/)
  assert.match(globalThis.__commands[0], /^zt-digest figures -Step explain-start -Work "D:\\zt-digest\\_figure-explain\\95-1283" -FileIds 95,1283 -Model deepseek-official\/deepseek-flash$/)
  assert.match(globalThis.__commands[1], /^zt-digest figures -Step explain-finish -Work "D:\\zt-digest\\_figure-explain\\95-1283" -Model /)
})

await test('nothing open: no reader runs and no finish', async () => {
  const r = await run([{ label: 'start 1', answer: started({ jobs: 0, chunks: 0 }) }])
  assert.equal(r.error, undefined, String(r.error))
  assert.equal(managers(r).length, 0)
  assert.ok(!r.calls.some((c) => /finish/.test(label(c))))
})

await test('what stays open gets one more round, then the stage fails with explanations_open', async () => {
  const r = await run([
    { label: 'start 1', answer: started() }, { label: 'finish 1', answer: line({ ok: true, saved: 7, open: 2, refused: 0 }) },
    { label: 'start 2', answer: started({ jobs: 1, chunks: 1 }) }, { label: 'finish 2', answer: line({ ok: true, saved: 1, open: 1, refused: 0 }) },
  ])
  assert.ok(r.calls.some((c) => /start 2/.test(label(c))))
  assert.match(String(r.error || JSON.stringify(r.result)), /explanations_open/)
})

await test('a failed step fails the stage, never an empty success', async () => {
  const r = await run([{ label: 'start 1', answer: line({ ok: false, error: 'bridge refused' }) }])
  assert.match(String(r.error || JSON.stringify(r.result)), /bridge refused/)
})

console.log(`${passed} passed`)
