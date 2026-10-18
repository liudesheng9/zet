// ==== module zt-event-layer v3 ====
// The Event layer of the economics set: the ledger's ready event work, worked while the digest groups are
// still running. `event-compiler` plans one job per few groups from the book's dated sentences and each
// group's own Idea Cards; DeepSeek writes the events as JSON and runs the job's SELF-CHECK; the compiler
// creates each Event Card beside the Idea Card it supports (or adds this book's source to an existing Event
// Card) and links it from every supported Idea Card with the kind word 案例. This module claims the ready
// `events/gNNN@1` units in bounded blocks, runs that compiler chain once per block in one manager (R250), then
// records each group through `events-finish`, so a group whose job has not been applied stays open instead of
// being claimed complete. The layer gate is the stage's result (R257).
defineModule({
  id: 'zt-event-layer', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest event layer: the ledger\'s ready Event work, plan → DeepSeek JSON → apply → per-group checked completion.',
  params: {
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
          ],
          cleanup: true, keep: true,
        })
        const applied = Z.jsonOf(chained, 'apply')
        const plan = Z.jsonOf(chained, 'plan')
        log(`Events: ${plan ? `${plan.candidates || 0} dated sentences in ${plan.groups || 0} groups, ${(plan.jobs || []).length} DeepSeek jobs` : 'plan gave no output'}; ${applied ? `${applied.created} Event Cards made, ${applied.merged} merged, ${applied.linked} Links, ${(applied.refused || []).length} refused` : 'apply gave no output'}.`)
        const out = []
        for (const unit of units) {
          out.push(await Z.finishOrFault(`events ${unit.id}`, unit.id, unit.token, unit.inputs))
        }
        return out
      },
    })
    log(`Events: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} groups checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, dispositions: result.dispositions, gate: result.gate }
  },
})
// ==== end module zt-event-layer ====
