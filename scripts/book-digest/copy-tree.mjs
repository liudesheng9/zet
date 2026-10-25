#!/usr/bin/env node
// copy-tree.mjs: copy one agent Literature tree from the live ZT box into a test box.
//
//   node copy-tree.mjs --tree <citation-key> --to-config <ZT_CONFIG_DIR of the test box>
//        [--zt <path to zt.exe used for the test box>] [--from-zt <zt.exe for the live box>]
//        [--from-root <live archive_root, default D:\zt>] [--out <folder for dumps and report>] [--dump-only] [--verify-only]
//
// The live box is read through a `zt mcp` stdio process started WITHOUT ZT_CONFIG_DIR, and
// only read tools are called on it (get_tree, get_card). The test box is written through a
// `zt mcp` process started with ZT_CONFIG_DIR=<to-config>. Cards are created parent before
// child; each allocated address must equal the source address, or the script stops.
// Artifact files the Cards reference (artifacts/...) are copied as files from the live
// archive_root to the test archive_root; the live sqlite is never touched.

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';

const READ_TOOLS = new Set(['get_tree', 'get_card', 'get_cards', 'list_trees', 'list_workspaces', 'list_broken_links', 'list_links', 'get_model']);

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) throw new Error(`unexpected argument ${a}`);
    const key = a.slice(2).toLowerCase();
    if (['dump-only', 'verify-only'].includes(key)) { out[key] = true; continue; }
    out[key] = argv[++i];
    if (out[key] === undefined) throw new Error(`missing value for ${a}`);
  }
  return out;
}

class McpClient {
  constructor(exe, env, label, readOnly) {
    this.label = label;
    this.readOnly = readOnly;
    this.nextId = 1;
    this.pending = new Map();
    this.buf = '';
    this.proc = spawn(exe, ['mcp'], { env, stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
    this.stderr = '';
    this.proc.stdout.setEncoding('utf8');
    this.proc.stdout.on('data', (d) => this.onData(d));
    this.proc.stderr.setEncoding('utf8');
    this.proc.stderr.on('data', (d) => { this.stderr += d; });
    this.proc.on('exit', (code) => {
      for (const [, p] of this.pending) p.reject(new Error(`${label} zt mcp exited (${code}): ${this.stderr.slice(-2000)}`));
      this.pending.clear();
    });
  }
  onData(d) {
    this.buf += d;
    let nl;
    while ((nl = this.buf.indexOf('\n')) >= 0) {
      const line = this.buf.slice(0, nl).trim();
      this.buf = this.buf.slice(nl + 1);
      if (!line) continue;
      let msg;
      try { msg = JSON.parse(line); } catch { continue; }
      if (msg.id !== undefined && this.pending.has(msg.id)) {
        const p = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) p.reject(new Error(`${this.label}: ${JSON.stringify(msg.error)}`));
        else p.resolve(msg.result);
      }
    }
  }
  request(method, params) {
    const id = this.nextId++;
    const msg = { jsonrpc: '2.0', id, method, params };
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.proc.stdin.write(JSON.stringify(msg) + '\n');
    });
  }
  notify(method, params) {
    this.proc.stdin.write(JSON.stringify({ jsonrpc: '2.0', method, params }) + '\n');
  }
  async init() {
    await this.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'copy-tree', version: '1.0.0' } });
    this.notify('notifications/initialized', {});
  }
  async call(name, args) {
    if (this.readOnly && !READ_TOOLS.has(name)) throw new Error(`${this.label} is read-only; refused tool ${name}`);
    const result = await this.request('tools/call', { name, arguments: args });
    const text = (result.content || []).filter((c) => c.type === 'text').map((c) => c.text).join('');
    let value = result.structuredContent;
    if (value === undefined) { try { value = JSON.parse(text); } catch { value = text; } }
    if (result.isError) {
      const err = new Error(`${this.label} ${name} failed: ${typeof value === 'string' ? value : JSON.stringify(value)}`);
      err.value = value;
      throw err;
    }
    return value;
  }
  close() { try { this.proc.stdin.end(); } catch {} setTimeout(() => { try { this.proc.kill(); } catch {} }, 2000).unref(); }
}

function liveEnv() {
  const env = { ...process.env };
  delete env.ZT_CONFIG_DIR;
  delete env.ZT_GUI_PORT;
  return env;
}
function testEnv(configDir) {
  return { ...process.env, ZT_CONFIG_DIR: configDir };
}

function readConfigRoot(configDir) {
  const text = fs.readFileSync(path.join(configDir, 'config.toml'), 'utf8');
  // zt writes `archive_root=<path>` lines (no quotes); quoted forms are accepted too.
  const m = text.match(/^\s*archive_root\s*=\s*(?:'([^']*)'|"((?:[^"\\]|\\.)*)"|(.+?))\s*$/m);
  if (!m) throw new Error(`no archive_root in ${configDir}\\config.toml`);
  if (m[1] !== undefined) return m[1];
  if (m[2] !== undefined) return JSON.parse(`"${m[2]}"`);
  return m[3];
}

// --- Card helpers ---------------------------------------------------------------------------

const REVERSE_HEADINGS = [/\n#{1,6}\s*Reverse links\s*\n[\s\S]*$/i, /\n#{1,6}\s*反向链接\s*\n[\s\S]*$/];

function authoredBody(card) {
  let body = card.body ?? '';
  for (const re of REVERSE_HEADINGS) body = body.replace(re, '');
  return body.replace(/\s+$/, '');
}

// The Card's outgoing Links as get_tree reports them (addresses may contain `|`).
function linksOf(card) {
  return Array.isArray(card.links) ? card.links : [];
}

// The artifact digests a Card shows: `![...](artifact:<sha256>)`.
function artifactRefs(text) {
  const out = new Set();
  const re = /artifact:([0-9a-f]{64})/g;
  let m;
  while ((m = re.exec(text || ''))) out.add(m[1]);
  return [...out];
}

function treeCards(tree) {
  return tree.cards || tree.nodes || tree.tree || tree;
}

// --- main -----------------------------------------------------------------------------------

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const treeId = args.tree;
  const toConfig = args['to-config'];
  if (!treeId || !toConfig) throw new Error('usage: node copy-tree.mjs --tree <key> --to-config <dir> [--zt <exe>] [--from-zt <exe>] [--out <dir>]');
  const zt = args.zt || path.join(path.dirname(toConfig), 'bin', 'zt.exe');
  const fromZt = args['from-zt'] || path.join(os.homedir(), '.cargo', 'bin', 'zt.exe');
  const outDir = args.out || path.join(path.dirname(toConfig), 'copy-runs', treeId);
  fs.mkdirSync(outDir, { recursive: true });
  const log = (...a) => { const s = a.join(' '); console.log(s); fs.appendFileSync(path.join(outDir, 'copy.log'), s + '\n'); };

  const live = new McpClient(fromZt, liveEnv(), 'live', true);
  await live.init();
  const src = await live.call('get_tree', { tree: treeId, bodies: true });
  fs.writeFileSync(path.join(outDir, 'live-tree.json'), JSON.stringify(src, null, 2));
  const cards = treeCards(src);
  log(`live tree ${treeId}: ${cards.length} cards`);

  if (args['dump-only']) { live.close(); return; }

  const test = new McpClient(zt, testEnv(toConfig), 'test', false);
  await test.init();

  if (!args['verify-only']) {
    await copyArtifacts(args['from-root'] || 'D:\\zt', cards, toConfig, log);
    await copyCards(test, cards, src, log);
  }
  const dst = await test.call('get_tree', { tree: treeId, bodies: true });
  fs.writeFileSync(path.join(outDir, 'test-tree.json'), JSON.stringify(dst, null, 2));
  const report = compare(cards, treeCards(dst), treeId);
  report.broken_outside_links = outsideLinks(cards, treeId);
  fs.writeFileSync(path.join(outDir, 'report.json'), JSON.stringify(report, null, 2));
  log(`compared ${report.live_count} live / ${report.test_count} test cards; ${report.differences.length} differences; ${report.outside_inbound.length} inbound Links from other trees (not copied); ${report.broken_outside_links.length} outside links`);
  live.close();
  test.close();
}

// Artifact files are plain content-addressed files (<sha>.<ext> and <sha>.json); copying them
// reads the live archive's artifacts folder only. The live sqlite is never opened.
async function copyArtifacts(liveRoot, cards, toConfig, log) {
  const refs = new Set();
  for (const c of cards) for (const r of artifactRefs(`${c.body ?? ''}\n${c.text ?? ''}`)) refs.add(r);
  if (!refs.size) { log('no artifact references'); return; }
  const fromDir = path.join(liveRoot, 'artifacts');
  const toDir = path.join(readConfigRoot(toConfig), 'artifacts');
  fs.mkdirSync(toDir, { recursive: true });
  const names = fs.readdirSync(fromDir);
  let copied = 0;
  for (const sha of refs) {
    const files = names.filter((n) => n.startsWith(`${sha}.`));
    if (!files.some((n) => !n.endsWith('.json'))) throw new Error(`artifact ${sha} has no image file in ${fromDir}`);
    for (const n of files) { fs.copyFileSync(path.join(fromDir, n), path.join(toDir, n)); copied++; }
  }
  log(`artifacts: ${refs.size} referenced, ${copied} files copied to ${toDir}`);
}

function createArgs(card, parentAddr) {
  const a = { at: parentAddr, title: card.title, kind: card.edge === 'side' ? 'side' : 'direct' };
  const body = authoredBody(card);
  if (body) a.body = body;
  if (card.type && card.type !== 'idea') a.type = card.type;
  if (card.facet) a.facet = card.facet;
  if (card.emphasis) a.emphasis = card.emphasis;
  if (card.level) a.level = card.level;
  if (card.form) a.form = card.form;
  return a;
}

function allocated(result) {
  return result?.address || result?.card?.address || result?.location || result?.created?.address;
}

// zt refuses a Link to a Card that does not exist yet. Pass 1 writes each body with the Links
// to not-yet-created Cards turned into plain text; pass 2 writes the exact body once every
// Card exists.
function withoutPendingLinks(body, done) {
  return body.replace(/\[\[([^\]]+)\]\]/g, (whole, target) => (done.has(target.trim()) ? whole : target));
}

async function copyCards(test, cards, src, log) {
  const root = cards.find((c) => c.edge === 'root');
  if (!root) throw new Error('no root card');
  const done = new Set();
  const bib = root.bibtex || src.bibtex;
  const stripped = new Set();
  const rootBody = withoutPendingLinks(authoredBody(root), new Set());
  if (rootBody !== authoredBody(root)) stripped.add(root.address);
  const r = await test.call('create_literature', { bibtex: bib, workspace: 'agent', ...(rootBody ? { body: rootBody } : {}) });
  const rootAddr = allocated(r) ?? r?.citation_key;
  if (rootAddr !== root.address) throw new Error(`root allocated ${JSON.stringify(r)} but source is ${root.address}`);
  done.add(root.address);
  log(`created ${root.address}`);
  const queue = cards.filter((c) => c !== root);
  let progress = true;
  while (queue.length && progress) {
    progress = false;
    for (let i = 0; i < queue.length; i++) {
      const c = queue[i];
      if (!done.has(c.parent)) continue;
      const a = createArgs(c, c.parent);
      if (a.body) {
        a.body = withoutPendingLinks(a.body, done);
        if (a.body !== authoredBody(c)) stripped.add(c.address);
      }
      const res = await test.call('create_successor', a);
      const got = allocated(res);
      if (got !== c.address) throw new Error(`allocated ${got} for source ${c.address} (parent ${c.parent}): ${JSON.stringify(res).slice(0, 500)}`);
      done.add(c.address);
      queue.splice(i, 1); i--;
      progress = true;
      log(`created ${c.address}`);
    }
  }
  if (queue.length) throw new Error(`cards without a created parent: ${queue.map((c) => c.address).join(', ')}`);
  // Pass 2: the exact bodies, now that every Link target exists.
  for (const c of cards) {
    if (!stripped.has(c.address)) continue;
    await test.call('edit_card', { at: c.address, body: authoredBody(c) });
  }
  log(`pass 2: ${stripped.size} bodies rewritten with their full Links`);
}

function linkSet(card) {
  return { links: linksOf(card), kinds: card.kinds ?? {} };
}

const REVERSE_MARK = '<!-- zt:reverse-links -->';
const inTree = (addr, treeId) => addr === treeId || addr.startsWith(`${treeId}/`);

// The generated reverse-links section lists every Card that links this one. A copy of one tree
// cannot hold the Links from other trees (another book's Bridge, a Topic Card), so the live text
// is compared with those lines dropped (and the section dropped when none is left), and its inbound
// count lowered by as many; the dropped Links are reported in outside_inbound, not as differences.
function withoutOutsideInbound(card, treeId) {
  const text = card.text ?? '';
  const at = text.indexOf(REVERSE_MARK);
  if (at < 0 || !treeId) return { text, inbound: card.inbound ?? null, outside: [] };
  const lines = text.slice(at).split('\n');
  const outside = [];
  const kept = lines.filter((l) => {
    const m = l.match(/^- \[\[([^\]]+)\]\]/);
    if (m && !inTree(m[1].trim(), treeId)) { outside.push(m[1].trim()); return false; }
    return true;
  });
  const anyLeft = kept.some((l) => /^- \[\[/.test(l));
  const head = text.slice(0, at);
  // With no line left the copy has no section at all; its text then ends where the body ends.
  const out = anyLeft ? head + kept.join('\n') : head.trimEnd();
  return { text: out, inbound: card.inbound == null ? null : card.inbound - outside.length, outside, trimmed: !anyLeft };
}

export function compare(liveCards, testCards, treeId) {
  const byAddr = new Map(testCards.map((c) => [c.address, c]));
  const differences = [];
  const outsideInbound = [];
  const fields = ['type', 'edge', 'parent', 'facet', 'emphasis', 'level', 'form', 'title', 'bibtex'];
  for (const lc of liveCards) {
    const tc = byAddr.get(lc.address);
    if (!tc) { differences.push({ address: lc.address, field: 'missing' }); continue; }
    byAddr.delete(lc.address);
    for (const f of fields) {
      if ((lc[f] ?? null) !== (tc[f] ?? null)) differences.push({ address: lc.address, field: f, live: lc[f] ?? null, test: tc[f] ?? null });
    }
    if (authoredBody(lc) !== authoredBody(tc)) differences.push({ address: lc.address, field: 'body', live: authoredBody(lc), test: authoredBody(tc) });
    const lv = withoutOutsideInbound(lc, treeId);
    for (const from of lv.outside) outsideInbound.push({ to: lc.address, from });
    if (lv.text !== (lv.trimmed ? (tc.text ?? '').trimEnd() : (tc.text ?? ''))) differences.push({ address: lc.address, field: 'text (with generated reverse links)' });
    if ((lv.inbound ?? null) !== (tc.inbound ?? null)) differences.push({ address: lc.address, field: 'inbound', live: lv.inbound, test: tc.inbound });
    const ll = JSON.stringify(linkSet(lc)); const tl = JSON.stringify(linkSet(tc));
    if (ll !== tl) differences.push({ address: lc.address, field: 'links', live: linkSet(lc), test: linkSet(tc) });
  }
  for (const [addr] of byAddr) differences.push({ address: addr, field: 'extra_in_test' });
  return { live_count: liveCards.length, test_count: testCards.length, differences, outside_inbound: outsideInbound };
}

function outsideLinks(cards, treeId) {
  const out = [];
  for (const c of cards) {
    for (const t of linksOf(c)) {
      if (t !== treeId && !t.startsWith(`${treeId}/`)) out.push({ from: c.address, to: t });
    }
  }
  return out;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((e) => { console.error(e.stack || String(e)); process.exit(1); });
}
