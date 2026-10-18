// ==== module zt-model-lib v2 ====
// The model layer's machinery (docs/zt-digest-framework.md), moved from zt-digest-lib: the model list
// (list-models.ps1) and the model compiler per batch: DeepSeek brief → Sonnet architect plan → DeepSeek fill,
// derive, review, split → report and clean-up (model-compiler.ps1 does every structural step). A book type
// installs the model layer with its profile (economics: the economic architect; mathematics: the math
// supplements and no Theory Card). zmlMake(ctx, params) returns listModels, elaborateAll, logModels, modelCleanup.
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
  const listModels = (selector, label) => zAgent(
    `List the Model Cards of a ZET book digest. Run this one PowerShell command in the foreground with timeout_ms: 600000 and read its last output line, a compact JSON object {"models":[{"address","title","group","level","parts","issues","faults"}]}:
${Z.tool('list-models')} -Staging "${OUT}" ${selector}
If it fails, return ok=false with the error text and models=[]; do not retry with other options. Otherwise return ok=true and the models array exactly.`,
    { label, phase: 'Models', model: 'haiku', schema: MODEL_LIST },
  )

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
  const compileBatch = async (batch, kind) => {
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
    const plans = (planned && planned.plans) || []
    if (!plans.some(p => p.status === 'planned')) {
      await run('clean up', step('report'), [], DSH_EFFORT, { cleanup: true })
      return batch.map(m => skipped.has(m.address) ? { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
        : { model: m.address, status: (plans.find(p => p.model === m.address) || {}).status || 'failed', parts: [], charts: 0, issues: planned ? [] : ['no plan from the architect'] })
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
    // 6. Report (script) and clean-up.
    const reported = zdLastJson((await run('report', step('report'), [], DSH_EFFORT, { cleanup: true }) || {}).output)
    const reports = (reported && reported.models) || []
    return batch.map(m => {
      const r = reports.find(x => x.model === m.address)
      const pl = plans.find(x => x.model === m.address) || {}
      if (skipped.has(m.address)) return { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
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
  const elaborateAll = async (models, label) => {
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
    const results = (await parallel(batches.map(b => () => compileBatch(b.models, b.kind).catch(error => { log(`batch ${b.models[0].address}: ${error}`); return null })))).flatMap((r, i) => r || batches[i].models.map(() => null))
    const ordered = batches.flatMap(b => b.models)
    return results.map((r, i) => r || { model: ordered[i].address, status: 'failed', parts: [], charts: 0, issues: ['no report from its batch'] })
  }
  const kindTally = (reports) => {
    const tally = {}
    for (const r of reports) for (const [kind, n] of Object.entries(r.kinds || {})) tally[kind] = (tally[kind] || 0) + n
    return Object.entries(tally).sort((a, b) => b[1] - a[1]).map(([kind, n]) => `${kind} ${n}`).join(', ') || 'no kinds reported'
  }
  const logModels = (reports) => {
    const done = reports.filter(r => r.status === 'done')
    const sum = (field) => done.reduce((s, r) => s + (r[field] || 0), 0)
    const deriv = (field) => done.reduce((s, r) => s + ((r.derivations && r.derivations[field]) || 0), 0)
    log(`Build: ${deriv('total')} Derivation parts in ${deriv('chains')} chains of two or more (longest ${done.reduce((m, r) => Math.max(m, (r.derivations && r.derivations.longest) || 0), 0)}), ${sum('split')} parts split by review or for the Card size, ${done.reduce((s, r) => s + ((r.moved || []).length), 0)} Cards moved into ranges, ${sum('shared')} shared parts taken in, ${sum('tolerated')} tolerated premises, ${sum('style')} style faults left, ${done.reduce((s, r) => s + ((r.todo || []).length), 0)} parts still 待写; ${dshJobs} DeepSeek jobs.`)
    log(`Levels: ${done.filter(r => r.level === 'concept').length} concept models (level 1), ${done.filter(r => r.level === 'formal').length} formal models (level 2), ${done.filter(r => !r.level).length} without a level.`)
    log(`Models: ${done.length} structured (${done.reduce((s, r) => s + r.parts.length, 0)} parts, ${done.reduce((s, r) => s + r.parts.filter(pp => pp.emphasis === 'core').length, 0)} core, ${sum('relations')} relations (${kindTally(done)}), ${sum('neighbours')} neighbours, ${sum('loose')} loose, ${sum('charts')} charts), ${reports.filter(r => r.status === 'skipped').length} skipped, failed ${reports.filter(r => r.status === 'failed').map(r => r.model).join(', ') || 'none'}; open issues ${reports.reduce((s, r) => s + r.issues.length, 0)}.`)
  }
  // A batch's cleanup deletes its own DeepSeek conversations; this sweep deletes any a failed batch left.
  const modelCleanup = async () => {
    if (!managerFolders.length) return null
    const c = await zAgent(
      `Clean up after the batches of a ZET model stage (a batch deletes its own conversations; this catches a batch that failed). Load tools with ToolSearch query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_delete,mcp__deepseek__deepseek_remove_workspace".
For each folder below: deepseek_list with that cwd (a workspace that is not found needs nothing); deepseek_delete every conversation there that is not running, and count them; then, when none is running, deepseek_remove_workspace with that cwd and delete_conversations true. Never archive a conversation.
${managerFolders.join('\n')}
Then delete what is left of those folders with one PowerShell call: ${managerFolders.map(f => `Remove-Item -LiteralPath "${f}" -Recurse -Force -ErrorAction SilentlyContinue`).join('; ')}
Return late_deleted (the count) and folders (how many folders you handled).`,
      {
        label: 'cleanup model managers', phase: 'Models', model: 'haiku',
        schema: { type: 'object', properties: { late_deleted: { type: 'number' }, folders: { type: 'number' } }, required: ['late_deleted'] },
      },
    )
    if (c) log(`Model clean-up: ${c.late_deleted} conversations deleted late.`)
    return c
  }


  return { Z, listModels, elaborateAll, logModels, modelCleanup, dshJobs: () => dshJobs }
}

defineModule({
  id: 'zt-model-lib', version: '2', library: true, requires: ['zt-digest-lib'],
  summary: 'Library: the model list and the model compiler (brief, architect plan, fill, derive, review, split) for the model layer.',
})
// ==== end module zt-model-lib ====
