// node check.mjs <key> [<snapshot.tree.json>]
// Structure only, never the form of a Card. Prints the top line with each Card's own branch size, then faults:
// Links broken in this tree, a list (Term, Cited) not at the tail, and, against the snapshot, a changed Card
// count or a whole unit that changed size.
import fs from 'node:fs'
import { tree, reads } from './zt.mjs'
import { isWhole, children, size, unitSize } from './outline.mjs'

/** Whole units whose size changed between two snapshots. Units are matched by type and title; several units may
 *  share one (the same specification in two empirical models), so each title compares its sorted list of sizes. */
export function unitChanges(old, list) {
  const sizes = (cards) => {
    const kids = children(cards), out = new Map()
    for (const c of cards.filter(isWhole)) {
      const k = `${c.type} ${c.title}`
      out.set(k, [...(out.get(k) || []), unitSize(kids, c)])
    }
    for (const v of out.values()) v.sort((a, b) => a - b)
    return out
  }
  const before = sizes(old), now = sizes(list), faults = []
  for (const [k, want] of before) {
    const got = now.get(k)
    if (got === undefined || got.join() === want.join()) continue
    faults.push(`${k.slice(k.indexOf(' ') + 1, k.indexOf(' ') + 41)}: ${want.join('+')} Cards before, ${got.join('+')} now`)
  }
  return faults
}

if (process.argv[1].endsWith('check.mjs')) {
  const [key, snapshot] = process.argv.slice(2)
  const list = tree(key)
  const kids = children(list)
  const top = []
  for (let c = list.find(x => x.edge === 'root'); c; c = (kids[c.address] || []).find(k => k.edge === 'direct')) if (c.edge === 'direct') top.push(c)
  top.forEach((c, i) => {
    const own = size(kids, c.address) - (top[i + 1] ? size(kids, top[i + 1].address) : 0)
    console.log(`${c.address.slice(key.length + 1).padStart(4)} ${c.type.padEnd(6)} ${String(own).padStart(5)} ${c.title.slice(0, 50)}`)
  })

  const faults = []
  const isList = c => c.type === 'cited' || (c.type === 'term' && !c.term_list)
  const lists = top.filter(isList)
  if (list.filter(c => isList(c) && !list.find(p => p.address === c.parent && isList(p))).length !== lists.length) faults.push('a list is not on the top line')
  const first = top.findIndex(isList)
  if (first >= 0 && top.slice(first).some(c => !isList(c))) faults.push('a content Card stands after a list')

  const broken = reads([{ tool: 'list_broken_links', arguments: {} }])[0]?.broken ?? []
  for (const b of broken) if (JSON.stringify(b).includes(`${key}/`)) faults.push(`broken: ${JSON.stringify(b).slice(0, 160)}`)

  if (snapshot) {
    const old = JSON.parse(fs.readFileSync(snapshot))
    if (old.length !== list.length) faults.push(`${old.length} Cards before, ${list.length} now`)
    faults.push(...unitChanges(old, list))
  }
  console.log(faults.length ? faults.join('\n') : 'no fault')
  process.exit(faults.length ? 1 : 0)
}
