// Run: node scripts/book-digest/tests/grow-workflow-harness.mjs
// The frozen-workflow fixture of the idea-grow Tree Lock seam (#140, R326/R337/R338). It builds
// zt-idea-grow from its blueprint with the workflow-framework-protocol assembler and runs the built
// script against a stand-in runtime: the growth tool answers from a canned table, so no model,
// DeepSeek, zt or archive call runs.
//
// What it checks:
//  - the grow stage stops with the tool's typed `card_locked` refusal when the owner's Lock refuses its
//    destination write, and reports the refusal (with its tree and Lock id) as the stage's error —
//    never as a content failure of one section and never retried;
//  - a stage that stops this way leaves no completed result behind, so the run is not described as
//    complete;
//  - a normal grow run (plan, no section job, check, report) still completes, and a refusal the tool
//    prints as its last line on stdout is recognized without matching English text.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..', '..', '..')
const wfpRoot = process.env.WFP_ROOT || path.join(os.homedir(), '.claude', 'skills', 'workflow-framework-protocol')
const lib = await import(pathToFileURL(path.join(wfpRoot, 'tools', 'lib.mjs')).href)
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor

const loaded = lib.loadBlueprint(path.join(root, 'scripts', 'idea-grow', 'workflow', 'zt-idea-grow.blueprint.json'))
const build = lib.build(loaded.bp, lib.loadModules(loaded.dirs))
const source = build.script.replace(/^export /m, '')
const json = (o) => JSON.stringify(o)

let failed = 0
const check = (cond, message) => { console.log(`${cond ? 'ok' : 'FAIL'}: ${message}`); if (!cond) failed++ }

// The refusal the growth tool prints as its last stdout line when the owner locked the destination.
const refusal = {
  ok: false, code: 'card_locked', error: 'the agent Topic 4/0 is locked by the owner', mode: 'grow',
  step: 'topics', tree: '4/0', lock_id: 3, locked_at: '2026-10-22T10:00:00.000Z',
  hint: 'unlock the tree in the GUI first; its Cards stay readable as sources',
}

// One grow command as the framework's `script` module relays it.
function scriptAnswer(command, answers) {
  const key = Object.keys(answers).find((k) => command.includes(k))
  const answer = key ? answers[key] : { exit_code: 0, last_line: json({ ok: true }), tail: '' }
  return {
    exit_code: answer.exit_code,
    last_line: answer.last_line,
    tail: answer.exit_code === 0 ? answer.last_line : `${answer.last_line}\n${answer.last_line}`,
  }
}

function runtime(answers) {
  const commands = [], labels = [], logs = []
  const agent = async (prompt, opts = {}) => {
    const text = String(prompt)
    labels.push(opts.label || '')
    if (/^Run one command and report its output\./.test(text)) {
      const command = (text.match(/^COMMAND:\n(.+)$/m) || [])[1] || ''
      commands.push(command)
      return scriptAnswer(command, answers)
    }
    throw new Error(`the stand-in runtime answers no other agent: ${text.slice(0, 120)}`)
  }
  return { agent, commands, labels, logs, phase: () => {}, log: (m) => logs.push(m), parallel: async (fns) => Promise.all(fns.map((f) => f())), pipeline: async (items, ...stages) => Promise.all(items.map(async (item, i) => { let v = item; for (const s of stages) v = await s(v, item, i); return v })) }
}

async function run(args, answers) {
  const rt = runtime(answers)
  const fn = new AsyncFunction('args', 'agent', 'phase', 'log', 'parallel', 'pipeline', source)
  let report, thrown
  try { report = await fn(args, rt.agent, rt.phase, rt.log, rt.parallel, rt.pipeline) } catch (error) { thrown = error }
  return { report, thrown, ...rt }
}

const args = { out: 'D:\\x\\grow', from: 1, to: 1 }
const plan = { exit_code: 0, last_line: json({ ok: true, pending: 1, topic_jobs: 0, books: 1, sections: 1, cards: 3, box: 10, topics: 2, baseline: { share: 0.1 }, left: 0 }), tail: '' }
const topicsDone = { exit_code: 0, last_line: json({ jobs: 0, topics: 0, created: 0, refused: 0 }), tail: '' }
const checkDone = { exit_code: 0, last_line: json({ faults: 0, fix: 0 }), tail: '' }
const reportDone = { exit_code: 0, last_line: json({ report: 'grow-report.md', created: 0, share_before: 0.1, share_after: 0.1, faults_left: 0 }), tail: '' }

// ================================================================================================================
// The frozen build carries the seam
// ================================================================================================================
check(build.modules['zt-grow-cycle'] === '9', `frozen: the build carries zt-grow-cycle v9 (${build.modules['zt-grow-cycle']})`)
check(source.includes('const lockRefusal = (text) =>') && source.includes('card_locked at ${where}'), 'frozen: the built script carries the Lock reader and the stop')
check(build.embedded.stages.filter((s) => s.id === 'grow').length === 1, 'frozen: the one grow stage is the module that owns the seam')

// ================================================================================================================
// A normal run still completes (the seam reads only refusals)
// ================================================================================================================
{
  const run1 = await run(args, { '-Step plan': plan, '-Step topics': topicsDone, '-Step check': checkDone, '-Step report': reportDone })
  check(!run1.thrown && run1.report && !run1.report.halted, `normal: the grow stage and the report stage complete (${JSON.stringify(run1.report && run1.report.halted)})`)
  check(run1.report && run1.report.stages.grow.status === 'done' && run1.report.result && run1.report.result.report && run1.report.result.report.report === 'grow-report.md', 'normal: the run reports its grow result')
  check(run1.commands.some((c) => /grow -Step plan/.test(c)), 'normal: the plan step runs')
}

// ================================================================================================================
// The owner's Lock refuses the destination: the stage stops with the typed refusal
// ================================================================================================================
{
  const refused = { exit_code: 2, last_line: json(refusal), tail: json(refusal) }
  const run2 = await run(args, { '-Step plan': plan, '-Step topics': refused })
  const halted = run2.report && run2.report.halted
  check(halted && halted.stage === 'grow', `lock: the grow stage stops (${JSON.stringify(halted && halted.stage)})`)
  const said = String((halted && halted.error) || '')
  check(/card_locked/.test(said), `lock: the stage error names card_locked (${said.slice(0, 160)})`)
  check(/agent Topic 4\/0 is locked by the owner/.test(said), `lock: the error keeps the tool's own sentence (${said.slice(0, 160)})`)
  check(/"tree":"4\/0"/.test(said) && /"lock_id":3/.test(said), 'lock: the error keeps the refusal JSON (tree and Lock id)')
  check(!/script: exit code/.test(said) || /card_locked/.test(said), 'lock: the refusal is not reported as a bare exit code')
  check(!(run2.report && run2.report.result), 'lock: a stopped run reports no completed result')
  check(!(run2.report && run2.report.stages.report && run2.report.stages.report.status === 'done'), 'lock: the report stage does not run after the stop')
  const retried = run2.commands.filter((c) => /-Step topics -Work/.test(c))
  check(retried.length === 1, `lock: the refused step runs once, never retried (${retried.length} runs)`)
  check(!run2.commands.some((c) => /-Step apply/.test(c)), 'lock: no Topic Card write is attempted after the refusal')
  check(run2.logs.some((l) => /Grow: 1 sections/.test(l)), 'lock: the work done before the Lock stays reported (the plan line is kept)')
}

// ================================================================================================================
// The refusal is read from its JSON, not from an English sentence
// ================================================================================================================
{
  // The same refusal without its sentence: only the typed code is left, and the stage still stops.
  const bare = { ...refusal, error: '', message: '' }
  const run3 = await run(args, { '-Step plan': plan, '-Step topics': { exit_code: 2, last_line: json(bare), tail: json(bare) } })
  check(run3.report && run3.report.halted && run3.report.halted.stage === 'grow', 'typed code: a refusal with no sentence still stops the stage')
  check(/card_locked/.test(String(run3.report.halted.error)), 'typed code: the stop names the typed code')
}

// ================================================================================================================
// 3.0.0 (docs/literature-only-digest-design.md): idea-grow owns every Topic tree, Topic Card, hub and the register
// ================================================================================================================
{
  // Mode hubs: only the hubs round and the register, with the hub options the digests had.
  const hubsPlan = { exit_code: 0, last_line: json({ hub_jobs: 0, topics: 3, trees: 2, limits: { line_min: 12 } }), tail: '' }
  const hubsReport = { exit_code: 0, last_line: json({ hubs: { topic: 2 }, faults: {}, missing: 0, mirror: 0, register: { status: 'published', entries: 9 } }), tail: '' }
  const r = await run({ out: 'D:/x/hubs', mode: 'hubs', hubTrees: '12,13', hubRebuild: true, hubLimits: { line_min: 12 } }, { '-Step hubs-plan': hubsPlan, '-Step hubs-report': hubsReport })
  check(!r.thrown && r.report && !r.report.halted && r.report.stages.grow.status === 'done', `mode hubs: the run completes (${JSON.stringify(r.report && r.report.halted)})`)
  check(!r.commands.some((c) => /grow -Step (plan|topics|apply|links|check|report|restate-cards) /.test(c)), `mode hubs: no Card is grown, checked or reviewed (${r.commands.join(' | ')})`)
  const plan = r.commands.find((c) => /grow -Step hubs-plan/.test(c)) || ''
  check(/-Trees '12,13'/.test(plan) && /-Rebuild/.test(plan) && plan.includes(`-Limits '{"line_min":12}'`), `mode hubs: the hub options reach the hub plan (${plan})`)
  check(r.commands.some((c) => /grow -Step hubs-report .*-Register 'apply'/.test(c)), 'mode hubs: the register is rebuilt after the hubs')
  check(r.report.result && r.report.result.mode === 'hubs' && r.report.result.report && r.report.result.report.mode === 'hubs', 'mode hubs: the result says it was a hubs run')
}
{
  // The restatement review of the run's new Topic Cards, one review per source book.
  const reviews = { exit_code: 0, last_line: json({ books: 1, reviews: [{ id: 'r01', book: 'Key', staging: 'D:/x/grow/restate/r01', cards: 3 }] }), tail: '' }
  const prepared = { exit_code: 0, last_line: json({ ok: true, step: 'prepare', cards: 3, reviewable: 0, jobs: [] }), tail: '' }
  const reported = { exit_code: 0, last_line: json({ ok: true, step: 'report', report_id: 'rr-1', status: 'resolved', complete: true, counts: { resolved_by_rewrite: 0, pending_existing_card_change: 0, unresolved: 0 } }), tail: '' }
  // The restate keys come first: the stand-in answers the first key a command contains.
  const answers = { '-Step restate-cards': reviews, 'restate -Step prepare': prepared, 'restate -Step report': reported, '-Step plan': plan, '-Step topics': topicsDone, '-Step check': checkDone, '-Step report': reportDone }
  const r = await run(args, answers)
  check(!r.thrown && r.report && !r.report.halted, `restate: the run completes (${JSON.stringify(r.report && r.report.halted)})`)
  check(r.commands.some((c) => c.includes("restate -Step prepare -Staging 'D:/x/grow/restate/r01'") && /-Cards '[^']*cards\.txt'/.test(c)), `restate: the review reads the run's own new Topic Cards of each book (${r.commands.filter((c) => /restate/.test(c)).join(' | ')})`)
  const at = (re) => r.commands.findIndex((c) => re.test(c))
  check(at(/grow -Step check/) < at(/grow -Step restate-cards/) && at(/grow -Step restate-cards/) < at(/grow -Step hubs-plan/), 'restate: the review follows the fix rounds and comes before the hubs')
  check(r.report.result && r.report.result.restate && r.report.result.restate.books === 1 && r.report.result.restate.reviews[0].status === 'resolved', `restate: the result reports each book's review (${JSON.stringify(r.report.result && r.report.result.restate)})`)
  const off = await run({ ...args, restate: false }, answers)
  check(!off.commands.some((c) => /restate/.test(c)), 'restate: restate false runs no review')
}
check((source.match(/\], p\.hubEffort\)\)/g) || []).length === 2, 'hub effort: the hub jobs and their fix jobs run at hubEffort')

console.log(`failed: ${failed}`)
process.exit(failed ? 1 : 0)
