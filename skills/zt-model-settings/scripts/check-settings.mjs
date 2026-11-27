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
// The stricter pass (2026-11-03) adds:
//   place     the setting card is not the Model Card's first Side (<head>|a), or the notation card
//             not the setting card's first Side
//   uncovered a letter a setting uses in a formula (outside sub- and superscripts) with no notation row
//   unused    a notation row whose symbol no card of the model uses
//   first     a notation row whose 首次出现 Link is not the card where a reader meets the symbol first
//             (the setting part, then the chapters)
//   format    a setting card not in the format: title (设定：… / 假设 N：…), sections (**起点**, **设定**
//             or **假设**, optional **直觉**, optional **说明**), a 依赖 line outside **起点**, a symbol or term table
//   repeat    a setting that restates a statement of an earlier one (assumptions with different names
//             are different statements)
//   cite      a card that names 假设 N without a Link to its card
// Advice (`balance`): a setting with more than ten items, a one-item setting, more than seven branches.
// Advice (`uses`): lines that use an assumption mid-sentence (由于假设 …); read them, they normally stay.
// Advice (`introduces`): the notation symbols each setting uses first, in reading order; each must
// be defined in that card (or the card moves after the one that defines it).
// Exit 0 when every setting is in the setting part and the part is a dependency tree, 1 otherwise.
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const SETTING = /^(设定|假设|假定|本章设定|本阶段的设定)/
const SETTING_EXT = /^(设定|假设|假定|本章设定|本阶段的设定|定义|判据|均衡定义|解概念)/
// Proposal §5.1: Benchmark restrictions (基准) and claim/assumption Boundaries (边界) are also legal inside the setting tree.
const SETTING_TREE_ALLOWED = /^(设定|假设|假定|本章设定|本阶段的设定|定义|判据|均衡定义|解概念|基准|边界)/
const CRITERION = /^(判据|均衡定义|解概念)/
const DEFINITION = /^定义/
const NOTATION = /^符号/
const HEADER = /^\*\*(本章设定|本阶段的设定|设定|假设|假定|记号|符号|符号表)\*\*/
const LIST = /^\s*(?:[-*](?=\s)|\d+[.、])\s*/
const DEFINES = /(?:^|[\s。；，：:、(（])记(?:号[:：])?\s*\$([^$]+)\$/g
// Unified assumption label grammar (matched ASCII/full-width parens, dotted/hyphenated IDs, primes, bare letters, Chinese numerals).
const ASSUMP_CORE = String.raw`(?:[A-Za-z0-9]+(?:[.\-][A-Za-z0-9]+)*[′']*|[一二三四五六七八九十]+)`
const ASSUMP_LABEL_SRC = `(?:（(${ASSUMP_CORE})）|\\((${ASSUMP_CORE})\\)|(${ASSUMP_CORE}))`
const ASSUMP_TITLE_RE = new RegExp(`^(?:假设|假定)(?:\\s*${ASSUMP_LABEL_SRC})?[：:]`)
const ASSUMP_NAME_RE = new RegExp(`^(?:假设|假定)\\s*${ASSUMP_LABEL_SRC}(?=[：:\\s]|$)`)
const ASSUMP_CITE_RE = new RegExp(`(?:假设|假定)\\s*${ASSUMP_LABEL_SRC}(?![A-Za-z0-9′'一二三四五六七八九十）)]|[.\\-][A-Za-z0-9])`, 'g')

const isSetting = (c) => SETTING.test(c.facet || '') || SETTING.test(c.title || '')
const isCriterion = (c) => CRITERION.test(c.facet || '') || CRITERION.test(c.title || '')
const isDefinition = (c) => DEFINITION.test(c.facet || '') || DEFINITION.test(c.title || '')
const isSettingMember = (c) => SETTING_EXT.test(c.facet || '') || SETTING_EXT.test(c.title || '')
const isSettingTreeAllowed = (c) => SETTING_TREE_ALLOWED.test(c.facet || '') || SETTING_TREE_ALLOWED.test(c.title || '')
const isNotation = (c) => NOTATION.test(c.facet || '') || NOTATION.test(c.title || '')
const symbolOf = (tex) => tex.split(/:=|=|\\equiv|\\coloneqq/)[0].replace(/\s+/g, '')
const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/** Split a Markdown table row without splitting on `|` inside `[[...]]` Links or escaped `\|`. */
function splitTableCells(line) {
  const inner = line.trim().replace(/^\|/, '').replace(/\|$/, '')
  const cells = []
  let start = 0
  let inLink = false
  for (let i = 0; i < inner.length; i++) {
    if (!inLink && inner[i] === '\\' && inner[i + 1] === '|') { i++; continue }
    if (!inLink && inner[i] === '[' && inner[i + 1] === '[') { inLink = true; i++; continue }
    if (inLink && inner[i] === ']' && inner[i + 1] === ']') { inLink = false; i++; continue }
    if (!inLink && inner[i] === '|') { cells.push(inner.slice(start, i).trim()); start = i + 1 }
  }
  cells.push(inner.slice(start).trim())
  return cells
}

/** The rows of the notation card's tables: each row's symbols (its first cell, split at 、 , ； and cut at
 * = > < \in) and the card its 首次出现 (3rd) cell links, if any. */
export function notationRows(body) {
  const rows = []
  let inTerms = false
  for (const line of (body || '').split('\n')) {
    const t = line.trim()
    if (t.includes('统一用语') || t.includes('术语表')) { inTerms = true; continue }
    if (inTerms && /^\*\*[^*]+\*\*/.test(t)) inTerms = false
    if (inTerms || !t.startsWith('|')) continue
    const cells = splitTableCells(t)
    if (!cells.length || /符号|术语|---/.test(cells[0])) continue
    const symbols = []
    for (const tex of [...cells[0].matchAll(/\$([^$]+)\$/g)].map((x) => x[1])) {
      for (const part of splitTop(tex)) {
        // Spaces go, except the one that ends a command before a letter (\bar Z, \theta Z).
        const sym = part.replace(/\\[ ,;:!]/g, '').split(/=|>|<|\\in\b|\\le\b|\\ge\b|\\equiv/)[0].trim()
          .replace(/(\\[A-Za-z]+)\s+(?=[A-Za-z])/g, '$1\u0000').replace(/\s+/g, '').replace(/\u0000/g, ' ')
        if (sym && !symbols.includes(sym)) symbols.push(sym)
      }
    }
    // Read first-use link strictly from the 3rd column (首次出现), ordinary Card link only (no @ Term links).
    const firstCell = cells.length >= 3 ? cells[2] : ''
    const link = firstCell.match(/\[\[([^\]@]+)\]\]/)
    if (symbols.length) rows.push({ symbols, link: link ? link[1].trim() : null, hasThirdCol: cells.length >= 3 })
  }
  return rows
}
/** The symbols of the notation card's tables, each once. */
export function notationSymbols(body) {
  return [...new Set(notationRows(body).flatMap((r) => r.symbols))]
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
// TeX is read as tokens: a command, an escaped character or one character. A decoration (\bar Z,
// \mathcal F) makes one symbol with the letter after it; braces around one token are dropped.
const DECOR = ['bar', 'hat', 'tilde', 'mathcal', 'mathbb', 'mathbf', 'mathscr', 'mathfrak', 'boldsymbol', 'overline', 'widetilde', 'widehat', 'check', 'dot', 'ddot', 'vec']
const isDecor = (t) => DECOR.includes(t.slice(1))
const GREEK = 'alpha beta gamma delta epsilon varepsilon zeta eta theta vartheta iota kappa lambda mu nu xi pi varpi rho varrho sigma varsigma tau upsilon phi varphi chi psi omega Gamma Delta Theta Lambda Xi Pi Sigma Upsilon Phi Psi Omega ell'.split(' ')
const isLetter = (t) => /^[A-Za-z]$/.test(t) || GREEK.includes(t.slice(1)) && t[0] === '\\'
/** The formulas of a body ($…$ and $$…$$). */
const formulas = (body) => [...(body || '').matchAll(/\$\$?([^$]+)\$\$?/g)].map((x) => x[1])
/** The tokens of a TeX string; text, tags and labels are dropped. */
export function tokens(tex) {
  // An environment's name and an array's column spec (\begin{array}{l l}, \end{array}) are no symbols.
  const s = tex.replace(/\\begin\s*\{[^{}]*\}(?:\s*\{[^{}]*\})?|\\end\s*\{[^{}]*\}/g, ' ')
    .replace(/\\(?:text|textrm|textit|mathrm|operatorname|mbox|tag|label)\s*\{[^{}]*\}/g, ' ')
  let out = [...s.matchAll(/\\[A-Za-z]+|\\.|\S/g)].map((x) => x[0]).filter((t) => !/^\\[ ,;:!]$|^\\(?:left|right|big|Big|bigg|Bigg|quad|qquad|displaystyle|limits)$/.test(t))
  for (let changed = true; changed;) {
    changed = false
    for (let i = 0; i + 2 < out.length; i++) {
      if (out[i] === '{' && out[i + 2] === '}' && out[i + 1] !== '{' && out[i + 1] !== '}') { out.splice(i, 3, out[i + 1]); changed = true }
    }
  }
  return out
}
/** The base symbols a TeX string uses: its letters and decorated letters, outside sub- and superscripts;
 * the differential d, ℝ and a prime are not symbols. */
export function baseSymbols(tex) {
  const t = tokens(tex)
  const out = []
  for (let i = 0; i < t.length; i++) {
    if (t[i] === '_' || t[i] === '^') { i = skipGroup(t, i + 1); continue }
    let sym = null
    if (isDecor(t[i]) && t[i + 1]) { sym = t[i] + ' ' + (t[i + 1] === '{' ? t[i + 2] : t[i + 1]); i = t[i + 1] === '{' ? skipGroup(t, i + 1) : i + 1 }
    else if (isLetter(t[i])) sym = t[i]
    if (!sym || sym === '\\mathbb R') continue
    if (sym === 'd' && t[i + 1] && (isLetter(t[i + 1]) || isDecor(t[i + 1]))) continue
    if (sym === 'R' && (t[i + 1] === '^' || t[i + 1] === '_')) continue
    if (!out.includes(sym)) out.push(sym)
  }
  return out
}
/** The index of the last token of the group that starts at i (one token, or a braced group). */
function skipGroup(t, i) {
  if (t[i] !== '{') return i
  for (let depth = 0; i < t.length; i++) {
    if (t[i] === '{') depth++
    if (t[i] === '}' && --depth === 0) return i
  }
  return i
}
/** The tokens a notation symbol is matched by: a function symbol C(\theta) by its name C. */
const symbolTokens = (sym) => {
  const t = tokens(sym)
  const open = t.indexOf('(')
  return open > 0 ? t.slice(0, open) : t
}
const HEADING_LINE_RE = /^\s*\*\*([^*]+)\*\*/
function startEndIdx(lines, startIdx) {
  const nextHeading = lines.findIndex((l, idx) => idx > startIdx && (HEADING_LINE_RE.test(l) || /^\s*来源[：:]/.test(l)))
  if (nextHeading >= 0) return nextHeading
  // Malformed card lacking a following heading: stop after the **起点** list or opening line.
  let k = startIdx + 1
  while (k < lines.length && (LIST.test(lines[k]) || (!lines[k].trim() && k + 1 < lines.length && LIST.test(lines[k + 1])) || /^\s{2,}\S/.test(lines[k]))) k++
  if (k === startIdx + 1 && k < lines.length && lines[k].trim()) k++
  return k
}
/** The text of a card outside its **起点** section (including any lines before **起点** and all
 * sections after **起点** up to the end of the body). */
function outsideStart(body) {
  const lines = (body || '').replace(/\r/g, '').split('\n')
  const startIdx = lines.findIndex((l) => /^\s*\*\*起点\*\*/.test(l))
  if (startIdx < 0) return body || ''
  const endIdx = startEndIdx(lines, startIdx)
  return [...lines.slice(0, startIdx), ...lines.slice(endIdx)].join('\n')
}
/** The text of a card after its **起点** section (the whole body when it has none). */
function afterStart(body) {
  const lines = (body || '').replace(/\r/g, '').split('\n')
  const startIdx = lines.findIndex((l) => /^\s*\*\*起点\*\*/.test(l))
  if (startIdx < 0) return body || ''
  return lines.slice(startEndIdx(lines, startIdx)).join('\n')
}
/** For each token, whether it sits under a decoration (the Z of \bar Z is not Z). A symbol in a sub- or
 * superscript is used there (R^{K_s}, \nabla_{\theta}). */
function scripted(t) {
  const mark = t.map(() => false)
  for (let i = 0; i < t.length; i++) {
    if (isDecor(t[i])) {
      const end = skipGroup(t, i + 1)
      for (let k = i + 1; k <= end; k++) mark[k] = true
    }
  }
  return mark
}
/** Whether a body uses a symbol in its formulas: the symbol's tokens, starting outside a decoration.
 * `longer` lists other notation symbols: an occurrence that is one of them written in full (β inside the
 * row symbol β_R) belongs to that row, not to this one. */
const usesSymbol = (body, sym, longer = []) => {
  const want = symbolTokens(sym)
  if (!want.length || (want.length === 1 && isDecor(want[0]))) return false
  const over = longer.map(symbolTokens).filter((o) => o.length > want.length && want.every((w, k) => o[k] === w))
  for (const f of formulas(body)) {
    const t = tokens(f)
    const mark = scripted(t)
    for (let i = 0; i + want.length <= t.length; i++) {
      if (mark[i] || !want.every((w, k) => t[i + k] === w)) continue
      if (over.some((o) => i + o.length <= t.length && o.every((w, k) => t[i + k] === w))) continue
      return true
    }
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

  const isFormalV2 = m.level === 'formal' && !m.form
  const borrowed = (c) => c.line === 'shared'
  const notes = all.filter((c) => (inPart(c.address) || (borrowed(c) && c.import)) && isNotation(c))
  // One line binds a whole block's symbols (owner decision 3, #205).
  // For Level 2 formal models, the binding lives on <head>|a|a (a Notation card) or an imported Notation card, matching Rust formal_shape.
  const binds = isFormalV2
    ? notes.some((c) => /沿用\s*\[\[[^\]]+\]\]\s*的全部符号/.test(c.body || ''))
    : all.some((c) => inPart(c.address) && /沿用\s*\[\[[^\]]+\]\]\s*的全部符号/.test(c.body || ''))
  // Targeted Stage 2 deferral (matching check-model --stage settings): if a pending cross-model closure
  // specifically targets a Notation card, defer `missing` to Stage 4 instead of blocking Stage 2.
  const pendingNotationImport = (m.build?.closure || []).some((f) => {
    const c = f && f.card
    return Boolean(c && c.type === 'model' && (c.models || []).length && (c.home_role === 'notation' || /^符号/.test(c.facet || '')))
  })
  const missingRaw = notes.length || binds ? [] : [{ fix: `the setting part of ${S} has no notation card (facet 符号表与读法): add one at ${S}|a (or import one via **引入**), optionally writing 沿用 [[<lender notation card>]] 的全部符号 inside ${S}|a` }]
  const missing = pendingNotationImport ? [] : missingRaw
  const deferredMissing = pendingNotationImport ? missingRaw : []
  // Follow 1-hop `沿用 [[...]]` borrowed notation bindings (ordinary links only, matching Rust borrowed_notation_targets).
  const borrowedBodies = []
  const wrongNotationTargets = []
  const borrowSources = isFormalV2 ? notes.filter((c) => inPart(c.address)) : all.filter((x) => inPart(x.address))
  for (const c of borrowSources) {
    for (const line of (c.body || '').split('\n')) {
      for (const clause of line.split(/[。；;]/)) {
        const at = clause.indexOf('沿用')
        if (at < 0 || !clause.slice(at).includes('符号')) continue
        for (const m2 of clause.slice(at).matchAll(/\[\[([^\]@]+)\]\]/g)) {
          const targetAddr = m2[1].trim()
          const t = cards.get(targetAddr)
          if (isFormalV2 && (!t || !isNotation(t))) {
            wrongNotationTargets.push({
              card: c.address,
              what: 'notation_target',
              fix: `${c.address} binds 沿用 [[${targetAddr}]] 的全部符号, but ${targetAddr} (${t ? (t.facet || t.title || 'unknown') : 'missing'}) is not a 符号表与读法 card: link the lender's notation card (<lender_head>|a|a) directly`,
            })
            continue
          }
          if (t && !isNotation(t) && !/^\|\s*符号\s*\|/m.test(t.body || '')) {
            wrongNotationTargets.push({
              card: c.address,
              what: 'notation_target',
              fix: `${c.address} binds 沿用 [[${targetAddr}]] 的全部符号, but ${targetAddr} (${t.facet || t.title || 'unknown'}) is not a 符号表与读法 card: link the lender's notation card (<lender_head>|a|a) directly`,
            })
            continue
          }
          if (t?.body && !borrowedBodies.includes(t.body)) borrowedBodies.push(t.body)
        }
      }
    }
  }
  const noteText = [...notes.map((c) => c.body || ''), ...borrowedBodies].join('\n').replace(/\s+/g, '')

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
    .filter((c) => inPart(c.address) && c.address !== S && !isSettingTreeAllowed(c) && !isNotation(c))
    .map((c) => ({ card: c.address, facet: c.facet, fix: `${c.address} (${c.facet}) is not a setting-tree role: move it out of the setting part, after its latest premise (check-network names the chapter)` }))

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
  const settings = all.filter((c) => inPart(c.address) && (isSettingMember(c) || c.address === S) && !isNotation(c))
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
  const syms = (notes.length || borrowedBodies.length)
    ? notationSymbols([...notes.map((c) => c.body), ...borrowedBodies].join('\n'))
    : []
  const introduces = []
  for (const c of settings) {
    const fresh = syms.filter((x) => !known.has(x) && usesSymbol(c.body, x, syms))
    fresh.forEach((x) => known.add(x))
    if (fresh.length) introduces.push({ card: c.address, symbols: fresh })
  }

  // Placement: the setting card is the Model Card's first Side, the notation card the setting card's.
  const place = []
  const head = m.head?.address
  if (head && S !== head + '|a') place.push({ card: S, fix: `the setting card ${S} is not the first Side successor of the Model Card: move it to ${head}|a, the chapters stay its Direct successors` })
  const own = notes.find((c) => inPart(c.address))
  if (own && own.address !== S + '|a') place.push({ card: own.address, fix: `the notation card ${own.address} is not the first Side successor of the setting card: move it to ${S}|a` })

  // Symbols: every symbol a setting uses is in the notation card; every notation row names symbols the
  // model uses, and links the card where a reader meets them first (settings first, then the chapters).
  const own_ = (c) => !borrowed(c) && !(isNotation(c) && notes.includes(c))
  const covered = new Set(syms.flatMap((x) => baseSymbols(x)))
  const uncovered = []
  if (isFormalV2 && Array.isArray(m.build.symbol_uncovered)) {
    for (const f of m.build.symbol_uncovered) {
      uncovered.push({ card: f.address, symbols: f.symbol ? [f.symbol] : [], fix: f.fix })
    }
  } else if (notes.length || borrowedBodies.length) {
    for (const c of settings) {
      const lack = [...new Set(formulas(c.body).flatMap(baseSymbols))].filter((x) => !covered.has(x))
      if (lack.length) uncovered.push({ card: c.address, symbols: lack, fix: `${c.address} uses ${lack.map((x) => '$' + x + '$').join(' ')}, which the notation card does not list: add a row for each (its meaning and the card that defines it), or write the symbol as the notation card does` })
    }
  }
  const reading = [...all.filter((c) => inPart(c.address)), ...all.filter((c) => !inPart(c.address))].filter(own_)
  const unused = []
  const first = []
  const rowsAll = notes.filter((c) => inPart(c.address)).flatMap((c) => notationRows(c.body))
  const allSyms = rowsAll.flatMap((r) => r.symbols)
  for (const r of rowsAll) {
    for (const sym of r.symbols) {
      const meet = reading.find((c) => usesSymbol(c.body, sym, allSyms))
      if (!meet) { unused.push({ symbol: sym, fix: `the notation card lists $${sym}$, which no card of the model uses: drop the row, or write the symbol as the cards do` }); continue }
      if (isFormalV2 && !r.link) {
        first.push({ symbol: sym, listed: null, reads: meet.address, fix: `the notation row for $${sym}$ lacks a 首次出现 Card Link in column 3: point it to [[${meet.address}]]` })
      } else if (r.link && r.link !== meet.address) {
        first.push({ symbol: sym, listed: r.link, reads: meet.address, fix: `the notation card says $${sym}$ is first met in ${r.link}, but a reader meets it first in ${meet.address}: define it there and point the row to it` })
      }
    }
  }

  // Card format: **起点**, then **设定** (an assumption: **假设**), then an optional **说明**; the content
  // only; premises only in **起点**; tables of symbols and terms only in the notation card.
  const isAssumption = (c) => /^(假设|假定)/.test(c.facet || '') || /^(假设|假定)/.test(c.title || '')
  const sectionsOf = (body) => (body || '').split('\n').map((l) => l.match(/^\s*\*\*([^*]+)\*\*/)).filter(Boolean).map((x) => x[1].trim())
  const sectionText = (body, name) => {
    const m2 = (body || '').split(/^\s*\*\*([^*]+)\*\*/m)
    const i = m2.indexOf(name)
    return i > 0 ? m2[i + 1] : ''
  }
  const format = [...wrongNotationTargets]
  const kindOf = (c) => {
    if (isAssumption(c)) return '假设'
    if (isCriterion(c)) {
      const m2 = (c.facet || c.title || '').match(CRITERION)
      return m2 ? m2[1] : '判据'
    }
    if (isDefinition(c)) return '定义'
    return '设定'
  }
  const titleOk = (c, kind) => {
    const t = c.title || ''
    if (kind === '设定') return /^设定[：:]/.test(t)
    if (kind === '假设') return ASSUMP_TITLE_RE.test(t)
    if (kind === '定义') return /^定义[：:]/.test(t)
    return /^(?:判据|均衡定义|解概念)[：:]/.test(t)
  }
  const allowedTails = new Set(['', '直觉', '说明', '直觉,说明', '作用域', '作用域,说明', '作用域,直觉', '作用域,直觉,说明'])
  const criterionTails = new Set(['作用域', '作用域,说明', '作用域,直觉', '作用域,直觉,说明'])
  for (const c of settings) {
    const kind = kindOf(c)
    const bad = (what, fix) => format.push({ card: c.address, what, fix: `${c.address} (${c.facet}): ${fix}` })
    if (!titleOk(c, kind)) bad('title', kind === '设定' ? 'the title opens with 设定：' : kind === '假设' ? 'the title opens with 假设 <name>： (or 假设 (B)： / 假设：)' : `the title opens with ${kind}：`)
    const sec = sectionsOf(c.body)
    // After the content: Criterion cards require **作用域** (matching FM2); other setting-tree cards allow optional **作用域**/**直觉**/**说明**.
    const tail = sec.slice(2).join(',')
    const okTail = isCriterion(c) ? criterionTails.has(tail) : allowedTails.has(tail)
    if (sec[0] !== '起点' || sec[1] !== kind || !okTail) {
      bad('section', isCriterion(c)
        ? `a criterion card requires **起点**, **${kind}**, **作用域**, then optional **直觉**/**说明** (now ${sec.map((x) => '**' + x + '**').join(', ') || 'none'})`
        : `the sections are **起点**, **${kind}**, then optional **作用域**/**直觉**/**说明** (now ${sec.map((x) => '**' + x + '**').join(', ') || 'none'})`)
    }
    if (/依赖\s*\[\[/.test(outsideStart(c.body))) bad('depends', 'a 依赖 line outside **起点**: move the premise into **起点**')
    if (/^\|\s*(符号|术语)\s*\|/m.test(c.body || '')) bad('table', 'a symbol or term table: its rows belong in the notation card')
  }

  // An assumption is stated once: a statement repeated in a later setting is replaced by a pointer.
  const plain = (s) => s.replace(/\[\[([^\]@]*)@[^\]]+\]\]/g, '$1').replace(/\[\[[^\]]+\]\]/g, '').replace(/[\s，。；：、,.;:()（）]/g, '')
  const statementsOf = (c) => {
    // Display equations are no statements: a variant system extends an equation it does not restate.
    const text = (sectionText(c.body, kindOf(c)) || '').replace(/\$\$[\s\S]*?\$\$/g, '')
    return text.split('\n').filter((l) => !/^\s*\$\$/.test(l)).map((l) => l.replace(LIST, '')).flatMap((l) => l.split('。')).map(plain).filter((s) => s.length >= 12)
  }
  const bigrams = (s) => new Set([...s].slice(1).map((ch, i) => s[i] + ch))
  const dice = (a, b) => { const A = bigrams(a), B = bigrams(b); let n = 0; for (const x of A) if (B.has(x)) n++; return (2 * n) / (A.size + B.size || 1) }
  // Two assumptions with different names are different statements, however parallel (a weakening).
  const nameOf = (c) => {
    if (!isAssumption(c)) return null
    const m2 = (c.title || '').match(ASSUMP_NAME_RE)
    return m2 ? (m2[1] || m2[2] || m2[3] || null) : null
  }
  const apart = (a, b) => nameOf(a) && nameOf(b) && nameOf(a) !== nameOf(b)
  const repeat = []
  const said = []
  for (const c of settings) {
    const mine = statementsOf(c)
    const hit = said.find(([a, s]) => !apart(row(a), c) && mine.some((x) => dice(x, s) >= 0.8))
    if (hit) repeat.push({ card: c.address, other: hit[0], fix: `${c.address} restates what ${hit[0]} states: keep the statement in ${hit[0]}; ${c.address} names it in **起点** and states only what it changes` })
    for (const s of mine) said.push([c.address, s])
  }

  // Assumptions are named the same way everywhere: a card that names 假设 N, 假设 1.2, 假设 B, or 假设 (B) links its card.
  const named = new Map()
  for (const c of settings.filter(isAssumption)) {
    const n = nameOf(c)
    if (n) named.set(n, [...(named.get(n) || []), c.address])
  }
  const cite = []
  for (const c of all.filter(own_)) {
    const seen = new Set()
    for (const line of (c.body || '').split('\n')) {
      const bare = line.replace(/\[\[[^\]]+\]\]/g, '')
      for (const x of bare.matchAll(ASSUMP_CITE_RE)) {
        const key = x[1] || x[2] || x[3]
        const at = named.get(key)
        if (!at || at.includes(c.address) || seen.has(key)) continue
        if (at.some((a) => line.includes(`@${a}]]`) || line.includes(`[[${a}]]`))) continue
        seen.add(key)
        cite.push({ card: c.address, name: '假设 ' + key, line: line.trim(), fix: `${c.address} names 假设 ${key} without a Link: write [[${at[0]}]] in its place (the GUI shows a Link to a model part by its facet, 假设 ${key})` })
      }
    }
  }

  // Balance (advice): a setting with more than ten statements is split; a one-statement setting
  // without a display formula joins the setting it builds on (assumptions, definitions, and criteria stay their own card);
  // a setting with more than seven branches groups them in blocks.
  const balance = []
  for (const c of settings) {
    // A card not yet in the format counts its whole text after **起点**, without tables.
    const text = sectionText(c.body, kindOf(c)) || afterStart(c.body).split('\n').filter((l) => !/^\|/.test(l)).join('\n')
    const items = text.split('\n').filter((l) => LIST.test(l) && l.replace(LIST, '').trim())
    const count = items.length || text.split('\n').filter((l) => !/^\s*\$\$/.test(l)).join('').split('。').filter((s) => s.trim()).length
    if (count > 10) balance.push({ card: c.address, what: 'heavy', fix: `${c.address} states ${count} things: split it into settings that each set up one piece` })
    else if (c.address !== S && !isAssumption(c) && !isDefinition(c) && !isCriterion(c) && count <= 1 && !/\$\$/.test(c.body || '')) balance.push({ card: c.address, what: 'light', fix: `${c.address} states one thing: consider joining it to the setting it builds on` })
    const kids = settings.filter((d) => parents.get(d.address) === c.address)
    if (kids.length > 7) balance.push({ card: c.address, what: 'wide', fix: `${c.address} has ${kids.length} settings filed under it: group them in blocks` })
  }

  const ok = !missing.length && !outside.length && !inside.length && !stated.length && !notation.length && !start.length && !misfiled.length && !order2.length &&
    !place.length && !uncovered.length && !unused.length && !first.length && !format.length && !repeat.length && !cite.length
  return { setting: S, notation_card: notes[0]?.address || null, outside, inside, stated, notation, missing, deferred_missing: deferredMissing, start, misfiled, order: order2, place, uncovered, unused, first, format, repeat, cite, balance, uses, introduces, ok }
}

// --- level 3: the data tree and the specification ladder (docs/empirical-models-proposal.md §9.2) -------------
// An empirical model's setting part is its data card (the Model Card's first Side successor) with the 变量表 as
// its first Side, the samples and measures under it; the design card holds the identifying assumptions; the
// baseline 规格 comes before every variant, each variant filed under the specification it changes. Faults:
//   place           the data card is not the first Side successor, or the 变量表 not the data card's first Side
//   uncovered       a symbol of a specification's equation with no 变量表 row (the build report's variable_uncovered)
//   unused          a 变量表 row whose symbol no card of the model uses
//   variant_change  a specification variant with no **相对基准的改动** section
//   spec_in_chapter a specification filed in a chapter: specifications live on the ladder under 规格：基准
//   equation        a specification with no formula in **估计式**, or one that says the book writes none
//   assumption      an assumption (假设 k) that is not a Side successor of the design card
//   start           a sample, measure or descriptive card of the data tree with no **起点** section
//   order           a card of the data tree that rests (依赖) on a card of the data tree a reader meets later
// The build report's role faults (empirical_shape, estimate_unbound, spec_unbound, test_unbound) are listed too.
const roleOf = (o) => o.role || null
/** The 变量表 rows' base symbols, from the first cell's $…$ formulas. */
/** Greek letters as a card writes them in prose, by their TeX names. */
const PROSE_GREEK = { α: 'alpha', β: 'beta', γ: 'gamma', δ: 'delta', ε: 'epsilon', ζ: 'zeta', η: 'eta', θ: 'theta', κ: 'kappa', λ: 'lambda', μ: 'mu', ν: 'nu', ξ: 'xi', π: 'pi', ρ: 'rho', σ: 'sigma', τ: 'tau', φ: 'phi', χ: 'chi', ψ: 'psi', ω: 'omega', Γ: 'Gamma', Δ: 'Delta', Θ: 'Theta', Λ: 'Lambda', Π: 'Pi', Σ: 'Sigma', Φ: 'Phi', Ψ: 'Psi', Ω: 'Omega' }

export function variableSymbols(body) {
  const out = []
  for (const line of (body || '').split('\n')) {
    const cell = line.match(/^\|\s*([^|]+?)\s*\|/)
    if (!cell || /^变量$|---/.test(cell[1].trim())) continue
    for (const tex of [...cell[1].matchAll(/\$([^$]+)\$/g)].map((x) => x[1])) {
      for (const s of baseSymbols(tex)) if (!out.includes(s)) out.push(s)
    }
  }
  return out
}
export function auditEmpirical(m, cards) {
  // A Card another model lends (an imported block, build line 'shared') is audited in its home model.
  const order = (m.build.order || []).filter((o) => o.line !== 'shared')
  const head = m.head.address
  const body = (a) => (cards.get(a) || {}).body || ''
  const data = order.find((o) => roleOf(o) === 'data' && o.line === 'main')
  const place = []
  if (!data || data.address !== `${head}|a`) place.push({ card: data?.address || head, fix: `the data card (数据) is the first Side successor of ${head} (${head}|a)` })
  const table = order.find((o) => roleOf(o) === 'variables')
  if (data && (!table || table.address !== `${data.address}|a`)) place.push({ card: table?.address || data.address, fix: `the 变量表 is the first Side successor of the data card ${data.address} (${data.address}|a)` })
  const uncovered = (m.build.variable_uncovered || []).map((f) => ({ card: f.address, symbol: f.symbol, fix: f.fix }))
  const used = new Set()
  for (const o of order) if (o.address !== table?.address) for (const tex of formulas(body(o.address))) for (const s of baseSymbols(tex)) used.add(s)
  // A Greek letter written in prose (σ, θ) uses every row built on it ($\sigma$, $\hat \theta$): becker2025division/6|g.
  const prose = new Set()
  for (const o of order) if (o.address !== table?.address) for (const ch of body(o.address)) if (PROSE_GREEK[ch]) prose.add(PROSE_GREEK[ch])
  const inProse = (s) => [...s.matchAll(/\\([a-zA-Z]+)/g)].some((m) => prose.has(m[1]))
  const unused = table ? variableSymbols(body(table.address)).filter((s) => !used.has(s) && !inProse(s)).map((s) => ({ card: table.address, symbol: s, fix: `the 变量表 row $${s}$ names a symbol no card of the model uses: drop the row` })) : []
  const chapters = order.filter((o) => roleOf(o) === 'chapter' && o.line === 'main').map((o) => o.address)
  const specs = order.filter((o) => roleOf(o) === 'spec')
  const variantChange = specs.filter((o) => o.line !== 'main' && !/^\s*\*\*相对基准的改动\*\*/m.test(body(o.address)))
    .map((o) => ({ card: o.address, fix: `${o.address} is a specification variant without **相对基准的改动**: state the one change against the specification it is filed under` }))
  const specInChapter = specs.filter((o) => chapters.some((c) => o.address.startsWith(c + '|')))
    .map((o) => ({ card: o.address, fix: `${o.address} is a specification filed in a chapter: move it onto the ladder, under the specification it changes (a chapter holds the estimates of a variant, never the variant)` }))
  // Owner rule (2026-11-12): every specification, baseline and variant, states its equation in **估计式**; when the
  // source writes none, the card composes it as 补写 (原书没有写出本规格的估计式。本卡按……补写：). A missing
  // section, a section with no formula, or "书中没有写出估计式" / "书中未写" is a fault.
  const equation = specs.flatMap((o) => {
    const text = body(o.address)
    const section = text.match(/^\s*\*\*估计式\*\*[^\n]*\n([\s\S]*?)(?=^\s*\*\*[^*\n]+\*\*\s*$|(?![\s\S]))/m)
    if (/书中没有写出估计式|书中未写/.test(text)) return [{ card: o.address, fix: `${o.address} says the book writes no equation: compose it in **估计式** and open with 原书没有写出本规格的估计式。本卡按……补写：` }]
    if (!section || !/\$[^$]+\$/.test(section[1])) return [{ card: o.address, fix: `${o.address} is a specification without an equation in **估计式**: write it (from the source, or as 补写 opening with 原书没有写出本规格的估计式。本卡按……补写：)` }]
    return []
  })
  const design = order.find((o) => roleOf(o) === 'design' && o.line === 'main')
  const parentOf = (a) => a.replace(/\|[^|]+$/, '')
  const assumption = order.filter((o) => roleOf(o) === 'assumption' && (!design || parentOf(o.address) !== design.address || /\|\d+$/.test(o.address)))
    .map((o) => ({ card: o.address, fix: `${o.address} is an identifying assumption that is not a Side successor of the design card${design ? ` ${design.address}` : ''}: file it as the design's next Side successor` }))
  // The data tree: samples, measures and descriptive facts under the data card name their premises first and rest on nothing later.
  const tree = order.filter((o) => ['sample', 'measure', 'descriptive'].includes(roleOf(o)) && data && o.address.startsWith(data.address + '|'))
  const start = tree.filter((o) => !/^\s*\*\*起点\*\*/m.test(body(o.address)))
    .map((o) => ({ card: o.address, fix: `${o.address} has no **起点**: after its one-line summary, list what it builds on (依赖 [[…]]：…) under **起点**` }))
  const at = new Map(order.map((o, i) => [o.address, i]))
  const dataTree = new Set([data?.address, ...tree.map((o) => o.address)])
  const orderFaults = tree.flatMap((o) => [...body(o.address).matchAll(/依赖\s*\[\[([^\]@]+?)\]\]/g)].map((x) => x[1])
    .filter((p) => dataTree.has(p) && at.get(p) > at.get(o.address))
    .map((p) => ({ card: o.address, premise: p, fix: `${o.address} rests on ${p}, which a reader meets later: reorder the data card's Side branches so ${p} comes first` })))
  const isStage3Shape = (f) => f && (f.lacks === '估计' || /估计/.test(f.lacks || '') || /zero.*estimate|no.*估计|lacks.*估计/i.test(f.fix || ''))
  const roles = [
    ...(m.build.empirical_shape || []).filter((f) => !isStage3Shape(f)).map((f) => ({ card: f.address, kind: 'empirical_shape', fix: f.fix })),
    ...(m.build.spec_unbound || []).map((f) => ({ card: f.address, kind: 'spec_unbound', fix: f.fix })),
  ]
  // Stage 3 evidence-spine faults (estimate_unbound, test_unbound, and missing-estimate shape) are reported without blocking Stage 2 Settings.
  const deferredRoles = [
    ...(m.build.empirical_shape || []).filter(isStage3Shape).map((f) => ({ card: f.address, kind: 'empirical_shape', fix: f.fix })),
    ...['estimate_unbound', 'test_unbound'].flatMap((k) => (m.build[k] || []).map((f) => ({ card: f.address, kind: k, fix: f.fix }))),
  ]
  const ok = !place.length && !uncovered.length && !unused.length && !variantChange.length && !specInChapter.length && !equation.length && !assumption.length && !start.length && !orderFaults.length && !roles.length
  return { level: 'empirical', data: data?.address || null, variables: table?.address || null, place, uncovered, unused, variant_change: variantChange, spec_in_chapter: specInChapter, equation, assumption, start, order: orderFaults, roles, deferred_roles: deferredRoles, ok }
}

async function main() {
  const [head, flag] = process.argv.slice(2)
  if (!head) { console.error('usage: node check-settings.mjs <modelHead> [--json]'); process.exit(2) }
  const { model, cards, tree } = await import('./zt.mjs')
  const m = model(head)
  if (!m?.head || m.head.address !== head) { console.error(`${head} is not a Model Card${m?.head ? `: it belongs to the model ${m.head.address}; give that address` : ''}`); process.exit(2) }
  const order = m.build.order.map((o) => o.address)
  if (m.level === 'empirical') {
    const r = auditEmpirical(m, new Map(cards(order).map((c) => [c.address, c])))
    if (flag === '--json') { console.log(JSON.stringify(r, null, 2)); process.exit(r.ok ? 0 : 1) }
    console.log(`empirical model: data card ${r.data || 'none'}, 变量表 ${r.variables || 'none'}`)
    for (const k of ['place', 'uncovered', 'unused', 'variant_change', 'spec_in_chapter', 'equation', 'assumption', 'start', 'order', 'roles']) r[k].forEach((x) => console.log(`${x.kind || k}: ${x.fix}`))
    console.log(r.ok ? 'settings: the data tree and the specification ladder are in place' : 'settings: faults above; move cards, never delete a Link')
    process.exit(r.ok ? 0 : 1)
  }
  const parents = new Map(tree(head.split('/')[0]).map((c) => [c.address, c.parent]))
  const cardMap = new Map(cards(order).map((c) => [c.address, c]))
  // Fetch any 1-hop `沿用 [[...]]` borrowed notation targets outside `build.order` (ordinary links only, matching Rust).
  const extraAddrs = new Set()
  for (const c of cardMap.values()) {
    for (const line of (c.body || '').split('\n')) {
      for (const clause of line.split(/[。；;]/)) {
        const at = clause.indexOf('沿用')
        if (at < 0 || !clause.slice(at).includes('符号')) continue
        for (const m2 of clause.slice(at).matchAll(/\[\[([^\]@]+)\]\]/g)) {
          const targetAddr = m2[1].trim()
          if (!cardMap.has(targetAddr)) extraAddrs.add(targetAddr)
        }
      }
    }
  }
  if (extraAddrs.size) {
    for (const c of cards([...extraAddrs])) cardMap.set(c.address, c)
  }
  const r = audit(m, cardMap, parents)
  if (flag === '--json') { console.log(JSON.stringify(r, null, 2)); process.exit(r.ok ? 0 : 1) }
  const sh = (a) => (a && a.startsWith(head) ? 'H' + a.slice(head.length) : a)
  const say = (kind, list) => list.forEach((f) => console.log(`${kind}: ${f.fix.replaceAll(head, 'H')}`))
  console.log(`setting part: ${sh(r.setting)}, notation card: ${sh(r.notation_card) || 'none'}`)
  say('missing', r.missing); say('place', r.place); say('outside', r.outside); say('inside', r.inside); say('stated', r.stated); say('notation', r.notation); say('start', r.start); say('misfiled', r.misfiled); say('order', r.order)
  say('uncovered', r.uncovered); say('unused', r.unused); say('first', r.first); say('format', r.format); say('repeat', r.repeat); say('cite', r.cite)
  say('balance', r.balance)
  if (process.argv.includes('--symbols')) r.introduces.forEach((x) => console.log(`reads: ${sh(x.card)} first uses ${x.symbols.map((y) => '$' + y + '$').join(' ')}`))
  r.uses.forEach((u) => console.log(`advice: ${sh(u.card)} uses an assumption: ${u.line.slice(0, 120)}`))
  console.log(r.ok ? 'settings: all in the setting part, as a dependency tree' : 'settings: faults above; move cards, never delete a Link')
  process.exit(r.ok ? 0 : 1)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
