// ==== module zt-explain v2 ====
// Mode "explain": 思路, 说明, and 解读 for the finished Derivation parts of a book (or of the Model Cards in
// args.only). model-compiler.ps1 plans one job per chain, DeepSeek writes JSON, the script writes the Cards and
// refuses any change to 起点, a step, or 结果. A refused part goes back to the same conversation once.
defineModule({
  id: 'zt-explain', version: '2', requires: ['zt-model-lib'],
  summary: 'ZT digest: explanations of finished Derivation parts (mode explain).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmlMake(ctx, p)
    const Z = M.Z
    const A = Z.A
    const work = `${Z.OUT}\\explain`
    const xstep = (s, extra = '') => `pwsh -NoProfile -File "${Z.COMPILER}" -Step ${s} -Work "${work}" -Staging "${Z.OUT}" ${Z.LIMITS}${extra}`
    let models = Array.isArray(A.only) && A.only.length ? A.only : null
    if (!models) {
      const listed = await M.listModels(`-All -CitationKey ${Z.KEY}`, 'find models: explain')
      if (!listed || !listed.ok) return { status: 'failed', error: `listing failed: ${listed && listed.error}` }
      models = listed.models.filter(m => m.level !== 'concept').map(m => m.address)
    }
    if (!models.length) return { status: 'done', models: [] }
    const sessions = {}
    let jobs = 0
    const xrun = async (what, command, opts = {}) => {
      const r = await Z.drive(`explain: ${what}`, {
        command, dsh: work, skills: Z.FILL_SKILLS, effort: Z.DSH_EFFORT, sessions: opts.continues ? sessions : null,
        cleanup: opts.cleanup, phase: 'Models', writes: `${work}\\out`, keep: true,
      })
      for (const x of (r && r.results) || []) {
        jobs++
        if (x.session_id) sessions[x.id] = x.session_id
      }
      if (!r || !r.ok) log(`explain ${what}: ${(r && r.error) || 'no report from its driver'}`)
      return r
    }
    const planned = zdLastJson(((await xrun('plan', xstep('explain', ` -Models "${models.join(',')}"${A.explainMax ? ` -MaxParts ${A.explainMax}` : ''}`))) || {}).output)
    if (!planned) return { status: 'failed', error: 'explain plan: no output from model-compiler.ps1' }
    log(`Explain: ${planned.parts || 0} Derivation parts of ${models.length} models in ${(planned.jobs || []).length} DeepSeek jobs.`)
    // The first apply writes what passes and sends refused parts back once; the second writes the follow-ups.
    const first = zdLastJson(((await xrun('apply 1', xstep('explain-apply'), { continues: true })) || {}).output)
    const second = first && (first.jobs || []).length ? zdLastJson(((await xrun('apply 2', xstep('explain-apply'))) || {}).output) : null
    const reported = zdLastJson(((await xrun('report', xstep('explain-report'), { cleanup: true })) || {}).output)
    const rows = (reported && reported.models) || []
    const sum = (field) => rows.reduce((s, r) => s + (Number(r[field]) || 0), 0)
    const refused = [...((first && first.refused) || []), ...((second && second.refused) || [])]
    log(`Explain: ${sum('explained')} of ${sum('derivations')} Derivation parts have 思路 and 解读; ${(first ? first.applied : 0) + (second ? second.applied : 0)} written now, ${refused.length} refused, ${rows.reduce((s, r) => s + (r.blocked || []).length, 0)} blocked; ${jobs} DeepSeek jobs.`)
    return { status: reported && first ? 'done' : 'failed', parts: planned.parts, applied: [first, second].filter(Boolean), refused, report: reported }
  },
})
// ==== end module zt-explain ====
