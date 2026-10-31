// node check-settings.mjs <modelHead> [--json] [--symbols]
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
//   missing   no notation card in the setting part, none imported (**引入**), and no line
//             `沿用 [[block]] 的全部符号` in the setting part (#205)
// A Card another model lends (build line 'shared') is never audited here: its home model's audit
// checks it; the notation cards an import brings define symbols for this model.
// and that the setting part reads as a dependency tree, so a reader builds the settings from scratch:
//   start     a setting with no **起点** section (its premises, or 原始设定 for a primitive)
//   misfiled  a setting filed under another setting it does not rest on: file it under its main
//             premise, or as a top-level Side of the setting card (a new block, a model variant)
//   order     a setting that rests on a card later in the reading order (a later setting, a chapter)
// Advice (`uses`): lines that use an assumption mid-sentence (由于假设 …); read them, they normally stay.
// Advice (`introduces`): the notation symbols each setting uses first, in reading order; each must
// be defined in that card (or the card moves after the one that defines it).
// Exit 0 when every setting is in the setting part and the part is a dependency tree, 1 otherwise.
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

/** The symbols of the notation card's tables: the first cell of each row, split at 、 , ； and cut at = > < \in. */
export function notationSymbols(body) {
  const out = []
  for (const line of (body || '').split('\n')) {
    const cell = line.match(/^\|\s*([^|]+?)\s*\|/)
    if (!cell || /符号|---/.test(cell[1])) continue
    for (const tex of [...cell[1].matchAll(/\$([^$]+)\$/g)].map((x) => x[1])) {
      for (const part of splitTop(tex)) {
        const sym = part.replace(/\\[ ,;:!]/g, '').split(/=|>|<|\\in\b|\\le\b|\\ge\b|\\equiv/)[0].replace(/\s+/g, '')
        if (sym && !out.includes(sym)) out.push(sym)
      }
    }
  }
  return out
}
/** A list of symbols split at the commas and semicolons outside any bracket. */
function splitTop(tex) {
  const out = ['']
  let depth = 0
  for (const ch of tex) {
    if ('({['.includes(ch)) depth++
    if (')}]'.includes(ch)) depth--
    if (depth === 0 && ',，；'.includes(ch)) out.push('')
    else out[out.length - 1] += ch
  }
  return out
}
/** Whether a body uses a symbol in its formulas: the symbol, not part of a longer command, name or subscript. */
const usesSymbol = (body, sym) => {
  const flat = [...(body || '').matchAll(/\$\$?([^$]+)\$\$?/g)].map((x) => x[1].replace(/\s+/g, '')).join('|')
  let i = flat.indexOf(sym)
  while (i >= 0) {
    const before = flat[i - 1] || '', next = flat[i + sym.length] || ''
    if (!/[A-Za-z\\_^]/.test(before) && !/[A-Za-z]/.test(next)) return true
    i = flat.indexOf(sym, i + 1)
  }
  return false
}

/** The settings audit of a model: `m` is get_model output (build, relations), `cards` maps address ->
 * {facet, title, body}, `parents` maps address -> the card it is filed under (get_tree's parent). */
export function audit(m, cards, parents = new Map()) {
  const S = m.build.setting
  const order = m.build.order
  const inPart = (a) => a === S || (a.startsWith(S + '|') && /^\|[a-z]/.test(a.slice(S.length)))
  const mains = order.filter((o) => o.line === 'main').map((o) => o.address)
  const chapterOf = (a) => mains.filter((h) => a === h || a.startsWith(h + '|')).sort((x, y) => y.length - x.length)[0] || null
  const row = (a) => ({ ...order.find((o) => o.address === a), ...(cards.get(a) || {}) })
  const all = order.map((o) => row(o.address))

  const borrowed = (c) => c.line === 'shared'
  const notes = all.filter((c) => (inPart(c.address) || (borrowed(c) && c.import)) && isNotation(c))
  // One line binds a whole block's symbols (owner decision 3, #205).
  const binds = all.some((c) => inPart(c.address) && /沿用\s*\[\[[^\]]+\]\]\s*的全部符号/.test(c.body || ''))
  const missing = notes.length || binds ? [] : [{ fix: `the setting part of ${S} has no notation card (facet 符号表与读法): add one as its first Side successor, import one (**引入**), or write 沿用 [[…]] 的全部符号` }]
  const noteText = notes.map((c) => c.body || '').join('\n').replace(/\s+/g, '')

  const outside = []
  // A part shared from another model (line 'shared') is filed in its home model, whose own audit checks it.
  const placed = all.filter((c) => !inPart(c.address) && c.line !== 'shared' && (isSetting(c) || isNotation(c)))
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
    if (borrowed(c)) continue
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
    if (isNotation(c) && notes.includes(c)) continue
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

  // The dependency tree of the setting part.
  const pos = new Map(order.map((o, i) => [o.address, i]))
  const settings = all.filter((c) => inPart(c.address) && (isSetting(c) || c.address === S) && !isNotation(c))
  const rests = (m.relations || []).filter((r) => r.kind === 'rests-on')
  const premisesOf = (a) => [...new Set(rests.filter((r) => r.from === a).map((r) => r.to))]
  const start = settings.filter((c) => !/^\*\*起点\*\*/m.test(c.body || '')).map((c) => ({ card: c.address, fix: `${c.address} (${c.facet}) has no **起点** section: list the settings it builds on (依赖 [[…]] 的设定：…), or write 原始设定 for a primitive` }))
  const misfiled = []
  const order2 = []
  for (const c of settings) {
    const prem = premisesOf(c.address)
    for (const p of prem) {
      if (!pos.has(p) || pos.get(p) > pos.get(c.address)) order2.push({ card: c.address, premise: p, fix: `${c.address} (${c.facet}) rests on ${p}, which a reader meets later: move the setting after it, or turn the relation (the later card rests on the setting)` })
    }
    const parent = parents.get(c.address)
    if (!parent || parent === S || !inPart(parent) || isNotation(row(parent)) || prem.includes(parent)) continue
    const main = prem.filter((p) => inPart(p) && pos.get(p) < pos.get(c.address)).sort((x, y) => pos.get(y) - pos.get(x))[0]
    misfiled.push({ card: c.address, parent, fix: `${c.address} (${c.facet}) is filed under ${parent} but does not build on it: file it under ${main ? 'its main premise ' + main : 'the setting it builds on'}, or as a top-level Side of ${S}; if it does build on ${parent}, say so in its **起点**` })
  }
  const known = new Set()
  const syms = notes.length ? notationSymbols(notes.map((c) => c.body).join('\n')) : []
  const introduces = []
  for (const c of settings) {
    const fresh = syms.filter((x) => !known.has(x) && usesSymbol(c.body, x))
    fresh.forEach((x) => known.add(x))
    if (fresh.length) introduces.push({ card: c.address, symbols: fresh })
  }

  const ok = !missing.length && !outside.length && !inside.length && !stated.length && !notation.length && !start.length && !misfiled.length && !order2.length
  return { setting: S, notation_card: notes[0]?.address || null, outside, inside, stated, notation, missing, start, misfiled, order: order2, uses, introduces, ok }
}

async function main() {
  const [head, flag] = process.argv.slice(2)
  if (!head) { console.error('usage: node check-settings.mjs <modelHead> [--json]'); process.exit(2) }
  const { model, cards, tree } = await import('./zt.mjs')
  const m = model(head)
  if (!m.head || m.head.address !== head) { console.error(`${head} is not a Model Card${m.head ? `: it belongs to the model ${m.head.address}; give that address` : ''}`); process.exit(2) }
  const order = m.build.order.map((o) => o.address)
  const parents = new Map(tree(head.split('/')[0]).map((c) => [c.address, c.parent]))
  const r = audit(m, new Map(cards(order).map((c) => [c.address, c])), parents)
  if (flag === '--json') { console.log(JSON.stringify(r, null, 2)); process.exit(r.ok ? 0 : 1) }
  const sh = (a) => (a && a.startsWith(head) ? 'H' + a.slice(head.length) : a)
  const say = (kind, list) => list.forEach((f) => console.log(`${kind}: ${f.fix.replaceAll(head, 'H')}`))
  console.log(`setting part: ${sh(r.setting)}, notation card: ${sh(r.notation_card) || 'none'}`)
  say('missing', r.missing); say('outside', r.outside); say('inside', r.inside); say('stated', r.stated); say('notation', r.notation); say('start', r.start); say('misfiled', r.misfiled); say('order', r.order)
  if (process.argv.includes('--symbols')) r.introduces.forEach((x) => console.log(`reads: ${sh(x.card)} first uses ${x.symbols.map((y) => '$' + y + '$').join(' ')}`))
  r.uses.forEach((u) => console.log(`advice: ${sh(u.card)} uses an assumption: ${u.line.slice(0, 120)}`))
  console.log(r.ok ? 'settings: all in the setting part, as a dependency tree' : 'settings: faults above; move cards, never delete a Link')
  process.exit(r.ok ? 0 : 1)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
