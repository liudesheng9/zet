// node model-survey.mjs <citationKey>
// Lists every Model Card of one Literature tree, with its level, parts, shared parts and the
// rests-on Links it makes into other Model Cards: the evidence that several Model Cards are one model.
import { reads, model } from './zt.mjs'

const key = process.argv[2]
if (!key) { console.error('usage: node model-survey.mjs <citationKey>'); process.exit(2) }
const rows = reads([{ tool: 'search_cards', arguments: { type: 'model', query: key, limit: 200 } }])[0].results
const heads = rows.filter(r => !r.facet && r.address.startsWith(key + '/')).map(r => r.address)
const owner = a => heads.filter(h => a === h || a.startsWith(h + '|')).sort((x, y) => y.length - x.length)[0]
console.log(`${heads.length} Model Card(s) in ${key}`)
for (const h of heads) {
  const m = model(h)
  const order = m.build.order || []
  const shared = order.filter(o => o.line === 'shared').length
  const into = {}
  for (const n of m.neighbours || []) {
    if (n.way !== 'out' || n.kind !== 'rests-on') continue
    const o = owner(n.to)
    if (o && o !== h) into[o.slice(key.length + 1)] = (into[o.slice(key.length + 1)] || 0) + 1
  }
  const notes = (m.head.body.match(/(本模型只[^。]*。|参照[^。]*的[^。]*时序[^。]*。)/g) || []).slice(0, 2).join(' ')
  console.log(`${h.slice(key.length + 1).padEnd(8)} ${String(m.level).padEnd(7)} parts ${String(order.length).padStart(3)} shared ${String(shared).padStart(2)}  rests on: ${JSON.stringify(into)}  ${m.head.title}${notes ? '  | ' + notes : ''}`)
}
console.log('One model when: later cards rest on earlier ones, take parts from them, and keep their agents, symbols and timing.')
