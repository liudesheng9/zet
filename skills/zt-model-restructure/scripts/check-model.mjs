// node check-model.mjs <modelHead>
// Prints the main line and the build faults of one model; self_sufficient must be true.
import { model } from './zt.mjs'

const head = process.argv[2]
if (!head) { console.error('usage: node check-model.mjs <modelHead>'); process.exit(2) }
const m = model(head)
const b = m.build
const lines = {}
for (const o of b.order) lines[o.line] = (lines[o.line] || 0) + 1
const sh = a => (a.startsWith(head) ? 'H' + a.slice(head.length) : a)
console.log(`members ${b.order.length} ${JSON.stringify(lines)}`)
console.log('main line: ' + b.order.filter(o => o.line === 'main').map(o => `${sh(o.address)} ${o.facet}`).join(' | '))
for (const k of ['closure', 'reach_in', 'forward', 'unanchored', 'beyond_level']) {
  for (const f of b[k] || []) console.log(`${k}: ${f.fix || JSON.stringify(f)}`)
}
const rel = {}
for (const r of m.relations || []) rel[r.kind] = (rel[r.kind] || 0) + 1
console.log(`relations ${JSON.stringify(rel)}; loose ${(m.loose || []).length}; self_sufficient ${b.self_sufficient}`)
process.exit(b.self_sufficient ? 0 : 1)
