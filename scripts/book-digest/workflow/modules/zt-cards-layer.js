// ==== module zt-cards-layer v1 ====
// The Cards layer of a managed run: one deterministic offline check per verified group (`cards/gNNN@1`).
// `group-finish` registers the check; this module claims the ready checks in bounded blocks and records each
// one through `cards-finish`, which re-reads the group's owned Cards, runs the shared checker in-process and
// hands every hard fault to a registered repair consumer. It never writes a Card and never starts an agent for
// a check that is not ready (R252/R254/R257).
defineModule({
  id: 'zt-cards-layer', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the per-group offline Card checks, claimed from the ledger ready work and recorded by cards-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const result = await Z.layer('cards', {
      layer: 'cards', take: 4, rounds: Z.MAX_ATTEMPTS,
      work: async (units) => {
        const out = []
        for (const unit of units) {
          out.push(await Z.finishOrFault(`cards ${unit.id}`, unit.id, unit.token, unit.inputs))
        }
        return out
      },
    })
    log(`Cards: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} verified, ${result.dispositions.filter(d => d.status === 'failed').length} failed${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, dispositions: result.dispositions, gate: result.gate }
  },
})
// ==== end module zt-cards-layer ====
