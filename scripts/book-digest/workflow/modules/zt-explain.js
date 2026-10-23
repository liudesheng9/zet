// ==== module zt-explain v6 ====
// Explanations of finished Derivation parts (R219/R301/R302): the ledger's ready `explain/<model-key>@1` work,
// which a checked Model completion registers. Each claimed block is written by the existing explanation chain
// (plan one job per chain, DeepSeek JSON, the script writes 思路/说明/解读 and refuses a changed 起点/step/结果,
// one refused part going back to its own conversation once); every unit is then recorded through
// `explain-finish`, which re-reads the parts and refuses a part that still lacks text. Runs in mode explain and
// after the model layer in mode full.
// 6: the chain ends with the block's checked finishes in one command (`zt-digest finish-batch`), run by the chain's own
// manager; a unit its line does not answer is finished by one relay, then on its own (docs/digest-claude-token-proposal.md P1/P2).
defineModule({
  id: 'zt-explain', version: '6', requires: ['zt-model-lib'],
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
        const only = Array.isArray(A.only) && A.only.length ? ` -Models "${models.join(',')}"` : ` -Models "${models.join(',')}"`
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
