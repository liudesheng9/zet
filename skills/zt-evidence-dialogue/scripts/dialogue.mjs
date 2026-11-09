// The evidence dialogue of one empirical model (docs/empirical-models-proposal.md §9.5): the conversation of §7
// between its estimates and the rest of the box, found by script, judged by DeepSeek, written by script.
//   node dialogue.mjs survey <head> [--out <dir>]   the candidate pairs and the judge's message files
//   node dialogue.mjs apply  <head> --answers <file> [--out <dir>]   writes the judged Links (agent Cards only)
//   node dialogue.mjs check  <head> [--out <dir>]   every written Link resolves; a second survey proposes nothing
// A candidate pair is one estimate of this model and one Card elsewhere that may state the same estimand: an
// estimate part of another empirical model, or a Finding Card of another book, whose text shares a Term with this
// model's estimand (a Term link `[[label@address]]`, or a Term the zt-term-sync pass linked to it). A pair that
// is already linked either way is never proposed again.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

/** The Term addresses a text links (`[[label@address]]`). */
export function termTargets(text) {
  return [...String(text || '').matchAll(/\[\[[^\]@]*@([^\]]+)\]\]/g)].map((m) => m[1].trim())
}

/** The plain Link targets of a text (`[[address]]`, Term links excluded). */
export function linkTargets(text) {
  return [...String(text || '').matchAll(/\[\[([^\]@]+)\]\]/g)].map((m) => m[1].trim())
}

/** One bold section of a body (`**估计对象**`), up to the next bold label or the source line. */
export function section(body, name) {
  const lines = String(body || '').split('\n')
  const at = lines.findIndex((l) => l.trim().replace(/[：:]$/, '') === `**${name}**`)
  if (at < 0) return ''
  const out = []
  for (const l of lines.slice(at + 1)) {
    if (/^\s*\*\*[^*]+\*\*\s*[：:]?\s*$/.test(l) || /^\s*来源[：:]/.test(l)) break
    out.push(l)
  }
  return out.join('\n').trim()
}

/** The Terms of a set of texts, widened by the equivalences zt-term-sync wrote (a Term Card's Links to Term Cards). */
export function widen(terms, equivalents) {
  const out = new Set(terms)
  for (const t of terms) for (const e of equivalents.get(t) || []) out.add(e)
  return out
}

/**
 * The Terms of the model's estimand: the Terms the head links in its title, **问题** and **估计对象**. A Term is
 * linked only where a Card first names it (ebrill2001modern/5|a links 增值税 in **问题**, and **估计对象** names it as
 * plain text), so **估计对象** alone often holds none. A Term of a later section (a regressor) is not the estimand.
 */
export function headTerms(head) {
  const body = head.body || ''
  const text = [head.title || '', section(body, '问题'), section(body, '估计对象')].join('\n')
  return [...new Set(termTargets(text))]
}

/**
 * The Terms one estimate of ours pairs on: the Terms of what it measures (its own text's Terms, widened, without the
 * head's Terms that every estimate of the model shares), `specific`; an estimate that names no Term of its own pairs
 * on the head's Terms. `widened`: the estimate's own Terms with their zt-term-sync equivalents (default: its links).
 */
export function estimateTerms(text, head, widened = new Set(termTargets(text))) {
  const own = new Set([...widened].filter((t) => !head.has(t)))
  return own.size ? { terms: own, specific: true } : { terms: new Set(head), specific: false }
}

/**
 * The other side of the dialogue, read from every Literature tree (`get_tree` replies): the heads of the other
 * empirical models, and the Finding Cards of the other books. search_cards stops at 200 rows and has no paging.
 */
export function otherSide(trees, head) {
  const key = head.split('/')[0]
  const rows = trees.flatMap((t) => t.cards || [])
  return {
    heads: rows.filter((r) => r.type === 'model' && r.level === 'empirical' && !r.head && r.address !== head).map((r) => r.address),
    findings: rows.filter((r) => r.type === 'finding' && !r.address.startsWith(key + '/')).map((r) => r.address),
  }
}

/** The keys of judged pairs, in both directions (an A-B verdict also answers B-A). */
export function judgedKeys(records) {
  const keys = new Set()
  for (const r of records) { keys.add(`${r.from}>${r.to}`); keys.add(`${r.to}>${r.from}`) }
  return keys
}

/** The pairs no earlier judgment answered, numbered again. */
export function unjudged(pairs, keys) {
  return pairs.filter((p) => !keys.has(`${p.from}>${p.to}`)).map((p, k) => ({ ...p, id: `p${k + 1}` }))
}

/**
 * The judged record of one box: a JSON-lines file under the home folder, one per zt config folder (the test box
 * and the live box never share one).
 */
export function judgedFile(env = process.env, home = os.homedir()) {
  const box = env.ZT_CONFIG_DIR ? String(env.ZT_CONFIG_DIR).replace(/[^A-Za-z0-9]+/g, '-').replace(/^-|-$/g, '') : 'default'
  return path.join(home, '.zt-evidence-dialogue', `judged-${box}.jsonl`)
}

function readJudged(file = judgedFile()) {
  if (!fs.existsSync(file)) return []
  return fs.readFileSync(file, 'utf8').split(/\r?\n/).filter((l) => l.startsWith('{')).map((l) => JSON.parse(l))
}

/**
 * The candidate pairs, not already linked either way. `ours`: [{address, body, terms, specific}] (estimateTerms),
 * `theirs`: [{address, body, kind, own, head}]: `own` the Terms of their Card's text, `head` its model head's Terms
 * (empty for a Finding Card). A specific estimate pairs with a Card whose own text shares one of its Terms; an
 * estimate without Terms of its own pairs with a Card that shares a head Term in its text or its head.
 */
export function pairsFor(ours, theirs) {
  const pairs = []
  for (const t of theirs) {
    for (const o of ours) {
      const pool = o.specific ? t.own : new Set([...t.own, ...t.head])
      const shared = [...pool].filter((x) => o.terms.has(x))
      if (!shared.length) continue
      const linked = linkTargets(o.body).includes(t.address) || linkTargets(t.body).includes(o.address)
      if (linked) continue
      pairs.push({ id: `p${pairs.length + 1}`, from: o.address, to: t.address, kind: t.kind, shared })
    }
  }
  return pairs
}

/** The judge's messages: twelve pairs a message, each with both texts. */
export function messages(pairs, text) {
  const out = []
  for (let i = 0; i < pairs.length; i += 12) {
    const chunk = pairs.slice(i, i + 12)
    out.push(chunk.map((p) => `## ${p.id}\n\n### 本模型的估计 [[${p.from}]]\n\n${text(p.from)}\n\n### 另一张卡（${p.kind === 'finding' ? 'Finding Card' : '估计'}）[[${p.to}]]\n\n${text(p.to)}\n\n共同的术语：${p.shared.join('、')}`).join('\n\n'))
  }
  return out
}

/** A body with one more line in its `**对应**` section (made when missing); none when the Link is there already. */
export function withCorrespondence(body, target, line) {
  if (linkTargets(body).includes(target)) return null
  const lines = String(body || '').split('\n')
  const at = lines.findIndex((l) => l.trim() === '**对应**')
  if (at >= 0) {
    let end = at + 1
    while (end < lines.length && lines[end].trim() && !/^\s*\*\*[^*]+\*\*\s*$/.test(lines[end]) && !/^\s*来源[：:]/.test(lines[end])) end++
    lines.splice(end, 0, line)
    return lines.join('\n')
  }
  const source = lines.findIndex((l) => /^\s*来源[：:]/.test(l))
  const block = ['**对应**', line, '']
  if (source >= 0) lines.splice(source, 0, ...block)
  else lines.push('', ...block.slice(0, 2))
  return lines.join('\n')
}

const KINDS = new Set(['对照', '印证', '反驳'])

/** The Links an answer file asks for: one per judged pair of the same estimand with a kind word. */
export function linksFromAnswers(pairs, answers) {
  const byId = new Map(pairs.map((p) => [p.id, p]))
  const out = []
  for (const a of answers) {
    const p = byId.get(a.pair)
    // A not-comparable pair of the same estimand is written with 对照 (judge.zh.md); same: false never is.
    if (!p || a.same !== true || !KINDS.has(a.kind)) continue
    const reason = String(a.reason || '').replace(/[。\s]+$/, '')
    out.push({ pair: p.id, from: p.from, to: p.to, line: `${a.kind} [[${p.to}]]：${reason}。`, back: p.kind === 'estimate' ? `${a.kind} [[${p.from}]]：${reason}。` : null })
  }
  return out
}

// --- the box ---------------------------------------------------------------------------------------------------

async function box() {
  const { reads, writes, model, cards } = await import('./zt.mjs')
  return { reads, writes, model, cards }
}

async function survey(head, out) {
  const { reads, model, cards } = await box()
  const m = model(head)
  if (m.level !== 'empirical') throw new Error(`${head} is not an empirical model (level ${m.level})`)
  const order = m.build.order || []
  const ourAddresses = order.filter((o) => ['estimate', 'parameter'].includes(o.role) && o.line !== 'shared').map((o) => o.address)
  const ours = cards(ourAddresses)
  // Term equivalences: each Term of our estimand and its zt-term-sync Links to Term Cards.
  const equivalents = new Map()
  const termCard = (at) => reads([{ tool: 'get_card', arguments: { at } }])[0]
  const equiv = (t) => {
    if (!equivalents.has(t)) {
      const c = termCard(t)
      equivalents.set(t, c ? (c.outbound || []).filter((l) => l.kind !== 'term' && !l.broken).map((l) => l.address) : [])
    }
    return equivalents.get(t)
  }
  const widened = (terms) => widen(terms, new Map(terms.map((t) => [t, equiv(t)])))
  const estimandTerms = widened(headTerms(m.head))
  const oursTerms = ours.map((c) => ({ ...c, ...estimateTerms('', estimandTerms, widened(termTargets(`${c.title}\n${c.body}`))) }))
  // Their side: the estimates of every other empirical model (with their head's Terms), and the Finding Cards of
  // other books.
  const literature = reads([{ tool: 'list_trees', arguments: {} }])[0].trees.filter((t) => t.kind === 'literature')
  const side = otherSide(reads(literature.map((t) => ({ tool: 'get_tree', arguments: { tree: t.tree } }))), head)
  const theirs = []
  const ownTerms = (c) => new Set(termTargets(`${c.title}\n${c.body}`))
  for (const h of side.heads) {
    const other = model(h)
    const theirHead = new Set(headTerms(other.head))
    const addresses = (other.build.order || []).filter((o) => ['estimate', 'parameter'].includes(o.role) && o.line !== 'shared').map((o) => o.address)
    for (const c of cards(addresses)) theirs.push({ ...c, kind: 'estimate', own: ownTerms(c), head: theirHead })
  }
  for (const c of cards(side.findings)) theirs.push({ ...c, kind: 'finding', own: ownTerms(c), head: new Set() })
  // A pair judged before (by this model's survey or the other model's) is not judged again.
  const pairs = unjudged(pairsFor(oursTerms, theirs), judgedKeys(readJudged()))
  const text = (at) => { const c = [...ours, ...theirs].find((x) => x.address === at); return `${c.title}\n\n${c.body}` }
  fs.mkdirSync(out, { recursive: true })
  fs.writeFileSync(path.join(out, 'pairs.json'), JSON.stringify({ head, estimand_terms: [...estimandTerms], pairs }, null, 2))
  const files = messages(pairs, text).map((body, k) => {
    const file = path.join(out, `judge-${k + 1}.md`)
    fs.writeFileSync(file, `# 证据对话：[[${head}]] 第 ${k + 1} 组\n\n${body}\n`)
    return file
  })
  return { head, ours: ours.length, theirs: theirs.length, pairs: pairs.length, messages: files }
}

async function apply(head, out, answersFile) {
  const { reads, writes } = await box()
  const { pairs } = JSON.parse(fs.readFileSync(path.join(out, 'pairs.json'), 'utf8'))
  const answers = fs.readFileSync(answersFile, 'utf8').split(/\r?\n/).map((l) => l.trim()).filter((l) => l.startsWith('{')).map((l) => JSON.parse(l))
  const links = linksFromAnswers(pairs, answers)
  // Every answered pair goes into the box's judged record, whatever its verdict.
  const byId = new Map(pairs.map((p) => [p.id, p]))
  const judged = answers.filter((a) => byId.has(a.pair)).map((a) => ({ from: byId.get(a.pair).from, to: byId.get(a.pair).to, same: a.same, kind: a.kind, verdict: a.verdict, head, at: new Date().toISOString() }))
  if (judged.length) {
    fs.mkdirSync(path.dirname(judgedFile()), { recursive: true })
    fs.appendFileSync(judgedFile(), judged.map((j) => `${JSON.stringify(j)}\n`).join(''))
  }
  const calls = []
  const bodies = new Map()
  const body = (at) => {
    if (!bodies.has(at)) bodies.set(at, reads([{ tool: 'get_card', arguments: { at } }])[0])
    return bodies.get(at)
  }
  for (const l of links) {
    for (const [at, target, line] of [[l.from, l.to, l.line], ...(l.back ? [[l.to, l.from, l.back]] : [])]) {
      const card = body(at)
      // A human Card is never a carrier.
      if (!card || card.workspace !== 'agent') continue
      const next = withCorrespondence(card.body, target, line)
      if (next) { card.body = next; calls.push({ tool: 'edit_card', arguments: { at, body: next } }) }
    }
  }
  const merged = new Map()
  for (const c of calls) merged.set(c.arguments.at, c)
  const result = merged.size ? writes([...merged.values()]) : { ok: 0, errors: [] }
  fs.writeFileSync(path.join(out, 'applied.json'), JSON.stringify({ head, links, written: [...merged.keys()], errors: result.errors }, null, 2))
  return { head, links: links.length, written: merged.size, errors: result.errors.length }
}

async function check(head, out) {
  const { reads } = await box()
  const applied = JSON.parse(fs.readFileSync(path.join(out, 'applied.json'), 'utf8'))
  const broken = []
  for (const l of applied.links) {
    for (const at of [l.from, l.to]) {
      const card = reads([{ tool: 'get_card', arguments: { at } }])[0]
      if (!card) broken.push(at)
    }
  }
  const again = await survey(head, path.join(out, 'second'))
  const judged = new Set(applied.links.map((l) => `${l.from}>${l.to}`))
  const fresh = JSON.parse(fs.readFileSync(path.join(out, 'second', 'pairs.json'), 'utf8')).pairs.filter((p) => judged.has(`${p.from}>${p.to}`))
  return { head, broken: broken.length, proposed_again: fresh.length, ok: !broken.length && !fresh.length }
}

async function main() {
  const [verb, head, ...rest] = process.argv.slice(2)
  const opt = (name) => { const i = rest.indexOf(name); return i >= 0 ? rest[i + 1] : null }
  if (!verb || !head) { console.error('usage: node dialogue.mjs survey|apply|check <head> [--out <dir>] [--answers <file>]'); process.exit(2) }
  const out = opt('--out') || path.join(os.tmpdir(), 'zt-evidence-dialogue', head.replace(/[^A-Za-z0-9]+/g, '-'))
  const result = verb === 'survey' ? await survey(head, out) : verb === 'apply' ? await apply(head, out, opt('--answers')) : await check(head, out)
  console.log(JSON.stringify(result))
  process.exit(result.ok === false ? 1 : 0)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
