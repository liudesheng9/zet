// node leads.mjs <key> <leads.json> [--commit]
// Gives each division a summary lead, one batch per entry, deepest first. An entry, in full addresses (<key>/<location>)
// of the tree as it stands when the run starts:
//   { lead, mode: "demote", title, body, keep: [Side children that stay with the old lead], first?: [Side children placed before the old lead], old_title?, old_drop?: [line starts] }
//   { lead, mode: "rewrite", title, body }
// demote: a new summary Card takes the lead's place, the old lead becomes its first Side branch (with `keep` and its
// Direct chain), every other Side child moves up to the summary with its own Direct chain. rewrite: the lead keeps its place with a new text.
// Term names in the new text become Term uses, the first time each appears. Resumable: <leads.json>.state.json.
import fs from 'node:fs'
import { call, tree } from './zt.mjs'

/** Wraps the first plain occurrence of each term in a Term use; Links, formulas and the source line stay as they are. */
export function linkTerms(body, terms) {
  const hidden = []
  const hide = s => { hidden.push(s); return `\u0001${hidden.length - 1}\u0002` }
  let text = body.replace(/\[\[[^\]]*\]\]|\$[^$\n]*\$|^来源[:：].*$/gm, hide)
  for (const [term, address] of [...terms].sort((a, b) => b[0].length - a[0].length)) {
    if (text.includes(term)) text = text.replace(term, () => hide(`[[${term}@${address}]]`))
  }
  const show = s => s.replace(/\u0001(\d+)\u0002/g, (_, n) => show(hidden[+n]))
  return show(text)
}

/** The Direct chain that runs on from a Card, in order; empty for a Card on the top line, whose chain is the top line. */
export function chainAfter(now, address) {
  const out = []
  if (/\/\d+$/.test(address)) return out
  for (let c = now.find(k => k.parent === address && k.edge === 'direct'); c; c = now.find(k => k.parent === c.address && k.edge === 'direct')) out.push(c)
  return out
}

/** One demote batch: a new summary takes the lead's place, the old lead (with `keep` and its Direct chain) becomes its
 *  first Side branch after `first`, and every other Side child moves up to the summary with its own Direct chain. */
export function demoteItems({ now, lead, title, body, keep = [], first = [], run }) {
  const item = (tool, args, n) => ({ op_key: `${run}/1/step/i${n}`, client_ref: `op${n}`, tool, arguments: args })
  const items = [
    { ...item('create_successor', { at: lead, kind: 'side', title, body }, 0), client_ref: 's' },
    item('move_span', { from: '@@s', before: lead }, 1),
  ]
  const through = a => { const chain = chainAfter(now, a); return chain.length ? { through: chain.at(-1).address } : {} }
  const up = a => items.push(item('move_span', { from: a, ...through(a), side_of: '@@s' }, items.length))
  first.forEach(up)
  items.push(item('move_span', { from: lead, ...through(lead), side_of: '@@s' }, items.length))
  for (const c of now.filter(k => k.parent === lead)) if (c.edge === 'side' && !keep.includes(c.address) && !first.includes(c.address)) up(c.address)
  return items
}

function send(args) {
  const out = call([{ tool: 'apply_batch', arguments: args }])
  const line = out.split('\n').find(l => /^\d+ \{/.test(l))
  const reply = line && JSON.parse(line.slice(line.indexOf(' ') + 1))
  if (!reply || (!reply.plan && !reply.applied)) { console.log(out.slice(0, 2000)); process.exit(1) }
  return reply
}

function edit(args) {
  const out = call([{ tool: 'edit_card', arguments: args }])
  if (!/^0 ok /m.test(out)) { console.log(args.at, out.slice(0, 1200)); process.exit(1) }
}

const depth = a => a.split(/[|/]/).length

if (process.argv[1].endsWith('leads.mjs')) {
  const [key, file, flag] = process.argv.slice(2)
  const K = `${key}/`
  const stateFile = `${file}.state.json`
  const st = fs.existsSync(stateFile) ? JSON.parse(fs.readFileSync(stateFile)) : { done: [], map: {} }
  const rev = new Map(Object.entries(st.map).map(([o, c]) => [c, o]))
  const T = a => st.map[a] ?? a
  // A Link to another entry's lead means that division, so it reaches the division's summary; a lead's own text
  // links its old Card by its old address.
  const follow = (body, self) => body.replace(new RegExp(`\\[\\[(?:[^\\]@]*@)?${key}/[^\\]]*\\]\\]`, 'g'), m => {
    const at = m.lastIndexOf(K)
    const address = m.slice(at, -2)
    return m.slice(0, at) + (address === self ? T(address) : st.map[`@${address}`] ?? T(address)) + ']]'
  })
  const entries = JSON.parse(fs.readFileSync(file)).map((e, i) => ({ ...e, i })).sort((a, b) => depth(b.lead) - depth(a.lead) || a.i - b.i)
  const start = tree(key)
  const terms = start.filter(c => c.type === 'term' && c.term_role === 'card').map(c => [c.term, c.address])
  for (const e of entries) {
    if (st.done.includes(e.lead)) continue
    const lead = T(e.lead)
    const body = linkTerms(follow(e.body, e.lead), terms)
    if (flag !== '--commit') { console.log(`${e.mode} ${lead}: ${e.title}`); continue }
    if (e.mode === 'rewrite') {
      edit({ at: lead, title: e.title, body })
    } else {
      const now = tree(key)
      const run = `organize/${key}-${process.pid}-${e.i}`
      const items = demoteItems({ now, lead, title: e.title, body, keep: (e.keep ?? []).map(T), first: (e.first ?? []).map(T), run })
      if (items.length > 64) throw new Error(`${lead}: ${items.length} items`)
      const pre = send({ items, preview: true, lean: true })
      if (!pre.complete || pre.plan.links_that_will_break.length) { console.log(lead, JSON.stringify(pre.refusals).slice(0, 1500)); process.exit(1) }
      send({ items, confirm: pre.plan_hash })
      const updates = Object.entries(pre.plan.moved).map(([f, to]) => [rev.get(f) ?? f, f, to])
      for (const [, f] of updates) rev.delete(f)
      for (const [o, , to] of updates) { st.map[o] = to; rev.set(to, o) }
      st.map[`@${e.lead}`] = lead
      rev.set(lead, `@${e.lead}`)
      const old = reads1(T(e.lead))
      const shown = l => l.replace(/\[\[([^\]@]+)@[^\]]+\]\]/g, '$1')
      const drop = ['本讲各节：', ...(e.old_drop ?? [])]
      const stripped = old.body.split('\n').filter(l => !drop.some(d => shown(l).startsWith(d))).join('\n').replace(/\n{3,}/g, '\n\n')
      if (stripped !== old.body || e.old_title) edit({ at: old.address, ...(e.old_title ? { title: e.old_title } : {}), ...(stripped !== old.body ? { body: stripped } : {}) })
    }
    st.done.push(e.lead)
    fs.writeFileSync(stateFile, JSON.stringify(st))
    console.log(`${e.mode} ${e.lead} -> ${lead}`)
  }
  if (flag !== '--commit') process.exit(0)
  // Every Card has settled: the summaries' Links reach their final summaries, and the root's section headings too.
  for (const e of entries) edit({ at: st.map[`@${e.lead}`] ?? T(e.lead), body: linkTerms(follow(e.body, e.lead), terms) })
  const root = reads1(key)
  const toSummary = new Map(entries.filter(e => e.mode === 'demote').map(e => [T(e.lead), st.map[`@${e.lead}`]]))
  const headed = root.body.split('\n').map(l => l.startsWith('## ') ? l.replace(/\[\[([^\]]+)\]\]/g, (m, a) => (toSummary.has(a) ? `[[${toSummary.get(a)}]]` : m)) : l).join('\n')
  if (headed !== root.body) edit({ at: key, body: headed })
}

function reads1(at) {
  const out = call([{ tool: 'get_card', arguments: { at } }])
  const line = out.split('\n').find(l => /^\d+ \{/.test(l))
  return JSON.parse(line.slice(line.indexOf(' ') + 1))
}
