#!/usr/bin/env node
// Reports the state the zt-issue-dev workflow requires after each issue: the checkout is clean and pushed, the
// issue state on GitHub, and the deployed zt matches the source version with the service up. Read-only: it fetches
// from origin but changes nothing.
//   node scripts/issue-dev/repo-state.mjs --repo liudesheng9/zet --cwd E:/zet --issue 65
// The last stdout line is compact JSON:
//   {ok, clean, branch, head, upstream, pushed, dirty: [...], issue: {number, state}, zt: {source, installed, match, service}}
// ok is true when clean, pushed, zt matches the source version and the service is up. Always exits 0 unless
// the arguments are wrong, so the caller reads the facts instead of a failed command.
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import path from 'node:path'

const flag = name => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : undefined
}
const repo = flag('repo'), cwd = flag('cwd'), issue = Number(flag('issue'))
if (!repo || !cwd || !issue) { console.log(JSON.stringify({ ok: false, error: 'need --repo, --cwd and --issue' })); process.exit(1) }

const run = (cmd, args) => {
  try { return execFileSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 120000 }).trim() }
  catch (e) { return null }
}

const dirty = (run('git', ['status', '--porcelain']) ?? '?').split('\n').filter(Boolean)
const branch = run('git', ['rev-parse', '--abbrev-ref', 'HEAD'])
run('git', ['fetch', '--quiet', 'origin'])
const head = run('git', ['rev-parse', 'HEAD'])
const upstream = run('git', ['rev-parse', '@{u}'])
let issueState = null
const iv = run('gh', ['issue', 'view', String(issue), '--repo', repo, '--json', 'state'])
try { issueState = JSON.parse(iv).state } catch {}

let source = null
try { source = (readFileSync(path.join(cwd, 'Cargo.toml'), 'utf8').match(/^version\s*=\s*"([^"]+)"/m) || [])[1] || null } catch {}
const installed = ((run('zt', ['version']) || '').match(/\d+\.\d+\.\d+\S*/) || [])[0] || null
const service = ((run('zt', ['status']) || '').match(/^state:\s*(\S+)/m) || [])[1] || null

const out = {
  clean: dirty.length === 0, branch, head, upstream, pushed: !!head && head === upstream, dirty: dirty.slice(0, 20),
  issue: { number: issue, state: issueState },
  zt: { source, installed, match: !!source && source === installed, service },
}
out.ok = out.clean && out.pushed && out.zt.match && service === 'up'
console.log(JSON.stringify(out))
