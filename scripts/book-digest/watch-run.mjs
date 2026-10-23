// Watches one digest Workflow run for the session that started it, so the session wakes only when something
// needs it (docs/digest-claude-token-proposal.md P9). On ebrill2001modern trial 1, 13 watches that expired every
// 30 minutes and replayed the journal woke a 200k-token session 82 times, 55 of them with nothing to do.
//
//   node scripts/book-digest/watch-run.mjs <journal.jsonl> <staging out> [--summary-minutes 30] [--stall-minutes 20]
//
// <journal.jsonl> is the run's journal (~/.claude/projects/<project>/<session>/subagents/workflows/<run id>/journal.jsonl).
// stdout carries only lines that ask for attention, each starting with its kind:
//   ABNORMAL  an agent answered a failure, a refusal, a crash or an undefined value
//   FAILED    the ledger holds a unit failed that was not failed before
//   EXPIRED   a claimed unit's lease ran out more than a minute ago
//   STALL     no journal event for --stall-minutes
//   SUMMARY   the ledger's open/done/failed counts per layer, every --summary-minutes, so an unforeseen problem still shows
//   END       the run reported its run report; the watch exits
// Every finished agent goes to <out>\watch\progress.log, which a person can read at any time. The watch keeps its
// place in <out>\watch\state.json: a watch started again reports nothing it already reported.
// It only reads: the journal, and `zt-digest ledger status` (a local command, no model).
import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'

const argv = process.argv.slice(2)
const option = (name, fallback) => { const i = argv.indexOf(`--${name}`); return i >= 0 ? Number(argv[i + 1]) : fallback }
const [journal, out] = argv.filter((a, i) => !a.startsWith('--') && !(i > 0 && argv[i - 1].startsWith('--')))
if (!journal || !out) {
  console.error('usage: node watch-run.mjs <journal.jsonl> <staging out> [--summary-minutes 30] [--stall-minutes 20]')
  process.exit(2)
}
const SUMMARY_MS = option('summary-minutes', 30) * 60000
const STALL_MS = option('stall-minutes', 20) * 60000
const LEDGER_MS = 120000
const dir = path.join(out, 'watch')
fs.mkdirSync(dir, { recursive: true })
const stateFile = path.join(dir, 'state.json')
const progressFile = path.join(dir, 'progress.log')
const read = (file, fallback) => { try { return JSON.parse(fs.readFileSync(file, 'utf8')) } catch { return fallback } }
const state = { journal, lines: 0, failed: [], expired: [], lastEventAt: Date.now(), lastSummaryAt: 0, stallReported: false, ...read(stateFile, {}) }
if (state.journal !== journal) Object.assign(state, { journal, lines: 0, failed: [], expired: [], stallReported: false })
const save = () => fs.writeFileSync(stateFile, JSON.stringify(state))
const clip = (text, n) => String(text).replace(/\s+/g, ' ').slice(0, n)
const say = (kind, text) => process.stdout.write(`${kind} | ${clip(text, 400)}\n`)
const note = (text) => fs.appendFileSync(progressFile, `${new Date().toISOString()} ${clip(text, 300)}\n`)

// The words that mean an agent did not do its step. A lane that claims nothing is normal and is not listed.
function abnormal(text) {
  const t = text.replace(/[\\"]*refused[\\"]*:(\d+|\[\])/g, '')
  if (/"ok":false|stale_claim|panicked|exit code [1-9]|"status":"failed"|\\"status\\":\\"failed|not recorded|not a declared|card_locked|cannot find|找不到(文件|路径|指定)/i.test(t)) return 'failure'
  if (/undefined|incomplete prepared contract/.test(t)) return 'undefined value'
  if (/"recorded":false/.test(t)) return 'unrecorded'
  if (/I (cannot|can.t|won.t|will not)|not (authori[sz]ed|permitted)|decline|without (explicit )?(user )?(approval|confirmation)|拒绝/i.test(t)) return 'refusal'
  return null
}

const labels = new Map()
let ended = !!state.ended
function readJournal() {
  if (!fs.existsSync(journal)) return
  const lines = fs.readFileSync(journal, 'utf8').split('\n').filter(Boolean)
  for (let i = 0; i < lines.length; i++) {
    let x
    try { x = JSON.parse(lines[i]) } catch { continue }
    if (x.type === 'started') labels.set(x.agentId, x.label)
    if (i < state.lines) continue
    state.lastEventAt = Date.now()
    state.stallReported = false
    if (x.type !== 'result') continue
    const label = labels.get(x.agentId) || x.agentId
    const text = JSON.stringify(x.result)
    const kind = abnormal(text) || (/undefined/.test(label) ? 'undefined label' : null)
    note(`${kind ? 'ABNORMAL ' : ''}${label} ${clip(text, kind ? 300 : 120)}`)
    if (kind) say('ABNORMAL', `${kind} | ${label} | ${text.slice(0, 300)}`)
    if (/^report the run from the ledger$/.test(label)) ended = state.ended = true
  }
  state.lines = lines.length
}

let lastLedger = 0
function readLedger(summary) {
  let v
  try {
    const raw = execFileSync('zt-digest', ['ledger', 'status', '-Out', out], { encoding: 'utf8', maxBuffer: 512 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] })
    v = JSON.parse(raw.trim().split('\n').pop())
  } catch (e) {
    if (summary) say('SUMMARY', `ledger unreadable: ${clip(e.message, 200)}`)
    return
  }
  const now = Date.now() / 1000
  const units = Array.isArray(v.units) ? v.units : []
  const failed = units.filter((u) => u.state === 'failed').map((u) => u.id)
  const fresh = failed.filter((id) => !state.failed.includes(id))
  if (fresh.length) say('FAILED', `${fresh.length} unit(s): ${fresh.slice(0, 8).join(', ')}${fresh.length > 8 ? ', ...' : ''}`)
  state.failed = failed
  const expired = units.filter((u) => u.state === 'claimed' && u.lease_until && u.lease_until < now - 60).map((u) => `${u.id}@${u.token}`)
  const freshExpired = expired.filter((id) => !state.expired.includes(id))
  if (freshExpired.length) say('EXPIRED', `${freshExpired.length} claim(s) past their lease: ${freshExpired.slice(0, 8).join(', ')}`)
  state.expired = expired
  if (summary) {
    const layers = {}
    for (const u of units) {
      const l = (layers[u.layer] = layers[u.layer] || { done: 0, open: 0, failed: 0 })
      if (u.state === 'done') l.done++
      else if (u.state === 'failed') l.failed++
      else if (u.state !== 'registered') l.open++
    }
    const line = Object.entries(layers).filter(([, c]) => c.done + c.open + c.failed).map(([k, c]) => `${k} ${c.done}/${c.done + c.open + c.failed}${c.failed ? ` (${c.failed} failed)` : ''}`).join(', ')
    say('SUMMARY', `${labels.size} agents so far; ${line}`)
  }
}

for (;;) {
  readJournal()
  const now = Date.now()
  const summary = now - state.lastSummaryAt >= SUMMARY_MS
  if (summary || now - lastLedger >= LEDGER_MS) {
    lastLedger = now
    if (summary) state.lastSummaryAt = now
    readLedger(summary)
  }
  if (!state.stallReported && now - state.lastEventAt >= STALL_MS) {
    say('STALL', `no journal event for ${Math.round((now - state.lastEventAt) / 60000)} minutes`)
    state.stallReported = true
  }
  save()
  if (ended) {
    readLedger(true)
    say('END', 'the run reported its run report; see run-report.md in the staging folder')
    save()
    process.exit(0)
  }
  await new Promise((resolve) => setTimeout(resolve, 15000))
}
