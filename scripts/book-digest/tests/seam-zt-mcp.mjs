// A test seam for the zt-digest tools: a stand-in for `zt mcp` that forwards every message to the
// real `zt mcp` (zt on PATH) and injects one archive event. A test points zt-digest at it with
// ZT_CHECK_ZT=<this folder>\seam-zt-mcp.cmd (zt-digest starts `<ZT_CHECK_ZT> mcp`), so a tool that
// reads and writes in-process meets the same faults and races as the scripts met through the
// helper stand-ins faulting-zt-mcp.ps1 and racing-zt-mcp.ps1, with the same variables:
//
//   ZT_RACE_TARGET, ZT_RACE_FLAG, ZT_RACE_LINE   once (no flag file yet), before the first edit_card
//       of the Card ZT_RACE_TARGET, append ZT_RACE_LINE to that Card, as a parallel digest stage would
//       between a compiler's read and its write; then create the flag file.
//   ZT_FAIL_TREE                                 get_tree of this tree fails.
//   ZT_FAIL_AFTER_EDIT, ZT_FAIL_CARD, ZT_FAIL_FLAG   after an edit_card of ZT_FAIL_AFTER_EDIT succeeds,
//       create the flag file; while it exists, get_card of ZT_FAIL_CARD fails.
import { spawn } from 'node:child_process'
import fs from 'node:fs'
import readline from 'node:readline'

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
const ask = message => new Promise(resolve => { waiting.set(message.id, resolve); real.stdin.write(JSON.stringify(message) + '\n') })
const call = (name, args) => ask({ jsonrpc: '2.0', id: own++, method: 'tools/call', params: { name, arguments: args } })
const send = message => process.stdout.write(JSON.stringify(message) + '\n')
const failed = reply => !!(reply.error || (reply.result && reply.result.isError))
const fault = (id, text) => ({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text }], isError: true } })
const flag = path => !!path && fs.existsSync(path)

async function handle(line) {
  let message
  try { message = JSON.parse(line) } catch { return }
  if (message.id === undefined) { real.stdin.write(line + '\n'); return }
  const tool = message.method === 'tools/call' ? message.params.name : ''
  const at = (message.params && message.params.arguments) || {}
  if (tool === 'get_tree' && env.ZT_FAIL_TREE && at.tree === env.ZT_FAIL_TREE) return send(fault(message.id, 'simulated unavailable tree'))
  if (tool === 'get_card' && flag(env.ZT_FAIL_FLAG) && at.at === env.ZT_FAIL_CARD) return send(fault(message.id, 'simulated unavailable target after persistence'))
  if (tool === 'edit_card' && env.ZT_RACE_TARGET && at.at === env.ZT_RACE_TARGET && env.ZT_RACE_FLAG && !flag(env.ZT_RACE_FLAG)) {
    fs.writeFileSync(env.ZT_RACE_FLAG, '')
    const read = await call('get_card', { at: at.at })
    const card = JSON.parse(read.result.content.map(c => c.text).join('\n'))
    await call('edit_card', { at: at.at, body: card.body.trimEnd() + '\n\n' + env.ZT_RACE_LINE })
  }
  const reply = await ask(message)
  if (tool === 'edit_card' && env.ZT_FAIL_AFTER_EDIT && at.at === env.ZT_FAIL_AFTER_EDIT && env.ZT_FAIL_FLAG && !failed(reply)) fs.writeFileSync(env.ZT_FAIL_FLAG, '')
  send(reply)
}

let chain = Promise.resolve()
const input = readline.createInterface({ input: process.stdin })
input.on('line', line => { chain = chain.then(() => handle(line)) })
input.on('close', () => { chain.then(() => real.stdin.end()) })
