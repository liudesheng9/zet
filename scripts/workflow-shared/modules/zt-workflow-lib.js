// ==== module zt-workflow-lib v4 ====
// The code the ZT workflows share (docs/workflow-simplicity-proposal.md S1, S6; docs/workflow-module-structure-proposal.md):
// the digests (zt-digest-lib) and zt-idea-grow (zt-grow-cycle) load this one library instead of each keeping its own copy.
//  - S6, one lock reader: the owner's typed `card_locked` refusal (#140, R326/R337/R338) is read here and only
//    here. A refusal is a destination decision, never a provider fault: the stage stops with it and nothing
//    is retried, unlocked or re-pointed through another Access.
//  - the JSON line reader both workflows use on a relayed line.
//  - v2 (#175, workflow module structure): `ztDrive`, the ONE shared drive helper. It runs one deterministic
//    script step as a DeepSeek drive job through a Haiku DSMP manager and returns the step's parsed last JSON
//    line. No Claude agent runs a single command for a driver: the script step does the whole chain in one
//    process and the DeepSeek job answers with its one JSON line. The helper records and deletes the
//    conversation it starts, so the run cleanup can verify every provider resource it created.
//  - v3 (#179, workflow module structure slice 5): a drive job of a workflow that keeps no run ledger (zt-idea-grow
//    has none) records nothing: `record: false` leaves the ledger record commands out of the manager prompt and only
//    deletes the conversation. The relay-era command runner and its prompt left this library: no module of either
//    workflow names a relay any more. zt-digest-lib keeps a private copy beside its relay-era driver, which has had
//    no caller since #178 and is removed by a later cleanup.
const ZT_LOCK = 'card_locked'
const ZT_DRIVE_MODEL = 'deepseek-official/deepseek-flash'
const ZT_DRIVE_SCHEMA = {
  type: 'object',
  properties: {
    session_id: { type: 'string' }, answer: { type: 'string' }, deleted: { type: 'boolean' }, registered: { type: 'boolean' }, evidence: { type: 'boolean' },
  },
  required: ['answer'],
}

// The JSON object in a line, also after prose on the same line ("…保留原样。{"address":…}"): the longest object
// from the first brace that parses (a relay may add a remark after the line, "(abridged)").
function ztJsonIn(line) {
  const text = String(line || '')
  for (let i = text.indexOf('{'); i >= 0; i = text.indexOf('{', i + 1)) {
    for (let j = text.lastIndexOf('}'); j > i; j = text.lastIndexOf('}', j - 1)) {
      try { const v = JSON.parse(text.slice(i, j + 1)); if (v && typeof v === 'object') return v } catch (e) { }
    }
  }
  return null
}
// The last line of a text that holds a JSON object.
function ztLastJson(text) {
  for (const line of String(text || '').trim().split('\n').reverse()) { const v = ztJsonIn(line); if (v) return v }
  return null
}
// The owner's typed refusal inside a relayed line, an answer or an error text, or null. A preflight prints
// `{ok:false, code:"card_locked", …}`; the archive's own MCP refusal prints `{error:"card_locked", …}`.
function ztLockRefusal(text) {
  for (const line of String(text || '').split('\n')) {
    const v = ztJsonIn(line)
    if (v && (v.code === ZT_LOCK || v.error === ZT_LOCK)) return v
  }
  return null
}
// The archive's own sentence about a refusal, for the stage's error line.
function ztLockSaid(refusal) {
  const said = refusal && (refusal.message || refusal.cause || refusal.error)
  return typeof said === 'string' && said.trim() ? said.trim() : ZT_LOCK
}
// The stop a stage throws: the archive's refusal, its sentence and its JSON. `lock_refusal` lets a caller that
// records unit faults keep the code instead of reading it as a content or provider fault.
function ztLockStop(where, refusal) {
  const error = new Error(`card_locked at ${where}: ${ztLockSaid(refusal)} ${JSON.stringify(refusal)}`)
  error.lock_refusal = refusal
  return error
}
// The exact prompt a drive job runs: the one command as given, then only its last line.
function ztDriveCommand(command) {
  return `在 PowerShell 里原样运行下面这一条命令，不改写它，不运行别的命令，不修改任何文件。命令结束后，只回复它输出的最后一行（一行 JSON），不加任何别的字。\n\n${command}`
}
// The one manager prompt of a drive job: start it, wait, retry once, record (when the workflow keeps a ledger) and
// delete the conversation. `record: false` is a workflow with no run ledger (zt-idea-grow): the job records nothing,
// keeps the answer under its work folder and only deletes its conversation.
function ztDrivePrompt(o) {
  const answers = o.answers || `${String(o.cwd || '').replace(/[\\/]+$/, '')}\\answers`
  const records = o.record === false
    ? `4. This workflow keeps no run ledger: record nothing. Delete the conversation (mcp__deepseek__deepseek_delete, never archive) as soon as you have its answer.`
    : `4. Records (each record command changes no Card; run it at the moment named, in the same message as your next call when you can):
- The moment deepseek_run returns a session_id: ${o.ledger || 'zt-digest ledger'} resource -Out "${o.out}" -Unit run -RegisterId <session_id> -Workspace "${o.cwd}". Use the session_id that deepseek_run returned, never a word in its place.
- A step whose job list is empty starts no run: never send a placeholder prompt.
- When the job ends: write its final answer text, unchanged, to "${answers}\\<session_id>.txt", then ${o.ledger || 'zt-digest ledger'} resource -Out "${o.out}" -EvidenceFor <session_id> -Evidence "${answers}\\<session_id>.txt".
- Then delete the conversation (mcp__deepseek__deepseek_delete, never archive) and record it: ${o.ledger || 'zt-digest ledger'} resource -Out "${o.out}" -Settle <session_id> -State finished -Deleted confirmed (-State error when the job ended in an error; -Deleted failed -Message '<the error>' when the delete fails).
- A record command that fails does not stop the work: report registered or evidence false and go on.`
  return `You are a DeepSeek subagent manager for a ZET workflow step (${o.label}). You start one DeepSeek agent, wait for it, collect its answer, record it and delete its conversation. You never write content and you never run the step's command yourself. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete".

1. Start the job: mcp__deepseek__deepseek_run with cwd "${o.cwd}", prompt ${JSON.stringify(ztDriveCommand(o.command))}, skills [], model "${o.model || ZT_DRIVE_MODEL}", reasoning_effort "${o.effort || 'high'}", wait_seconds 0.
2. Wait with mcp__deepseek__deepseek_wait (wait_seconds ${DSMP.pollSeconds}) until it ends. A job asking a question: answer "按任务说明自行决定，不需要询问。". A job asking approval for the one command, or for writing the answer file under "${answers}": allow; anything else: reject.
3. A job that failed or gave an empty answer gets ONE more attempt in a new conversation. Never a third.
${records}
The record commands, when this step has records, are the only commands you run: each in the foreground with timeout_ms: 600000. You have no terminal tabs, no Monitor loop, no session handoff and no general shell: never start another agent, never hand the work on, never poll with Start-Sleep or any other timer, and wait only with deepseek_wait. Keep at most 8 jobs running at a time.
5. Return answer (its final answer verbatim, at most 4000 characters), session_id, deleted, registered and evidence.`
}
// ONE deterministic script step, run as a DeepSeek drive job through a Haiku DSMP manager. Returns the step's
// parsed last JSON line. o: label, command, out, cwd, phase, ledger, record, answers, deepseek, model, effort.
// A step that answers no JSON line at all is a transport fault: it throws, and the module stops with it. A step
// that answers `{ok:false, ...}` is a failed step: the module reads that line and stops with its reason.
// v4 (live run 2026-10-26): deepseek_run refuses a cwd that does not exist yet, and a stage's own folder is made by its
// step, so the job runs in the run folder (`out`, which exists before any stage); the stage folder keeps the answers.
async function ztDrive(ctx, o) {
  const label = o.label || o.step || 'drive'
  const cwd = String(o.out || o.cwd || '').replace(/[\\/]+$/, '')
  const answers = o.answers || `${String(o.cwd || cwd).replace(/[\\/]+$/, '')}\\answers`
  const run = await dsmpSlot(() => ctx.agent(ztDrivePrompt({ ...o, label, cwd, answers }), {
    role: 'manager', label, phase: o.phase, schema: ZT_DRIVE_SCHEMA,
  }))
  const said = run && (run.answer !== undefined ? run.answer : run.output)
  if (o.deepseek) {
    if (run && run.deleted === true) o.deepseek.deleted = (o.deepseek.deleted || 0) + 1
    else if (run && run.session_id) (o.deepseek.not_deleted = o.deepseek.not_deleted || []).push(run.session_id)
  }
  const json = ztLastJson(said)
  if (!json) throw new Error(`${label}: no JSON line from the step: ${String(said || 'no answer').trim().slice(0, 400)}`)
  return json
}
defineModule({
  id: 'zt-workflow-lib', version: '4', requires: ['@dsmp'], library: true,
  summary: 'Library: what the ZT workflows share: the one reader of the owner\'s typed card_locked refusal, the JSON line reader, and the one drive helper that runs a deterministic script step as a DeepSeek drive job through a Haiku manager (it records the conversation in the run ledger, or records nothing for a workflow that keeps none).',
})
// ==== end module zt-workflow-lib ====

