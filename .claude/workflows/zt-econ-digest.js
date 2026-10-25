export const meta = {
  "name": "zt-econ-digest",
  "description": "Digest a Chinese-translated economics book into ZET Cards: the ZT digest base with the economics set: the model layer (Model Cards in the digest, the model compiler with the economic architect), the event layer (Event Cards from the book's dated sentences), the empirical-results layer (Finding Cards, one per result), and the economics prompts; it builds only the Literature tree (Topic trees, hubs and the register are zt-idea-grow's)",
  "whenToUse": "Turn a Scholar Archive economics book (Chinese bilingual Markdown, digested as Chinese only) into an agent Literature tree with Idea, Event, Finding, and Model Cards (economic models at concept or formal level). It writes no Topic tree, no Hub Card and no register: zt-idea-grow grows those from the Literature trees afterwards. Modes link, models, cards, explain, events, findings, figures, audit and bridge re-run single stages on an existing digest. (v6.5.4; built by the workflow-framework-protocol; structure frozen at install. Modes: link, cards, figures, audit, bridge, models, explain, events, findings, resume.)",
  "phases": [
    {
      "title": "Prepare",
      "detail": "the required args per mode"
    },
    {
      "title": "Term cards",
      "detail": "the Term-card branch beside the digest writers (design 14.1): the compiler's writer jobs in waves of four on the shared DSMP pool, their checks, the independent mathematical reviews and the one finite fix; each job unit and the book unit complete only from the compiler's own checked artifacts, so a failed or partial book stays incomplete"
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
      "detail": "the deterministic Term-link pass after every Card writer and the Bridge (design 14.1-14.2): claim the run's termlink/book@1 unit, freeze the plan, apply it under the claim and complete the unit only from the compiler's apply report; a failed Term-card book or Card writer leaves it blocked and reported, never linked"
    },
    {
      "title": "Audit",
      "detail": "digest-audit.ps1, read-only: faults per rule, orphans, models, figures (it counts the box's Topic trees and hubs as information, never as a digest fault)"
    },
    {
      "title": "Finalize",
      "detail": "the finite finalization: the audit plan, at most one compiler correction cycle scoped to its Cards, the changed-Card/touched-tree manifest, the dependency-scoped publication refresh, and the final read-only audit"
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

// zt-econ-digest v6.5.4: built by the workflow-framework-protocol (assemble.mjs) from its blueprint.
// DO NOT EDIT THIS FILE. The structure and logistics of an installed workflow are frozen: change the
// blueprint (zt-econ-digest.blueprint.json), run the pre-flight checks, and install a new version.
// Arguments: out, citationKey, source, bibtex, bookTitle, book, fileId, startHeading, endHeading, skipHeadings, groups, organize, maxChars, maxUnits, dshEffort, models, rebuild, perSonnet, perConcept, dshPerBatch, dshManagers, applyShare, only, level, briefEffort, deriveEffort, reviewEffort, reviewRounds, maxSentences, maxPartChars, maxSteps, maxExplainSentences, maxExplainChars, terms, audit, finalize, figures, ideaSample, cardRules, cardFixRounds, cardMax, explainMax, bridge, bridgeCandidates, bridgeBooks, bridgeReport, bridgeMax, bridgeApply, bridgeApplyEarlier, maxAttempts, dshRunCap, cardChecks, termCards, termLink, termCardsPerJob, termCardsMathPerJob, termLinkOccurrences, events, findings; mode: full, link, cards, figures, audit, bridge, models, explain, events, findings, resume; until: digest

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

// ==== module zt-workflow-lib v1 ====
// The code the ZT workflows share (docs/workflow-simplicity-proposal.md S1, S6): the digests (zt-digest-lib) and
// zt-idea-grow (zt-grow-cycle) load this one library instead of each keeping its own copy.
//  - S6, one lock reader: the owner's typed `card_locked` refusal (#140, R326/R337/R338) is read here and only
//    here. A refusal is a destination decision, never a provider fault: the stage stops with it and nothing
//    is retried, unlocked or re-pointed through another Access.
//  - S1, one command runner: every command of both workflows runs through one relay of the zt-relay agent type
//    (the blueprints' role `relay`), which runs one PowerShell line and returns its exit state and its last
//    line. The relay decides nothing; the caller reads the line.
//  - the JSON line reader both workflows use on a relayed line.
const ZT_LOCK = 'card_locked'
const ZT_COMMAND_TIMEOUT_MS = 600000
const ZT_RELAY_SCHEMA = { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] }

// The JSON object in a line, also after prose on the same line ("…保留原样。{"address":…}"): the longest object
// from the first brace that parses (a relay may add a remark after the line, "(abridged)").
function ztJsonIn(line) {
  const text = String(line || '')
  for (let i = text.indexOf('{'); i >= 0; i = text.indexOf('{', i + 1)) {
    for (let j = text.lastIndexOf('}'); j > i; j = text.lastIndexOf('}', j - 1)) {
      try { const v = JSON.parse(text.slice(i, j + 1)); if (v && typeof v === 'object') return v } catch (e) { }
    }
  }
  return null
}
// The last line of a text that holds a JSON object.
function ztLastJson(text) {
  for (const line of String(text || '').trim().split('\n').reverse()) { const v = ztJsonIn(line); if (v) return v }
  return null
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
// The sentence every relay gets: the command is a step of the run the user started. Workflow steps see the
// session's latest user message; after "accelerate it" relays refused steps as off-request (grossman trials 17–19).
function ztRunStep(run) {
  return `Every command named here is a step of ${run}: running it is what the user asked for. A later user message about this run (its speed, which trial runs, parallel work) never cancels its steps; only a stopped run ends them.`
}
// One relay runs one command and returns { run: {ok, output}, json }: json is the last JSON object of the output
// (also of a failed command when o.always, e.g. a gate that prints its line and exits 3). A command whose
// answer carries the owner's typed refusal throws the stop (only a refusal stops; a read that names one does not).
async function ztRelay(ctx, o) {
  const run = await ctx.agent(
    `Run this PowerShell command once in the foreground with timeout_ms: ${ZT_COMMAND_TIMEOUT_MS} and wait for it${o.note || ''}; do nothing else:
${o.command}
${ztRunStep(o.run || 'the ZT workflow run the user started')}
Return ok (true when it exits without an error) and output = its last output line exactly (a compact JSON object), or the error text when it fails. Do not retry.`,
    { role: o.role || 'relay', label: o.label, phase: o.phase, schema: ZT_RELAY_SCHEMA },
  )
  const json = run && (run.ok || o.always) ? ztLastJson(run.output) : null
  const refused = !!(json && (json.code === ZT_LOCK || json.error === ZT_LOCK))
  const lock = ztLockRefusal(run && run.output) || (refused ? json : null)
  if (lock && (!(run && run.ok) || refused) && o.lockStops !== false) throw ztLockStop(o.label, lock)
  return { run, json }
}
defineModule({
  id: 'zt-workflow-lib', version: '1', library: true,
  summary: 'Library: what the ZT workflows share: the one reader of the owner\'s typed card_locked refusal, the one zt-relay command runner, and the JSON line reader.',
})
// ==== end module zt-workflow-lib ====

// ==== module zt-digest-lib v37 ====
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
// the operator to repeat args.citationKey (#87, R274/R276). 13 makes every claim cover the provider run it
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
  const KEY = (p && p.key) || A.citationKey
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
  // One relay (the zt-relay agent type, the blueprint's role relay) runs one script command and returns its last JSON
  // line (or null). A gate prints its JSON and then exits 3 while the layer still has open work, so a caller that
  // asks (o.always) still reads a typed result. R326/R337: a command whose answer carries the owner's typed refusal
  // stops the stage here (the shared ztRelay); a read that merely names a retained fault keeps going.
  const runScript = (label, phaseName, command, note = '', o = {}) => ztRelay(ctx, {
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
  With a session: deepseek_prompt with that session_id, prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, reasoning_effort "${o.effort}", wait_seconds 0. Without one: deepseek_run as above, with prompt_files = fallback.
Keep at most ${DSH_PER_BATCH} jobs running at a time; start the next job when one ends. Start independent jobs in one message.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 600).
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: the Bridge compiler (${ZD_DIGEST} bridge, or bridge-compiler.ps1) with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}), or the check zt-check (read-only); a read-only self-check of a draft: the card, event, or finding compiler (${ZD_DIGEST} card, event, or finding, or card-compiler.ps1, event-compiler.ps1, finding-compiler.ps1) with -Step precheck, the model compiler (${ZD_DIGEST} model, or model-compiler.ps1) with -Step explain-check, or the book skeleton (${ZD_DIGEST} skeleton, or book-skeleton.ps1) with -Step apply and without -Apply; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
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
- continue: reuse the session_id of the earlier job of this chain with that id (from this chain's own results); without one, deepseek_run with prompt_files = fallback (or files when it names none).
Keep at most ${DSH_PER_BATCH} jobs running at a time; start independent jobs in one message and start the next job when one ends.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 90), then start the next waiting job and keep polling; never wait longer than 90 seconds at once.${renewLine}
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: the Bridge compiler (${ZD_DIGEST} bridge, or bridge-compiler.ps1) with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}), or the check zt-check (read-only); a read-only self-check of a draft: the card, event, or finding compiler (${ZD_DIGEST} card, event, or finding, or card-compiler.ps1, event-compiler.ps1, finding-compiler.ps1) with -Step precheck, the model compiler (${ZD_DIGEST} model, or model-compiler.ps1) with -Step explain-check, or the book skeleton (${ZD_DIGEST} skeleton, or book-skeleton.ps1) with -Step apply and without -Apply; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
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

  // --- The block driver: ONE manager runs up to BLOCK_TAKE claimed units side by side ------------------------
  // The DeepSeek Subagent Managed Protocol lets one Haiku manager run 8 DeepSeek conversations. A unit whose
  // work is one DeepSeek job (a digest group, a Link group) is therefore claimed in blocks: the manager claims
  // up to 8 units, starts each, runs their jobs side by side, renews every held claim while it waits, settles
  // each job's conversation itself (record, evidence, delete), and finishes each unit with its checked command.
  const BLOCK_TAKE = ZD_PER_MANAGER_MAX
  const BLOCK = {
    type: 'object',
    properties: {
      claim: { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' }, error: { type: 'string' } }, required: ['ok'] },
      units: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            unit: { type: 'string' }, token: { type: 'number' }, group: { type: 'number' },
            start: { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' }, error: { type: 'string' } } },
            finish: { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' }, error: { type: 'string' } } },
            jobs: {
              type: 'array',
              items: {
                type: 'object',
                properties: {
                  id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] },
                  registered: { type: 'boolean' }, evidence: { type: 'boolean' }, deleted: { type: 'boolean' },
                },
                required: ['id', 'status'],
              },
            },
          },
          required: ['unit', 'token'],
        },
      },
    },
    required: ['claim', 'units'],
  }
  const blockPrompt = (label, o) => `You run one block of a ZET book digest: ${label}. A script and DeepSeek Harness (DSH) do the work; you claim up to ${BLOCK_TAKE} units, start each, run their DSH jobs side by side, and finish each. Keep your messages short.
${RUNNER}
Load the DSH tools with one ToolSearch call: query "${DRIVE_TOOLS}".
BLOCK PLAN: ${JSON.stringify({ dsh: o.dsh, claim: o.claim, start: o.start, finish: o.finish })}
1. Claim: run this command in the foreground with timeout_ms: ${ZD_COMMAND_TIMEOUT_MS} and read its last output line (compact JSON):
   ${o.claim}
   Its "units" array lists the units you now hold, each with "id" and "token"; a unit's group number is the digits of gNNN in its id. If "units" is empty, stop here: return claim (ok, output) and units [].
2. Start: for each held unit, run this command with {{group}} and {{token}} replaced by that unit's values, and read its last line:
   ${o.start}
   It names a "job" file, "expected_jobs" and a "draft" path. Read the job file: its "jobs" entries name files and prompt. When expected_jobs is 0, that unit runs no job.
3. Jobs: run the jobs of all held units side by side, at most ${DSH_PER_BATCH} at a time; start them in one message: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills || [])}, model "${DSH_MODEL}", reasoning_effort "${o.effort || DSH_EFFORT}", wait_seconds 0.
   In the record commands below, <unit_id> and <token> are the id and token of the unit whose job it is.
${recordRules("<unit_id>", "<token>", o.dsh, true)}
   Follow "status" until each job ends:
   - "running": deepseek_wait (session_id, wait_seconds 90); never wait longer than 90 seconds at once. After every wait round, in the same message, renew each claim whose job still runs: ${ZD_LEDGER} renew -Out "${OUT}" -Unit "<unit id>" -Token <token>
   - "needs_response": approve (decision "allow") a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}), the check zt-check (read-only), a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}, and writing JSON files under "${o.writes || o.dsh}". Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
   - "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
4. Finish: as soon as a unit's jobs have all ended (or it had none), run this command with {{group}}, {{token}} and {{draft}} replaced by that unit's values, and read its last line:
   ${o.finish}
Return claim (ok, output = the claim's last line unchanged, or error) and units: one entry per held unit with unit, token, group, start (ok, output = its last line unchanged, or error), finish (ok, output = its last line unchanged, or error), and jobs (id, session_id, status "done" or "failed", registered, evidence, deleted).
Copy each of those output lines character for character: never summarize, abbreviate or shorten them (a claim line is a few kilobytes). The script reads them as JSON.`
  // Runs one block in ONE manager. The ledger's claim line is the truth about which units the block holds;
  // a held unit the manager did not report comes back with no finish (a missing result).
  const block = async (label, o) => {
    // A manager whose structured answer fails (trial 17: five invalid JSON answers carrying a 2.6 KB claim
    // line, and the agent call threw) is a block with no report, never a halted stage.
    let run = null, thrown = ''
    try {
      run = await dsmpSlot(() => zAgent(blockPrompt(label, o), { label, phase: o.phase || 'Digest', model: 'haiku', schema: BLOCK }))
    } catch (e) {
      thrown = String((e && e.message) || e)
      log(`${label}: the block manager returned no report (${thrown.slice(0, 160)})`)
    }
    const got = run || { claim: { ok: false, error: thrown || 'no report from the block manager' }, units: [] }
    const claimLine = zdLastJson(got.claim && got.claim.output)
    // The claim line is the ledger's own word on the held units. A relay that summarized it (run 5: "Claimed
    // 8 units ... Output abbreviated") leaves only the manager's unit reports, each with its id and token:
    // without them the lane stopped and a failed unit stayed claimed with no fault recorded.
    let reported = (got.units || []).filter((u) => u && typeof u.unit === 'string' && /@\d+$/.test(u.unit) && Number(u.token) > 0)
      .map((u) => ({ id: u.unit, token: Number(u.token), input_hash: '' }))
    // No claim line and no unit report: the units this block's owner holds come from the ledger.
    const owner = (String(o.claim || '').match(/-Owner "([^"]+)"/) || [])[1]
    if (!claimLine && !reported.length && owner) {
      const held = await runScript(`${label}: held units`, LEDGER_PHASE, ledgerCommand(`status -Out "${OUT}" -Held "${owner}"`))
      const rows = held.json && Array.isArray(held.json.units) ? held.json.units : []
      reported = rows.filter((u) => u && u.id && Number(u.token) > 0).map((u) => ({ id: u.id, token: Number(u.token), input_hash: u.input_hash || '' }))
      if (reported.length) log(`${label}: ${reported.length} unit(s) the block still holds are read from the ledger.`)
    }
    const claimed = claimLine && Array.isArray(claimLine.units)
      ? claimLine.units.filter((u) => u && u.id)
      : reported
    if (!claimLine && reported.length) log(`${label}: the claim line came back summarized; the ${reported.length} held unit(s) are taken from the manager's own unit reports.`)
    const units = claimed.map((c) => {
      const said = (got.units || []).find((u) => u.unit === c.id) || {}
      return {
        unit: c.id, token: c.token, input_hash: c.input_hash || '', group: Number((/g(\d+)@/.exec(c.id) || [])[1]) || null,
        start: said.start || null, finish: said.finish || null, jobs: Array.isArray(said.jobs) ? said.jobs : [],
      }
    })
    const unrecorded = units.flatMap((u) => u.jobs).filter((j) => j.session_id && j.registered === false).map((j) => j.id)
    if (unrecorded.length) log(`${label}: ${unrecorded.length} job id(s) not recorded by the manager: ${unrecorded.join(', ')}`)
    // The ledger is the truth about each held unit: a manager may return its unit reports incomplete (run 6:
    // one of eight, or none), and a done unit must never be faulted for a missing report.
    if (units.length) {
      const read = await runScript(`${label}: unit states`, LEDGER_PHASE, ledgerCommand(`status -Out "${OUT}" -Units ${units.map((u) => u.unit).join(',')}`), ' (it reads the held units\' states; it changes nothing)')
      const states = (read.json && Array.isArray(read.json.units)) ? read.json.units : []
      for (const u of units) u.ledger = states.find((x) => x && x.id === u.unit) || null
      if (!states.length) log(`${label}: the unit states could not be read; the manager's reports decide`)
    }
    // empty: the ledger said no unit was ready (or the manager held none); a lane ends only on that.
    const empty = !claimed.length && (claimLine ? Array.isArray(claimLine.units) : !!(got.claim && got.claim.ok))
    return { ok: !!(got.claim && got.claim.ok), claimed, units, empty, thrown: !!thrown, error: got.claim && got.claim.error }
  }
  // One block unit's outcome. The ledger decides: done is done (whatever the manager reported), a unit some
  // other path already settled or reclaimed is left alone, and only a unit this block still holds gets a
  // fault, classified from the manager's report (a refused or checked content failure, else retryable).
  const blockSettle = async (u, label) => {
    const finish = blockFinish(u)
    const said = u.ledger
    if (said && said.state === 'done') return { status: 'done', parsed: finish.parsed || { disposition: said.disposition || 'checked' } }
    if (said && said.state === 'failed') return { status: 'failed', error: `the ledger holds ${u.unit} failed (${said.fault_class || 'fault'})` }
    if (said && (said.state !== 'claimed' || Number(said.token) !== Number(u.token))) return { status: 'retry', error: `the ledger holds ${u.unit} ${said.state}` }
    // R326/R337: the unit's own start or checked finish carries the owner's typed refusal. The unit
    // keeps that refusal as its content fault (never a retryable one), and the stage stops with it.
    const lock = zdLockRefusal((u.start && (u.start.error || u.start.output)) || '')
      || zdLockRefusal((u.finish && (u.finish.error || u.finish.output)) || '')
      || ((finish.parsed && (finish.parsed.code === ZD_LOCK || finish.parsed.error === ZD_LOCK)) ? finish.parsed : null)
    if (lock) {
      await failUnit(`${label}: ${u.unit} locked`, u.unit, u.token, {
        schema_version: 1, unit: u.unit, input_hash: u.input_hash || (u.ledger && u.ledger.input_hash) || '',
        code: ZD_LOCK, cause: `${zdLockSaid(lock)} ${JSON.stringify(lock)}`.slice(0, 800),
      })
      log(`${label}: ${u.unit} met the owner's Lock; the stage stops and keeps its drafts, errors and receipts.`)
      throw zdLockStop(`${label}: ${u.unit}`, lock)
    }
    const cause = (finish.faults[0] || (u.start && u.start.error) || 'the unit did not complete').slice(0, 800)
    // S5: the block states the facts it saw (a start or finish that never answered is a missing result; a finish
    // the verifier refused is a checked failure); a cause it cannot type goes to the ledger, which reads it.
    const transport = !u.start || u.start.ok === false || !u.finish || u.finish.ok === false
    const refused = !transport && finish.parsed && finish.parsed.state === 'failed'
    const code = refused ? 'check_failed' : transport ? 'missing_result' : (finish.faults.length ? undefined : 'missing_result')
    const fault = { schema_version: 1, unit: u.unit, input_hash: u.input_hash || (u.ledger && u.ledger.input_hash) || '', ...(code ? { code } : {}), cause }
    const status = await failUnit(`${label}: ${u.unit} fault`, u.unit, u.token, fault)
    log(`${label}: ${u.unit} ${status === 'failed' ? 'content' : 'retryable'} failure: ${cause}`)
    return { status, error: cause }
  }
  // One block unit's checked finish line, with the same checks as a chain step (R271/R272).
  const blockFinish = (u) => resultLine({ steps: u.finish ? [{ name: 'finish', ok: u.finish.ok !== false, output: u.finish.output, error: u.finish.error }] : [] }, 'finish', { unit: u.unit, token: u.token })
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
      const g = await gate(`${label}: ${name} gate`, name)
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
    // The ready-work driver (9.3): one manager per declared chain, plus the ledger adapter.
    chain, block, blockFinish, blockSettle, BLOCK_TAKE, ANSWERS, stepOf, jsonOf, resultLine, claim, gate, settledOnResume, holding, failUnit, finishOf, finishOrFault, layer, psQuote,
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
  id: 'zt-digest-lib', version: '37', library: true, requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Library: the ZT digest machinery (settings, managers, one-manager chains, the ledger ready-work driver, the model compiler, the card cycle) for the zt-* stage modules, and the Tree Lock seam that stops an affected write stage on the owner\'s typed card_locked refusal.',
})
// ==== end module zt-digest-lib ====

// ==== module zt-audit v3 ====
// The audit: digest-audit.ps1, read-only, on one book (args.citationKey) or on every book of the agent Workspace.
// It changes no Card; it writes audit.json, audit.md, and audit-faults.txt in args.out.
defineModule({
  id: 'zt-audit', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the read-only audit of the book\'s Cards (digest-audit.ps1).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('audit', ['audit'])) return { status: 'done', reused: true }
    const { run, json: summary } = await Z.runScript('audit cards', 'Audit',
      `${Z.tool('digest-audit')}${Z.KEY ? ` -CitationKey ${Z.KEY}` : ''} -Out "${Z.OUT}"`,
      ' (it may take several minutes)')
    if (!summary) { log(`Audit failed: ${run && run.output}`); return { status: 'failed', error: run && run.output } }
    for (const b of summary.books || []) log(`Audit ${b.key}: ${b.cards} Cards in ${b.topic_trees} Topic trees; ${b.style_faults} Card language faults; ${b.layer_faults ?? 0} Card layer faults (${b.layer_advisory ?? 0} advisory); ${b.orphans} orphans; ${b.no_source} with no source; ${b.unkinded} with no kinded Link; ${b.not_self_sufficient} of ${b.models} models not self-sufficient; ${b.explained ?? 0} of ${b.derivations ?? 0} Derivation parts with 思路 and 解读; ${b.figures_missing} figures named but not shown.`)
    log(`Audit: ${summary.hubs_missing} missing hubs, ${summary.broken_links} broken Links in the Workspace. Report: ${Z.OUT}\\audit.md`)
    return { status: 'done', ...summary }
  },
})
// ==== end module zt-audit ====

// ==== module zt-book-terms v4 ====
// v4 (S5): a fault names its code and leaves the class to the ledger.
// The book term table, as ready ledger work: the `terms/book@1` unit that every digest group waits for. The
// chain claims it, plans the candidates (book-terms finds the bold terms, definition sentences, bracketed
// original-language terms and frequent word groups), runs the one DeepSeek job that chooses the terms and the
// words not to use, and applies the table: the script writes terms.md and the Card 术语表 before any digest run,
// through archive receipts, and completes the unit itself with its checked evidence. A failure is recorded
// against the unit, so the groups that depend on it stay waiting instead of digesting without the table.
defineModule({
  id: 'zt-book-terms', version: '4', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the claimed Term table (terms/book@1): plan → DeepSeek → apply, completing the unit.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const termWork = `${Z.OUT}\\terms`
    const got = await Z.claim('terms claim', 'terms', 1, 0)
    if (!got.ok) return { status: 'failed', error: got.error, report: null }
    const unit = (got.units || [])[0]
    if (!unit) {
      log('Term table: no ready Terms unit (a disabled layer or a completed table); the digest goes on.')
      return { status: 'skipped', report: null, reason: 'no ready terms unit' }
    }
    const token = unit.token
    const tstep = (s, more = '') => `${Z.tool('book-terms')} -Step ${s} -Staging "${Z.OUT}" -Token ${token}${more}`
    const chained = await Z.chain('terms', {
      dsh: termWork, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Prepare', writes: `${termWork}\\out`,
      steps: [
        { name: 'plan', command: tstep('plan'), jobs: true },
        { name: 'apply', command: tstep('apply', ' -Apply') },
      ],
      cleanup: true, keep: true,
    })
    const report = Z.jsonOf(chained, 'apply')
    const ok = report && (report.ok === true || report.state === 'done')
    if (!ok) {
      const cause = String((Z.stepOf(chained, 'apply') || {}).error || (Z.stepOf(chained, 'plan') || {}).error || 'no output from the Term table steps').slice(0, 800)
      await Z.failUnit('terms fault', unit.id, token, { schema_version: 1, unit: unit.id, input_hash: unit.input_hash || '', code: 'check_failed', cause })
      log(`Term table failed: ${cause}; the digest groups stay waiting on it.`)
      return { status: 'failed', report, error: cause }
    }
    log(`Term table: ${report.terms} terms (${report.with_avoid} with words not to use) in ${report.card}.`)
    return { status: 'done', unit: unit.id, report }
  },
})
// ==== end module zt-book-terms ====

// ==== module zt-bridge v9 ====
// v9: a discovery applies every new-carrier proposal of its report itself (no user review); earlier carriers stay explicit.
// v8 (S8): the no-op cleanupDsh call leaves; the run-level cleanup settles the recorded conversations.
defineModule({
  id: 'zt-bridge', version: '9', requires: ['zt-digest-lib'],
  summary: 'Bounded cross-book discovery publishes a report and applies its new-carrier proposals; an identified selection (earlier carriers too) applies through guarded script edits in mode bridge.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'run arguments' },
    mode: { type: 'string', required: true, description: 'run mode' },
    model: { type: 'any', description: 'resolved judge model; unused for apply' },
    profile: { type: 'string', required: true, description: 'book or math' },
  },
  run: async (ctx, p) => {
    const Z=zdMake(ctx,p), A=Z.A, work=`${Z.OUT}\\bridge`
    if (await Z.settledOnResume('bridge', ['bridge'])) return { status: 'done', reused: true }
    for(const [key,low,high] of [['bridgeMax',1,120],['bridgeCandidates',1,8]])if(A[key]!==undefined&&(!Number.isInteger(A[key])||A[key]<low||A[key]>high))return {status:'failed',error:`${key} must be an integer in ${low}..${high}.`}
    if((A.bridgeBooks!==undefined&&typeof A.bridgeBooks!=='string')||(A.bridgeReport!==undefined&&typeof A.bridgeReport!=='string'))return {status:'failed',error:'bridgeBooks and bridgeReport must be strings.'}
    const q=v=>`'${String(v).replace(/'/g,"''")}'`
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts
    // by hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the
    // revision stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const { run, json } = await Z.runScript(`${layer}: publish`, 'Bridge', `${Z.DIGEST} finalize -Step stage -Out "${Z.OUT}" -Layer ${layer}${more}`, ' (it records the stage publication in the ledger)')
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(run && run.output) || 'no output'}`)
      return json || { layer, state: 'failed', error: (run && run.output) || 'no output' }
    }
    const selection=v=>{
      if(v===undefined)return ''
      if(v==='all')return 'all'
      if(!Array.isArray(v)||v.some(id=>typeof id!=='string'||!/^b[0-9]{3}$/.test(id))||new Set(v).size!==v.length)throw Error('Bridge selection must be all or a unique array of proposal ids.')
      return v.join(',')
    }
    let accept,earlier
    try{accept=selection(A.bridgeApply);earlier=selection(A.bridgeApplyEarlier)}catch(e){return {status:'failed',error:String(e)}}
    const applying=!!(A.bridgeReport||accept||earlier)
    if(applying&&(p.mode!=='bridge'||!A.bridgeReport||!(accept||earlier)))
      return {status:'failed',error:'Bridge apply requires mode bridge, bridgeReport, and a nonempty acceptance section.'}
    const step=(s,more='')=>`${Z.tool('bridge-compiler')} -Step ${s} -Staging ${q(Z.OUT)} -CitationKey ${q(Z.KEY)} -Work ${q(work)}${more}`
    if(applying){
      const extra=` -ReportId ${q(A.bridgeReport)}${accept?` -Accept ${q(accept)}`:''}${earlier?` -AcceptEarlier ${q(earlier)}`:''} -Apply`
      const result=await Z.runScript('bridge: apply','Bridge',step('apply',extra),'; filing must stay quiet: no insertion, movement, deletion, key rename, retype, or Organize during apply')
      return result.json||{status:'failed',error:result.run&&result.run.output}
    }
    const opts={dsh:work,skills:['zettelkasten','asd-ste100-zh'],effort:Z.DSH_EFFORT,phase:'Bridge',writes:`${work}\\out`,readOnly:true,remove:'delete',keep:true}
    // A report freezes its plan and proposals. So publish only after a plan whose jobs all answered;
    // otherwise keep the folder unpublished, so a rerun of mode bridge on the same out resumes it.
    // The cleanup (delete every judge conversation and the DSH workspace) always runs, even after a throw.
    let planned,reported,error
    try{
      planned=await Z.drive('bridge: plan',{...opts,command:step('plan',` -Profile ${q(p.profile)} -MaxSources ${q(A.bridgeMax??60)} -Candidates ${q(A.bridgeCandidates??5)}${A.bridgeBooks?` -Books ${q(A.bridgeBooks)}`:''}`)})
      const plan=planned&&planned.ok?zdLastJson(planned.output):null
      const failedJobs=((planned&&planned.results)||[]).filter(r=>r.status!=='done').map(r=>r.id)
      if(!plan||plan.ok===false)error=`Bridge planning failed: ${(plan&&(plan.errors||plan.error))?JSON.stringify(plan.errors||plan.error):((planned&&(planned.error||planned.output))||'no output')}`
      else if(failedJobs.length)error=`Bridge judge jobs failed: ${failedJobs.join(', ')}. The report is not published; rerun mode bridge with the same out to resume.`
      else reported=await Z.drive('bridge: report',{...opts,command:step('report')})
    }catch(e){error=error||String(e)}
    const summary=zdLastJson((reported&&reported.output)||'')
    if(error)return {status:'failed',error}
    if(!summary||!summary.step||!summary.report_id)return {status:'failed',error:'No Bridge report summary (step and report_id missing).'}
    // v9: the stage applies every new-carrier proposal of its own report at once, so no review waits for the
    // user (acemoglu2007equilibrium: 67 proposals left open). The apply publishes the Bridge revision itself.
    // Earlier-carrier proposals write into another book's tree, usually locked: they stay for an explicit apply.
    if(Number((summary.counts||{}).new)>0){
      const applied=await Z.runScript('bridge: apply new','Bridge',step('apply',` -ReportId ${q(summary.report_id)} -Accept 'all' -Apply`),'; filing must stay quiet: no insertion, movement, deletion, key rename, retype, or Organize during apply')
      const result=applied.json||{status:'failed',error:(applied.run&&applied.run.output)||'no output'}
      if(result.status==='failed')return {...summary,status:'failed',error:`Bridge apply failed: ${result.error||'no detail'}`,applied:result}
      return {...summary,applied:result,publication:result.publication}
    }
    const publication=await publish('bridge',` -Report "${work}\\proposals.json"`)
    return {...summary,publication}
  },
})
// ==== end module zt-bridge ====

// ==== module zt-cards-book v4 ====
// The whole-book Card cycle as ready ledger work (modes cards and full, R296/R247): publish the current
// `cards-book` revision from the settled digest, Model and Cards contributions, claim it, run the one cycle in
// ONE manager chain (plan → DeepSeek JSON → apply in shares → check → one fix round → report), and record it
// with `book-cards-finish`, which re-derives the deterministic pass and requires one reviewed owner per Model
// draft. The cycle's coverage gate waits for the delegated Model reviews to settle while model compilation
// never waits for it (R248). In full it also reviews the source of a Model draft no model chain owns.
defineModule({
  id: 'zt-cards-book', version: '4', requires: ['zt-digest-lib'],
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
    const sample = Z.IDEA_SAMPLE ? ` -Sample ${Z.IDEA_SAMPLE}` : ''
    // The publication revision is derived from the present contributions; the same manifest reuses it.
    const published = await Z.runScript('cards publish', 'Cards', `${Z.LEDGER} publish -Out "${Z.OUT}" -Layer cards-book`, ' (the same present inputs reuse the registered revision)')
    const unit = published.json && published.json.unit
    if (!unit) return { status: 'failed', error: (published.run && published.run.output) || 'the cards-book revision was not registered' }
    const got = await Z.claim('cards-book claim', 'cards-book', 1, 300)
    if (!got.ok) return { status: 'failed', error: got.error }
    const claimed = (got.units || []).find(u => u.id === unit) || (got.units || [])[0]
    if (!claimed) return { status: 'active', error: `the ${unit} revision is waiting for its Model reviews to settle`, gate: got }
    log(`Cards whole book: one cycle over ${Z.KEY} on ${claimed.id}${modelReview ? ', reviewing the source of every Model draft no model chain owns' : ''}.`)
    const cycle = await Z.runCards('whole book', `${Z.OUT}\\cards`, sample, '', { modelReview })
    if (cycle.status !== 'done') {
      // v4 (S5): the code is the fact (no result); the ledger classifies it.
      await Z.failUnit('cards-book fault', claimed.id, claimed.token, {
        schema_version: 1, unit: claimed.id, input_hash: claimed.input_hash || '',
        code: 'missing_result', cause: String(cycle.error || 'the whole-book card cycle did not complete').slice(0, 800),
      })
      return { status: 'failed', ...cycle, unit: claimed.id }
    }
    const finished = await Z.finishOrFault('cards-book finish', claimed.id, claimed.token, claimed.inputs, { work: `${Z.OUT}\\cards` })
    if (finished.status !== 'done') {
      log(`Cards whole book: the cycle ran but its checked completion failed: ${finished.error}`)
      return { status: 'partial', cycle, unit: claimed.id, error: finished.error }
    }
    return { status: 'done', cycle, unit: claimed.id, completion: finished.result }
  },
})
// ==== end module zt-cards-book ====

// ==== module zt-cards-layer v2 ====
// The Cards layer of a managed run: one deterministic offline check per verified group (`cards/gNNN@1`).
// `group-finish` registers the check; this module claims the ready checks in bounded blocks and records each
// one through `cards-finish`, which re-reads the group's owned Cards, runs the shared checker in-process and
// hands every hard fault to a registered repair consumer. It never writes a Card and never starts an agent for
// a check that is not ready (R252/R254/R257).
// 2: the claim and the checked finishes of a block run as ONE command (`zt-digest finish-batch -Layer cards`), so a
// block costs one relay instead of a claim relay and one relay per group (docs/digest-claude-token-proposal.md P1/P2).
defineModule({
  id: 'zt-cards-layer', version: '2', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the per-group offline Card checks, claimed from the ledger ready work and recorded by cards-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const result = await Z.layer('cards', {
      layer: 'cards', take: 4, rounds: Z.MAX_ATTEMPTS, claimFinish: true,
    })
    log(`Cards: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} verified, ${result.dispositions.filter(d => d.status === 'failed').length} failed${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, dispositions: result.dispositions, gate: result.gate }
  },
})
// ==== end module zt-cards-layer ====

// ==== module zt-digest-prepare v13 ====
// Prepare (mode full): prepare-book-digest.ps1 -Apply writes the Chinese text, the section groups, the
// Literature tree with one anchor per group, and the shared prefixes, then binds the immutable run contract with
// the effective settings (maxAttempts, dshRunCap) and initializes the run ledger. No stage reads the
// account-wide usage ledger: run usage is recorded per provider run from that run's own result (R343-R346).
// args.book (a Scholar Archive Work id or exact title) replaces the explicit source,
// BibTeX and title: the tool resolves them through the read-only bridge, and an unresolved choice stops with its
// candidates. A resume (mode resume) prepares nothing and generates no file: it runs the one supported
// `ledger resume`, which verifies the frozen identity, reconciles stranded claims and retained results, opens
// invocation 2 and spends the one-rerun budget, then reads the retained group enumeration, settings and
// identity from the ledger (#87, R274-R285). Other modes take the group count from args.groups.
// The output also carries groupList, the per-group items the digest stage's chains are built from.
// 10 (Tree Lock S17, #140): a preparation the owner's Lock refused stops the run with the archive's own
// typed `card_locked` refusal (R326/R337), never as a generic preparation failure.
// 11 (docs/literature-only-digest-design.md): no hubs switch is frozen; the prepared run never has a hubs layer.
// 13 (S24, #167): the Term-card branch and the Term-link pass freeze their switches too (termCards, termLink),
// so a layer this run disables is excluded in the prepared target and no unit waits for work that will not come (R0430).
defineModule({
  id: 'zt-digest-prepare', version: '13', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the book (mode full, from args.book or the explicit source), resume the retained run (mode resume, one ledger resume) or take args.groups; output {groups, groupList}.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    needGroups: { type: 'array', default: ['full', 'link'], description: 'modes that need a group count' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile): the prepared-run variant' },
    instructions: { type: 'any', description: 'the digest instruction files (comma-separated, from consts.profile): the base prompt and the supplements of the book type' },
    noEventCandidates: { type: 'any', description: 'true: the group files leave out the event candidates (the event layer reads events.json)' },
  },
  run: async (ctx, p) => {
    const A = p.args
    const Z = zdMake(ctx, { args: A, mode: p.mode })
    const groupList = (count) => Array.from({ length: count || 0 }, (_, i) => ({ n: i + 1, nn: String(i + 1).padStart(2, '0') }))
    let groups = A.groups || 0
    if (p.mode === 'resume') {
      // R278-R285: the resume command verifies the retained identity, reclaims a lost owner, opens
      // invocation 2, resets only the unresolved units and spends resume_count. It never prepares and
      // never regenerates a file; a changed source, group manifest, contract or archive is refused here.
      const resumed = await Z.runScript('resume run', 'Prepare', `${Z.LEDGER} resume -Out "${Z.OUT}"`, ' (the one supported resume of the retained prepared run)')
      const facts = resumed.json
      if (!facts || !['resumed', 'nothing_to_resume'].includes(facts.status)) {
        throw new Error(`mode resume needs a resumable prepared run ledger: ${(resumed.run && resumed.run.output) || 'no ledger resume'}`)
      }
      // -Summary: the units counted per layer and state. The full status of a long book (about 450 KB
      // for 171 groups) is more than one relayed output line can carry.
      const status = await Z.runScript('resume status', 'Prepare', `${Z.LEDGER} status -Out "${Z.OUT}" -Summary`, ' (the retained run ledger, summarized)')
      const view = status.json
      if (!view) throw new Error(`mode resume needs a prepared run ledger with its frozen inputs: ${(status.run && status.run.output) || 'no ledger status'}`)
      groups = (view.layers && view.layers.digest && view.layers.digest.total) || 0
      if (!groups) {
        // grossman2002integration trial 17: a relay returned the summary without `layers` (a `layers_open`
        // map of its own instead); the digest layer's own brief gate carries the retained group count.
        const digestGate = await Z.gate('resume digest groups', 'digest')
        groups = (digestGate && digestGate.counts && (digestGate.counts.total || digestGate.counts.declared)) || 0
        log(`Resume: the status line carried no layer counts; the digest gate gives ${groups} retained groups.`)
      }
      if (!groups) throw new Error('mode resume found no retained digest groups; prepare the book again')
      const settings = view.settings || {}
      if (facts.status === 'nothing_to_resume') {
        log(`Resume: no executable or failed unit remains (${groups} retained groups, invocation ${facts.invocation}); the retained coverage stays what the ledger reports, and no resume was spent.`)
      } else {
        const reset = (facts.reset || []).length
        const reevaluated = (facts.reevaluated || []).length
        log(`Resume: invocation ${facts.invocation}, resume_count ${facts.resume_count}; ${reset} unresolved unit(s) reset, ${reevaluated} blocked dependant(s) back to dependency evaluation, ${(facts.reclaimed || []).length} lost claim(s) reclaimed, ${((facts.closure && facts.closure.downstream) || []).length} unit(s) in the closure to drain. ${groups} retained groups, variant ${view.variant}, settings maxAttempts ${settings.max_attempts}, dshRunCap ${settings.dsh_run_cap} s.`)
      }
      return { groups, groupList: groupList(groups), resumed: facts.status === 'resumed', resume: facts, settings, citation_key: view.citation_key }
    }
    if (p.mode === 'full') {
      // args.book names the book in the Scholar Archive; the tool resolves its Markdown, BibTeX and
      // title, so the explicit arguments are left out rather than passed empty.
      const book = A.book ? `-Book '${A.book}'`
        : `-Source "${A.source}" -BibTeX '${A.bibtex}' -BookTitle '${A.bookTitle}'`
      // The layer switches the run declares are frozen into the prepared target: a layer this run
      // deliberately excludes is excluded in the ledger too, so no unit waits for work that will not come.
      const switches = {}
      for (const name of ['terms', 'models', 'events', 'findings', 'figures', 'bridge', 'notation', 'boundary', 'proof', 'theory', 'termCards', 'termLink']) {
        if (typeof A[name] === 'boolean') switches[name] = A[name]
      }
      const runOptions = Object.keys(switches).length ? ` -RunOptions '${JSON.stringify(switches)}'` : ''
      // v12 (S1): one relay runs the prepare command; -Brief ends it with one short line (the group count), which
      // a Haiku agent had counted out of the whole manifest. Exit 3 with needs_selection names the Works or files
      // to choose from: the run halts for the choice (pass one Work id as args.book, or one file id as args.fileId).
      const prepared = await Z.runScript('prepare book', 'Prepare', `${Z.tool('prepare-book-digest')} ${book} -Out "${Z.OUT}" -CitationKey ${Z.KEY} -MaxChars ${A.maxChars || 20000} -MaxUnits ${A.maxUnits || 3}${A.startHeading ? ` -StartHeading '${A.startHeading}'` : ''}${A.endHeading ? ` -EndHeading '${A.endHeading}'` : ''}${A.skipHeadings ? ` -SkipHeadings '${A.skipHeadings}'` : ''}${A.fileId ? ` -FileId ${A.fileId}` : ''}${p.instructions ? ` -Instructions '${p.instructions}'` : ''}${p.noEventCandidates ? ' -NoEventCandidates' : ''} -Variant ${p.profile && p.profile.book === 'mathematics' ? 'math' : 'econ'} -MaxAttempts ${Z.MAX_ATTEMPTS} -DshRunCap ${Z.DSH_RUN_CAP}${runOptions} -Apply -Brief`, ' (it prepares the book; never retry it with other options)', { always: true })
      const line = prepared.json
      const prep = line && line.ok === true && Number(line.groups) > 0
        ? { ok: true, groups: Number(line.groups) }
        : { ok: false, error: line ? JSON.stringify(line) : String((prepared.run && prepared.run.output) || 'no output') }
      if (!prep || !prep.ok || !prep.groups) {
        log(`Preparation failed: ${prep && prep.error}`)
        // R326/R329/R337: preparation writes the Literature tree first, so a Lock on that tree stops the
        // run here with the archive's own typed refusal, never as a generic preparation failure.
        const lock = Z.lockRefusal((prep && prep.error) || '')
        if (lock) throw Z.lockStop('prepare', lock)
        throw new Error(`preparation failed: ${(prep && prep.error) || 'no report from the prepare agent'}`)
      }
      groups = prep.groups
    }
    if (!groups && (p.needGroups || ['full', 'link']).includes(p.mode)) throw new Error(`mode "${p.mode}" needs args.groups`)
    return { groups, groupList: groupList(groups), citation_key: Z.KEY }
  },
})
// ==== end module zt-digest-prepare ====

// ==== module zt-digest-report v17 ====
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
defineModule({
  id: 'zt-digest-report', version: '17', requires: ['zt-digest-lib'],
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
    cardsBook: { type: 'any', description: 'the whole-book card cycle (mode cards)' },
    explain: { type: 'any', description: 'the explain stage output (mode explain)' },
    link: { type: 'any', description: 'the link pass output' },
    linkCheck: { type: 'any', description: 'the link check card cycle' },
    linkBlocked: { type: 'array', default: [], description: 'the groups the Link pass left blocked because their digest unit is not done (#92)' },
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
    // The group stage reports one entry per digest unit (blocks of up to eight per manager); an older
    // per-item stage output (items) is still read the same way.
    const digestReports = Array.isArray(p.digest && p.digest.items)
      ? p.digest.items.map(i => ({ group: (i.item && i.item.n) ?? null, ...(i.steps && i.steps.group ? i.steps.group : {}), status: i.ok ? ((i.steps && i.steps.group && i.steps.group.status) || 'done') : 'failed', errors: i.error || null }))
      : (p.digest && p.digest.reports) || []
    const layerSummary = (value) => value && value.dispositions
      ? { status: value.status, verified: value.dispositions.filter(d => d.status === 'done').length, failed: value.dispositions.filter(d => d.status === 'failed').length, retrying: value.dispositions.filter(d => d.status === 'retry').length, open: (value.gate && value.gate.counts && (value.gate.counts.open || 0)) || 0, complete: !!(value.gate && value.gate.complete) }
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
        jobs: value.jobs ? { total: value.jobs.total || 0, done: value.jobs.done || 0, failed: (value.jobs.failed || []).map(f => f.id) } : null,
        readiness: value.readiness || null,
        publication: value.publication || null,
        completion: value.completion || null,
        counts: value.counts || null,
        no_op: value.no_op === true,
        blocked: (value.unresolved || []).slice(0, 32),
        reason: value.reason || value.error || null,
      }
    const schedule = {
      settings: { maxAttempts: Z.MAX_ATTEMPTS, dshRunCap: Z.DSH_RUN_CAP },
      digest: p.digest && (Array.isArray(p.digest.items) || Array.isArray(p.digest.reports)) ? { verified: digestReports.filter(r => r.status === 'done').length, failed: digestReports.filter(r => r.status !== 'done').length } : null,
      cards: layerSummary(p.cards),
      models: layerSummary(p.models),
      explain: layerSummary(p.explain),
      events: layerSummary((extra && extra.events) || null),
      findings: layerSummary((extra && extra.findings) || null),
      link: p.link ? { scheduled: p.link.scheduled ?? 0, skipped: p.link.skipped ?? 0, verified: p.link.verified ?? 0, failed: p.link.failed ?? 0, provider_runs: p.link.provider_runs ?? 0 } : null,
      term_cards: termSummary(p.termCards, 'the Term-card branch').status,
      term_link: termSummary(p.termLink, 'the Term-link pass').status,
    }
    if (MODE === 'bridge') return { mode: MODE, citation_key: KEY, bridge: or(p.bridge, { status: 'failed', error: 'Bridge stage produced no summary' }) }
    if (MODE === 'cards') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, cards: or(p.cardsBook, null), schedule, run_report: runReport }
    // One whole-book card cycle (R296): book is that cycle's report, link the link check's, groups the per-group checks.
    const cardsReport = { book: or(p.cardsBook, null), link: or(p.linkCheck, null), groups: layerSummary(p.cards) }
    if (MODE === 'explain') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, explain: or(p.explain, null), schedule, run_report: runReport }
    if (MODE === 'figures') return { mode: MODE, citation_key: KEY, figures: or(p.figures, null), run_report: runReport }
    if (MODE === 'audit') return { mode: MODE, citation_key: KEY || null, audit: or(p.audit, null), run_report: runReport }
    if (MODE === 'models' && !A.organize) return { mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, models: (p.models && p.models.models) || [], model_cleanup: (p.models && p.models.model_cleanup) || null, schedule, run_report: runReport }
    if (!['full', 'link', 'models', 'resume'].includes(MODE)) return { mode: MODE, dsh_model: dshModel, citation_key: KEY, ...extra, schedule }
    const modelReports = (p.models && p.models.models) || []
    const linkReports = (p.link && p.link.groups) || []
    const linkGaps = p.link
      ? { scheduled: p.link.scheduled ?? 0, skipped: p.link.skipped ?? 0, gaps: p.link.gap ?? 0, skipped_cards: p.link.skipped_cards ?? 0, linked: p.link.linked ?? 0, linked_from: p.link.linked_from ?? 0, already_linked: p.link.already_linked ?? 0, no_link: p.link.no_link ?? 0, provider_runs: p.link.provider_runs ?? 0, quality: p.link.quality || null }
      : null
    const groupCount = p.groups || 0
    const next = { workflow: 'zt-idea-grow', script: '.claude/workflows/zt-idea-grow.js', args: { book: KEY, out: '<a fresh work folder>' }, why: 'this digest built only the Literature tree; zt-idea-grow grows its Topic Cards, hubs and register' }
    ctx.log(`Next: run zt-idea-grow with book "${KEY}" and a fresh out folder: the digest wrote no Topic Card, Hub Card or register entry.`)
    if (p.termCards || p.termLink) {
      const tc = termSummary(p.termCards, 'the Term-card branch')
      const tl = termSummary(p.termLink, 'the Term-link pass')
      ctx.log(`Terms: term-cards ${tc.status}${tc.unit ? ` (${tc.unit})` : ''}${tc.reason ? `: ${tc.reason}` : ''}; term-link ${tl.status}${tl.unit ? ` (${tl.unit})` : ''}${tl.reason ? `: ${tl.reason}` : ''}.`)
    }
    return {
      mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, groups: groupCount, managers_per_pass: Z.toBlocks(Z.groupsOf(groupCount)).length,
      digest: digestReports.map(r => ({ group: r.group, status: r.status, unit: r.unit || null, disposition: r.disposition || null, owned: r.owned ?? null, errors: r.errors || null })),
      models: modelReports.map(r => ({ model: r.model, level: r.level, status: r.status, parts: Array.isArray(r.parts) ? r.parts.length : (Number(r.parts) || 0), relations: r.relations, kinds: r.kinds, charts: r.charts, derivations: r.derivations, split: r.split, moved: r.moved, shared: r.shared, tolerated: r.tolerated, style: r.style, accepted: r.accepted, dsh: r.dsh, issues: r.issues })),
      model_cleanup: (p.models && p.models.model_cleanup) || null,
      link: linkReports.map(r => ({ group: r.group, status: r.status, unit: r.unit || null, gaps: r.gaps ?? null, skipped: r.skipped ?? null, scheduled: r.scheduled ?? null, linked: r.linked ?? null, linked_from: r.linked_from ?? null, already_linked: r.already_linked ?? null, no_link: r.no_link ?? null, provider_runs: r.provider_runs ?? null, changed: r.changed || [], quality: r.quality || null, errors: r.errors || null })),
      link_gaps: linkGaps,
      link_blocked: (p.linkBlocked || []).map(b => ({ group: b.n, unit: b.unit, state: b.state, reason: b.reason })),
      terms: (p.terms && p.terms.report) || null, cards: cardsReport,
      term_cards: termSummary(p.termCards, 'the Term-card branch'),
      term_link: termSummary(p.termLink, 'the Term-link pass'),
      deepseek_usage: runUsage, cleanup: (p.cleanup && p.cleanup.cleanup) || null, run_report: runReport,
      organize: or(p.organize, null), figures: or(p.figures, null), bridge: or(p.bridge, MODE === 'full' ? (A.bridge === false ? { status: 'skipped' } : { status: 'failed', error: 'Bridge stage produced no summary' }) : null), audit: or(p.audit, null), finalize: or(p.finalize, null),
      schedule,
      next,
      ...extra,
    }
  },
})
// ==== end module zt-digest-report ====

// ==== module zt-dsh-cleanup v11 ====
// R231/R322–R330: the run's cleanup is settled from ledger evidence, never from a workspace listing's titles.
// v11 (2026-10-23): no model reads, relays or decides any cleanup fact. One Haiku agent makes three calls:
// `zt-digest cleanup-run -Step plan` records the evidence already on disk and writes the plan file (every
// recorded id still to delete, every recorded workspace not yet removed); the bridge's `deepseek_settle` runs
// that plan in the MCP server that holds the conversations open (it closes each first) and writes the results
// file; `cleanup-run -Step apply` records every fact and settles every ready cleanup unit through its checked
// verifier. No id list passes through the agent: it sees two paths and three short lines. The stage had spent
// a Haiku agent per 40 conversations, per 8 workspaces and (until v10) per cleanup unit, about 150 agents one
// after another on ebrill2001modern trial 1. A workspace that still lists a conversation the run did not record
// is kept and reported (that member is the user's); a refused deletion stays a retryable provider fact.
// The run report (#85, R331–R346) is rendered here, after cleanup settles: every stage of the run has
// finished, so the report describes the whole invocation. Its usage section is the exact per-run usage
// the ledger recorded from each run's own result; this module never reads or differences the account-wide
// usage ledger, because the difference of two rolling totals is not a run's cost (R344).
defineModule({
  id: 'zt-dsh-cleanup', version: '11', requires: ['zt-digest-lib'],
  summary: 'ZT digest: settle every recorded DeepSeek conversation and workspace from ledger evidence, then render the verified run report.',
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
      properties: { plan: { type: 'string' }, settle: { type: 'string' }, ok: { type: 'boolean' }, output: { type: 'string' } },
      required: ['ok', 'output'],
    }
    const step = (name) => `${Z.DIGEST} cleanup-run -Out "${OUT}" -Step ${name}${name === 'apply' ? ' -Owner "cleanup-run"' : ''}`
    const got = await dsmpSlot(() => Z.zAgent(
      `Settle the DeepSeek Harness conversations and workspaces of one digest run. A script decides every id; you make three calls and copy short lines. Never delete, list or remove anything yourself, and never call deepseek_delete or deepseek_remove_workspace.
${Z.RUNNER}
1. Run this command once in the foreground with timeout_ms: ${Z.COMMAND_TIMEOUT_MS} and read its last line (JSON; it changes no Card):
   ${step('plan')}
2. Load the tool with one ToolSearch call: query "select:mcp__deepseek__deepseek_settle". Call deepseek_settle once with plan_file "${PLAN}" and results_file "${RESULTS}". Put its answer, as one line, in settle. If the tool is missing or fails, put the error in settle and skip to step 4.
3. Run this command once with timeout_ms: ${Z.COMMAND_TIMEOUT_MS}; it records the results and settles the cleanup units (it changes no Card):
   ${step('apply')}
   Return ok (true when it exits without an error) and output = its last line exactly. Stop here.
4. Only when step 2 failed: run ${Z.DIGEST} cleanup-finish -Out "${OUT}" -Ready -Owner "cleanup-run" once with timeout_ms: ${Z.COMMAND_TIMEOUT_MS} (it settles the units whose conversations are already gone) and return ok and output = its last line exactly.
Return plan = the last line of step 1.`,
      { label: 'cleanup: settle the run', phase: 'Cleanup', model: 'haiku', schema: SETTLED },
    ))
    const line = got && got.ok ? zdLastJson(got.output) : null
    const ran = line && line.kind === 'cleanup_run' ? line : null
    const ready = ran ? ran.ready : line && line.kind === 'cleanup_ready' ? line : null
    if (!ran) log(`Cleanup: the provider batch did not run (${String((got && (got.settle || got.output)) || 'no answer').slice(0, 240)}).`)
    else log(`Cleanup: deleted ${ran.deleted}, already gone ${ran.absent}, failed ${ran.failed_count}; workspaces removed ${ran.workspaces_removed}, kept ${ran.workspaces_kept}${ran.unknown_count ? `; ${ran.unknown_count} conversation(s) the run did not record stay: ${JSON.stringify(ran.unknown)}` : ''}${ran.unrecorded_count ? `; ${ran.unrecorded_count} fact(s) not recorded: ${JSON.stringify(ran.unrecorded)}` : ''}.`)
    const settled = []
    if (ready) {
      const open = (ready.retry || []).concat(ready.failed || [])
      log(`Cleanup: ${ready.done} of ${ready.claimed} claimed unit(s) settled (${ready.partial} partial); ${ready.retry_count} retryable, ${ready.failed_count} failed${open.length ? `: ${JSON.stringify(open.slice(0, 4))}` : ''}.`)
      for (const unit of open) settled.push({ unit: unit.unit, status: unit.state === 'retry' ? 'retry' : 'failed', error: unit.cause })
      if (ready.book && ready.book.unit) settled.push(ready.book.ok ? { unit: ready.book.unit, status: ready.book.status || 'done', json: ready.book } : { unit: ready.book.unit, status: 'retry', error: ready.book.cause })
    }
    const run = settled.find((s) => s.unit === 'cleanup/book@1' && s.json) || null
    const cleanup = {
      provider: ran ? { deleted: ran.deleted, absent: ran.absent, failed: ran.failed_count, workspaces_removed: ran.workspaces_removed, workspaces_kept: ran.workspaces_kept, unknown: ran.unknown_count, unrecorded: ran.unrecorded_count } : null,
      settled: ready ? { claimed: ready.claimed, done: ready.done, partial: ready.partial, retry: ready.retry_count, failed: ready.failed_count } : null,
      units: settled.map((s) => ({ unit: s.unit, status: s.status, cleanup_complete: s.json ? s.json.cleanup_complete : null, content_complete: s.json ? s.json.content_complete : null, error: s.error })),
      workspace_removed: ran ? ran.workspaces_kept === 0 && !(ran.failed || []).some((f) => f.workspace) : false,
      files_removed: run && run.json ? Number(run.json.scratch_removed) || 0 : 0,
      content_complete: run ? run.json.content_complete : null,
      cleanup_complete: run ? run.json.cleanup_complete : null,
      status: run ? run.json.status : 'unsettled',
    }
    if (cleanup.status === 'partial') log(`Cleanup is partial: ${JSON.stringify((run.json && run.json.partial_reasons) || [])}`)
    // R331: the verified run report. Every stage has settled, so the report describes this invocation:
    // completeness, gaps, current placeholders, timings and exact per-run usage. A failed render leaves the
    // run partial and names why; it never substitutes an agent's summary for the report.
    const rendered = await Z.ledgerReport('report the run from the ledger')
    let facts = null
    if (!rendered.ok) {
      log(`The run report failed: ${rendered.error}`)
    } else {
      facts = rendered.json
      const c = facts.completeness || {}
      const u = facts.usage || {}
      log(`Run report: settled ${c.settled}, content ${c.content_complete}, quality ${c.quality_complete}, cleanup ${c.cleanup_complete}, complete ${c.complete}; ${(facts.gaps || []).length} gap(s); placeholders ${facts.placeholders && facts.placeholders.count}; usage ${u.known_runs} run(s) known, ${u.missing_runs} missing. Report: ${OUT}\\run-report.md`)
    }
    const usage = facts ? facts.usage : null
    return {
      status: facts ? (facts.completeness && facts.completeness.complete ? 'complete' : 'partial') : (cleanup.status === 'complete' ? 'partial' : cleanup.status),
      cleanup,
      usage,
      report: facts ? {
        json: (facts.report && facts.report['run-report.json'] && facts.report['run-report.json'].path) || `${OUT}\\run-report.json`,
        markdown: (facts.report && facts.report['run-report.md'] && facts.report['run-report.md'].path) || `${OUT}\\run-report.md`,
        completeness: facts.completeness,
        counts: facts.counts,
        gaps: facts.gaps,
        placeholders: facts.placeholders,
        timing: facts.timing,
        resume: facts.resume,
        bridge: facts.bridge,
        publications: facts.publications,
      } : null,
      error: facts ? undefined : rendered.error,
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

// ==== module zt-event-layer v4 ====
// The Event layer of the economics set: the ledger's ready event work, worked while the digest groups are
// still running. `event-compiler` plans one job per few groups from the book's dated sentences and each
// group's own Idea Cards; DeepSeek writes the events as JSON and runs the job's SELF-CHECK; the compiler
// creates each Event Card beside the Idea Card it supports (or adds this book's source to an existing Event
// Card) and links it from every supported Idea Card with the kind word 案例. This module claims the ready
// `events/gNNN@1` units in bounded blocks, runs that compiler chain once per block in one manager (R250), then
// records each group through `events-finish`, so a group whose job has not been applied stays open instead of
// being claimed complete. The layer gate is the stage's result (R257).
// 4: the chain ends with the block's checked finishes in one command (`zt-digest finish-batch`), run by the chain's own
// manager; a unit its line does not answer is finished by one relay, then on its own (docs/digest-claude-token-proposal.md P1/P2).
defineModule({
  id: 'zt-event-layer', version: '4', requires: ['zt-digest-lib'],
  summary: 'ZT digest event layer: the ledger\'s ready Event work, plan → DeepSeek JSON → apply → per-group checked completion.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\events`
    const estep = (s, more = '') => `${Z.tool('event-compiler')} -Step ${s} -Staging "${Z.OUT}"${more}`
    const result = await Z.layer('events', {
      layer: 'events', take: 4, rounds: Math.max(3, Z.MAX_ATTEMPTS), wait: 600, producer: 'digest', phase: 'Events',
      work: async (units) => {
        // One compiler chain for this block: plan → its jobs → apply → report. Content failures are
        // recorded against the units they leave unfinished, not thrown out of the stage.
        const chained = await Z.chain(`events ${units.map(u => u.id).join(', ')}`, {
          dsh: work, unit: units[0].id, token: units[0].token, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Events', writes: `${work}\\out`,
          steps: [
            { name: 'plan', command: estep('plan'), jobs: true },
            { name: 'apply', command: estep('apply', ' -Apply'), optional: true },
            { name: 'report', command: estep('report'), optional: true },
            { name: 'finish', command: Z.finishBatchCommand(units), always: true },
          ],
          cleanup: true, keep: true,
        })
        const applied = Z.jsonOf(chained, 'apply')
        const plan = Z.jsonOf(chained, 'plan')
        log(`Events: ${plan ? `${plan.candidates || 0} dated sentences in ${plan.groups || 0} groups, ${(plan.jobs || []).length} DeepSeek jobs` : 'plan gave no output'}; ${applied ? `${applied.created} Event Cards made, ${applied.merged} merged, ${applied.linked} Links, ${(applied.refused || []).length} refused` : 'apply gave no output'}.`)
        return Z.finishUnits(`events ${units.map(u => u.id).join(', ')}`, units, Z.jsonOf(chained, 'finish'))
      },
    })
    log(`Events: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} groups checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, dispositions: result.dispositions, gate: result.gate }
  },
})
// ==== end module zt-event-layer ====

// ==== module zt-model-lib v10 ====
// The model layer's machinery (docs/zt-digest-framework.md), moved from zt-digest-lib: the model list
// (list-models.ps1) and the model compiler per batch: DeepSeek brief → Sonnet architect plan → DeepSeek fill,
// derive, review, split → report and clean-up (model-compiler.ps1 does every structural step). A book type
// installs the model layer with its profile (economics: the economic architect; mathematics: the math
// supplements and no Theory Card). zmlMake(ctx, params) returns listModels, elaborateAll, logModels, modelCleanup.
// 9 (docs/digest-claude-token-proposal.md P1/P7): a batch registers its models' references with ONE `model-deps -Units`
// command instead of one relay per model, and a close round that sends nothing back writes the batch report in the
// same process (`-Step close -Report`), so such a batch needs no report step of its own.
// 10 (docs/workflow-simplicity-proposal.md S1, S10): the model list runs through the relay (a Haiku agent had copied the
// array back and once dropped `form`), and whether a batch fills only or derives and reviews is read from the plans the
// architect wrote (`model -Step plan-status`), never from its answer; the answer keeps only its notes and summary.
function zmlMake(ctx, p) {
  const Z = zdMake(ctx, p)
  const { A, KEY, OUT, SCRIPTS, DSH_EFFORT, LIMITS, COMPILER, FILL_SKILLS, profile, ARCH_DOC, ARCH_SUPPLEMENT, MODEL_WORK, PER_SONNET,
    PER_CONCEPT, DSH_PER_BATCH, REBUILD, LEVEL, BRIEF_EFFORT, DERIVE_EFFORT, REVIEW_EFFORT, REVIEW_ROUNDS, COMPILER_DOCS, SKIP_FORMS,
    zAgent, drive, answerLines } = Z
  const MODEL_LIST = {
    type: 'object',
    properties: {
      ok: { type: 'boolean' }, error: { type: 'string' },
      models: { type: 'array', items: { type: 'object', properties: { address: { type: 'string' }, title: { type: 'string' }, level: { type: ['string', 'null'] }, parts: { type: 'number' }, issues: { type: 'number' }, faults: { type: 'number' } }, required: ['address'] } },
    },
    required: ['ok', 'models'],
  }
  const listModels = async (selector, label) => {
    const { run, json } = await Z.runScript(label, 'Models', `${Z.tool('list-models')} -Staging "${OUT}" ${selector}`, ' (it lists the Model Cards; it writes no Card)')
    return json && Array.isArray(json.models) ? { ok: true, models: json.models } : { ok: false, models: [], error: String((run && run.output) || 'no output') }
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
  // The model-deps lines of a batch's planned models, in their order (null where none is readable), from ONE
  // `model-deps -Units` command. A batch line no relay returned whole is never read as "resolved": the ledger
  // says which units model-deps already registered to wait (registered) or refused (failed), and only a unit
  // still claimed under its token runs model-deps again on its own.
  const depsOf = async (planned, claims, tag) => {
    const units = planned.map((m) => claims[m.address])
    const r = await Z.runScript(`model-deps: ${tag}`, 'Models',
      `${Z.DIGEST} model-deps -Out "${OUT}" -Units "${units.map((u) => `${u.unit}:${u.token}`).join(',')}"`,
      ' (it registers the models\' references to parts of other models; it writes no Card)')
    const rows = r.json && r.json.kind === 'model_deps_batch' && Array.isArray(r.json.results) ? r.json.results : null
    if (rows) return units.map((u) => { const row = rows.find((x) => x && x.unit === u.unit); return row && row.ok !== undefined ? row : null })
    log(`model-deps ${tag}: no readable batch line (${String((r.run && r.run.output) || 'no output').slice(0, 200)}); the ledger decides each model.`)
    const read = await Z.runScript(`model-deps: ${tag}: states`, 'Models', `${Z.LEDGER} status -Out "${OUT}" -Units ${units.map((u) => u.unit).join(',')}`, ' (it reads the units\' states; it changes nothing)')
    const states = read.json && Array.isArray(read.json.units) ? read.json.units : []
    return Promise.all(units.map(async (u, i) => {
      const held = states.find((x) => x && x.id === u.unit)
      if (held && held.state === 'registered') return { ok: true, waiting: true, state: 'registered' }
      // A unit this batch no longer holds (model-deps recorded its refused reference) is not filled here.
      if (held && (held.state !== 'claimed' || Number(held.token) !== Number(u.token))) return { ok: false, state: held.state }
      const one = await Z.runScript(`model-deps: ${planned[i].address}`, 'Models',
        `${Z.DIGEST} model-deps -Out "${OUT}" -Model ${Z.psQuote(planned[i].address)} -Token ${u.token}`,
        ' (it registers the model\'s references to parts of other models; it writes no Card)')
      return one.json || null
    }))
  }
  const slug = (address) => String(address).replace(/[\/|]/g, '_')
  const managerFolders = []
  let dshJobs = 0
  const compileBatch = async (batch, kind, claims = null) => {
    const name = `m-${slug(batch[0].address)}`
    const dsh = `${MODEL_WORK}\\${name}`
    managerFolders.push(dsh)
    const tag = `${kind} ${batch[0].address}${batch.length > 1 ? ` +${batch.length - 1}` : ''}`
    const step = (s, extra = '') => `${COMPILER} -Step ${s} -Work "${dsh}" -Staging "${OUT}" ${LIMITS}${COMPILER_DOCS}${extra}`
    const sessions = {}
    const run = async (what, command, skills, effort, opts = {}) => {
      const r = await drive(`${what}: ${tag}`, { command, dsh, skills, effort, sessions: opts.continues ? sessions : null, cleanup: opts.cleanup })
      for (const x of (r && r.results) || []) {
        dshJobs++
        if (x.session_id) sessions[x.id] = x.session_id
      }
      if (!r || !r.ok) log(`${what} ${tag}: ${(r && r.error) || 'no report from its driver'}`)
      return r
    }
    const splitNeeds = async (driven) => {
      const need = answerLines(driven).filter(l => l.status === 'split' && l.address && l.need > 1).map(l => `${l.address}=${l.need}`)
      if (need.length) await run('split', step('split', ` -Need "${need.join(';')}"`), FILL_SKILLS, DERIVE_EFFORT, { continues: true })
    }
    const failed = (why) => batch.map(m => ({ model: m.address, status: 'failed', parts: [], charts: 0, issues: [why] }))
    // 1. Brief: the script writes each context pack; DeepSeek reads the source and writes brief.md.
    const brief = await run('brief', step('brief', ` -Models "${batch.map(m => m.address).join(',')}"${REBUILD ? ' -Rebuild' : ''}${LEVEL ? ` -Level ${LEVEL}` : ''}`), [], BRIEF_EFFORT)
    const briefed = zdLastJson(brief && brief.output)
    if (!briefed) return failed(`brief failed: ${(brief && brief.error) || 'no output'}`)
    const skipped = new Set(briefed.skipped || [])
    const todo = batch.filter(m => !skipped.has(m.address))
    if (!todo.length) return batch.map(m => ({ model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }))
    // 2. Plan: Sonnet reads the briefs and writes plan.json for each model; nothing else.
    const entry = (m, i) => `## Model ${i + 1}
MODEL: ${m.address} (${m.title || ''})
DIR: ${dsh}\\${slug(m.address)}
LEVEL NOW: ${m.level || 'none'}`
    const planned = await zAgent(
      `You are the Sonnet architect in the Models stage of the ZET digest of 《${A.bookTitle || KEY}》. Read your instructions first: ${ARCH_DOC}${ARCH_SUPPLEMENT ? `, then ${profile.architectSupplement.name || 'the supplement'} ${ARCH_SUPPLEMENT}` : ''} (Read tool). Then follow them for each model in turn. These models are near each other in the book: where one part serves two of them, plan it in the first and take it in from the second (\`<model>#<id>\`).
COMPILER: ${COMPILER}
WORK: ${dsh}
LIMITS: ${LIMITS}${REBUILD ? '\nREBUILD: rebuild each model even if it already has parts.' : ''}${LEVEL ? `\nLEVEL: ${LEVEL}` : ''}

${todo.map(entry).join('\n\n')}

Return plans: one entry per model, in this order.`,
      { label: `plan: ${tag}`, phase: 'Models', model: 'sonnet', schema: PLANS },
    )
    // S10: the plans as the architect wrote them (plan.json per model), read by the compiler; the answer adds notes.
    const status = await Z.runScript(`plan status: ${tag}`, 'Models', step('plan-status'), ' (it reads the plans the architect wrote; it changes nothing)')
    const said = (planned && planned.plans) || []
    const plans = status.json && Array.isArray(status.json.plans)
      ? status.json.plans.map((x) => ({ ...(said.find((y) => y.model === x.model) || {}), model: x.model, status: x.status, level: x.level, parts: x.parts, derivations: x.derivations }))
      : todo.map((m) => ({ model: m.address, status: 'failed', notes: [], summary: '' }))
    if (!status.json) log(`plan status ${tag}: no readable line (${String((status.run && status.run.output) || 'no output').slice(0, 200)}); the batch builds nothing.`)
    if (!plans.some(p => p.status === 'planned')) {
      await run('clean up', step('report'), [], DSH_EFFORT, { cleanup: true })
      return batch.map(m => skipped.has(m.address) ? { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
        : { model: m.address, status: (plans.find(p => p.model === m.address) || {}).status || 'failed', parts: [], charts: 0, issues: planned ? [] : ['no plan from the architect'] })
    }
    // R233/R234: a plan may take in or rest on a part of a model outside this batch (`X#A`); the compiler resolves
    // it only after `zt-digest model-deps` has registered the producer and written the part's checked address
    // into this batch's state. Each planned model with a claim registers its references before the fill; a
    // model whose producer is not done yet is released to wait (the ledger readies it again once the producer
    // is done), and a reference the ledger refuses is recorded as that unit's content fault by model-deps itself.
    // Nothing called it before (grossman2002integration trial 8: batch 2 took `2|a#DA` from batch 1 and all
    // three of its models failed at the fill with "model … has no part DA yet").
    const deferred = new Map()
    if (claims) {
      const planned = todo.filter(m => (plans.find(p => p.model === m.address) || {}).status === 'planned' && claims[m.address])
      const registered = await depsOf(planned, claims, tag)
      registered.forEach((line, i) => {
        const at = planned[i].address
        if (line && line.ok && line.waiting) deferred.set(at, 'waiting')
        else if (line && line.ok === false) deferred.set(at, 'refused')
        else if (!line) log(`model-deps ${at}: no readable line${line === null ? '' : ` (${line})`}`)
      })
      if (deferred.size) log(`${tag}: ${[...deferred].map(([at, why]) => `${at} ${why === 'waiting' ? 'waits for the model it takes a part from' : 'has a reference the ledger refused'}`).join('; ')}.`)
    }
    // A released or refused model leaves the batch before the fill (`-Step defer`): no later step writes or
    // reports it, and its plan is set aside so the batch that claims it again plans it afresh. On
    // grossman2002integration trial 9 the fill held jobs for all four models after three were released to wait.
    const outcome = (m) => ({ model: m.address, status: deferred.get(m.address), parts: [], charts: 0, issues: [deferred.get(m.address) === 'waiting' ? 'waits for the model it takes a part from' : 'a reference to another model\'s part was refused'] })
    if (deferred.size) {
      const dropped = await Z.runScript(`defer: ${tag}`, 'Models', step('defer', ` -Models "${[...deferred.keys()].join(',')}"`), ' (it takes the waiting models out of this batch; it writes no Card)')
      if (!(dropped.json && dropped.json.ok)) {
        log(`defer ${tag}: the waiting models could not leave the batch, so nothing is filled (${String((dropped.run && dropped.run.output) || 'no output').slice(0, 200)})`)
        await run('clean up', step('report'), [], DSH_EFFORT, { cleanup: true })
        return batch.map(m => skipped.has(m.address) ? { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
          : deferred.has(m.address) ? outcome(m) : { model: m.address, status: 'failed', parts: [], charts: 0, issues: ['the batch could not set aside its waiting models before the fill'] })
      }
      if (!todo.some(m => !deferred.has(m.address) && (plans.find(p => p.model === m.address) || {}).status === 'planned')) {
        await run('clean up', step('report'), [], DSH_EFFORT, { cleanup: true })
        return batch.map(m => skipped.has(m.address) ? { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
          : deferred.has(m.address) ? outcome(m) : { model: m.address, status: (plans.find(p => p.model === m.address) || {}).status || 'failed', parts: [], charts: 0, issues: [] })
      }
    }
    // 3. Fill, 4. derive and 5. review (only where the plans have Derivation parts).
    await splitNeeds(await run('fill', step('apply'), FILL_SKILLS, DSH_EFFORT))
    if (plans.some(p => p.status === 'planned' && p.derivations > 0)) {
      await splitNeeds(await run('derive', step('derive'), FILL_SKILLS, DERIVE_EFFORT))
      for (let round = 0; round < REVIEW_ROUNDS; round++) {
        const review = await run(`review ${round + 1}`, step('review', round ? ' -Changed' : ''), [], REVIEW_EFFORT)
        const verdicts = answerLines(review).reduce((s, l) => ({ split: s.split + (Number(l.split) || 0), fix: s.fix + (Number(l.fix) || 0) }), { split: 0, fix: 0 })
        log(`review ${round + 1} ${tag}: ${verdicts.split} parts to split, ${verdicts.fix} to fix`)
        if (!verdicts.split && !verdicts.fix) break
        await splitNeeds(await run(`split ${round + 1}`, step('split'), FILL_SKILLS, DERIVE_EFFORT, { continues: true }))
      }
    }
    // 6. Close: the build faults the compiler still sees (closure, forward, reach-in, unanchored) go back
    // once, as 修正 notes, to the conversations that wrote those Cards; model-finish refuses a model that
    // keeps any (grossman2002integration trial 6: all four models were refused, and a concept model has no
    // review round that could have caught them).
    // Close runs again while its last round sent notes and faults remain, at most CLOSE_ROUNDS rounds: a writer
    // can answer a note and leave the fault in place (trials 17-19: 6|a kept a rests-on sentence to another model,
    // 7|a a malformed marker, and model-finish refused both after the one close round).
    const CLOSE_ROUNDS = 3
    // A close round that sends nothing back also writes the batch report (-Report); the report step runs only when
    // the last close round still sent follow-ups or its line came back unreadable.
    let closeReport = null
    for (let round = 0; round < CLOSE_ROUNDS; round++) {
      const closed = await run(round ? `close ${round + 1}` : 'close', step('close', ' -Report'), FILL_SKILLS, DERIVE_EFFORT, { continues: true })
      const closeLine = zdLastJson(closed && closed.output)
      closeReport = closeLine && closeLine.report && Array.isArray(closeLine.report.models) ? closeLine.report : null
      if (!closeLine || !closeLine.faults) break
      log(`close ${round + 1} ${tag}: ${closeLine.faults} build fault(s) sent back in ${(closeLine.jobs || []).length} follow-up(s)${(closeLine.unowned || []).length ? `, ${closeLine.unowned.length} in Cards no job of this batch wrote` : ''}`)
      if (!(closeLine.jobs || []).length) break
    }
    // 7. Report (script) and clean-up.
    const reported = closeReport || zdLastJson((await run('report', step('report'), [], DSH_EFFORT, { cleanup: true }) || {}).output)
    const reports = (reported && reported.models) || []
    return batch.map(m => {
      const r = reports.find(x => x.model === m.address)
      const pl = plans.find(x => x.model === m.address) || {}
      if (skipped.has(m.address)) return { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
      if (deferred.has(m.address)) return outcome(m)
      if (!r) return { model: m.address, status: 'failed', parts: [], charts: 0, issues: ['no report from the compiler'] }
      const faults = Object.entries(r.faults || {}).filter(([, n]) => n).map(([k, n]) => `${k} ${n}`)
      return {
        ...r,
        issues: [...(r.issues || []), ...(r.error ? [r.error] : []), ...faults, ...((r.todo || []).length ? [`still 待写: ${r.todo.join(', ')}`] : [])],
        notes: [...(r.notes || []), ...(pl.notes || [])], summary: pl.summary,
      }
    })
  }
  const groupNo = (m) => Number(m.group) || Number(String(m.address).split('/')[1].split('|')[0]) || 0
  const elaborateAll = async (models, label, claims = null) => {
    const only = Array.isArray(A.only) && A.only.length ? new Set(A.only) : null
    // A model whose level the run changes is never skipped; a form the profile skips (math: theory) never goes in.
    const todo = models.filter(m => !SKIP_FORMS.includes(m.form)).filter(m => !only || only.has(m.address)).filter(m => REBUILD || (LEVEL && m.level !== LEVEL) || !(m.parts > 0 && !m.issues))
      .sort((a, b) => groupNo(a) - groupNo(b) || String(a.address).localeCompare(String(b.address)))
    if (todo.length < models.length) log(`${label}: ${models.length - todo.length} of ${models.length} models already have their parts; skipped.`)
    const concept = todo.filter(m => (LEVEL || m.level) === 'concept')
    const formal = todo.filter(m => (LEVEL || m.level) !== 'concept')
    const batches = []
    for (let i = 0; i < concept.length; i += PER_CONCEPT) batches.push({ kind: 'concept', models: concept.slice(i, i + PER_CONCEPT) })
    for (let i = 0; i < formal.length; i += PER_SONNET) batches.push({ kind: 'formal', models: formal.slice(i, i + PER_SONNET) })
    log(`${label}: ${todo.length} models (${concept.length} concept, ${formal.length} formal or undecided) in ${batches.length} batches, one Sonnet architect each; up to ${DSH_PER_BATCH} DeepSeek runs at a time per batch.`)
    const results = (await parallel(batches.map(b => () => compileBatch(b.models, b.kind, claims).catch(error => { log(`batch ${b.models[0].address}: ${error}`); return null })))).flatMap((r, i) => r || batches[i].models.map(() => null))
    const ordered = batches.flatMap(b => b.models)
    return results.map((r, i) => r || { model: ordered[i].address, status: 'failed', parts: [], charts: 0, issues: ['no report from its batch'] })
  }
  const kindTally = (reports) => {
    const tally = {}
    for (const r of reports) for (const [kind, n] of Object.entries(r.kinds || {})) tally[kind] = (tally[kind] || 0) + n
    return Object.entries(tally).sort((a, b) => b[1] - a[1]).map(([kind, n]) => `${kind} ${n}`).join(', ') || 'no kinds reported'
  }
  // The compiler's report line gives counts (parts, core, moved); an older line gave lists. Either reads.
  const nOf = (v) => Array.isArray(v) ? v.length : (Number(v) || 0)
  const coreOf = (r) => r.core != null ? (Number(r.core) || 0) : Array.isArray(r.parts) ? r.parts.filter(pp => pp && pp.emphasis === 'core').length : 0
  const logModels = (reports) => {
    const done = reports.filter(r => r.status === 'done')
    const sum = (field) => done.reduce((s, r) => s + (r[field] || 0), 0)
    const deriv = (field) => done.reduce((s, r) => s + ((r.derivations && r.derivations[field]) || 0), 0)
    log(`Build: ${deriv('total')} Derivation parts in ${deriv('chains')} chains of two or more (longest ${done.reduce((m, r) => Math.max(m, (r.derivations && r.derivations.longest) || 0), 0)}), ${sum('split')} parts split by review or for the Card size, ${done.reduce((s, r) => s + nOf(r.moved), 0)} Cards moved into ranges, ${sum('shared')} shared parts taken in, ${sum('tolerated')} tolerated premises, ${sum('style')} style faults left, ${done.reduce((s, r) => s + ((r.todo || []).length), 0)} parts still 待写; ${dshJobs} DeepSeek jobs.`)
    log(`Levels: ${done.filter(r => r.level === 'concept').length} concept models (level 1), ${done.filter(r => r.level === 'formal').length} formal models (level 2), ${done.filter(r => !r.level).length} without a level.`)
    log(`Models: ${done.length} structured (${done.reduce((s, r) => s + nOf(r.parts), 0)} parts, ${done.reduce((s, r) => s + coreOf(r), 0)} core, ${sum('relations')} relations (${kindTally(done)}), ${sum('neighbours')} neighbours, ${sum('loose')} loose, ${sum('charts')} charts), ${reports.filter(r => r.status === 'skipped').length} skipped, failed ${reports.filter(r => r.status === 'failed').map(r => r.model).join(', ') || 'none'}; open issues ${reports.reduce((s, r) => s + nOf(r.issues), 0)}.`)
  }
  // R323: the model stage no longer deletes conversations or folders itself. Every batch conversation
  // was recorded when the provider returned its id, so the run's cleanup stage deletes exactly those
  // ids after the owning work settles; a batch folder is disposable scratch only when its accepted
  // plan is retained first (R328/R329). This resolves that the sweep owes nothing.
  const modelCleanup = async () => {
    if (!managerFolders.length) return null
    log(`Model clean-up: ${managerFolders.length} batch folders stay until the run-level cleanup settles their recorded conversations.`)
    return { late_deleted: 0, folders: managerFolders.length, deferred: true }
  }


  return { Z, listModels, elaborateAll, logModels, modelCleanup, dshJobs: () => dshJobs }
}

defineModule({
  id: 'zt-model-lib', version: '10', library: true, requires: ['zt-digest-lib'],
  summary: 'Library: the model list and the model compiler (brief, architect plan, fill, derive, review, split) for the model layer.',
})
// ==== end module zt-model-lib ====

// ==== module zt-explain v7 ====
// Explanations of finished Derivation parts (R219/R301/R302): the ledger's ready `explain/<model-key>@1` work,
// which a checked Model completion registers. Each claimed block is written by the existing explanation chain
// (plan one job per chain, DeepSeek JSON, the script writes 思路/说明/解读 and refuses a changed 起点/step/结果,
// one refused part going back to its own conversation once); every unit is then recorded through
// `explain-finish`, which re-reads the parts and refuses a part that still lacks text. Runs in mode explain and
// after the model layer in mode full.
// 6: the chain ends with the block's checked finishes in one command (`zt-digest finish-batch`), run by the chain's own
// manager; a unit its line does not answer is finished by one relay, then on its own (docs/digest-claude-token-proposal.md P1/P2).
defineModule({
  id: 'zt-explain', version: '7', requires: ['zt-model-lib'],
  summary: 'ZT digest: the ledger\'s ready explanations of finished Derivation parts, written by the explanation chain and recorded by explain-finish.',
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
    const xstep = (s, extra = '') => `${Z.COMPILER} -Step ${s} -Work "${work}" -Staging "${Z.OUT}" ${Z.LIMITS}${extra}`
    const result = await Z.layer('explain', {
      // hold: the chain renews only its first unit's claim; the driver keeps every claim of the block current.
      layer: 'explain', take: 4, rounds: Z.MAX_ATTEMPTS, phase: 'Models', hold: true,
      work: async (units) => {
        const models = units.map(u => (u.inputs && u.inputs.model) || '').filter(Boolean)
        // v7 (S8): the claimed models, whatever args.only says (the two branches were identical).
        const only = ` -Models "${models.join(',')}"`
        const chained = await Z.chain(`explain ${models.join(', ')}`, {
          dsh: work, unit: units[0].id, token: units[0].token, skills: Z.FILL_SKILLS, effort: Z.DSH_EFFORT, phase: 'Models', writes: `${work}\\out`,
          steps: [
            { name: 'plan', command: xstep('explain', `${only}${A.explainMax ? ` -MaxParts ${A.explainMax}` : ''}`), jobs: true },
            { name: 'apply 1', command: xstep('explain-apply'), jobs: true },
            { name: 'apply 2', command: xstep('explain-apply'), jobs: true, optional: true },
            { name: 'report', command: xstep('explain-report'), optional: true },
            { name: 'finish', command: Z.finishBatchCommand(units), always: true },
          ],
          cleanup: true, keep: true,
        })
        const planned = Z.jsonOf(chained, 'plan')
        const reported = Z.jsonOf(chained, 'report')
        const refused = (Z.jsonOf(chained, 'apply 1') || {}).refused || []
        log(`Explain: ${planned ? `${planned.parts || 0} Derivation parts in ${(planned.jobs || []).length} DeepSeek jobs` : 'plan gave no output'}; ${reported ? `${(reported.models || []).length} models reported` : 'no report'}; ${refused.length} refused.`)
        return Z.finishUnits(`explain ${models.join(', ')}`, units, Z.jsonOf(chained, 'finish'))
      },
    })
    log(`Explain: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} models checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, dispositions: result.dispositions, gate: result.gate }
  },
})
// ==== end module zt-explain ====

// ==== module zt-figures v4 ====
// The book's figures in its Cards: attach-figures.ps1 puts every figure a Card names but does not show yet into
// that Card, and links the book's documents from the Literature Card. It runs after Organize, which rewrites bodies.
defineModule({
  id: 'zt-figures', version: '4', requires: ['zt-digest-lib'],
  summary: 'ZT digest: show the book\'s own figures in the Cards that name them (attach-figures.ps1).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('figures', ['figures'])) return { status: 'done', reused: true }
    const A = Z.A
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts
    // by hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the
    // revision stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const { run, json } = await Z.runScript(`${layer}: publish`, 'Figures', `${Z.DIGEST} finalize -Step stage -Out "${Z.OUT}" -Layer ${layer}${more}`, ' (it records the stage publication in the ledger)')
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(run && run.output) || 'no output'}`)
      return json || { layer, state: 'failed', error: (run && run.output) || 'no output' }
    }
    const { run, json: summary } = await Z.runScript('show figures', 'Figures',
      `${Z.tool('attach-figures')} -CitationKey ${Z.KEY} -Staging "${Z.OUT}"${A.fileId ? ` -FileId ${A.fileId}` : ''} -Report "${Z.OUT}\\figures-report.json" -Apply`,
      ' (it may take several minutes)')
    log(summary ? `Figures: ${summary.shown} figures shown in ${summary.cards} Cards (${summary.figures} kept, ${summary.labelled} labelled); documents ${summary.literature ? 'linked' : 'already linked'}.` : `Figures failed: ${run && run.output}`)
    if (!summary) return { status: 'failed', error: run && run.output }
    const publication = await publish('figures', ` -Report "${Z.OUT}\\figures-report.json"`)
    return { status: 'done', ...summary, publication }
  },
})
// ==== end module zt-figures ====

// ==== module zt-finalize v2 ====
// The declared finite finalization branch (R299/R300, #86): the read-only audit plan, at most ONE
// correction cycle scoped to the plan's Cards, the exact changed-Card/touched-tree manifest, the
// dependency-scoped refresh of the affected publications this branch re-derives, and the final
// read-only audit published at the new graph revision. The branch is a declared sequence, never a
// loop: the plan refuses a second cycle, and a stage-owned publication names its own stage instead
// of being republished from an artifact this branch did not re-derive. A residual hard fault leaves
// the result partial, with the fault named, and never republishes the audit that scheduled it.
defineModule({
  id: 'zt-finalize', version: '2', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the finite finalization (audit plan, one scoped correction cycle, the changed-Card manifest, dependency-scoped publication refresh, final read-only audit).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('finalize', ['audit', 'link-check'])) return { status: 'done', reused: true }
    const step = (name, more = '') => `${Z.DIGEST} finalize -Step ${name} -Out "${Z.OUT}"${more}`
    const plan = await Z.runScript('finalize plan', 'Finalize', step('plan'), ' (the audit plan: at most one correction cycle)')
    if (!plan.json) return { status: 'failed', error: (plan.run && plan.run.output) || 'the audit plan was not derived' }
    const facts = plan.json
    const counts = facts.counts || {}
    log(`Finalize: audit ${facts.audit.unit} at graph revision ${facts.audit.graph_revision}; ${facts.faults.total} faults (${counts.fix || 0} correctable, ${counts.conflict || 0} conflicting, ${counts.blocked || 0} blocked, ${counts.advisory || 0} advisory); branch ${facts.branch}.`)
    let cycle = null
    let scope = null
    const refreshed = []
    if (facts.branch === 'fix') {
      const work = `${Z.OUT}\\finalize\\fix`
      const rules = (facts.fix_cycle.rules || []).join(',')
      const cards = facts.fix_cycle.card_list ? ` -Cards "${facts.fix_cycle.card_list}"` : ''
      cycle = await Z.runCards('final correction', work, cards, rules, { fixRounds: 1 })
      if (cycle.status !== 'done') {
        return { status: 'partial', plan: facts, cycle, error: `the one correction cycle did not complete: ${cycle.error || 'no report'}` }
      }
      const scoped = await Z.runScript('finalize scope', 'Finalize', step('scope', ` -FixWork "${work}"`), ' (the exact changed-Card manifest of the one cycle)')
      if (!scoped.json) return { status: 'partial', plan: facts, cycle, error: (scoped.run && scoped.run.output) || 'the changed-Card manifest was not recorded' }
      scope = scoped.json
      for (const entry of (scope.refresh || [])) {
        if (!entry.applicable || entry.layer === 'audit') continue
        const more = ` -Layer ${entry.layer} -Report "${work}\\report.json"`
        const done = await Z.runScript(`finalize refresh ${entry.layer}`, 'Finalize', step('refresh', more))
        refreshed.push(done.json
          ? { layer: entry.layer, unit: done.json.unit, revision: done.json.revision, state: done.json.state, changed_cards: done.json.changed_cards }
          : { layer: entry.layer, error: (done.run && done.run.output) || 'the refresh did not complete' })
        if (!done.json) log(`Finalize: the ${entry.layer} refresh did not complete: ${(done.run && done.run.output) || 'no output'}.`)
      }
      const owned = (scope.refresh || []).filter(e => e.stage_owned).map(e => e.layer)
      if (owned.length) log(`Finalize: ${owned.join(', ')} keep their own stage as the owner of that refresh; the changed-Card manifest is recorded for it.`)
    }
    const final = await Z.runScript('finalize final', 'Finalize', step('final'), ' (the final read-only audit at the new revision)')
    if (!final.json) return { status: 'failed', plan: facts, cycle, scope, refreshed, error: (final.run && final.run.output) || 'the final audit was not published' }
    const report = final.json
    log(`Finalize: ${report.status} at graph revision ${report.graph_revision}; final audit ${report.audit.final.unit} (revision ${report.audit.final.revision}); ${report.faults.residual} residual hard/source faults, ${report.infrastructure.length} infrastructure faults.`)
    return {
      status: report.status === 'done' ? 'done' : 'partial',
      branch: facts.branch, plan: facts, cycle, scope, refreshed, final: report,
    }
  },
})
// ==== end module zt-finalize ====

// ==== module zt-finding-layer v4 ====
// The empirical-results layer of the economics set (docs/finding-cards-design.md): the ledger's ready finding
// work, worked while the digest groups are still running. `finding-compiler` plans one job per few groups from
// the sentences that report data with a number and each group's own Idea Cards; DeepSeek writes one Finding
// Card per result as JSON and runs the job's SELF-CHECK; the compiler creates each Finding Card beside its
// group's anchor and links it from every supported Idea Card with the kind word 印证. This module claims the
// ready `findings/gNNN@1` units in bounded blocks, runs that compiler chain once per block in one manager, and
// records each group through `findings-finish`, so an unapplied job leaves the group open. Needs zt 0.20.0
// (the Card type finding).
// 4: the chain ends with the block's checked finishes in one command (`zt-digest finish-batch`), run by the chain's own
// manager; a unit its line does not answer is finished by one relay, then on its own (docs/digest-claude-token-proposal.md P1/P2).
defineModule({
  id: 'zt-finding-layer', version: '4', requires: ['zt-digest-lib'],
  summary: 'ZT digest empirical-results layer: the ledger\'s ready Finding work, plan → DeepSeek JSON → apply → per-group checked completion.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\findings`
    const fstep = (s, more = '') => `${Z.tool('finding-compiler')} -Step ${s} -Staging "${Z.OUT}"${more}`
    const result = await Z.layer('findings', {
      layer: 'findings', take: 4, rounds: Math.max(3, Z.MAX_ATTEMPTS), wait: 600, producer: 'digest', phase: 'Findings',
      work: async (units) => {
        const chained = await Z.chain(`findings ${units.map(u => u.id).join(', ')}`, {
          dsh: work, unit: units[0].id, token: units[0].token, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Findings', writes: `${work}\\out`,
          steps: [
            { name: 'plan', command: fstep('plan'), jobs: true },
            { name: 'apply', command: fstep('apply', ' -Apply'), optional: true },
            { name: 'report', command: fstep('report'), optional: true },
            { name: 'finish', command: Z.finishBatchCommand(units), always: true },
          ],
          cleanup: true, keep: true,
        })
        const applied = Z.jsonOf(chained, 'apply')
        const plan = Z.jsonOf(chained, 'plan')
        log(`Findings: ${plan ? `${plan.candidates || 0} result sentences in ${plan.groups || 0} groups, ${(plan.jobs || []).length} DeepSeek jobs` : 'plan gave no output'}; ${applied ? `${applied.created} Finding Cards made, ${applied.linked} Links, ${(applied.refused || []).length} refused` : 'apply gave no output'}.`)
        return Z.finishUnits(`findings ${units.map(u => u.id).join(', ')}`, units, Z.jsonOf(chained, 'finish'))
      },
    })
    log(`Findings: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} groups checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, dispositions: result.dispositions, gate: result.gate }
  },
})
// ==== end module zt-finding-layer ====

// ==== module zt-group-cycle v6 ====
// The Digest stage of a managed run, in blocks. One Haiku manager runs up to eight digest groups side by side
// (the DeepSeek Subagent Managed Protocol lets one manager run eight DeepSeek conversations): it claims up to
// eight `digest/gNNN@1` units, starts each (the snapshot is generated before any usable job manifest exists,
// R262), runs their frozen DeepSeek jobs together, renews every held claim while it waits, records and deletes
// each conversation itself, and records each checked completion with `group-finish` (R250/R251/R253).
// At most eight such lanes run at once; each lane claims block after block until no group is ready. A unit that
// fails is recorded against its own attempt and claimed again by a later block; the stage is never rerun as a
// whole (R258). A Haiku manager per single group spent a whole manager conversation on one DeepSeek job.
// #92: the claim requests a lease that covers the frozen provider-run cap.
// #87 (R280): `group-start` reconciles a retained accepted plan against the archive receipts; a recovered
// group's job declares `expected_jobs: 0`, runs no DeepSeek job, and completes from the committed result.
defineModule({
  id: 'zt-group-cycle', version: '6', requires: ['zt-digest-lib'],
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
    const lanes = Math.max(1, Math.min(8, Math.ceil(groups / Z.BLOCK_TAKE)))
    // Every block a lane may run: each group at most MAX_ATTEMPTS times, plus the empty claim that ends it.
    const maxBlocks = Math.ceil(groups / Z.BLOCK_TAKE) * Z.MAX_ATTEMPTS + 1
    const latest = new Map()
    // One held unit's outcome: the ledger decides (zt-digest-lib blockSettle); the manager's finish line adds
    // the details when it came back whole.
    const settle = async (u, label) => {
      const start = zdLastJson(u.start && u.start.output)
      const got = await Z.blockSettle(u, label)
      if (got.status !== 'done') return { status: got.status, group: u.group, unit: u.unit, error: got.error }
      if (start && start.recovered) log(`${label}: ${u.unit} recovered from its committed receipts; no DeepSeek job ran.`)
      return { status: 'done', group: u.group, unit: u.unit, disposition: got.parsed.disposition, owned: (got.parsed.owned || []).length, recovered: !!(start && start.recovered), jobs: u.jobs.length }
    }
    const lane = async (n) => {
      for (let round = 1; round <= maxBlocks; round++) {
        const label = `digest lane ${n} block ${round}`
        const got = await Z.block(label, {
          dsh: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, phase: 'Digest', writes: work,
          // The bounded claim: up to eight units, a lease that covers the frozen run cap (#92), and a
          // bounded readiness wait after the first block (a retried unit becomes ready again).
          claim: `${Z.LEDGER} next -Out "${Z.OUT}" -Layer digest -Take ${Z.BLOCK_TAKE} -Brief -Owner "${label}"${round > 1 ? ' -Wait 60' : ''} -Lease ${Z.CLAIM_LEASE}`,
          start: `${Z.DIGEST} group-start -Out "${Z.OUT}" -Group {{group}} -Token {{token}}`,
          finish: `${Z.DIGEST} group-finish -Out "${Z.OUT}" -Group {{group}} -Token {{token}} -Draft "{{draft}}"`,
        })
        if (!got.claimed.length) {
          // A manager that threw before its claim ran holds nothing: take the next round (trial 17).
          if (got.thrown && round < maxBlocks) continue
          if (!got.ok) log(`${label}: the claim did not run: ${got.error || 'no claim line'}`)
          return
        }
        for (const u of got.units) latest.set(u.unit, await settle(u, label))
      }
    }
    await Promise.all(Array.from({ length: lanes }, (_, i) => lane(i + 1)))
    const reports = [...latest.values()].sort((a, b) => (a.group || 0) - (b.group || 0))
    const verified = reports.filter((r) => r.status === 'done').length
    const failed = reports.filter((r) => r.status !== 'done').length
    log(`Digest: ${verified} group(s) verified, ${failed} not verified, in ${lanes} lane(s) of up to ${Z.BLOCK_TAKE} groups per manager.`)
    return { status: failed ? 'partial' : 'done', reports, verified, failed, lanes }
  },
})
// ==== end module zt-group-cycle ====

// ==== module zt-link-check v2 ====
// The Link check and the orphan pass on the whole book. In mode "full" the block cycles have already fixed the
// book, so the check runs every hard rule (the link pass can add R2, R4, and I2 faults); on an older book
// (mode "link") it keeps to L1 and L2 unless args.cardRules says otherwise.
// v2: the stage publishes its own `link-check` revision from its cycle report (`zt-digest finalize -Step stage`).
// The finalization refreshes that revision only when its one correction cycle changed a Card, so on
// acemoglu2025nobel trial 1 (nothing to correct) link-check/book@1 stayed ready and the run was never content-complete.
defineModule({
  id: 'zt-link-check', version: '2', requires: ['zt-digest-lib'],
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
    const cycle = await Z.runCards('link check', work, '', p.mode === 'full' ? '' : (Z.A.cardRules || 'L1,L2'))
    if (!cycle || cycle.status !== 'done') return cycle
    const { run, json } = await Z.runScript('link-check: publish', 'Link', `${Z.DIGEST} finalize -Step stage -Out "${Z.OUT}" -Layer link-check -Report "${work}\\report.json"`, ' (it records the stage publication in the ledger)')
    log(json && json.state ? `link-check: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `link-check: the publication failed: ${(run && run.output) || 'no output'}`)
    return { ...cycle, publication: json || { layer: 'link-check', state: 'failed', error: (run && run.output) || 'no output' } }
  },
})
// ==== end module zt-link-check ====

// ==== module zt-link-pass v7 ====
// The Link pass over the receipt-owned gap set, in blocks: one Haiku manager claims up to eight
// `link/gNNN@1` units, runs `link-start` for each (the frozen gap scope and the one job that writes the
// draft), runs their jobs side by side, and records each checked completion with `link-finish`
// (R250/R251/R263). At most eight lanes run at once, each claiming block after block until none is ready.
// Only the gap set receives a Link: `link-finish` refuses a call on any other Card, and the frozen
// accepted plan keeps a lost reply recoverable and a changed body a conflict (D123, D124, AC08).
// A group whose gap set is empty still settles its unit, with no provider run at all (AC07):
// `link-start` declares expected_jobs 0 and the chain applies the empty checked draft.
// A group whose digest did not complete stays blocked and is passed through, never worked (#92).
defineModule({
  id: 'zt-link-pass', version: '7', requires: ['zt-digest-lib'],
  summary: 'ZT digest: verified Link units in blocks of up to eight per manager, for every scheduled or gap-free group, with the gap counts and L1/L2 quality recorded.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
    pending: { type: 'array', required: true, description: 'the group numbers whose semantic gap set is not empty (from the link preparation)' },
    noGap: { type: 'array', default: [], description: 'the group numbers whose semantic gap set is empty; their Link unit is settled without a provider run' },
    gaps: { type: 'object', default: {}, description: 'the semantic gap count per group from the link preparation' },
    blocked: { type: 'array', default: [], description: 'the groups the link preparation blocked because their digest did not complete' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const scheduled = Z.groupsOf(p.groups).filter(g => (p.pending || []).includes(g.n))
    const skipped = Z.groupsOf(p.groups).filter(g => (p.noGap || []).includes(g.n) && !(p.pending || []).includes(g.n))
    const todo = [...scheduled, ...skipped]
    const work = `${Z.OUT}\\link-jobs`
    let providerRuns = 0
    const latest = new Map()
    // One held Link unit's outcome: the ledger decides (zt-digest-lib blockSettle); the manager's finish line
    // adds the counts and the quality when it came back whole.
    const settle = async (u, label) => {
      const gap = Number((p.gaps || {})[u.group]) || 0
      const start = zdLastJson(u.start && u.start.output)
      const ran = u.jobs.length
      providerRuns += ran
      const got = await Z.blockSettle(u, label)
      if (got.status !== 'done') return { group: u.group, status: got.status, unit: u.unit, gaps: gap, error: got.error }
      const counts = got.parsed.counts || {}
      if (start && start.expected_jobs === 0) log(`${label}: ${u.unit} settled with no semantic gap and no provider run (${counts.already_linked ?? 0} already linked).`)
      return {
        group: u.group, status: 'done', unit: u.unit,
        gaps: counts.gaps ?? gap, skipped: counts.skipped ?? 0,
        scheduled: (start && start.expected_jobs === 0) ? 0 : 1,
        linked: counts.linked ?? 0, linked_from: counts.linked_from ?? 0,
        already_linked: counts.already_linked ?? 0, no_link: counts.no_link ?? 0,
        provider_runs: ran, changed: got.parsed.changed || [],
        quality: got.parsed.quality || null,
      }
    }
    const lanes = Math.max(1, Math.min(8, Math.ceil(todo.length / Z.BLOCK_TAKE)))
    const maxBlocks = Math.ceil(todo.length / Z.BLOCK_TAKE) * Z.MAX_ATTEMPTS + 1
    const lane = async (n) => {
      for (let round = 1; round <= maxBlocks; round++) {
        const label = `link lane ${n} block ${round}`
        const got = await Z.block(label, {
          dsh: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, phase: 'Link', writes: work,
          claim: `${Z.LEDGER} next -Out "${Z.OUT}" -Layer link -Take ${Z.BLOCK_TAKE} -Brief -Owner "${label}"${round > 1 ? ' -Wait 60' : ''} -Lease ${Z.CLAIM_LEASE}`,
          start: `${Z.DIGEST} link-start -Out "${Z.OUT}" -Group {{group}} -Token {{token}}`,
          finish: `${Z.DIGEST} link-finish -Out "${Z.OUT}" -Group {{group}} -Token {{token}} -Draft "{{draft}}"`,
        })
        if (!got.claimed.length) {
          // A manager that threw before its claim ran holds nothing: take the next round (trial 17).
          if (got.thrown && round < maxBlocks) continue
          if (!got.ok) log(`${label}: the claim did not run: ${got.error || 'no claim line'}`)
          return
        }
        for (const u of got.units) latest.set(u.unit, await settle(u, label))
      }
    }
    if (todo.length) await Promise.all(Array.from({ length: lanes }, (_, i) => lane(i + 1)))
    const results = [...latest.values()].sort((x, y) => (x.group || 0) - (y.group || 0))
    const blocked = (p.blocked || []).filter(b => b && !todo.some(g => g.n === b.n))
    if (blocked.length) log(`Link: ${blocked.length} group(s) stay blocked (their digest unit is not done): ${blocked.map(b => `group ${b.n}`).join(', ')}.`)
    const total = (name) => results.reduce((sum, r) => sum + (Number(r[name]) || 0), 0)
    const quality = { rules: 'L1,L2', faults: results.reduce((sum, r) => sum + (Number(r.quality && r.quality.faults) || 0), 0), checked: results.reduce((sum, r) => sum + (Number(r.quality && r.quality.checked) || 0), 0) }
    log(`Link: ${results.filter(r => r.status === 'done').length} of ${todo.length} group(s) verified; ${total('gaps')} gaps, ${total('skipped')} skipped Cards, ${total('linked')} written, ${total('linked_from')} linked from another Card, ${total('no_link')} none; ${providerRuns} provider run(s); L1/L2 faults ${quality.faults}.`)
    return {
      groups: results, blocked,
      scheduled: scheduled.length, skipped: skipped.length,
      gap: total('gaps'), skipped_cards: total('skipped'),
      linked: total('linked'), linked_from: total('linked_from'), already_linked: total('already_linked'), no_link: total('no_link'),
      provider_runs: providerRuns, quality,
      verified: results.filter(r => r.status === 'done').length,
      failed: results.filter(r => r.status !== 'done').length,
    }
  },
})
// ==== end module zt-link-pass ====

// ==== module zt-link-prepare v6 ====
// v6 (S1): one relay runs prepare-link -Brief and the module reads its line; the Haiku agent that copied the arrays
// back (and ran `zt up` itself) leaves: zt-digest starts and waits for the service.
// The Link pass preparation: prepare-link-pass.ps1 computes each completed group's semantic gap set
// from its own write receipts (§5.5; D118-D124, #110). Only the gap set becomes a Link job; a Card
// that already carries a valid kinded Link outside its group is skipped and reported. A group whose
// digest unit is not `done` is not scheduled: its allow-list is empty (#92).
// Output: pending (the groups with gaps), no_gap (the groups whose gap set is empty; their Link
// unit is still settled, without a provider run), blocked, and the public counts. Fails the stage
// when the script fails. No stage reads the account-wide usage ledger: run usage is recorded per
// provider run from that run's own result (R343-R346).
defineModule({
  id: 'zt-link-prepare', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the receipt-owned semantic gap set of the Link pass (prepare-link-pass.ps1); output the scheduled, gap-free and blocked groups.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const prepared = await Z.runScript('prepare link pass', 'Link', `${Z.tool('prepare-link-pass')} -Staging "${Z.OUT}" -Brief`, ' (on a large book it takes several minutes; it writes no Card)')
    const line = prepared.json
    const linkPrep = line && Array.isArray(line.pending) ? { ok: true, ...line } : { ok: false, error: String((prepared.run && prepared.run.output) || 'no output') }
    if (!linkPrep || !linkPrep.ok) {
      log(`Link preparation failed: ${linkPrep && linkPrep.error}`)
      throw new Error(`link preparation failed: ${(linkPrep && linkPrep.error) || 'no report'}`)
    }
    const inRun = (n) => n >= 1 && n <= p.groups
    const pending = linkPrep.pending.filter(inRun)
    const noGap = (linkPrep.no_gap || []).filter(inRun)
    const blocked = (linkPrep.blocked || []).filter(b => b && inRun(b.n))
    const gaps = {}
    for (const [n, count] of Object.entries(linkPrep.gaps || {})) if (inRun(Number(n))) gaps[Number(n)] = count
    const gapTotal = linkPrep.gap_total ?? Object.values(gaps).reduce((sum, n) => sum + (n || 0), 0)
    log(`Link pass: ${gapTotal} semantic gap(s) on ${pending.length} of ${p.groups} groups; ${noGap.length} group(s) need no job; ${linkPrep.skipped_total ?? 0} owned Card(s) skipped.`)
    if (blocked.length) {
      log(`Link pass: ${blocked.length} group(s) blocked because their digest did not complete: ${blocked.map(b => `group ${b.n} (${b.unit} ${b.state})`).join(', ')}. No link job runs for them.`)
    }
    return {
      pending, no_gap: noGap, blocked, gaps,
      gap_total: gapTotal, skipped_total: linkPrep.skipped_total ?? 0,
      owned_total: linkPrep.owned_total ?? 0, unowned: linkPrep.unowned ?? 0,
      scheduled: pending.length, skipped: noGap.length,
      digest_state: linkPrep.digest_state || null,
    }
  },
})
// ==== end module zt-link-prepare ====

// ==== module zt-model-layer v6 ====
// The model layer: the ledger's ready `models/gNNN-mIIII@1` work, released as the digest groups settle. Each
// claimed block of Model units is built by the existing model compiler chain (one Sonnet architect per batch,
// DeepSeek writers, the late clean-up sweep) and every unit is then recorded through `model-finish`, which
// re-derives the build from the compiler's own state and the authored Cards. A model the compiler could not
// finish stays open with its own fault, so the next attempt works only that unit (R235/R258). The layer gate
// is the stage's result.
// 6: the built units of a block are recorded through `model-finish` in ONE command (`zt-digest finish-batch`) instead of one
// relay per model; a unit its line does not answer is finished on its own (docs/digest-claude-token-proposal.md P1).
defineModule({
  id: 'zt-model-layer', version: '6', requires: ['zt-model-lib'],
  summary: 'ZT digest model layer: the ledger\'s ready Model units built by the model compiler chain and recorded by model-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
  },
  run: async (ctx, p) => {
    const M = zmlMake(ctx, p)
    const Z = M.Z
    const listed = await M.listModels(`-All -CitationKey ${Z.KEY}`, 'find models: whole book')
    if (!listed || !listed.ok) {
      // On an existing digest the listing is the stage's whole input; after a digest the run goes on.
      if (p.mode === 'models') throw new Error(`model listing failed: ${listed && listed.error}`)
      log(`Models: listing failed (${listed && listed.error}); no model is built.`)
      return { status: 'failed', models: [], model_cleanup: null, error: `listing failed: ${listed && listed.error}` }
    }
    log(`${listed.models.length} Model Cards in the digest of ${Z.KEY}.`)
    const result = await Z.layer('models', {
      // hold: the compiler's chains outlive a claim's lease, so the driver keeps the block's claims current.
      // A block of up to eight Model units (the protocol's eight per manager): with up to eight formal models per
      // architect batch, a book's models are planned together, their references to each other resolve in-batch, and
      // no second block waits for the first (grossman2002integration trials 16-17: two blocks of four, 76 minutes).
      layer: 'models', take: 8, rounds: Z.MAX_ATTEMPTS, phase: 'Models', hold: true,
      work: async (units) => {
        const wanted = units.map(u => (u.inputs && (u.inputs.header || u.inputs.model)) || '').filter(Boolean)
        const models = listed.models.filter(m => wanted.includes(m.address))
        if (models.length < wanted.length) {
          log(`Models: ${wanted.length - models.length} claimed Model Cards are not in the current listing; they stay open.`)
        }
        // Each claimed model's claim, so the compiler chain registers its references to other models' parts.
        const claims = {}
        for (const u of units) {
          const at = (u.inputs && (u.inputs.header || u.inputs.model)) || ''
          if (at) claims[at] = { unit: u.id, token: u.token }
        }
        const built = models.length ? await M.elaborateAll(models, `models ${units.map(u => u.id).join(', ')}`, claims) : []
        M.logModels(built)
        const out = []
        const finish = []
        for (const unit of units) {
          const at = (unit.inputs && (unit.inputs.header || unit.inputs.model)) || ''
          // A model released to wait for its producer (or refused by model-deps) is not finished here: the ledger
          // holds it registered or failed, and it is claimed again once its producer is done.
          const state = (built.find((b) => b && b.model === at) || {}).status
          if (state === 'waiting' || state === 'refused') {
            out.push({ unit: unit.id, status: state === 'waiting' ? 'deferred' : 'failed', error: state })
            continue
          }
          finish.push(unit)
        }
        return [...out, ...await Z.finishUnits(`models ${finish.map(u => u.id).join(', ')}`, finish)]
      },
    })
    const modelCleanup = await M.modelCleanup()
    log(`Models: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} models checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, dispositions: result.dispositions, gate: result.gate, model_cleanup: modelCleanup }
  },
})
// ==== end module zt-model-layer ====

// ==== module zt-organize v8 ====
// Organize: organize-survey.ps1 lists what to decide; Opus writes only decisions.json (the thesis and Card
// notes); organize-apply.ps1 records that no merge or move is decided; a card cycle hands the notes to DeepSeek;
// book-skeleton.ps1 and one DeepSeek job write the Literature Card index and the structure Card.
// v6 (docs/literature-only-digest-design.md): a digest writes only the book's Literature tree. Topic merges,
// Topic Card moves and Topic bodies are zt-idea-grow's, so Opus decides none of them, the skeleton writes no
// Topic body, and the old single Opus job (args.organize "legacy") is gone.
// v7 (pilot 2026-10-23): the decision reads run the helper with -Allow on the survey's empty read-only list:
// reads only, and a path in the staging folder, so a run on a box copy (zt-box.txt) reads that box, not the live one.
// v8 (S10): whether the notes cycle runs is read from decisions.json by organize-apply (its `notes` count), never from
// the decider's answer; the unread option remove: 'delete' leaves.
defineModule({
  id: 'zt-organize', version: '8', requires: ['zt-digest-lib'],
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
    const A = Z.A, KEY = Z.KEY, OUT = Z.OUT, SCRIPTS = Z.SCRIPTS, MODE = Z.MODE
    const groupCount = p.groups || 0
    const modelReports = p.models || p.modelsBook || []
    const modelLines = modelReports.filter(r => r.status === 'done').map(r => `- [[${r.model}]]${r.level === 'concept' ? '（概念模型）' : r.level === 'formal' ? '（形式模型）' : ''}：${Array.isArray(r.parts) ? r.parts.map(pp => `${pp.facet}${pp.emphasis === 'core' ? '（核心）' : ''} [[${pp.address}]]`).join('、') : `${Number(r.parts) || 0} 张分卡（${Number(r.core) || 0} 张核心，见模型卡的结构列表）`}${(r.issues || []).length ? `；未解决：${r.issues.join('；')}` : ''}${r.notes && r.notes.length ? `；建议：${r.notes.join('；')}` : ''}`).join('\n')
    const groupLines = ((MODE === 'full' ? p.digest : p.link) || []).map(r => `- 第 ${r.group} 组 [[${KEY}/${r.group}]]：${r.status}${r.cards ? `，${r.cards} 张` : ''}${r.models ? `，模型卡 ${r.models}` : ''}${r.events ? `，事件卡 ${r.events}` : ''}${r.errors && r.errors !== '无' ? `；问题：${r.errors}` : ''}`).join('\n')
    const ORG = `${OUT}\\organize`
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts
    // by hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the
    // revision stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const { run, json } = await Z.runScript(`${layer}: publish`, 'Organize', `${Z.DIGEST} finalize -Step stage -Out "${Z.OUT}" -Layer ${layer}${more}`, ' (it records the stage publication in the ledger)')
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(run && run.output) || 'no output'}`)
      return json || { layer, state: 'failed', error: (run && run.output) || 'no output' }
    }
    // 1. The survey: a script lists what Opus must decide.
    const surveyed = await Z.runScript('organize: survey', 'Organize', `${Z.tool('organize-survey')} -Staging "${OUT}" -Work "${ORG}"`)
    const survey = surveyed.json
    if (!survey) log(`Organize: the survey failed (${surveyed.run && surveyed.run.output}).`)
    // 2. Opus decides; it writes decisions.json and nothing else.
    const decided = survey ? await Z.zAgent(
      `You decide the structure of the ZET digest of 《${A.bookTitle}》 (Literature Card [[${KEY}]], anchors [[${KEY}/1]] … [[${KEY}/${groupCount}]]). Scripts and DeepSeek do every edit; you only decide. You make no zt write.
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
    const applied = decided ? await Z.runScript('organize: apply', 'Organize', `${Z.tool('organize-apply')} -Work "${ORG}"`) : null
    const moved = applied ? applied.json : null
    if (decided) log(`Organize: thesis "${decided.thesis}"; ${moved ? `${moved.merges} merges, ${moved.moves} moves, ${moved.refused} refused, ${moved.notes || 0} notes in decisions.json${moved.thesis === false ? ' (no thesis written)' : ''}` : `apply failed (${applied && applied.run && applied.run.output})`}.`)
    // 4. The notes go to DeepSeek as a card cycle: every hard rule in mode "full" (the script adds N1), else N1.
    const notes = moved && Number(moved.notes) > 0 ? await Z.runCards('organize notes', `${OUT}\\cards-notes`, ` -Notes "${ORG}\\decisions.json" -NoOrphans`, MODE === 'full' ? '' : 'N1') : null
    // 4a. Organize publishes its revision once its decisions are applied and its notes are disposed.
    const organizePublished = moved ? await publish('organize', ` -Decisions "${ORG}\\decisions.json" -Applied "${ORG}\\applied.json"`) : null
    // 5. The skeleton: the Literature Card index and the structure Card.
    const SK = `${OUT}\\skeleton`
    const kstep = (s, more = '') => `${Z.tool('book-skeleton')} -Step ${s} -Staging "${OUT}" -Work "${SK}"${more}`
    const kopts = { dsh: SK, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Organize', writes: `${SK}\\out`, keep: true }
    await Z.drive('organize: skeleton', { ...kopts, command: kstep('plan') })
    const sk = zdLastJson(((await Z.drive('organize: skeleton apply', { ...kopts, command: kstep('apply', ` -Decisions "${ORG}\\decisions.json" -Apply`), cleanup: true })) || {}).output)
    log(sk && sk.ok ? `Organize: structure Card ${sk.written && sk.written.structure} ("${sk.thesis}"), ${sk.argument} argument lines, ${sk.models} models, ${sk.events} events.` : `Organize: the skeleton failed: ${JSON.stringify((sk && sk.problems) || 'no output')}`)
    const skeletonPublished = sk && sk.ok ? await publish('skeleton', ` -Skeleton "${SK}\\out\\skeleton.json"`) : null
    return { survey, decisions: decided, applied: moved, notes: notes && notes.report, skeleton: sk, publications: { organize: organizePublished, skeleton: skeletonPublished } }
  },
})
// ==== end module zt-organize ====

// ==== module zt-term-cards v2 ====
// S20 (#164) built the branch; S24 (#167) makes it ledger work. The Term-card compiler's writer,
// reviewer and fix jobs run on the shared DeepSeek Subagent Managed Protocol (DSMP) pool (design
// 12.4), and the branch is the run's own `termcards/tNNN@<plan revision>` job units plus its
// `termcards/book@<plan revision>` unit (design 14.2, R0423/R0424). The compiler (zt-digest
// term-cards) prints the exact JSON jobs and registers the units at `plan`; this module schedules
// them and completes them only from the compiler's own checked artifacts.
//
// The schedule (R0335): ready job units are claimed in waves of at most four. A wave runs its
// writers (retained, never archived), the compiler's check, the independent reviews of the
// mathematical drafts (at most four reviewer conversations alongside the writers), the one permitted
// fix in each writer's own conversation, and the recheck/re-review of a changed draft; each job unit
// then completes from `check/readiness.json`, which binds the job schema, the shared check, the
// required reviews and the fix disposition. The wave's conversations are deleted before the next
// wave is admitted. Four retained writers + four reviewers = the protocol's eight conversations; no
// manager holds eight writers and then looks for a reviewer slot, so the shared pool cannot deadlock.
//
// The book unit completes only from `report.json` when every required row's head and parts are
// committed and read back. A partial prefix, a refused row or a failed writer leaves it incomplete,
// so the Term-link pass stays blocked and the audit/report show the coverage debt (R0428). No unit is
// ever marked done to unblock a later stage.
//
// Every manager call takes one dsmpSlot from the script-wide pool (at most eight managers alive), and
// the runtime policy is the installed protocol's own: 240 minutes per conversation, nonblocking polls
// of all open sessions, at most 90 seconds blocking on one, one provider retry, one semantic fix.
// A conversation is registered, its answer evidence is written into the local staging folder, and its
// deletion is recorded, so the run's own cleanup can verify the provider resources. The workspace
// registration is removed when the branch is done (success or failure); the staging files stay.
defineModule({
  id: 'zt-term-cards', version: '2', requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Term cards: claim the run\'s Term-card job units in waves of four, schedule their writers, reviews and fixes on the shared DSMP pool, complete each unit from the compiler\'s readiness result and the book unit from its publication report, and leave a failed or partial book incomplete.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder (the compiler\'s -Staging)' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command, or the path of a zt-digest binary' },
    perJob: { type: 'number', min: 1, max: 8, description: 'concept terms per writer job (the compiler\'s default is 4)' },
    mathPerJob: { type: 'number', min: 1, max: 8, description: 'math terms per writer job (the compiler\'s default is 1)' },
    maxStepsPerCard: { type: 'number', min: 1, max: 64, description: 'steps per rendered continuation part (the compiler\'s default is 12)' },
    maxProse: { type: 'number', min: 1, max: 20000, description: 'prose budget of a Card (the compiler\'s default is 1600)' },
    relay: { type: 'string', default: 'relay', role: true, description: 'the declared role that relays the compiler commands (the zt-relay agent type)' },
  },
  run: async (ctx, p) => {
    // The four constants this module reconciles against the protocol it was built with (R0332/R0334).
    const WANTED = { perManager: 8, maxManagers: 8, pollSeconds: 90, budgetMinutes: 240 }
    const WAVE = 4 // R0335: at most four live writers, and at most four reviewers alongside them.
    const MAX_WAVES = 200 // a bounded claim loop: one wave can never be claimed for ever.

    const q = (v) => `'${String(v).replace(/'/g, "''")}'`
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const WORK = `${OUT}\\term-cards`
    const ANSWERS = `${WORK}\\answers`
    const exe = p.tool && p.tool !== 'zt-digest' ? `& ${q(p.tool)}` : 'zt-digest'
    const phase = ctx.phase
    const chunk = (list, size) => { const out = []; for (let i = 0; i < list.length; i += size) out.push(list.slice(i, i + size)); return out }
    // The review batches of one wave (R0335): at most four reviewer conversations, and one grouped
    // writer job's promoted terms are reviewed one after another, never side by side.
    const reviewBatches = (jobs) => {
      const batches = []
      let left = jobs.slice()
      while (left.length) {
        const batch = []
        const seen = new Set()
        const rest = []
        for (const job of left) {
          const writer = String(job.id).replace(/-r\d+-review$/, '')
          if (batch.length < WAVE && !seen.has(writer)) { seen.add(writer); batch.push(job) } else rest.push(job)
        }
        batches.push(batch)
        left = rest
      }
      return batches
    }
    const tail = (text) => String(text || '').trim().slice(0, 400)

    // R0332/AC08: the runtime policy is the installed protocol's own. A changed protocol is reported
    // explicitly, never silently stretched by this workflow.
    const changed = Object.entries(WANTED).filter(([k, v]) => DSMP[k] !== v)
    if (changed.length) {
      return { status: 'failed', error: `the managed protocol changed (${changed.map(([k, v]) => `${k} ${DSMP[k]} ≠ ${v}`).join(', ')}); reconcile the term-card waves before running`, book: p.key, work: WORK }
    }

    const notes = { registered: 0, evidence: 0, deleted: 0, not_deleted: [], workspaces: [] }
    const sessions = {} // a retained writer's job id -> its conversation id
    const report = { status: 'done', book: p.key, work: WORK, ledger: false, jobs: { total: 0, done: 0, failed: [] }, waves: [], reviews: { planned: 0, approved: 0, refused: 0 }, fixes: { planned: 0, final: 0 }, readiness: { counts: {}, complete: false }, book_unit: null, publication: null, deepseek: notes }
    let error = ''
    let lock = null
    let readiness = { counts: {}, complete: false }
    let bookUnit = null // { id, token, input_hash }

    // One compiler command through the one relay. Its last JSON line is the step's answer.
    const command = async (label, args, always = false) => {
      const got = await ztRelay(ctx, { command: `${exe} term-cards ${args}`, label: `${ctx.stage}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it writes only the staging folder)', always })
      return got
    }
    const ledger = async (label, args, always = false) => {
      const got = await ztRelay(ctx, { command: `${exe} ledger ${args}`, label: `${ctx.stage}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it reads or records the run ledger only)', always, lockStops: false })
      return { ok: !!(got.run && got.run.ok), line: got.json, output: (got.run && got.run.output) || '' }
    }
    // Record one unit's content fault: the ledger classifies the code from its cause (S5), and a
    // card_check refusal keeps the archive's own code.
    const fault = async (label, unit, code, cause) => {
      const document = { schema_version: 1, unit: unit.id, input_hash: unit.input_hash, cause: tail(cause) }
      if (code) document.code = code
      const got = await ledger(label, `fail -Out "${OUT}" -Unit "${unit.id}" -Token ${unit.token} -Fault ${q(JSON.stringify(document))}`)
      if (!got.ok) log(`Term cards: the ledger refused the fault of ${unit.id}: ${tail(got.output)}`)
      return got
    }
    const bookkeeping = (line) => { if (line && line.counts) readiness = { counts: line.counts, complete: line.complete === true } }
    // The final review state of each term, keyed by job and row: a later review-plan repeats the
    // terms already approved, so only a changed state is counted (R0322: one verdict per binding).
    const reviewStates = new Map()
    const countReviews = (line) => {
      for (const notice of (line && line.reviews) || []) {
        const key = `${notice.job}#${notice.row}`
        const before = reviewStates.get(key)
        if (before === notice.status) continue
        if (before && report.reviews[before] !== undefined) report.reviews[before]--
        if (report.reviews[notice.status] !== undefined) report.reviews[notice.status]++
        reviewStates.set(key, notice.status)
      }
    }

    // The record commands each manager runs itself, at the moment the fact exists (R0322-R0325).
    const register = `${exe} ledger resource -Out ${q(OUT)} -Unit run -RegisterId <session_id> -Workspace ${q(WORK)}`
    const evidence = (file) => `${exe} ledger resource -Out ${q(OUT)} -Unit run -EvidenceFor <session_id> -Evidence ${q(file)}`
    const settle = `${exe} ledger resource -Out ${q(OUT)} -Unit run -Settle <session_id> -State finished -Deleted confirmed`

    // The manager prompt: the DSMP v1.1 loop (one retry, 240 min, 90 s polls) with the one extension
    // the Term-card chain needs: a writer conversation survives until its wave's fix/review
    // opportunity is complete (R0330).
    const managerPrompt = (kind, jobs) => {
      const table = Object.entries(sessions).map(([job, session]) => `${job} ${session}`).join('\n')
      return `You are a DeepSeek subagent manager for ZET Term Cards (${kind}). You start DeepSeek agents, collect their answers, and delete the conversations the jobs tell you to delete. You do not write Card prose or JSON yourself: the DeepSeek content jobs do. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_prompt,mcp__deepseek__deepseek_delete".

JOBS (${jobs.length}):
${JSON.stringify(jobs, null, 1)}

1. Start, in ONE message. For a job without "continue": mcp__deepseek__deepseek_run with cwd "${WORK}", prompt_files = its "files", prompt = its "prompt", skills = its "skills", model = its "model", reasoning_effort = its "effort", wait_seconds 0. For a job with "continue", take that job id's session_id from this table (job id, then session_id):
${table || '(empty)'}
   With a session: mcp__deepseek__deepseek_prompt with that session_id, prompt_files = its "files", prompt = its "prompt", skills = its "skills", reasoning_effort = its "effort", wait_seconds 0. Without a session: mcp__deepseek__deepseek_run with prompt_files = its "fallback", and the same other fields.
   The moment a deepseek_run returns a session_id, record it: ${register}
   Register only the session_id deepseek_run returned, never a word in its place: never invent an id, a PENDING placeholder or a job name. A step with no job starts no run: never send a placeholder prompt.
2. Collect loop. A session is OPEN until you have collected it in step 4. Repeat until no session is open:
   a. Poll, in ONE message: call mcp__deepseek__deepseek_wait with wait_seconds 0 for EVERY open session.
   b. Act on every result before waiting again:
      - finished, failed or cancelled: collect it now (steps 3, 4 and 5).
      - needs_response with an approval: allow only a read of a file under "${WORK}", and writing the one file the job's own prompt names ("${WORK}\\out" or "${WORK}\\review\\answers"). Reject everything else: shell commands, every zt command, deletes, network, installs, and files outside "${WORK}". Answer a question with custom text "按任务说明自行决定，不需要询问。".
      - running longer than ${DSMP.budgetMinutes} minutes: mcp__deepseek__deepseek_cancel it. It is collected as a timeout and gets no retry.
   c. Block: if any session is still running, call mcp__deepseek__deepseek_wait with wait_seconds ${DSMP.pollSeconds} on exactly ONE running session (the one started first). Never use a larger wait and never block on two sessions.
3. Check: a job succeeds when its status is finished and its answer is not empty. A failed job gets ONE more attempt, in a new conversation: deepseek_run with prompt_files = its "fallback" (or its "files" when it has none) and the same other fields. Never start a third attempt. A timeout gets no retry.
4. Collect: copy each job's final answer verbatim, at most 4000 characters, into "${ANSWERS}\\<session_id>.txt" (the line "no answer: <status>" when it gave none), then record it: ${evidence(`${ANSWERS}\\<session_id>.txt`)}
5. Delete: when the job's "delete_after_collect" is true, call mcp__deepseek__deepseek_delete with its session_id and record it: ${settle}
   Never call deepseek_archive and never call deepseek_remove_workspace. A job whose delete_after_collect is false stays open: its wave deletes it after the fix/review opportunity.
   A record command that fails does not stop the work: return registered/evidence/deleted false for that job and go on.
6. Return ok=true and one entry per job: id, session_id, status ("done" or "failed"), answer, registered, evidence, deleted. Copy answers verbatim; never summarize.`
    }

    const RESULT = {
      type: 'object',
      properties: {
        ok: { type: 'boolean' }, error: { type: 'string' }, deleted: { type: 'number' },
        not_deleted: { type: 'array', items: { type: 'string' } },
        jobs: { type: 'array', items: { type: 'object', properties: {
          id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] },
          answer: { type: 'string' }, registered: { type: 'boolean' }, evidence: { type: 'boolean' }, deleted: { type: 'boolean' },
        }, required: ['id', 'status'] } },
      },
      required: ['ok', 'jobs'],
    }
    // One manager call takes one slot of the script-wide pool: the term branch and the digest branch
    // share the same eight managers (R0327/R0328).
    const manager = async (kind, jobs) => {
      if (!jobs.length) return []
      const run = await dsmpSlot(() => ctx.agent(managerPrompt(kind, jobs), { role: 'manager', label: `${ctx.stage}: ${kind}`, phase, schema: RESULT }))
      if (!run || !Array.isArray(run.jobs)) throw new Error(`term-cards: the ${kind} manager returned no report`)
      for (const entry of run.jobs) {
        if (entry.registered === true) notes.registered++
        if (entry.evidence === true) notes.evidence++
        if (entry.deleted === true) notes.deleted++
      }
      if ((run.not_deleted || []).length) notes.not_deleted.push(...run.not_deleted)
      // A job the manager did not answer is a failed job, never a silent success.
      return jobs.map((job) => (run.jobs || []).find((entry) => entry.id === job.id) || { id: job.id, status: 'failed', answer: 'the manager returned no entry' })
    }

    // The settle of one wave: delete the writer conversations that the protocol is not allowed to
    // keep, after the review/fix opportunity, and record every outcome (R0330).
    const SETTLE = {
      type: 'object',
      properties: {
        ok: { type: 'boolean' }, error: { type: 'string' }, deleted: { type: 'number' },
        not_deleted: { type: 'array', items: { type: 'string' } },
        sessions: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, deleted: { type: 'boolean' } }, required: ['id', 'session_id', 'deleted'] } },
      },
      required: ['ok', 'sessions'],
    }
    const settlePrompt = (list) => `You are a DeepSeek subagent manager for ZET Term Cards (settle a wave). Delete the conversations of one finished wave and record every deletion. Never archive a conversation and never remove a workspace.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_delete".

SESSIONS (${list.length}):
${JSON.stringify(list.map(([id, session_id]) => ({ id, session_id })))}

For each session in order: call mcp__deepseek__deepseek_delete with its session_id. If a delete is refused, do not retry it and record the reason. Then record the outcome once for that session: ${settle} (or ${exe} ledger resource -Out ${q(OUT)} -Unit run -Settle <session_id> -State finished -Deleted failed -Message '<reason>').
Return ok=true, deleted (how many), not_deleted ("session_id: reason" for each), and one entry per session: id, session_id, deleted.`
    const settleWave = async (label, ids) => {
      const list = ids.filter((id) => sessions[id]).map((id) => [id, sessions[id]])
      if (!list.length) return
      let run = null
      try {
        run = await dsmpSlot(() => ctx.agent(settlePrompt(list), { role: 'manager', label: `${ctx.stage}: ${label}`, phase, schema: SETTLE }))
      } catch (e) {
        // The run-level cleanup still knows the registered ids; a settle that throws is reported.
        notes.not_deleted.push(`${label}: ${tail(e && e.message)}`)
        return
      }
      if (!run || !Array.isArray(run.sessions)) { notes.not_deleted.push(`${label}: the settle manager returned no report`); return }
      notes.deleted += Number(run.deleted) || 0
      if (Array.isArray(run.not_deleted)) notes.not_deleted.push(...run.not_deleted)
      // A conversation the settle manager did not delete stays registered: the run cleanup retries it.
      for (const [id, session] of list) {
        const entry = run.sessions.find((s) => s.session_id === session)
        if (!entry || entry.deleted !== true) notes.not_deleted.push(`${session}: not deleted`)
        delete sessions[id]
      }
    }

    const planOptions = [[p.perJob, '-PerJob'], [p.mathPerJob, '-MathPerJob'], [p.maxStepsPerCard, '-MaxStepsPerCard'], [p.maxProse, '-MaxProse']]
      .filter(([value]) => value !== undefined && value !== null).map(([value, flag]) => ` ${flag} ${Number(value)}`).join('')

    // One wave: write, check, review, fix once, recheck and re-review, then (in a prepared run)
    // complete each claimed unit from the readiness result that says every row of that job is ready.
    const runWave = async (index, units, given) => {
      const ids = units ? units.map((unit) => String(unit.inputs.job.id)) : (given || []).map((job) => job.id)
      const row = { wave: index + 1, jobs: ids, reviews: 0, fixes: 0, completed: [] }
      report.waves.push(row)
      try {
        let wave = []
        if (units) {
          const listed = await command('jobs', `-Step jobs -Staging ${q(OUT)} -Tokens ${q(units.map((unit) => `${unit.inputs.job.id}:${unit.token}`).join(','))}`)
          const line = listed.json
          if (!line || line.ok !== true || !Array.isArray(line.jobs)) throw new Error(`term-cards jobs: ${tail((listed.run && listed.run.output) || 'no output')}`)
          wave = line.jobs
        } else {
          const listed = await command('jobs', `-Step jobs -Staging ${q(OUT)} -Jobs ${q(ids.join(','))}`)
          const line = listed.json
          if (!line || line.ok !== true || !Array.isArray(line.jobs)) throw new Error(`term-cards jobs: ${tail((listed.run && listed.run.output) || 'no output')}`)
          wave = line.jobs
        }
        // The writer conversations of this wave are retained (delete_after_collect false). A failed
        // writer's conversation is retained too, so the wave's settle deletes it as well.
        const written = await manager('write', wave)
        const failed = []
        for (const entry of written) {
          if (entry.session_id) sessions[entry.id] = entry.session_id
          if (entry.status === 'done') report.jobs.done++
          else {
            report.jobs.failed.push({ id: entry.id, answer: tail(entry.answer) })
            failed.push(entry.id)
            if (!entry.session_id) notes.not_deleted.push(`${entry.id}: no session id returned`)
          }
        }
        if (failed.length) {
          for (const unit of (units || []).filter((unit) => failed.includes(String(unit.inputs.job.id)))) {
            const answer = ((report.jobs.failed.find((f) => f.id === String(unit.inputs.job.id)) || {}).answer) || ''
            await fault('writer fault', unit, '', `the writer job ${unit.inputs.job.id} failed: ${answer}`)
          }
          throw new Error(`term-cards: writer job(s) failed: ${failed.join(', ')}`)
        }
        bookkeeping((await command('check', `-Step check -Staging ${q(OUT)}`)).json)

        // The independent review of every mathematical draft, in waves of at most four, alongside
        // the four retained writers.
        const reviews = async () => {
          const got = await command('review-plan', `-Step review-plan -Staging ${q(OUT)}`)
          const line = got.json
          if (!line || line.ok !== true) throw new Error(`term-cards review-plan: ${tail((got.run && got.run.output) || 'no output')}`)
          bookkeeping(line)
          countReviews(line)
          for (const group of reviewBatches((line.jobs || []).filter((job) => job.delete_after_collect === true))) {
            row.reviews += group.length
            report.reviews.planned += group.length
            await manager('review', group)
          }
        }
        await reviews()

        // One finite fix per writer job, sent back to that writer's own conversation; a changed
        // draft is checked and re-reviewed.
        const fixed = await command('fix-plan', `-Step fix-plan -Staging ${q(OUT)}`)
        const fixLine = fixed.json
        if (!fixLine || fixLine.ok !== true) throw new Error(`term-cards fix-plan: ${tail((fixed.run && fixed.run.output) || 'no output')}`)
        bookkeeping(fixLine)
        report.fixes.final += (fixLine.final || []).length
        const fixes = (fixLine.jobs || []).filter((job) => job.continue)
        report.fixes.planned += fixes.length
        for (const group of chunk(fixes, WAVE)) {
          row.fixes += group.length
          await manager('fix', group)
        }
        if (fixes.length) {
          bookkeeping((await command('check', `-Step check -Staging ${q(OUT)}`)).json)
          await reviews()
        }

        // Complete each claimed unit from the readiness result: a job whose every row is ready is
        // the unit's proof (R0423); a job that is not ready keeps its unit open and is named.
        const notReady = []
        for (const unit of units || []) {
          const done = await ledger('complete', `done -Out "${OUT}" -Unit "${unit.id}" -Token ${unit.token} -Outputs "${WORK}\\check\\readiness.json"`)
          if (done.ok && done.line && done.line.state === 'done') {
            row.completed.push(unit.id)
            continue
          }
          notReady.push(String(unit.inputs.job.id))
          await fault('readiness fault', unit, '', `the job ${unit.inputs.job.id} is not ready: ${tail(done.output)}`)
        }
        if (notReady.length) throw new Error(`term-cards: job(s) not ready after the permitted fix: ${notReady.join(', ')}`)
      } finally {
        // The wave is complete or stopped: delete its retained writers before the next wave is
        // admitted (R0330/R0335). Every staged file stays on disk.
        await settleWave(`wave ${index + 1}`, ids)
      }
    }

    try {
      const planned = await command('plan', `-Step plan -Staging ${q(OUT)}${planOptions}`)
      const planLine = planned.json
      if (!planLine || planLine.ok !== true) throw new Error(`term-cards plan: ${tail((planned.run && planned.run.output) || 'no output')}`)
      report.plan_revision = planLine.revision
      report.plan_hash = planLine.plan_hash
      const units = planLine.units || null
      report.book_unit = units && units.book ? units.book : null
      report.ledger = !!(units && (units.book || (units.jobs && Object.keys(units.jobs).length)))

      if (!report.ledger) {
        // No prepared run ledger (a fixture or a diagnostic branch): the compiler lists its plan's
        // jobs, and the branch reports the branch's own result without a ledger unit.
        const listed = await command('jobs', `-Step jobs -Staging ${q(OUT)}`)
        const jobs = (listed.json && listed.json.jobs) || []
        report.jobs.total = jobs.length
        if (!jobs.length) {
          report.readiness = readiness
          report.status = 'failed'
          report.error = 'the plan has no writer jobs (no usable Term rows)'
          report.staging = WORK
          return report
        }
        for (let index = 0; index < chunk(jobs, WAVE).length && !error && !lock; index++) {
          const wave = chunk(jobs, WAVE)[index]
          try {
            await runWave(index, null, wave)
          } catch (e) {
            error = error || tail(e && e.message) || 'term-cards: the wave failed'
          }
        }
      } else {
        report.jobs.total = Object.keys(units.jobs || {}).length
        let claimedWaves = 0
        while (!error && !lock && claimedWaves < MAX_WAVES && !bookUnit) {
          const claimed = await ledger('claim', `next -Out "${OUT}" -Layer termcards -Take ${WAVE} -Owner "term cards wave ${claimedWaves + 1}" -Lease 3600`)
          if (!claimed.ok || !claimed.line) { error = `the Term-card claim returned no answer: ${tail(claimed.output)}`; break }
          const held = (claimed.line.units || []).filter((unit) => unit && typeof unit.id === 'string')
          // The book unit sorts before the job units (logical key `book`): when every job is done, the
          // claim offers it alone, and the branch goes straight to its publication under this claim.
          const book = held.find((unit) => unit.id.startsWith('termcards/book@'))
          if (book) {
            bookUnit = book
            report.book_unit = book.id
          }
          const ready = held.filter((unit) => unit.id.startsWith('termcards/') && !unit.id.startsWith('termcards/book@') && unit.inputs && unit.inputs.job && unit.inputs.job.id)
          if (!ready.length) break
          claimedWaves++
          await runWave(claimedWaves - 1, ready)
        }
        if (claimedWaves >= MAX_WAVES) error = error || `the Term-card branch claimed ${MAX_WAVES} waves without settling`
        if (!error && !lock && !bookUnit) {
          const claimed = await ledger('claim', `next -Out "${OUT}" -Layer termcards -Take 1 -Owner "term cards book" -Wait 60 -Lease 3600`)
          const unit = ((claimed.line && claimed.line.units) || []).find((u) => u && typeof u.id === 'string' && u.id.startsWith('termcards/book@'))
          if (!unit) {
            // A resume can find the whole branch already settled: every unit is done, so the layer's
            // gate is complete and there is nothing left to claim. That is not a failure.
            const gate = await ledger('gate', `gate -Out "${OUT}" -Layer termcards -Brief`, true)
            const view = gate.line || {}
            report.readiness = readiness
            if (view.complete === true) {
              report.status = 'done'
              report.already_complete = true
              report.book_unit = units.book || report.book_unit
              log('Term cards: every unit of the branch is already complete; nothing to claim.')
              return report
            }
            report.status = 'failed'
            report.error = 'the Term-card book unit is not ready: a writer job or the Term table it waits on is not done'
            report.staging = WORK
            return report
          }
          bookUnit = unit
          report.book_unit = unit.id
        }
      }
      if (!error && !lock) {
        const applied = await command('apply', `-Step apply -Staging ${q(OUT)}${bookUnit ? ` -Token ${bookUnit.token}` : ''} -Apply`)
        const line = applied.json
        const ok = !!(applied.run && applied.run.ok) && !!line && line.ok === true && line.complete === true
        report.publication = line ? { complete: line.complete === true, rows: line.rows, committed: line.committed, ready_unpublished: line.ready_unpublished, refused: line.refused, pending: line.pending, conflicted: line.conflicted, staged: line.staged, stop: line.stop } : null
        if (!ok) {
          const stop = (line && (line.code || (line.stop && line.stop.code))) || 'incomplete'
          if (bookUnit) await fault('publication fault', bookUnit, '', `the Term-card publication is not complete (${stop}): ${tail((line && line.error) || (applied.run && applied.run.output) || 'no output')}`)
          report.status = 'failed'
          report.error = `the Term-card publication is not complete (${stop}); the book unit stays incomplete`
        } else if (bookUnit) {
          const completed = await ledger('complete', `done -Out "${OUT}" -Unit "${bookUnit.id}" -Token ${bookUnit.token} -Outputs "${WORK}\\report.json"`)
          if (!completed.ok || !completed.line || completed.line.state !== 'done') {
            await fault('book fault', bookUnit, '', `the Term-card book completion was refused: ${tail(completed.output)}`)
            report.status = 'failed'
            report.error = `the Term-card book unit could not complete: ${tail(completed.output)}`
          } else {
            report.completion = completed.line
          }
        }
      }
    } catch (e) {
      if (e && e.lock_refusal) lock = e
      error = error || tail(e && e.message) || 'term-cards: the branch failed'
    } finally {
      // R0331: remove the workspace registration once the branch's work and continuations are done;
      // removing the registration never deletes the staging folder. The local files stay for audit.
      if (notes.registered) {
        try {
          // The workspace removal is one more manager of the shared pool, never a second path.
          const folders = await dsmpSlot(() => dsCleanupWorkspaces([WORK], { label: `${ctx.stage}: workspace`, phase }))
          notes.workspaces.push(...(folders || []))
        } catch (e) {
          notes.not_deleted.push(`workspace: ${tail(e && e.message)}`)
        }
      }
    }

    report.readiness = readiness
    if (lock) { report.status = 'failed'; report.error = `card_locked: ${tail(lock.message)}`; report.staging = WORK }
    else if (error) { report.status = 'failed'; report.error = error; report.staging = WORK }
    else if (report.jobs.failed.length) { report.status = 'failed'; report.error = `writer job(s) failed: ${report.jobs.failed.map((f) => f.id).join(', ')}`; report.staging = WORK }
    if (notes.not_deleted.length) log(`Term cards: ${notes.not_deleted.length} conversation(s) not deleted by the branch; the run cleanup still holds their ids.`)
    log(`Term cards: ${report.jobs.done}/${report.jobs.total} writer job(s), ${report.reviews.planned} review(s), ${report.fixes.planned} fix prompt(s), ${notes.deleted} conversation(s) deleted, ${notes.workspaces.length} workspace(s) removed.`)
    if (lock) throw lock
    return report
  },
})
// ==== end module zt-term-cards ====

// ==== module zt-term-link v1 ====
// S24 (#167): the deterministic Term-link pass of one book, after every Card writer and the Bridge
// (design 13.6-13.7, 14.1-14.2). It owns no content decision: it claims the pass's ledger unit,
// runs the compiler's frozen `plan` (which registers that unit in a prepared run), applies the exact
// plan under the claim, and completes the unit only from the compiler's own apply report. The
// whole-book pass is `termlink/book@1`; a finalization correction round is `termlink/final@R`, so a
// later correction can never be suppressed by a reusable unit (R0426/R0427).
//
// The ledger is the scheduler and the truth. A pass whose Term-card book unit or Card writers are
// not done is not ready: the module reports it `skipped` with the ledger's unresolved units, so
// audit and report still run and show the coverage debt, and a failed or partial publication can
// never be represented as a linked book (R0428). A diagnostic `-Cards` subset without a round owns
// no unit at all (R0429). A stopped or incomplete apply records its fault on the unit and leaves it
// incomplete; the run's report shows it, and a resume retries only that unit.
defineModule({
  id: 'zt-term-link', version: '1', requires: ['zt-workflow-lib'],
  summary: 'Term link: claim the run\'s Term-link unit, freeze the deterministic plan, apply it under the claim and complete the unit from the apply report; a blocked or failed pass stays incomplete and is reported, never silently linked.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder (the compiler\'s -Staging)' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    occurrences: { type: 'string', enum: ['first', 'all'], default: 'first', description: 'the occurrence policy: first or all (args.termLinkOccurrences)' },
    cards: { type: 'array', default: [], description: 'a finalization round\'s exact changed Cards (empty for the whole-book pass)' },
    round: { type: 'number', description: 'the finalization correction round the -Cards scope serves' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command, or the path of a zt-digest binary' },
    relay: { type: 'string', default: 'relay', role: true, description: 'the declared role that relays the compiler commands (the zt-relay agent type)' },
  },
  run: async (ctx, p) => {
    const q = (v) => `'${String(v).replace(/'/g, "''")}'`
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const WORK = `${OUT}\\term-link`
    const exe = p.tool && p.tool !== 'zt-digest' ? `& ${q(p.tool)}` : 'zt-digest'
    const phase = ctx.phase
    const tail = (text) => String(text || '').trim().slice(0, 400)
    const cards = Array.isArray(p.cards) ? p.cards.map(String).filter(Boolean) : []
    const round = Number.isFinite(Number(p.round)) && Number(p.round) > 0 ? Number(p.round) : null
    if (round !== null && !cards.length) return { status: 'failed', error: `finalization round ${round} names no changed Card`, book: p.key, work: WORK }
    if (round === null && cards.length) return { status: 'skipped', reason: 'a diagnostic -Cards subset without a round owns no ledger unit; run the stage without -Cards for the book pass', book: p.key, work: WORK }
    const scoped = cards.length ? ` -Cards ${q(cards.join(','))}${round !== null ? ` -Round ${round}` : ''}` : ''
    const report = { status: 'done', book: p.key, work: WORK, unit: null, pass: round === null ? 'book' : 'final', round, cards: cards.length, plan: null, counts: null, no_op: false, unresolved: [], staging: WORK }

    // One compiler command through the one relay; its last JSON line is the step's answer. `always`
    // reads the line of a step that exits non-zero too: the archive's typed stop is its last line.
    const command = async (label, args, always = false) => {
      const got = await ztRelay(ctx, { command: `${exe} term-link ${args}`, label: `${ctx.stage}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it writes only the staging folder)', always })
      return got
    }
    const ledger = async (label, args, always = false) => {
      const got = await ztRelay(ctx, { command: `${exe} ledger ${args}`, label: `${ctx.stage}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it reads or records the run ledger only)', always, lockStops: false })
      return { ok: !!(got.run && got.run.ok), line: got.json, output: (got.run && got.run.output) || '' }
    }
    // Record one unit's content fault and leave it incomplete: the ledger's own classifier reads the
    // code from the cause (S5), so a card_check refusal keeps its code.
    const fault = async (label, unit, code, cause) => {
      const document = { schema_version: 1, unit: unit.id, input_hash: unit.input_hash, cause: tail(cause) }
      if (code) document.code = code
      const got = await ledger(label, `fail -Out "${OUT}" -Unit "${unit.id}" -Token ${unit.token} -Fault ${q(JSON.stringify(document))}`)
      if (!got.ok) log(`Term link: the ledger refused the fault of ${unit.id}: ${tail(got.output)}`)
      return got
    }

    let unit = null
    try {
      // The plan comes first: in a prepared run it registers this pass's unit (`termlink/book@1`, or
      // `termlink/final@R` for a finalization round). The claim then reads the unit the dependency
      // ledger has made ready; a blocked pass has no ready unit and is reported, never forced.
      const planned = await command('plan', `-Step plan -Staging ${q(OUT)} -Occurrences ${p.occurrences}${scoped}`)
      const plan = planned.json
      if (!plan || plan.ok !== true) {
        report.status = 'failed'
        report.error = `term-link plan: ${tail((planned.run && planned.run.output) || 'no output')}`
        return report
      }
      report.plan = { revision: plan.plan_revision, hash: plan.plan_hash, cards_changed: plan.cards_changed, cards_refused: plan.cards_refused, missing_heads: plan.missing_heads, complete: plan.complete }
      if (plan.unit) report.unit = plan.unit

      const claimed = await ledger('claim', `next -Out "${OUT}" -Layer termlink -Take 1 -Owner "term-link" -Wait 60 -Lease 3600`)
      if (!claimed.ok || !claimed.line || !Array.isArray(claimed.line.units)) {
        report.status = 'failed'
        report.error = `the Term-link claim returned no units: ${tail(claimed.output)}`
        return report
      }
      unit = claimed.line.units.find((u) => u && typeof u.id === 'string' && u.id.startsWith('termlink/')) || null
      if (!unit) {
        // No ready unit: the pass is already complete, or the work it depends on is not.
        const gate = await ledger('gate', `gate -Out "${OUT}" -Layer termlink -Brief`, true)
        const view = gate.line || {}
        report.unresolved = Array.isArray(view.unresolved) ? view.unresolved : []
        const counts = view.counts || {}
        if (view.complete === true) {
          report.status = 'skipped'
          report.reason = 'the Term-link pass is already complete'
          log('Term link: the run\'s Term-link pass is already complete; nothing to do.')
          return report
        }
        report.status = 'skipped'
        report.reason = `the Term-link unit is not ready: ${report.unresolved.length} unresolved unit(s) (${counts.failed || 0} failed, ${counts.blocked || 0} blocked); a failed Term-card book or Card writer never becomes a linked book`
        log(`Term link: skipped — ${report.reason}.`)
        return report
      }
      report.unit = unit.id
      report.token = unit.token
      log(`Term link: ${unit.id} claimed; ${p.occurrences} occurrence(s)${cards.length ? `, ${cards.length} Card(s)` : ', whole book'}.`)

      const applied = await command('apply', `-Step apply -Staging ${q(OUT)} -Occurrences ${p.occurrences}${scoped} -Token ${unit.token} -Apply`, true)
      const summary = applied.json && applied.json.step === 'apply' ? applied.json : null
      const ok = !!(applied.run && applied.run.ok) && !!summary && summary.complete === true
      if (summary) {
        report.counts = summary.counts || null
        report.no_op = summary.no_op === true
        report.chunks = summary.chunks
      }
      if (!ok) {
        const detail = applied.json && applied.json.step !== 'apply' ? applied.json : null
        const stop = (summary && summary.code) || (detail && (detail.code || detail.error)) || 'incomplete'
        await fault('apply fault', unit, stop === 'card_check' ? 'card_check' : '', `the Term-link pass stopped as ${stop}: ${tail((applied.run && applied.run.output) || 'no output')}`)
        report.status = 'failed'
        report.error = `the Term-link pass stopped as ${stop}; its unit stays incomplete`
        log(`Term link: failed — ${report.error}.`)
        return report
      }

      const completed = await ledger('complete', `done -Out "${OUT}" -Unit "${unit.id}" -Token ${unit.token} -Outputs "${summary.report || `${WORK}\\apply.json`}"`)
      if (!completed.ok || !completed.line || completed.line.state !== 'done') {
        await fault('complete fault', unit, '', `the Term-link completion was refused: ${tail(completed.output)}`)
        report.status = 'failed'
        report.error = `the Term-link unit could not complete: ${tail(completed.output)}`
        return report
      }
      report.status = 'done'
      report.completion = completed.line
      report.counts = report.counts || completed.line.counts || null
      log(`Term link: ${unit.id} complete — ${(report.counts && report.counts.cards_changed) || 0} Card(s) changed, ${(report.counts && report.counts.links_added) || 0} link(s) added${report.no_op ? ', proven no-op' : ''}.`)
      return report
    } catch (e) {
      if (e && e.lock_refusal) {
        // The owner's Lock stops the pass: the unit keeps no completion and the run keeps its debt.
        if (unit) await fault('lock fault', unit, 'card_locked', e.message)
        throw e
      }
      report.status = 'failed'
      report.error = tail((e && e.message) || 'the Term-link pass failed')
      if (unit) await fault('branch fault', unit, '', report.error)
      return report
    }
  },
})
// ==== end module zt-term-link ====

const BLUEPRINT = {
 "protocol": "workflow-framework-protocol/1",
 "name": "zt-econ-digest",
 "version": "6.5.4",
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
        "value": "^(audit|resume)$"
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
        "path": "mode",
        "op": "!=",
        "value": "resume"
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
      "message": "mode resume reads the retained identity, source, Citation key, enabled layers and settings from its ledger and accepts only out: a new identity, source, group count, model list or Bridge selection needs its own preparation"
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
   "detail": "the Term-card branch beside the digest writers (design 14.1): the compiler's writer jobs in waves of four on the shared DSMP pool, their checks, the independent mathematical reviews and the one finite fix; each job unit and the book unit complete only from the compiler's own checked artifacts, so a failed or partial book stays incomplete",
   "after": [
    "terms"
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
       "value": "^(full|resume)$"
      },
      {
       "path": "args.termCards",
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
   "detail": "the deterministic Term-link pass after every Card writer and the Bridge (design 14.1-14.2): claim the run's termlink/book@1 unit, freeze the plan, apply it under the claim and complete the unit only from the compiler's apply report; a failed Term-card book or Card writer leaves it blocked and reported, never linked",
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
       "value": "^(full|resume)$"
      },
      {
       "path": "args.termLink",
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
   "detail": "the finite finalization: the audit plan, at most one compiler correction cycle scoped to its Cards, the changed-Card/touched-tree manifest, the dependency-scoped publication refresh, and the final read-only audit",
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
  "version": "5.5.4"
 },
 "installed": {
  "protocol": "workflow-framework-protocol/1",
  "kernel": "v1",
  "modules": {
   "gate": "1",
   "zt-workflow-lib": "1",
   "zt-digest-lib": "37",
   "zt-audit": "3",
   "zt-book-terms": "4",
   "zt-bridge": "9",
   "zt-cards-book": "4",
   "zt-cards-layer": "2",
   "zt-digest-prepare": "13",
   "zt-digest-report": "17",
   "zt-dsh-cleanup": "11",
   "zt-dsh-source": "2",
   "zt-event-layer": "4",
   "zt-model-lib": "10",
   "zt-explain": "7",
   "zt-figures": "4",
   "zt-finalize": "2",
   "zt-finding-layer": "4",
   "zt-group-cycle": "6",
   "zt-link-check": "2",
   "zt-link-pass": "7",
   "zt-link-prepare": "6",
   "zt-model-layer": "6",
   "zt-organize": "8",
   "zt-term-cards": "2",
   "zt-term-link": "1"
  }
 }
}
const FINGERPRINT = 'c61196114487a6f4'
return await wfpRun(BLUEPRINT, FINGERPRINT)
