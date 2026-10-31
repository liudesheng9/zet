// node scripts/model-imports/migrate.mjs [--book <citationKey>] [--apply] [--out <plan.json>]
// Migrates the legacy shares of the box's models to **引入** imports (#213, I8 of #205;
// docs/model-shared-blocks-proposal.md "Migration of the live box"). For every Model Card with a
// （共用） line in its 结构 list it plans one **引入** line per shared root:
//   整块  when the borrower shares more Cards of that Card's block (they are absorbed into the one
//         line), or when a tolerated premise of it lies in its home model (the closure brings it in);
//   单卡  otherwise (the membership does not change).
// It lists, and never changes: the False shares (Links from the Model Card outside 结构 and 引入) and
// the possible copies (an own part that only corresponds to a shared Card: it may restate it; owner
// decision 5). A model in a locked tree is skipped. Without --apply it prints the plan and writes
// nothing; with --apply it edits each planned Model Card through `zt-digest call` and reads every
// migrated model again: tolerated before and after, and the build faults.
// Exit 0 when the plan (or the apply) has no error, 1 otherwise.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

/** Whether `at` lies in the block of `root`: the root, or a Card under one of its Side successors. */
export const inBlock = (at, root) => at === root || (at.startsWith(root + '|') && /^[a-z]/.test(at.slice(root.length + 1)))

const targetOf = (line) => (line.match(/\[\[([^\]@]+)\]\]/) || [])[1] || null
const HEADER = /^\s*(\*\*[^*]+\*\*|#{1,4}\s|来源[：:])/

/** The plan for one model: `record` holds get_model's head, imports, tolerated, false_share, relations,
 * the model's own part addresses (`own`), the Model Card's body and whether its tree is locked. */
export function planModel(record) {
  const legacy = (record.imports || []).filter((i) => i.mode === 'legacy')
  const homeOf = new Map(legacy.map((i) => [i.root, i.home]))
  const roots = legacy.map((i) => i.root)
  const lines = []
  for (const root of roots) {
    // A share inside another share's block is absorbed by that one's 整块 line.
    if (roots.some((other) => other !== root && inBlock(root, other))) continue
    const absorbed = roots.filter((other) => other !== root && inBlock(other, root))
    const home = homeOf.get(root)
    const brought = (record.tolerated || []).filter((t) => (t.part === root || absorbed.includes(t.part)) && (t.premise?.models || []).includes(home))
    const block = absorbed.length > 0 || brought.length > 0
    const reason = absorbed.length
      ? `shares ${absorbed.length} more Card(s) of its block`
      : brought.length
        ? `the closure brings ${brought.length} tolerated premise(s) in from ${home}`
        : 'one Card'
    const text = (record.body.split('\n').find((l) => l.includes(`[[${root}]]`) && l.includes('（共用）')) || '').replace(/^[^：:]*[：:]\s*/, '') || '共用'
    lines.push({ root, mode: block ? '整块' : '单卡', text, absorbed, reason })
  }
  const shared = new Set(roots.concat((record.imports || []).map((i) => i.root)))
  const rel = record.relations || []
  const own = new Set(record.own || [])
  const copies = rel
    .filter((r) => own.has(r.from) && shared.has(r.to) && r.kind === 'corresponds' && !rel.some((x) => x.from === r.from && x.to === r.to && x.kind === 'rests-on'))
    .map((r) => ({ part: r.from, shared: r.to }))
  const falseShares = (record.false_share || []).map((f) => f.target)
  if (record.locked) return { head: record.head, skip: 'locked', lines, falseShares, copies, body: null }
  return { head: record.head, lines, falseShares, copies, body: lines.length ? rewriteBody(record.body, lines) : null }
}

/** A Model Card body with the legacy （共用） lines of the migrated roots (and the shares they absorb) out
 * of 结构, and their **引入** lines in the 引入 section (made before 结构 when there is none). */
export function rewriteBody(body, plan) {
  const drop = new Set(plan.flatMap((l) => [l.root, ...l.absorbed]))
  let lines = body.replace(/\r/g, '').split('\n').filter((l) => !(l.includes('（共用）') && drop.has(targetOf(l))))
  const start = lines.findIndex((l) => /^\s*(\*\*引入\*\*|#{1,4}\s*引入)/.test(l))
  const have = new Set()
  if (start >= 0) {
    let end = start + 1
    while (end < lines.length && !HEADER.test(lines[end])) end++
    for (const l of lines.slice(start + 1, end)) if (targetOf(l)) have.add(targetOf(l))
    const fresh = plan.filter((l) => !have.has(l.root)).map((l) => `- ${l.mode} [[${l.root}]]：${l.text}`)
    let last = end - 1
    while (last > start && !lines[last].trim()) last--
    lines.splice(last + 1, 0, ...fresh)
  } else {
    const fresh = plan.map((l) => `- ${l.mode} [[${l.root}]]：${l.text}`)
    let at = lines.findIndex((l) => /^\s*(\*\*结构\*\*|#{1,4}\s*结构)/.test(l))
    if (at < 0) at = lines.findIndex((l) => /^\s*来源[：:]/.test(l))
    if (at < 0) at = lines.length
    lines.splice(at, 0, '**引入**', '', ...fresh, '')
  }
  return lines.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd()
}

async function main() {
  const argv = process.argv.slice(2)
  const flag = (name) => argv.includes(name)
  const value = (name) => { const i = argv.indexOf(name); return i >= 0 ? argv[i + 1] : null }
  const book = value('--book')
  const { reads, writes } = await import('../../skills/zt-model-fold/scripts/zt.mjs')
  const trees = reads([{ tool: 'list_trees', arguments: {} }])[0].trees
  const locked = new Set(trees.filter((t) => t.locked).map((t) => t.tree))
  // Every Model Card of every tree that holds a model (a type listing stops at 200 rows).
  const heads = trees
    .filter((t) => t.models > 0 && (!book || t.tree === book))
    .flatMap((t) => reads([{ tool: 'get_tree', arguments: { tree: t.tree } }])[0].cards)
    .filter((c) => c.type === 'model' && !c.facet)
    .map((c) => c.address)
  const plans = []
  const before = new Map()
  for (const head of heads) {
    const [m, card] = reads([{ tool: 'get_model', arguments: { at: head } }, { tool: 'get_card', arguments: { at: head } }])
    if (!m.head || m.head.address !== head) continue
    const own = m.build.order.filter((o) => o.line !== 'shared').map((o) => o.address)
    const record = { head, locked: locked.has(head.split('/')[0]), body: card.body, imports: m.imports, tolerated: m.build.tolerated, false_share: m.build.false_share, relations: m.relations, own }
    const plan = planModel(record)
    before.set(head, { tolerated: m.build.tolerated.length, self: m.build.self_sufficient })
    if (plan.lines.length || plan.falseShares.length || plan.copies.length) plans.push(plan)
  }
  for (const p of plans) {
    console.log(`${p.head}${p.skip ? `  (skipped: ${p.skip})` : ''}`)
    for (const l of p.lines) console.log(`  ${l.mode} [[${l.root}]]  ${l.reason}${l.absorbed.length ? `; absorbs ${l.absorbed.join(', ')}` : ''}`)
    for (const f of p.falseShares) console.log(`  false share (listed, unchanged): [[${f}]]`)
    for (const c of p.copies) console.log(`  possible copy (listed, unchanged): ${c.part} corresponds to ${c.shared}`)
  }
  const todo = plans.filter((p) => p.body)
  console.log(`\n${heads.length} Model Card(s) read; ${todo.length} to migrate (${todo.reduce((n, p) => n + p.lines.length, 0)} 引入 lines), ${plans.filter((p) => p.skip).length} skipped (locked), ${plans.reduce((n, p) => n + p.falseShares.length, 0)} false share(s) and ${plans.reduce((n, p) => n + p.copies.length, 0)} possible cop(ies) listed`)
  const out = value('--out')
  if (out) fs.writeFileSync(out, JSON.stringify({ plans, before: Object.fromEntries(before) }, null, 2))
  if (!flag('--apply')) return 0
  let failed = 0
  for (const p of todo) {
    const r = writes([{ tool: 'edit_card', arguments: { at: p.head, body: p.body } }], [p.head])
    if (r.errors.length) { failed++; console.log(`error ${p.head}: ${r.errors.join(' / ')}`); continue }
    const m = reads([{ tool: 'get_model', arguments: { at: p.head } }])[0]
    const was = before.get(p.head)
    const faults = ['closure', 'forward', 'unanchored', 'import_target', 'import_cycle'].filter((k) => (m.build[k] || []).length)
    console.log(`migrated ${p.head}: tolerated ${was.tolerated} -> ${m.build.tolerated.length}, self-sufficient ${was.self} -> ${m.build.self_sufficient}${faults.length ? `, faults: ${faults.join(', ')}` : ''}`)
    if (faults.length || m.build.tolerated.length > was.tolerated) failed++
  }
  return failed ? 1 : 0
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().then((code) => process.exit(code))
