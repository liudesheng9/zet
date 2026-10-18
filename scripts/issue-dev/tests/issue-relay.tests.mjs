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
// A copy of the script that records the manager ops it is asked for, for checks on their arguments.
const spy = script.replace('async function zirManage(ctx, p, op, label) {', 'async function zirManage(ctx, p, op, label) {\n  ;(globalThis.__zirOps ||= []).push(op)')

const issue = (number, state = 'OPEN') => ({ number, title: `issue ${number}`, state, labels: [], url: `u${number}` })
const st = (o) => ({ session_id: 's', status: 'finished', answer: '', pending: [], goal_phase: 'active', goal_rounds: '1/60', goal_blocked_reason: '', error: '', context_pct: 1, waits: 1, ...o })
const done = (o = {}) => st({ goal_phase: 'complete', answer: 'STATUS: DONE — ok', ...o })
const repoOk = { ok: true, clean: true, pushed: true, zt: { match: true, service: 'up' } }

async function run(name, issues, answers, { args = {}, repo = repoOk, src = script } = {}) {
  globalThis.__zirOps = []
  const sim = {
    script: { 'issue-list\\.mjs': { ok: true, issues, missing: [] }, 'repo-state\\.mjs': repo },
    answers: [...answers, { label: 'manager delete$', answer: { deleted: true, cancelled: false, note: '' } }],
  }
  const r = await simulate(src, { sim, args: { issues: issues.map(i => i.number), ...args } })
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

await test('the manager starts the conversation with a goal: task, goal objective, cap, model, max effort, tdd', async () => {
  await run('start', [issue(70)], [{ label: 'manager start$', answer: done() }], { src: spy })
  const start = globalThis.__zirOps.find(o => o.op === 'start')
  const r = start.run
  assert.equal(r.model, 'deepseek-official/deepseek-flash')
  assert.equal(r.reasoning_effort, 'max')
  assert.deepEqual(r.skills, ['tdd'])
  assert.equal(r.max_goal_rounds, 60)
  assert.match(r.goal, /^\/tdd \n\nissue #70\n\nMake sure that all the features/)
  assert.match(r.goal, /If there are anything not good, fix\./)
  assert.match(r.goal, /mark this goal complete only when all of it is verified/)
  assert.match(r.prompt, /^issue #70\n\nMake sure/)
  assert.match(r.prompt, /This conversation has a goal \(set with \/goal\)/)
  assert.match(r.prompt, /gh issue view <number> --repo liudesheng9\/zet/)
  assert.match(r.prompt, /update_goal, action complete/)
  assert.doesNotMatch(r.prompt + r.goal, /\{N\}|\{REPO\}|\{CWD\}/)
})

await test('the manager prompt passes goal to deepseek_run and loads deepseek_goal', async () => {
  const s = spy.replace('async function zirManage(ctx, p, op, label) {', 'async function zirManage(ctx, p, op, label) {\n  ;(globalThis.__zirPrompts ||= []).push(zirManagerPrompt(op, p, p.waitsPerManager))')
  globalThis.__zirPrompts = []
  await run('prompt', [issue(71)], [{ label: 'manager start$', answer: done() }], { src: s })
  const text = globalThis.__zirPrompts[0]
  assert.match(text, /"goal": "\/tdd/)
  assert.match(text, /"max_goal_rounds": 60/)
  assert.match(text, /mcp__deepseek__deepseek_goal/)
  assert.match(text, /goal_phase/)
})

await test('dsEffort high and maxGoalRounds are passed through', async () => {
  await run('args', [issue(72)], [{ label: 'manager start$', answer: done() }], { src: spy, args: { dsEffort: 'high', maxGoalRounds: 12 } })
  const r = globalThis.__zirOps.find(o => o.op === 'start').run
  assert.equal(r.reasoning_effort, 'high')
  assert.equal(r.max_goal_rounds, 12)
})

await test('question in a goal round -> main agent -> respond; goal complete -> done', async () => {
  const r = await run('main path', [issue(65, 'CLOSED')], [
    { label: '#65 manager start$', answer: st({ status: 'needs_response', goal_rounds: '2/60', pending: [{ request_id: 'question-1', kind: 'question', detail: JSON.stringify({ questions: [{ id: 'q1', question: 'run X' }] }) }] }) },
    { label: '#65 main agent 1$', answer: { outcome: 'done', result: 'X ok', answers: [{ id: 'q1', text: 'X ran, exit 0' }], decision: 'none' } },
    { label: '#65 manager respond 1$', answer: done({ answer: 'tests ok\nSTATUS: DONE — commit abc', goal_rounds: '3/60' }) },
  ], { src: spy })
  const res = r.result.result
  assert.equal(res.done, 1)
  const i = res.issues[0]
  assert.equal(i.outcome, 'done')
  assert.equal(i.summary, 'commit abc')
  assert.equal(i.goalRounds, '3/60')
  assert.equal(i.asks, 1)
  assert.equal(i.trail[0].goalRounds, '2/60')
  assert.deepEqual(globalThis.__zirOps.find(o => o.op === 'respond').responses, [{ request_id: 'question-1', answers: [{ id: 'q1', custom: 'X ran, exit 0' }] }])
  assert.equal(r.result.stages.check.status, 'done')
  const byLabel = l => r.calls.find(c => new RegExp(l).test(c.label))
  assert.equal(byLabel('manager start$').model, 'haiku')
  assert.equal(byLabel('main agent 1$').model, undefined, 'the main agent inherits the session model')
  assert.equal(byLabel('repo state').model, 'haiku')
  assert.ok(r.calls.some(c => /workspace cleanup/.test(c.label)), 'the workspace cleanup ran')
  assert.equal(r.result.deepseek.deleted, 1)
})

await test('a goal complete without a STATUS line is still done, with the answer tail as summary', async () => {
  const r = await run('no status', [issue(73)], [{ label: 'manager start$', answer: done({ answer: 'all verified and pushed' }) }])
  assert.equal(r.result.result.issues[0].outcome, 'done')
  assert.equal(r.result.result.issues[0].summary, 'all verified and pushed')
})

await test('a blocked goal is needs-human and the next issue still runs, in order', async () => {
  const r = await run('blocked', [issue(90), issue(91)], [
    { label: '#90 manager start$', answer: st({ goal_phase: 'blocked', goal_blocked_reason: 'model-reported: the real pilot needs the user', answer: 'STATUS: NEEDS-HUMAN — authorize the pilot' }) },
    { label: '#91 manager start$', answer: done() },
  ])
  const res = r.result.result
  assert.deepEqual(res.issues.map(i => i.outcome), ['needs-human', 'done'])
  assert.match(res.issues[0].summary, /model-reported: the real pilot needs the user \| authorize the pilot/)
  assert.deepEqual(res.notDone, ['#90 needs-human'])
  assert.equal(r.result.stages.check.status, 'failed')
  assert.equal(r.result.stages.report.status, 'done')
  const order = r.calls.map(c => c.label).filter(l => /manager start|manager delete/.test(l))
  assert.deepEqual(order.map(l => l.replace(/^develop: /, '')), ['#90 manager start', '#90 manager delete', '#91 manager start', '#91 manager delete'])
})

await test('a goal blocked at its round limit is max-rounds', async () => {
  const r = await run('round limit', [issue(74)], [
    { label: 'manager start$', answer: st({ goal_phase: 'blocked', goal_blocked_reason: 'round-limit: Goal reached its configured limit of 60 rounds.', goal_rounds: '60/60' }) },
  ])
  assert.equal(r.result.result.issues[0].outcome, 'max-rounds')
})

await test('running -> wait managers; maxWaitCalls exceeded is timeout', async () => {
  const r = await run('waits', [issue(75)], [
    { label: 'manager start$', answer: st({ status: 'running' }) },
    { label: 'manager wait 1$', answer: done() },
  ])
  assert.equal(r.result.result.issues[0].outcome, 'done')
  const t = await run('timeout', [issue(76)], [{ label: 'manager (start|wait \\d+)$', answer: st({ status: 'running' }) }])
  assert.equal(t.result.result.issues[0].outcome, 'timeout')
  assert.equal(t.calls.filter(c => /manager wait/.test(c.label)).length, 8)
})

await test('a goal that stops short gets one resume; a second stop fails the issue', async () => {
  const r = await run('resume ok', [issue(77)], [
    { label: 'manager start$', answer: st({ goal_phase: 'paused' }) },
    { label: 'manager resume 1$', answer: done() },
  ], { src: spy })
  assert.equal(r.result.result.issues[0].outcome, 'done')
  assert.ok(globalThis.__zirOps.some(o => o.op === 'resume'))
  const f = await run('resume fails', [issue(78)], [
    { label: 'manager start$', answer: st({ status: 'error', goal_phase: 'active', error: 'provider 503' }) },
    { label: 'manager resume 1$', answer: st({ status: 'error', goal_phase: 'paused', error: 'provider 503 again' }) },
  ])
  const i = f.result.result.issues[0]
  assert.equal(i.outcome, 'failed')
  assert.match(i.summary, /503 again/)
  assert.ok(f.calls.some(c => /#78 manager delete$/.test(c.label)), 'the conversation is still deleted')
})

await test('maxAsks stops an issue that keeps asking', async () => {
  const ask = st({ status: 'needs_response', pending: [{ request_id: 'q', kind: 'question', detail: '{"questions":[{"id":"a"}]}' }] })
  const r = await run('max asks', [issue(79)], [
    { label: 'manager (start|respond \\d+)$', answer: ask },
    { label: 'main agent', answer: { outcome: 'done', result: 'ok', answers: [{ id: 'a', text: 'ok' }], decision: 'none' } },
  ], { args: { maxAsks: 3 } })
  const i = r.result.result.issues[0]
  assert.equal(i.outcome, 'max-asks')
  assert.equal(i.asks, 3)
})

await test('an approval is decided by the main agent and relayed as a decision', async () => {
  await run('approval', [issue(80)], [
    { label: 'manager start$', answer: st({ status: 'needs_response', pending: [{ request_id: 'approval-1', kind: 'approval', detail: '{"tool":"pwsh","input":"cargo test"}' }] }) },
    { label: 'main agent 1$', answer: { outcome: 'done', result: 'allowed: cargo test is in scope', answers: [], decision: 'allow' } },
    { label: 'manager respond 1$', answer: done() },
  ], { src: spy })
  assert.deepEqual(globalThis.__zirOps.find(o => o.op === 'respond').responses, [{ request_id: 'approval-1', decision: 'allow' }])
})

await test('a repo state that is not ok fails the check gate but not the run', async () => {
  const r = await run('repo', [issue(81)], [{ label: 'manager start$', answer: done() }], { repo: { ok: false, clean: false, pushed: false } })
  assert.deepEqual(r.result.result.repoNotOk, ['#81'])
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

await test('the old maxRounds arg is refused', async () => {
  const sim = { script: { 'issue-list\\.mjs': { ok: true, issues: [issue(65)], missing: [] } }, answers: [] }
  const r = await simulate(script, { sim, args: { issues: [65], maxRounds: 3 } })
  assert.match(String(r.error && r.error.message), /args\.maxRounds is not declared/)
})

console.log(`${passed} passed${process.exitCode ? ', some failed' : ''}`)
