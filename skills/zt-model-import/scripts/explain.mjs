// The explanation refresh of one model, the last step of the model passes: the passes move and extend proof
// chains, so a Derivation part may lack 思路/解读 or keep a stale position line (`第 k 张，共 n 张`). This runs the
// digest's own explain steps on this one model, without a workflow, a staging folder or a run ledger:
//   plan   <head>  zt-digest model -Step explain: the job(s) for the parts that need it (one per model)
//   apply  <head>  zt-digest model -Step explain-apply: writes the accepted 思路/说明/解读; lists follow-up jobs
//   finish <head>  Term-link on the changed parts only (-Cards), then the explain plan again (must find nothing)
// The DeepSeek conversations between plan and apply are the caller's (SKILL.md, step 8).
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { reads } from './zt.mjs'

/** The work folder of one model's refresh. */
export const workOf = (head, given) => given || path.join(os.tmpdir(), 'zt-model-explain', head.replace(/[^A-Za-z0-9]+/g, '-'))

/** The book of a Model Card address. */
export const keyOf = head => head.split('/')[0]

/** The JSON of a command's last non-empty line. */
export function lastJson(text) {
  const lines = String(text).split(/\r?\n/).map(l => l.trim()).filter(Boolean)
  for (let i = lines.length - 1; i >= 0; i--) if (lines[i].startsWith('{')) return JSON.parse(lines[i])
  throw new Error(`no JSON line in: ${String(text).slice(-400)}`)
}

/** The parts the explain jobs wrote: every job's done address that was neither refused nor blocked. */
export function changedParts(state) {
  const ex = state.explain || {}
  const not = new Set()
  for (const field of ['refused', 'blocked']) for (const texts of Object.values(ex[field] || {})) for (const t of texts || []) not.add(String(t).split(': ')[0])
  const out = []
  for (const job of Object.values(ex.jobs || {})) for (const a of job.done || []) if (!not.has(a) && !out.includes(a)) out.push(a)
  return out
}

function digest(args, cwd) {
  try {
    return execFileSync('zt-digest', args, { cwd, maxBuffer: 1 << 28 }).toString()
  } catch (e) {
    throw new Error(`zt-digest ${args.slice(0, 2).join(' ')} failed: ${(e.stdout || '').toString().slice(-600)}${(e.stderr || '').toString().slice(-600)}`)
  }
}

const model = (step, work, head) => lastJson(digest(['model', '-Step', step, '-Work', work, ...(head ? ['-Models', head] : [])], work))

const WORK_MARKER = '.zt-model-explain-work'

function readLiveArchiveIdentity() {
  const res = reads([{ tool: 'get_cards', arguments: { addresses: [], view: 'text' } }])[0]
  const arch = res?.archive_identity
  if (!arch || typeof arch.id !== 'string' || !arch.id || typeof arch.path !== 'string' || !arch.path) {
    throw new Error('could not read archive_identity {id, path} from zt service')
  }
  return { id: arch.id, path: arch.path }
}

function readMarker(work) {
  const markerPath = path.join(work, WORK_MARKER)
  if (!fs.existsSync(markerPath)) return null
  const raw = fs.readFileSync(markerPath, 'utf8').trim()
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw)
    if (parsed && typeof parsed === 'object') return parsed
  } catch {
    return null
  }
  return null
}

function writeMarker(work, head, archive) {
  fs.writeFileSync(path.join(work, WORK_MARKER), JSON.stringify({ head, archive }) + '\n')
}

export function assertWorkIdentity(head, work, archive) {
  const marker = readMarker(work)
  if (!marker || typeof marker.head !== 'string' || !marker.head) {
    throw new Error(`work directory ${work} has no valid ${WORK_MARKER} marker (missing head): run explain.mjs plan or pass --force`)
  }
  if (marker.head !== head) {
    throw new Error(`work directory ${work} belongs to ${marker.head}, not ${head}: use a separate --work directory or pass --force`)
  }
  if (!marker.archive || typeof marker.archive.id !== 'string' || !marker.archive.id || typeof marker.archive.path !== 'string' || !marker.archive.path) {
    throw new Error(`work directory ${work} marker lacks archive {id, path} identity: refusing to resume without verified archive binding (pass --force to reset)`)
  }
  if (archive) {
    if (marker.archive.id !== archive.id || marker.archive.path !== archive.path) {
      throw new Error(`work directory ${work} belongs to archive ${marker.archive.id} (${marker.archive.path}), not ${archive.id} (${archive.path}): refusing to resume across archives`)
    }
  }
}

/** Compiler completion predicate: a job is unfinished if any assigned address is absent from `done`
 * or `done` is empty, or if `finish()` has not yet recorded clean completion (`finished_ok: true`). */
export function hasUnfinishedState(state) {
  const ex = state?.explain || {}
  if (ex.finished_ok === true) return false
  const jobs = Object.values(ex.jobs || {})
  if (!jobs.length) return false
  for (const job of jobs) {
    const done = new Set(job.done || [])
    const assigned = job.addresses || job.parts || []
    if (assigned.length > 0) {
      if (assigned.some((a) => !done.has(a))) return true
    } else if (done.size === 0) {
      return true
    }
  }
  // Even if every assigned address is in `done`, `finish()` hasn't marked `finished_ok: true` yet.
  return true
}

export function plan(head, work, { force = false, archive = null } = {}) {
  let liveArchive = archive || null
  if (fs.existsSync(work)) {
    const entries = fs.readdirSync(work)
    const isToolOwned = fs.existsSync(path.join(work, WORK_MARKER))
    if (entries.length > 0 && !isToolOwned) {
      throw new Error(`refusing to wipe existing non-empty directory ${work} (not created by explain.mjs): choose a fresh --work path`)
    }
    if (isToolOwned && !force) {
      if (!liveArchive) liveArchive = readLiveArchiveIdentity()
      assertWorkIdentity(head, work, liveArchive)
      const stateFile = path.join(work, 'state.json')
      if (fs.existsSync(stateFile)) {
        let prev
        try {
          prev = JSON.parse(fs.readFileSync(stateFile, 'utf8'))
        } catch (e) {
          throw new Error(`refusing to wipe unreadable state.json in ${work} (${e.message}): inspect it or pass --force to reset`)
        }
        if (hasUnfinishedState(prev)) {
          throw new Error(`refusing to wipe unfinished explain state in ${work}: resume apply/finish or pass --force to reset`)
        }
      }
    }
    // Resolve live archive identity BEFORE removing `work` on both normal and --force paths.
    if (!liveArchive) liveArchive = readLiveArchiveIdentity()
    fs.rmSync(work, { recursive: true, force: true })
  }
  if (!liveArchive) liveArchive = readLiveArchiveIdentity()
  fs.mkdirSync(work, { recursive: true })
  writeMarker(work, head, liveArchive)
  const out = model('explain', work, head)
  return { work, parts: Number(out.parts || 0), jobs: out.jobs || [] }
}

export function apply(work, head, { archive = null } = {}) {
  assertWorkIdentity(head, work, archive || readLiveArchiveIdentity())
  const out = model('explain-apply', work)
  return { applied: out.applied || 0, refused: out.refused || [], blocked: out.blocked || [], jobs: out.jobs || [] }
}

export function finish(head, work, { archive = null } = {}) {
  const liveArchive = archive || readLiveArchiveIdentity()
  assertWorkIdentity(head, work, liveArchive)
  const statePath = path.join(work, 'state.json')
  const state = JSON.parse(fs.readFileSync(statePath, 'utf8'))
  const changed = changedParts(state)
  const key = keyOf(head)
  let link = { cards: 0, second: 0 }
  if (changed.length) {
    // A diagnostic Term-link pass over exactly these Cards: a minimal staging folder names the book.
    const staging = path.join(work, 'term-staging')
    fs.mkdirSync(staging, { recursive: true })
    const root = reads([{ tool: 'get_card', arguments: { at: key } }])[0]
    fs.writeFileSync(path.join(staging, 'manifest.json'), JSON.stringify({ citation_key: key, book_title: root.title || key }))
    const cards = changed.join(',')
    const first = lastJson(digest(['term-link', '-Step', 'plan', '-Staging', staging, '-Cards', cards], work))
    // The apply repeats the plan's -Cards subset: the plan freezes it, and another subset is a stale plan.
    if (first.cards_changed) digest(['term-link', '-Step', 'apply', '-Staging', staging, '-Cards', cards, '-Apply'], work)
    const second = lastJson(digest(['term-link', '-Step', 'plan', '-Staging', staging, '-Cards', cards], work))
    link = { cards: first.cards_changed || 0, second: second.cards_changed || 0 }
  }
  const again = plan(head, path.join(work, 'recheck'), { force: true, archive: liveArchive })
  const refused = Object.values(state.explain?.refused || {}).flat()
  const blocked = Object.values(state.explain?.blocked || {}).flat()
  if (!again.parts && !link.second && !refused.length && !blocked.length) {
    state.explain = { ...(state.explain || {}), finished_ok: true }
    fs.writeFileSync(statePath, JSON.stringify(state, null, 2))
  }
  return { changed, link, left: again.parts, refused, blocked }
}

function main() {
  const [cmd, head, ...rest] = process.argv.slice(2)
  const i = rest.indexOf('--work')
  const force = rest.includes('--force')
  if (!['plan', 'apply', 'finish'].includes(cmd) || !head) {
    console.error('usage: node explain.mjs plan|apply|finish <Model Card address> [--work <folder>] [--force]')
    process.exit(2)
  }
  const work = workOf(head, i >= 0 ? rest[i + 1] : undefined)
  if (cmd === 'plan') {
    const r = plan(head, work, { force })
    console.log(r.parts ? `${r.parts} Derivation part(s) to explain in ${r.jobs.length} job(s); work ${r.work}` : 'nothing to explain')
    console.log(JSON.stringify(r))
  } else if (cmd === 'apply') {
    const r = apply(work, head)
    console.log(`applied ${r.applied}, refused ${r.refused.length}, blocked ${r.blocked.length}, follow-up jobs ${r.jobs.length}`)
    console.log(JSON.stringify(r))
  } else {
    const r = finish(head, work)
    console.log(`explained ${r.changed.length} part(s); Term links changed ${r.link.cards} Card(s), second plan ${r.link.second}; left ${r.left}; refused ${r.refused.length}; blocked ${r.blocked.length}`)
    console.log(JSON.stringify(r))
    if (r.left || r.link.second || r.refused.length || r.blocked.length) process.exit(1)
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
