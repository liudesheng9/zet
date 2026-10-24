// Run: node scripts/book-digest/tests/grow-workflow-harness.mjs
// The frozen-workflow fixture of zt-idea-grow 4.0.0 (zt-grow-cycle v12, docs/workflow-simplicity-proposal.md S1-S4,
// S6, S7). It builds zt-idea-grow from its blueprint with the workflow-framework-protocol assembler and runs the
// built script against a stand-in runtime: `zt-digest grow -Step next` answers from a canned sequence of lines, so
// no model, DeepSeek, zt or archive call runs. The order of the rounds is zt-digest's (grow_next.rs, tested by
// cargo test and grow.tests.ps1); this harness checks the driver.
//
// What it checks:
//  - one loop with three branches: a jobs line goes to DSMP dsDispatch with `{id}` filled per job, and `next`
//    runs again with `-After <token>`; a done line ends the stage as its report; a fault line stops it;
//  - every command runs through the relay role (the zt-relay agent type), every DeepSeek job through a DSMP
//    manager, and no other agent kind runs (the DeepSeek workspace cleanup is the protocol's own);
//  - S2: a jobs line a relay cut or rewrote (its checksum fails) is read again from `grow -Step status`; a status
//    line that repeats the job set already dispatched starts no second dispatch; a second unreadable line stops;
//  - S6: a `card_locked` fault stops the stage with the owner's refusal (tree, Lock id), never retried, and the
//    report stage does not run;
//  - the options reach `next` as one JSON object (the box copy, the mode, the frozen model, high effort);
//  - the built script carries zt-grow-cycle v12 and zt-workflow-lib v1, and reads a Lock in one place.
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

let failed = 0
const check = (cond, message) => { console.log(`${cond ? 'ok' : 'FAIL'}: ${message}`); if (!cond) failed++ }

const W = 'D:\\x\\grow'
const fnv = (t) => { let h = 0x811c9dc5; for (const ch of String(t)) { h ^= ch.codePointAt(0); h = Math.imul(h, 0x01000193) >>> 0 } return h.toString(16).padStart(8, '0') }
const template = (family) => ({
  family, cwd: W, files: family === 'grow' ? [`${W}\\instructions.zh.md`, `${W}\\jobs\\{id}.md`] : [`${W}\\jobs\\{id}.md`],
  prompt: `完成任务 {id}。SELF-CHECK: zt-digest grow -Step precheck -Work "${W}" -Job {id}`, skills: ['zettelkasten', 'asd-ste100-zh'],
  model: 'deepseek-official/deepseek-flash', effort: 'high', expect: '(^|\\n)(OK|FLAG:)', approve: `allow only: the precheck for {id}; reads under "${W}"`,
})
const jobs = (round, token, ids, family = 'grow') => {
  const job = template(family)
  return { next: 'jobs', round, token, ids, job, sum: fnv([ids.join(','), job.cwd, job.files.join('\n'), job.prompt, job.approve, job.expect].join('\n')) }
}
const done = { next: 'done', mode: 'full', created: 4, topics: 1, linked: 2, faults_left: 0, share_before: 0.1, share_after: 0.5, report: `${W}\\grow-report.md`, hubs: { topic: 1 }, register: 'published' }
const lockFault = { next: 'fault', code: 'card_locked', round: 'apply', cause: 'the agent Topic 4/0 is locked by the owner', tree: '4/0', lock_id: 3 }

// The stand-in runtime. `lines` answers the relays in order: a `next` relay takes the next entry; a `status` relay
// takes the entry from `status` (or repeats the last `next` line). An entry may be { output } to send raw text.
function runtime(lines, status = []) {
  const commands = [], labels = [], logs = [], managers = [], kinds = [], opts = []
  let n = 0, s = 0, last = null
  const relay = (text) => {
    const command = (text.match(/^Run this PowerShell command once[^\n]*\n(.+)$/m) || [])[1] || ''
    commands.push(command)
    if (/ -Step status /.test(command)) {
      const e = s < status.length ? status[s++] : last
      return e && e.output !== undefined ? { ok: true, output: e.output } : { ok: true, output: JSON.stringify(e) }
    }
    const e = lines[n++]
    if (e === undefined) return { ok: true, output: 'no more lines' }
    if (e.output !== undefined) return { ok: true, output: e.output }
    last = e
    return { ok: true, output: JSON.stringify(e) }
  }
  const agent = async (prompt, o = {}) => {
    const text = String(prompt)
    labels.push(o.label || '')
    opts.push(o)
    if (/^Run this PowerShell command once/.test(text)) { kinds.push('relay'); return relay(text) }
    if (/^You are a DeepSeek subagent manager\./.test(text)) {
      kinds.push('manager')
      const list = JSON.parse(text.slice(text.indexOf('JOBS ('), text.indexOf('\n\n1. Start')).replace(/^JOBS \(\d+\):\n/, ''))
      managers.push({ label: o.label, jobs: list })
      return { jobs: list.map((j) => ({ id: j.id, ok: true, reply: 'OK', attempts: 1 })), started: list.map((j) => `s-${j.id}`), deleted: list.length, not_deleted: [] }
    }
    if (/^Remove finished DeepSeek workspaces/.test(text)) { kinds.push('cleanup'); return { folders: [{ cwd: W, removed: true, leftover: [], running: 0 }] } }
    throw new Error(`the stand-in runtime answers no other agent: ${text.slice(0, 120)}`)
  }
  return { agent, commands, labels, logs, managers, kinds, opts, phase: () => {}, log: (m) => logs.push(m), parallel: async (fns) => Promise.all(fns.map((f) => f())), pipeline: async (items, ...st) => Promise.all(items.map(async (item, i) => { let v = item; for (const x of st) v = await x(v, item, i); return v })) }
}

async function run(args, lines, status) {
  const rt = runtime(lines, status)
  const fn = new AsyncFunction('args', 'agent', 'phase', 'log', 'parallel', 'pipeline', source)
  let report, thrown
  try { report = await fn(args, rt.agent, rt.phase, rt.log, rt.parallel, rt.pipeline) } catch (error) { thrown = error }
  return { report, thrown, ...rt }
}
// The kernel records a stage that threw as halted, with the error text (the run itself does not throw).
const halted = (r) => (r.report && r.report.halted && r.report.halted.stage === 'grow' ? String(r.report.halted.error) : '')
const nexts = (r) => r.commands.filter((c) => / -Step next /.test(c))
const optionsOf = (command) => JSON.parse((command.match(/-Options '((?:[^']|'')*)'/) || [])[1].replace(/''/g, "'"))

// 1. A normal run: three job sets, then done.
{
  const r = await run({ out: W }, [jobs('topics-jobs', 1, ['gt-001']), jobs('ideas-jobs', 2, ['gw-001', 'gw-002']), jobs('fix-jobs', 3, ['fx1-001'], 'plain'), done])
  check(!r.thrown, `a normal run completes (${r.thrown && r.thrown.message})`)
  const n = nexts(r)
  check(n.length === 4, `next runs once per line (${n.length})`)
  check(!/ -After /.test(n[0]) && / -After 1$/.test(n[1]) && / -After 2$/.test(n[2]) && / -After 3$/.test(n[3]), 'each next after a dispatch names the token of the job set it confirms')
  check(r.managers.length === 3 && r.managers.map((m) => m.jobs.length).join() === '1,2,1', 'every job set goes to one DSMP manager')
  const gw = r.managers[1].jobs[1]
  check(gw.id === 'gw-002' && gw.run.prompt_files[1] === `${W}\\jobs\\gw-002.md` && /-Job gw-002/.test(gw.run.prompt) && /for gw-002|precheck for gw-002/.test(gw.approve), 'the job template is filled with each job id (files, prompt, approval rule)')
  check(gw.run.cwd === W && gw.run.model === 'deepseek-official/deepseek-flash' && gw.run.reasoning_effort === 'high' && gw.expect === '(^|\\n)(OK|FLAG:)', 'the job carries the cwd, the official model, high effort and the expected reply')
  check(r.managers[2].jobs[0].run.prompt_files.length === 1, 'a plain family job reads only its job file')
  check(r.kinds.every((k) => ['relay', 'manager', 'cleanup'].includes(k)) && r.kinds.filter((k) => k === 'cleanup').length === 1, `only relays, DSMP managers and one protocol cleanup run (${[...new Set(r.kinds)].join(', ')})`)
  const relayOpts = r.opts.filter((o, i) => r.kinds[i] === 'relay')
  check(relayOpts.length === 4 && relayOpts.every((o) => o.agentType === 'zt-relay' && o.model === 'haiku'), 'every command runs through the relay role (the zt-relay agent type)')
  check(r.report && r.report.result.report.created === 4 && r.report.result.report.register === 'published', 'the done line is the run\'s report')
  check(r.report && r.report.result.rounds.length === 3 && r.report.result.rounds[1].ok === 2, 'the result lists the dispatched rounds')
  const o = optionsOf(n[0])
  check(o.mode === 'full' && o.model === 'deepseek-official/deepseek-flash' && o.effort === 'high' && o.zt_config === '' && o.restate === true && o.hubs === true && o.organize === false, 'the options reach next as one JSON object with the frozen model and the defaults')
  check(n.every((c) => optionsOf(c).instructions === o.instructions), 'every next names the same options')
}

// 2. S2: a jobs line a relay cut, and one it rewrote (the checksum fails): the status is read once, then the run goes on.
{
  const good = jobs('ideas-jobs', 1, ['gw-001'])
  const rewritten = { ...good, ids: ['gw-001', 'gw-009'] }
  const r = await run({ out: W }, [{ output: JSON.stringify(good).slice(0, 120) + ' (abridged)' }, done], [good])
  check(!r.thrown && r.managers.length === 1 && r.managers[0].jobs[0].id === 'gw-001', `a cut jobs line is read again from the status (${r.thrown && r.thrown.message})`)
  check(r.commands.filter((c) => / -Step status /.test(c)).length === 1 && / -After 1$/.test(nexts(r)[1]), 'the status read happens once and the run confirms its job set')
  const r2 = await run({ out: W }, [rewritten, done], [good])
  check(!r2.thrown && r2.managers.length === 1 && r2.managers[0].jobs.length === 1, 'a rewritten jobs line (its checksum fails) is never dispatched; the status line is')
  const r3 = await run({ out: W }, [{ output: 'Command ran fine.' }], [{ output: 'still prose' }])
  check(/no readable line/.test(halted(r3)) && r3.managers.length === 0, 'a second unreadable line stops the stage')
  check(r3.report.stages.report.status !== 'done' && !(r3.report.result && r3.report.result.report), 'a stopped stage reports no completed result')
}

// 3. S2: an unreadable confirmation whose status repeats the dispatched set starts no second dispatch.
{
  const set = jobs('ideas-jobs', 1, ['gw-001'])
  const r = await run({ out: W }, [set, { output: 'garbled' }, done], [set])
  check(!r.thrown && r.managers.length === 1, `the repeated job set is not dispatched twice (${r.thrown && r.thrown.message})`)
  check(nexts(r).filter((c) => / -After 1$/.test(c)).length === 2, 'next -After runs again for the repeated set')
}

// 4. S6: the owner's Lock stops the stage with the typed refusal; nothing is retried.
{
  const r = await run({ out: W }, [jobs('ideas-jobs', 1, ['gw-001']), lockFault, done])
  const said = halted(r)
  check(said.includes('"tree":"4/0"') && said.includes('"lock_id":3') && said.includes('locked by the owner'), `a card_locked fault stops the stage with its tree and Lock id (${said})`)
  check(said.startsWith('card_locked at grow: apply') && r.report.stages.report.status !== 'done', 'the stop names the round the Lock refused; the report stage does not run')
  check(nexts(r).length === 2 && r.managers.length === 1, 'the refused round is never retried')
  check(r.kinds.includes('cleanup'), 'the DeepSeek workspace cleanup still runs')
}

// 5. A fault that is not a Lock stops the stage with its code and cause.
{
  const r = await run({ out: W }, [{ next: 'fault', code: 'step_failed', round: 'check', cause: 'zt-digest grow -Step exited 1: boom' }])
  check(/^grow stopped at check: step_failed: .*boom/.test(halted(r)), 'a step failure stops the stage with its code and cause')
  check(!r.kinds.includes('cleanup'), 'no cleanup runs when no DeepSeek job started')
}

// 6. The options: a box copy, mode hubs, the hub options; frozen args leave the list.
{
  const r = await run({ out: W, ztConfig: 'D:\\box\\config', hubTrees: '4,5', hubRebuild: true, hubLimits: { line_min: 12 }, organize: true, books: ['a', 'b'] }, [done])
  const c = nexts(r)[0]
  const o = optionsOf(c)
  check(/^\$env:ZT_CONFIG_DIR='D:\\box\\config'; & 'zt-digest' grow -Step next /.test(c), 'a box copy names its config before the command')
  check(o.zt_config === 'D:\\box\\config' && o.hub_trees === '4,5' && o.hub_rebuild === true && o.hub_limits.line_min === 12 && o.organize === true && o.books.join() === 'a,b', 'the box copy and the hub, organize and book options reach next')
  const h = await run({ out: W }, [{ ...done, mode: 'hubs' }])
  check(!h.thrown, 'mode full is the default')
  const bp = loaded.bp
  check(!['fixRounds', 'hubFixRounds', 'hubEffort', 'dshModel', 'organizeTrees'].some((k) => k in bp.args), 'fixRounds, hubFixRounds, hubEffort, dshModel and organizeTrees left the arguments (S9)')
  check(JSON.stringify(bp.args.register.enum) === '["apply","off"]', 'register takes apply or off')
  check(bp.roles.relay && bp.roles.relay.agentType === 'zt-relay' && !bp.roles.runner, 'the relay role replaces the runner role (S1)')
}

// 7. The frozen build.
{
  check(build.modules['zt-grow-cycle'] === '12' && build.modules['zt-workflow-lib'] === '1', `the build carries zt-grow-cycle v12 and zt-workflow-lib v1 (${JSON.stringify(build.modules)})`)
  const cycle = source.slice(source.indexOf('// ==== module zt-grow-cycle'), source.indexOf('// ==== end module zt-grow-cycle'))
  check((cycle.match(/ZT_LOCK/g) || []).length === 1 && !/JSON\.parse/.test(cycle) && (source.match(/function ztLockRefusal/g) || []).length === 1, 'the cycle checks the Lock once, on the typed fault line; the one reader is the shared library\'s')
  check(!/You run one compiler chain|You drive DeepSeek Harness|You manage \d+ DeepSeek/.test(source) && /dsDispatch\(line\.ids/.test(cycle), 'every DeepSeek job goes through dsDispatch; no hand-written manager prompt')
}

console.log(failed ? `${failed} check(s) FAILED` : 'all checks passed')
process.exit(failed ? 1 : 0)
