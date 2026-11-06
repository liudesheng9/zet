// What DeepSeek agents really did in a run: tool calls per job, failed tools, self-check faults by kind.
// A manager deletes each conversation as it ends, so keep the sessions while the run goes, then report.
//
//   node scripts/book-digest/session-report.mjs keep <session folder name> <dest> [minutes, default 60]
//   node scripts/book-digest/session-report.mjs report <folder that holds session folders>
//
// A session folder is ~/.dsh/sessions/--D-zt-digest-<key>-figures-- (the job's cwd with its separators as dashes).
// `keep` runs in the background and ends by itself; stop it when the run ends. See docs/digest-cost-experience.md.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import zlib from 'node:zlib'

const MAGIC = Buffer.from([0x28, 0xb5, 0x2f, 0xfd])

// A session file is many zstd frames, one per flush; the magic number can also occur inside a frame.
export function decodeSession(buffer) {
  const parts = []
  let at = 0
  while (at < buffer.length) {
    const next = buffer.indexOf(MAGIC, at + 4)
    const end = next < 0 ? buffer.length : next
    try {
      parts.push(zlib.zstdDecompressSync(buffer.subarray(at, end)))
    } catch {
      if (next >= 0) {
        const more = buffer.indexOf(MAGIC, next + 4)
        const stop = more < 0 ? buffer.length : more
        try {
          parts.push(zlib.zstdDecompressSync(buffer.subarray(at, stop)))
          at = stop
          continue
        } catch { /* a torn tail: the session was still being written */ }
      }
    }
    at = end
  }
  return Buffer.concat(parts).toString('utf8')
}

const textOf = (message) => (message?.content || []).map((part) => part.text || '').join(' ')

export function readSession(file) {
  const events = decodeSession(fs.readFileSync(file)).trim().split('\n').flatMap((line) => {
    try { return [JSON.parse(line)] } catch { return [] }
  })
  const job = { calls: 0, faults: 0, failed: 0, reasoning: 0, kinds: {} }
  for (const event of events) {
    if (event.type === 'tool/call') job.calls++
    if (event.type === 'assistant/message') {
      for (const part of event.data?.message?.content || []) if (part.type === 'reasoning') job.reasoning += JSON.stringify(part).length
    }
    if (event.type !== 'tool/result') continue
    const text = textOf(event.data?.message)
    if (/^\s*Error/.test(text)) job.failed++
    const found = text.match(/faults: (\d+)/)
    if (!found || Number(found[1]) === 0) continue
    job.faults += Number(found[1])
    for (const line of text.split(/\r?\n/)) {
      if (!line.includes('`')) continue
      const kind = line.replace(/`[^`]*`/g, '`…`').trim().slice(0, 70)
      job.kinds[kind] = (job.kinds[kind] || 0) + 1
    }
  }
  return job
}

function report(root) {
  const files = []
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const at = path.join(dir, entry.name)
      if (entry.isDirectory()) walk(at)
      else if (entry.name === 'session.v4.jsonl.zstd') files.push(at)
    }
  }
  walk(root)
  const jobs = files.map(readSession).filter((job) => job.calls > 0)
  const calls = jobs.map((job) => job.calls).sort((a, b) => a - b)
  const kinds = {}
  for (const job of jobs) for (const [kind, n] of Object.entries(job.kinds)) kinds[kind] = (kinds[kind] || 0) + n
  console.log(JSON.stringify({
    jobs: jobs.length,
    jobs_with_faults: jobs.filter((job) => job.faults > 0).length,
    faults: jobs.reduce((sum, job) => sum + job.faults, 0),
    failed_tools: jobs.reduce((sum, job) => sum + job.failed, 0),
    calls_mean: jobs.length ? Number((calls.reduce((a, b) => a + b, 0) / jobs.length).toFixed(1)) : 0,
    calls_max: calls.at(-1) || 0,
    reasoning_chars_mean: jobs.length ? Math.round(jobs.reduce((sum, job) => sum + job.reasoning, 0) / jobs.length) : 0,
  }))
  for (const [kind, n] of Object.entries(kinds).sort((a, b) => b[1] - a[1]).slice(0, 12)) console.log(`${n}x ${kind}`)
}

async function keep(name, dest, minutes) {
  const source = path.join(os.homedir(), '.dsh', 'sessions', name)
  const target = path.join(dest, name)
  const end = Date.now() + minutes * 60000
  while (Date.now() < end) {
    if (fs.existsSync(source)) {
      for (const session of fs.readdirSync(source)) {
        const from = path.join(source, session, 'session.v4.jsonl.zstd')
        try {
          fs.mkdirSync(path.join(target, session), { recursive: true })
          fs.copyFileSync(from, path.join(target, session, 'session.v4.jsonl.zstd'))
        } catch { /* deleted between the listing and the copy */ }
      }
    }
    await new Promise((done) => setTimeout(done, 15000))
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname.replace(/^\/(?=[A-Za-z]:)/, ''))) {
  const [command, first, second, third] = process.argv.slice(2)
  if (command === 'report' && first) report(first)
  else if (command === 'keep' && first && second) await keep(first, second, Number(third || 60))
  else {
    console.error('usage: session-report.mjs keep <session folder name> <dest> [minutes] | report <folder>')
    process.exitCode = 2
  }
}
