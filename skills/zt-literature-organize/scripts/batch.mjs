// node batch.mjs <key> <ops-file> [--commit]
// One all-or-nothing tree batch from a plain ops file; addresses are relative to the key:
//   move <from>[..<through>] side_of|after|before <dest>
//   reorder <parent> <child> <child> ...        every Side child, in the new order
// Previews always; --commit commits only a complete preview that breaks no Link. Writes <ops-file>.moved.json.
import fs from 'node:fs'
import { call } from './zt.mjs'

export function parseOps(text, key) {
  const a = s => `${key}/${s}`
  const lines = text.split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#'))
  return lines.map((line, n) => {
    const [op, ...rest] = line.split(/\s+/)
    if (op === 'move') {
      const [span, rel, dest] = rest
      if (!['side_of', 'after', 'before'].includes(rel) || !dest) throw new Error(`line ${n + 1}: ${line}`)
      const [from, through] = span.split('..')
      return { tool: 'move_span', arguments: { from: a(from), ...(through ? { through: a(through) } : {}), [rel]: a(dest) } }
    }
    if (op === 'reorder') {
      if (rest.length < 2) throw new Error(`line ${n + 1}: ${line}`)
      return { tool: 'reorder', arguments: { parent: a(rest[0]), side: rest.slice(1).map(a) } }
    }
    throw new Error(`line ${n + 1}: ${line}`)
  })
}

/** One apply_batch call; a refused call prints the server's answer and stops. */
function send(args) {
  const out = call([{ tool: 'apply_batch', arguments: args }])
  const line = out.split('\n').find(l => /^\d+ \{/.test(l))
  const reply = line && JSON.parse(line.slice(line.indexOf(' ') + 1))
  if (!reply || (!reply.plan && !reply.applied)) { console.log(out.slice(0, 3000)); process.exit(1) }
  return reply
}

if (process.argv[1].endsWith('batch.mjs')) {
  const [key, file, flag] = process.argv.slice(2)
  const run = `organize/${key}-${process.pid}-${Date.now()}`
  const items = parseOps(fs.readFileSync(file, 'utf8'), key).map((item, n) => ({ op_key: `${run}/1/step/i${n}`, client_ref: `op${n + 1}`, ...item }))
  if (items.length > 64) throw new Error(`${items.length} items: a batch takes at most 64; split the file`)
  const pre = send({ items, preview: true, lean: true })
  const { moved, links_that_will_break: broken, link_macros_rewritten: rewritten } = pre.plan
  fs.writeFileSync(`${file}.moved.json`, JSON.stringify(moved))
  console.log(`preview: complete ${pre.complete}, refusals ${JSON.stringify(pre.refusals)}, breaks ${broken.length}, rewrites ${rewritten}, renamed ${Object.keys(moved).length}`)
  if (flag !== '--commit') process.exit(0)
  if (!pre.complete || broken.length) { console.log('not committed'); process.exit(1) }
  console.log(`applied ${send({ items, confirm: pre.plan_hash }).applied}`)
}
