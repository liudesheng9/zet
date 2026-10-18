// Harness for the zt-issue-dev workflow: builds the blueprint with the Workflow Framework assembler and runs it in
// the framework's stand-in runtime (no model runs), one scenario per relay path. Also checks issue-list.mjs ranges.
//   node scripts/issue-dev/tests/issue-relay.tests.mjs
import path from 'node:path'
import os from 'node:os'
import { fileURLToPath, pathToFileURL } from 'node:url'
import assert from 'node:assert/strict'

const here = path.dirname(fileURLToPath(import.meta.url))
const FW = path.join(os.homedir(), '.claude', 'skills', 'workflow-framework-protocol', 'tools')
const { loadBlueprint, loadModules, build, validate } = await import(pathToFileURL(path.join(FW, 'lib.mjs')).href)
const { simulate } = await import(pathToFileURL(path.join(FW, 'sim.mjs')).href)
const { expandIssues } = await import(pathToFileURL(path.join(here, '..', 'issue-list.mjs')).href)

const BP_FILE = path.join(here, '..', 'workflow', 'zt-issue-dev.blueprint.json')
const { bp, dirs } = loadBlueprint(BP_FILE)
const mods = loadModules(dirs)
const v = validate(bp, mods)
assert.deepEqual(v.errors, [], 'the blueprint validates')
const { script } = build(bp, mods)

const issue = (number, state = 'OPEN') => ({ number, title: `issue ${number}`, state, labels: [], url: `u${number}` })
const st = (o) => ({ session_id: 's', status: 'finished', answer: '', pending: [], error: '', context_pct: 1, waits: 1, ...o })
const repoOk = { ok: true, clean: true, pushed: true, zt: { match: true, service: 'up' } }

async function run(name, issues, answers, { args = {}, repo = repoOk } = {}) {
  const sim = {
    script: { 'issue-list\\.mjs': { ok: true, issues, missing: [] }, 'repo-state\\.mjs': repo },
    answers: [...answers, { label: 'manager delete$', answer: { deleted: true, cancelled: false, note: '' } }],
  }
  const r = await simulate(script, { sim, args: { issues: issues.map(i => i.number), ...args } })
  if (r.error) throw new Error(`${name}: ${r.error.stack || r.error}`)
  return r
}

let passed = 0
const test = async (name, fn) => {
  try { await fn(); passed++; console.log(`ok   ${name}`) } catch (e) { console.log(`FAIL ${name}\n     ${e.message}`); process.exitCode = 1 }
}

await test('expandIssues: numbers, ranges, #, dedup', () => {
  assert.deepEqual(expandIssues([65, '#67', '70-72', '71']), [65, 67, 70, 71, 72])
  assert.throws(() => expandIssues(['x']))
  assert.throws(() => expandIssues(['9-3']))
})

await test('question -> main agent -> respond; ASK -> main agent -> prompt; DONE', async () => {
  const r = await run('main path', [issue(65, 'CLOSED')], [
    { label: '#65 manager start$', answer: st({ status: 'needs_response', pending: [{ request_id: 'question-1', kind: 'question', detail: JSON.stringify({ questions: [{ id: 'q1', question: 'run X' }] }) }] }) },
    { label: '#65 main agent 1$', answer: { outcome: 'done', result: 'X ok', answers: [{ id: 'q1', text: 'X ran, exit 0' }], decision: 'none' } },
    { label: '#65 manager respond 1$', answer: st({ answer: 'STATUS: ASK — deploy please' }) },
    { label: '#65 main agent 2$', answer: { outcome: 'done', result: 'deployed', answers: [], decision: 'none' } },
    { label: '#65 manager prompt 2$', answer: st({ answer: 'tests ok\nSTATUS: DONE — commit abc' }) },
  ])
  const res = r.result.result
  assert.equal(res.done, 1)
  assert.equal(res.issues[0].outcome, 'done')
  assert.equal(res.issues[0].summary, 'commit abc')
  assert.equal(res.issues[0].asks, 2)
  assert.equal(r.result.stages.check.status, 'done')
  const labels = r.calls.map(c => c.label)
  assert.ok(labels.some(l => /#65 manager respond 1$/.test(l)))
  assert.ok(labels.some(l => /#65 manager delete$/.test(l)))
  assert.ok(labels.some(l => /workspace cleanup/.test(l)), 'the workspace cleanup ran')
  const byLabel = l => r.calls.find(c => new RegExp(l).test(c.label))
  assert.equal(byLabel('manager start$').model, 'haiku')
  assert.equal(byLabel('main agent 1$').model, undefined, 'the main agent inherits the session model')
  assert.equal(byLabel('repo state').model, 'haiku')
  assert.equal(r.result.deepseek.deleted, 1)
})

await test('the start prompt carries the task, the model, max effort and the tdd skill', async () => {
  let prompt = ''
  const sim = {
    script: { 'issue-list\\.mjs': { ok: true, issues: [issue(70)], missing: [] }, 'repo-state\\.mjs': repoOk },
    answers: [{ label: 'manager start$', answer: st({ answer: 'STATUS: DONE — x' }) }],
  }
  const real = await simulate(script.replace('async function zirManage(ctx, p, op, label) {', 'async function zirManage(ctx, p, op, label) {\n  if (op.op === "start") globalThis.__zirStart = zirManagerPrompt(op, p, p.waitsPerManager)'), { sim, args: { issues: [70] } })
  if (real.error) throw real.error
  prompt = globalThis.__zirStart
  assert.match(prompt, /"model": "deepseek-official\/deepseek-flash"/)
  assert.match(prompt, /"reasoning_effort": "max"/)
  assert.match(prompt, /"skills": \[\n  "tdd"\n \]/)
  assert.match(prompt, /\/goal \/tdd \\n\\nissue #70\\n/)
  assert.match(prompt, /gh issue view <number> --repo liudesheng9\/zet/)
  assert.match(prompt, /issue is #70 of/)
  assert.doesNotMatch(prompt, /\{N\}|\{REPO\}|\{CWD\}/)
})

await test('dsEffort high is passed through', async () => {
  const sim = {
    script: { 'issue-list\\.mjs': { ok: true, issues: [issue(71)], missing: [] }, 'repo-state\\.mjs': repoOk },
    answers: [{ label: 'manager start$', answer: st({ answer: 'STATUS: DONE — x' }) }],
  }
  const r = await simulate(script.replace('async function zirManage(ctx, p, op, label) {', 'async function zirManage(ctx, p, op, label) {\n  if (op.op === "start") globalThis.__zirEffort = op.run.reasoning_effort'), { sim, args: { issues: [71], dsEffort: 'high' } })
  if (r.error) throw r.error
  assert.equal(globalThis.__zirEffort, 'high')
})

await test('NEEDS-HUMAN ends the issue and the next issue still runs, in order', async () => {
  const r = await run('needs human', [issue(90), issue(91)], [
    { label: '#90 manager start$', answer: st({ answer: 'STATUS: NEEDS-HUMAN — the user must authorize the pilot' }) },
    { label: '#91 manager start$', answer: st({ answer: 'STATUS: DONE — ok' }) },
  ])
  const res = r.result.result
  assert.deepEqual(res.issues.map(i => i.outcome), ['needs-human', 'done'])
  assert.deepEqual(res.notDone, ['#90 needs-human'])
  assert.equal(r.result.stages.check.status, 'failed')
  assert.equal(r.result.stages.report.status, 'done')
  const order = r.calls.map(c => c.label).filter(l => /manager start|manager delete/.test(l))
  assert.deepEqual(order.map(l => l.replace(/^develop: /, '')), ['#90 manager start', '#90 manager delete', '#91 manager start', '#91 manager delete'])
})

await test('no STATUS line -> nudge prompt; running -> wait manager', async () => {
  const r = await run('nudge', [issue(72)], [
    { label: 'manager start$', answer: st({ status: 'running', answer: '' }) },
    { label: 'manager wait 1$', answer: st({ answer: 'I did some work.' }) },
    { label: 'manager prompt 1$', answer: st({ answer: 'STATUS: DONE — finished' }) },
  ])
  assert.equal(r.result.result.issues[0].outcome, 'done')
  assert.equal(r.result.result.issues[0].rounds, 1)
})

await test('a failed turn gets one resume, a second failure ends the issue', async () => {
  const r = await run('failed', [issue(73)], [
    { label: 'manager start$', answer: st({ status: 'failed', error: 'provider 503' }) },
    { label: 'manager resume 1$', answer: st({ status: 'error', error: 'provider 503 again' }) },
  ])
  const i = r.result.result.issues[0]
  assert.equal(i.outcome, 'failed')
  assert.match(i.summary, /503 again/)
  assert.ok(r.calls.some(c => /#73 manager delete$/.test(c.label)), 'the conversation is still deleted')
})

await test('maxRounds stops an issue that never finishes', async () => {
  const r = await run('max rounds', [issue(74)], [
    { label: 'manager (start|prompt \\d+)$', answer: st({ answer: 'still going' }) },
  ], { args: { maxRounds: 3 } })
  const i = r.result.result.issues[0]
  assert.equal(i.outcome, 'max-rounds')
  assert.equal(i.rounds, 3)
})

await test('an approval is decided by the main agent and relayed as a decision', async () => {
  let responded = null
  const sim = {
    script: { 'issue-list\\.mjs': { ok: true, issues: [issue(75)], missing: [] }, 'repo-state\\.mjs': repoOk },
    answers: [
      { label: 'manager start$', answer: st({ status: 'needs_response', pending: [{ request_id: 'approval-1', kind: 'approval', detail: '{"tool":"pwsh","input":"cargo test"}' }] }) },
      { label: 'main agent 1$', answer: { outcome: 'done', result: 'allowed: cargo test is in scope', answers: [], decision: 'allow' } },
      { label: 'manager respond 1$', answer: st({ answer: 'STATUS: DONE — ok' }) },
      { label: 'manager delete$', answer: { deleted: true, note: '' } },
    ],
  }
  const r = await simulate(script.replace('async function zirManage(ctx, p, op, label) {', 'async function zirManage(ctx, p, op, label) {\n  if (op.op === "respond") globalThis.__zirResp = op.responses'), { sim, args: { issues: [75] } })
  if (r.error) throw r.error
  responded = globalThis.__zirResp
  assert.deepEqual(responded, [{ request_id: 'approval-1', decision: 'allow' }])
})

await test('a repo state that is not ok fails the check gate but not the run', async () => {
  const r = await run('repo', [issue(76)], [
    { label: 'manager start$', answer: st({ answer: 'STATUS: DONE — ok' }) },
  ], { repo: { ok: false, clean: false, pushed: false } })
  assert.deepEqual(r.result.result.repoNotOk, ['#76'])
  assert.equal(r.result.stages.check.status, 'failed')
  assert.equal(r.result.halted, null)
})

await test('a missing issue halts at prepare', async () => {
  const sim = { script: { 'issue-list\\.mjs': { ok: false, issues: [], missing: [999] } }, answers: [] }
  const r = await simulate(script, { sim, args: { issues: [999] } })
  assert.equal(r.result.halted.stage, 'prepare')
  assert.ok(!r.calls.some(c => /manager/.test(c.label)))
})

await test('until prepare stops before any DeepSeek work', async () => {
  const sim = { script: { 'issue-list\\.mjs': { ok: true, issues: [issue(65)], missing: [] } }, answers: [] }
  const r = await simulate(script, { sim, args: { issues: [65], until: 'prepare' } })
  assert.equal(r.result.stoppedAt, 'prepare')
  assert.ok(!r.calls.some(c => /manager/.test(c.label)))
})

console.log(`${passed} passed${process.exitCode ? ', some failed' : ''}`)
