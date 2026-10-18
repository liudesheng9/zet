// ==== module zt-link-pass v2 ====
// The cross-part link pass: each Haiku manager starts up to linkPerManager DeepSeek runs at once (the profile's
// link prefix plus the group's link file); each run appends reasoned Links to the Cards its group owns, through
// the helper. The Link units (`link/gNNN@1`) are claimed here and recorded honestly: the pass still writes
// through the helper, so a finished group has no receipt-bound draft for `link-finish`. Each claimed unit is
// therefore recorded as an explicit content failure naming the missing draft transport (R263's Link half), so
// the Link layer settles as a visible partial result instead of staying open forever, and downstream aggregate
// work records the exact unresolved Link units instead of guessing. The group draft transport (R263's group
// half) is implemented in zt-group-cycle.
defineModule({
  id: 'zt-link-pass', version: '2', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the cross-part link runs on the pending groups, with the Link units recorded as unverified legacy-transport work.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
    pending: { type: 'array', required: true, description: 'the pending group numbers from the link preparation' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const pending = Z.groupsOf(p.groups).filter(g => p.pending.includes(g.n))
    let reports = []
    if (pending.length) {
      const ran = await Z.runStage('link', Z.toBlocks(pending, Z.LINK_PER_MANAGER))
      reports = ran.reports
    }
    // Record the Link units: claimed, then an explicit content fault for the missing draft evidence.
    const linkUnits = []
    for (let round = 1; round <= 2; round++) {
      const got = await Z.claim(`link units ${round}`, 'link', 8, 0)
      const units = (got && got.units) || []
      if (!units.length) break
      for (const unit of units) {
        linkUnits.push(unit.id)
        await Z.failUnit(`link ${unit.id}`, unit.id, unit.token, 'content', {
          schema_version: 1, unit: unit.id, input_hash: unit.input_hash || '',
          code: 'unsupported_claim',
          cause: 'the Link pass wrote through the helper without a receipt-bound draft; link-finish cannot verify this unit (R263 Link transport remains open)',
        })
      }
    }
    if (linkUnits.length) log(`Link: ${linkUnits.length} Link units recorded as unverified legacy-transport work; the layer gate reports them instead of claiming completion.`)
    return { reports, link_units: linkUnits }
  },
})
// ==== end module zt-link-pass ====
