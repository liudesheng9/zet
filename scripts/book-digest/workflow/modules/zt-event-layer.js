// ==== module zt-event-layer v2 ====
// The event layer of the economics set: after the digest (whose runs write no Event Card), event-compiler.ps1
// plans one job per few groups from the book's event candidates (sentences that name a year) and each group's
// own Idea Cards; DeepSeek writes the events as JSON and runs the job's SELF-CHECK (precheck: the fields, the
// supported Cards, zt-check E1, E2 and the Card language on the drafts); the script creates each Event Card
// beside the Idea Card it supports (or adds this book's source to an existing Event Card) and links it from
// every supported Idea Card with the kind word 案例. The last step deletes the DeepSeek conversations.
defineModule({
  id: 'zt-event-layer', version: '2', requires: ['zt-digest-lib'],
  summary: 'ZT digest event layer: Event Cards from the book\'s dated sentences (event-compiler.ps1 plan → DeepSeek JSON → apply → report).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\events`
    const estep = (s, more = '') => `${Z.tool('event-compiler')} -Step ${s} -Staging "${Z.OUT}"${more}`
    const opts = { dsh: work, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Events', writes: `${work}\\out`, keep: true }
    const planned = await Z.drive('events: write', { ...opts, command: estep('plan') })
    const plan = zdLastJson(planned && planned.output)
    if (!plan) {
      await Z.drive('events: clean up', { ...opts, command: estep('report'), cleanup: true })
      return { status: 'failed', error: `plan: ${(planned && planned.error) || 'no output'}` }
    }
    const applied = zdLastJson(((await Z.drive('events: apply', { ...opts, command: estep('apply', ' -Apply') })) || {}).output)
    const reported = zdLastJson(((await Z.drive('events: report', { ...opts, command: estep('report'), cleanup: true })) || {}).output)
    log(`Events: ${plan.candidates || 0} dated sentences in ${plan.groups || 0} groups, ${(plan.jobs || []).length} DeepSeek jobs; ${applied ? `${applied.created} Event Cards made, ${applied.merged} merged into existing ones, ${applied.linked} Links from Idea Cards, ${(applied.refused || []).length} refused` : 'apply gave no output'}.`)
    return { status: applied && applied.ok !== false ? 'done' : 'failed', plan: { candidates: plan.candidates, groups: plan.groups, jobs: (plan.jobs || []).length }, applied, report: reported }
  },
})
// ==== end module zt-event-layer ====
