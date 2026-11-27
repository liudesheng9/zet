// node check-drafts.mjs <original.json> <draft.json> [--chapters a,b,…]
// Both files: [{address, title, body, facet?}]. Checks each draft against its original:
// every display formula and equation number kept (not for chapter cards, which drop detail on purpose),
// Links only to Cards of the current tree, no 本模型/该模型, titles of at most 40 characters, the
// derivation sections on 推导 cards, and chapter premises only on earlier chapters (--chapters gives
// the main line in order).
import fs from 'node:fs'
import { tree, cards } from './zt.mjs'

const [origFile, draftFile, flag, chapterList] = process.argv.slice(2)
if (!draftFile) { console.error('usage: node check-drafts.mjs <original.json> <draft.json> [--chapters a,b,…]'); process.exit(2) }
const orig = JSON.parse(fs.readFileSync(origFile, 'utf8'))
const drafts = JSON.parse(fs.readFileSync(draftFile, 'utf8'))
const chapters = flag === '--chapters' ? chapterList.split(',') : []
const key = orig[0].address.split('/')[0]
const valid = new Set([key, ...tree(key).map(c => c.address)])
const EQ_ID_SRC = String.raw`[A-Za-z]\.\d{1,2}[a-c]?|[A-Za-z]?\d{1,2}(?:\.\d{1,2})?[a-c]?`
const display = (b) => (b.match(/\$\$[\s\S]*?\$\$/g) || []).map((s) => s.trim().replace(/[ \t]+/g, ' '))
const tags = (b) => new Set([...(b.match(new RegExp(`\\((${EQ_ID_SRC})\\)`, 'g')) || [])])
const premise = /(依赖|用到|沿用|基于|代入|依据|承接|建立在|由此推出|取自|借用)\s*\[\[([^\]@]+)\]\]/g
// Batch-resolve external-tree link targets so unknown citation keys or dead cross-book links are caught.
const extTargets = new Set()
for (const d of drafts) {
  for (const [, l] of (d.body || '').matchAll(/\[\[([^\]]+)\]\]/g)) {
    const a = l.includes('@') ? l.split('@')[1] : l
    if (a !== key && !a.startsWith(key + '/')) extTargets.add(a)
  }
}
const validExt = new Set()
if (extTargets.size) {
  for (const c of cards([...extTargets])) if (c && c.address) validExt.add(c.address)
}
let bad = 0
if (drafts.length !== orig.length) { console.log(`count ${drafts.length} != ${orig.length}`); bad++ }
const countMap = (arr) => { const m = new Map(); for (const x of arr) m.set(x, (m.get(x) || 0) + 1); return m }
const origBatchCounts = countMap(orig.flatMap((o) => display(o.body || '')))
const draftBatchCounts = countMap(drafts.flatMap((d) => display(d.body || '')))
for (const o of orig) {
  const d = drafts.find(x => x.address === o.address)
  if (!d) { console.log(`${o.address}: missing`); bad++; continue }
  const f = []
  const isChapter = chapters.includes(d.address)
  const effectiveFacet = d.facet ?? o.facet ?? ''
  if (!isChapter) {
    const have = countMap(display(d.body))
    const want = countMap(display(o.body))
    let lost = 0
    for (const [eq, n] of want) if ((have.get(eq) || 0) < n) lost += n - (have.get(eq) || 0)
    if (lost) f.push(`lost ${lost} display formula occurrence(s)`)
    const t = tags(d.body + d.title)
    const lt = [...tags(o.body)].filter(x => !t.has(x))
    if (lt.length) f.push(`equation numbers no longer named: ${lt.join(' ')} (fine if only a mention was merged)`)
  } else {
    // A chapter card may move its display formulas to a Side card in the same batch, but total batch multiplicity must hold.
    const dropped = [...new Set(display(o.body))].filter((eq) => (draftBatchCounts.get(eq) || 0) < (origBatchCounts.get(eq) || 0))
    if (dropped.length) f.push(`chapter dropped ${dropped.length} display formula occurrence(s) not preserved in batch multiplicity`)
    for (const m of d.body.matchAll(premise)) {
      const i = chapters.indexOf(m[2])
      if (i >= chapters.indexOf(d.address)) f.push(`rests on ${m[2]}, which is not an earlier chapter`)
    }
  }
  for (const [, l] of d.body.matchAll(/\[\[([^\]]+)\]\]/g)) {
    const a = l.includes('@') ? l.split('@')[1] : l
    if (a.startsWith(key + '/') || a === key) {
      if (!valid.has(a)) f.push(`Link to ${a}, not in the tree`)
    } else if (!validExt.has(a)) {
      f.push(`external Link to ${a}, not found in archive`)
    }
  }
  if (/本模型|该模型/.test(d.body)) f.push('says 本模型/该模型')
  const maxTitle = isChapter ? 80 : 40
  if ([...d.title].length > maxTitle) f.push(`title ${[...d.title].length} characters (max ${maxTitle})`)
  if (effectiveFacet.startsWith('推导')) for (const h of ['目的：', '**思路**', '**起点**', '**步骤**', '**结果**', '**解读**']) if (!d.body.includes(h)) f.push(`no ${h}`)
  if (effectiveFacet.startsWith('论证')) for (const h of ['目标：', '**依据**', '**论证**', '**结论**']) if (!d.body.includes(h)) f.push(`no ${h}`)
  if (effectiveFacet.startsWith('结果')) for (const h of ['**陈述**', '**为什么**', '**依据**']) if (!d.body.includes(h)) f.push(`no ${h}`)
  if (effectiveFacet.startsWith('校准')) for (const h of ['**起点**', '**参数与出处**']) if (!d.body.includes(h)) f.push(`no ${h}`)
  if (effectiveFacet.startsWith('数值实验')) for (const h of ['**实验设定**', '**数值结果**', '**边界**']) if (!d.body.includes(h)) f.push(`no ${h}`)
  if (/^(?:判据|均衡定义|解概念)/.test(effectiveFacet)) for (const h of ['**起点**', '**作用域**']) if (!d.body.includes(h)) f.push(`no ${h}`)
  if (f.length) { bad++; console.log(`${o.address}: ${f.join('; ')}`) }
}
console.log(`${drafts.length} draft(s), ${bad} with findings`)
process.exit(bad ? 1 : 0)
