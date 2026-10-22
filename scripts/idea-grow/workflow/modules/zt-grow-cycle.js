// ==== module zt-grow-cycle v7 ====
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
defineModule({
  id: 'zt-grow-cycle', version: '7', requires: ['script', 'deepseek', '@dsmp'],
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
    const sh = async (args, what) => {
      let out
      try {
        out = await wfpScript(ctx, { command: `${env}& ${q(tool)} grow ${args}`, shell: 'powershell', role: p.runner || 'runner', label: `${ctx.stage}: ${what}` })
      } catch (error) {
        const lock = lockRefusal(error && error.message)
        if (lock) throw lockStop(`${ctx.stage}: ${what}`, lock)
        throw error
      }
      const lock = out && typeof out === 'object' && (out.code === LOCK || out.error === LOCK) ? out : null
      if (lock) throw lockStop(`${ctx.stage}: ${what}`, lock)
      return out
    }
    const approve = `allow only these: a command that runs the zt helper "${exe} call" with -Allow "${W}\\read-only.allow.txt" (an empty allow-list: reads only)${p.ztConfig ? `, which may first set $env:ZT_CONFIG_DIR='${p.ztConfig}'` : ''}; the self-check "${exe} grow -Step precheck" (read-only); a read-only read or search (Read, Get-Content, Select-String) of a file under "${W}"; and writing JSON files under "${W}\\out". Reject anything else that writes, moves, or deletes files, and every other zt-digest step. Answer a question with custom text "按任务说明自行决定，不需要询问。"`
    const job = (id, files) => ({
      id,
      run: {
        cwd: W,
        prompt_files: files,
        prompt: `按上面的任务说明和任务文件完成任务 ${id}。答案只写到任务文件给出的答案文件。\n\nSELF-CHECK: ${exe} grow -Step precheck -Work "${W}" -Job ${id}\n回复前运行 SELF-CHECK 命令。改正它报告的每个问题，再运行一次，直到它输出 "faults: 0"（最多三次）。最后一行回复 "OK" 或 "FLAG: <剩下的问题>"。`,
        model: p.model,
        reasoning_effort: p.effort === 'max' ? 'max' : 'high',
        skills: p.skills && p.skills.length ? p.skills : ['zettelkasten', 'asd-ste100-zh'],
      },
      expect: '(^|\\n)(OK|FLAG:)',
      approve,
    })
    const ids = (prefix, n) => Array.from({ length: n }, (_, i) => `${prefix}-${String(i + 1).padStart(3, '0')}`)
    const out = { plan: null, topics: null, rounds: [], report: null, workspaces: [] }
    const range = `${p.from != null ? ` -From ${Number(p.from)}` : ''}${p.to != null ? ` -To ${Number(p.to)}` : ''}`
    // v4: the box is the unit. With no book and no books, every Literature tree of the box is read: the
    // more Literature trees the box holds, the more each section's ideas can relate to.
    const books = Array.isArray(p.books) ? p.books.map(String).filter(Boolean) : []
    const which = books.length ? ` -Books ${q(books.join(','))}` : p.book ? ` -Book ${q(p.book)}` : ''
    const plan = await sh(`-Step plan${which} -Work ${q(W)} -Instructions ${q(p.instructions)}${tool !== 'zt-digest' ? ` -Tool ${q(tool)}` : ''}${p.ztConfig ? ` -ZtConfig ${q(p.ztConfig)}` : ''}${range}`, 'plan')
    out.plan = plan
    const pending = Math.max(0, Number(plan && plan.pending) || 0)
    log(`Grow: ${pending} sections of ${plan && plan.books} Literature trees to read${plan && plan.left ? ` (${plan.left} more for a rerun)` : ''} (${plan && plan.cards} Cards; box ${plan && plan.box} Cards, ${plan && plan.topics} Topics); Topic share before ${plan && plan.baseline ? plan.baseline.share : '?'}.`)
    let started = false
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
      out.report = await sh(`-Step report -Work ${q(W)}`, 'report')
      log(`Grow: report ${out.report && out.report.report}: ${out.report && out.report.created} Cards, Topic share ${out.report && out.report.share_before} → ${out.report && out.report.share_after}, ${out.report && out.report.faults_left} Cards with faults left.`)
    } finally {
      if (started) out.workspaces = await dsCleanupWorkspaces([W], { ownWorkspaces: true, label: `${ctx.stage} cleanup`, phase: ctx.phase })
    }
    return out
  },
})
// ==== end module zt-grow-cycle ====
