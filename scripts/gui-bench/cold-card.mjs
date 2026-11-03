#!/usr/bin/env node
// cold-card.mjs (#256): how long the GUI's first Term-linked Card read takes after another client
// writes, and how long a ping waits beside it. Builds its own test box, so it never touches the
// live box (it refuses port 4717).
//
//   node scripts/gui-bench/cold-card.mjs --zt <zt.exe under test> --box <new empty folder>
//        [--port 48778] [--heads 748] [--rounds 3] [--bound-card 500] [--bound-ping 100]
//
// The box holds one Literature tree `bk`: a Term list `bk/1` whose table has --heads rows, as many
// Term heads, and a Card `bk/2` that uses a Term. Its rows are written straight into the box's
// SQLite file (node:sqlite) after `zt up` made the schema. Each round edits `bk/3` through the
// box's own `zt mcp` (another client), then reads `bk/2` and pings at once. Exits 1 above a bound.
// `zt down` runs at the end, however the run ends; the box folder is left for the caller to remove.

import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';

const args = {};
for (let i = 2; i < process.argv.length; i += 2) args[process.argv[i].replace(/^--/, '')] = process.argv[i + 1];
const zt = args.zt;
const box = args.box && path.resolve(args.box);
const port = Number(args.port || 48778);
const heads = Number(args.heads || 748);
const rounds = Number(args.rounds || 3);
const boundCard = Number(args['bound-card'] || 500);
const boundPing = Number(args['bound-ping'] || 100);
if (!zt || !box) throw new Error('--zt and --box are required');
if (port === 4717) throw new Error('port 4717 is the live box; use a test port');
if (fs.existsSync(box) && fs.readdirSync(box).length) throw new Error(`${box} is not empty`);

const config = path.join(box, 'config');
const archive = path.join(box, 'archive');
fs.mkdirSync(config, { recursive: true });
fs.mkdirSync(archive, { recursive: true });
fs.writeFileSync(path.join(config, 'config.toml'), `archive_root=${archive}\n`);
const env = { ...process.env, ZT_CONFIG_DIR: config, ZT_GUI_PORT: String(port) };
const run = (...argv) => spawnSync(zt, argv, { env, encoding: 'utf8' });

function label(n) {
  let s = '';
  while (n > 0) { n -= 1; s = String.fromCharCode(97 + (n % 26)) + s; n = Math.floor(n / 26); }
  return s;
}

function fill() {
  const db = new DatabaseSync(path.join(archive, 'zt.sqlite3'));
  const insert = db.prepare(`INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, term_name)
                             VALUES(?, ?, 0, ?, ?, ?, 'agent', ?, ?)`);
  db.exec('BEGIN');
  insert.run(null, 'bk', 1, '@book{bk, title={B}}', '# 书\n\n索引。\n', 'idea', null);
  let table = '# 术语表\n\n术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n';
  for (let n = 1; n <= heads; n++) table += `| 术语${n} | — | 意思${n} | §${n} |\n`;
  insert.run('bk/1', null, 0, null, table, 'term', null);
  for (let n = 1; n <= heads; n++) insert.run(`bk/1|${label(n)}`, null, 0, null, `# 术语${n}\n\n术语${n}的定义。\n`, 'term', `术语${n}`);
  insert.run('bk/2', null, 0, null, '# 用到术语的卡\n\n这里用到[[术语5@bk/1|e]]。\n', 'idea', null);
  insert.run('bk/3', null, 0, null, '# 别处的卡\n\n第0版。\n', 'idea', null);
  db.exec('COMMIT');
  db.close();
}

class Mcp {
  constructor() {
    this.proc = spawn(zt, ['mcp'], { env, stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
    this.next = 1; this.pending = new Map(); this.buf = '';
    this.proc.stdout.setEncoding('utf8');
    this.proc.stdout.on('data', (d) => {
      this.buf += d;
      let nl;
      while ((nl = this.buf.indexOf('\n')) >= 0) {
        const line = this.buf.slice(0, nl).trim(); this.buf = this.buf.slice(nl + 1);
        if (!line) continue;
        const msg = JSON.parse(line);
        const p = this.pending.get(msg.id);
        if (p) { this.pending.delete(msg.id); msg.error ? p.reject(new Error(JSON.stringify(msg.error))) : p.resolve(msg.result); }
      }
    });
  }
  request(method, params) {
    const id = this.next++;
    return new Promise((resolve, reject) => { this.pending.set(id, { resolve, reject }); this.proc.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n'); });
  }
  async init() {
    await this.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'cold-card', version: '1.0.0' } });
    this.proc.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized', params: {} }) + '\n');
  }
  async call(name, a) {
    const result = await this.request('tools/call', { name, arguments: a });
    if (result.isError) throw new Error(`${name}: ${JSON.stringify(result.content)}`);
    return result;
  }
  close() { this.proc.stdin.end(); }
}

const base = `http://127.0.0.1:${port}`;
async function timed(url, token) {
  const start = performance.now();
  const response = await fetch(base + url, { headers: { 'X-ZT-Token': token } });
  const body = await response.text();
  return { ms: Math.round(performance.now() - start), status: response.status, body };
}

let exit = 0;
const up = run('up');
if (up.status !== 0) throw new Error(`zt up failed: ${up.stderr || up.stdout}`);
try {
  fill();
  const page = await (await fetch(base + '/')).text();
  const token = page.match(/name="zt-token" content="([0-9a-f]+)"/)[1];
  const warm = await timed('/api/card?at=bk%2F2', token);
  if (!warm.body.includes('data-meaning=\\"意思5\\"')) throw new Error(`the Term link has no meaning: ${warm.body.slice(0, 300)}`);
  const mcp = new Mcp();
  await mcp.init();
  const results = [];
  for (let round = 1; round <= rounds; round++) {
    await mcp.call('edit_card', { at: 'bk/3', text: `# 别处的卡\n\n第${round}版。\n` });
    const card = timed('/api/card?at=bk%2F2', token);
    await new Promise((r) => setTimeout(r, 20));
    const ping = await timed('/api/ping', token);
    results.push({ round, card: (await card).ms, ping: ping.ms });
  }
  mcp.close();
  const worst = { card: Math.max(...results.map((r) => r.card)), ping: Math.max(...results.map((r) => r.ping)) };
  const ok = worst.card <= boundCard && worst.ping <= boundPing;
  console.log(JSON.stringify({ zt, heads, warm: warm.ms, rounds: results, worst, bounds: { card: boundCard, ping: boundPing }, ok }));
  if (!ok) exit = 1;
} finally {
  run('down');
}
process.exit(exit);
