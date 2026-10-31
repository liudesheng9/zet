// node imports.mjs plan <modelHead> [--json]
// node imports.mjs apply <modelHead> [--only <root,root,...>]
// node imports.mjs check <modelHead> [--json]
// Lets one model use other models' building blocks without copying them (#205,
// docs/model-shared-blocks-proposal.md). A building block is imported by a line of the Model Card's
// **引入** section: `单卡 [[X]]` takes the Card X, `整块 [[X]]` takes X with every part under its Side
// successors and the parts of its home model they rest on (its closure), `整块 [[<Model Card>]]` a
// whole model.
//   plan   proposes the import lines the model needs, from get_model:
//            premise      a part this model rests on that lives in another model (a closure fault);
//                         one 整块 line when several such premises lie in one block
//            upgrade      a 单卡 import whose tolerated premises its home model holds: 整块 brings them in
//            legacy       a （共用） line of 结构 (the older form): an import line instead
//            false share  a part of another model that the Model Card only mentions, but a part of
//                         this model rests on it: an import line
//          and names, for the reader: a rests-on Link to a whole Model Card (rest on its parts), a
//          premise that is no model's part (restate it or link it as a neighbour), a possible copy (an
//          own part that only corresponds to a part of another model: import that part and keep only
//          the change, as a variant that specializes it), and a false share nothing rests on (move the
//          comparison to a part).
//   apply  writes the proposed lines into the Model Card (only those named with --only) and reads
//          the model again; a locked tree is refused.
//   check  exit 0 when the model has no import fault (import_target, import_cycle), no premise in
//          another model and no legacy line; tolerated premises, false shares and copies are advice.
import path from 'node:path'
import { fileURLToPath } from 'node:url'

/** Whether `at` lies in the block of `root`: the root, or a Card under one of its Side successors. */
export const inBlock = (at, root) => at === root || (at.startsWith(root + '|') && /^[a-z]/.test(at.slice(root.length + 1)))
const targetOf = (line) => (line.match(/\[\[([^\]@]+)\]\]/) || [])[1] || null
const HEADER = /^\s*(\*\*[^*]+\*\*|#{1,4}\s|来源[：:])/
const afterColon = (note) => (note || '').replace(/^[^：:]*[：:]\s*/, '')

/** The roots to import from a list of candidate addresses: a candidate inside another's block is
 * absorbed by that one's 整块 line. */
function grouped(addresses) {
  const out = []
  for (const at of addresses) {
    if (addresses.some((other) => other !== at && inBlock(at, other))) continue
    const absorbed = addresses.filter((other) => other !== at && inBlock(other, at))
    out.push({ root: at, absorbed })
  }
  return out
}

/** The plan for one model (`m`: get_model of its Model Card). `changes` are import lines to write;
 * `manual` and `advice` are for the reader. */
export function propose(m) {
  const b = m.build || {}
  const order = b.order || []
  const own = new Set(order.filter((o) => o.line !== 'shared').map((o) => o.address))
  const imports = m.imports || []
  const changes = []
  const manual = []
  const advice = []
  // Premises in other models.
  const outside = new Map()
  for (const c of b.closure || []) {
    const card = c.card || {}
    if (card.type === 'model' && card.facet && (card.models || []).length) {
      if (!outside.has(c.to)) outside.set(c.to, c)
    } else if (card.type === 'model' && !card.facet) {
      manual.push({ kind: 'whole model', from: c.from, target: c.to, fix: `${c.from} rests on the Model Card ${c.to}: rest on the part of it the step uses, and import that part` })
    } else {
      manual.push({ kind: 'not a part', from: c.from, target: c.to, fix: `${c.from} rests on ${c.to}, which is no model's part: restate it in a part of this model, or keep it as a neighbour` })
    }
  }
  for (const g of grouped([...outside.keys()])) {
    const c = outside.get(g.root)
    changes.push({ why: 'premise', root: g.root, mode: g.absorbed.length ? '整块' : '单卡', text: c.card.title || afterColon(c.note), absorbed: g.absorbed })
  }
  // 单卡 imports whose home model holds their tolerated premises.
  for (const i of imports.filter((x) => x.mode === 'card')) {
    if ((b.tolerated || []).some((t) => t.part === i.root && (t.premise?.models || []).includes(i.home))) {
      changes.push({ why: 'upgrade', root: i.root, mode: '整块', text: '', absorbed: [] })
    }
  }
  // Legacy （共用） lines.
  const legacy = imports.filter((x) => x.mode === 'legacy')
  for (const g of grouped(legacy.map((x) => x.root))) {
    const i = legacy.find((x) => x.root === g.root)
    const brought = (b.tolerated || []).some((t) => (t.part === g.root || g.absorbed.includes(t.part)) && (t.premise?.models || []).includes(i.home))
    changes.push({ why: 'legacy', root: g.root, mode: g.absorbed.length || brought ? '整块' : '单卡', text: i.title || '', absorbed: g.absorbed })
  }
  // False shares: imported when a part of this model rests on them, else left to the reader.
  const rel = m.relations || []
  for (const f of b.false_share || []) {
    if (rel.some((r) => own.has(r.from) && r.to === f.target && r.kind === 'rests-on')) {
      changes.push({ why: 'false share', root: f.target, mode: '单卡', text: afterColon(f.note) || f.target, absorbed: [] })
    } else {
      advice.push({ kind: 'false share', target: f.target, fix: `the Model Card mentions ${f.target} of another model, which makes it a member: move the comparison to a part, or import it if a part rests on it` })
    }
  }
  // Possible copies: an own part that only corresponds to a part of another model.
  const shared = new Set(order.filter((o) => o.line === 'shared').map((o) => o.address))
  const copies = [
    ...rel.filter((r) => own.has(r.from) && shared.has(r.to) && r.kind === 'corresponds').map((r) => [r.from, r.to]),
    ...(m.neighbours || []).filter((n) => n.way === 'out' && own.has(n.from) && n.kind === 'corresponds' && n.card?.type === 'model' && n.card?.facet).map((n) => [n.from, n.to]),
  ].filter(([from, to]) => !rel.some((r) => r.from === from && r.to === to && r.kind === 'rests-on'))
  for (const [part, other] of copies) {
    manual.push({ kind: 'copy', from: part, target: other, fix: `${part} corresponds to ${other} of another model: if it restates it, import ${other} and keep in ${part} only what this model changes: a variant that rests on it (依赖 [[${other}]]) and says how it differs (放宽 [[${other}]] for a relaxed assumption, 特例 for a special case)` })
  }
  return { head: m.head?.address, changes, manual, advice }
}

/** A Model Card body with the import lines of `changes`: a new root gets a line, an upgrade rewrites
 * the mode word of its line, a root a 整块 line absorbs loses its own line, and the legacy （共用）
 * lines of these roots leave 结构. The 引入 section is made before 结构 when there is none. */
export function rewrite(body, changes) {
  const roots = new Set(changes.map((c) => c.root))
  const absorbed = new Set(changes.flatMap((c) => c.absorbed))
  let lines = body.replace(/\r/g, '').split('\n').filter((l) => !(l.includes('（共用）') && (roots.has(targetOf(l)) || absorbed.has(targetOf(l)))))
  const start = lines.findIndex((l) => /^\s*(\*\*引入\*\*|#{1,4}\s*引入)/.test(l))
  const line = (c) => `- ${c.mode} [[${c.root}]]：${c.text || c.root}`
  if (start < 0) {
    let at = lines.findIndex((l) => /^\s*(\*\*结构\*\*|#{1,4}\s*结构)/.test(l))
    if (at < 0) at = lines.findIndex((l) => /^\s*来源[：:]/.test(l))
    if (at < 0) at = lines.length
    lines.splice(at, 0, '**引入**', '', ...changes.map(line), '')
    return lines.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd()
  }
  let end = start + 1
  while (end < lines.length && !HEADER.test(lines[end])) end++
  const section = lines.slice(start + 1, end)
    .filter((l) => !absorbed.has(targetOf(l)))
    .map((l) => {
      const c = changes.find((x) => x.root === targetOf(l))
      return c ? l.replace(/^(\s*[-*]\s*)(单卡|整块)?\s*/, `$1${c.mode} `) : l
    })
  const have = new Set(section.map(targetOf).filter(Boolean))
  while (section.length && !section[section.length - 1].trim()) section.pop()
  section.push(...changes.filter((c) => !have.has(c.root)).map(line), '')
  lines.splice(start + 1, end - start - 1, ...section)
  return lines.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd()
}

/** The gate: faults that keep the model's imports from being right, and advice. */
export function audit(m) {
  const b = m.build || {}
  const faults = [
    ...(b.import_target || []).map((f) => ({ kind: 'import target', target: f.target, fix: f.fix })),
    ...(b.import_cycle || []).map((f) => ({ kind: 'import cycle', target: f.to, fix: f.fix })),
    ...(b.closure || []).filter((c) => c.card?.type === 'model' && c.card?.facet && (c.card.models || []).length)
      .map((c) => ({ kind: 'outside premise', target: c.to, fix: `${c.from} rests on ${c.to} of ${c.card.models[0]}: import it (plan proposes the line)` })),
    ...(m.imports || []).filter((i) => i.mode === 'legacy').map((i) => ({ kind: 'legacy share', target: i.root, fix: `${i.root} is shared by a （共用） line: write it as an import line (plan proposes it)` })),
  ]
  const p = propose(m)
  const advice = [
    ...(b.tolerated || []).map((t) => ({ kind: 'tolerated', target: t.premise?.address, fix: `${t.part} rests on ${t.premise?.address}, which stays in its home model: a reader leaves this model there` })),
    ...p.advice,
    ...p.manual.filter((x) => x.kind === 'copy'),
  ]
  return { head: m.head?.address, ok: faults.length === 0, faults, advice }
}

async function main() {
  const [step, head, ...rest] = process.argv.slice(2)
  if (!['plan', 'apply', 'check'].includes(step) || !head) {
    console.error('usage: node imports.mjs plan|apply|check <modelHead> [--json] [--only root,...]')
    process.exit(2)
  }
  const { reads, writes, model } = await import('./zt.mjs')
  const m = model(head)
  if (!m.head || m.head.address !== head) {
    console.error(`${head} is not a Model Card${m.head ? `: it belongs to the model ${m.head.address}; give that address` : ''}`)
    process.exit(2)
  }
  const json = rest.includes('--json')
  if (step === 'check') {
    const r = audit(m)
    if (json) console.log(JSON.stringify(r, null, 2))
    else {
      for (const f of r.faults) console.log(`fault (${f.kind}): ${f.fix}`)
      for (const a of r.advice) console.log(`advice (${a.kind}): ${a.fix}`)
      console.log(r.ok ? `imports: ${head} takes what it uses from other models by import lines` : `imports: ${r.faults.length} fault(s); run plan, then apply`)
    }
    process.exit(r.ok ? 0 : 1)
  }
  const p = propose(m)
  if (step === 'plan') {
    if (json) console.log(JSON.stringify(p, null, 2))
    else {
      for (const c of p.changes) console.log(`${c.why}: ${c.mode} [[${c.root}]]${c.absorbed.length ? ` (absorbs ${c.absorbed.join(', ')})` : ''}`)
      for (const x of p.manual) console.log(`by hand (${x.kind}): ${x.fix}`)
      for (const x of p.advice) console.log(`advice (${x.kind}): ${x.fix}`)
      console.log(p.changes.length ? `${p.changes.length} import line(s) to write: node imports.mjs apply ${head}` : 'nothing to import')
    }
    process.exit(0)
  }
  const tree = head.split('/')[0]
  const locked = reads([{ tool: 'list_trees', arguments: {} }])[0].trees.some((t) => t.tree === tree && t.locked)
  if (locked) { console.error(`${tree} is locked: ask the owner to unlock it`); process.exit(1) }
  const onlyAt = rest.indexOf('--only')
  const only = onlyAt >= 0 ? new Set((rest[onlyAt + 1] || '').split(',').filter(Boolean)) : null
  const changes = p.changes.filter((c) => !only || only.has(c.root))
  if (!changes.length) { console.log('nothing to write'); process.exit(0) }
  const card = reads([{ tool: 'get_card', arguments: { at: head } }])[0]
  const r = writes([{ tool: 'edit_card', arguments: { at: head, body: rewrite(card.body, changes) } }], [head])
  if (r.errors.length) { console.error(r.errors.join('\n')); process.exit(1) }
  const after = audit(model(head))
  console.log(`wrote ${changes.length} import line(s); tolerated ${(m.build.tolerated || []).length} -> ${after.advice.filter((a) => a.kind === 'tolerated').length}; ${after.ok ? 'check passes' : `${after.faults.length} fault(s) left: run plan again`}`)
  process.exit(after.ok ? 0 : 1)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
