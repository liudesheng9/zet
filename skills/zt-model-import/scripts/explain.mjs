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

export function plan(head, work) {
  fs.rmSync(work, { recursive: true, force: true })
  fs.mkdirSync(work, { recursive: true })
  const out = model('explain', work, head)
  return { work, parts: Number(out.parts || 0), jobs: out.jobs || [] }
}

export function apply(work) {
  const out = model('explain-apply', work)
  return { applied: out.applied || 0, refused: out.refused || [], blocked: out.blocked || [], jobs: out.jobs || [] }
}

export function finish(head, work) {
  const state = JSON.parse(fs.readFileSync(path.join(work, 'state.json'), 'utf8'))
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
  const again = plan(head, path.join(work, 'recheck'))
  const refused = Object.values(state.explain?.refused || {}).flat()
  return { changed, link, left: again.parts, refused }
}

function main() {
  const [cmd, head, ...rest] = process.argv.slice(2)
  const i = rest.indexOf('--work')
  if (!['plan', 'apply', 'finish'].includes(cmd) || !head) {
    console.error('usage: node explain.mjs plan|apply|finish <Model Card address> [--work <folder>]')
    process.exit(2)
  }
  const work = workOf(head, i >= 0 ? rest[i + 1] : undefined)
  if (cmd === 'plan') {
    const r = plan(head, work)
    console.log(r.parts ? `${r.parts} Derivation part(s) to explain in ${r.jobs.length} job(s); work ${r.work}` : 'nothing to explain')
    console.log(JSON.stringify(r))
  } else if (cmd === 'apply') {
    const r = apply(work)
    console.log(`applied ${r.applied}, refused ${r.refused.length}, blocked ${r.blocked.length}, follow-up jobs ${r.jobs.length}`)
    console.log(JSON.stringify(r))
  } else {
    const r = finish(head, work)
    console.log(`explained ${r.changed.length} part(s); Term links changed ${r.link.cards} Card(s), second plan ${r.link.second}; left ${r.left}; refused ${r.refused.length}`)
    console.log(JSON.stringify(r))
    if (r.left || r.link.second || r.refused.length) process.exit(1)
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
