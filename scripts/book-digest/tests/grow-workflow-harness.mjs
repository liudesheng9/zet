// Run: node scripts/book-digest/tests/grow-workflow-harness.mjs
// The current zt-idea-grow build (zt-grow-cycle v14, zt-workflow-lib v7, #180
// docs/workflow-module-structure-proposal.md) against a stand-in runtime. The build is assembled from its blueprint
// and its nested script is run with stand-ins for the Workflow runtime, so no model, DeepSeek, zt or archive call
// happens. The order of the rounds is zt-digest's (grow_next.rs, tested by cargo test and grow.tests.ps1); this
// harness checks the driver.
//
// What it checks:
//  - one loop with three branches: a compact jobs line (`{ok, next:"jobs", round, token, jobs, file}`) goes to one
//    Haiku manager per eight entries of the file the step wrote, and the next step runs again with `-After <token>`;
//    a done line ends the stage as its report; a fault line stops it;
//  - every agent of the grow stage is a Haiku DeepSeek manager, no agent runs as the zt-relay type, and no agent
//    relays a command;
//  - every drive answer is at most 400 characters with a boolean top-level `ok`, and a prose or `ok`-less answer
//    fails the stage instead of reading a nested object (#180);
//  - the options reach the step as one JSON object (the box copy, the mode, the frozen model, high effort);
//  - the built script carries zt-grow-cycle v14 and zt-workflow-lib v7, names no relay helper, and carries the DSMP
//    block; the blueprint declares the manager role (no relay role).
import os from 'node:os'
import path from 'node:path'
import fs from 'node:fs'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..', '..', '..')
const wfpRoot = process.env.WFP_ROOT || path.join(os.homedir(), '.claude', 'skills', 'workflow-framework-protocol')
const lib = await import(pathToFileURL(path.join(wfpRoot, 'tools', 'lib.mjs')).href)
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor

const loaded = lib.loadBlueprint(path.join(root, 'scripts', 'idea-grow', 'workflow', 'zt-idea-grow.blueprint.json'))
const build = lib.build(loaded.bp, lib.loadModules(loaded.dirs))
const source = build.script.replace(/^export /m, '')

let failed = 0
const check = (cond, message) => { console.log(`${cond ? 'ok' : 'FAIL'}: ${message}`); if (!cond) failed++ }

const W = 'D:\\x\\grow'
const jobsLine = (round, token, ids) => ({ ok: true, next: 'jobs', round, token, sum: `${token}abc123`, jobs: ids.length, file: `${W}\\grow-jobs.json` })
const done = { ok: true, next: 'done', mode: 'full', created: 4, topics: 1, linked: 2, faults_left: 0, share_before: 0.1, share_after: 0.5, report: `${W}\\grow-report.md`, hubs: 1, register: 'published' }
const lockFault = { ok: true, next: 'fault', code: 'card_locked', round: 'apply', cause: 'the agent Topic 4/0 is locked by the owner', tree: '4/0', lock_id: 3 }

// The stand-in runtime. `lines` answers the grow step's drive jobs in order: one entry per `next` call, either the
// compact JSON line the step prints (an object) or a raw { output } text. The round manager answers the entries of
// the job set the last `next` line named.
function runtime(lines) {
  const commands = [], labels = [], managers = [], kinds = [], opts = [], answers = [], driveAnswers = []
  let n = 0
  let pending = []
  const agent = async (prompt, o = {}) => {
    const text = String(prompt)
    labels.push(o.label || '')
    opts.push(o)
    if (/^You are a DeepSeek subagent manager for a ZET workflow step/.test(text)) {
      kinds.push('drive')
      const quoted = text.match(/prompt ("(?:[^"\\]|\\.)*"), skills \[\]/)
      const decoded = quoted ? JSON.parse(quoted[1]) : ''
      const command = decoded.slice(decoded.indexOf('\n\n') + 2)
      commands.push(command)
      const e = lines[n++]
      if (e === undefined) return { answer: 'no more lines', session_id: `s-next-${n}`, deleted: true, registered: false, evidence: false }
      const answer = e.output !== undefined ? e.output : JSON.stringify(e)
      if (e.output === undefined) { answers.push(e); pending = (e.next === 'jobs' ? e.jobs : 0) }
      driveAnswers.push(answer)
      return { answer, session_id: `s-next-${n}`, deleted: true, registered: false, evidence: false }
    }
    if (/^You are a DeepSeek subagent manager for the ZET idea-grow round/.test(text)) {
      kinds.push('manager')
      const file = (text.match(/read the JSON array in "([^"]+)"/) || [])[1] || ''
      const range = text.match(/run its entries (\d+) through (\d+)/)
      managers.push({ label: o.label, file, from: Number(range && range[1] || 0), to: Number(range && range[2] || 0) })
      const count = range ? Number(range[2]) - Number(range[1]) + 1 : pending
      return { ok: true, jobs: Array.from({ length: count }, (_, i) => ({ id: `job-${i}`, session_id: `s-m${i}`, status: 'done', answer: 'OK', deleted: true })) }
    }
    if (/^Remove finished DeepSeek workspaces/.test(text)) { kinds.push('cleanup'); return { folders: [{ cwd: W, removed: true, leftover: [], running: 0 }] } }
    throw new Error(`the stand-in runtime answers no other agent: ${text.slice(0, 120)}`)
  }
  return { agent, commands, labels, managers, kinds, opts, answers, driveAnswers, phase: () => {}, log: () => {}, parallel: async (fns) => Promise.all(fns.map((f) => f())), pipeline: async (items, ...st) => Promise.all(items.map(async (item, i) => { let v = item; for (const x of st) v = await x(v, item, i); return v })) }
}

async function run(args, lines) {
  const rt = runtime(lines)
  const fn = new AsyncFunction('args', 'agent', 'phase', 'log', 'parallel', 'pipeline', source)
  let report, thrown
  try { report = await fn(args, rt.agent, rt.phase, rt.log, rt.parallel, rt.pipeline) } catch (error) { thrown = error }
  return { report, thrown, ...rt }
}
// The kernel records a stage that threw as halted, with the error text (the run itself does not throw).
const halted = (r) => (r.report && r.report.halted && r.report.halted.stage === 'grow' ? String(r.report.halted.error) : '')
const nexts = (r) => r.commands.filter((c) => /grow-steps\.ps1" -Step next /.test(c))
const optionsOf = (command) => JSON.parse((command.match(/-Options '((?:[^']|'')*)'/) || [])[1].replace(/''/g, "'"))

// 1. A normal run: three job sets, then done.
{
  const r = await run({ out: W }, [jobsLine('topics-jobs', 1, ['gt-001']), jobsLine('ideas-jobs', 2, ['gw-001', 'gw-002']), jobsLine('fix-jobs', 3, ['fx1-001']), done])
  check(!r.thrown, `a normal run completes (${r.thrown && r.thrown.message})`)
  const n = nexts(r)
  check(n.length === 4, `one grow-steps.ps1 -Step next drive job per round (${n.length})`)
  check(n.length === r.kinds.filter((k) => k === 'drive').length, 'every round is one drive job and nothing else runs a command')
  check(!/ -After /.test(n[0]) && / -After 1( |$)/.test(n[1]) && / -After 2( |$)/.test(n[2]) && / -After 3( |$)/.test(n[3]), 'each next after a dispatch names the token of the job set it confirms')
  check(r.managers.length === 3 && r.managers.map((m) => m.to - m.from + 1).join() === '1,2,1', `every job set goes to one manager per eight entries (${r.managers.map((m) => m.to - m.from + 1).join(',')})`)
  check(r.managers.every((m) => /grow-jobs\.json$/.test(m.file)), 'each manager reads the job file the step wrote')
  // Every agent of the stage is a Haiku DeepSeek manager: the drive job's manager and the round managers; the
  // protocol's own workspace cleanup is the only other agent, and no agent is a relay.
  const all = r.opts
  check(all.every((o) => o.model === 'haiku' && !o.agentType), `every agent of the grow stage is a Haiku manager with no relay agent type (${[...new Set(all.map((o) => `${o.model || 'none'}/${o.agentType || 'no-type'}`))].join(', ')})`)
  check(r.kinds.filter((k) => k === 'cleanup').length === 1 && !r.kinds.includes('relay'), 'only the protocol cleanup runs besides the managers; no relay runs')
  check(r.report && r.report.result.report.created === 4 && r.report.result.report.register === 'published', 'the done line is the run\'s report')
  check(r.report && r.report.result.rounds.length === 3 && r.report.result.rounds[1].ok === 2, 'the result lists the dispatched rounds')
  // #180: every drive answer is one compact JSON line with a boolean top-level `ok`.
  const longest = Math.max(...r.driveAnswers.map((a) => a.length))
  check(r.driveAnswers.every((a) => a.length <= 400 && typeof JSON.parse(a).ok === 'boolean'), `#180: every drive answer is at most 400 characters with a boolean ok (longest ${longest})`)
  const o = optionsOf(n[0])
  check(o.mode === 'full' && o.model === 'deepseek-official/deepseek-flash' && o.effort === 'high' && o.zt_config === '' && o.restate === true && o.hubs === true && o.organize === false, 'the options reach the step as one JSON object with the frozen model and the defaults')
  check(n.every((c) => optionsOf(c).instructions === o.instructions && / -Tool 'zt-digest'$/.test(c)), 'every next names the same options and the tool')
  check(n.every((c) => !/ -ZtConfig /.test(c)), 'a live-box run names no -ZtConfig')
}

// 2. A step that found no readable line ({ok:false}) stops the stage with its error; nothing is dispatched.
{
  const r = await run({ out: W }, [{ ok: false, step: 'next', error: 'next: no readable line from next or its status (still prose)' }])
  check(/no readable line/.test(halted(r)) && r.managers.length === 0 && nexts(r).length === 1, `a failed step stops the stage with its error (${halted(r)})`)
  check(r.report.stages.report.status !== 'done' && !(r.report.result && r.report.result.report), 'a stopped stage reports no completed result')
  check(!r.kinds.includes('cleanup'), 'no cleanup runs when no DeepSeek job started')
}

// 3. #180: an answer wrapped in prose, or one whose last line has no boolean `ok`, fails the stage.
{
  const prose = await run({ out: W }, [{ output: `Here is the step's answer: ${JSON.stringify(done)}` }])
  check(/is not its one JSON line/.test(halted(prose)) && /not one JSON object/.test(halted(prose)), `#180: a prose-wrapped answer fails the stage (${halted(prose).slice(0, 160)})`)
  const noOk = await run({ out: W }, [{ output: JSON.stringify({ next: 'jobs', round: 'x', token: 1, jobs: 1, file: `${W}\\grow-jobs.json` }) }])
  check(/no boolean top-level ok/.test(halted(noOk)), `#180: an answer without a boolean ok fails the stage (${halted(noOk).slice(0, 160)})`)
  const nested = await run({ out: W }, [{ output: JSON.stringify({ state: 'done', counts: { open: 0 } }) }])
  check(/no boolean top-level ok/.test(halted(nested)), '#180: a nested object is never read as the answer')
  // Live run 2026-10-26 (staging4, link start): the manager copied the line with each `\\` of a Windows path as one
  // `\`. The line is still the step's one object: the reader doubles the lone backslashes and reads it.
  const unescaped = await run({ out: W }, [{ output: JSON.stringify(jobsLine('ideas-jobs', 1, ['gw-001'])).replace(/\\\\/g, '\\') }, done])
  // Live run 2026-10-26 (staging5, cleanup report): the manager's `answer` carried the end of its own tool call after
  // the line (`</answer>` and a `<parameter name="session_id">` line). The markup is cut before the last line is read.
  const leaked = await run({ out: W }, [{ output: `${JSON.stringify(done)}</answer>\n<parameter name="session_id">eb840fdd-c9f8` }])
  check(!halted(leaked) && leaked.report && leaked.report.result.report.created === 4, `a line followed by leaked tool-call markup is read (${halted(leaked).slice(0, 160)})`)
  check(!halted(unescaped) && unescaped.managers.length === 1 && unescaped.managers[0].file === `${W}\\grow-jobs.json`, `a line copied with unescaped backslashes is read with its paths (${halted(unescaped).slice(0, 160)})`)
}

// 4. S6: the owner's Lock stops the stage with the typed refusal; nothing is retried.
{
  const r = await run({ out: W }, [jobsLine('ideas-jobs', 1, ['gw-001']), lockFault, done])
  const said = halted(r)
  check(said.includes('"tree":"4/0"') && said.includes('"lock_id":3') && said.includes('locked by the owner'), `a card_locked fault stops the stage with its tree and Lock id (${said})`)
  check(said.startsWith('card_locked at grow: apply') && r.report.stages.report.status !== 'done', 'the stop names the round the Lock refused; the report stage does not run')
  check(nexts(r).length === 2 && r.managers.length === 1, 'the refused round is never retried')
  check(r.kinds.includes('cleanup'), 'the DeepSeek workspace cleanup still runs')
}

// 5. A fault that is not a Lock stops the stage with its code and cause.
{
  const r = await run({ out: W }, [{ ok: true, next: 'fault', code: 'step_failed', round: 'check', cause: 'zt-digest grow -Step exited 1: boom' }])
  check(/^grow stopped at check: step_failed: .*boom/.test(halted(r)), 'a step failure stops the stage with its code and cause')
  check(!r.kinds.includes('cleanup'), 'no cleanup runs when no DeepSeek job started')
}

// 6. The options: a box copy, mode hubs, the hub options; frozen args leave the list.
{
  const r = await run({ out: W, ztConfig: 'D:\\box\\config', hubTrees: '4,5', hubRebuild: true, hubLimits: { line_min: 12 }, organize: true, books: ['a', 'b'] }, [done])
  const c = nexts(r)[0]
  const o = optionsOf(c)
  check(/ -ZtConfig 'D:\\box\\config'/.test(c), 'a box copy names its config on the step')
  check(o.zt_config === 'D:\\box\\config' && o.hub_trees === '4,5' && o.hub_rebuild === true && o.hub_limits.line_min === 12 && o.organize === true && o.books.join() === 'a,b', 'the box copy and the hub, organize and book options reach the step')
  const h = await run({ out: W }, [{ ...done, mode: 'hubs' }])
  check(!h.thrown, 'mode full is the default')
  const bp = loaded.bp
  check(!['fixRounds', 'hubFixRounds', 'hubEffort', 'dshModel', 'organizeTrees'].some((k) => k in bp.args), 'fixRounds, hubFixRounds, hubEffort, dshModel and organizeTrees left the arguments (S9)')
  check(JSON.stringify(bp.args.register.enum) === '["apply","off"]', 'register takes apply or off')
  check(bp.roles && !bp.roles.relay && !bp.roles.runner && bp.roles.manager && bp.roles.manager.model === 'haiku' && bp.roles.manager.effort === 'low', 'the manager role replaces the relay role: Haiku, low effort, no agent type')
  check(!Object.keys(bp.stages[0].params).includes('relay'), 'the grow stage no longer names a relay role param')
}

// 7. The frozen build.
{
  check(build.modules['zt-grow-cycle'] === '14' && build.modules['zt-workflow-lib'] === '9', `the build carries zt-grow-cycle v14 and zt-workflow-lib v9 (${JSON.stringify(build.modules)})`)
  const cycle = source.slice(source.indexOf('// ==== module zt-grow-cycle'), source.indexOf('// ==== end module zt-grow-cycle'))
  check(!/ztRelay\s*\(|runScript\s*\(|wfpScript\s*\(/.test(cycle), 'the grow cycle calls no relay helper')
  check(/ztDrive\(ctx/.test(cycle) && /grow-steps\.ps1/.test(cycle) && /dispatch\(line\)/.test(cycle), 'the cycle drives one grow-steps.ps1 step per round and dispatches each checked job set from its file')
  check(/function ztDrive\b/.test(source) && /function ztDrivePrompt/.test(source) && /function ztDriveAnswer/.test(source), 'the shared library keeps the one drive helper and its strict answer reader')
  check(/dsmpSlot/.test(source) && /dsCleanupWorkspaces/.test(source), 'the built script carries the DSMP block (dsmpSlot, dsCleanupWorkspaces)')
  const steps = fs.readFileSync(path.join(root, 'scripts', 'idea-grow', 'grow-steps.ps1'), 'utf8')
  check(/\[ValidateSet\('next'\)\]/.test(steps) && /grow -Step/.test(steps) && /-Step status/.test(steps), 'grow-steps.ps1 has the one next step, the grow command and its one status recovery')
  check(/Expand-Jobs/.test(steps) && /grow-jobs\.json/.test(steps), 'grow-steps.ps1 expands a job set to the file its manager reads')
  check(!/ztRelay|runScript|wfpScript/.test(steps), 'grow-steps.ps1 names no relay helper')
}

console.log(failed ? `${failed} check(s) FAILED` : 'all checks passed')
process.exit(failed ? 1 : 0)
