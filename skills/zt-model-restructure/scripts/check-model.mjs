// node check-model.mjs <modelHead> [--stage fold|settings|restructure|final] [--json]
// Prints the main line and the build faults of one model, with explicit stage-scoped gating:
//   fold         Stage 1 (zt-model-fold): blocks on head/retype/reach_in/import_target/import_cycle/non-model closure;
//                reports Settings/Restructure/Import faults as deferred to their responsible stage.
//   settings     Stage 2 (zt-model-settings): blocks on setting_dependency, symbol_uncovered, setting/notation formal_shape,
//                and structural errors; defers chapter/result/proof faults (Stage 3) and cross-model closure (Stage 4).
//   restructure  Stage 3 (zt-model-restructure): blocks on all local graph + formal Layer 1 faults; defers only
//                cross-model closure (premises in other models, repaired by Stage 4 zt-model-import).
//   final        Stage 4+ (zt-model-import / acceptance): blocks on everything; requires strict `self_sufficient === true`,
//                `formal_structured === true` for eligible L2 formal models (`level === 'formal' && !form`), and
//                `evidence_bound === true` for L3 empirical models.
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const ALL_FAULT_KEYS = [
  'closure', 'reach_in', 'forward', 'unanchored', 'parts', 'beyond_level', 'import_target', 'import_cycle',
  'formal_shape', 'setting_dependency', 'criterion_unbound', 'result_unbound', 'proof_unbound', 'experiment_unbound', 'symbol_uncovered',
  'empirical_shape', 'estimate_unbound', 'spec_unbound', 'test_unbound', 'variable_uncovered',
]

function isCrossModelClosure(f) {
  const c = f && f.card
  return Boolean(c && c.type === 'model' && c.facet && (c.models || []).length)
}

function hasPendingImportOf(b, pred) {
  return (b.closure || []).some((f) => isCrossModelClosure(f) && pred(f.card))
}

function isStage3EmpiricalShape(fault) {
  return Boolean(fault && fault.lacks === '估计')
}

const ALLOWED_LEVELS = new Set(['concept', 'formal', 'empirical'])
const ALLOWED_FORMS = new Set(['theory', 'definition', 'axiom', 'theorem', 'proposition', 'lemma', 'corollary'])

export function validateModelPayload(m, head = (m && m.head && m.head.address) || '<head>') {
  if (!m || typeof m !== 'object' || !m.build || typeof m.build !== 'object' || Array.isArray(m.build)) {
    throw new Error(`get_model answer for ${head} lacks build object`)
  }
  const b = m.build
  if (!Array.isArray(b.order)) throw new Error(`get_model answer for ${head} lacks build.order array`)
  if (typeof b.self_sufficient !== 'boolean') {
    throw new Error(`get_model answer for ${head} lacks boolean build.self_sufficient`)
  }
  for (const key of ['closure', 'reach_in', 'forward', 'unanchored', 'parts', 'beyond_level', 'import_target', 'import_cycle']) {
    if (!Array.isArray(b[key])) {
      throw new Error(`get_model answer for ${head} lacks required build.${key} array`)
    }
  }
  if (!Object.prototype.hasOwnProperty.call(m, 'level')) {
    throw new Error(`get_model answer for ${head} lacks top-level level field`)
  }
  if (m.level !== null && (typeof m.level !== 'string' || !ALLOWED_LEVELS.has(m.level))) {
    throw new Error(`get_model answer for ${head} has invalid or unknown level (${String(m.level)})`)
  }
  if (b.level !== undefined && b.level !== m.level) {
    throw new Error(`get_model answer for ${head} has contradictory level (${String(m.level)}) vs build.level (${String(b.level)})`)
  }
  if (m.form !== undefined && m.form !== null && (typeof m.form !== 'string' || !ALLOWED_FORMS.has(m.form))) {
    throw new Error(`get_model answer for ${head} has invalid or unknown form (${String(m.form)})`)
  }
  const hasForm = typeof m.form === 'string' && ALLOWED_FORMS.has(m.form)
  if (m.level === 'formal' && !hasForm) {
    if (typeof b.formal_structured !== 'boolean') {
      throw new Error(`get_model answer for ${head} lacks boolean build.formal_structured`)
    }
    for (const key of ['formal_shape', 'setting_dependency', 'criterion_unbound', 'result_unbound', 'proof_unbound', 'experiment_unbound', 'symbol_uncovered']) {
      if (!Array.isArray(b[key])) {
        throw new Error(`get_model answer for ${head} lacks required formal build.${key} array`)
      }
    }
  }
  if (m.level === 'empirical') {
    if (typeof b.evidence_bound !== 'boolean') {
      throw new Error(`get_model answer for ${head} lacks boolean build.evidence_bound`)
    }
    for (const key of ['empirical_shape', 'estimate_unbound', 'spec_unbound', 'test_unbound', 'variable_uncovered']) {
      if (!Array.isArray(b[key])) {
        throw new Error(`get_model answer for ${head} lacks required empirical build.${key} array`)
      }
    }
  }
  for (const key of ALL_FAULT_KEYS) {
    if (b[key] !== undefined && b[key] !== null && !Array.isArray(b[key])) {
      throw new Error(`get_model answer for ${head} has non-array build.${key}`)
    }
  }
}

function isHeadMissingNotation(head, fault) {
  if (!fault || fault.lacks !== '符号表与读法') return false
  return !fault.address || fault.address === head
}

function isCanonicalNotationFacet(facet) {
  const prefix = String(facet || '').split(/[：:]/)[0].replace(/\s+/g, '')
  return prefix === '符号表与读法'
}

function isCanonicalCriterionFacet(facet) {
  const prefix = String(facet || '').split(/[：:]/)[0].replace(/\s+/g, '')
  return prefix === '判据' || prefix === '均衡定义' || prefix === '解概念'
}

function ownerStage(key, fault) {
  if (key === 'closure') return isCrossModelClosure(fault) ? 'Stage 4 (import)' : 'Stage 1 (fold)'
  if (['reach_in', 'beyond_level', 'import_target', 'import_cycle'].includes(key)) return 'Stage 1 (fold)'
  if (['setting_dependency', 'symbol_uncovered', 'variable_uncovered', 'spec_unbound'].includes(key)) return 'Stage 2 (settings)'
  if (key === 'empirical_shape') {
    return isStage3EmpiricalShape(fault) ? 'Stage 3 (restructure)' : 'Stage 2 (settings)'
  }
  if (key === 'formal_shape') {
    return fault && (fault.lacks === '设定' || fault.lacks === '符号表与读法') ? 'Stage 2 (settings)' : 'Stage 3 (restructure)'
  }
  if (['forward', 'unanchored', 'parts', 'criterion_unbound', 'result_unbound', 'proof_unbound', 'experiment_unbound', 'estimate_unbound', 'test_unbound'].includes(key)) {
    return 'Stage 3 (restructure)'
  }
  return 'Stage 4 (final)'
}

function blocksStage(stage, head, key, fault, pending) {
  if (stage === 'final') return true
  if (stage === 'restructure') {
    if (key === 'closure' && isCrossModelClosure(fault)) return false
    // Defer criterion_unbound ONLY when a pending cross-model import specifically targets a Criterion card.
    if (key === 'criterion_unbound' && pending.criterion) return false
    return true
  }
  if (stage === 'settings') {
    if (['reach_in', 'beyond_level', 'import_target', 'import_cycle', 'setting_dependency', 'symbol_uncovered', 'variable_uncovered', 'spec_unbound'].includes(key)) return true
    if (key === 'empirical_shape') return !isStage3EmpiricalShape(fault)
    if (key === 'closure' && !isCrossModelClosure(fault)) return true
    if (key === 'formal_shape' && fault && (fault.lacks === '设定' || fault.lacks === '符号表与读法')) {
      if (fault.lacks === '符号表与读法' && isHeadMissingNotation(head, fault) && pending.notation) return false
      return true
    }
    return false
  }
  if (stage === 'fold') {
    if (['reach_in', 'beyond_level', 'import_target', 'import_cycle'].includes(key)) return true
    if (key === 'closure' && !isCrossModelClosure(fault)) return true
    return false
  }
  return true
}

/** Evaluate one `get_model` response under `stage` ('fold' | 'settings' | 'restructure' | 'final'). */
export function evaluate(m, stage = 'restructure') {
  const head = (m.head && m.head.address) || ''
  const b = m.build || {}
  const pending = {
    criterion: hasPendingImportOf(b, (c) => c.home_role === 'criterion' || isCanonicalCriterionFacet(c.facet)),
    notation: hasPendingImportOf(b, (c) => c.home_role === 'notation' || isCanonicalNotationFacet(c.facet)),
  }
  const blocking = []
  const deferred = []
  for (const key of ALL_FAULT_KEYS) {
    for (const f of b[key] || []) {
      const owner = (key === 'criterion_unbound' && pending.criterion) || (key === 'formal_shape' && isHeadMissingNotation(head, f) && pending.notation)
        ? 'Stage 4 (import)'
        : ownerStage(key, f)
      const item = { key, fix: f.fix || JSON.stringify(f), owner, fault: f }
      if (blocksStage(stage, head, key, f, pending)) blocking.push(item)
      else deferred.push(item)
    }
  }
  const hasForm = typeof m.form === 'string' && m.form.length > 0
  const eligibleFormal = m.level === 'formal' && !hasForm
  const eligibleEmpirical = m.level === 'empirical'
  if (stage === 'final') {
    if (b.self_sufficient !== true) {
      blocking.push({ key: 'self_sufficient', fix: `self_sufficient is ${String(b.self_sufficient)} (must be true)`, owner: 'Stage 4 (final)' })
    }
    if (eligibleFormal && b.formal_structured !== true) {
      blocking.push({ key: 'formal_structured', fix: `formal_structured is ${String(b.formal_structured)} (eligible L2 formal model requires true)`, owner: 'Stage 4 (final)' })
    }
    if (eligibleEmpirical && b.evidence_bound !== true) {
      blocking.push({ key: 'evidence_bound', fix: `evidence_bound is ${String(b.evidence_bound)} (L3 empirical model requires true)`, owner: 'Stage 4 (final)' })
    }
  }
  return {
    stage,
    ok: blocking.length === 0,
    blocking,
    deferred,
    self_sufficient: b.self_sufficient,
    formal_structured: b.formal_structured,
    evidence_bound: b.evidence_bound,
  }
}

export async function runCli(defaultStage = 'restructure', argv = process.argv.slice(2)) {
  const stageIdx = argv.indexOf('--stage')
  const stage = stageIdx >= 0 ? argv[stageIdx + 1] : defaultStage
  const jsonOut = argv.includes('--json')
  const positional = argv.filter((a, i) => !a.startsWith('--') && (stageIdx < 0 || i !== stageIdx + 1))
  const head = positional[0]
  if (!head || !['fold', 'settings', 'restructure', 'final'].includes(stage)) {
    console.error('usage: node check-model.mjs <modelHead> [--stage fold|settings|restructure|final] [--json]')
    process.exit(2)
  }
  const { model } = await import('./zt.mjs')
  const m = model(head)
  if (!m.head || m.head.address !== head) {
    console.error(`${head} is not a Model Card${m.head ? `: it belongs to the model ${m.head.address}; give that address` : ''}`)
    process.exit(2)
  }
  try {
    validateModelPayload(m, head)
  } catch (err) {
    console.error(err.message)
    process.exit(2)
  }
  const b = m.build
  const res = evaluate(m, stage)
  if (jsonOut) {
    console.log(JSON.stringify({ head, ...res }, null, 2))
    process.exit(res.ok ? 0 : 1)
  }
  const lines = {}
  for (const o of b.order || []) lines[o.line] = (lines[o.line] || 0) + 1
  const sh = (a) => (a.startsWith(head) ? 'H' + a.slice(head.length) : a)
  console.log(`members ${(b.order || []).length} ${JSON.stringify(lines)} (stage: ${stage})`)
  console.log('main line: ' + (b.order || []).filter((o) => o.line === 'main').map((o) => `${sh(o.address)} ${o.facet}`).join(' | '))
  for (const f of res.blocking) console.log(`[blocking] ${f.key}: ${f.fix}`)
  for (const f of res.deferred) console.log(`[deferred -> ${f.owner}] ${f.key}: ${f.fix}`)
  const rel = {}
  for (const r of m.relations || []) rel[r.kind] = (rel[r.kind] || 0) + 1
  const extras = [
    `self_sufficient ${b.self_sufficient}`,
    b.formal_structured !== undefined ? `formal_structured ${b.formal_structured}` : null,
    b.evidence_bound !== undefined ? `evidence_bound ${b.evidence_bound}` : null,
  ].filter(Boolean).join('; ')
  console.log(`relations ${JSON.stringify(rel)}; loose ${(m.loose || []).length}; ${extras}; stage(${stage}) ${res.ok ? 'pass' : 'fail'}`)
  process.exit(res.ok ? 0 : 1)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runCli('restructure')
}
