// ==== module zt-digest-prepare v1 ====
// Prepare (mode full): prepare-book-digest.ps1 -Apply writes the Chinese text, the section groups, the
// Literature tree with one anchor per group, and the shared prefixes; the Haiku agent also reads the DeepSeek
// usage ledger. Other modes take the group count from args.groups. A mode in needGroups fails without groups.
defineModule({
  id: 'zt-digest-prepare', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the book (mode full) or take args.groups; output {groups, usage_before}.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    needGroups: { type: 'array', default: ['full', 'link'], description: 'modes that need a group count' },
    instructions: { type: 'any', description: 'the digest instruction files (comma-separated, from consts.profile): the base prompt and the supplements of the book type' },
    noEventCandidates: { type: 'any', description: 'true: the group files leave out the event candidates (the event layer reads events.json)' },
  },
  run: async (ctx, p) => {
    const A = p.args
    const Z = zdMake(ctx, { args: A, mode: p.mode })
    let groups = A.groups || 0
    let usageBefore = null
    if (p.mode === 'full') {
      const prep = await Z.zAgent(
        `You prepare a book for a ZET digest. Do exactly these steps and nothing else.
1. Run this PowerShell command (one call) and read its last output line, a compact JSON manifest:
pwsh -NoProfile -File ${Z.SCRIPTS}\\prepare-book-digest.ps1 -Source "${A.source}" -Out "${Z.OUT}" -CitationKey ${Z.KEY} -BibTeX '${A.bibtex}' -BookTitle '${A.bookTitle}' -MaxChars ${A.maxChars || 20000} -MaxUnits ${A.maxUnits || 3}${A.startHeading ? ` -StartHeading '${A.startHeading}'` : ''}${A.endHeading ? ` -EndHeading '${A.endHeading}'` : ''}${A.skipHeadings ? ` -SkipHeadings '${A.skipHeadings}'` : ''}${A.fileId ? ` -FileId ${A.fileId}` : ''}${p.instructions ? ` -Instructions '${p.instructions}'` : ''}${p.noEventCandidates ? ' -NoEventCandidates' : ''} -Apply
If it fails, return ok=false with the error text; do not retry with other options and do not delete anything.
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
    return { groups, usage_before: usageBefore }
  },
})
// ==== end module zt-digest-prepare ====
