// ==== module zt-workflow-lib v6 ====
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
//  - v4 (live run 2026-10-26): the drive job runs in the run folder (`out`), because deepseek_run refuses a cwd
//    that does not exist yet; the stage folder keeps the answers.
//  - v5 (#180): the reader is strict and the manager copies the line verbatim. On the live run a Haiku manager
//    retyped a step's long line as prose ("[digest/g001@1 through digest/g008@1]"); `ztLastJson` then took the
//    nested `counts` object, the stage read no `jobs`, ran no writer and ended `done` with eight units claimed.
//    Now a drive answer is ONLY its last non-empty line, that line must parse as a JSON object whose top-level
//    `ok` is a boolean, and anything else throws: the stage fails with the answer text. A nested object is never
//    taken, and `ztDrive` has no "any JSON in the text" fallback. The manager prompt says the `answer` field is the
//    job's final line copied character for character, and every step script prints at most 400 characters
//    (`scripts/book-digest/step-line.ps1`).
//  - v6 (live run 2026-10-26, staging4 link start): a Haiku manager copied the line with each `\\` of a Windows path
//    as one `\`, so the right line did not parse. The reader then doubles the lone backslashes of that same last line
//    and parses it once more; it is still only the last line, one object with a boolean top-level `ok`.
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
// from the first brace that parses (a relay may add a remark after the line, "(abridged)"). Used for the lines a
// relayed command printed, never for a drive job's answer (#180: `ztDriveAnswer`).
function ztJsonIn(line) {
  const text = String(line || '')
  for (let i = text.indexOf('{'); i >= 0; i = text.indexOf('{', i + 1)) {
    for (let j = text.lastIndexOf('}'); j > i; j = text.lastIndexOf('}', j - 1)) {
      try { const v = JSON.parse(text.slice(i, j + 1)); if (v && typeof v === 'object') return v } catch (e) { }
    }
  }
  return null
}
// The last line of a text that holds a JSON object (a relayed command's output).
function ztLastJson(text) {
  for (const line of String(text || '').trim().split('\n').reverse()) { const v = ztJsonIn(line); if (v) return v }
  return null
}
// The answer of a drive job (#180), or null. ONLY its last non-empty line counts, that line must be one JSON
// object (no prose around it, no remark after it), and its top-level `ok` must be a boolean. A retyped or
// summarized answer, a line the manager rewrote, a nested object without `ok`, and an empty answer are all null:
// the stage stops with the answer text instead of reading a field the step never printed.
// The one object of a drive answer's last line, or undefined: the line as given, else (v6) the line with its lone
// backslashes doubled, as a manager that copied a Windows path unescaped gives it.
function ztDriveParse(last) {
  try { return JSON.parse(last) } catch (e) { }
  if (!last.includes('\\')) return undefined
  try { return JSON.parse(last.replace(/\\\\/g, '\u0000').replace(/\\/g, '\\\\').replace(/\u0000/g, '\\\\')) } catch (e) { return undefined }
}
function ztDriveAnswer(text) {
  const lines = String(text === undefined || text === null ? '' : text).split('\n').map((l) => l.trim()).filter(Boolean)
  if (!lines.length) return null
  const last = lines[lines.length - 1]
  const v = ztDriveParse(last)
  if (!v || typeof v !== 'object' || Array.isArray(v)) return null
  if (typeof v.ok !== 'boolean') return null
  return v
}
// Why an answer is not a drive answer, in the words the stage's error line uses.
function ztDriveWhy(text) {
  const said = String(text === undefined || text === null ? '' : text).trim()
  if (!said) return 'the answer is empty'
  const lines = said.split('\n').map((l) => l.trim()).filter(Boolean)
  const last = lines[lines.length - 1]
  const v = ztDriveParse(last)
  if (v === undefined) return 'its last line is not one JSON object'
  if (!v || typeof v !== 'object' || Array.isArray(v)) return 'its last line is not a JSON object'
  if (typeof v.ok !== 'boolean') return 'its last line has no boolean top-level ok'
  return 'it is not a drive answer'
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
// The exact prompt a drive job runs: the one command as given, then only its last line, copied.
function ztDriveCommand(command) {
  return `在 PowerShell 里原样运行下面这一条命令，不改写它，不运行别的命令，不修改任何文件。命令结束后，只回复它输出的最后一行：把那一行一字不改地复制过来，不要改写、缩写、翻译或补充任何字。那一行是一个紧凑的 JSON 对象，以 "ok" 开头。\n\n${command}`
}
// The one manager prompt of a drive job: start it, wait, retry once, record (when the workflow keeps a ledger) and
// delete the conversation. `record: false` is a workflow with no run ledger (zt-idea-grow): the job records nothing,
// keeps the answer under its work folder and only deletes its conversation.
// #180: the manager copies the final line character for character into `answer`; it never retypes or summarizes it.
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
5. Return answer: the job's final line copied character for character, with no word added, removed or changed: never a summary, never a retyped or shortened line. Also return session_id, deleted, registered and evidence.`
}
// ONE deterministic script step, run as a DeepSeek drive job through a Haiku DSMP manager. Returns the step's
// parsed last line (#180: only its last non-empty line, one JSON object with a boolean top-level `ok`).
// o: label, command, out, cwd, phase, ledger, record, answers, deepseek, model, effort.
// An answer that is not that line is a transport fault: it throws and the module stops with the answer text. A
// step that answers `{ok:false, ...}` is a failed step: the module reads that line and stops with its reason.
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
  const json = ztDriveAnswer(said)
  if (!json) {
    throw new Error(`${label}: the step's answer is not its one JSON line (${ztDriveWhy(said)}): ${String(said || 'no answer').trim().slice(0, 400)}`)
  }
  return json
}
defineModule({
  id: 'zt-workflow-lib', version: '6', requires: ['@dsmp'], library: true,
  summary: 'Library: what the ZT workflows share: the one reader of the owner\'s typed card_locked refusal, the JSON line reader, the strict drive answer reader (only the last line, one JSON object with a boolean top-level ok), and the one drive helper that runs a deterministic script step as a DeepSeek drive job through a Haiku manager (it records the conversation in the run ledger, or records nothing for a workflow that keeps none).',
})
// ==== end module zt-workflow-lib ====
