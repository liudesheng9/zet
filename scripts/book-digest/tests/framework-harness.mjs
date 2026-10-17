// Run: node scripts/book-digest/tests/framework-harness.mjs
// The fixture of the framework-built digests (docs/zt-digest-framework.md). It builds zt-econ-digest and
// zt-math-digest from their blueprints (scripts/book-digest/workflow) with the workflow-framework-protocol
// assembler, runs them with stand-ins for the Workflow runtime (every agent returns a canned answer that fits its
// label or command, as the removed workflow-harness.mjs and math-workflow-harness.mjs did), and runs every check of those two
// harnesses on them, plus the checks of the framework: one shared base, frozen structure, installed = built.
// The assembler: $WFP_ROOT, else ~/.claude/skills/workflow-framework-protocol.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..', '..', '..')
const workflowDir = path.join(here, '..', 'workflow')
const wfpRoot = process.env.WFP_ROOT || path.join(os.homedir(), '.claude', 'skills', 'workflow-framework-protocol')
const lib = await import(pathToFileURL(path.join(wfpRoot, 'tools', 'lib.mjs')).href)
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor

const builds = {}
for (const [which, name] of [['econ', 'zt-econ-digest'], ['math', 'zt-math-digest']]) {
  const l = lib.loadBlueprint(path.join(workflowDir, `${name}.blueprint.json`))
  builds[which] = { name, ...lib.build(l.bp, lib.loadModules(l.dirs)), bp: l.bp }
}
const sources = { econ: builds.econ.script.replace(/^export /m, ''), math: builds.math.script.replace(/^export /m, '') }

// --- the stand-in runtime: the canned answers (from the removed book and math workflow harnesses) -------------
let planJobs = 21
let applyPending = 0
let explainApplies = 0
// The scripts ported to zt-digest (ZD_TOOLS in zt-digest-lib.js): tool → script.
const ZT_DIGEST_TOOLS = { audit: 'digest-audit', card: 'card-compiler', model: 'model-compiler', 'model-context': 'model-context', 'list-models': 'list-models' }
// A tool command has a parameter after the tool name; the approval lines name tools without one.
const ZT_DIGEST_COMMAND = /zt-digest (?:audit|card|model-context|list-models|model) -[A-Za-z][^\n]*/
const canonical = (cmd) => cmd.replace(/^zt-digest (audit|card|model-context|list-models|model)\b/, (m, tool) => `pwsh -NoProfile -File "E:\\zet\\scripts\\book-digest\\${ZT_DIGEST_TOOLS[tool]}.ps1"`)
function runtime(kind, opts0 = {}) {
  const calls = []
  const managed = []
  const cleanups = []
  const logs = []
  const phases = []
  const pool = { alive: 0, peak: 0, prompts: [] }
  // The command in a prompt: a pwsh script, or a zt-digest tool. A tool's command is written in the form of
  // the script it replaced (`pwsh -NoProfile -File "<scripts>\<script>.ps1" ...`), so the checks below match
  // one form; `raw` keeps the command as the prompt gave it.
  const commandOf = (prompt) => {
    const text = String(prompt)
    const found = [text.match(/pwsh -NoProfile -File[^\n]*/), text.match(ZT_DIGEST_COMMAND)].filter(Boolean).sort((a, b) => a.index - b.index)[0]
    return found ? canonical(found[0]) : ''
  }
  const json = (o) => JSON.stringify(o)
  const job = (id) => ({ id, files: ['a', 'b'], prompt: 'p' })
  const answer = (prompt, opts) => {
    const label = opts.label || ''
    const cmd = commandOf(prompt)
    const raw = (String(prompt).match(ZT_DIGEST_COMMAND) || [''])[0]
    calls.push({ label, phase: opts.phase, model: opts.model, cmd, raw, prompt: String(prompt) })
    if (label === 'choose DeepSeek source') return { source: 'official', errors: '' }
    if (label === 'prepare book') return { ok: true, groups: 2, usage_before: { input_cache_miss: 0, cached_read: 0, output: 0, cost: 0, calls: 0 } }
    if (label === 'math: index') return { ok: true, output: json({ ok: true, units: 5, forms: { theorem: 1, lemma: 1 }, examples: 1, exercises: 1 }) }
    if (label === 'math: docs') return { ok: true, output: json({ ok: true, files: [] }) }
    if (label === 'math: skeleton') return { ok: true, output: json({ ok: true, units: 5, created: 5, theories: 2 }) }
    if (label === 'math: check') return { ok: true, output: json({ ok: true, units: 5, theories: 2, faults: 0, by_rule: {} }) }
    if (/^(digest|link) manager/.test(label)) {
      const groups = [...String(prompt).matchAll(/Group (\d+):/g)].map(m => Number(m[1]))
      const down = /^digest/.test(label) && opts0.digestFails
      return { reports: groups.map(g => ({ group: g, status: down ? 'failed' : 'done', cards: down ? 0 : 3, models: kind === 'math' ? 1 : 0, events: 0, links: 2, topics: [], errors: '无', deleted: true })) }
    }
    if (/^find models/.test(label)) return kind === 'math'
      ? { ok: true, models: [{ address: 'Key/2|e', title: '理论', form: 'theory', level: null, parts: 0 }, { address: 'Key/2|c', title: '介值定理', form: 'theorem', level: 'formal', parts: 0 }] }
      : { ok: true, models: [] }
    if (/^plan: /.test(label)) return { plans: [{ model: 'Key/2|c', status: 'planned', level: 'formal', parts: 4, derivations: 1 }] }
    if (label === 'prepare link pass') return { ok: true, pending: [1, 2], owned_total: 6, unowned: 0 }
    if (label === 'cleanup DeepSeek') return { late_deleted: 0, workspace_removed: true, files_removed: 0 }
    if (label === 'cleanup model managers') return { late_deleted: 0, folders: 1 }
    if (label === 'show figures') return { ok: true, output: json({ shown: 0, cards: 0, figures: 0, labelled: 0, literature: false }) }
    if (label === 'audit cards') return { ok: true, output: json({ hubs_missing: 0, broken_links: 0, books: [{ key: 'Key', cards: 9, topic_trees: 1, style_faults: 0, orphans: 0, no_source: 0, unkinded: 0, models: 0, not_self_sufficient: 0, figures_missing: 0 }] }) }
    if (label === 'organize: survey') return { ok: true, output: json({ survey: 'x', topics: 1, pairs: 0 }) }
    if (label === 'organize: decisions') return { thesis: '论点', merges: 0, moves: 0, notes: 1, report: '无' }
    if (label === 'organize: apply') return { ok: true, output: json({ merges: 0, moves: 0, refused: 0, renamed: 0 }) }
    if (label === 'hub manager: survey and limits') return { ok: true, survey: '{}', limits: { topic_min: 8, line_min: 10, max_lines: 20, max_groups: 7, min_reason: 8, neighbour_links: 3, field_min: 3, field_max: 8, per_job: 4 }, reasons: '' }
    if (String(prompt).startsWith('You drive DeepSeek Harness')) {
      if (/Run every job below/.test(prompt)) {
        const given = JSON.parse(String(prompt).split('\n').find(l => l.startsWith('[{')) || '[]')
        managed.push({ label, jobs: given.map(j => j.id), given })
        return { ok: true, output: '', results: given.map(j => ({ id: j.id, session_id: `s-${j.id}`, status: opts0.failJudge && /^bridge/.test(label) ? 'failed' : 'done', answer: '{}' })) }
      }
      if (/^Clean up:/m.test(prompt)) { cleanups.push(label); return { ok: true, results: [], deleted: 2 } }
      if (opts0.failStep && opts0.failStep.test(cmd)) return { ok: false, error: 'stand-in crash', output: '', results: [] }
      let output = json({ jobs: [] })
      if (/bridge-compiler\.ps1/.test(cmd)) return { ok: true, output: json({ok:true,step:/-Step plan/.test(cmd)?'plan':'report',status:'done',plan_id:'p-fixture',report_id:'r-fixture',counts:{selected:1,new:1,earlier:0},jobs: /-Step plan/.test(cmd)?[job('br-001')]:[]}), results: [] }
      const compact = / -Compact/.test(cmd)
      const head = { dir: 'D:\\x\\Key\\cards', lead: { fix: 'D:\\x\\Key\\cards\\card.md', orphans: 'D:\\x\\Key\\cards\\orphan.md' }, prompts: { fix: 'pf', orphans: 'po', follow: 'pw' } }
      if (/model-compiler\.ps1" -Step brief/.test(cmd)) output = json({ jobs: [job('brief')], skipped: [] })
      else if (/model-compiler\.ps1" -Step report/.test(cmd)) output = json({ models: [{ model: 'Key/2|c', status: 'done', level: 'formal', parts: [], faults: {} }] })
      else if (kind === 'book' && compact && /card-compiler\.ps1" -Step plan/.test(cmd)) output = json({ picked: 40, with_faults: 30, faults: 90, review: 1, orphans: 1, ...head, jobs: [...Array.from({ length: planJobs - 1 }, (_, i) => ({ id: `fix-${String(i + 1).padStart(2, '0')}`, kind: 'fix', file: `fix-${String(i + 1).padStart(2, '0')}.md` })), { id: 'orphans-99', kind: 'orphans', file: 'orphans-99.md' }] })
      else if (kind === 'book' && compact && /card-compiler\.ps1" -Step check/.test(cmd)) output = json({ faults: 1, orphans: 0, ...head, jobs: [{ id: 'fix-01-r2', kind: 'follow', file: 'fix-01-r2.md', continue: 'fix-01' }] })
      else if (/card-compiler\.ps1" -Step plan/.test(cmd)) output = json({ picked: 4, with_faults: 1, faults: 1, review: 0, orphans: 0, jobs: [job('fix-01')] })
      else if (/card-compiler\.ps1" -Step apply/.test(cmd)) { output = json({ applied_jobs: 1, pending_jobs: applyPending, counts: { fix: 2 } }); if (applyPending > 0) applyPending-- }
      else if (/card-compiler\.ps1" -Step check/.test(cmd)) output = json({ jobs: [], faults: 0, orphans: 0 })
      else if (/card-compiler\.ps1" -Step report/.test(cmd)) output = json({ cards: 2, faults_before: 3, faults_after: 0, verdicts: { fix: 2 }, splits: 0, merges: 0, refused: 0, orphan_links: 0, orphans_after: 0 })
      else if (/book-terms\.ps1" -Step plan/.test(cmd)) output = json({ bold: 2, definitions: 1, jobs: [job('terms')] })
      else if (/book-terms\.ps1" -Step apply/.test(cmd)) output = json({ ok: true, terms: 2, with_avoid: 1, card: 'Key/1|a' })
      else if (/book-skeleton\.ps1" -Step plan/.test(cmd)) output = json({ groups: 2, jobs: [job('skeleton')] })
      else if (/book-skeleton\.ps1" -Step apply/.test(cmd)) output = json({ ok: true, thesis: '论点', argument: 2, models: 0, events: 0, topics: 1, written: { structure: 'Key/1|b' } })
      else if (/hub-compiler\.ps1" -Step plan/.test(cmd)) output = json({ tree_count: 1, jobs: [job('h')] })
      else if (/hub-compiler\.ps1" -Step check/.test(cmd)) output = json({ hubs: 1, faults: 0, jobs: [] })
      else if (/hub-compiler\.ps1" -Step report/.test(cmd)) output = json({ counts: {}, faults: {}, mirror: 0, missing_count: 0 })
      else if (/hub-compiler\.ps1" -Step apply/.test(cmd)) output = json({ ok: true, hubs: 1 })
      else if (/model-compiler\.ps1" -Step explain-apply /.test(cmd)) {
        explainApplies++
        output = explainApplies === 1
          ? json({ applied: 2, refused: [], blocked: [], jobs: [{ id: 'm:x01+2', continue: 'm:x01', files: ['follow.md'], fallback: ['x.md', 'follow.md'], prompt: 'p' }] })
          : json({ applied: 1, refused: [], blocked: [], jobs: [] })
      }
      else if (/model-compiler\.ps1" -Step explain-report /.test(cmd)) output = json({ models: [{ model: 'Key/3|a', derivations: 3, explained: 3, missing: [], refused: [], blocked: [] }] })
      else if (/model-compiler\.ps1" -Step explain /.test(cmd)) output = json({ parts: 3, jobs: [{ id: 'm:x01', files: ['a', 'b', 'c'], prompt: 'p' }] })
      else if (/math-compiler\.ps1" -Step notation /.test(cmd + ' ')) output = json({ candidates: 3, jobs: [job('notation')] })
      else if (/math-compiler\.ps1" -Step notation-apply/.test(cmd)) output = json({ ok: true, symbols: 3, card: 'Key/1|c' })
      else if (/math-compiler\.ps1" -Step boundary /.test(cmd + ' ')) output = json({ units: 2, jobs: [job('b-01')] })
      else if (/math-compiler\.ps1" -Step boundary-apply/.test(cmd)) output = json({ created: 3, skipped_supplied: 0, jobs: [job('v-b-01')] })
      else if (/math-compiler\.ps1" -Step boundary-settle/.test(cmd)) output = json({ kept: 1, deleted: 0, parts: [] })
      else if (/math-compiler\.ps1" -Step proof /.test(cmd + ' ')) output = json({ units: 2, jobs: [job('p-01')] })
      else if (/math-compiler\.ps1" -Step proof-apply/.test(cmd)) output = json({ ok: 3, fixed: 1, refused: 0, parts: [] })
      else if (/math-compiler\.ps1" -Step theory /.test(cmd + ' ')) output = json({ theories: 2, jobs: [job('t-01')] })
      else if (/math-compiler\.ps1" -Step theory-apply/.test(cmd)) output = json({ ok: true, theories: 2, answered: 2 })
      else if (/event-compiler\.ps1" -Step plan/.test(cmd)) output = json({ candidates: 2, groups: 2, jobs: [job('ev-01')] })
      else if (/event-compiler\.ps1" -Step apply/.test(cmd)) output = json({ ok: true, applied_jobs: 1, created: 1, merged: 0, linked: 1, refused: [], pending_jobs: 0 })
      else if (/event-compiler\.ps1" -Step report/.test(cmd)) output = json({ jobs: 1, created: 1, merged: 0, linked: 1, refused: 0, errors: [] })
      else if (/finding-compiler\.ps1" -Step plan/.test(cmd)) output = json({ candidates: 3, groups: 2, jobs: [job('fd-01')] })
      else if (/finding-compiler\.ps1" -Step apply/.test(cmd)) output = json({ ok: true, applied_jobs: 1, created: 2, linked: 2, refused: [], pending_jobs: 0 })
      else if (/finding-compiler\.ps1" -Step report/.test(cmd)) output = json({ jobs: 1, created: 2, linked: 2, refused: 0, errors: [] })
      return { ok: true, output, results: [], deleted: 0 }
    }
    return {}
  }
  return {
    calls, logs, phases, managed, cleanups, pool,
    agent: async (prompt, opts = {}) => {
      const manager = /^You manage \d+ DeepSeek/.test(prompt) || /Run every job below/.test(prompt)
      if (!manager) return answer(prompt, opts)
      pool.prompts.push(String(prompt))
      pool.alive++
      pool.peak = Math.max(pool.peak, pool.alive)
      await new Promise(resolve => setTimeout(resolve, 3))
      try { return answer(prompt, opts) } finally { pool.alive-- }
    },
    phase: (p) => phases.push(p),
    log: (m) => logs.push(m),
    parallel: async (fns) => Promise.all(fns.map(fn => fn())),
    pipeline: async (items, ...stages) => Promise.all(items.map(async (item, i) => {
      let value = item
      for (const stage of stages) value = await stage(value, item, i)
      return value
    })),
  }
}
// The kernel returns a report; result is the workflow's result, or {stage, error} where the run halted.
async function run(which, args, opts0, kind) {
  const rt = runtime(kind || (which === 'math' ? 'math' : 'book'), opts0)
  const fn = new AsyncFunction('args', 'agent', 'phase', 'log', 'parallel', 'pipeline', sources[which])
  let report, thrown
  try { report = await fn(args, rt.agent, rt.phase, rt.log, rt.parallel, rt.pipeline) } catch (e) { thrown = e }
  const result = thrown ? { stage: 'kernel', error: thrown.message } : report.halted ? { stage: report.halted.stage, error: report.halted.error } : report.result
  return { result, report, thrown, ...rt }
}

let failed = 0
const check = (cond, message) => { console.log(`${cond ? 'ok' : 'FAIL'}: ${message}`); if (!cond) failed++ }

// ================================================================================================================
// The framework: one base, two variants, frozen
// ================================================================================================================
{
  const base = lib.loadBlueprint(path.join(workflowDir, 'zt-digest-base.blueprint.json'))
  const e = builds.econ.embedded, m = builds.math.embedded
  const baseIds = base.bp.stages.map(s => s.id)
  const pick = (bp, id) => bp.stages.find(s => s.id === id)
  const same = baseIds.filter(id => id !== 'report').every(id => JSON.stringify(pick(e, id)) === JSON.stringify(pick(m, id).after ? pick(m, id) : pick(m, id)) || JSON.stringify({ ...pick(e, id), after: null }) === JSON.stringify({ ...pick(m, id), after: null }))
  check(baseIds.every(id => pick(e, id) && pick(m, id)) && same, `base: both variants hold every base stage with the same module and params (${baseIds.length} stages)`)
  const own = (bp) => bp.stages.map(s => s.id).filter(id => !baseIds.includes(id)).join(',')
  check(own(e) === 'models,explain,events,findings', `base: the economics set adds the model layer, explain, the event layer, and the empirical-results layer (${own(e)})`)
  check(Object.keys(e.installed.modules).includes('zt-finding-layer') && !Object.keys(m.installed.modules).includes('zt-finding-layer'), 'modules: the empirical-results layer is installed only in the economics workflow')
  check(own(m) === 'math-skeleton,models,explain,math-boundary,math-proof,math-theory,math-check', `base: the mathematics set adds the model layer, explain, and its five math stages (${own(m)})`)
  check(!baseIds.some(id => /^(models|explain|events)$/.test(id)), 'base: no model, explain, or event layer in the shared base')
  check(Object.keys(e.installed.modules).includes('zt-event-layer') && !Object.keys(m.installed.modules).includes('zt-event-layer'), 'modules: the event layer is installed only in the economics workflow')
  check(['zt-model-layer', 'zt-model-lib'].every(id => e.installed.modules[id] && m.installed.modules[id]), 'modules: each book type installs its own model layer')
  check(e.consts.profile.noEventCandidates === true && m.consts.profile.noEventCandidates === true && /dsh-digest-base\.zh\.md,dsh-model-cards\.zh\.md/.test(e.consts.profile.instructions), 'profile: the digest prompt is the base prompt plus the model-card supplement, without event candidates')
  check(e.consts.profile.digestPrefix === 'shared-prefix.md' && m.consts.profile.digestPrefix === 'math-prefix.md', 'base: each variant sets its own profile')
  check(Object.keys(m.installed.modules).includes('zt-math-lib') && !Object.keys(e.installed.modules).some(id => /^zt-math/.test(id)), 'modules: the math set is installed only in the math workflow')
  for (const which of ['econ', 'math']) {
    const installed = path.join(root, '.claude', 'workflows', `${builds[which].name}.js`)
    const disk = fs.existsSync(installed) ? fs.readFileSync(installed, 'utf8') : ''
    check(disk === builds[which].script, `install: .claude/workflows/${builds[which].name}.js is the current build of its blueprint${disk ? '' : ' (not installed)'}`)
  }
  const tampered = sources.econ.replace('"perManager"', '"perManagerX"')
  const rt = runtime('book')
  let err = null
  try { await new AsyncFunction('args', 'agent', 'phase', 'log', 'parallel', 'pipeline', tampered)({ out: 'D:\\x\\Key', mode: 'audit' }, rt.agent, rt.phase, rt.log, rt.parallel, rt.pipeline) } catch (x) { err = x }
  check(err && /changed after it was installed/.test(err.message), 'frozen: an edited blueprint inside the script refuses to run')
  const undeclared = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'cards', architectEffort: 'high' })
  check(undeclared.thrown && /args\.architectEffort is not declared/.test(undeclared.thrown.message), 'frozen: an undeclared arg (architectEffort: the architect role is fixed) is refused')
  const badMode = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'math' })
  check(badMode.thrown && /mode "math" is not declared/.test(badMode.thrown.message), 'frozen: the econ workflow has no math mode')
  const until = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', source: 'S.md', bibtex: '@book{Key}', bookTitle: '书', hubs: false, figures: false, until: 'digest' })
  check(until.report && until.report.stoppedAt === 'digest' && !until.calls.some(c => /^link manager|prepare link pass/.test(c.label)), 'checkpoint: until "digest" stops before the link pass')
  const noKey = await run('econ', { out: 'D:\\x\\Key', mode: 'cards' })
  check(noKey.result.stage === 'args' && /citationKey/.test(noKey.result.error), 'args: mode cards without citationKey halts at the args stage')
}

// ================================================================================================================
// zt-econ-digest: every check of the removed workflow-harness.mjs
// ================================================================================================================
const base = { out: 'D:\\x\\Key', citationKey: 'Key', source: 'S.md', bibtex: '@book{Key}', bookTitle: '书', hubs: false }
{
  const { result, calls, phases, logs } = await run('econ', { ...base, mode: 'full', figures: false })
  const labels = calls.map(c => c.label)
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  check(result && result.mode === 'full', 'econ full: runs to its end')
  const ported = /(card-compiler|model-compiler|digest-audit|list-models|model-context)\.ps1/
  check(cmds.some(c => ported.test(c)) && calls.filter(c => ported.test(c.cmd)).every(c => /^zt-digest (card|model|audit|list-models|model-context) -/.test(c.raw)),
    'econ full: the ported scripts run as zt-digest tools, never through pwsh')
  check(calls.filter(c => /^(digest|link) manager/.test(c.label)).every(c => /zt-digest call/.test(c.prompt)), 'econ full: digest and link managers approve the zt-digest helper')
  check(labels.indexOf('terms: plan') > labels.indexOf('prepare book') && labels.indexOf('terms: plan') < labels.findIndex(l => /^digest manager/.test(l)), 'econ full: the term table runs after Prepare and before the digest')
  check(cmds.some(c => /card-compiler\.ps1" -Step plan .* -Book Key -Staging "D:\\x\\Key".* -Groups "1,2" -Sample 0\.1 -NoOrphans/.test(c)), 'econ full: a card cycle per digest block, with -Groups, the sample, and no orphans')
  check(cmds.some(c => /-Step plan -Work "[^"]*cards-link"/.test(c)) && !cmds.some(c => /cards-link".*-Rules/.test(c)), 'econ full: the link check runs with the default hard rules')
  check(cmds.filter(c => /-Rules/.test(c)).every(c => (c.match(/-Rules/g) || []).length === 1), 'econ full: no command passes -Rules twice')
  check(labels.filter(l => /^link manager/.test(l)).length === 1 && labels.includes('link manager 1: groups 1-2'), 'econ full: one link manager runs both groups')
  check(labels.includes('organize: survey') && labels.includes('organize: decisions') && labels.includes('organize: apply'), 'econ full: Organize runs the survey, the decisions, and the apply')
  check(calls.find(c => c.label === 'organize: decisions').model === 'opus', 'econ full: Opus makes the decisions')
  check(cmds.some(c => /cards-notes"/.test(c)) && cmds.some(c => /-Notes "D:\\x\\Key\\organize\\decisions\.json" -NoOrphans/.test(c)) && !cmds.some(c => /cards-notes".*-Rules/.test(c)), 'econ full: the notes go to a card cycle with the default hard rules')
  check(cmds.some(c => /book-skeleton\.ps1" -Step apply .* -Decisions "D:\\x\\Key\\organize\\decisions\.json" -Apply/.test(c)), 'econ full: the skeleton applies with the decisions')
  check(labels[labels.length - 1] === 'audit cards', 'econ full: the audit runs last')
  check(result.terms && result.terms.ok && result.cards && result.cards.blocks.length === 1 && result.cards.link && result.organize && result.organize.skeleton.ok && result.audit.status === 'done', 'econ full: the result holds terms, cards, organize, and audit')
  check(phases.includes('Cards') || calls.some(c => c.phase === 'Cards'), 'econ full: the card cycle runs in the Cards phase')
  check(/shared-prefix\.md/.test(calls.find(c => /^digest manager/.test(c.label)).prompt) && /link-prefix\.md/.test(calls.find(c => /^link manager/.test(c.label)).prompt), 'econ full: the digest and link runs read the economics profile\'s prefixes')
  check(cmds.some(c => /prepare-book-digest\.ps1"? .* -Instructions 'dsh-digest-base\.zh\.md,dsh-model-cards\.zh\.md' -NoEventCandidates -Apply/.test(c)), 'econ full: Prepare builds the prompt from the base and the model-card supplement, with no event candidates')
  const lastDigest = labels.map((l, i) => (/^digest manager/.test(l) ? i : -1)).reduce((a, b) => Math.max(a, b), -1)
  check(labels.indexOf('find models: whole book') > lastDigest && labels.indexOf('events: write') > lastDigest && labels.indexOf('events: report') < labels.indexOf('prepare link pass'), 'econ full: the model layer and the event layer run after the digest and before the link pass')
  check(cmds.some(c => /event-compiler\.ps1" -Step apply -Staging "D:\\x\\Key" -Apply/.test(c)) && result.events && result.events.status === 'done' && result.events.applied.created === 1, 'econ full: the event layer applies and reports its Event Cards')
  check(!logs.some(l => /failed(?! none)/.test(l)), `econ full: no stage logs a failure${logs.filter(l => /failed(?! none)/.test(l)).map(l => ` (${l})`).join('')}`)
}
{
  const { calls } = await run('econ', { ...base, mode: 'full', figures: false, organize: 'legacy', terms: false, cardChecks: false, audit: false })
  const labels = calls.map(c => c.label)
  check(labels.includes('organize card structure') && !labels.includes('organize: survey'), 'econ legacy: one Opus job, no survey')
  check(!labels.some(l => /^terms:/.test(l)) && !calls.some(c => /card-compiler/.test(c.cmd)), 'econ legacy: terms: false and cardChecks: false skip their stages')
}
{
  const { result, calls, managed, cleanups } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'cards', cardMax: 5 })
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  check(result && result.mode === 'cards' && result.cards.status === 'done', 'econ cards: runs to its end')
  check(cmds.some(c => /card-compiler\.ps1" -Step plan -Work "D:\\x\\Key\\cards" -Compact -Book Key -Staging "D:\\x\\Key" -MaxCards 5/.test(c)) && !cmds.some(c => /-NoOrphans/.test(c)), 'econ cards: plans the whole book with orphans and the pilot cap')
  check(!calls.some(c => /^(digest|link) manager|prepare book|organize/.test(c.label)), 'econ cards: runs no other stage')
  const planManagers = managed.filter(m => /cards whole book: plan/.test(m.label))
  const planJobsSeen = planManagers.flatMap(m => m.jobs)
  check(planManagers.length === 3, `econ cards: 21 jobs go to 3 Haiku managers, one per 8 jobs (${planManagers.length})`)
  check(Math.max(...planManagers.map(m => m.jobs.length)) <= 8, 'econ cards: no manager holds more than 8 jobs')
  check(planJobsSeen.length === 21 && new Set(planJobsSeen).size === 21, 'econ cards: every job goes to exactly one manager')
  check(calls.filter(c => /cards whole book: plan/.test(c.label)).every(c => c.model === 'haiku'), 'econ cards: the step agent and the managers are Haiku')
  check(cleanups.some(l => /cards whole book: report: clean up/.test(l)), 'econ cards: one cleanup agent ends the conversations after the report')
  check(result.cards.plan && result.cards.plan.jobs.length === 21 && result.cards.plan.jobs.every(j => Array.isArray(j.files)), 'econ cards: the plan reaches the stage intact, its jobs expanded')
  const given = planManagers.flatMap(m => m.given)
  const fix = given.find(j => j.id === 'fix-01')
  const orphan = given.find(j => j.id === 'orphans-99')
  check(fix && fix.files[0] === 'D:\\x\\Key\\cards\\card.md' && fix.files[1] === 'D:\\x\\Key\\cards\\fix-01.md' && fix.prompt === 'pf', 'econ cards: a compact fix job expands to its instruction file, job file, and prompt')
  check(orphan && orphan.files[0] === 'D:\\x\\Key\\cards\\orphan.md' && orphan.prompt === 'po', 'econ cards: a compact orphan job takes the orphan instructions')
  const follow = managed.filter(m => /check 1/.test(m.label)).flatMap(m => m.given).find(j => j.id === 'fix-01-r2')
  check(follow && follow.continue === 'fix-01' && follow.files.length === 1 && follow.fallback[0] === 'D:\\x\\Key\\cards\\card.md' && follow.prompt === 'pw', 'econ cards: a compact follow-up continues its job, with the instructions as fallback')
  check(calls.filter(c => /card-compiler\.ps1/.test(c.cmd)).every(c => / -Compact/.test(c.cmd)), 'econ cards: every card-compiler step gets -Compact')
}
for (const [n, want] of [[5, 1], [8, 1], [16, 2], [17, 3], [64, 8], [70, 9], [200, 25]]) {
  planJobs = n
  const { managed, pool } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'cards' })
  const plan = managed.filter(m => /cards whole book: plan/.test(m.label))
  const all = plan.flatMap(m => m.jobs)
  check(plan.length === want && all.length === n && new Set(all).size === n, `econ managers: ${n} jobs go to ${want} managers (${plan.length}), each job once`)
  check(Math.max(...plan.map(m => m.jobs.length)) <= 8 && pool.peak <= 8, `econ managers: ${n} jobs: at most 8 jobs per manager, at most 8 managers at once (peak ${pool.peak})`)
}
planJobs = 21
{
  const { pool } = await run('econ', { ...base, mode: 'full', figures: false, perManager: 1 })
  check(pool.prompts.length > 0 && pool.prompts.every(p => /deepseek_delete/.test(p) && !/deepseek_archive/.test(p)), `econ protocol: every DeepSeek manager deletes, none archives (${pool.prompts.length} managers)`)
  check(pool.peak <= 8, `econ protocol: at most 8 DeepSeek managers at once in a full run (peak ${pool.peak})`)
}
{
  applyPending = 2
  const { calls } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'cards' })
  const applies = calls.filter(c => /card-compiler\.ps1" -Step apply/.test(c.cmd))
  check(applies.filter(c => /: apply 1/.test(c.label)).length === 3 && applies.every(c => / -MaxJobs 30/.test(c.cmd)), `econ apply: 2 pending jobs take 3 calls of -MaxJobs 30 (${applies.map(c => c.label).join(', ')})`)
  applyPending = 0
}
{
  explainApplies = 0
  const { result, calls, managed, cleanups } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'explain', only: ['Key/3|a'], explainMax: 5 })
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  const steps = cmds.map(c => (c.match(/-Step (\S+)/) || [])[1]).filter(Boolean)
  check(result && result.mode === 'explain' && result.explain && result.explain.status === 'done', 'econ explain: runs to its end')
  check(steps.join(',') === 'explain,explain-apply,explain-apply,explain-report', `econ explain: plan, apply, apply, report (${steps.join(',')})`)
  check(cmds.some(c => /-Step explain -Work "D:\\x\\Key\\explain" .*-MaxExplainSentences 16 -MaxExplainChars 600 -Models "Key\/3\|a" -MaxParts 5/.test(c)), 'econ explain: the plan gets the explanation budget, the Model Cards, and the pilot cap')
  const follow = managed.find(m => /explain: apply 1/.test(m.label))
  check(follow && follow.given.length === 1 && follow.given[0].continue === 'm:x01', 'econ explain: the refused part goes back to the same conversation')
  check(cleanups.some(l => /explain: report: clean up/.test(l)), 'econ explain: the report step deletes the conversations')
  check(!calls.some(c => /^(digest|link) manager|^plan: |prepare book/.test(c.label)), 'econ explain: no other stage and no architect runs')
}
{
  const { result, calls } = await run('econ', { out: 'D:\\x\\audit', mode: 'audit' })
  check(result && result.mode === 'audit' && result.audit.status === 'done', 'econ audit: runs to its end without a citation key')
  check(calls.length === 1 && calls[0].label === 'audit cards', 'econ audit: one call, no source probe')
}
{
  const { calls } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'link', groups: 2, hubs: false })
  check(calls.some(c => /cards-link"/.test(c.cmd)), 'econ link: the link check runs after the link pass')
  check(calls.some(c => /cards-link".*-Rules "L1,L2"/.test(c.cmd)), 'econ link: on a book without card cycles the link check keeps rules L1 and L2')
}
{
  const { result, calls } = await run('econ', { ...base, mode: 'full', figures: false }, { digestFails: true })
  const labels = calls.map(c => c.label)
  check(result && result.stage === 'digest' && /every digest group failed/.test(result.error || ''), `econ full: a digest with no group done stops with an error (${JSON.stringify(result && { stage: result.stage, error: result.error })})`)
  check(!labels.some(l => /^link manager|^organize|^hub|audit cards/.test(l)), 'econ full: no link, Organize, hub, or audit stage runs after it')
}
{
  const { result } = await run('econ', { ...base, mode: 'full', figures: false, hubs: true }, { failStep: /(hub|card)-compiler\.ps1" -Step apply/ })
  check(result && result.hubs && result.hubs.status === 'failed' && /apply/.test(result.hubs.error || ''), 'econ full: a hub apply crash fails the hub stage')
  check(result && result.cards && result.cards.blocks.every(b => b.status === 'failed'), 'econ full: a card apply crash fails its card cycle')
}

{
  const { result, calls } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'models' })
  const labels = calls.map(c => c.label)
  check(result && result.mode === 'models' && Array.isArray(result.models) && labels.includes('find models: whole book') && !labels.some(l => /^(digest|link) manager|^events:/.test(l)), 'econ models: only the model layer on an existing digest')
}
{
  const { result, calls } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'events' })
  const labels = calls.map(c => c.label)
  check(result && result.mode === 'events' && result.events && result.events.status === 'done' && !labels.some(l => /^(digest|link) manager|^find models/.test(l)), 'econ events: only the event layer on an existing digest')
}
{
  const { calls } = await run('econ', { ...base, mode: 'full', figures: false, events: false, models: false })
  check(!calls.some(c => /^(events:|find models)/.test(c.label)) && calls.some(c => /^findings:/.test(c.label)), 'econ full: events false and models false skip their layers (findings still runs)')
}
{
  const { result, calls } = await run('econ', { ...base, mode: 'full', figures: false })
  const labels = calls.map(c => c.label)
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  const lastDigest = labels.map((l, i) => (/^digest manager/.test(l) ? i : -1)).reduce((a, b) => Math.max(a, b), -1)
  check(labels.indexOf('findings: write') > lastDigest && labels.indexOf('findings: report') < labels.indexOf('prepare link pass'), 'econ full: the empirical-results layer runs after the digest and before the link pass')
  check(cmds.some(c => /finding-compiler\.ps1" -Step apply -Staging "D:\\x\\Key" -Apply/.test(c)) && result.findings && result.findings.status === 'done' && result.findings.applied.created === 2, 'econ full: the empirical-results layer applies and reports its Finding Cards')
  check(calls.filter(c => /^findings:/.test(c.label)).every(c => c.model === 'haiku'), 'econ full: Haiku relays every finding step and manager')
}
{
  const { result, calls } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'findings' })
  const labels = calls.map(c => c.label)
  check(result && result.mode === 'findings' && result.findings && result.findings.status === 'done' && !labels.some(l => /^(digest|link) manager|^find models|^events:/.test(l)), 'econ findings: only the empirical-results layer on an existing digest')
}
{
  const { calls } = await run('econ', { ...base, mode: 'full', figures: false, findings: false })
  check(!calls.some(c => /^findings:/.test(c.label)), 'econ full: findings false skips the empirical-results layer')
}

// ================================================================================================================
// zt-math-digest: every check of the removed math-workflow-harness.mjs
// ================================================================================================================
const mbase = { ...base, hubs: true, figures: false }
{
  const { result, calls, logs, cleanups, pool, managed } = await run('math', { ...mbase, mode: 'full' })
  const labels = calls.map(c => c.label)
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  const at = (pattern) => labels.findIndex(l => pattern.test(l))
  check(result && result.mode === 'full' && result.math, 'math full: runs to its end and reports the math stages')
  check(at(/^prepare book$/) < at(/^math: index$/) && at(/^math: index$/) < at(/^math: docs$/) && at(/^math: docs$/) < at(/^math: skeleton$/), 'math full: index, docs, and skeleton follow Prepare')
  check(at(/^math: skeleton$/) < at(/^notation: plan/) && at(/^notation: apply/) < at(/^terms: plan/) && at(/^terms: plan/) < at(/^digest manager/), 'math full: the notation table, then the term table, before the digest')
  const digestPrompt = calls.find(c => /^digest manager/.test(c.label)).prompt
  check(/math-prefix\.md/.test(digestPrompt) && !/shared-prefix\.md/.test(digestPrompt), 'math full: the digest runs read the math prefix')
  check(/math-link-prefix\.md/.test(calls.find(c => /^link manager/.test(c.label)).prompt), 'math full: the link runs read the math link prefix')
  const compiler = cmds.filter(c => /model-compiler\.ps1/.test(c))
  check(compiler.length > 0 && compiler.every(c => /-BriefDoc "D:\\x\\Key\\math\\brief\.zh\.md" -FillDoc "D:\\x\\Key\\math\\fill\.zh\.md" -ReviewDoc "D:\\x\\Key\\math\\review\.zh\.md"/.test(c)), 'math full: every model-compiler step takes the math instruction files')
  check(!compiler.some(c => /Key\/2\|e/.test(c)), 'math full: the model compiler never builds a Theory Card')
  check(/model-architect-math\.md/.test(calls.find(c => /^plan: /.test(c.label)).prompt), 'math full: the architect reads the math supplement')
  check(at(/^boundary: write/) > at(/^find models: whole book/) && at(/^boundary: settle/) < at(/^proof: review/) && at(/^proof: apply/) < at(/^link manager/), 'math full: Boundary and Proof run after the models and before the link pass')
  check(at(/^theory: write/) > at(/^cards link check: report/) && at(/^theory: apply/) < at(/^cleanup DeepSeek$/), 'math full: Theory runs after the link check and before the cleanup')
  check(at(/^math: check$/) > at(/^audit cards$/), 'math full: the math check runs after the audit')
  check(['notation: apply', 'boundary: settle', 'proof: apply', 'theory: apply'].every(s => cleanups.some(l => l.startsWith(`${s}: clean up`))), 'math full: each math stage\'s last step deletes its DeepSeek conversations')
  check(pool.prompts.every(p => /deepseek_delete/.test(p) && !/deepseek_archive/.test(p)), `math protocol: every DeepSeek manager deletes, none archives (${pool.prompts.length} managers)`)
  check(pool.peak <= 8 && managed.every(m => m.jobs.length <= 8), `math protocol: at most 8 managers at once and 8 jobs per manager (peak ${pool.peak})`)
  check(calls.filter(c => /^(boundary|proof|theory|notation):/.test(c.label)).every(c => c.model === 'haiku'), 'math protocol: Haiku runs every math step and manager')
  check(!labels.some(l => /^(events|findings):/.test(l)) && !cmds.some(c => /(event|finding)-compiler/.test(c)), 'math full: no event or empirical-results layer (the economics set\'s)')
  check(cmds.some(c => /prepare-book-digest\.ps1"? .* -Instructions 'dsh-digest-base\.zh\.md,dsh-model-cards\.zh\.md' -NoEventCandidates -Apply/.test(c)), 'math full: Prepare builds the base prompt with the model-card supplement (the math supplement follows in math-prefix.md)')
  check(!logs.some(l => /failed(?! none)/.test(l)), `math full: no stage logs a failure${logs.filter(l => /failed(?! none)/.test(l)).map(l => ` (${l})`).join('')}`)
}
{
  const shared = /card-compiler\.ps1|hub-compiler\.ps1|book-skeleton\.ps1|organize-(survey|apply)\.ps1|digest-audit\.ps1|book-terms\.ps1|prepare-link-pass\.ps1|prepare-book-digest\.ps1/
  const math = (await run('math', { ...mbase, mode: 'full' })).calls.map(c => c.cmd).filter(c => shared.test(c))
  const econ = (await run('econ', { ...mbase, mode: 'full' }, {}, 'math')).calls.map(c => c.cmd).filter(c => shared.test(c))
  const sort = (list) => [...list].sort()
  check(math.length > 10 && JSON.stringify(sort(math)) === JSON.stringify(sort(econ)), `standards: the shared stages build the same ${econ.length} commands in both workflows`)
}
{
  const { result, calls } = await run('math', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'math' })
  const labels = calls.map(c => c.label)
  check(result && result.mode === 'math' && result.math.boundary && result.math.proof && result.math.theory && result.math.check, 'math mode: runs Boundary, Proof, Theory, and the check')
  check(!labels.some(l => /^(digest|link) manager|^prepare book|^organize|^hub manager/.test(l)), 'math mode: runs no other stage')
}
{
  const { calls } = await run('math', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'math', supply: 'none', proof: false })
  const cmds = calls.map(c => c.cmd).filter(Boolean)
  check(cmds.some(c => /-Step boundary -Staging "D:\\x\\Key" -UnitsPerJob 4 -Supply none/.test(c)) && cmds.some(c => /-Step boundary-apply .* -Supply none/.test(c)), 'math mode: supply none reaches the boundary steps')
  check(!cmds.some(c => /-Step proof/.test(c)), 'math mode: proof false skips the proof review')
}
{
  const { result } = await run('math', { ...mbase, mode: 'full' }, { failStep: /never/ }, 'math')
  check(result && result.math && result.math.index && result.math.skeleton && result.math.notation, 'math full: the result holds the skeleton reports (index, skeleton, notation)')
}

// Report-first Bridge stage order and failure visibility.
for(const kind of ['econ','math']) {
{
 const {result,calls} = await run(kind, {...base,mode:'full',figures:true,hubs:true})
 const labels=calls.map(c=>c.label), at=s=>labels.indexOf(s)
 check(result.bridge?.status==='done' && at('show figures')<at('bridge: plan') && at('bridge: plan')<at('hub manager: survey and limits'), 'full: Figures then report-only Bridge then Hubs')
 check(!calls.some(c=>/bridge-compiler/.test(c.cmd)&&/-Apply\b/.test(c.cmd)), 'full: discovery never applies proposals')
}
{
 const {result,calls}=await run(kind, {...base,mode:'full',bridge:false})
 check(result.bridge?.status==='skipped'&&!calls.some(c=>/bridge-compiler/.test(c.cmd)), 'full: bridge false returns skipped without commands')
}
{
 const {result,calls}=await run(kind, {...base,mode:'full'}, {failStep:/bridge-compiler\.ps1.*-Step plan/})
 check(result.bridge?.status==='failed'&&result.audit?.status==='done', 'full: Bridge failure survives successful later stages')
 check(calls.some(c=>c.label==='bridge: clean up'), 'full: failed planning still cleans stage conversations')
 check(!calls.some(c=>/bridge-compiler\.ps1.*-Step report/.test(c.cmd)), 'full: failed planning publishes no report')
}
{
 const {result,calls}=await run(kind, {...base,mode:'full'}, {failJudge:true})
 check(result.bridge?.status==='failed'&&/rerun mode bridge/.test(result.bridge?.error||''), 'full: a failed judge job fails the stage and says how to resume')
 check(!calls.some(c=>/bridge-compiler\.ps1.*-Step report/.test(c.cmd))&&calls.some(c=>c.label==='bridge: clean up'), 'full: a failed judge job publishes no report and still cleans up')
}
}
console.log(`failed: ${failed}`)
process.exit(failed ? 1 : 0)
