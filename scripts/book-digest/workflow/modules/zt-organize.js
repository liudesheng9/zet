// ==== module zt-organize v1 ====
// Organize: organize-survey.ps1 lists what to decide; Opus writes only decisions.json (thesis, Topic merges,
// moves, Card notes); organize-apply.ps1 carries out the merges and moves; a card cycle hands the notes to
// DeepSeek; book-skeleton.ps1 and one DeepSeek job write the Literature Card index, the structure Card, and the
// Topic bodies. args.organize "legacy" runs the old single Opus job instead.
defineModule({
  id: 'zt-organize', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: Organize (survey → Opus decisions → apply → notes card cycle → skeleton), or the legacy Opus job.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    groups: { type: 'number', description: 'the group count from the prepare stage' },
    digest: { type: 'any', description: 'the digest group reports (mode full)' },
    link: { type: 'any', description: 'the link group reports (other modes)' },
    models: { type: 'any', description: 'the model reports of the digest stage (mode full)' },
    modelsBook: { type: 'any', description: 'the model reports of the models stage (mode models)' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const A = Z.A, KEY = Z.KEY, OUT = Z.OUT, SCRIPTS = Z.SCRIPTS, MODE = Z.MODE
    const groupCount = p.groups || 0
    const modelReports = p.models || p.modelsBook || []
    const modelLines = modelReports.filter(r => r.status === 'done').map(r => `- [[${r.model}]]${r.level === 'concept' ? '（概念模型）' : r.level === 'formal' ? '（形式模型）' : ''}：${(r.parts || []).map(pp => `${pp.facet}${pp.emphasis === 'core' ? '（核心）' : ''} [[${pp.address}]]`).join('、')}${(r.issues || []).length ? `；未解决：${r.issues.join('；')}` : ''}${r.notes && r.notes.length ? `；建议：${r.notes.join('；')}` : ''}`).join('\n')
    const groupLines = ((MODE === 'full' ? p.digest : p.link) || []).map(r => `- 第 ${r.group} 组 [[${KEY}/${r.group}]]：${r.status}${r.cards ? `，${r.cards} 张` : ''}${r.models ? `，模型卡 ${r.models}` : ''}${r.events ? `，事件卡 ${r.events}` : ''}${r.topics && r.topics.length ? `；新主题：${r.topics.join('、')}` : ''}${r.errors && r.errors !== '无' ? `；问题：${r.errors}` : ''}`).join('\n')
    const ORG = `${OUT}\\organize`
    const HELPER = ZD_HELPER
    if (A.organize === 'legacy') {
      const report = await Z.zAgent(
        `You are the final coordinator of a ZET book digest. DeepSeek agents have digested 《${A.bookTitle}》 group by group into the agent Workspace of the ZET slip-box, then added reasoned cross-part Links to their own Cards. Your job is to organize the whole card structure. Use the zettelkasten and zt skills (load them with the Skill tool first). Make every zt call through the helper script, not the mcp__zt__* tools (the conversation's MCP server may be an older zt): write a JSON array of calls and run it in one PowerShell command, e.g.
  @'
  [{"tool":"list_trees","arguments":{"workspace":"agent"}},{"tool":"search_cards","arguments":{"type":"model","workspace":"agent"}}]
  '@ | pwsh -NoProfile -File "${HELPER}" -
  Each output line is "N <json>", "N ok <address>", or "N error <reason>" (then a resume line); "@@N" in an argument is the address returned by call N. Batch many reads or writes per command. delete_card and move_card return a plan first; call again with the confirm text. Write all Card text in Chinese. Change only the agent Workspace; human Cards are read-only but you may link to them.

  Book: Literature Card [[${KEY}]] (agent Workspace, Literature Zone). Its Direct-successor line [[${KEY}/1]] … [[${KEY}/${groupCount}]] holds one anchor Card per group, in book order; each anchor holds that part's central claim, a section index, and its place in the whole book.
  Group reports:
  ${groupLines}
  ${modelLines ? `Structured models (a Sonnet architect planned each model's parts, facets, core parts, and kinded relations, and DeepSeek wrote them with charts; act on the architect's notes for Cards outside the model and for other Model Cards that should link a part to share it):\n${modelLines}\n` : ''}
  Work in this order, reading cheaply (list_trees, get_tree excerpts; get_card only when you need a body):
  1. Survey every agent tree (list_trees with workspace "agent", then get_tree each).
  2. Merge duplicate or overlapping Topics: keep the clearer Topic, move_card the other's first Card (with its subtree) to the best free slot in the kept tree (plan first, then confirm with "move"), then delete the emptied Topic root (plan, then confirm with its address). Fix clearly misplaced Cards the same way. Never delete a Card that still has content.
  3. Rewrite every agent Topic body: 2–3 sentences on the question it pursues, then Links to its 1–3 best entry Cards. Keep a line that links a Hub Card.
  4. Do not write, rewrite, or retype Hub Cards (type hub), and do not write an Idea Card that lists a subject's Cards: the hub stage runs after you and builds every topic, line, and field hub and the register from the trees you leave. When you merge two Topics, leave their Hub Cards where they are; the hub stage merges them.
  5. Write the book's structure Card as a Side successor of [[${KEY}/1]]: title = the book's central thesis as one sentence; body = the argument in reading order, one line per part linking its anchor and its 1–2 key Topic Cards, each line saying what it contributes and how it follows from the previous part; then a paragraph listing the book's Model Cards (search_cards with type "model") in order, one line each on what the model shows, its level (概念模型 or 形式模型), and which later models build on it, and a paragraph listing its Event Cards (type "event") with the ideas each supports.
  5a. Read each model's build report: run get_model on every Model Card and read \`level\` and \`build\`. Each Model Card has an elaboration level: \`concept\` (level 1: the Model Card and at most four parts in words, no Derivation parts) or \`formal\` (level 2: a self-sufficient model). A formal model is self-sufficient when \`build.closure\`, \`build.forward\`, and \`build.unanchored\` are empty; a concept model also needs an empty \`build.beyond_level\`; tolerated premises of shared parts are not faults. Do not change a model's level; report a level that looks wrong. Re-point each outside Card that a model manager named in its notes (a Card whose content a new part of the model now states): that Card links the new part, with its kind word. Check that each model with shared parts takes them in through its Model Card's 结构 list. Do not insert or move Cards inside a model's range.
  5b. Check Model and Event Cards: each model the book sets out has exactly one Model Card (merge duplicates; set type "model" on a Card that states a whole model but is typed idea); run get_model on every Model Card and fix each reported issue (a part that belongs to no model: link it from its Model Card's 结构 list), but keep the structure the architect planned (its facets, core and aside parts, kinded relations, and loose parts). A part belongs to the Model Card it is filed under and to every Model Card that links it, so where two models of the book share a part (a correspondence, a common setting), link it from the second Model Card's 结构 list rather than writing a duplicate. Every Card that derives, interprets, applies, extends, or criticises a model links to its Model Card, or better to the Model part it concerns (a critique of an assumption links the part that states it), and the word right before the Link names the kind of relation, which ZT reads: 依赖/基于 (rests on), 例示/案例 (illustrates), 对照/区别 (contrasts), 放宽/推广 (relaxes), 对应/类比 (corresponds), 反驳/批评 (objects). Read each model's neighbours in get_model, rewrite those that read "refers" but state a real relation so the kind word stands right before the Link, then search for discussions that miss the model; every Card that uses a real-world event as evidence links to its Event Card.
  6. Rewrite the Literature Card [[${KEY}]] body (edit_card body only) as the book's index: one line per chapter, "第N章 章名 - 讲了什么 → [[anchor]]", then a line linking the structure Card. Keep its last line "原书文档：…" (Document links such as [原书 PDF](scholar:85) that open the book's PDF and Markdown) exactly as it is.
  6a. A line \`![图 2.C.1](artifact:<sha256>)\` in a Card shows the book's own figure; when you rewrite or merge a Card, keep each such line exactly, next to the text that discusses the figure.
  7. Check the cross-part Links the DeepSeek agents added: keep those whose sentence states a real relationship and reason; rewrite vague ones. Add Links still missing between Topics that share a mechanism, and to related human Cards (search_cards with workspace "human"). Remove a "# 标题" line that repeats the title at the top of a body.
  8. Check list_broken_links and fix any in the agent Workspace.
  Return a short Chinese report: the final agent Topics (address and name), merges and moves done, the structure Card address, link counts added or rewritten, and anything left for the user.`,
        { label: 'organize card structure', phase: 'Organize', model: 'opus' },
      )
      return report || 'no report from the Opus coordinator'
    }
    // 1. The survey: a script lists what Opus must decide.
    const surveyed = await Z.runScript('organize: survey', 'Organize', `pwsh -NoProfile -File "${SCRIPTS}\\organize-survey.ps1" -Staging "${OUT}" -Work "${ORG}"`)
    const survey = surveyed.json
    if (!survey) log(`Organize: the survey failed (${surveyed.run && surveyed.run.output}).`)
    // 2. Opus decides; it writes decisions.json and nothing else.
    const decided = survey ? await Z.zAgent(
      `You decide the structure of the ZET digest of 《${A.bookTitle}》 (Literature Card [[${KEY}]], anchors [[${KEY}/1]] … [[${KEY}/${groupCount}]]). Scripts and DeepSeek do every edit; you only decide. You make no zt write.
Read ${ORG}\\survey.md first (Read tool). It lists the book's Topics, the duplicate candidates, small Topics, Card merge proposals, the models with faults, broken Links, and missing hubs. When a decision needs a Card's text, read it through the helper (read-only calls only: get_card, get_tree, get_model, search_cards), e.g.
@'
[{"tool":"get_tree","arguments":{"tree":"62"}},{"tool":"get_card","arguments":{"at":"62/1"}}]
'@ | pwsh -NoProfile -File "${HELPER}" -
${modelLines ? `The architect's notes on the book's models (Cards outside a model that should link a part of it):\n${modelLines}\n` : ''}
Decide, then write ${ORG}\\decisions.json (UTF-8, Write tool) with exactly these fields:
- "thesis": the book's central thesis as one Chinese sentence of at most 40 characters. It becomes the title of the structure Card.
- "merges": Topic merges, [{"drop": "<n>/0", "keep": "<m>/0", "under": "<a Card of the kept Topic>", "reason": "…"}]. Merge only two agent Topics that pursue the same question. "under" is the Card under which the dropped Topic's Cards fit best (default: the kept Topic's first Card).
- "moves": Cards that sit in the wrong place, [{"at": "<Card>", "under": "<new parent>", "reason": "…"}]. Only Idea and Event Cards of Topic trees; never a Model Card, a Model part, a Hub Card, or a Literature tree Card.
- "notes": Card-level work for DeepSeek, [{"address": "<Card>", "note": "<one Chinese sentence: what to change>"}]: a Card that should link a model part (the architect's notes), a Card whose Link reads as a plain reference but states a real relation, a Card that should link an Event Card. Only Idea Cards, Event Cards, and anchors.
Use empty lists where nothing needs doing. Do not plan Hub Cards: the hub stage runs after you. Do not rewrite the Literature Card, the structure Card, or Topic bodies: a script and DeepSeek write them after your decisions.
Return thesis, merges, moves, notes (the counts), and report: a short Chinese report of what you decided and what you leave for the user.`,
      {
        label: 'organize: decisions', phase: 'Organize', model: 'opus',
        schema: { type: 'object', properties: { thesis: { type: 'string' }, merges: { type: 'number' }, moves: { type: 'number' }, notes: { type: 'number' }, report: { type: 'string' } }, required: ['thesis', 'report'] },
      },
    ) : null
    // 3. A script carries out the merges and moves.
    const applied = decided ? await Z.runScript('organize: apply', 'Organize', `pwsh -NoProfile -File "${SCRIPTS}\\organize-apply.ps1" -Work "${ORG}"`) : null
    const moved = applied ? applied.json : null
    if (decided) log(`Organize: thesis "${decided.thesis}"; ${moved ? `${moved.merges} merges, ${moved.moves} moves, ${moved.refused} refused` : `apply failed (${applied && applied.run && applied.run.output})`}; ${decided.notes || 0} notes.`)
    // 4. The notes go to DeepSeek as a card cycle: every hard rule in mode "full" (the script adds N1), else N1.
    const notes = decided && decided.notes ? await Z.runCards('organize notes', `${OUT}\\cards-notes`, ` -Notes "${ORG}\\decisions.json" -NoOrphans`, MODE === 'full' ? '' : 'N1') : null
    // 5. The skeleton: the Literature Card index, the structure Card, the Topic bodies.
    const SK = `${OUT}\\skeleton`
    const kstep = (s, more = '') => `pwsh -NoProfile -File "${SCRIPTS}\\book-skeleton.ps1" -Step ${s} -Staging "${OUT}" -Work "${SK}"${more}`
    const kopts = { dsh: SK, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Organize', writes: `${SK}\\out`, remove: 'delete', keep: true }
    await Z.drive('organize: skeleton', { ...kopts, command: kstep('plan') })
    const sk = zdLastJson(((await Z.drive('organize: skeleton apply', { ...kopts, command: kstep('apply', ` -Decisions "${ORG}\\decisions.json" -Apply`), cleanup: true })) || {}).output)
    log(sk && sk.ok ? `Organize: structure Card ${sk.written && sk.written.structure} ("${sk.thesis}"), ${sk.argument} argument lines, ${sk.models} models, ${sk.events} events, ${sk.topics} Topic bodies.` : `Organize: the skeleton failed: ${JSON.stringify((sk && sk.problems) || 'no output')}`)
    return { survey, decisions: decided, applied: moved, notes: notes && notes.report, skeleton: sk }
  },
})
// ==== end module zt-organize ====
