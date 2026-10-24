// ==== module zt-grow-cycle v12 ====
// The idea-grow compiler cycle (docs/idea-grow-design.md). v12 (zt-idea-grow 4.0.0, docs/workflow-simplicity-proposal.md
// S1-S4, S6, S7, S9) is one loop with three branches. `zt-digest grow -Step next` runs every deterministic step it can
// (plan, topics, apply, links, check, the restatement and concept reviews, the Topic maintenance, the hubs and the
// register, the report) in the order v11 ran them, and stops at the next DeepSeek job set or at the end. Its line is
//   {"next":"jobs",…}  -> the jobs go to DSMP dsDispatch, then `next -After <token>` runs again;
//   {"next":"done",…}  -> the stage ends with that line as its report;
//   {"next":"fault",…} -> the stage stops (a `card_locked` fault with the owner's refusal; S6).
// The command decides every branch; no agent decides control flow (S3). The relays are the zt-relay agent type (S1).
// A line that is not one JSON object, or a jobs line whose checksum does not match, is read again once from
// `grow -Step status` (S2); a second unreadable line stops the stage. Every DeepSeek job goes through dsDispatch
// (S4): the job template, its approval rule and its expected reply come from the command (`job`), and the ids fill
// `{id}`. Resume is a rerun with the same work folder and options (S7): `next` skips the rounds it recorded done.
// v11 history: the restatement and concept reviews prepared every book first and sent all the judge jobs in one
// dispatch; on a copy of the box the live-box rounds did not run. v12 runs them on a copy too: `next` writes the box
// pin (zt-box.txt) into the work folder, so every step and every DeepSeek job of the run reads and writes that copy.
defineModule({
  id: 'zt-grow-cycle', version: '12', requires: ['deepseek', '@dsmp', 'zt-workflow-lib'],
  summary: 'ZT idea grow: one loop over `zt-digest grow -Step next` (jobs -> DSMP dsDispatch, done, fault); the command plans, applies, checks, reviews, builds the hubs and the register and reports; a write the owner\'s Lock refused stops the stage with the typed card_locked fault.',
  params: {
    book: { type: 'string', description: 'one Literature tree to grow from (its Citation key)' },
    books: { type: 'array', description: 'the Literature trees to grow from; with neither book nor books, every Literature tree of the box' },
    work: { type: 'string', required: true, description: 'the work folder (also the DeepSeek workspace)' },
    instructions: { type: 'string', required: true, description: 'the prompt template dsh-grow-instructions.zh.md' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command or the path of a zt-digest binary' },
    ztConfig: { type: 'string', description: 'the ZT_CONFIG_DIR of a copy of the box with its own zt service (the run pins its work folder to it); empty: the live box' },
    from: { type: 'number', description: 'the first section number to grow' },
    to: { type: 'number', description: 'the last section number to grow' },
    model: { type: 'string', required: true, description: 'DeepSeek Harness model of the jobs (frozen: the official source)' },
    effort: { type: 'string', default: 'high', enum: ['high', 'max'], description: 'DeepSeek reasoning effort of every job, the hub jobs included (never below high)' },
    hubs: { type: 'boolean', default: true, description: 'run the hubs round (hub compiler and register) after the Cards are written' },
    register: { type: 'string', default: 'apply', enum: ['apply', 'off'], description: 'publish the rebuilt register (apply) or leave it (off)' },
    mode: { type: 'string', default: 'full', description: 'the run\'s mode ({{mode}}): full grows Topic Cards; hubs runs only the concept review, the hubs round and the register' },
    hubTrees: { type: 'string', description: 'only these Topic trees get hubs (comma-separated Topic numbers; a pilot)' },
    hubRebuild: { type: 'boolean', default: false, description: 'rewrite every hub, not only the hubs of trees that changed' },
    hubLimits: { type: 'any', description: 'hub limits that replace the survey\'s (an object; the hub compiler clamps them)' },
    restate: { type: 'boolean', default: true, description: 'run the restatement review of the run\'s new Topic Cards' },
    concepts: { type: 'boolean', default: true, description: 'the concept review of each source book\'s term table, read by the register plan as -Review (needs the hubs round and a register)' },
    organize: { type: 'boolean', default: false, description: 'the Topic maintenance round before the hubs: duplicate-Topic merges and misplaced-Card moves on the run\'s Topics' },
    relay: { type: 'string', default: 'relay', role: true, description: 'the declared role that relays the commands (the zt-relay agent type)' },
  },
  run: async (ctx, p) => {
    const q = (v) => `'${String(v).replace(/'/g, "''")}'`
    const W = String(p.work).replace(/[\\/]+$/, '')
    const tool = p.tool || 'zt-digest'
    // A copy of the box runs its own zt service: the first command names its config, and `next` pins the folder.
    const env = p.ztConfig ? `$env:ZT_CONFIG_DIR=${q(p.ztConfig)}; ` : ''
    const options = {
      mode: p.mode === 'hubs' ? 'hubs' : 'full', books: Array.isArray(p.books) ? p.books.map(String).filter(Boolean) : [], book: p.book || '',
      instructions: p.instructions, tool, zt_config: p.ztConfig || '', model: p.model, effort: p.effort === 'max' ? 'max' : 'high',
      restate: p.restate !== false, concepts: p.concepts !== false, organize: p.organize === true, hubs: p.hubs !== false, register: p.register === 'off' ? 'off' : 'apply',
      hub_trees: p.hubTrees || '', hub_rebuild: p.hubRebuild === true, hub_limits: p.hubLimits && typeof p.hubLimits === 'object' ? p.hubLimits : null,
      ...(p.from != null ? { from: Number(p.from) } : {}), ...(p.to != null ? { to: Number(p.to) } : {}),
    }
    const relay = (args, what) => ztRelay(ctx, {
      command: `${env}& ${q(tool)} grow ${args}`, label: `${ctx.stage}: ${what}`, phase: ctx.phase, role: p.relay || 'relay',
      run: `the idea-grow run the user started for "${W}"`, lockStops: false,
    })
    // The checksum `next` prints over a job set (FNV-1a over the Unicode scalar values; grow_next.rs `fnv`).
    const fnv = (text) => {
      let h = 0x811c9dc5
      for (const ch of String(text)) { h ^= ch.codePointAt(0); h = Math.imul(h, 0x01000193) >>> 0 }
      return h.toString(16).padStart(8, '0')
    }
    const jobSum = (line) => fnv([line.ids.join(','), line.job.cwd, line.job.files.join('\n'), line.job.prompt, line.job.approve, line.job.expect].join('\n'))
    const readable = (v) => {
      if (!v || typeof v !== 'object' || !['jobs', 'done', 'fault'].includes(v.next)) return null
      if (v.next !== 'jobs') return v
      const ok = Array.isArray(v.ids) && v.ids.length && v.job && Array.isArray(v.job.files) && typeof v.job.prompt === 'string' && typeof v.job.approve === 'string' && Number(v.token) > 0
      return ok && jobSum(v) === v.sum ? v : null
    }
    // S2: one recovery for an unreadable line: the status command prints the last line `next` printed.
    const read = async (args, what) => {
      const got = await relay(args, what)
      let line = readable(got.json)
      if (line) return line
      log(`Grow: the ${what} line came back unreadable (${String((got.run && got.run.output) || 'no output').slice(0, 160)}); it is read again from the status.`)
      const again = await relay(`-Step status -Work ${q(W)}`, `${what}: status`)
      line = readable(again.json)
      if (line) return line
      throw new Error(`grow: no readable line from ${what} or its status (${String((again.run && again.run.output) || 'no output').slice(0, 300)})`)
    }
    const fill = (text, id) => String(text).split('{id}').join(id)
    const jobOf = (id, t) => ({
      id,
      run: { cwd: t.cwd, prompt_files: t.files.map((f) => fill(f, id)), prompt: fill(t.prompt, id), model: t.model, reasoning_effort: t.effort === 'max' ? 'max' : 'high', skills: t.skills },
      expect: t.expect,
      approve: fill(t.approve, id),
    })
    const out = { mode: options.mode, rounds: [], report: null, workspaces: [] }
    const command = `-Step next -Work ${q(W)} -Options ${q(JSON.stringify(options))}`
    let started = false
    let after = null
    let same = 0
    try {
      for (let step = 1; step <= 400; step++) {
        const line = await read(`${command}${after != null ? ` -After ${after}` : ''}`, 'next')
        if (line.next === 'done') {
          out.report = line
          break
        }
        if (line.next === 'fault') {
          // S6: the one Lock check. The command stopped where it stood: the Cards it filed and the answers stay.
          if (line.code === ZT_LOCK) {
            log(`Grow: the owner's Lock refused a write at ${line.round}; the stage stops and keeps the Cards already filed.`)
            throw ztLockStop(`${ctx.stage}: ${line.round}`, line)
          }
          throw new Error(`grow stopped at ${line.round || 'next'}: ${line.code}: ${line.cause}`)
        }
        // The status repeated the job set this loop already dispatched: `next -After` runs again.
        if (after != null && Number(line.token) === Number(after)) {
          if (++same >= 3) throw new Error(`grow: next kept answering the dispatched job set ${after}`)
          continue
        }
        same = 0
        started = true
        log(`Grow: ${line.round}: ${line.ids.length} DeepSeek job(s).`)
        const d = await dsDispatch(line.ids.map((id) => jobOf(id, line.job)), { label: `${ctx.stage} ${line.round}`, phase: ctx.phase, keepWorkspaces: true })
        const failed = d.results.filter((r) => !r || !r.ok).map((r) => r && r.id)
        out.rounds.push({ round: line.round, jobs: line.ids.length, ok: line.ids.length - failed.length, failed })
        after = line.token
      }
      if (!out.report) throw new Error('grow: next did not end within 400 steps')
      const r = out.report
      log(`Grow: ${r.mode === 'hubs' ? 'hubs' : `report ${r.report}: ${r.created} Cards, Topic share ${r.share_before} → ${r.share_after}, ${r.faults_left} Cards with faults left`}${r.hubs ? `; hubs ${JSON.stringify(r.hubs)}, register ${r.register}` : ''}.`)
    } finally {
      if (started) out.workspaces = await dsCleanupWorkspaces([W], { ownWorkspaces: true, label: `${ctx.stage} cleanup`, phase: ctx.phase })
    }
    return out
  },
})
// ==== end module zt-grow-cycle ====
