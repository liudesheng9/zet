// ==== module zt-hubs v3 ====
// The hub compiler on the whole agent Workspace: a Haiku hub manager reads the survey and chooses the limits;
// hub-compiler.ps1 does every structural step; DeepSeek plans the fields and writes each hub as JSON; Haiku
// drivers relay the DeepSeek jobs; check and at most hubFixRounds fix rounds; report and clean-up.
// v3 adds the concept register's own publication (#101/#104, S23/#115): after the hub report, and only in the
// final `hubs` stage (params.register), the stage plans `zt-digest register` from every agent book term table,
// the reviewed concept report when the run has one, and the accepted pinyin readings. The plan is read-only and
// is always reported; it is applied only for the exact accepted plan id in args.registerPlan, and then checked
// by reading every page back. Report-first is the default: a new book's authorization does not maintain an
// existing register (S23-AC07), and an interrupted publication keeps its plan so a retry replays its receipts.
const ZH_LIMITS = {
  type: 'object',
  properties: {
    ok: { type: 'boolean' }, error: { type: 'string' }, survey: { type: 'string' },
    limits: { type: 'object', properties: {
      topic_min: { type: 'number' }, line_min: { type: 'number' }, max_lines: { type: 'number' }, max_groups: { type: 'number' },
      min_reason: { type: 'number' }, neighbour_links: { type: 'number' }, field_min: { type: 'number' }, field_max: { type: 'number' }, per_job: { type: 'number' },
    }, required: ['topic_min', 'line_min', 'max_lines', 'max_groups', 'min_reason', 'neighbour_links', 'field_min', 'field_max', 'per_job'] },
    reasons: { type: 'string' },
  },
  required: ['ok', 'limits'],
}
defineModule({
  id: 'zt-hubs', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the Hub Cards of the whole agent Workspace (hub-compiler.ps1 with a Haiku hub manager), then the concept-register plan and, only under the accepted plan id, its publication.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    register: { type: 'boolean', default: false, description: 'true: this stage is the final hub stage, so it also plans the concept register (report-first) and applies only the accepted plan id' },
    conceptReview: { type: 'any', description: 'the concept review output (stages.concepts.output): its frozen report is read-only evidence for the register plan' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const A = Z.A
    const HUB_WORK = Z.HUB_WORK
    const hstep = (s, extra = '') => `${Z.HUB_COMPILER} -Step ${s} -Work "${HUB_WORK}"${extra}`
    const manager = await Z.zAgent(
      `You are the hub manager of a ZET slip-box. Hub Cards are the ways into a subject: a topic hub for each large Topic tree, a line hub for each large branch, field hubs over related Topics, and one register. A script builds them; DeepSeek writes them. You choose the limits that decide which hubs the policy requires and how large a hub may be. You read no Card.
1. Run this one PowerShell command in the foreground with timeout_ms: 600000 and wait for it:
${hstep('survey')}
   Its last output line is compact JSON: topics, tree_cards, branch_cards, and neighbour_links (each a distribution: n, min, p25, median, p75, p90, max), hubs (counts per level), legacy (Idea Cards used as hubs), register. Put that line, unchanged, in survey. If the command fails, return ok=false with the error text; do not retry with other options.
2. Choose the limits from the distributions with these rules, and round to whole numbers:
   - topic_min (a Topic tree with this many Cards needs a topic hub): tree_cards.p25, but at least 5 and at most 12. A tree smaller than most trees needs no map.
   - line_min (a branch with this many Cards needs a line hub): branch_cards.p90, but at least 6, at most 20, and never below topic_min.
   - max_lines (the most member lines of one hub): tree_cards.p75, but at least 15 and at most 25. Luhmann's collective reference cards held at most about 25 references.
   - max_groups: 7; 5 when max_lines is below 18.
   - min_reason (the fewest characters of a line's reason): 8.
   - neighbour_links (two trees with this many Links between them are neighbours): neighbour_links.median, but at least 2 and at most 6; 3 when n is 0.
   - field_min: 3. field_max (the most Topics in one field): the square root of topics, but at least 6 and at most 15.
   - per_job (Topic trees per DeepSeek writer job): 6 when tree_cards.median is 15 or less, 4 when it is 30 or less, else 2.
3. Return ok=true, survey, limits, and reasons: one short sentence per limit that names the number from the survey it came from.`,
      { label: 'hub manager: survey and limits', phase: 'Hubs', model: 'haiku', schema: ZH_LIMITS },
    )
    if (!manager || !manager.ok) {
      log(`Hubs: the survey failed (${manager && manager.error}).`)
      return { status: 'failed', error: (manager && manager.error) || 'no report from the hub manager' }
    }
    const limits = { ...manager.limits, ...(A.hubLimits && typeof A.hubLimits === 'object' ? A.hubLimits : {}) }
    log(`Hub limits from the Haiku hub manager: ${JSON.stringify(limits)}. ${manager.reasons || ''}`)
    const sessions = {}
    let jobs = 0
    const hrun = async (what, command, opts = {}) => {
      const r = await Z.drive(`hubs ${what}`, {
        command, dsh: HUB_WORK, skills: Z.HUB_SKILLS, effort: Z.HUB_EFFORT, sessions: opts.continues ? sessions : null,
        cleanup: opts.cleanup, phase: 'Hubs', writes: `${HUB_WORK}\\out`, remove: 'delete', keep: true,
      })
      for (const x of (r && r.results) || []) {
        jobs++
        if (x.session_id) sessions[x.id] = x.session_id
      }
      if (!r || !r.ok) log(`hubs ${what}: ${(r && r.error) || 'no report from its driver'}`)
      return r
    }
    const limitArg = JSON.stringify(limits).replace(/'/g, "''")
    const written = await hrun('write', hstep('plan', ` -Limits '${limitArg}'${A.hubTrees ? ` -Trees "${A.hubTrees}"` : ''}${A.hubRebuild ? ' -Rebuild' : ''}`))
    const plan = zdLastJson(written && written.output)
    if (!plan) return { status: 'failed', limits, error: `plan failed: ${(written && written.error) || 'no output'}` }
    const failedJobs = ((written && written.results) || []).filter(x => x.status !== 'done').map(x => x.id)
    log(`Hubs: ${plan.tree_count ?? (plan.trees || []).length} Topic trees to write in ${plan.jobs.length} DeepSeek jobs (one is the field planner)${failedJobs.length ? `; failed jobs ${failedJobs.join(', ')}` : ''}.`)
    let applied = zdLastJson(((await hrun('apply 1', hstep('apply'))) || {}).output)
    let checked = null
    for (let round = 1; round <= Z.HUB_FIX_ROUNDS; round++) {
      checked = zdLastJson(((await hrun(`check ${round}`, hstep('check'), { continues: true })) || {}).output)
      log(`Hubs check ${round}: ${checked ? `${checked.hubs} hubs, ${checked.faults} faults, ${checked.jobs.length} fix jobs` : 'no output'}.`)
      if (!checked || !checked.jobs.length) break
      applied = zdLastJson(((await hrun(`apply ${round + 1}`, hstep('apply'))) || {}).output)
    }
    const reported = zdLastJson(((await hrun('report', hstep('report'), { cleanup: true })) || {}).output)
    log(`Hubs: ${JSON.stringify((reported && reported.counts) || {})}; faults ${JSON.stringify((reported && reported.faults) || {})}; ${reported ? reported.mirror : '?'} mirror hubs; ${reported ? reported.missing_count : '?'} required hubs still missing; ${jobs} DeepSeek jobs.`)
    if (!applied) log('Hubs: an apply step failed; the stage is marked failed.')
    const hubsDone = !!(reported && applied)
    // The concept register: plan always (read-only), publish only for the accepted plan id.
    let register = null
    if (p.register) {
      const work = `${Z.OUT}\\register`
      const review = p.conceptReview && p.conceptReview.report_path ? ` -Review "${p.conceptReview.report_path}"` : ''
      const rstep = (s, more = '') => `${Z.DIGEST} register -Step ${s} -Work "${work}"${more}`
      const planned = await Z.runScript('register plan', 'Hubs', rstep('plan', review), ' (read-only: it stages the register plan and writes no Card)')
      const staged = planned.json
      if (!staged || staged.ok !== true) {
        log(`Register: the plan failed (${(planned.run && planned.run.output) || 'no output'}); nothing was published.`)
        register = { status: 'failed', error: (planned.run && planned.run.output) || 'no register plan', accepted: false }
      } else {
        const accepted = typeof A.registerPlan === 'string' && A.registerPlan === staged.plan_id
        log(`Register plan ${staged.plan_id}: ${staged.entries ?? 0} term entries, ${staged.unresolved ?? 0} unresolved, ${staged.faults ?? 0} faults, ${staged.read_only ?? 0} read-only targets, ${staged.parts ?? 0} parts${accepted ? '; the run accepts exactly this plan, so it is published now' : '; report-first: name it as registerPlan to publish it'}.`)
        register = {
          status: accepted ? 'applying' : 'planned', plan_id: staged.plan_id, entries: staged.entries ?? 0,
          unresolved: staged.unresolved ?? 0, faults: staged.faults ?? 0, read_only: staged.read_only ?? 0,
          parts: staged.parts ?? 0, review: staged.review ?? null, accepted, path: staged.path || `${work}\\out\\register-plan.json`,
        }
        if (accepted) {
          const published = await Z.runScript('register apply', 'Hubs', rstep('apply'), ' (identified writes under the accepted plan; a retry replays the same receipts)')
          const done = published.json
          const verified = done && done.ok === true
            ? await Z.runScript('register check', 'Hubs', rstep('check'), ' (it reads every page and writes nothing)')
            : null
          const audit = verified && verified.json
          register.applied = !!verified
          register.complete = done ? done.complete === true : false
          register.verified = audit ? audit.complete === true && (audit.faults ?? 1) === 0 : false
          register.missing = audit ? audit.missing ?? 0 : null
          register.diverged = audit ? audit.diverged ?? 0 : null
          register.extra = audit ? audit.extra ?? 0 : null
          register.status = !verified ? 'pending' : register.verified ? 'published' : 'partial'
          if (!verified) log(`Register: the publication was interrupted or refused (${(published.run && published.run.output) || 'no output'}); the accepted plan stays staged, so a retry replays its receipts.`)
          else log(`Register ${staged.plan_id}: ${register.verified ? 'verified' : 'published with unresolved rows'}; ${register.entries} entries, ${register.missing ?? '?'} missing, ${register.diverged ?? '?'} diverged, ${register.extra ?? '?'} extra.`)
        }
      }
    }
    const status = hubsDone ? (register && register.status === 'failed' ? 'partial' : 'done') : 'failed'
    return {
      status, limits, reasons: manager.reasons, trees: plan.tree_count ?? (plan.trees || []).length, applied, report: reported, register,
      ...(hubsDone ? {} : { error: 'apply failed: no output from hub-compiler.ps1 -Step apply' }),
    }
  },
})
// ==== end module zt-hubs ====
