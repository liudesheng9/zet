// node mask-links.mjs mask <address> <partPrefix> | restore <address>
// zt refuses to retype a Model Card while its body links any Model part. `mask` saves the body and
// turns every Link whose target is <partPrefix> (the merged head, e.g. Key/4|a) or a Card under it into
// plain text; retype the Card (edit_card facet/emphasis/level ""), then `restore` writes the saved body back.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { cards, writes } from './zt.mjs'

/** A Link to the prefix itself or to any Card filed under it; Term links (`label@…`) never match. */
export function maskPattern(prefix) {
  const esc = prefix.replace(/[|\\/.*+?^${}()[\]]/g, m => String.fromCharCode(92) + m)
  return new RegExp(`\\[\\[(${esc}(?:\\|[^\\]@]*)?)\\]\\]`, 'g')
}

function main([mode, at, prefix]) {
  if (!at || !['mask', 'restore'].includes(mode) || (mode === 'mask' && !prefix)) {
    console.error('usage: node mask-links.mjs mask <address> <partPrefix> | restore <address>'); process.exit(2)
  }
  const saved = path.join(os.tmpdir(), 'zt-mask-' + at.replace(/[^A-Za-z0-9]/g, '_') + '.md')
  if (mode === 'mask') {
    const body = cards([at])[0].body
    if (!fs.existsSync(saved)) fs.writeFileSync(saved, body) // keep the first original across repeated masks
    const re = maskPattern(prefix)
    const n = (body.match(re) || []).length
    if (n) console.log(JSON.stringify(writes([{ tool: 'edit_card', arguments: { at, body: body.replace(re, '$1') } }], [at])))
    console.log(`masked ${n} Link(s); original saved at ${saved}`)
  } else {
    const body = fs.readFileSync(saved, 'utf8')
    console.log(JSON.stringify(writes([{ tool: 'edit_card', arguments: { at, body } }], [at])))
    fs.unlinkSync(saved)
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2))
