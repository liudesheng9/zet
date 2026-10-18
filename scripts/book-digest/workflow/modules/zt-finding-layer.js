// ==== module zt-finding-layer v3 ====
// The empirical-results layer of the economics set (docs/finding-cards-design.md): the ledger's ready finding
// work, worked while the digest groups are still running. `finding-compiler` plans one job per few groups from
// the sentences that report data with a number and each group's own Idea Cards; DeepSeek writes one Finding
// Card per result as JSON and runs the job's SELF-CHECK; the compiler creates each Finding Card beside its
// group's anchor and links it from every supported Idea Card with the kind word 印证. This module claims the
// ready `findings/gNNN@1` units in bounded blocks, runs that compiler chain once per block in one manager, and
// records each group through `findings-finish`, so an unapplied job leaves the group open. Needs zt 0.20.0
// (the Card type finding).
defineModule({
  id: 'zt-finding-layer', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest empirical-results layer: the ledger\'s ready Finding work, plan → DeepSeek JSON → apply → per-group checked completion.',
  params: {
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
          ],
          cleanup: true, keep: true,
        })
        const applied = Z.jsonOf(chained, 'apply')
        const plan = Z.jsonOf(chained, 'plan')
        log(`Findings: ${plan ? `${plan.candidates || 0} result sentences in ${plan.groups || 0} groups, ${(plan.jobs || []).length} DeepSeek jobs` : 'plan gave no output'}; ${applied ? `${applied.created} Finding Cards made, ${applied.linked} Links, ${(applied.refused || []).length} refused` : 'apply gave no output'}.`)
        const out = []
        for (const unit of units) {
          out.push(await Z.finishOrFault(`findings ${unit.id}`, unit.id, unit.token, unit.inputs))
        }
        return out
      },
    })
    log(`Findings: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} groups checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, dispositions: result.dispositions, gate: result.gate }
  },
})
// ==== end module zt-finding-layer ====
