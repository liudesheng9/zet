// After a Cited list is read again and its Cards are edited in row order, a Link from another Card into a Cited
// Card may now name another work. This points each recorded Link back to the Card of its own work.
//
//   node cited-relink.mjs record <out.json> <key> [<key> ...]   record every Link into the books' Cited Cards
//   node cited-relink.mjs apply <out.json> <key> [<key> ...]    re-point the recorded Links of these books
//
// A work is found by the letters and digits of its `文献：` line: the same line first; else the one new Card whose
// line stands inside the old line (the old row had merged several works) and whose surname the linking Card
// names. Anything else is printed as `unresolved` and left alone.
import { spawn } from 'node:child_process';
import fs from 'node:fs';

const [mode, file, ...keys] = process.argv.slice(2);
const p = spawn('zt', ['mcp'], { stdio: ['pipe', 'pipe', 'inherit'] });
p.stdout.setEncoding('utf8');
let buf = '', seq = 1; const pend = new Map();
p.stdout.on('data', (d) => { buf += d; let i; while ((i = buf.indexOf('\n')) >= 0) { const l = buf.slice(0, i); buf = buf.slice(i + 1); if (!l.trim()) continue; const m = JSON.parse(l); pend.get(m.id)?.(m.result); } });
const rq = (method, params) => new Promise((r) => { const k = seq++; pend.set(k, r); p.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: k, method, params }) + '\n'); });
const call = async (name, a) => { const r = await rq('tools/call', { name, arguments: a }); if (r.isError) throw new Error(`${name}: ${r.content[0].text.slice(0, 300)}`); return r.structuredContent ?? JSON.parse(r.content[0].text); };
await rq('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'cited-relink', version: '1' } });
p.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized', params: {} }) + '\n');

const fold = (s) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]/g, '');
const entryOf = (body) => (body.match(/^文献：(.*)$/m) || [])[1] || '';
async function cards(key) {
  const c = await call('get_cited', { at: key });
  const out = [];
  const addrs = c.cards.map((x) => x.address);
  for (let i = 0; i < addrs.length; i += 100) {
    const r = await call('get_cards', { addresses: addrs.slice(i, i + 100) });
    for (const card of r.cards) out.push({ address: card.address, entry: entryOf(card.body), title: card.title, inbound: card.inbound || [] });
  }
  return { list: c.list, cards: out };
}

if (mode === 'record') {
  const rows = [];
  for (const key of keys) {
    const { list, cards: all } = await cards(key);
    for (const card of all) for (const x of card.inbound) if (!String(x.address).startsWith(list)) rows.push({ book: key, to: card.address, from: x.address, entry: card.entry });
  }
  fs.writeFileSync(file, JSON.stringify(rows, null, 1));
  console.log(JSON.stringify({ ok: true, links: rows.length }));
} else if (mode === 'apply') {
  const rows = JSON.parse(fs.readFileSync(file, 'utf8')).filter((r) => keys.includes(r.book));
  const done = [], kept = [], unresolved = [];
  const books = {};
  for (const key of keys) books[key] = await cards(key);
  for (const r of rows) {
    const all = books[r.book].cards;
    const old = fold(r.entry);
    const now = all.find((c) => c.address === r.to);
    if (now && fold(now.entry) === old) { kept.push(r.from); continue; }
    let target = all.find((c) => fold(c.entry) === old);
    if (!target) {
      const source = await call('get_card', { at: r.from });
      const text = fold(`${source.title} ${source.body}`);
      const inside = all.filter((c) => fold(c.entry).length >= 12 && old.includes(fold(c.entry)));
      const named = inside.filter((c) => { const surname = fold((c.entry.split(/[,，.(]/)[0] || '')); return surname.length >= 3 && text.includes(surname); });
      target = named.length === 1 ? named[0] : inside.length === 1 ? inside[0] : null;
    }
    if (!target) { unresolved.push(r); continue; }
    await call('relink', { from: r.to, to: target.address, sources: [r.from] });
    done.push({ from: r.from, was: r.to, now: target.address });
  }
  console.log(JSON.stringify({ ok: unresolved.length === 0, relinked: done.length, kept: kept.length, unresolved }));
}
p.stdin.end();
setTimeout(() => process.exit(0), 500);
