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

// The goal the bridge echoes for a correct start: it ends with the issue's scope mark.
const scoped = n => `… Work on no other issue: other issues get their own conversations. Scope: issue #${n} only)`
// Give every manager answer that lacks one the correctly scoped echo of its issue (from the label, else the only issue).
const withScope = (answers, issues) => answers.map(a => {
  if (!a.answer || a.answer.session_id === undefined || a.answer.goal_objective !== undefined) return a
  const n = ((a.label || '').match(/#(\d+)/) || [])[1] || issues[0].number
  return { ...a, answer: { ...a.answer, goal_objective: scoped(n) } }
})

const timesQuiet = { ok: true, commands: 3, shellSec: 10, testSec: 5, tests: 1, slow: [] }

async function run(name, issues, answers, { args = {}, repo = repoOk, src = script, times = timesQuiet } = {}) {
  globalThis.__zirOps = []
  const sim = {
    script: { 'issue-list\\.mjs': { ok: true, issues, missing: [] }, 'repo-state\\.mjs': repo, 'dsh-times\\.mjs': times },
    answers: [...withScope(answers, issues), { label: 'manager delete', answer: { deleted: true, cancelled: false, note: '' } }],
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
  assert.equal(byLabel('main agent 1$').model, 'opus', 'the main agent is Opus')
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

await test('one issue per conversation: the scope rules reach the agent, the goal, the managers and the main agent', async () => {
  const s = spy.replace('async function zirManage(ctx, p, op, label) {', 'async function zirManage(ctx, p, op, label) {\n  ;(globalThis.__zirPrompts ||= []).push(zirManagerPrompt(op, p, p.waitsPerManager))')
  globalThis.__zirPrompts = []
  const r = await run('scope', [issue(97)], [
    { label: 'manager start$', answer: st({ status: 'needs_response', pending: [{ request_id: 'q', kind: 'question', detail: '{"questions":[{"id":"a"}]}' }] }) },
    { label: 'main agent 1$', answer: { outcome: 'done', result: 'ok', answers: [{ id: 'a', text: 'ok' }], decision: 'none' } },
    { label: 'manager respond 1$', answer: done() },
  ], { src: s })
  const run0 = globalThis.__zirOps.find(o => o.op === 'start').run
  assert.match(run0.prompt, /\*\*Scope: issue #97 only\.\*\*/)
  assert.match(run0.prompt, /Do not implement, test, comment on, relabel or close any other issue/)
  assert.match(run0.prompt, /If #97 is a PRD or tracking issue, verify that its plan and its map of child issues are right and current; do not implement the child issues/)
  assert.match(run0.prompt, /never `git add -A` or `git add \.`/)
  assert.match(run0.goal, /Scope: issue #97 only\)$/)
  for (const text of globalThis.__zirPrompts) {
    assert.match(text, /This job is exactly one conversation for GitHub issue #97 and nothing else/)
    assert.match(text, /Never add another issue to a prompt or a goal/)
  }
  assert.ok(globalThis.__zirOps.every(o => o.issue === 97), 'every manager op carries the issue')
  assert.equal(r.result.result.issues[0].outcome, 'done')
})

await test('the main agent is told the conversation is one issue and to refuse other issues\' work', async () => {
  const s = script.replace('async function zirExecute(ctx, p, issue, ask, label) {', 'async function zirExecute(ctx, p, issue, ask, label) {\n  const __a = ctx.agent; ctx = { ...ctx, agent: (prompt, o) => { (globalThis.__zirExec ||= []).push(prompt); return __a(prompt, o) } }')
  globalThis.__zirExec = []
  await run('exec scope', [issue(98)], [
    { label: 'manager start$', answer: st({ status: 'needs_response', pending: [{ request_id: 'q', kind: 'question', detail: '{"questions":[{"id":"a"}]}' }] }) },
    { label: 'main agent 1$', answer: { outcome: 'refused', result: 'stay within #98', answers: [{ id: 'a', text: 'stay within #98' }], decision: 'none' } },
    { label: 'manager respond 1$', answer: done() },
  ], { src: s })
  assert.match(globalThis.__zirExec[0], /Its conversation is for issue #98 only\. If the ask is work that belongs to another issue/)
  // The kernel puts the role's instructions in front of every executor prompt.
  assert.match(bp.roles.executor.instructions, /Refuse \(outcome "refused"\) any ask that is another issue's work/)
})

await test('a start whose echoed goal is not this issue is deleted and started once more', async () => {
  const r = await run('off scope once', [issue(97)], [
    { label: 'manager start$', answer: st({ session_id: 'bad', goal_objective: '… issues #92 through #122 (one at a time, in number order) …' }) },
    { label: 'manager start 2$', answer: done({ session_id: 'good' }) },
  ])
  const i = r.result.result.issues[0]
  assert.equal(i.outcome, 'done')
  assert.equal(i.session, 'good')
  assert.equal(i.trail[0].kind, 'scope')
  assert.match(i.trail[0].result, /goal is not "issue #97 only": … issues #92 through #122/)
  const labels = r.calls.map(c => c.label.replace(/^develop: /, ''))
  assert.deepEqual(labels.filter(l => /#97 manager/.test(l)), ['#97 manager start', '#97 manager delete off-scope 1', '#97 manager start 2', '#97 manager delete'])
})

await test('a start without any goal echo counts as off scope too', async () => {
  const r = await run('no goal', [issue(97)], [
    { label: 'manager start$', answer: st({ session_id: 'nogoal', goal_objective: '' }) },
    { label: 'manager start 2$', answer: done({ session_id: 'good' }) },
  ])
  assert.equal(r.result.result.issues[0].outcome, 'done')
  assert.ok(r.calls.some(c => /#97 manager delete off-scope 1$/.test(c.label)))
})

await test('two off-scope starts fail the issue, delete both, and the next issue still runs', async () => {
  const wrong = st({ goal_objective: '… Scope: issue #96 only)' })
  const r = await run('off scope twice', [issue(97), issue(98)], [
    { label: '#97 manager start$', answer: { ...wrong, session_id: 'b1' } },
    { label: '#97 manager start 2$', answer: { ...wrong, session_id: 'b2' } },
    { label: '#98 manager start$', answer: done() },
  ])
  const [a, b] = r.result.result.issues
  assert.equal(a.outcome, 'failed')
  assert.match(a.summary, /goal is not "issue #97 only", twice/)
  assert.equal(a.session, '')
  assert.equal(a.trail.filter(t => t.kind === 'scope').length, 2)
  assert.equal(r.calls.filter(c => /#97 manager delete off-scope/.test(c.label)).length, 2)
  assert.ok(!r.calls.some(c => /#97 (main agent|manager wait|manager respond|manager resume)/.test(c.label)), 'no relay for an off-scope conversation')
  assert.equal(b.outcome, 'done')
  assert.equal(r.result.deepseek.deleted, 3)
})

await test('a mark for another issue whose number starts the same does not pass (#9 vs #97)', async () => {
  const r = await run('prefix', [issue(9)], [
    { label: 'manager start$', answer: st({ session_id: 'x', goal_objective: '… Scope: issue #97 only)' }) },
    { label: 'manager start 2$', answer: done({ session_id: 'y', goal_objective: '… Scope: issue #9 only)' }) },
  ])
  assert.equal(r.result.result.issues[0].outcome, 'done')
  assert.equal(r.result.result.issues[0].session, 'y')
})

await test('GUI work goes to the Opus main agent under gui-first-principles; DeepSeek writes no GUI code', async () => {
  await run('gui', [issue(120)], [
    { label: 'manager start$', answer: st({ status: 'needs_response', pending: [{ request_id: 'q', kind: 'question', detail: '{"questions":[{"id":"g","question":"GUI request: show the reply bar"}]}' }] }) },
    { label: 'main agent 1$', answer: { outcome: 'done', result: 'commit abc: reply bar', answers: [{ id: 'g', text: 'commit abc: reply bar' }], decision: 'none' } },
    { label: 'manager respond 1$', answer: done() },
  ], { src: spy })
  const prompt = globalThis.__zirOps.find(o => o.op === 'start').run.prompt
  assert.match(prompt, /\*\*GUI work belongs to the main agent \(Claude Opus\)\.\*\* Do not write or change GUI code yourself: the browser front end under `src\/gui\/`/)
  assert.match(prompt, /call ask_user_question with ONE GUI request/)
  const ex = bp.roles.executor
  assert.equal(ex.model, 'opus')
  assert.match(ex.instructions, /gui-first-principles\\SKILL\.md and follow both principles/)
  assert.match(ex.instructions, /real index cards: card stock, a red header line, blue rules/)
  assert.match(ex.instructions, /never by writing D:\\zt/)
  assert.match(ex.instructions, /Commit only your GUI files, with a message that names the issue; do not push/)
})

await test('the old maxRounds arg is refused', async () => {
  const sim = { script: { 'issue-list\\.mjs': { ok: true, issues: [issue(65)], missing: [] } }, answers: [] }
  const r = await simulate(script, { sim, args: { issues: [65], maxRounds: 3 } })
  assert.match(String(r.error && r.error.message), /args\.maxRounds is not declared/)
})

// ---- test-time notes (1.4.0) ----
const { decodeFrames, records, shellTimes, summarize, commandKey } = await import(pathToFileURL(path.join(here, '..', 'dsh-times.mjs')).href)
const zlib = await import('node:zlib')

await test('dsh-times: frames, foreground, background by notice and by job_output, merged runs', () => {
  const T = 1_000_000
  const recs = [
    { type: 'session', time: T },
    { type: 'tool/call', time: T + 0, data: { callId: 'a', name: 'pwsh', arguments: JSON.stringify({ command: 'cargo test --bin zt 2>&1 | Select-Object -Last 5', description: 'unit' }) } },
    { type: 'tool/result', time: T + 30_000, data: { message: { toolCallId: 'a', content: [{ type: 'text', text: 'ok' }] } } },
    { type: 'tool/call', time: T + 31_000, data: { callId: 'b', name: 'pwsh', arguments: JSON.stringify({ command: 'cargo test --all-targets 2>&1 | Out-String', run_in_background: true }) } },
    { type: 'tool/result', time: T + 31_010, data: { message: { toolCallId: 'b', content: [{ type: 'text', text: 'started background job pwsh-1' }] } } },
    { type: 'agent/inbox/spliced', time: T + 231_000, data: { inserted: [{ content: [{ type: 'text', text: 'background job pwsh-1 (pwsh: cargo test --all-targets 2>&1 | Out-String) finished [status: completed, exit code: 0]. Read its output with job_output.' }] }] } },
    { type: 'tool/call', time: T + 232_000, data: { callId: 'c', name: 'pwsh', arguments: JSON.stringify({ command: 'cargo test --all-targets 2>&1 | Select-String ok', run_in_background: true }) } },
    { type: 'tool/result', time: T + 232_010, data: { message: { toolCallId: 'c', content: [{ type: 'text', text: 'started background job pwsh-2' }] } } },
    { type: 'tool/call', time: T + 233_000, data: { callId: 'd', name: 'job_output', arguments: JSON.stringify({ job_id: 'pwsh-2', wait: true }) } },
    { type: 'tool/result', time: T + 240_000, data: { message: { toolCallId: 'd', content: [{ type: 'text', text: '(no new output)\n[status: running]' }] } } },
    { type: 'tool/call', time: T + 241_000, data: { callId: 'e', name: 'job_output', arguments: JSON.stringify({ job_id: 'pwsh-2', wait: true }) } },
    { type: 'tool/result', time: T + 332_000, data: { message: { toolCallId: 'e', content: [{ type: 'text', text: 'test result: ok. 3 passed' }] } } },
    { type: 'tool/call', time: T + 333_000, data: { callId: 'f', name: 'read', arguments: '{}' } },
    { type: 'tool/result', time: T + 334_000, data: { message: { toolCallId: 'f', content: [] } } },
  ]
  // Two frames, written as dsh appends them.
  const lines = recs.map(r => JSON.stringify(r) + '\n')
  const buf = Buffer.concat([zlib.zstdCompressSync(Buffer.from(lines.slice(0, 5).join(''))), zlib.zstdCompressSync(Buffer.from(lines.slice(5).join('')))])
  const got = shellTimes(records(decodeFrames(buf)))
  assert.deepEqual(got.map(c => c.sec), [30, 200, 100])
  assert.deepEqual(got.map(c => c.exit), [null, 0, null])
  assert.ok(got.every(c => c.finished && c.test))
  assert.equal(commandKey('cd E:\\zet; cargo test --all-targets 2>&1 | Out-String'), 'cd E:\\zet; cargo test --all-targets')
  const s = summarize(got, { minSec: 60 })
  assert.equal(s.commands, 3); assert.equal(s.shellSec, 330); assert.equal(s.tests, 3)
  assert.deepEqual(s.slow.map(m => [m.cmd, m.sec, m.runs]), [['cargo test --all-targets', 300, 2]])
})

const slowTimes = { ok: true, commands: 40, shellSec: 900, testSec: 800, tests: 6, slow: [{ sec: 700, test: true, background: true, exit: 0, cmd: 'cargo test --all-targets', example: 'cargo test --all-targets', description: 'whole suite', runs: 4, unfinished: 0 }] }
const note = { command: 'cargo test --all-targets', sec: 700, reason: 'run 4 times with no code change between the last 3', instead: 'cargo test --bin zt -- <module>::' }

await test('times are read before the delete; slow runs go to the main agent; its notes reach the next issue only', async () => {
  const r = await run('notes', [issue(150), issue(151)], [
    { label: '#150 manager start$', answer: done({ session_id: 's150' }) },
    { label: '#150 test notes$', answer: { notes: [note], summary: 'added the repeated full suite' } },
    { label: '#151 manager start$', answer: done({ session_id: 's151' }) },
    { label: '#151 test notes$', answer: { notes: [note], summary: 'kept' } },
  ], { src: spy, times: slowTimes })
  const labels = r.calls.map(c => c.label)
  const at = s => labels.findIndex(l => l.endsWith(s))
  assert.ok(at('#150 test times: script') >= 0 || at('#150 test times') >= 0, 'times measured')
  const iTimes = labels.findIndex(l => /#150 test times/.test(l)), iNotes = at('#150 test notes'), iDel = at('#150 manager delete')
  assert.ok(iTimes < iNotes && iNotes < iDel, `order times < notes < delete: ${labels.join(' | ')}`)
  const timesCall = r.calls.find(c => /#150 test times/.test(c.label))
  assert.match(timesCall.command, /dsh-times\.mjs --session s150 --issue 150 --out 'C:\/Users\/chenx\/\.zt-issue-dev\/times\/issue-150\.json'/)
  const starts = globalThis.__zirOps.filter(o => o.op === 'start')
  assert.doesNotMatch(starts[0].run.prompt, /Slow test runs not to repeat/)
  assert.match(starts[1].run.prompt, /--- Slow test runs not to repeat ---[\s\S]*Do not run them in this issue[\s\S]*- `cargo test --all-targets` \(700 s\): run 4 times with no code change between the last 3 Run instead: cargo test --bin zt -- <module>::$/)
  assert.match(starts[1].run.goal, /Scope: issue #151 only\)$/, 'the goal is unchanged')
  const res = r.result.result
  assert.deepEqual(res.testNotes, [note])
  assert.deepEqual(res.testTime, [{ issue: 150, shellSec: 900, testSec: 800, tests: 6 }, { issue: 151, shellSec: 900, testSec: 800, tests: 6 }])
  assert.equal(res.issues[0].times.slow[0].runs, 4)
  assert.equal(res.issues[1].notesGiven, 1)
})

await test('no slow command: no main-agent call; seeded testNotes reach the first issue; a failed times read keeps the notes', async () => {
  let r = await run('quiet', [issue(160)], [{ label: 'manager start$', answer: done() }], { src: spy, args: { testNotes: [note] } })
  assert.ok(!r.calls.some(c => /test notes/.test(c.label)))
  assert.match(globalThis.__zirOps.find(o => o.op === 'start').run.prompt, /`cargo test --all-targets` \(700 s\)/)
  assert.deepEqual(r.result.result.testNotes, [note])
  r = await run('broken', [issue(161)], [{ label: 'manager start$', answer: done() }], { args: { testNotes: [note] }, times: { ok: false, error: 'no stored log for session s' } })
  assert.equal(r.result.result.issues[0].outcome, 'done')
  assert.match(r.result.result.issues[0].times.error, /no stored log/)
  assert.deepEqual(r.result.result.testNotes, [note])
  assert.ok(!r.calls.some(c => /test notes/.test(c.label)))
})

console.log(`${passed} passed${process.exitCode ? ', some failed' : ''}`)
