// ==== module zt-digest-lib v38 ====
// The shared machinery of the ZT digest workflows (docs/zt-digest-framework.md), first ported from the
// hand-written zt-book-digest.js (since removed): the settings read from the args, the digest and link managers, the
// compiler-step driver (drive), the ready-work driver (chain), and the card layer cycle (runCards). The model
// compiler is zt-model-lib (the model layer, which each book type installs). A library module: the stage modules
// call zdMake(ctx, params) and use what it returns.
// One card cycle per book, not one per block (R296): `runCards` runs the whole-book cycle, and its
// `-ModelReview` option lets that cycle own the source review of a Model draft no model chain elaborates (R247).
// 11 adds `fixRounds`: the finalization's one correction cycle is declared, not inherited from the pilot
// setting (#86, R299). 12 resolves the run's Citation key from the prepared run (parameter `key`, from
// `stages.prepare.output.citation_key`), so mode resume uses the ledger's retained key instead of requiring
// the operator to repeat args.citationKey (#87, R274/R276). 13 makes every claim cover the provider run it
// starts: `ledger next -Lease <dshRunCap + 300>` (#92), and a chain renews that claim after every wait that
// reports a running job, so a healthy job that outlives the 120-second base lease is never re-readied.
// Every Haiku, Sonnet, and Opus agent goes through ctx.agent, so the blueprint's roles decide the models:
// manager (Haiku), architect (Sonnet), organizer (Opus). Managers share the DeepSeek Subagent Managed
// Protocol pool (dsmpSlot): at most 8 managers alive in the whole run.
// The ready-work schedule (9.3/9.4, #82): the ledger owns readiness. A driver claims ready units in bounded
// blocks (`ledger next`), runs each unit's compiler chain in ONE manager's conversation (`chain`), records the
// result through that family's checked finish command, and seals with the layer gate. Retries are per unit
// (`maxAttempts`), never a whole-stage rerun. No module reads a clock: the bounded readiness wait lives in
// zt-digest (`ledger next -Wait`).
// 31 (Tree Lock S17, #140): a write the owner's Lock refused is a planning error, never a provider fault
// (R326/R337). `zdLockRefusal` reads the archive's typed `card_locked` JSON out of a relayed line, and the
// stage stops with it — one failed script command, a failed compiler step, a claimed unit's start or
// checked finish, or a refused chain step — while the drafts and the receipts already committed stay. The
// unit keeps the refusal as its content fault (`code: card_locked`), never a retryable one, and nothing is
// retried, unlocked, or re-pointed through another Access (R338).
// 35 (docs/digest-claude-token-proposal.md, P1/P2/P4): a block's checked finishes run in one command
// (`zt-digest finish-batch`, one short line) instead of one relay per unit; the Cards layer claims and finishes
// in that one command; the events, findings and explain chains end with it in their own manager; and a compiler
// step's command agent loads no DeepSeek tool. Every finish keeps its own verifier, token and fault class.
// 36 (P5): a one-command relay (runScript) runs as the blueprint's relay role, the zt-relay agent type (PowerShell
// only, a short prompt): its first request carries about 9k tokens of context instead of about 59k.
// 37 (docs/workflow-simplicity-proposal.md S1, S5, S6, S8): the JSON line reader, the Lock reader and the relay are the
// shared library's (zt-workflow-lib, which zt-idea-grow loads too); a compiler step's command runs through the relay
// (no manager prompt runs a command); the DeepSeek source is frozen at the official one (no probe); the ledger is the
// one fault classifier (a fault leaves out its class and, where only a cause text says it, its code); and the dead code
// (the stage managers, runStage, drain, jobOf, renew, resourceList, finishCleanup, cleanupDsh, CARD_CHECKS,
// LINK_PER_MANAGER) and the unused or alias args (perManager, linkPerManager, dshPerSonnet, redoModels, dshSource,
// dshModel) leave.
// 38 (#175, docs/workflow-module-structure-proposal.md): the layer driver becomes script steps. `layerRun` drives one
// layer in blocks; every block is one `layer-steps.ps1` step run by a DeepSeek drive job (zt-workflow-lib's shared
// `ztDrive`): `start` claims the block and lists the jobs, the block's content jobs run in Haiku managers, and `finish`
// records every held unit's checked completion, faults the rest and runs the layer gate. A module never relays a
// command; per-unit outcomes are the ledger's and the step's JSON line. The relay-era block driver (block, blockSettle,
// blockFinish, blockPrompt) leaves. The relay helpers (`runScript`, `chain`, `claim`, `finishUnits`, `holding`, `gate`,
// `layer`, `ledgerReport`) stay only for the modules the later slices of the structure migrate (the model layer, the
// whole-book cycle, the preparation and the settlement).
const ZD_SCRIPTS = 'E:\\zet\\scripts\\book-digest'
// The digest scripts ported to zt-digest (tools/zt-digest, one fast binary): script name → tool. zdTool(name)
// is the command that runs a script: `zt-digest <tool>` once it is ported, else pwsh with the script. The
// helper is `zt-digest call` (ZD_CALL); the old helper path zt-mcp.ps1 forwards to it.
const ZD_DIGEST = 'zt-digest'
const ZD_TOOLS = {
  'digest-audit': 'audit', 'card-compiler': 'card', 'model-compiler': 'model', 'model-context': 'model-context', 'list-models': 'list-models',
  'bridge-compiler': 'bridge', 'math-compiler': 'math', 'event-compiler': 'event', 'finding-compiler': 'finding',
  'book-terms': 'terms', 'book-skeleton': 'skeleton', 'organize-survey': 'organize-survey', 'organize-apply': 'organize-apply',
  'attach-figures': 'figures', 'prepare-book-digest': 'prepare', 'prepare-link-pass': 'prepare-link', 'snapshot-topics': 'snapshot-topics',
}
const ZD_CALL = `${ZD_DIGEST} call`
// 32 (docs/literature-only-digest-design.md): the helper runs in the Literature scope only without -Open, so
// every approval rule that lets a DeepSeek job run it refuses a helper command that carries -Open.
// 33: the hub settings (hubWork, the hub compiler, hubEffort, hubFixRounds) and the hub compiler's self-check in the
// approval rules leave the library with the unreachable zt-concepts, zt-review and zt-hubs modules: hubs are zt-idea-grow's.
const ZD_NO_OPEN = 'refuse a zt helper command that carries -Open, in any letter case: it leaves the digest\x27s Literature scope'
const ZD_LEDGER = `${ZD_DIGEST} ledger`
// R265: every manager's compiler command carries this explicit timeout (milliseconds).
const ZD_COMMAND_TIMEOUT_MS = 600000
// R269: a write-capable compiler command gives its child `zt mcp` this many seconds of edit patience.
const ZD_EDIT_PATIENCE = 180
function zdTool(name, scripts = ZD_SCRIPTS) {
  return ZD_TOOLS[name] ? `${ZD_DIGEST} ${ZD_TOOLS[name]}` : `pwsh -NoProfile -File "${scripts}\\${name}.ps1"`
}
const ZD_DSH_SOURCES = {
  official: 'deepseek-official/deepseek-flash',
}
const ZD_PER_MANAGER_MAX = 8
const ZD_MAX_MANAGERS = 8
// R343–R346: run usage is per provider run, recorded from that run's own result. This framework never
// reads the account-wide usage ledger: the difference of two rolling totals is not a run's cost (I22/R344).
const ZD_RUN_SCHEMA = { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] }

// The JSON line reader of the shared library (zt-workflow-lib).
// (thin wrappers: a module file is loaded alone to read its manifest, so it names no other library at load time)
function zdJsonIn(line) { return ztJsonIn(line) }
function zdLastJson(text) { return ztLastJson(text) }

// --- Tree Lock: the owner's refusal is a planning error, not a provider fault (#140, R326/R337) ------
// A write into a tree the owner locked is refused by the storage gate with its typed `card_locked`
// refusal (a preflight prints `{ok:false, code:"card_locked", …}`; the archive's own MCP refusal
// prints `{error:"card_locked", …}`). The stage reads that JSON, never an English sentence (R338):
// it stops with the refusal as its error, keeps the drafts and the receipts it already committed,
// and never retries the refused write or asks for another Access (R326).
// S6: the one Lock reader is the shared library's (zt-workflow-lib); these names keep the stage modules' calls.
const ZD_LOCK = 'card_locked'
function zdLockRefusal(text) { return ztLockRefusal(text) }
function zdLockSaid(refusal) { return ztLockSaid(refusal) }
function zdLockStop(where, refusal) { return ztLockStop(where, refusal) }

// The DeepSeek source is frozen at the official one (the user's rule, 2026-10-20; S1): no probe, no argument.
function zdDefaultModel() {
  return { source: 'official', model: ZD_DSH_SOURCES.official }
}

function zdMake(ctx, p) {
  const A = p.args || {}
  const MODE = p.mode || 'full'
  const KEY = (p && p.key) || A.citationKey
  const OUT = A.out
  const profile = p.profile || {}
  const SCRIPTS = ZD_SCRIPTS
  const DSH_MODEL = p.model || zdDefaultModel().model
  const DIGEST_PREFIX = `${OUT}\\${profile.digestPrefix || 'shared-prefix.md'}`
  const LINK_PREFIX = `${OUT}\\${profile.linkPrefix || 'link-prefix.md'}`
  // Extra instruction files of the model compiler (math: -BriefDoc, -FillDoc, -ReviewDoc), in the order given.
  const COMPILER_DOCS = Object.entries(profile.compilerDocs || {}).map(([k, f]) => ` -${k} "${OUT}\\${f}"`).join('')
  const SKIP_FORMS = profile.skipForms || []
  const PER_MANAGER = ZD_PER_MANAGER_MAX
  const raised = []
  const atLeastHigh = (value, name) => {
    if (value === 'high' || value === 'max') return value
    if (value) raised.push(`${name} ${value} → high`)
    return 'high'
  }
  const DSH_EFFORT = atLeastHigh(A.dshEffort, 'dshEffort')
  const MODELS = A.models !== false
  const ARCH_DOC = `${SCRIPTS}\\model-architect.md`
  const ARCH_SUPPLEMENT = profile.architectSupplement ? `${SCRIPTS}\\${profile.architectSupplement.file}` : null
  const COMPILER = zdTool('model-compiler', SCRIPTS)
  const MODEL_WORK = `${OUT}\\models`
  const PER_SONNET = Math.max(1, A.perSonnet || 8)
  const DSH_PER_BATCH = Math.min(ZD_PER_MANAGER_MAX, Math.max(1, A.dshPerBatch || 8))
  const REBUILD = !!A.rebuild
  const LEVEL = ['concept', 'formal'].includes(A.level) ? A.level : null
  const PER_CONCEPT = Math.max(1, A.perConcept || 10)
  const BRIEF_EFFORT = atLeastHigh(A.briefEffort, 'briefEffort')
  const DERIVE_EFFORT = atLeastHigh(A.deriveEffort, 'deriveEffort')
  const REVIEW_EFFORT = atLeastHigh(A.reviewEffort, 'reviewEffort')
  const REVIEW_ROUNDS = Math.max(0, A.reviewRounds ?? 2)
  const LIMITS = `-MaxSentences ${A.maxSentences || 12} -MaxChars ${A.maxPartChars || 400} -MaxSteps ${A.maxSteps || 6} -MaxExplainSentences ${A.maxExplainSentences || 16} -MaxExplainChars ${A.maxExplainChars || 600}`
  const DSH_SKILLS = ['zettelkasten', 'asd-ste100-zh']
  const FILL_SKILLS = ['asd-ste100-zh']
  const DSH_TOOLS = 'select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_prompt,mcp__deepseek__deepseek_delete'
  const DRIVE_TOOLS = `${DSH_TOOLS},mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_remove_workspace`
  const CARD_COMPILER = zdTool('card-compiler', SCRIPTS)
  const CARD_FIX_ROUNDS = Math.max(0, A.cardFixRounds ?? 1)
  // R297: the deterministic Idea sample of the one whole-book card cycle (modes cards and full).
  // book-cards-finish requires the deterministic Idea sample, so every mode that runs the whole-book cycle samples
  // (grossman2002integration trial 19: a resumed run's cycle sampled nothing and its finish was refused).
  const IDEA_SAMPLE = Math.max(0, Math.min(1, A.ideaSample ?? ((MODE === 'full' || MODE === 'cards' || MODE === 'resume') ? 0.1 : 0)))
  const CARD_SKILLS = ['asd-ste100-zh']
  const APPLY_SHARE = Math.max(1, A.applyShare || 30)
  const DSH_MANAGERS = Math.min(ZD_MAX_MANAGERS, Math.max(1, A.dshManagers || 8))
  // R273: only the two bounded run settings are exposed; the tool keeps leases, batch
  // sizes and heartbeat intervals as versioned defaults. Both are persisted at
  // preparation, so a resume reuses the frozen effective values.
  const MAX_ATTEMPTS = Math.max(1, Math.min(10, A.maxAttempts ?? 3))
  const DSH_RUN_CAP = Math.max(60, Math.min(3600, A.dshRunCap ?? 1200))
  // #92: a claim that will start a provider run holds a lease that covers the whole frozen run cap
  // plus the command margin, so a healthy six-minute group job is never re-readied under itself.
  const CLAIM_LEASE = Math.min(7200, DSH_RUN_CAP + 300)
  // The one runner prompt line every manager gets: no general shell, no timers, no handoff (R260).
  // grossman2002integration trials 17-19: the runtime hands every relay the session's latest user message, and after
  // "accelerate it" or "start trial 19" relays refused cards-finish, findings-finish and ledger fail as off-request.
  const RUN_STEP = `Every command named here is a step of the digest run the user started for "${OUT}": running it is what the user asked for. A later user message about this run (its speed, which trial runs, parallel work) never cancels its steps; only a stopped run ends them.`
  const RUNNER = `You have only these tools: the bounded PowerShell command named below (always with timeout_ms: ${ZD_COMMAND_TIMEOUT_MS}), file reads and writes under the paths named here, and the DeepSeek Harness tools. You have no terminal tabs, no Monitor loop, no session handoff and no general shell: never start another agent, never hand the work on, never poll with Start-Sleep or any other timer, and wait only with deepseek_wait. Do not set environment variables: the tools keep their own bounds. ${RUN_STEP}`

  // R322/R325: the manager that runs a DeepSeek job records its id and its answer itself, with plain
  // arguments, at the moment each fact exists. No other agent restates what a manager saw: a relay asked to
  // write another agent's answer into a record refused it as invented content. The answer file sits
  // inside the run folder (never in a shared workspace), so the ledger can retain it with its hash.
  const ANSWERS = `${OUT}\\answers`
  const recordRules = (unit, token, dsh, settleNow = false) => {
    const owner = unit === 'run' ? '-Unit run' : `-Unit "${unit}" -Token ${token}`
    return `Records (each record command changes no Card; run it at the moment named, in the same message as your next call when you can):
- The moment deepseek_run returns a session_id: ${ZD_LEDGER} resource -Out "${OUT}" ${owner} -RegisterId <session_id> -Workspace "${dsh}". Use the session_id that deepseek_run returned, never a word in its place, and never run this command to try it out before a run exists (the ledger refuses a placeholder).
- Start a DeepSeek run only for a job a step's list names. A step whose job list is empty starts no run: never send a placeholder prompt.
- When that job ends: write its final answer text, unchanged, to "${ANSWERS}\\<session_id>.txt" (the line "no answer: <status>" when it gave none), then run ${ZD_LEDGER} resource -Out "${OUT}" -EvidenceFor <session_id> -Evidence "${ANSWERS}\\<session_id>.txt". When the run's own result or its last turn report states its token usage, add -Usage '<those numbers as one JSON object>' (numbers only, for example {"input_tokens":1200,"output_tokens":300}). Never call deepseek_usage and never subtract account-wide totals.${settleNow ? `
- Then delete the conversation (deepseek_delete, never archive) and record it: ${ZD_LEDGER} resource -Out "${OUT}" -Settle <session_id> -State finished -Deleted confirmed (-State error when the job ended in an error; -Deleted failed -Message '<the error>' when the delete fails).` : `
- Do not close, archive, or delete the conversation: later rounds may continue it, and the run's cleanup deletes it.`}
- A record command that fails does not stop the work: put registered/evidence false on that job and go on.`
  }

  // Every agent through the blueprint's roles: Haiku is "manager", Sonnet "architect", Opus "organizer"; a
  // one-command relay asks for "relay" (o.role).
  const zAgent = (prompt, o) => ctx.agent(prompt, {
    role: o.role || (o.model === 'sonnet' ? 'architect' : o.model === 'opus' ? 'organizer' : 'manager'),
    label: o.label, phase: o.phase, schema: o.schema,
  })
  // One relay (the zt-relay agent type, the blueprint's role relay) runs one script command and returns its last JSON
  // line (or null). A gate prints its JSON and then exits 3 while the layer still has open work, so a caller that
  // asks (o.always) still reads a typed result. R326/R337: a command whose answer carries the owner's typed refusal
  // stops the stage here (the shared ztRelay); a read that merely names a retained fault keeps going.
  const runScript = (label, phaseName, command, note = '', o = {}) => ztRelay(ctx, {
    command, note, label, phase: phaseName, role: 'relay', always: !!o.always, run: `the digest run the user started for "${OUT}"`,
  })

  const groupsOf = (count) => Array.from({ length: count || 0 }, (_, i) => {
    const n = i + 1
    const nn = String(n).padStart(2, '0')
    return { n, nn, file: `${OUT}\\groups\\g${nn}.md`, snapshot: `${OUT}\\snapshot-g${nn}.md`, link: `${OUT}\\link-g${nn}.md` }
  })
  const toBlocks = (list, size = PER_MANAGER) => {
    const out = []
    for (let i = 0; i < list.length; i += size) out.push(list.slice(i, i + size))
    return out
  }

  // --- The compiler-step driver (drive): one Haiku agent runs a step, managers run its DeepSeek jobs ---
  const DRIVE = {
    type: 'object',
    properties: {
      ok: { type: 'boolean' }, error: { type: 'string' }, output: { type: 'string' },
      results: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' }, registered: { type: 'boolean' }, evidence: { type: 'boolean' } }, required: ['id', 'status'] } },
      deleted: { type: 'number' },
    },
    required: ['ok', 'results'],
  }
  // One Haiku driver runs one compiler step and the DeepSeek jobs it prints. o: command, dsh (the DSH
  // workspace), skills, effort, sessions (continued jobs), cleanup (the last round), phase, writes, keep.
  const driveAgent = (label, o, part) => {
    const table = Object.entries(o.sessions || {}).map(([job, session]) => `${job} ${session}`).join('\n')
    const lead = `You drive DeepSeek Harness (DSH) jobs for a ZET compiler script. You do not read the book and you do not write Cards: a script and DSH do. Keep your messages short.
${RUNNER}
`
    const head = `${lead}Load the tools with one ToolSearch call: query "${DRIVE_TOOLS}".
`
    if (part === 'command') {
      // S1 (37): the step command runs through the relay, like every other command; the command itself saves its
      // last line as the step's job list, which the job managers read (a relay never retypes a job list).
      const command = o.jobsFile ? `${o.command} | Tee-Object -Variable zdLines; $zdExit = $LASTEXITCODE; [void][System.IO.Directory]::CreateDirectory("${OUT}\\jobs"); ($zdLines | Select-Object -Last 1) | Set-Content -LiteralPath "${o.jobsFile}" -Encoding utf8; if ($zdExit) { exit $zdExit }` : o.command
      return ztRelay(ctx, { command, label, phase: o.phase || 'Models', role: 'relay', lockStops: false, run: `the digest run the user started for "${OUT}"` })
        .then(({ run }) => ({ ok: !!(run && run.ok), output: (run && run.output) || '', error: run && !run.ok ? run.output : undefined, results: [] }))
    }
    const jobs = o.jobs || []
    return dsmpSlot(() => zAgent(`${head}
${o.jobsFile ? `Run the jobs with these ids: ${JSON.stringify(jobs.map((j) => j.id))}. Other managers run the other jobs of this step at the same time; run only these.
Their exact files, prompt, continue and fallback are in the job list the step command saved: read "${o.jobsFile}" once (one JSON line) and take each job from its "jobs" array by id, exactly as written. Never retype a path. A compact list (it has "lead" and "prompts") expands each entry: kind "fix" → files [lead.fix, dir\\file], prompt prompts.fix; kind "orphans" → files [lead.orphans, dir\\file], prompt prompts.orphans; kind "follow" → files [dir\\file], prompt prompts.follow, continue = its "continue", fallback [lead.fix, dir\\file].` : `Run every job below. A job has id, files, prompt, and sometimes continue and fallback. Other managers run the other jobs of this step at the same time; run only these.
${JSON.stringify(jobs)}`}
- No continue: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, model "${DSH_MODEL}", reasoning_effort "${o.effort}", wait_seconds 0.
- continue: find the session of the job id named by continue in this table (job id, then session_id):
${table || '(empty)'}
  With a session: deepseek_prompt with that session_id, prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, reasoning_effort "${o.effort}", wait_seconds 0. Without one: deepseek_run as above, with prompt_files = fallback.
Keep at most ${DSH_PER_BATCH} jobs running at a time; start the next job when one ends. Start independent jobs in one message.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 600).
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: the Bridge compiler (${ZD_DIGEST} bridge, or bridge-compiler.ps1) with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}), or the check zt-check (read-only); a read-only self-check of a draft: the card, event, or finding compiler (${ZD_DIGEST} card, event, or finding, or card-compiler.ps1, event-compiler.ps1, finding-compiler.ps1) with -Step precheck, the model compiler (${ZD_DIGEST} model, or model-compiler.ps1) with -Step explain-check, or the book skeleton (${ZD_DIGEST} skeleton, or book-skeleton.ps1) with -Step apply and without -Apply; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
${(() => { const owner = resourceOwner(o); return owner === 'run' ? recordRules('run', 1, o.dsh) : recordRules(owner.unit, owner.token, o.dsh) })()}
Return ok=true, output="", and results: one entry per job with id, session_id, status ("done" or "failed"), answer, registered and evidence (true when those record commands succeeded).`,
      { label, phase: o.phase || 'Models', model: 'haiku', schema: DRIVE }))
  }
  // A step's jobs. A compact list (card-compiler.ps1 -Compact) expands to { id, files, prompt, continue?, fallback? }.
  const expandJobs = (parsed) => {
    const list = (parsed && Array.isArray(parsed.jobs)) ? parsed.jobs : []
    if (!parsed || !parsed.lead || !parsed.prompts) return list
    const at = (name) => `${parsed.dir}\\${name}`
    return list.map(j => {
      if (j.files) return j
      if (j.kind === 'follow') return { id: j.id, continue: j.continue, files: [at(j.file)], fallback: [parsed.lead.fix, at(j.file)], prompt: parsed.prompts.follow }
      const lead = j.kind === 'orphans' ? parsed.lead.orphans : parsed.lead.fix
      return { id: j.id, files: [lead, at(j.file)], prompt: parsed.prompts[j.kind] }
    })
  }
  // Runs one compiler step: the command, its jobs spread over up to DSH_MANAGERS managers (one per
  // DSH_PER_BATCH jobs, all under dsmpSlot). The run ids are recorded at once; the run-level cleanup
  // later deletes exactly those ids (R322/R323).
  // A drive step's job list is saved by the command itself (Tee-Object → Set-Content), one file per step, so
  // the managers read the exact files and prompts: a relay that retyped a long job list dropped part of a
  // path (grossman2002integration trial 2: a derivation job named a folder that does not exist).
  let jobsSeq = 0
  const drive = async (label, o0) => {
    const o = { ...o0, jobsFile: `${OUT}\\jobs\\${String(label).replace(/[^A-Za-z0-9._-]+/g, '-').slice(0, 60)}-${++jobsSeq}.json` }
    const step = await driveAgent(label, o, 'command')
    if (!step || !step.ok) {
      // R326/R337: the compiler step the owner's Lock refused stops the stage with its own refusal.
      const lock = zdLockRefusal((step && (step.error || step.output)) || '')
      if (lock) throw zdLockStop(label, lock)
      return { ok: false, error: (step && step.error) || 'no report from the step', output: (step && step.output) || '', results: [] }
    }
    const parsed = zdLastJson(step.output)
    const jobs = expandJobs(parsed)
    let results = []
    if (jobs.length) {
      const shares = []
      for (let i = 0; i < jobs.length; i += DSH_PER_BATCH) shares.push(jobs.slice(i, i + DSH_PER_BATCH))
      const managers = shares.length
      const done = new Array(managers)
      let next = 0
      const lane = async () => {
        while (next < managers) {
          const i = next++
          done[i] = await driveAgent(managers > 1 ? `${label} [${i + 1}/${managers}]` : label, { ...o, jobs: shares[i] }, 'jobs')
            .catch(error => ({ ok: false, error: String(error), results: [] }))
        }
      }
      await Promise.all(Array.from({ length: Math.min(DSH_MANAGERS, managers) }, lane))
      results = done.flatMap((d, i) => {
        const got = (d && d.results) || []
        const missing = shares[i].filter(j => !got.some(r => r.id === j.id)).map(j => ({ id: j.id, status: 'failed', answer: '' }))
        return [...got, ...missing]
      })
      const failedManagers = done.filter(d => !d || !d.ok).length
      if (failedManagers) log(`${label}: ${failedManagers} of ${managers} managers reported a failure.`)
    }
    // R322: each job manager recorded its own ids and answers; name the jobs whose records failed.
    const unrecorded = results.filter((r) => r.session_id && r.registered === false).map((r) => r.id)
    if (unrecorded.length) log(`${label}: ${unrecorded.length} job id(s) not recorded by their manager: ${unrecorded.join(', ')}`)
    return { ok: true, output: step.output, results, deleted: 0 }
  }
  const answerLines = (driven) => ((driven && driven.results) || []).flatMap(r => String(r.answer || '').split('\n').map(zdJsonIn).filter(Boolean))

  // --- The ready-work driver (9.3, R250-R258): claim a bounded block, run each unit's chain in one manager ----
  // One manager per chain: it runs the chain's commands and DeepSeek jobs in its own conversation (R250/R251).
  // The chain is declared here, frozen in the blueprint's module versions, never improvised at run time.
  const CHAIN = {
    type: 'object',
    properties: {
      steps: {
        type: 'array',
        items: {
          type: 'object',
          properties: { name: { type: 'string' }, ok: { type: 'boolean' }, exit: { type: 'number' }, output: { type: 'string' }, error: { type: 'string' } },
          required: ['name', 'ok'],
        },
      },
      results: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            id: { type: 'string' }, step: { type: 'string' }, session_id: { type: 'string' },
            status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' },
            registered: { type: 'boolean' }, evidence: { type: 'boolean' },
          },
          required: ['id', 'status'],
        },
      },
    },
    required: ['steps', 'results'],
  }
  // The steps a chain runs: { name, command, jobs?, repeat? }. `jobs` collects the DeepSeek jobs the step
  // prints (the default). `repeat` runs the same command again while its JSON still reports work, at most
  // `max` times; that is the bounded correction path, not an unbounded loop. A command may hold
  // {{unit}}, {{token}}, {{group}} and {{draft}}; they come from what the chain has already read: the claim
  // step's `units[0]` (its `id`, `token` and the number in `gNNN`) and a start step's `draft`.
  const chainPrompt = (label, o) => {
    const steps = o.steps || []
    // #92: a chain that holds a claim keeps it current while its jobs run, so a job that outlives
    // the base lease still finishes under its own token.
    const claimedUnit = o.unit ? `"${o.unit}"` : '{{unit}}'
    const claimedToken = o.unit ? String(o.token) : '{{token}}'
    const holdsClaim = !!o.unit || steps.some(s => s.claim)
    const renewLine = holdsClaim
      ? `\n- While any job of this chain runs, keep the claim current: after every deepseek_wait that answers "running", run this command in the same message (it changes no Card; it renews the claim this chain holds):\n   ${ZD_LEDGER} renew -Out "${OUT}" -Unit ${claimedUnit} -Token ${claimedToken}`
      : ''
    const listed = steps.map((s, i) => {
      const lines = [`${i + 1}. ${s.name}: run this command in the foreground with timeout_ms: ${ZD_COMMAND_TIMEOUT_MS} and wait for it, then read its last output line (compact JSON, it may be tens of kilobytes; keep it whole):`, `   ${s.command}`]
      if (s.claim) lines.push(`   This is the claim step. If its JSON has an empty "units" array, stop the chain here: run no later step and no job, and return the steps you ran. Otherwise remember its "units"[0].id, "units"[0].token and the group number in that id (the digits in gNNN) for the commands below.`)
      if (s.jobs) lines.push(`   When its JSON has a nonempty "jobs" array, run those jobs before the next step (the job rules below). When it names a "job" file instead (a group start), read that file and run the jobs it declares (its "jobs" entries name files and prompt; its "expected_jobs" must equal the number you run).`)
      else lines.push(`   It starts no DeepSeek job by itself.`)
      if (s.repeat) lines.push(`   Repeat this same command while its JSON still reports ${s.repeat.path} ${s.repeat.op || '>'} ${s.repeat.value}, at most ${s.repeat.max || 100} times, and record the last JSON in this step's output.`)
      if (s.optional) lines.push(`   A failure here is recorded and the chain goes on with the next step.`)
      if (s.always) lines.push(`   Run this step even when an earlier step failed or printed no JSON.`)
      return lines.join('\n')
    }).join('\n')
    return `You run one compiler chain of a ZET book digest: ${label}. A script and DeepSeek Harness (DSH) do the work; you only run the bounded commands and the DSH jobs they print. Keep your messages short.
${RUNNER}
Load the DSH tools with one ToolSearch call: query "${DRIVE_TOOLS}".
The chain is fixed. Run every step in this order and nothing else. A command may contain {{unit}}, {{token}}, {{group}} or {{draft}}: replace each with the value the chain has already read (the claim's unit id, its token, the digits of that id's gNNN, and a start step's "draft").
CHAIN PLAN: ${JSON.stringify({ dsh: o.dsh, steps: steps.map(s => ({ name: s.name, command: s.command, claim: !!s.claim, jobs: !!s.jobs, repeat: s.repeat || null, optional: !!s.optional, ...(s.always ? { always: true } : {}) })) })}
${listed}
Jobs (for every step that prints them):
- A compact list (the card compiler's -Compact) is { dir, lead: { fix, orphans }, prompts: { fix, orphans, follow }, jobs: [{ id, kind, file, continue? }] }. Expand each entry: kind "fix" → files [lead.fix, dir\file], prompt prompts.fix; kind "orphans" → files [lead.orphans, dir\file], prompt prompts.orphans; kind "follow" → files [dir\file], prompt prompts.follow, and continue = its "continue" id. Any other entry already names files and prompt.
- No continue: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills || [])}, model "${DSH_MODEL}", reasoning_effort "${o.effort || DSH_EFFORT}", wait_seconds 0.
- continue: reuse the session_id of the earlier job of this chain with that id (from this chain's own results); without one, deepseek_run with prompt_files = fallback (or files when it names none).
Keep at most ${DSH_PER_BATCH} jobs running at a time; start independent jobs in one message and start the next job when one ends.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 90), then start the next waiting job and keep polling; never wait longer than 90 seconds at once.${renewLine}
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: the Bridge compiler (${ZD_DIGEST} bridge, or bridge-compiler.ps1) with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}), or the check zt-check (read-only); a read-only self-check of a draft: the card, event, or finding compiler (${ZD_DIGEST} card, event, or finding, or card-compiler.ps1, event-compiler.ps1, finding-compiler.ps1) with -Step precheck, the model compiler (${ZD_DIGEST} model, or model-compiler.ps1) with -Step explain-check, or the book skeleton (${ZD_DIGEST} skeleton, or book-skeleton.ps1) with -Step apply and without -Apply; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
${o.unit ? recordRules(o.unit, o.token, o.dsh) : steps.some((s) => s.claim) ? recordRules('{{unit}}', '{{token}}', o.dsh) : recordRules('run', 1, o.dsh)}
Return steps: one entry per step with name, ok (true when it exited without an error), exit (its exit code when known), output (its last line unchanged) or error (the failure text); and results: one entry per job with id, step, session_id, status ("done" or "failed"), answer, registered and evidence (true when those record commands succeeded).`
  }
  // Runs one declared chain in ONE manager. o: label, dsh, skills, effort, phase, writes, steps, sessions, cleanup, keep.
  const chain = async (label, o) => {
    const run = await dsmpSlot(() => zAgent(chainPrompt(label, o), {
      label, phase: o.phase || 'Models', model: 'haiku', schema: CHAIN,
    }))
    const driven = run || { steps: [], results: [] }
    const steps = Array.isArray(driven.steps) ? driven.steps : []
    const results = Array.isArray(driven.results) ? driven.results : []
    // R326/R337: a chain step whose command the owner's Lock refused stops the stage with the refusal.
    for (const step of steps) {
      if (step && step.ok !== false) continue
      const lock = zdLockRefusal(step && (step.error || step.output))
      if (lock) {
        log(`${label}: ${step.name} was refused by the owner's Lock; the stage stops and keeps its drafts and receipts.`)
        throw zdLockStop(`${label}: ${step.name}`, lock)
      }
    }
    // R322: the chain manager recorded its own ids against the unit that owns them (the claim step's unit,
    // the chain's given unit, or the run). Name the jobs whose records failed.
    const claimStep = (o.steps || []).find((s) => s.claim)
    const claimRun = claimStep ? steps.find((s) => s.name === claimStep.name) : null
    const claimed = claimRun ? ((zdLastJson(claimRun.output) || {}).units || []) : []
    const owner = claimed[0] && claimed[0].id ? { unit: claimed[0].id, token: claimed[0].token } : resourceOwner(o)
    const unrecorded = results.filter((r) => r.session_id && r.registered === false).map((r) => r.id)
    if (unrecorded.length) log(`${label}: ${unrecorded.length} job id(s) not recorded by their manager: ${unrecorded.join(', ')}`)
    return { ok: steps.every(s => s.ok), steps, results, deleted: 0, raw: driven, unit: owner === 'run' ? 'run' : owner.unit }
  }

  // --- The layer driver: script steps run by DeepSeek drive jobs (#175) ------------------------------------------
  // One layer's block is one deterministic chain (`scripts/book-digest/layer-steps.ps1`): `start` claims the
  // block and lists the DeepSeek jobs it printed, the block's content jobs run in Haiku managers, and `finish`
  // records every held unit's checked completion, faults the units that cannot finish, runs the layer's own
  // apply and report commands and the ledger gate. Each step is ONE script call run by a DeepSeek drive job
  // (zt-workflow-lib's shared ztDrive); the module relays no command and decides nothing per unit.
  const LAYER_STEPS = p.layerScript || `${SCRIPTS}\\layer-steps.ps1`
  // A layer claims its block with the longest lease the ledger takes: the block's content jobs are capped by
  // the provider run cap, and the explain layer's second content round runs after the first.
  const LAYER_LEASE = 7200
  // One step of the layer script, run by one DeepSeek drive job. A failed step (`ok:false`) is returned for
  // the module to stop with; the owner's typed refusal stops the stage here.
  const driveStep = async (label, step, o = {}) => {
    const parts = [`pwsh -NoProfile -File "${LAYER_STEPS}"`, `-Step ${step}`, `-Out "${OUT}"`, `-Layer ${o.layer}`]
    if (o.owner) parts.push(`-Owner "${o.owner}"`)
    if (o.take) parts.push(`-Take ${o.take}`)
    if (o.wait) parts.push(`-Wait ${o.wait}`)
    if (o.lease) parts.push(`-Lease ${o.lease}`)
    if (o.maxParts) parts.push(`-MaxParts ${o.maxParts}`)
    if (o.limits) parts.push(`-Limits ${psQuote(o.limits)}`)
    const json = await ztDrive(ctx, { label, command: parts.join(' '), out: OUT, cwd: o.cwd || OUT, phase: o.phase, ledger: ZD_LEDGER, deepseek: o.deepseek })
    const lock = ztLockRefusal(JSON.stringify(json)) || ((json.code === ZD_LOCK || json.error === ZD_LOCK) ? json : null)
    if (lock) throw ztLockStop(label, lock.refusal || lock)
    if (json.ok !== true) return { ok: false, error: String(json.error || `the ${step} step failed`).slice(0, 800), gate: json.gate || null, raw: json }
    return json
  }
  // The jobs one step listed, run by Haiku managers: one manager per up to DSH_PER_BATCH jobs, in parallel. Each
  // entry names its own files, prompt, unit and token; every manager records its conversations and deletes them
  // (or, when the layer continues them in a second round, leaves them to the run cleanup).
  const CONTENT = {
    type: 'object',
    properties: {
      ok: { type: 'boolean' }, deleted: { type: 'number' }, not_deleted: { type: 'array', items: { type: 'string' } },
      results: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] },
            answer: { type: 'string' }, registered: { type: 'boolean' }, evidence: { type: 'boolean' }, deleted: { type: 'boolean' },
          },
          required: ['id', 'status'],
        },
      },
    },
    required: ['ok', 'results'],
  }
  const contentPrompt = (label, o) => `You are a DeepSeek subagent manager for ZET ${o.what} (${label}). You start DeepSeek agents, wait for them, collect their answers, record each conversation and delete it. You never write content and you never run a content command yourself. Keep your messages short.
${RUNNER}
Load the DSH tools with one ToolSearch call: query "${DRIVE_TOOLS}".
JOBS: read the JSON array in "${o.file}" (Read tool) and run its entries ${o.from} through ${o.to - 1} (0-based), and only those. Each entry has id, files, prompt, unit and token.
- Start each job: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills || [])}, model "${DSH_MODEL}", reasoning_effort "${o.effort || DSH_EFFORT}", wait_seconds 0.${o.table === undefined ? '' : `
- A job with "continue": find the session of the job id it names in this table (job id, then session_id):
${o.table || '(empty)'}
  With a session: deepseek_prompt with that session_id, prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills || [])}, reasoning_effort "${o.effort || DSH_EFFORT}", wait_seconds 0. Without one: deepseek_run as above, with prompt_files = the job's fallback.`}
Keep at most ${DSH_PER_BATCH} jobs running at a time; start the next job when one ends. Start independent jobs in one message.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 600).
- "needs_response": approve (decision "allow") a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1; ${ZD_NO_OPEN}), the check zt-check (read-only), a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}, and writing JSON files under "${o.writes || o.dsh}". Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
In the record commands below, <unit_id> and <token> are the entry's own unit and token. Use the session_id that deepseek_run returned, never a word in its place. A step whose job list is empty starts no run: never send a placeholder prompt.
${recordRules('<unit_id>', '<token>', o.dsh, !o.keep)}
Return ok, deleted, not_deleted and results: one entry per job with id, session_id, status ("done" or "failed"), answer, registered, evidence and deleted.`
  // One manager's share of a step's jobs. A manager that answers no report leaves its jobs undelivered: the
  // finish step faults the units they belonged to.
  const runJobs = async (label, file, count, o) => {
    const managers = []
    for (let from = 0; from < Number(count || 0); from += DSH_PER_BATCH) {
      const to = Math.min(Number(count), from + DSH_PER_BATCH)
      const at = `${label} ${from + 1}-${to}`
      managers.push(dsmpSlot(() => zAgent(contentPrompt(at, { ...o, file, from, to }), { label: at, phase: o.phase || 'Digest', model: 'haiku', schema: CONTENT }))
        .then((r) => (r && r.results) || [])
        .catch((error) => { log(`${label}: a content manager answered no report (${String((error && error.message) || error).slice(0, 160)}); its jobs stay unfinished.`); return [] }))
    }
    return (await Promise.all(managers)).flat()
  }
  // The session table a second content round continues: the plan round's job ids and session ids.
  const sessionTable = (results) => (results || []).filter((r) => r && r.session_id).map((r) => `${r.id} ${r.session_id}`).join('\n')
  // Runs one layer in blocks until it offers no ready unit (at most `rounds` claims that found nothing). o:
  // layer, take, rounds, wait, dsh, writes, skills, effort, phase, what (the manager's subject), jobs (false:
  // the layer lists no content job), apply (a second content round between start and finish), maxParts, limits,
  // deepseek (the deletion tally), onFinish (a module's own reading of the finish line).
  const layerRun = async (name, o) => {
    const layer = o.layer || name
    const take = Math.max(1, Math.min(64, o.take || 8))
    const rounds = Math.max(1, Math.min(12, o.rounds || MAX_ATTEMPTS))
    const MAX_BLOCKS = 200
    const dispositions = []
    const finishes = []
    let claimed = 0
    let idle = 0
    let gate = null
    for (let block = 1; block <= MAX_BLOCKS && idle < rounds; block++) {
      const label = `${name} block ${block}`
      const started = await driveStep(`${label}: start`, 'start', {
        layer, take, owner: label, wait: o.wait, lease: LAYER_LEASE, phase: o.phase, cwd: o.dsh, maxParts: o.maxParts, limits: o.limits, deepseek: o.deepseek,
      })
      if (!started.ok) return { status: 'failed', layer, error: started.error, dispositions, claimed, gate: started.gate, finishes }
      gate = started.gate || gate
      if (o.onStart) o.onStart(started, label)
      const units = Number(started.units || 0)
      if (!units) {
        idle += 1
        if (!gate || gate.complete === true) break
        if (o.wait === undefined) break
        continue
      }
      idle = 0
      claimed += units
      // The Cards layer's work is the checked finish the start step already ran.
      if (o.jobs === false) {
        for (const r of started.results || []) dispositions.push(r)
        continue
      }
      let sessions = null
      if (Number(started.jobs || 0)) sessions = await runJobs(`${label}: write`, started.file, started.jobs, o)
      if (o.apply) {
        const applied = await driveStep(`${label}: apply`, 'apply', { layer, phase: o.phase, cwd: o.dsh, limits: o.limits })
        if (!applied.ok) return { status: 'failed', layer, error: applied.error, dispositions, claimed, gate, finishes }
        if (o.onApply) o.onApply(applied, label, started)
        if (Number(applied.jobs || 0)) await runJobs(`${label}: apply`, applied.file, applied.jobs, { ...o, table: sessionTable(sessions) })
      }
      const finished = await driveStep(`${label}: finish`, 'finish', { layer, phase: o.phase, cwd: o.dsh, deepseek: o.deepseek })
      if (!finished.ok) return { status: 'failed', layer, error: finished.error, dispositions, claimed, gate: finished.gate || gate, finishes }
      finishes.push(finished)
      for (const r of finished.results || []) dispositions.push(r)
      gate = finished.gate || gate
      if (o.onFinish) o.onFinish(finished, label, started)
    }
    const final = gate || { complete: false, counts: {} }
    const failed = dispositions.filter((d) => d.status === 'failed').length
    const open = Number((final.counts && final.counts.open) || 0)
    return { status: final.complete === true ? 'complete' : failed && !open ? 'partial' : 'active', layer, gate: final, dispositions, claimed, finishes }
  }
  const stepOf = (driven, name) => ((driven && driven.steps) || []).find(s => s.name === name) || null
  const jsonOf = (driven, name) => zdLastJson((stepOf(driven, name) || {}).output)
  // R271/R272: one compact JSON object on the last line, with the expected job count, the unit and the
  // input hash checked before a run trusts it. `jobs: []` is a success only with an explicit no-op reason.
  const resultLine = (driven, name, o = {}) => {
    const step = stepOf(driven, name)
    const parsed = zdLastJson(step && step.output)
    const faults = []
    if (!step) faults.push(`the chain has no step ${name}`)
    else if (!step.ok) faults.push(`${name} failed: ${step.error || 'nonzero exit'}`)
    else if (!parsed) faults.push(`${name} printed no JSON line`)
    if (parsed && o.unit && (o.unit !== parsed.unit) && (parsed.unit !== undefined)) faults.push(`${name} answered unit ${parsed.unit}, not ${o.unit}`)
    if (parsed && o.token !== undefined && parsed.token !== undefined && parsed.token !== o.token) faults.push(`${name} answered token ${parsed.token}, not ${o.token}`)
    if (parsed && o.inputHash && typeof parsed.input_hash === 'string' && parsed.input_hash !== o.inputHash) faults.push(`${name} answered input ${parsed.input_hash}, not ${o.inputHash}`)
    if (parsed && o.expectedJobs !== undefined) {
      const declared = parsed.expected_jobs
      const jobs = Array.isArray(parsed.jobs) ? parsed.jobs.length : Array.isArray(parsed.results) ? parsed.results.length : null
      if (declared === undefined) faults.push(`${name} did not declare expected_jobs`)
      else if (declared !== o.expectedJobs) faults.push(`${name} declared ${declared} expected jobs, not ${o.expectedJobs}`)
      else if (declared === 0 && jobs !== 0) faults.push(`${name} declared no jobs but printed ${jobs}`)
      if (declared === 0 && !parsed.no_op && !parsed.no_op_reason && !parsed.reason) faults.push(`${name} declared no jobs without a no-op reason`)
    }
    if (parsed && parsed.pending_jobs > 0) faults.push(`${name} left ${parsed.pending_jobs} jobs pending`)
    return { parsed, faults }
  }
  // The phase a ledger-only call is filed under: a declared stage title of the base blueprint.
  const LEDGER_PHASE = 'Digest'
  // The phase of the run-level cleanup: the base declares one `Cleanup` stage.
  const CLEANUP_PHASE = 'Cleanup'
  // The ledger adapter: every call is one bounded command in a manager's own conversation. The ledger owns
  // readiness, attempts and completion evidence; a module never reads a clock or a file to decide it (R270).
  const ledgerCommand = (args) => `${ZD_LEDGER} ${args}`
  // Claims ready units of one layer in a bounded block. `wait` is the tool-side readiness wait (seconds).
  // The claim requests a lease that covers the frozen provider-run cap (#92).
  // A claim's units are the ledger's word: when the relayed line lost its units array (grossman2002integration
  // trial 3: "(4 units claimed: cards/g005@1, ...)", and the driver faulted an undefined unit), the driver
  // reads the units this claim holds from the ledger by its owner label (`ledger status -Held`).
  // A claim is brief (`ledger next -Brief`): each unit keeps its id, token, input hash and lease, and of its
  // inputs only the Model address a model or explain unit names (`header`, `model`). On trial 4 a relay dropped
  // the full 1.3 KB inputs of four model units and the driver finished `-Model 'undefined'`; a model or explain
  // unit that arrives without its address is read from the ledger too.
  const ADDRESSED = /^(models|explain)$/
  const heldUnits = (units, layer = '') => Array.isArray(units) && units.every((u) => u && typeof u.id === 'string' && Number(u.token) > 0
    && (!ADDRESSED.test(layer) || (u.inputs && (typeof u.inputs.header === 'string' || typeof u.inputs.model === 'string'))))
  // The input hash of every unit this run claimed: a fault names the claim's hash (`ledger fail` refuses one
  // without it; it is a claim field, never part of the unit's inputs).
  const HASHES = new Map()
  const remember = (units) => { for (const u of units) if (u && u.id && u.input_hash) HASHES.set(u.id, u.input_hash); return units }
  const hashOf = (unit, inputs) => (inputs && inputs.input_hash) || HASHES.get(unit) || ''
  // A claim command that fails is tried again, three times in all, before the caller hears of it: on
  // ebrill2001modern trial 1 one `ledger next` refused while parallel lanes changed the archive, and the
  // events layer ended with 40 of its 73 units never claimed.
  const CLAIM_TRIES = 3
  const claim = async (label, layer, take, wait = 0) => {
    let out
    for (let attempt = 1; attempt <= CLAIM_TRIES; attempt++) {
      out = await runScript(attempt === 1 ? label : `${label} (try ${attempt})`, LEDGER_PHASE, ledgerCommand(`next -Out "${OUT}" -Layer ${layer} -Take ${take} -Owner "${label}"${wait ? ` -Wait ${wait}` : ''} -Lease ${CLAIM_LEASE} -Brief`))
      if (out.json || (out.run && out.run.ok)) break
      log(`${label}: the claim failed (${String((out.run && out.run.output) || 'no output').slice(0, 200)})${attempt < CLAIM_TRIES ? '; it is tried again' : ''}.`)
    }
    const parsed = out.json
    // A line without a `units` array is unreadable, never an empty claim: a relay replaced it with its own
    // `units_claimed` id list (grossman2002integration trial 17), and the layer ended with four models held.
    if (parsed && Array.isArray(parsed.units) && heldUnits(parsed.units, layer)) return { ok: true, units: remember(parsed.units), layer: parsed.layer, settled: parsed.settled, complete: parsed.complete, counts: parsed.counts }
    if (!parsed && !(out.run && out.run.ok)) return { ok: false, error: (out.run && out.run.output) || 'no output from ledger next', units: [] }
    log(`${label}: the claim line came back without its units; the held units are read from the ledger.`)
    const held = await runScript(`${label}: held units`, LEDGER_PHASE, ledgerCommand(`status -Out "${OUT}" -Held "${label}"`))
    const units = held.json && heldUnits(held.json.units, layer) ? remember(held.json.units) : null
    if (!units) return { ok: false, error: `the claim's held units could not be read: ${(held.run && held.run.output) || 'no output'}`, units: [] }
    return { ok: true, units, layer, settled: parsed ? parsed.settled : undefined, complete: parsed ? parsed.complete : undefined, counts: parsed ? parsed.counts : undefined }
  }
  const gate = async (label, layer) => {
    const out = await runScript(label, LEDGER_PHASE, ledgerCommand(`gate -Out "${OUT}" -Layer ${layer} -Brief`), ' (exit 3 means the layer still has open work)', { always: true })
    return out.json || { ok: false, error: (out.run && out.run.output) || 'no output from ledger gate' }
  }
  // In mode resume a stage whose ledger layers are already complete reuses the retained work instead of redoing it
  // (grossman2002integration trial 20: the resume reran Organize's survey and decisions, which can merge or move Cards
  // the retained run had finished). Outside mode resume it never skips.
  const settledOnResume = async (label, layers) => {
    if (MODE !== 'resume') return false
    for (const name of layers) {
      const g = await gate(`${label}: ${name} gate`, name)
      if (!(g && g.complete === true)) return false
    }
    log(`${label}: the retained run completed its ${layers.join(', ')} work; the resume reuses it.`)
    return true
  }
  // S5: one fault of a claimed unit, classified by the ledger (fault_class of its code; a fault without a code takes
  // the ledger's one reading of its cause). Its answer is the unit's state after the fault: 'failed' when the unit
  // stays failed (a content fault, or its attempts are spent), else 'retry'.
  const failUnit = async (label, unit, token, fault) => {
    const out = await runScript(label, LEDGER_PHASE, `${ZD_LEDGER} fail -Out "${OUT}" -Unit "${unit}" -Token ${token} -Fault ${psQuote(JSON.stringify(fault))}`)
    return out.json && out.json.state === 'failed' ? 'failed' : out.json ? 'retry' : (fault.code === 'check_failed' || fault.code === ZD_LOCK ? 'failed' : 'retry')
  }
  // A PowerShell single-quoted literal (the tool's -Fault and -Json values are JSON text).
  function psQuote(text) {
    return `'${String(text).replace(/'/g, "''")}'`
  }
  // The family finish command of one claimed unit: the checked verifier that records the completion.
  const finishOf = async (label, unit, token, extra = {}, o = {}) => {
    const number = /\/g(\d+)@1$/.exec(unit)
    const model = o.model || (o.inputs && (o.inputs.model || o.inputs.header))
    let command = ''
    if (/^digest\//.test(unit)) command = `${ZD_DIGEST} group-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token} -Draft ${psQuote(o.draft || '')}`
    else if (/^cards\//.test(unit)) command = `${ZD_DIGEST} cards-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token}`
    else if (/^events\//.test(unit)) command = `${ZD_DIGEST} events-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token}`
    else if (/^findings\//.test(unit)) command = `${ZD_DIGEST} findings-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token}`
    else if (/^link\//.test(unit)) command = `${ZD_DIGEST} link-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token} -Draft ${psQuote(o.draft || '')}`
    else if (/^models\//.test(unit)) command = `${ZD_DIGEST} model-finish -Out "${OUT}" -Model ${psQuote(model)} -Token ${token}`
    else if (/^explain\//.test(unit)) command = `${ZD_DIGEST} explain-finish -Out "${OUT}" -Model ${psQuote(model)} -Token ${token}`
    else if (/^cards-book\//.test(unit)) command = `${ZD_DIGEST} book-cards-finish -Out "${OUT}" -Work ${psQuote(extra.work || `${OUT}\\cards`)} -Unit "${unit}" -Token ${token}`
    else if (/^terms\//.test(unit)) command = `${ZD_LEDGER} done -Out "${OUT}" -Unit "${unit}" -Token ${token} -Outputs ${psQuote(`${OUT}\\terms\\completion.json`)}`
    else if (/^cleanup\//.test(unit)) command = `${ZD_DIGEST} cleanup-finish -Out "${OUT}" -Unit "${unit}" -Token ${token}`
    else return { ok: false, error: `no checked finish command for ${unit}` }
    if (extra.checkOnly) command += ' -CheckOnly'
    const out = await runScript(label, o.phase || LEDGER_PHASE, command, ' (a checked completion; never retry it with other options)')
    return { ok: !!(out.run && out.run.ok), json: out.json, command, error: out.run && out.run.output }
  }
  // One claimed unit's checked finish. A failure is recorded against that unit's own attempt
  // (`ledger fail`): a transient transport or lease fault schedules the retry, a checked content
  // failure keeps the unit failed and visible. Never a whole-stage rerun (R258).
  const finishOrFault = async (label, unit, token, inputs, extra = {}) => {
    const at = `${label}: ${unit}`
    let got
    try {
      got = await finishOf(at, unit, token, { ...extra, inputs }, { inputs })
    } catch (error) {
      // R326/R337: a checked finish the archive refused because the owner locked the destination keeps
      // the archive's typed refusal as the unit's content fault, then stops the stage.
      if (error && error.lock_refusal) {
        await failUnit(`${at} locked`, unit, token, {
          schema_version: 1, unit, input_hash: hashOf(unit, inputs), code: ZD_LOCK,
          cause: `${zdLockSaid(error.lock_refusal)} ${JSON.stringify(error.lock_refusal)}`.slice(0, 800),
        })
        throw error
      }
      throw error
    }
    const done = got.ok && got.json && (got.json.state === 'done' || got.json.complete === true)
    if (done) return { unit, status: got.json.status === 'partial' ? 'partial' : 'done', result: got.json }
    // The ledger decides every finish that is not plainly done before any fault: on trial 5 a relay cut the end
    // off a 2.5 KB cards completion that the ledger had recorded done, and on trial 10 a relay rewrote the end of
    // one as prose, so the line still held a nested object (a file record, no state) and the driver faulted a
    // done unit; the ledger refused that fault as stale.
    let ledgerHash = ''
    if (!done) {
      const read = await runScript(`${at}: state`, LEDGER_PHASE, ledgerCommand(`status -Out "${OUT}" -Units ${unit}`), ' (it reads the unit\'s state; it changes nothing)')
      const held = read.json && Array.isArray(read.json.units) ? read.json.units.find((u) => u && u.id === unit) : null
      ledgerHash = (held && held.input_hash) || ''
      if (held && (held.state === 'done' || held.state === 'noop')) {
        log(`${at}: the finish line came back cut; the ledger holds the unit ${held.state}.`)
        return { unit, status: 'done', result: { unit, state: held.state, disposition: held.disposition || null, from: 'ledger' } }
      }
    }
    const cause = String((got.error && got.error.message) || got.error || (got.json && (got.json.fault || got.json.error)) || 'the checked completion did not complete').slice(0, 800)
    // S5: the ledger reads the cause (cause_code, fault_class); the claim's hash or the ledger's own (trial 19: a
    // cards-book fault went out with an empty hash and was refused).
    const fault = { schema_version: 1, unit, input_hash: hashOf(unit, inputs) || ledgerHash, cause }
    const status = await failUnit(`${at} fault`, unit, token, fault)
    return { unit, status, error: cause, command: got.command }
  }
  // --- A block's checked finishes in one command (35; docs/digest-claude-token-proposal.md P1/P2/P4) ----
  // `zt-digest finish-batch` runs each claimed unit's own verifier in turn in one process, records a unit that
  // cannot finish against that unit alone (`ledger fail`, the classes finishOrFault gives), and prints one
  // short line: counts and one entry per unit (unit, token, status, disposition or cause). On
  // ebrill2001modern trial 1 one relay per unit finish was 311 agents and the largest relay group.
  // The ledger stays the judge: a batch line no relay returned whole is run again for the units it does not
  // answer (every finish is idempotent and fenced by its token), and a unit still unanswered then is finished
  // on its own by finishOrFault, which asks the ledger before any fault.
  const BATCH_MAX = ZD_PER_MANAGER_MAX
  const FINISHED = ['done', 'partial', 'retry', 'failed']
  const finishBatchCommand = (units) => `${ZD_DIGEST} finish-batch -Out "${OUT}" -Units "${units.map((u) => `${u.id}:${u.token}`).join(',')}"`
  // The per-unit outcomes one finish_batch line answers, by unit id (only the units asked for).
  const batchOutcomes = (line, units) => {
    const got = new Map()
    if (!line || line.kind !== 'finish_batch' || !Array.isArray(line.results)) return got
    for (const r of line.results) {
      if (r && FINISHED.includes(r.status) && units.some((u) => u.id === r.unit)) got.set(r.unit, r)
    }
    return got
  }
  const asDisposition = (r) => (r.status === 'done' || r.status === 'partial'
    ? { unit: r.unit, status: r.status, result: { unit: r.unit, state: 'done', disposition: r.disposition || null, ...(r.from ? { from: r.from } : {}) } }
    : { unit: r.unit, status: r.status, error: r.cause || r.code || r.status })
  // The checked finishes of claimed units. `given` is the line of a finish step a chain manager already ran
  // (the events, findings and explain chains end with it); without one, the batch runs as one relay.
  const finishUnits = async (label, units, given) => {
    const list = (units || []).filter((u) => u && typeof u.id === 'string' && Number(u.token) > 0)
    if (!list.length) return []
    const settled = batchOutcomes(given, list)
    const open = () => list.filter((u) => !settled.has(u.id))
    if (given !== undefined && open().length) log(`${label}: the chain's finish line answers ${settled.size} of ${list.length} unit(s); the rest are finished in one command.`)
    for (let i = 0; open().length && i < list.length; i += BATCH_MAX) {
      const share = open().slice(0, BATCH_MAX)
      const out = await runScript(`${label}: finish`, LEDGER_PHASE, finishBatchCommand(share), ' (the checked completions of these claims, one after another; never retry it with other options)')
      const got = batchOutcomes(out.json, share)
      for (const [id, r] of got) settled.set(id, r)
      if (!got.size) break
    }
    const out = []
    for (const u of list) {
      if (settled.has(u.id)) { out.push(asDisposition(settled.get(u.id))); continue }
      log(`${label}: ${u.id} has no readable batch outcome; it is finished on its own.`)
      out.push(await finishOrFault(label, u.id, u.token, u.inputs))
    }
    return out
  }
  // The Cards layer's claim and checked finishes in one command (P2): its work is the finish alone, so
  // `finish-batch -Layer cards -Take -Owner` claims the ready units and finishes each in the same process.
  // A line no relay returned whole is never read as an empty claim: the units this owner still holds come
  // from the ledger (`ledger status -Held`) and are finished as above.
  const claimFinish = async (label, layerName, take, wait = 0) => {
    let out
    for (let attempt = 1; attempt <= CLAIM_TRIES; attempt++) {
      out = await runScript(attempt === 1 ? label : `${label} (try ${attempt})`, LEDGER_PHASE,
        `${ZD_DIGEST} finish-batch -Out "${OUT}" -Layer ${layerName} -Take ${Math.min(BATCH_MAX, take)} -Owner "${label}"${wait ? ` -Wait ${wait}` : ''} -Lease ${CLAIM_LEASE}`,
        ' (it claims the ready units and records each one\'s checked completion; never retry it with other options)')
      const line = out.json
      if (line && line.kind === 'finish_batch' && Array.isArray(line.results)) {
        const answered = line.results.filter((r) => r && FINISHED.includes(r.status))
        // A unit the time budget left unstarted is still claimed under this owner, with its token in the line.
        const left = line.results.filter((r) => r && r.status === 'not_run' && Number(r.token) > 0).map((r) => ({ id: r.unit, token: Number(r.token) }))
        const dispositions = [...answered.map(asDisposition), ...(left.length ? await finishUnits(label, left) : [])]
        return { ok: true, claimed: Number(line.claimed) || line.results.length, empty: !line.results.length, dispositions, gate: line }
      }
      const held = await runScript(`${label}: held units`, LEDGER_PHASE, ledgerCommand(`status -Out "${OUT}" -Held "${label}"`))
      const units = held.json && heldUnits(held.json.units, layerName) ? remember(held.json.units) : null
      if (units && units.length) {
        log(`${label}: the claim-and-finish line came back unreadable; the ${units.length} unit(s) it still holds are finished from the ledger.`)
        return { ok: true, claimed: units.length, empty: false, dispositions: await finishUnits(label, units) }
      }
      // Nothing held: the command finished every unit it claimed, claimed none, or failed before its claim.
      if (out.run && out.run.ok) return { ok: true, claimed: 0, empty: false, dispositions: [] }
      log(`${label}: the claim failed (${String((out.run && out.run.output) || 'no output').slice(0, 200)})${attempt < CLAIM_TRIES ? '; it is tried again' : ''}.`)
    }
    return { ok: false, error: (out && out.run && out.run.output) || 'no output from finish-batch', dispositions: [] }
  }
  // --- Owned DeepSeek resources (R322–R330) -----------------------------------------------------------
  // Every created conversation is recorded the moment the provider returns its id, by the manager that
  // started it (recordRules), against the work that created it: the unit it holds, or the run-level cleanup
  // unit for a stage-wide chain. The run-level cleanup deletes exactly the recorded ids; nothing is deleted
  // by title.
  const resourceOwner = (o = {}) => {
    if (o.unit) return { unit: o.unit, token: Number(o.token) || 0 }
    return 'run'
  }
  // R331: the run report. It reads the ledger and verifies the current placeholder state, writes
  // run-report.json and run-report.md, and prints the same facts ledger status reports.
  const ledgerReport = async (label) => {
    const out = await runScript(label, CLEANUP_PHASE, ledgerCommand(`report -Out "${OUT}"`), ' (it writes run-report.json and run-report.md)')
    if (!out.json) return { ok: false, error: String((out.run && out.run.output) || 'no output from ledger report') }
    return { ok: true, json: out.json }
  }
  // Keeps a block's claims current while its work runs: `ledger hold` renews every listed claim each minute for
  // up to nine minutes and returns as soon as none is still held (finished, failed or reclaimed), and the next
  // hold starts while the work goes on. The model compiler's chains outlive a claim's lease and renew nothing
  // themselves (grossman2002integration trial 5: four model claims expired mid-build, so their finishes would
  // have been refused and the built models claimed again).
  const holding = async (label, units, work) => {
    const list = units.filter((u) => u && typeof u.id === 'string' && Number(u.token) > 0).map((u) => `${u.id}:${u.token}`).join(',')
    if (!list) return work()
    let finished = false
    let failures = 0
    const beat = (async () => {
      for (let n = 1; !finished && n <= 60; n++) {
        const out = await runScript(`${label}: hold ${n}`, LEDGER_PHASE, ledgerCommand(`hold -Out "${OUT}" -Units "${list}" -Seconds 540 -Every 60`), ' (it keeps the claims current for up to nine minutes and changes no Card; it ends early once no claim is held)')
        const held = out.json && Array.isArray(out.json.held) ? out.json.held : null
        if (held && !held.length) return
        if (!held) {
          failures += 1
          log(`${label}: hold ${n} gave no readable line (${String((out.run && out.run.output) || 'no output').slice(0, 160)}).`)
          if (failures >= 3) return
        } else failures = 0
      }
    })()
    try {
      return await work()
    } finally {
      finished = true
      await beat
    }
  }
  // The driver of one layer (R252-R257): claim ready units in bounded blocks, work each unit's own chain,
  // record every result, and settle with the layer gate. `wait` is the tool-side readiness wait in seconds;
  // `producer` names the layer whose settled enumeration means no further unit can appear. Returns
  // complete/partial/active only after the gate says so.
  const layer = async (name, o) => {
    const layerName = o.layer || name
    const take = Math.max(1, Math.min(64, o.take || 1))
    // `rounds` bounds the consecutive claims that found nothing ready, never the claims that found work: on
    // grossman2002integration trial 6 the events layer spent its three rounds on three productive claims while
    // one digest group was still retrying, and gave up before that group's Event unit appeared.
    const rounds = Math.max(1, Math.min(12, o.rounds || MAX_ATTEMPTS))
    const MAX_CLAIMS = 200
    const dispositions = []
    let claimedCount = 0
    let gateResult = null
    let idle = 0
    let blind = 0
    for (let round = 1; round <= MAX_CLAIMS && idle < rounds; round++) {
      // o.claimFinish (35, the Cards layer): one command claims the ready units and records their checked
      // finishes. A round whose line was unreadable while the owner holds nothing is "blind": it may have
      // finished units or claimed none, so the layer claims again, at most CLAIM_TRIES times in a row.
      if (o.claimFinish) {
        const done = await claimFinish(`${name} claim ${round}`, layerName, take, o.wait === undefined ? 0 : o.wait)
        if (!done.ok) return { status: 'failed', layer: layerName, error: done.error, dispositions }
        for (const r of done.dispositions) dispositions.push(r)
        blind = !done.empty && !done.claimed ? blind + 1 : 0
        if (!done.empty && (done.claimed || blind < CLAIM_TRIES)) {
          idle = 0
          claimedCount += done.claimed
          continue
        }
      } else {
        const got = await claim(`${name} claim ${round}`, layerName, take, o.wait === undefined ? 0 : o.wait)
        if (!got.ok) return { status: 'failed', layer: layerName, error: got.error, dispositions }
        const units = got.units || []
        if (units.length) {
          idle = 0
          claimedCount += units.length
          const results = o.hold ? await holding(`${name} claim ${round}`, units, () => o.work(units)) : await o.work(units)
          for (const r of results) dispositions.push(r)
          continue
        }
      }
      idle += 1
      // Nothing ready: settle only on the gate's own facts, never on an empty queue.
      gateResult = await gate(`${name} gate ${round}`, layerName)
      if (gateResult.complete === true) return { status: 'complete', layer: layerName, gate: gateResult, dispositions, claimed: claimedCount }
      const open = (gateResult.counts && (gateResult.counts.open || 0)) || 0
      if (!o.producer && open === 0) {
        // A settled layer keeps an explicit partial result when a unit failed; an unsettled one is active.
        const failed = dispositions.filter(d => d.status === 'failed').length
        return { status: failed ? 'partial' : 'active', layer: layerName, gate: gateResult, dispositions, claimed: claimedCount }
      }
      if (o.producer) {
        const produced = await gate(`${name} producer ${round}`, o.producer)
        if (produced.settled === true) break
      }
      if (o.wait === undefined) break
    }
    gateResult = await gate(`${name} gate`, layerName)
    const failed = dispositions.filter(d => d.status === 'failed').length
    const open = (gateResult.counts && (gateResult.counts.open || 0)) || 0
    return {
      status: gateResult.complete === true ? 'complete' : failed && !open ? 'partial' : 'active',
      layer: layerName, gate: gateResult, dispositions, claimed: claimedCount,
    }
  }
  // --- Cards: the card layer cycle -----------------------------------------------------------------
  // One cycle over one scope, run as ONE manager's chain: plan → the jobs it prints → apply in shares
  // until none is pending → check → its follow-up jobs → apply → report (R250/R251). The bounded
  // correction path is declared here, not improvised. o.modelReview: this whole-book cycle also reviews
  // the Model drafts no model chain owns (R247); a draft an active chain owns reuses that chain's review.
  const runCards = async (label, work, extra = '', only = '', o = {}) => {
    const cstep = (s, more = '') => `${CARD_COMPILER} -Step ${s} -Work "${work}" -Compact${more}`
    const rules = only ? ` -Rules "${only}"` : A.cardRules ? ` -Rules "${A.cardRules}"` : ''
    const max = A.cardMax ? ` -MaxCards ${A.cardMax}` : ''
    const models = o.modelReview ? ' -ModelReview' : ''
    const applyStep = (n) => ({ name: `apply ${n}`, command: cstep('apply', ` -MaxJobs ${APPLY_SHARE}`), repeat: { path: 'pending_jobs', value: 0, max: 100 } })
    // R299: the finalization spends its one declared correction cycle, so it passes fixRounds 1
    // whatever the run's cardFixRounds pilot setting is.
    const rounds = o.fixRounds === undefined ? CARD_FIX_ROUNDS : Math.max(0, Math.min(12, o.fixRounds))
    const steps = [{ name: 'plan', command: cstep('plan', ` -Book ${KEY} -Staging "${OUT}"${rules}${max}${models}${extra}`), jobs: true }, applyStep(1)]
    for (let round = 1; round <= rounds; round++) {
      steps.push({ name: `check ${round}`, command: cstep('check'), jobs: true })
      steps.push(applyStep(round + 1))
    }
    steps.push({ name: 'report', command: cstep('report'), optional: true })
    const chained = await chain(`cards ${label}`, {
      dsh: work, skills: CARD_SKILLS, effort: DSH_EFFORT, phase: 'Cards', writes: `${work}\\out`, steps, cleanup: true, keep: true,
    })
    const rawPlan = jsonOf(chained, 'plan')
    const applied = steps.filter(s => /^apply /.test(s.name)).map(s => jsonOf(chained, s.name)).filter(Boolean).pop() || null
    const reported = jsonOf(chained, 'report')
    const jobs = (chained.results || []).length
    // The plan line carries the job prompts and the sample and is long; its chain reads the jobs from the
    // file the step saved. A plan line the relay rewrote is no failure when the cycle reported: on
    // grossman2002integration trial 10 the relay abridged the whole-book plan inside its jobs array, the cycle
    // ran and reported 0 faults after, and the stage faulted the claim as "plan failed: no output".
    const plan = rawPlan ? { ...rawPlan, jobs: expandJobs(rawPlan) }
      : reported ? { picked: reported.picked, with_faults: '?', faults: reported.faults_before, review: '?', orphans: '?', jobs: [], from: 'report' } : null
    if (!rawPlan && plan) log(`cards ${label}: the plan line came back unreadable; the report line carries the cycle.`)
    if (!plan) {
      log(`cards ${label}: plan failed: ${((stepOf(chained, 'plan') || {}).error) || 'no output'}.`)
      return { status: 'failed', label, error: `plan failed: ${((stepOf(chained, 'plan') || {}).error) || 'no output'}` }
    }
    log(`Cards ${label}: ${plan.picked} Cards of the book; ${plan.with_faults} with ${plan.faults} faults, ${plan.review} for review, ${plan.orphans} orphans; ${plan.jobs.length} DeepSeek jobs.`)
    const checked = steps.filter(s => /^check /.test(s.name)).map(s => jsonOf(chained, s.name)).filter(Boolean)
    for (const c of checked) log(`Cards ${label}: ${c.faults} faults left, ${(c.jobs || []).length} follow-up jobs.`)
    if (reported) log(`Cards ${label}: faults ${reported.faults_before} → ${reported.faults_after} on ${reported.cards} Cards; verdicts ${JSON.stringify(reported.verdicts)}; ${reported.splits} splits, ${reported.merges} merge proposals, ${reported.refused} refused; ${reported.orphan_links} orphan Links (${reported.orphans_after} orphans left); ${jobs} DeepSeek jobs.`)
    const applyFailed = steps.filter(s => /^apply /.test(s.name)).some(s => jsonOf(chained, s.name) === null)
    if (applyFailed) log(`Cards ${label}: an apply step failed; the cycle is marked failed.`)
    return { status: reported && !applyFailed ? 'done' : 'failed', label, plan, applied, report: reported, ...(applyFailed ? { error: 'apply failed: no output from the card compiler -Step apply' } : {}) }
  }
  return {
    A, MODE, KEY, OUT, SCRIPTS, DSH_MODEL, DSH_EFFORT, MODELS, IDEA_SAMPLE, LIMITS, COMPILER, FILL_SKILLS,
    PER_MANAGER, raised,
    MAX_ATTEMPTS, DSH_RUN_CAP, CLAIM_LEASE, RUNNER, COMMAND_TIMEOUT_MS: ZD_COMMAND_TIMEOUT_MS,
    // The command prefixes and skills a stage module builds a chain with.
    LEDGER: ZD_LEDGER, DIGEST: ZD_DIGEST, CALL: ZD_CALL, DSH_SKILLS,
    zAgent, runScript, groupsOf, toBlocks, drive, expandJobs, answerLines,
    // The layer driver (#175): one script step per block, each run by a DeepSeek drive job, and its managers.
    layerRun, driveStep, runJobs, LAYER_STEPS, LAYER_LEASE,
    // The ready-work driver (9.3): one manager per declared chain, plus the ledger adapter.
    chain, ANSWERS, stepOf, jsonOf, resultLine, claim, gate, settledOnResume, holding, failUnit, finishOf, finishOrFault, layer, psQuote,
    // 35: a block's checked finishes in one command, and its outcomes.
    finishBatchCommand, finishUnits, batchOutcomes,
    // The command that runs a digest script: zt-digest <tool> once ported, else pwsh with the script.
    tool: name => zdTool(name, SCRIPTS),
    // Tree Lock (#140): the owner's typed refusal, its sentence and the stop a stage throws with it.
    LOCK: ZD_LOCK, lockRefusal: zdLockRefusal, lockSaid: zdLockSaid, lockStop: zdLockStop,
    // Owned provider resources (R322-R330): the owner of each recorded id, the cleanup phase, the run report.
    resourceOwner, CLEANUP_PHASE, ledgerReport,
    runCards, profile, ARCH_DOC, ARCH_SUPPLEMENT, MODEL_WORK, PER_SONNET, PER_CONCEPT, DSH_PER_BATCH, REBUILD, LEVEL,
    BRIEF_EFFORT, DERIVE_EFFORT, REVIEW_EFFORT, REVIEW_ROUNDS, COMPILER_DOCS, SKIP_FORMS,
  }
}

defineModule({
  id: 'zt-digest-lib', version: '38', library: true, requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Library: the ZT digest machinery (settings, managers, the drive-step layer driver, the model compiler, the card cycle) for the zt-* stage modules, and the Tree Lock seam that stops an affected write stage on the owner\'s typed card_locked refusal.',
})
// ==== end module zt-digest-lib ====


