// ==== module zt-workflow-lib v1 ====
// The code the ZT workflows share (docs/workflow-simplicity-proposal.md S1, S6): the digests (zt-digest-lib) and
// zt-idea-grow (zt-grow-cycle) load this one library instead of each keeping its own copy.
//  - S6, one lock reader: the owner's typed `card_locked` refusal (#140, R326/R337/R338) is read here and only
//    here. A refusal is a destination decision, never a provider fault: the stage stops with it and nothing
//    is retried, unlocked or re-pointed through another Access.
//  - S1, one command runner: every command of both workflows runs through one relay of the zt-relay agent type
//    (the blueprints' role `relay`), which runs one PowerShell line and returns its exit state and its last
//    line. The relay decides nothing; the caller reads the line.
//  - the JSON line reader both workflows use on a relayed line.
const ZT_LOCK = 'card_locked'
const ZT_COMMAND_TIMEOUT_MS = 600000
const ZT_RELAY_SCHEMA = { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] }

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
// The sentence every relay gets: the command is a step of the run the user started. Workflow steps see the
// session's latest user message; after "accelerate it" relays refused steps as off-request (grossman trials 17–19).
function ztRunStep(run) {
  return `Every command named here is a step of ${run}: running it is what the user asked for. A later user message about this run (its speed, which trial runs, parallel work) never cancels its steps; only a stopped run ends them.`
}
// One relay runs one command and returns { run: {ok, output}, json }: json is the last JSON object of the output
// (also of a failed command when o.always, e.g. a gate that prints its line and exits 3). A command whose
// answer carries the owner's typed refusal throws the stop (only a refusal stops; a read that names one does not).
async function ztRelay(ctx, o) {
  const run = await ctx.agent(
    `Run this PowerShell command once in the foreground with timeout_ms: ${ZT_COMMAND_TIMEOUT_MS} and wait for it${o.note || ''}; do nothing else:
${o.command}
${ztRunStep(o.run || 'the ZT workflow run the user started')}
Return ok (true when it exits without an error) and output = its last output line exactly (a compact JSON object), or the error text when it fails. Do not retry.`,
    { role: o.role || 'relay', label: o.label, phase: o.phase, schema: ZT_RELAY_SCHEMA },
  )
  const json = run && (run.ok || o.always) ? ztLastJson(run.output) : null
  const refused = !!(json && (json.code === ZT_LOCK || json.error === ZT_LOCK))
  const lock = ztLockRefusal(run && run.output) || (refused ? json : null)
  if (lock && (!(run && run.ok) || refused) && o.lockStops !== false) throw ztLockStop(o.label, lock)
  return { run, json }
}
defineModule({
  id: 'zt-workflow-lib', version: '1', library: true,
  summary: 'Library: what the ZT workflows share: the one reader of the owner\'s typed card_locked refusal, the one zt-relay command runner, and the JSON line reader.',
})
// ==== end module zt-workflow-lib ====
