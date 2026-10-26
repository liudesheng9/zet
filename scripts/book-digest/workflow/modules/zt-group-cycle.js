// ==== module zt-group-cycle v7 ====
// The Digest stage of a managed run, in blocks. One layer driver runs the stage as script steps: `start`
// claims a block of up to eight `digest/gNNN@1` units and runs each group's `group-start` (the snapshot is
// generated before any usable job manifest exists, R262), the block's DeepSeek writer jobs run in Haiku
// managers (at most eight per manager), and `finish` records each held unit's checked completion with
// `group-finish` (R250/R251/R253), faults the units whose draft is not ready and runs the layer gate. Every
// step is one `layer-steps.ps1` call run by a DeepSeek drive job, so no Claude agent relays a command and no
// Claude agent is spent per group. A unit that fails is recorded against its own attempt and claimed again by
// a later block; the stage is never rerun as a whole (R258).
// #92: the claim requests a lease that covers the frozen provider-run cap.
// #87 (R280): `group-start` reconciles a retained accepted plan against the archive receipts; a recovered
// group's job declares `expected_jobs: 0`, runs no DeepSeek job, and completes from the committed result.
defineModule({
  id: 'zt-group-cycle', version: '7', requires: ['zt-digest-lib'],
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
    const deepseek = { deleted: 0, not_deleted: [] }
    const result = await Z.layerRun('digest', {
      layer: 'digest', take: Z.PER_MANAGER, rounds: Z.MAX_ATTEMPTS, phase: 'Digest',
      dsh: work, writes: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, what: 'digest group writers', deepseek,
    })
    const reports = result.dispositions
      .map((d) => ({
        group: Number(d.group) || null, unit: d.unit, status: d.status === 'done' ? 'done' : d.status,
        disposition: d.disposition || null, owned: Number(d.owned || 0), recovered: d.recovered === true, jobs: Number(d.jobs || 0),
      }))
      .sort((a, b) => (a.group || 0) - (b.group || 0))
    const verified = reports.filter((r) => r.status === 'done').length
    const failed = reports.filter((r) => r.status !== 'done').length
    if (result.status === 'failed') {
      log(`Digest: the layer stopped: ${result.error}`)
      return { status: 'failed', error: result.error, reports, verified, failed, claimed: result.claimed, deepseek }
    }
    log(`Digest: ${verified} group(s) verified, ${failed} not verified, in ${result.claimed} claimed unit(s) of ${groups}, ${Z.PER_MANAGER} per manager, all Haiku managers and DeepSeek jobs.`)
    return { status: failed ? 'partial' : 'done', reports, verified, failed, claimed: result.claimed, deepseek }
  },
})
// ==== end module zt-group-cycle ====
