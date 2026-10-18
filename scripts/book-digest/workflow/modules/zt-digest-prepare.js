// ==== module zt-digest-prepare v5 ====
// Prepare (mode full): prepare-book-digest.ps1 -Apply writes the Chinese text, the section groups, the
// Literature tree with one anchor per group, and the shared prefixes, then binds the immutable run contract with
// the effective settings (maxAttempts, dshRunCap) and initializes the run ledger; the Haiku agent also reads the
// DeepSeek usage ledger. args.book (a Scholar Archive Work id or exact title) replaces the explicit source,
// BibTeX and title: the tool resolves them through the read-only bridge, and an unresolved choice stops with its
// candidates. A resume (mode resume) prepares nothing: it reads the retained group enumeration from the ledger,
// so the run reuses the frozen inputs, settings and identity. Other modes take the group count from args.groups.
// The output also carries groupList, the per-group items the digest stage's chains are built from.
defineModule({
  id: 'zt-digest-prepare', version: '5', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the book (mode full, from args.book or the explicit source), read the retained groups (mode resume) or take args.groups; output {groups, groupList, usage_before}.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    needGroups: { type: 'array', default: ['full', 'link'], description: 'modes that need a group count' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile): the prepared-run variant' },
    instructions: { type: 'any', description: 'the digest instruction files (comma-separated, from consts.profile): the base prompt and the supplements of the book type' },
    noEventCandidates: { type: 'any', description: 'true: the group files leave out the event candidates (the event layer reads events.json)' },
  },
  run: async (ctx, p) => {
    const A = p.args
    const Z = zdMake(ctx, { args: A, mode: p.mode })
    const groupList = (count) => Array.from({ length: count || 0 }, (_, i) => ({ n: i + 1, nn: String(i + 1).padStart(2, '0') }))
    let groups = A.groups || 0
    let usageBefore = null
    if (p.mode === 'resume') {
      // A resume reuses the retained preparation, its settings and its identity; it never regenerates files.
      const status = await Z.runScript('resume status', 'Prepare', `${Z.LEDGER} status -Out "${Z.OUT}"`, ' (the retained run ledger)')
      const view = status.json
      if (!view || !Array.isArray(view.units)) throw new Error(`mode resume needs a prepared run ledger with its frozen inputs: ${(status.run && status.run.output) || 'no ledger status'}`)
      const digestUnits = view.units.filter(u => u.layer === 'digest')
      if (!digestUnits.length) throw new Error('mode resume found no retained digest groups; prepare the book again')
      groups = digestUnits.length
      const settings = view.settings || {}
      log(`Resume: ${groups} retained groups, variant ${view.variant}, ${view.archive_identity && view.archive_identity.archive_id ? 'the same archive identity' : 'a retained archive identity'}; settings maxAttempts ${settings.max_attempts}, dshRunCap ${settings.dsh_run_cap} s.`)
      return { groups, groupList: groupList(groups), usage_before: null, resumed: true, settings }
    }
    if (p.mode === 'full') {
      // args.book names the book in the Scholar Archive; the tool resolves its Markdown, BibTeX and
      // title, so the explicit arguments are left out rather than passed empty.
      const book = A.book ? `-Book '${A.book}'`
        : `-Source "${A.source}" -BibTeX '${A.bibtex}' -BookTitle '${A.bookTitle}'`
      // The layer switches the run declares are frozen into the prepared target: a layer this run
      // deliberately excludes is excluded in the ledger too, so no unit waits for work that will not come.
      const switches = {}
      for (const name of ['terms', 'models', 'events', 'findings', 'hubs', 'figures', 'bridge', 'notation', 'boundary', 'proof', 'theory']) {
        if (typeof A[name] === 'boolean') switches[name] = A[name]
      }
      const runOptions = Object.keys(switches).length ? ` -RunOptions '${JSON.stringify(switches)}'` : ''
      const prep = await Z.zAgent(
        `You prepare a book for a ZET digest. Do exactly these steps and nothing else.
1. Run this PowerShell command (one call) in the foreground with timeout_ms: 600000 and read its last output line, a compact JSON manifest:
${Z.tool('prepare-book-digest')} ${book} -Out "${Z.OUT}" -CitationKey ${Z.KEY} -MaxChars ${A.maxChars || 20000} -MaxUnits ${A.maxUnits || 3}${A.startHeading ? ` -StartHeading '${A.startHeading}'` : ''}${A.endHeading ? ` -EndHeading '${A.endHeading}'` : ''}${A.skipHeadings ? ` -SkipHeadings '${A.skipHeadings}'` : ''}${A.fileId ? ` -FileId ${A.fileId}` : ''}${p.instructions ? ` -Instructions '${p.instructions}'` : ''}${p.noEventCandidates ? ' -NoEventCandidates' : ''} -Variant ${p.profile && p.profile.book === 'mathematics' ? 'math' : 'econ'} -MaxAttempts ${Z.MAX_ATTEMPTS} -DshRunCap ${Z.DSH_RUN_CAP}${runOptions} -Apply
If it fails, return ok=false with the error text; do not retry with other options and do not delete anything. Exit code 3 with needs_selection means the last line names the Works or files to choose from: put that line in error, and the run halts for the choice; pass one Work id as args.book (or one file id as args.fileId) and run again.
2. ${ZD_USAGE_STEP}
Return groups = the number of entries in manifest.groups and usage_before = the summed usage.`,
        {
          label: 'prepare book', phase: 'Prepare', model: 'haiku',
          schema: {
            type: 'object',
            properties: { ok: { type: 'boolean' }, error: { type: 'string' }, groups: { type: 'number' }, usage_before: ZD_USAGE },
            required: ['ok', 'groups'],
          },
        },
      )
      if (!prep || !prep.ok || !prep.groups) {
        log(`Preparation failed: ${prep && prep.error}`)
        throw new Error(`preparation failed: ${(prep && prep.error) || 'no report from the prepare agent'}`)
      }
      groups = prep.groups
      usageBefore = prep.usage_before || null
    }
    if (!groups && (p.needGroups || ['full', 'link']).includes(p.mode)) throw new Error(`mode "${p.mode}" needs args.groups`)
    return { groups, groupList: groupList(groups), usage_before: usageBefore }
  },
})
// ==== end module zt-digest-prepare ====
