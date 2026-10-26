// ==== module zt-finding-layer v6 ====
// The empirical-results layer of the economics set (docs/finding-cards-design.md): the ledger's ready finding
// work, worked while the digest groups are still running. `finding-compiler` plans one job per few groups from
// the sentences that report data with a number and each group's own Idea Cards; DeepSeek writes one Finding
// Card per result as JSON and runs the job's SELF-CHECK; the compiler creates each Finding Card beside its
// group's anchor and links it from every supported Idea Card with the kind word 印证. The layer driver runs the
// stage as script steps: `start` claims a block of ready `findings/gNNN@1` units and runs the plan, the block's
// jobs run in Haiku managers, and `finish` applies, reports and records each group through `findings-finish`, so
// an unapplied job leaves the group open. Every step is one `layer-steps.ps1` call run by a DeepSeek drive job:
// no Claude agent relays a command and no Claude agent is spent per group or per finish. Needs zt 0.20.0 (the
// Card type finding).
// 4: the chain ends with the block's checked finishes in one command (`zt-digest finish-batch`), run by the chain's own
// manager; a unit its line does not answer is finished by one relay, then on its own (docs/digest-claude-token-proposal.md P1/P2).
// 5 (#175): the whole block is two script steps and one content round; the relay and the chain are gone.
// 6 (#180): a step answers counts, so the stage reports the block's created/linked totals and how many group
// units were verified; the per-unit outcomes are the ledger's run report.
defineModule({
  id: 'zt-finding-layer', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest empirical-results layer: the ledger\'s ready Finding work, plan → DeepSeek JSON → apply → per-group checked completion, in two script steps run by DeepSeek drive jobs.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\findings`
    const result = await Z.layerRun('findings', {
      layer: 'findings', take: 4, rounds: Math.max(3, Z.MAX_ATTEMPTS), wait: 600, phase: 'Findings',
      dsh: work, writes: `${work}\\out`, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, what: 'finding writers',
      onStart: (started) => {
        log(`Findings: ${started.plan_candidates || 0} result sentences in ${started.plan_groups || 0} groups, ${started.jobs || 0} DeepSeek jobs.`)
      },
      onFinish: (finished) => {
        log(`Findings: ${finished.mid_applied_created || 0} Finding Cards made, ${finished.mid_applied_linked || 0} Links, ${finished.mid_applied_refused || 0} refused.`)
      },
    })
    const t = result.totals || {}
    const applied = { created: Number(t.mid_applied_created || 0), linked: Number(t.mid_applied_linked || 0), refused: Number(t.mid_applied_refused || 0) }
    const verified = Number(result.verified || 0)
    const failed = Number(result.failed || 0)
    log(`Findings: ${result.status} — ${verified} groups checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    if (result.status === 'failed') return { status: 'failed', error: result.error, verified, failed, applied, gate: result.gate }
    return { status: result.status, verified, failed, applied, gate: result.gate }
  },
})
// ==== end module zt-finding-layer ====
