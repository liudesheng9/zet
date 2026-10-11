export const meta = {
  name: 'zt-book-digest',
  description: 'Digest a Chinese-translated book into ZET Cards: Haiku managers drive DeepSeek Harness runs that write idea, event, and Model Cards for their sections; as each block finishes, Sonnet studies the source behind every new Model Card and designs its structure (parts it names, core and aside parts, reasoned relations, HTML charts); a link pass ties the parts of the book together, then Opus organizes the structure',
  whenToUse: 'Turn a Scholar Archive book (Chinese bilingual Markdown, digested as Chinese only) into an agent Literature tree plus Topic trees in the ZET slip-box; mode "link" re-runs only the cross-part link pass on an existing digest; mode "models" re-runs only the Sonnet model stage on its Model Cards.',
  phases: [
    { title: 'Prepare', detail: 'Chinese-only book text, section groups, agent Literature tree with anchors', model: 'haiku' },
    { title: 'Digest', detail: 'each Haiku manager drives its block of DeepSeek runs; each run gets its own sections inline, searches the rest of the book on disk, and writes Cards for its sections, one whole Model Card per model', model: 'haiku' },
    { title: 'Models', detail: 'as soon as a block of DeepSeek runs finishes, one Sonnet agent per new Model Card reads a context pack (the card, its discussion, its group source files, the other models of the book), studies the source, and designs the model: the parts it needs under its own facet labels, core and aside parts, reasoned relations (loose parts allowed), and HTML charts', model: 'sonnet' },
    { title: 'Link', detail: 'each DeepSeek run gets only the Cards it owns, searches the book and the Card index on disk, and appends reasoned cross-part Links', model: 'haiku' },
    { title: 'Cleanup', detail: 'Haiku archives conversations, removes the DeepSeek workspace and scratch files', model: 'haiku' },
    { title: 'Organize', detail: 'Opus merges Topics, writes hubs, entry points, the book structure Card, and links', model: 'opus' },
  ],
}

// args: { mode ('full' | 'link' | 'models'), source, out, citationKey, bibtex, bookTitle,
//         groups (modes 'link' and 'models': the existing group count), organize (modes 'link' and 'models': default false),
//         perManager (5), maxChars (20000), maxUnits (3),
//         dshModel ('deepseek-official/deepseek-flash'), dshEffort ('low'),
//         models (true: run the Sonnet model stage), redoModels (false: also redo models that already have parts),
//         perSonnet (1: Model Cards per Sonnet agent; neighbouring models of the book share one agent) }
//
// How the stages hand work on:
//   Prepare ─▶ Digest block 1 (Haiku ▸ DeepSeek ×5) ─▶ Models for block 1 (Haiku lists, Sonnet ×N) ─┐
//           ─▶ Digest block 2 (Haiku ▸ DeepSeek ×5) ─▶ Models for block 2 (Haiku lists, Sonnet ×N) ─┼─▶ Link ─▶ Cleanup ─▶ Organize (Opus)
//           ─▶ …                                                                                    ─┘
// Blocks run in parallel; a block's models are elaborated while later blocks are still digesting.
const A = args
const MODE = A.mode || 'full'
const KEY = A.citationKey
const OUT = A.out
const DIGEST_PREFIX = `${OUT}\\shared-prefix.md`
const LINK_PREFIX = `${OUT}\\link-prefix.md`
const PER_MANAGER = A.perManager || 5
const DSH_MODEL = A.dshModel || 'deepseek-official/deepseek-flash'
const DSH_EFFORT = A.dshEffort || 'low'
const SCRIPTS = 'E:\\zet\\scripts\\book-digest'
const MODELS = A.models !== false
const MODEL_DOC = `${SCRIPTS}\\model-elaboration.md`
const MODEL_WORK = `${OUT}\\models`
const PER_SONNET = Math.max(1, A.perSonnet || 1)
const HELPER = 'C:\\Users\\chenx\\.codex\\skills\\zt\\scripts\\zt-mcp.ps1'
const DSH_TOOLS = 'select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_prompt,mcp__deepseek__deepseek_close,mcp__deepseek__deepseek_archive'

const USAGE = {
  type: 'object',
  properties: {
    input_cache_miss: { type: 'number' }, cached_read: { type: 'number' },
    output: { type: 'number' }, cost: { type: 'number' }, calls: { type: 'number' },
  },
  required: ['input_cache_miss', 'cached_read', 'output', 'cost', 'calls'],
}
const usageStep = 'Load the DeepSeek tools with ToolSearch query "select:mcp__deepseek__deepseek_usage" and call deepseek_usage with days=1. Sum each field over the rows for the last day listed.'

// --- Prepare ---------------------------------------------------------------------------------
phase('Prepare')
let groupCount = A.groups || 0
let usageBefore = null
if (MODE === 'full') {
  const prep = await agent(
    `You prepare a book for a ZET digest. Do exactly these steps and nothing else.
1. Run this PowerShell command (one call) and read its last output line, a compact JSON manifest:
pwsh -NoProfile -File ${SCRIPTS}\\prepare-book-digest.ps1 -Source "${A.source}" -Out "${OUT}" -CitationKey ${KEY} -BibTeX '${A.bibtex}' -BookTitle '${A.bookTitle}' -MaxChars ${A.maxChars || 20000} -MaxUnits ${A.maxUnits || 3} -Apply
If it fails, return ok=false with the error text; do not retry with other options and do not delete anything.
2. ${usageStep}
Return groups = the number of entries in manifest.groups and usage_before = the summed usage.`,
    {
      label: 'prepare book', phase: 'Prepare', model: 'haiku', effort: 'low',
      schema: {
        type: 'object',
        properties: { ok: { type: 'boolean' }, error: { type: 'string' }, groups: { type: 'number' }, usage_before: USAGE },
        required: ['ok', 'groups'],
      },
    },
  )
  if (!prep || !prep.ok || !prep.groups) {
    log(`Preparation failed: ${prep && prep.error}`)
    return { stage: 'prepare', prep }
  }
  groupCount = prep.groups
  usageBefore = prep.usage_before
}
if (!groupCount && MODE !== 'models') return { stage: 'prepare', error: 'mode "link" needs args.groups' }

const groups = Array.from({ length: groupCount }, (_, i) => {
  const n = i + 1
  const nn = String(n).padStart(2, '0')
  return { n, nn, file: `${OUT}\\groups\\g${nn}.md`, snapshot: `${OUT}\\snapshot-g${nn}.md`, link: `${OUT}\\link-g${nn}.md` }
})
// Contiguous blocks in book order: managers run in parallel on different chapters (few duplicate
// Topics), and inside a block each run starts after its predecessor and sees its new Cards.
const toBlocks = list => {
  const out = []
  for (let i = 0; i < list.length; i += PER_MANAGER) out.push(list.slice(i, i + PER_MANAGER))
  return out
}
const blocks = toBlocks(groups)

// --- one Haiku manager per block -----------------------------------------------------------------
const REPORT = {
  type: 'object',
  properties: {
    group: { type: 'number' }, status: { type: 'string', enum: ['done', 'failed'] },
    cards: { type: 'number' }, models: { type: 'number' }, events: { type: 'number' }, links: { type: 'number' }, topics: { type: 'array', items: { type: 'string' } },
    errors: { type: 'string' }, archived: { type: 'boolean' },
    tool_calls: { type: 'number' }, context_used: { type: 'number' },
  },
  required: ['group', 'status', 'errors', 'archived'],
}

const STAGES = {
  digest: {
    phase: 'Digest',
    steps: g => `Group ${g.n}:
  a. Snapshot the slip-box (one PowerShell call): pwsh -NoProfile -File ${SCRIPTS}\\snapshot-topics.ps1 -Out "${g.snapshot}"
  b. deepseek_run with exactly: cwd "${OUT}"; prompt_files ["${DIGEST_PREFIX}", "${g.file}"] (the book, its index and the snapshot stay on disk; DSH reads what it needs); prompt "按以上说明完成你的组，只回复一行 JSON。"; skills ["zettelkasten"]; model "${DSH_MODEL}"; reasoning_effort "${DSH_EFFORT}"; wait_seconds 900.`,
    answer: '{"group","cards","models","events","topics","anchor","errors"}',
    fields: 'cards (Cards written), models (Model Cards), events (Event Cards), topics (as "地址 主题名")',
  },
  link: {
    phase: 'Link',
    steps: g => `Group ${g.n}:
  deepseek_run with exactly: cwd "${OUT}"; prompt_files ["${LINK_PREFIX}", "${g.link}"]; prompt "按以上说明完成你的组的连接，只回复一行 JSON。"; skills ["zettelkasten"]; model "${DSH_MODEL}"; reasoning_effort "${DSH_EFFORT}"; wait_seconds 900.
  When it finished with status "done", mark it (one PowerShell call): Set-Content -LiteralPath "${OUT}\\linked-g${g.nn}.txt" -Value done`,
    answer: '{"group","cards_linked","links_added","errors"}',
    fields: 'cards (= cards_linked), links (= links_added)',
  },
}

const manage = (block, b, stage) => {
  const S = STAGES[stage]
  return agent(
    `You manage ${block.length} DeepSeek Harness (DSH) runs of a ZET book digest (${stage} pass), one after another. DSH reads the book and changes the Cards; you only start, supervise, and clean up each run. Keep your messages short.
Load all tools with one ToolSearch call: query "${DSH_TOOLS}".

For each group below, in order, finish it completely before starting the next:
${block.map(S.steps).join('\n')}

Pass every deepseek_run value unchanged: the identical leading prompt files, prompt, skills, model and effort keep the DSH prompt cache warm.
Then follow "status" until the run ends:
  - "running": deepseek_wait (session_id, wait_seconds 900).
  - "needs_response": approve (decision "allow") a pwsh command that runs zt-mcp.ps1, and read-only reads or searches (Read, Get-Content, Select-String) of files under "${OUT}" or of "${A.source}"; reject anything that writes, moves or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
  - "error", or "finished" without a one-line JSON answer: once, deepseek_prompt (same session, wait_seconds 900) with "按 resume 行修正失败的调用后重跑，最后只回复一行 JSON。"; if it still fails, mark the group "failed".
  - "finished": parse its JSON ${S.answer}.
  Then clean up: deepseek_close and deepseek_archive (session_id); archived=true when archiving succeeds.
  Record tool_calls = the sum of the counts in the last turn report's tools_used, and context_used = context.used.

Return reports: one entry per group in this block with group, status, ${S.fields}, errors, archived, tool_calls, context_used.`,
    {
      label: `${stage} manager ${b + 1}: groups ${block[0].n}-${block[block.length - 1].n}`, phase: S.phase, model: 'haiku', effort: 'low',
      schema: { type: 'object', properties: { reports: { type: 'array', items: REPORT } }, required: ['reports'] },
    },
  )
}

// --- Models: Sonnet turns each new Model Card into a structured model -------------------------------
const MODEL_LIST = {
  type: 'object',
  properties: {
    ok: { type: 'boolean' }, error: { type: 'string' },
    models: { type: 'array', items: { type: 'object', properties: { address: { type: 'string' }, title: { type: 'string' }, parts: { type: 'number' }, issues: { type: 'number' } }, required: ['address'] } },
  },
  required: ['ok', 'models'],
}
const MODEL_REPORT = {
  type: 'object',
  properties: {
    model: { type: 'string' }, status: { type: 'string', enum: ['done', 'skipped', 'failed'] },
    parts: { type: 'array', items: { type: 'object', properties: { address: { type: 'string' }, facet: { type: 'string' }, emphasis: { type: 'string' } }, required: ['address', 'facet'] } },
    relations: { type: 'number' }, loose: { type: 'number' },
    charts: { type: 'number' }, issues: { type: 'array', items: { type: 'string' } },
    notes: { type: 'array', items: { type: 'string' } }, summary: { type: 'string' },
  },
  required: ['model', 'status', 'parts', 'charts', 'issues'],
}
const listModels = (selector, label) => agent(
  `List the Model Cards of a ZET book digest. Run this one PowerShell command and read its last output line, a compact JSON object {"models":[{"address","title","group","parts","issues"}]}:
pwsh -NoProfile -File ${SCRIPTS}\\list-models.ps1 -Staging "${OUT}" ${selector}
If it fails, return ok=false with the error text and models=[]; do not retry with other options. Otherwise return ok=true and the models array exactly.`,
  { label, phase: 'Models', model: 'haiku', effort: 'low', schema: MODEL_LIST },
)
// Sonnet decides the model's structure itself: it starts from a context pack (model-context.ps1)
// and may read any book file, any Card, and the Scholar Archive.
const elaborate = (model) => {
  const work = `${MODEL_WORK}\\${model.address.replace(/[\/|]/g, '_')}`
  return agent(
    `Elaborate one Model Card of the ZET digest of 《${A.bookTitle}》: understand the model from the book and design its structure yourself. Read your instructions first: ${MODEL_DOC} (Read tool), then follow them exactly.
MODEL: ${model.address} (${model.title || ''})
KEY: ${KEY}
GROUP: ${model.group ? `${model.group} (anchor [[${KEY}/${model.group}]])` : 'unknown; the context script finds it'}
BOOK: ${OUT} (sections\\sNN.md, sections\\index.md, book.zh.md, groups\\gNN.md, manifest.json; the bilingual source is ${A.source || 'in the Scholar Archive'})
SCRIPTS: ${SCRIPTS}
CONTEXT: ${work}\\context.md
HELPER: ${HELPER}
WORK: ${work} (create it for your calls files)${A.redoModels ? '\nREDO: restructure this model even if it already has parts.' : ''}`,
    { label: `model ${model.address}`, phase: 'Models', model: 'sonnet', schema: MODEL_REPORT },
  )
}
// With perSonnet > 1, one Sonnet agent elaborates several neighbouring Model Cards in turn, each by
// the same instructions, so what it learns from the source for one serves the next.
const MODEL_BATCH = {
  type: 'object',
  properties: { reports: { type: 'array', items: MODEL_REPORT } },
  required: ['reports'],
}
const elaborateBatch = (batch) => {
  if (batch.length === 1) return elaborate(batch[0]).then(r => [r])
  const entry = (m, i) => {
    const work = `${MODEL_WORK}\\${m.address.replace(/[\/|]/g, '_')}`
    return `## Model ${i + 1}
MODEL: ${m.address} (${m.title || ''})
GROUP: ${m.group ? `${m.group} (anchor [[${KEY}/${m.group}]])` : 'unknown; the context script finds it'}
CONTEXT: ${work}\\context.md
WORK: ${work} (create it for your calls files)`
  }
  return agent(
    `Elaborate ${batch.length} Model Cards of the ZET digest of 《${A.bookTitle}》, one after another: for each, understand the model from the book and design its structure yourself. Read your instructions first: ${MODEL_DOC} (Read tool), then follow them exactly for each model in turn, as if it were the only one (its own context pack, WORK folder, filing, and get_model check). Finish one model before you start the next. These models sit near each other in the book: what you learn from the source for one may serve the next, and where the book relates two of them, link the parts concerned.
KEY: ${KEY}
BOOK: ${OUT} (sections\\sNN.md, sections\\index.md, book.zh.md, groups\\gNN.md, manifest.json; the bilingual source is ${A.source || 'in the Scholar Archive'})
SCRIPTS: ${SCRIPTS}
HELPER: ${HELPER}${A.redoModels ? '\nREDO: restructure each model even if it already has parts.' : ''}

${batch.map(entry).join('\n\n')}

Return reports: one report per model, in this order, each as the instructions' Report section describes.`,
    { label: `models ${batch.map(m => m.address).join(', ')}`, phase: 'Models', model: 'sonnet', schema: MODEL_BATCH },
  ).then(r => batch.map(m => ((r && r.reports) || []).find(x => x.model === m.address) || null))
}
const groupNo = (m) => Number(m.group) || Number(String(m.address).split('/')[1].split('|')[0]) || 0
const elaborateAll = async (models, label) => {
  const todo = models.filter(m => A.redoModels || !(m.parts > 0 && !m.issues))
    .sort((a, b) => groupNo(a) - groupNo(b) || String(a.address).localeCompare(String(b.address)))
  if (todo.length < models.length) log(`${label}: ${models.length - todo.length} of ${models.length} models already have their parts; skipped.`)
  const batches = []
  for (let i = 0; i < todo.length; i += PER_SONNET) batches.push(todo.slice(i, i + PER_SONNET))
  if (PER_SONNET > 1) log(`${label}: ${todo.length} models in ${batches.length} Sonnet agents, up to ${PER_SONNET} each.`)
  const results = (await parallel(batches.map(b => () => elaborateBatch(b)))).flatMap((r, i) => r || batches[i].map(() => null))
  return results.map((r, i) => r || { model: todo[i].address, status: 'failed', parts: [], charts: 0, issues: ['no report from its agent'] })
}
// After a digest block: the Model Cards its DeepSeek runs wrote, to Sonnet agents (perSonnet each).
const elaborateBlock = async (block, managed) => {
  const reports = (managed && managed.reports) || []
  const groups = block.map(g => g.n).filter(n => {
    const r = reports.find(x => x.group === n)
    return r && r.status === 'done' && r.models !== 0
  })
  if (!groups.length) return []
  const listed = await listModels(`-Groups ${groups.join(',')}`, `find models: groups ${groups[0]}-${groups[groups.length - 1]}`)
  if (!listed || !listed.ok) { log(`Models for groups ${groups.join(',')}: listing failed (${listed && listed.error})`); return [] }
  return elaborateAll(listed.models, `groups ${groups[0]}-${groups[groups.length - 1]}`)
}

// A stage runs its blocks in parallel; with `after`, each block goes on to `after` as soon as its
// own manager returns (a pipeline, not a barrier), so later stages overlap slower blocks.
const runStage = async (stage, stageBlocks, after) => {
  const stages = [(block, _item, b) => manage(block, b, stage)]
  if (after) {
    stages.push((managed, block) => Promise.resolve(after(block, managed))
      .catch(error => { log(`${stage}: follow-up of groups ${block[0].n}-${block[block.length - 1].n} failed: ${error}`); return [] })
      .then(extra => ({ managed, extra })))
  }
  const results = await pipeline(stageBlocks, ...stages)
  const managed = results.map(r => (after ? r && r.managed : r))
  const extras = after ? results.flatMap(r => (r && r.extra) || []) : []
  const reports = []
  stageBlocks.forEach((block, b) => {
    const got = (managed[b] && managed[b].reports) || []
    for (const g of block) {
      reports.push(got.find(r => r.group === g.n) ||
        { group: g.n, status: 'failed', cards: 0, links: 0, topics: [], errors: 'no report from its manager', archived: false })
    }
  })
  const failed = reports.filter(r => r.status !== 'done').map(r => r.group)
  const calls = reports.map(r => r.tool_calls).filter(x => typeof x === 'number')
  log(`${stage}: ${reports.reduce((s, r) => s + (r.cards || 0), 0)} cards, ${reports.reduce((s, r) => s + (r.links || 0), 0)} links; tool calls per group ${calls.join(', ') || 'n/a'}; failed ${failed.join(', ') || 'none'}`)
  reports.extras = extras
  return reports
}

const logModels = (reports) => {
  const done = reports.filter(r => r.status === 'done')
  const sum = (field) => done.reduce((s, r) => s + (r[field] || 0), 0)
  log(`Models: ${done.length} structured (${done.reduce((s, r) => s + r.parts.length, 0)} parts, ${done.reduce((s, r) => s + r.parts.filter(p => p.emphasis === 'core').length, 0)} core, ${sum('relations')} relations, ${sum('loose')} loose, ${sum('charts')} charts),${reports.filter(r => r.status === 'skipped').length} skipped, failed ${reports.filter(r => r.status === 'failed').map(r => r.model).join(', ') || 'none'}; open issues ${reports.reduce((s, r) => s + r.issues.length, 0)}.`)
}

let digestReports = []
let modelReports = []
if (MODE === 'full') {
  phase('Digest')
  log(`${groupCount} groups in ${KEY}; ${blocks.length} Haiku managers, up to ${PER_MANAGER} DeepSeek runs each${MODELS ? '; each block\'s Model Cards go to Sonnet as soon as the block finishes' : ''}.`)
  digestReports = await runStage('digest', blocks, MODELS ? elaborateBlock : null)
  modelReports = digestReports.extras
  if (MODELS) logModels(modelReports)
}
if (MODE === 'models') {
  phase('Models')
  const listed = await listModels(`-All -CitationKey ${KEY}`, 'find models: whole book')
  if (!listed || !listed.ok) return { stage: 'models', error: listed && listed.error }
  log(`${listed.models.length} Model Cards in the digest of ${KEY}.`)
  modelReports = await elaborateAll(listed.models, 'whole book')
  logModels(modelReports)
  if (!A.organize) return { mode: MODE, book: A.bookTitle, citation_key: KEY, models: modelReports }
}

// --- Link ----------------------------------------------------------------------------------------
if (MODE !== 'models') phase('Link')
const linkPrep = MODE === 'models' ? { ok: true, pending: [] } : await agent(
  `Prepare the cross-part link pass of a ZET book digest. Do exactly these steps.
1. Run this PowerShell command (one call): pwsh -NoProfile -File ${SCRIPTS}\\prepare-link-pass.ps1 -Staging "${OUT}"
   Its last line is JSON {"pending":[n…],"groups":[{"n","owned","done"}],"unowned","cards_all"}. If it fails, return ok=false with the error.
2. ${MODE === 'link' ? usageStep : 'Skip this step.'}
Return ok, pending (the "pending" array exactly), owned_total (sum of owned), unowned${MODE === 'link' ? ', usage_before' : ''}.`,
  {
    label: 'prepare link pass', phase: 'Link', model: 'haiku', effort: 'low',
    schema: {
      type: 'object',
      properties: {
        ok: { type: 'boolean' }, error: { type: 'string' }, pending: { type: 'array', items: { type: 'number' } },
        owned_total: { type: 'number' }, unowned: { type: 'number' }, usage_before: USAGE,
      },
      required: ['ok', 'pending'],
    },
  },
)
if (!linkPrep || !linkPrep.ok) {
  log(`Link preparation failed: ${linkPrep && linkPrep.error}`)
  return { stage: 'link-prepare', linkPrep, digestReports }
}
if (MODE === 'link') usageBefore = linkPrep.usage_before
// Groups whose anchor already states its place in the book are skipped, so a rerun links only the rest.
const pending = groups.filter(g => linkPrep.pending.includes(g.n))
if (MODE !== 'models') log(`Link pass: ${pending.length} of ${groupCount} groups pending; ${linkPrep.owned_total} owned Cards; ${linkPrep.unowned} unowned (hubs, structure Cards, Model parts).`)
const linkReports = pending.length ? await runStage('link', toBlocks(pending)) : []

// --- Cleanup -------------------------------------------------------------------------------------
if (MODE !== 'models') phase('Cleanup')
const cleanup = MODE === 'models' ? null : await agent(
  `Clean up after a DeepSeek Harness batch. Load tools with ToolSearch query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_archive,mcp__deepseek__deepseek_close,mcp__deepseek__deepseek_remove_workspace,mcp__deepseek__deepseek_usage".
1. deepseek_list with cwd "${OUT}". Close and archive every conversation there that is not archived yet; count them.
2. deepseek_remove_workspace with cwd "${OUT}" (removes only the sidebar entry; history is kept).
3. Delete the per-run scratch files with one PowerShell call and report the count it prints:
   $f = Get-ChildItem -LiteralPath "${OUT}" -File | Where-Object { $_.Name -like '*calls*.json' -or $_.Name -like 'snapshot-g*.md' -or $_.Name -like 'link-g*.md' -or $_.Name -like 'allow-g*.txt' }; $f | Remove-Item; $f.Count
   Delete nothing else (keep the book, prefixes, manifest, cards-all.md and owned-g*.txt).
4. deepseek_usage with days=1; sum each field over the rows for the last day listed.
Return late_archived, workspace_removed, files_removed, usage_after.`,
  {
    label: 'cleanup DeepSeek', phase: 'Cleanup', model: 'haiku', effort: 'low',
    schema: {
      type: 'object',
      properties: { late_archived: { type: 'number' }, workspace_removed: { type: 'boolean' }, files_removed: { type: 'number' }, usage_after: USAGE },
      required: ['late_archived', 'workspace_removed', 'files_removed'],
    },
  },
)

let dshUsage = null
if (usageBefore && cleanup && cleanup.usage_after) {
  const b = usageBefore, a = cleanup.usage_after
  const miss = a.input_cache_miss - b.input_cache_miss, hit = a.cached_read - b.cached_read
  const runs = Math.max(1, (MODE === 'full' ? groupCount : 0) + linkReports.length)
  dshUsage = {
    input_cache_miss: miss, cached_read: hit, output: a.output - b.output,
    calls: a.calls - b.calls, cost: Math.round((a.cost - b.cost) * 10000) / 10000,
    cache_hit_rate: miss + hit > 0 ? Math.round((hit / (miss + hit)) * 1000) / 1000 : null,
    per_run: { input: Math.round((miss + hit) / runs), output: Math.round((a.output - b.output) / runs), calls: Math.round(((a.calls - b.calls) / runs) * 10) / 10 },
  }
  log(`DeepSeek usage for this batch: ${JSON.stringify(dshUsage)}`)
}

// --- Organize ------------------------------------------------------------------------------------
let organize = null
if (MODE === 'full' || A.organize) {
  const modelLines = modelReports.filter(r => r.status === 'done').map(r => `- [[${r.model}]]：${r.parts.map(p => `${p.facet}${p.emphasis === 'core' ? '（核心）' : ''} [[${p.address}]]`).join('、')}${r.issues.length ? `；未解决：${r.issues.join('；')}` : ''}${r.notes && r.notes.length ? `；建议：${r.notes.join('；')}` : ''}`).join('\n')
  phase('Organize')
  const groupLines = (MODE === 'full' ? digestReports : linkReports).map(r => `- 第 ${r.group} 组 [[${KEY}/${r.group}]]：${r.status}${r.cards ? `，${r.cards} 张` : ''}${r.models ? `，模型卡 ${r.models}` : ''}${r.events ? `，事件卡 ${r.events}` : ''}${r.topics && r.topics.length ? `；新主题：${r.topics.join('、')}` : ''}${r.errors && r.errors !== '无' ? `；问题：${r.errors}` : ''}`).join('\n')
  organize = await agent(
    `You are the final coordinator of a ZET book digest. DeepSeek agents have digested 《${A.bookTitle}》 group by group into the agent Workspace of the ZET slip-box, then added reasoned cross-part Links to their own Cards. Your job is to organize the whole card structure. Use the zettelkasten and zt skills (load them with the Skill tool first). Make every zt call through the helper script, not the mcp__zt__* tools (the conversation's MCP server may be an older zt): write a JSON array of calls and run it in one PowerShell command, e.g.
@'
[{"tool":"list_trees","arguments":{"workspace":"agent"}},{"tool":"search_cards","arguments":{"type":"model","workspace":"agent"}}]
'@ | pwsh -NoProfile -File "${HELPER}" -
Each output line is "N <json>", "N ok <address>", or "N error <reason>" (then a resume line); "@@N" in an argument is the address returned by call N. Batch many reads or writes per command. delete_card and move_card return a plan first; call again with the confirm text. Write all Card text in Chinese. Change only the agent Workspace; human Cards are read-only but you may link to them.

Book: Literature Card [[${KEY}]] (agent Workspace, Literature Zone). Its Direct-successor line [[${KEY}/1]] … [[${KEY}/${groupCount}]] holds one anchor Card per group, in book order; each anchor holds that part's central claim, a section index, and its place in the whole book.
Group reports:
${groupLines}
${modelLines ? `Structured models (Sonnet designed each model's parts, facets, core parts, and relations, with charts; act on its suggestions for Cards outside the model):\n${modelLines}\n` : ''}
Work in this order, reading cheaply (list_trees, get_tree excerpts; get_card only when you need a body):
1. Survey every agent tree (list_trees with workspace "agent", then get_tree each).
2. Merge duplicate or overlapping Topics: keep the clearer Topic, move_card the other's first Card (with its subtree) to the best free slot in the kept tree (plan first, then confirm with "move"), then delete the emptied Topic root (plan, then confirm with its address). Fix clearly misplaced Cards the same way. Never delete a Card that still has content.
3. Rewrite every agent Topic body: 2–3 sentences on the question it pursues, then Links to its 1–3 best entry Cards.
4. For a Topic or subject with about seven or more Cards, write a hub Card as a Side successor of <n>/1 that lists Links to the core Cards with one line each on what they contribute, and Link to it from the Topic body.
5. Write the book's structure Card as a Side successor of [[${KEY}/1]]: title = the book's central thesis as one sentence; body = the argument in reading order, one line per part linking its anchor and its 1–2 key Topic Cards, each line saying what it contributes and how it follows from the previous part; then a paragraph listing the book's Model Cards (search_cards with type "model") in order, one line each on what the model shows and which later models build on it, and a paragraph listing its Event Cards (type "event") with the ideas each supports.
5b. Check Model and Event Cards: each model the book sets out has exactly one Model Card (merge duplicates; set type "model" on a Card that states a whole model but is typed idea); run get_model on every Model Card and fix each reported issue (the Model Card links every part, every part links its Model Card), but keep the structure Sonnet designed (its facets, core and aside parts, relations, and loose parts); every Card that derives, interprets, extends, or criticises a model links to its Model Card, or better to the Model part it concerns (a critique of an assumption links the assumptions part), in a sentence naming which part of the model it concerns (read the inbound Links, then search for discussions that miss it); every Card that uses a real-world event as evidence links to its Event Card.
6. Rewrite the Literature Card [[${KEY}]] body (edit_card body only) as the book's index: one line per chapter, "第N章 章名 - 讲了什么 → [[anchor]]", then a line linking the structure Card.
7. Check the cross-part Links the DeepSeek agents added: keep those whose sentence states a real relationship and reason; rewrite vague ones. Add Links still missing between Topics that share a mechanism, and to related human Cards (search_cards with workspace "human"). Remove a "# 标题" line that repeats the title at the top of a body.
8. Check list_broken_links and fix any in the agent Workspace.
Return a short Chinese report: the final agent Topics (address and name), merges and moves done, hubs created, the structure Card address, link counts added or rewritten, and anything left for the user.`,
    { label: 'organize card structure', phase: 'Organize', model: 'opus' },
  )
}

return {
  mode: MODE, book: A.bookTitle, citation_key: KEY, groups: groupCount, managers_per_pass: blocks.length,
  digest: digestReports.map(r => ({ group: r.group, status: r.status, cards: r.cards, tool_calls: r.tool_calls ?? null })),
  models: modelReports.map(r => ({ model: r.model, status: r.status, parts: r.parts.length, charts: r.charts, issues: r.issues })),
  link: linkReports.map(r => ({ group: r.group, status: r.status, cards_linked: r.cards, links: r.links, tool_calls: r.tool_calls ?? null, errors: r.errors })),
  deepseek_usage: dshUsage, cleanup, organize,
}
