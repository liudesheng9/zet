// node check-drafts.mjs <original.json> <draft.json> [--chapters a,b,…]
// Both files: [{address, title, body, facet?}]. Checks each draft against its original:
// every display formula and equation number kept (not for chapter cards, which drop detail on purpose),
// Links only to Cards of the current tree, no 本模型/该模型, titles of at most 40 characters, the
// derivation sections on 推导 cards, and chapter premises only on earlier chapters (--chapters gives
// the main line in order).
import fs from 'node:fs'
import { tree } from './zt.mjs'

const [origFile, draftFile, flag, chapterList] = process.argv.slice(2)
if (!draftFile) { console.error('usage: node check-drafts.mjs <original.json> <draft.json> [--chapters a,b,…]'); process.exit(2) }
const orig = JSON.parse(fs.readFileSync(origFile, 'utf8'))
const drafts = JSON.parse(fs.readFileSync(draftFile, 'utf8'))
const chapters = flag === '--chapters' ? chapterList.split(',') : []
const key = orig[0].address.split('/')[0]
const valid = new Set([key, ...tree(key).map(c => c.address)])
const display = b => (b.match(/\$\$[\s\S]*?\$\$/g) || []).map(s => s.replace(/\s+/g, ''))
const tags = b => new Set(b.match(/\(\d{1,2}[a-c]?\)/g) || [])
const premise = /(依赖|用到|沿用|基于|代入|依据|承接|建立在|由此推出|取自|借用)\s*\[\[([^\]@]+)\]\]/g
let bad = 0
if (drafts.length !== orig.length) { console.log(`count ${drafts.length} != ${orig.length}`); bad++ }
for (const o of orig) {
  const d = drafts.find(x => x.address === o.address)
  if (!d) { console.log(`${o.address}: missing`); bad++; continue }
  const f = []
  const isChapter = chapters.includes(d.address)
  if (!isChapter) {
    const have = new Set(display(d.body))
    const lost = display(o.body).filter(x => !have.has(x))
    if (lost.length) f.push(`lost ${lost.length} display formula(s)`)
    const t = tags(d.body + d.title)
    const lt = [...tags(o.body)].filter(x => !t.has(x))
    if (lt.length) f.push(`equation numbers no longer named: ${lt.join(' ')} (fine if only a mention was merged)`)
  } else {
    for (const m of d.body.matchAll(premise)) {
      const i = chapters.indexOf(m[2])
      if (i >= chapters.indexOf(d.address)) f.push(`rests on ${m[2]}, which is not an earlier chapter`)
    }
  }
  for (const [, l] of d.body.matchAll(/\[\[([^\]]+)\]\]/g)) { const a = l.includes('@') ? l.split('@')[1] : l; if (!valid.has(a)) f.push(`Link to ${a}, not in the tree`) }
  if (/本模型|该模型/.test(d.body)) f.push('says 本模型/该模型')
  if ([...d.title].length > 40) f.push(`title ${[...d.title].length} characters`)
  if ((o.facet || '').startsWith('推导')) for (const h of ['目的：', '**思路**', '**起点**', '**步骤**', '**结果**', '**解读**']) if (!d.body.includes(h)) f.push(`no ${h}`)
  if (f.length) { bad++; console.log(`${o.address}: ${f.join('; ')}`) }
}
console.log(`${drafts.length} draft(s), ${bad} with findings`)
process.exit(bad ? 1 : 0)
