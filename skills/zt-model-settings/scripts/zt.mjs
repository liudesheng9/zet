// Shared helper for the zt-model-fold, zt-model-settings and zt-model-restructure scripts (each skill carries a copy): runs `zt-digest call` on a JSON array of
// {tool, arguments} and parses its output. Temp files go to the OS temp folder.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { execFileSync } from 'node:child_process'

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'zt-restructure-'))
let seq = 0

/** Runs the calls; returns the raw output text. `allow` (addresses) limits writes; `movesLog` collects rename maps. */
export function call(calls, { allow, movesLog } = {}) {
  const file = path.join(tmp, `calls-${++seq}.json`)
  fs.writeFileSync(file, JSON.stringify(calls))
  const args = ['call', file]
  if (allow) {
    const allowFile = path.join(tmp, `allow-${seq}.txt`)
    fs.writeFileSync(allowFile, allow.join('\n') + '\n')
    args.push('-Allow', allowFile)
  }
  if (movesLog) args.push('-Moves', movesLog)
  // zt-digest exits 1 when any call in the batch fails; its output still names every call's result.
  try {
    return execFileSync('zt-digest', args, { maxBuffer: 1 << 28 }).toString()
  } catch (e) {
    if (e.stdout && /^\d+ (ok|error) /m.test(e.stdout.toString())) return e.stdout.toString()
    throw e
  }
}

/** The JSON results of read calls, in order. */
export function reads(calls) {
  return call(calls).split('\n').filter(l => /^\d+ \{/.test(l)).map(l => JSON.parse(l.slice(l.indexOf(' ') + 1)))
}

/** Writes; returns {ok, errors} counts and the error lines. */
export function writes(calls, allow) {
  const out = call(calls, { allow })
  const errors = out.split('\n').filter(l => /^\d+ error /.test(l))
  return { ok: out.split('\n').filter(l => /^\d+ ok /.test(l)).length, errors }
}

export const model = at => reads([{ tool: 'get_model', arguments: { at } }])[0]
export const tree = key => reads([{ tool: 'get_tree', arguments: { tree: key } }])[0].cards

/** Card bodies in the text view, 25 at a time (the full view can pass the 8 MiB read limit). */
export function cards(addresses) {
  const out = []
  for (let i = 0; i < addresses.length; i += 25) out.push(...reads([{ tool: 'get_cards', arguments: { addresses: addresses.slice(i, i + 25), view: 'text' } }])[0].cards)
  return out
}

/** One move; returns the rename map {old: new} for every Card it renumbered. */
export function move(args) {
  const log = path.join(tmp, `moves-${++seq}.log`)
  const out = call([{ tool: 'move_card', arguments: { ...args, confirm: 'move' } }], { movesLog: log })
  const ok = out.match(/^0 ok (\S+)/m)
  if (!ok) throw new Error(`move ${JSON.stringify(args)} failed: ${out.slice(0, 400)}`)
  const logged = fs.existsSync(log) ? fs.readFileSync(log, 'utf8').trim().split('\n').filter(Boolean).map(l => JSON.parse(l)) : []
  return Object.assign({ [args.at]: ok[1] }, ...logged.map(r => r.moved || {}))
}

export const short = (a, head) => (head && a.startsWith(head) ? 'H' + a.slice(head.length) : a)
