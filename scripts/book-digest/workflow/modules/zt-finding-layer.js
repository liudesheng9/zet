// ==== module zt-finding-layer v1 ====
// The empirical-results layer of the economics set (docs/finding-cards-design.md): after the digest,
// finding-compiler.ps1 plans one job per few groups from the sentences that report data with a number and each
// group's own Idea Cards; DeepSeek writes one Finding Card per result as JSON and runs the job's SELF-CHECK
// (precheck: the fields, the supported Cards, zt-check F1 to F3 and the Card language on the drafts); the
// script creates each Finding Card (type finding) beside its group's anchor and links it from every supported
// Idea Card with the kind word 印证. Needs zt 0.20.0 (the Card type finding). The last step deletes the DeepSeek
// conversations.
defineModule({
  id: 'zt-finding-layer', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest empirical-results layer: Finding Cards, one per empirical result (finding-compiler.ps1 plan → DeepSeek JSON → apply → report).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\findings`
    const fstep = (s, more = '') => `pwsh -NoProfile -File "${Z.SCRIPTS}\\finding-compiler.ps1" -Step ${s} -Staging "${Z.OUT}"${more}`
    const opts = { dsh: work, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Findings', writes: `${work}\\out`, keep: true }
    const planned = await Z.drive('findings: write', { ...opts, command: fstep('plan') })
    const plan = zdLastJson(planned && planned.output)
    if (!plan) {
      await Z.drive('findings: clean up', { ...opts, command: fstep('report'), cleanup: true })
      return { status: 'failed', error: `plan: ${(planned && planned.error) || 'no output'}` }
    }
    const applied = zdLastJson(((await Z.drive('findings: apply', { ...opts, command: fstep('apply', ' -Apply') })) || {}).output)
    const reported = zdLastJson(((await Z.drive('findings: report', { ...opts, command: fstep('report'), cleanup: true })) || {}).output)
    log(`Findings: ${plan.candidates || 0} result sentences in ${plan.groups || 0} groups, ${(plan.jobs || []).length} DeepSeek jobs; ${applied ? `${applied.created} Finding Cards made, ${applied.linked} Links from Idea Cards, ${(applied.refused || []).length} refused` : 'apply gave no output'}.`)
    return { status: applied && applied.ok !== false ? 'done' : 'failed', plan: { candidates: plan.candidates, groups: plan.groups, jobs: (plan.jobs || []).length }, applied, report: reported }
  },
})
// ==== end module zt-finding-layer ====
