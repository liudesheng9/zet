// node check-settings.mjs <modelHead> [--json]
// Checks that a model keeps every setting, assumption and symbol in its setting part, and nothing
// else there. The setting part is the setting card (get_model's build.setting) with everything filed
// in its Side branches; the chapters are its Direct successors. Faults:
//   outside   a setting, assumption or notation card filed in a chapter: move it into the setting
//             part; its own derivations (filed under it) stay in the chapter
//   inside    a card in the setting part that is not a setting, an assumption or the notation card
//             (a derivation, a result): move it out, after its latest premise
//   stated    a chapter card that states an assumption itself (a **设定**/**假设**/**本章设定** section,
//             a line that opens with 假设/假定, a symbol table): write it in a setting card, point to it
//   notation  a symbol a card introduces (记 $X$ …, 记号：$X=…$) that the notation card does not define
//   missing   no notation card in the setting part
// Advice (`uses`): lines that use an assumption mid-sentence (由于假设 …); read them, they normally stay.
// Exit 0 when every setting is in the setting part, 1 otherwise.
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const SETTING = /^(设定|假设|假定|本章设定|本阶段的设定)/
const NOTATION = /^符号/
const HEADER = /^\*\*(本章设定|本阶段的设定|设定|假设|假定|记号|符号|符号表)\*\*/
const LIST = /^\s*(?:[-*](?=\s)|\d+[.、])\s*/
const DEFINES = /(?:^|[\s。；，：:、(（])记(?:号[:：])?\s*\$([^$]+)\$/g

const isSetting = (c) => SETTING.test(c.facet || '') || SETTING.test(c.title || '')
const isNotation = (c) => NOTATION.test(c.facet || '') || NOTATION.test(c.title || '')
const symbolOf = (tex) => tex.split(/=|\\equiv|\\coloneqq/)[0].replace(/\s+/g, '')
const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/** The settings audit of a model: `m` is get_model output, `cards` maps address -> {facet, title, body}. */
export function audit(m, cards) {
  const S = m.build.setting
  const order = m.build.order
  const inPart = (a) => a === S || (a.startsWith(S + '|') && /^\|[a-z]/.test(a.slice(S.length)))
  const mains = order.filter((o) => o.line === 'main').map((o) => o.address)
  const chapterOf = (a) => mains.filter((h) => a === h || a.startsWith(h + '|')).sort((x, y) => y.length - x.length)[0] || null
  const row = (a) => ({ ...order.find((o) => o.address === a), ...(cards.get(a) || {}) })
  const all = order.map((o) => row(o.address))

  const notes = all.filter((c) => inPart(c.address) && isNotation(c))
  const missing = notes.length ? [] : [{ fix: `the setting part of ${S} has no notation card (facet 符号表与读法): add one as its first Side successor` }]
  const noteText = notes.map((c) => c.body || '').join('\n').replace(/\s+/g, '')

  const outside = []
  const placed = all.filter((c) => !inPart(c.address) && (isSetting(c) || isNotation(c)))
  for (const c of placed) {
    // A setting filed under another misplaced setting moves with it.
    if (placed.some((p) => p !== c && c.address.startsWith(p.address + '|'))) continue
    const leave = all.filter((d) => d.address.startsWith(c.address + '|') && !isSetting(d) && !isNotation(d)).map((d) => d.address)
    const ch = chapterOf(c.address)
    const fix = `${c.address} (${c.facet}) is a ${isNotation(c) ? 'notation card' : 'setting'} filed in ${ch}: move it into the setting part, as a Side successor of ${S} or of the setting it refines` +
      (leave.length ? `; first file its own derivations ${leave.join(', ')} in ${ch} (under the card that uses them), they stay in the chapter` : '')
    outside.push({ card: c.address, facet: c.facet, chapter: ch, leave, fix })
  }

  const inside = all
    .filter((c) => inPart(c.address) && c.address !== S && !isSetting(c) && !isNotation(c))
    .map((c) => ({ card: c.address, facet: c.facet, fix: `${c.address} (${c.facet}) is not a setting: move it out of the setting part, after its latest premise (check-network names the chapter); a setting states the assumption only` }))

  const stated = []
  const uses = []
  const notation = []
  for (const c of all) {
    const lines = (c.body || '').split('\n')
    if (!inPart(c.address) && !isSetting(c) && !isNotation(c)) {
      for (const l of lines) {
        const t = l.replace(LIST, '').trim()
        const h = t.match(HEADER)
        if (h) stated.push({ card: c.address, what: h[0] })
        else if (/^\|\s*符号\s*\|/.test(t)) stated.push({ card: c.address, what: 'symbol table' })
        else if (/^(假设|假定)/.test(t)) stated.push({ card: c.address, what: t })
        else if (/假设|假定/.test(t)) uses.push({ card: c.address, line: t })
      }
    }
    if (isNotation(c) && inPart(c.address)) continue
    const seen = new Set()
    for (const x of (c.body || '').matchAll(DEFINES)) {
      const sym = symbolOf(x[1])
      if (!sym || seen.has(sym)) continue
      seen.add(sym)
      if (!new RegExp('\\$' + escape(sym) + '(\\$|=|\\\\equiv|,|，)').test(noteText)) notation.push({ card: c.address, symbol: sym })
    }
  }
  for (const s of stated) s.fix = `${s.card} states an assumption itself (${s.what.slice(0, 60)}): write it in a setting card and let ${s.card} rest on it (依赖 [[setting]])`
  for (const n of notation) n.fix = `${n.card} introduces $${n.symbol}$: define it in the notation card${notes[0] ? ' ' + notes[0].address : ''}`

  const ok = !missing.length && !outside.length && !inside.length && !stated.length && !notation.length
  return { setting: S, notation_card: notes[0]?.address || null, outside, inside, stated, notation, missing, uses, ok }
}

async function main() {
  const [head, flag] = process.argv.slice(2)
  if (!head) { console.error('usage: node check-settings.mjs <modelHead> [--json]'); process.exit(2) }
  const { model, cards } = await import('./zt.mjs')
  const m = model(head)
  if (!m.head || m.head.address !== head) { console.error(`${head} is not a Model Card${m.head ? `: it belongs to the model ${m.head.address}; give that address` : ''}`); process.exit(2) }
  const order = m.build.order.map((o) => o.address)
  const r = audit(m, new Map(cards(order).map((c) => [c.address, c])))
  if (flag === '--json') { console.log(JSON.stringify(r, null, 2)); process.exit(r.ok ? 0 : 1) }
  const sh = (a) => (a && a.startsWith(head) ? 'H' + a.slice(head.length) : a)
  const say = (kind, list) => list.forEach((f) => console.log(`${kind}: ${f.fix.replaceAll(head, 'H')}`))
  console.log(`setting part: ${sh(r.setting)}, notation card: ${sh(r.notation_card) || 'none'}`)
  say('missing', r.missing); say('outside', r.outside); say('inside', r.inside); say('stated', r.stated); say('notation', r.notation)
  r.uses.forEach((u) => console.log(`advice: ${sh(u.card)} uses an assumption: ${u.line.slice(0, 120)}`))
  console.log(r.ok ? 'settings: all in the setting part' : 'settings: faults above; move cards, never delete a Link')
  process.exit(r.ok ? 0 : 1)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
