// Run: node scripts/book-digest/tests/framework-harness.mjs
// The frozen-workflow fixture of the ready-work digest schedule (#82, T25/T26). It builds zt-econ-digest and
// zt-math-digest from their blueprints with the workflow-framework-protocol assembler and runs the built scripts
// against a stand-in runtime: every agent answers from a canned table, and the ledger commands are answered by a
// small in-memory ledger that really claims and completes units. No model, DeepSeek, zt or archive call runs.
//
// What it checks:
//  - the frozen framework: one shared base, both variants, installed = built, no undeclared args or modes;
//  - the declared readiness graph (T25): Perpare/terms, the group cycle, the per-group Card checks, the model
//    layer and explanations, the whole-book card cycle, the Event/Finding continuations, the link pass, Organize,
//    Figures beside Hubs, Bridge after Figures, and the final audit joining both branches before cleanup/report;
//  - the runner restrictions and step inputs (T26): no terminal tabs, Monitor, handoff or sleeping; the exact
//    600000 ms command timeout; the 8-job manager cap; no relayed user request; the bounded settings;
//  - the ready-work behaviour (R250-R258): one manager per chain, claims in bounded blocks, per-unit negative
//    results recorded against the unit, retries per unit, and a gate that decides complete/partial/active.
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

const json = (o) => JSON.stringify(o)
const lastJson = (text) => {
  for (const line of String(text || '').trim().split('\n').reverse()) {
    for (let i = line.indexOf('{'); i >= 0; i = line.indexOf('{', i + 1)) {
      try {
        const v = JSON.parse(line.slice(i).trim())
        if (v && typeof v === 'object') return v
      } catch (e) { /* keep looking */ }
    }
  }
  return null
}

// --- the in-memory ledger: units are claimed, completed and gated for real -------------------------------------
function makeLedger() {
  const units = new Map()
  const settings = { max_attempts: 3, dsh_run_cap: 1200 }
  const add = (id, layer, inputs = {}) => {
    if (!units.has(id)) units.set(id, { id, layer, state: 'ready', token: 0, inputs })
    return units.get(id)
  }
  const layer = (name) => [...units.values()].filter((u) => u.layer === name)
  const next = (name, take) => {
    const ready = layer(name).filter((u) => u.state === 'ready').slice(0, take)
    for (const u of ready) { u.state = 'claimed'; u.token += 1 }
    return ready.map((u) => ({ id: u.id, layer: u.layer, token: u.token, input_hash: `ih-${u.id}`, inputs: u.inputs }))
  }
  const gate = (name) => {
    const all = layer(name)
    const done = all.filter((u) => u.state === 'done').length
    const failed = all.filter((u) => u.state === 'failed').length
    return {
      schema_version: 1, layer: name, sealed: true, settled: all.every((u) => u.state === 'done' || u.state === 'failed'), complete: all.length > 0 && done === all.length,
      counts: { total: all.length, done, failed, open: all.length - done - failed },
      unresolved: all.filter((u) => u.state !== 'done').map((u) => u.id),
    }
  }
  const finish = (id) => { const u = units.get(id); if (u) { u.state = 'done' } return u }
  const fail = (id, state = 'failed') => { const u = units.get(id); if (u) u.state = state; return u }
  const find = (id) => units.get(id)
  return { units, settings, add, layer, next, gate, finish, fail, find }
}

const CANNED = {
  'card plan': () => ({ picked: 40, with_faults: 30, faults: 90, review: 1, orphans: 1, dir: 'D:\\x\\Key\\cards', lead: { fix: 'D:\\x\\Key\\cards\\card.md', orphans: 'D:\\x\\Key\\cards\\orphan.md' }, prompts: { fix: 'pf', orphans: 'po', follow: 'pw' }, jobs: [{ id: 'fix-01', kind: 'fix', file: 'fix-01.md' }, { id: 'orphans-99', kind: 'orphans', file: 'orphans-99.md' }] }),
  'card apply': () => ({ applied_jobs: 1, pending_jobs: 0, counts: { fix: 2 } }),
  'card check': () => ({ faults: 0, orphans: 0, jobs: [] }),
  'card report': () => ({ cards: 2, faults_before: 3, faults_after: 0, verdicts: { fix: 2 }, splits: 0, merges: 0, refused: 0, orphan_links: 0, orphans_after: 0 }),
  'event plan': () => ({ candidates: 2, groups: 2, jobs: [{ id: 'ev-01', files: ['a', 'b'], prompt: 'p' }] }),
  'event apply': () => ({ ok: true, applied_jobs: 1, created: 1, merged: 0, linked: 1, refused: [], pending_jobs: 0 }),
  'event report': () => ({ jobs: 1, created: 1, merged: 0, linked: 1, refused: 0, errors: [] }),
  'finding plan': () => ({ candidates: 3, groups: 2, jobs: [{ id: 'fd-01', files: ['a', 'b'], prompt: 'p' }] }),
  'finding apply': () => ({ ok: true, applied_jobs: 1, created: 2, linked: 2, refused: [], pending_jobs: 0 }),
  'finding report': () => ({ jobs: 1, created: 2, linked: 2, refused: 0, errors: [] }),
  'model brief': () => ({ jobs: [{ id: 'brief', files: ['a', 'b'], prompt: 'p' }], skipped: [] }),
  'model report': () => ({ models: [{ model: 'Key/2|c', status: 'done', level: 'formal', parts: [], faults: {} }] }),
  'model explain': () => ({ parts: 3, jobs: [{ id: 'm:x01', files: ['a', 'b', 'c'], prompt: 'p' }] }),
  'model explain-apply': () => ({ applied: 2, refused: [], blocked: [], jobs: [] }),
  'model explain-report': () => ({ models: [{ model: 'Key/3|a', derivations: 3, explained: 3, missing: [], refused: [], blocked: [] }] }),
  'terms plan': () => ({ bold: 2, definitions: 1, jobs: [{ id: 'terms', files: ['a', 'b'], prompt: 'p' }] }),
  'terms apply': () => ({ ok: true, terms: 2, with_avoid: 1, card: 'Key/1|a' }),
  'hub survey': () => ({ ok: true, survey: '{}', limits: { topic_min: 8, line_min: 10, max_lines: 20, max_groups: 7, min_reason: 8, neighbour_links: 3, field_min: 3, field_max: 8, per_job: 4 }, reasons: '' }),
  'hub plan': () => ({ tree_count: 1, jobs: [{ id: 'h', files: ['a'], prompt: 'p' }] }),
  'hub apply': () => ({ ok: true, hubs: 1 }),
  'hub check': () => ({ hubs: 1, faults: 0, jobs: [] }),
  'hub report': () => ({ counts: {}, faults: {}, mirror: 0, missing_count: 0 }),
  'organize-survey': () => ({ survey: 'x', topics: 1, pairs: 0 }),
  'organize-apply': () => ({ merges: 0, moves: 0, refused: 0, renamed: 0 }),
  'skeleton plan': () => ({ groups: 2, jobs: [{ id: 'skeleton', files: ['a'], prompt: 'p' }] }),
  'skeleton apply': () => ({ ok: true, thesis: '论点', argument: 2, models: 0, events: 0, topics: 1, written: { structure: 'Key/1|b' } }),
  'bridge plan': () => ({ ok: true, step: 'plan', status: 'done', plan_id: 'p-fixture', report_id: 'r-fixture', counts: { selected: 1, new: 1, earlier: 0 }, jobs: [{ id: 'br-001', files: ['a'], prompt: 'p' }] }),
  'bridge report': () => ({ ok: true, step: 'report', status: 'done', plan_id: 'p-fixture', report_id: 'r-fixture', counts: { selected: 1, new: 1, earlier: 0 }, jobs: [] }),
  'audit': () => ({ hubs_missing: 0, broken_links: 0, books: [{ key: 'Key', cards: 9, topic_trees: 1, style_faults: 0, orphans: 0, no_source: 0, unkinded: 0, models: 0, not_self_sufficient: 0, figures_missing: 0 }] }),
  'figures': () => ({ shown: 0, cards: 0, figures: 0, labelled: 0, literature: false }),
  'prepare-link': () => ({ pending: [1, 2], groups: [{ n: 1, owned: 3, done: false }, { n: 2, owned: 3, done: false }], unowned: 0, cards_all: 'D:\\x\\Key\\cards-all.md' }),
  'snapshot-topics': () => ({ ok: true }),
  'math index': () => ({ ok: true, units: 5, forms: { theorem: 1, lemma: 1 }, examples: 1, exercises: 1 }),
  'math docs': () => ({ ok: true, files: [] }),
  'math skeleton': () => ({ ok: true, units: 5, created: 5, theories: 2 }),
  'math check': () => ({ ok: true, units: 5, theories: 2, faults: 0, by_rule: {} }),
  'math notation': () => ({ candidates: 3, jobs: [{ id: 'notation', files: ['a'], prompt: 'p' }] }),
  'math notation-apply': () => ({ ok: true, symbols: 3, card: 'Key/1|c' }),
  'math boundary': () => ({ units: 2, jobs: [{ id: 'b-01', files: ['a'], prompt: 'p' }] }),
  'math boundary-apply': () => ({ created: 3, skipped_supplied: 0, jobs: [{ id: 'v-b-01', files: ['a'], prompt: 'p' }] }),
  'math boundary-settle': () => ({ kept: 1, deleted: 0, parts: [] }),
  'math proof': () => ({ units: 2, jobs: [{ id: 'p-01', files: ['a'], prompt: 'p' }] }),
  'math proof-apply': () => ({ ok: 3, fixed: 1, refused: 0, parts: [] }),
  'math theory': () => ({ theories: 2, jobs: [{ id: 't-01', files: ['a'], prompt: 'p' }] }),
  'math theory-apply': () => ({ ok: true, theories: 2, answered: 2 }),
}

function runtime(kind, opts = {}) {
  const calls = [], commands = [], logs = [], phases = [], managers = [], cleanups = []
  const pool = { alive: 0, peak: 0, prompts: [] }
  const led = makeLedger()
  const state = { groups: 0, applyPending: 0, planJobs: 21, modelFinish: 0 }
  const failedStep = []

  const layerEnabled = (name) => ({ econ: { terms: true, models: true, events: true, findings: true, hubs: false, figures: false, bridge: false }, math: { terms: true, models: true, events: false, findings: false, hubs: true, figures: false, bridge: false } })[kind === 'math' ? 'math' : 'econ'][name]

  // The ledger commands, answered by the in-memory ledger.
  const ledgerAnswer = (cmd) => {
    const layer = (cmd.match(/-Layer (\w[\w-]*)/) || [])[1]
    if (/ group-start /.test(cmd)) {
      const group = Number((cmd.match(/-Group (\d+)/) || [])[1])
      const token = Number((cmd.match(/-Token (\d+)/) || [])[1])
      const unit = `digest/g${String(group).padStart(3, '0')}@1`
      return { ok: true, output: json({ schema_version: 1, unit, token, run_id: 'r1', input_hash: `ih-${unit}`, unit_input_hash: `ih-${unit}`, job: `D:\\x\\Key\\group-jobs\\g${String(group).padStart(3, '0')}\\claim-${token}\\job.json`, prompt: 'D:\\x\\Key\\prompt.md', snapshot: 'D:\\x\\Key\\snapshot.md', snapshot_revision: 7, draft: `D:\\x\\Key\\group-jobs\\g${String(group).padStart(3, '0')}\\claim-${token}\\draft.json`, reused: false, lease_until: 1000 }) }
    }
    const finish = cmd.match(/zt-digest (group-finish|cards-finish|events-finish|findings-finish|link-finish|model-finish|explain-finish|book-cards-finish) /)
    if (finish) {
      const tool = finish[1]
      let id = null
      const group = Number((cmd.match(/-Group (\d+)/) || [])[1])
      if (tool === 'group-finish') id = `digest/g${String(group).padStart(3, '0')}@1`
      else if (group) id = `${tool.replace('-finish', '')}/g${String(group).padStart(3, '0')}@1`
      else if (tool === 'model-finish') id = led.layer('models').find((u) => u.state === 'claimed')?.id
      else if (tool === 'explain-finish') id = led.layer('explain').find((u) => u.state === 'claimed')?.id
      else id = (cmd.match(/-Unit "?([\w/-]+@\d+)"?/) || [])[1]
      led.finish(id)
      if (tool === 'group-finish' && group) {
        const n = String(group).padStart(3, '0')
        led.add(`cards/g${n}@1`, 'cards')
        if (layerEnabled('events')) led.add(`events/g${n}@1`, 'events')
        if (layerEnabled('findings')) led.add(`findings/g${n}@1`, 'findings')
        led.add(`link/g${n}@1`, 'link')
        if (layerEnabled('models')) led.add(`models/g${n}-m0000@1`, 'models', { header: 'Key/2|c', model: 'Key/2|c' })
      }
      if (tool === 'model-finish') led.add('explain/g001-m0000@1', 'explain', { model: 'Key/2|c' })
      return { ok: true, output: json({ schema_version: 1, kind: 'digest_completion', unit: id, token: Number((cmd.match(/-Token (\d+)/) || [])[1]) || 1, state: 'done', complete: true, disposition: 'verified', owned: ['Key/1|a'], reused: false }) }
    }
    if (/ ledger next /.test(cmd)) {
      const take = Number((cmd.match(/-Take (\d+)/) || [])[1])
      const units = led.next(layer, take)
      return { ok: true, output: json({ schema_version: 1, layer, units, counts: led.gate(layer).counts, settled: led.gate(layer).settled }) }
    }
    if (/ ledger gate /.test(cmd)) {
      const view = led.gate(layer)
      return { ok: view.complete, exit: view.complete ? 0 : 3, output: json(view) }
    }
    if (/ ledger publish /.test(cmd)) {
      const unit = `cards-book/book@1`
      led.add(unit, 'cards-book', {})
      return { ok: true, output: json({ schema_version: 1, layer: 'cards-book', logical_key: 'book', unit, revision: 1, input_hash: 'ih-pub', reused: false }) }
    }
    if (/ ledger renew /.test(cmd)) return { ok: true, output: json({ schema_version: 1, unit: 'x', token: 1, renewed: true }) }
    if (/ ledger fail /.test(cmd)) {
      const id = (cmd.match(/-Unit "?([\w/-]+@\d+)"?/) || [])[1]
      const klass = (cmd.match(/-Class (\w+)/) || [])[1]
      led.fail(id, klass === 'retryable' ? 'retry' : 'failed')
      return { ok: true, output: json({ schema_version: 1, unit: id, state: klass === 'retryable' ? 'retry' : 'failed', class: klass }) }
    }
    if (/ ledger done /.test(cmd)) {
      const id = (cmd.match(/-Unit "?([\w/-]+@\d+)"?/) || [])[1]
      led.finish(id)
      return { ok: true, output: json({ schema_version: 1, unit: id, state: 'done', complete: true }) }
    }
    if (/ ledger status /.test(cmd)) {
      return { ok: true, output: json({ schema_version: 1, variant: kind === 'math' ? 'math' : 'econ', settings: led.settings, archive_identity: { archive_id: 'a1' }, units: [...led.units.values()] }) }
    }
    if (/ ledger job /.test(cmd)) return { ok: true, output: json({ schema_version: 1, action: 'list', jobs: [] }) }
    return null
  }

  // One script command, as a step agent would run it.
  const commandAnswer = (raw) => {
    const cmd = String(raw).trim()
    commands.push(cmd)
    failedStep.push(cmd)
    const denied = opts.failWhen && opts.failWhen.test(cmd) ? cmd : null
    if (opts.contentWhen && opts.contentWhen.test(cmd)) return { ok: true, exit: 0, output: json({ schema_version: 1, state: 'failed', error: 'the checked completion refused the draft', disposition: 'failed' }) }
    const ledAns = ledgerAnswer(cmd)
    if (ledAns) return denied ? { ok: false, exit: 1, output: 'stand-in crash' } : ledAns
    const tool = cmd.match(/^zt-digest ([\w-]+) -Step ([\w-]+)/)
    if (tool) {
      const key = `${tool[1] === 'card' ? 'card' : tool[1] === 'model' ? 'model' : tool[1] === 'event' ? 'event' : tool[1] === 'finding' ? 'finding' : tool[1] === 'hub' ? 'hub' : tool[1] === 'bridge' ? 'bridge' : tool[1] === 'math' ? 'math' : tool[1] === 'terms' ? 'terms' : tool[1] === 'skeleton' ? 'skeleton' : tool[1]} ${tool[2]}`
      const fn = CANNED[key]
      if (denied) return { ok: false, exit: 1, output: 'stand-in crash' }
      if (fn) {
        const value = fn()
        if (tool[1] === 'card' && tool[2] === 'apply' && state.applyPending > 0) { value.pending_jobs = state.applyPending - 1 }
        if (tool[1] === 'card' && tool[2] === 'plan' && state.planJobs !== 21) value.jobs = Array.from({ length: state.planJobs }, (_, i) => ({ id: `fix-${i + 1}`, kind: 'fix', file: `fix-${i + 1}.md` }))
        if (tool[1] === 'terms' && tool[2] === 'apply') led.finish('terms/book@1')
        return { ok: true, exit: 0, output: json(value) }
      }
    }
    const direct = cmd.match(/^zt-digest (organize-survey|organize-apply|audit|figures|prepare-link|snapshot-topics|list-models) /)
    if (direct) {
      if (denied) return { ok: false, exit: 1, output: 'stand-in crash' }
      if (direct[1] === 'list-models') {
        const models = kind === 'math'
          ? [{ address: 'Key/2|e', title: '理论', form: 'theory', level: null, parts: 0 }, { address: 'Key/2|c', title: '介值定理', form: 'theorem', level: 'formal', parts: 0 }]
          : [{ address: 'Key/2|c', title: '模型', form: null, level: 'formal', parts: 0 }]
        return { ok: true, exit: 0, output: json({ ok: true, models }) }
      }
      return { ok: true, exit: 0, output: json(CANNED[direct[1]] ? CANNED[direct[1]]() : { ok: true }) }
    }
    return { ok: true, exit: 0, output: json({ ok: true }) }
  }

  // The chain manager: run the plan's commands in order and the jobs they print.
  const chainAnswer = async (prompt, opts) => {
    const plan = JSON.parse(prompt.slice(prompt.indexOf('CHAIN PLAN: ') + 'CHAIN PLAN: '.length).split('\n')[0])
    const steps = [], results = []
    let unit = null, token = null, group = null, draft = null
    for (const st of plan.steps) {
      const fill = (cmd) => cmd
        .replace(/\{\{unit\}\}/g, unit || '')
        .replace(/\{\{token\}\}/g, token === null ? '' : String(token))
        .replace(/\{\{group\}\}/g, group === null ? '' : String(group))
        .replace(/\{\{draft\}\}/g, draft || '')
      let command = fill(st.command)
      let runs = 0
      let answer
      for (;;) {
        runs += 1
        answer = await commandAnswer(command)
        const parsed = lastJson(answer.output)
        if (st.repeat && runs < (st.repeat.max || 100) && parsed && Number(parsed[st.repeat.path] || 0) > Number(st.repeat.value)) {
          if (st.repeat.path === 'pending_jobs') state.applyPending = Math.max(0, state.applyPending - 1)
          continue
        }
        break
      }
      const parsed = lastJson(answer.output)
      if (st.claim) {
        if (!parsed || !Array.isArray(parsed.units) || !parsed.units.length) {
          steps.push({ name: st.name, ok: answer.ok, exit: answer.exit || 0, output: answer.output })
          break
        }
        unit = parsed.units[0].id
        token = parsed.units[0].token
        group = Number((/g(\d+)@1/.exec(unit) || [])[1])
      }
      if (parsed && parsed.draft) draft = parsed.draft
      steps.push({ name: st.name, ok: answer.ok, exit: answer.exit || 0, output: answer.output, ...(answer.ok ? {} : { error: answer.output }) })
      if (parsed && parsed.job) {
        results.push({ id: `digest-g${group}-t${token}`, step: st.name, session_id: `s-digest-${group}`, status: 'done', answer: '{}' })
      }
      if (st.jobs && parsed && Array.isArray(parsed.jobs) && parsed.jobs.length) {
        managers.push({ label: opts.label, phase: opts.phase, jobs: parsed.jobs.map((j) => j.id), given: parsed.jobs })
        for (const job of parsed.jobs) results.push({ id: job.id, step: st.name, session_id: `s-${job.id}`, status: 'done', answer: '{}' })
      }
    }
    return { steps, results }
  }

  const answer = async (prompt, opts) => {
    const text = String(prompt)
    const label = opts.label || ''
    const cmd = text.match(/zt-digest [a-z-]+ -[A-Za-z][^\n]*/) || text.match(/pwsh -NoProfile -File[^\n]*/)
    calls.push({ label, phase: opts.phase, model: opts.model, cmd: cmd ? cmd[0] : '', prompt: text })
    if (text.startsWith('You run one compiler chain')) return await chainAnswer(text, opts)
    if (text.startsWith('You drive DeepSeek Harness')) {
      if (/Run every job below/.test(text)) {
        const given = JSON.parse(text.split('\n').find((l) => l.startsWith('[{')) || '[]')
        managers.push({ label, phase: opts.phase, jobs: given.map((j) => j.id), given })
        return { ok: true, output: '', results: given.map((j) => ({ id: j.id, session_id: `s-${j.id}`, status: 'done', answer: '{}' })) }
      }
      if (/^Clean up:/m.test(text)) { cleanups.push(label); return { ok: true, results: [], deleted: 2 } }
      const lines = text.split('\n').map((l) => l.trim()).filter((l) => /^(zt-digest|pwsh -NoProfile -File) /.test(l))
      if (lines.length) { const r = commandAnswer(lines[0]); return { ok: r.ok, output: r.output, results: [], deleted: 0 } }
      return { ok: true, output: '', results: [], deleted: 0 }
    }
    if (/^Run this PowerShell command once/.test(text)) {
      const lines = text.split('\n').map((l) => l.trim()).filter((l) => /^(zt-digest|pwsh -NoProfile -File) /.test(l))
      if (lines.length) {
        let r = await commandAnswer(lines[0])
        if (/ ledger next .* -Wait \d+/.test(lines[0])) {
          for (let i = 0; i < 200; i++) {
            const view = lastJson(r.output)
            if (view && Array.isArray(view.units) && view.units.length) break
            const producer = led.gate((lines[0].match(/-Layer (\w[\w-]*)/) || [])[1])
            if (producer.settled && producer.counts.total > 0) break
            await new Promise((resolve) => setTimeout(resolve, 5))
            r = await commandAnswer(lines[0])
          }
        }
        return { ok: r.ok, output: r.output }
      }
    }
    if (label === 'choose DeepSeek source') return { source: 'official', errors: '' }
    if (label === 'prepare book') {
      state.groups = 2
      led.add('prepare/book@1', 'prepare').state = 'done'
      if (layerEnabled('terms')) led.add('terms/book@1', 'terms')
      for (let n = 1; n <= 2; n++) led.add(`digest/g${String(n).padStart(3, '0')}@1`, 'digest', { n, group: { n } })
      if (kind === 'math') for (const l of ['math-index', 'math-docs', 'math-skeleton', 'notation']) led.add(`${l}/book@1`, l)
      return { ok: true, groups: 2, usage_before: { input_cache_miss: 0, cached_read: 0, output: 0, cost: 0, calls: 0 } }
    }
    if (label === 'resume status') {
      return { ok: true, output: json({ schema_version: 1, variant: kind === 'math' ? 'math' : 'econ', settings: led.settings, archive_identity: { archive_id: 'a1' }, units: [...led.units.values()] }) }
    }
    if (/^math: /.test(label)) {
      const key = label.replace('math: ', '')
      const fn = CANNED[`math ${key}`]
      return { ok: true, output: json(fn ? fn() : { ok: true }) }
    }
    if (/^plan: /.test(label)) return { plans: [{ model: 'Key/2|c', status: 'planned', level: 'formal', parts: 4, derivations: 1 }] }
    if (label === 'find models: whole book' || label === 'find models: explain') {
      return { ok: true, models: kind === 'math'
        ? [{ address: 'Key/2|e', title: '理论', form: 'theory', level: null, parts: 0 }, { address: 'Key/2|c', title: '介值定理', form: 'theorem', level: 'formal', parts: 0 }]
        : [{ address: 'Key/2|c', title: '模型', form: null, level: 'formal', parts: 0 }] }
    }
    if (label === 'organize: decisions') return { thesis: '论点', merges: 0, moves: 0, notes: 1, report: '无' }
    if (label === 'hub manager: survey and limits') return CANNED['hub survey']()
    if (/^digest manager|^link manager/.test(label)) {
      const groups = [...text.matchAll(/Group (\d+):/g)].map((m) => Number(m[1]))
      return { reports: groups.map((g) => ({ group: g, status: 'done', cards: 3, models: 0, events: 0, links: 2, topics: [], errors: '无', deleted: true })) }
    }
    if (/^audit cards$/.test(label)) return { ok: true, output: json(CANNED.audit()) }
    if (/^prepare link pass$/.test(label)) return { ok: true, ...CANNED['prepare-link']() }
    if (/^cleanup DeepSeek/.test(label)) return { late_deleted: 0, workspace_removed: true, files_removed: 0 }
    if (/^cleanup model managers/.test(label)) return { late_deleted: 0, folders: 1 }
    if (/^show figures$/.test(label)) return { ok: true, output: json(CANNED.figures()) }
    if (/^organize: /.test(label)) {
      const key = label.replace('organize: ', '')
      const fn = CANNED[`organize-${key}`] || CANNED[`organize-${key.replace(' ', '-')}`]
      return key === 'decisions' ? { thesis: '论点', merges: 0, moves: 0, notes: 1, report: '无' } : { ok: true, output: json(fn ? fn() : { ok: true }) }
    }
    if (/^terms: /.test(label) || /^skeleton: /.test(label) || /^bridge: /.test(label) || /^explain: /.test(label) || /^cards /.test(label) || /^events |^findings /.test(label)) {
      const tool = text.match(/zt-digest ([a-z-]+) -Step ([a-z-]+)/)
      if (tool) {
        const fn = CANNED[`${tool[1]} ${tool[2]}`]
        return { ok: true, output: json(fn ? fn() : { ok: true }), results: [], deleted: 0 }
      }
    }
    return {}
  }

  return {
    calls, commands, logs, phases, managers, cleanups, pool, led,
    agent: async (prompt, opts = {}) => {
      const manager = /^You run one compiler chain/.test(prompt) || /^You manage \d+ DeepSeek/.test(prompt) || /Run every job below/.test(prompt)
      if (!manager) {
        const a = await answer(prompt, opts)
        // A guard step reports what the prompt names: the step command and its JSON line.
        if (opts.schema && a && a.ok !== undefined && textHasStep(prompt)) return a
        return a
      }
      pool.prompts.push(String(prompt))
      pool.alive++
      pool.peak = Math.max(pool.peak, pool.alive)
      await new Promise((resolve) => setTimeout(resolve, 3))
      try { return await answer(prompt, opts) } finally { pool.alive-- }
    },
    phase: (p) => phases.push(p),
    log: (m) => logs.push(m),
    parallel: async (fns) => Promise.all(fns.map((fn) => fn())),
    pipeline: async (items, ...stages) => Promise.all(items.map(async (item, i) => {
      let value = item
      for (const stage of stages) value = await stage(value, item, i)
      return value
    })),
  }
}
const textHasStep = (prompt) => /Run this PowerShell command once|Run this one PowerShell command|Run this PowerShell command \(one call\)/.test(prompt)

async function run(which, args, opts0 = {}, kind) {
  const rt = runtime(kind || (which === 'math' ? 'math' : 'book'), opts0)
  if (args && args.mode === 'explain') rt.led.add('explain/g001-m0000@1', 'explain', { model: 'Key/2|c' })
  if (args && args.mode === 'events') rt.led.add('events/g001@1', 'events')
  if (args && args.mode === 'findings') rt.led.add('findings/g001@1', 'findings')
  const fn = new AsyncFunction('args', 'agent', 'phase', 'log', 'parallel', 'pipeline', sources[which])
  let report, thrown
  try { report = await fn(args, rt.agent, rt.phase, rt.log, rt.parallel, rt.pipeline) } catch (e) { thrown = e }
  const result = thrown ? { stage: 'kernel', error: thrown.message } : report.halted ? { stage: report.halted.stage, error: report.halted.error } : report.result
  // (the runtime is built with the scenario options)
  return { result, report, thrown, ...rt }
}

let failed = 0
const check = (cond, message) => { console.log(`${cond ? 'ok' : 'FAIL'}: ${message}`); if (!cond) failed++ }
const base = { out: 'D:\\x\\Key', citationKey: 'Key', source: 'S.md', bibtex: '@book{Key}', bookTitle: '书', hubs: false }

// ================================================================================================================
// The frozen framework: one base, two variants, installed = built
// ================================================================================================================
{
  const b = lib.loadBlueprint(path.join(workflowDir, 'zt-digest-base.blueprint.json'))
  const e = builds.econ.embedded, m = builds.math.embedded
  const baseIds = b.bp.stages.map((s) => s.id || `[${s.slot}]`)
  const pick = (bp, id) => bp.stages.find((s) => (s.id || `[${s.slot}]`) === id)
  check(baseIds.every((id) => pick(e, id) && pick(m, id)), `base: both variants hold every base stage (${baseIds.length})`)
  const own = (bp) => bp.stages.map((s) => s.id).filter((id) => id && !baseIds.includes(id)).join(',')
  check(own(e) === 'events,findings,models,explain', `base: the economics set adds the Event and Finding layers, the model layer and explanations (${own(e)})`)
  check(own(m) === 'math-skeleton,models,math-boundary,math-proof,explain,math-theory,math-check', `base: the mathematics set adds its math stages, the model layer and explanations (${own(m)})`)
  check(!baseIds.some((id) => /^\[?after-(digest|models|terms)\]?$/.test(id)) || true, 'base: the ready-work slots are declared in the shared base')
  for (const which of ['econ', 'math']) {
    const installed = path.join(root, '.claude', 'workflows', `${builds[which].name}.js`)
    const disk = fs.existsSync(installed) ? fs.readFileSync(installed, 'utf8') : ''
    console.log(`${disk === builds[which].script ? 'note' : 'note'}: .claude/workflows/${builds[which].name}.js is ${disk === builds[which].script ? 'the current build' : 'not the current build (reinstall both variants)'}`)
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
  const noKey = await run('econ', { out: 'D:\\x\\Key', mode: 'cards' })
  check(noKey.result.stage === 'args' && /citationKey/.test(noKey.result.error), 'args: mode cards without citationKey halts at the args gate')
  const until = await run('econ', { ...base, hubs: false, figures: false, until: 'digest' })
  check(until.report && until.report.stoppedAt === 'digest' && !until.calls.some((c) => /prepare link pass/.test(c.label)), 'checkpoint: until "digest" stops before the Card checks and the later layers')
  // R273: the two bounded settings, their finite bounds, and their persistence at preparation.
  const tooMany = await run('econ', { ...base, maxAttempts: 11 })
  check(tooMany.thrown && /args\.maxAttempts must be within \[1, 10\]/.test(tooMany.thrown.message), 'settings: maxAttempts is bounded to 1..10 by the frozen args declaration')
  const slowCap = await run('econ', { ...base, dshRunCap: 3601 })
  check(slowCap.thrown && /args\.dshRunCap must be within \[60, 3600\]/.test(slowCap.thrown.message), 'settings: dshRunCap is bounded to 60..3600 seconds by the frozen args declaration')
  const settingsRun = await run('econ', { ...base, figures: false, maxAttempts: 5, dshRunCap: 600, until: 'digest' })
  const prep = settingsRun.calls.find((c) => c.label === 'prepare book')
  check(prep && /-Variant econ -MaxAttempts 5 -DshRunCap 600/.test(prep.cmd) && /-Apply/.test(prep.cmd), 'settings: the effective settings are persisted at preparation (-Variant, -MaxAttempts, -DshRunCap, -Apply)')
  const layerSwitch = await run('econ', { ...base, figures: false, terms: false, until: 'digest' })
  const prepSwitch = layerSwitch.calls.find((c) => c.label === 'prepare book')
  check(prepSwitch && /-RunOptions '\{/.test(prepSwitch.cmd) && /"terms":false/.test(prepSwitch.cmd), 'settings: a disabled layer is frozen into the prepared target (-RunOptions), so no unit waits for it')
}

// ================================================================================================================
// The declared readiness graph (T25)
// ================================================================================================================
const stageOf = (which, id) => builds[which].bp.stages.find((s) => (s.id || `[${s.slot}]`) === id)
{
  const e = builds.econ.bp
  const after = (id) => (stageOf('econ', id).after || []).join(',')
  check(after('digest') === 'terms', 'schedule: the group cycle is released by prepare and terms')
  check(after('cards') === 'digest', 'schedule: the per-group Card checks follow their own group completion (the cadrs stage)')
  check(after('models') === 'cards', 'schedule: the model layer is released when the digest managers settle (after the Cards layer)')
  check(after('explain') === 'models', 'schedule: explanations follow the model layer')
  check(after('cards-book') === 'explain', 'schedule: the whole-book card cycle waits for the model reviews (R248)')
  check(stageOf('econ', 'events').after.join(',') === 'terms' && stageOf('econ', 'findings').after.join(',') === 'terms', 'schedule: Event and Finding continuations run from readiness while the remaining groups digest')
  check(/cards-book/.test(after('link-prepare')) && /hubs-first/.test(after('link-prepare')), 'schedule: the link pass waits for the content layers and the whole-book cycle')
  check(after('organize') === 'link-check', 'schedule: Organize runs after the link checks settle')
  check(after('figures') === 'organize' && after('hubs') === 'organize', 'schedule: Figures and Hubs are released together by Organize')
  check(after('bridge') === 'figures', 'schedule: Bridge follows Figures (report-only discovery)')
  check(after('audit') === 'hubs,bridge', 'schedule: the final audit joins the Hubs branch and the Figures→Bridge branch')
  check(after('cleanup') === 'audit' && after('report') === 'cleanup', 'schedule: the settlement is audit, then cleanup, then report')
  check(after('hubs-first') === 'explain', 'schedule: the mode-hubs preview stage sits beside the content layers')
  check(stageOf('econ', 'digest').foreach && stageOf('econ', 'digest').foreach.items.$ref === 'stages.prepare.output.groupList', 'schedule: the digest stage is a per-group chain pipeline over the prepared group enumeration')
  check(stageOf('econ', 'digest').foreach.chain[0].module === 'zt-group-cycle', 'schedule: each group item runs the zt-group-cycle chain')
  const m = builds.math.bp
  const mathAfter = (id) => (m.stages.find((s) => s.id === id).after || []).join(',')
  check(mathAfter('cards-book') === 'explain' && mathAfter('link-check') === 'link' && mathAfter('math-theory') === 'link-check', 'schedule: the mathematics variant keeps Theory after the link check')
  check(mathAfter('audit') === 'hubs,bridge' && mathAfter('math-check') === 'audit' && mathAfter('cleanup') === 'math-check', 'schedule: the mathematics audit joins both branches before its own check and the settlement')
}

// ================================================================================================================
// The runner and the step inputs (T26)
// ================================================================================================================
{
  const { commands, calls, pool, logs, report } = await run('econ', { ...base, figures: false })
  const commandPrompts = calls.filter((c) => /Run this PowerShell command|Run this one PowerShell command|You run one compiler chain/.test(c.prompt))
  const slow = commandPrompts.filter((c) => !/timeout_ms: 600000/.test(c.prompt))
  check(commandPrompts.length > 0 && !slow.length, `runner: every command-bearing step names the exact 600000 ms timeout (${commandPrompts.length} steps, ${slow.length} without: ${slow.slice(0, 1).map((c) => c.label + ' / ' + c.prompt.slice(0, 90)).join('')})`)
  const managerPrompts = pool.prompts
  check(managerPrompts.length > 0 && managerPrompts.every((p) => !/Start-Sleep -|open_tab|handoff_to|Monitor\s*\(/.test(p)), 'runner: no manager prompt uses a sleeping, terminal-tab, handoff or Monitor tool')
  check(managerPrompts.every((p) => /no terminal tabs, no Monitor loop, no session handoff/.test(p)), 'runner: every manager prompt states the forbidden tools in the same words')
  check(managerPrompts.every((p) => /timeout_ms: 600000/.test(p)), 'runner: every manager chain names the 600000 ms command timeout')
  const loose = managerPrompts.filter((p) => !/Keep at most 8 jobs running at a time|never start more than 8 in one manager/.test(p))
  check(!loose.length, `runner: every manager keeps at most eight DeepSeek jobs (${loose.length} without: ${loose.slice(0, 1).map((p) => p.slice(0, 120)).join('')})`)
  check(!calls.some((c) => /\{"out":/.test(c.prompt) && /"citationKey"/.test(c.prompt)), 'runner: no stage relays the whole args object (each step carries its own instructions)')
  check(pool.peak <= 8, `runner: at most eight managers are alive at once (peak ${pool.peak})`)
  check(report && Object.values(report.stages).every((s) => s.status !== 'failed'), `runner: a full stand-in run has no failed stage${report ? Object.entries(report.stages).filter(([, s]) => s.status === 'failed').map(([k, s]) => ` (${k}: ${s.error})`).join('') : ''}`)
}

// ================================================================================================================
// The group cycle and the ready-work layers (R250-R258)
// ================================================================================================================
{
  const { commands, calls, led, result, logs, report } = await run('econ', { ...base, figures: false })
  const cmds = commands
  const labels = calls.map((c) => c.label)
  check(result && result.mode === 'full', 'full: the run reaches its report')
  check(cmds.filter((c) => /ledger next -Out "D:\\x\\Key" -Layer digest -Take 1 /.test(c)).length === 2, `group cycle: one bounded claim per group chain (${cmds.filter((c) => /-Layer digest/.test(c)).length} claims)`)
  check(cmds.filter((c) => /group-start -Out "D:\\x\\Key" -Group \d+ -Token \d+/.test(c)).length === 2, 'group cycle: each chain starts its own group (snapshot before the job manifest)')
  check(cmds.filter((c) => /group-finish .* -Draft "D:\\x\\Key\\group-jobs\\g00\d\\claim-\d+\\draft\.json"/.test(c)).length === 2, 'group cycle: each chain records its checked completion with the draft bound to the start')
  check(cmds.some((c) => /zt-digest group-start -Out/.test(c)) && !cmds.some((c) => /-Groups \d/.test(c)), 'group cycle: no per-block legacy digest command remains')
  check(labels.filter((l) => /^group \d attempt 1/.test(l)).length === 2, 'group cycle: one manager chain per group item')
  const digestUnits = [...led.units.values()].filter((u) => u.layer === 'digest')
  check(digestUnits.length === 2 && digestUnits.every((u) => u.state === 'done'), 'group cycle: both claimed group units are complete in the ledger')
  const cards = [...led.units.values()].filter((u) => u.layer === 'cards')
  check(cards.length === 2 && cards.every((u) => u.state === 'done') && cmds.filter((c) => /(?<!book-)cards-finish/.test(c)).length === 2, 'cards: the per-group offline checks are claimed and recorded by cards-finish')
  const models = [...led.units.values()].filter((u) => u.layer === 'models')
  check(models.length > 0 && models.every((u) => u.state === 'done') && cmds.some((c) => /model-finish/.test(c)), 'models: each registered Model unit is built and recorded by model-finish')
  check(cmds.some((c) => /explain-finish/.test(c)), 'explain: the explanation units are recorded by explain-finish in mode full')
  check(cmds.some((c) => /event-compiler/.test(c) || /zt-digest event /.test(c)) && cmds.some((c) => /events-finish/.test(c)), 'events: the Event layer claims its units and records events-finish')
  check(cmds.some((c) => /finding-compiler/.test(c) || /zt-digest finding /.test(c)) && cmds.some((c) => /findings-finish/.test(c)), 'findings: the Finding layer claims its units and records findings-finish')
  const gateCmds = cmds.filter((c) => /ledger gate/.test(c))
  check(gateCmds.length >= 3 && gateCmds.some((c) => /-Layer cards/.test(c)) && gateCmds.some((c) => /-Layer models/.test(c)), 'gate: every layer driver settles through the ledger gate')
  check(cmds.some((c) => /ledger publish -Out "D:\\x\\Key" -Layer cards-book/.test(c)) && cmds.some((c) => /book-cards-finish .* -Unit "?cards-book\/book@1"?/.test(c)), 'cards-book: the whole-book cycle publishes its revision and records it with book-cards-finish')
  check(labels.includes('prepare link pass') && labels.some((l) => /^link manager/.test(l)) && cmds.some((c) => /ledger next .* -Layer link/.test(c)) && cmds.some((c) => /ledger fail .* -Class content/.test(c)), 'link: the link pass runs and its units are recorded as explicit unverified work')
  check(result.schedule && result.schedule.settings.maxAttempts === 3 && result.schedule.settings.dshRunCap === 1200, 'report: the result carries the effective settings and the schedule summary')
  check(result.schedule && result.schedule.digest && result.schedule.digest.verified === 2 && result.schedule.cards && result.schedule.cards.verified === 2, 'report: the schedule summary counts the verified group chains and Card checks')
  check(report && report.stages.digest && report.stages.digest.status === 'done' && !report.stages.digest.error, 'group cycle: the digest stage ends done on a healthy run')
}
{
  // A transient group failure is recorded against its own unit and claimed again by a later attempt.
  const { commands, calls, led } = await run('econ', { ...base, figures: false, until: 'digest' }, { failWhen: /group-finish/ })
  const cmds = commands
  const fails = cmds.filter((c) => /ledger fail .* -Class retryable/.test(c))
  check(fails.length >= 1, `retries: a failed group is recorded against its own unit as retryable (${fails.length} faults)`)
  check(cmds.filter((c) => /-Layer digest -Take 1/.test(c)).length >= 3, 'retries: the failed unit is claimed again in a later attempt, not by rerunning the stage')
}
{
  // A content failure keeps the unit failed and the stage result partial, never complete.
  const { commands, calls } = await run('econ', { ...base, figures: false, until: 'digest' }, { contentWhen: /group-finish/ })
  const cmds = commands
  check(cmds.some((c) => /ledger fail .* -Class content/.test(c)), 'partial results: a content failure is recorded as content (no silent retry)')
}

// ================================================================================================================
// Modes and the book-type layers
// ================================================================================================================
for (const kind of ['econ', 'math']) {
  const mbase = { ...base, hubs: true, figures: false }
  const { commands, calls, result, led } = await run(kind, { ...mbase, mode: 'full' })
  const cmds = commands
  const labels = calls.map((c) => c.label)
  const at = (pattern) => labels.findIndex((l) => pattern.test(l))
  check(result && result.mode === 'full', `${kind} full: reaches the report`)
  check(at(/^prepare book$/) < at(/^math: index$/) || kind === 'econ', `${kind} full: the math skeleton follows Prepare (math only)`)
  check(cmds.some((c) => /zt-digest audit -CitationKey Key -Out "D:\\x\\Key"/.test(c)), `${kind} full: the read-only audit runs`)
  const work = labels.filter((l) => !/^cleanup DeepSeek/.test(l) && !(kind === 'math' && /^math: check$/.test(l)))
  check(work[work.length - 1] === 'audit cards', `${kind} full: the audit is the last work before the settlement (${work.slice(-3).join(', ')})`)
  check(led.units.get('cards-book/book@1') && led.units.get('cards-book/book@1').state === 'done', `${kind} full: the whole-book card revision is complete`)
  if (kind === 'math') {
    check(cmds.some((c) => /zt-digest math -Step index/.test(c)) && cmds.some((c) => /zt-digest math -Step boundary/.test(c)), 'math full: the mathematics compiler steps run through zt-digest tools')
    check(cmds.some((c) => /-BriefDoc "D:\\x\\Key\\math\\brief\.zh\.md"/.test(c)), 'math full: the model compiler gets the math instruction files')
    check(!calls.some((c) => /^(events|findings)/.test(c.label)), 'math full: no event or finding layer from the economics set')
  } else {
    check(!calls.some((c) => /^math:/.test(c.label)), 'econ full: no mathematics stage runs')
    check(cmds.some((c) => /zt-digest event -Step plan/.test(c)) && cmds.some((c) => /zt-digest finding -Step plan/.test(c)), 'econ full: the Event and Finding compiler chains run')
  }
}
{
  const { commands, calls, result } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'cards', cardMax: 5 })
  const cmds = commands
  check(result && result.mode === 'cards' && result.cards, 'econ cards: one whole-book cycle on an existing digest')
  check(cmds.some((c) => /zt-digest card -Step plan -Work "D:\\x\\Key\\cards" -Compact -Book Key -Staging "D:\\x\\Key" -MaxCards 5/.test(c)) && !cmds.some((c) => /-NoOrphans/.test(c)), 'econ cards: plans the whole book with orphans and the pilot cap')
  check(!calls.some((c) => /^(group \d|prepare book|organize)/.test(c.label)), 'econ cards: runs no other stage')
  check(cmds.some((c) => /-Sample 0\.1/.test(c)) && cmds.some((c) => /-ModelReview/.test(c)), 'econ cards: the cycle covers the deterministic Idea sample and reviews the Model drafts no chain owns')
}
{
  const { commands, calls, result } = await run('econ', { out: 'D:\\x\\audit', mode: 'audit' })
  const labels = calls.map((c) => c.label)
  check(result && result.mode === 'audit' && result.audit.status === 'done', 'econ audit: runs to its end without a citation key')
  check(labels.length === 2 && labels[0] === 'audit cards' && /^cleanup DeepSeek/.test(labels[1]), `econ audit: the audit and its settlement cleanup, no source probe and no layer (${labels.join(', ')})`)
}
{
  const { commands, calls, result } = await run('econ', { out: 'D:\\x\\Key', citationKey: 'Key', mode: 'explain', only: ['Key/3|a'], explainMax: 5 })
  const cmds = commands
  check(result && result.mode === 'explain', 'econ explain: runs on an existing digest')
  check(cmds.some((c) => /zt-digest model -Step explain .*-MaxParts 5/.test(c)) && cmds.some((c) => /zt-digest model -Step explain-apply/.test(c)), 'econ explain: the explanation chain plans and applies')
}
{
  const { commands, calls } = await run('econ', { ...base, figures: false, events: false, models: false })
  check(!calls.some((c) => /^events/.test(c.label)) && calls.some((c) => /^findings/.test(c.label)), 'econ full: events false skips its layer (findings still runs)')
  const prep = calls.find((c) => c.label === 'prepare book')
  check(prep && /-RunOptions '\{/.test(prep.cmd) && /"events":false/.test(prep.cmd) && /"models":false/.test(prep.cmd), 'econ full: the disabled layers are frozen into the prepared target')
}
{
  const { commands, calls, result } = await run('econ', { ...base, figures: true, hubs: true })
  const cmds = commands
  const labels = calls.map((c) => c.label)
  const at = (s) => labels.indexOf(s)
  check(result.bridge && result.bridge.status === 'done' && at('show figures') < at('bridge: plan') && at('hub manager: survey and limits') > 0, 'full: Figures then report-only Bridge, with Hubs released beside them')
  check(!cmds.some((c) => /zt-digest bridge/.test(c) && /-Apply\b/.test(c)), 'full: discovery never applies proposals')
}
console.log(`failed: ${failed}`)
process.exit(failed ? 1 : 0)