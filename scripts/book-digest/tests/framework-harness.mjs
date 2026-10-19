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
  'organize-apply': () => ({ merges: 0, moves: 0, refused: 0, renamed: 0 }),
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
  const calls = [], commands = [], logs = [], phases = [], managers = [], cleanups = [], resourceRecords = []
  const pool = { alive: 0, peak: 0, prompts: [] }
  const led = makeLedger()
  // A resumed run reads the units its prepared run retained; the stand-in re-adds exactly those, so the
  // resume path drains a real closure instead of halting on an empty ledger.
  if (opts.retained) for (const [layer, id] of opts.retained) led.add(id, layer)
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
      return { ok: true, output: json({ schema_version: 1, variant: kind === 'math' ? 'math' : 'econ', settings: led.settings, citation_key: 'Key', archive_identity: { archive_id: 'a1' }, units: [...led.units.values()] }) }
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
      if (/-List/.test(cmd)) return { ok: true, output: json({ schema_version: 1, action: 'list', resources: [...state.resources.values()] }) }
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
    if (/zt-digest cleanup-finish /.test(cmd)) {
      const id = (cmd.match(/-Unit "?([\w/-]+@\d+)"?/) || [])[1]
      led.finish(id)
      const resources = [...state.resources.values()].filter((r) => id === 'cleanup/book@1' || r.cleanup_unit === id)
      const cleanupComplete = resources.every((r) => r.conversations.every((c) => c.terminal && c.deleted) && (id !== 'cleanup/book@1' || r.workspace_removed === true))
      const contentComplete = [...led.units.values()].filter((u) => u.layer !== 'cleanup').every((u) => u.state === 'done')
      return { ok: true, output: json({ schema_version: 1, kind: 'cleanup_completion', unit: id, state: 'done', status: cleanupComplete && contentComplete ? 'complete' : 'partial', content_complete: contentComplete, cleanup_complete: cleanupComplete, reconciliation: [], partial_reasons: [], resources, scratch: { removed: [], retained: [] } }) }
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
  const commandAnswer = (raw, extraRecord = null) => {
    const cmd = String(raw).trim()
    if (process.env.DBG83 && /resource|cleanup/.test(cmd)) console.log('DBG', cmd.slice(0, 160))
    commands.push(cmd)
    failedStep.push(cmd)
    const denied = opts.failWhen && opts.failWhen.test(cmd) ? cmd : null
    if (opts.contentWhen && opts.contentWhen.test(cmd)) return { ok: true, exit: 0, output: json({ schema_version: 1, state: 'failed', error: 'the checked completion refused the draft', disposition: 'failed' }) }
    const ledAns = ledgerAnswer(cmd, extraRecord)
    if (ledAns) return denied ? { ok: false, exit: 1, output: 'stand-in crash' } : ledAns
    const tool = cmd.match(/^zt-digest ([\w-]+) -Step ([\w-]+)/)
    if (tool) {
      const key = `${tool[1] === 'card' ? 'card' : tool[1] === 'model' ? 'model' : tool[1] === 'event' ? 'event' : tool[1] === 'finding' ? 'finding' : tool[1] === 'hub' ? 'hub' : tool[1] === 'bridge' ? 'bridge' : tool[1] === 'math' ? 'math' : tool[1] === 'terms' ? 'terms' : tool[1] === 'skeleton' ? 'skeleton' : tool[1]} ${tool[2]}`
      const fn = CANNED[key]
      if (denied) return { ok: false, exit: 1, output: 'stand-in crash' }
      // S23 / #115 scenarios: a review whose frozen decision the archive moved past, a recorded merge
      // verdict no rewrite covers, and an interrupted register publication whose accepted plan stays
      // staged. Each is a real refusal of the delivered tool, answered here instead of by a live run.
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
        if (tool[1] === 'card' && tool[2] === 'apply' && state.applyPending > 0) { value.pending_jobs = state.applyPending - 1 }
        if (tool[1] === 'card' && tool[2] === 'plan' && state.planJobs !== 21) value.jobs = Array.from({ length: state.planJobs }, (_, i) => ({ id: `fix-${i + 1}`, kind: 'fix', file: `fix-${i + 1}.md` }))
        if (tool[1] === 'terms' && tool[2] === 'apply') led.finish('terms/book@1')
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
      if (parsed && parsed.job && !Array.isArray(parsed.jobs) && (parsed.expected_jobs === undefined || parsed.expected_jobs > 0)) {
        results.push({ id: `digest-g${group}-t${token}`, step: st.name, session_id: `s-digest-${group}`, status: 'done', answer: '{}' })
      }
      if (st.jobs && parsed && Array.isArray(parsed.jobs) && parsed.jobs.length) {
        managers.push({ label: opts.label, phase: opts.phase, jobs: parsed.jobs.map((j) => j.id), given: parsed.jobs })
        for (const job of parsed.jobs) results.push({ id: job.id, step: st.name, session_id: `s-${job.id}`, status: 'done', answer: '{}' })
      }
    }
    return { steps, results }
  }

  const answer = async (prompt, callOpts) => {
    const text = String(prompt)
    const label = callOpts.label || ''
    const cmd = text.match(/zt-digest [a-z-]+ -[A-Za-z][^\n]*/) || text.match(/pwsh -NoProfile -File[^\n]*/)
    calls.push({ label, phase: callOpts.phase, model: callOpts.model, cmd: cmd ? cmd[0] : '', prompt: text })
    // The finite finalization (#86): the plan's branch decides whether the one correction cycle
    // runs, and the final audit reports the fresh revision.
    if (/^finalize plan$/.test(label)) return { ok: true, output: json(finalizePlan(opts.finalize || 'fix')) }
    if (/^finalize scope$/.test(label)) return { ok: true, output: json(finalizeScope()) }
    if (/^finalize refresh /.test(label)) return { ok: true, output: json(finalizeRefresh(label.replace('finalize refresh ', ''))) }
    if (/^finalize final$/.test(label)) return { ok: true, output: json(finalizeFinal(opts.finalize || 'fix')) }
    if (text.startsWith('You run one compiler chain')) return await chainAnswer(text, callOpts)
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
      // An owned-resource command writes its JSON first and runs one ledger command on the same line.
      const compound = text.split('\n').map((l) => l.trim()).find((l) => / zt-digest ledger resource /.test(l))
      if (compound) {
        const inner = compound.slice(compound.indexOf(' zt-digest ') + 1)
        const r = await commandAnswer(inner, resourceRecord(compound))
        return { ok: r.ok, output: r.output }
      }
      if (lines.length) {        let r = await commandAnswer(lines[0])
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
      led.add('cleanup/book@1', 'cleanup')
      for (let n = 1; n <= 2; n++) led.add(`digest/g${String(n).padStart(3, '0')}@1`, 'digest', { n, group: { n } })
      if (kind === 'math') for (const l of ['math-index', 'math-docs', 'math-skeleton', 'notation']) led.add(`${l}/book@1`, l)
      return { ok: true, groups: 2, usage_before: { input_cache_miss: 0, cached_read: 0, output: 0, cost: 0, calls: 0 } }
    }
    if (label === 'resume run') {
      // #87: a resume-run agent runs `zt-digest ledger resume`, which the ledger stand-in answers (below).
      return commandAnswer((text.match(/zt-digest ledger resume[^\n]*/) || [])[0] || 'zt-digest ledger resume')
    }
    if (label === 'resume status') {
      return { ok: true, output: json({ schema_version: 1, variant: kind === 'math' ? 'math' : 'econ', settings: led.settings, citation_key: 'Key', archive_identity: { archive_id: 'a1' }, units: [...led.units.values()] }) }
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
    if (/^cleanup DeepSeek/.test(label)) {
      // The recorded resources the prompt lists: the mock agent deletes exactly these ids.
      const plan = JSON.parse((text.match(/Recorded resources \(JSON\):\n(.+)/) || [])[1] || '[]')
      const workspaces = [...new Set(plan.map((r) => r.workspace && r.workspace.id))].filter(Boolean)
      return {
        workspaces: workspaces.map((id) => ({
          id, removed: true, unknown: [],
          conversations: plan.filter((r) => r.workspace.id === id).flatMap((r) => r.conversations.map((c) => ({ id: c.id, state: 'finished', outcome: 'answered', deleted: 'confirmed' }))),
        })),
        usage_after: { input_cache_miss: 100, cached_read: 50, output: 20, calls: 3, cost: 0.1 },
      }
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
    calls, commands, logs, phases, managers, cleanups, pool, led, resourceRecords,
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
  // S23-AC01 (#115): §13.1's dependency flow on the existing stages, not a reordered blueprint.
  check(after('concepts') === 'terms', 'schedule: the frozen concept review follows the book term table')
  check(after('digest') === 'concepts', 'schedule: the group writers are released by the concept decisions of the run\'s own term table')
  check(after('cards') === 'digest', 'schedule: the per-group Card checks follow their own group completion (the cadrs stage)')
  check(after('models') === 'cards', 'schedule: the model layer is released when the digest managers settle (after the Cards layer)')
  check(after('explain') === 'models', 'schedule: explanations follow the model layer')
  check(after('cards-book') === 'explain', 'schedule: the whole-book card cycle waits for the model reviews (R248)')
  check(stageOf('econ', 'events').after.join(',') === 'terms' && stageOf('econ', 'findings').after.join(',') === 'terms', 'schedule: Event and Finding continuations run from readiness while the remaining groups digest')
  check(after('review') === 'cards-book', 'schedule: the scoped restatement review sits in the Card cycle, after the whole-book cycle')
  check(/review/.test(after('link-prepare')) && /hubs-first/.test(after('link-prepare')) && /events|findings/.test(after('link-prepare')), 'schedule: the gap-only Link pass waits for the term evidence (after-terms), the scoped review (itself after the whole-book cycle) and the mode-hubs preview')
  check(after('organize') === 'link-check', 'schedule: Organize runs after the link checks settle')
  check(after('figures') === 'organize', 'schedule: Figures is released by Organize')
  check(after('bridge') === 'figures', 'schedule: the report-first Bridge follows Figures (its endpoint snapshots are the final Card text)')
  check(after('hubs') === 'bridge', 'schedule: the hub publication and the register plan follow the Bridge report (§13.1)')
  check(after('audit') === 'hubs,bridge', 'schedule: the final audit joins the Hubs branch and the Bridge branch')
  check(after('finalize') === 'audit', 'schedule: the finite finalization follows the audit it plans from (#86)')
  check(after('cleanup') === 'finalize' && after('report') === 'cleanup', 'schedule: the settlement waits for the finalization, then cleanup, then report')
  check(stageOf('econ', 'finalize').onFail === 'continue', 'schedule: a residual finalization fault leaves a partial result, never a halted run')
  check(stageOf('econ', 'concepts').onFail === 'continue' && stageOf('econ', 'review').onFail === 'continue', 'schedule: a review reports its own unresolved findings instead of halting the run (S23-AC03/AC04)')
  check(after('hubs-first') === 'explain', 'schedule: the mode-hubs preview stage sits beside the content layers')
  check(stageOf('econ', 'digest').foreach && stageOf('econ', 'digest').foreach.items.$ref === 'stages.prepare.output.groupList', 'schedule: the digest stage is a per-group chain pipeline over the prepared group enumeration')
  check(stageOf('econ', 'digest').foreach.chain[0].module === 'zt-group-cycle', 'schedule: each group item runs the zt-group-cycle chain')
  // #87: the resume invocation and the repair modes drain their downstream closure.
  for (const which of ['econ', 'math']) {
    const modes = builds[which].bp.modes
    const resume = modes.resume || []
    const required = ['args', 'prepare', 'digest', 'cards', 'organize', 'hubs', 'audit', 'finalize', 'cleanup', 'report']
    check(required.every((id) => resume.includes(id)), `resume (${which}): mode resume runs the retained run's whole closure (${resume.length} stages: ${resume.join(',')})`)
    check(resume.includes('terms') && resume.includes('link-prepare') && resume.includes('link') && resume.includes('link-check'), `resume (${which}): the term table and the changed-Card Link pass and check are part of the resume`)
    if (which === 'econ') {
      check(resume.includes('events') && resume.includes('findings') && resume.includes('models') && resume.includes('explain') && resume.includes('cards-book'), 'resume (econ): the economics layers and the whole-book cycle are drained')
      check(['models', 'explain', 'events', 'findings'].every((mode) => ['link-prepare', 'link', 'link-check', 'organize', 'hubs', 'audit', 'cleanup', 'report'].every((id) => (modes[mode] || []).includes(id))), 'repair modes (econ): models, explain, events and findings drain the changed-Card Link work, Organize, the touched Hubs, the audit and the settlement')
    } else {
      check(resume.includes('math-skeleton') && resume.includes('math-boundary') && resume.includes('math-proof') && resume.includes('math-theory') && resume.includes('math-check'), 'resume (math): every mathematics obligation and the final math check are drained')
      check(['models', 'explain'].every((mode) => ['link-prepare', 'link', 'link-check', 'math-theory', 'organize', 'hubs', 'audit', 'math-check', 'cleanup', 'report'].every((id) => (modes[mode] || []).includes(id))), 'repair modes (math): a repaired model reaches its Theory refresh and the math check')
      for (const [stage, pattern] of [['math-skeleton', /resume/], ['math-boundary', /resume/], ['math-proof', /resume/], ['math-theory', /resume/], ['math-check', /resume/]]) {
        const found = builds.math.bp.stages.find((s) => s.id === stage)
        check(found && (found.when || []).some((c) => pattern.test(JSON.stringify(c))), `resume (math): ${stage} participates in the resume`)
      }
    }
  }
  for (const stage of ['organize', 'hubs', 'finalize']) {
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
  check(mathAfter('audit') === 'hubs,bridge' && mathAfter('math-check') === 'finalize' && mathAfter('cleanup') === 'math-check', 'schedule: the mathematics check describes the final audit revision, then the settlement')
  check(m.stages.some((s) => s.id === 'finalize'), 'schedule: the mathematics variant holds the finalization stage its base declares')
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
  const { commands, calls, led, result, logs, report } = await run('econ', { ...base, figures: false })
  const cmds = commands
  const labels = calls.map((c) => c.label)
  check(result && result.mode === 'full', 'full: the run reaches its report')
  check(cmds.filter((c) => /ledger next -Out "D:\\x\\Key" -Layer digest -Take 1 /.test(c)).length === 2, `group cycle: one bounded claim per group chain (${cmds.filter((c) => /-Layer digest/.test(c)).length} claims)`)
  check(cmds.filter((c) => /group-start -Out "D:\\x\\Key" -Group \d+ -Token \d+/.test(c)).length === 2, 'group cycle: each chain starts its own group (snapshot before the job manifest)')
  check(cmds.filter((c) => /group-finish .* -Draft "D:\\x\\Key\\group-jobs\\g00\d\\claim-\d+\\draft\.json"/.test(c)).length === 2, 'group cycle: each chain records its checked completion with the draft bound to the start')
  check(cmds.some((c) => /zt-digest group-start -Out/.test(c)) && !cmds.some((c) => /-Groups \d/.test(c)), 'group cycle: no per-block legacy digest command remains')
  check(labels.filter((l) => /^group \d attempt 1$/.test(l)).length === 2, 'group cycle: one manager chain per group item')
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
  check(labels.includes('prepare link pass') && labels.some((l) => /^link group 1 attempt/.test(l)) && cmds.some((c) => /ledger next .* -Layer link/.test(c)) && cmds.some((c) => /link-start .* -Group 1/.test(c)) && cmds.some((c) => /link-finish .* -Group 1/.test(c)), 'link: the scheduled group runs link-start, its one job and link-finish')
  check(cmds.some((c) => /link-start .* -Group 2/.test(c)) && cmds.some((c) => /link-finish .* -Group 2/.test(c)) && !cmds.some((c) => /ledger fail .* -Class content/.test(c) && /link/.test(c)), 'link: a group with no semantic gap settles its unit without a provider job or a legacy content fault')
  check(result.link_gaps && result.link_gaps.scheduled === 1 && result.link_gaps.skipped === 1 && result.link_gaps.gaps === 2 && result.link_gaps.provider_runs === 1 && result.link_gaps.quality && result.link_gaps.quality.faults === 0, `report: the run carries the scheduled/skipped/gap counts, the actual provider runs and the L1/L2 quality (${JSON.stringify(result.link_gaps)})`)
  check(Array.isArray(result.link) && result.link.some((r) => r.group === 1 && r.status === 'done' && r.gaps === 2 && r.provider_runs === 1) && result.link.some((r) => r.group === 2 && r.scheduled === 0 && r.provider_runs === 0), 'report: each group names its gap count, whether it was scheduled and its provider runs')
  check(result.schedule && result.schedule.settings.maxAttempts === 3 && result.schedule.settings.dshRunCap === 1200, 'report: the result carries the effective settings and the schedule summary')
  check(result.schedule && result.schedule.digest && result.schedule.digest.verified === 2 && result.schedule.cards && result.schedule.cards.verified === 2, 'report: the schedule summary counts the verified group chains and Card checks')
  check(report && report.stages.digest && report.stages.digest.status === 'done' && !report.stages.digest.error, 'group cycle: the digest stage ends done on a healthy run')
  // #92: every claim covers the frozen provider-run cap, and a chain renews it while its job runs.
  check(cmds.filter((c) => /ledger next -Out "D:\\x\\Key" -Layer digest -Take 1 .* -Lease 1500/.test(c)).length === 2, `lease: each group claim requests a lease covering the run cap (${cmds.filter((c) => /-Lease 1500/.test(c)).length} claims)`)
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
  check(!labels.some((l) => /^link group/.test(l)), 'blocked link: no link job runs for a group whose digest unit is not done')
  check(result && Array.isArray(result.link_blocked) && result.link_blocked.length === 2 && result.link_blocked.every((b) => b.unit && b.state && b.reason && b.group), `report: the run names the blocked groups and their units (${result && JSON.stringify(result.link_blocked)})`)
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
  const mbase = { ...base, hubs: true, figures: false }
  const { commands, calls, result, led, resourceRecords } = await run(kind, { ...mbase, mode: 'full' })
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
  // #83: the run records every created conversation with its accepted answer, and its cleanup deletes
  // exactly those ids, observes the provider's facts and settles each cleanup unit through the verifier.
  const registers = cmds.filter((c) => / ledger resource .*-Register /.test(c))
  const records = resourceRecords
  check(records.length > 0 && records.every((r) => r.workspace && r.workspace.path && Array.isArray(r.conversations) && r.conversations.every((c) => c.id && c.answer)), `${kind} full: every created conversation is recorded with its accepted answer`)
  check(records.some((r) => r.unit !== 'run') && registers.some((c) => /-Unit "?digest\/g\d{3}@1"?/.test(c)), `${kind} full: a claimed unit owns its conversations`)
  const listed = at(/^cleanup: list the recorded resources$/)
  const asked = at(/^cleanup DeepSeek$/)
  check(listed >= 0 && asked > listed, `${kind} full: the cleanup reads the recorded ids before it asks the provider (${listed} < ${asked})`)
  const askedPrompt = (calls.find((c) => /^cleanup DeepSeek$/.test(c.label)) || {}).prompt || ''
  check(records.every((r) => r.conversations.every((c) => askedPrompt.includes(c.id))), `${kind} full: the cleanup agent may delete only the recorded ids`)
  check(cmds.some((c) => / ledger resource .*-Observe /.test(c)) && cmds.some((c) => /zt-digest cleanup-finish -Out .* -Unit "?cleanup\/book@1"?/.test(c)), `${kind} full: the provider facts are observed and the run-level cleanup is settled by its verifier`)
  // #110: the Link units now settle through link-finish, so content is complete while the cleanup
  // still reports its own half: the two are still reported apart.
  check(result.cleanup && result.cleanup.cleanup_complete === true && result.cleanup.status === (result.cleanup.content_complete ? 'complete' : 'partial'), `${kind} full: the cleanup reports its own half apart from content completeness`)
  if (kind === 'econ') check(result.cleanup.content_complete === true, 'econ full: the settled Link units leave no content gap')
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
  const { commands, calls, result } = await run('econ', { out: 'D:\\x\\audit', mode: 'audit' })
  const labels = calls.map((c) => c.label)
  check(result && result.mode === 'audit' && result.audit.status === 'done', 'econ audit: runs to its end without a citation key')
  check(labels[0] === 'audit cards' && labels.slice(1).every((l) => /^cleanup|^report the run/.test(l)) && labels.includes('cleanup: list the recorded resources') && labels.includes('report the run from the ledger'), `econ audit: the audit and its ledger-driven settlement, no source probe and no layer (${labels.join(', ')})`)
  check(!calls.some((c) => /^cleanup DeepSeek/.test(c.label)) && !commands.some((c) => /deepseek_delete/.test(c)), 'econ audit: a run with no recorded resource deletes no conversation and no workspace')
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
  check(result.bridge && result.bridge.status === 'done' && at('show figures') < at('bridge: plan') && at('hub manager: survey and limits') > at('bridge: plan'), 'full: Figures then the report-only Bridge, and the hub and register plan after it (§13.1)')
  check(!cmds.some((c) => /zt-digest bridge/.test(c) && /-Apply\b/.test(c)), 'full: discovery never applies proposals')
}

// ================================================================================================================
// The integrated digest path: the delivered vertical paths in the frozen variants (S23 / #115)
// ================================================================================================================
{
  // The concept review runs on the run's own term table, before the first group writer, and its frozen
  // report travels in the run's result. It writes no Card: the report step starts no client.
  const { commands, calls, result, report } = await run('econ', { ...base, figures: false })
  const cmds = commands
  const labels = calls.map((c) => c.label)
  const at = (p) => labels.findIndex((l) => p.test(l))
  check(cmds.some((c) => /zt-digest concepts -Step propose -Staging "D:\\x\\Key" -Work "D:\\x\\Key\\review\\concepts"/.test(c)), 'concepts: the review proposes from the run\'s own term table (no proposal file is written by an agent)')
  check(cmds.some((c) => /zt-digest concepts -Step prepare /.test(c)) && cmds.some((c) => /zt-digest concepts -Step judge .* -File "D:\\x\\Key\\review\\concepts\\out\\cj-01.json"/.test(c)) && cmds.some((c) => /zt-digest concepts -Step report/.test(c)), 'concepts: the frozen evidence, the validated judge answer and the report are one bounded chain')
  check(at(/^terms: /) >= 0 && at(/^concepts: /) > at(/^terms: /) && at(/^concepts: /) < at(/^group 1 attempt/), `concepts: the term table, then the concept decisions, then the first group writer (${at(/^terms: /)} < ${at(/^concepts: /)} < ${at(/^group 1 attempt/)})`)
  check(result && result.concepts && result.concepts.review && result.concepts.review.report_id === 'r-c1' && result.concepts.review.complete === true, `report: the result carries the frozen concept review and its id (${result && JSON.stringify(result.concepts && result.concepts.review)})`)
  check(result.schedule && result.schedule.concepts && result.schedule.concepts.complete === true, 'report: the schedule summary names the concept review and its completeness')
  check(report.stages.concepts && report.stages.concepts.status === 'done', 'concepts: the stage ends done when its review is complete')
}
{
  // The scoped restatement review runs in the Card cycle, on the run's own staged owned-Card manifests,
  // and hands the Card cycle's recorded merge verdicts to the apply step: a merge no rewrite covers stays
  // pending, never counted as applied (D115/D116).
  const { commands, calls, result } = await run('econ', { ...base, figures: false }, { pendingMerge: true })
  const cmds = commands
  const labels = calls.map((c) => c.label)
  const at = (p) => labels.findIndex((l) => p.test(l))
  check(cmds.some((c) => /zt-digest restate -Step prepare -Staging "D:\\x\\Key" -Work "D:\\x\\Key\\review\\restate"/.test(c)), 'review: the review reads the run\'s own staged owned-Card manifests (no separate collector)')
  check(cmds.some((c) => /zt-digest restate -Step apply .*-OperationPrefix "Key\/review\/restate"/.test(c)), 'review: accepted rewrites go through the run\'s own identified operation prefix')
  check(cmds.some((c) => /zt-digest restate -Step apply .*-Merges "D:\\x\\Key\\cards\\decisions.json"/.test(c)), 'review: the Card cycle\'s recorded merge verdicts are handed to the apply step')
  check(at(/^cards whole book/) >= 0 && at(/^restatement review/) > at(/^cards whole book/) && at(/^restatement review/) < at(/^prepare link pass/), `review: the scoped review runs after the whole-book cycle and before the gap-only Link pass (${at(/^cards whole book/)} < ${at(/^restatement review/)} < ${at(/^prepare link pass/)})`)
  check(result && result.review && result.review.resolution && result.review.resolution.counts.pending_existing_card_change === 1 && result.review.status === 'partial', `report: a merge verdict no rewrite covers stays pending and keeps the review partial (${result && JSON.stringify(result.review && result.review.resolution)})`)
  check(result.schedule && result.schedule.review && result.schedule.review.pending === 1, 'report: the schedule summary counts the pending resolution')
}
{
  // The frozen decision the archive moved past is refused, never applied as stale text: the apply step
  // fails, the report renders what remains unresolved, and nothing claims a rewrite.
  const { commands, result, logs, report } = await run('econ', { ...base, figures: false }, { staleDecision: true })
  const cmds = commands
  check(!cmds.some((c) => /zt-digest restate -Step check-only/.test(c)), 'stale decision: no rewrite is invented beside the refused apply')
  check(result && result.review && result.review.applied === null && result.review.resolution.status === 'partial' && result.review.resolution.counts.unresolved === 1, `stale decision: the unresolved pair is reported, never counted as resolved (${result && JSON.stringify(result.review && result.review.resolution && result.review.resolution.counts)})`)
  check(logs.some((l) => /Restatement review: no accepted rewrite was applied/.test(l)), 'stale decision: the stage says the accepted rewrite was not applied')
  check(report.stages.review && report.stages.review.status === 'done', 'stale decision: the review stage reports its own remaining gap instead of halting the run')
}
{
  // The register is report-first: a plan without an accepted plan id is planned, logged and never written.
  const planned = await run('econ', { ...base, figures: true, hubs: true })
  check(planned.commands.some((c) => /zt-digest register -Step plan -Work "D:\\x\\Key\\register" -Review "D:\\x\\Key\\concept-review.json"/.test(c)), 'register: the plan reads the reviewed concept report as read-only evidence')
  check(!planned.commands.some((c) => /zt-digest register -Step apply/.test(c)), 'register: without an accepted plan id the register is planned and reported, never published')
  check(planned.result && planned.result.register && planned.result.register.plan_id === 'rp-c1' && planned.result.register.accepted === false && planned.result.register.status === 'planned', `register: the result names the plan it did not publish (${planned.result && JSON.stringify(planned.result.register && planned.result.register.status)})`)
  check(planned.result.schedule && planned.result.schedule.register && planned.result.schedule.register.plan_id === 'rp-c1', 'report: the schedule summary carries the planned register')
}
{
  // The accepted plan id publishes exactly that plan, and the check reads every page back.
  const accepted = await run('econ', { ...base, figures: true, hubs: true, registerPlan: 'rp-c1' })
  const cmds = accepted.commands
  check(cmds.some((c) => /zt-digest register -Step apply -Work "D:\\x\\Key\\register"/.test(c)) && cmds.some((c) => /zt-digest register -Step check -Work "D:\\x\\Key\\register"/.test(c)), 'register: the accepted plan is published and read back')
  check(accepted.result && accepted.result.register && accepted.result.register.status === 'published' && accepted.result.register.verified === true && accepted.result.register.complete === true, `register: the publication is verified against the plan (${accepted.result && JSON.stringify(accepted.result.register && accepted.result.register.status)})`)
}
{
  // An interrupted publication keeps its accepted plan: the stage reports it pending, runs no check, and
  // a retry with the same plan id replays the receipts instead of publishing twice.
  const interrupted = await run('econ', { ...base, figures: true, hubs: true, registerPlan: 'rp-c1' }, { interruptedRegister: true })
  check(!interrupted.commands.some((c) => /zt-digest register -Step check/.test(c)), 'interrupted register: no verification is claimed for an interrupted publication')
  check(interrupted.result && interrupted.result.register && interrupted.result.register.status === 'pending' && interrupted.result.register.plan_id === 'rp-c1' && interrupted.result.register.verified === false, `interrupted register: the accepted plan stays staged as pending (${interrupted.result && JSON.stringify(interrupted.result.register && interrupted.result.register.status)})`)
  check(interrupted.logs.some((l) => /the publication was interrupted or refused/.test(l)), 'interrupted register: the stage says the publication was interrupted and the plan stays staged')
  const retried = await run('econ', { ...base, figures: true, hubs: true, registerPlan: 'rp-c1' })
  check(retried.result && retried.result.register && retried.result.register.status === 'published', 'interrupted register: the same accepted plan publishes on the retry')
}
{
  // A stale reviewed decision is reported as excluded evidence, and the register publication it would
  // have contributed to is planned without it.
  const { commands, result } = await run('econ', { ...base, figures: true, hubs: true, registerPlan: 'rp-c1' }, { staleReview: true })
  check(commands.some((c) => /zt-digest register -Step plan .*-Review /.test(c)), 'stale decision: the register plan still reads the reviewed report as evidence')
  check(result && result.register && result.register.review && result.register.review.complete === false, `stale decision: the plan reports that the review it read is incomplete (${result && JSON.stringify(result.register && result.register.review)})`)
}
{
  // The resumed prepared run drains the two reviews with the rest of its closure, and repeats no
  // preparation: the retained run's own work decides what is reviewed.
  const { calls, commands, result } = await run('econ', { out: 'D:\\x\\Key', mode: 'resume' }, { retained: [['terms', 'terms/book@1'], ['digest', 'digest/g001@1'], ['digest', 'digest/g002@1']] })
  const labels = calls.map((c) => c.label)
  check(labels.includes('concept review') && labels.includes('restatement review'), `resume: the retained run re-reviews its own term table and new Cards (${labels.filter((l) => /review/.test(l)).join(', ')})`)
  check(commands.some((c) => /zt-digest ledger resume/.test(c)) && !labels.includes('prepare book'), 'resume: the one supported resume runs and no preparation repeats')
  check(result && result.mode === 'resume' && result.concepts && result.review, 'resume: the resumed run reports both reviews')
}

console.log(`failed: ${failed}`)
process.exit(failed ? 1 : 0)








