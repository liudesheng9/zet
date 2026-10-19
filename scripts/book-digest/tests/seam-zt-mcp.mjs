// A test seam for the zt-digest tools: a stand-in for `zt mcp` that forwards every message to the
// real `zt mcp` (zt on PATH) and injects one archive event. A test points zt-digest at it with
// ZT_CHECK_ZT=<this folder>\seam-zt-mcp.cmd (zt-digest starts `<ZT_CHECK_ZT> mcp`), so a tool that
// reads and writes in-process meets the same faults and races as the scripts met through the
// helper stand-ins faulting-zt-mcp.ps1 and racing-zt-mcp.ps1 (removed), with the same variables:
//
//   ZT_RACE_TARGET, ZT_RACE_FLAG, ZT_RACE_LINE   once (no flag file yet), before the first edit_card
//       of the Card ZT_RACE_TARGET, append ZT_RACE_LINE to that Card, as a parallel digest stage would
//       between a compiler's read and its write; then create the flag file.
//   ZT_RACE_AFTER_READ   (see above) the injected edit happens before an addressed read is
//       forwarded, so the read already sees it.
//   ZT_RACE_AFTER_BULK_REPLY, with ZT_RACE_TARGET/ZT_RACE_LINE/ZT_RACE_FLAG   once, AFTER an
//       addressed `get_cards` reply is produced and before it is sent back: another writer commits
//       its unrelated Card while the caller still holds this unit's own output read.
//   ZT_FAIL_TREE                                 get_tree of this tree fails.
//   ZT_FAIL_AFTER_EDIT, ZT_FAIL_CARD, ZT_FAIL_FLAG   after an edit_card of ZT_FAIL_AFTER_EDIT succeeds,
//       create the flag file; while it exists, get_card of ZT_FAIL_CARD fails.
//   ZT_RACE_AFTER_CREATE, with ZT_RACE_TARGET/ZT_RACE_LINE/ZT_RACE_FLAG   once, AFTER a non-preview
//       apply_batch that creates a Card succeeds and before its reply is sent: another writer commits
//       its change to ZT_RACE_TARGET while the caller still holds this unit's creation receipt, so a
//       two-step transfer meets a changed old Card between its writes.
//   ZT_SLOW_TOOL, ZT_SLOW_MS, ZT_SLOW_FLAG   every call of this tool waits ZT_SLOW_MS before it is
//       forwarded, so a real command outlives a shortened ledger lease; the flag marks the wait.
// ZT_DRIFT_REVISION   a parallel writer that keeps committing: every read reply reports a higher
//       archive revision than the one before it, so a caller that needs one view must retry once
//       and then report its changing-view fault. `once` drifts on the first drifted read only.
//       ZT_DRIFT_AFTER <n> leaves the first n-1 read replies (get_cards, export_titles) clean, so a
//       test can drift one read set of a command instead of its earliest reads.
//       ZT_DRIFT_MIN_ADDRESSES <n> drifts only a get_cards reply that carries at least n Cards, so a
//       test can cross one bulk read set (a bounded body read) and leave identity probes alone.
//   ZT_SCHOLAR_FIXTURE   a JSON file that answers the read-only Scholar Archive bridge, so
//       `zt-digest prepare -Book` runs without a live archive. It holds `works` (a Work id to the
//       `get_work` result), `search` (a normalised title to Work ids), `files` (a Work id to
//       `list_work_files` rows), `documents` (a file id to `{name, relative_path, path}`, whose
//       bytes the seam reads and hashes) and `errors` (`get_work`/`work_files`/`file` lists or a
//       `list_works` flag) for the refusal paths. A bridge tool the file does not answer is a
//       fault, so a test proves which calls the resolution made.
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import readline from 'node:readline'
import path from 'node:path'
import os from 'node:os'

const env = process.env
const real = spawn('zt', process.argv.slice(2).length ? process.argv.slice(2) : ['mcp'], { stdio: ['pipe', 'pipe', 'ignore'] })
const waiting = new Map()
readline.createInterface({ input: real.stdout }).on('line', line => {
  let message
  try { message = JSON.parse(line) } catch { return }
  const resolve = waiting.get(message.id)
  if (resolve) { waiting.delete(message.id); resolve(message) }
})
real.on('exit', () => process.exit(0))

let own = 1_000_000_000
let identityViews = 0
const ask = message => new Promise(resolve => { waiting.set(message.id, resolve); real.stdin.write(JSON.stringify(message) + '\n') })
const call = (name, args) => ask({ jsonrpc: '2.0', id: own++, method: 'tools/call', params: { name, arguments: args } })
const send = message => process.stdout.write(JSON.stringify(message) + '\n')
const failed = reply => !!(reply.error || (reply.result && reply.result.isError))
const fault = (id, text) => ({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text }], isError: true } })
const flag = path => !!path && fs.existsSync(path)
// The Scholar Archive fixture of `ZT_SCHOLAR_FIXTURE`, read on every call so a test may change it.
const scholar = () => {
  if (!env.ZT_SCHOLAR_FIXTURE || !fs.existsSync(env.ZT_SCHOLAR_FIXTURE)) return null
  try { return JSON.parse(fs.readFileSync(env.ZT_SCHOLAR_FIXTURE, 'utf8')) } catch { return null }
}
const digest = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex')
// ZT_DRIFT_REVISION: a read whose archive revision is not the archive's own current one.
let readReplies = 0
let drifted = 0
const drift = reply => {
  if (!env.ZT_DRIFT_REVISION || failed(reply)) return
  const value = reply.result.structuredContent
  if (!value || typeof value.archive_revision !== 'number') return
  readReplies += 1
  if (env.ZT_DRIFT_AFTER && readReplies < Number(env.ZT_DRIFT_AFTER)) return
  if (env.ZT_DRIFT_MIN_ADDRESSES && (value.cards ? value.cards.length : 0) < Number(env.ZT_DRIFT_MIN_ADDRESSES)) return
  drifted += 1
  if (env.ZT_DRIFT_REVISION === 'once' && drifted > 1) return
  value.archive_revision = Number(value.archive_revision) + drifted
  reply.result.content = [{ type: 'text', text: JSON.stringify(value) }]
}
const numbered = (list, id) => (list || []).map(Number).includes(Number(id))
// One `scholar_archive_read` answer: `{result}` or `{error}`.
const scholarRead = (fixture, inner, args) => {
  const errors = fixture.errors || {}
  if (inner === 'get_work') {
    const id = Number(args.work_id)
    if (numbered(errors.get_work, id)) return { error: `simulated archive fault: no Work ${id}` }
    const work = (fixture.works || {})[String(id)]
    return work ? { result: work } : { error: `Scholar Archive has no Work ${id}` }
  }
  if (inner === 'list_works') {
    if (errors.list_works) return { error: 'simulated unavailable search' }
    const key = String(args.query || '').trim().replace(/\s+/g, ' ').toLowerCase()
    const items = ((fixture.search || {})[key] || []).map(id => (fixture.works || {})[String(id)]).filter(Boolean)
    return { result: { total: items.length, items, offset: 0, limit: Number(args.limit || 25) } }
  }
  if (inner === 'list_work_files') {
    const id = Number(args.work_id)
    if (numbered(errors.work_files, id)) return { error: `simulated unavailable file list of Work ${id}` }
    const items = ((fixture.files || {})[String(id)] || []).map(file => ({ work_id: id, kind: 'markdown', availability: 'present', ...file }))
    return { result: { total: items.length, items, next_after_file_id: null } }
  }
  return { error: `the Scholar Archive seam does not answer \`${inner}\`` }
}
// One `scholar_archive_file` answer: the document as the bridge's cached copy.
const scholarFile = (fixture, id) => {
  if (numbered((fixture.errors || {}).file, id)) return { error: `simulated unavailable document ${id}` }
  const document = (fixture.documents || {})[String(id)]
  if (!document) return { error: `Scholar Archive has no document ${id}` }
  const local = path.resolve(document.path)
  const bytes = fs.readFileSync(local)
  return {
    result: {
      file_id: Number(id), name: document.name || path.basename(local),
      relative_path: document.relative_path || document.name || path.basename(local),
      path: local, bytes: bytes.length, sha256: digest(local),
    },
  }
}
const hangBatch = async filename => {
  const target = path.resolve(filename)
  const relative = path.relative(os.tmpdir(), target)
  if (relative.startsWith('..') || !relative.split(path.sep)[0].startsWith('mcp-lifecycle-test-')) throw new Error('hung batch must stay in the isolated lifecycle fixture')
  fs.writeFileSync(target, JSON.stringify({ proxy: process.pid, writer: real.pid }))
  await new Promise(() => {})
}

async function handle(line) {
  let message
  try { message = JSON.parse(line) } catch { return }
  if (message.id === undefined) { real.stdin.write(line + '\n'); return }
  const tool = message.method === 'tools/call' ? message.params.name : ''
  const at = (message.params && message.params.arguments) || {}
  if (tool && env.ZT_TRACE_TOOLS) {
    const trace = { tool, process_id: process.pid, address_count: at.addresses?.length, item_count: at.items?.length }
    if (tool === 'scholar_archive_read') {
      trace.scholar_tool = at.tool
      trace.work_id = at.arguments?.work_id
      trace.query = at.arguments?.query
    }
    if (tool === 'scholar_archive_file') trace.file_id = at.file_id
    fs.appendFileSync(env.ZT_TRACE_TOOLS, JSON.stringify(trace) + '\n')
  }
  const fixture = scholar()
  if (fixture && tool === 'scholar_archive_read') {
    const answer = scholarRead(fixture, String(at.tool || ''), at.arguments || {})
    if (answer.error) return send(fault(message.id, answer.error))
    const value = { tool: at.tool, result: answer.result }
    return send({ jsonrpc: '2.0', id: message.id, result: { content: [{ type: 'text', text: JSON.stringify(value) }], structuredContent: value } })
  }
  if (fixture && tool === 'scholar_archive_file') {
    const answer = scholarFile(fixture, at.file_id)
    if (answer.error) return send(fault(message.id, answer.error))
    return send({ jsonrpc: '2.0', id: message.id, result: { content: [{ type: 'text', text: JSON.stringify(answer.result) }], structuredContent: answer.result } })
  }
  if (tool && env.ZT_SLOW_TOOL && tool === env.ZT_SLOW_TOOL && Number(env.ZT_SLOW_MS) > 0) {
    if (env.ZT_SLOW_FLAG) fs.writeFileSync(env.ZT_SLOW_FLAG, `${tool} ${process.pid}\n`)
    await new Promise(resolve => setTimeout(resolve, Number(env.ZT_SLOW_MS)))
  }
  if (tool === 'get_cards' && env.ZT_BULK_LIMIT && at.addresses.length > Number(env.ZT_BULK_LIMIT)) {
    const data = { error: 'simulated reply limit', code: 'read_limit', limit_kind: 'reply_bytes', limit: 1024, actual: 2048, complete: false }
    return send({ jsonrpc: '2.0', id: message.id, result: { content: [{ type: 'text', text: JSON.stringify(data) }], structuredContent: data, isError: true } })
  }
  if (tool === 'get_tree' && env.ZT_FAIL_TREE && at.tree === env.ZT_FAIL_TREE) return send(fault(message.id, 'simulated unavailable tree'))
  if (tool === 'export_titles' && env.ZT_FAIL_EXPORT_TITLES) return send(fault(message.id, 'simulated unavailable title export'))
  if (tool === 'get_model' && env.ZT_FAIL_MODEL && at.at === env.ZT_FAIL_MODEL) return send(fault(message.id, 'simulated unavailable model'))
  if (tool === 'get_card' && flag(env.ZT_FAIL_FLAG) && at.at === env.ZT_FAIL_CARD) return send(fault(message.id, 'simulated unavailable target after persistence'))
  if (tool === 'edit_card' && env.ZT_RACE_TARGET && at.at === env.ZT_RACE_TARGET && env.ZT_RACE_FLAG && !flag(env.ZT_RACE_FLAG)) {
    fs.writeFileSync(env.ZT_RACE_FLAG, '')
    const read = await call('get_card', { at: at.at })
    const card = JSON.parse(read.result.content.map(c => c.text).join('\n'))
    await call('edit_card', { at: at.at, body: card.body.trimEnd() + '\n\n' + env.ZT_RACE_LINE })
  }
  if (tool === 'apply_batch' && !at.preview && env.ZT_FAIL_BATCH_BEFORE_WRITE) return send(fault(message.id, 'injected failure before the archive write'))
  if (tool === 'apply_batch' && !at.preview && env.ZT_HANG_BATCH_BEFORE_WRITE_FLAG && !flag(env.ZT_HANG_BATCH_BEFORE_WRITE_FLAG)) await hangBatch(env.ZT_HANG_BATCH_BEFORE_WRITE_FLAG)
  if (tool === 'apply_batch' && !at.preview && env.ZT_RESERVE_BATCH_PARENT && env.ZT_RESERVE_BATCH_FLAG && !flag(env.ZT_RESERVE_BATCH_FLAG)) {
    const reserved = await call('create_successor', { at: env.ZT_RESERVE_BATCH_PARENT, kind: 'side', title: '并行写入占据这个位置', body: '另一位写作者独立创建了这张卡片。' })
    if (failed(reserved)) return send(fault(message.id, 'concurrent reservation failed'))
    fs.writeFileSync(env.ZT_RESERVE_BATCH_FLAG, JSON.stringify(reserved.result.structuredContent))
  }
  const reply = await ask(message)
  if (!at.preview && (tool === 'get_cards' || tool === 'export_titles')) drift(reply)
  if (tool === 'apply_batch' && !at.preview && env.ZT_RACE_AFTER_CREATE && env.ZT_RACE_TARGET && env.ZT_RACE_FLAG && !flag(env.ZT_RACE_FLAG) && !failed(reply)
      && (at.items || []).some(item => item.tool === 'create_successor')) {
    fs.writeFileSync(env.ZT_RACE_FLAG, '')
    const read = await call('get_card', { at: env.ZT_RACE_TARGET })
    const card = JSON.parse(read.result.content.map(c => c.text).join('\n'))
    await call('edit_card', { at: env.ZT_RACE_TARGET, body: card.body.trimEnd() + '\n\n' + env.ZT_RACE_LINE })
  }
  if (tool === 'apply_batch' && !at.preview && env.ZT_HANG_BATCH_REPLY_FLAG && !flag(env.ZT_HANG_BATCH_REPLY_FLAG) && !failed(reply)) await hangBatch(env.ZT_HANG_BATCH_REPLY_FLAG)
  if (tool === 'get_cards' && !at.addresses.length && env.ZT_IDENTITY_FAULT && !failed(reply)) {
    const value = reply.result.structuredContent
    identityViews++
    if (env.ZT_IDENTITY_FAULT === 'missing') delete value.archive_identity
    if (env.ZT_IDENTITY_FAULT === 'incomplete') value.complete = false
    if (env.ZT_IDENTITY_FAULT === 'after_read' && identityViews >= 2) value.archive_identity.id = '00000000000000000000000000000000'
    reply.result.content = [{ type: 'text', text: JSON.stringify(value) }]
  }
  if (tool === 'apply_batch' && at.preview && env.ZT_DRAFT_FAULT && !failed(reply)) {
    const value = reply.result.structuredContent
    if (env.ZT_DRAFT_FAULT === 'paragraphs') value.paragraphs = []
    if (env.ZT_DRAFT_FAULT === 'cards') value.cards = []
    if (env.ZT_DRAFT_FAULT === 'complete') value.complete = false
    reply.result.content = [{ type: 'text', text: JSON.stringify(value) }]
  }
  if (tool === 'apply_batch' && at.preview && env.ZT_REPLACE_PREVIEW_INPUT && env.ZT_PREVIEW_REPLACEMENT_FILE && !failed(reply)) {
    const target = path.toNamespacedPath(path.resolve(env.ZT_REPLACE_PREVIEW_INPUT))
    const source = path.toNamespacedPath(path.resolve(env.ZT_PREVIEW_REPLACEMENT_FILE))
    const temp = path.toNamespacedPath(path.resolve(os.tmpdir())) + path.sep
    if (!target.startsWith(temp) || !source.startsWith(temp)) throw new Error('input replacement must stay in isolated temp fixtures')
    fs.copyFileSync(source, target)
  }
  if (tool === 'apply_batch' && !at.preview && env.ZT_DROP_BATCH_REPLY_FLAG && !flag(env.ZT_DROP_BATCH_REPLY_FLAG) && !failed(reply)) {
    fs.writeFileSync(env.ZT_DROP_BATCH_REPLY_FLAG, 'committed; reply not delivered\n')
    real.stdin.end()
    process.exit(86)
  }
  if (tool === 'get_cards' && env.ZT_BULK_MISSING && at.addresses.includes(env.ZT_BULK_MISSING) && !failed(reply)) {
    const value = reply.result.structuredContent
    value.cards = value.cards.filter(card => card.address !== env.ZT_BULK_MISSING)
    value.missing = [env.ZT_BULK_MISSING]
    value.complete = false
    reply.result.content = [{ type: 'text', text: JSON.stringify(value) }]
  }
  if (tool === 'get_cards' && at.addresses.length && env.ZT_RACE_AFTER_READ && env.ZT_RACE_TARGET && env.ZT_RACE_FLAG && !flag(env.ZT_RACE_FLAG) && !failed(reply)) {
    fs.writeFileSync(env.ZT_RACE_FLAG, '')
    const read = await call('get_card', { at: env.ZT_RACE_TARGET })
    const card = JSON.parse(read.result.content.map(c => c.text).join('\n'))
    await call('edit_card', { at: env.ZT_RACE_TARGET, body: card.body.trimEnd() + '\n\n' + env.ZT_RACE_LINE })
  }
  if (tool === 'get_cards' && at.addresses.length && env.ZT_RACE_AFTER_BULK_REPLY && env.ZT_RACE_TARGET && env.ZT_RACE_FLAG && !flag(env.ZT_RACE_FLAG) && !failed(reply)) {
    fs.writeFileSync(env.ZT_RACE_FLAG, '')
    const read = await call('get_card', { at: env.ZT_RACE_TARGET })
    const card = JSON.parse(read.result.content.map(c => c.text).join('\n'))
    await call('edit_card', { at: env.ZT_RACE_TARGET, body: card.body.trimEnd() + '\n\n' + env.ZT_RACE_LINE })
  }
  if (tool === 'edit_card' && env.ZT_FAIL_AFTER_EDIT && at.at === env.ZT_FAIL_AFTER_EDIT && env.ZT_FAIL_FLAG && !failed(reply)) fs.writeFileSync(env.ZT_FAIL_FLAG, '')
  send(reply)
}

let chain = Promise.resolve()
const input = readline.createInterface({ input: process.stdin })
input.on('line', line => { chain = chain.then(() => handle(line)) })
input.on('close', () => { chain.then(() => real.stdin.end()) })
