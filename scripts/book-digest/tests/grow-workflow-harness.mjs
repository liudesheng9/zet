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
  const key = Object.keys(answers).find((k) => k !== '__agents' && command.includes(k))
  const answer = key ? answers[key] : { exit_code: 0, last_line: json({ ok: true }), tail: '' }
  return {
    exit_code: answer.exit_code,
    last_line: answer.last_line,
    tail: answer.exit_code === 0 ? answer.last_line : `${answer.last_line}\n${answer.last_line}`,
  }
}

function runtime(answers) {
  const commands = [], labels = [], logs = [], prompts = []
  const agent = async (prompt, opts = {}) => {
    const text = String(prompt)
    labels.push(opts.label || '')
    if (/^Run one command and report its output\./.test(text)) {
      const command = (text.match(/^COMMAND:\n(.+)$/m) || [])[1] || ''
      commands.push(command)
      return scriptAnswer(command, answers)
    }
    // 3.1.0: a DSMP manager or the cleanup agent, answered by its label (as the sim answers them).
    const managed = (answers.__agents || []).find(([re]) => new RegExp(re).test(opts.label || ''))
    if (managed) { prompts.push({ label: opts.label || '', text }); return managed[1] }
    throw new Error(`the stand-in runtime answers no other agent: ${text.slice(0, 120)}`)
  }
  return { agent, commands, labels, logs, prompts, phase: () => {}, log: (m) => logs.push(m), parallel: async (fns) => Promise.all(fns.map((f) => f())), pipeline: async (items, ...stages) => Promise.all(items.map(async (item, i) => { let v = item; for (const s of stages) v = await s(v, item, i); return v })) }
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
check(build.modules['zt-grow-cycle'] === '10', `frozen: the build carries zt-grow-cycle v10 (${build.modules['zt-grow-cycle']})`)
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

// ================================================================================================================
// 3.1.0: the concept review of each source book (the register plan's -Review) and the Topic maintenance round
// ================================================================================================================
{
  const line = (o) => ({ exit_code: 0, last_line: json(o), tail: '' })
  const mgr = (id, reply) => ({ jobs: [{ id, ok: true, reply, attempts: 1 }], started: ['s1'], deleted: 1, not_deleted: [] })
  const cleanup = { folders: [{ cwd: 'D:\\x\\grow', removed: true, leftover: [], running: 0 }] }
  const staging = 'D:/x/grow/concepts/c01'
  // The concept keys come first: the stand-in answers the first key a command contains.
  const answers = {
    'concepts -Step propose': line({ ok: true, step: 'propose', book: 'Key', term_rows: 2, proposals: 2 }),
    'concepts -Step prepare': line({ ok: true, step: 'prepare', evidence_id: 'e-1', jobs: [{ id: 'cj-01', files: [`${staging}/work/dsh-concept-instructions.zh.md`, `${staging}/work/cj-01.md`], prompt: 'judge', answer: `${staging}/work/out/cj-01.json` }] }),
    'concepts -Step judge': line({ ok: true, step: 'judge', judged: 3, pairs: 3, unjudged: 0 }),
    'concepts -Step report': line({ ok: true, step: 'report', report_id: 'r-1', status: 'complete', complete: true, findings: 3, judged: 3, unjudged: 0 }),
    '-Step concept-books': line({ books: 1, reviews: [{ id: 'c01', book: 'Key', staging, table: 'Key/1|t' }], skipped: [{ book: 'Other', reason: 'no term-table Card' }] }),
    '-Step organize-plan': line({ organize_jobs: 1, topics: 2, pairs: 1, small: 0, moves: 1, locked: 0 }),
    '-Step organize-apply': line({ applied: true, merges: 1, moves: 1, refused: 0, blocked: 0, renamed: 2, complete: true, invalid: 0 }),
    '-Step restate-cards': line({ books: 0, reviews: [] }),
    '-Step hubs-plan': line({ hub_jobs: 0, topics: 3, trees: 2, limits: {} }),
    '-Step hubs-report': line({ hubs: { topic: 2 }, faults: {}, missing: 0, mirror: 0, register: { status: 'published', entries: 9, reviews: 1 } }),
    '-Step plan': plan, '-Step topics': topicsDone, '-Step check': checkDone, '-Step report': reportDone,
    __agents: [['organize mgr', mgr('go-001', 'OK')], ['concepts c01 mgr', mgr('c01-cj', '{"judged":3}')], ['cleanup', cleanup]],
  }
  const r = await run({ ...args, organize: true }, answers)
  check(!r.thrown && r.report && !r.report.halted, `rounds: the run completes (${JSON.stringify(r.report && r.report.halted)} ${r.thrown && r.thrown.message})`)
  const at = (re) => r.commands.findIndex((c) => re.test(c))
  const order = [/-Step restate-cards/, /-Step organize-plan/, /-Step organize-apply/, /-Step concept-books/, /concepts -Step propose/, /concepts -Step report/, /-Step hubs-plan/, /-Step hubs-report/].map(at)
  check(order.every((x, i) => x >= 0 && (i === 0 || x > order[i - 1])), `rounds: restate, then the Topic maintenance, then the concept review, then the hubs (${order.join(',')})`)
  check(r.commands.some((c) => /-Step hubs-report .*-Concepts/.test(c)), 'concepts: the register plan reads the rendered concept reports (hubs-report -Concepts)')
  check(r.commands.some((c) => c.includes(`concepts -Step judge -Staging '${staging}'`) && c.includes(`-File '${staging}/work/out/cj-01.json'`)), 'concepts: the judge step validates the judge job\'s own answer file')
  const go = r.prompts.find((x) => /organize mgr/.test(x.label))
  check(go && go.text.includes('go-001') && go.text.includes('-Job go-001') && go.text.includes('jobs\\\\go-001.md'), 'organize: one DeepSeek decision job go-001 with its SELF-CHECK and its job file')
  const cj = r.prompts.find((x) => /concepts c01 mgr/.test(x.label))
  check(cj && cj.text.includes('c01-cj') && cj.text.includes('concepts -Step judge'), 'concepts: one DeepSeek judge job per book, approved only for the concept self-check')
  const res = r.report && r.report.result
  check(res && res.organize && res.organize.applied && res.organize.applied.merges === 1 && res.organize.plan.pairs === 1, `organize: the result reports the survey and the merges and moves (${JSON.stringify(res && res.organize)})`)
  check(res && res.concepts && res.concepts.books === 1 && res.concepts.reviews[0].report_id === 'r-1' && res.concepts.skipped.length === 1, `concepts: the result reports each book's review and the skipped books (${JSON.stringify(res && res.concepts)})`)
  check(r.labels.some((l) => /cleanup/.test(l)), 'rounds: the DeepSeek cleanup runs after the rounds')

  // The defaults: no maintenance round unless asked; concepts false runs no review and no -Concepts.
  const quiet = await run({ ...args, concepts: false }, answers)
  check(!quiet.thrown && !quiet.commands.some((c) => /organize-|concept-books|concepts -Step/.test(c)), 'defaults: organize is off by default, and concepts false runs no concept review')
  check(quiet.commands.some((c) => /-Step hubs-report/.test(c) && !/-Concepts/.test(c)), 'defaults: without a review the register plan reads none')
  // A book whose term table proposes nothing is skipped: no evidence, no judge job, no -Concepts.
  const none = await run(args, { ...answers, 'concepts -Step propose': line({ ok: true, step: 'propose', proposals: 0, reason: 'no row' }) })
  check(!none.thrown && !none.commands.some((c) => /concepts -Step prepare/.test(c)) && !none.prompts.some((x) => /concepts/.test(x.label)), 'concepts: a term table with no concept starts no judge job')
  check(none.report.result.concepts.reviews[0].status === 'skipped' && none.commands.some((c) => /-Step hubs-report/.test(c) && !/-Concepts/.test(c)), 'concepts: the skipped review is reported and the register reads no report')
  // Mode hubs: the concept review of the named books still feeds the register.
  const hubs = await run({ out: 'D:/x/hubs', mode: 'hubs', books: ['Key'] }, answers)
  check(!hubs.thrown && hubs.commands.some((c) => /-Step concept-books .*-Books 'Key'/.test(c)) && hubs.commands.some((c) => /-Step hubs-report .*-Concepts/.test(c)), 'mode hubs: the named books\' concept review feeds the register')
  check(!hubs.commands.some((c) => /organize-/.test(c)), 'mode hubs: no Topic maintenance (it moves Cards)')
  // The owner's Lock refuses a maintenance write: the stage stops with the typed refusal.
  const locked = await run({ ...args, organize: true }, { ...answers, '-Step organize-apply': { exit_code: 2, last_line: json(refusal), tail: json(refusal) } })
  check(locked.report && locked.report.halted && locked.report.halted.stage === 'grow' && /card_locked/.test(String(locked.report.halted.error)), 'organize: a Lock refusal stops the stage with card_locked')
  check(!locked.commands.some((c) => /-Step hubs-plan/.test(c)), 'organize: nothing after the Lock stop runs')
}

console.log(`failed: ${failed}`)
process.exit(failed ? 1 : 0)
