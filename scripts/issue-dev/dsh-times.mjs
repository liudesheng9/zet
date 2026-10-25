// Reads one DeepSeek Harness conversation from its stored log and measures the time of every shell command it ran.
// The zt-issue-dev workflow runs it after an issue's goal ends and before the conversation is deleted, so the slow
// test runs of one issue can be named to the next issue's agent.
//   node scripts/issue-dev/dsh-times.mjs --session <id> [--issue N] [--out file] [--min-sec 60] [--top 12]
// A foreground command lasts from its tool call to its tool result; a background job from its call to the
// "background job <id> ... finished" notice or the first job_output read that does not say "[status: running]"
// (an unfinished job ends at the last record). The last stdout line is
// compact JSON: {ok, session, issue, commands, shellSec, testSec, tests, slow: [{sec, test, background, exit, cmd,
// example, description, runs, unfinished}]}, where slow merges the runs of one command (commandKey) and keeps those
// of at least --min-sec seconds in all.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import zlib from 'node:zlib'
import { fileURLToPath } from 'node:url'

const SHELL_TOOLS = new Set(['pwsh', 'bash', 'shell', 'powershell', 'cmd'])
const TEST_RE = /\bcargo\s+(test|nextest)\b|run-tests|\.tests?\.(mjs|js|ps1)\b|Invoke-Pester|\bnpm\s+(run\s+)?test\b|\bnode\s+--test\b|[\\/]tests?[\\/][^\s'"]*\.(ps1|mjs|js)\b/i

// The stored log is a sequence of zstd frames; a frame's magic bytes may also occur inside another frame, so a
// cut point counts only when the bytes before it decode.
export function decodeFrames(buf) {
  const marks = []
  for (let i = 0; i + 3 < buf.length; i++) if (buf[i] === 0x28 && buf[i + 1] === 0xb5 && buf[i + 2] === 0x2f && buf[i + 3] === 0xfd) marks.push(i)
  marks.push(buf.length)
  const out = []
  let start = 0
  for (let k = 1; k < marks.length; k++) {
    try { out.push(zlib.zstdDecompressSync(buf.subarray(start, marks[k]))); start = marks[k] } catch { /* not a frame end */ }
  }
  if (start < buf.length) throw new Error(`undecodable bytes from offset ${start}`)
  return Buffer.concat(out).toString('utf8')
}

export function records(text) {
  return text.split('\n').filter(Boolean).map(l => { try { return JSON.parse(l) } catch { return null } }).filter(Boolean)
}

const textOf = m => ((m && m.content) || []).map(c => (c && c.type === 'text' ? c.text : '')).join('')

// One entry per shell command: {cmd, description, background, start, end, sec, exit, finished, test}.
export function shellTimes(recs) {
  const calls = new Map()
  const jobs = new Map()
  const reads = new Map()
  let last = 0
  for (const r of recs) {
    if (typeof r.time === 'number') last = Math.max(last, r.time)
    if (r.type === 'tool/call' && SHELL_TOOLS.has(String(r.data && r.data.name).toLowerCase())) {
      let a = {}
      try { a = JSON.parse(r.data.arguments || '{}') } catch { a = {} }
      calls.set(r.data.callId, { cmd: String(a.command || ''), description: String(a.description || ''), background: !!a.run_in_background, start: r.time, end: null, exit: null, finished: false })
    } else if (r.type === 'tool/call' && /^job_(output|wait)$/.test(String(r.data && r.data.name))) {
      let a = {}
      try { a = JSON.parse(r.data.arguments || '{}') } catch { a = {} }
      if (a.job_id) reads.set(r.data.callId, String(a.job_id))
    } else if (r.type === 'tool/result' && reads.has(r.data && r.data.message && r.data.message.toolCallId)) {
      // A job read ends the job unless it says the job still runs.
      const m = r.data.message
      const c = jobs.get(reads.get(m.toolCallId))
      const t = textOf(m)
      if (c && c.end === null && !/\[status: running\]\s*$/.test(t)) {
        c.end = r.time; c.finished = true
        const ex = t.match(/exit code:?\s*(-?\d+)/i)
        if (ex) c.exit = Number(ex[1])
      }
    } else if (r.type === 'tool/result') {
      const m = r.data && r.data.message
      const c = m && calls.get(m.toolCallId)
      if (!c || c.end !== null) continue
      const t = textOf(m)
      const job = t.match(/started background job (\S+)/)
      if (job) { c.background = true; jobs.set(job[1], c); continue }
      c.end = r.time; c.finished = true
      const ex = t.match(/exit code:?\s*(-?\d+)/i)
      if (ex) c.exit = Number(ex[1])
    } else if (r.type === 'user/message' || r.type === 'agent/inbox/spliced') {
      const parts = r.type === 'user/message' ? [r.data] : ((r.data && r.data.inserted) || [])
      for (const p of parts) {
        for (const m of String(textOf(p)).matchAll(/background job (\S+) \(.*?\) finished \[status: [^,\]]*(?:, exit code: (-?\d+))?/gs)) {
          const c = jobs.get(m[1])
          if (c && c.end === null) { c.end = r.time; c.finished = true; if (m[2] !== undefined) c.exit = Number(m[2]) }
        }
      }
    }
  }
  return [...calls.values()].map(c => {
    const end = c.end === null ? last : c.end
    return { ...c, end, sec: Math.max(0, Math.round((end - c.start) / 100) / 10), test: TEST_RE.test(c.cmd) }
  })
}

const oneLine = s => String(s).replace(/\s+/g, ' ').trim()
// Runs of one command count together: the output filter after the first pipe or redirect does not matter.
export const commandKey = cmd => oneLine(String(cmd).split(/\s(?:\d?>&?\d?|\|)\s?/)[0])

export function summarize(times, { minSec = 60, top = 12 } = {}) {
  const sum = xs => Math.round(xs.reduce((a, c) => a + c.sec, 0))
  const merged = new Map()
  for (const c of times) {
    const key = commandKey(c.cmd)
    const m = merged.get(key) || { sec: 0, test: c.test, background: c.background, exit: c.exit, cmd: key.slice(0, 400), example: oneLine(c.cmd).slice(0, 300), description: oneLine(c.description).slice(0, 160), runs: 0, unfinished: 0 }
    m.sec = Math.round((m.sec + c.sec) * 10) / 10; m.runs++; if (!c.finished) m.unfinished++; if (c.exit !== null) m.exit = c.exit
    merged.set(key, m)
  }
  const slow = [...merged.values()].filter(m => m.sec >= minSec).sort((a, b) => b.sec - a.sec).slice(0, top)
  return { commands: times.length, shellSec: sum(times), testSec: sum(times.filter(c => c.test)), tests: times.filter(c => c.test).length, slow }
}

export function findSessionFile(id, root = path.join(os.homedir(), '.dsh', 'sessions')) {
  for (const d of fs.readdirSync(root)) {
    const f = path.join(root, d, id, 'session.v4.jsonl.zstd')
    if (fs.existsSync(f)) return f
  }
  return null
}

function arg(name, dflt) {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 && i + 1 < process.argv.length ? process.argv[i + 1] : dflt
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const session = arg('session', '')
  const issue = Number(arg('issue', 0)) || 0
  try {
    if (!/^[\w.-]+$/.test(session)) throw new Error(`bad --session "${session}"`)
    const file = arg('file', '') || findSessionFile(session)
    if (!file) throw new Error(`no stored log for session ${session}`)
    const times = shellTimes(records(decodeFrames(fs.readFileSync(file))))
    const s = summarize(times, { minSec: Number(arg('min-sec', 60)), top: Number(arg('top', 12)) })
    const out = arg('out', '')
    if (out) {
      fs.mkdirSync(path.dirname(out), { recursive: true })
      fs.writeFileSync(out, JSON.stringify({ session, issue, ...s, all: times }, null, 1))
    }
    console.log(JSON.stringify({ ok: true, session, issue, ...s, ...(out ? { out } : {}) }))
  } catch (e) {
    console.log(JSON.stringify({ ok: false, session, issue, error: String(e && e.message || e) }))
  }
}
