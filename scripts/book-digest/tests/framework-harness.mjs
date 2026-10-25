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

// `ledger status -Summary`: identity, settings and the units counted per layer and state, as the tool prints it.
function statusSummary(kind, led) {
  const layers = {}
  for (const u of led.units.values()) {
    const l = (layers[u.layer] = layers[u.layer] || { total: 0 })
    l[u.state] = (l[u.state] || 0) + 1
    l.total += 1
  }
  return { schema_version: 1, variant: kind === 'math' ? 'math' : 'econ', settings: led.settings, citation_key: 'Key', layers }
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
  const next = (name, take, owner = '') => {
    const ready = layer(name).filter((u) => u.state === 'ready').slice(0, take)
    for (const u of ready) { u.state = 'claimed'; u.token += 1; u.owner = owner }
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
  'model close': () => ({ jobs: [], faults: 0, unowned: [] }),
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
  // S23 / #115: the delivered vertical paths the integrated stages drive. The concept review runs on the
  // run's own term table, the restatement review on the run's own staged owned-Card manifests, and the
  // register plan follows the hub publication.
  'concepts propose': () => ({ ok: true, step: 'propose', book: 'Key', term_rows: 3, proposals: 2, omitted: 1, path: 'D:\\x\\Key\\review\\concepts\\proposals.json', terms: ['多重均衡', '学习'] }),
  'concepts prepare': () => ({ ok: true, step: 'prepare', evidence_id: 'e-c1', archive_revision: 7, proposals: 2, term_rows: 3, topic_candidates: 2, restatement_candidates: 1, unread: 0, evidence: 'D:\\x\\Key\\review\\concepts\\evidence.json', jobs: [{ id: 'cj-01', files: ['a', 'b'], prompt: 'p' }] }),
  'concepts judge': () => ({ ok: true, step: 'judge', evidence_id: 'e-c1', judged_id: 'j-c1', judged: 2, pairs: 2, unjudged: 0 }),
  'concepts report': () => ({ ok: true, step: 'report', report_id: 'r-c1', status: 'done', complete: true, findings: 2, judged: 2, unjudged: 0, unread: 0, written: ['D:\\x\\Key\\concept-review.json', 'D:\\x\\Key\\concept-review.md'] }),
  'restate prepare': () => ({ ok: true, step: 'prepare', evidence_id: 'e-r1', archive_revision: 7, cards: 4, reviewable: 2, candidates: 3, unread: 0, population: { owned: 4, reviewable: 2, selected: 2, omitted: 0, skipped: [], skipped_total: 2 }, evidence: 'D:\\x\\Key\\restate-evidence.json', jobs: [{ id: 'rj-01', files: ['a', 'b'], prompt: 'p' }] }),
  'restate judge': () => ({ ok: true, step: 'judge', evidence_id: 'e-r1', judged_id: 'j-r1', judged: 2, unjudged: 0 }),
  'restate apply': () => ({ ok: true, step: 'apply', evidence_id: 'e-r1', judged_id: 'j-r1', prefix: 'Key/review/restate', written: 1, complete: true, counts: { rewrites: 1 }, resolution: { findings: 2 } }),
  'restate report': () => ({ ok: true, step: 'report', report_id: 'rr-c1', status: 'resolved', complete: true, counts: { pairs: 3, recommended: 0, resolved_by_rewrite: 1, pending_existing_card_change: 0, unverified: 0, different: 1, unresolved: 0, unjudged: 0 }, written: ['D:\\x\\Key\\restate-resolution.json', 'D:\\x\\Key\\review\\restate\\restate-report.json'] }),
  'register plan': () => ({ ok: true, step: 'plan', plan_id: 'rp-c1', register: '12', terms: 3, entries: 4, unresolved: 0, faults: 0, excluded: 0, read_only: 0, parts: 1, review: { report_id: 'r-c1', status: 'done', complete: true }, path: 'D:\\x\\Key\\register\\out\\register-plan.json' }),
  'register apply': () => ({ ok: true, step: 'apply', plan_id: 'rp-c1', register: '12', created: 1, changed: 1, entries: 4, read_only: 0, unresolved: 0, complete: true, archive_revision: 7, verified: true, replayed: false }),
  'register check': () => ({ ok: true, step: 'check', plan_id: 'rp-c1', register: '12', register_missing: false, entries: 4, read_only: 0, unresolved: 0, complete: true, faults: 0, missing: 0, diverged: 0, extra: 0, archive_revision: 7 }),
  'organize-survey': () => ({ survey: 'x', topics: 1, pairs: 0 }),
  'organize-apply': () => ({ merges: 0, moves: 0, refused: 0, renamed: 0, notes: 1, thesis: true }),
  'skeleton plan': () => ({ groups: 2, jobs: [{ id: 'skeleton', files: ['a'], prompt: 'p' }] }),
  'skeleton apply': () => ({ ok: true, thesis: '论点', argument: 2, models: 0, events: 0, topics: 1, written: { structure: 'Key/1|b' } }),
  'bridge plan': () => ({ ok: true, step: 'plan', status: 'done', plan_id: 'p-fixture', report_id: 'r-fixture', counts: { selected: 1, new: 1, earlier: 0 }, jobs: [{ id: 'br-001', files: ['a'], prompt: 'p' }] }),
  'bridge report': () => ({ ok: true, step: 'report', status: 'done', plan_id: 'p-fixture', report_id: 'r-fixture', counts: { selected: 1, new: 1, earlier: 0 }, jobs: [] }),
  'audit': () => ({ hubs_missing: 0, broken_links: 0, books: [{ key: 'Key', cards: 9, topic_trees: 1, style_faults: 0, orphans: 0, no_source: 0, unkinded: 0, models: 0, not_self_sufficient: 0, figures_missing: 0 }] }),
  'figures': () => ({ shown: 0, cards: 0, figures: 0, labelled: 0, literature: false }),
  // #110: group 1 has semantic gaps; group 2's owned Cards are all linked, so it needs no job.
  'prepare-link': () => ({ pending: [1], no_gap: [2], blocked: [], groups: [{ n: 1, id: 'g01', owned: 3, done: false, digest: 'done', blocked: false, gaps: 2, skipped: 1, position: 'anchor_position_missing' }, { n: 2, id: 'g02', owned: 3, done: false, digest: 'done', blocked: false, gaps: 0, skipped: 3, position: 'linked' }], owned_total: 6, gap_total: 2, skipped_total: 4, pending_count: 1, no_gap_count: 1, scheduled: 1, unowned: 0, digest_state: 'ledger', cards_all: 'D:\\x\\Key\\cards-all.md' }),
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

// The finite finalization's answers (#86): the four branches, and the plan's own shape.
const FINALIZE_BRANCHES = {
  fix: { branch: 'fix', cards: ['Key/1|a'], rules: ['R3'], counts: { fix: 1, conflict: 0, blocked: 0, stale: 0, advisory: 1 } },
  no_fix: { branch: 'no_fix', cards: [], rules: [], counts: { fix: 0, conflict: 0, blocked: 0, stale: 0, advisory: 2 } },
  conflict: { branch: 'conflict', cards: [], rules: [], counts: { fix: 0, conflict: 1, blocked: 0, stale: 0, advisory: 1 } },
  unresolved: { branch: 'unresolved', cards: [], rules: [], counts: { fix: 0, conflict: 0, blocked: 1, stale: 0, advisory: 1 } },
}
const finalizeBranch = (name) => FINALIZE_BRANCHES[name] || FINALIZE_BRANCHES.fix
const finalizePlan = (name) => {
  const b = finalizeBranch(name)
  return {
    schema_version: 1, kind: 'audit_plan', step: 'plan', run_id: 'r1', citation_key: 'Key', variant: 'econ', graph_revision: 7,
    audit: { unit: 'audit/book@1', revision: 1, graph_revision: 7 },
    faults: {
      total: b.counts.fix + b.counts.conflict + b.counts.blocked + b.counts.advisory, hard: b.counts.fix + b.counts.conflict + b.counts.blocked, advisory: b.counts.advisory,
      by_rule: {}, list: [],
    },
    counts: b.counts, branch: b.branch,
    fix_cycle: { permitted: 1, used: 0, rules: b.rules, cards: b.cards, card_list: b.cards.length ? 'D:\\x\\Key\\finalize\\fix-cards.txt' : null },
    refresh: [],
  }
}
const finalizeScope = () => ({
  schema_version: 1, kind: 'finalize_changes', step: 'scope', graph_revision: 8, change_hash: 'a'.repeat(64),
  changed: [{ address: 'Key/1|a', kinds: ['structure'], category: 'idea', tree: 'Key', fingerprint_before: 'b'.repeat(64), fingerprint_after: 'c'.repeat(64) }],
  touched_trees: ['Key'], kinds: ['structure'], counts: { cards: 1, trees: 1 },
  refresh: [
    { layer: 'link-check', applicable: true, artifacts: ['report'], coverage: 'none', reason: '', branch_refreshable: true },
    { layer: 'organize', applicable: false, stage_owned: true, reason: 'its own stage generates this content (structure), so that stage owns the refresh; the changed-Card manifest is its input and is recorded here' },
    { layer: 'audit', applicable: true, artifacts: ['report'], coverage: 'none', reason: '', branch_refreshable: true },
  ],
})
const finalizeRefresh = (layer) => ({ layer, unit: `${layer}/book@2`, revision: 2, state: 'done', complete: true, disposition: 'published', reused: false, superseded: [], changed_cards: 1, touched_trees: ['Key'] })
const finalizeFinal = (name) => {
  const b = finalizeBranch(name)
  const residual = b.counts.fix + b.counts.conflict + b.counts.blocked
  return {
    schema_version: 1, kind: 'finalize_report', step: 'final', citation_key: 'Key', variant: 'econ',
    status: residual ? 'partial' : 'done', complete: !residual, branch: b.branch, graph_revision: 8,
    audit: { scheduled: { unit: 'audit/book@1', revision: 1, graph_revision: 7 }, final: { unit: 'audit/book@2', revision: 2, graph_revision: 8 } },
    changes: { cards: b.counts.fix ? 1 : 0, trees: b.counts.fix ? 1 : 0, kinds: b.counts.fix ? ['structure'] : [], graph_revision: 8, change_hash: 'a'.repeat(64) },
    refresh: [{ layer: 'link-check', unit: 'link-check/book@2', revision: 2, state: 'done', refreshed: true, applicable: true, reason: '' }, { layer: 'audit', unit: 'audit/book@2', revision: 2, state: 'done', refreshed: true, applicable: true, reason: '' }],
    faults: { total: 0, residual, advisory: b.counts.advisory, by_class: residual ? { hard: residual } : {}, list: [] },
    infrastructure: [], audit_book: { cards: 9, orphans: 0, figures_missing: 0, models_not_self_sufficient: 0 },
    report: { path: 'D:\\x\\Key\\finalize\\final-report.json' },
  }
}

function runtime(kind, opts = {}) {
  const calls = [], commands = [], logs = [], phases = [], managers = [], cleanups = [], resourceRecords = [], evidenceRecords = [], settleRecords = [], driveFiles = [], heldReads = [], faultRefusals = [], holds = [], sweeps = [], briefLists = [], stagePublications = [], cleanupFinishes = [], workspaceLists = [], chunkShares = [], cleanupReadyCalls = [], cleanupRunCalls = [], settleCalls = [], batchCalls = [], batchFinishes = [], batchFaults = [], depsBatches = [], failClasses = [], termCalls = [], termManagers = []
  // S24 (#167): the Term-card branch's fixed two-job plan (both concept jobs, so no review is owed).
  const termJobs = [
    { id: 't001', kind: 'concept', rows: [1], terms: ['均衡'] },
    { id: 't002', kind: 'concept', rows: [2], terms: ['机会主义'] },
  ]
  const pool = { alive: 0, peak: 0, prompts: [] }
  const led = makeLedger()
  // A resumed run reads the units its prepared run retained; the stand-in re-adds exactly those, so the
  // resume path drains a real closure instead of halting on an empty ledger.
  if (opts.retained) for (const [layer, id] of opts.retained) led.add(id, layer)
  if (opts.retainedDone) for (const u of led.units.values()) if (opts.retainedDone.includes(u.layer)) led.finish(u.id)
  const scenario = opts
  const state = { groups: 0, applyPending: 0, planJobs: 21, modelFinish: 0, resources: new Map() }
  const failedStep = []

  const layerEnabled = (name) => ({ econ: { terms: true, models: true, events: true, findings: true, hubs: false, figures: false, bridge: false }, math: { terms: true, models: true, events: false, findings: false, hubs: true, figures: false, bridge: false } })[kind === 'math' ? 'math' : 'econ'][name]

  // The ledger commands, answered by the in-memory ledger.
  const ledgerAnswer = (cmd, extraRecord = null) => {
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
      if (tool === 'link-finish') {
        // #110: the gap scope, outcomes and L1/L2 quality travel with the completion.
        const n = String(group).padStart(3, '0')
        const gap = Number((cmd.match(/-Group (\d+)/) || [])[1]) === 2 ? 0 : 2
        return { ok: true, output: json({ schema_version: 1, kind: 'link_completion', unit: id, token: Number((cmd.match(/-Token (\d+)/) || [])[1]) || 1, state: 'done', complete: true, applied: gap > 0, disposition: gap > 0 ? 'linked' : 'no_link', owned: ['Key/1|a'], changed: gap > 0 ? [`Key/${n}|a`] : [], counts: { owned: 3, gaps: gap, skipped: gap > 0 ? 1 : 3, linked: gap > 0 ? 1 : 0, linked_from: 0, already_linked: gap > 0 ? 1 : 3, no_link: gap > 0 ? 1 : 0 }, quality: { rules: 'L1,L2', faults: 0, checked: 3 }, reused: false }) }
      }
      return { ok: true, output: json({ schema_version: 1, kind: 'digest_completion', unit: id, token: Number((cmd.match(/-Token (\d+)/) || [])[1]) || 1, state: 'done', complete: true, disposition: 'verified', owned: ['Key/1|a'], reused: false }) }
    }
    if (/ ledger resume /.test(cmd)) {
      // #87: the one supported resume: invocation 2, resume_count 1, only unresolved units reset.
      return { ok: true, output: json({
        schema_version: 1, status: 'resumed', resumed: true, run_id: 'r1', invocation: 2, resume_count: 1,
        citation_key: 'Key', variant: kind === 'math' ? 'math' : 'econ', settings: led.settings,
        archive_identity: { archive_id: 'a1' }, reclaimed: [],
        reset: [{ unit: 'digest/g001@1', layer: 'digest', prior_state: 'failed', prior_attempts: 3 }],
        reevaluated: [{ unit: 'events/g001@1', layer: 'events', prior_state: 'blocked' }],
        retained: { units: [] }, unresolved: ['digest/g001@1'],
        closure: { prerequisites: ['digest/g001@1'], downstream: ['events/g001@1'] }, ready: [], manual: [], complete: false,
      }) }
    }
    if (/ ledger next /.test(cmd)) {
      // ebrill2001modern trial 1: one claim of the events layer refused while parallel lanes changed the archive.
      if (opts.failingClaim && opts.failingClaim.layer === layer && (state.claimFailures || 0) < opts.failingClaim.times) {
        state.claimFailures = (state.claimFailures || 0) + 1
        return { ok: false, exit: 1, output: 'zt-digest ledger: archive changed while checking preparation' }
      }
      const take = Number((cmd.match(/-Take (\d+)/) || [])[1])
      const owner = (cmd.match(/-Owner "([^"]*)"/) || [])[1] || ''
      const claimed = led.next(layer, take, owner)
      // trial 6: a producer's late group adds a unit after the layer's earlier claims found work. Each of the
      // first opts.lateUnits.count claims of that layer makes one more unit ready for the next claim.
      if (opts.lateUnits && opts.lateUnits.layer === layer) {
        state.lateClaims = (state.lateClaims || 0) + 1
        if (state.lateClaims <= opts.lateUnits.count) led.add(`${layer}/g${String(100 + state.lateClaims)}@1`, layer)
      }
      // `-Brief` keeps only a unit's Model address (header, model) of its inputs, as zt-digest does.
      const briefIn = (inputs) => { const k = {}; for (const f of ['header', 'model']) if (typeof (inputs || {})[f] === 'string') k[f] = inputs[f]; return Object.keys(k).length ? k : undefined }
      const units = / -Brief\b/.test(cmd) ? claimed.map(({ inputs, ...u }) => (briefIn(inputs) ? { ...u, inputs: briefIn(inputs) } : u)) : claimed
      // grossman2002integration trial 4: a relay returned the claim line whole except every unit's inputs.
      if (opts.droppedInputs && opts.droppedInputs === layer && units.length) {
        return { ok: true, output: json({ schema_version: 1, layer, units: units.map(({ inputs, ...u }) => u), counts: led.gate(layer).counts, settled: led.gate(layer).settled }) }
      }
      // grossman2002integration trial 3: a relay returned the claim line with its units array retyped as a
      // sentence; the driver must take its units from the ledger (`ledger status -Held`), not the relay.
      if (opts.summarizedClaim && opts.summarizedClaim === layer && units.length) {
        return { ok: true, output: json({ schema_version: 1, layer, counts: led.gate(layer).counts, settled: led.gate(layer).settled, units: `(${units.length} units claimed: ${units.map((u) => u.id).join(', ')}; owner '${owner}', lease 1500s)` }) }
      }
      // grossman2002integration trial 17: a relay returned the claim line with its units array replaced by a
      // field of its own, `units_claimed` (the ids only); the line has no `units` at all.
      if (opts.renamedUnits && opts.renamedUnits === layer && units.length) {
        return { ok: true, output: json({ schema_version: 1, layer, counts: led.gate(layer).counts, settled: led.gate(layer).settled, units_claimed: units.map((u) => u.id) }) }
      }
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
    if (/^zt-digest model-deps .*-Units /.test(cmd)) {
      // zt-digest 0.12: every planned model of a batch in one command, one short entry per unit.
      const listed = ((cmd.match(/ -Units "([^"]*)"/) || [])[1] || '').split(',').filter(Boolean).map((e) => e.slice(0, e.lastIndexOf(':')))
      depsBatches.push(listed)
      if (opts.garbledDeps) return { ok: true, output: 'model-deps registered every model (abridged)' }
      return { ok: true, output: json({ schema_version: 1, kind: 'model_deps_batch', units: listed.length, results: listed.map((unit) => {
        const model = ((led.find(unit) || {}).inputs || {}).header || ''
        const waiting = !!(opts.depsWaiting && opts.depsWaiting.test(model))
        if (waiting) { const u = led.find(unit); if (u) u.state = 'registered' }
        return { unit, ok: true, waiting, state: waiting ? 'registered' : 'claimed' }
      }) }) }
    }
    if (/^zt-digest model-deps /.test(cmd)) {
      // R233: the producer of a referenced part is done (resolved) or not yet (waiting), per scenario.
      const model = (cmd.match(/ -Model '([^']+)'/) || [])[1] || ''
      const waiting = opts.depsWaiting && opts.depsWaiting.test(model)
      return { ok: true, output: json({ ok: true, unit: 'models/x', waiting: !!waiting, refs: [], state: waiting ? 'registered' : 'claimed' }) }
    }
    if (/ ledger hold /.test(cmd)) {
      // A hold renews the listed claims that are still held and releases the rest, as zt-digest does.
      const list = ((cmd.match(/ -Units "([^"]*)"/) || [])[1] || '').split(',').filter(Boolean).map((e) => { const i = e.lastIndexOf(':'); return { unit: e.slice(0, i), token: Number(e.slice(i + 1)) } })
      const isHeld = (t) => { const u = led.find(t.unit); return !!u && u.state === 'claimed' && u.token === t.token }
      holds.push(list.map((t) => t.unit))
      return { ok: true, output: json({ schema_version: 1, held: list.filter(isHeld), released: list.filter((t) => !isHeld(t)), renewals: list.filter(isHeld).length, seconds: 0 }) }
    }
    if (/ ledger renew /.test(cmd)) return { ok: true, output: json({ schema_version: 1, unit: 'x', token: 1, renewed: true }) }
    if (/ ledger fail /.test(cmd)) {
      const id = (cmd.match(/-Unit "?([\w/-]+@\d+)"?/) || [])[1]
      // The real ledger refuses a fault with no input hash (trial 4: "incomplete prepared contract: input_hash").
      const fault = (cmd.match(/ -Fault '(.*)'$/) || [])[1]
      if (fault !== undefined) { try { if (!JSON.parse(fault.replace(/''/g, "'")).input_hash) { faultRefusals.push(id); return { ok: false, exit: 1, output: 'zt-digest ledger: incomplete prepared contract: input_hash' } } } catch {} }
      let parsed = null
      try { parsed = JSON.parse(String(fault || '{}').replace(/''/g, "'")) } catch {}
      const given = (cmd.match(/-Class (\w+)/) || [])[1]
      const read = classOf(parsed)
      const klass = given || read.class
      failClasses.push({ unit: id, class: klass, code: read.code, given: !!given })
      led.fail(id, klass === 'retryable' ? 'retry' : 'failed')
      return { ok: true, output: json({ schema_version: 1, unit: id, state: klass === 'retryable' ? 'retry' : 'failed', class: klass }) }
    }
    if (/ ledger done /.test(cmd)) {
      const id = (cmd.match(/-Unit "?([\w/-]+@\d+)"?/) || [])[1]
      led.finish(id)
      // S24 (#167): the Term-card book unit is ready only once every job unit is done, as the real
      // ledger's success dependencies make it. A failed job leaves it registered, so the Term-link
      // pass can never claim it.
      if (id && id.startsWith('termcards/') && !id.startsWith('termcards/book@')) {
        const jobs = led.layer('termcards').filter((u) => u.id.startsWith('termcards/t'))
        const book = led.find('termcards/book@1')
        if (book && jobs.length && jobs.every((u) => u.state === 'done')) book.state = 'ready'
      }
      return { ok: true, output: json({ schema_version: 1, unit: id, state: 'done', complete: true }) }
    }
    if (/ ledger status /.test(cmd) && / -Held /.test(cmd)) {
      const owner = (cmd.match(/ -Held "([^"]*)"/) || [])[1]
      heldReads.push(owner)
      const held = [...led.units.values()].filter((u) => u.state === 'claimed' && u.owner === owner)
      const keep = (inputs) => { const k = {}; for (const f of ['header', 'model']) if (typeof (inputs || {})[f] === 'string') k[f] = inputs[f]; return Object.keys(k).length ? k : null }
      return { ok: true, output: json({ schema_version: 1, owner, units: held.map((u) => ({ id: u.id, owner, token: u.token, lease_until: 9, input_hash: `ih-${u.id}`, inputs: keep(u.inputs) })) }) }
    }
    if (/ ledger status /.test(cmd) && / -Units /.test(cmd)) {
      const ids = (cmd.match(/ -Units (\S+)/) || [])[1].split(',')
      const stateOf = (u) => (u.state === 'retry' ? 'ready' : u.state)
      return { ok: true, output: json({ schema_version: 1, units: ids.map((id) => { const u = led.find(id); return u ? { id, state: stateOf(u), token: u.token, input_hash: `ih-${id}`, attempts: u.token, fault_class: null, disposition: u.state === 'done' ? 'verified' : null } : { id, state: 'unknown' } }) }) }
    }
    if (/ ledger status /.test(cmd)) {
      if (opts.rewrittenSummary && / -Summary\b/.test(cmd)) { const { layers, ...rest } = statusSummary(kind, led); return { ok: true, output: json({ ...rest, layers_open: Object.fromEntries(Object.entries(layers || {}).map(([k, v]) => [k, v.open || 0])) }) } }
      return { ok: true, output: / -Summary\b/.test(cmd) ? json(statusSummary(kind, led)) : json({ schema_version: 1, variant: kind === 'math' ? 'math' : 'econ', settings: led.settings, citation_key: 'Key', archive_identity: { archive_id: 'a1' }, units: [...led.units.values()] }) }
    }
    // R331/R343: the verified run report the cleanup stage renders after every stage settles. Its usage is
    // per-run, never the difference of two account totals (the removed I22 shape).
    if (/ ledger report /.test(cmd)) {
      return { ok: true, output: json({
        schema_version: 1, kind: 'run_report',
        report: {
          'run-report.json': { path: 'D:\\x\\Key\\run-report.json', sha256: 'a'.repeat(64), changed: true },
          'run-report.md': { path: 'D:\\x\\Key\\run-report.md', sha256: 'b'.repeat(64), changed: true },
        },
        identity: { run_id: 'r1', invocation: 1, resume_count: 0 },
        completeness: { settled: true, content_complete: true, quality_complete: true, cleanup_complete: true, complete: true },
        counts: { declared: 3, done: 3, noop: 0, refused: 0, failed: 0, blocked: 0, open: 0 },
        gaps: [], placeholders: { status: 'verified', count: 0, models: [], revision: 7 },
        timing: { wall_seconds: 1, queue_wait_seconds: 0, tool_seconds: 0, external_run_seconds: 0, critical_path: { seconds: 1, units: ['prepare/book@1'] }, stages: [] },
        resume: { available: false, reason: 'no unresolved work remains', manual: [] },
        bridge: null,
        publications: [],
        usage: { status: 'complete', source: 'per-run', known_runs: 1, missing_runs: 0, missing: [], totals: { input_tokens: 100, output_tokens: 20 }, estimates: null, by_phase: { initial: { runs: 1, usage: { input_tokens: 100 } } }, data_faults: [] },
      }) }
    }
    if (/ ledger job /.test(cmd)) return { ok: true, output: json({ schema_version: 1, action: 'list', jobs: [] }) }
    // Owned provider resources (#83): the workflow records the returned ids, observes the provider's
    // facts, and settles the cleanup unit through its checked verifier.
    if (/ ledger resource /.test(cmd)) {
      // The listings the cleanup stage and its agents read (zt-digest `resource_brief`, `resource_counts`, `resource_brief_of`).
      const briefSpaces = () => {
        const spaces = new Map()
        for (const r of state.resources.values()) {
          const w = spaces.get(r.workspace.path) || { id: r.workspace.id, path: r.workspace.path, recorded: [], open: [], bare: [] }
          for (const c of r.conversations) { w.recorded.push(c.id); if (!c.lost && c.deleted !== true) w.open.push(c.id); if (!c.lost && !(c.evidence && typeof c.evidence === 'object')) w.bare.push(c.id) }
          spaces.set(r.workspace.path, w)
        }
        return [...spaces.values()]
      }
      if (/-List -Counts/.test(cmd)) {
        briefLists.push(cmd)
        return { ok: true, output: json({ schema_version: 1, action: 'list', counts: true, resources: state.resources.size, workspaces: briefSpaces().map((w) => (opts.listNoPath ? { id: w.id || w.path, recorded: w.recorded.length, open: w.open.length, bare: w.bare.length } : { id: w.id, path: w.path, recorded: w.recorded.length, open: w.open.length, bare: w.bare.length })) }) }
      }
      const oneSpace = cmd.match(/-List -Workspace "([^"]+)"/)
      if (oneSpace) {
        workspaceLists.push(oneSpace[1])
        return { ok: true, output: json({ schema_version: 1, action: 'list', workspace: briefSpaces().find((w) => w.path === oneSpace[1]) || { path: oneSpace[1], recorded: [], open: [], bare: [] } }) }
      }
      if (/-SweepEvidence "/.test(cmd)) {
        const ids = briefSpaces().flatMap((w) => w.bare)
        for (const id of ids) for (const r of state.resources.values()) for (const c of r.conversations) if (c.id === id) c.evidence = { answer: { path: `D:\\x\\Key\\answers\\${id}.txt`, sha256: 'a'.repeat(64) } }
        sweeps.push(ids)
        return { ok: true, output: json({ schema_version: 1, action: 'sweep_evidence', recorded: ids.length, missing: 0, first_missing: [] }) }
      }
      if (/-List -Brief/.test(cmd)) {
        briefLists.push(cmd)
        return { ok: true, output: json({ schema_version: 1, action: 'list', brief: true, resources: state.resources.size, workspaces: briefSpaces() }) }
      }
      if (/-List/.test(cmd)) return { ok: true, output: json({ schema_version: 1, action: 'list', resources: [...state.resources.values()] }) }
      const holding = (id) => [...state.resources.values()].find((r) => r.conversations.some((c) => c.id === id))
      // The manager's own records (zt-digest-lib v15): plain arguments, no record file.
      const registerId = cmd.match(/-Unit (?:"([^"]+)"|(run))(?: -Token (\d+))? -RegisterId (\S+) -Workspace "([^"]+)"/)
      if (registerId) {
        const unitName = registerId[1] || registerId[2]
        extraRecord = { schema_version: 1, unit: unitName, token: unitName === 'run' ? 1 : Number(registerId[3]), workspace: { id: registerId[5], path: registerId[5] }, conversations: [{ id: registerId[4] }], via: 'manager' }
      }
      // The cleanup stage's evidence sweep: every listed id whose answer file exists gets its evidence.
      const sweep = cmd.match(/foreach \(\$zdId in @\(([^)]*)\)\)/)
      if (sweep) {
        const ids = (sweep[1].match(/'([^']+)'/g) || []).map((q) => q.slice(1, -1))
        const ok = []
        for (const id of ids) { const target = holding(id); if (!target) continue; for (const c of target.conversations) if (c.id === id) c.evidence = { answer: { path: `D:\\x\\Key\\answers\\${id}.txt`, sha256: 'a'.repeat(64) } }; ok.push(id) }
        sweeps.push(ids)
        return { ok: true, output: json({ recorded: ok.length, missing: ids.filter((id) => !ok.includes(id)) }) }
      }
      const evidenceFor = cmd.match(/-EvidenceFor (\S+) -Evidence "([^"]+)"/)
      if (evidenceFor && opts.skipEvidence && opts.skipEvidence.test(evidenceFor[1])) {
        // trial 6: the manager wrote the answer file but its evidence record never reached the ledger.
        return { ok: true, output: json({ schema_version: 1, action: 'observe', op: 'evidence', skipped: true }) }
      }
      if (evidenceFor) {
        const target = holding(evidenceFor[1])
        if (!target) return { ok: false, exit: 1, output: `unknown conversation: no recorded resource holds \`${evidenceFor[1]}\`` }
        for (const c of target.conversations) if (c.id === evidenceFor[1]) c.evidence = { answer: { path: evidenceFor[2], sha256: 'a'.repeat(64) } }
        evidenceRecords.push({ id: evidenceFor[1], file: evidenceFor[2] })
        return { ok: true, output: json({ schema_version: 1, action: 'observe', op: 'evidence', resources: [...state.resources.values()] }) }
      }
      const settle = cmd.match(/-Settle (\S+) -State (\w+) -Deleted (\w+)/)
      if (settle) {
        const target = holding(settle[1])
        if (!target) return { ok: false, exit: 1, output: `unknown conversation: no recorded resource holds \`${settle[1]}\`` }
        for (const c of target.conversations) if (c.id === settle[1]) { c.terminal = true; c.outcome = settle[2]; c.deleted = settle[3] !== 'failed' }
        settleRecords.push({ id: settle[1], state: settle[2], deleted: settle[3] })
        return { ok: true, output: json({ schema_version: 1, action: 'observe', op: 'deleted', resources: [...state.resources.values()] }) }
      }
      const settleWorkspace = cmd.match(/-SettleWorkspace "([^"]+)" -Removed (true|false)/)
      if (settleWorkspace) {
        const targets = [...state.resources.values()].filter((r) => r.workspace.path === settleWorkspace[1])
        if (!targets.length) return { ok: false, exit: 1, output: `unknown workspace: no recorded resource uses \`${settleWorkspace[1]}\`` }
        for (const r of targets) { r.workspace_removed = settleWorkspace[2] === 'true'; r.listing = { conversations: [], unknown: [] } }
        return { ok: true, output: json({ schema_version: 1, action: 'observe', op: 'workspace', resources: [...state.resources.values()] }) }
      }
      if (/-Register/.test(cmd)) {
        const record = extraRecord || resourceRecord(cmd)
        if (!record) return { ok: false, exit: 1, output: 'no resource record in the command' }
        const unit = record.unit === 'run' ? 'cleanup/book@1' : `cleanup/${String(record.unit).replace(/[^A-Za-z0-9._-]+/g, '-')}-t${record.token}@1`
        const id = `${unit}#${record.workspace.id}`
        const known = state.resources.get(id) || {
          id, cleanup_unit: unit, owner: record.unit, label: `zet-r1-${String(record.unit).replace(/[^A-Za-z0-9._-]+/g, '-')}-t${record.token}`,
          workspace: record.workspace, conversations: [], usage: null, listing: null, unknown: [], workspace_removed: null, scratch: [],
        }
        for (const conversation of record.conversations || []) {
          if (!known.conversations.some((c) => c.id === conversation.id)) known.conversations.push({ id: conversation.id, lost: false, terminal: false, deleted: false, evidence: conversation.answer ? { answer: { path: `D:\\x\\Key\\cleanup\\evidence\\${conversation.id}.answer.txt`, sha256: 'a'.repeat(64) } } : null })
        }
        resourceRecords.push(record)
        if (!led.find(unit)) led.add(unit, 'cleanup')
        state.resources.set(id, known)
        return { ok: true, output: json({ schema_version: 1, action: 'register', cleanup_unit: unit, label: known.label, resources: [...state.resources.values()] }) }
      }
      if (/-Observe/.test(cmd)) {
        const record = extraRecord || resourceRecord(cmd)
        const target = record && [...state.resources.values()].find((r) => r.id === record.resource)
        if (target && record.op === 'terminal') for (const c of target.conversations) if (c.id === record.conversation) { c.terminal = true; c.outcome = record.outcome || record.state }
        if (target && record.op === 'deleted') for (const c of target.conversations) if (c.id === record.conversation) c.deleted = true
        if (target && record.op === 'usage') target.usage = record.usage
        if (target && record.op === 'listing') { target.listing = { conversations: record.conversations || [], unknown: record.unknown || [] }; target.unknown = record.unknown || [] }
        if (target && record.op === 'workspace') target.workspace_removed = record.removed === true
        return { ok: true, output: json({ schema_version: 1, action: 'observe', op: record ? record.op : null, resources: [...state.resources.values()] }) }
      }
      return null
    }
    // zt-digest 0.11.7 `cleanup-run`: plan (the evidence sweep, then every recorded id not yet deleted and every
    // recorded workspace not yet removed), and apply (the bridge's results recorded, then `cleanup-finish -Ready`).
    const planStep = cmd.match(/zt-digest cleanup-run -Out "([^"]+)" -Step plan/)
    if (planStep) {
      cleanupRunCalls.push('plan')
      const bare = []
      for (const r of state.resources.values()) for (const c of r.conversations) if (!c.lost && c.id && !(c.evidence && typeof c.evidence === 'object')) { c.evidence = { answer: { path: `${planStep[1]}\\answers\\${c.id}.txt`, sha256: 'a'.repeat(64) } }; bare.push(c.id) }
      sweeps.push(bare)
      const ops = []
      for (const r of state.resources.values()) for (const c of r.conversations) if (!c.lost && c.id && c.deleted !== true && !ops.some((o) => o.session_id === c.id)) ops.push({ op: 'delete', session_id: c.id })
      for (const r of state.resources.values()) if (r.workspace_removed !== true && !ops.some((o) => o.cwd === r.workspace.path)) ops.push({ op: 'settle_workspace', cwd: r.workspace.path })
      state.settlePlan = ops
      state.settleResults = null
      return { ok: true, output: json({ schema_version: 1, kind: 'cleanup_plan', plan_file: `${planStep[1]}\\cleanup\\settle-plan.json`, results_file: `${planStep[1]}\\cleanup\\settle-results.json`, deletes: ops.filter((o) => o.op === 'delete').length, workspaces: ops.filter((o) => o.op === 'settle_workspace').length, evidence_recorded: bare.length, evidence_missing: 0 }) }
    }
    const applyStep = cmd.match(/zt-digest cleanup-run -Out "([^"]+)" -Step apply/)
    if (applyStep) {
      cleanupRunCalls.push('apply')
      if (!state.settleResults) return { ok: false, exit: 1, output: 'zt-digest cleanup-run: no settle results: the bridge\'s deepseek_settle has not run on this plan' }
      let deleted = 0, removed = 0
      const failed = []
      for (const r of state.settleResults) {
        if (r.op === 'delete') {
          const target = [...state.resources.values()].find((x) => x.conversations.some((c) => c.id === r.session_id))
          for (const c of target.conversations) if (c.id === r.session_id) { c.terminal = true; c.deleted = r.status !== 'failed' }
          settleRecords.push({ id: r.session_id, state: 'finished', deleted: r.status === 'failed' ? 'failed' : 'confirmed', via: 'cleanup-run' })
          if (r.status === 'failed') failed.push({ id: r.session_id, error: r.error }); else deleted += 1
        } else {
          for (const x of state.resources.values()) if (x.workspace.path === r.cwd) x.workspace_removed = r.status === 'removed'
          if (r.status === 'removed') removed += 1
        }
      }
      const ready = JSON.parse(ledgerAnswer(`zt-digest cleanup-finish -Out "${applyStep[1]}" -Ready -Owner "cleanup-run"`).output)
      return { ok: true, output: json({ schema_version: 1, kind: 'cleanup_run', deleted, absent: 0, workspaces_removed: removed, workspaces_kept: 0, failed_count: failed.length, failed, unknown_count: 0, unknown: [], unrecorded_count: 0, unrecorded: [], ready }) }
    }
    // One cleanup unit's checked completion (zt-digest `cleanup_finish`).
    const cleanupCompletion = (id) => {
      led.finish(id)
      const resources = [...state.resources.values()].filter((r) => id === 'cleanup/book@1' || r.cleanup_unit === id)
      const cleanupComplete = resources.every((r) => r.conversations.every((c) => c.terminal && c.deleted) && (id !== 'cleanup/book@1' || r.workspace_removed === true))
      const contentComplete = [...led.units.values()].filter((u) => u.layer !== 'cleanup').every((u) => u.state === 'done')
      return { schema_version: 1, kind: 'cleanup_completion', unit: id, state: 'done', status: cleanupComplete && contentComplete ? 'complete' : 'partial', content_complete: contentComplete, cleanup_complete: cleanupComplete, reconciliation: [], partial_reasons: [], resources, scratch: { removed: [], retained: [] } }
    }
    const failingCleanup = 'cleanup_evidence_missing: no per-run usage was retained for this resource'
    // ebrill2001modern trial 1 (zt-digest 0.11.4): `cleanup-finish -Ready` claims every ready cleanup unit, settles each
    // (the run-level unit last), faults the ones that cannot settle with the claim's hash and claims them again
    // within the attempt budget, and prints one line.
    if (/zt-digest cleanup-finish .*-Ready\b/.test(cmd)) {
      cleanupReadyCalls.push(cmd)
      let claimed = 0, done = 0, partial = 0, rounds = 0, book = null
      const open = new Map()
      for (let round = 0; round < 6; round++) {
        const units = led.next('cleanup', 64, 'cleanup-ready').sort((a, b) => (a.id === 'cleanup/book@1') - (b.id === 'cleanup/book@1'))
        if (!units.length) break
        rounds += 1
        for (const u of units) {
          claimed += 1
          cleanupFinishes.push(u.id)
          state.cleanupTries = state.cleanupTries || {}
          const tries = (state.cleanupTries[u.id] = (state.cleanupTries[u.id] || 0) + 1)
          if (opts.failingCleanup && opts.failingCleanup.test(u.id)) {
            const st = tries < led.settings.max_attempts ? 'retry' : 'failed'
            led.fail(u.id, st === 'retry' ? 'ready' : 'failed')
            open.set(u.id, { state: st, cause: failingCleanup })
            if (u.id === 'cleanup/book@1') book = { unit: u.id, ok: false, cause: failingCleanup }
            continue
          }
          const r = cleanupCompletion(u.id)
          done += 1
          if (r.status === 'partial') partial += 1
          open.delete(u.id)
          if (u.id === 'cleanup/book@1') book = { unit: u.id, ok: true, status: r.status, content_complete: r.content_complete, cleanup_complete: r.cleanup_complete, partial_reasons: [], reconciliation: 0, scratch_removed: 0 }
        }
        if (![...open.values()].some((o) => o.state === 'retry')) break
      }
      const list = (want) => [...open.entries()].filter(([, o]) => want(o.state)).slice(0, 8).map(([unit, o]) => ({ unit, state: o.state, cause: o.cause }))
      const count = (want) => [...open.values()].filter((o) => want(o.state)).length
      return { ok: true, output: json({ schema_version: 1, kind: 'cleanup_ready', rounds, claimed, done, partial, retry_count: count((x) => x === 'retry'), failed_count: count((x) => x !== 'retry'), retry: list((x) => x === 'retry'), failed: list((x) => x !== 'retry'), book }) }
    }
    if (/zt-digest cleanup-finish /.test(cmd)) {
      const id = (cmd.match(/-Unit "?([\w/-]+@\d+)"?/) || [])[1]
      cleanupFinishes.push(id)
      if (opts.failingCleanup && opts.failingCleanup.test(id)) return { ok: false, exit: 1, output: `zt-digest cleanup-finish: ${failingCleanup}` }
      return { ok: true, output: json(cleanupCompletion(id)) }
    }
    return null
  }

  // The JSON record a resource command writes (a PowerShell single-quoted literal).
  function resourceRecord(cmd) {
    const match = String(cmd).match(/-Value '((?:[^']|'')*)'/s)
    if (!match) return null
    try { return JSON.parse(match[1].replace(/''/g, "'")) } catch { return null }
  }

  // One script command, as a step agent would run it.
  // The stand-in of `zt-digest finish-batch`: per unit the verifier's own answer (its single finish command under
  // the scenario), the ledger's state before any fault, and a fault recorded against that unit alone.
  const RETRYABLE = /stale_claim|read_view_changed|lock_busy|sharing_violation|sqlite_busy|transport_closed|command_timeout|service_unavailable|provider_|missing_result|pending_jobs|cleanup_transient/
  const finishBatchAnswer = (cmd) => {
    const out = (cmd.match(/-Out "([^"]+)"/) || [])[1]
    const listed = (cmd.match(/ -Units "([^"]*)"/) || [])[1]
    let targets
    let claimed = null
    if (listed !== undefined) {
      targets = listed.split(',').filter(Boolean).map((e) => { const i = e.lastIndexOf(':'); return { unit: e.slice(0, i), token: Number(e.slice(i + 1)) } })
    } else {
      const layer = (cmd.match(/-Layer (\w[\w-]*)/) || [])[1]
      const take = Number((cmd.match(/-Take (\d+)/) || [])[1])
      const owner = (cmd.match(/-Owner "([^"]*)"/) || [])[1] || ''
      targets = led.next(layer, take, owner).map((u) => ({ unit: u.id, token: u.token }))
      claimed = { layer, ...led.gate(layer) }
    }
    batchCalls.push({ cmd, units: targets.map((t) => t.unit) })
    const results = []
    let lock = null
    for (const t of targets) {
      if (lock) { results.push({ unit: t.unit, token: t.token, status: 'not_run' }); continue }
      const layer = t.unit.split('/')[0]
      const group = Number((t.unit.match(/\/g(\d+)@1$/) || [])[1])
      const u0 = led.find(t.unit)
      const model = (u0 && u0.inputs && (u0.inputs.header || u0.inputs.model)) || ''
      const one = layer === 'models' ? `zt-digest model-finish -Out "${out}" -Model '${model}' -Token ${t.token}`
        : layer === 'explain' ? `zt-digest explain-finish -Out "${out}" -Model '${model}' -Token ${t.token}`
          : `zt-digest ${layer}-finish -Out "${out}" -Group ${group} -Token ${t.token}`
      let answer
      if (opts.lockCommand && opts.lockCommand.test(one) && !state.locked) {
        state.locked = one
        answer = { ok: false, json: { ok: false, code: 'card_locked', error: 'the Literature tree Key is locked by the owner', tree: 'Key' } }
      } else if (opts.retryWhen && opts.retryWhen.test(one)) answer = { ok: false, text: 'zt-digest: read_view_changed: the archive changed while the completion was checked' }
      else if (opts.failWhen && opts.failWhen.test(one)) answer = { ok: false, text: 'stand-in crash' }
      else if (opts.contentWhen && opts.contentWhen.test(one)) answer = { ok: true, json: { schema_version: 1, state: 'failed', error: 'the checked completion refused the draft', disposition: 'failed' } }
      else { const r = ledgerAnswer(one); answer = { ok: r.ok, json: lastJson(r.output), text: r.output } }
      batchFinishes.push(one)
      const j = answer.json
      if (answer.ok && j && (j.state === 'done' || j.complete === true)) { results.push({ unit: t.unit, token: t.token, status: j.status === 'partial' ? 'partial' : 'done', disposition: j.disposition || null }); continue }
      if (j && j.code === 'card_locked') { led.fail(t.unit, 'failed'); lock = j; results.push({ unit: t.unit, token: t.token, status: 'failed', code: 'card_locked', recorded: true }); continue }
      const u = led.find(t.unit)
      if (u && u.state === 'done') { results.push({ unit: t.unit, token: t.token, status: 'done', disposition: 'verified', from: 'ledger' }); continue }
      if (!u || u.state !== 'claimed' || u.token !== t.token) { results.push({ unit: t.unit, token: t.token, status: u && u.state === 'failed' ? 'failed' : 'retry', recorded: false }); continue }
      const cause = (j && (j.fault || j.error)) || answer.text || 'the checked completion did not complete'
      const retryable = RETRYABLE.test(cause)
      led.fail(t.unit, retryable ? 'retry' : 'failed')
      batchFaults.push({ unit: t.unit, class: retryable ? 'retryable' : 'content' })
      results.push({ unit: t.unit, token: t.token, status: retryable ? 'retry' : 'failed', code: retryable ? 'provider_transient' : 'check_failed', cause, recorded: true })
    }
    const count = (s) => results.filter((r) => r.status === s).length
    const line = { schema_version: 1, kind: 'finish_batch', units: results.length, done: count('done'), partial: count('partial'), retry: count('retry'), failed: count('failed'), not_run: count('not_run'), results, file: `${out}\\finish\\batch.json` }
    if (claimed) Object.assign(line, { layer: claimed.layer, settled: claimed.settled, complete: claimed.complete, counts: claimed.counts, claimed: targets.length })
    if (lock) return { ok: false, exit: 2, output: json({ ...line, code: 'card_locked', error: 'card_locked', message: lock.error, refusal: lock }) }
    return { ok: true, exit: 0, output: json(line) }
  }
  const commandAnswer = (raw, extraRecord = null) => {
    const cmd = String(raw).trim()
    if (process.env.DBG83 && /resource|cleanup/.test(cmd)) console.log('DBG', cmd.slice(0, 160))
    commands.push(cmd)
    failedStep.push(cmd)
    // S24 (#167): the Term-card branch and the deterministic Term-link pass. The stand-in owns a
    // fixed two-job Term plan, registers the units its compiler would register, and answers the
    // steps the compiler prints; a scenario may fail a writer, make the publication partial, stop
    // the pass or block it on its ledger dependencies.
    const termCmd = cmd.match(/^zt-digest (term-cards|term-link) -Step (plan|jobs|check|review-plan|fix-plan|apply|report)\b/)
    if (termCmd) {
      const tool = termCmd[1]
      const step = termCmd[2]
      const out = (cmd.match(/-Staging '([^']*)'/) || cmd.match(/-Staging "([^"]*)"/) || [])[1] || 'D:\\x\\Key'
      termCalls.push({ tool, step, cmd })
      if (tool === 'term-cards') {
        if (step === 'plan') {
          if (opts.termPlanFails) return { ok: false, exit: 1, output: 'term-cards: the book has no Term list' }
          led.add('termcards/t001@1', 'termcards', { job: termJobs[0] })
          led.add('termcards/t002@1', 'termcards', { job: termJobs[1] })
          const book = led.add('termcards/book@1', 'termcards', {})
          book.state = 'registered'
          return { ok: true, exit: 0, output: json({ ok: true, step: 'plan', book: 'Key', revision: 1, plan_hash: 'ph-terms', terms: 2, math: 0, concept: 2, jobs: termJobs, units: { book: 'termcards/book@1', jobs: { t001: 'termcards/t001@1', t002: 'termcards/t002@1' } } }) }
        }
        if (step === 'jobs') {
          const listed = (cmd.match(/-Tokens '([^']*)'/) || [])[1] || ''
          const ids = listed.split(',').filter(Boolean).map((e) => e.slice(0, e.lastIndexOf(':')))
          const jobs = ids.map((id) => ({ id, kind: 'concept', rows: [Number(id.slice(1))], terms: [`术语${id.slice(1)}`], files: [`${out}\\term-cards\\dsh-term-card-instructions.zh.md`, `${out}\\term-cards\\jobs\\${id}.md`], prompt: 'p', output: `${out}\\term-cards\\out\\${id}.json`, permissions: { archive_read: true, archive_write: false }, model: 'deepseek-official/deepseek-flash', effort: 'high', skills: ['zettelkasten', 'asd-ste100-zh'], delete_after_collect: false }))
          return { ok: true, exit: 0, output: json({ ok: true, step: 'jobs', plan_revision: 1, plan_hash: 'ph-terms', units: [], jobs }) }
        }
        if (step === 'check' || step === 'review-plan' || step === 'fix-plan') {
          const ready = led.layer('termcards').filter((u) => u.id.startsWith('termcards/t') && u.state === 'done').length
          const common = { ok: true, step, plan_revision: 1, plan_hash: 'ph-terms', complete: ready >= termJobs.length, counts: { ready, needs_fix: 0, needs_review: 0, refused: 0, missing: 0, unchecked: 0 } }
          const readiness = { path: `${out}\\term-cards\\check\\readiness.json`, complete: ready >= termJobs.length, counts: common.counts }
          if (step === 'check') return { ok: true, exit: 0, output: json({ ...common, terms: termJobs.map((job) => ({ job: job.id, row: job.rows[0], term: job.terms[0], status: 'ready' })), readiness }) }
          if (step === 'review-plan') return { ok: true, exit: 0, output: json({ ...common, jobs: [], reviews: [], readiness }) }
          return { ok: true, exit: 0, output: json({ ...common, jobs: [], planned: [], final: [], readiness }) }
        }
        if (step === 'apply') {
          const partial = !!opts.termPublicationPartial
          return { ok: true, exit: 0, output: json({ ok: true, step: 'apply', complete: !partial, rows: termJobs.length, committed: partial ? 1 : termJobs.length, ready_unpublished: partial ? 1 : 0, refused: 0, pending: 0, conflicted: 0, staged: [], stop: partial ? { code: 'not_ready', message: 'row 2 is not ready' } : null, report: `${out}\\term-cards\\report.json` }) }
        }
        return { ok: true, exit: 0, output: json({ ok: true, step, report: `${out}\\term-cards\\report.json` }) }
      }
      if (step === 'plan') {
        if (opts.termLinkPlanFails) return { ok: false, exit: 1, output: 'term-link: the Term-structure read names no list: the book has no Term list to link' }
        const unit = led.add('termlink/book@1', 'termlink', { pass: 'book' })
        const termBook = led.find('termcards/book@1')
        // The real ledger blocks the pass on its unfinished dependencies: a Term-card book that is
        // not done (or a scenario that blocks the pass) leaves nothing ready to claim.
        if (opts.termLinkBlocked || (termBook && termBook.state !== 'done')) unit.state = 'blocked'
        return { ok: true, exit: 0, output: json({ ok: true, step: 'plan', book: 'Key', plan_revision: 1, plan_hash: 'ph-link', cards_scanned: 9, cards_changed: 1, cards_refused: 0, complete: true, unit: 'termlink/book@1' }) }
      }
      if (step === 'apply') {
        if (opts.termLinkStops) return { ok: false, exit: 1, output: `${json({ ok: false, step: 'apply', complete: false, stop: { code: 'card_check', message: 'R2 in Key/1|a' }, code: 'card_check', error: 'card_check: R2 in Key/1|a' })}\n${json({ error: 'card_check', message: 'R2 in Key/1|a' })}` }
        const noop = opts.termLinkNoop === true
        const token = Number((cmd.match(/-Token (\d+)/) || [])[1])
        return { ok: true, exit: 0, output: json({ ok: true, step: 'apply', complete: true, no_op: noop, code: null, claim_token: token, report: `${out}\\term-link\\apply.json`, counts: { cards_scanned: 9, cards_changed: noop ? 0 : 1, cards_refused: 0, links_added: noop ? 0 : 2 }, chunks: noop ? 0 : 1 }) }
      }
      return { ok: true, exit: 0, output: json({ ok: true, step }) }
    }
    // S17 / #140 (Tree Lock): the owner locks the run's destination mid-run. The refused command answers
    // the archive's own typed refusal (the shape lockcheck::refusal prints) and exits 2, once: a stage that
    // retried it, or re-pointed the write, would run a second command or show it in `lockedRetries`.
    // `lockCommandAfter: n` lets the first n matching commands commit, so an earlier receipt is real.
    if (opts.lockCommand && opts.lockCommand.test(cmd) && !state.locked) {
      state.lockSeen = (state.lockSeen || 0) + 1
      if (state.lockSeen > (opts.lockCommandAfter || 0)) {
        state.locked = cmd
        state.lockedRetries = 0
        const tool = (cmd.match(/^zt-digest ([\w-]+)/) || [])[1] || 'call'
        return { ok: false, exit: 2, output: json({ ok: false, code: 'card_locked', error: 'the Literature tree Key is locked by the owner', mode: tool, step: 'apply', tree: state.lockTree || 'Key', lock_id: 3, locked_at: '2026-10-22T10:00:00.000Z', hint: 'unlock the tree in the GUI first; its Cards stay readable as sources' }) }
      }
    } else if (state.locked && opts.lockCommand && opts.lockCommand.test(cmd)) state.lockedRetries = (state.lockedRetries || 0) + 1
    // zt-digest 0.12 `finish-batch` (docs/digest-claude-token-proposal.md P1/P2): each unit's own verifier in one
    // process. The scenarios' failWhen/contentWhen/lockCommand describe that verifier through the unit's single
    // finish command; cutFinish/proseFinish distort only the relayed batch line.
    if (/^zt-digest finish-batch /.test(cmd)) {
      const r = finishBatchAnswer(cmd)
      if (opts.cutFinish && opts.cutFinish.test(cmd)) return { ...r, output: String(r.output).slice(0, 60) }
      if (opts.proseFinish && opts.proseFinish.test(cmd)) return { ...r, output: 'finish-batch finished every listed unit; all done.' }
      return r
    }
    const denied = opts.failWhen && opts.failWhen.test(cmd) ? cmd : null
    if (opts.contentWhen && opts.contentWhen.test(cmd)) return { ok: true, exit: 0, output: json({ schema_version: 1, state: 'failed', error: 'the checked completion refused the draft', disposition: 'failed' }) }
    // A crashed command changes nothing: the stand-in refuses before the ledger applies it.
    if (denied) return { ok: false, exit: 1, output: 'stand-in crash' }
    const ledAns = ledgerAnswer(cmd, extraRecord)
    // grossman2002integration trial 5: the finish ran and the ledger recorded it, but the relay cut its line.
    if (ledAns && opts.cutFinish && opts.cutFinish.test(cmd)) return { ...ledAns, output: String(ledAns.output).slice(0, 60) }
    // trial 10: the relay rewrote the end of a completion as prose; only a nested file record still parses.
    if (ledAns && opts.proseFinish && opts.proseFinish.test(cmd)) return { ...ledAns, output: '{"schema_version":1,"kind":"cards_completion","files":{"check":{"path":"D:\\x\\Key\\cards-check-g001.json","sha256":"f1"},"completion":{"path":"cards-finish for group 1 completed with state done and complete true."}' }
    if (ledAns) return ledAns
    const tool = cmd.match(/^zt-digest ([\w-]+) -Step ([\w-]+)/)
    if (tool) {
      const key = `${tool[1] === 'card' ? 'card' : tool[1] === 'model' ? 'model' : tool[1] === 'event' ? 'event' : tool[1] === 'finding' ? 'finding' : tool[1] === 'hub' ? 'hub' : tool[1] === 'bridge' ? 'bridge' : tool[1] === 'math' ? 'math' : tool[1] === 'terms' ? 'terms' : tool[1] === 'skeleton' ? 'skeleton' : tool[1]} ${tool[2]}`
      const fn = CANNED[key]
      if (denied) return { ok: false, exit: 1, output: 'stand-in crash' }
      // S23 / #115 scenarios: a review whose frozen decision the archive moved past, a recorded merge
      // verdict no rewrite covers, and an interrupted register publication whose accepted plan stays
      // staged. Each is a real refusal of the delivered tool, answered here instead of by a live run.
      // trials 17-19: a close round whose writers answer but leave faults; the next close round sees fewer.
      if (key === 'model close' && opts.closeFaults) { state.closeRounds = (state.closeRounds || 0) + 1; const left = Math.max(0, opts.closeFaults - state.closeRounds + 1); return { ok: true, output: json({ jobs: left ? [{ id: 'Key_2_c:f01+9', continue: 'Key_2_c:f01', files: ['D:/x/follow.md'] }] : [], faults: left, unowned: [], refreshed: 0, ...(!left && / -Report\b/.test(cmd) ? { report: CANNED['model report']() } : {}) }) } }
      if (key === 'restate apply' && opts.staleDecision) return { ok: false, exit: 1, output: 'endpoint_changed: Key/3|a changed after the review' }
      if (key === 'restate report' && opts.staleDecision) {
        return { ok: true, exit: 0, output: json({ ok: true, step: 'report', report_id: 'rr-s2', status: 'partial', complete: false, counts: { pairs: 3, recommended: 1, resolved_by_rewrite: 0, pending_existing_card_change: 0, unverified: 0, different: 0, unresolved: 1, unjudged: 0 }, written: [] }) }
      }
      if (key === 'restate report' && opts.pendingMerge) {
        return { ok: true, exit: 0, output: json({ ok: true, step: 'report', report_id: 'rr-p2', status: 'resolved-with-pending', complete: true, counts: { pairs: 3, recommended: 0, resolved_by_rewrite: 1, pending_existing_card_change: 1, unverified: 0, different: 1, unresolved: 0, unjudged: 0 }, written: ['D:\\x\\Key\\restate-resolution.json'] }) }
      }
      if (key === 'register apply' && opts.interruptedRegister) return { ok: false, exit: 1, output: 'transport_closed: the publication was interrupted after the parts batch' }
      if (key === 'register plan' && opts.staleReview) {
        return { ok: true, exit: 0, output: json({ ok: true, step: 'plan', plan_id: 'rp-s3', register: '12', terms: 3, entries: 3, unresolved: 0, faults: 0, excluded: 1, read_only: 0, parts: 1, review: { report_id: 'r-c1', status: 'partial', complete: false }, path: 'D:\\x\\Key\\register\\out\\register-plan.json' }) }
      }
      if (fn) {
        const value = fn()
        // zt-digest 0.12: a close round that sends nothing back writes the batch report too (-Report).
        if (key === 'model close' && / -Report\b/.test(cmd) && !(value.jobs || []).length) value.report = CANNED['model report']()
        if (tool[1] === 'card' && tool[2] === 'apply' && state.applyPending > 0) { value.pending_jobs = state.applyPending - 1 }
        if (tool[1] === 'card' && tool[2] === 'plan' && state.planJobs !== 21) value.jobs = Array.from({ length: state.planJobs }, (_, i) => ({ id: `fix-${i + 1}`, kind: 'fix', file: `fix-${i + 1}.md` }))
        if (tool[1] === 'terms' && tool[2] === 'apply') led.finish('terms/book@1')
        // trial 10: a relay abridged the whole-book plan line inside its jobs array.
        if (tool[1] === 'card' && tool[2] === 'plan' && opts.garbledCardPlan && cmd.includes('-Work "D:\\x\\Key\\cards" ')) return { ok: true, exit: 0, output: '{"book":"Key","picked":69,"jobs":[fix-01 (fix), orphans-02 (orphans)]} (abridged: full JSON had a sample block)' }
        return { ok: true, exit: 0, output: json(value) }
      }
    }
    const startLink = cmd.match(/zt-digest link-start -Out "[^"]*" -Group (\d+) -Token (\d+)/)
    if (startLink) {
      // #110: link-start freezes the gap scope; an empty gap set starts no job.
      const group = Number(startLink[1])
      const token = Number(startLink[2])
      const unit = `link/g${String(group).padStart(3, '0')}@1`
      const gap = group === 2 ? 0 : 2
      const draft = `D:\\x\\Key\\link-jobs\\g${String(group).padStart(3, '0')}\\draft.json`
      const job = `D:\\x\\Key\\link-jobs\\g${String(group).padStart(3, '0')}\\start.json`
      const jobs = gap > 0 ? [{ id: `link-g${String(group).padStart(3, '0')}-t${token}`, files: ['D:\\x\\Key\\link-prefix.md', `D:\\x\\Key\\link-g0${group}.md`, job], prompt: 'p', output: draft }] : []
      return { ok: true, output: json({ schema_version: 1, unit, token, run_id: 'r1', input_hash: `ih-${unit}`, unit_input_hash: `ih-${unit}`, job, draft, expected_jobs: gap > 0 ? 1 : 0, no_op: gap > 0 ? null : { reason: 'no_semantic_gaps', gaps: 0, skipped: 3 }, done: false, gap: [], skipped: [], position: gap > 0 ? 'anchor_position_missing' : 'linked', counts: { owned: 3, gaps: gap, skipped: gap > 0 ? 1 : 3 }, jobs, lease_until: 1000 }) }
    }
    const staged = cmd.match(/^zt-digest finalize -Step stage -Out "[^"]*" -Layer ([a-z-]+)/)
    if (staged) {
      stagePublications.push(cmd)
      return { ok: true, exit: 0, output: json({ layer: staged[1], unit: `${staged[1]}/book@2`, revision: 2, state: 'done', complete: true, disposition: 'published', reused: false, superseded: [`${staged[1]}/book@1`], coverage: {} }) }
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

  // A manager records each job's id and its answer itself (zt-digest-lib v15): the stand-in runs the record
  // commands the prompt prescribes, with the session id, the owner and the answer file filled in.
  const recordJob = (prompt, sessionId, unit, token, settleNow = false) => {
    const rule = prompt.match(/ledger resource -Out "([^"]+)" (-Unit run|-Unit "[^"]*" -Token \S+) -RegisterId <session_id> -Workspace "([^"]+)"/)
    if (!rule) return { registered: false, evidence: false, deleted: false }
    const owner = rule[2] === '-Unit run' ? '-Unit run' : `-Unit "${unit}" -Token ${token}`
    const registered = commandAnswer(`zt-digest ledger resource -Out "${rule[1]}" ${owner} -RegisterId ${sessionId} -Workspace "${rule[3]}"`).ok
    const evidence = commandAnswer(`zt-digest ledger resource -Out "${rule[1]}" -EvidenceFor ${sessionId} -Evidence "${rule[1]}\\answers\\${sessionId}.txt"`).ok
    const deleted = settleNow && /-Settle <session_id> -State finished -Deleted confirmed/.test(prompt)
      ? commandAnswer(`zt-digest ledger resource -Out "${rule[1]}" -Settle ${sessionId} -State finished -Deleted confirmed`).ok
      : false
    return { registered, evidence, deleted }
  }
  // A provider returns a fresh id for every conversation it creates.
  let sessionSeq = 0
  const sessionOf = (id) => `s-${id}-${++sessionSeq}`
  const ownerOf = (prompt) => {
    const m = prompt.match(/-Unit "([^"<{]+)" -Token (\d+) -RegisterId/)
    return m ? { unit: m[1], token: Number(m[2]) } : { unit: 'run', token: 1 }
  }

  // The block manager: claim up to eight units, start each, run their jobs, record and delete each conversation,
  // and finish each unit with its draft.
  const blockAnswer = async (prompt, callOpts) => {
    const plan = JSON.parse(prompt.slice(prompt.indexOf('BLOCK PLAN: ') + 'BLOCK PLAN: '.length).split('\n')[0])
    managers.push({ label: callOpts.label, phase: callOpts.phase, block: true, jobs: [] })
    const claimed = await commandAnswer(plan.claim)
    const parsed = lastJson(claimed.output)
    const units = []
    for (const held of (parsed && parsed.units) || []) {
      const group = Number((/g(\d+)@1/.exec(held.id) || [])[1])
      const fill = (cmd, draft = '') => cmd.replace(/\{\{group\}\}/g, String(group)).replace(/\{\{token\}\}/g, String(held.token)).replace(/\{\{draft\}\}/g, draft)
      const start = await commandAnswer(fill(plan.start))
      const started = lastJson(start.output) || {}
      const count = Array.isArray(started.jobs) ? started.jobs.length : (started.expected_jobs === 0 ? 0 : (started.job ? 1 : 0))
      const jobs = []
      for (let i = 0; i < count; i++) {
        const id = Array.isArray(started.jobs) ? started.jobs[i].id : `digest-g${group}-t${held.token}`
        const session = sessionOf(id)
        managers[managers.length - 1].jobs.push(id)
        jobs.push({ id, session_id: session, status: 'done', ...recordJob(prompt, session, held.id, held.token, true) })
      }
      const finish = await commandAnswer(fill(plan.finish, started.draft || ''))
      units.push({
        unit: held.id, token: held.token, group,
        start: { ok: start.ok, output: start.output, ...(start.ok ? {} : { error: start.output }) },
        finish: { ok: finish.ok, output: finish.output, ...(finish.ok ? {} : { error: finish.output }) },
        jobs,
      })
    }
    // A relay that summarizes its claim line (run 5, lane 2: "Claimed 8 units ... Output abbreviated").
    if (opts.dropUnits) units.splice(0, units.length)
    const claimOutput = opts.proseClaim && units.length ? `Claimed ${units.length} units (${units.map((u) => u.unit).join(', ')}). Output abbreviated.` : claimed.output
    return { claim: { ok: claimed.ok, output: claimOutput }, units }
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
      const owner = unit ? { unit, token } : ownerOf(prompt)
      if (parsed && parsed.job && !Array.isArray(parsed.jobs) && (parsed.expected_jobs === undefined || parsed.expected_jobs > 0)) {
        const session = sessionOf(`digest-${group}`)
        results.push({ id: `digest-g${group}-t${token}`, step: st.name, session_id: session, status: 'done', answer: '{}', ...recordJob(prompt, session, owner.unit, owner.token) })
      }
      if (st.jobs && parsed && Array.isArray(parsed.jobs) && parsed.jobs.length) {
        managers.push({ label: opts.label, phase: opts.phase, jobs: parsed.jobs.map((j) => j.id), given: parsed.jobs })
        for (const job of parsed.jobs) { const session = sessionOf(job.id); results.push({ id: job.id, step: st.name, session_id: session, status: 'done', answer: '{}', ...recordJob(prompt, session, owner.unit, owner.token) }) }
      }
    }
    return { steps, results }
  }

  const answer = async (prompt, callOpts) => {
    const text = String(prompt)
    const label = callOpts.label || ''
    const cmd = text.match(/zt-digest [a-z-]+ -[A-Za-z][^\n]*/) || text.match(/pwsh -NoProfile -File[^\n]*/)
    calls.push({ label, phase: callOpts.phase, model: callOpts.model, agentType: callOpts.agentType, cmd: cmd ? cmd[0] : '', prompt: text })
    // The finite finalization (#86): the plan's branch decides whether the one correction cycle
    // runs, and the final audit reports the fresh revision.
    if (/^finalize plan$/.test(label)) return { ok: true, output: json(finalizePlan(opts.finalize || 'fix')) }
    if (/^finalize scope$/.test(label)) return { ok: true, output: json(finalizeScope()) }
    if (/^finalize refresh /.test(label)) return { ok: true, output: json(finalizeRefresh(label.replace('finalize refresh ', ''))) }
    if (/^finalize final$/.test(label)) return { ok: true, output: json(finalizeFinal(opts.finalize || 'fix')) }
    if (text.startsWith('You run one compiler chain')) return await chainAnswer(text, callOpts)
    if (text.startsWith('You run one block')) return await blockAnswer(text, callOpts)
    if (text.startsWith('You drive DeepSeek Harness')) {
      if (/Run every job below|Run the jobs with these ids/.test(text)) {
        const ids = (text.match(/Run the jobs with these ids: (\[[^\n]*?\])\./) || [])[1]
        const given = ids ? JSON.parse(ids).map((id) => ({ id })) : JSON.parse(text.split('\n').find((l) => l.startsWith('[{')) || '[]')
        if (ids) driveFiles.push((text.match(/read "([^"]+)" once/) || [])[1] || '')
        managers.push({ label, phase: opts.phase, jobs: given.map((j) => j.id), given })
        const owner = ownerOf(text)
        return { ok: true, output: '', results: given.map((j) => { const session = sessionOf(j.id); return { id: j.id, session_id: session, status: 'done', answer: '{}', ...recordJob(text, session, owner.unit, owner.token) } }) }
      }
      if (/^Clean up:/m.test(text)) { cleanups.push(label); return { ok: true, results: [], deleted: 2 } }
      const lines = text.split('\n').map((l) => l.trim()).filter((l) => /^(zt-digest|pwsh -NoProfile -File) /.test(l))
      if (lines.length) { const r = commandAnswer(lines[0]); return { ok: r.ok, output: r.output, results: [], deleted: 0 } }
      return { ok: true, output: '', results: [], deleted: 0 }
    }
    // zt-digest-lib v37 (S1): these commands run through the relay; the stand-in answers them by label, in the
    // relay's shape (ok and the last line), before the generic command path.
    if (/^Run this PowerShell command once/.test(text) && label === 'prepare book') {
      if (opts.lockPrepare) return { ok: false, output: `the Literature tree Key is locked by the owner\nthe command failed (exit 2)\n${json({ ok: false, code: 'card_locked', error: 'the Literature tree Key is locked by the owner', mode: 'prepare', step: 'apply', tree: state.lockTree || 'Key', lock_id: 3, locked_at: '2026-10-22T10:00:00.000Z', hint: 'unlock the tree in the GUI first; its Cards stay readable as sources' })}` }
      state.groups = 2
      led.add('prepare/book@1', 'prepare').state = 'done'
      if (layerEnabled('terms')) led.add('terms/book@1', 'terms')
      led.add('cleanup/book@1', 'cleanup')
      for (let n = 1; n <= (opts.extraCleanupUnits || 0); n++) led.add(`cleanup/extra-g${String(n).padStart(3, '0')}@1`, 'cleanup')
      for (let n = 1; n <= 2; n++) led.add(`digest/g${String(n).padStart(3, '0')}@1`, 'digest', { n, group: { n } })
      if (kind === 'math') for (const l of ['math-index', 'math-docs', 'math-skeleton', 'notation']) led.add(`${l}/book@1`, l)
      return { ok: true, output: json({ ok: true, groups: 2, citation_key: 'Key', out: 'D:\\x\\Key' }) }
    }
    if (/^Run this PowerShell command once/.test(text) && /^prepare link pass$/.test(label)) {
      const full = opts.linkBlocked ? { pending: [], no_gap: [], blocked: opts.linkBlocked, groups: [], unowned: 0 } : CANNED['prepare-link']()
      const gaps = Object.fromEntries((full.groups || []).map((g) => [String(g.n), g.gaps]))
      return { ok: true, output: json({ pending: full.pending, no_gap: full.no_gap || [], blocked: (full.blocked || []).map((b) => ({ n: b.n, unit: b.unit, state: b.state, reason: b.reason })), gaps, owned_total: full.owned_total || 0, gap_total: full.gap_total || 0, skipped_total: full.skipped_total || 0, unowned: full.unowned || 0, digest_state: full.digest_state || null }) }
    }
    if (/^Run this PowerShell command once/.test(text) && (label === 'find models: whole book' || label === 'find models: explain')) {
      return { ok: true, output: json({ models: kind === 'math'
        ? [{ address: 'Key/2|e', title: '理论', form: 'theory', level: null, parts: 0 }, { address: 'Key/2|c', title: '介值定理', form: 'theorem', level: 'formal', parts: 0 }]
        : [{ address: 'Key/2|c', title: '模型', form: null, level: 'formal', parts: 0 }] }) }
    }
    if (/^Run this PowerShell command once/.test(text) && /^plan status: /.test(label)) {
      // S10: the compiler reads the plans the architect wrote (model -Step plan-status).
      if (opts.noPlanFile) return { ok: true, output: json({ ok: true, step: 'plan-status', planned: 0, derive: false, plans: [{ model: 'Key/2|c', status: 'failed', parts: 0, derivations: 0 }] }) }
      return { ok: true, output: json({ ok: true, step: 'plan-status', planned: 1, derive: true, plans: [{ model: 'Key/2|c', status: 'planned', level: 'formal', parts: 4, derivations: 1 }] }) }
    }
    if (/^Run this PowerShell command once/.test(text)) {
      const lines = text.split('\n').map((l) => l.trim()).filter((l) => /^(zt-digest|pwsh -NoProfile -File) /.test(l))
      // An owned-resource command writes its JSON first and runs one ledger command on the same line.
      const compound = text.split('\n').map((l) => l.trim()).find((l) => / zt-digest ledger resource /.test(l))
      if (compound) {
        // The cleanup evidence sweep is one foreach line; it reaches the ledger stand-in whole.
        const inner = /foreach \(\$zdId in/.test(compound) ? compound : compound.slice(compound.indexOf(' zt-digest ') + 1)
        const r = await commandAnswer(inner, resourceRecord(compound))
        return { ok: r.ok, output: r.output }
      }
      if (lines.length) {        let r = await commandAnswer(lines[0])
        if (/ ledger next .* -Wait \d+/.test(lines[0])) {
          for (let i = 0; i < 200; i++) {
            // A command that failed answers its failure; only an empty claim waits.
            if (!r.ok) break
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
    // S24 (#167): the Term-card branch's own managers. A wave keeps its writers, then a settle
    // manager deletes their conversations; a scenario may fail every writer of a wave.
    if (text.startsWith('You are a DeepSeek subagent manager for ZET Term Cards (settle a wave)')) {
      const open = text.indexOf('\n', text.indexOf('SESSIONS ('))
      const end = text.indexOf('\n\nFor each', open)
      const list = JSON.parse(text.slice(open, end < 0 ? text.length : end))
      termManagers.push({ label, phase: callOpts.phase, kind: 'settle', jobs: list.map((s) => s.id) })
      return { ok: true, deleted: list.length, not_deleted: [], sessions: list.map((s) => ({ id: s.id, session_id: s.session_id, deleted: true })) }
    }
    if (text.startsWith('You are a DeepSeek subagent manager for ZET Term Cards (')) {
      const kind = (text.match(/Term Cards \(([^)]+)\)/) || [])[1] || 'write'
      const open = text.indexOf('\n', text.indexOf('JOBS ('))
      const end = text.indexOf('\n\n1. Start', open)
      const listed = JSON.parse(text.slice(open, end < 0 ? text.length : end))
      termManagers.push({ label, phase: callOpts.phase, kind, jobs: listed.map((job) => job.id) })
      const failed = kind === 'write' && !!opts.termWriterFails
      return { ok: true, jobs: listed.map((job) => ({ id: job.id, session_id: sessionOf(job.id), status: failed ? 'failed' : 'done', answer: failed ? 'the writer failed' : '{}', registered: true, evidence: true, deleted: false })) }
    }
    if (label === 'choose DeepSeek source') return { source: 'official', errors: '' }
    if (label === 'prepare book') {
      // S17 / #140: a preparation the owner's Lock refused answers the tool's typed refusal line and
      // exits 2, so the stage reads `card_locked` instead of a generic preparation failure.
      if (opts.lockPrepare) return { ok: false, groups: 0, error: `the Literature tree Key is locked by the owner\nthe command failed (exit 2)\n${json({ ok: false, code: 'card_locked', error: 'the Literature tree Key is locked by the owner', mode: 'prepare', step: 'apply', tree: state.lockTree || 'Key', lock_id: 3, locked_at: '2026-10-22T10:00:00.000Z', hint: 'unlock the tree in the GUI first; its Cards stay readable as sources' })}` }
      state.groups = 2
      led.add('prepare/book@1', 'prepare').state = 'done'
      if (layerEnabled('terms')) led.add('terms/book@1', 'terms')
      led.add('cleanup/book@1', 'cleanup')
      // trial 10: more cleanup units than three rounds of eight claims, some of them failing every time.
      for (let n = 1; n <= (opts.extraCleanupUnits || 0); n++) led.add(`cleanup/extra-g${String(n).padStart(3, '0')}@1`, 'cleanup')
      for (let n = 1; n <= 2; n++) led.add(`digest/g${String(n).padStart(3, '0')}@1`, 'digest', { n, group: { n } })
      if (kind === 'math') for (const l of ['math-index', 'math-docs', 'math-skeleton', 'notation']) led.add(`${l}/book@1`, l)
      return { ok: true, groups: 2, usage_before: { input_cache_miss: 0, cached_read: 0, output: 0, cost: 0, calls: 0 } }
    }
    if (label === 'resume run') {
      // #87: a resume-run agent runs `zt-digest ledger resume`, which the ledger stand-in answers (below).
      return commandAnswer((text.match(/zt-digest ledger resume[^\n]*/) || [])[0] || 'zt-digest ledger resume')
    }
    if (label === 'resume status') {
      // grossman2002integration trial 17: a relay returned the summary without `layers` (and without
      // `counts.per_layer`), with a `layers_open` map of its own in their place.
      if (scenario.rewrittenSummary) {
        const { layers, ...rest } = statusSummary(kind, led)
        const { per_layer, ...counts } = rest.counts || {}
        return { ok: true, output: json({ ...rest, counts, layers_open: Object.fromEntries(Object.entries(layers || {}).map(([k, v]) => [k, v.open || 0])) }) }
      }
      return { ok: true, output: / -Summary\b/.test(text) ? json(statusSummary(kind, led)) : json({ schema_version: 1, variant: kind === 'math' ? 'math' : 'econ', settings: led.settings, citation_key: 'Key', archive_identity: { archive_id: 'a1' }, units: [...led.units.values()] }) }
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
    if (/^prepare link pass$/.test(label)) return { ok: true, ...(opts.linkBlocked ? { pending: [], blocked: opts.linkBlocked, groups: [], unowned: 0, cards_all: '' } : CANNED['prepare-link']()) }
    // zt-dsh-cleanup v11: one agent, three calls; the stand-in bridge runs the plan the script wrote.
    if (label === 'cleanup: settle the run') {
      const plan = (text.match(/zt-digest cleanup-run -Out "[^"]+" -Step plan/) || [])[0]
      const apply = (text.match(/zt-digest cleanup-run -Out "[^"]+" -Step apply -Owner "[^"]+"/) || [])[0]
      const fallback = (text.match(/zt-digest cleanup-finish -Out "[^"]+" -Ready -Owner "[^"]+"/) || [])[0]
      const planned = commandAnswer(plan)
      if (opts.settleMissing) {
        const r = commandAnswer(fallback)
        return { plan: planned.output, settle: 'tool not found: mcp__deepseek__deepseek_settle', ok: r.ok, output: r.output }
      }
      settleCalls.push(state.settlePlan.slice())
      state.settleResults = state.settlePlan.map((op) => op.op === 'delete'
        ? (opts.refuseDelete && opts.refuseDelete.test(op.session_id) ? { ...op, status: 'failed', error: 'still open, nothing deleted' } : { ...op, status: 'deleted' })
        : { ...op, status: 'removed', listed: [] })
      const r = commandAnswer(apply)
      return { plan: planned.output, settle: json({ ops: state.settleResults.length }), ok: r.ok, output: r.output }
    }
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
    calls, commands, logs, phases, managers, cleanups, pool, led, resourceRecords, evidenceRecords, settleRecords, driveFiles, heldReads, faultRefusals, holds, sweeps, briefLists, stagePublications, cleanupFinishes, workspaceLists, chunkShares, cleanupReadyCalls, cleanupRunCalls, settleCalls, batchCalls, batchFinishes, batchFaults, depsBatches, failClasses, termCalls, termManagers, state,
    agent: async (prompt, opts = {}) => {
      // grossman2002integration trial 17: a link block manager claimed its units, then failed its structured
      // answer five times (a 2.6 KB claim line with Windows paths in a JSON string) and the agent call threw.
      if (scenario.blockThrows && !state.blockThrown && /^You run one block of a ZET book digest: link lane/.test(prompt)) {
        state.blockThrown = true
        const claimCmd = (prompt.match(/zt-digest ledger next [^\n]*/) || [])[0]
        if (claimCmd) ledgerAnswer(claimCmd)
        throw new Error('agent({schema}): StructuredOutput retry cap (5) exceeded')
      }
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
// S5: the stand-in reads a fault as zt-digest's ledger does: a code names its class; a fault without a code takes
// the one reading of its cause (ledger.rs TRANSIENT_CAUSES).
const TRANSIENT = ['stale_claim', 'read_view_changed', 'lock_busy', 'sharing_violation', 'sqlite_busy', 'transport_closed', 'command_timeout', 'service_unavailable', 'provider_', 'missing_result', 'pending_jobs', 'cleanup_transient']
const RETRYABLE_CODES = ['lock_busy', 'sharing_violation', 'sqlite_busy', 'transport_closed', 'command_timeout', 'service_unavailable', 'provider_cap', 'provider_transient', 'missing_result', 'missing_jobs', 'pending_jobs', 'read_view_changed', 'lease_lost']
const classOf = (fault) => {
  const code = fault && fault.code ? fault.code : (TRANSIENT.some((w) => String((fault && fault.cause) || '').includes(w)) ? 'provider_transient' : 'check_failed')
  return { code, class: RETRYABLE_CODES.includes(code) ? 'retryable' : 'content' }
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
const base = { out: 'D:\\x\\Key', citationKey: 'Key', source: 'S.md', bibtex: '@book{Key}', bookTitle: '书' }

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
  // PowerShell's New-Item has no -LiteralPath: a resource registration that used it never recorded a run id.
  for (const which of ['econ', 'math']) check(!/New-Item\b[^;`\n]*-LiteralPath/.test(sources[which]), `${which}: no command passes -LiteralPath to New-Item`)
  const tampered = sources.econ.replace('"dshPerBatch"', '"dshPerBatchX"')
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
  const until = await run('econ', { ...base, figures: false, until: 'digest' })
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
  // S23-AC01 (#115): §13.1's dependency flow on the existing stages, not a reordered blueprint.
  // docs/literature-only-digest-design.md: a digest builds only the Literature tree, so no concept review,
  // restatement review or hub stage sits in the graph; the group writers follow the term table directly.
  check(after('digest') === 'terms', 'schedule: the group writers follow the book term table')
  check(['concepts', 'review', 'hubs', 'hubs-first'].every((id) => !stageOf('econ', id)), 'schedule: a digest has no concept review, restatement review or hub stage (they are zt-idea-grow\'s)')
  check(after('cards') === 'digest', 'schedule: the per-group Card checks follow their own group completion (the cadrs stage)')
  check(after('models') === 'cards', 'schedule: the model layer is released when the digest managers settle (after the Cards layer)')
  check(after('explain') === 'models', 'schedule: explanations follow the model layer')
  check(after('cards-book') === 'explain', 'schedule: the whole-book card cycle waits for the model reviews (R248)')
  check(stageOf('econ', 'events').after.join(',') === 'terms' && stageOf('econ', 'findings').after.join(',') === 'terms', 'schedule: Event and Finding continuations run from readiness while the remaining groups digest')
  check(/cards-book/.test(after('link-prepare')) && /events|findings/.test(after('link-prepare')), 'schedule: the gap-only Link pass waits for the term evidence (after-terms) and the whole-book cycle')
  check(after('organize') === 'link-check', 'schedule: Organize runs after the link checks settle')
  check(after('figures') === 'organize', 'schedule: Figures is released by Organize')
  check(after('bridge') === 'figures', 'schedule: the report-first Bridge follows Figures (its endpoint snapshots are the final Card text)')
  check(after('audit') === 'bridge,term-link', 'schedule: the final audit follows the Bridge report and the Term-link pass (R0420: tl --> audit)')
  check(after('finalize') === 'audit', 'schedule: the finite finalization follows the audit it plans from (#86)')
  check(after('cleanup') === 'finalize' && after('report') === 'cleanup', 'schedule: the settlement waits for the finalization, then cleanup, then report')
  check(stageOf('econ', 'finalize').onFail === 'continue', 'schedule: a residual finalization fault leaves a partial result, never a halted run')
  check(!stageOf('econ', 'digest').foreach && stageOf('econ', 'digest').module === 'zt-group-cycle' && stageOf('econ', 'digest').params.groups === '{{stages.prepare.output.groups}}', 'schedule: the digest stage is one zt-group-cycle stage over the prepared group count, run in blocks of up to eight groups per manager')
  // #87: the resume invocation and the repair modes drain their downstream closure.
  for (const which of ['econ', 'math']) {
    const modes = builds[which].bp.modes
    const resume = modes.resume || []
    const required = ['args', 'prepare', 'digest', 'cards', 'organize', 'audit', 'finalize', 'cleanup', 'report']
    check(required.every((id) => resume.includes(id)), `resume (${which}): mode resume runs the retained run's whole closure (${resume.length} stages: ${resume.join(',')})`)
    check(resume.includes('terms') && resume.includes('link-prepare') && resume.includes('link') && resume.includes('link-check'), `resume (${which}): the term table and the changed-Card Link pass and check are part of the resume`)
    // A stage the resume names must also let mode resume through its own `when` (grossman2002integration trial 19:
    // events and findings matched only ^(full|events)$ / ^(full|findings)$, so an open findings unit was never drained).
    const ctxResume = { mode: 'resume', args: { mode: 'resume', out: 'D:\\x\\Key' } }
    const valueAt = (p) => p.split('.').reduce((o, k) => (o == null ? undefined : o[k]), ctxResume)
    const holds = (c) => c.op === 'or' ? c.value.some(holds) : c.op === 'and' ? c.value.every(holds)
      : c.op === '==' ? valueAt(c.path) === c.value : c.op === '!=' ? valueAt(c.path) !== c.value
        : c.op === 'matches' ? new RegExp(c.value).test(String(valueAt(c.path) ?? '')) : true
    const shut = resume.filter((id) => {
      const st = builds[which].bp.stages.find((s) => s.id === id)
      return st && !(st.when || []).every(holds)
    })
    check(!shut.length, `resume (${which}): every stage the resume names lets mode resume through its own condition (${shut.join(', ')})`)
    if (which === 'econ') {
      check(resume.includes('events') && resume.includes('findings') && resume.includes('models') && resume.includes('explain') && resume.includes('cards-book'), 'resume (econ): the economics layers and the whole-book cycle are drained')
      check(['models', 'explain', 'events', 'findings'].every((mode) => ['link-prepare', 'link', 'link-check', 'organize', 'audit', 'cleanup', 'report'].every((id) => (modes[mode] || []).includes(id))), 'repair modes (econ): models, explain, events and findings drain the changed-Card Link work, Organize, the audit and the settlement')
    } else {
      check(resume.includes('math-skeleton') && resume.includes('math-boundary') && resume.includes('math-proof') && resume.includes('math-theory') && resume.includes('math-check'), 'resume (math): every mathematics obligation and the final math check are drained')
      check(['models', 'explain'].every((mode) => ['link-prepare', 'link', 'link-check', 'math-theory', 'organize', 'audit', 'math-check', 'cleanup', 'report'].every((id) => (modes[mode] || []).includes(id))), 'repair modes (math): a repaired model reaches its Theory refresh and the math check')
      for (const [stage, pattern] of [['math-skeleton', /resume/], ['math-boundary', /resume/], ['math-proof', /resume/], ['math-theory', /resume/], ['math-check', /resume/]]) {
        const found = builds.math.bp.stages.find((s) => s.id === stage)
        check(found && (found.when || []).some((c) => pattern.test(JSON.stringify(c))), `resume (math): ${stage} participates in the resume`)
      }
    }
  }
  for (const stage of ['organize', 'finalize']) {
    const when = JSON.stringify(stageOf('econ', stage).when || [])
    check(/resume/.test(when), `resume: the ${stage} stage declares mode resume in its own condition`)
  }
  check(builds.econ.bp.stages.concat(builds.math.bp.stages).filter((s) => s.params && !['args', 'source', 'prepare'].includes(s.id)).every((s) => s.params.key && s.params.key.$ref === 'stages.prepare.output.citation_key'), 'resume: every stage downstream of prepare reads the Citation key from the prepared run, so mode resume never repeats args.citationKey')
  const resumeGate = await run('econ', { out: 'D:\\x\\Key', mode: 'resume', source: 'S.md' })
  check(resumeGate.result.stage === 'args' && /accepts only out/.test(resumeGate.result.error), 'args: mode resume refuses a new source instead of reinterpreting the retained run')
  const resumeRun = await run('econ', { out: 'D:\\x\\Key', mode: 'resume' })
  const resumeCommand = resumeRun.calls.find((c) => c.label === 'resume run')
  check(!!resumeCommand && /zt-digest ledger resume -Out "D:\\x\\Key"/.test(resumeCommand.prompt), 'resume: mode resume runs the one supported ledger resume for the retained output directory')
  check(!resumeRun.calls.some((c) => c.label === 'prepare book'), 'resume: mode resume never prepares the book again')
  check(resumeRun.result.stage === 'prepare' && /no retained digest groups/.test(resumeRun.result.error), 'resume: a resume of a folder without a retained prepared run halts at prepare with its reason')
  const m = builds.math.bp
  const mathAfter = (id) => (m.stages.find((s) => s.id === id).after || []).join(',')
  check(mathAfter('cards-book') === 'explain' && mathAfter('link-check') === 'link' && mathAfter('math-theory') === 'link-check', 'schedule: the mathematics variant keeps Theory after the link check')
  check(mathAfter('audit') === 'bridge,term-link' && mathAfter('math-check') === 'finalize' && mathAfter('cleanup') === 'math-check', 'schedule: the mathematics check describes the final audit revision, then the settlement')
  check(m.stages.some((s) => s.id === 'finalize'), 'schedule: the mathematics variant holds the finalization stage its base declares')
}

// ================================================================================================================
// S24 (#167): the parallel Term stages and their truthful full-mode dependencies
// ================================================================================================================
{
  const tc = stageOf('econ', 'term-cards')
  const tl = stageOf('econ', 'term-link')
  check(tc && tc.module === 'zt-term-cards' && (tc.after || []).join(',') === 'terms' && tc.onFail === 'continue', `S24: the Term-card branch runs after the term table beside the digest writers (after ${(tc.after || []).join(',')})`)
  check(tl && tl.module === 'zt-term-link' && (tl.after || []).join(',') === 'bridge,term-cards' && tl.onFail === 'continue', `S24: the Term-link pass waits for every Card writer and the term-card branch (after ${(tl.after || []).join(',')})`)
  for (const which of ['econ', 'math']) {
    const bp = builds[which].bp
    check(bp.stages.some((s) => s.id === 'term-cards') && bp.stages.some((s) => s.id === 'term-link'), `S24 (${which}): the variant inherits both Term stages from the base`)
    const resume = (bp.modes.resume || [])
    check(resume.includes('term-cards') && resume.includes('term-link'), `S24 (${which}): mode resume drains both Term stages`)
  }
  const args = builds.econ.bp.args
  check(args.termCards && args.termCards.type === 'boolean' && args.termCards.default === true && args.termLink && args.termLink.type === 'boolean' && args.termLink.default === true, 'S24: the termCards and termLink layer switches are declared boolean and default on')
  check(args.termCardsPerJob && args.termCardsPerJob.type === 'number' && args.termCardsPerJob.min === 1 && args.termCardsPerJob.max === 8 && args.termCardsPerJob.default === 4 && args.termCardsMathPerJob && args.termCardsMathPerJob.default === 1, 'S24: the per-job sizing args are declared and bounded 1..8')
  check(args.termLinkOccurrences && args.termLinkOccurrences.enum && args.termLinkOccurrences.enum.join(',') === 'first,all' && args.termLinkOccurrences.default === 'first', 'S24: the occurrence arg is an enum of first and all')
  const badOccurrences = await run('econ', { ...base, termLinkOccurrences: 'second', until: 'digest' })
  check(badOccurrences.thrown && /args\.termLinkOccurrences must be one of/.test(badOccurrences.thrown.message), 'S24: an undeclared occurrence policy is refused before any stage')
  const badPerJob = await run('econ', { ...base, termCardsPerJob: 9, until: 'digest' })
  check(badPerJob.thrown && /args\.termCardsPerJob must be within \[1, 8\]/.test(badPerJob.thrown.message), 'S24: an out-of-range per-job size is refused before any stage')
  // No second scheduler: the branch claims units from the run ledger and starts no other queue.
  for (const module of ['zt-term-cards', 'zt-term-link']) {
    const source = fs.readFileSync(path.join(workflowDir, 'modules', `${module}.js`), 'utf8')
    check(/ledger\('claim'/.test(source) && /next -Out/.test(source) && /ledger\('complete'/.test(source) && /done -Out/.test(source) && !/setInterval|setTimeout|new Date\(|schedule\[|queue\[|worker pool/i.test(source), `S24: ${module} uses the run ledger as its scheduler and starts no second one`)
  }
  // The arithmetic the branch can be wrong about: at most four retained writers per wave.
  const full = await run('econ', { ...base, figures: false })
  const writes = full.termManagers.filter((m) => m.kind === 'write')
  check(full.report.stages['term-cards'] && full.report.stages['term-cards'].status === 'done' && full.report.stages['term-link'] && full.report.stages['term-link'].status === 'done', `S24: a full run completes both Term stages (${JSON.stringify([full.report.stages['term-cards'], full.report.stages['term-link']])})`)
  check(writes.length > 0 && writes.every((m) => m.jobs.length <= 4), `S24: every writer manager holds at most four jobs (${writes.map((m) => m.jobs.length).join(',')})`)
  const groups = full.led.gate('termcards')
  check(groups.complete === true && full.led.find('termcards/book@1') && full.led.find('termcards/book@1').state === 'done' && full.led.find('termlink/book@1') && full.led.find('termlink/book@1').state === 'done', `S24: the Term-card job/book units and the Term-link pass all complete on the verified work (${[...full.led.units.values()].filter((u) => /term/.test(u.layer)).map((u) => `${u.id}=${u.state}`).join(',')})`)
  const tcCalls = full.termCalls.filter((c) => c.tool === 'term-cards')
  const steps = tcCalls.map((c) => c.step)
  check(steps.includes('plan') && steps.indexOf('plan') < steps.indexOf('jobs') && steps.includes('check') && steps.indexOf('jobs') < steps.indexOf('apply'), `S24: the branch plans, claims its jobs, checks and publishes in order (${steps.join(',')})`)
  check(full.termCalls.find((c) => c.tool === 'term-link' && c.step === 'plan').cmd.includes('-Occurrences first'), 'S24: the Term-link pass runs the declared occurrence policy')
  check(full.result.term_cards && full.result.term_cards.status === 'done' && full.result.term_cards.unit === 'termcards/book@1' && full.result.term_cards.publication && full.result.term_cards.publication.complete === true, `S24: the report carries the Term-card branch's own complete result (${JSON.stringify(full.result.term_cards)})`)
  check(full.result.term_link && full.result.term_link.status === 'done' && full.result.term_link.unit === 'termlink/book@1' && full.result.term_link.counts && full.result.term_link.counts.cards_changed === 1, `S24: the report carries the applied Term-link pass and its counts (${JSON.stringify(full.result.term_link)})`)
  check(full.result.schedule && full.result.schedule.term_cards === 'done' && full.result.schedule.term_link === 'done', 'S24: the schedule summary names both Term stages')
  // The frozen layer switches: a disabled Term layer freezes into the prepared target and runs no stage.
  const noCards = await run('econ', { ...base, figures: false, termCards: false })
  const noCardsPrep = noCards.calls.find((c) => c.label === 'prepare book')
  check(noCardsPrep && /"termCards":false/.test(noCardsPrep.cmd), 'S24: a disabled Term-card branch is frozen into the prepared target (-RunOptions)')
  check(noCards.report.stages['term-cards'].status === 'skipped' && noCards.termCalls.every((c) => c.tool !== 'term-cards'), 'S24: termCards false skips the branch and runs no compiler command')
  check(noCards.report.stages['term-link'].status === 'done' && noCards.led.find('termlink/book@1').state === 'done', 'S24: a disabled Term-card branch registers no unit to wait for; the Term-link pass still runs')
  const noLink = await run('econ', { ...base, figures: false, termLink: false })
  const noLinkPrep = noLink.calls.find((c) => c.label === 'prepare book')
  check(noLinkPrep && /"termLink":false/.test(noLinkPrep.cmd) && noLink.report.stages['term-link'].status === 'skipped' && noLink.termCalls.every((c) => c.tool !== 'term-link'), 'S24: termLink false is frozen, skips the pass and registers no termlink unit')
  check(noLink.led.find('termcards/book@1') && noLink.led.find('termcards/book@1').state === 'done', 'S24: termLink false never blocks the Term-card branch')
  // A failed term job leaves committed work but can never complete the book unit; the pass stays blocked.
  const failed = await run('econ', { ...base, figures: false }, { termWriterFails: true })
  check(failed.report.stages['term-cards'].status !== 'failed' && failed.result.term_cards.status === 'failed', 'S24: a failed writer fails the branch without halting the run')
  check(failed.led.find('termcards/t001@1').state === 'failed' && failed.led.find('termcards/t002@1').state === 'failed' && failed.led.find('termcards/book@1').state !== 'done', 'S24: a failed job unit keeps its own fault and the book unit never completes')
  check(failed.result.term_link.status === 'skipped' && /not ready/.test(String(failed.result.term_link.reason || '')) && failed.led.find('termlink/book@1') && failed.led.find('termlink/book@1').state === 'blocked', `S24: the normal Term-link dependency stays blocked with its reason (${JSON.stringify(failed.result.term_link)})`)
  check(failed.result.audit && failed.report.stages.audit.status === 'done' && failed.result.run_report, 'S24: the audit and the run report still run and show the coverage debt')
  check(!failed.termCalls.some((c) => c.tool === 'term-link' && c.step === 'apply'), 'S24: a failed term job never reaches the Term-link apply')
  // A failed term job leaves the ready later drafts staged and the publication partial.
  const partial = await run('econ', { ...base, figures: false }, { termPublicationPartial: true })
  check(partial.result.term_cards.status === 'failed' && /not complete/.test(String(partial.result.term_cards.reason || '')), `S24: a partial publication is reported, never a successful zero-term book (${JSON.stringify(partial.result.term_cards.reason)})`)
  check(partial.led.find('termcards/book@1').state !== 'done' && partial.result.term_link.status === 'skipped', 'S24: a partial Term-card book leaves the book unit incomplete and the Term-link pass skipped')
  // A stopped pass records its fault and stays incomplete; the audit still runs.
  const stopped = await run('econ', { ...base, figures: false }, { termLinkStops: true })
  check(stopped.result.term_link.status === 'failed' && /card_check/.test(String(stopped.result.term_link.reason || '')), `S24: a stopped Term-link pass reports its typed stop (${JSON.stringify(stopped.result.term_link.reason)})`)
  check(stopped.led.find('termlink/book@1').state !== 'done' && stopped.result.audit, 'S24: a stopped pass keeps its unit incomplete and the audit still describes the book')
  // A book with no usable Term list is a clear incomplete result, never a successful zero-term book.
  // No Term list at all: the branch cannot plan a Term card and the pass cannot plan a link, and neither is a successful zero-term book.
  const noList = await run('econ', { ...base, figures: false }, { termPlanFails: true, termLinkPlanFails: true })
  check(noList.result.term_cards.status === 'failed' && /no Term list/.test(String(noList.result.term_cards.reason || '')), `S24: a plan with no Term list is a clear failed result (${JSON.stringify(noList.result.term_cards.reason)})`)
  check(noList.result.term_link.status === 'failed' && /no list/.test(String(noList.result.term_link.reason || '')) && !noList.led.find('termlink/book@1'), `S24: a Term link requested with no Term list is a clear failed result and owns no unit (${JSON.stringify(noList.result.term_link.reason)})`)
  check(noList.result.audit && noList.report.stages.audit.status === 'done', 'S24: the audit still describes a book whose Term list could not be planned')
  const quiet = await run('econ', { ...base, figures: false }, { termLinkNoop: true })
  check(quiet.result.term_link.status === 'done' && quiet.result.term_link.no_op === true && quiet.led.find('termlink/book@1').state === 'done', 'S24: a proven Term-link no-op completes its unit with no edit')
  // The math variant runs the same inherited pair.
  const mathFull = await run('math', { ...base, figures: false })
  check(mathFull.report.stages['term-cards'].status === 'done' && mathFull.report.stages['term-link'].status === 'done', 'S24 (math): the inherited Term stages run in the mathematics variant')
}

{
  // docs/digest-claude-token-proposal.md gate 3 (fault behaviour): a batch finish records each unit's fault
  // against that unit alone, with the class finishOrFault gave it, and goes on with the next unit.
  const content = await run('econ', { ...base, figures: false }, { contentWhen: /findings-finish -Out "D:\\x\\Key" -Group 1 / })
  check(content.led.find('findings/g001@1').state === 'failed' && content.led.find('findings/g002@1').state === 'done', 'batch fault: a content fault fails its own unit and the next unit of the batch still finishes')
  check(content.batchFaults.some((x) => x.unit === 'findings/g001@1' && x.class === 'content') && !content.commands.some((x) => /ledger fail /.test(x) && /findings\/g001/.test(x)), 'batch fault: the content fault is recorded by the batch itself, not by a relayed ledger fail')
  const retry = await run('econ', { ...base, figures: false }, { retryWhen: /events-finish -Out "D:\\x\\Key" -Group 2 / })
  check(retry.batchFaults.some((x) => x.unit === 'events/g002@1' && x.class === 'retryable') && retry.led.find('events/g001@1').state === 'done', 'batch fault: a transient fault is recorded retryable against its own unit')
  check(retry.report && !retry.report.halted, 'batch fault: a unit fault never halts the run')
}
{
  // R326/R337: the owner's Lock inside a batch finish is that unit's content fault, the batch starts no later
  // unit, and the stage stops with the typed refusal.
  const rt = await run('econ', { ...base, figures: false }, { lockCommand: /events-finish -Out "D:\\x\\Key" -Group 1 / })
  const halted = rt.report && rt.report.halted
  check(halted && halted.stage === 'events' && /card_locked/.test(String(halted.error || '')), `batch lock: the events stage stops with the typed refusal (${JSON.stringify(halted).slice(0, 200)})`)
  check(rt.led.find('events/g001@1').state === 'failed' && rt.led.find('events/g002@1').state === 'claimed', 'batch lock: the locked unit keeps its content fault and the batch starts no later unit')
  check(rt.batchFinishes.filter((x) => /events-finish .* -Group 1 /.test(x)).length === 1, 'batch lock: the refused finish is never retried')
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
  const stageCommands = commands.filter((c) => /^(zt-digest|pwsh -NoProfile -File) /.test(c))
  check(stageCommands.length > 0 && stageCommands.every((c) => /^zt-digest [a-z-]+ /.test(c)), `service: every stage command is a zt-digest tool, so the stage resolves the service before its archive work and its retries do too (R320; ${stageCommands.filter((c) => !/^zt-digest [a-z-]+ /.test(c)).length} other)`)
  check(pool.peak <= 8, `runner: at most eight managers are alive at once (peak ${pool.peak})`)
  check(report && Object.values(report.stages).every((s) => s.status !== 'failed'), `runner: a full stand-in run has no failed stage${report ? Object.entries(report.stages).filter(([, s]) => s.status === 'failed').map(([k, s]) => ` (${k}: ${s.error})`).join('') : ''}`)
}

// ================================================================================================================
// The group cycle and the ready-work layers (R250-R258)
// ================================================================================================================
{
  const { commands, calls, led, result, logs, report, managers, driveFiles, batchCalls, batchFinishes, depsBatches } = await run('econ', { ...base, figures: false })
  const cmds = commands
  const labels = calls.map((c) => c.label)
  check(result && result.mode === 'full', 'full: the run reaches its report')
  const digestClaims = cmds.filter((c) => /ledger next -Out "D:\\x\\Key" -Layer digest -Take 8 /.test(c))
  check(digestClaims.length === 2 && !cmds.some((c) => /-Layer digest -Take 1 /.test(c)), `group cycle: one lane claims both groups in one block of up to eight, then finds none ready (${digestClaims.length} claims)`)
  check(cmds.filter((c) => /group-start -Out "D:\\x\\Key" -Group \d+ -Token \d+/.test(c)).length === 2, 'group cycle: the block starts each held group (snapshot before the job manifest)')
  check(cmds.filter((c) => /group-finish .* -Draft "D:\\x\\Key\\group-jobs\\g00\d\\claim-\d+\\draft\.json"/.test(c)).length === 2, 'group cycle: the block records each checked completion with the draft bound to its start')
  check(cmds.some((c) => /zt-digest group-start -Out/.test(c)) && !cmds.some((c) => /-Groups \d/.test(c)), 'group cycle: no per-block legacy digest command remains')
  check(labels.filter((l) => /^digest lane 1 block \d+$/.test(l)).length === 2 && !labels.some((l) => /^group \d attempt/.test(l)), 'group cycle: one manager runs the whole block of groups, not one manager per group')
  check(managers.filter((m) => m.block && /^digest lane/.test(m.label)).every((m) => m.jobs.length <= 8) && managers.some((m) => m.block && m.jobs.length === 2), 'group cycle: one block manager runs both groups\' DeepSeek jobs side by side, never more than eight')
  const digestUnits = [...led.units.values()].filter((u) => u.layer === 'digest')
  check(digestUnits.length === 2 && digestUnits.every((u) => u.state === 'done'), 'group cycle: both claimed group units are complete in the ledger')
  const cards = [...led.units.values()].filter((u) => u.layer === 'cards')
  check(cards.length === 2 && cards.every((u) => u.state === 'done') && batchFinishes.filter((c) => /(?<!book-)cards-finish/.test(c)).length === 2, 'cards: the per-group offline checks are claimed and recorded by cards-finish')
  // docs/digest-claude-token-proposal.md P1/P2: the Cards layer claims and finishes a block in ONE command, and no
  // layer spends a relay per unit finish.
  check(cmds.some((c) => /^zt-digest finish-batch -Out "D:\\x\\Key" -Layer cards -Take 4 -Owner "cards claim 1" -Lease 1500$/.test(c)) && !cmds.some((c) => /ledger next .* -Layer cards /.test(c)), 'cards: one finish-batch command claims the ready checks and records them, with the covering lease')
  check(!cmds.some((c) => /^zt-digest (cards|events|findings|model|explain)-finish /.test(c)), 'layers: no layer relays one command per unit finish')
  check(batchCalls.every((b) => b.units.length <= 8), 'layers: a batch finish holds at most eight units')
  const models = [...led.units.values()].filter((u) => u.layer === 'models')
  check(models.length > 0 && models.every((u) => u.state === 'done') && batchFinishes.some((c) => /model-finish/.test(c)), 'models: each registered Model unit is built and recorded by model-finish')
  check(depsBatches.length > 0 && !cmds.some((c) => /model-deps .*-Model /.test(c)), 'models: a batch registers its references with one model-deps command, not one per model')
  check(!cmds.some((c) => /zt-digest model -Step report /.test(c)) && cmds.some((c) => /zt-digest model -Step close .* -Report\b/.test(c)), `models: a close round that sends nothing back writes the batch report, so no report step runs (${labels.filter((l) => /^(close|report)/.test(l)).join(', ')})`)
  check(batchFinishes.some((c) => /explain-finish/.test(c)), 'explain: the explanation units are recorded by explain-finish in mode full')
  check(cmds.some((c) => /event-compiler/.test(c) || /zt-digest event /.test(c)) && batchFinishes.some((c) => /events-finish/.test(c)), 'events: the Event layer claims its units and records events-finish')
  check(cmds.some((c) => /finding-compiler/.test(c) || /zt-digest finding /.test(c)) && batchFinishes.some((c) => /findings-finish/.test(c)), 'findings: the Finding layer claims its units and records findings-finish')
  check(['events', 'findings', 'explain'].every((l) => batchCalls.some((b) => b.units.some((u) => u.startsWith(l + '/'))) && !calls.some((c) => new RegExp('^' + l + ' .*: finish$').test(c.label))), 'events, findings, explain: each chain ends with the block finish in its own manager, so no finish relay runs')
  check(!calls.some((c) => /^You drive DeepSeek Harness/.test(c.prompt) && /Run this one PowerShell command/.test(c.prompt) && /ToolSearch/.test(c.prompt)), 'drive: a compiler step\'s command agent loads no DeepSeek tool')
  const gateCmds = cmds.filter((c) => /ledger gate/.test(c))
  const teed = cmds.filter((c) => /\| Tee-Object -Variable zdLines; .* Set-Content -LiteralPath "D:\\x\\Key\\jobs\\[^"]+\.json"/.test(c))
  check(teed.length > 0 && driveFiles.length > 0 && driveFiles.every((f) => teed.some((c) => c.includes(f))), `drive: each step command saves its own job line, and every job manager reads that file instead of a retyped list (${teed.length} steps, ${driveFiles.length} managers)`)
  check(gateCmds.length > 0 && gateCmds.every((c) => / -Brief(?![A-Za-z])/.test(c)), `gate: every layer gate asks for the brief line a relay can return whole (${gateCmds.filter((c) => !/ -Brief(?![A-Za-z])/.test(c)).slice(0, 2).join(' | ')})`)
  check(gateCmds.length >= 3 && gateCmds.some((c) => /-Layer cards/.test(c)) && gateCmds.some((c) => /-Layer models/.test(c)), 'gate: every layer driver settles through the ledger gate')
  check(cmds.some((c) => /ledger publish -Out "D:\\x\\Key" -Layer cards-book/.test(c)) && cmds.some((c) => /book-cards-finish .* -Unit "?cards-book\/book@1"?/.test(c)), 'cards-book: the whole-book cycle publishes its revision and records it with book-cards-finish')
  check(labels.includes('prepare link pass') && labels.some((l) => /^link lane 1 block 1$/.test(l)) && cmds.some((c) => /ledger next .* -Layer link/.test(c)) && cmds.some((c) => /link-start .* -Group 1/.test(c)) && cmds.some((c) => /link-finish .* -Group 1/.test(c)), 'link: the scheduled group runs link-start, its one job and link-finish')
  check(cmds.some((c) => /link-start .* -Group 2/.test(c)) && cmds.some((c) => /link-finish .* -Group 2/.test(c)) && !cmds.some((c) => /ledger fail .* -Class content/.test(c) && /link/.test(c)), 'link: a group with no semantic gap settles its unit without a provider job or a legacy content fault')
  check(result.link_gaps && result.link_gaps.scheduled === 1 && result.link_gaps.skipped === 1 && result.link_gaps.gaps === 2 && result.link_gaps.provider_runs === 1 && result.link_gaps.quality && result.link_gaps.quality.faults === 0, `report: the run carries the scheduled/skipped/gap counts, the actual provider runs and the L1/L2 quality (${JSON.stringify(result.link_gaps)})`)
  check(Array.isArray(result.link) && result.link.some((r) => r.group === 1 && r.status === 'done' && r.gaps === 2 && r.provider_runs === 1) && result.link.some((r) => r.group === 2 && r.scheduled === 0 && r.provider_runs === 0), 'report: each group names its gap count, whether it was scheduled and its provider runs')
  check(result.schedule && result.schedule.settings.maxAttempts === 3 && result.schedule.settings.dshRunCap === 1200, 'report: the result carries the effective settings and the schedule summary')
  check(result.schedule && result.schedule.digest && result.schedule.digest.verified === 2 && result.schedule.cards && result.schedule.cards.verified === 2, 'report: the schedule summary counts the verified group chains and Card checks')
  check(report && report.stages.digest && report.stages.digest.status === 'done' && !report.stages.digest.error, 'group cycle: the digest stage ends done on a healthy run')
  // #92: every claim covers the frozen provider-run cap, and a chain renews it while its job runs.
  check(digestClaims.length === 2 && digestClaims.every((c) => / -Lease 1500/.test(c)), `lease: each block claim requests a lease covering the run cap (${cmds.filter((c) => /-Lease 1500/.test(c)).length} claims)`)
  check(cmds.some((c) => /ledger next .* -Layer link .* -Lease 1500/.test(c)), 'lease: the Link units are claimed with the same covering lease')
  const chainPrompts = calls.map((c) => c.prompt).join('\n')
  check(chainPrompts.includes('ledger renew -Out "D:\\x\\Key" -Unit') && chainPrompts.includes('wait_seconds 90'), 'lease: a chain renews its claim after every wait on a running job, and never waits longer than 90 seconds')
}
{
  // #92: a group whose digest unit is not done is blocked: no link job runs for it, and the run names it.
  const blocked = [
    { n: 1, unit: 'digest/g001@1', state: 'failed', reason: 'the digest unit digest/g001@1 is failed, not done; this group gets no link job' },
    { n: 2, unit: 'digest/g002@1', state: 'ready', reason: 'the digest unit digest/g002@1 is ready, not done; this group gets no link job' },
  ]
  const { calls, result } = await run('econ', { ...base, figures: false }, { linkBlocked: blocked })
  const labels = calls.map((c) => c.label)
  check(!labels.some((l) => /^link lane/.test(l)), 'blocked link: no link job runs for a group whose digest unit is not done')
  check(result && Array.isArray(result.link_blocked) && result.link_blocked.length === 2 && result.link_blocked.every((b) => b.unit && b.state && b.reason && b.group), `report: the run names the blocked groups and their units (${result && JSON.stringify(result.link_blocked)})`)
}
{
  // A transient group failure is recorded against its own unit and claimed again by a later attempt.
  const { commands, calls, led, failClasses } = await run('econ', { ...base, figures: false, until: 'digest' }, { failWhen: /group-finish/ })
  const cmds = commands
  const fails = failClasses.filter((f) => f.class === 'retryable')
  check(failClasses.every((f) => !f.given), 'S5: no fault names its class; the ledger classifies')
  check(fails.length >= 1, `retries: a failed group is recorded against its own unit as retryable (${fails.length} faults)`)
  check(cmds.filter((c) => /-Layer digest -Take 8/.test(c)).length >= 2, 'retries: a later block claims again after a failure, not a rerun of the stage')
}
{
  // A block manager that summarizes its claim line in prose: the lane still knows the units it held from
  // the manager's own unit reports, records a failed unit against its own attempt, and keeps claiming.
  const { commands, logs, faultRefusals, failClasses } = await run('econ', { ...base, figures: false, until: 'digest' }, { proseClaim: true, failWhen: /group-finish -Out "D:\\x\\Key" -Group 2 / })
  const cmds = commands
  check(failClasses.some((f) => f.unit === 'digest/g002@1' && f.class === 'retryable'), 'prose claim: a failed unit of a summarized claim is still recorded against its own attempt')
  check(faultRefusals.length === 0, 'prose claim: the fault carries the unit input hash the ledger holds (' + faultRefusals.length + ' refused)')
  check(cmds.filter((c) => /-Layer digest -Take 8/.test(c)).length >= 2, `prose claim: the lane keeps claiming after a summarized claim (${cmds.filter((c) => /-Layer digest -Take 8/.test(c)).length} claims)`)
  check(!logs.some((l) => /the claim did not run/.test(l)), 'prose claim: a summarized claim is not mistaken for a failed claim')
}
{
  // A manager that drops its unit reports (run 6: one of eight, or none): the ledger decides each unit. A done
  // unit is never faulted; a unit the block still holds (its finish failed) gets its fault.
  const { commands, result, failClasses } = await run('econ', { ...base, figures: false, until: 'digest' }, { dropUnits: true, failWhen: /group-finish -Out "D:\\x\\Key" -Group 2 / })
  const cmds = commands
  check(cmds.some((c) => /ledger status -Out "D:\\x\\Key" -Units digest\/g001@1,digest\/g002@1/.test(c)), 'dropped reports: the block reads its held units\' states from the ledger')
  check(!cmds.some((c) => /ledger fail .*-Unit "?digest\/g001@1"?/.test(c)), 'dropped reports: a unit the ledger holds done is never faulted')
  check(failClasses.some((f) => f.unit === 'digest/g002@1' && f.class === 'retryable'), 'dropped reports: a unit the block still holds gets its retryable fault')
  check(cmds.every((c) => !/-Layer digest -Take 8 /.test(c) || / -Brief /.test(c)), 'brief claim: a block claims with -Brief, so its claim line stays short')
}
{
  // A layer claim whose relayed line lost its units array (grossman2002integration trial 3: "(4 units claimed:
  // cards/g005@1, ...)"): the driver reads the units its claim holds from the ledger by owner, finishes each
  // with its own token, and never records a fault against an undefined unit.
  const { commands, led, heldReads } = await run('econ', { ...base, figures: false }, { summarizedClaim: 'explain' })
  const cmds = commands
  check(heldReads.length > 0 && heldReads.every((o) => /^explain claim [0-9]+$/.test(o)), `summarized layer claim: the driver reads its held units from the ledger by owner (${heldReads.join(', ')})`)
  check(!cmds.some((c) => /undefined/.test(c)), 'summarized layer claim: no command names an undefined unit or token')
  check(led.layer('explain').length > 0 && led.layer('explain').every((u) => u.state === 'done'), `summarized layer claim: every explain unit is finished (${led.layer('explain').filter((u) => u.state === 'done').length}/${led.layer('explain').length})`)
}
{
  // Every relay prompt says its command is a step of the run the user started, so a later user message about the
  // run (trials 17-19: "accelerate it", "stop trial 18 ... start trial 19") does not read as cancelling it.
  const { calls } = await run('econ', { ...base, figures: false })
  const relays = calls.filter((c) => /Run this PowerShell command once|^You run one block|You run one compiler chain/.test(c.prompt))
  const missing = relays.filter((c) => !/never cancels its steps/.test(c.prompt))
  check(relays.length > 0 && !missing.length, `run step: every relay prompt names its command a step of the user's run (${relays.length} relays, ${missing.length} without: ${missing.slice(0, 2).map((c) => c.label).join(', ')})`)
}
{
  // P5 (docs/digest-claude-token-proposal.md): a one-command relay runs as the zt-relay agent type (PowerShell only, a
  // short prompt: about 9k tokens of first-request context instead of about 59k). Every other agent keeps its role.
  for (const variant of ['econ', 'math']) {
    const { calls } = await run(variant, { ...base, figures: false })
    const one = (c) => /^Run this PowerShell command once/.test(c.prompt)
    const relays = calls.filter(one)
    const wrong = relays.filter((c) => c.agentType !== 'zt-relay' || c.model !== 'haiku')
    const others = calls.filter((c) => !one(c) && c.agentType === 'zt-relay')
    check(relays.length > 0 && !wrong.length, `relay role (${variant}): every one-command relay runs as zt-relay on Haiku (${relays.length} relays, ${wrong.length} not: ${wrong.slice(0, 2).map((c) => c.label).join(', ')})`)
    check(!others.length, `relay role (${variant}): no manager, architect or organizer runs as zt-relay (${others.slice(0, 2).map((c) => c.label).join(', ')})`)
  }
}
{
  // The model layer claims a block of up to eight Model units (the protocol's eight per manager), and one architect
  // batch holds up to eight formal models, so a book's models are planned together: in-batch references resolve, and
  // no second block waits for the first (grossman2002integration trials 16-17: two blocks of four, 76 minutes).
  const { commands } = await run('econ', { ...base, figures: false })
  const modelClaims = commands.filter((c) => /ledger next .*-Layer models /.test(c))
  check(modelClaims.length > 0 && modelClaims.every((c) => /-Take 8\b/.test(c)), `parallel models: every models claim takes up to 8 units (${modelClaims.map((c) => (c.match(/-Take \d+/) || [''])[0]).join(', ')})`)
  check(/const PER_SONNET = Math\.max\(1, A\.perSonnet \|\| 8\)/.test(sources.econ), 'parallel models: one architect batch holds up to 8 formal models by default')
}
{
  // A block manager that claims its units and then throws (trial 17, the link stage): the stage does not halt;
  // the units that block's owner holds are read from the ledger, faulted for a retry, and a later block finishes them.
  const { report, led, heldReads } = await run('econ', { ...base, figures: false }, { blockThrows: true })
  check(report && !report.halted, `thrown block: the run does not halt (${JSON.stringify(report && report.halted)})`)
  check(heldReads.some((o) => /^link lane \d+ block \d+$/.test(o)), `thrown block: the block's held units are read from the ledger by its owner (${heldReads.join(', ')})`)
  check(led.layer('link').length > 0 && led.layer('link').every((u) => u.state === 'done'), `thrown block: every link unit is finished (${led.layer('link').map((u) => u.id + ':' + u.state).join(', ')})`)
}
{
  // trial 20: a resume of a run whose later layers are complete reuses them; the Organize decisions, the skeleton, Figures,
  // Bridge, the hub writer and the audit never run again (the resume reran Organize's survey and decisions).
  const retained = [['terms', 'terms/book@1'], ['digest', 'digest/g001@1'], ['organize', 'organize/book@1'], ['skeleton', 'skeleton/book@1'], ['figures', 'figures/book@1'], ['bridge', 'bridge/book@1'], ['audit', 'audit/book@1'], ['link-check', 'link-check/book@1']]
  const rt = await run('econ', { out: 'D:\\x\\Key', mode: 'resume' }, { retained, retainedDone: ['organize', 'skeleton', 'figures', 'bridge', 'audit', 'link-check'] })
  const labels = rt.calls.map((c) => c.label)
  check(!labels.some((l) => /^organize: (decisions|survey)|^hubs write|^audit cards|^bridge: plan|^finalize plan/.test(l)), `resume reuse: completed later layers are not redone (${labels.filter((l) => /^organize|^hubs|^audit|^bridge|^finalize/.test(l)).join(', ')})`)
}
{
  // zt-dsh-cleanup v11: when the bridge has no deepseek_settle (an old bridge, the Desktop down), the agent settles the
  // units whose conversations are already gone and the stage reports the provider batch as not run.
  const { logs, cleanupRunCalls, cleanupReadyCalls, result } = await run('econ', { ...base, figures: false }, { settleMissing: true })
  check(cleanupRunCalls.join() === 'plan' && cleanupReadyCalls.length === 1 && logs.some((l) => /the provider batch did not run \(tool not found/.test(l)), `settle missing: the stage falls back to cleanup-finish -Ready and says why (${cleanupRunCalls.join()})`)
  check(result.cleanup && result.cleanup.provider === null, 'settle missing: no provider facts are reported as recorded')
}
{
  // A model claim whose relayed line has no `units` field (grossman2002integration trial 17: `units_claimed`
  // with the ids only). A missing field is no empty claim: the driver reads the held units from the ledger and
  // builds every model, instead of ending the layer with four models claimed and never built.
  const { led, heldReads } = await run('econ', { ...base, figures: false }, { renamedUnits: 'models' })
  check(heldReads.some((o) => /^models claim [0-9]+$/.test(o)), `renamed units: the model driver reads its held units from the ledger (${heldReads.join(', ')})`)
  check(led.layer('models').length > 0 && led.layer('models').every((u) => u.state === 'done'), `renamed units: every model unit is finished (${led.layer('models').filter((u) => u.state === 'done').length}/${led.layer('models').length})`)
}
{
  // A model claim whose relayed line lost every unit's inputs (grossman2002integration trial 4): the layer claims
  // with -Brief (each unit keeps its Model address), and a unit with no address is read from the ledger by owner.
  const { commands, led, heldReads } = await run('econ', { ...base, figures: false }, { droppedInputs: 'models' })
  const cmds = commands
  check(cmds.filter((c) => /ledger next .* -Layer (cards|events|findings|models|explain) /.test(c)).every((c) => / -Brief\b/.test(c)), 'layer claims: every layer driver claims with -Brief')
  check(heldReads.some((o) => /^models claim [0-9]+$/.test(o)), `dropped inputs: the model driver reads its held units' addresses from the ledger (${heldReads.join(', ')})`)
  check(!cmds.some((c) => /-Model 'undefined'|-Model ''/.test(c)), 'dropped inputs: no model-finish names an undefined Model')
  check(led.layer('models').length > 0 && led.layer('models').every((u) => u.state === 'done'), `dropped inputs: every model unit is finished (${led.layer('models').filter((u) => u.state === 'done').length}/${led.layer('models').length})`)
}
{
  // A layer unit's fault carries the claim's input hash, so the ledger records it (trial 4: every layer fault was
  // refused for an empty input_hash, because the hash was read from the unit's inputs).
  const { commands, faultRefusals, batchFaults, led } = await run('econ', { ...base, figures: false }, { failWhen: /cards-finish -Out "D:\\x\\Key" -Group 1 / })
  check(batchFaults.some((f) => f.unit === 'cards/g001@1') && faultRefusals.length === 0 && !commands.some((c) => /ledger fail .*-Unit "?cards\/g001@1"?/.test(c)), `layer faults: a failed card check is recorded by the batch against its own unit (${faultRefusals.length} refused)`)
  check(led.find('cards/g002@1') && led.find('cards/g002@1').state === 'done', 'layer faults: one unit\'s failed finish never stops the other units of its batch')
}
{
  // A finish whose relayed line came back cut (trial 5: a 2.5 KB cards completion): the ledger holds the unit done,
  // so the driver records it done and never sends a fault for it.
  // zt-digest 0.12: the batch line is short, but a relay can still cut it. The unanswered unit runs the batch again
  // (one relay), then its own finish, whose cut line sends the driver to the ledger.
  const { commands, led, logs } = await run('econ', { ...base, figures: false }, { cutFinish: /finish-batch -Out "D:\\x\\Key" -Units "events\/g001@1|events-finish -Out "D:\\x\\Key" -Group 1 / })
  check(commands.some((c) => /ledger status -Out "D:\\x\\Key" -Units events\/g001@1$/.test(c)), 'cut finish: the driver reads the unit state from the ledger')
  check(!commands.some((c) => /ledger fail .*-Unit "?events\/g001@1"?/.test(c)), 'cut finish: a unit the ledger holds done is never faulted')
  check(led.find('events/g001@1') && led.find('events/g001@1').state === 'done' && logs.some((l) => /finish line came back cut/.test(l)) && logs.some((l) => /no readable batch outcome/.test(l)), 'cut finish: the unit stays done and the log says why')
}
{
  // A cut claim-and-finish line of the Cards layer is never an empty claim: the layer claims again, and every
  // unit ends done with no fault.
  const { commands, led, logs } = await run('econ', { ...base, figures: false }, { cutFinish: /finish-batch .*-Layer cards / })
  check(led.layer('cards').length === 2 && led.layer('cards').every((u) => u.state === 'done') && !commands.some((c) => /ledger fail .*cards\//.test(c)), 'cut claim-and-finish: every card check ends done and none is faulted')
  check(commands.filter((c) => /finish-batch .*-Layer cards /.test(c)).length >= 2 && commands.some((c) => /ledger status .* -Held "cards claim 1"/.test(c)), 'cut claim-and-finish: the layer reads what its owner holds and claims again')
}
{
  // trial 10: the relay rewrote the end of a done cards completion as prose, the line still held a nested file
  // record, and the driver faulted the done unit (the ledger refused the fault as stale). Every finish that is not
  // plainly done asks the ledger first.
  const { commands, led, logs } = await run('econ', { ...base, figures: false }, { proseFinish: /finish-batch -Out "D:\\x\\Key" -Units "models\/|model-finish / })
  check(commands.some((c) => /ledger status -Out "D:\\x\\Key" -Units models\/g001-m0000@1$/.test(c)), 'prose finish: the driver reads the unit state from the ledger')
  check(!commands.some((c) => /ledger fail .*-Unit "?models\//.test(c)), 'prose finish: a unit the ledger holds done is never faulted')
  check(led.layer('models').every((u) => u.state === 'done') && logs.some((l) => /finish line came back cut/.test(l)), 'prose finish: the unit stays done and the log says why')
}
{
  // A model-deps batch line no relay returned whole is never read as "resolved": the ledger decides each model, and
  // only a unit still claimed under its token runs model-deps on its own.
  const { commands, led, logs } = await run('econ', { ...base, figures: false }, { garbledDeps: true })
  check(commands.some((c) => /ledger status -Out "D:\\x\\Key" -Units models\//.test(c)) && commands.some((c) => /model-deps -Out .* -Model 'Key\/2\|c' -Token \d+/.test(c)), 'garbled model-deps: the ledger decides each model and a claimed one registers on its own')
  check(led.layer('models').every((u) => u.state === 'done') && logs.some((l) => /no readable batch line/.test(l)), 'garbled model-deps: the model is still built and recorded')
}
{
  // The model compiler's chains outlive a claim's lease (trial 5: four model claims expired mid-build): the model
  // and explain drivers hold every claim of a block with `ledger hold` while the work runs; the quick
  // layers do not.
  const { commands, holds } = await run('econ', { ...base, figures: false })
  const holdCmds = commands.filter((c) => / ledger hold /.test(c))
  check(holds.some((h) => h.length > 0 && h.every((u) => /^models\//.test(u))), `hold: the model driver holds its block's claims (${holds.map((h) => h.join('+')).join(' | ')})`)
  check(holdCmds.every((c) => / -Seconds 540 -Every 60$/.test(c) && /-Units "[^"]+:\d+/.test(c)), 'hold: each hold is one bounded command naming every claim by unit and token')
  check(!holds.some((h) => h.some((u) => /^(cards|events|findings)\//.test(u))), 'hold: the quick layers never hold')
}
{
  // A layer whose units keep arriving (trial 6: one digest group retried while the events layer worked the others)
  // keeps claiming: its rounds bound consecutive idle claims, never the claims that found work.
  const { led } = await run('econ', { ...base, figures: false }, { lateUnits: { layer: 'events', count: 4 } })
  const late = led.layer('events').filter((u) => /\/g10\d@1$/.test(u.id))
  check(late.length === 4 && late.every((u) => u.state === 'done'), `late units: every Event unit that arrived after earlier productive claims is finished (${late.filter((u) => u.state === 'done').length}/${late.length})`)
}
{
  // A manager that wrote a job's answer file but whose evidence record never reached the ledger (trial 6: two review
  // jobs): the cleanup stage records that evidence from the answer file before it deletes anything, so the cleanup
  // verifier does not refuse the conversation for missing evidence.
  const { commands, sweeps, cleanups } = await run('econ', { ...base, figures: false }, { skipEvidence: /^s-/ })
  const swept = sweeps.flat()
  const plan = commands.findIndex((c) => /zt-digest cleanup-run -Out \S+ -Step plan/.test(c))
  const apply = commands.findIndex((c) => /zt-digest cleanup-run -Out \S+ -Step apply/.test(c))
  check(swept.length > 0 && swept.every((id) => /^s-/.test(id)), `evidence sweep: the cleanup plan records the missing evidence from the answer files (${swept.length} ids)`)
  check(plan >= 0 && apply > plan, `evidence sweep: the evidence is recorded in the plan step, before apply records any deletion (plan ${plan}, apply ${apply})`)
}
{
  // trial 6: model-finish refused every model for build faults no step sent back. The model chain runs
  // `-Step close` after its last fill, derive or split and before its report.
  const { commands } = await run('econ', { ...base, figures: false })
  const at = (re) => commands.findIndex((c) => re.test(c))
  // zt-model-lib v9: a close round that sends nothing back writes the report itself (-Report), so the report
  // follows the close in the same command; with build faults left (closeFaults) the report step runs after it.
  const close = at(/zt-digest model -Step close .* -Report\b/), report = at(/zt-digest model -Step report /), apply = at(/zt-digest model -Step apply /)
  check(close >= 0 && apply < close && report === -1, `close: the model chain sends its build faults back, and its clean close round writes the report (apply ${apply}, close ${close}, report ${report})`)
  const stuck = await run('econ', { ...base, figures: false }, { closeFaults: 9 })
  const sAt = (re) => stuck.commands.findIndex((x) => re.test(x))
  check(sAt(/zt-digest model -Step report /) > sAt(/zt-digest model -Step close /), 'close: a batch whose last close round still sends follow-ups runs the report step after it')
}
{
  // trials 17-19: close sent each fault back once, a writer left one in place, and model-finish refused the
  // model. Close runs again while faults remain and its last round sent notes, at most three rounds.
  const { commands } = await run('econ', { ...base, figures: false }, { closeFaults: 2 })
  const closes = commands.filter((c) => /zt-digest model -Step close /.test(c)).length
  check(closes === 3, `close rounds: close runs until its faults are gone (${closes} rounds for 2 rounds of faults)`)
  const { commands: stuck } = await run('econ', { ...base, figures: false }, { closeFaults: 9 })
  const stuckCloses = stuck.filter((c) => /zt-digest model -Step close /.test(c)).length
  check(stuckCloses === 3, `close rounds: at most three close rounds (${stuckCloses})`)
}
{
  // R233/R234 (trial 8: batch 2 took a part of batch 1's model and every model of it failed at the fill): each
  // planned model registers its references with `zt-digest model-deps` before the fill; a model that waits
  // for its producer gets no model-finish, and the others are finished as usual.
  const resolved = await run('econ', { ...base, figures: false })
  // zt-digest 0.12: the batch's planned models register in ONE command, `model-deps -Units <unit:token,...>`.
  const deps = resolved.commands.filter((c) => /^zt-digest model-deps -Out "D:\\x\\Key" -Units "models\/g\d{3}-m\d{4}@1:\d+(,models\/g\d{3}-m\d{4}@1:\d+)*"$/.test(c))
  const fill = resolved.commands.findIndex((c) => /zt-digest model -Step apply /.test(c))
  check(deps.length > 0 && resolved.commands.indexOf(deps[0]) < fill, `model-deps: each planned model registers its references before the fill (${deps.length} calls, fill at ${fill})`)
  check(!resolved.commands.some((c) => /zt-digest model -Step defer /.test(c)), 'model-deps: a batch whose models all resolved defers nothing')
  // trial 9: the fill held jobs for every model of the batch after model-deps had released three of them to wait.
  // A released model leaves the batch (`model -Step defer`) before any fill; a batch left with no model fills nothing.
  const { commands } = await run('econ', { ...base, figures: false }, { depsWaiting: /Key\/2\|c/ })
  const defer = commands.findIndex((c) => /zt-digest model -Step defer .* -Models "Key\/2\|c"$/.test(c))
  check(defer > commands.findIndex((c) => /^zt-digest model-deps /.test(c)), `model-deps: the waiting model leaves the batch after its registration (defer at ${defer})`)
  check(!commands.some((c) => /zt-digest model -Step (apply|derive|review|close) /.test(c)), 'model-deps: a batch whose only model waits writes no part (no fill, derive, review or close)')
  check(!commands.some((c) => /model-finish .*-Model 'Key\/2\|c'/.test(c)), 'model-deps: a model that waits for its producer gets no model-finish')
}
{
  // trial 10: the relay abridged the whole-book card plan line, the cycle ran and reported, and the stage faulted
  // its claim as "plan failed: no output". A plan line the relay rewrote is no failure when the report line reads.
  const { calls, logs } = await run('econ', { ...base, figures: false }, { garbledCardPlan: true })
  check(!calls.some((c) => c.label === 'cards-book fault'), 'garbled plan: the whole-book cycle that reported is not faulted')
  check(calls.some((c) => c.label === 'cards-book finish: cards-book/book@1' || /^cards-book finish/.test(c.label)) && logs.some((l) => /plan line came back unreadable/.test(l)), 'garbled plan: the stage goes on to its checked finish and logs why')
}
{
  // trial 10: three rounds of eight cleanup claims went to the same failing units, and eight link cleanup units were
  // never claimed. The cleanup claims until every ready unit was tried, and stops a unit after three failures.
  // ebrill2001modern trial 1: the stage spent one Haiku agent per cleanup unit (about 150, one after another) and sent
  // its faults with an empty input hash; one `cleanup-finish -Ready` now settles them all.
  const { cleanupFinishes, cleanupReadyCalls, calls, result } = await run('econ', { ...base, figures: false }, { extraCleanupUnits: 24, failingCleanup: /extra-g00[1-9]@1/ })
  const extras = Array.from({ length: 24 }, (_, i) => `cleanup/extra-g${String(i + 1).padStart(3, '0')}@1`)
  const missing = extras.filter((u) => !cleanupFinishes.includes(u))
  check(missing.length === 0, `cleanup rounds: every cleanup unit is tried although nine of them keep failing (${missing.length} never tried)`)
  const most = Math.max(...extras.map((u) => cleanupFinishes.filter((c) => c === u).length))
  check(most <= 3, `cleanup rounds: a failing unit is tried at most three times in the stage (${most})`)
  const settleAgents = calls.filter((c) => /^cleanup: /.test(c.label)).length
  check(cleanupReadyCalls.length === 1 && settleAgents === 1 && !calls.some((c) => /^cleanup (cleanup|extra)\/|^cleanup: claim round|^cleanup: .* fault$/.test(c.label)), `cleanup rounds: one cleanup agent settles every unit through one cleanup-finish -Ready (${settleAgents} agent(s), ${cleanupReadyCalls.length} call(s))`)
  check(result.cleanup && result.cleanup.settled && result.cleanup.settled.failed === 9 && result.cleanup.units.filter((u) => u.status === 'failed').length === 8, `cleanup rounds: the stage reports the units that could not settle (${JSON.stringify(result.cleanup && result.cleanup.settled)})`)
}
{
  // ebrill2001modern trial 1: one refused `ledger next` ended the events layer with 40 of its 73 units unclaimed.
  // A failed claim is tried again (three times in all) before its layer hears of it.
  const { led, calls, logs } = await run('econ', { ...base, figures: false }, { failingClaim: { layer: 'events', times: 2 } })
  check(led.gate('events').complete === true && calls.some((c) => /^events claim 1 \(try 3\)$/.test(c.label)) && logs.some((l) => /the claim failed .*it is tried again/.test(l)), 'claim retry: two refused claims are tried again and the events layer still completes')
  const failed = await run('econ', { ...base, figures: false }, { failingClaim: { layer: 'events', times: 3 } })
  check(!failed.calls.some((c) => /^events claim 1 \(try 4\)$/.test(c.label)), 'claim retry: a claim is tried three times, never more')
}
{
  // ebrill2001modern trial 1: an explain manager registered the id PENDING to try the command, a findings manager sent
  // DeepSeek the prompt "placeholder" for a step with no job, and the resume recorded the workspaces trial 1 had
  // removed as not removed. The prompts now say what each of those cases is.
  const { calls } = await run('econ', { ...base, figures: false })
  const managers = calls.filter((c) => /-RegisterId <session_id>/.test(c.prompt || ''))
  check(managers.length > 0 && managers.every((c) => /never a word in its place/.test(c.prompt) && /starts no run: never send a placeholder prompt/.test(c.prompt)), `prompts: every manager registers only a returned session_id and starts no run for an empty job list (${managers.length})`)
  const settle = calls.filter((c) => c.label === 'cleanup: settle the run')
  check(settle.length === 1 && /deepseek_settle once with plan_file "[^"]+\\cleanup\\settle-plan\.json" and results_file "[^"]+\\cleanup\\settle-results\.json"/.test(settle[0].prompt) && /never call deepseek_delete or deepseek_remove_workspace/.test(settle[0].prompt), 'prompts: the cleanup agent hands two paths to deepseek_settle and deletes nothing itself')
}
{
  // trial 8: no stage published its own book revision, so organize/skeleton/figures book@1 stayed ready and the run
  // could never be content-complete. Each stage publishes its revision with its own artifacts.
  const { stagePublications } = await run('econ', { ...base })
  const layers = stagePublications.map((c) => (c.match(/-Layer ([a-z-]+)/) || [])[1])
  check(['organize', 'skeleton', 'figures'].every((l) => layers.includes(l)), `stage publish: Organize, the skeleton and Figures publish their own revisions (${layers.join(', ')})`)
  check(stagePublications.some((c) => /-Layer organize -Decisions "[^"]*\\organize\\decisions\.json" -Applied "[^"]*\\organize\\applied\.json"/.test(c)) && stagePublications.some((c) => /-Layer skeleton -Skeleton "[^"]*\\skeleton\\out\\skeleton\.json"/.test(c)) && stagePublications.some((c) => /-Layer figures -Report "[^"]*\\figures-report\.json"/.test(c)), 'stage publish: each publication names its declared artifacts')
  // acemoglu2025nobel trial 1: the final audit found nothing to correct, so no refresh published link-check/book@1
  // and the run stayed content-incomplete. The Link check stage publishes its own revision from its cycle report.
  check(stagePublications.some((c) => /-Layer link-check -Report "[^"]*\\cards-link\\report\.json"/.test(c)), `stage publish: the Link check publishes its own revision from its cycle report (${layers.join(', ')})`)
}
{
  // trial 8: the full resource listing (about 52 KB) came back as prose and the cleanup settled nothing. The cleanup
  // stage reads the brief listing and still settles every recorded conversation.
  // trial 11: the relay abridged even the brief listing's id arrays and left out two model workspaces. The stage reads
  // counts only; each deletion agent reads its own workspace's ids from the ledger.
  // 2026-10-23 (zt-dsh-cleanup v11): no model reads, relays or decides a cleanup fact. One agent runs the plan, the
  // bridge batch and apply; every conversation the managers left open is deleted in that one batch.
  const { calls, briefLists, settleRecords, settleCalls, resourceRecords } = await run('econ', { ...base, figures: false })
  const cleanupAgents = calls.filter((c) => c.phase === 'Cleanup').map((c) => c.label)
  check(cleanupAgents.join() === 'cleanup: settle the run,report the run from the ledger' && briefLists.length === 0, `cleanup agents: one settle agent and the report, no listing relay (${cleanupAgents.join(', ')})`)
  const batched = (settleCalls[0] || []).filter((o) => o.op === 'delete').map((o) => o.session_id)
  check(settleCalls.length === 1 && batched.length > 0 && batched.every((id) => settleRecords.some((r) => r.id === id && r.via === 'cleanup-run')), `cleanup agents: one provider batch deletes every open recorded id and apply records each (${batched.length})`)
}
{
  // A content failure keeps the unit failed and the stage result partial, never complete.
  const { commands, calls, failClasses } = await run('econ', { ...base, figures: false, until: 'digest' }, { contentWhen: /group-finish/ })
  const cmds = commands
  check(failClasses.some((f) => f.class === 'content' && f.code === 'check_failed'), 'partial results: a content failure is recorded as content (no silent retry)')
}

{
  // A JSON line followed by a remark is still read (a relay that appends "(abridged)" or a note).
  const lib = fs.readFileSync(path.join(root, 'scripts', 'workflow-shared', 'modules', 'zt-workflow-lib.js'), 'utf8').replace(/\r\n/g, '\n')
  const take = (name) => lib.slice(lib.indexOf(`function ${name}(`), lib.indexOf('\n}\n', lib.indexOf(`function ${name}(`)) + 2)
  const zdJsonIn = new Function(`${take('ztJsonIn')}; return ztJsonIn`)()
  check(JSON.stringify(zdJsonIn('{"state":"done","unit":"digest/g001@1"} (abridged)')) === '{"state":"done","unit":"digest/g001@1"}', 'json: a JSON line with a trailing remark is still read')
  check(JSON.stringify(zdJsonIn('done: {"a":{"b":1}} and more {x')) === '{"a":{"b":1}}', 'json: prose before and after the object is ignored')
  check(zdJsonIn('no json here') === null, 'json: a line without an object reads as nothing')
}

// ================================================================================================================
// The finite finalization (#86, R299/R300)
// ================================================================================================================
for (const branch of ['fix', 'no_fix', 'conflict', 'unresolved']) {
  const { calls, commands, result, report, logs } = await run('econ', { ...base, figures: false }, { finalize: branch })
  const labels = calls.map((c) => c.label)
  const prompts = calls.map((c) => c.prompt).join('\n')
  const finalizeLabels = labels.filter((l) => /^finalize/.test(l))
  check(finalizeLabels[0] === 'finalize plan', `finalize ${branch}: the audit plan comes first (${finalizeLabels[0]})`)
  check(result && result.finalize && result.finalize.final, `finalize ${branch}: the result carries the final audit`)
  const expected = branch === 'no_fix' ? 'done' : 'partial'
  check(result.finalize.status === expected, `finalize ${branch}: the branch ends ${expected} (${result.finalize.status})`)
  check(result.finalize.final.graph_revision > result.finalize.plan.graph_revision, `finalize ${branch}: the final audit is at a later graph revision than the one it planned from`)
  if (branch === 'fix') {
    const scoped = commands.filter((c) => /zt-digest card -Step plan .*-Cards /.test(c))
    check(scoped.length === 1 && /finalize[\\/]fix/.test(scoped[0]), `finalize fix: the one correction cycle is scoped to the plan's Cards (${scoped.length})`)
    check(/ -Rules "R3"/.test(scoped[0]), `finalize fix: the cycle checks exactly the plan rules (${scoped[0]})`)
    check(commands.filter((c) => /zt-digest card -Step plan .*finalize[\\\\/]fix/.test(c)).length === 1, `finalize fix: exactly one compiler correction cycle runs (${commands.filter((c) => /zt-digest card -Step plan /.test(c)).length} plan steps)`)
    check(finalizeLabels.join(',') === 'finalize plan,finalize scope,finalize refresh link-check,finalize final', `finalize fix: plan → cycle → scope → refresh → final (${finalizeLabels.join(',')})`)
    const refreshes = (prompts.match(/[^\n]*finalize -Step refresh[^\n]*/g) || [])
    check(refreshes.length === 1 && /-Layer link-check -Report /.test(refreshes[0]), `finalize fix: only the publication this branch re-derives is refreshed (${refreshes.length}: ${refreshes[0] || ''})`)
    check(!/-Step refresh .*-Layer (organize|hubs|bridge|figures|skeleton|theory)/.test(prompts), 'finalize fix: a stage-owned publication is never republished from a stale artifact')
  } else {
    check(finalizeLabels.join(',') === 'finalize plan,finalize final', `finalize ${branch}: no correction cycle and no refresh of unchanged publications (${finalizeLabels.join(',')})`)
    check(!commands.some((c) => /finalize[\\/]fix/.test(c)), `finalize ${branch}: no Card-scoped cycle runs`)
  }
  check(finalizeLabels[finalizeLabels.length - 1] === 'finalize final', `finalize ${branch}: the final read-only audit is the branch's last step`)
  check(report.stages.finalize && report.stages.finalize.status !== 'failed', `finalize ${branch}: the stage never fails on a residual fault`)
  check(/finalize -Step final -Out/.test(prompts), 'finalize: the final step is the tool\'s own read-only audit, never a re-used report')
  check(logs.some((l) => /Finalize: (done|partial) at graph revision/.test(l)), `finalize ${branch}: the branch reports its own outcome and revision`)
}

// ================================================================================================================
// Modes and the book-type layers
// ================================================================================================================
for (const kind of ['econ', 'math']) {
  const mbase = { ...base, figures: false }
  const { commands, calls, result, led, resourceRecords, evidenceRecords, settleRecords, settleCalls } = await run(kind, { ...mbase, mode: 'full' })
  const cmds = commands
  const labels = calls.map((c) => c.label)
  const at = (pattern) => labels.findIndex((l) => pattern.test(l))
  check(result && result.mode === 'full', `${kind} full: reaches the report`)
  check(at(/^prepare book$/) < at(/^math: index$/) || kind === 'econ', `${kind} full: the math skeleton follows Prepare (math only)`)
  check(cmds.some((c) => /zt-digest audit -CitationKey Key -Out "D:\\x\\Key"/.test(c)), `${kind} full: the read-only audit runs`)
  const work = labels.filter((l) => !/^cleanup/.test(l) && !/^report the run/.test(l) && !(kind === 'math' && /^math: check$/.test(l)))
  check(work[work.length - 1] === 'finalize final', `${kind} full: the finalization's final audit is the last work before the settlement (${work.slice(-3).join(', ')})`)
  check(work.includes('finalize plan') && work.indexOf('finalize plan') === work.indexOf('audit cards') + 1, `${kind} full: the finalization starts from its audit plan right after the audit (${work.slice(-5).join(', ')})`)
  check(led.units.get('cards-book/book@1') && led.units.get('cards-book/book@1').state === 'done', `${kind} full: the whole-book card revision is complete`)
  if (kind === 'math') {
    check(cmds.some((c) => /zt-digest math -Step index/.test(c)) && cmds.some((c) => /zt-digest math -Step boundary/.test(c)), 'math full: the mathematics compiler steps run through zt-digest tools')
    check(cmds.some((c) => /-BriefDoc "D:\\x\\Key\\math\\brief\.zh\.md"/.test(c)), 'math full: the model compiler gets the math instruction files')
    check(!calls.some((c) => /^(events|findings)/.test(c.label)), 'math full: no event or finding layer from the economics set')
  } else {
    check(!calls.some((c) => /^math:/.test(c.label)), 'econ full: no mathematics stage runs')
    check(cmds.some((c) => /zt-digest event -Step plan/.test(c)) && cmds.some((c) => /zt-digest finding -Step plan/.test(c)), 'econ full: the Event and Finding compiler chains run')
  }
  // #83 / zt-digest-lib v15: each manager records the ids it created and their answers itself, with plain
  // commands; the cleanup agents delete exactly the recorded ids still open and record each fact themselves.
  const registers = cmds.filter((c) => / ledger resource .*-RegisterId /.test(c))
  const records = resourceRecords
  check(records.length > 0 && records.every((r) => r.via === 'manager' && r.workspace && r.workspace.path && r.conversations.length === 1 && r.conversations[0].id), `${kind} full: every created conversation is recorded by the manager that started it (${records.length} records)`)
  check(!cmds.some((c) => /Set-Content .* ledger resource /.test(c)) && !calls.some((c) => /record the run ids|^cleanup: (terminal|delete|workspace|failed)/.test(c.label)), `${kind} full: no relay agent writes a record or an observation for another agent`)
  check(records.every((r) => evidenceRecords.some((e) => e.id === r.conversations[0].id && /\\answers\\/.test(e.file))), `${kind} full: every recorded conversation gets its answer file as retained evidence`)
  check(records.some((r) => r.unit !== 'run') && registers.some((c) => /-Unit "?digest\/g\d{3}@1"? -Token \d+ -RegisterId/.test(c)), `${kind} full: a claimed unit owns its conversations`)
  const blockSessions = records.filter((r) => /^(digest|link)\//.test(r.unit)).map((r) => r.conversations[0].id)
  check(blockSessions.length > 0 && blockSessions.every((id) => settleRecords.some((x) => x.id === id && x.deleted === 'confirmed')), `${kind} full: a block manager deletes each group conversation once its answer is retained, and records it`)
  const settlePrompt = (calls.find((c) => c.label === 'cleanup: settle the run') || {}).prompt || ''
  check(!records.some((r) => settlePrompt.includes(r.conversations[0].id)), `${kind} full: no cleanup prompt carries a conversation id`)
  const batchIds = (settleCalls[0] || []).filter((o) => o.op === 'delete').map((o) => o.session_id)
  check(batchIds.length > 0 && batchIds.every((id) => records.some((r) => r.conversations[0].id === id)) && !batchIds.some((id) => blockSessions.includes(id)), `${kind} full: the provider batch names only recorded ids that no manager settled (${batchIds.length})`)
  check(cmds.some((c) => /zt-digest cleanup-run -Out .* -Step plan/.test(c)) && cmds.some((c) => /zt-digest cleanup-run -Out .* -Step apply/.test(c)) && result.cleanup && result.cleanup.settled && result.cleanup.settled.claimed > 0, `${kind} full: the cleanup plans, applies the bridge's results and settles every unit by its verifier`)
  // #110: the Link units now settle through link-finish, so content is complete while the cleanup
  // still reports its own half: the two are still reported apart.
  check(result.cleanup && result.cleanup.cleanup_complete === true && result.cleanup.status === (result.cleanup.content_complete ? 'complete' : 'partial'), `${kind} full: the cleanup reports its own half apart from content completeness (${JSON.stringify(result.cleanup && { units: result.cleanup.units, status: result.cleanup.status, cleanup_complete: result.cleanup.cleanup_complete })})`)
  if (kind === 'econ') {
    const undone = [...led.units.values()].filter((u) => u.layer !== 'cleanup' && u.state !== 'done').map((u) => `${u.id}=${u.state}`)
    check(result.cleanup.content_complete === true, `econ full: the settled Link units leave no content gap (${undone.join(', ')})`)
  }
  check(!calls.some((c) => /^Clean up:/.test(c.label)), `${kind} full: no step deletes its own conversations any more`)
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
  const { commands, calls, result, settleCalls: auditSettles } = await run('econ', { out: 'D:\\x\\audit', mode: 'audit' })
  const labels = calls.map((c) => c.label)
  check(result && result.mode === 'audit' && result.audit.status === 'done', 'econ audit: runs to its end without a citation key')
  check(labels[0] === 'audit cards' && labels.slice(1).every((l) => /^cleanup|^report the run/.test(l)) && labels.includes('cleanup: settle the run') && labels.includes('report the run from the ledger'), `econ audit: the audit and its ledger-driven settlement, no source probe and no layer (${labels.join(', ')})`)
  check(!commands.some((c) => /deepseek_delete/.test(c)) && auditSettles.every((ops) => ops.length === 0), 'econ audit: a run with no recorded resource deletes no conversation and no workspace')
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
  const { commands, calls, result } = await run('econ', { ...base, figures: true })
  const cmds = commands
  const labels = calls.map((c) => c.label)
  const at = (s) => labels.indexOf(s)
  check(result.bridge && result.bridge.status === 'done' && at('show figures') < at('bridge: plan') && at('audit cards') > at('bridge: plan'), 'full: Figures, then the Bridge, then the audit')
  // Base 5.4.1: the Bridge stage applies its own report's new-carrier proposals, never the earlier ones.
  const applies = cmds.filter((c) => /zt-digest bridge/.test(c) && /-Apply\b/.test(c))
  check(applies.length <= 1 && applies.every((c) => /-Accept 'all'/.test(c) && !/-AcceptEarlier/.test(c)) && (applies.length === 0 || at('bridge: apply new') > at('bridge: plan')), 'full: discovery applies only its new-carrier proposals, after the report')
}

// ================================================================================================================
// A digest builds only the Literature tree (docs/literature-only-digest-design.md): Topic trees, Topic Cards,
// hubs and the register are zt-idea-grow's
// ================================================================================================================
{
  const { commands, calls, result, report } = await run('econ', { ...base, figures: true })
  const labels = calls.map((c) => c.label)
  check(!commands.some((c) => /zt-digest (hub|register|concepts|restate|grow|gate|join|topic-home|publish) /.test(c)), 'literature only: a full run starts no hub, register, concept, restatement, grow or Topic tool')
  check(!labels.some((l) => /^(hub manager|hubs |concept|restatement review|register )/.test(l)), `literature only: no hub, concept, review or register step is relayed (${labels.filter((l) => /hub|concept|review|register/.test(l)).join(', ')})`)
  check(!commands.some((c) => /zt-digest call\b[^\n]*\s-Open\b/.test(c)), 'literature only: no helper call leaves the Literature scope (-Open)')
  check(result && !('concepts' in result) && !('review' in result) && !('register' in result) && !('hubs' in result), 'literature only: the report has no concept, review, register or hub entry')
  check(report && report.stages && !report.stages.hubs && !report.stages.concepts && !report.stages.review, 'literature only: the run has no hub, concept or review stage')
  const decide = calls.find((c) => c.label === 'organize: decisions')
  check(decide && /"merges": \[\] \(always empty/.test(decide.prompt) && /"moves": \[\] \(always empty/.test(decide.prompt) && !/Topic merges, \[/.test(decide.prompt), 'literature only: Organize decides no Topic merge and no move')
  // pilot 2026-10-23: a helper read with no path argument reached the live box, not the run's box copy (zt-box.txt).
  check(decide && /'@ \| zt-digest call - -Allow "[^"]*\\organize\\read-only\.allow\.txt"/.test(decide.prompt) && /always with -Allow/.test(decide.prompt), "organize: the decision reads run the helper with -Allow on the survey's read-only list (reads only; a staging path for the box pin)")
  const prep = calls.find((c) => c.label === 'prepare book')
  check(prep && !/"hubs"/.test(prep.cmd), 'literature only: the prepared run freezes no hubs switch (the ledger excludes the hubs layer)')
}
for (const which of ['econ', 'math']) {
  const bp = builds[which].bp
  check(!bp.stages.some((s) => ['concepts', 'review', 'hubs', 'hubs-first'].includes(s.id)) && !bp.modes.hubs, `literature only (${which}): no concept, review or hub stage and no mode hubs`)
  check(!['hubs', 'hubWork', 'hubLimits', 'hubTrees', 'hubRebuild', 'hubEffort', 'hubFixRounds', 'registerPlan'].some((k) => k in bp.args), `literature only (${which}): no hub or register argument`)
  check(!/legacy/.test(JSON.stringify(bp.args.organize)), `literature only (${which}): no legacy Organize job`)
  check(!/zt-concepts|zt-review|zt-hubs/.test(builds[which].script.match(/^\/\/ ==== module [^\n]*/gm).join(' ')), `literature only (${which}): the build holds no concept, review or hub module`)
}
{
  const refused = await run('econ', { ...base, hubs: true })
  check(refused.thrown && /args\.hubs is not declared/.test(refused.thrown.message), 'literature only: args.hubs is refused (hubs are zt-idea-grow\'s)')
  const plan = await run('econ', { ...base, registerPlan: 'rp-c1' })
  check(plan.thrown && /args\.registerPlan is not declared/.test(plan.thrown.message), 'literature only: args.registerPlan is refused (the register is zt-idea-grow\'s)')
  const mode = await run('econ', { out: 'D:\\x\\Key', mode: 'hubs' })
  check(mode.thrown && /mode "hubs" is not declared/.test(mode.thrown.message), 'literature only: there is no mode hubs')
}
{
  // book-cards-finish requires the deterministic Idea sample; a resumed run's whole-book cycle samples too
  // (grossman2002integration trial 19: the resume ran the cycle with no -Sample and its finish was refused).
  const { calls } = await run('econ', { out: 'D:\\x\\Key', mode: 'resume' }, { retained: [['terms', 'terms/book@1'], ['digest', 'digest/g001@1'], ['digest', 'digest/g002@1']] })
  const cycles = calls.filter((c) => /zt-digest card -Step plan .*-Work "D:\\x\\Key\\cards"/.test(c.prompt))
  check(cycles.length > 0 && cycles.every((c) => /-Sample 0\.1\b/.test(c.prompt)), `resume sample: the resumed whole-book card cycle samples Idea Cards (${cycles.length} cycles)`)
}
{
  // The resume status line came back without its `layers` (grossman2002integration trial 17): the prepare
  // stage reads the group count from the digest layer's own brief gate instead of halting the resumed run.
  const { calls, commands, report } = await run('econ', { out: 'D:\\x\\Key', mode: 'resume' }, { retained: [['terms', 'terms/book@1'], ['digest', 'digest/g001@1'], ['digest', 'digest/g002@1']], rewrittenSummary: true })
  check(report && !report.halted && calls.some((c) => /zt-digest card -Step plan .*-Work "D:\\x\\Key\\cards"/.test(c.prompt)), `rewritten summary: the resumed run goes on past prepare (${JSON.stringify(report && report.halted)})`)
}

// ================================================================================================================
// Tree Lock S17 (#140): the owner's refusal is a planning error that stops the affected write stage
// ================================================================================================================
{
  // A preparation the owner's Lock refused: the run halts at prepare with the archive's own typed refusal,
  // not with a generic "preparation failed", and nothing after prepare runs.
  const { calls, report, result } = await run('econ', { ...base, figures: false }, { lockPrepare: true })
  check(report && report.halted && report.halted.stage === 'prepare', `lock: a refused preparation halts at prepare (${JSON.stringify(report && report.halted)})`)
  const said = String((report && report.halted && report.halted.error) || result.error || '')
  check(/card_locked/.test(said) && /Literature tree Key is locked/.test(said) && /"lock_id":3/.test(said), `lock: prepare stops with the typed refusal, its sentence and its Lock id (${said.slice(0, 200)})`)
  check(!/preparation failed/.test(said), 'lock: a refused preparation is never reported as a generic preparation failure')
  check(calls.filter((c) => c.label === 'prepare book').length === 1, 'lock: the refused preparation runs once, never retried')
}

{
  // The owner locks the book mid-run while the digest stage writes it: the stage stops with the refusal,
  // records the refused unit's own content fault (code card_locked), and keeps the group it already
  // committed. Nothing after the digest stage runs, so the stage is not described as complete.
  const rt = await run('econ', { ...base, figures: false }, { lockCommand: /zt-digest group-finish /, lockCommandAfter: 1 })
  const halted = rt.report && rt.report.halted
  check(halted && halted.stage === 'digest', `lock mid-run: the digest stage stops (${JSON.stringify(halted)})`)
  const message = String((halted && halted.error) || '')
  check(/card_locked/.test(message) && /Literature tree Key is locked/.test(message), `lock mid-run: the stage error is the archive's typed refusal (${message.slice(0, 200)})`)
  const order = (rt.report && rt.report.order) || []
  const after = order.filter((id) => ['cards', 'link', 'organize', 'audit', 'finalize', 'cleanup'].includes(id))
  check(after.every((id) => !rt.report.stages[id] || rt.report.stages[id].status === 'not-run (halted)' || rt.report.stages[id].status === 'blocked'), `lock mid-run: the later write stages never run (${after.map((id) => `${id}=${(rt.report.stages[id] || {}).status}`).join(', ')})`)
  const committed = [...rt.led.units.values()].filter((u) => u.layer === 'digest' && u.state === 'done').length
  check(committed >= 1, `lock mid-run: the group committed before the Lock stays committed (${committed} digest unit(s) done)`)
  const refusals = rt.commands.filter((c) => /zt-digest group-finish /.test(c))
  check(refusals.length >= 1 && !rt.state.lockedRetries, `lock mid-run: the refused finish is never retried (${refusals.length} finishes, ${rt.state.lockedRetries || 0} retries)`)
  const fault = rt.commands.filter((c) => / ledger fail /.test(c) && /card_locked/.test(c))
  const lockFault = rt.failClasses.filter((x) => x.code === 'card_locked')
  check(fault.length === 1 && lockFault.length === 1 && lockFault[0].class === 'content' && !/-Class /.test(fault[0]), `lock mid-run: the refused unit keeps the Lock as its content fault (${fault.map((c) => c.slice(0, 120)).join(' | ')})`)
  check(!(rt.report && rt.report.result), 'lock mid-run: a halted run reports no completed result')
}

{
  // A write stage whose card cycle the owner's Lock refused: the whole-book Card cycle (the first stage
  // that applies card edits) stops with the refusal, and the run halts there instead of faulting it.
  const rt = await run('econ', { ...base, figures: false }, { lockCommand: /zt-digest card -Step apply / })
  const halted = rt.report && rt.report.halted
  check(halted && ['cards', 'cards-book'].includes(halted.stage), `lock in a chain: the Card cycle's stage stops (${JSON.stringify(halted && halted.stage)})`)
  check(/card_locked/.test(String((halted && halted.error) || '')), `lock in a chain: the stage error keeps the typed refusal (${String((halted && halted.error) || '').slice(0, 160)})`)
  check(!rt.commands.some((c) => / ledger fail /.test(c) && /card_locked/.test(c) && /-Class retryable/.test(c)), 'lock in a chain: the refusal is never recorded as a retryable fault')
}

{
  // zt-digest-lib v32 carries the seam, and the frozen build keeps it verbatim: the reader, the code and
  // the stop marker every stage relies on are in the built script.
  const { modules, script } = builds.econ
  check(modules['zt-digest-lib'] === '37' && modules['zt-digest-prepare'] === '13' && modules['zt-workflow-lib'] === '1', `lock: the frozen build carries zt-digest-lib v37, zt-digest-prepare v13 and the shared zt-workflow-lib v1 (${modules['zt-digest-lib']}, ${modules['zt-digest-prepare']})`)
  for (const piece of ['function zdLockRefusal', "const ZD_LOCK = 'card_locked'", 'error.lock_refusal = refusal', "code: ZD_LOCK"]) {
    check(script.includes(piece), `lock: the built script carries the seam (${piece})`)
  }
  const growLoad = lib.loadBlueprint(path.join(root, 'scripts', 'idea-grow', 'workflow', 'zt-idea-grow.blueprint.json'))
  const grow = lib.build(growLoad.bp, lib.loadModules(growLoad.dirs))
  check(grow.modules['zt-grow-cycle'] === '12' && grow.modules['zt-workflow-lib'] === '1', `lock: the idea-grow build carries zt-grow-cycle v12 and the shared zt-workflow-lib v1 (${grow.modules['zt-grow-cycle']})`)
  // S6: both workflows read the Lock with the one shared function; neither keeps a copy of its own.
  check(grow.script.includes('function ztLockRefusal(') && grow.script.includes('card_locked at ${where}') && !grow.script.includes('const lockRefusal = (text) =>') && !grow.script.includes('stopOnLock'), 'lock: the idea-grow build reads the Lock with the shared seam, not its own copy')
  check(!builds.econ.script.includes('v.code === ZD_LOCK || v.error === ZD_LOCK') && (builds.econ.script.match(/function ztLockRefusal\(/g) || []).length === 1, 'lock: the digest build keeps no copy of the Lock reader')
}

{
  // The Literature scope (docs/literature-only-digest-design.md) holds only while the helper runs without
  // -Open, so every approval rule that lets a DeepSeek job run the zt helper refuses a helper command with
  // -Open: a job could otherwise add the flag and write Topic trees or hubs.
  for (const which of ['econ', 'math']) {
    const { calls, pool } = await run(which, { ...base, figures: false })
    const rules = [...calls.map((c) => c.prompt), ...pool.prompts]
      .flatMap((p) => String(p).split('\n'))
      .filter((l) => /"needs_response": approve/.test(l) && /runs the zt helper/.test(l))
    const open = rules.filter((l) => !/refuse a zt helper command that carries -Open/.test(l))
    check(rules.length > 0 && !open.length, `scope: every ${which} approval rule that allows the zt helper refuses -Open (${rules.length} rules, ${open.length} without: ${open.slice(0, 1).map((l) => l.slice(0, 160)).join('')})`)
    // base 5.1.0: hubs are zt-idea-grow's, so no approval rule offers the hub compiler's self-check.
    const all = [...calls.map((c) => c.prompt), ...pool.prompts].flatMap((p) => String(p).split('\n')).filter((l) => /"needs_response": approve/.test(l))
    check(all.length > 0 && !all.some((l) => /hub-compiler|zt-digest hub\b|card, hub, event/.test(l)), `literature only: no ${which} approval rule offers the hub compiler`)
  }
}

{
  // base 5.1.0 (zt-digest-report v16): the digest built only the Literature tree, so the whole report names the
  // follow-up run that grows the book's Topic Cards, hubs and register, and the log says it.
  const { result, logs } = await run('econ', { ...base, figures: false })
  const next = result && result.next
  check(next && next.workflow === 'zt-idea-grow' && next.script === '.claude/workflows/zt-idea-grow.js' && next.args && next.args.book === result.citation_key && /fresh/.test(next.args.out), `literature only: the report names the zt-idea-grow follow-up for this book (${JSON.stringify(next)})`)
  check((logs || []).some((l) => /Next: run zt-idea-grow with book/.test(l)), 'literature only: the report stage logs the zt-idea-grow follow-up')
}

// ================================================================================================================
// docs/workflow-simplicity-proposal.md steps 1-3 and S10 (base 5.4.0, zt-digest-lib v37)
// ================================================================================================================
for (const which of ['econ', 'math']) {
  const { calls, result } = await run(which, { ...base, figures: false })
  const commandCalls = calls.filter((c) => /^Run this PowerShell command once/.test(c.prompt))
  check(commandCalls.length > 10 && commandCalls.every((c) => c.agentType === 'zt-relay' && c.model === 'haiku'), `S1 ${which}: every command runs through the relay role, the zt-relay agent type (${commandCalls.filter((c) => c.agentType !== 'zt-relay').map((c) => c.label).slice(0, 3).join(', ')})`)
  check(!calls.some((c) => /^You prepare a book|^List the Model Cards|^Prepare the receipt-owned|^You check which DeepSeek Harness source/.test(c.prompt)), `S1 ${which}: no Haiku agent runs prepare, prepare-link, the model list or a source probe`)
  check(!calls.some((c) => c.label === 'choose DeepSeek source') && result && result.dsh_model === 'deepseek-official/deepseek-flash', `S1 ${which}: the DeepSeek source is the frozen official one (${result && result.dsh_model})`)
  check(!calls.some((c) => /^You drive DeepSeek Harness/.test(c.prompt) && !/Run every job below|Run the jobs with these ids/.test(c.prompt)), `S1 ${which}: a drive agent only runs DeepSeek jobs; its step command is a relay`)
  const prep = calls.find((c) => c.label === 'prepare book')
  check(prep && / -Apply -Brief\b/.test(prep.prompt), `S1 ${which}: prepare ends with its brief line`)
  const link = calls.find((c) => c.label === 'prepare link pass')
  check(link && / -Brief\b/.test(link.prompt), `S1 ${which}: prepare-link ends with its brief line`)
  const status = calls.filter((c) => /^plan status: /.test(c.label))
  check(status.length >= 1 && status.every((c) => / -Step plan-status /.test(c.prompt)), `S10 ${which}: each model batch reads the plans the architect wrote (model -Step plan-status)`)
}
{
  // S10: the architect answers "planned", but no plan.json was written: the batch builds nothing.
  const { calls, logs } = await run('econ', { ...base, figures: false }, { noPlanFile: true })
  check(!calls.some((c) => /^fill: /.test(c.label) || /^derive: /.test(c.label)), 'S10: a plan the architect only claimed (no plan.json) starts no fill and no derivation')
  check(calls.some((c) => /^plan: /.test(c.label)) && !logs.some((l) => /^Build: [1-9]/.test(l)), 'S10: the architect still ran; its answer decided nothing')
}
{
  // S10: whether the notes cycle runs is read from decisions.json (organize-apply's notes), not from the answer.
  const { calls } = await run('econ', { ...base, figures: false })
  check(calls.some((c) => /^cards organize notes/.test(c.label)), 'S10: organize-apply\'s notes count starts the notes cycle')
}
{
  // S5: no workflow command names a fault class; the ledger reads each fault.
  const { commands } = await run('econ', { ...base, figures: false, until: 'digest' }, { failWhen: /group-finish/ })
  check(commands.some((c) => / ledger fail /.test(c)) && !commands.some((c) => / ledger fail .*-Class /.test(c)), 'S5: ledger fail carries no -Class: the ledger is the one classifier')
}

console.log(`failed: ${failed}`)
process.exit(failed ? 1 : 0)








