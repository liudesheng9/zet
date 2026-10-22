// ==== module zt-grow-cycle v10 ====
// The idea-grow compiler cycle (docs/idea-grow-design.md): `zt-digest grow` plans the run's Topics first (v2: one
// DeepSeek job over every section of every book, gt-001; in pilot 1 parallel sections each opened a Topic for one concept),
// then one DeepSeek job per section; v3 adds a link round (gl-NNN, one job per Topic tree) for Links across trees,
// then
// of a Literature tree, DeepSeek writes each section's ideas as JSON and runs the job's SELF-CHECK, the tool
// writes the Topic-tree Cards, checks them, and plans at most `fixRounds` fix rounds; then the report and the
// DeepSeek cleanup. The relayed lines carry counts only: the module names the jobs itself (gw-001 …,
// fx<round>-001 …), so no id list passes through a relay agent.
// v7 (Tree Lock S17, #140): a grow step or DeepSeek job the owner's Lock refused answers the tool's typed
// `card_locked` JSON. The stage reads that JSON, not an English sentence (R338), and stops where it stands:
// the Topic Cards already filed stay, its answer files stay, and nothing is retried or re-pointed through
// another write path (R326/R337).
// v8: the hubs round: topic, line and field hubs through the hub compiler (gh-NNN DeepSeek jobs, fix rounds),
// then the register rebuilt so its terms point at the Cards that carry them.
// v9 (docs/literature-only-digest-design.md): the digests build only Literature trees, so this cycle owns every
// Topic tree, Topic Card, hub and the register. It takes over from them: mode hubs (the hubs round and the
// register alone, no new Cards), the hub options (hubTrees for a pilot, hubRebuild, hubLimits, hubEffort,
// hubFixRounds), and the restatement review of the run's new Topic Cards (zt-digest restate: one judge job per
// source book, accepted rewrites through the run's own operation prefix).
// v10 (3.1.0): the two Topic-side jobs the digests had left. The concept review of each source book's term
// table against the Topics (zt-digest concepts: propose, prepare, one judge job, judge, report; grow -Step
// concept-books stages one folder per book) becomes the register plan's read-only -Review (hubs-report
// -Concepts), in mode full and mode hubs. The optional Topic maintenance round (organize: true) runs the
// old Organize's Topic merges and Card moves on the run's Topics before the hubs: grow -Step organize-plan
// surveys them (organize-survey -Topics), one DeepSeek job go-001 decides, grow -Step organize-apply checks
// and carries out the valid decisions (organize-apply -Open) and moves the run's own addresses with them.
defineModule({
  id: 'zt-grow-cycle', version: '10', requires: ['script', 'deepseek', '@dsmp'],
  summary: 'ZT idea grow: plan sections → DeepSeek ideas with SELF-CHECK → apply Topic Cards → check → fix rounds → report → cleanup; a write the owner\'s Lock refused stops the stage with the tool\'s typed card_locked refusal.',
  params: {
    book: { type: 'string', description: 'one Literature tree to grow from (its Citation key)' },
    books: { type: 'array', description: 'the Literature trees to grow from; with neither book nor books, every Literature tree of the box' },
    work: { type: 'string', required: true, description: 'the work folder (also the DeepSeek workspace)' },
    instructions: { type: 'string', required: true, description: 'the prompt template dsh-grow-instructions.zh.md' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command or the path of a zt-digest binary' },
    ztConfig: { type: 'string', description: 'the ZT_CONFIG_DIR of the box to grow (v5: a copy of the box with its own zt service); empty: the live box' },
    from: { type: 'number', description: 'the first section number to grow' },
    to: { type: 'number', description: 'the last section number to grow' },
    model: { type: 'string', required: true, description: 'DeepSeek Harness model of the jobs' },
    effort: { type: 'string', default: 'high', enum: ['high', 'max'], description: 'DeepSeek reasoning effort (never below high)' },
    skills: { type: 'array', default: ['zettelkasten', 'asd-ste100-zh'], description: 'DeepSeek Harness skills for every job' },
    fixRounds: { type: 'number', default: 1, min: 0, max: 2, description: 'fix rounds after the first apply' },
    hubs: { type: 'boolean', default: true, description: 'v8: run the hubs round (hub compiler and register) after the Cards are written' },
    register: { type: 'string', default: 'apply', enum: ['apply', 'plan', 'off'], description: 'v8: publish the rebuilt register (apply), stage it only (plan), or leave it (off)' },
    mode: { type: 'string', default: 'full', description: 'v9: the run\'s mode ({{mode}}): full grows Topic Cards; hubs runs only the hubs round and the register' },
    hubTrees: { type: 'string', description: 'v9: only these Topic trees get hubs (comma-separated Topic numbers; a pilot)' },
    hubRebuild: { type: 'boolean', default: false, description: 'v9: rewrite every hub, not only the hubs of trees that changed' },
    hubLimits: { type: 'any', description: 'v9: hub limits that replace the survey\'s (an object; the hub compiler clamps them)' },
    hubEffort: { type: 'string', default: 'high', enum: ['high', 'max'], description: 'v9: DeepSeek reasoning effort of the hub jobs (never below high)' },
    hubFixRounds: { type: 'number', default: 1, min: 0, max: 2, description: 'v9: hub fix rounds after the first hub apply' },
    restate: { type: 'boolean', default: true, description: 'v9: run the restatement review of the run\'s new Topic Cards' },
    concepts: { type: 'boolean', default: true, description: 'v10: the concept review of each source book\'s term table, read by the register plan as -Review (needs the hubs round and a register)' },
    organize: { type: 'boolean', default: false, description: 'v10: the Topic maintenance round before the hubs: duplicate-Topic merges and misplaced-Card moves on the run\'s Topics' },
    organizeTrees: { type: 'string', description: 'v10: the Topic trees the maintenance round surveys (comma-separated Topic numbers); empty: the run\'s own Topics' },
    runner: { type: 'string', default: 'runner', role: true, description: 'the declared role that relays the commands' },
  },
  run: async (ctx, p) => {
    const q = (v) => `'${String(v).replace(/'/g, "''")}'`
    const W = String(p.work).replace(/[\\/]+$/, '')
    const tool = p.tool || 'zt-digest'
    // Tree Lock S17 (#140, R326/R337): a write the owner's Lock refused is a planning error, not a
    // provider fault. The tool answers it as its typed `card_locked` JSON (exit 2, the JSON on the
    // last stdout line), so the stage reads that JSON rather than an English sentence (R338): it
    // stops where it stands, the Topic Cards it already filed stay, and nothing is retried or
    // re-pointed through another path.
    const LOCK = 'card_locked'
    const lockRefusal = (text) => {
      for (const line of String(text || '').split('\n')) {
        for (let i = line.indexOf('{'); i >= 0; i = line.indexOf('{', i + 1)) {
          for (let j = line.lastIndexOf('}'); j > i; j = line.lastIndexOf('}', j - 1)) {
            try {
              const v = JSON.parse(line.slice(i, j + 1))
              if (v && typeof v === 'object' && (v.code === LOCK || v.error === LOCK)) return v
            } catch (e) { }
          }
        }
      }
      return null
    }
    const lockStop = (where, refusal) => {
      const said = (refusal && (refusal.message || refusal.error)) || LOCK
      const error = new Error(`card_locked at ${where}: ${said} ${JSON.stringify(refusal)}`)
      error.lock_refusal = refusal
      return error
    }
    // A DeepSeek job that met the refusal reports it in its own answer ("FLAG: card_locked …"): the
    // stage stops with the same refusal instead of counting that section as a content failure.
    const stopOnLock = (where, results) => {
      for (const r of results || []) {
        const lock = lockRefusal([r && r.answer, r && r.error, r && r.id].filter(Boolean).join('\n'))
        if (lock) throw lockStop(`${where}${r && r.id ? ` ${r.id}` : ''}`, lock)
      }
    }
    // v5: a copy of the box runs its own zt service; every step names its config, and plan writes it
    // into the job files' helper lines.
    const env = p.ztConfig ? `$env:ZT_CONFIG_DIR=${q(p.ztConfig)}; ` : ''
    const exe = tool === 'zt-digest' ? 'zt-digest' : `& ${q(tool)}`
    // v6: the plan step runs before the work folder exists, so no step sets it as the working folder.
    // v7: a step whose answer carries the owner's typed refusal stops the stage with that refusal.
    const zd = async (sub, args, what) => {
      let out
      try {
        out = await wfpScript(ctx, { command: `${env}& ${q(tool)} ${sub} ${args}`, shell: 'powershell', role: p.runner || 'runner', label: `${ctx.stage}: ${what}` })
      } catch (error) {
        const lock = lockRefusal(error && error.message)
        if (lock) throw lockStop(`${ctx.stage}: ${what}`, lock)
        throw error
      }
      const lock = out && typeof out === 'object' && (out.code === LOCK || out.error === LOCK) ? out : null
      if (lock) throw lockStop(`${ctx.stage}: ${what}`, lock)
      return out
    }
    const sh = (args, what) => zd('grow', args, what)
    const approve = `allow only these: a command that runs the zt helper "${exe} call" with -Allow "${W}\\read-only.allow.txt" (an empty allow-list: reads only)${p.ztConfig ? `, which may first set $env:ZT_CONFIG_DIR='${p.ztConfig}'` : ''}; the self-check "${exe} grow -Step precheck" (read-only); a read-only read or search (Read, Get-Content, Select-String) of a file under "${W}"; and writing JSON files under "${W}\\out" or "${W}\\hubs\\out"; for a hub job (gh-…) also the hub self-check "${exe} hub -Step precheck" (read-only) and the zt helper with -Allow "${W}\\hubs\\none.allow.txt" (reads only). Reject anything else that writes, moves, or deletes files, and every other zt-digest step. Answer a question with custom text "按任务说明自行决定，不需要询问。"`
    const job = (id, files, effort) => ({
      id,
      run: {
        cwd: W,
        prompt_files: files,
        prompt: `按上面的任务说明和任务文件完成任务 ${id}。答案只写到任务文件给出的答案文件。\n\nSELF-CHECK: ${exe} grow -Step precheck -Work "${W}" -Job ${id}\n回复前运行 SELF-CHECK 命令。改正它报告的每个问题，再运行一次，直到它输出 "faults: 0"（最多三次）。最后一行回复 "OK" 或 "FLAG: <剩下的问题>"。`,
        model: p.model,
        reasoning_effort: (effort || p.effort) === 'max' ? 'max' : 'high',
        skills: p.skills && p.skills.length ? p.skills : ['zettelkasten', 'asd-ste100-zh'],
      },
      expect: '(^|\\n)(OK|FLAG:)',
      approve,
    })
    const ids = (prefix, n) => Array.from({ length: n }, (_, i) => `${prefix}-${String(i + 1).padStart(3, '0')}`)
    const out = { mode: p.mode === 'hubs' ? 'hubs' : 'full', plan: null, topics: null, rounds: [], restate: null, organize: null, concepts: null, hubs: null, report: null, workspaces: [] }
    // v9: the hubs round, shared by mode full (after the Cards) and mode hubs (alone). The hub options
    // reach the hub plan; the hub jobs run at hubEffort; their fix rounds are hubFixRounds. v10: with
    // concepts, the register plan reads the run's concept reports (hubs-report -Concepts).
    const hubsRound = async (concepts) => {
      const limits = p.hubLimits && typeof p.hubLimits === 'object' && Object.keys(p.hubLimits).length ? ` -Limits ${q(JSON.stringify(p.hubLimits))}` : ''
      const trees = p.hubTrees ? ` -Trees ${q(p.hubTrees)}` : ''
      const hp = await sh(`-Step hubs-plan -Work ${q(W)}${limits}${trees}${p.hubRebuild ? ' -Rebuild' : ''}`, 'hubs plan')
      const h = Math.max(0, Number(hp && hp.hub_jobs) || 0)
      log(`Hubs: ${h} hub jobs for ${hp && hp.topics} Topics; limits ${JSON.stringify(hp && hp.limits)}.`)
      if (h) {
        started = true
        const dh = await dsDispatch(ids('gh', h).map(id => job(id, [`${W}\\jobs\\${id}.md`], p.hubEffort)), { label: `${ctx.stage} hubs`, phase: ctx.phase, keepWorkspaces: true })
        stopOnLock(`${ctx.stage} hubs`, dh.results)
        const ha = await sh(`-Step hubs-apply -Work ${q(W)}`, 'hubs apply')
        out.rounds.push({ round: 'hubs', jobs: h, ok: dh.results.filter(r => r && r.ok).length, applied: ha })
        for (let round = 1; round <= (p.hubFixRounds ?? p.fixRounds ?? 1); round++) {
          const hc = await sh(`-Step hubs-check -Work ${q(W)} -Round ${round}`, `hubs check ${round}`)
          const m = Math.max(0, Number(hc && hc.fix) || 0)
          out.rounds.push({ round: `hubs-check${round}`, fix: m, hubs: hc && hc.hubs, faults: hc && hc.faults })
          if (!m) break
          const df = await dsDispatch(ids(`gh${round}`, m).map(id => job(id, [`${W}\\jobs\\${id}.md`], p.hubEffort)), { label: `${ctx.stage} hubs fix${round}`, phase: ctx.phase, keepWorkspaces: true })
          stopOnLock(`${ctx.stage} hubs fix${round}`, df.results)
          await sh(`-Step hubs-apply -Work ${q(W)}`, `hubs apply fix${round}`)
        }
      }
      out.hubs = await sh(`-Step hubs-report -Work ${q(W)} -Register ${q(p.register || 'apply')}${concepts ? ' -Concepts' : ''}`, 'hubs report')
      log(`Hubs: ${JSON.stringify(out.hubs && out.hubs.hubs)}; ${out.hubs && out.hubs.missing} required hubs missing, ${out.hubs && out.hubs.mirror} mirror hubs; register ${out.hubs && out.hubs.register && out.hubs.register.status} (${out.hubs && out.hubs.register && out.hubs.register.entries} terms).`)
    }
    // v9: the restatement review of the run's new Topic Cards, one review per source book (zt-digest restate:
    // prepare freezes the evidence and one judge job, judge validates the answer, apply writes each accepted
    // rewrite of a new Card through the run's own operation prefix, report renders the statuses).
    const runId = `grow-${(W.split(/[\\/]/).pop() || 'run').replace(/[^A-Za-z0-9_-]/g, '-')}`
    const restateRound = async () => {
      const listed = await sh(`-Step restate-cards -Work ${q(W)}`, 'restate cards')
      const reviews = (listed && listed.reviews) || []
      const results = []
      for (const r of reviews) {
        const st = String(r.staging)
        const work = `${st}\\work`
        const step = (name, more = '') => `-Step ${name} -Staging ${q(st)} -Work ${q(work)}${more}`
        let prepared = null
        try { prepared = await zd('restate', step('prepare', ` -Cards ${q(`${st}\\cards.txt`)}`), `restate ${r.id} prepare`) } catch (error) {
          if (error && error.lock_refusal) throw error
          results.push({ id: r.id, book: r.book, status: 'skipped', reason: String((error && error.message) || error).slice(0, 400) })
          continue
        }
        const rj = ((prepared && prepared.jobs) || [])[0]
        if (rj) {
          started = true
          const rid = `${r.id}-rj`
          const ds = await dsDispatch([{
            id: rid,
            run: { cwd: W, prompt_files: rj.files, prompt: rj.prompt, model: p.model, reasoning_effort: p.effort === 'max' ? 'max' : 'high', skills: p.skills && p.skills.length ? p.skills : ['zettelkasten', 'asd-ste100-zh'] },
            expect: '\\{', // the restatement prompt asks for one JSON line as the answer
            approve: `allow only these: the restatement self-check "${exe} restate -Step judge" with -Staging "${st}" (it validates the answer and writes only under "${st}"); a read-only read or search (Read, Get-Content, Select-String) of a file under "${W}"; and writing JSON files under "${work}\\out". Reject anything else that writes, moves, or deletes files, and every other zt-digest step. Answer a question with custom text "按任务说明自行决定，不需要询问。"`,
          }], { label: `${ctx.stage} restate ${r.id}`, phase: ctx.phase, keepWorkspaces: true })
          stopOnLock(`${ctx.stage} restate ${r.id}`, ds.results)
        }
        const answer = rj && rj.answer ? rj.answer : `${work}\\out\\rj-01.json`
        const soft = async (args, what) => { try { return await zd('restate', args, what) } catch (error) { if (error && error.lock_refusal) throw error; return null } }
        const judged = rj ? await soft(step('judge', ` -File ${q(answer)}`), `restate ${r.id} judge`) : null
        const applied = judged ? await soft(step('apply', ` -OperationPrefix ${q(`${runId}/restate-${r.id}/1/apply`)}`), `restate ${r.id} apply`) : null
        const reported = await soft(step('report'), `restate ${r.id} report`)
        const counts = (reported && reported.counts) || {}
        results.push({ id: r.id, book: r.book, cards: r.cards, status: reported ? reported.status : 'failed', report_id: reported && reported.report_id, resolved_by_rewrite: counts.resolved_by_rewrite || 0, pending: counts.pending_existing_card_change || 0, unresolved: counts.unresolved || 0, written: applied ? applied.written || 0 : 0 })
        log(`Restatement review ${r.id} (${r.book}, ${r.cards} new Cards): ${reported ? reported.status : 'failed'}; ${counts.resolved_by_rewrite || 0} resolved by rewrite, ${counts.pending_existing_card_change || 0} pending, ${counts.unresolved || 0} unresolved.`)
      }
      out.restate = { books: reviews.length, reviews: results }
    }
    // v10: the Topic maintenance round (the old Organize's Topic merges and Card moves). The tool surveys the
    // run's Topics; one DeepSeek job decides as JSON (its SELF-CHECK is the grow precheck: O1-O8); the tool
    // checks the answer again, carries out the valid decisions through organize-apply -Open and moves the run's
    // own addresses with them. A Locked destination stops the stage with the typed refusal (v7).
    const organizeRound = async () => {
      const trees = p.organizeTrees ? ` -Trees ${q(p.organizeTrees)}` : ''
      const op = await sh(`-Step organize-plan -Work ${q(W)}${trees}`, 'organize plan')
      out.organize = { plan: op, applied: null }
      log(`Topic maintenance: ${op && op.topics} Topics surveyed; ${op && op.pairs || 0} duplicate candidates, ${op && op.small || 0} small Topics, ${op && op.moves || 0} move candidates.`)
      if (!(Number(op && op.organize_jobs) > 0)) return
      started = true
      const d = await dsDispatch([job('go-001', [`${W}\\jobs\\go-001.md`])], { label: `${ctx.stage} organize`, phase: ctx.phase, keepWorkspaces: true })
      stopOnLock(`${ctx.stage} organize`, d.results)
      const oa = await sh(`-Step organize-apply -Work ${q(W)}`, 'organize apply')
      out.organize.applied = oa
      out.rounds.push({ round: 'organize', jobs: 1, ok: d.results.filter(r => r && r.ok).length, merges: oa && oa.merges, moves: oa && oa.moves })
      log(`Topic maintenance: ${oa && oa.merges || 0} Topic merges, ${oa && oa.moves || 0} Card moves, ${oa && oa.refused || 0} refused, ${oa && oa.blocked || 0} blocked, ${oa && oa.invalid || 0} decisions failed the check.`)
    }
    // v10: the concept review of each source book's term table against the Topics, one review per book (the
    // digests' former concepts stage): concept-books stages the folders; per book zt-digest concepts propose ->
    // prepare -> one DeepSeek judge job -> judge -> report. The rendered reports are the register plan's
    // read-only evidence. A book whose review fails is reported for that book and the run goes on.
    const conceptRound = async (which) => {
      const listed = await sh(`-Step concept-books -Work ${q(W)}${which || ''}`, 'concept books')
      const reviews = (listed && listed.reviews) || []
      const results = []
      const soft = async (args, what) => { try { return await zd('concepts', args, what) } catch (error) { if (error && error.lock_refusal) throw error; return null } }
      for (const r of reviews) {
        const st = String(r.staging)
        const work = `${st}\\work`
        const step = (name, more = '') => `-Step ${name} -Staging ${q(st)} -Work ${q(work)}${more}`
        const proposed = await soft(step('propose'), `concepts ${r.id} propose`)
        if (!proposed || !Number(proposed.proposals)) {
          results.push({ id: r.id, book: r.book, status: proposed ? 'skipped' : 'failed', reason: proposed ? proposed.reason : 'propose failed' })
          continue
        }
        const prepared = await soft(step('prepare'), `concepts ${r.id} prepare`)
        const cj = ((prepared && prepared.jobs) || [])[0]
        if (!prepared) { results.push({ id: r.id, book: r.book, status: 'failed', reason: 'prepare failed' }); continue }
        if (cj) {
          started = true
          const ds = await dsDispatch([{
            id: `${r.id}-cj`,
            run: { cwd: W, prompt_files: cj.files, prompt: cj.prompt, model: p.model, reasoning_effort: p.effort === 'max' ? 'max' : 'high', skills: ['asd-ste100-zh'] },
            expect: '\\{', // the concept prompt asks for one JSON line as the answer
            approve: `allow only these: the concept self-check "${exe} concepts -Step judge" with -Staging "${st}" (it validates the answer and writes only under "${st}"); a read-only read or search (Read, Get-Content, Select-String) of a file under "${W}"; and writing JSON files under "${work}\\out". Reject anything else that writes, moves, or deletes files, and every other zt-digest step. Answer a question with custom text "按任务说明自行决定，不需要询问。"`,
          }], { label: `${ctx.stage} concepts ${r.id}`, phase: ctx.phase, keepWorkspaces: true })
          stopOnLock(`${ctx.stage} concepts ${r.id}`, ds.results)
        }
        const judged = cj ? await soft(step('judge', ` -File ${q(cj.answer || `${work}\\out\\cj-01.json`)}`), `concepts ${r.id} judge`) : null
        const reported = await soft(step('report'), `concepts ${r.id} report`)
        results.push({ id: r.id, book: r.book, status: reported ? reported.status : 'failed', report_id: reported && reported.report_id, findings: reported ? reported.findings || 0 : 0, judged: judged ? judged.judged || 0 : 0, unjudged: reported ? reported.unjudged || 0 : 0 })
        log(`Concept review ${r.id} (${r.book}): ${reported ? `${reported.status}; ${reported.findings || 0} findings, ${reported.judged || 0} judged, ${reported.unjudged || 0} unjudged` : 'failed'}.`)
      }
      out.concepts = { books: reviews.length, skipped: (listed && listed.skipped) || [], reviews: results }
      return results.some(r => r.report_id)
    }
    const range = `${p.from != null ? ` -From ${Number(p.from)}` : ''}${p.to != null ? ` -To ${Number(p.to)}` : ''}`
    // v4: the box is the unit. With no book and no books, every Literature tree of the box is read: the
    // more Literature trees the box holds, the more each section's ideas can relate to.
    const books = Array.isArray(p.books) ? p.books.map(String).filter(Boolean) : []
    const which = books.length ? ` -Books ${q(books.join(','))}` : p.book ? ` -Book ${q(p.book)}` : ''
    let started = false
    if (out.mode === 'hubs') {
      // Mode hubs: the hubs round and the register over the whole agent Workspace, no new Card (the digests'
      // former mode hubs). Only on the live box: the hub jobs' helper lines name no zt config.
      if (p.ztConfig) throw new Error('mode hubs runs on the live box only (the hub jobs name no zt config)')
      try {
        const reviewed = p.concepts !== false && p.register !== 'off' ? await conceptRound(which) : false
        await hubsRound(reviewed)
        out.report = { mode: 'hubs', hubs: out.hubs && out.hubs.hubs, faults: out.hubs && out.hubs.faults, missing: out.hubs && out.hubs.missing, register: out.hubs && out.hubs.register }
      } finally {
        if (started) out.workspaces = await dsCleanupWorkspaces([W], { ownWorkspaces: true, label: `${ctx.stage} cleanup`, phase: ctx.phase })
      }
      return out
    }
    const plan = await sh(`-Step plan${which} -Work ${q(W)} -Instructions ${q(p.instructions)}${tool !== 'zt-digest' ? ` -Tool ${q(tool)}` : ''}${p.ztConfig ? ` -ZtConfig ${q(p.ztConfig)}` : ''}${range}`, 'plan')
    out.plan = plan
    const pending = Math.max(0, Number(plan && plan.pending) || 0)
    log(`Grow: ${pending} sections of ${plan && plan.books} Literature trees to read${plan && plan.left ? ` (${plan.left} more for a rerun)` : ''} (${plan && plan.cards} Cards; box ${plan && plan.box} Cards, ${plan && plan.topics} Topics); Topic share before ${plan && plan.baseline ? plan.baseline.share : '?'}.`)
    try {
      // The run's Topics first (gt-001, one job over every book), then the section jobs that file under them.
      let n = 0
      if (pending) {
        if (Number(plan.topic_jobs) > 0) {
          started = true
          const ds = await dsDispatch([job('gt-001', [`${W}\\instructions.zh.md`, `${W}\\jobs\\gt-001.md`])], { label: `${ctx.stage} topics`, phase: ctx.phase, keepWorkspaces: true })
          out.rounds.push({ round: 'topics', jobs: 1, ok: ds.results.filter(r => r && r.ok).length })
        }
        const topics = await sh(`-Step topics -Work ${q(W)} -Apply`, 'topics')
        out.topics = topics
        n = Math.max(0, Number(topics && topics.jobs) || 0)
        log(`Grow: ${topics && topics.topics} run Topics (${topics && topics.created} new, ${topics && topics.refused} refused); ${n} section jobs.`)
      }
      if (n) {
        started = true
        const ds = await dsDispatch(ids('gw', n).map(id => job(id, [`${W}\\instructions.zh.md`, `${W}\\jobs\\${id}.md`])), { label: `${ctx.stage} ideas`, phase: ctx.phase, keepWorkspaces: true })
        stopOnLock(`${ctx.stage} ideas`, ds.results)
        const applied = await sh(`-Step apply -Work ${q(W)} -Apply`, 'apply')
        out.rounds.push({ round: 'ideas', jobs: n, ok: ds.results.filter(r => r && r.ok).length, failed: ds.results.filter(r => !r || !r.ok).map(r => r && r.id), applied })
        log(`Grow: ${applied.created} Topic Cards, ${applied.topics} new Topics, ${applied.same} same, ${applied.refused} refused, ${applied.duplicates} duplicates, ${applied.unanswered} sections unanswered.`)
        // The link round (v3): one job per Topic tree the run wrote in, Links to Cards of other trees.
        const planned = await sh(`-Step links -Work ${q(W)}`, 'links')
        const m = Math.max(0, Number(planned && planned.link_jobs) || 0)
        if (m) {
          const dl = await dsDispatch(ids('gl', m).map(id => job(id, [`${W}\\instructions.zh.md`, `${W}\\jobs\\${id}.md`])), { label: `${ctx.stage} links`, phase: ctx.phase, keepWorkspaces: true })
          stopOnLock(`${ctx.stage} links`, dl.results)
          const linked = await sh(`-Step apply -Work ${q(W)} -Apply`, 'apply links')
          out.rounds.push({ round: 'links', jobs: m, ok: dl.results.filter(r => r && r.ok).length, linked: linked.linked })
          log(`Grow: ${linked.linked} Links across trees from ${m} link jobs.`)
        }
      }
      for (let round = 1; round <= (p.fixRounds ?? 1); round++) {
        const checked = await sh(`-Step check -Work ${q(W)} -Round ${round}`, `check ${round}`)
        const m = Math.max(0, Number(checked && checked.fix) || 0)
        if (!m) { out.rounds.push({ round: `check${round}`, fix: 0, faults: checked && checked.faults }); break }
        started = true
        const ds = await dsDispatch(ids(`fx${round}`, m).map(id => job(id, [`${W}\\jobs\\${id}.md`])), { label: `${ctx.stage} fix${round}`, phase: ctx.phase, keepWorkspaces: true })
        stopOnLock(`${ctx.stage} fix${round}`, ds.results)
        const applied = await sh(`-Step apply -Work ${q(W)} -Apply`, `apply fix${round}`)
        out.rounds.push({ round: `fix${round}`, jobs: m, faults: checked.faults, ok: ds.results.filter(r => r && r.ok).length, edited: applied.edited })
      }
      // The restatement review (v9): each new Topic Card is compared with its book's anchors and models and
      // with the earlier Topic Idea Cards; an accepted 'same' rewrites only the new Card.
      if (p.restate !== false && !p.ztConfig) await restateRound()
      // The Topic maintenance round (v10, optional): merges and moves on the run's Topics, before the hubs
      // map them. Only on the live box, like the rounds after it.
      if (p.organize === true && !p.ztConfig) await organizeRound()
      // The hubs round (v8): the grown Topics get their ways in. The hub compiler writes topic, line and
      // field hubs (DeepSeek writes each hub as JSON, gh-NNN); then the register is rebuilt from the box's
      // term tables so its terms point at the Cards that carry them. Only on the live box: the hub jobs'
      // helper lines name no zt config. v10: first the concept review of each source book, whose reports
      // the register plan reads.
      if (p.hubs !== false && !p.ztConfig) {
        const reviewed = p.concepts !== false && p.register !== 'off' ? await conceptRound('') : false
        await hubsRound(reviewed)
      }
      out.report = await sh(`-Step report -Work ${q(W)}`, 'report')
      log(`Grow: report ${out.report && out.report.report}: ${out.report && out.report.created} Cards, Topic share ${out.report && out.report.share_before} → ${out.report && out.report.share_after}, ${out.report && out.report.faults_left} Cards with faults left.`)
    } finally {
      if (started) out.workspaces = await dsCleanupWorkspaces([W], { ownWorkspaces: true, label: `${ctx.stage} cleanup`, phase: ctx.phase })
    }
    return out
  },
})
// ==== end module zt-grow-cycle ====
