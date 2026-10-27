export const meta = {
  "name": "zt-econ-digest",
  "description": "Digest a Chinese-translated economics book into ZET Cards: the ZT digest base with the economics set: the model layer (Model Cards in the digest, the model compiler with the economic architect), the event layer (Event Cards from the book's dated sentences), the empirical-results layer (Finding Cards, one per result), and the economics prompts; it builds only the Literature tree (Topic trees, hubs and the register are zt-idea-grow's)",
  "whenToUse": "Turn a Scholar Archive economics book (Chinese bilingual Markdown, digested as Chinese only) into an agent Literature tree with Idea, Event, Finding, and Model Cards (economic models at concept or formal level). It writes no Topic tree, no Hub Card and no register: zt-idea-grow grows those from the Literature trees afterwards. Modes link, models, cards, explain, events, findings, figures, audit and bridge re-run single stages on an existing digest. (v6.7.2; built by the workflow-framework-protocol; structure frozen at install. Modes: link, cards, figures, audit, bridge, term-cards, term-link, models, explain, events, findings, resume.)",
  "phases": [
    {
      "title": "Prepare",
      "detail": "the required args per mode"
    },
    {
      "title": "Term cards",
      "detail": "the Term-card branch, once the Literature content is settled (after the whole-book Cards cycle; live run 2026-10-26: beside the digest writers its plan went stale before it published): the compiler's writer jobs in waves of four on the shared DSMP pool, their checks, the independent mathematical reviews and the one finite fix; each job unit and the book unit complete only from the compiler's own checked artifacts, so a failed or partial book stays incomplete"
    },
    {
      "title": "Events",
      "detail": "the event layer: event-compiler.ps1 lists the dated sentences and each group's Idea Cards; DeepSeek writes the Event Cards as JSON with its SELF-CHECK; the script creates them beside the Idea Cards they support and links them"
    },
    {
      "title": "Findings",
      "detail": "the empirical-results layer: finding-compiler.ps1 lists the sentences that report data with a number and each group's Idea Cards; DeepSeek writes one Finding Card per result as JSON with its SELF-CHECK (F1 to F3); the script creates them beside the group anchors and links them with 印证"
    },
    {
      "title": "Digest",
      "detail": "the group cycle in blocks: one manager claims up to 8 digest/gNNN@1 units, a snapshot and frozen job per group, their DeepSeek runs side by side, each conversation recorded and deleted by that manager, checked group-finish (R250/R252/R253); at most 8 lanes"
    },
    {
      "title": "Cards",
      "detail": "the per-group offline Card checks (cards/gNNN@1), claimed from the ready work and recorded by cards-finish"
    },
    {
      "title": "Models",
      "detail": "the model layer: the model compiler on every Model Card of the book (DeepSeek brief → Sonnet plan → DeepSeek fill, derive, review, split)"
    },
    {
      "title": "Link",
      "detail": "prepare-link-pass.ps1: the receipt-owned semantic gap set and the scheduled, gap-free and blocked groups"
    },
    {
      "title": "Organize",
      "detail": "survey → Opus thesis and notes (decisions.json) → apply (no merge, no move) → notes card cycle → skeleton (the Literature index and the structure Card)"
    },
    {
      "title": "Figures",
      "detail": "attach-figures.ps1: the book's own figures in the Cards that name them"
    },
    {
      "title": "Bridge",
      "detail": "bounded frozen cross-book discovery, immutable report, and its new-carrier proposals applied; earlier carriers by explicit apply in bridge mode"
    },
    {
      "title": "Term link",
      "detail": "the deterministic Term-link pass after every Card writer and the Bridge (design 14.1-14.2): claim the run's termlink/book@1 unit, freeze the plan, apply it under the claim and complete the unit only from the compiler's apply report; a failed Term-card book or Card writer leaves it blocked and reported, never linked. The finite finalization runs the same shared pass for its one correction round (termlink/final@R), for the changed Cards or the whole book when the correction changed the Term mapping (S26, #169)"
    },
    {
      "title": "Audit",
      "detail": "digest-audit.ps1, read-only: faults per rule, orphans, models, figures (it counts the box's Topic trees and hubs as information, never as a digest fault)"
    },
    {
      "title": "Finalize",
      "detail": "the finite finalization: the audit plan, at most one compiler correction cycle scoped to its Cards, the changed-Card/touched-tree manifest, the Term-link round for the Cards that cycle changed (the whole book when it changed the Term mapping), the dependency-scoped publication refresh, and the final read-only audit"
    },
    {
      "title": "Cleanup",
      "detail": "settle every DeepSeek conversation and workspace the run recorded, delete exactly those ids, remove only registered scratch inside the staging root, then render the verified run report (completeness, gaps, current placeholders, timings, exact per-run usage), reporting content_complete apart from cleanup_complete"
    },
    {
      "title": "Report",
      "detail": "the run's result per mode"
    }
  ]
}

// zt-econ-digest v6.7.2: built by the workflow-framework-protocol (assemble.mjs) from its blueprint.
// DO NOT EDIT THIS FILE. The structure and logistics of an installed workflow are frozen: change the
// blueprint (zt-econ-digest.blueprint.json), run the pre-flight checks, and install a new version.
// Arguments: out, citationKey, source, bibtex, bookTitle, book, fileId, startHeading, endHeading, skipHeadings, groups, organize, maxChars, maxUnits, dshEffort, models, rebuild, perSonnet, perConcept, dshPerBatch, dshManagers, applyShare, only, level, briefEffort, deriveEffort, reviewEffort, reviewRounds, maxSentences, maxPartChars, maxSteps, maxExplainSentences, maxExplainChars, terms, audit, finalize, figures, ideaSample, cardRules, cardFixRounds, cardMax, explainMax, bridge, bridgeCandidates, bridgeBooks, bridgeReport, bridgeMax, bridgeApply, bridgeApplyEarlier, maxAttempts, dshRunCap, cardChecks, termCards, termLink, termCardsPerJob, termCardsMathPerJob, termLinkOccurrences, events, findings; mode: full, link, cards, figures, audit, bridge, term-cards, term-link, models, explain, events, findings, resume; until: digest

// ==== Workflow Framework Kernel v1 ==========================================================================
// The fixed base of every workflow that the workflow-framework-protocol installs. assemble.mjs copies this block
// verbatim into the installed script. Do not edit it inside an installed script: change the blueprint and reinstall.
// Spec: ~/.claude/skills/workflow-framework-protocol/SKILL.md
//
// The kernel runs a frozen BLUEPRINT. The blueprint fixes the structure (stages, modules, dependencies, per-item
// chains) and the logistics (roles and their models, retries, failure policy, checks, modes, checkpoints).
// At run time only data moves: args fill declared inputs, and each stage's output feeds later stages.
// Nothing at run time can add, remove, reorder, or rewire a stage, or change a role's model.

const WFP_MODULES = Object.create(null)
// A module is a stage module (it has run) or a library module (library: true: shared code that other modules
// name in `requires`; it is never a stage).
function defineModule(m) {
  if (!m || !m.id || (!m.library && typeof m.run !== 'function')) throw new Error('defineModule: a module needs id and run (or library: true)')
  if (WFP_MODULES[m.id]) throw new Error(`defineModule: module ${m.id} is defined twice`)
  WFP_MODULES[m.id] = Object.freeze(m)
}

function wfpFreeze(o) {
  if (o && typeof o === 'object' && !Object.isFrozen(o)) {
    Object.freeze(o)
    for (const k of Object.keys(o)) wfpFreeze(o[k])
  }
  return o
}
const wfpClone = v => (v === undefined ? undefined : JSON.parse(JSON.stringify(v)))

// Stable JSON (sorted keys) and a 64-bit FNV-1a fingerprint. assemble.mjs uses the same two functions.
function wfpStable(v) {
  if (Array.isArray(v)) return '[' + v.map(wfpStable).join(',') + ']'
  if (v && typeof v === 'object') return '{' + Object.keys(v).sort().filter(k => v[k] !== undefined).map(k => JSON.stringify(k) + ':' + wfpStable(v[k])).join(',') + '}'
  return JSON.stringify(v === undefined ? null : v)
}
function wfpFingerprint(s) {
  let a = 0x811c9dc5, b = 0x01000193 ^ 0x5bd1e995
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i)
    a = Math.imul(a ^ c, 0x01000193) >>> 0
    b = Math.imul(b ^ c, 0x5bd1e995) >>> 0
  }
  return a.toString(16).padStart(8, '0') + b.toString(16).padStart(8, '0')
}

function wfpGet(obj, path) {
  let cur = obj
  for (const part of String(path).split('.').filter(Boolean)) {
    if (cur == null) return undefined
    cur = cur[part]
  }
  return cur
}

// Values in a blueprint may hold templates:
//   "{{args.out}}"                  the whole string is one reference: the raw value (an array stays an array)
//   "Read {{item.path}} now"        interpolation: strings as they are, other values as JSON
//   {"$ref": "stages.plan.output.jobs", "default": []}   a raw reference with a default
// Scope roots: args, mode (the run's declared mode), stages (each finished stage: {status, output}), item, index,
// prev, steps (per-item chain), consts.
// A path walks plain properties, so "stages.list.output.files.length" works.
// A filter after "|" quotes a value for a command: psq (PowerShell single-quoted literal), shq (POSIX shell
// single-quoted literal), json (JSON text). Put every arg or item value that goes into a script command
// through psq or shq: "Get-ChildItem -LiteralPath {{args.folder|psq}}".
// Path segments may hold "-" (stage ids are kebab-case: stages.link-prepare.output).
const WFP_TPL = /\{\{\s*([A-Za-z_$][\w$.-]*)\s*(?:\|\s*(psq|shq|json)\s*)?\}\}/g
const WFP_FILTERS = {
  psq: s => `'${String(s).replace(/['‘’‚‛]/g, "''")}'`,
  shq: s => `'${String(s).replace(/'/g, `'\\''`)}'`,
  json: s => JSON.stringify(s),
}
function wfpResolve(v, scope) {
  if (typeof v === 'string') {
    const whole = v.match(/^\{\{\s*([A-Za-z_$][\w$.-]*)\s*\}\}$/)
    if (whole) return wfpClone(wfpGet(scope, whole[1]))
    return v.replace(WFP_TPL, (_, p, f) => {
      const x = wfpGet(scope, p)
      if (f) return WFP_FILTERS[f](f === 'json' ? (x === undefined ? null : x) : x === undefined ? '' : typeof x === 'string' ? x : JSON.stringify(x))
      return x === undefined ? '' : typeof x === 'string' ? x : JSON.stringify(x)
    })
  }
  if (Array.isArray(v)) return v.map(x => wfpResolve(x, scope))
  if (v && typeof v === 'object') {
    if (typeof v.$ref === 'string' && Object.keys(v).every(k => k === '$ref' || k === 'default')) {
      const x = wfpGet(scope, v.$ref)
      return wfpClone(x === undefined ? v.default : x)
    }
    const out = {}
    for (const k of Object.keys(v)) out[k] = wfpResolve(v[k], scope)
    return out
  }
  return v
}

// Declarative checks: { path, op, value, message }. path reads the given root (a stage output, or the scope).
function wfpTest(root, c) {
  const x = c.path ? wfpGet(root, c.path) : root
  const n = v => (Array.isArray(v) ? v.length : v)
  switch (c.op) {
    case 'exists': return x !== undefined && x !== null
    case 'missing': return x === undefined || x === null
    case 'empty': return x == null || (Array.isArray(x) ? x.length === 0 : typeof x === 'object' ? Object.keys(x).length === 0 : x === '')
    case 'nonempty': return !wfpTest(root, { ...c, op: 'empty' })
    case 'true': return x === true
    case 'false': return x === false
    case '==': return wfpStable(x) === wfpStable(c.value)
    case '!=': return wfpStable(x) !== wfpStable(c.value)
    case '>': return n(x) > c.value
    case '>=': return n(x) >= c.value
    case '<': return n(x) < c.value
    case '<=': return n(x) <= c.value
    case 'matches': return new RegExp(c.value).test(typeof x === 'string' ? x : JSON.stringify(x))
    case 'every': return Array.isArray(x) && x.every(e => wfpTest(e, c.value))
    // or / and: value is a list of checks on the same root (path is not used)
    case 'or': return Array.isArray(c.value) && c.value.some(d => wfpTest(root, d))
    case 'and': return Array.isArray(c.value) && c.value.every(d => wfpTest(root, d))
    case 'some': return Array.isArray(x) && x.some(e => wfpTest(e, c.value))
    default: throw new Error(`unknown check op ${c.op}`)
  }
}
const wfpDescribe = c => c.message || (c.op === 'or' || c.op === 'and' ? `(${(c.value || []).map(wfpDescribe).join(` ${c.op} `)})` : `${c.path || '(value)'} ${c.op}${c.value === undefined ? '' : ' ' + JSON.stringify(c.value)}`)

function wfpArgs(bp, raw) {
  const given = raw && typeof raw === 'object' && !Array.isArray(raw) ? raw : {}
  const decl = bp.args || {}
  const out = {}
  for (const k of Object.keys(given)) {
    if (k === 'mode' || k === 'until') continue
    if (!decl[k]) throw new Error(`args.${k} is not declared by this workflow. Declared args: ${Object.keys(decl).join(', ') || 'none'}. The structure of an installed workflow cannot change at run time.`)
  }
  for (const k of Object.keys(decl)) {
    const d = decl[k]
    let v = given[k] === undefined ? wfpClone(d.default) : given[k]
    if (v === undefined || v === null) {
      if (d.required) throw new Error(`args.${k} is required: ${d.description || ''}`)
      out[k] = v
      continue
    }
    const t = Array.isArray(v) ? 'array' : typeof v
    if (d.type && d.type !== 'any' && t !== d.type) throw new Error(`args.${k} must be ${d.type}, got ${t}`)
    if (d.enum && !d.enum.some(e => wfpStable(e) === wfpStable(v))) throw new Error(`args.${k} must be one of ${JSON.stringify(d.enum)}`)
    if (d.type === 'number' && ((d.min !== undefined && v < d.min) || (d.max !== undefined && v > d.max))) throw new Error(`args.${k} must be within [${d.min ?? '-inf'}, ${d.max ?? 'inf'}]`)
    out[k] = v
  }
  return out
}

async function wfpRun(BP, FP) {
  if (wfpFingerprint(wfpStable(BP)) !== FP) throw new Error('This workflow\'s blueprint was changed after it was installed. Run the pre-flight again and reinstall; never edit an installed workflow.')
  wfpFreeze(BP)
  for (const [id, ver] of Object.entries(BP.installed.modules)) {
    if (!WFP_MODULES[id]) throw new Error(`module ${id} is missing from this script`)
    if (WFP_MODULES[id].version !== ver) throw new Error(`module ${id} is version ${WFP_MODULES[id].version}, the blueprint was installed with ${ver}`)
  }
  const raw = typeof args === 'object' && args ? args : {}
  const A = wfpFreeze(wfpArgs(BP, raw))
  const mode = raw.mode || 'full'
  const modes = BP.modes || {}
  if (mode !== 'full' && !modes[mode]) throw new Error(`args.mode "${mode}" is not declared. Declared modes: full${Object.keys(modes).map(m => ', ' + m).join('')}`)
  const selected = new Set(mode === 'full' ? BP.stages.map(s => s.id) : modes[mode])
  const until = raw.until
  if (until !== undefined && !(BP.checkpoints || []).includes(until)) throw new Error(`args.until "${until}" is not a declared checkpoint. Declared checkpoints: ${(BP.checkpoints || []).join(', ') || 'none'}`)

  const roles = BP.roles
  const stages = {}  // id -> {status, output, error, attempts}
  const scopeBase = () => ({ args: A, mode, stages, consts: BP.consts || {} })
  // The progress groups: every stage title and the extra titles the blueprint declares in `phases`.
  const phaseTitles = new Set([...BP.stages.map(s => s.title), ...(BP.phases || []).map(p => (typeof p === 'string' ? p : p.title))])
  const report = { workflow: BP.name, version: BP.version, fingerprint: FP, mode, order: [], halted: null, stoppedAt: null }

  const makeCtx = (stage, label) => wfpFreeze({
    stage: stage.id,
    phase: stage.title,
    // the progress label of this unit of work: "<stage>" or, in a chain, "<stage>/<step> #<item number>"
    label: label || stage.id,
    mode,
    args: A,
    log: m => log(`[${stage.id}] ${m}`),
    role: name => {
      if (!roles[name]) throw new Error(`stage ${stage.id}: role ${name} is not declared`)
      return roles[name]
    },
    // Modules start agents only through ctx.agent: the role decides model and effort, the stage decides the phase.
    agent: (prompt, o = {}) => {
      const r = roles[o.role || 'worker']
      if (!r) throw new Error(`stage ${stage.id}: role ${o.role || 'worker'} is not declared`)
      // o.phase: a module may file an agent under another declared phase title (progress view only).
      if (o.phase !== undefined && !phaseTitles.has(o.phase)) throw new Error(`stage ${stage.id}: phase "${o.phase}" is not declared`)
      const opts = { label: o.label || label || stage.id, phase: o.phase || stage.title }
      if (r.model) opts.model = r.model
      if (r.effort) opts.effort = r.effort
      if (r.agentType) opts.agentType = r.agentType
      if (r.isolation) opts.isolation = r.isolation
      if (o.schema) opts.schema = o.schema
      const instr = r.instructions ? `${r.instructions}\n\n` : ''
      return agent(instr + prompt, opts)
    },
  })

  async function step(stage, unit, scope, label) {
    const mod = WFP_MODULES[unit.module]
    const tries = 1 + (unit.retries || 0)
    let last, lastOut
    for (let t = 1; t <= tries; t++) {
      lastOut = undefined
      try {
        const params = wfpFreeze(wfpResolve(unit.params || {}, scope))
        const out = await mod.run(makeCtx(stage, label), params)
        if (out === null && mod.nullable !== true) throw new Error('the module returned nothing (an agent was skipped or died)')
        lastOut = out  // kept when a check fails, so the report shows why
        for (const c of unit.expect || []) if (!wfpTest(out, c)) throw new Error(`check failed: ${wfpDescribe(c)}`)
        return { ok: true, output: out, attempts: t }
      } catch (e) {
        last = e
        if (t < tries) log(`[${stage.id}] attempt ${t} failed: ${e.message}; one more try`)
      }
    }
    return { ok: false, error: String(last && last.message || last), attempts: tries, ...(lastOut !== undefined ? { output: lastOut } : {}) }
  }

  async function runStage(stage) {
    const scope = scopeBase()
    if (stage.when && !stage.when.every(c => wfpTest(scope, c))) return { status: 'skipped', reason: `when: ${stage.when.map(wfpDescribe).join('; ')}` }
    if (!stage.foreach) {
      const r = await step(stage, stage, scope)
      return r.ok ? { status: 'done', output: r.output, attempts: r.attempts } : { status: 'failed', error: r.error, attempts: r.attempts, ...(r.output !== undefined ? { output: r.output } : {}) }
    }
    // A per-item chain: each item runs through every step with no barrier between steps.
    const fe = stage.foreach
    const items = wfpResolve(fe.items, scope)
    if (!Array.isArray(items)) return { status: 'failed', error: `foreach.items did not resolve to an array (${fe.items})` }
    const max = fe.maxItems ?? BP.limits?.maxItems ?? 500
    if (items.length > max) return { status: 'failed', error: `${items.length} items, more than the declared limit ${max}` }
    const fails = []
    const chain = fe.chain.map((unit, k) => async (prev0, item, index) => {
      const prev = k === 0 ? undefined : prev0  // the runtime hands the first step the item itself
      if (prev && prev.__wfpFailed) return prev
      const steps = (prev && prev.__wfpSteps) || {}
      const s = { ...scope, item, index, prev: prev ? prev.value : undefined, steps }
      if (unit.when && !unit.when.every(c => wfpTest(s, c))) return { value: s.prev, __wfpSteps: { ...steps, [unit.id]: { status: 'skipped' } } }
      const r = await step(stage, unit, s, `${stage.id}/${unit.id} #${index + 1}`)
      if (!r.ok) {
        fails.push({ index, step: unit.id, error: r.error })
        return { __wfpFailed: true, index, step: unit.id, error: r.error, __wfpSteps: r.output !== undefined ? { ...steps, [unit.id]: { status: 'failed', output: r.output } } : steps }
      }
      return { value: r.output, __wfpSteps: { ...steps, [unit.id]: { status: 'done', output: r.output } } }
    })
    const done = await pipeline(items, ...chain)
    // Every item keeps its input and the outputs of its steps; a failed item also names the step and the error
    // (and keeps that step's output when the failure was a check on it).
    const stepsOf = d => Object.fromEntries(Object.entries((d && d.__wfpSteps) || {}).map(([k, v]) => [k, v.output]))
    const results = done.map((d, i) => d && !d.__wfpFailed
      ? { index: i, item: items[i], ok: true, steps: stepsOf(d) }
      : { index: i, item: items[i], ok: false, step: d ? d.step : '?', error: d ? d.error : 'item dropped', steps: stepsOf(d) })
    const output = { items: results, ok: results.filter(r => r.ok).length, failed: results.filter(r => !r.ok).length }
    const policy = fe.itemFail || 'collect'
    if (output.failed && policy === 'halt') return { status: 'failed', output, error: `${output.failed} item(s) failed: ${fails.slice(0, 3).map(f => `#${f.index + 1} ${f.step}: ${f.error}`).join('; ')}` }
    for (const c of stage.expect || []) if (!wfpTest(output, c)) return { status: 'failed', output, error: `check failed: ${wfpDescribe(c)}` }
    return { status: 'done', output }
  }

  // Run the stage graph: a stage starts when every stage in its "after" list has ended. Stages outside the
  // selected mode count as ended with no output. A failed stage with onFail "halt" stops new stages from starting.
  // A stage outside the mode passes through: it ends (with no output) only when every stage it waits for has
  // ended, so the order of the stages around it holds (a stage two steps after a running one still waits).
  const ended = new Set()
  const passThrough = new Set(BP.stages.filter(s => !selected.has(s.id)).map(s => s.id))
  const settle = () => {
    for (let moved = true; moved;) {
      moved = false
      for (const s of BP.stages) {
        if (passThrough.has(s.id) && !ended.has(s.id) && (s.after || []).every(d => ended.has(d))) { stages[s.id] = { status: 'not-in-mode' }; ended.add(s.id); moved = true }
      }
    }
  }
  let stop = false
  const running = new Map()
  const ready = () => BP.stages.filter(s => !passThrough.has(s.id) && !ended.has(s.id) && !running.has(s.id) && (s.after || []).every(d => ended.has(d)))
  const blockedByFailure = s => (s.after || []).some(d => stages[d] && (stages[d].status === 'failed' || stages[d].status === 'blocked') && !(BP.stages.find(x => x.id === d).onFail === 'continue'))
  while (true) {
    settle()
    if (!stop) {
      for (const s of ready()) {
        if (blockedByFailure(s)) { stages[s.id] = { status: 'blocked' }; ended.add(s.id); report.order.push(s.id); continue }
        running.set(s.id, runStage(s).then(r => ({ s, r }), e => ({ s, r: { status: 'failed', error: String(e && e.message || e) } })))
      }
    }
    if (!running.size) break
    const { s, r } = await Promise.race(running.values())
    running.delete(s.id)
    stages[s.id] = wfpFreeze(r)
    ended.add(s.id)
    report.order.push(s.id)
    log(`[${s.id}] ${r.status}${r.error ? ': ' + r.error : ''}`)
    if (r.status === 'failed' && (s.onFail || 'halt') === 'halt') { stop = true; report.halted = { stage: s.id, error: r.error } }
    if (until && s.id === until) { stop = true; report.stoppedAt = until }
    if (stop && !running.size) break
    settle()
    if (!stop && !ready().length && !running.size) break
  }
  for (const s of BP.stages) if (!stages[s.id]) stages[s.id] = passThrough.has(s.id) ? { status: 'not-in-mode' } : { status: report.halted ? 'not-run (halted)' : 'not-run (checkpoint)' }

  report.stages = Object.fromEntries(BP.stages.map(s => {
    const st = stages[s.id], o = st.output
    return [s.id, { status: st.status, ...(st.error ? { error: st.error } : {}), ...(s.foreach && o ? { items: { ok: o.ok, failed: o.failed } } : {}) }]
  }))
  report.result = BP.result ? wfpResolve(BP.result, scopeBase()) : Object.fromEntries(BP.stages.map(s => [s.id, stages[s.id].output]))
  if (typeof dsmpLog !== 'undefined') report.deepseek = dsmpLog
  return report
}
// ==== end Workflow Framework Kernel v1 ======================================================================

// ==== DeepSeek Subagent Managed Protocol v1.1 =================================================================
// Copy this block verbatim into a Workflow script (scripts are self-contained, so it cannot be imported).
// Spec: ~/.claude/skills/deepseek-subagent-managed-protocol/SKILL.md
//
//   const out = await dsDispatch(jobs, { label: 'translate', phase: 'Translate' })
//   out.results  -> one { id, ok, reply, attempts, model } per job, in input order
//
// A job is { id, run, expect?, retry_model?, approve? }:
//   id          unique string
//   run         the exact deepseek_run arguments: { cwd, prompt | prompt_files, model?, reasoning_effort?, skills?, images? }
//   expect      optional JavaScript regex source; the answer must match it (default: any non-empty answer)
//   retry_model optional model for the one retry
//   approve     optional rule for approvals (default DSMP_DEFAULT_APPROVE)
//
// Rules: one Haiku manager per 8 jobs (1-8 jobs -> 1 manager, 9-16 -> 2, ...), at most 8 managers alive at once
// in the whole script (so at most 64 DeepSeek conversations at once). Each manager deletes (never archives) every
// conversation it started once the answer is collected. When all jobs of a workspace (cwd) are done, that
// workspace is removed from the DeepSeek Desktop.
//
// v1.1 (2026-10-18): the manager starts every conversation with wait_seconds 0 (the bridge runs deepseek_run calls
// one after another, so a blocking run delayed each later start by 240 s), then loops: poll ALL uncollected
// sessions with wait_seconds 0 in one message, collect and delete the finished ones at once, block on ONE running
// session for at most pollSeconds, repeat. A session still running after budgetMinutes of its own run time is
// cancelled, recorded as a timeout (no retry) and deleted.

const DSMP = { perManager: 8, maxManagers: 8, pollSeconds: 90, budgetMinutes: 240, replyMax: 4000 }

const DSMP_DEFAULT_APPROVE = 'allow only if the tool call reads files, or creates or edits files inside the job\'s run.cwd; reject everything else (shell commands, deletes, network, installs, files outside run.cwd)'

const DSMP_MGR = {
  type: 'object',
  properties: {
    jobs: { type: 'array', items: { type: 'object', properties: {
      id: { type: 'string' }, ok: { type: 'boolean' },
      reply: { type: 'string', description: 'final answer verbatim, or the error' },
      attempts: { type: 'integer' }, model: { type: 'string', description: 'model of the attempt that produced reply' } },
      required: ['id', 'ok', 'reply', 'attempts'] } },
    started: { type: 'array', items: { type: 'string' }, description: 'session_id of every session you started' },
    deleted: { type: 'integer', description: 'sessions deleted in step 5' },
    not_deleted: { type: 'array', items: { type: 'string' }, description: '"session_id: reason" for each session not deleted' },
  },
  required: ['jobs', 'started', 'deleted', 'not_deleted'],
}

function dsmpManager(jobs, label, phase) {
  return agent(`You are a DeepSeek subagent manager. You start DeepSeek agents, collect their answers, and delete their conversations. You never edit files yourself.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_delete,mcp__deepseek__deepseek_cancel".

JOBS (${jobs.length}):
${JSON.stringify(jobs, null, 1)}

1. Start: in ONE message, call mcp__deepseek__deepseek_run once per job. Use the job's "run" object as the arguments, exactly, and add wait_seconds 0 (it replaces any wait_seconds in "run"). Change nothing else. Each call returns at once with status "running". Record each session_id.
2. Collect loop. A session is OPEN until you have collected it in step 4. Repeat a, b and c until no session is open:
   a. Poll: in ONE message, call mcp__deepseek__deepseek_wait with wait_seconds 0 for EVERY open session. These calls return at once.
   b. Act on every poll result before you wait again:
      - Status "finished", "failed" or "cancelled" (any status that is not "running" or "needs_response"): do steps 3, 4 and 5 for that session now. Collect and delete all such sessions in this round, in ONE message.
      - Status "needs_response" with an approval: use the job's "approve" rule; if the job has no "approve", the rule is: ${DSMP_DEFAULT_APPROVE}. Call mcp__deepseek__deepseek_respond with decision "allow" or "reject".
      - Status "needs_response" with a question: call mcp__deepseek__deepseek_respond with custom text "Follow the task message and use your best judgment."
      - Status "running" and elapsed_seconds more than ${DSMP.budgetMinutes * 60}: call mcp__deepseek__deepseek_cancel for it. In the next poll it is no longer running; then the job fails with reply "timeout after ${DSMP.budgetMinutes} min" and gets NO retry.
   c. Block: if a session is still "running", call mcp__deepseek__deepseek_wait with wait_seconds ${DSMP.pollSeconds} on ONE running session only (the one started first). Then go to a. Never use a larger wait_seconds. Never wait on two or more sessions with wait_seconds above 0.
3. Check: a job succeeds when status is "finished" and the answer matches the job's "expect" (a JavaScript regular expression). If the job has no "expect", any non-empty answer succeeds. If a job fails (also when deepseek_run returns an error), start it ONE more time with a new deepseek_run with the same "run" object and wait_seconds 0; if the job has "retry_model", set model to that value. The new session is open; the loop of step 2 collects it. Do not start a third attempt.
4. Collect: copy each job's final answer (or error) verbatim, at most ${DSMP.replyMax} characters.
5. Delete: after step 4 has the answer of a session, call mcp__deepseek__deepseek_delete with its session_id. Delete EVERY session you started, failed first attempts too. Never delete a session whose status is "running". Never call deepseek_archive. Never call deepseek_remove_workspace. If a delete is refused, do not retry it; record "session_id: reason" in not_deleted.
6. Return one entry per job: id, ok, reply, attempts, model. Also return started (every session_id you started), deleted (how many you deleted), and not_deleted.`,
    { label, phase, model: 'haiku', effort: 'low', schema: DSMP_MGR })
}

const DSMP_CLEAN = {
  type: 'object',
  properties: { folders: { type: 'array', items: { type: 'object', properties: {
    cwd: { type: 'string' }, removed: { type: 'boolean' },
    leftover: { type: 'array', items: { type: 'string' }, description: 'session_id of each conversation still in the workspace' },
    running: { type: 'integer' }, note: { type: 'string' } },
    required: ['cwd', 'removed', 'leftover', 'running'] } } },
  required: ['folders'],
}

function dsmpCleanup(cwds, own, label, phase) {
  return agent(`Remove finished DeepSeek workspaces from the DeepSeek Harness Desktop. These tools never delete a folder or its files on disk.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_remove_workspace".

FOLDERS:
${cwds.map(c => '- ' + c).join('\n')}

For each folder, in order:
1. Call mcp__deepseek__deepseek_list with cwd = that folder. If the workspace is not found, record removed=false, leftover=[], running=0, note "absent", and go to the next folder.
2. Record leftover = the session_id of every listed conversation, and running = how many are running.
${own
  ? `3. If running is 0: call mcp__deepseek__deepseek_remove_workspace with cwd = that folder and delete_conversations: true. Record removed=true. If running is more than 0: do not remove it; record removed=false, note "running".`
  : `3. If the list has no conversations: call mcp__deepseek__deepseek_remove_workspace with cwd = that folder (do not pass delete_conversations). Record removed=true. Otherwise do not remove it; record removed=false, note "has conversations".`}
If a call is refused, do not retry it; put the reason in note.

Never call deepseek_delete or deepseek_archive. Never call deepseek_list without cwd. Never touch a folder that is not in FOLDERS.
Return one entry per folder: cwd, removed, leftover, running, note.`,
    { label, phase, model: 'haiku', effort: 'low', schema: DSMP_CLEAN })
}

// One pool for the whole script: at most DSMP.maxManagers managers alive at once, across all dsDispatch calls.
let dsmpBusy = 0
const dsmpQueue = []
const dsmpPump = () => {
  while (dsmpBusy < DSMP.maxManagers && dsmpQueue.length) {
    dsmpBusy++
    const { fn, res, rej } = dsmpQueue.shift()
    Promise.resolve().then(fn).then(res, rej).then(() => { dsmpBusy--; dsmpPump() })
  }
}
const dsmpSlot = fn => new Promise((res, rej) => { dsmpQueue.push({ fn, res, rej }); dsmpPump() })

const dsmpLog = { deleted: 0, notDeleted: [], workspaces: [] }  // whole-script totals, for the final report

async function dsDispatch(jobs, opts = {}) {
  const { label = 'deepseek', phase, keepWorkspaces = false, ownWorkspaces = false } = opts
  const seen = new Set()
  for (const j of jobs) {
    if (!j || !j.id || seen.has(j.id)) throw new Error(`dsDispatch: missing or duplicate job id ${j && j.id}`)
    if (!j.run || !j.run.cwd || !(j.run.prompt || (j.run.prompt_files || []).length)) throw new Error(`dsDispatch: job ${j.id} needs run.cwd and run.prompt or run.prompt_files`)
    seen.add(j.id)
  }
  const groups = []
  for (let i = 0; i < jobs.length; i += DSMP.perManager) groups.push(jobs.slice(i, i + DSMP.perManager))
  const left = {}  // jobs not yet finished, per workspace
  jobs.forEach(j => { left[j.run.cwd] = (left[j.run.cwd] || 0) + 1 })
  const byId = {}, notDeleted = [], cleanups = []
  let deleted = 0
  await Promise.all(groups.map((g, i) => dsmpSlot(async () => {
    const r = await dsmpManager(g, `${label} mgr${i + 1}/${groups.length}`, phase)
    g.forEach(j => { byId[j.id] = { id: j.id, ok: false, reply: 'manager died', attempts: 0 } })
    if (r) {
      r.jobs.forEach(x => { if (seen.has(x.id)) byId[x.id] = x })
      deleted += r.deleted || 0
      notDeleted.push(...(r.not_deleted || []))
    }
    // a workspace is removed as soon as every job in it is done
    g.forEach(j => { left[j.run.cwd]-- })
    const done = [...new Set(g.map(j => j.run.cwd))].filter(c => left[c] === 0)
    if (!keepWorkspaces && done.length) cleanups.push(dsmpCleanup(done, ownWorkspaces, `${label} cleanup`, phase))
  })))
  const workspaces = (await Promise.all(cleanups)).filter(Boolean).flatMap(c => c.folders)
  dsmpLog.deleted += deleted
  dsmpLog.notDeleted.push(...notDeleted)
  dsmpLog.workspaces.push(...workspaces)
  return { results: jobs.map(j => byId[j.id]), deleted, notDeleted, workspaces }
}

// For scripts that pass keepWorkspaces: true (one cwd reused by several dsDispatch calls): call this once at the end.
async function dsCleanupWorkspaces(cwds, opts = {}) {
  const c = await dsmpCleanup([...new Set(cwds)], !!opts.ownWorkspaces, opts.label || 'deepseek cleanup', opts.phase)
  const folders = c ? c.folders : []
  dsmpLog.workspaces.push(...folders)
  return folders
}
// ==== end DeepSeek Subagent Managed Protocol v1.1 =============================================================

// ==== module gate v1 ====
// No agent. Fails the stage when a check on the subject fails, so the stage's onFail policy applies.
defineModule({
  id: 'gate', version: '1',
  summary: 'No agent: checks a subject with declarative checks and fails the stage when one fails.',
  params: {
    subject: { type: 'any', required: true, description: 'the value to check; usually a $ref to an earlier output' },
    checks: { type: 'array', required: true, description: 'list of {path, op, value, message}; ops as in stage expect' },
  },
  run: async (ctx, p) => {
    for (const c of p.checks) if (!wfpTest(p.subject, c)) throw new Error(`gate: ${wfpDescribe(c)}`)
    return { passed: p.checks.length }
  },
})
// ==== end module gate ====

// ==== module zt-workflow-lib v7 ====
// The code the ZT workflows share (docs/workflow-simplicity-proposal.md S1, S6; docs/workflow-module-structure-proposal.md):
// the digests (zt-digest-lib) and zt-idea-grow (zt-grow-cycle) load this one library instead of each keeping its own copy.
//  - S6, one lock reader: the owner's typed `card_locked` refusal (#140, R326/R337/R338) is read here and only
//    here. A refusal is a destination decision, never a provider fault: the stage stops with it and nothing
//    is retried, unlocked or re-pointed through another Access.
//  - the JSON line reader both workflows use on a relayed line.
//  - v2 (#175, workflow module structure): `ztDrive`, the ONE shared drive helper. It runs one deterministic
//    script step as a DeepSeek drive job through a Haiku DSMP manager and returns the step's parsed last JSON
//    line. No Claude agent runs a single command for a driver: the script step does the whole chain in one
//    process and the DeepSeek job answers with its one JSON line. The helper records and deletes the
//    conversation it starts, so the run cleanup can verify every provider resource it created.
//  - v3 (#179, workflow module structure slice 5): a drive job of a workflow that keeps no run ledger (zt-idea-grow
//    has none) records nothing: `record: false` leaves the ledger record commands out of the manager prompt and only
//    deletes the conversation. The relay-era command runner and its prompt left this library: no module of either
//    workflow names a relay any more. zt-digest-lib keeps a private copy beside its relay-era driver, which has had
//    no caller since #178 and is removed by a later cleanup.
//  - v4 (live run 2026-10-26): the drive job runs in the run folder (`out`), because deepseek_run refuses a cwd
//    that does not exist yet; the stage folder keeps the answers.
//  - v5 (#180): the reader is strict and the manager copies the line verbatim. On the live run a Haiku manager
//    retyped a step's long line as prose ("[digest/g001@1 through digest/g008@1]"); `ztLastJson` then took the
//    nested `counts` object, the stage read no `jobs`, ran no writer and ended `done` with eight units claimed.
//    Now a drive answer is ONLY its last non-empty line, that line must parse as a JSON object whose top-level
//    `ok` is a boolean, and anything else throws: the stage fails with the answer text. A nested object is never
//    taken, and `ztDrive` has no "any JSON in the text" fallback. The manager prompt says the `answer` field is the
//    job's final line copied character for character, and every step script prints at most 400 characters
//    (`scripts/book-digest/step-line.ps1`).
//  - v6 (live run 2026-10-26, staging4 link start): a Haiku manager copied the line with each `\\` of a Windows path
//    as one `\`, so the right line did not parse. The reader then doubles the lone backslashes of that same last line
//    and parses it once more; it is still only the last line, one object with a boolean top-level `ok`.
//  - v7 (live run 2026-10-26, staging5 cleanup report): a manager's `answer` carried the end of its own tool call after
//    the line (`…}</answer>` and a `<parameter name="session_id">` line). The answer is cut at the first such tag
//    (`ztDriveText`) before its last line is read; nothing else about the reading changes.
const ZT_LOCK = 'card_locked'
const ZT_DRIVE_MODEL = 'deepseek-official/deepseek-flash'
const ZT_DRIVE_SCHEMA = {
  type: 'object',
  properties: {
    session_id: { type: 'string' }, answer: { type: 'string' }, deleted: { type: 'boolean' }, registered: { type: 'boolean' }, evidence: { type: 'boolean' },
  },
  required: ['answer'],
}

// The JSON object in a line, also after prose on the same line ("…保留原样。{"address":…}"): the longest object
// from the first brace that parses (a relay may add a remark after the line, "(abridged)"). Used for the lines a
// relayed command printed, never for a drive job's answer (#180: `ztDriveAnswer`).
function ztJsonIn(line) {
  const text = String(line || '')
  for (let i = text.indexOf('{'); i >= 0; i = text.indexOf('{', i + 1)) {
    for (let j = text.lastIndexOf('}'); j > i; j = text.lastIndexOf('}', j - 1)) {
      try { const v = JSON.parse(text.slice(i, j + 1)); if (v && typeof v === 'object') return v } catch (e) { }
    }
  }
  return null
}
// The last line of a text that holds a JSON object (a relayed command's output).
function ztLastJson(text) {
  for (const line of String(text || '').trim().split('\n').reverse()) { const v = ztJsonIn(line); if (v) return v }
  return null
}
// The answer of a drive job (#180), or null. ONLY its last non-empty line counts, that line must be one JSON
// object (no prose around it, no remark after it), and its top-level `ok` must be a boolean. A retyped or
// summarized answer, a line the manager rewrote, a nested object without `ok`, and an empty answer are all null:
// the stage stops with the answer text instead of reading a field the step never printed.
// The one object of a drive answer's last line, or undefined: the line as given, else (v6) the line with its lone
// backslashes doubled, as a manager that copied a Windows path unescaped gives it.
function ztDriveParse(last) {
  try { return JSON.parse(last) } catch (e) { }
  if (!last.includes('\\')) return undefined
  try { return JSON.parse(last.replace(/\\\\/g, '\u0000').replace(/\\/g, '\\\\').replace(/\u0000/g, '\\\\')) } catch (e) { return undefined }
}
// A drive answer without the tool-call markup a manager may leak after the line (v7).
function ztDriveText(text) {
  const said = String(text === undefined || text === null ? '' : text)
  const at = said.search(/<\/?(?:answer|parameter|invoke|function_calls|antml:[a-z_]+)\b/)
  return at < 0 ? said : said.slice(0, at)
}
function ztDriveAnswer(text) {
  const lines = ztDriveText(text).split('\n').map((l) => l.trim()).filter(Boolean)
  if (!lines.length) return null
  const last = lines[lines.length - 1]
  const v = ztDriveParse(last)
  if (!v || typeof v !== 'object' || Array.isArray(v)) return null
  if (typeof v.ok !== 'boolean') return null
  return v
}
// Why an answer is not a drive answer, in the words the stage's error line uses.
function ztDriveWhy(text) {
  const said = ztDriveText(text).trim()
  if (!said) return 'the answer is empty'
  const lines = said.split('\n').map((l) => l.trim()).filter(Boolean)
  const last = lines[lines.length - 1]
  const v = ztDriveParse(last)
  if (v === undefined) return 'its last line is not one JSON object'
  if (!v || typeof v !== 'object' || Array.isArray(v)) return 'its last line is not a JSON object'
  if (typeof v.ok !== 'boolean') return 'its last line has no boolean top-level ok'
  return 'it is not a drive answer'
}
// The owner's typed refusal inside a relayed line, an answer or an error text, or null. A preflight prints
// `{ok:false, code:"card_locked", …}`; the archive's own MCP refusal prints `{error:"card_locked", …}`.
function ztLockRefusal(text) {
  for (const line of String(text || '').split('\n')) {
    const v = ztJsonIn(line)
    if (v && (v.code === ZT_LOCK || v.error === ZT_LOCK)) return v
  }
  return null
}
// The archive's own sentence about a refusal, for the stage's error line.
function ztLockSaid(refusal) {
  const said = refusal && (refusal.message || refusal.cause || refusal.error)
  return typeof said === 'string' && said.trim() ? said.trim() : ZT_LOCK
}
// The stop a stage throws: the archive's refusal, its sentence and its JSON. `lock_refusal` lets a caller that
// records unit faults keep the code instead of reading it as a content or provider fault.
function ztLockStop(where, refusal) {
  const error = new Error(`card_locked at ${where}: ${ztLockSaid(refusal)} ${JSON.stringify(refusal)}`)
  error.lock_refusal = refusal
  return error
}
// The exact prompt a drive job runs: the one command as given, then only its last line, copied.
function ztDriveCommand(command) {
  return `在 PowerShell 里原样运行下面这一条命令，不改写它，不运行别的命令，不修改任何文件。命令结束后，只回复它输出的最后一行：把那一行一字不改地复制过来，不要改写、缩写、翻译或补充任何字。那一行是一个紧凑的 JSON 对象，以 "ok" 开头。\n\n${command}`
}
// The one manager prompt of a drive job: start it, wait, retry once, record (when the workflow keeps a ledger) and
// delete the conversation. `record: false` is a workflow with no run ledger (zt-idea-grow): the job records nothing,
// keeps the answer under its work folder and only deletes its conversation.
// #180: the manager copies the final line character for character into `answer`; it never retypes or summarizes it.
function ztDrivePrompt(o) {
  const answers = o.answers || `${String(o.cwd || '').replace(/[\\/]+$/, '')}\\answers`
  const records = o.record === false
    ? `4. This workflow keeps no run ledger: record nothing. Delete the conversation (mcp__deepseek__deepseek_delete, never archive) as soon as you have its answer.`
    : `4. Records (each record command changes no Card; run it at the moment named, in the same message as your next call when you can):
- The moment deepseek_run returns a session_id: ${o.ledger || 'zt-digest ledger'} resource -Out "${o.out}" -Unit run -RegisterId <session_id> -Workspace "${o.cwd}". Use the session_id that deepseek_run returned, never a word in its place.
- A step whose job list is empty starts no run: never send a placeholder prompt.
- When the job ends: write its final answer text, unchanged, to "${answers}\\<session_id>.txt", then ${o.ledger || 'zt-digest ledger'} resource -Out "${o.out}" -EvidenceFor <session_id> -Evidence "${answers}\\<session_id>.txt".
- Then delete the conversation (mcp__deepseek__deepseek_delete, never archive) and record it: ${o.ledger || 'zt-digest ledger'} resource -Out "${o.out}" -Settle <session_id> -State finished -Deleted confirmed (-State error when the job ended in an error; -Deleted failed -Message '<the error>' when the delete fails).
- A record command that fails does not stop the work: report registered or evidence false and go on.`
  return `You are a DeepSeek subagent manager for a ZET workflow step (${o.label}). You start one DeepSeek agent, wait for it, collect its answer, record it and delete its conversation. You never write content and you never run the step's command yourself. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete".

1. Start the job: mcp__deepseek__deepseek_run with cwd "${o.cwd}", prompt ${JSON.stringify(ztDriveCommand(o.command))}, skills [], model "${o.model || ZT_DRIVE_MODEL}", reasoning_effort "${o.effort || 'high'}", wait_seconds 0.
2. Wait with mcp__deepseek__deepseek_wait (wait_seconds ${DSMP.pollSeconds}) until it ends. A job asking a question: answer "按任务说明自行决定，不需要询问。". A job asking approval for the one command, or for writing the answer file under "${answers}": allow; anything else: reject.
3. A job that failed or gave an empty answer gets ONE more attempt in a new conversation. Never a third.
${records}
The record commands, when this step has records, are the only commands you run: each in the foreground with timeout_ms: 600000. You have no terminal tabs, no Monitor loop, no session handoff and no general shell: never start another agent, never hand the work on, never poll with Start-Sleep or any other timer, and wait only with deepseek_wait. Keep at most 8 jobs running at a time.
5. Return answer: the job's final line copied character for character, with no word added, removed or changed: never a summary, never a retyped or shortened line. Also return session_id, deleted, registered and evidence.`
}
// ONE deterministic script step, run as a DeepSeek drive job through a Haiku DSMP manager. Returns the step's
// parsed last line (#180: only its last non-empty line, one JSON object with a boolean top-level `ok`).
// o: label, command, out, cwd, phase, ledger, record, answers, deepseek, model, effort.
// An answer that is not that line is a transport fault: it throws and the module stops with the answer text. A
// step that answers `{ok:false, ...}` is a failed step: the module reads that line and stops with its reason.
async function ztDrive(ctx, o) {
  const label = o.label || o.step || 'drive'
  const cwd = String(o.out || o.cwd || '').replace(/[\\/]+$/, '')
  const answers = o.answers || `${String(o.cwd || cwd).replace(/[\\/]+$/, '')}\\answers`
  const run = await dsmpSlot(() => ctx.agent(ztDrivePrompt({ ...o, label, cwd, answers }), {
    role: 'manager', label, phase: o.phase, schema: ZT_DRIVE_SCHEMA,
  }))
  const said = run && (run.answer !== undefined ? run.answer : run.output)
  if (o.deepseek) {
    if (run && run.deleted === true) o.deepseek.deleted = (o.deepseek.deleted || 0) + 1
    else if (run && run.session_id) (o.deepseek.not_deleted = o.deepseek.not_deleted || []).push(run.session_id)
  }
  const json = ztDriveAnswer(said)
  if (!json) {
    throw new Error(`${label}: the step's answer is not its one JSON line (${ztDriveWhy(said)}): ${String(said || 'no answer').trim().slice(0, 400)}`)
  }
  return json
}
defineModule({
  id: 'zt-workflow-lib', version: '7', requires: ['@dsmp'], library: true,
  summary: 'Library: what the ZT workflows share: the one reader of the owner\'s typed card_locked refusal, the JSON line reader, the strict drive answer reader (only the last line, one JSON object with a boolean top-level ok), and the one drive helper that runs a deterministic script step as a DeepSeek drive job through a Haiku manager (it records the conversation in the run ledger, or records nothing for a workflow that keeps none).',
})
// ==== end module zt-workflow-lib ====

// ==== module zt-digest-lib v44 ====
// 44 (#187): the card layer's review-and-fill job runs two read-only SELF-CHECK commands, the Card
// check and the coverage measurement; the manager's allow-list names both, and the group draft check
// (group-finish -CheckOnly) the digest group job runs (#186).
// 43 (live run 2026-10-26, staging6): a continued job whose conversation the Desktop answers as unknown or closed
// runs once as a new conversation from its fallback files, instead of failing the close round's repair.
// The shared machinery of the ZT digest workflows (docs/zt-digest-framework.md), first ported from the
// hand-written zt-book-digest.js (since removed): the settings read from the args, the digest and link managers, the
// compiler-step driver (drive), the ready-work driver (chain), and the card layer cycle (runCards). The model
// compiler is zt-model-lib (the model layer, which each book type installs). A library module: the stage modules
// call zdMake(ctx, params) and use what it returns.
// One card cycle per book, not one per block (R296): `runCards` runs the whole-book cycle, and its
// `-ModelReview` option lets that cycle own the source review of a Model draft no model chain elaborates (R247).
// 11 adds `fixRounds`: the finalization's one correction cycle is declared, not inherited from the pilot
// setting (#86, R299). 12 resolves the run's Citation key from the prepared run (parameter `key`, from
// `stages.prepare.output.citation_key`), so mode resume uses the ledger's retained key instead of requiring
// the operator to repeat args.citationKey (#87, R274/R276). 41 (#181) makes that the ONE source: a module reads
// the key from its own `key` param only (the blueprint passes args.citationKey to prepare, which creates the
// prepared run, and stages.prepare.output.citation_key everywhere else), so no module reads args.citationKey and
// a resume, which carries only `out`, never loses the book's identity; each step script reads the same key from
// <out>\manifest.json when its options name none. 13 makes every claim cover the provider run it
// starts: `ledger next -Lease <dshRunCap + 300>` (#92), and a chain renews that claim after every wait that
// reports a running job, so a healthy job that outlives the 120-second base lease is never re-readied.
// Every Haiku, Sonnet, and Opus agent goes through ctx.agent, so the blueprint's roles decide the models:
// manager (Haiku), architect (Sonnet), organizer (Opus). Managers share the DeepSeek Subagent Managed
// Protocol pool (dsmpSlot): at most 8 managers alive in the whole run.
// The ready-work schedule (9.3/9.4, #82): the ledger owns readiness. A driver claims ready units in bounded
// blocks (`ledger next`), runs each unit's compiler chain in ONE manager's conversation (`chain`), records the
// result through that family's checked finish command, and seals with the layer gate. Retries are per unit
// (`maxAttempts`), never a whole-stage rerun. No module reads a clock: the bounded readiness wait lives in
// zt-digest (`ledger next -Wait`).
// 31 (Tree Lock S17, #140): a write the owner's Lock refused is a planning error, never a provider fault
// (R326/R337). `zdLockRefusal` reads the archive's typed `card_locked` JSON out of a relayed line, and the
// stage stops with it — one failed script command, a failed compiler step, a claimed unit's start or
// checked finish, or a refused chain step — while the drafts and the receipts already committed stay. The
// unit keeps the refusal as its content fault (`code: card_locked`), never a retryable one, and nothing is
// retried, unlocked, or re-pointed through another Access (R338).
// 35 (docs/digest-claude-token-proposal.md, P1/P2/P4): a block's checked finishes run in one command
// (`zt-digest finish-batch`, one short line) instead of one relay per unit; the Cards layer claims and finishes
// in that one command; the events, findings and explain chains end with it in their own manager; and a compiler
// step's command agent loads no DeepSeek tool. Every finish keeps its own verifier, token and fault class.
// 36 (P5): a one-command relay (runScript) runs as the blueprint's relay role, the zt-relay agent type (PowerShell
// only, a short prompt): its first request carries about 9k tokens of context instead of about 59k.
// 37 (docs/workflow-simplicity-proposal.md S1, S5, S6, S8): the JSON line reader, the Lock reader and the relay are the
// shared library's (zt-workflow-lib, which zt-idea-grow loads too); a compiler step's command runs through the relay
// (no manager prompt runs a command); the DeepSeek source is frozen at the official one (no probe); the ledger is the
// one fault classifier (a fault leaves out its class and, where only a cause text says it, its code); and the dead code
// (the stage managers, runStage, drain, jobOf, renew, resourceList, finishCleanup, cleanupDsh, CARD_CHECKS,
// LINK_PER_MANAGER) and the unused or alias args (perManager, linkPerManager, dshPerSonnet, redoModels, dshSource,
// dshModel) leave.
// 38 (#175, docs/workflow-module-structure-proposal.md): the layer driver becomes script steps. `layerRun` drives one
// layer in blocks; every block is one `layer-steps.ps1` step run by a DeepSeek drive job (zt-workflow-lib's shared
// `ztDrive`): `start` claims the block and lists the jobs, the block's content jobs run in Haiku managers, and `finish`
// records every held unit's checked completion, faults the rest and runs the layer gate. A module never relays a
// command; per-unit outcomes are the ledger's and the step's JSON line. The relay-era block driver (block, blockSettle,
// blockFinish, blockPrompt) leaves. The relay helpers (`runScript`, `chain`, `claim`, `finishUnits`, `holding`, `gate`,
// `layer`, `ledgerReport`) stay only for the modules the later slices of the structure migrate (the model layer, the
// whole-book cycle, the preparation and the settlement).
// 39 (#176, docs/workflow-module-structure-proposal.md): the book-level stages run their chains as script steps too.
// `bookStep` runs one `book-steps.ps1` step (the preparation, a stage publication, the card cycle, Organize, Figures,
// the Bridge, the audit, the finalization, the Term-link round, the settlement) as a DeepSeek drive job; `cardCycle`
// drives the whole-book and per-scope card cycle in declared rounds; a resume reads its retained layers' gates through
// `bookStep`; and a content manager records run-level jobs (`run: true`) for a scope that owns no ledger unit. The
// relay-era driver stays only for the model and math layers (the later slices).
// 40 (#179, workflow module structure slice 5): the relay-era command runner (`zdRelay`, its prompt and its schema)
// moves into this library beside its only remaining caller, the relay-era driver no module has named since #178; the
// shared zt-workflow-lib v3 is drive-only, so no idea-grow build carries a relay.
const ZD_SCRIPTS = 'E:\\zet\\scripts\\book-digest'
// The digest scripts ported to zt-digest (tools/zt-digest, one fast binary): script name → tool. zdTool(name)
// is the command that runs a script: `zt-digest <tool>` once it is ported, else pwsh with the script. The
// helper is `zt-digest call` (ZD_CALL); the old helper path zt-mcp.ps1 forwards to it.
const ZD_DIGEST = 'zt-digest'
const ZD_TOOLS = {
  'digest-audit': 'audit', 'card-compiler': 'card', 'model-compiler': 'model', 'model-context': 'model-context', 'list-models': 'list-models',
  'bridge-compiler': 'bridge', 'math-compiler': 'math', 'event-compiler': 'event', 'finding-compiler': 'finding',
  'book-terms': 'terms', 'book-skeleton': 'skeleton', 'organize-survey': 'organize-survey', 'organize-apply': 'organize-apply',
  'attach-figures': 'figures', 'prepare-book-digest': 'prepare', 'prepare-link-pass': 'prepare-link', 'snapshot-topics': 'snapshot-topics',
}
const ZD_CALL = `${ZD_DIGEST} call`
// 32 (docs/literature-only-digest-design.md): the helper runs in the Literature scope only without -Open, so
// every approval rule that lets a DeepSeek job run it refuses a helper command that carries -Open.
// 33: the hub settings (hubWork, the hub compiler, hubEffort, hubFixRounds) and the hub compiler's self-check in the
// approval rules leave the library with the unreachable zt-concepts, zt-review and zt-hubs modules: hubs are zt-idea-grow's.
const ZD_NO_OPEN = 'refuse a zt helper command that carries -Open, in any letter case: it leaves the digest\x27s Literature scope'
const ZD_LEDGER = `${ZD_DIGEST} ledger`
// R265: every manager's compiler command carries this explicit timeout (milliseconds).
const ZD_COMMAND_TIMEOUT_MS = 600000
// R269: a write-capable compiler command gives its child `zt mcp` this many seconds of edit patience.
const ZD_EDIT_PATIENCE = 180
function zdTool(name, scripts = ZD_SCRIPTS) {
  return ZD_TOOLS[name] ? `${ZD_DIGEST} ${ZD_TOOLS[name]}` : `pwsh -NoProfile -File "${scripts}\\${name}.ps1"`
}
const ZD_DSH_SOURCES = {
  official: 'deepseek-official/deepseek-flash',
}
const ZD_PER_MANAGER_MAX = 8
const ZD_MAX_MANAGERS = 8
// R343–R346: run usage is per provider run, recorded from that run's own result. This framework never
// reads the account-wide usage ledger: the difference of two rolling totals is not a run's cost (I22/R344).
const ZD_RUN_SCHEMA = { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] }

// The JSON line reader of the shared library (zt-workflow-lib).
// (thin wrappers: a module file is loaded alone to read its manifest, so it names no other library at load time)
function zdJsonIn(line) { return ztJsonIn(line) }
function zdLastJson(text) { return ztLastJson(text) }

// --- Tree Lock: the owner's refusal is a planning error, not a provider fault (#140, R326/R337) ------
// A write into a tree the owner locked is refused by the storage gate with its typed `card_locked`
// refusal (a preflight prints `{ok:false, code:"card_locked", …}`; the archive's own MCP refusal
// prints `{error:"card_locked", …}`). The stage reads that JSON, never an English sentence (R338):
// it stops with the refusal as its error, keeps the drafts and the receipts it already committed,
// and never retries the refused write or asks for another Access (R326).
// S6: the one Lock reader is the shared library's (zt-workflow-lib); these names keep the stage modules' calls.
const ZD_LOCK = 'card_locked'
function zdLockRefusal(text) { return ztLockRefusal(text) }
function zdLockSaid(refusal) { return ztLockSaid(refusal) }
function zdLockStop(where, refusal) { return ztLockStop(where, refusal) }

// The DeepSeek source is frozen at the official one (the user's rule, 2026-10-20; S1): no probe, no argument.
function zdDefaultModel() {
  return { source: 'official', model: ZD_DSH_SOURCES.official }
}

function zdMake(ctx, p) {
  const A = p.args || {}
  const MODE = p.mode || 'full'
  const KEY = (p && p.key) || ''
  const OUT = A.out
  const profile = p.profile || {}
  const SCRIPTS = ZD_SCRIPTS
  const DSH_MODEL = p.model || zdDefaultModel().model
  const DIGEST_PREFIX = `${OUT}\\${profile.digestPrefix || 'shared-prefix.md'}`
  const LINK_PREFIX = `${OUT}\\${profile.linkPrefix || 'link-prefix.md'}`
  // Extra instruction files of the model compiler (math: -BriefDoc, -FillDoc, -ReviewDoc), in the order given.
  const COMPILER_DOCS = Object.entries(profile.compilerDocs || {}).map(([k, f]) => ` -${k} "${OUT}\\${f}"`).join('')
  const SKIP_FORMS = profile.skipForms || []
  const PER_MANAGER = ZD_PER_MANAGER_MAX
  const raised = []
  const atLeastHigh = (value, name) => {
    if (value === 'high' || value === 'max') return value
    if (value) raised.push(`${name} ${value} → high`)
    return 'high'
  }
  const DSH_EFFORT = atLeastHigh(A.dshEffort, 'dshEffort')
  const MODELS = A.models !== false
  const ARCH_DOC = `${SCRIPTS}\\model-architect.md`
  const ARCH_SUPPLEMENT = profile.architectSupplement ? `${SCRIPTS}\\${profile.architectSupplement.file}` : null
  const COMPILER = zdTool('model-compiler', SCRIPTS)
  const MODEL_WORK = `${OUT}\\models`
  const PER_SONNET = Math.max(1, A.perSonnet || 8)
  const DSH_PER_BATCH = Math.min(ZD_PER_MANAGER_MAX, Math.max(1, A.dshPerBatch || 8))
  const REBUILD = !!A.rebuild
  const LEVEL = ['concept', 'formal'].includes(A.level) ? A.level : null
  const PER_CONCEPT = Math.max(1, A.perConcept || 10)
  const BRIEF_EFFORT = atLeastHigh(A.briefEffort, 'briefEffort')
  const DERIVE_EFFORT = atLeastHigh(A.deriveEffort, 'deriveEffort')
  const REVIEW_EFFORT = atLeastHigh(A.reviewEffort, 'reviewEffort')
  const REVIEW_ROUNDS = Math.max(0, A.reviewRounds ?? 2)
  const LIMITS = `-MaxSentences ${A.maxSentences || 12} -MaxChars ${A.maxPartChars || 400} -MaxSteps ${A.maxSteps || 6} -MaxExplainSentences ${A.maxExplainSentences || 16} -MaxExplainChars ${A.maxExplainChars || 600}`
  const DSH_SKILLS = ['zettelkasten', 'asd-ste100-zh']
  const FILL_SKILLS = ['asd-ste100-zh']
  const DSH_TOOLS = 'select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_prompt,mcp__deepseek__deepseek_delete'
  const DRIVE_TOOLS = `${DSH_TOOLS},mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_remove_workspace`
  const CARD_COMPILER = zdTool('card-compiler', SCRIPTS)
  const CARD_FIX_ROUNDS = Math.max(0, A.cardFixRounds ?? 1)
  // R297: the deterministic Idea sample of the one whole-book card cycle (modes cards and full).
  // book-cards-finish requires the deterministic Idea sample, so every mode that runs the whole-book cycle samples
  // (grossman2002integration trial 19: a resumed run's cycle sampled nothing and its finish was refused).
  const IDEA_SAMPLE = Math.max(0, Math.min(1, A.ideaSample ?? ((MODE === 'full' || MODE === 'cards' || MODE === 'resume') ? 0.1 : 0)))
  const CARD_SKILLS = ['asd-ste100-zh']
  const APPLY_SHARE = Math.max(1, A.applyShare || 30)
  const DSH_MANAGERS = Math.min(ZD_MAX_MANAGERS, Math.max(1, A.dshManagers || 8))
  // R273: only the two bounded run settings are exposed; the tool keeps leases, batch
  // sizes and heartbeat intervals as versioned defaults. Both are persisted at
  // preparation, so a resume reuses the frozen effective values.
  const MAX_ATTEMPTS = Math.max(1, Math.min(10, A.maxAttempts ?? 3))
  const DSH_RUN_CAP = Math.max(60, Math.min(3600, A.dshRunCap ?? 1200))
  // #92: a claim that will start a provider run holds a lease that covers the whole frozen run cap
  // plus the command margin, so a healthy six-minute group job is never re-readied under itself.
  const CLAIM_LEASE = Math.min(7200, DSH_RUN_CAP + 300)
  // The one runner prompt line every manager gets: no general shell, no timers, no handoff (R260).
  // grossman2002integration trials 17-19: the runtime hands every relay the session's latest user message, and after
  // "accelerate it" or "start trial 19" relays refused cards-finish, findings-finish and ledger fail as off-request.
  const RUN_STEP = `Every command named here is a step of the digest run the user started for "${OUT}": running it is what the user asked for. A later user message about this run (its speed, which trial runs, parallel work) never cancels its steps; only a stopped run ends them.`
  const RUNNER = `You have only these tools: the bounded PowerShell command named below (always with timeout_ms: ${ZD_COMMAND_TIMEOUT_MS}), file reads and writes under the paths named here, and the DeepSeek Harness tools. You have no terminal tabs, no Monitor loop, no session handoff and no general shell: never start another agent, never hand the work on, never poll with Start-Sleep or any other timer, and wait only with deepseek_wait. Do not set environment variables: the tools keep their own bounds. ${RUN_STEP}`

  // R322/R325: the manager that runs a DeepSeek job records its id and its answer itself, with plain
  // arguments, at the moment each fact exists. No other agent restates what a manager saw: a relay asked to
  // write another agent's answer into a record refused it as invented content. The answer file sits
  // inside the run folder (never in a shared workspace), so the ledger can retain it with its hash.
  const ANSWERS = `${OUT}\\answers`
  const recordRules = (unit, token, dsh, settleNow = false) => {
    const owner = unit === 'run' ? '-Unit run' : `-Unit "${unit}" -Token ${token}`
    return `Records (each record command changes no Card; run it at the moment named, in the same message as your next call when you can):
- The moment deepseek_run returns a session_id: ${ZD_LEDGER} resource -Out "${OUT}" ${owner} -RegisterId <session_id> -Workspace "${dsh}". Use the session_id that deepseek_run returned, never a word in its place, and never run this command to try it out before a run exists (the ledger refuses a placeholder).
- Start a DeepSeek run only for a job a step's list names. A step whose job list is empty starts no run: never send a placeholder prompt.
- When that job ends: write its final answer text, unchanged, to "${ANSWERS}\\<session_id>.txt" (the line "no answer: <status>" when it gave none), then run ${ZD_LEDGER} resource -Out "${OUT}" -EvidenceFor <session_id> -Evidence "${ANSWERS}\\<session_id>.txt". When the run's own result or its last turn report states its token usage, add -Usage '<those numbers as one JSON object>' (numbers only, for example {"input_tokens":1200,"output_tokens":300}). Never call deepseek_usage and never subtract account-wide totals.${settleNow ? `
- Then delete the conversation (deepseek_delete, never archive) and record it: ${ZD_LEDGER} resource -Out "${OUT}" -Settle <session_id> -State finished -Deleted confirmed (-State error when the job ended in an error; -Deleted failed -Message '<the error>' when the delete fails).` : `
- Do not close, archive, or delete the conversation: later rounds may continue it, and the run's cleanup deletes it.`}
- A record command that fails does not stop the work: put registered/evidence false on that job and go on.`
  }

  // Every agent through the blueprint's roles: Haiku is "manager", Sonnet "architect", Opus "organizer"; a
  // one-command relay asks for "relay" (o.role).
  const zAgent = (prompt, o) => ctx.agent(prompt, {
    role: o.role || (o.model === 'sonnet' ? 'architect' : o.model === 'opus' ? 'organizer' : 'manager'),
    label: o.label, phase: o.phase, schema: o.schema,
  })
  // 40 (#179, docs/workflow-module-structure-proposal.md): the relay-era command runner moves here with its only
  // remaining caller, this library's relay-era driver (no module has named it since #178). The shared library
  // (zt-workflow-lib v3) is drive-only now, and the idea-grow build carries no relay at all.
  const ZD_RELAY_SCHEMA = { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] }
  const ZD_RELAY_TIMEOUT_MS = 600000
  const zdRunStep = (run) => `Every command named here is a step of ${run}: running it is what the user asked for. A later user message about this run (its speed, which trial runs, parallel work) never cancels its steps; only a stopped run ends them.`
  // One relay (the zt-relay agent type, the blueprint's role relay) runs one script command and returns its last JSON
  // line (or null). A gate prints its JSON and then exits 3 while the layer still has open work, so a caller that
  // asks (o.always) still reads a typed result. R326/R337: a command whose answer carries the owner's typed refusal
  // stops the stage here (the shared Lock reader); a read that merely names a retained fault keeps going.
  async function zdRelay(ctx, o) {
    const run = await ctx.agent(
      `Run this PowerShell command once in the foreground with timeout_ms: ${ZD_RELAY_TIMEOUT_MS} and wait for it${o.note || ''}; do nothing else:
${o.command}
${zdRunStep(o.run || 'the ZT digest run the user started')}
Return ok (true when it exits without an error) and output = its last output line exactly (a compact JSON object), or the error text when it fails. Do not retry.`,
      { role: o.role || 'relay', label: o.label, phase: o.phase, schema: ZD_RELAY_SCHEMA },
    )
    const json = run && (run.ok || o.always) ? zdLastJson(run.output) : null
    const refused = !!(json && (json.code === ZD_LOCK || json.error === ZD_LOCK))
    const lock = zdLockRefusal(run && run.output) || (refused ? json : null)
    if (lock && (!(run && run.ok) || refused) && o.lockStops !== false) throw zdLockStop(o.label, lock)
    return { run, json }
  }
  const runScript = (label, phaseName, command, note = '', o = {}) => zdRelay(ctx, {
    command, note, label, phase: phaseName, role: 'relay', always: !!o.always, run: `the digest run the user started for "${OUT}"`,
  })

  const groupsOf = (count) => Array.from({ length: count || 0 }, (_, i) => {
    const n = i + 1
    const nn = String(n).padStart(2, '0')
    return { n, nn, file: `${OUT}\\groups\\g${nn}.md`, snapshot: `${OUT}\\snapshot-g${nn}.md`, link: `${OUT}\\link-g${nn}.md` }
  })
  const toBlocks = (list, size = PER_MANAGER) => {
    const out = []
    for (let i = 0; i < list.length; i += size) out.push(list.slice(i, i + size))
    return out
  }

  // --- The compiler-step driver (drive): one Haiku agent runs a step, managers run its DeepSeek jobs ---
  const DRIVE = {
    type: 'object',
    properties: {
      ok: { type: 'boolean' }, error: { type: 'string' }, output: { type: 'string' },
      results: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' }, registered: { type: 'boolean' }, evidence: { type: 'boolean' } }, required: ['id', 'status'] } },
      deleted: { type: 'number' },
    },
    required: ['ok', 'results'],
  }
  // One Haiku driver runs one compiler step and the DeepSeek jobs it prints. o: command, dsh (the DSH
  // workspace), skills, effort, sessions (continued jobs), cleanup (the last round), phase, writes, keep.
  const driveAgent = (label, o, part) => {
    const table = Object.entries(o.sessions || {}).map(([job, session]) => `${job} ${session}`).join('\n')
    const lead = `You drive DeepSeek Harness (DSH) jobs for a ZET compiler script. You do not read the book and you do not write Cards: a script and DSH do. Keep your messages short.
${RUNNER}
`
    const head = `${lead}Load the tools with one ToolSearch call: query "${DRIVE_TOOLS}".
`
    if (part === 'command') {
      // S1 (37): the step command runs through the relay, like every other command; the command itself saves its
      // last line as the step's job list, which the job managers read (a relay never retypes a job list).
      const command = o.jobsFile ? `${o.command} | Tee-Object -Variable zdLines; $zdExit = $LASTEXITCODE; [void][System.IO.Directory]::CreateDirectory("${OUT}\\jobs"); ($zdLines | Select-Object -Last 1) | Set-Content -LiteralPath "${o.jobsFile}" -Encoding utf8; if ($zdExit) { exit $zdExit }` : o.command
      return ztRelay(ctx, { command, label, phase: o.phase || 'Models', role: 'relay', lockStops: false, run: `the digest run the user started for "${OUT}"` })
        .then(({ run }) => ({ ok: !!(run && run.ok), output: (run && run.output) || '', error: run && !run.ok ? run.output : undefined, results: [] }))
    }
    const jobs = o.jobs || []
    return dsmpSlot(() => zAgent(`${head}
${o.jobsFile ? `Run the jobs with these ids: ${JSON.stringify(jobs.map((j) => j.id))}. Other managers run the other jobs of this step at the same time; run only these.
Their exact files, prompt, continue and fallback are in the job list the step command saved: read "${o.jobsFile}" once (one JSON line) and take each job from its "jobs" array by id, exactly as written. Never retype a path. A compact list (it has "lead" and "prompts") expands each entry: kind "fix" → files [lead.fix, dir\\file], prompt prompts.fix; kind "orphans" → files [lead.orphans, dir\\file], prompt prompts.orphans; kind "follow" → files [dir\\file], prompt prompts.follow, continue = its "continue", fallback [lead.fix, dir\\file].` : `Run every job below. A job has id, files, prompt, and sometimes continue and fallback. Other managers run the other jobs of this step at the same time; run only these.
${JSON.stringify(jobs)}`}
- No continue: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, model "${DSH_MODEL}", reasoning_effort "${o.effort}", wait_seconds 0.
- continue: find the session of the job id named by continue in this table (job id, then session_id):
${table || '(empty)'}
  With a session: deepseek_prompt with that session_id, prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, reasoning_effort "${o.effort}", wait_seconds 0. Without one: deepseek_run as above, with prompt_files = fallback. When deepseek_prompt answers that the session is unknown or closed, run the job once as a new conversation: deepseek_run as above, with prompt_files = fallback.
Keep at most ${DSH_PER_BATCH} jobs running at a time; start the next job when one ends. Start independent jobs in one message.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 600).
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: the Bridge compiler (${ZD_DIGEST} bridge, or bridge-compiler.ps1) with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}), or the check zt-check (read-only); a read-only self-check of a draft: the card, event, or finding compiler (${ZD_DIGEST} card, event, or finding, or card-compiler.ps1, event-compiler.ps1, finding-compiler.ps1) with -Step precheck, the model compiler (${ZD_DIGEST} model, or model-compiler.ps1) with -Step explain-check, the coverage measurement (${ZD_DIGEST} coverage), the group draft check (${ZD_DIGEST} group-finish with -CheckOnly), or the book skeleton (${ZD_DIGEST} skeleton, or book-skeleton.ps1) with -Step apply and without -Apply; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
${(() => { const owner = resourceOwner(o); return owner === 'run' ? recordRules('run', 1, o.dsh) : recordRules(owner.unit, owner.token, o.dsh) })()}
Return ok=true, output="", and results: one entry per job with id, session_id, status ("done" or "failed"), answer, registered and evidence (true when those record commands succeeded).`,
      { label, phase: o.phase || 'Models', model: 'haiku', schema: DRIVE }))
  }
  // A step's jobs. A compact list (card-compiler.ps1 -Compact) expands to { id, files, prompt, continue?, fallback? }.
  const expandJobs = (parsed) => {
    const list = (parsed && Array.isArray(parsed.jobs)) ? parsed.jobs : []
    if (!parsed || !parsed.lead || !parsed.prompts) return list
    const at = (name) => `${parsed.dir}\\${name}`
    return list.map(j => {
      if (j.files) return j
      if (j.kind === 'follow') return { id: j.id, continue: j.continue, files: [at(j.file)], fallback: [parsed.lead.fix, at(j.file)], prompt: parsed.prompts.follow }
      const lead = j.kind === 'orphans' ? parsed.lead.orphans : parsed.lead.fix
      return { id: j.id, files: [lead, at(j.file)], prompt: parsed.prompts[j.kind] }
    })
  }
  // Runs one compiler step: the command, its jobs spread over up to DSH_MANAGERS managers (one per
  // DSH_PER_BATCH jobs, all under dsmpSlot). The run ids are recorded at once; the run-level cleanup
  // later deletes exactly those ids (R322/R323).
  // A drive step's job list is saved by the command itself (Tee-Object → Set-Content), one file per step, so
  // the managers read the exact files and prompts: a relay that retyped a long job list dropped part of a
  // path (grossman2002integration trial 2: a derivation job named a folder that does not exist).
  let jobsSeq = 0
  const drive = async (label, o0) => {
    const o = { ...o0, jobsFile: `${OUT}\\jobs\\${String(label).replace(/[^A-Za-z0-9._-]+/g, '-').slice(0, 60)}-${++jobsSeq}.json` }
    const step = await driveAgent(label, o, 'command')
    if (!step || !step.ok) {
      // R326/R337: the compiler step the owner's Lock refused stops the stage with its own refusal.
      const lock = zdLockRefusal((step && (step.error || step.output)) || '')
      if (lock) throw zdLockStop(label, lock)
      return { ok: false, error: (step && step.error) || 'no report from the step', output: (step && step.output) || '', results: [] }
    }
    const parsed = zdLastJson(step.output)
    const jobs = expandJobs(parsed)
    let results = []
    if (jobs.length) {
      const shares = []
      for (let i = 0; i < jobs.length; i += DSH_PER_BATCH) shares.push(jobs.slice(i, i + DSH_PER_BATCH))
      const managers = shares.length
      const done = new Array(managers)
      let next = 0
      const lane = async () => {
        while (next < managers) {
          const i = next++
          done[i] = await driveAgent(managers > 1 ? `${label} [${i + 1}/${managers}]` : label, { ...o, jobs: shares[i] }, 'jobs')
            .catch(error => ({ ok: false, error: String(error), results: [] }))
        }
      }
      await Promise.all(Array.from({ length: Math.min(DSH_MANAGERS, managers) }, lane))
      results = done.flatMap((d, i) => {
        const got = (d && d.results) || []
        const missing = shares[i].filter(j => !got.some(r => r.id === j.id)).map(j => ({ id: j.id, status: 'failed', answer: '' }))
        return [...got, ...missing]
      })
      const failedManagers = done.filter(d => !d || !d.ok).length
      if (failedManagers) log(`${label}: ${failedManagers} of ${managers} managers reported a failure.`)
    }
    // R322: each job manager recorded its own ids and answers; name the jobs whose records failed.
    const unrecorded = results.filter((r) => r.session_id && r.registered === false).map((r) => r.id)
    if (unrecorded.length) log(`${label}: ${unrecorded.length} job id(s) not recorded by their manager: ${unrecorded.join(', ')}`)
    return { ok: true, output: step.output, results, deleted: 0 }
  }
  const answerLines = (driven) => ((driven && driven.results) || []).flatMap(r => String(r.answer || '').split('\n').map(zdJsonIn).filter(Boolean))

  // --- The ready-work driver (9.3, R250-R258): claim a bounded block, run each unit's chain in one manager ----
  // One manager per chain: it runs the chain's commands and DeepSeek jobs in its own conversation (R250/R251).
  // The chain is declared here, frozen in the blueprint's module versions, never improvised at run time.
  const CHAIN = {
    type: 'object',
    properties: {
      steps: {
        type: 'array',
        items: {
          type: 'object',
          properties: { name: { type: 'string' }, ok: { type: 'boolean' }, exit: { type: 'number' }, output: { type: 'string' }, error: { type: 'string' } },
          required: ['name', 'ok'],
        },
      },
      results: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            id: { type: 'string' }, step: { type: 'string' }, session_id: { type: 'string' },
            status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' },
            registered: { type: 'boolean' }, evidence: { type: 'boolean' },
          },
          required: ['id', 'status'],
        },
      },
    },
    required: ['steps', 'results'],
  }
  // The steps a chain runs: { name, command, jobs?, repeat? }. `jobs` collects the DeepSeek jobs the step
  // prints (the default). `repeat` runs the same command again while its JSON still reports work, at most
  // `max` times; that is the bounded correction path, not an unbounded loop. A command may hold
  // {{unit}}, {{token}}, {{group}} and {{draft}}; they come from what the chain has already read: the claim
  // step's `units[0]` (its `id`, `token` and the number in `gNNN`) and a start step's `draft`.
  const chainPrompt = (label, o) => {
    const steps = o.steps || []
    // #92: a chain that holds a claim keeps it current while its jobs run, so a job that outlives
    // the base lease still finishes under its own token.
    const claimedUnit = o.unit ? `"${o.unit}"` : '{{unit}}'
    const claimedToken = o.unit ? String(o.token) : '{{token}}'
    const holdsClaim = !!o.unit || steps.some(s => s.claim)
    const renewLine = holdsClaim
      ? `\n- While any job of this chain runs, keep the claim current: after every deepseek_wait that answers "running", run this command in the same message (it changes no Card; it renews the claim this chain holds):\n   ${ZD_LEDGER} renew -Out "${OUT}" -Unit ${claimedUnit} -Token ${claimedToken}`
      : ''
    const listed = steps.map((s, i) => {
      const lines = [`${i + 1}. ${s.name}: run this command in the foreground with timeout_ms: ${ZD_COMMAND_TIMEOUT_MS} and wait for it, then read its last output line (compact JSON, it may be tens of kilobytes; keep it whole):`, `   ${s.command}`]
      if (s.claim) lines.push(`   This is the claim step. If its JSON has an empty "units" array, stop the chain here: run no later step and no job, and return the steps you ran. Otherwise remember its "units"[0].id, "units"[0].token and the group number in that id (the digits in gNNN) for the commands below.`)
      if (s.jobs) lines.push(`   When its JSON has a nonempty "jobs" array, run those jobs before the next step (the job rules below). When it names a "job" file instead (a group start), read that file and run the jobs it declares (its "jobs" entries name files and prompt; its "expected_jobs" must equal the number you run).`)
      else lines.push(`   It starts no DeepSeek job by itself.`)
      if (s.repeat) lines.push(`   Repeat this same command while its JSON still reports ${s.repeat.path} ${s.repeat.op || '>'} ${s.repeat.value}, at most ${s.repeat.max || 100} times, and record the last JSON in this step's output.`)
      if (s.optional) lines.push(`   A failure here is recorded and the chain goes on with the next step.`)
      if (s.always) lines.push(`   Run this step even when an earlier step failed or printed no JSON.`)
      return lines.join('\n')
    }).join('\n')
    return `You run one compiler chain of a ZET book digest: ${label}. A script and DeepSeek Harness (DSH) do the work; you only run the bounded commands and the DSH jobs they print. Keep your messages short.
${RUNNER}
Load the DSH tools with one ToolSearch call: query "${DRIVE_TOOLS}".
The chain is fixed. Run every step in this order and nothing else. A command may contain {{unit}}, {{token}}, {{group}} or {{draft}}: replace each with the value the chain has already read (the claim's unit id, its token, the digits of that id's gNNN, and a start step's "draft").
CHAIN PLAN: ${JSON.stringify({ dsh: o.dsh, steps: steps.map(s => ({ name: s.name, command: s.command, claim: !!s.claim, jobs: !!s.jobs, repeat: s.repeat || null, optional: !!s.optional, ...(s.always ? { always: true } : {}) })) })}
${listed}
Jobs (for every step that prints them):
- A compact list (the card compiler's -Compact) is { dir, lead: { fix, orphans }, prompts: { fix, orphans, follow }, jobs: [{ id, kind, file, continue? }] }. Expand each entry: kind "fix" → files [lead.fix, dir\file], prompt prompts.fix; kind "orphans" → files [lead.orphans, dir\file], prompt prompts.orphans; kind "follow" → files [dir\file], prompt prompts.follow, and continue = its "continue" id. Any other entry already names files and prompt.
- No continue: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills || [])}, model "${DSH_MODEL}", reasoning_effort "${o.effort || DSH_EFFORT}", wait_seconds 0.
- continue: reuse the session_id of the earlier job of this chain with that id (from this chain's own results); without one, deepseek_run with prompt_files = fallback (or files when it names none). When deepseek_prompt answers that the session is unknown or closed, run the job once as a new conversation the same way.
Keep at most ${DSH_PER_BATCH} jobs running at a time; start independent jobs in one message and start the next job when one ends.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 90), then start the next waiting job and keep polling; never wait longer than 90 seconds at once.${renewLine}
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: the Bridge compiler (${ZD_DIGEST} bridge, or bridge-compiler.ps1) with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}), or the check zt-check (read-only); a read-only self-check of a draft: the card, event, or finding compiler (${ZD_DIGEST} card, event, or finding, or card-compiler.ps1, event-compiler.ps1, finding-compiler.ps1) with -Step precheck, the model compiler (${ZD_DIGEST} model, or model-compiler.ps1) with -Step explain-check, the coverage measurement (${ZD_DIGEST} coverage), the group draft check (${ZD_DIGEST} group-finish with -CheckOnly), or the book skeleton (${ZD_DIGEST} skeleton, or book-skeleton.ps1) with -Step apply and without -Apply; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
${o.unit ? recordRules(o.unit, o.token, o.dsh) : steps.some((s) => s.claim) ? recordRules('{{unit}}', '{{token}}', o.dsh) : recordRules('run', 1, o.dsh)}
Return steps: one entry per step with name, ok (true when it exited without an error), exit (its exit code when known), output (its last line unchanged) or error (the failure text); and results: one entry per job with id, step, session_id, status ("done" or "failed"), answer, registered and evidence (true when those record commands succeeded).`
  }
  // Runs one declared chain in ONE manager. o: label, dsh, skills, effort, phase, writes, steps, sessions, cleanup, keep.
  const chain = async (label, o) => {
    const run = await dsmpSlot(() => zAgent(chainPrompt(label, o), {
      label, phase: o.phase || 'Models', model: 'haiku', schema: CHAIN,
    }))
    const driven = run || { steps: [], results: [] }
    const steps = Array.isArray(driven.steps) ? driven.steps : []
    const results = Array.isArray(driven.results) ? driven.results : []
    // R326/R337: a chain step whose command the owner's Lock refused stops the stage with the refusal.
    for (const step of steps) {
      if (step && step.ok !== false) continue
      const lock = zdLockRefusal(step && (step.error || step.output))
      if (lock) {
        log(`${label}: ${step.name} was refused by the owner's Lock; the stage stops and keeps its drafts and receipts.`)
        throw zdLockStop(`${label}: ${step.name}`, lock)
      }
    }
    // R322: the chain manager recorded its own ids against the unit that owns them (the claim step's unit,
    // the chain's given unit, or the run). Name the jobs whose records failed.
    const claimStep = (o.steps || []).find((s) => s.claim)
    const claimRun = claimStep ? steps.find((s) => s.name === claimStep.name) : null
    const claimed = claimRun ? ((zdLastJson(claimRun.output) || {}).units || []) : []
    const owner = claimed[0] && claimed[0].id ? { unit: claimed[0].id, token: claimed[0].token } : resourceOwner(o)
    const unrecorded = results.filter((r) => r.session_id && r.registered === false).map((r) => r.id)
    if (unrecorded.length) log(`${label}: ${unrecorded.length} job id(s) not recorded by their manager: ${unrecorded.join(', ')}`)
    return { ok: steps.every(s => s.ok), steps, results, deleted: 0, raw: driven, unit: owner === 'run' ? 'run' : owner.unit }
  }

  // --- The layer driver: script steps run by DeepSeek drive jobs (#175) ------------------------------------------
  // One layer's block is one deterministic chain (`scripts/book-digest/layer-steps.ps1`): `start` claims the
  // block and lists the DeepSeek jobs it printed, the block's content jobs run in Haiku managers, and `finish`
  // records every held unit's checked completion, faults the units that cannot finish, runs the layer's own
  // apply and report commands and the ledger gate. Each step is ONE script call run by a DeepSeek drive job
  // (zt-workflow-lib's shared ztDrive); the module relays no command and decides nothing per unit.
  const LAYER_STEPS = p.layerScript || `${SCRIPTS}\\layer-steps.ps1`
  // A layer claims its block with the longest lease the ledger takes: the block's content jobs are capped by
  // the provider run cap, and the explain layer's second content round runs after the first.
  const LAYER_LEASE = 7200
  // One step of the layer script, run by one DeepSeek drive job. A failed step (`ok:false`) is returned for
  // the module to stop with; the owner's typed refusal stops the stage here.
  const driveStep = async (label, step, o = {}) => {
    const parts = [`pwsh -NoProfile -File "${LAYER_STEPS}"`, `-Step ${step}`, `-Out "${OUT}"`, `-Layer ${o.layer}`]
    if (o.owner) parts.push(`-Owner "${o.owner}"`)
    if (o.take) parts.push(`-Take ${o.take}`)
    if (o.wait) parts.push(`-Wait ${o.wait}`)
    if (o.lease) parts.push(`-Lease ${o.lease}`)
    if (o.maxParts) parts.push(`-MaxParts ${o.maxParts}`)
    if (o.limits) parts.push(`-Limits ${psQuote(o.limits)}`)
    const json = await ztDrive(ctx, { label, command: parts.join(' '), out: OUT, cwd: o.cwd || OUT, phase: o.phase, ledger: ZD_LEDGER, deepseek: o.deepseek })
    const lock = ztLockRefusal(JSON.stringify(json)) || ((json.code === ZD_LOCK || json.error === ZD_LOCK) ? json : null)
    if (lock) throw ztLockStop(label, lock.refusal || lock)
    if (json.ok !== true) return { ok: false, error: String(json.error || `the ${step} step failed`).slice(0, 800), gate: zdGateOf(json), raw: json }
    return json
  }
  // The layer gate a step's flat counts describe (#180): the line carries `gate_complete`, `gate_open` and
  // `gate_total`, never the gate itself. The driver's gate shape stays `{complete, counts}` for the stage modules
  // and the report, which read only those.
  const zdGateOf = (json) => (json && (json.gate_complete !== undefined || json.gate_open !== undefined)
    ? { complete: json.gate_complete === true, counts: { open: Number(json.gate_open || 0), total: Number(json.gate_total || 0) } }
    : (json && json.gate ? json.gate : null))
  // The count fields a layer's steps answer; the driver sums them over the layer's blocks, so a stage reports
  // totals and reads no per-unit list (#180).
  const LAYER_TOTALS = [
    'done', 'failed', 'partial', 'applied', 'refused', 'blocked',
    'link_gaps', 'link_skipped', 'link_linked', 'link_linked_from', 'link_already_linked', 'link_no_link', 'link_changed', 'link_scheduled',
    'link_quality_faults', 'link_quality_checked',
    'mid_applied_created', 'mid_applied_merged', 'mid_applied_linked', 'mid_applied_refused', 'mid_report_models', 'mid_report_jobs',
    'terms',
  ]
  // The jobs one step listed, run by Haiku managers: one manager per up to DSH_PER_BATCH jobs, in parallel. Each
  // entry names its own files, prompt, unit and token; every manager records its conversations and deletes them
  // (or, when the layer continues them in a second round, leaves them to the run cleanup).
  const CONTENT = {
    type: 'object',
    properties: {
      ok: { type: 'boolean' }, deleted: { type: 'number' }, not_deleted: { type: 'array', items: { type: 'string' } },
      results: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] },
            answer: { type: 'string' }, registered: { type: 'boolean' }, evidence: { type: 'boolean' }, deleted: { type: 'boolean' },
          },
          required: ['id', 'status'],
        },
      },
    },
    required: ['ok', 'results'],
  }
  const contentPrompt = (label, o) => `You are a DeepSeek subagent manager for ZET ${o.what} (${label}). You start DeepSeek agents, wait for them, collect their answers, record each conversation and delete it. You never write content and you never run a content command yourself. Keep your messages short.
${RUNNER}
Load the DSH tools with one ToolSearch call: query "${DRIVE_TOOLS}".
JOBS: read the JSON array in "${o.file}" (Read tool) and run its entries ${o.from} through ${o.to - 1} (0-based), and only those. Each entry has id, files, prompt, unit and token.
- Start each job: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills || [])}, model "${DSH_MODEL}", reasoning_effort "${o.effort || DSH_EFFORT}", wait_seconds 0.${o.table === undefined ? '' : `
- A job with "continue": find the session of the job id it names in this table (job id, then session_id):
${o.table || '(empty)'}
  With a session: deepseek_prompt with that session_id, prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills || [])}, reasoning_effort "${o.effort || DSH_EFFORT}", wait_seconds 0. Without one: deepseek_run as above, with prompt_files = the job's fallback. When deepseek_prompt answers that the session is unknown or closed, run the job once as a new conversation: deepseek_run as above, with prompt_files = the job's fallback.`}
Keep at most ${DSH_PER_BATCH} jobs running at a time; start the next job when one ends. Start independent jobs in one message.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 600).
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: the Bridge compiler (${ZD_DIGEST} bridge, or bridge-compiler.ps1) with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}), the check zt-check (read-only), a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}, and writing JSON files under "${o.writes || o.dsh}"`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
${o.run ? 'The record commands below name the run unit; use them as written.' : "In the record commands below, <unit_id> and <token> are the entry's own unit and token."} Use the session_id that deepseek_run returned, never a word in its place. A step whose job list is empty starts no run: never send a placeholder prompt.
${o.run ? recordRules('run', 1, o.dsh, !o.keep) : recordRules('<unit_id>', '<token>', o.dsh, !o.keep)}
Return ok, deleted, not_deleted and results: one entry per job with id, session_id, status ("done" or "failed"), answer, registered, evidence and deleted.`
  // One manager's share of a step's jobs. A manager that answers no report leaves its jobs undelivered: the
  // finish step faults the units they belonged to.
  const runJobs = async (label, file, count, o) => {
    const managers = []
    for (let from = 0; from < Number(count || 0); from += DSH_PER_BATCH) {
      const to = Math.min(Number(count), from + DSH_PER_BATCH)
      const at = `${label} ${from + 1}-${to}`
      managers.push(dsmpSlot(() => zAgent(contentPrompt(at, { ...o, file, from, to }), { label: at, phase: o.phase || 'Digest', model: 'haiku', schema: CONTENT }))
        .then((r) => (r && r.results) || [])
        .catch((error) => { log(`${label}: a content manager answered no report (${String((error && error.message) || error).slice(0, 160)}); its jobs stay unfinished.`); return [] }))
    }
    return (await Promise.all(managers)).flat()
  }
  // The session table a second content round continues: the plan round's job ids and session ids.
  const sessionTable = (results) => (results || []).filter((r) => r && r.session_id).map((r) => `${r.id} ${r.session_id}`).join('\n')
  // Runs one layer in blocks until it offers no ready unit (at most `rounds` claims that found nothing). o:
  // layer, take, rounds, wait, dsh, writes, skills, effort, phase, what (the manager's subject), jobs (false:
  // the layer lists no content job), apply (a second content round between start and finish), maxParts, limits,
  // deepseek (the deletion tally), onStart/onApply/onFinish (a module's own reading of a step's flat counts).
  // #180: a step answers counts only, so the driver sums them over the layer's blocks and the stage reports
  // totals; the per-unit outcomes are the ledger's (its run report), never a module's array.
  const layerRun = async (name, o) => {
    const layer = o.layer || name
    const take = Math.max(1, Math.min(64, o.take || 8))
    const rounds = Math.max(1, Math.min(12, o.rounds || MAX_ATTEMPTS))
    const MAX_BLOCKS = 200
    const totals = Object.fromEntries(LAYER_TOTALS.map((k) => [k, 0]))
    const finishes = []
    let claimed = 0
    let idle = 0
    let gate = null
    const add = (line) => { for (const k of LAYER_TOTALS) if (line && line[k] !== undefined) totals[k] += Number(line[k]) || 0 }
    for (let block = 1; block <= MAX_BLOCKS && idle < rounds; block++) {
      const label = `${name} block ${block}`
      const started = await driveStep(`${label}: start`, 'start', {
        layer, take, owner: label, wait: o.wait, lease: LAYER_LEASE, phase: o.phase, cwd: o.dsh, maxParts: o.maxParts, limits: o.limits, deepseek: o.deepseek,
      })
      if (!started.ok) return { status: 'failed', layer, error: started.error, totals, claimed, gate: started.gate, finishes }
      add(started)
      gate = started.gate || gate
      if (o.onStart) o.onStart(started, label)
      const units = Number(started.units || 0)
      if (!units) {
        idle += 1
        if (!gate || gate.complete === true) break
        if (o.wait === undefined) break
        continue
      }
      idle = 0
      claimed += units
      // The Cards layer's work is the checked finish the start step already ran.
      if (o.jobs === false) continue
      let sessions = null
      if (Number(started.jobs || 0)) sessions = await runJobs(`${label}: write`, started.file, started.jobs, o)
      if (o.apply) {
        const applied = await driveStep(`${label}: apply`, 'apply', { layer, phase: o.phase, cwd: o.dsh, limits: o.limits })
        if (!applied.ok) return { status: 'failed', layer, error: applied.error, totals, claimed, gate, finishes }
        add(applied)
        if (o.onApply) o.onApply(applied, label, started)
        if (Number(applied.jobs || 0)) await runJobs(`${label}: apply`, applied.file, applied.jobs, { ...o, table: sessionTable(sessions) })
      }
      const finished = await driveStep(`${label}: finish`, 'finish', { layer, phase: o.phase, cwd: o.dsh, deepseek: o.deepseek })
      if (!finished.ok) return { status: 'failed', layer, error: finished.error, totals, claimed, gate: finished.gate || gate, finishes }
      finishes.push(finished)
      add(finished)
      gate = finished.gate || gate
      if (o.onFinish) o.onFinish(finished, label, started)
    }
    const final = gate || { complete: false, counts: {} }
    const open = Number((final.counts && final.counts.open) || 0)
    return {
      status: final.complete === true ? 'complete' : totals.failed && !open ? 'partial' : 'active',
      layer, gate: final, totals, claimed, finishes,
      verified: totals.done, failed: totals.failed, partial: totals.partial,
    }
  }
  // --- The book-level stages: one book-steps.ps1 step per block (#176) -----------------------------------------
  // The preparation, the whole-book card cycle, Organize, Figures, the Bridge, the audit, the finalization, the
  // Term-link round and the settlement run their deterministic chains in scripts/book-digest/book-steps.ps1: one
  // call is one step, run by a DeepSeek drive job (the shared ztDrive), and it prints one compact JSON line. The
  // module reads that line; it decides nothing per command and relays none.
  const BOOK_STEPS = p.bookScript || `${SCRIPTS}\\book-steps.ps1`
  const bookStep = async (label, step, options = {}, o = {}) => {
    const parts = [`pwsh -NoProfile -File "${BOOK_STEPS}"`, `-Step ${step}`, `-Out "${OUT}"`]
    if (options && Object.keys(options).length) parts.push(`-Json ${psQuote(JSON.stringify(options))}`)
    if (o.tool) parts.push(`-Tool ${o.tool}`)
    const json = await ztDrive(ctx, { label, step, command: parts.join(' '), out: OUT, cwd: o.cwd || OUT, phase: o.phase, ledger: ZD_LEDGER, deepseek: o.deepseek })
    // R326/R337: a step whose command the owner's Lock refused stops the stage with the archive's typed refusal.
    const lock = (json && (json.code === ZD_LOCK || json.error === ZD_LOCK)) ? json : ztLockRefusal(JSON.stringify(json || {}))
    if (lock) throw ztLockStop(label, lock)
    return json
  }
  // The card cycle of one scope (#176): one step plans and lists its DeepSeek jobs, the managers write them, one
  // step a round applies every ready draft and runs the check (listing the follow-up jobs), and one step applies
  // the last drafts and reports. `rounds` is the declared correction count (R299), never a retry loop; a step that
  // answers ok:false stops the cycle with its reason.
  const cardCycle = async (label, work, o = {}) => {
    const json = { Work: work, Book: o.book || KEY, Staging: OUT }
    const rules = o.rules ? o.rules : (A.cardRules || '')
    if (rules) json.Rules = rules
    const maxCards = o.maxCards === undefined ? A.cardMax : o.maxCards
    if (maxCards) json.MaxCards = maxCards
    if (o.sample !== undefined) json.Sample = o.sample
    if (o.modelReview) json.ModelReview = true
    if (o.notes) json.Notes = o.notes
    if (o.noOrphans) json.NoOrphans = true
    if (o.cards) json.Cards = o.cards
    const jobs = { dsh: work, writes: `${work}\\out`, phase: o.phase || 'Cards', skills: CARD_SKILLS, effort: DSH_EFFORT, what: 'card writers', run: true }
    const plan = await bookStep(`${label}: plan`, 'cards-start', json, jobs)
    if (plan.ok !== true) {
      log(`Cards ${label}: plan failed: ${(plan && plan.error) || 'no output'}.`)
      return { status: 'failed', label, error: `plan failed: ${(plan && plan.error) || 'no output'}` }
    }
    log(`Cards ${label}: ${plan.picked} Cards of the book; ${plan.with_faults} with ${plan.faults} faults, ${plan.review} for review, ${plan.orphans} orphans; ${plan.jobs} DeepSeek jobs.`)
    if (Number(plan.jobs || 0)) await runJobs(`${label}: write`, plan.file, plan.jobs, jobs)
    // R299: the finalization spends its one declared correction cycle, so it passes fixRounds 1 whatever the
    // run's cardFixRounds pilot setting is.
    const rounds = o.fixRounds === undefined ? CARD_FIX_ROUNDS : Math.max(0, Math.min(12, o.fixRounds))
    for (let round = 1; round <= rounds; round++) {
      const step = await bookStep(`${label}: check ${round}`, 'cards-apply', { Work: work }, jobs)
      if (step.ok !== true) return { status: 'failed', label, plan, error: `check ${round} failed: ${(step && step.error) || 'no output'}` }
      log(`Cards ${label}: ${step.faults} faults left, ${step.jobs} follow-up jobs.`)
      if (Number(step.jobs || 0)) await runJobs(`${label}: fix ${round}`, step.file, step.jobs, jobs)
    }
    const report = await bookStep(`${label}: report`, 'cards-finish', { Work: work }, jobs)
    if (report.ok !== true) return { status: 'failed', label, plan, error: `report failed: ${(report && report.error) || 'no output'}` }
    log(`Cards ${label}: faults ${report.faults_before} → ${report.faults_after} on ${report.cards} Cards; verdicts ${JSON.stringify(report.verdicts)}; ${report.splits} splits, ${report.merges} merge proposals, ${report.refused} refused; ${report.orphan_links} orphan Links (${report.orphans_after} orphans left).`)
    return { status: 'done', label, plan, applied: { applied_jobs: Number(report.applied) || 0 }, report }
  }
  const stepOf = (driven, name) => ((driven && driven.steps) || []).find(s => s.name === name) || null
  const jsonOf = (driven, name) => zdLastJson((stepOf(driven, name) || {}).output)
  // R271/R272: one compact JSON object on the last line, with the expected job count, the unit and the
  // input hash checked before a run trusts it. `jobs: []` is a success only with an explicit no-op reason.
  const resultLine = (driven, name, o = {}) => {
    const step = stepOf(driven, name)
    const parsed = zdLastJson(step && step.output)
    const faults = []
    if (!step) faults.push(`the chain has no step ${name}`)
    else if (!step.ok) faults.push(`${name} failed: ${step.error || 'nonzero exit'}`)
    else if (!parsed) faults.push(`${name} printed no JSON line`)
    if (parsed && o.unit && (o.unit !== parsed.unit) && (parsed.unit !== undefined)) faults.push(`${name} answered unit ${parsed.unit}, not ${o.unit}`)
    if (parsed && o.token !== undefined && parsed.token !== undefined && parsed.token !== o.token) faults.push(`${name} answered token ${parsed.token}, not ${o.token}`)
    if (parsed && o.inputHash && typeof parsed.input_hash === 'string' && parsed.input_hash !== o.inputHash) faults.push(`${name} answered input ${parsed.input_hash}, not ${o.inputHash}`)
    if (parsed && o.expectedJobs !== undefined) {
      const declared = parsed.expected_jobs
      const jobs = Array.isArray(parsed.jobs) ? parsed.jobs.length : Array.isArray(parsed.results) ? parsed.results.length : null
      if (declared === undefined) faults.push(`${name} did not declare expected_jobs`)
      else if (declared !== o.expectedJobs) faults.push(`${name} declared ${declared} expected jobs, not ${o.expectedJobs}`)
      else if (declared === 0 && jobs !== 0) faults.push(`${name} declared no jobs but printed ${jobs}`)
      if (declared === 0 && !parsed.no_op && !parsed.no_op_reason && !parsed.reason) faults.push(`${name} declared no jobs without a no-op reason`)
    }
    if (parsed && parsed.pending_jobs > 0) faults.push(`${name} left ${parsed.pending_jobs} jobs pending`)
    return { parsed, faults }
  }
  // The phase a ledger-only call is filed under: a declared stage title of the base blueprint.
  const LEDGER_PHASE = 'Digest'
  // The phase of the run-level cleanup: the base declares one `Cleanup` stage.
  const CLEANUP_PHASE = 'Cleanup'
  // The ledger adapter: every call is one bounded command in a manager's own conversation. The ledger owns
  // readiness, attempts and completion evidence; a module never reads a clock or a file to decide it (R270).
  const ledgerCommand = (args) => `${ZD_LEDGER} ${args}`
  // Claims ready units of one layer in a bounded block. `wait` is the tool-side readiness wait (seconds).
  // The claim requests a lease that covers the frozen provider-run cap (#92).
  // A claim's units are the ledger's word: when the relayed line lost its units array (grossman2002integration
  // trial 3: "(4 units claimed: cards/g005@1, ...)", and the driver faulted an undefined unit), the driver
  // reads the units this claim holds from the ledger by its owner label (`ledger status -Held`).
  // A claim is brief (`ledger next -Brief`): each unit keeps its id, token, input hash and lease, and of its
  // inputs only the Model address a model or explain unit names (`header`, `model`). On trial 4 a relay dropped
  // the full 1.3 KB inputs of four model units and the driver finished `-Model 'undefined'`; a model or explain
  // unit that arrives without its address is read from the ledger too.
  const ADDRESSED = /^(models|explain)$/
  const heldUnits = (units, layer = '') => Array.isArray(units) && units.every((u) => u && typeof u.id === 'string' && Number(u.token) > 0
    && (!ADDRESSED.test(layer) || (u.inputs && (typeof u.inputs.header === 'string' || typeof u.inputs.model === 'string'))))
  // The input hash of every unit this run claimed: a fault names the claim's hash (`ledger fail` refuses one
  // without it; it is a claim field, never part of the unit's inputs).
  const HASHES = new Map()
  const remember = (units) => { for (const u of units) if (u && u.id && u.input_hash) HASHES.set(u.id, u.input_hash); return units }
  const hashOf = (unit, inputs) => (inputs && inputs.input_hash) || HASHES.get(unit) || ''
  // A claim command that fails is tried again, three times in all, before the caller hears of it: on
  // ebrill2001modern trial 1 one `ledger next` refused while parallel lanes changed the archive, and the
  // events layer ended with 40 of its 73 units never claimed.
  const CLAIM_TRIES = 3
  const claim = async (label, layer, take, wait = 0) => {
    let out
    for (let attempt = 1; attempt <= CLAIM_TRIES; attempt++) {
      out = await runScript(attempt === 1 ? label : `${label} (try ${attempt})`, LEDGER_PHASE, ledgerCommand(`next -Out "${OUT}" -Layer ${layer} -Take ${take} -Owner "${label}"${wait ? ` -Wait ${wait}` : ''} -Lease ${CLAIM_LEASE} -Brief`))
      if (out.json || (out.run && out.run.ok)) break
      log(`${label}: the claim failed (${String((out.run && out.run.output) || 'no output').slice(0, 200)})${attempt < CLAIM_TRIES ? '; it is tried again' : ''}.`)
    }
    const parsed = out.json
    // A line without a `units` array is unreadable, never an empty claim: a relay replaced it with its own
    // `units_claimed` id list (grossman2002integration trial 17), and the layer ended with four models held.
    if (parsed && Array.isArray(parsed.units) && heldUnits(parsed.units, layer)) return { ok: true, units: remember(parsed.units), layer: parsed.layer, settled: parsed.settled, complete: parsed.complete, counts: parsed.counts }
    if (!parsed && !(out.run && out.run.ok)) return { ok: false, error: (out.run && out.run.output) || 'no output from ledger next', units: [] }
    log(`${label}: the claim line came back without its units; the held units are read from the ledger.`)
    const held = await runScript(`${label}: held units`, LEDGER_PHASE, ledgerCommand(`status -Out "${OUT}" -Held "${label}"`))
    const units = held.json && heldUnits(held.json.units, layer) ? remember(held.json.units) : null
    if (!units) return { ok: false, error: `the claim's held units could not be read: ${(held.run && held.run.output) || 'no output'}`, units: [] }
    return { ok: true, units, layer, settled: parsed ? parsed.settled : undefined, complete: parsed ? parsed.complete : undefined, counts: parsed ? parsed.counts : undefined }
  }
  const gate = async (label, layer) => {
    const out = await runScript(label, LEDGER_PHASE, ledgerCommand(`gate -Out "${OUT}" -Layer ${layer} -Brief`), ' (exit 3 means the layer still has open work)', { always: true })
    return out.json || { ok: false, error: (out.run && out.run.output) || 'no output from ledger gate' }
  }
  // In mode resume a stage whose ledger layers are already complete reuses the retained work instead of redoing it
  // (grossman2002integration trial 20: the resume reran Organize's survey and decisions, which can merge or move Cards
  // the retained run had finished). Outside mode resume it never skips.
  const settledOnResume = async (label, layers) => {
    if (MODE !== 'resume') return false
    for (const name of layers) {
      const g = await bookStep(`${label}: ${name} gate`, 'gate', { Layer: name })
      if (!(g && g.complete === true)) return false
    }
    log(`${label}: the retained run completed its ${layers.join(', ')} work; the resume reuses it.`)
    return true
  }
  // S5: one fault of a claimed unit, classified by the ledger (fault_class of its code; a fault without a code takes
  // the ledger's one reading of its cause). Its answer is the unit's state after the fault: 'failed' when the unit
  // stays failed (a content fault, or its attempts are spent), else 'retry'.
  const failUnit = async (label, unit, token, fault) => {
    const out = await runScript(label, LEDGER_PHASE, `${ZD_LEDGER} fail -Out "${OUT}" -Unit "${unit}" -Token ${token} -Fault ${psQuote(JSON.stringify(fault))}`)
    return out.json && out.json.state === 'failed' ? 'failed' : out.json ? 'retry' : (fault.code === 'check_failed' || fault.code === ZD_LOCK ? 'failed' : 'retry')
  }
  // A PowerShell single-quoted literal (the tool's -Fault and -Json values are JSON text).
  function psQuote(text) {
    return `'${String(text).replace(/'/g, "''")}'`
  }
  // The family finish command of one claimed unit: the checked verifier that records the completion.
  const finishOf = async (label, unit, token, extra = {}, o = {}) => {
    const number = /\/g(\d+)@1$/.exec(unit)
    const model = o.model || (o.inputs && (o.inputs.model || o.inputs.header))
    let command = ''
    if (/^digest\//.test(unit)) command = `${ZD_DIGEST} group-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token} -Draft ${psQuote(o.draft || '')}`
    else if (/^cards\//.test(unit)) command = `${ZD_DIGEST} cards-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token}`
    else if (/^events\//.test(unit)) command = `${ZD_DIGEST} events-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token}`
    else if (/^findings\//.test(unit)) command = `${ZD_DIGEST} findings-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token}`
    else if (/^link\//.test(unit)) command = `${ZD_DIGEST} link-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token} -Draft ${psQuote(o.draft || '')}`
    else if (/^models\//.test(unit)) command = `${ZD_DIGEST} model-finish -Out "${OUT}" -Model ${psQuote(model)} -Token ${token}`
    else if (/^explain\//.test(unit)) command = `${ZD_DIGEST} explain-finish -Out "${OUT}" -Model ${psQuote(model)} -Token ${token}`
    else if (/^cards-book\//.test(unit)) command = `${ZD_DIGEST} book-cards-finish -Out "${OUT}" -Work ${psQuote(extra.work || `${OUT}\\cards`)} -Unit "${unit}" -Token ${token}`
    else if (/^terms\//.test(unit)) command = `${ZD_LEDGER} done -Out "${OUT}" -Unit "${unit}" -Token ${token} -Outputs ${psQuote(`${OUT}\\terms\\completion.json`)}`
    else if (/^cleanup\//.test(unit)) command = `${ZD_DIGEST} cleanup-finish -Out "${OUT}" -Unit "${unit}" -Token ${token}`
    else return { ok: false, error: `no checked finish command for ${unit}` }
    if (extra.checkOnly) command += ' -CheckOnly'
    const out = await runScript(label, o.phase || LEDGER_PHASE, command, ' (a checked completion; never retry it with other options)')
    return { ok: !!(out.run && out.run.ok), json: out.json, command, error: out.run && out.run.output }
  }
  // One claimed unit's checked finish. A failure is recorded against that unit's own attempt
  // (`ledger fail`): a transient transport or lease fault schedules the retry, a checked content
  // failure keeps the unit failed and visible. Never a whole-stage rerun (R258).
  const finishOrFault = async (label, unit, token, inputs, extra = {}) => {
    const at = `${label}: ${unit}`
    let got
    try {
      got = await finishOf(at, unit, token, { ...extra, inputs }, { inputs })
    } catch (error) {
      // R326/R337: a checked finish the archive refused because the owner locked the destination keeps
      // the archive's typed refusal as the unit's content fault, then stops the stage.
      if (error && error.lock_refusal) {
        await failUnit(`${at} locked`, unit, token, {
          schema_version: 1, unit, input_hash: hashOf(unit, inputs), code: ZD_LOCK,
          cause: `${zdLockSaid(error.lock_refusal)} ${JSON.stringify(error.lock_refusal)}`.slice(0, 800),
        })
        throw error
      }
      throw error
    }
    const done = got.ok && got.json && (got.json.state === 'done' || got.json.complete === true)
    if (done) return { unit, status: got.json.status === 'partial' ? 'partial' : 'done', result: got.json }
    // The ledger decides every finish that is not plainly done before any fault: on trial 5 a relay cut the end
    // off a 2.5 KB cards completion that the ledger had recorded done, and on trial 10 a relay rewrote the end of
    // one as prose, so the line still held a nested object (a file record, no state) and the driver faulted a
    // done unit; the ledger refused that fault as stale.
    let ledgerHash = ''
    if (!done) {
      const read = await runScript(`${at}: state`, LEDGER_PHASE, ledgerCommand(`status -Out "${OUT}" -Units ${unit}`), ' (it reads the unit\'s state; it changes nothing)')
      const held = read.json && Array.isArray(read.json.units) ? read.json.units.find((u) => u && u.id === unit) : null
      ledgerHash = (held && held.input_hash) || ''
      if (held && (held.state === 'done' || held.state === 'noop')) {
        log(`${at}: the finish line came back cut; the ledger holds the unit ${held.state}.`)
        return { unit, status: 'done', result: { unit, state: held.state, disposition: held.disposition || null, from: 'ledger' } }
      }
    }
    const cause = String((got.error && got.error.message) || got.error || (got.json && (got.json.fault || got.json.error)) || 'the checked completion did not complete').slice(0, 800)
    // S5: the ledger reads the cause (cause_code, fault_class); the claim's hash or the ledger's own (trial 19: a
    // cards-book fault went out with an empty hash and was refused).
    const fault = { schema_version: 1, unit, input_hash: hashOf(unit, inputs) || ledgerHash, cause }
    const status = await failUnit(`${at} fault`, unit, token, fault)
    return { unit, status, error: cause, command: got.command }
  }
  // --- A block's checked finishes in one command (35; docs/digest-claude-token-proposal.md P1/P2/P4) ----
  // `zt-digest finish-batch` runs each claimed unit's own verifier in turn in one process, records a unit that
  // cannot finish against that unit alone (`ledger fail`, the classes finishOrFault gives), and prints one
  // short line: counts and one entry per unit (unit, token, status, disposition or cause). On
  // ebrill2001modern trial 1 one relay per unit finish was 311 agents and the largest relay group.
  // The ledger stays the judge: a batch line no relay returned whole is run again for the units it does not
  // answer (every finish is idempotent and fenced by its token), and a unit still unanswered then is finished
  // on its own by finishOrFault, which asks the ledger before any fault.
  const BATCH_MAX = ZD_PER_MANAGER_MAX
  const FINISHED = ['done', 'partial', 'retry', 'failed']
  const finishBatchCommand = (units) => `${ZD_DIGEST} finish-batch -Out "${OUT}" -Units "${units.map((u) => `${u.id}:${u.token}`).join(',')}"`
  // The per-unit outcomes one finish_batch line answers, by unit id (only the units asked for).
  const batchOutcomes = (line, units) => {
    const got = new Map()
    if (!line || line.kind !== 'finish_batch' || !Array.isArray(line.results)) return got
    for (const r of line.results) {
      if (r && FINISHED.includes(r.status) && units.some((u) => u.id === r.unit)) got.set(r.unit, r)
    }
    return got
  }
  const asDisposition = (r) => (r.status === 'done' || r.status === 'partial'
    ? { unit: r.unit, status: r.status, result: { unit: r.unit, state: 'done', disposition: r.disposition || null, ...(r.from ? { from: r.from } : {}) } }
    : { unit: r.unit, status: r.status, error: r.cause || r.code || r.status })
  // The checked finishes of claimed units. `given` is the line of a finish step a chain manager already ran
  // (the events, findings and explain chains end with it); without one, the batch runs as one relay.
  const finishUnits = async (label, units, given) => {
    const list = (units || []).filter((u) => u && typeof u.id === 'string' && Number(u.token) > 0)
    if (!list.length) return []
    const settled = batchOutcomes(given, list)
    const open = () => list.filter((u) => !settled.has(u.id))
    if (given !== undefined && open().length) log(`${label}: the chain's finish line answers ${settled.size} of ${list.length} unit(s); the rest are finished in one command.`)
    for (let i = 0; open().length && i < list.length; i += BATCH_MAX) {
      const share = open().slice(0, BATCH_MAX)
      const out = await runScript(`${label}: finish`, LEDGER_PHASE, finishBatchCommand(share), ' (the checked completions of these claims, one after another; never retry it with other options)')
      const got = batchOutcomes(out.json, share)
      for (const [id, r] of got) settled.set(id, r)
      if (!got.size) break
    }
    const out = []
    for (const u of list) {
      if (settled.has(u.id)) { out.push(asDisposition(settled.get(u.id))); continue }
      log(`${label}: ${u.id} has no readable batch outcome; it is finished on its own.`)
      out.push(await finishOrFault(label, u.id, u.token, u.inputs))
    }
    return out
  }
  // The Cards layer's claim and checked finishes in one command (P2): its work is the finish alone, so
  // `finish-batch -Layer cards -Take -Owner` claims the ready units and finishes each in the same process.
  // A line no relay returned whole is never read as an empty claim: the units this owner still holds come
  // from the ledger (`ledger status -Held`) and are finished as above.
  const claimFinish = async (label, layerName, take, wait = 0) => {
    let out
    for (let attempt = 1; attempt <= CLAIM_TRIES; attempt++) {
      out = await runScript(attempt === 1 ? label : `${label} (try ${attempt})`, LEDGER_PHASE,
        `${ZD_DIGEST} finish-batch -Out "${OUT}" -Layer ${layerName} -Take ${Math.min(BATCH_MAX, take)} -Owner "${label}"${wait ? ` -Wait ${wait}` : ''} -Lease ${CLAIM_LEASE}`,
        ' (it claims the ready units and records each one\'s checked completion; never retry it with other options)')
      const line = out.json
      if (line && line.kind === 'finish_batch' && Array.isArray(line.results)) {
        const answered = line.results.filter((r) => r && FINISHED.includes(r.status))
        // A unit the time budget left unstarted is still claimed under this owner, with its token in the line.
        const left = line.results.filter((r) => r && r.status === 'not_run' && Number(r.token) > 0).map((r) => ({ id: r.unit, token: Number(r.token) }))
        const dispositions = [...answered.map(asDisposition), ...(left.length ? await finishUnits(label, left) : [])]
        return { ok: true, claimed: Number(line.claimed) || line.results.length, empty: !line.results.length, dispositions, gate: line }
      }
      const held = await runScript(`${label}: held units`, LEDGER_PHASE, ledgerCommand(`status -Out "${OUT}" -Held "${label}"`))
      const units = held.json && heldUnits(held.json.units, layerName) ? remember(held.json.units) : null
      if (units && units.length) {
        log(`${label}: the claim-and-finish line came back unreadable; the ${units.length} unit(s) it still holds are finished from the ledger.`)
        return { ok: true, claimed: units.length, empty: false, dispositions: await finishUnits(label, units) }
      }
      // Nothing held: the command finished every unit it claimed, claimed none, or failed before its claim.
      if (out.run && out.run.ok) return { ok: true, claimed: 0, empty: false, dispositions: [] }
      log(`${label}: the claim failed (${String((out.run && out.run.output) || 'no output').slice(0, 200)})${attempt < CLAIM_TRIES ? '; it is tried again' : ''}.`)
    }
    return { ok: false, error: (out && out.run && out.run.output) || 'no output from finish-batch', dispositions: [] }
  }
  // --- Owned DeepSeek resources (R322–R330) -----------------------------------------------------------
  // Every created conversation is recorded the moment the provider returns its id, by the manager that
  // started it (recordRules), against the work that created it: the unit it holds, or the run-level cleanup
  // unit for a stage-wide chain. The run-level cleanup deletes exactly the recorded ids; nothing is deleted
  // by title.
  const resourceOwner = (o = {}) => {
    if (o.unit) return { unit: o.unit, token: Number(o.token) || 0 }
    return 'run'
  }
  // R331: the run report. It reads the ledger and verifies the current placeholder state, writes
  // run-report.json and run-report.md, and prints the same facts ledger status reports.
  const ledgerReport = async (label) => {
    const out = await runScript(label, CLEANUP_PHASE, ledgerCommand(`report -Out "${OUT}"`), ' (it writes run-report.json and run-report.md)')
    if (!out.json) return { ok: false, error: String((out.run && out.run.output) || 'no output from ledger report') }
    return { ok: true, json: out.json }
  }
  // Keeps a block's claims current while its work runs: `ledger hold` renews every listed claim each minute for
  // up to nine minutes and returns as soon as none is still held (finished, failed or reclaimed), and the next
  // hold starts while the work goes on. The model compiler's chains outlive a claim's lease and renew nothing
  // themselves (grossman2002integration trial 5: four model claims expired mid-build, so their finishes would
  // have been refused and the built models claimed again).
  const holding = async (label, units, work) => {
    const list = units.filter((u) => u && typeof u.id === 'string' && Number(u.token) > 0).map((u) => `${u.id}:${u.token}`).join(',')
    if (!list) return work()
    let finished = false
    let failures = 0
    const beat = (async () => {
      for (let n = 1; !finished && n <= 60; n++) {
        const out = await runScript(`${label}: hold ${n}`, LEDGER_PHASE, ledgerCommand(`hold -Out "${OUT}" -Units "${list}" -Seconds 540 -Every 60`), ' (it keeps the claims current for up to nine minutes and changes no Card; it ends early once no claim is held)')
        const held = out.json && Array.isArray(out.json.held) ? out.json.held : null
        if (held && !held.length) return
        if (!held) {
          failures += 1
          log(`${label}: hold ${n} gave no readable line (${String((out.run && out.run.output) || 'no output').slice(0, 160)}).`)
          if (failures >= 3) return
        } else failures = 0
      }
    })()
    try {
      return await work()
    } finally {
      finished = true
      await beat
    }
  }
  // The driver of one layer (R252-R257): claim ready units in bounded blocks, work each unit's own chain,
  // record every result, and settle with the layer gate. `wait` is the tool-side readiness wait in seconds;
  // `producer` names the layer whose settled enumeration means no further unit can appear. Returns
  // complete/partial/active only after the gate says so.
  const layer = async (name, o) => {
    const layerName = o.layer || name
    const take = Math.max(1, Math.min(64, o.take || 1))
    // `rounds` bounds the consecutive claims that found nothing ready, never the claims that found work: on
    // grossman2002integration trial 6 the events layer spent its three rounds on three productive claims while
    // one digest group was still retrying, and gave up before that group's Event unit appeared.
    const rounds = Math.max(1, Math.min(12, o.rounds || MAX_ATTEMPTS))
    const MAX_CLAIMS = 200
    const dispositions = []
    let claimedCount = 0
    let gateResult = null
    let idle = 0
    let blind = 0
    for (let round = 1; round <= MAX_CLAIMS && idle < rounds; round++) {
      // o.claimFinish (35, the Cards layer): one command claims the ready units and records their checked
      // finishes. A round whose line was unreadable while the owner holds nothing is "blind": it may have
      // finished units or claimed none, so the layer claims again, at most CLAIM_TRIES times in a row.
      if (o.claimFinish) {
        const done = await claimFinish(`${name} claim ${round}`, layerName, take, o.wait === undefined ? 0 : o.wait)
        if (!done.ok) return { status: 'failed', layer: layerName, error: done.error, dispositions }
        for (const r of done.dispositions) dispositions.push(r)
        blind = !done.empty && !done.claimed ? blind + 1 : 0
        if (!done.empty && (done.claimed || blind < CLAIM_TRIES)) {
          idle = 0
          claimedCount += done.claimed
          continue
        }
      } else {
        const got = await claim(`${name} claim ${round}`, layerName, take, o.wait === undefined ? 0 : o.wait)
        if (!got.ok) return { status: 'failed', layer: layerName, error: got.error, dispositions }
        const units = got.units || []
        if (units.length) {
          idle = 0
          claimedCount += units.length
          const results = o.hold ? await holding(`${name} claim ${round}`, units, () => o.work(units)) : await o.work(units)
          for (const r of results) dispositions.push(r)
          continue
        }
      }
      idle += 1
      // Nothing ready: settle only on the gate's own facts, never on an empty queue.
      gateResult = await gate(`${name} gate ${round}`, layerName)
      if (gateResult.complete === true) return { status: 'complete', layer: layerName, gate: gateResult, dispositions, claimed: claimedCount }
      const open = (gateResult.counts && (gateResult.counts.open || 0)) || 0
      if (!o.producer && open === 0) {
        // A settled layer keeps an explicit partial result when a unit failed; an unsettled one is active.
        const failed = dispositions.filter(d => d.status === 'failed').length
        return { status: failed ? 'partial' : 'active', layer: layerName, gate: gateResult, dispositions, claimed: claimedCount }
      }
      if (o.producer) {
        const produced = await gate(`${name} producer ${round}`, o.producer)
        if (produced.settled === true) break
      }
      if (o.wait === undefined) break
    }
    gateResult = await gate(`${name} gate`, layerName)
    const failed = dispositions.filter(d => d.status === 'failed').length
    const open = (gateResult.counts && (gateResult.counts.open || 0)) || 0
    return {
      status: gateResult.complete === true ? 'complete' : failed && !open ? 'partial' : 'active',
      layer: layerName, gate: gateResult, dispositions, claimed: claimedCount,
    }
  }
  // --- Cards: the card layer cycle -----------------------------------------------------------------
  // One cycle over one scope, run as ONE manager's chain: plan → the jobs it prints → apply in shares
  // until none is pending → check → its follow-up jobs → apply → report (R250/R251). The bounded
  // correction path is declared here, not improvised. o.modelReview: this whole-book cycle also reviews
  // the Model drafts no model chain owns (R247); a draft an active chain owns reuses that chain's review.
  const runCards = async (label, work, extra = '', only = '', o = {}) => {
    const cstep = (s, more = '') => `${CARD_COMPILER} -Step ${s} -Work "${work}" -Compact${more}`
    const rules = only ? ` -Rules "${only}"` : A.cardRules ? ` -Rules "${A.cardRules}"` : ''
    const max = A.cardMax ? ` -MaxCards ${A.cardMax}` : ''
    const models = o.modelReview ? ' -ModelReview' : ''
    const applyStep = (n) => ({ name: `apply ${n}`, command: cstep('apply', ` -MaxJobs ${APPLY_SHARE}`), repeat: { path: 'pending_jobs', value: 0, max: 100 } })
    // R299: the finalization spends its one declared correction cycle, so it passes fixRounds 1
    // whatever the run's cardFixRounds pilot setting is.
    const rounds = o.fixRounds === undefined ? CARD_FIX_ROUNDS : Math.max(0, Math.min(12, o.fixRounds))
    const steps = [{ name: 'plan', command: cstep('plan', ` -Book ${KEY} -Staging "${OUT}"${rules}${max}${models}${extra}`), jobs: true }, applyStep(1)]
    for (let round = 1; round <= rounds; round++) {
      steps.push({ name: `check ${round}`, command: cstep('check'), jobs: true })
      steps.push(applyStep(round + 1))
    }
    steps.push({ name: 'report', command: cstep('report'), optional: true })
    const chained = await chain(`cards ${label}`, {
      dsh: work, skills: CARD_SKILLS, effort: DSH_EFFORT, phase: 'Cards', writes: `${work}\\out`, steps, cleanup: true, keep: true,
    })
    const rawPlan = jsonOf(chained, 'plan')
    const applied = steps.filter(s => /^apply /.test(s.name)).map(s => jsonOf(chained, s.name)).filter(Boolean).pop() || null
    const reported = jsonOf(chained, 'report')
    const jobs = (chained.results || []).length
    // The plan line carries the job prompts and the sample and is long; its chain reads the jobs from the
    // file the step saved. A plan line the relay rewrote is no failure when the cycle reported: on
    // grossman2002integration trial 10 the relay abridged the whole-book plan inside its jobs array, the cycle
    // ran and reported 0 faults after, and the stage faulted the claim as "plan failed: no output".
    const plan = rawPlan ? { ...rawPlan, jobs: expandJobs(rawPlan) }
      : reported ? { picked: reported.picked, with_faults: '?', faults: reported.faults_before, review: '?', orphans: '?', jobs: [], from: 'report' } : null
    if (!rawPlan && plan) log(`cards ${label}: the plan line came back unreadable; the report line carries the cycle.`)
    if (!plan) {
      log(`cards ${label}: plan failed: ${((stepOf(chained, 'plan') || {}).error) || 'no output'}.`)
      return { status: 'failed', label, error: `plan failed: ${((stepOf(chained, 'plan') || {}).error) || 'no output'}` }
    }
    log(`Cards ${label}: ${plan.picked} Cards of the book; ${plan.with_faults} with ${plan.faults} faults, ${plan.review} for review, ${plan.orphans} orphans; ${plan.jobs.length} DeepSeek jobs.`)
    const checked = steps.filter(s => /^check /.test(s.name)).map(s => jsonOf(chained, s.name)).filter(Boolean)
    for (const c of checked) log(`Cards ${label}: ${c.faults} faults left, ${(c.jobs || []).length} follow-up jobs.`)
    if (reported) log(`Cards ${label}: faults ${reported.faults_before} → ${reported.faults_after} on ${reported.cards} Cards; verdicts ${JSON.stringify(reported.verdicts)}; ${reported.splits} splits, ${reported.merges} merge proposals, ${reported.refused} refused; ${reported.orphan_links} orphan Links (${reported.orphans_after} orphans left); ${jobs} DeepSeek jobs.`)
    const applyFailed = steps.filter(s => /^apply /.test(s.name)).some(s => jsonOf(chained, s.name) === null)
    if (applyFailed) log(`Cards ${label}: an apply step failed; the cycle is marked failed.`)
    return { status: reported && !applyFailed ? 'done' : 'failed', label, plan, applied, report: reported, ...(applyFailed ? { error: 'apply failed: no output from the card compiler -Step apply' } : {}) }
  }
  return {
    A, MODE, KEY, OUT, SCRIPTS, DSH_MODEL, DSH_EFFORT, MODELS, IDEA_SAMPLE, LIMITS, COMPILER, FILL_SKILLS,
    PER_MANAGER, raised,
    MAX_ATTEMPTS, DSH_RUN_CAP, CLAIM_LEASE, RUNNER, COMMAND_TIMEOUT_MS: ZD_COMMAND_TIMEOUT_MS,
    // The command prefixes and skills a stage module builds a chain with.
    LEDGER: ZD_LEDGER, DIGEST: ZD_DIGEST, CALL: ZD_CALL, DSH_SKILLS,
    zAgent, runScript, groupsOf, toBlocks, drive, expandJobs, answerLines,
    // The layer driver (#175): one script step per block, each run by a DeepSeek drive job, and its managers.
    layerRun, driveStep, runJobs, LAYER_STEPS, LAYER_LEASE,
    // The book-level stages (#176): one book-steps.ps1 step per block, and the card cycle's declared rounds.
    bookStep, cardCycle, BOOK_STEPS,
    // The ready-work driver (9.3): one manager per declared chain, plus the ledger adapter.
    chain, ANSWERS, stepOf, jsonOf, resultLine, claim, gate, settledOnResume, holding, failUnit, finishOf, finishOrFault, layer, psQuote,
    // 35: a block's checked finishes in one command, and its outcomes.
    finishBatchCommand, finishUnits, batchOutcomes,
    // The command that runs a digest script: zt-digest <tool> once ported, else pwsh with the script.
    tool: name => zdTool(name, SCRIPTS),
    // Tree Lock (#140): the owner's typed refusal, its sentence and the stop a stage throws with it.
    LOCK: ZD_LOCK, lockRefusal: zdLockRefusal, lockSaid: zdLockSaid, lockStop: zdLockStop,
    // Owned provider resources (R322-R330): the owner of each recorded id, the cleanup phase, the run report.
    resourceOwner, CLEANUP_PHASE, ledgerReport,
    runCards, profile, ARCH_DOC, ARCH_SUPPLEMENT, MODEL_WORK, PER_SONNET, PER_CONCEPT, DSH_PER_BATCH, REBUILD, LEVEL,
    BRIEF_EFFORT, DERIVE_EFFORT, REVIEW_EFFORT, REVIEW_ROUNDS, COMPILER_DOCS, SKIP_FORMS,
  }
}

defineModule({
  id: 'zt-digest-lib', version: '44', library: true, requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Library: the ZT digest machinery (settings, managers, the drive-step layer driver, the book-level stage steps and their card cycle, the model compiler) for the zt-* stage modules, and the Tree Lock seam that stops an affected write stage on the owner\'s typed card_locked refusal.',
})
// ==== end module zt-digest-lib ====

// ==== module zt-audit v6 ====
// The audit: digest-audit, read-only, on one book (args.citationKey) or on every book of the agent Workspace.
// It changes no Card; it writes audit.json, audit.md, and audit-faults.txt in args.out.
// 4 (S26, #169): the audit publishes the per-book and Workspace Term quality beside its own completion, so the
// stage log and the run report show Term coverage debt without reading the whole audit report.
// 5 (#176): the audit's one command is a book-steps.ps1 step (`audit`) run by a DeepSeek drive job.
// 6 (#180): the step answers the audit's counts (the per-book rows stay in its detail file), so the stage logs one
// truthful summary line and returns the same counts.
defineModule({
  id: 'zt-audit', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the read-only audit of the book\'s Cards (one book-steps.ps1 step), with its Term coverage and quality.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('audit', ['audit'])) return { status: 'done', reused: true }
    const summary = await Z.bookStep('audit cards', 'audit', Z.KEY ? { CitationKey: Z.KEY } : {})
    if (!summary || summary.ok !== true) {
      const error = (summary && summary.error) || 'no output'
      log(`Audit failed: ${error}`)
      return { status: 'failed', error }
    }
    const n = (v) => Number(v || 0)
    log(`Audit: ${n(summary.books)} book(s), ${n(summary.cards)} Cards; ${n(summary.style_faults)} Card language faults; ${n(summary.layer_faults)} Card layer faults; ${n(summary.orphans)} orphans; ${n(summary.no_source)} with no source; ${n(summary.unkinded)} with no kinded Link; ${n(summary.not_self_sufficient)} of ${n(summary.models)} models not self-sufficient; ${n(summary.explained)} of ${n(summary.derivations)} Derivation parts with 思路 and 解读; ${n(summary.figures_missing)} figures named but not shown. Terms ${summary.terms || 'unknown'} (${n(summary.term_published)}/${n(summary.term_rows)} rows, ${n(summary.term_uses)} uses, ${n(summary.term_faults)} T faults).`)
    log(`Audit: ${n(summary.hubs_missing)} missing hubs, ${n(summary.broken_links)} broken Links in the Workspace. Report: ${Z.OUT}\\audit.md`)
    return { status: 'done', ...summary, ok: undefined }
  },
})
// ==== end module zt-audit ====

// ==== module zt-book-terms v6 ====
// 4 (S5): a fault names its code and leaves the class to the ledger.
// 5 (#175): the chain is two script steps run by DeepSeek drive jobs under Haiku managers.
// 6 (#180): the finish step answers the table's counts (`terms`, `card`) instead of a nested report.
// The book term table, as ready ledger work: the `terms/book@1` unit that every digest group waits for. The
// layer driver runs the stage as script steps: `start` claims the one ready terms unit and runs the candidates
// plan (book-terms finds the bold terms, definition sentences, bracketed original-language terms and frequent
// word groups), the one DeepSeek job that chooses the terms and the words not to use runs in a Haiku manager, and
// `finish` applies the table: the script writes terms.md and the Card 术语表 before any digest run, through
// archive receipts, and completes the unit itself with its checked evidence. A failure is recorded against the
// unit, so the groups that depend on it stay waiting instead of digesting without the table.
defineModule({
  id: 'zt-book-terms', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the claimed Term table (terms/book@1): plan → DeepSeek → apply, completing the unit, in two script steps run by DeepSeek drive jobs.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const termWork = `${Z.OUT}\\terms`
    const result = await Z.layerRun('terms', {
      layer: 'terms', take: 1, rounds: 1, phase: 'Prepare',
      dsh: termWork, writes: `${termWork}\\out`, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, what: 'the Term table writer',
    })
    if (result.status === 'failed') return { status: 'failed', error: result.error, report: null }
    const finish = result.finish || {}
    const report = { terms: Number(finish.terms || 0), with_avoid: Number(finish.with_avoid || 0), card: finish.card || null }
    if (!result.claimed) {
      log('Term table: no ready Terms unit (a disabled layer or a completed table); the digest goes on.')
      return { status: 'skipped', report: null, reason: 'no ready terms unit' }
    }
    if (!Number(result.verified || 0)) {
      const cause = String(result.error || 'the Term table steps gave no output').slice(0, 800)
      log(`Term table failed: ${cause}; the digest groups stay waiting on it.`)
      return { status: 'failed', report, error: cause }
    }
    log(`Term table: ${report.terms} terms (${report.with_avoid} with words not to use) in ${report.card}.`)
    return { status: 'done', report }
  },
})
// ==== end module zt-book-terms ====

// ==== module zt-bridge v10 ====
// Bounded cross-book discovery publishes a report and applies its new-carrier proposals; an identified selection
// (earlier carriers too) applies through guarded script edits in mode bridge.
// v9: a discovery applies every new-carrier proposal of its report itself (no user review); earlier carriers stay explicit.
// v8 (S8): the no-op cleanupDsh call leaves; the run-level cleanup settles the recorded conversations.
// v10 (#176, docs/workflow-module-structure-proposal.md): the plan, the report, the apply and the publication are
// book-steps.ps1 steps run by DeepSeek drive jobs; the judge jobs run in the layer driver's read-only content
// managers. No Claude agent runs a command.
defineModule({
  id: 'zt-bridge', version: '10', requires: ['zt-digest-lib'],
  summary: 'Bounded cross-book discovery publishes a report and applies its new-carrier proposals; an identified selection (earlier carriers too) applies through guarded script edits in mode bridge.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'run arguments' },
    mode: { type: 'string', required: true, description: 'run mode' },
    model: { type: 'any', description: 'resolved judge model; unused for apply' },
    profile: { type: 'string', required: true, description: 'book or math' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p), A = Z.A, work = `${Z.OUT}\\bridge`
    if (await Z.settledOnResume('bridge', ['bridge'])) return { status: 'done', reused: true }
    for (const [key, low, high] of [['bridgeMax', 1, 120], ['bridgeCandidates', 1, 8]]) if (A[key] !== undefined && (!Number.isInteger(A[key]) || A[key] < low || A[key] > high)) return { status: 'failed', error: `${key} must be an integer in ${low}..${high}.` }
    if ((A.bridgeBooks !== undefined && typeof A.bridgeBooks !== 'string') || (A.bridgeReport !== undefined && typeof A.bridgeReport !== 'string')) return { status: 'failed', error: 'bridgeBooks and bridgeReport must be strings.' }
    const q = v => `'${String(v).replace(/'/g, "''")}'`
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts by
    // hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the revision
    // stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const json = await Z.bookStep(`${layer}: publish`, 'publish', { Layer: layer, ...more })
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(json && json.error) || 'no output'}`)
      return json || { layer, state: 'failed', error: 'no output' }
    }
    const selection = v => {
      if (v === undefined) return ''
      if (v === 'all') return 'all'
      if (!Array.isArray(v) || v.some(id => typeof id !== 'string' || !/^b[0-9]{3}$/.test(id)) || new Set(v).size !== v.length) throw Error('Bridge selection must be all or a unique array of proposal ids.')
      return v.join(',')
    }
    let accept, earlier
    try { accept = selection(A.bridgeApply); earlier = selection(A.bridgeApplyEarlier) } catch (e) { return { status: 'failed', error: String(e) } }
    const applying = !!(A.bridgeReport || accept || earlier)
    if (applying && (p.mode !== 'bridge' || !A.bridgeReport || !(accept || earlier)))
      return { status: 'failed', error: 'Bridge apply requires mode bridge, bridgeReport, and a nonempty acceptance section.' }
    const options = (more = {}) => ({ Work: work, CitationKey: Z.KEY, ...more })
    if (applying) {
      const result = await Z.bookStep('bridge: apply', 'bridge-apply', options({ ReportId: A.bridgeReport, ...(accept ? { Accept: accept } : {}), ...(earlier ? { AcceptEarlier: earlier } : {}) }))
      return result || { status: 'failed', error: 'no output' }
    }
    const jobs = { dsh: work, writes: `${work}\\out`, phase: 'Bridge', skills: ['zettelkasten', 'asd-ste100-zh'], effort: Z.DSH_EFFORT, what: 'bridge judge writers', readOnly: true, run: true }
    // A report freezes its plan and proposals. So publish only after a plan whose jobs all answered; otherwise keep the
    // folder unpublished, so a rerun of mode bridge on the same out resumes it.
    let plan, error
    try {
      plan = await Z.bookStep('bridge: plan', 'bridge-start', options({ Profile: p.profile, MaxSources: A.bridgeMax ?? 60, Candidates: A.bridgeCandidates ?? 5, ...(A.bridgeBooks ? { Books: A.bridgeBooks } : {}) }))
      if (!plan || plan.ok !== true) error = `Bridge planning failed: ${(plan && plan.error) || 'no output'}`
      else if (Number(plan.jobs || 0)) {
        const written = await Z.runJobs('bridge: plan write', plan.file, plan.jobs, jobs)
        const failed = (written || []).filter(r => r.status !== 'done').map(r => r.id)
        if (failed.length) error = `Bridge judge jobs failed: ${failed.join(', ')}. The report is not published; rerun mode bridge with the same out to resume.`
      }
    } catch (e) { error = error || String(e) }
    if (error) return { status: 'failed', error }
    const summary = plan && plan.ok === true ? await Z.bookStep('bridge: report', 'bridge-report', options()) : null
    if (!summary || summary.ok !== true || !summary.step || !summary.report_id) return { status: 'failed', error: (summary && summary.error) || 'No Bridge report summary (step and report_id missing).' }
    // v9: the stage applies every new-carrier proposal of its own report at once, so no review waits for the user
    // (acemoglu2007equilibrium: 67 proposals left open). The apply publishes the Bridge revision itself.
    // Earlier-carrier proposals write into another book's tree, usually locked: they stay for an explicit apply.
    if (Number(summary.new || 0) > 0) {
      const result = await Z.bookStep('bridge: apply new', 'bridge-apply', options({ ReportId: summary.report_id, Accept: 'all' }))
      if (!result || result.status === 'failed') return { ...summary, status: 'failed', error: `Bridge apply failed: ${(result && (result.error || result.status)) || 'no detail'}`, applied: result }
      return { ...summary, applied: result, publication: result.publication }
    }
    const publication = await publish('bridge', { Report: `${work}\\proposals.json` })
    return { ...summary, publication }
  },
})
// ==== end module zt-bridge ====

// ==== module zt-cards-book v6 ====
// The whole-book Card cycle as ready ledger work (modes cards and full, R296/R247): publish the current
// `cards-book` revision from the settled digest, Model and Cards contributions, claim it, run the one cycle in
// its declared steps (plan → DeepSeek jobs → apply/check rounds → apply → report), and record it with
// `book-cards-finish`, which re-derives the deterministic pass and requires one reviewed owner per Model draft.
// The cycle's coverage gate waits for the delegated Model reviews to settle while model compilation never waits for
// it (R248). In full it also reviews the source of a Model draft no model chain owns.
// 5 (#176, docs/workflow-module-structure-proposal.md): the publication, the claim, the cycle's steps and the
// checked completion are book-steps.ps1 steps run by DeepSeek drive jobs; no Claude relay runs a command.
// 6 (live run 2026-10-26, staging5): the claim holds the layers' longest lease (LAYER_LEASE): a 300-second lease
// expired during the cycle and book-cards-finish refused it as stale_claim.
defineModule({
  id: 'zt-cards-book', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the claimed whole-book card cycle (modes cards and full), recorded by book-cards-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    // In mode cards no model chain runs in this invocation; in full the chain exists unless args.models is false.
    const modelReview = Z.MODE === 'cards' || Z.A.models === false
    // The publication revision is derived from the present contributions; the same manifest reuses it.
    const published = await Z.bookStep('cards publish', 'publish', { Layer: 'cards-book', Ledger: true })
    const unit = published && published.unit
    if (!unit) return { status: 'failed', error: (published && published.error) || 'the cards-book revision was not registered' }
    const got = await Z.bookStep('cards-book claim', 'cards-book-start', { Wait: 300, Lease: Z.LAYER_LEASE })
    if (!got || got.ok !== true) return { status: 'failed', error: (got && got.error) || 'the cards-book claim failed' }
    const claimed = got.unit ? { id: got.unit, token: got.token, input_hash: got.input_hash, inputs: got.inputs || {} } : null
    if (!claimed) return { status: 'active', error: `the ${unit} revision is waiting for its Model reviews to settle`, gate: got.gate }
    log(`Cards whole book: one cycle over ${Z.KEY} on ${claimed.id}${modelReview ? ', reviewing the source of every Model draft no model chain owns' : ''}.`)
    const cycle = await Z.cardCycle('whole book', `${Z.OUT}\\cards`, { sample: Z.IDEA_SAMPLE, modelReview })
    if (cycle.status !== 'done') {
      // v4 (S5): the code is the fact (no result); the ledger classifies it.
      await Z.bookStep('cards-book fault', 'unit-fail', {
        Unit: claimed.id, Token: claimed.token, InputHash: claimed.input_hash,
        Code: 'missing_result', Cause: String(cycle.error || 'the whole-book card cycle did not complete').slice(0, 800),
      })
      return { status: 'failed', ...cycle, unit: claimed.id }
    }
    const finished = await Z.bookStep('cards-book finish', 'cards-book-finish', { Unit: claimed.id, Token: claimed.token, InputHash: claimed.input_hash, Work: `${Z.OUT}\\cards` })
    if (!finished || finished.ok !== true) {
      log(`Cards whole book: the cycle ran but its checked completion failed: ${(finished && finished.error) || 'no output'}`)
      return { status: 'partial', cycle, unit: claimed.id, error: (finished && finished.error) || 'no output' }
    }
    return { status: 'done', cycle, unit: claimed.id, completion: finished }
  },
})
// ==== end module zt-cards-book ====

// ==== module zt-cards-layer v4 ====
// The Cards layer of a managed run: one deterministic offline check per verified group (`cards/gNNN@1`).
// `group-finish` registers the check; `layer-steps.ps1 -Step start` claims a block of the ready checks and
// records each one through `zt-digest finish-batch -Layer cards`, which re-reads the group's owned Cards, runs
// the shared checker in-process and hands every hard fault to a registered repair consumer. The step is one
// script call run by a DeepSeek drive job: no Claude agent claims or finishes a unit, and the layer stops
// when a claim finds no ready check. It never writes a Card and never starts a content job for a check that is
// not ready (R252/R254/R257).
// 2: the claim and the checked finishes of a block run as ONE command (`zt-digest finish-batch -Layer cards`), so a
// block costs one command instead of a claim and one command per group (docs/digest-claude-token-proposal.md P1/P2).
// 3 (#175): that command is the `start` step of `layer-steps.ps1`, run by a DeepSeek drive job under a Haiku manager.
// 4 (#180): a step answers counts (`done`, `failed`), never the block's per-unit results.
defineModule({
  id: 'zt-cards-layer', version: '4', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the per-group offline Card checks, claimed from the ledger ready work and recorded by cards-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const result = await Z.layerRun('cards', {
      layer: 'cards', take: 4, rounds: Z.MAX_ATTEMPTS, phase: 'Cards', jobs: false,
    })
    const verified = Number(result.verified || 0)
    const failed = Number(result.failed || 0)
    if (result.status === 'failed') return { status: 'failed', error: result.error, verified, failed, gate: result.gate }
    log(`Cards: ${result.status} — ${verified} verified, ${failed} failed${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, verified, failed, gate: result.gate }
  },
})
// ==== end module zt-cards-layer ====

// ==== module zt-digest-prepare v18 ====
// Prepare (mode full): prepare-book-digest writes the Chinese text, the section groups, the Literature tree with one
// anchor per group, and the shared prefixes, then binds the immutable run contract with the effective settings
// (maxAttempts, dshRunCap) and initializes the run ledger. No stage reads the account-wide usage ledger: run usage is
// recorded per provider run from that run's own result (R343-R346).
// args.book (a Scholar Archive Work id or exact title) replaces the explicit source, BibTeX and title: the tool resolves
// them through the read-only bridge, and an unresolved choice stops with its candidates. A resume (mode resume)
// prepares nothing and generates no file: it runs the one supported `ledger resume`, which verifies the frozen
// identity, reconciles stranded claims and retained results, opens invocation 2 and spends the one-rerun budget, then
// reads the retained group enumeration, settings and identity from the ledger (#87, R274-R285). Other modes take the
// group count from args.groups.
// The output also carries groupList, the per-group items the digest stage's chains are built from.
// 10 (Tree Lock S17, #140): a preparation the owner's Lock refused stops the run with the archive's own typed
// `card_locked` refusal (R326/R337), never as a generic preparation failure.
// 11 (docs/literature-only-digest-design.md): no hubs switch is frozen; the prepared run never has a hubs layer.
// 13 (S24, #167): the Term-card branch and the Term-link pass freeze their switches too (termCards, termLink), so a
// layer this run disables is excluded in the prepared target and no unit waits for work that will not come (R0430).
// 14 (S25, #168): modes term-cards and term-link are the backfill's own preparation. They run the one existing
// zt-digest prepare with -Sections (reuse or reconstruct the staged source slices) or -Validate (inspect the book and
// the staging only), read the book's Citation key from the prepared run's manifest and stop before any provider work on
// a Lock or a conflict. -Sections creates no root, anchor, group, model or narrative Card (R0435); neither mode
// reruns an ordinary digest or model stage (R0436).
// 16 (#176, docs/workflow-module-structure-proposal.md): the stage runs its whole chain as one book-steps.ps1 step
// (prepare: the full preparation, the one resume with its summary and digest-gate fallback, or a backfill branch),
// run by a DeepSeek drive job; no Claude relay runs a command.
// 18 (#181): the stage takes the fresh preparation's Citation key from its own `key` param (the blueprint passes
// args.citationKey) instead of zdMake reading args.citationKey: the prepared run it creates is the one source every
// later stage reads the key from (stages.prepare.output.citation_key), so a resume that names only `out` still
// hands each stage the book's key.
defineModule({
  id: 'zt-digest-prepare', version: '18', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the book (mode full, from args.book or the explicit source), resume the retained run (mode resume, one ledger resume), validate or re-stage one already digested book for a Term backfill (modes term-cards and term-link) or take args.groups; output {groups, groupList, citation_key, backfill?}.',
  params: {
    key: { type: 'any', description: "a fresh preparation's Citation key (args.citationKey): the prepared run this stage creates is what every later stage reads it from (#181)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    needGroups: { type: 'array', default: ['full', 'link'], description: 'modes that need a group count' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile): the prepared-run variant' },
    instructions: { type: 'any', description: 'the digest instruction files (comma-separated, from consts.profile): the base prompt and the supplements of the book type' },
    noEventCandidates: { type: 'any', description: 'true: the group files leave out the event candidates (the event layer reads events.json)' },
  },
  run: async (ctx, p) => {
    const A = p.args
    const Z = zdMake(ctx, { args: A, mode: p.mode, key: p.key })
    const groupList = (count) => Array.from({ length: count || 0 }, (_, i) => ({ n: i + 1, nn: String(i + 1).padStart(2, '0') }))
    let groups = A.groups || 0
    if (p.mode === 'resume') {
      // R278-R285: the step runs the one supported resume and its summarized status; the stage reads the retained
      // group enumeration and identity, and a summary without layer counts is answered by the digest gate inside
      // the step.
      const resumed = await Z.bookStep('prepare resume', 'prepare', { Mode: 'resume' })
      if (!resumed || resumed.ok !== true) throw new Error(`mode resume needs a resumable prepared run ledger: ${(resumed && resumed.error) || 'no ledger resume'}`)
      // #180: the resume step answers the retained facts as flat counts; its full tool lines are its detail file.
      const n = (v) => Number(v || 0)
      const view = { citation_key: resumed.citation_key, variant: resumed.variant }
      groups = n(resumed.groups)
      if (!groups) throw new Error('mode resume found no retained digest groups; prepare the book again')
      const settings = { max_attempts: n(resumed.max_attempts), dsh_run_cap: n(resumed.dsh_run_cap) }
      if (!resumed.resumed) {
        log(`Resume: no executable or failed unit remains (${groups} retained groups, invocation ${n(resumed.invocation)}); the retained coverage stays what the ledger reports, and no resume was spent.`)
      } else {
        log(`Resume: invocation ${n(resumed.invocation)}, resume_count ${n(resumed.resume_count)}; ${n(resumed.reset)} unresolved unit(s) reset, ${n(resumed.reevaluated)} blocked dependant(s) back to dependency evaluation, ${n(resumed.reclaimed)} lost claim(s) reclaimed, ${n(resumed.downstream)} unit(s) in the closure to drain. ${groups} retained groups, variant ${view.variant}, settings maxAttempts ${settings.max_attempts}, dshRunCap ${settings.dsh_run_cap} s.`)
      }
      // v15 (S30 pilot, #172): the prepared run's frozen layer switches. A resume gets only `out`, so the
      // Term stages read termCards/termLink here instead of their args defaults (true).
      // #180: the resume step names the frozen switches a layer switched off, as one short list.
      const off = String(resumed.off || '').split(',').filter(Boolean)
      const runOptions = off.length ? Object.fromEntries(off.map((k) => [k, false])) : null
      return { groups, groupList: groupList(groups), resumed: resumed.resumed === true, resume: null, settings, citation_key: view.citation_key, run_options: runOptions }
    }
    if (p.mode === 'term-cards' || p.mode === 'term-link') {
      // S25 (#168): the backfill's own preparation. `-Sections` reuses or reconstructs the staged source slices the
      // Term-card branch reads; `-Validate` only inspects the already digested book, because the deterministic
      // Term-link pass needs no source and starts no provider probe (R0436). Both write no Card, read the Citation
      // key from the prepared run's own manifest, and stop before any provider work on a Lock or a conflict.
      const line = await Z.bookStep(`prepare backfill: ${p.mode}`, 'prepare', { Mode: p.mode })
      if (!line || line.ok !== true) {
        throw new Error(`the backfill of ${(line && line.book) || 'the book'} cannot start: ${(line && line.error) || 'no output'}`)
      }
      const n = (v) => Number(v || 0)
      log(`Backfill (${line.step}): ${line.book} — ${n(line.sections)} section(s) ${line.reused ? 'reused' : 'reconstructed'}${n(line.rebuilt) ? ` (${n(line.rebuilt)})` : ''}; ${n(line.rows)} Term row(s)${line.legacy ? ' in the legacy table' : ''}; ${n(line.heads)} published head(s), ${n(line.parts)} part(s), ${n(line.occupying)} occupying; ${n(line.conflicts)} conflict(s).`)
      return { groups: 0, groupList: [], citation_key: line.book, backfill: line }
    }
    if (p.mode === 'full') {
      // The layer switches the run declares are frozen into the prepared target: a layer this run deliberately
      // excludes is excluded in the ledger too, so no unit waits for work that will not come.
      const switches = {}
      for (const name of ['terms', 'models', 'events', 'findings', 'figures', 'bridge', 'notation', 'boundary', 'proof', 'theory', 'termCards', 'termLink']) {
        if (typeof A[name] === 'boolean') switches[name] = A[name]
      }
      const options = {
        Mode: 'full',
        CitationKey: Z.KEY,
        MaxChars: A.maxChars || 20000,
        MaxUnits: A.maxUnits || 3,
        Variant: p.profile && p.profile.book === 'mathematics' ? 'math' : 'econ',
        MaxAttempts: Z.MAX_ATTEMPTS,
        DshRunCap: Z.DSH_RUN_CAP,
      }
      // args.book names the book in the Scholar Archive; the tool resolves its Markdown, BibTeX and title, so the
      // explicit arguments are left out rather than passed empty.
      if (A.book) options.Book = A.book
      else { options.Source = A.source; options.BibTeX = A.bibtex; options.BookTitle = A.bookTitle }
      if (A.startHeading) options.StartHeading = A.startHeading
      if (A.endHeading) options.EndHeading = A.endHeading
      if (A.skipHeadings) options.SkipHeadings = A.skipHeadings
      if (A.fileId) options.FileId = A.fileId
      if (p.instructions) options.Instructions = p.instructions
      if (p.noEventCandidates) options.NoEventCandidates = true
      if (Object.keys(switches).length) options.RunOptions = JSON.stringify(switches)
      // v12 (S1): the preparation ends with one short line (the group count), which a Haiku agent had counted out of
      // the whole manifest. Exit 3 with needs_selection names the Works or files to choose from: the run halts for the
      // choice (pass one Work id as args.book, or one file id as args.fileId).
      const prepared = await Z.bookStep('prepare book', 'prepare', options)
      const prep = prepared && prepared.ok === true && Number(prepared.groups) > 0
        ? { ok: true, groups: Number(prepared.groups) }
        : { ok: false, error: prepared ? JSON.stringify(prepared) : 'no output' }
      if (!prep || !prep.ok || !prep.groups) {
        log(`Preparation failed: ${prep && prep.error}`)
        // R326/R329/R337: preparation writes the Literature tree first, so a Lock on that tree stops the run here with
        // the archive's own typed refusal, never as a generic preparation failure.
        const lock = (prepared && (prepared.code === Z.LOCK || prepared.error === Z.LOCK)) ? prepared : Z.lockRefusal(JSON.stringify(prepared || {}))
        if (lock) throw Z.lockStop('prepare', lock)
        throw new Error(`preparation failed: ${(prep && prep.error) || 'no report from the prepare step'}`)
      }
      groups = prep.groups
    }
    if (!groups && (p.needGroups || ['full', 'link']).includes(p.mode)) throw new Error(`mode "${p.mode}" needs args.groups`)
    return { groups, groupList: groupList(groups), citation_key: Z.KEY }
  },
})
// ==== end module zt-digest-prepare ====

// ==== module zt-digest-report v20 ====
// The run's result, in the shape zt-book-digest.js returned for each mode: a stage-only mode returns that
// stage's report; mode full (and link, and models with organize) returns the whole digest report. A
// variant's own mode (math) returns {mode, dsh_model, citation_key} with the variant's extra reports; in the
// other modes the extra reports join the whole report (e.g. math: {...}). 4.0.0 adds the effective settings
// (maxAttempts, dshRunCap), the Cards layer's gate, and a schedule summary of what every group chain and layer
// driver settled: verified, failed, retrying and open work, never a claim of work that no gate confirmed.
// 8 adds run_report: the verified ledger report (completeness, gaps, current placeholders, timings, exact
// per-run usage and the supported resume invocation), so a run's result carries the report's own facts
// instead of an agent's summary (#85, R331-R346). 9 adds finalize: the finite finalization's plan, one
// correction cycle, changed-Card manifest, refreshed publications and final audit (#86, R299/R300). 10 adds
// link_blocked: the groups the Link pass did not work because their digest unit is not done (#92). 11 adds
// link_gaps: the scheduled, skipped and gap counts and the L1/L2 quality of the gap-only Link pass, with each
// group's own outcome and actual provider runs (#110). 12 adds concepts and review: the frozen concept review
// of the run's own term table and the scoped restatement resolution, with their report ids, completeness and
// counts, so the integrated digest path reports what it reviewed and what it left pending (S23/#115).
// 15 (docs/literature-only-digest-design.md): a digest builds only the Literature tree, so the concept review,
// the restatement review of new Topic Cards, the hubs, the register and mode hubs leave the report; zt-idea-grow
// reports them. 16 adds next: the digest built the Literature tree only, so the whole report (and the log) names
// the follow-up run that grows its Topic Cards, hubs and register: zt-idea-grow with this book and a fresh folder.
// 17 (S24, #167) adds term_cards and term_link: each stage's truthful complete/degraded/skipped status with its
// ledger unit, plan, readiness/publication or apply counts and the reason it stopped, so a failed Term-card job or
// a blocked Term-link pass is visible coverage debt in the run's own result (R0428).
// 18 (S25, #168) adds the backfill modes term-cards and term-link: the pre-dispatch validation (Workspace, list,
// occupancy, source/sections fingerprints, Lock and conflicts) plus the Term stages' own result and the read-only
// audit, so one selected book's Term backfill reports its coverage and debt without a fresh digest.
// 19 (S26, #169) adds term_link.final: the finite finalization's own round (generation, scope, unit, status and
// counts), read from the final report's ledger facts, and the final audit's Term quality under `terms`, so a run
// that corrected Cards reports whether their Term links were restored and audit-ran never reads as all-terms-passed.
// 20 (#180): the stage outputs it assembles carry counts only (a step line is at most 400 characters and a module
// reads no unit list), so `digest`, `link`, `link_blocked` and the Term summaries report totals and how many groups
// were verified, skipped or blocked; the per-unit facts stay in the ledger's run report (`run_report`).
defineModule({
  id: 'zt-digest-report', version: '20', requires: ['zt-digest-lib'],
  summary: 'ZT digest: assemble the run\'s result from the stage outputs, per mode, with the settings, the schedule summary and the verified ledger run report.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'any', description: 'the DeepSeek model from the source stage' },
    groups: { type: 'any', description: 'the group count' },
    digest: { type: 'any', description: 'the digest stage output (its per-group chains)' },
    cards: { type: 'any', description: 'the Cards layer output (the per-group offline checks)' },
    termCards: { type: 'any', description: 'the Term-card branch output: its units, waves, readiness and publication (S24, #167)' },
    termLink: { type: 'any', description: 'the deterministic Term-link pass output: its unit, plan, counts and no_op (S24, #167)' },
    models: { type: 'any', description: 'the output of the model layer (the models stage of a book type)' },
    terms: { type: 'any', description: 'the term table stage output' },
    prepare: { type: 'any', description: 'the prepare stage output: the backfill validation of modes term-cards and term-link (S25, #168)' },
    cardsBook: { type: 'any', description: 'the whole-book card cycle (mode cards)' },
    explain: { type: 'any', description: 'the explain stage output (mode explain)' },
    link: { type: 'any', description: 'the link pass output' },
    linkCheck: { type: 'any', description: 'the link check card cycle' },
    linkBlocked: { type: 'number', default: 0, description: 'how many groups the Link pass left blocked because their digest unit is not done (#92)' },
    cleanup: { type: 'any', description: 'the cleanup stage output, with the verified run report' },
    organize: { type: 'any', description: 'the Organize stage output' },
    figures: { type: 'any', description: 'the figures stage output' },
    bridge: { type: 'any', description: 'Bridge report or explicit apply summary' },
    audit: { type: 'any', description: 'the audit stage output' },
    finalize: { type: 'any', description: 'the finite finalization branch output (#86)' },
    extra: { type: 'object', default: {}, description: 'a variant\'s own reports, added to the result (e.g. {"math": {...}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const A = Z.A, MODE = p.mode, KEY = Z.KEY
    const dshModel = p.model || Z.DSH_MODEL
    const extra = p.extra || {}
    const or = (v, d) => (v === undefined ? d : v)
    // The verified ledger report every mode carries: the run's own completeness, gaps, placement, timings,
    // exact per-run usage and one supported resume. It never substitutes for a stage's own checked output.
    const runReport = (p.cleanup && p.cleanup.report) || null
    const runUsage = (p.cleanup && p.cleanup.usage) || (runReport && runReport.usage) || null
    // #180: a layer answers counts (verified and failed units, the gate), never per-unit rows.
    const digestSummary = p.digest
      ? { groups: p.digest.groups ?? null, claimed: p.digest.claimed ?? 0, verified: p.digest.verified ?? 0, failed: p.digest.failed ?? 0 }
      : null
    const layerSummary = (value) => value && (value.verified !== undefined || value.failed !== undefined || value.gate)
      ? { status: value.status, verified: value.verified ?? 0, failed: value.failed ?? 0, open: (value.gate && value.gate.counts && (value.gate.counts.open || 0)) || 0, complete: !!(value.gate && value.gate.complete) }
      : null
    // S24 (#167): the two Term stages' own truthful result. A stage that did not run is `skipped` with
    // its reason, never an absent or silently successful entry.
    const termSummary = (value, what) => !value
      ? { status: 'skipped', reason: `${what} did not run` }
      : {
        status: value.status || 'unknown',
        unit: value.unit || value.book_unit || null,
        pass: value.pass || null,
        round: value.round ?? null,
        plan: value.plan || (value.plan_revision !== undefined ? { revision: value.plan_revision, hash: value.plan_hash } : null),
        jobs: value.jobs ? { total: value.jobs.total || 0, done: value.jobs.done || 0, failed: Number(value.jobs.failed || 0) } : null,
        readiness: value.readiness || null,
        publication: value.publication || null,
        completion: value.completion || null,
        counts: value.counts || null,
        no_op: value.no_op === true,
        blocked: Number(value.not_ready || 0),
        reason: value.reason || value.error || null,
      }
    const schedule = {
      settings: { maxAttempts: Z.MAX_ATTEMPTS, dshRunCap: Z.DSH_RUN_CAP },
      digest: digestSummary ? { verified: digestSummary.verified, failed: digestSummary.failed } : null,
      cards: layerSummary(p.cards),
      models: layerSummary(p.models),
      explain: layerSummary(p.explain),
      events: layerSummary((extra && extra.events) || null),
      findings: layerSummary((extra && extra.findings) || null),
      link: p.link ? { scheduled: p.link.scheduled ?? 0, skipped: p.link.skipped ?? 0, verified: p.link.verified ?? 0, failed: p.link.failed ?? 0, provider_runs: p.link.provider_runs ?? 0 } : null,
      term_cards: termSummary(p.termCards, 'the Term-card branch').status,
      term_link: termSummary(p.termLink, 'the Term-link pass').status,
      term_link_final: (p.finalize && p.finalize.final && p.finalize.final.term_link && p.finalize.final.term_link.status) || null,
    }
    // S26 (#169): the finite finalization's own Term-link round and the final audit's Term quality,
    // both read from the final report's ledger facts, never inferred from the whole-book pass.
    const termLinkSummary = (value, finalize) => {
      const summary = termSummary(value, 'the Term-link pass')
      const round = finalize && finalize.final && finalize.final.term_link
      if (round) {
        summary.final = {
          applicable: round.applicable !== false,
          round: round.round ?? null,
          scope: round.scope || null,
          status: round.status || 'unknown',
          unit: round.unit || null,
          state: round.state || null,
          counts: round.counts || null,
          no_op: round.no_op === true,
          reason: round.reason || null,
        }
      }
      if (finalize && finalize.final && finalize.final.terms) summary.audit_terms = finalize.final.terms
      return summary
    }
    if (MODE === 'bridge') return { mode: MODE, citation_key: KEY, bridge: or(p.bridge, { status: 'failed', error: 'Bridge stage produced no summary' }) }
    if (MODE === 'cards') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, cards: or(p.cardsBook, null), schedule, run_report: runReport }
    // One whole-book card cycle (R296): book is that cycle's report, link the link check's, groups the per-group checks.
    const cardsReport = { book: or(p.cardsBook, null), link: or(p.linkCheck, null), groups: layerSummary(p.cards) }
    if (MODE === 'explain') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, explain: or(p.explain, null), schedule, run_report: runReport }
    if (MODE === 'figures') return { mode: MODE, citation_key: KEY, figures: or(p.figures, null), run_report: runReport }
    if (MODE === 'audit') return { mode: MODE, citation_key: KEY || null, audit: or(p.audit, null), run_report: runReport }
    if (MODE === 'models' && !A.organize) return { mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, models: (p.models && p.models.models) || [], model_cleanup: (p.models && p.models.model_cleanup) || null, schedule, run_report: runReport }
    // S25 (#168): one selected book's Term backfill. The result carries the pre-dispatch validation it ran
    // (Workspace, list, occupancy, source/sections fingerprints, Lock and conflicts), each Term stage's own
    // truthful result and the read-only audit; it is never a fresh digest's report.
    if (MODE === 'term-cards' || MODE === 'term-link') {
      const backfill = (p.prepare && p.prepare.backfill) || null
      const tc = termSummary(p.termCards, 'the Term-card branch')
      const tl = termLinkSummary(p.termLink, p.finalize)
      const conflicts = Number((backfill && backfill.conflicts) || 0)
      ctx.log(`Backfill ${MODE}: ${tc.status} term-cards, ${tl.status} term-link${backfill ? `; ${Number(backfill.rows || 0)} Term row(s), ${Number(backfill.heads || 0)} published head(s)` : ''}${conflicts ? `, ${conflicts} conflict(s)` : ''}.`)
      return {
        mode: MODE,
        citation_key: KEY,
        backfill,
        term_cards: tc,
        term_link: tl,
        audit: or(p.audit, null),
        schedule,
        run_report: runReport,
      }
    }
    if (!['full', 'link', 'models', 'resume'].includes(MODE)) return { mode: MODE, dsh_model: dshModel, citation_key: KEY, ...extra, schedule }
    const modelReports = (p.models && p.models.models) || []
    // #180: the Link pass answers the run's own totals, never per-group rows.
    const linkGaps = p.link
      ? { scheduled: p.link.scheduled ?? 0, skipped: p.link.skipped ?? 0, gaps: p.link.gap ?? 0, skipped_cards: p.link.skipped_cards ?? 0, linked: p.link.linked ?? 0, linked_from: p.link.linked_from ?? 0, already_linked: p.link.already_linked ?? 0, no_link: p.link.no_link ?? 0, changed: p.link.changed ?? 0, provider_runs: p.link.provider_runs ?? 0, verified: p.link.verified ?? 0, failed: p.link.failed ?? 0, blocked: p.link.blocked ?? 0, quality: p.link.quality || null }
      : null
    const groupCount = p.groups || 0
    const next = { workflow: 'zt-idea-grow', script: '.claude/workflows/zt-idea-grow.js', args: { book: KEY, out: '<a fresh work folder>' }, why: 'this digest built only the Literature tree; zt-idea-grow grows its Topic Cards, hubs and register' }
    ctx.log(`Next: run zt-idea-grow with book "${KEY}" and a fresh out folder: the digest wrote no Topic Card, Hub Card or register entry.`)
    if (p.termCards || p.termLink) {
      const tc = termSummary(p.termCards, 'the Term-card branch')
      const tl = termLinkSummary(p.termLink, p.finalize)
      ctx.log(`Terms: term-cards ${tc.status}${tc.unit ? ` (${tc.unit})` : ''}${tc.reason ? `: ${tc.reason}` : ''}; term-link ${tl.status}${tl.unit ? ` (${tl.unit})` : ''}${tl.reason ? `: ${tl.reason}` : ''}${tl.final ? `; final round ${tl.final.round} ${tl.final.status} (${tl.final.scope})` : ''}.`)
    }
    return {
      mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, groups: groupCount, managers_per_pass: Z.toBlocks(Z.groupsOf(groupCount)).length,
      digest: digestSummary,
      models: modelReports.map(r => ({ model: r.model, level: r.level, status: r.status, parts: Array.isArray(r.parts) ? r.parts.length : (Number(r.parts) || 0), relations: r.relations, kinds: r.kinds, charts: r.charts, derivations: r.derivations, split: r.split, moved: r.moved, shared: r.shared, tolerated: r.tolerated, style: r.style, accepted: r.accepted, dsh: r.dsh, issues: r.issues })),
      model_cleanup: (p.models && p.models.model_cleanup) || null,
      link: linkGaps ? { scheduled: linkGaps.scheduled, skipped: linkGaps.skipped, verified: linkGaps.verified, failed: linkGaps.failed, provider_runs: linkGaps.provider_runs } : null,
      link_gaps: linkGaps,
      link_blocked: Number(p.linkBlocked || 0),
      terms: (p.terms && p.terms.report) || null, cards: cardsReport,
      term_cards: termSummary(p.termCards, 'the Term-card branch'),
      term_link: termLinkSummary(p.termLink, p.finalize),
      deepseek_usage: runUsage, cleanup: (p.cleanup && p.cleanup.cleanup) || null, run_report: runReport,
      organize: or(p.organize, null), figures: or(p.figures, null), bridge: or(p.bridge, MODE === 'full' ? (A.bridge === false ? { status: 'skipped' } : { status: 'failed', error: 'Bridge stage produced no summary' }) : null), audit: or(p.audit, null), finalize: or(p.finalize, null),
      schedule,
      next,
      ...extra,
    }
  },
})
// ==== end module zt-digest-report ====

// ==== module zt-dsh-cleanup v13 ====
// R231/R322–R330: the run's cleanup is settled from ledger evidence, never from a workspace listing's titles.
// v11 (2026-10-23): no model reads, relays or decides any cleanup fact. `zt-digest cleanup-run -Step plan` records
// the evidence already on disk and writes the plan file (every recorded id still to delete, every recorded workspace
// not yet removed); the bridge's `deepseek_settle` runs that plan in the MCP server that holds the conversations open
// (it closes each first) and writes the results file; `cleanup-run -Step apply` records every fact and settles every
// ready cleanup unit through its checked verifier. The run report is rendered after cleanup settles.
// v12 (#176, docs/workflow-module-structure-proposal.md): the plan, the apply (with the `cleanup-finish -Ready`
// fallback when the settle tool is missing or its results are unusable) and the run report are book-steps.ps1 steps
// run by DeepSeek drive jobs. One Haiku manager makes the one call no script can: `deepseek_settle` with the plan
// file and the results file. No Claude agent runs a command, so no id list passes through an agent: it sees two paths
// and one answer.
// A workspace that still lists a conversation the run did not record is kept and reported (that member is the
// user's); a refused deletion stays a retryable provider fact.
// The run report (#85, R331–R346) is rendered here, after cleanup settles: every stage of the run has finished, so the
// report describes the whole invocation. Its usage section is the exact per-run usage the ledger recorded from each
// run's own result; this module never reads or differences the account-wide usage ledger, because the difference of
// two rolling totals is not a run's cost (R344).
defineModule({
  id: 'zt-dsh-cleanup', version: '13', requires: ['zt-digest-lib'],
  summary: 'ZT digest: settle every recorded DeepSeek conversation and workspace from ledger evidence (one settle call, two steps), then render the verified run report.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const OUT = Z.OUT
    const PLAN = `${OUT}\\cleanup\\settle-plan.json`
    const RESULTS = `${OUT}\\cleanup\\settle-results.json`
    const SETTLED = {
      type: 'object',
      properties: { settle: { type: 'string' }, ok: { type: 'boolean' } },
      required: ['settle'],
    }
    // 1. The plan: every recorded id still open and every recorded workspace not yet removed.
    const planned = await Z.bookStep('cleanup: plan', 'cleanup-start')
    if (!planned || planned.ok !== true) {
      log(`Cleanup: the plan did not run (${(planned && planned.error) || 'no output'}).`)
      return { status: 'partial', cleanup: { status: 'unsettled' }, error: (planned && planned.error) || 'the cleanup plan did not run' }
    }
    // 2. The one provider call no script makes: `deepseek_settle` runs the plan inside the MCP server that holds the
    //    conversations open. The manager copies one short line; it deletes nothing and reads no id list.
    const got = await dsmpSlot(() => Z.zAgent(
      `Settle the DeepSeek Harness conversations and workspaces of one digest run. A script decides every id and has already written the plan. You make ONE call and copy its short answer. Never delete, list or remove anything yourself, and never call deepseek_delete or deepseek_remove_workspace.
1. Load the tool with one ToolSearch call: query "select:mcp__deepseek__deepseek_settle". Call deepseek_settle once with plan_file "${PLAN}" and results_file "${RESULTS}". Put its answer, as one line, in settle. If the tool is missing or fails, put the error in settle.
2. Return settle = that one line, and ok = true when the call answered.`,
      { label: 'cleanup: settle the run', phase: 'Cleanup', model: 'haiku', schema: SETTLED },
    ))
    if (!got || !got.ok) log(`Cleanup: the provider batch did not run (${String((got && got.settle) || 'no answer').slice(0, 240)}).`)
    // 3. Record the results and settle every ready cleanup unit; without usable results the units whose conversations
    //    are already gone still settle through `cleanup-finish -Ready`.
    const applied = await Z.bookStep('cleanup: apply', 'cleanup-apply')
    if (!applied || applied.ok !== true) {
      log(`Cleanup: the settlement did not run (${(applied && applied.error) || 'no output'}).`)
      return { status: 'partial', cleanup: { status: 'unsettled' }, error: (applied && applied.error) || 'the cleanup settlement did not run' }
    }
    // #180: the apply step answers the provider and settle facts as flat counts.
    const n = (v) => Number(v || 0)
    const ran = applied.fallback !== true ? true : null
    if (!ran) log(`Cleanup: the provider batch did not run (${String(applied.provider_error || 'the settle results were not usable').slice(0, 240)}).`)
    else log(`Cleanup: deleted ${n(applied.deleted)}, already gone ${n(applied.absent)}, failed ${n(applied.failed)}; workspaces removed ${n(applied.workspaces_removed)}, kept ${n(applied.workspaces_kept)}${n(applied.unknown) ? `; ${n(applied.unknown)} conversation(s) the run did not record stay` : ''}${n(applied.unrecorded) ? `; ${n(applied.unrecorded)} fact(s) not recorded` : ''}.`)
    log(`Cleanup: ${n(applied.ready_done)} of ${n(applied.ready_claimed)} claimed unit(s) settled (${n(applied.ready_partial)} partial); ${n(applied.ready_retry)} retryable, ${n(applied.ready_failed)} failed.`)
    const cleanup = {
      provider: ran ? { deleted: n(applied.deleted), absent: n(applied.absent), failed: n(applied.failed), workspaces_removed: n(applied.workspaces_removed), workspaces_kept: n(applied.workspaces_kept), unknown: n(applied.unknown), unrecorded: n(applied.unrecorded) } : null,
      settled: { claimed: n(applied.ready_claimed), done: n(applied.ready_done), partial: n(applied.ready_partial), retry: n(applied.ready_retry), failed: n(applied.ready_failed) },
      units: [],
      workspace_removed: ran ? n(applied.workspaces_kept) === 0 : false,
      files_removed: 0,
      content_complete: null,
      cleanup_complete: n(applied.ready_failed) === 0 ? true : null,
      status: applied.book_status || (n(applied.ready_failed) ? 'partial' : 'complete'),
    }
    // 4. R331: the verified run report. Every stage has settled, so the report describes this invocation:
    //    completeness, gaps, current placeholders, timings and exact per-run usage. A failed render leaves the run
    //    partial and names why; it never substitutes an agent's summary for the report.
    const facts = await Z.bookStep('report the run from the ledger', 'cleanup-report')
    let report = null
    if (!facts || facts.ok !== true) {
      log(`The run report failed: ${(facts && facts.error) || 'no output'}`)
    } else {
      // #180: the report step answers its verified facts as counts and paths; the whole report is its detail file
      // and the rendered run-report.json / run-report.md under the staging folder.
      const u = String(facts.usage || '').split(',')
      const usage = { status: u[0] || 'unknown', source: u[1] || 'unknown', known_runs: n(u[2]), missing_runs: n(u[3]), missing: [], totals: { input_tokens: n(u[4]), output_tokens: n(u[5]) } }
      report = {
        json: `${OUT}\\run-report.json`,
        markdown: `${OUT}\\run-report.md`,
        completeness: { settled: facts.settled === true, content_complete: facts.content_complete === true, quality_complete: facts.quality_complete === true, cleanup_complete: facts.cleanup_complete === true, complete: facts.complete === true },
        counts: { declared: n(facts.declared), done: n(facts.done), noop: 0, failed: n(facts.failed), open: n(facts.open) },
        gaps: n(facts.gaps),
        placeholders: { count: n(facts.placeholders) },
        timing: null,
        resume: { available: facts.resume_available === true },
        bridge: null,
        publications: n(facts.publications),
        usage,
      }
      log(`Run report: settled ${report.completeness.settled}, content ${report.completeness.content_complete}, quality ${report.completeness.quality_complete}, cleanup ${report.completeness.cleanup_complete}, complete ${report.completeness.complete}; ${report.gaps} gap(s); placeholders ${report.placeholders.count}; usage ${usage.known_runs} run(s) known, ${usage.missing_runs} missing. Report: ${OUT}\\run-report.md`)
    }
    const usage = report ? report.usage : null
    return {
      status: report ? (report.completeness.complete ? 'complete' : 'partial') : (cleanup.status === 'complete' ? 'partial' : cleanup.status),
      cleanup,
      usage,
      report,
      error: report ? undefined : ((facts && facts.error) || 'the run report did not render'),
    }
  },
})
// ==== end module zt-dsh-cleanup ====

// ==== module zt-dsh-source v2 ====
// The DeepSeek source. v2 (docs/workflow-simplicity-proposal.md S1): the source is frozen at the official one (the
// user's rule, 2026-10-20: never OpenCode Go), so no Haiku probe runs and no argument chooses it. The stage reports the
// model and logs the DeepSeek efforts raised to high.
defineModule({
  id: 'zt-dsh-source', version: '2', requires: ['zt-digest-lib'],
  summary: 'ZT digest: report the frozen DeepSeek source (the official one) and its model.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const A = p.args
    const Z = zdMake(ctx, { args: A, mode: p.mode })
    if (Z.raised.length) log(`DeepSeek efforts raised to high (the user wants at least medium; V4.1 Flash has no medium): ${Z.raised.join(', ')}.`)
    const { source, model } = zdDefaultModel()
    log(`DeepSeek source: ${source} (${model}).`)
    return { source, model }
  },
})
// ==== end module zt-dsh-source ====

// ==== module zt-event-layer v6 ====
// The Event layer of the economics set: the ledger's ready event work, worked while the digest groups are
// still running. `event-compiler` plans one job per few groups from the book's dated sentences and each
// group's own Idea Cards; DeepSeek writes the events as JSON and runs the job's SELF-CHECK; the compiler
// creates each Event Card beside the Idea Card it supports (or adds this book's source to an existing Event
// Card) and links it from every supported Idea Card with the kind word 案例. The layer driver runs the stage as
// script steps: `start` claims a block of ready `events/gNNN@1` units and runs the plan, the block's jobs run in
// Haiku managers, and `finish` applies, reports and records each group through `events-finish`, so a group whose
// job has not been applied stays open instead of being claimed complete. The layer gate is the stage's result
// (R257). Every step is one `layer-steps.ps1` call run by a DeepSeek drive job: no Claude agent relays a command
// and no Claude agent is spent per group or per finish.
// 4: the chain ends with the block's checked finishes in one command (`zt-digest finish-batch`), run by the chain's own
// manager; a unit its line does not answer is finished by one relay, then on its own (docs/digest-claude-token-proposal.md P1/P2).
// 5 (#175): the whole block is two script steps and one content round; the relay and the chain are gone.
// 6 (#180): a step answers counts, so the stage reports the block's created/merged/linked totals and how many
// group units were verified; the per-unit outcomes are the ledger's run report.
defineModule({
  id: 'zt-event-layer', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest event layer: the ledger\'s ready Event work, plan → DeepSeek JSON → apply → per-group checked completion, in two script steps run by DeepSeek drive jobs.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\events`
    const result = await Z.layerRun('events', {
      layer: 'events', take: 4, rounds: Math.max(3, Z.MAX_ATTEMPTS), wait: 600, phase: 'Events',
      dsh: work, writes: `${work}\\out`, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, what: 'event writers',
      onStart: (started) => {
        log(`Events: ${started.plan_candidates || 0} dated sentences in ${started.plan_groups || 0} groups, ${started.jobs || 0} DeepSeek jobs.`)
      },
      onFinish: (finished) => {
        log(`Events: ${finished.mid_applied_created || 0} Event Cards made, ${finished.mid_applied_merged || 0} merged, ${finished.mid_applied_linked || 0} Links, ${finished.mid_applied_refused || 0} refused.`)
      },
    })
    const t = result.totals || {}
    const applied = { created: Number(t.mid_applied_created || 0), merged: Number(t.mid_applied_merged || 0), linked: Number(t.mid_applied_linked || 0), refused: Number(t.mid_applied_refused || 0) }
    const verified = Number(result.verified || 0)
    const failed = Number(result.failed || 0)
    log(`Events: ${result.status} — ${verified} groups checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    if (result.status === 'failed') return { status: 'failed', error: result.error, verified, failed, applied, gate: result.gate }
    return { status: result.status, verified, failed, applied, gate: result.gate }
  },
})
// ==== end module zt-event-layer ====

// ==== module zt-model-lib v13 ====
// The model layer's machinery (docs/zt-digest-framework.md), moved from zt-digest-lib: the model list and the
// model compiler per batch: DeepSeek brief → Sonnet architect plan → DeepSeek fill, derive, review, split →
// report and clean-up. A book type installs the model layer with its profile (economics: the economic
// architect; mathematics: the math supplements and no Theory Card). zmlMake(ctx, params) returns start,
// elaborate, finish, logModels, modelCleanup.
// 9 (docs/digest-claude-token-proposal.md P1/P7): a batch registers its models' references with ONE `model-deps -Units`
// command, and a close round that sends nothing back writes the batch report in the same process (`-Step close -Report`).
// 10 (docs/workflow-simplicity-proposal.md S1, S10): whether a batch fills only or derives and reviews is read from
// the plans the architect wrote (`model -Step plan-status`), never from its answer.
// 11 (#177, docs/workflow-module-structure-proposal.md): every deterministic chain is ONE `model-steps.ps1` step
// run by a DeepSeek drive job (zt-workflow-lib's shared `ztDrive`); the content jobs (brief, fill, derive,
// review, split, close) run in Haiku managers through zt-digest-lib's `runJobs`; the Sonnet architect stays the
// one designer role. The module relays no command and reads no file: each step answers one JSON line.
// 12 (#180): a step's line is at most 400 characters, so the module reads counts, packed lists and file paths,
// never per-unit rows. The architect batching moved into the `start` step (it answers the plan as
// `concept:a,b|formal:c`), a plan is a packed `address:status` list, and a batch's report answers the models'
// statuses instead of their rows. The per-model rows stay in the compiler's own `report.json` and the step's
// detail file; the run report's per-model facts come from the archive (`run_report.placeholders.models`).
// 13 (live run 2026-10-26, staging4): the fill line of six formal models named each one twice in full and overflowed
// its 400 characters; the overflow answered {ok, detail}, the module read a fill with no jobs and six models stayed
// unwritten. The steps now name models without the book key (`2|a`) and the module adds it back (`full`); the
// brief's skipped models travel the same way (a list, no longer a count). An overflow is now a failed step.
function zmlMake(ctx, p) {
  const Z = zdMake(ctx, p)
  const {
    A, KEY, OUT, SCRIPTS, DSH_EFFORT, LIMITS, FILL_SKILLS, profile, ARCH_DOC, ARCH_SUPPLEMENT, MODEL_WORK, PER_SONNET,
    PER_CONCEPT, DSH_PER_BATCH, REBUILD, LEVEL, BRIEF_EFFORT, DERIVE_EFFORT, REVIEW_EFFORT, REVIEW_ROUNDS, SKIP_FORMS,
    zAgent, answerLines, runJobs, psQuote, LEDGER,
  } = Z
  // The model stage's one step script: every deterministic chain runs there, one process per step.
  const MODEL_STEPS = p.modelScript || `${SCRIPTS}\\model-steps.ps1`
  const phase = 'Models'
  const tail = (text) => String(text || '').trim().slice(0, 400)
  const n = (v) => Number(v || 0)
  // A packed `a:b;c:d` list of pairs (#180).
  const pairs = (text, sep = ';') => String(text || '').split(sep).filter(Boolean).map((entry) => {
    const at = entry.indexOf(':')
    return at < 0 ? [entry, ''] : [entry.slice(0, at), entry.slice(at + 1)]
  })
  // A model the step lines name without its book key (`2|a`), as its full address (`Key/2|a`).
  const full = (at) => { const t = String(at || '').trim(); return !t || t.includes('/') ? t : `${KEY}/${t}` }
  const fullPairs = (text) => pairs(text).map(([at, v]) => [full(at), v])
  // One deterministic step: the model-steps.ps1 call a DeepSeek drive job runs and answers with its one JSON line.
  // A failed step is returned for the module to stop with; the owner's typed refusal stops the stage here.
  const modelStep = async (label, step, o = {}) => {
    const parts = [`pwsh -NoProfile -File "${MODEL_STEPS}"`, `-Step ${step}`, `-Out "${OUT}"`]
    if (o.work) parts.push(`-Work "${o.work}"`)
    if (o.models && o.models.length) parts.push(`-Models "${o.models.join(',')}"`)
    if (o.units && o.units.length) parts.push(`-Units "${o.units.map((u) => `${u.id}:${u.token}`).join(',')}"`)
    if (o.citationKey) parts.push(`-CitationKey "${o.citationKey}"`)
    if (o.need) parts.push(`-Need ${psQuote(o.need)}`)
    if (o.level) parts.push(`-Level ${o.level}`)
    if (o.rebuild) parts.push('-Rebuild')
    if (o.changed) parts.push('-Changed')
    if (o.take) parts.push(`-Take ${o.take}`)
    if (o.owner) parts.push(`-Owner ${psQuote(o.owner)}`)
    if (o.only && o.only.length) parts.push(`-Only "${o.only.join(',')}"`)
    if (o.skipForms && o.skipForms.length) parts.push(`-SkipForms "${o.skipForms.join(',')}"`)
    if (o.perConcept) parts.push(`-PerConcept ${o.perConcept}`)
    if (o.perSonnet) parts.push(`-PerSonnet ${o.perSonnet}`)
    if (o.limits) parts.push(o.limits)
    // The math variant's own instruction files: every compiler command of that variant takes them.
    for (const [flag, file] of Object.entries(profile.compilerDocs || {})) parts.push(`-${flag} "${OUT}\\${file}"`)
    if (o.tool) parts.push(`-Tool ${o.tool}`)
    const json = await ztDrive(ctx, { label, step, command: parts.join(' '), out: OUT, cwd: o.cwd || OUT, phase, ledger: LEDGER, deepseek: o.deepseek })
    const lock = ztLockRefusal(JSON.stringify(json || {})) || ((json && (json.code === Z.LOCK || json.error === Z.LOCK)) ? json : null)
    if (lock) throw ztLockStop(label, lock.refusal || lock)
    const gate = json && (json.gate_complete !== undefined || json.gate_open !== undefined)
      ? { complete: json.gate_complete === true, counts: { open: n(json.gate_open) } }
      : (json && json.gate ? json.gate : null)
    if (!json || json.ok !== true) return { ok: false, error: tail((json && json.error) || `the ${step} step failed`), gate, raw: json }
    return { ...json, gate }
  }
  const PLANS = {
    type: 'object',
    properties: {
      plans: { type: 'array', items: { type: 'object', properties: {
        model: { type: 'string' }, status: { type: 'string', enum: ['planned', 'skipped', 'failed'] }, level: { type: 'string' },
        parts: { type: 'number' }, derivations: { type: 'number' }, chains: { type: 'number' },
        notes: { type: 'array', items: { type: 'string' } }, summary: { type: 'string' },
      }, required: ['model', 'status'] } },
    },
    required: ['plans'],
  }
  const slug = (address) => String(address).replace(/[\/|]/g, '_')
  const managerFolders = []
  let dshJobs = 0
  // The block's start step: release the claims a stopped run left, list the Model Cards, claim up to `take`
  // ready units, and answer the block's architect batches as `concept:<addresses>|formal:<addresses>` (#180: the
  // batching is the step's, because a module reads no listing row).
  const start = async (o = {}) => modelStep(`find models: block ${o.block || 1}`, 'start', {
    citationKey: KEY, take: o.take || 8, owner: o.owner || 'models', cwd: o.cwd, deepseek: o.deepseek,
    only: A.only, skipForms: SKIP_FORMS, perConcept: PER_CONCEPT, perSonnet: PER_SONNET, level: LEVEL, rebuild: REBUILD,
  })
  // One batch of up to eight models: brief -> the Sonnet architect's plan -> fill -> derive -> review and
  // split rounds -> the close rounds -> the report. Every command is one step; every job round is one or more
  // Haiku managers reading the step's job file.
  const compileBatch = async (batch, o = {}) => {
    const addresses = batch.models
    const name = `m-${slug(addresses[0])}`
    const dsh = `${MODEL_WORK}\\${name}`
    managerFolders.push(dsh)
    const tag = `${addresses[0]}${addresses.length > 1 ? ` +${addresses.length - 1}` : ''}`
    const sessions = new Map()
    const table = () => [...sessions].map(([id, session]) => `${id} ${session}`).join('\n')
    const step = (what, s, more = {}) => modelStep(`${what}: ${tag}`, s, { work: dsh, cwd: dsh, ...more, deepseek: o.deepseek })
    // The step's content jobs: one or more Haiku managers, at most eight jobs each. A job that names
    // "continue" reuses the conversation its earlier job started (the table of this batch's own jobs).
    const content = async (what, file, count, effort) => {
      const total = Number(count || 0)
      if (!total) return []
      const results = await runJobs(`${what}: ${tag}`, file, total, {
        what: 'model compiler', dsh, writes: dsh, skills: FILL_SKILLS, effort: effort || DSH_EFFORT, phase, run: true, keep: true, table: table(),
      })
      for (const r of results) { dshJobs++; if (r && r.id && r.session_id) sessions.set(r.id, r.session_id) }
      return results
    }
    const failed = (why) => addresses.map((model) => ({ model, status: 'failed', issues: [why] }))
    // 1. Brief: the script writes each context pack; DeepSeek reads the source and writes brief.md.
    const brief = await step('brief', 'brief', { models: addresses, rebuild: REBUILD, level: LEVEL, limits: LIMITS })
    if (!brief.ok) return failed(`brief failed: ${brief.error}`)
    if (Number(brief.jobs || 0)) await content('brief', brief.file, brief.jobs, BRIEF_EFFORT)
    const skipped = new Set((Array.isArray(brief.skipped) ? brief.skipped : String(brief.skipped || '').split(',')).map(full).filter(Boolean))
    const todo = addresses.filter((at) => !skipped.has(at))
    if (!todo.length) return addresses.map((model) => ({ model, status: 'skipped' }))
    // 2. Plan: Sonnet reads the briefs and writes plan.json for each model; nothing else.
    const levelOf = (at) => (LEVEL || (batch.kind === 'concept' ? 'concept' : 'formal'))
    const entry = (at, i) => `## Model ${i + 1}
MODEL: ${at}
DIR: ${dsh}\\${slug(at)}
LEVEL NOW: ${levelOf(at)}`
    const planned = await zAgent(
      `You are the Sonnet architect in the Models stage of the ZET digest of 《${A.bookTitle || KEY}》. Read your instructions first: ${ARCH_DOC}${ARCH_SUPPLEMENT ? `, then ${profile.architectSupplement.name || 'the supplement'} ${ARCH_SUPPLEMENT}` : ''} (Read tool). Then follow them for each model in turn. These models are near each other in the book: where one part serves two of them, plan it in the first and take it in from the second (\`<model>#<id>\`). Each model's own brief is \`brief.md\` in its DIR; read it for the book's wording and the model's title.
COMPILER: ${Z.tool('model-compiler')}
WORK: ${dsh}
LIMITS: ${LIMITS}${REBUILD ? '\nREBUILD: rebuild each model even if it already has parts.' : ''}${LEVEL ? `\nLEVEL: ${LEVEL}` : ''}

${todo.map(entry).join('\n\n')}

Return plans: one entry per model, in this order.`,
      { label: `plan: ${tag}`, phase, model: 'sonnet', schema: PLANS },
    )
    // 3. Fill: plan-status, the batch's model-deps references, the defer of the models they release, and the
    // fill, in ONE step. S10: the plans as the architect wrote them (plan.json per model), read by the compiler;
    // the answer only adds notes.
    const fill = await step('fill', 'fill', { models: todo, limits: LIMITS })
    if (!fill.ok) return failed(`fill failed: ${fill.error}`)
    const said = (planned && planned.plans) || []
    const statusOf = new Map(fullPairs(fill.statuses))
    const plans = todo.map((model) => ({ ...(said.find((y) => y.model === model) || {}), model, status: statusOf.get(model) || 'failed', level: levelOf(model) }))
    const deferred = new Map(fullPairs(fill.deferred).map(([at, why]) => [at, why || 'refused']))
    if (deferred.size) log(`${tag}: ${[...deferred].map(([at, why]) => `${at} ${why === 'waiting' ? 'waits for the model it takes a part from' : 'has a reference the ledger refused'}`).join('; ')}.`)
    const outcome = (at) => ({ model: at, status: deferred.get(at), issues: [deferred.get(at) === 'waiting' ? 'waits for the model it takes a part from' : 'a reference to another model\'s part was refused'] })
    if (!plans.some((x) => x.status === 'planned' && !deferred.has(x.model))) {
      log(`${tag}: no model of the batch is planned; the batch builds nothing.`)
      return addresses.map((m) => skipped.has(m) ? { model: m, status: 'skipped' }
        : deferred.has(m) ? outcome(m)
          : { model: m, status: 'failed' })
    }
    // A composer that needs more parts for one part asks through its answer; the split step takes the need.
    const splitNeeds = async (results) => {
      const need = answerLines({ results }).filter((l) => l.status === 'split' && l.address && l.need > 1).map((l) => `${l.address}=${l.need}`)
      if (!need.length) return
      const s = await step('split', 'split', { need: need.join(';') })
      if (!s.ok) { log(`split needs ${tag}: ${s.error}`); return }
      if (Number(s.jobs || 0)) await content('split', s.file, s.jobs, DERIVE_EFFORT)
    }
    // 4. The fill, 5. derive and 6. review (only where the plans have Derivation parts).
    const written = await content('fill', fill.file, fill.jobs, DSH_EFFORT)
    await splitNeeds(written)
    if (fill.derive) {
      const derive = await step('derive', 'derive')
      if (!derive.ok) return failed(`derive failed: ${derive.error}`)
      await splitNeeds(await content('derive', derive.file, derive.jobs, DERIVE_EFFORT))
      for (let round = 0; round < REVIEW_ROUNDS; round++) {
        const review = await step(`review ${round + 1}`, 'review', { changed: round > 0 })
        if (!review.ok) return failed(`review failed: ${review.error}`)
        if (!Number(review.jobs || 0)) break
        const verdicts = answerLines({ results: await content(`review ${round + 1}`, review.file, review.jobs, REVIEW_EFFORT) })
          .reduce((s, l) => ({ split: s.split + (Number(l.split) || 0), fix: s.fix + (Number(l.fix) || 0) }), { split: 0, fix: 0 })
        log(`review ${round + 1} ${tag}: ${verdicts.split} parts to split, ${verdicts.fix} to fix`)
        if (!verdicts.split && !verdicts.fix) break
        const split = await step(`split ${round + 1}`, 'split')
        if (!split.ok) return failed(`split failed: ${split.error}`)
        await splitNeeds(await content(`split ${round + 1}`, split.file, split.jobs, DERIVE_EFFORT))
      }
    }
    // 7. Close: the build faults the compiler still sees go back once, as 修正 notes, to the conversations that
    // wrote those Cards; the round that sends nothing back also writes the batch report (-Report). At most
    // CLOSE_ROUNDS rounds: a writer can answer a note and leave the fault in place.
    const CLOSE_ROUNDS = 3
    let reported = null
    for (let round = 0; round < CLOSE_ROUNDS; round++) {
      const close = await step(round ? `close ${round + 1}` : 'close', 'close')
      if (!close.ok) return failed(`close failed: ${close.error}`)
      // #180: the close round's report is its models' packed statuses, never their rows.
      if (Number(close.report_models || 0)) reported = new Map(fullPairs(close.statuses))
      if (!close.faults) break
      log(`close ${round + 1} ${tag}: ${close.faults} build fault(s) sent back in ${Number(close.jobs || 0)} follow-up(s)${Number(close.unowned || 0) ? `, ${close.unowned} in Cards no job of this batch wrote` : ''}`)
      if (!Number(close.jobs || 0)) break
      await content(`close ${round + 1}`, close.file, close.jobs, DERIVE_EFFORT)
    }
    // 8. Report, when the last close round sent follow-ups and wrote none.
    let statuses = reported
    if (!statuses) {
      const report = await step('report', 'report')
      if (!report.ok) return failed(`report failed: ${report.error}`)
      statuses = new Map(fullPairs(report.statuses))
    }
    return addresses.map((m) => {
      if (skipped.has(m)) return { model: m, status: 'skipped' }
      if (deferred.has(m)) return outcome(m)
      if (!statuses.has(m)) return { model: m, status: 'failed', issues: ['no report from the compiler'] }
      return { model: m, status: statuses.get(m), level: levelOf(m) }
    })
  }
  // The block's models, in the batches the start step planned: a model whose level the run changes is never
  // skipped; a form the profile skips (math: theory) never goes in; a model already built with no issue is left
  // out. The batches travel in the start line's `plan` (#180).
  const elaborate = async (started, label, o = {}) => {
    const plan = String((started && started.plan) || '')
    const batches = plan.split(';').filter(Boolean).map((entry) => {
      const at = entry.indexOf(':')
      return { kind: entry.slice(0, at), models: entry.slice(at + 1).split(',').filter(Boolean).map(full) }
    }).filter((b) => b.models.length)
    if (!batches.length) {
      log(`${label}: no model of the block is planned; the block builds nothing.`)
      return []
    }
    log(`${label}: ${batches.reduce((sum, b) => sum + b.models.length, 0)} models in ${batches.length} batches, one Sonnet architect each; up to ${DSH_PER_BATCH} DeepSeek runs at a time per batch.`)
    const results = (await parallel(batches.map((b) => () => compileBatch(b, o).catch((error) => { log(`batch ${b.models[0]}: ${error}`); return null })))).flatMap((r, i) => r || batches[i].models.map((model) => ({ model, status: 'failed' })))
    return results
  }
  // One block's checked completions: the finish step runs each held unit's own verifier in ONE command and
  // records a unit it cannot finish against that unit alone. The ledger stays the judge.
  const finish = async (o = {}) => {
    const r = await modelStep('models: finish', 'finish', { ...o, deepseek: o.deepseek })
    if (!r.ok) return { ok: false, error: r.error, done: 0, failed: 0, partial: 0, gate: r.gate }
    return { ok: true, done: n(r.done), failed: n(r.failed), partial: n(r.partial), gate: r.gate }
  }
  // #180: the stage's log from the batch results (a status each), never a per-model row.
  const logModels = (reports) => {
    const done = reports.filter((r) => r.status === 'done')
    log(`Models: ${done.length} structured, ${reports.filter((r) => r.status === 'skipped').length} skipped, failed ${reports.filter((r) => r.status === 'failed').map((r) => r.model).join(', ') || 'none'}; ${done.filter((r) => r.level === 'concept').length} concept, ${done.filter((r) => r.level !== 'concept').length} formal; ${dshJobs} DeepSeek jobs.`)
  }
  // R323: the model stage no longer deletes conversations or folders itself. Every batch conversation was
  // recorded when the provider returned its id, so the run's cleanup stage deletes exactly those ids after
  // the owning work settles; a batch folder is disposable scratch only when its accepted plan is retained
  // first (R328/R329). This resolves that the sweep owes nothing.
  const modelCleanup = async () => {
    if (!managerFolders.length) return null
    log(`Model clean-up: ${managerFolders.length} batch folders stay until the run-level cleanup settles their recorded conversations.`)
    return { late_deleted: 0, folders: managerFolders.length, deferred: true }
  }

  return { Z, start, elaborate, finish, logModels, modelCleanup, dshJobs: () => dshJobs }
}

defineModule({
  id: 'zt-model-lib', version: '13', library: true, requires: ['zt-digest-lib'],
  summary: 'Library: the model layer\'s model-steps.ps1 steps (list, claim and batch; brief, fill, derive, review, split, close, report, finish), the Sonnet architect, and the model compiler\'s report and clean-up.',
})
// ==== end module zt-model-lib ====

// ==== module zt-explain v9 ====
// Explanations of finished Derivation parts (R219/R301/R302): the ledger's ready `explain/<model-key>@1` work,
// which a checked Model completion registers. The layer driver runs the stage as script steps: `start` claims a
// block of ready explanation units and runs the explanation plan (one job per chain), the block's jobs run in
// Haiku managers, `apply` writes 思路/说明/解读 through `explain-apply` (which refuses a changed 起点/step/结果 and
// lists one fix job per refused part, run in that part's own conversation), and `finish` reports and records
// every unit through `explain-finish`, which re-reads the parts and refuses a part that still lacks text. Every
// step is one `layer-steps.ps1` call run by a DeepSeek drive job: no Claude agent relays a command and no Claude
// agent is spent per model or per finish. The block's conversations stay open across its two content rounds and
// are deleted by the run cleanup. Runs in mode explain and after the model layer in mode full.
// 6: the chain ends with the block's checked finishes in one command (`zt-digest finish-batch`), run by the chain's own
// manager; a unit its line does not answer is finished by one relay, then on its own (docs/digest-claude-token-proposal.md P1/P2).
// 7 (S8): the claimed models, whatever args.only says (the two branches were identical).
// 8 (#175): the block is three script steps and two content rounds, run by DeepSeek drive jobs under Haiku managers.
// 9 (#180): a step answers counts (`plan_parts`, `refused`, `done`), so the stage reports the parts it planned, the
// fixes it refused and how many models were verified; the per-unit outcomes are the ledger's run report.
defineModule({
  id: 'zt-explain', version: '9', requires: ['zt-model-lib'],
  summary: 'ZT digest: the ledger\'s ready explanations of finished Derivation parts, written in two DeepSeek content rounds and recorded by explain-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmlMake(ctx, p)
    const Z = M.Z
    const A = Z.A
    const work = `${Z.OUT}\\explain`
    const tally = { parts: 0, refused: 0 }
    const result = await Z.layerRun('explain', {
      // The block's conversations stay open for the second content round; the run cleanup deletes them.
      layer: 'explain', take: 4, rounds: Z.MAX_ATTEMPTS, phase: 'Models', jobs: true, keep: true,
      dsh: work, writes: `${work}\\out`, skills: Z.FILL_SKILLS, effort: Z.DSH_EFFORT, what: 'explanation writers',
      limits: Z.LIMITS, maxParts: A.explainMax || 0, apply: true,
      onStart: (started) => { tally.parts += Number(started.plan_parts || 0) },
      onApply: (applied) => { tally.refused += Number(applied.refused || 0) },
      onFinish: (finished) => { if (Number(finished.mid_report_models || 0)) log(`Explain: ${finished.mid_report_models} models reported.`) },
    })
    const verified = Number(result.verified || 0)
    const failed = Number(result.failed || 0)
    log(`Explain: ${tally.parts} Derivation part(s) planned; ${tally.refused} refused; ${result.status} — ${verified} models checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    if (result.status === 'failed') return { status: 'failed', error: result.error, verified, failed, parts: tally.parts, refused: tally.refused, gate: result.gate }
    return { status: result.status, verified, failed, parts: tally.parts, refused: tally.refused, gate: result.gate }
  },
})
// ==== end module zt-explain ====

// ==== module zt-figures v6 ====
// The book's figures in its Cards: attach-figures puts every figure a Card names but does not show yet into that
// Card, and links the book's documents from the Literature Card. It runs after Organize, which rewrites bodies.
// 6 (#176, docs/workflow-module-structure-proposal.md): the figure pass and the stage's own publication are two
// book-steps.ps1 steps run by DeepSeek drive jobs; no Claude relay runs a command.
defineModule({
  id: 'zt-figures', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest: show the book\'s own figures in the Cards that name them (one book-steps.ps1 step).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('figures', ['figures'])) return { status: 'done', reused: true }
    const A = Z.A
    const options = { CitationKey: Z.KEY }
    if (A.fileId) options.FileId = A.fileId
    const summary = await Z.bookStep('show figures', 'figures', options)
    log(summary && summary.ok === true ? `Figures: ${summary.shown} figures shown in ${summary.cards} Cards (${summary.figures} kept, ${summary.labelled} labelled); documents ${summary.literature ? 'linked' : 'already linked'}.` : `Figures failed: ${(summary && summary.error) || 'no output'}`)
    if (!summary || summary.ok !== true) return { status: 'failed', error: (summary && summary.error) || 'no output' }
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts
    // by hash and checks the coverage it derives.
    const publication = await Z.bookStep('figures: publish', 'publish', { Layer: 'figures', Report: `${Z.OUT}\\figures-report.json` })
    log(publication && publication.state ? `figures: publication ${publication.unit || ''} ${publication.state}${publication.disposition ? ` (${publication.disposition})` : ''}.` : `figures: the publication failed: ${(publication && publication.error) || 'no output'}.`)
    return { status: 'done', ...summary, ok: undefined, publication }
  },
})
// ==== end module zt-figures ====

// ==== module zt-term-link-lib v4 ====
// The deterministic Term-link pass of one book's finalization round (S24, #167; S26, #169), shared by the finite
// finalization. It owns no content decision: one book-steps.ps1 step (`term-link`) freezes the round's plan, claims
// the pass's ledger unit, applies the exact plan under the claim, completes the unit only from the compiler's own
// apply report and plans again, so the report shows the second pass's changes. A finalization correction round is
// `termlink/final@R` with the round's changed Cards, or the whole book when the round changed the Term mapping
// (R0439), so a later correction can never be suppressed by a reusable unit (R0438/R0439).
//
// The ledger is the scheduler and the truth. A pass whose Term-card book unit or Card writers are not done has no
// ready unit: the step answers `skipped` with the ledger's own gate, so audit and report still run and show the
// coverage debt, and a failed or partial publication can never be represented as a linked book (R0428). A stopped or
// incomplete apply records its fault on the unit inside the step and leaves it incomplete; the run's report shows it,
// and a resume retries only that unit.
//
// 3 (#176, docs/workflow-module-structure-proposal.md): the pass is one DeepSeek drive job that runs one
// book-steps.ps1 step and answers its one JSON line. The relay-era whole-book prefix path leaves with the relay: the
// whole-book pass is `zt-term-link`'s term-steps.ps1 step; this library serves the finalization round alone.
const ZT_TERM_LINK_SCRIPT = 'E:\\zet\\scripts\\book-digest\\book-steps.ps1'
async function ztTermLinkPass(ctx, p) {
  const OUT = String(p.out).replace(/[\\/]+$/, '')
  const WORK = `${OUT}\\term-link`
  const phase = ctx.phase
  const tag = p.stage || ctx.stage
  const tail = (text) => String(text || '').trim().slice(0, 400)
  const cards = Array.isArray(p.cards) ? p.cards.map(String).filter(Boolean) : []
  const round = Number.isFinite(Number(p.round)) && Number(p.round) > 0 ? Number(p.round) : null
  const report = { status: 'done', book: p.key, work: WORK, unit: null, pass: 'final', round, scope: cards.length ? 'cards' : 'book', cards: cards.length, plan: null, counts: null, no_op: false, unresolved: [], staging: WORK }
  if (round === null && cards.length) {
    report.status = 'skipped'
    report.reason = 'a diagnostic -Cards subset without a round owns no ledger unit; run the stage without -Cards for the book pass'
    return report
  }
  const options = { Round: round, Occurrences: p.occurrences || 'first' }
  if (cards.length) options.Cards = cards
  const deepseek = p.deepseek || { deleted: 0, not_deleted: [] }
  const command = `pwsh -NoProfile -File "${p.script || ZT_TERM_LINK_SCRIPT}" -Step term-link -Out "${OUT}" -Json '${JSON.stringify(options).replace(/'/g, "''")}'`
  let json
  try {
    json = await ztDrive(ctx, { label: `${tag}: term-link round`, step: 'term-link', command, out: OUT, cwd: WORK, phase, ledger: 'zt-digest ledger', deepseek })
  } catch (error) {
    report.status = 'failed'
    report.error = tail((error && error.message) || 'the Term-link pass failed')
    return report
  }
  // #180: the step answers counts, so the round's plan and application facts are flat fields.
  const n = (v) => Number(v || 0)
  report.plan = json.plan_revision !== undefined ? { revision: n(json.plan_revision) } : null
  report.unit = json.unit || null
  report.counts = { cards_changed: n(json.cards_changed), links_added: n(json.links_added) }
  report.no_op = json.no_op === true
  report.completion = { state: json.complete === true ? 'done' : null }
  report.second = { cards_changed: n(json.second_cards_changed), cards_refused: n(json.second_cards_refused) }
  if (json.status === 'done') {
    report.status = 'done'
    log(`Term link: ${report.unit || 'the round'} complete — ${report.counts.cards_changed} Card(s) changed, ${report.counts.links_added} link(s) added${report.no_op ? ', proven no-op' : ''}.`)
    return report
  }
  if (json.status === 'skipped') {
    report.status = 'skipped'
    report.reason = json.reason || 'the Term-link unit is not ready'
    log(`Term link: skipped — ${report.reason}.`)
    return report
  }
  report.status = 'failed'
  report.code = json.code || null
  report.error = tail(json.error || 'the Term-link pass did not complete')
  if (json.complete !== true) report.state = 'incomplete'
  log(`Term link: failed — ${report.error}.`)
  return report
}

defineModule({
  id: 'zt-term-link-lib', version: '4', library: true, requires: ['zt-workflow-lib'],
  summary: 'Library: the finalization\'s deterministic Term-link round (one book-steps.ps1 step: plan, claim, apply, complete, second plan) as one DeepSeek drive job.',
})
// ==== end module zt-term-link-lib ====

// ==== module zt-finalize v5 ====
// The declared finite finalization branch (R299/R300, #86; S26, #169): the read-only audit plan, at
// most ONE correction cycle scoped to the plan's Cards, the exact changed-Card/touched-tree
// manifest, a Term-link round for the Cards that cycle actually changed (the whole book when the
// correction changed the Term mapping), the dependency-scoped refresh of the affected publications
// this branch re-derives, and the final read-only audit published at the new graph revision. The
// branch is a declared sequence, never a loop: the plan refuses a second cycle, and a stage-owned
// publication names its own stage instead of being republished from an artifact this branch did not
// re-derive. A residual hard fault leaves the result partial, with the fault named, and never
// republishes the audit that scheduled it.
//
// 3 (S26, #169): after `scope`, the round named in the immutable manifest relinks its Cards (the
// shared `zt-term-link-lib` pass, owned by `termlink/final@R`) before the final audit and the
// mathematics check, so a correction that dropped a Term link never becomes the book's final text
// (R0438/R0439/R0440). A resume that finds the correction cycle already recorded but the final
// audit not yet published continues from the retained manifest (`zt-digest finalize -Step
// changes`) instead of re-entering the one permitted cycle or skipping the relink.
// 4 (#176, docs/workflow-module-structure-proposal.md): the plan, the retained manifest, the correction cycle's
// steps, the scope, the refreshes and the final audit are book-steps.ps1 steps run by DeepSeek drive jobs, and the
// Term-link round is zt-term-link-lib's one drive job. No Claude agent runs a command.
defineModule({
  id: 'zt-finalize', version: '5', requires: ['zt-digest-lib', 'zt-term-link-lib'],
  summary: 'ZT digest: the finite finalization (audit plan, one scoped correction cycle, the changed-Card manifest, the round\'s Term-link pass, dependency-scoped publication refresh, final read-only audit).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const n = (v) => Number(v || 0)
    let plan = null
    let cycle = null
    let scope = null
    let recovered = false
    if (await Z.settledOnResume('finalize', ['audit', 'link-check'])) {
      // A resume after the scheduling audit: the branch either already published its final audit (reuse it) or
      // stopped before the correction cycle/final audit (continue from the recorded manifest). Re-entering the cycle
      // would either be refused as spent or redo finished writes.
      const retained = await Z.bookStep('finalize changes', 'finalize-changes')
      if (retained && retained.ok === true && retained.finalized === true) {
        log('Finalize: the branch already published its final audit; the resume reuses it.')
        return { status: 'done', reused: true }
      }
      if (retained && retained.ok === true && retained.term_applicable !== undefined) {
        scope = retained
        recovered = true
        log(`Finalize: the resume continues the recorded correction cycle from ${scope.changes}; it does not re-enter the cycle.`)
      }
    }
    if (!recovered) {
      const planned = await Z.bookStep('finalize plan', 'finalize-plan')
      if (!planned || planned.ok !== true || !planned.audit_unit) return { status: 'failed', error: (planned && planned.error) || 'the audit plan was not derived' }
      plan = planned
      log(`Finalize: audit ${plan.audit_unit} at graph revision ${n(plan.graph_revision)}; ${n(plan.faults)} faults (${n(plan.fix)} correctable, ${n(plan.conflict)} conflicting, ${n(plan.blocked)} blocked, ${n(plan.advisory)} advisory); branch ${plan.branch}.`)
      if (plan.branch === 'fix') {
        const work = `${Z.OUT}\\finalize\\fix`
        const cards = plan.cycle_card_list || ''
        const rules = String(plan.cycle_rules || '').split(',').filter(Boolean).join(',')
        cycle = await Z.cardCycle('final correction', work, { cards, rules, fixRounds: 1 })
        if (cycle.status !== 'done') {
          return { status: 'partial', plan, cycle, error: `the one correction cycle did not complete: ${cycle.error || 'no report'}` }
        }
        // The changed-Card manifest is recorded once, with the round's Term-link scope.
        const scoped = await Z.bookStep('finalize scope', 'finalize-scope', { FixWork: work })
        if (!scoped || scoped.ok !== true) return { status: 'partial', plan, cycle, error: (scoped && scoped.error) || 'the changed-Card manifest was not recorded' }
        scope = scoped
      }
    }
    const refreshed = []
    // #180: the manifest step answers the stages to refresh and the stage-owned ones as two packed name lists.
    const cycleReport = scope && scope.cycle_report ? String(scope.cycle_report) : ''
    for (const layer of String((scope && scope.refresh) || '').split(',').filter(Boolean)) {
      if (!cycleReport) {
        log(`Finalize: the ${layer} refresh has no retained cycle report to read; its own stage owns that content.`)
        continue
      }
      const done = await Z.bookStep(`finalize refresh ${layer}`, 'finalize-refresh', { Layer: layer, Report: cycleReport })
      refreshed.push(done && done.ok === true
        ? { layer, unit: done.unit, revision: done.revision, state: done.state, changed_cards: done.changed_cards }
        : { layer, error: (done && done.error) || 'the refresh did not complete' })
      if (!done || done.ok !== true) log(`Finalize: the ${layer} refresh did not complete: ${(done && done.error) || 'no output'}.`)
    }
    const owned = String((scope && scope.owned) || '').split(',').filter(Boolean)
    if (owned.length) log(`Finalize: ${owned.join(', ')} keep their own stage as the owner of that refresh; the changed-Card manifest is recorded for it.`)

    // S26 (#169): the round named by the immutable manifest relinks the Cards the correction actually changed (or the
    // whole book when it changed the Term mapping) before the final audit.
    let termLink = null
    const round = scope && scope.term_applicable !== undefined ? { applicable: scope.term_applicable, scope: scope.term_scope, round: scope.term_round, cards: n(scope.term_cards), reason: scope.term_reason } : null
    if (round && round.applicable === true && round.scope !== 'none') {
      const occurrences = (Z.A && Z.A.termLinkOccurrences) || 'first'
      termLink = await ztTermLinkPass(ctx, {
        out: Z.OUT,
        key: Z.KEY,
        args: Z.A,
        mode: p.mode,
        occurrences,
        cards: null,
        round: round.round,
        tool: 'zt-digest',
        stage: 'zt-finalize',
      })
      log(`Finalize: Term-link round ${round.round} (${round.scope}) is ${termLink.status}${termLink.unit ? ` (${termLink.unit})` : ''}${termLink.error || termLink.reason ? `: ${termLink.error || termLink.reason}` : ''}.`)
      if (termLink.status !== 'done') log('Finalize: the final audit still runs; the run reports the Term coverage debt.')
    } else if (round) {
      log(`Finalize: no Term-link round: ${round.reason || round.scope}.`)
    }

    const report = await Z.bookStep('finalize final', 'finalize-final')
    if (!report || report.ok !== true) return { status: 'failed', plan, cycle, scope, refreshed, term_link: termLink, error: (report && report.error) || 'the final audit was not published' }
    // #180: the final step answers counts; the report's nested facts are rebuilt here from them.
    const final = {
      status: report.status, branch: report.branch, graph_revision: n(report.graph_revision),
      audit: { final: { unit: report.audit_unit, revision: n(report.audit_revision) } },
      faults: { residual: n(report.residual) },
      infrastructure: [],
      terms: { status: report.terms_status, published: n(report.terms_published), rows: n(report.terms_rows), heads: n(report.terms_heads), uses: n(report.terms_uses), faults: n(report.terms_faults), complete: report.terms_complete === true },
      term_link: { applicable: report.term_applicable !== false, round: n(report.term_round), scope: report.term_scope, status: report.term_status, unit: report.term_unit, state: report.term_state, no_op: report.term_no_op === true, counts: { cards_changed: n(report.term_cards_changed), links_added: n(report.term_links_added) }, reason: report.term_reason },
    }
    log(`Finalize: ${final.status} at graph revision ${final.graph_revision}; final audit ${final.audit.final.unit} (revision ${final.audit.final.revision}); ${final.faults.residual} residual hard/source faults; terms ${final.terms.status || 'unknown'}.`)
    return {
      status: final.status === 'done' ? 'done' : 'partial',
      branch: plan ? plan.branch : final.branch,
      plan,
      cycle,
      scope,
      recovered,
      refreshed,
      term_link: termLink,
      final,
    }
  },
})
// ==== end module zt-finalize ====

// ==== module zt-finding-layer v6 ====
// The empirical-results layer of the economics set (docs/finding-cards-design.md): the ledger's ready finding
// work, worked while the digest groups are still running. `finding-compiler` plans one job per few groups from
// the sentences that report data with a number and each group's own Idea Cards; DeepSeek writes one Finding
// Card per result as JSON and runs the job's SELF-CHECK; the compiler creates each Finding Card beside its
// group's anchor and links it from every supported Idea Card with the kind word 印证. The layer driver runs the
// stage as script steps: `start` claims a block of ready `findings/gNNN@1` units and runs the plan, the block's
// jobs run in Haiku managers, and `finish` applies, reports and records each group through `findings-finish`, so
// an unapplied job leaves the group open. Every step is one `layer-steps.ps1` call run by a DeepSeek drive job:
// no Claude agent relays a command and no Claude agent is spent per group or per finish. Needs zt 0.20.0 (the
// Card type finding).
// 4: the chain ends with the block's checked finishes in one command (`zt-digest finish-batch`), run by the chain's own
// manager; a unit its line does not answer is finished by one relay, then on its own (docs/digest-claude-token-proposal.md P1/P2).
// 5 (#175): the whole block is two script steps and one content round; the relay and the chain are gone.
// 6 (#180): a step answers counts, so the stage reports the block's created/linked totals and how many group
// units were verified; the per-unit outcomes are the ledger's run report.
defineModule({
  id: 'zt-finding-layer', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest empirical-results layer: the ledger\'s ready Finding work, plan → DeepSeek JSON → apply → per-group checked completion, in two script steps run by DeepSeek drive jobs.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\findings`
    const result = await Z.layerRun('findings', {
      layer: 'findings', take: 4, rounds: Math.max(3, Z.MAX_ATTEMPTS), wait: 600, phase: 'Findings',
      dsh: work, writes: `${work}\\out`, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, what: 'finding writers',
      onStart: (started) => {
        log(`Findings: ${started.plan_candidates || 0} result sentences in ${started.plan_groups || 0} groups, ${started.jobs || 0} DeepSeek jobs.`)
      },
      onFinish: (finished) => {
        log(`Findings: ${finished.mid_applied_created || 0} Finding Cards made, ${finished.mid_applied_linked || 0} Links, ${finished.mid_applied_refused || 0} refused.`)
      },
    })
    const t = result.totals || {}
    const applied = { created: Number(t.mid_applied_created || 0), linked: Number(t.mid_applied_linked || 0), refused: Number(t.mid_applied_refused || 0) }
    const verified = Number(result.verified || 0)
    const failed = Number(result.failed || 0)
    log(`Findings: ${result.status} — ${verified} groups checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    if (result.status === 'failed') return { status: 'failed', error: result.error, verified, failed, applied, gate: result.gate }
    return { status: result.status, verified, failed, applied, gate: result.gate }
  },
})
// ==== end module zt-finding-layer ====

// ==== module zt-group-cycle v8 ====
// The Digest stage of a managed run, in blocks. One layer driver runs the stage as script steps: `start`
// claims a block of up to eight `digest/gNNN@1` units and runs each group's `group-start` (the snapshot is
// generated before any usable job manifest exists, R262), the block's DeepSeek writer jobs run in Haiku
// managers (at most eight per manager), and `finish` records each held unit's checked completion with
// `group-finish` (R250/R251/R253), faults the units whose draft is not ready and runs the layer gate. Every
// step is one `layer-steps.ps1` call run by a DeepSeek drive job, so no Claude agent relays a command and no
// Claude agent is spent per group. A unit that fails is recorded against its own attempt and claimed again by
// a later block; the stage is never rerun as a whole (R258).
// #92: the claim requests a lease that covers the frozen provider-run cap.
// #87 (R280): `group-start` reconciles a retained accepted plan against the archive receipts; a recovered
// group's job declares `expected_jobs: 0`, runs no DeepSeek job, and completes from the committed result.
// 8 (#180): a step answers counts, so the stage reports how many group units were verified and how many were
// not; the per-unit outcomes are the ledger's run report, never a module's array.
defineModule({
  id: 'zt-group-cycle', version: '8', requires: ['zt-digest-lib'],
  summary: 'ZT digest: verified groups in blocks of up to eight per manager, at most eight managers, claimed from the ledger ready work, recovering a group whose accepted plan already committed.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\group-jobs`
    const groups = Math.max(1, Number(p.groups) || 1)
    const deepseek = { deleted: 0, not_deleted: [] }
    const result = await Z.layerRun('digest', {
      layer: 'digest', take: Z.PER_MANAGER, rounds: Z.MAX_ATTEMPTS, phase: 'Digest',
      dsh: work, writes: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, what: 'digest group writers', deepseek,
    })
    const verified = Number(result.verified || 0)
    const failed = Number(result.failed || 0)
    if (result.status === 'failed') {
      log(`Digest: the layer stopped: ${result.error}`)
      return { status: 'failed', error: result.error, verified, failed, claimed: result.claimed, groups, deepseek }
    }
    log(`Digest: ${verified} group(s) verified, ${failed} not verified, in ${result.claimed} claimed unit(s) of ${groups}, ${Z.PER_MANAGER} per manager, all Haiku managers and DeepSeek jobs.`)
    return { status: failed ? 'partial' : 'done', verified, failed, claimed: result.claimed, groups, deepseek }
  },
})
// ==== end module zt-group-cycle ====

// ==== module zt-link-check v3 ====
// The Link check and the orphan pass on the whole book. In mode "full" the block cycles have already fixed the
// book, so the check runs every hard rule (the link pass can add R2, R4, and I2 faults); on an older book
// (mode "link") it keeps to L1 and L2 unless args.cardRules says otherwise.
// v2: the stage publishes its own `link-check` revision from its cycle report (`zt-digest finalize -Step stage`).
// The finalization refreshes that revision only when its one correction cycle changed a Card, so on
// acemoglu2025nobel trial 1 (nothing to correct) link-check/book@1 stayed ready and the run was never content-complete.
// v3 (#176, docs/workflow-module-structure-proposal.md): the card cycle is its declared book-steps.ps1 steps (plan
// with its jobs, one apply/check step per declared correction round, then apply and report) and the publication is
// one more step; each runs as a DeepSeek drive job, so no Claude relay runs a command.
defineModule({
  id: 'zt-link-check', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the card layer cycle on the new Links and the orphans after the link pass, published as the link-check revision.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\cards-link`
    const cycle = await Z.cardCycle('link check', work, { rules: p.mode === 'full' ? '' : (Z.A.cardRules || 'L1,L2') })
    if (!cycle || cycle.status !== 'done') return cycle
    const publication = await Z.bookStep('link-check: publish', 'publish', { Layer: 'link-check', Report: `${work}\\report.json` })
    log(publication && publication.state ? `link-check: publication ${publication.unit || ''} ${publication.state}${publication.disposition ? ` (${publication.disposition})` : ''}.` : `link-check: the publication failed: ${(publication && publication.error) || 'no output'}`)
    return { ...cycle, publication: publication || { layer: 'link-check', state: 'failed', error: 'no output' } }
  },
})
// ==== end module zt-link-check ====

// ==== module zt-link-pass v9 ====
// The Link pass over the receipt-owned gap set, in blocks. The layer driver runs the stage as script steps:
// `start` claims a block of up to eight `link/gNNN@1` units and runs each group's `link-start` (the frozen gap
// scope and the one job that writes the draft), the block's jobs run in Haiku managers, and `finish` records each
// checked completion with `link-finish` (R250/R251/R263). Every step is one `layer-steps.ps1` call run by a
// DeepSeek drive job, so no Claude agent relays a command and no Claude agent is spent per group.
// Only the gap set receives a Link: `link-finish` refuses a call on any other Card, and the frozen
// accepted plan keeps a lost reply recoverable and a changed body a conflict (D123, D124, AC08).
// A group whose gap set is empty still settles its unit, with no provider run at all (AC07):
// `link-start` declares expected_jobs 0 and the finish applies the empty checked draft.
// A group whose digest did not complete stays blocked and is passed through, never worked (#92).
// 8 (#175): the block is two script steps and one content round under Haiku managers; the relay-era block is gone.
// 9 (#180): a step answers the block's totals (gaps, linked, linked_from, already_linked, no_link, changed, L1/L2
// faults), never per-group rows; the per-group outcomes are the ledger's run report. The report carries the
// run's own totals and how many groups were scheduled, gap-free, verified and blocked.
defineModule({
  id: 'zt-link-pass', version: '9', requires: ['zt-digest-lib'],
  summary: 'ZT digest: verified Link units in blocks of up to eight per manager, for every scheduled or gap-free group, with the gap totals and L1/L2 quality recorded.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
    pending: { type: 'number', default: 0, description: 'the number of groups whose semantic gap set is not empty (from the link preparation)' },
    noGap: { type: 'number', default: 0, description: 'the number of groups whose semantic gap set is empty; their Link unit is settled without a provider run' },
    blocked: { type: 'number', default: 0, description: 'the number of groups the link preparation blocked because their digest did not complete' },
    gaps: { type: 'number', default: 0, description: 'the semantic gap count of the prepared gap set (from the link preparation)' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\link-jobs`
    const deepseek = { deleted: 0, not_deleted: [] }
    const result = await Z.layerRun('link', {
      layer: 'link', take: Z.PER_MANAGER, rounds: Z.MAX_ATTEMPTS, phase: 'Link',
      dsh: work, writes: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, what: 'Link writers', deepseek,
    })
    const t = result.totals || {}
    const quality = { rules: 'L1,L2', faults: Number(t.link_quality_faults || 0), checked: Number(t.link_quality_checked || 0) }
    const summary = {
      scheduled: Number(p.pending || 0), skipped: Number(p.noGap || 0),
      gap: Number(p.gaps || 0), skipped_cards: Number(t.link_skipped || 0),
      linked: Number(t.link_linked || 0), linked_from: Number(t.link_linked_from || 0),
      already_linked: Number(t.link_already_linked || 0), no_link: Number(t.link_no_link || 0),
      changed: Number(t.link_changed || 0), provider_runs: Number(t.link_scheduled || 0),
      verified: Number(result.verified || 0), failed: Number(result.failed || 0),
      blocked: Number(p.blocked || 0), quality,
    }
    log(`Link: ${summary.verified} of ${Number(p.pending || 0) + Number(p.noGap || 0)} group(s) verified; ${summary.gap} gaps, ${summary.skipped_cards} skipped Cards, ${summary.linked} written, ${summary.linked_from} linked from another Card, ${summary.no_link} none; ${summary.provider_runs} provider run(s); L1/L2 faults ${quality.faults}.`)
    if (summary.blocked) log(`Link: ${summary.blocked} group(s) stay blocked (their digest unit is not done).`)
    if (result.status === 'failed') return { status: 'failed', error: result.error, ...summary, deepseek }
    return { status: result.status, ...summary, deepseek }
  },
})
// ==== end module zt-link-pass ====

// ==== module zt-link-prepare v8 ====
// v6 (S1): one relay runs prepare-link -Brief and the module reads its line; the Haiku agent that copied the arrays
// back (and ran `zt up` itself) leaves: zt-digest starts and waits for the service.
// v7 (#176, docs/workflow-module-structure-proposal.md): the stage's one command is a book-steps.ps1 step
// (`link-prepare`) run by a DeepSeek drive job; no Claude relay runs a command.
// v8 (#180): the step answers counts of the prepared gap set, not its group arrays. The scheduling is the
// ledger's anyway (the Link units are claimed from it), so the stage reports how many groups are scheduled,
// gap-free and blocked, the gap total and the digest state; the group lists stay in the step's detail file.
// The Link pass preparation: prepare-link-pass computes each completed group's semantic gap set
// from its own write receipts (§5.5; D118-D124, #110). Only the gap set becomes a Link job; a Card
// that already carries a valid kinded Link outside its group is skipped and reported. A group whose
// digest unit is not `done` is not scheduled: its allow-list is empty (#92).
// Fails the stage when the step fails. No stage reads the account-wide usage ledger: run usage is recorded per
// provider run from that run's own result (R343-R346).
defineModule({
  id: 'zt-link-prepare', version: '8', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the receipt-owned semantic gap set of the Link pass (one book-steps.ps1 step); output the scheduled, gap-free and blocked group counts.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const line = await Z.bookStep('prepare link pass', 'link-prepare')
    if (!line || line.ok !== true || line.pending === undefined) {
      const error = (line && line.error) || 'no report'
      log(`Link preparation failed: ${error}`)
      throw new Error(`link preparation failed: ${error}`)
    }
    const n = (v) => Number(v || 0)
    const pending = n(line.pending)
    const noGap = n(line.no_gap)
    const blocked = n(line.blocked)
    const gapTotal = n(line.gap_total)
    log(`Link pass: ${gapTotal} semantic gap(s) on ${pending} of ${p.groups} groups; ${noGap} group(s) need no job; ${n(line.skipped_total)} owned Card(s) skipped.`)
    if (blocked) log(`Link pass: ${blocked} group(s) blocked because their digest did not complete. No link job runs for them.`)
    return {
      pending, no_gap: noGap, blocked, gaps: gapTotal,
      gap_total: gapTotal, skipped_total: n(line.skipped_total),
      owned_total: n(line.owned_total), unowned: n(line.unowned),
      scheduled: pending, skipped: noGap,
      digest_state: line.digest_state || null,
    }
  },
})
// ==== end module zt-link-prepare ====

// ==== module zt-model-layer v8 ====
// The model layer: the ledger's ready `models/gNNN-mIIII@1` work, released as the digest groups settle. Each
// claimed block of Model units is built by the existing model compiler chain (one Sonnet architect per batch,
// DeepSeek writers, the late clean-up sweep) and every unit is then recorded through `model-finish`, which
// re-derives the build from the compiler's own state and the authored Cards. A model the compiler could not
// finish stays open with its own fault, so the next attempt works only that unit (R235/R258). The layer gate
// is the stage's result.
// 6: the built units of a block are recorded through `model-finish` in ONE command (`zt-digest finish-batch`)
// instead of one relay per model (docs/digest-claude-token-proposal.md P1).
// 7 (#177, docs/workflow-module-structure-proposal.md): the layer is a straight line of model-steps.ps1 steps
// run by DeepSeek drive jobs. `start` releases a stopped run's claims, lists the Model Cards and claims a block
// of up to eight units; the block's models are built by brief, fill, derive, review, split and close steps with
// their content jobs; `finish` records the block's checked completions and reads the gate. No Claude agent
// relays a command and no JavaScript retries a step: a failed step stops the stage with its reason, per-unit
// outcomes are ledger data.
// 8 (#180): the block's units and batches are the start step's counts and packed plan, and its finished,
// failed and deferred models are counts; the layer logs and reports totals, never a per-unit row.
defineModule({
  id: 'zt-model-layer', version: '8', requires: ['zt-model-lib'],
  summary: 'ZT digest model layer: the ledger\'s ready Model units built by the model compiler chain and recorded by model-finish, as model-steps.ps1 steps run by DeepSeek drive jobs.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: "the run's mode ({{mode}})" },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
  },
  run: async (ctx, p) => {
    const M = zmlMake(ctx, p)
    const Z = M.Z
    const n = (v) => Number(v || 0)
    // A block of up to eight Model units (the protocol's eight per manager): with up to eight formal models per
    // architect batch, a book's models are planned together, their references to each other resolve in-batch,
    // and no second block waits for the first (grossman2002integration trials 16-17: two blocks of four).
    const TAKE = 8
    const ROUNDS = Z.MAX_ATTEMPTS
    const deepseek = { deleted: 0, not_deleted: [] }
    const totals = { verified: 0, failed: 0, deferred: 0, skipped: 0 }
    let claimed = 0
    let idle = 0
    let gate = null
    let started = await M.start({ block: 1, take: TAKE, owner: 'models block 1' })
    if (!started.ok) {
      // On an existing digest the listing is the stage's whole input; after a digest the run goes on.
      if (p.mode === 'models') throw new Error(`model listing failed: ${started.error}`)
      log(`Models: listing failed (${started.error}); no model is built.`)
      return { status: 'failed', ...totals, model_cleanup: null, error: `listing failed: ${started.error}` }
    }
    log(`${n(started.models)} Model Cards in the digest of ${Z.KEY}.`)
    for (let block = 1; block <= 200 && idle < ROUNDS; block++) {
      if (block > 1) {
        started = await M.start({ block, take: TAKE, owner: `models block ${block}` })
        if (!started.ok) {
          const cleanup = await M.modelCleanup()
          return { status: 'failed', ...totals, gate: started.gate || gate, error: started.error, model_cleanup: cleanup }
        }
      }
      gate = started.gate || gate
      const units = n(started.units)
      if (!units) {
        idle += 1
        if (!gate || gate.complete === true) break
        if (!(gate.counts && n(gate.counts.open))) break
        continue
      }
      idle = 0
      claimed += units
      log(`Models block ${block}: ${units} unit(s) claimed; ${n(started.batches)} architect batch(es) planned of ${n(started.models)} model(s) in the listing.`)
      const built = await M.elaborate(started, 'models', { deepseek })
      M.logModels(built)
      // A model the compiler released to wait (or refused by model-deps) is not finished here: the ledger
      // holds it registered or failed, and it is claimed again once its producer is done.
      for (const b of built) {
        if (!b) continue
        if (b.status === 'waiting' || b.status === 'refused') { totals.deferred += 1; continue }
        if (b.status !== 'done') totals.failed += 1
      }
      const finished = await M.finish({ deepseek, gate })
      if (!finished.ok) {
        const cleanup = await M.modelCleanup()
        return { status: 'failed', ...totals, gate: finished.gate || gate, error: finished.error, model_cleanup: cleanup }
      }
      totals.verified += n(finished.done)
      log(`Models block ${block}: ${n(finished.done)} finished, ${n(finished.failed)} not recorded.`)
      gate = finished.gate || gate
    }
    const final = gate || { complete: false, counts: {} }
    const unresolved = n(final.counts && final.counts.open)
    const status = final.complete === true ? 'complete' : totals.failed && !unresolved ? 'partial' : 'active'
    const modelCleanup = await M.modelCleanup()
    log(`Models: ${status} — ${totals.verified} models checked${final.counts ? `, ${unresolved} open` : ''} (${claimed} unit(s) claimed, ${totals.deferred} deferred).`)
    return { status, ...totals, gate: final, model_cleanup: modelCleanup }
  },
})
// ==== end module zt-model-layer ====

// ==== module zt-organize v9 ====
// Organize: organize-survey lists what to decide; Opus writes only decisions.json (the thesis and Card notes);
// organize-apply records that no merge or move is decided; a card cycle hands the notes to DeepSeek;
// book-skeleton and one DeepSeek job write the Literature Card index and the structure Card.
// v6 (docs/literature-only-digest-design.md): a digest writes only the book's Literature tree. Topic merges, Topic
// Card moves and Topic bodies are zt-idea-grow's, so Opus decides none of them, the skeleton writes no Topic body,
// and the old single Opus job (args.organize "legacy") is gone.
// v7 (pilot 2026-10-23): the decision reads run the helper with -Allow on the survey's empty read-only list: reads
// only, and a path in the staging folder, so a run on a box copy (zt-box.txt) reads that box, not the live one.
// v8 (S10): whether the notes cycle runs is read from decisions.json by organize-apply (its `notes` count), never
// from the decider's answer; the unread option remove: 'delete' leaves.
// v9 (#176, docs/workflow-module-structure-proposal.md): the survey, the apply, the notes card cycle, the
// skeleton's plan and apply and both publications are book-steps.ps1 steps run by DeepSeek drive jobs, and the
// DeepSeek note writers and skeleton writer are content managers of the layer driver. Opus still makes the one
// approved decision; no Claude agent runs a command.
defineModule({
  id: 'zt-organize', version: '9', requires: ['zt-digest-lib'],
  summary: 'ZT digest: Organize on the Literature tree only (survey → Opus thesis and notes → apply → notes card cycle → skeleton: the Literature index and the structure Card).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    groups: { type: 'number', description: 'the group count from the prepare stage' },
    digest: { type: 'any', description: 'the digest group reports (mode full)' },
    link: { type: 'any', description: 'the link group reports (other modes)' },
    models: { type: 'any', description: 'the model reports of the digest stage (mode full)' },
    modelsBook: { type: 'any', description: 'the model reports of the models stage (mode models)' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('organize', ['organize', 'skeleton'])) return { status: 'done', reused: true, publications: {} }
    const A = Z.A, KEY = Z.KEY, OUT = Z.OUT, MODE = Z.MODE
    const groupCount = p.groups || 0
    const modelReports = p.models || p.modelsBook || []
    const modelLines = modelReports.filter(r => r.status === 'done').map(r => `- [[${r.model}]]${r.level === 'concept' ? '（概念模型）' : r.level === 'formal' ? '（形式模型）' : ''}：${Array.isArray(r.parts) ? r.parts.map(pp => `${pp.facet}${pp.emphasis === 'core' ? '（核心）' : ''} [[${pp.address}]]`).join('、') : `${Number(r.parts) || 0} 张分卡（${Number(r.core) || 0} 张核心，见模型卡的结构列表）`}${(r.issues || []).length ? `；未解决：${r.issues.join('；')}` : ''}${r.notes && r.notes.length ? `；建议：${r.notes.join('；')}` : ''}`).join('\n')
    const groupLines = ((MODE === 'full' ? p.digest : p.link) || []).map(r => `- 第 ${r.group} 组 [[${KEY}/${r.group}]]：${r.status}${r.cards ? `，${r.cards} 张` : ''}${r.models ? `，模型卡 ${r.models}` : ''}${r.events ? `，事件卡 ${r.events}` : ''}${r.errors && r.errors !== '无' ? `；问题：${r.errors}` : ''}`).join('\n')
    const ORG = `${OUT}\\organize`
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts by
    // hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the revision
    // stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const json = await Z.bookStep(`${layer}: publish`, 'publish', { Layer: layer, ...more })
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(json && json.error) || 'no output'}`)
      return json || { layer, state: 'failed', error: 'no output' }
    }
    // 1. The survey: a script lists what Opus must decide.
    const survey = await Z.bookStep('organize: survey', 'organize-survey', { Work: ORG })
    if (!survey || survey.ok !== true) log(`Organize: the survey failed (${(survey && survey.error) || 'no output'}).`)
    // 2. Opus decides; it writes decisions.json and nothing else.
    const decided = survey && survey.ok === true ? await Z.zAgent(
      `You decide the structure of the ZET digest of 《${A.bookTitle || KEY}》 (Literature Card [[${KEY}]], anchors [[${KEY}/1]] … [[${KEY}/${groupCount}]]). Scripts and DeepSeek do every edit; you only decide. You make no zt write.
Read ${ORG}\\survey.md first (Read tool). It lists the Card merge proposals, the models with faults, and broken Links (it may also list Topics and hubs: they are not yours; ignore them). The digest writes only the book's Literature tree: Topic trees, Topic bodies, hubs and the register belong to the zt-idea-grow workflow. When a decision needs a Card's text, read it through the helper (read-only calls only: get_card, get_tree, get_model, search_cards), always with -Allow "${ORG}\\read-only.allow.txt" (the survey's empty list: reads only), e.g.
@'
[{"tool":"get_tree","arguments":{"tree":"${KEY}"}},{"tool":"get_card","arguments":{"at":"${KEY}/1"}}]
'@ | ${ZD_CALL} - -Allow "${ORG}\\read-only.allow.txt"
${modelLines ? `The architect's notes on the book's models (Cards outside a model that should link a part of it):\n${modelLines}\n` : ''}
Decide, then write ${ORG}\\decisions.json (UTF-8, Write tool) with exactly these fields:
- "thesis": the book's central thesis as one Chinese sentence of at most 40 characters. It becomes the title of the structure Card.
- "merges": [] (always empty: the digest merges no Topic).
- "moves": [] (always empty: the digest moves no Card).
- "notes": Card-level work for DeepSeek, [{"address": "<Card>", "note": "<one Chinese sentence: what to change>"}]: a Card that should link a model part (the architect's notes), a Card whose Link reads as a plain reference but states a real relation, a Card that should link an Event Card. Only Idea Cards, Event Cards, and anchors of the book's Literature tree [[${KEY}]]; never a Topic-tree Card, and never a note that asks for a Link to a Topic-tree Card.
Use an empty list where nothing needs doing. Do not plan Topics or Hub Cards. Do not rewrite the Literature Card or the structure Card: a script and DeepSeek write them after your decisions.
Return thesis, notes (the count), and report: a short Chinese report of what you decided and what you leave for the user.`,
      {
        label: 'organize: decisions', phase: 'Organize', model: 'opus',
        schema: { type: 'object', properties: { thesis: { type: 'string' }, merges: { type: 'number' }, moves: { type: 'number' }, notes: { type: 'number' }, report: { type: 'string' } }, required: ['thesis', 'report'] },
      },
    ) : null
    // 3. The apply script records the (empty) merges and moves; in the Literature scope it could move no Topic Card.
    const moved = decided ? await Z.bookStep('organize: apply', 'organize-apply', { Work: ORG }) : null
    if (decided) log(`Organize: thesis "${decided.thesis}"; ${moved ? `${moved.merges} merges, ${moved.moves} moves, ${moved.refused} refused, ${moved.notes || 0} notes in decisions.json${moved.thesis === false ? ' (no thesis written)' : ''}` : 'apply failed (no output)'}.`)
    // 4. The notes go to DeepSeek as a card cycle: every hard rule in mode "full" (the script adds N1), else N1.
    const notes = moved && Number(moved.notes) > 0
      ? await Z.cardCycle('organize notes', `${OUT}\\cards-notes`, { notes: `${ORG}\\decisions.json`, noOrphans: true, rules: MODE === 'full' ? '' : 'N1' })
      : null
    // 4a. Organize publishes its revision once its decisions are applied and its notes are disposed.
    const organizePublished = moved ? await publish('organize', { Decisions: `${ORG}\\decisions.json`, Applied: `${ORG}\\applied.json` }) : null
    // 5. The skeleton: the Literature Card index and the structure Card.
    const SK = `${OUT}\\skeleton`
    const planned = await Z.bookStep('organize: skeleton', 'skeleton-start', { Work: SK })
    if (planned && planned.ok === true && Number(planned.jobs || 0)) {
      await Z.runJobs('organize: skeleton write', planned.file, planned.jobs, { dsh: SK, writes: `${SK}\\out`, phase: 'Organize', skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, what: 'skeleton writer', run: true })
    }
    const sk = planned && planned.ok === true
      ? await Z.bookStep('organize: skeleton apply', 'skeleton-finish', { Work: SK, Decisions: `${ORG}\\decisions.json` })
      : null
    log(sk && sk.ok === true ? `Organize: structure Card ${sk.written && sk.written.structure} ("${sk.thesis}"), ${sk.argument} argument lines, ${sk.models} models, ${sk.events} events.` : `Organize: the skeleton failed: ${JSON.stringify((sk && (sk.problems || sk.error)) || 'no output')}`)
    const skeletonPublished = sk && sk.ok === true ? await publish('skeleton', { Skeleton: `${SK}\\out\\skeleton.json` }) : null
    return { survey, decisions: decided, applied: moved, notes: notes && notes.report, skeleton: sk, publications: { organize: organizePublished, skeleton: skeletonPublished } }
  },
})
// ==== end module zt-organize ====

// ==== module zt-term-cards v8 ====
// The Term-card branch (S20 #164, S24 #167; v4 #172; v5 #175; v6 #180): Haiku managers and DeepSeek agents only.
// v7 (live run 2026-10-26, staging5): a publication that printed no line is named in the error (`publish_error`).
// v8 (2026-10-26, the user's request): one DeepSeek job reviews AND fixes each math draft (it runs the writer's
// self-check and writes its verdict for the draft it leaves); the separate fix round and the second review are gone.
// Every deterministic step is one `term-steps.ps1` call that a DeepSeek agent runs (a "drive" job, the shared
// zt-workflow-lib helper `ztDrive`), so no Claude agent relays a single command. The flow is linear:
//   start (plan, claim every ready job unit, list the jobs) -> writers (all jobs, at most eight per
//   manager, managers in parallel) -> review (check + review plan) -> review-and-fix jobs -> finish
//   (bind each job's verdict to the draft it left, complete each job from the readiness result, fail the
//   rest; publish every ready row).
// A job that is not ready fails only its own unit; the book unit completes only when every row is
// published (R0343). Every conversation is deleted after its answer is collected; the workspace
// registration is removed at the end (DSMP).
// v6 (#180): a step answers one compact line whose fields are top-level counts and paths (`ztDrive` reads
// only its last line, and only with a boolean top-level `ok`); the module reads counts, never a unit list.
defineModule({
  id: 'zt-term-cards', version: '8', requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Term cards: Haiku managers run DeepSeek drive jobs for the deterministic steps and DeepSeek writers and review-and-fix jobs for the content; each job completes from the readiness result and the book from its publication report.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command' },
    perJob: { type: 'number', min: 1, max: 8, description: 'concept terms per writer job (the compiler\'s default is 4)' },
    mathPerJob: { type: 'number', min: 1, max: 8, description: 'math terms per writer job (the compiler\'s default is 1)' },
    script: { type: 'string', default: 'E:\\zet\\scripts\\book-digest\\term-steps.ps1', description: 'the Term step script the drive jobs run' },
  },
  run: async (ctx, p) => {
    // The kernel passes only the params a blueprint gives; the script path has one default.
    const DEFAULT_SCRIPT = 'E:\\zet\\scripts\\book-digest\\term-steps.ps1'
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const WORK = `${OUT}\\term-cards`
    const DRIVE = `${WORK}\\drive`
    const MODEL = 'deepseek-official/deepseek-flash'
    const phase = ctx.phase
    const tail = (text) => String(text || '').trim().slice(0, 400)
    const report = { status: 'done', book: p.key, work: WORK, jobs: { total: 0, done: 0, failed: 0 }, reviews: 0, publication: null, deepseek: { deleted: 0, not_deleted: [] } }

    // One manager runs the DeepSeek jobs listed inline, or entries [from, to) of a job file.
    const RESULT = {
      type: 'object',
      properties: {
        ok: { type: 'boolean' },
        jobs: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' }, deleted: { type: 'boolean' } }, required: ['id', 'status'] } },
      },
      required: ['ok', 'jobs'],
    }
    const manager = (label, source) => dsmpSlot(() => ctx.agent(`You are a DeepSeek subagent manager for ZET Term Cards (${label}). You start DeepSeek agents, wait for them, collect their answers and delete their conversations. You never write content and never run commands yourself. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete".

JOBS: ${source}
Each job has id, files, prompt, skills, model, effort.

1. Start every job in ONE message: mcp__deepseek__deepseek_run with cwd "${WORK}", prompt_files = its "files", prompt = its "prompt", skills = its "skills", model = its "model", reasoning_effort = its "effort", wait_seconds 0.
2. Keep at most 8 jobs running at a time. Until no job is running: call mcp__deepseek__deepseek_wait (wait_seconds ${DSMP.pollSeconds}) on one running job, then mcp__deepseek__deepseek_wait (wait_seconds 0) on the others. A job asking a question: answer "按任务说明自行决定，不需要询问。". A job asking approval for a command or a file under "${OUT}": allow; anything else: reject. A job running longer than ${DSMP.budgetMinutes} minutes: cancel it.
3. A job that failed or gave an empty answer gets ONE more attempt in a new conversation. Never a third.
4. When a job ends, delete its conversation with mcp__deepseek__deepseek_delete. Never archive.
5. Use the session_id that deepseek_run returned, never a word in its place. A step whose job list is empty starts no run: never send a placeholder prompt.
5. You have no terminal tabs, no Monitor loop, no session handoff and no general shell: never start another agent, never hand the work on, never poll with Start-Sleep or any other timer, and wait only with deepseek_wait. Any command you are given to run is run in the foreground with timeout_ms: 600000.
6. Return ok=true and one entry per job: id, session_id, status ("done" or "failed"), answer (its final answer verbatim, at most 2000 characters), deleted.`, { role: 'manager', label: `${ctx.stage}: ${label}`, phase, schema: RESULT }))

    const collect = (run) => {
      const jobs = (run && run.jobs) || []
      for (const j of jobs) { if (j.deleted === true) report.deepseek.deleted++; else if (j.session_id) report.deepseek.not_deleted.push(j.session_id) }
      return jobs
    }
    // A deterministic step: one DeepSeek drive job runs one script call and answers with its last line
    // (the shared zt-workflow-lib helper ztDrive). A step that answers no JSON line stops the branch; a step
    // that answers `ok:false` is the branch's failed reason.
    const drive = async (step) => {
      const command = `pwsh -NoProfile -File "${p.script || DEFAULT_SCRIPT}" -Step ${step} -Out "${OUT}"${p.perJob ? ` -PerJob ${p.perJob}` : ''}${p.mathPerJob ? ` -MathPerJob ${p.mathPerJob}` : ''}`
      const json = await ztDrive(ctx, { label: `${ctx.stage}: drive ${step}`, step, command, out: OUT, cwd: WORK, phase, ledger: 'zt-digest ledger', deepseek: report.deepseek })
      if (json.ok !== true) throw new Error(`term-cards ${step}: ${tail(json.error || 'the step failed')}`)
      return json
    }
    // Content jobs from a job file the script wrote: at most eight per manager, managers in parallel.
    const runFile = async (label, file, count) => {
      const managers = []
      for (let from = 0; from < count; from += DSMP.perManager) {
        const to = Math.min(count, from + DSMP.perManager)
        managers.push(manager(`${label} ${from + 1}-${to}`, `read the JSON array in "${file}" (Read tool) and run its entries ${from} through ${to - 1} (0-based), and only those.`).then(collect))
      }
      return (await Promise.all(managers)).flat()
    }

    try {
      const started = await drive('start')
      report.jobs.total = started.jobs
      report.plan_revision = started.plan_revision
      if (started.jobs > 0) {
        const written = await runFile('write', `${DRIVE}\\jobs.json`, started.jobs)
        report.writer_failures = written.filter((j) => j.status !== 'done').length
        const r = await drive('review')
        report.reviews = r.reviews
        if (r.reviews > 0) await runFile('review', `${DRIVE}\\reviews.json`, r.reviews)
      }
      const finished = await drive('finish')
      report.jobs.done = Number(finished.done || 0) || 0
      report.not_ready = Number(finished.failed || 0) || 0
      report.publication = { complete: finished.complete === true, rows: Number(finished.rows || 0), committed: Number(finished.committed || 0), ready_unpublished: Number(finished.ready_unpublished || 0) }
      report.book_state = finished.book
      if (finished.book !== 'done') {
        report.status = 'failed'
        report.error = `the Term-card book is incomplete: ${report.not_ready} job(s) not ready; ${report.publication.committed} ready row(s) are published${finished.publish_error ? `; the publication failed: ${finished.publish_error}` : ''}`
      }
    } catch (e) {
      report.status = 'failed'
      report.error = tail(e && e.message)
    } finally {
      try { await dsmpSlot(() => dsCleanupWorkspaces([WORK], { label: `${ctx.stage}: workspace`, phase })) } catch (e) { report.deepseek.not_deleted.push(`workspace: ${tail(e && e.message)}`) }
    }
    log(`Term cards: ${report.jobs.done}/${report.jobs.total} job(s) done, ${report.reviews} review-and-fix job(s); ${report.status}${report.error ? ` — ${report.error}` : ''}.`)
    return report
  },
})
// ==== end module zt-term-cards ====

// ==== module zt-term-link v5 ====
// S24 (#167); v3 (#172); v4 (#175); v5 (#180): the run's whole-book Term-link pass as one DeepSeek drive job that a
// Haiku manager starts through the shared zt-workflow-lib helper `ztDrive`: `term-steps.ps1 -Step link` plans, applies
// under the claim (or, when the unit is blocked by an incomplete Term-card book, links the published prefix and
// leaves the unit open), and plans again so the report shows the second pass's changes (0 expected). The
// finalization's own correction round still uses the shared zt-term-link-lib.
// v5 (#180): the step answers one compact line whose fields are top-level counts (`cards_changed`,
// `links_added`, `second_cards_changed`, `second_cards_refused`); the module reads those and builds its report.
defineModule({
  id: 'zt-term-link', version: '5', requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Term link: one DeepSeek drive job runs the deterministic Term-link pass (plan, apply, complete, second plan) and reports its result.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    occurrences: { type: 'string', enum: ['first', 'all'], default: 'first', description: 'the occurrence policy (args.termLinkOccurrences)' },
    script: { type: 'string', default: 'E:\\zet\\scripts\\book-digest\\term-steps.ps1', description: 'the Term step script the drive job runs' },
  },
  run: async (ctx, p) => {
    // The kernel passes only the params a blueprint gives; the script path has one default.
    const DEFAULT_SCRIPT = 'E:\\zet\\scripts\\book-digest\\term-steps.ps1'
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const tail = (text) => String(text || '').trim().slice(0, 400)
    const n = (v) => Number(v || 0)
    const deepseek = { deleted: 0, not_deleted: [] }
    const command = `pwsh -NoProfile -File "${p.script || DEFAULT_SCRIPT}" -Step link -Out "${OUT}" -Occurrences ${p.occurrences}`
    const json = await ztDrive(ctx, { label: `${ctx.stage}: drive link`, step: 'link', command, out: OUT, cwd: `${OUT}\\term-link`, phase: ctx.phase, ledger: 'zt-digest ledger', deepseek })
    if (json.ok !== true) return { status: 'failed', book: p.key, error: tail(json.error || 'the link step failed'), deepseek }
    const counts = { cards_changed: n(json.cards_changed), links_added: n(json.links_added) }
    const second = { cards_changed: n(json.second_cards_changed), cards_refused: n(json.second_cards_refused) }
    log(`Term link: ${json.status}; ${counts.cards_changed} Card(s) changed; second plan changes ${second.cards_changed} Card(s).`)
    return { status: json.status, book: p.key, unit: json.unit || null, counts, second, reason: json.reason || null, deepseek }
  },
})

const BLUEPRINT = {
 "protocol": "workflow-framework-protocol/1",
 "name": "zt-econ-digest",
 "version": "6.7.2",
 "description": "Digest a Chinese-translated economics book into ZET Cards: the ZT digest base with the economics set: the model layer (Model Cards in the digest, the model compiler with the economic architect), the event layer (Event Cards from the book's dated sentences), the empirical-results layer (Finding Cards, one per result), and the economics prompts; it builds only the Literature tree (Topic trees, hubs and the register are zt-idea-grow's)",
 "whenToUse": "Turn a Scholar Archive economics book (Chinese bilingual Markdown, digested as Chinese only) into an agent Literature tree with Idea, Event, Finding, and Model Cards (economic models at concept or formal level). It writes no Topic tree, no Hub Card and no register: zt-idea-grow grows those from the Literature trees afterwards. Modes link, models, cards, explain, events, findings, figures, audit and bridge re-run single stages on an existing digest.",
 "roles": {
  "manager": {
   "model": "haiku",
   "effort": "low",
   "description": "Haiku: runs the DeepSeek job managers (drive, chain, block) and the cleanup settle; it runs no command of its own"
  },
  "architect": {
   "model": "sonnet",
   "description": "Sonnet: the model architect, writes plan.json only"
  },
  "organizer": {
   "model": "opus",
   "description": "Opus: the Organize decisions, writes decisions.json only"
  },
  "relay": {
   "model": "haiku",
   "effort": "low",
   "agentType": "zt-relay",
   "description": "Haiku as the zt-relay agent type: runs one script command and returns its last output line"
  }
 },
 "args": {
  "out": {
   "type": "string",
   "required": true,
   "description": "the digest's staging folder"
  },
  "citationKey": {
   "type": "string",
   "description": "the book's BibTeX key (all modes but audit)"
  },
  "source": {
   "type": "string",
   "description": "the book's Chinese or bilingual Markdown (mode full)"
  },
  "bibtex": {
   "type": "string",
   "description": "the BibTeX entry (mode full)"
  },
  "bookTitle": {
   "type": "string",
   "description": "the book's title (mode full)"
  },
  "book": {
   "type": "any",
   "description": "a Scholar Archive Work id or an exact title: mode full resolves the Work, its Chinese/bilingual Markdown, the BibTeX and the heading range instead of source, bibtex and bookTitle"
  },
  "fileId": {
   "type": "any",
   "description": "the book Markdown's Scholar Archive file id"
  },
  "startHeading": {
   "type": "string",
   "description": "the Chinese heading where the digested text starts"
  },
  "endHeading": {
   "type": "string",
   "description": "the Chinese heading where the digested text stops"
  },
  "skipHeadings": {
   "type": "string",
   "description": "headings whose units are left out, e.g. 参考文献,习题"
  },
  "groups": {
   "type": "number",
   "description": "the existing group count (modes link and models)"
  },
  "organize": {
   "type": "boolean",
   "description": "true: Organize after modes link and models (the Literature index, the structure Card and the notes on the book's own Cards)"
  },
  "maxChars": {
   "type": "number",
   "description": "the most characters of a group (default 20000)"
  },
  "maxUnits": {
   "type": "number",
   "description": "the most units of a group (default 3)"
  },
  "dshEffort": {
   "type": "string",
   "description": "DeepSeek effort of digest, link, and fill runs (raised to high)"
  },
  "models": {
   "type": "boolean",
   "description": "false: skip the model stage"
  },
  "rebuild": {
   "type": "boolean",
   "description": "rebuild models that already have parts"
  },
  "perSonnet": {
   "type": "number",
   "min": 1,
   "description": "formal models per architect batch (default 8)"
  },
  "perConcept": {
   "type": "number",
   "min": 1,
   "description": "concept models per batch (default 10)"
  },
  "dshPerBatch": {
   "type": "number",
   "min": 1,
   "description": "DeepSeek runs one Haiku manager keeps going (default 8, at most 8)"
  },
  "dshManagers": {
   "type": "number",
   "min": 1,
   "description": "Haiku managers of one compiler step in parallel (default 8, at most 8)"
  },
  "applyShare": {
   "type": "number",
   "min": 1,
   "description": "jobs one card-cycle apply call writes (default 30)"
  },
  "only": {
   "type": "array",
   "description": "the Model Card addresses to work on (a pilot)"
  },
  "level": {
   "type": "string",
   "enum": [
    "concept",
    "formal"
   ],
   "description": "build every model of the run to this level"
  },
  "briefEffort": {
   "type": "string",
   "description": "DeepSeek effort of the briefs (raised to high)"
  },
  "deriveEffort": {
   "type": "string",
   "description": "DeepSeek effort of the Derivation parts (raised to high)"
  },
  "reviewEffort": {
   "type": "string",
   "description": "DeepSeek effort of the review (raised to high)"
  },
  "reviewRounds": {
   "type": "number",
   "min": 0,
   "description": "review, split, review again (default 2; 0 skips)"
  },
  "maxSentences": {
   "type": "number",
   "description": "Card size of a formal model part: sentences (default 12)"
  },
  "maxPartChars": {
   "type": "number",
   "description": "Card size of a formal model part: characters (default 400)"
  },
  "maxSteps": {
   "type": "number",
   "description": "steps per Derivation part (default 6)"
  },
  "maxExplainSentences": {
   "type": "number",
   "description": "explanation budget: sentences (default 16)"
  },
  "maxExplainChars": {
   "type": "number",
   "description": "explanation budget: characters (default 600)"
  },
  "terms": {
   "type": "boolean",
   "description": "mode full: false skips the book term table"
  },
  "audit": {
   "type": "boolean",
   "description": "mode full: false skips the audit"
  },
  "finalize": {
   "type": "boolean",
   "default": true,
   "description": "mode full: false skips the finite finalization branch (audit plan, one scoped correction cycle, publication refresh, final read-only audit)"
  },
  "figures": {
   "type": "boolean",
   "description": "mode full: false skips the Figures stage"
  },
  "ideaSample": {
   "type": "number",
   "min": 0,
   "max": 1,
   "description": "share of fault-free Idea Cards reviewed against the source (mode full: 0.1)"
  },
  "cardRules": {
   "type": "string",
   "description": "the rules the card cycle fixes (default all hard rules)"
  },
  "cardFixRounds": {
   "type": "number",
   "min": 0,
   "description": "card check, fix, apply rounds (default 1)"
  },
  "cardMax": {
   "type": "number",
   "min": 1,
   "description": "a pilot: at most this many Cards with faults and orphans"
  },
  "explainMax": {
   "type": "number",
   "min": 1,
   "description": "mode explain, a pilot: at most this many Derivation parts"
  },
  "bridge": {
   "type": "boolean",
   "description": "full mode: false skips Bridge discovery"
  },
  "bridgeCandidates": {
   "type": "number",
   "min": 1,
   "max": 8,
   "description": "similarity candidates per source, default 5 plus at most two literal matches"
  },
  "bridgeBooks": {
   "type": "string",
   "description": "comma-separated eligible target book keys"
  },
  "bridgeReport": {
   "type": "string",
   "description": "exact published report id; only mode bridge"
  },
  "bridgeMax": {
   "type": "number",
   "min": 1,
   "max": 120,
   "description": "selected Bridge sources, default 60"
  },
  "bridgeApply": {
   "type": "any",
   "description": "all or a unique array of new-carrier ids from bridgeReport"
  },
  "bridgeApplyEarlier": {
   "type": "any",
   "description": "separate all or unique array of earlier-carrier ids from bridgeReport"
  },
  "maxAttempts": {
   "type": "number",
   "min": 1,
   "max": 10,
   "description": "the per-unit attempt budget of the managed run (default 3); persisted at preparation"
  },
  "dshRunCap": {
   "type": "number",
   "min": 60,
   "max": 3600,
   "description": "the seconds one external DeepSeek run may live (default 1200); persisted at preparation"
  },
  "cardChecks": {
   "type": "boolean",
   "description": "modes full and link: false skips the card layer cycle"
  },
  "termCards": {
   "type": "boolean",
   "default": true,
   "description": "false skips the Term-card branch of a full run; the layer is then excluded in the prepared run, so no Term-link pass waits for it"
  },
  "termLink": {
   "type": "boolean",
   "default": true,
   "description": "false skips the deterministic Term-link pass of a full run; the layer is then excluded in the prepared run"
  },
  "termCardsPerJob": {
   "type": "number",
   "min": 1,
   "max": 8,
   "default": 4,
   "description": "concept terms per Term-card writer job (the compiler's default is 4)"
  },
  "termCardsMathPerJob": {
   "type": "number",
   "min": 1,
   "max": 8,
   "default": 1,
   "description": "mathematical terms per Term-card writer job (the compiler's default is 1)"
  },
  "termLinkOccurrences": {
   "type": "string",
   "enum": [
    "first",
    "all"
   ],
   "default": "first",
   "description": "which eligible occurrences the Term-link pass links: the first per term and body, or all"
  },
  "events": {
   "type": "boolean",
   "description": "mode full: false skips the event layer"
  },
  "findings": {
   "type": "boolean",
   "description": "mode full: false skips the empirical-results layer"
  }
 },
 "consts": {
  "profile": {
   "book": "economics",
   "instructions": "dsh-digest-base.zh.md,dsh-model-cards.zh.md",
   "noEventCandidates": true,
   "digestPrefix": "shared-prefix.md",
   "linkPrefix": "link-prefix.md",
   "compilerDocs": {},
   "architectSupplement": null,
   "skipForms": [],
   "bridgeProfile": "book"
  }
 },
 "stages": [
  {
   "id": "args",
   "title": "Prepare",
   "detail": "the required args per mode",
   "module": "gate",
   "params": {
    "subject": {
     "args": "{{args}}",
     "mode": "{{mode}}"
    },
    "checks": [
     {
      "op": "or",
      "value": [
       {
        "path": "mode",
        "op": "matches",
        "value": "^(audit|resume|term-cards|term-link)$"
       },
       {
        "path": "args.citationKey",
        "op": "nonempty"
       }
      ],
      "message": "args must be an object with citationKey"
     },
     {
      "op": "or",
      "value": [
       {
        "path": "mode",
        "op": "!=",
        "value": "full"
       },
       {
        "path": "args.book",
        "op": "nonempty"
       },
       {
        "op": "and",
        "value": [
         {
          "path": "args.source",
          "op": "nonempty"
         },
         {
          "path": "args.bibtex",
          "op": "nonempty"
         },
         {
          "path": "args.bookTitle",
          "op": "nonempty"
         }
        ]
       }
      ],
      "message": "mode full needs args book, or args source, bibtex, and bookTitle"
     },
     {
      "op": "or",
      "value": [
       {
        "op": "and",
        "value": [
         {
          "path": "args.bridgeReport",
          "op": "empty"
         },
         {
          "path": "args.bridgeApply",
          "op": "empty"
         },
         {
          "path": "args.bridgeApplyEarlier",
          "op": "empty"
         }
        ]
       },
       {
        "op": "and",
        "value": [
         {
          "path": "mode",
          "op": "==",
          "value": "bridge"
         },
         {
          "path": "args.bridgeReport",
          "op": "nonempty"
         },
         {
          "op": "or",
          "value": [
           {
            "path": "args.bridgeApply",
            "op": "nonempty"
           },
           {
            "path": "args.bridgeApplyEarlier",
            "op": "nonempty"
           }
          ]
         }
        ]
       }
      ],
      "message": "Bridge acceptance requires mode bridge, a report id, and a nonempty selection"
     },
     {
      "op": "or",
      "value": [
       {
        "path": "args.bridgeMax",
        "op": "empty"
       },
       {
        "path": "args.bridgeMax",
        "op": "matches",
        "value": "^(?:[1-9][0-9]{0,2})$"
       }
      ],
      "message": "bridgeMax must be a whole number in 1..120"
     },
     {
      "op": "or",
      "value": [
       {
        "path": "args.bridgeCandidates",
        "op": "empty"
       },
       {
        "path": "args.bridgeCandidates",
        "op": "matches",
        "value": "^(?:[1-9][0-9]{0,2})$"
       }
      ],
      "message": "bridgeCandidates must be a whole number in 1..8"
     },
     {
      "op": "or",
      "value": [
       {
        "op": "and",
        "value": [
         {
          "path": "mode",
          "op": "!=",
          "value": "resume"
         },
         {
          "path": "mode",
          "op": "!=",
          "value": "term-cards"
         },
         {
          "path": "mode",
          "op": "!=",
          "value": "term-link"
         }
        ]
       },
       {
        "op": "and",
        "value": [
         {
          "path": "args.book",
          "op": "empty"
         },
         {
          "path": "args.source",
          "op": "empty"
         },
         {
          "path": "args.bibtex",
          "op": "empty"
         },
         {
          "path": "args.bookTitle",
          "op": "empty"
         },
         {
          "path": "args.citationKey",
          "op": "empty"
         },
         {
          "path": "args.groups",
          "op": "empty"
         },
         {
          "path": "args.only",
          "op": "empty"
         },
         {
          "path": "args.bridgeReport",
          "op": "empty"
         },
         {
          "path": "args.bridgeApply",
          "op": "empty"
         },
         {
          "path": "args.bridgeApplyEarlier",
          "op": "empty"
         }
        ]
       }
      ],
      "message": "modes resume, term-cards and term-link read the retained book, its source and its staging from the prepared run and accepts only out (with the Term settings): a new identity, source, group count, model list or Bridge selection needs its own preparation"
     },
     {
      "op": "or",
      "value": [
       {
        "path": "mode",
        "op": "!=",
        "value": "term-cards"
       },
       {
        "path": "args.termCards",
        "op": "!=",
        "value": false
       }
      ],
      "message": "mode term-cards is the Term-card branch: args.termCards false contradicts the mode"
     },
     {
      "op": "or",
      "value": [
       {
        "path": "mode",
        "op": "!=",
        "value": "term-link"
       },
       {
        "path": "args.termLink",
        "op": "!=",
        "value": false
       }
      ],
      "message": "mode term-link is the deterministic Term-link pass: args.termLink false contradicts the mode"
     }
    ]
   }
  },
  {
   "id": "source",
   "title": "Prepare",
   "detail": "the DeepSeek source (probe official, then OpenCode Go)",
   "after": [
    "args"
   ],
   "module": "zt-dsh-source",
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}"
   },
   "when": [
    {
     "op": "or",
     "value": [
      {
       "path": "mode",
       "op": "!=",
       "value": "bridge"
      },
      {
       "op": "and",
       "value": [
        {
         "path": "args.bridgeReport",
         "op": "empty"
        },
        {
         "path": "args.bridgeApply",
         "op": "empty"
        },
        {
         "path": "args.bridgeApplyEarlier",
         "op": "empty"
        }
       ]
      }
     ]
    }
   ]
  },
  {
   "id": "prepare",
   "title": "Prepare",
   "detail": "Chinese-only book text, section groups, the Literature tree with anchors, the shared prefixes",
   "after": [
    "source"
   ],
   "module": "zt-digest-prepare",
   "params": {
    "key": "{{args.citationKey}}",
    "args": "{{args}}",
    "mode": "{{mode}}",
    "needGroups": [
     "full",
     "link"
    ],
    "instructions": "{{consts.profile.instructions}}",
    "noEventCandidates": "{{consts.profile.noEventCandidates}}",
    "profile": "{{consts.profile}}"
   }
  },
  {
   "id": "terms",
   "title": "Prepare",
   "detail": "the book term table (book-terms.ps1 and one DeepSeek job)",
   "after": [
    "prepare"
   ],
   "module": "zt-book-terms",
   "onFail": "continue",
   "when": [
    {
     "path": "args.terms",
     "op": "!=",
     "value": false
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "term-cards",
   "title": "Term cards",
   "detail": "the Term-card branch, once the Literature content is settled (after the whole-book Cards cycle; live run 2026-10-26: beside the digest writers its plan went stale before it published): the compiler's writer jobs in waves of four on the shared DSMP pool, their checks, the independent mathematical reviews and the one finite fix; each job unit and the book unit complete only from the compiler's own checked artifacts, so a failed or partial book stays incomplete",
   "after": [
    "cards-book"
   ],
   "module": "zt-term-cards",
   "onFail": "continue",
   "when": [
    {
     "op": "and",
     "value": [
      {
       "path": "mode",
       "op": "matches",
       "value": "^(full|resume|term-cards)$"
      },
      {
       "path": "args.termCards",
       "op": "!=",
       "value": false
      },
      {
       "path": "stages.prepare.output.run_options.termCards",
       "op": "!=",
       "value": false
      }
     ]
    }
   ],
   "params": {
    "out": "{{args.out}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    },
    "perJob": {
     "$ref": "args.termCardsPerJob",
     "default": null
    },
    "mathPerJob": {
     "$ref": "args.termCardsMathPerJob",
     "default": null
    }
   }
  },
  {
   "id": "events",
   "title": "Events",
   "detail": "the event layer: event-compiler.ps1 lists the dated sentences and each group's Idea Cards; DeepSeek writes the Event Cards as JSON with its SELF-CHECK; the script creates them beside the Idea Cards they support and links them",
   "module": "zt-event-layer",
   "when": [
    {
     "path": "mode",
     "op": "matches",
     "value": "^(full|events|resume)$"
    },
    {
     "path": "args.events",
     "op": "!=",
     "value": false
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   },
   "after": [
    "terms"
   ]
  },
  {
   "id": "findings",
   "title": "Findings",
   "detail": "the empirical-results layer: finding-compiler.ps1 lists the sentences that report data with a number and each group's Idea Cards; DeepSeek writes one Finding Card per result as JSON with its SELF-CHECK (F1 to F3); the script creates them beside the group anchors and links them with 印证",
   "module": "zt-finding-layer",
   "when": [
    {
     "path": "mode",
     "op": "matches",
     "value": "^(full|findings|resume)$"
    },
    {
     "path": "args.findings",
     "op": "!=",
     "value": false
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   },
   "after": [
    "terms"
   ]
  },
  {
   "id": "digest",
   "title": "Digest",
   "detail": "the group cycle in blocks: one manager claims up to 8 digest/gNNN@1 units, a snapshot and frozen job per group, their DeepSeek runs side by side, each conversation recorded and deleted by that manager, checked group-finish (R250/R252/R253); at most 8 lanes",
   "after": [
    "terms"
   ],
   "module": "zt-group-cycle",
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "groups": "{{stages.prepare.output.groups}}",
    "profile": "{{consts.profile}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "cards",
   "title": "Cards",
   "detail": "the per-group offline Card checks (cards/gNNN@1), claimed from the ready work and recorded by cards-finish",
   "module": "zt-cards-layer",
   "after": [
    "digest"
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "models",
   "title": "Models",
   "detail": "the model layer: the model compiler on every Model Card of the book (DeepSeek brief → Sonnet plan → DeepSeek fill, derive, review, split)",
   "module": "zt-model-layer",
   "when": [
    {
     "op": "or",
     "value": [
      {
       "path": "mode",
       "op": "==",
       "value": "resume"
      },
      {
       "op": "and",
       "value": [
        {
         "path": "mode",
         "op": "==",
         "value": "full"
        },
        {
         "path": "args.models",
         "op": "!=",
         "value": false
        }
       ]
      },
      {
       "path": "mode",
       "op": "==",
       "value": "models"
      }
     ]
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "profile": "{{consts.profile}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   },
   "after": [
    "cards"
   ]
  },
  {
   "id": "explain",
   "title": "Models",
   "detail": "mode explain: 思路, 说明, and 解读 for finished Derivation parts",
   "module": "zt-explain",
   "when": [
    {
     "op": "or",
     "value": [
      {
       "path": "mode",
       "op": "==",
       "value": "explain"
      },
      {
       "op": "and",
       "value": [
        {
         "path": "mode",
         "op": "==",
         "value": "full"
        },
        {
         "path": "args.models",
         "op": "!=",
         "value": false
        }
       ]
      },
      {
       "path": "mode",
       "op": "==",
       "value": "resume"
      }
     ]
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   },
   "after": [
    "models"
   ]
  },
  {
   "id": "cards-book",
   "title": "Cards",
   "detail": "one whole-book card cycle after the digest runs (mode full), or on its own (mode cards); it also reviews the source of every Model draft no model chain elaborates",
   "after": [
    "explain"
   ],
   "module": "zt-cards-book",
   "when": [
    {
     "op": "or",
     "value": [
      {
       "path": "mode",
       "op": "==",
       "value": "cards"
      },
      {
       "op": "and",
       "value": [
        {
         "path": "mode",
         "op": "==",
         "value": "full"
        },
        {
         "path": "args.cardChecks",
         "op": "!=",
         "value": false
        }
       ]
      },
      {
       "path": "mode",
       "op": "==",
       "value": "resume"
      }
     ]
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "link-prepare",
   "title": "Link",
   "detail": "prepare-link-pass.ps1: the receipt-owned semantic gap set and the scheduled, gap-free and blocked groups",
   "after": [
    "events",
    "findings",
    "cards-book"
   ],
   "module": "zt-link-prepare",
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "groups": "{{stages.prepare.output.groups}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "link",
   "title": "Link",
   "detail": "DeepSeek Link runs fill only the receipt-owned semantic gaps",
   "after": [
    "link-prepare"
   ],
   "module": "zt-link-pass",
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "profile": "{{consts.profile}}",
    "groups": "{{stages.prepare.output.groups}}",
    "pending": {
     "$ref": "stages.link-prepare.output.pending",
     "default": []
    },
    "blocked": {
     "$ref": "stages.link-prepare.output.blocked",
     "default": []
    },
    "noGap": {
     "$ref": "stages.link-prepare.output.no_gap",
     "default": []
    },
    "gaps": {
     "$ref": "stages.link-prepare.output.gaps",
     "default": {}
    },
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "link-check",
   "title": "Cards",
   "detail": "the card layer cycle on the new Links and the orphans",
   "after": [
    "link"
   ],
   "module": "zt-link-check",
   "when": [
    {
     "path": "args.cardChecks",
     "op": "!=",
     "value": false
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "organize",
   "title": "Organize",
   "detail": "survey → Opus thesis and notes (decisions.json) → apply (no merge, no move) → notes card cycle → skeleton (the Literature index and the structure Card)",
   "after": [
    "link-check"
   ],
   "module": "zt-organize",
   "onFail": "continue",
   "when": [
    {
     "op": "or",
     "value": [
      {
       "path": "mode",
       "op": "matches",
       "value": "^(full|resume|models|explain|events|findings)$"
      },
      {
       "path": "args.organize",
       "op": "==",
       "value": true
      }
     ]
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "groups": "{{stages.prepare.output.groups}}",
    "digest": {
     "$ref": "stages.digest.output.reports"
    },
    "link": {
     "$ref": "stages.link.output.reports"
    },
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    },
    "models": {
     "$ref": "stages.models.output.models"
    }
   }
  },
  {
   "id": "figures",
   "title": "Figures",
   "detail": "attach-figures.ps1: the book's own figures in the Cards that name them",
   "after": [
    "organize"
   ],
   "module": "zt-figures",
   "when": [
    {
     "op": "or",
     "value": [
      {
       "path": "mode",
       "op": "==",
       "value": "figures"
      },
      {
       "path": "args.figures",
       "op": "!=",
       "value": false
      }
     ]
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "bridge",
   "title": "Bridge",
   "detail": "bounded frozen cross-book discovery, immutable report, and its new-carrier proposals applied; earlier carriers by explicit apply in bridge mode",
   "after": [
    "figures"
   ],
   "module": "zt-bridge",
   "onFail": "continue",
   "when": [
    {
     "op": "or",
     "value": [
      {
       "path": "mode",
       "op": "==",
       "value": "bridge"
      },
      {
       "op": "and",
       "value": [
        {
         "path": "mode",
         "op": "==",
         "value": "full"
        },
        {
         "path": "args.bridge",
         "op": "!=",
         "value": false
        }
       ]
      },
      {
       "path": "mode",
       "op": "==",
       "value": "resume"
      }
     ]
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "profile": "{{consts.profile.bridgeProfile}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   },
   "expect": [
    {
     "path": "status",
     "op": "!=",
     "value": "failed"
    }
   ]
  },
  {
   "id": "term-link",
   "title": "Term link",
   "detail": "the deterministic Term-link pass after every Card writer and the Bridge (design 14.1-14.2): claim the run's termlink/book@1 unit, freeze the plan, apply it under the claim and complete the unit only from the compiler's apply report; a failed Term-card book or Card writer leaves it blocked and reported, never linked. The finite finalization runs the same shared pass for its one correction round (termlink/final@R), for the changed Cards or the whole book when the correction changed the Term mapping (S26, #169)",
   "after": [
    "bridge",
    "term-cards"
   ],
   "module": "zt-term-link",
   "onFail": "continue",
   "when": [
    {
     "op": "and",
     "value": [
      {
       "path": "mode",
       "op": "matches",
       "value": "^(full|resume|term-cards|term-link)$"
      },
      {
       "path": "args.termLink",
       "op": "!=",
       "value": false
      },
      {
       "path": "stages.prepare.output.run_options.termLink",
       "op": "!=",
       "value": false
      }
     ]
    }
   ],
   "params": {
    "out": "{{args.out}}",
    "args": "{{args}}",
    "mode": "{{mode}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    },
    "occurrences": {
     "$ref": "args.termLinkOccurrences",
     "default": "first"
    }
   }
  },
  {
   "id": "audit",
   "title": "Audit",
   "detail": "digest-audit.ps1, read-only: faults per rule, orphans, models, figures (it counts the box's Topic trees and hubs as information, never as a digest fault)",
   "after": [
    "bridge",
    "term-link"
   ],
   "module": "zt-audit",
   "when": [
    {
     "op": "or",
     "value": [
      {
       "path": "mode",
       "op": "==",
       "value": "audit"
      },
      {
       "path": "args.audit",
       "op": "!=",
       "value": false
      }
     ]
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "finalize",
   "title": "Finalize",
   "detail": "the finite finalization: the audit plan, at most one compiler correction cycle scoped to its Cards, the changed-Card/touched-tree manifest, the Term-link round for the Cards that cycle changed (the whole book when it changed the Term mapping), the dependency-scoped publication refresh, and the final read-only audit",
   "after": [
    "audit"
   ],
   "module": "zt-finalize",
   "onFail": "continue",
   "when": [
    {
     "op": "and",
     "value": [
      {
       "path": "mode",
       "op": "matches",
       "value": "^(full|resume)$"
      },
      {
       "path": "args.finalize",
       "op": "!=",
       "value": false
      }
     ]
    }
   ],
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   }
  },
  {
   "id": "cleanup",
   "title": "Cleanup",
   "detail": "settle every DeepSeek conversation and workspace the run recorded, delete exactly those ids, remove only registered scratch inside the staging root, then render the verified run report (completeness, gaps, current placeholders, timings, exact per-run usage), reporting content_complete apart from cleanup_complete",
   "after": [
    "finalize"
   ],
   "module": "zt-dsh-cleanup",
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    }
   },
   "onFail": "continue"
  },
  {
   "id": "report",
   "title": "Report",
   "detail": "the run's result per mode",
   "after": [
    "cleanup"
   ],
   "module": "zt-digest-report",
   "params": {
    "args": "{{args}}",
    "mode": "{{mode}}",
    "model": {
     "$ref": "stages.source.output.model"
    },
    "groups": {
     "$ref": "stages.prepare.output.groups"
    },
    "digest": {
     "$ref": "stages.digest.output"
    },
    "terms": {
     "$ref": "stages.terms.output"
    },
    "cardsBook": {
     "$ref": "stages.cards-book.output"
    },
    "link": {
     "$ref": "stages.link.output"
    },
    "linkBlocked": {
     "$ref": "stages.link.output.blocked",
     "default": []
    },
    "linkCheck": {
     "$ref": "stages.link-check.output"
    },
    "cleanup": {
     "$ref": "stages.cleanup.output"
    },
    "organize": {
     "$ref": "stages.organize.output"
    },
    "figures": {
     "$ref": "stages.figures.output"
    },
    "audit": {
     "$ref": "stages.audit.output"
    },
    "extra": {
     "events": {
      "$ref": "stages.events.output"
     },
     "findings": {
      "$ref": "stages.findings.output"
     }
    },
    "prepare": {
     "$ref": "stages.prepare.output",
     "default": null
    },
    "bridge": {
     "$ref": "stages.bridge.output"
    },
    "cards": {
     "$ref": "stages.cards.output"
    },
    "finalize": {
     "$ref": "stages.finalize.output"
    },
    "termCards": {
     "$ref": "stages.term-cards.output"
    },
    "termLink": {
     "$ref": "stages.term-link.output"
    },
    "key": {
     "$ref": "stages.prepare.output.citation_key"
    },
    "models": {
     "$ref": "stages.models.output"
    },
    "explain": {
     "$ref": "stages.explain.output"
    }
   }
  }
 ],
 "modes": {
  "link": [
   "args",
   "source",
   "prepare",
   "link-prepare",
   "link",
   "link-check",
   "organize",
   "figures",
   "cleanup",
   "report"
  ],
  "cards": [
   "args",
   "source",
   "prepare",
   "cards-book",
   "report"
  ],
  "figures": [
   "args",
   "figures",
   "report"
  ],
  "audit": [
   "args",
   "audit",
   "cleanup",
   "report"
  ],
  "bridge": [
   "args",
   "source",
   "bridge",
   "report"
  ],
  "term-cards": [
   "args",
   "source",
   "prepare",
   "term-cards",
   "term-link",
   "audit",
   "cleanup",
   "report"
  ],
  "term-link": [
   "args",
   "prepare",
   "term-link",
   "audit",
   "cleanup",
   "report"
  ],
  "models": [
   "args",
   "source",
   "prepare",
   "models",
   "explain",
   "link-prepare",
   "link",
   "link-check",
   "organize",
   "audit",
   "cleanup",
   "report"
  ],
  "explain": [
   "args",
   "source",
   "prepare",
   "explain",
   "link-prepare",
   "link",
   "link-check",
   "organize",
   "audit",
   "cleanup",
   "report"
  ],
  "events": [
   "args",
   "source",
   "prepare",
   "events",
   "link-prepare",
   "link",
   "link-check",
   "organize",
   "audit",
   "cleanup",
   "report"
  ],
  "findings": [
   "args",
   "source",
   "prepare",
   "findings",
   "link-prepare",
   "link",
   "link-check",
   "organize",
   "audit",
   "cleanup",
   "report"
  ],
  "resume": [
   "args",
   "prepare",
   "terms",
   "term-cards",
   "digest",
   "cards",
   "events",
   "findings",
   "models",
   "explain",
   "cards-book",
   "link-prepare",
   "link",
   "link-check",
   "organize",
   "figures",
   "bridge",
   "term-link",
   "audit",
   "finalize",
   "cleanup",
   "report"
  ]
 },
 "checkpoints": [
  "digest"
 ],
 "result": {
  "$ref": "stages.report.output"
 },
 "composedFrom": {
  "base": "zt-digest-base",
  "version": "5.7.2"
 },
 "installed": {
  "protocol": "workflow-framework-protocol/1",
  "kernel": "v1",
  "modules": {
   "gate": "1",
   "zt-workflow-lib": "7",
   "zt-digest-lib": "44",
   "zt-audit": "6",
   "zt-book-terms": "6",
   "zt-bridge": "10",
   "zt-cards-book": "6",
   "zt-cards-layer": "4",
   "zt-digest-prepare": "18",
   "zt-digest-report": "20",
   "zt-dsh-cleanup": "13",
   "zt-dsh-source": "2",
   "zt-event-layer": "6",
   "zt-model-lib": "13",
   "zt-explain": "9",
   "zt-figures": "6",
   "zt-term-link-lib": "4",
   "zt-finalize": "5",
   "zt-finding-layer": "6",
   "zt-group-cycle": "8",
   "zt-link-check": "3",
   "zt-link-pass": "9",
   "zt-link-prepare": "8",
   "zt-model-layer": "8",
   "zt-organize": "9",
   "zt-term-cards": "8",
   "zt-term-link": "5"
  }
 }
}
const FINGERPRINT = 'bcc2c572b6972a81'
return await wfpRun(BLUEPRINT, FINGERPRINT)
