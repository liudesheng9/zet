// ==== module zt-event-layer v5 ====
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
defineModule({
  id: 'zt-event-layer', version: '5', requires: ['zt-digest-lib'],
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
        const plan = started.plan || {}
        log(`Events: ${plan.candidates || 0} dated sentences in ${plan.groups || 0} groups, ${started.jobs || 0} DeepSeek jobs.`)
      },
      onFinish: (finished) => {
        const applied = (finished.mid || []).find((m) => m.step === 'apply') || {}
        log(`Events: ${applied.created || 0} Event Cards made, ${applied.merged || 0} merged, ${applied.linked || 0} Links, ${applied.refused || 0} refused.`)
      },
    })
    log(`Events: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} groups checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    if (result.status === 'failed') return { status: 'failed', error: result.error, dispositions: result.dispositions, gate: result.gate }
    return { status: result.status, dispositions: result.dispositions, gate: result.gate }
  },
})
// ==== end module zt-event-layer ====
