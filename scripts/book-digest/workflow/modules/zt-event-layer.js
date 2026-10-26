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
