// Run: node scripts/book-digest/tests/term-cards-harness.mjs
// The provider-contract fixture of the Term-card branch (S20, #164). It builds
// term-cards-fixture.blueprint.json with the workflow-framework-protocol assembler and runs it
// against a stand-in runtime: the stand-in answers the compiler commands (plan, jobs, check,
// review-plan, fix-plan) with one fixed six-row scenario, and plays the DeepSeek provider: it
// starts, collects, continues and deletes conversations, counts the live managers and
// conversations, and removes a workspace only when it is empty. No model, DeepSeek, zt or archive
// call runs.
//
// What it checks:
//  - roles: the compiler commands run through the relay role, every job manager is the declared
//    Haiku/low manager, and every content job carries the tool's official model, high effort and
//    skills (the manager prompt embeds the job records unchanged);
//  - waves: at most four writer jobs per manager, at most four review jobs, and never more than
//    eight conversations (four retained writers plus their reviewers) per branch;
//  - continuation: the one fix goes back to the writer's own conversation (deepseek_prompt), with
//    the writer's session named in the table and its fallback prompt files;
//  - deletion: reviews are deleted after their answer, a wave's writers after its fix/review
//    opportunity, the workspace registration is removed at the end, and the staging files stay;
//  - the pool: ten concurrent branches never hold more than eight managers, so the branch uses the
//    script-wide dsmpSlot pool; the prompts carry the protocol's own 240 min / 90 s budget;
//  - the failure path: a failed writer job leaves the branch failed, still settles the wave's
//    conversations, still removes the workspace registration and keeps the staged files.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..', '..', '..')
const wfpRoot = process.env.WFP_ROOT || path.join(os.homedir(), '.claude', 'skills', 'workflow-framework-protocol')
const lib = await import(pathToFileURL(path.join(wfpRoot, 'tools', 'lib.mjs')).href)
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor
const fixture = path.join(here, 'term-cards-fixture.blueprint.json')

function build(file) {
  const l = lib.loadBlueprint(file)
  const b = { name: l.bp.name, ...lib.build(l.bp, lib.loadModules(l.dirs)), bp: l.bp }
  return { ...b, source: b.script.replace(/^export /m, '') }
}
const builds = { one: build(fixture) }

// --- the compiler stand-in: one book's fixed six-row scenario ------------------------------------
const workOf = (out) => `${out}\\term-cards`
const WRITER_JOB = (out, id, kind, rows, terms) => {
  const work = workOf(out)
  return {
    id, kind, rows, terms,
    files: [`${work}\\dsh-term-card-instructions.zh.md`, `${work}\\jobs\\${id}.md`],
    prompt: '按任务说明和任务文件完成本组术语，写出任务文件指定的 JSON 文件。最后只回复一行 JSON。',
    output: `${work}\\out\\${id}.json`,
    permissions: { archive_read: true, archive_write: false },
    model: 'deepseek-official/deepseek-flash', effort: 'high',
    skills: ['zettelkasten', 'asd-ste100-zh'], delete_after_collect: false,
  }
}
const REVIEW_JOB = (out, id, row, term) => {
  const work = workOf(out)
  return {
    id: `${id}-r${String(row).padStart(3, '0')}-review`, kind: 'review', row, term,
    files: [`${work}\\dsh-term-review-instructions.zh.md`, `${work}\\review\\requests\\${id}-r${String(row).padStart(3, '0')}.md`],
    prompt: '按审查说明和任务文件审查这一张数学术语卡草稿，只写任务文件指定的答案文件，最后只回复一行 JSON。',
    answer: `${work}\\review\\answers\\${id}-r${String(row).padStart(3, '0')}.json`,
    writes: `${work}\\review\\answers`,
    request: `${work}\\review\\requests\\${id}-r${String(row).padStart(3, '0')}.md`,
    model: 'deepseek-official/deepseek-flash', effort: 'high',
    skills: ['asd-ste100-zh'], delete_after_collect: true,
  }
}
const FIX_JOB = (out, id, row, term) => {
  const work = workOf(out)
  const output = `${work}\\out\\${id}.json`
  return {
    id: `${id}-fix`, kind: 'fix', continue: id, row, term,
    files: [`${work}\\dsh-term-card-instructions.zh.md`, `${work}\\fix\\${id}-fix.md`],
    fallback: [`${work}\\dsh-term-card-instructions.zh.md`, `${work}\\jobs\\${id}.md`, output],
    prompt: '按修复任务只改任务文件列出的问题，重写你负责的那一份 JSON 草稿，只写任务文件指定的输出文件，最后只回复一行 JSON。',
    output,
    model: 'deepseek-official/deepseek-flash', effort: 'high',
    skills: ['zettelkasten', 'asd-ste100-zh'], delete_after_collect: false,
  }
}

// One book's compiler state: which outputs exist, which reviews were answered and whether the one
// fix has been written. The counts are the readiness counts the real tool would print.
function book(out, opts = {}) {
  const work = workOf(out)
  return {
    out, work, written: new Set(), approved: new Set(), fixed: false, grouped: opts.grouped === true,
    jobs: opts.empty ? [] : [
      WRITER_JOB(out, 't001', 'math', [1], ['均衡']),
      WRITER_JOB(out, 't002', 'math', [2], ['替代弹性']),
      // A grouped concept job whose two terms are promoted to math: its reviews must run one after
      // the other within the reviewer slots (R0335), never side by side.
      opts.grouped ? WRITER_JOB(out, 't003', 'concept', [3, 4], ['剩余控制权', '敲竹杠']) : WRITER_JOB(out, 't003', 'math', [3], ['剩余控制权']),
      WRITER_JOB(out, 't004', 'math', [4], ['敲竹杠']),
      WRITER_JOB(out, 't005', 'concept', [5, 6], ['机会主义', '专用性']),
      WRITER_JOB(out, 't006', 'concept', [7, 8], ['交易成本', '治理']),
    ],
    // The term rows a written job still owes a review for.
    reviewRows(job) {
      if (this.grouped && job.id === 't003') return job.rows
      return job.kind === 'math' ? [job.rows[0]] : []
    },
    states() {
      return this.jobs.map((job) => {
        if (!this.written.has(job.id)) return 'missing'
        if (job.id === 't005' && !this.fixed) return 'needs_fix'
        if (this.reviewRows(job).some((row) => !this.approved.has(`${job.id}#${row}`))) return 'needs_review'
        return 'ready'
      })
    },
    counts() {
      const counts = { missing: 0, needs_fix: 0, needs_review: 0, ready: 0, refused: 0, unchecked: 0 }
      for (const state of this.states()) counts[state]++
      return counts
    },
    readiness() {
      const counts = this.counts()
      return { counts, complete: counts.ready === this.jobs.length }
    },
    plan() {
      return { ok: true, step: 'plan', book: 'Key', revision: 1, plan_revision: 1, plan_hash: 'ph-1', terms: this.jobs.length, math: this.jobs.filter((j) => j.kind === 'math').length, concept: this.jobs.filter((j) => j.kind === 'concept').length, jobs: this.jobs.map((job) => ({ id: job.id, kind: job.kind })) }
    },
    jobsLine(command) {
      // The branch asks for one wave at a time (`-Jobs` without a prepared ledger, `-Tokens` with).
      const listed = (String(command || '').match(/-Tokens '([^']*)'/) || [])[1] || (String(command || '').match(/-Jobs '([^']*)'/) || [])[1] || ''
      const ids = listed ? listed.split(',').filter(Boolean).map((e) => (e.includes(':') ? e.slice(0, e.indexOf(':')) : e)) : []
      const jobs = ids.length ? this.jobs.filter((job) => ids.includes(job.id)) : this.jobs
      return { ok: true, step: 'jobs', plan_revision: 1, plan_hash: 'ph-1', units: [], jobs }
    },
    check() {
      return { ok: true, step: 'check', plan_revision: 1, plan_hash: 'ph-1', ...this.readiness(), terms: this.jobs.map((job, index) => ({ job: job.id, row: job.rows[0], term: job.terms[0], status: this.states()[index] })) }
    },
    reviewPlan() {
      const jobs = []
      const notices = []
      for (const job of this.jobs) {
        if (!this.written.has(job.id)) continue
        for (const row of this.reviewRows(job)) {
          const term = job.terms[job.rows.indexOf(row)] || job.terms[0]
          const approved = this.approved.has(`${job.id}#${row}`)
          if (!approved) jobs.push(REVIEW_JOB(this.out, job.id, row, term))
          notices.push({ job: job.id, row, term, status: approved ? 'approved' : 'planned', reason: '' })
        }
      }
      return { ok: true, step: 'review-plan', plan_revision: 1, plan_hash: 'ph-1', jobs, reviews: notices, ...this.readiness() }
    },
    fixPlan() {
      const jobs = this.written.has('t005') && !this.fixed ? [FIX_JOB(this.out, 't005', 5, '机会主义')] : []
      return { ok: true, step: 'fix-plan', plan_revision: 1, plan_hash: 'ph-1', jobs, planned: jobs.map(() => ({ job: 't005', status: 'planned' })), final: [], ...this.readiness() }
    },
    // S24 (#167): the branch publishes its ready rows after the waves. This fixture has no prepared
    // ledger (its plan line registers no units), so the compiler answers the publication and report
    // steps without a claim token.
    publication(step) {
      const counts = this.counts()
      const complete = counts.ready === this.jobs.length && this.written.size === this.jobs.length
      return { ok: true, step, plan_revision: 1, plan_hash: 'ph-1', rows: this.jobs.length, ready: this.jobs.length, committed: complete ? this.jobs.length : 0, ready_unpublished: complete ? 0 : this.jobs.length, refused: 0, pending: 0, conflicted: 0, staged: [], stop: complete ? null : { code: 'not_ready', message: 'later rows stay staged' }, complete, report: `${this.work}\\report.json` }
    },
    answer(step, command) {
      const line = { plan: () => this.plan(), jobs: () => this.jobsLine(command), check: () => this.check(), 'review-plan': () => this.reviewPlan(), 'fix-plan': () => this.fixPlan(), apply: () => this.publication('apply'), report: () => this.publication('report') }[step]
      if (!line) throw new Error(`stand-in: unknown compiler step ${step}`)
      return line.call(this)
    },
  }
}

// --- the stand-in runtime -----------------------------------------------------------------------
function runtime(opts = {}) {
  const books = new Map()
  const state = {
    calls: [], logs: [], phases: [], managers: [], sessions: new Map(), live: new Set(),
    deleted: new Set(), workspaces: [], peakLive: 0, peakLiveBook: 0, failedWriter: opts.failedWriter || null, empty: opts.empty === true, grouped: opts.grouped === true,
  }
  let seq = 0
  const bookOf = (out) => {
    if (!books.has(out)) books.set(out, book(out, { empty: state.empty, grouped: state.grouped }))
    return books.get(out)
  }
  const bookByWork = (work) => [...books.values()].find((b) => b.work === work) || bookOf(work.replace(/\\term-cards$/, ''))
  const liveIn = (work) => [...state.live].filter((id) => (state.sessions.get(id) || {}).work === work).length
  const parseJobs = (prompt) => {
    const open = prompt.indexOf('\n', prompt.indexOf('JOBS ('))
    const end = prompt.indexOf('\n\n1. Start', open)
    return JSON.parse(prompt.slice(open, end < 0 ? prompt.length : end))
  }
  const parseSessions = (prompt) => {
    const open = prompt.indexOf('\n', prompt.indexOf('SESSIONS ('))
    const end = prompt.indexOf('\n\nFor each', open)
    return JSON.parse(prompt.slice(open, end < 0 ? prompt.length : end))
  }
  const record = (session, job, work) => {
    state.sessions.set(session, { job: job.id, kind: job.kind, work })
    state.live.add(session)
    state.peakLiveBook = Math.max(state.peakLiveBook, liveIn(work))
    state.peakLive = Math.max(state.peakLive, state.live.size)
  }
  const remove = (session) => { state.live.delete(session); state.deleted.add(session) }
  const write = (file, text) => { fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, text, 'utf8') }

  async function relay(prompt) {
    const command = (prompt.match(/zt-digest term-cards -Step [\w-]+(?: -Staging '[^']*')?[^\n]*/) || [])[0]
    if (!command) throw new Error(`stand-in: no compiler command in the relay prompt: ${prompt.slice(0, 160)}`)
    const step = (command.match(/-Step ([\w-]+)/) || [])[1]
    const out = (command.match(/-Staging '([^']+)'/) || [])[1]
    state.calls.push({ line: command })
    return { ok: true, output: JSON.stringify(bookOf(out).answer(step, command)) }
  }

  async function manager(prompt) {
    if (/^Remove finished DeepSeek workspaces/.test(prompt)) return cleanup(prompt)
    if (/ZET Term Cards \(settle a wave\)/.test(prompt)) return settle(prompt)
    const jobs = parseJobs(prompt)
    const work = (prompt.match(/cwd "([^"]+)"/) || [])[1]
    const book = bookByWork(work)
    const results = []
    const reviewed = []
    for (const job of jobs) {
      if (job.kind === 'review') {
        // The manager starts every review in one message, so all of them are live beside the four
        // retained writers; each is deleted after its answer is collected.
        const session = `s-${++seq}`
        record(session, job, work)
        write(job.answer, '{"ok":true}')
        write(`${work}\\answers\\${session}.txt`, '{"ok":true}')
        book.approved.add(`${job.id.replace(/-r\d+-review$/, '')}#${job.row}`)
        reviewed.push(session)
        results.push({ id: job.id, session_id: session, status: 'done', answer: '{"ok":true}', registered: true, evidence: true, deleted: true })
        continue
      }
      if (job.kind === 'fix') {
        const session = [...state.sessions.entries()].find(([, s]) => s.job === job.continue && s.work === work)?.[0]
        if (!session) throw new Error(`stand-in: no writer session for the fix job ${job.id}`)
        write(job.output, '{"terms":[{"term":"剩余控制权"}]}')
        write(`${work}\\answers\\${session}.txt`, '{"terms":1,"fixed":true}')
        book.fixed = true
        results.push({ id: job.id, session_id: session, status: 'done', answer: '{"terms":1,"fixed":true}', registered: true, evidence: true, deleted: false })
        continue
      }
      const session = `s-${++seq}`
      record(session, job, work)
      if (job.id === state.failedWriter) {
        write(`${work}\\answers\\${session}.txt`, 'no answer: failed')
        results.push({ id: job.id, session_id: session, status: 'failed', answer: 'the provider failed', registered: true, evidence: true, deleted: false })
        continue
      }
      write(job.output, '{"terms":[{"term":"x"}]}')
      write(`${work}\\answers\\${session}.txt`, '{"terms":1,"math":1,"steps":3}')
      book.written.add(job.id)
      results.push({ id: job.id, session_id: session, status: 'done', answer: '{"terms":1,"math":1,"steps":3}', registered: true, evidence: true, deleted: false })
    }
    // A review conversation ends as soon as its answer is durably collected.
    for (const session of reviewed) remove(session)
    return { ok: true, jobs: results, deleted: reviewed.length, not_deleted: [] }
  }

  async function settle(prompt) {
    let deleted = 0
    const sessions = []
    for (const entry of parseSessions(prompt)) {
      if (state.live.has(entry.session_id)) { remove(entry.session_id); deleted++ }
      sessions.push({ id: entry.id, session_id: entry.session_id, deleted: true })
    }
    return { ok: true, deleted, not_deleted: [], sessions }
  }

  async function cleanup(prompt) {
    const folders = [...prompt.matchAll(/^- (.+)$/gm)].map((m) => m[1].trim())
    return { folders: folders.map((cwd) => {
      const leftover = [...state.live].filter((id) => (state.sessions.get(id) || {}).work === cwd)
      const removed = leftover.length === 0
      if (removed) state.workspaces.push(cwd)
      return { cwd, removed, leftover, running: 0, note: removed ? '' : 'has conversations' }
    }) }
  }

  return {
    agent: async (prompt, opts = {}) => {
      if (!/^Run this PowerShell command once/.test(prompt)) state.managers.push({ label: opts.label, phase: opts.phase, prompt })
      state.calls.push({ prompt, opts })
      return /^Run this PowerShell command once/.test(prompt) ? relay(prompt) : manager(prompt)
    },
    phase: (title) => state.phases.push(title),
    log: (message) => state.logs.push(message),
    parallel: async (fns) => Promise.all(fns.map((fn) => fn())),
    pipeline: async (items, ...stages) => Promise.all(items.map(async (item, index) => {
      let value = item
      for (const stage of stages) value = await stage(value, item, index)
      return value
    })),
    state, books, workOf,
    get live() { return state.live },
    get peakLive() { return state.peakLive },
    get peakLiveBook() { return state.peakLiveBook },
    get workspaces() { return state.workspaces },
    get managers() { return state.managers },
    get poolPrompts() { return state.calls.filter((c) => c.prompt && !/^Run this PowerShell command once/.test(c.prompt)).map((c) => c.prompt) },
    get calls() { return state.calls },
    get logs() { return state.logs },
  }
}

async function runSource(source, args, rt) {
  const fn = new AsyncFunction('args', 'agent', 'phase', 'log', 'parallel', 'pipeline', source)
  let report, thrown
  try { report = await fn(args, rt.agent, rt.phase, rt.log, rt.parallel, rt.pipeline) } catch (e) { thrown = e }
  const result = thrown ? { stage: 'kernel', error: thrown.message, thrown } : report.halted ? { stage: report.halted.stage, error: report.halted.error } : report.result
  return { result, report, thrown, rt }
}
const run = (build, args, rt) => runSource(build.source, args, rt)

// The shared pool's own accounting: every manager agent call is one dsmpSlot and at most eight are
// alive at once in the built script.
function pooled(rt) {
  rt.pool = { alive: 0, peak: 0 }
  const inner = rt.agent
  rt.agent = async (prompt, opts = {}) => {
    if (/^Run this PowerShell command once/.test(prompt)) return inner(prompt, opts)
    rt.pool.alive++
    rt.pool.peak = Math.max(rt.pool.peak, rt.pool.alive)
    try { return await inner(prompt, opts) } finally { rt.pool.alive-- }
  }
  return rt
}

let failed = 0
const check = (cond, message) => { console.log(`${cond ? 'ok' : 'FAIL'}: ${message}`); if (!cond) failed++ }
const kindOf = (prompt) => (prompt.match(/ZET Term Cards \(([^)]+)\)/) || [])[1] || ''
const parseJobs = (prompt) => {
  const open = prompt.indexOf('\n', prompt.indexOf('JOBS ('))
  const end = prompt.indexOf('\n\n1. Start', open)
  return JSON.parse(prompt.slice(open, end < 0 ? prompt.length : end))
}
const parseSessions = (prompt) => {
  const open = prompt.indexOf('\n', prompt.indexOf('SESSIONS ('))
  const end = prompt.indexOf('\n\nFor each', open)
  return JSON.parse(prompt.slice(open, end < 0 ? prompt.length : end))
}

// ==================================================================================================
// One book: two waves, two reviews, one fix, deletion and cleanup
// ==================================================================================================
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'zt-term-cards-'))
  const out = path.join(dir, 'Key')
  const rt = pooled(runtime())
  const { result, report, thrown } = await run(builds.one, { out, citationKey: 'Key' }, rt)
  check(!thrown && !report.halted && report.stages['term-cards'].status === 'done', `one book: the stage is done (${thrown ? thrown.message : report.halted ? report.halted.error : report.stages['term-cards'].status})`)
  check(result && result.status === 'done', `one book: the branch reports done (${result && result.status}${result && result.error ? ': ' + result.error : ''})`)
  check(result && result.jobs.total === 6 && result.jobs.done === 6, 'one book: all six writer jobs ran')
  check(result && result.waves.length === 2 && result.waves[0].jobs.length === 4 && result.waves[1].jobs.length === 2, 'waves: four writer jobs, then two')
  check(result && result.reviews.planned === 4 && result.fixes.planned === 1, `one book: four reviews and one fix (${result && result.reviews.planned}/${result && result.fixes.planned})`)
  check(result && result.reviews.approved === 4 && result.reviews.refused === 0, `one book: all four mathematical drafts are approved (${result && result.reviews.approved})`)
  check(result && result.readiness.complete === true, 'one book: the readiness result is complete')
  check(result && result.deepseek.deleted === 10, `one book: four reviews and six writers deleted (${result && result.deepseek.deleted})`)
  check(rt.live.size === 0, `one book: no conversation is left open (${[...rt.live].join(', ')})`)
  check(rt.peakLiveBook === 8, `one book: the four retained writers and four reviewers are eight live conversations (${rt.peakLiveBook})`)
  check(rt.workspaces.length === 1 && rt.workspaces[0] === workOf(out), 'one book: the workspace registration is removed')

  const managers = rt.calls.filter((c) => c.opts && c.opts.model === 'haiku' && !c.opts.agentType)
  const relays = rt.calls.filter((c) => c.opts && c.opts.agentType === 'zt-relay')
  check(managers.length > 0 && managers.every((c) => c.opts.effort === 'low'), `roles: every job manager is Haiku at low effort (${managers.length})`)
  check(relays.length > 0, `roles: every compiler command is a zt-relay relay (${relays.length})`)

  const firstWrite = rt.poolPrompts.find((p) => kindOf(p) === 'write') || ''
  check(/"model": "deepseek-official\/deepseek-flash"/.test(firstWrite) && /"effort": "high"/.test(firstWrite), 'jobs: the writer job carries the official model at high effort')
  check(/"skills": \[\n\s+"zettelkasten",\n\s+"asd-ste100-zh"/.test(firstWrite), 'jobs: the Chinese language skill travels in the job skills field')
  const waveJobs = rt.poolPrompts.filter((p) => kindOf(p) === 'write').map((p) => parseJobs(p).length)
  check(waveJobs.length === 2 && waveJobs[0] === 4 && waveJobs[1] === 2, `waves: the managers receive four then two writer jobs (${waveJobs.join(',')})`)
  const reviews = rt.poolPrompts.filter((p) => kindOf(p) === 'review').map((p) => parseJobs(p))
  check(reviews.length === 1 && reviews[0].length === 4 && reviews[0].map((j) => j.row).join(',') === '1,2,3,4' && reviews[0].every((j) => j.delete_after_collect === true), 'reviews: one wave of four fresh review jobs in term order, each deleted after its answer')
  const fixPrompt = rt.poolPrompts.find((p) => kindOf(p) === 'fix') || ''
  const fixes = parseJobs(fixPrompt)
  check(fixes.length === 1 && fixes[0].continue === 't005' && fixes[0].delete_after_collect === false, 'fix: the fix continues the writer job and keeps its conversation')
  check(/deepseek_prompt/.test(fixPrompt) && /t005 s-\d+/.test(fixPrompt), 'continuation: the fix is prompted in the writer\'s own conversation (deepseek_prompt and its session)')
  check(/fallback/.test(fixPrompt), 'continuation: the last writer prompt falls back to a fresh conversation')
  const settles = rt.poolPrompts.filter((p) => /settle a wave/.test(p)).map((p) => parseSessions(p).length)
  check(settles.length === 2 && settles[0] === 4 && settles[1] === 2, `deletion: each wave settles its own writers (${settles.join(',')})`)
  check(/wait_seconds 90/.test(firstWrite) && /\b240\b/.test(firstWrite) && /wait_seconds 0 for EVERY open session/.test(firstWrite), 'runtime policy: the prompts carry the protocol\'s 90 s poll, 240 min budget and nonblocking polls')
  check(/ONE more attempt/.test(firstWrite) && /timeout gets no retry/.test(firstWrite), 'runtime policy: a provider failure gets one retry, a timeout none, and the semantic fix is a separate step')
  const settlePrompt = rt.poolPrompts.find((p) => /settle a wave/.test(p)) || ''
  check(/Never call deepseek_archive/.test(firstWrite) && /Never archive a conversation/.test(settlePrompt), 'deletion: no conversation is ever archived')

  const kept = ['t001', 't003', 't006'].every((id) => fs.existsSync(path.join(workOf(out), 'out', `${id}.json`)))
  check(kept && fs.existsSync(path.join(workOf(out), 'review', 'answers')), 'cleanup: the staging outputs and review answers stay after the workspace registration is removed')
  fs.rmSync(dir, { recursive: true, force: true })
}

// ==================================================================================================
// The shared pool: ten concurrent branches never hold more than eight managers
// ==================================================================================================
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'zt-term-cards-pool-'))
  const names = Array.from({ length: 10 }, (_, i) => `out${String(i + 1).padStart(2, '0')}`)
  const bp = JSON.parse(fs.readFileSync(fixture, 'utf8'))
  bp.name = 'zt-term-cards-pool-fixture'
  bp.args = Object.fromEntries(names.map((name) => [name, { type: 'string', required: true, description: `staging folder ${name}` }]))
  bp.stages = names.map((name) => ({ id: `term-cards-${name}`, title: 'Term cards', module: 'zt-term-cards', params: { out: `{{args.${name}}}`, key: 'Key' } }))
  delete bp.result
  bp.modules = [path.join(here, '..', 'workflow', 'modules'), path.join(root, 'scripts', 'workflow-shared', 'modules')]
  const file = path.join(dir, 'pool.blueprint.json')
  fs.writeFileSync(file, JSON.stringify(bp, null, 2))
  const buildPool = build(file)
  const args = Object.fromEntries(names.map((name) => [name, path.join(dir, name)]))
  const rt = pooled(runtime())
  const { report, thrown } = await run(buildPool, args, rt)
  check(!thrown && !report.halted, `pool: ten branches run (${thrown ? thrown.message : report.halted ? report.halted.error : ''})`)
  check(names.every((name) => report.stages[`term-cards-${name}`].status === 'done'), 'pool: every branch completes')
  check(rt.pool.peak === 8, `pool: exactly eight managers run at once, the shared dsmpSlot cap (${rt.pool.peak})`)
  check(rt.peakLive <= 64, `pool: at most 64 conversations are open, the protocol's total (${rt.peakLive})`)
  check(rt.live.size === 0 && rt.workspaces.length === 10, `pool: every conversation is deleted and every workspace removed (${rt.live.size}/${rt.workspaces.length})`)
  fs.rmSync(dir, { recursive: true, force: true })
}

// ==================================================================================================
// The failure path: one failed writer job still settles its wave and removes the workspace
// ==================================================================================================
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'zt-term-cards-failure-'))
  const out = path.join(dir, 'Key')
  const rt = pooled(runtime({ failedWriter: 't002' }))
  const { result, report, thrown } = await run(builds.one, { out, citationKey: 'Key' }, rt)
  check(!thrown && !report.halted, `failure: the run does not throw (${thrown ? thrown.message : ''})`)
  check(result && result.status === 'failed' && /t002/.test(result.error || ''), `failure: the branch is failed and names the writer job (${result && result.error})`)
  check(rt.live.size === 0, `failure: the wave's conversations are settled (${[...rt.live].join(', ')})`)
  check(rt.workspaces.length === 1, 'failure: the workspace registration is still removed')
  check(fs.existsSync(path.join(workOf(out), 'out', 't001.json')), 'failure: the staged drafts stay on disk')
  fs.rmSync(dir, { recursive: true, force: true })
}

// ==================================================================================================
// A plan with no writer job is no Term branch: no manager runs and no success is claimed
// ==================================================================================================
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'zt-term-cards-empty-'))
  const out = path.join(dir, 'Key')
  const rt = pooled(runtime({ empty: true }))
  const { result, report, thrown } = await run(builds.one, { out, citationKey: 'Key' }, rt)
  check(!thrown && !report.halted, `empty book: the run does not throw (${thrown ? thrown.message : ''})`)
  check(result && result.status === 'failed' && /no writer jobs/.test(result.error || ''), `empty book: the branch is failed, never a successful zero-term book (${result && result.error})`)
  check(rt.managers.length === 0 && rt.live.size === 0 && rt.workspaces.length === 0, 'empty book: no manager starts, no conversation exists and no workspace is registered')
  fs.rmSync(dir, { recursive: true, force: true })
}

// ==================================================================================================
// A changed managed protocol is reported, never silently stretched
// ==================================================================================================
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'zt-term-cards-protocol-'))
  const out = path.join(dir, 'Key')
  const rt = pooled(runtime())
  const tampered = builds.one.source.replace('maxManagers: 8', 'maxManagers: 9')
  check(tampered !== builds.one.source, 'protocol: the fixture actually changes the inlined protocol')
  const { result, report, thrown } = await runSource(tampered, { out, citationKey: 'Key' }, rt)
  check(!thrown && !report.halted, `protocol: the run does not throw (${thrown ? thrown.message : ''})`)
  check(result && result.status === 'failed' && /managed protocol changed/.test(result.error || ''), `protocol: the branch reports the change instead of stretching it (${result && result.error})`)
  check(rt.managers.length === 0 && rt.live.size === 0, 'protocol: no manager starts before the changed protocol is reconciled')
  fs.rmSync(dir, { recursive: true, force: true })
}

// ==================================================================================================
// A grouped concept job's promoted terms are reviewed one after another, not side by side
// ==================================================================================================
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'zt-term-cards-grouped-'))
  const out = path.join(dir, 'Key')
  const rt = pooled(runtime({ grouped: true }))
  const { result, report, thrown } = await run(builds.one, { out, citationKey: 'Key' }, rt)
  check(!thrown && !report.halted && result && result.status === 'done', `grouped: the branch completes (${thrown ? thrown.message : result && result.error})`)
  const batches = rt.poolPrompts.filter((p) => kindOf(p) === 'review').map((p) => parseJobs(p))
  check(batches.length === 2 && batches[0].length === 4, `grouped: the four different writer jobs are reviewed together (${batches.map((b) => b.length).join('+')})`)
  check(batches[1] && batches[1].length === 1 && batches[1][0].id === 't003-r004-review', `grouped: the promoted job's second term waits for the next slot (${batches[1] && batches[1][0] && batches[1][0].id})`)
  check(result && result.reviews.planned === 5 && result.reviews.approved === 5, `grouped: five promoted terms are reviewed and approved (${result && result.reviews.planned}/${result && result.reviews.approved})`)
  check(rt.live.size === 0, `grouped: no conversation is left open (${[...rt.live].join(', ')})`)
  fs.rmSync(dir, { recursive: true, force: true })
}

console.log(`\n${failed ? `${failed} check(s) failed` : 'all checks passed'}`)
if (failed) process.exit(1)
