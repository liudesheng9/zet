// ==== module zt-group-cycle v6 ====
// The Digest stage of a managed run, in blocks. One Haiku manager runs up to eight digest groups side by side
// (the DeepSeek Subagent Managed Protocol lets one manager run eight DeepSeek conversations): it claims up to
// eight `digest/gNNN@1` units, starts each (the snapshot is generated before any usable job manifest exists,
// R262), runs their frozen DeepSeek jobs together, renews every held claim while it waits, records and deletes
// each conversation itself, and records each checked completion with `group-finish` (R250/R251/R253).
// At most eight such lanes run at once; each lane claims block after block until no group is ready. A unit that
// fails is recorded against its own attempt and claimed again by a later block; the stage is never rerun as a
// whole (R258). A Haiku manager per single group spent a whole manager conversation on one DeepSeek job.
// #92: the claim requests a lease that covers the frozen provider-run cap.
// #87 (R280): `group-start` reconciles a retained accepted plan against the archive receipts; a recovered
// group's job declares `expected_jobs: 0`, runs no DeepSeek job, and completes from the committed result.
defineModule({
  id: 'zt-group-cycle', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest: verified groups in blocks of up to eight per manager, at most eight managers, claimed from the ledger ready work, recovering a group whose accepted plan already committed.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\group-jobs`
    const groups = Math.max(1, Number(p.groups) || 1)
    const lanes = Math.max(1, Math.min(8, Math.ceil(groups / Z.BLOCK_TAKE)))
    // Every block a lane may run: each group at most MAX_ATTEMPTS times, plus the empty claim that ends it.
    const maxBlocks = Math.ceil(groups / Z.BLOCK_TAKE) * Z.MAX_ATTEMPTS + 1
    const latest = new Map()
    // One held unit's outcome: the ledger decides (zt-digest-lib blockSettle); the manager's finish line adds
    // the details when it came back whole.
    const settle = async (u, label) => {
      const start = zdLastJson(u.start && u.start.output)
      const got = await Z.blockSettle(u, label)
      if (got.status !== 'done') return { status: got.status, group: u.group, unit: u.unit, error: got.error }
      if (start && start.recovered) log(`${label}: ${u.unit} recovered from its committed receipts; no DeepSeek job ran.`)
      return { status: 'done', group: u.group, unit: u.unit, disposition: got.parsed.disposition, owned: (got.parsed.owned || []).length, recovered: !!(start && start.recovered), jobs: u.jobs.length }
    }
    const lane = async (n) => {
      for (let round = 1; round <= maxBlocks; round++) {
        const label = `digest lane ${n} block ${round}`
        const got = await Z.block(label, {
          dsh: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, phase: 'Digest', writes: work,
          // The bounded claim: up to eight units, a lease that covers the frozen run cap (#92), and a
          // bounded readiness wait after the first block (a retried unit becomes ready again).
          claim: `${Z.LEDGER} next -Out "${Z.OUT}" -Layer digest -Take ${Z.BLOCK_TAKE} -Brief -Owner "${label}"${round > 1 ? ' -Wait 60' : ''} -Lease ${Z.CLAIM_LEASE}`,
          start: `${Z.DIGEST} group-start -Out "${Z.OUT}" -Group {{group}} -Token {{token}}`,
          finish: `${Z.DIGEST} group-finish -Out "${Z.OUT}" -Group {{group}} -Token {{token}} -Draft "{{draft}}"`,
        })
        if (!got.claimed.length) {
          // A manager that threw before its claim ran holds nothing: take the next round (trial 17).
          if (got.thrown && round < maxBlocks) continue
          if (!got.ok) log(`${label}: the claim did not run: ${got.error || 'no claim line'}`)
          return
        }
        for (const u of got.units) latest.set(u.unit, await settle(u, label))
      }
    }
    await Promise.all(Array.from({ length: lanes }, (_, i) => lane(i + 1)))
    const reports = [...latest.values()].sort((a, b) => (a.group || 0) - (b.group || 0))
    const verified = reports.filter((r) => r.status === 'done').length
    const failed = reports.filter((r) => r.status !== 'done').length
    log(`Digest: ${verified} group(s) verified, ${failed} not verified, in ${lanes} lane(s) of up to ${Z.BLOCK_TAKE} groups per manager.`)
    return { status: failed ? 'partial' : 'done', reports, verified, failed, lanes }
  },
})
// ==== end module zt-group-cycle ====
