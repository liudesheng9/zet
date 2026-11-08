// Run the Cited list reading jobs of one or more stagings without a workflow (a backfill of finished books):
// each job is one DeepSeek conversation of one message a part (its `then`), a part whose answer already passes
// `zt-digest cited -Step precheck` is not sent again, a part that still fails gets one more conversation of its
// own, and every conversation is deleted. It talks to the DeepSeek bridge over stdio, so no manager agent relays
// the messages (2026-11-07, docs/cited-list-reading-proposal.md).
//
//   node cited-read-drive.mjs <staging> [<staging> ...] [--parallel 8] [--cards] [--bridge E:/dsh-mcp-bridge/dist/index.js]
//
// --cards runs the Cited Card writer jobs instead (`zt-digest cited-cards -Step jobs`): an entry is one conversation of
// one message a job (its first message, then each item of its `then`, 2026-11-08), and a job is done when
// `cited-cards -Step precheck` of its draft prints faults: 0. A conversation with a failing draft runs once more.
//
// Prints one JSON line per job and a summary line {"ok","jobs","parts","passed","failed"}.
import { spawn, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const argv = process.argv.slice(2);
const opt = (name, fallback) => { const i = argv.indexOf(`--${name}`); return i < 0 ? fallback : argv[i + 1]; };
const stagings = argv.filter((a, i) => !a.startsWith('--') && !['--parallel', '--bridge'].includes(argv[i - 1] || ''));
const PARALLEL = Number(opt('parallel', 8));
const CARDS = argv.includes('--cards');
const BRIDGE = opt('bridge', 'E:/dsh-mcp-bridge/dist/index.js');
const MODEL = 'deepseek-official/deepseek-flash';
const FIRST = '按以上说明读本段的参考文献，写出 JSON 文件并运行自查。最后只回复一行 JSON。';
const NEXT = '继续：按同一份说明读下一段，写出 JSON 文件并运行自查。最后只回复一行 JSON。';

const bridge = spawn('node', [BRIDGE], { stdio: ['pipe', 'pipe', 'inherit'], env: { ...process.env, DSH_BRIDGE_REASONING_EFFORT: 'high' } });
bridge.stdout.setEncoding('utf8');
let buf = '', seq = 1; const pending = new Map();
bridge.stdout.on('data', (d) => {
  buf += d; let i;
  while ((i = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, i); buf = buf.slice(i + 1);
    if (!line.trim()) continue;
    let m; try { m = JSON.parse(line); } catch { continue; }
    if (m.id !== undefined && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
  }
});
const request = (method, params) => new Promise((resolve) => { const id = seq++; pending.set(id, resolve); bridge.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n'); });
const call = async (name, args) => {
  const m = await request('tools/call', { name, arguments: args });
  if (m.error) throw new Error(`${name}: ${JSON.stringify(m.error).slice(0, 300)}`);
  const text = m.result?.content?.[0]?.text ?? '';
  if (m.result?.isError) throw new Error(`${name}: ${text.slice(0, 300)}`);
  try { return JSON.parse(text); } catch { return { text }; }
};
await request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'cited-read-drive', version: '1' } });
bridge.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized', params: {} }) + '\n');

const passes = (staging, answer) => {
  if (!fs.existsSync(answer)) return false;
  try {
    const out = execFileSync('zt-digest', ['cited', '-Step', 'precheck', '-Staging', staging, '-Draft', answer], { encoding: 'utf8' });
    return /faults: 0\s*$/.test(out.trim());
  } catch { return false; }
};

// One conversation over the given parts; the answers end with the last part. Returns its session id.
async function converse(read, parts) {
  let sid = null;
  for (const [k, part] of parts.entries()) {
    const file = path.join(read, `part-${part}.md`);
    let r = k === 0
      ? await call('deepseek_run', { cwd: read, prompt_files: [path.join(read, 'dsh-cited-read-instructions.zh.md'), file], prompt: FIRST, skills: ['asd-ste100-zh'], model: MODEL, reasoning_effort: 'high', wait_seconds: 600 })
      : await call('deepseek_prompt', { session_id: sid, prompt_files: [file], prompt: NEXT, skills: ['asd-ste100-zh'], reasoning_effort: 'high', wait_seconds: 600 });
    sid = r.session_id || sid;
    const started = Date.now();
    while (r.status === 'running' || r.status === 'needs_response') {
      if (r.status === 'needs_response') {
        for (const p of r.pending || []) {
          if (p.kind === 'question' || p.questions) await call('deepseek_respond', { session_id: sid, request_id: p.request_id, decline: '按任务说明自行决定，不需要询问。' }).catch(() => {});
          else await call('deepseek_respond', { session_id: sid, request_id: p.request_id, decision: JSON.stringify(p).includes('_cited-backfill') || JSON.stringify(p).includes(read.replace(/\\/g, '\\\\')) ? 'allow' : 'reject' }).catch(() => {});
        }
      }
      if (Date.now() - started > 40 * 60 * 1000) { await call('deepseek_cancel', { session_id: sid }).catch(() => {}); break; }
      r = await call('deepseek_wait', { session_id: sid, wait_seconds: 600 });
    }
  }
  return sid;
}

if (CARDS) {
  const passesCard = (staging, draft) => {
    if (!fs.existsSync(draft)) return false;
    try { return /faults: 0\s*$/.test(execFileSync('zt-digest', ['cited-cards', '-Step', 'precheck', '-Staging', staging, '-Draft', draft], { encoding: 'utf8' }).trim()); } catch { return false; }
  };
  const all = [];
  for (const staging of stagings) {
    const out = execFileSync('zt-digest', ['cited-cards', '-Step', 'jobs', '-Staging', staging], { encoding: 'utf8', maxBuffer: 64 << 20 });
    const listed = JSON.parse(out.trim().split(String.fromCharCode(10)).pop()).jobs;
    const work = path.join(staging, 'cited-cards');
    const old = await call('deepseek_list', { cwd: work }).catch(() => ({}));
    for (const c of old.conversations || []) { await call('deepseek_cancel', { session_id: c.session_id }).catch(() => {}); await call('deepseek_delete', { session_id: c.session_id }).catch(() => {}); }
    for (const j of listed) {
      const messages = [{ id: j.id, files: j.files, prompt: j.prompt, answer: j.answer }, ...(j.then || [])];
      if (!messages.every((m) => passesCard(staging, m.answer))) all.push({ staging, work, j, messages });
    }
  }
  let n = 0, failed = 0;
  const one = async ({ staging, work, j, messages }) => {
    const sessions = [];
    const settled = () => messages.every((m) => passesCard(staging, m.answer));
    for (let attempt = 0; attempt < 2 && !settled(); attempt++) {
      try {
        let sid = null;
        for (const [k, m] of messages.entries()) {
          let r = k === 0
            ? await call('deepseek_run', { cwd: work, prompt_files: m.files, prompt: m.prompt, skills: j.skills, model: j.model, reasoning_effort: j.effort, wait_seconds: 600 })
            : await call('deepseek_prompt', { session_id: sid, prompt_files: m.files, prompt: m.prompt, skills: j.skills, reasoning_effort: j.effort, wait_seconds: 600 });
          if (k === 0) { sid = r.session_id; sessions.push(sid); }
          const t0 = Date.now();
          while (r.status === 'running' || r.status === 'needs_response') {
            if (r.status === 'needs_response') for (const p of r.pending || []) await call('deepseek_respond', { session_id: sid, request_id: p.request_id, ...(p.questions ? { decline: '按任务说明自行决定，不需要询问。' } : { decision: 'allow' }) }).catch(() => {});
            if (Date.now() - t0 > 40 * 60 * 1000) { await call('deepseek_cancel', { session_id: sid }).catch(() => {}); break; }
            r = await call('deepseek_wait', { session_id: sid, wait_seconds: 600 });
          }
        }
      } catch (e) { console.log(JSON.stringify({ job: j.id, error: String(e.message).slice(0, 200) })); }
    }
    for (const s of sessions) await call('deepseek_delete', { session_id: s }).catch(() => {});
    for (const m of messages) {
      const ok = passesCard(staging, m.answer); if (!ok) failed++;
      console.log(JSON.stringify({ book: path.basename(staging), job: m.id, conversation: j.id, ok }));
    }
  };
  await Promise.all(Array.from({ length: Math.min(PARALLEL, all.length) }, async () => { while (n < all.length) await one(all[n++]); }));
  for (const staging of stagings) await call('deepseek_remove_workspace', { cwd: path.join(staging, 'cited-cards') }).catch(() => {});
  console.log(JSON.stringify({ ok: failed === 0, conversations: all.length, jobs: all.reduce((k, x) => k + x.messages.length, 0), failed }));
  bridge.stdin.end();
  setTimeout(() => process.exit(failed === 0 ? 0 : 1), 500);
} else {
// The jobs: from each staging's plan, the parts of each job that have no passing answer yet.
const jobs = [];
for (const staging of stagings) {
  const work = path.join(staging, 'cited');
  const read = path.join(work, 'read');
  const plan = JSON.parse(fs.readFileSync(path.join(work, 'plan.json'), 'utf8'));
  // Conversations left by an earlier attempt in this workspace are stopped and deleted first.
  const listed = await call('deepseek_list', { cwd: read }).catch(() => ({}));
  for (const c of listed.conversations || []) {
    await call('deepseek_cancel', { session_id: c.session_id || c.id }).catch(() => {});
    await call('deepseek_delete', { session_id: c.session_id || c.id }).catch(() => {});
  }
  const byJob = new Map();
  for (const part of plan.parts) {
    if (passes(staging, path.join(read, 'out', `${part.id}.json`))) continue;
    if (!byJob.has(part.job)) byJob.set(part.job, []);
    byJob.get(part.job).push(part.id);
  }
  for (const [job, parts] of byJob) jobs.push({ staging, read, job, parts });
}

const results = [];
let next = 0;
async function worker() {
  while (next < jobs.length) {
    const j = jobs[next++];
    const sessions = [];
    let error = null;
    try { sessions.push(await converse(j.read, j.parts)); } catch (e) { error = String(e.message).slice(0, 200); }
    // A part that still fails gets one more conversation of its own.
    const failed = j.parts.filter((p) => !passes(j.staging, path.join(j.read, 'out', `${p}.json`)));
    for (const p of failed) { try { sessions.push(await converse(j.read, [p])); } catch (e) { error = String(e.message).slice(0, 200); } }
    for (const s of sessions.filter(Boolean)) await call('deepseek_delete', { session_id: s }).catch(() => {});
    const still = j.parts.filter((p) => !passes(j.staging, path.join(j.read, 'out', `${p}.json`)));
    const line = { book: path.basename(j.staging), job: j.job, parts: j.parts.length, failed: still, error };
    results.push(line);
    console.log(JSON.stringify(line));
  }
}
await Promise.all(Array.from({ length: Math.min(PARALLEL, jobs.length) }, worker));
for (const staging of stagings) await call('deepseek_remove_workspace', { cwd: path.join(staging, 'cited', 'read') }).catch(() => {});
const parts = results.reduce((n, r) => n + r.parts, 0);
const failed = results.reduce((n, r) => n + r.failed.length, 0);
console.log(JSON.stringify({ ok: failed === 0, jobs: results.length, parts, passed: parts - failed, failed }));
bridge.stdin.end();
setTimeout(() => process.exit(failed === 0 ? 0 : 1), 500);
}
