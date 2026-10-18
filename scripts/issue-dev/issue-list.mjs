#!/usr/bin/env node
// Resolves a list of GitHub issue numbers (and "a-b" ranges) into issue records for the zt-issue-dev workflow.
//   node scripts/issue-dev/issue-list.mjs --repo liudesheng9/zet --issues '[65,"70-72"]'
// The last stdout line is compact JSON: {ok, repo, issues: [{number, title, state, labels, url}], missing: [n, ...]}.
// Exits 1 when an issue is missing or the list is empty or malformed.
import { execFileSync } from 'node:child_process'

const flag = name => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : undefined
}
const done = (o, code = 0) => { console.log(JSON.stringify(o)); process.exit(code) }

export function expandIssues(list) {
  if (!Array.isArray(list)) throw new Error('issues must be a JSON array of numbers or "a-b" ranges')
  const out = []
  for (const x of list) {
    const s = String(x).trim().replace(/^#/, '')
    const range = s.match(/^#?(\d+)\s*-\s*#?(\d+)$/)
    if (range) {
      const a = Number(range[1]), b = Number(range[2])
      if (b < a || b - a > 500) throw new Error(`bad range ${s}`)
      for (let n = a; n <= b; n++) out.push(n)
    } else if (/^\d+$/.test(s)) out.push(Number(s))
    else throw new Error(`not an issue number or range: ${JSON.stringify(x)}`)
  }
  return [...new Set(out)]
}

if (/issue-list\.mjs$/.test(process.argv[1] || '')) {
  const repo = flag('repo')
  let numbers
  try { numbers = expandIssues(JSON.parse(flag('issues') || 'null')) } catch (e) { done({ ok: false, error: e.message }, 1) }
  if (!repo || !numbers.length) done({ ok: false, error: 'need --repo and a nonempty --issues list' }, 1)
  const issues = [], missing = []
  for (const n of numbers) {
    try {
      const raw = execFileSync('gh', ['issue', 'view', String(n), '--repo', repo, '--json', 'number,title,state,labels,url'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] })
      const j = JSON.parse(raw)
      issues.push({ number: j.number, title: j.title, state: j.state, labels: (j.labels || []).map(l => l.name), url: j.url })
    } catch (e) {
      missing.push(n)
    }
  }
  done({ ok: missing.length === 0, repo, count: issues.length, issues, missing }, missing.length ? 1 : 0)
}
