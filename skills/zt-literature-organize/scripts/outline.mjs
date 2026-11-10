// node outline.mjs <key> [--out <dir>] [--depth <n>]
// Saves the tree with bodies to <dir>/<key>.tree.json (the snapshot check.mjs compares with) and prints an
// outline: address, type, title and the Card's source line when it has one, whatever form that line takes.
// A Card that is a whole unit (a Model Card, a Term list, a Cited list) prints as one row with its size.
import fs from 'node:fs'
import path from 'node:path'
import { tree, cards } from './zt.mjs'

const NOTE_TYPES = ['idea', 'event', 'finding']
export const isWhole = c => !NOTE_TYPES.includes(c.type) && c.edge !== 'root'

/** The last body line naming a source, loosely; '' when the Card has none. */
export function sourceLine(body = '') {
  const lines = body.split('\n').filter(l => /^\s*(来源|出处|Source|Sources|Ref)\s*[:：]/i.test(l))
  return lines.length ? lines[lines.length - 1].replace(/\[\[[^\]]*\]\]/g, '').replace(/^\s*\S+?[:：]\s*/, '').trim() : ''
}

export function children(list) {
  const kids = {}
  for (const c of list) if (c.parent) (kids[c.parent] ||= []).push(c)
  return kids
}

export function size(kids, address) {
  return 1 + (kids[address] || []).reduce((n, k) => n + size(kids, k.address), 0)
}

/** A whole unit's own Cards: the unit's size without the top line that runs on after it. */
export function unitSize(kids, c) {
  const onTop = /\/\d+$/.test(c.address)
  return 1 + (kids[c.address] || []).filter(k => !(onTop && k.edge === 'direct')).reduce((n, k) => n + size(kids, k.address), 0)
}

/** The outline's lines. A whole unit prints as one row; on the top line its Direct successor is the next top
 * Card, not part of the unit, so the walk goes on there (a Term list that is not last hid every Card after it). */
export function outline(full, key, maxDepth = Infinity) {
  const kids = children(full)
  const lines = []
  const walk = (c, depth) => {
    const whole = isWhole(c)
    const src = sourceLine(c.body)
    lines.push(`${'  '.repeat(depth)}${c.address.slice(key.length + 1) || key} [${c.type}${whole ? ` ${unitSize(kids, c)}` : ''}] ${c.title.slice(0, 50)}${src ? `  <${src.slice(0, 30)}>` : ''}`)
    const onTop = /\/\d+$/.test(c.address)
    if (!whole && depth < maxDepth) for (const k of kids[c.address] || []) if (k.edge === 'side') walk(k, depth + 1)
    if (!whole || onTop) for (const k of kids[c.address] || []) if (k.edge === 'direct') walk(k, depth)
  }
  walk(full.find(c => c.edge === 'root'), 0)
  return lines
}

if (process.argv[1].endsWith('outline.mjs')) {
  const argv = process.argv.slice(2)
  const key = argv[0]
  const opt = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1] }
  const out = opt('--out') ?? '.'
  const maxDepth = Number(opt('--depth') ?? Infinity)
  const list = tree(key)
  const bodies = Object.fromEntries(cards(list.map(c => c.address)).map(c => [c.address, c.body]))
  const full = list.map(c => ({ ...c, body: bodies[c.address] ?? '' }))
  fs.writeFileSync(path.join(out, `${key}.tree.json`), JSON.stringify(full))
  for (const l of outline(full, key, maxDepth)) console.log(l)
}
