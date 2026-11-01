// Time zt tree operations on a box copy of the live archive (never the live database).
// node scripts/tree-ops-bench.mjs --zt <zt.exe> --label <name> [--work <dir>] [--src D:/zt/zt.sqlite3] [--ops a,b,...]
// Results and the box live in --work (default: the system temp folder), never in the repository.
// docs/tree-operations-timings.md records the runs.
import { DatabaseSync } from 'node:sqlite';
import { spawn, spawnSync } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync, copyFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { createInterface } from 'node:readline';
import { tmpdir } from 'node:os';

const args = Object.fromEntries(process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith('--') ? [...acc, [a.slice(2), all[i + 1]]] : acc), []));
const here = args.work ?? join(tmpdir(), 'zt-tree-ops-bench');
mkdirSync(here, { recursive: true });
const label = args.label ?? 'run';
const src = args.src ?? 'D:/zt/zt.sqlite3';
const box = join(here, 'box-' + label);
rmSync(box, { recursive: true, force: true });
mkdirSync(join(box, 'config'), { recursive: true });
mkdirSync(join(box, 'archive'), { recursive: true });
// A private copy of the exe, so the service never locks the build output.
const zt = join(box, 'zt.exe');
copyFileSync(args.zt, zt);
const copy = join(box, 'archive', 'zt.sqlite3').replaceAll('\\', '/');
new DatabaseSync(src, { readOnly: true }).exec(`VACUUM INTO '${copy}'`);
const env = { ...process.env, ZT_CONFIG_DIR: join(box, 'config'), ZT_GUI_PORT: '0' };
const run = (...a) => {
  const r = spawnSync(zt, a, { env, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`zt ${a.join(' ')}: ${r.stderr}${r.stdout}`);
  return r.stdout;
};
run('config', 'set', 'archive_root', join(box, 'archive'));
run('up');

// Candidates, read from the copy.
const db = new DatabaseSync(copy, { readOnly: true });
const has = (at) => !!db.prepare('SELECT 1 FROM cards WHERE location=?').get(at);
const agent = db.prepare("SELECT location FROM cards WHERE workspace='agent' AND location IS NOT NULL AND is_topic=0 ORDER BY row_id").all().map((r) => r.location);
const sides = agent.find((x) => has(x + '|a') && has(x + '|b') && has(x + '|c'));
const sideCount = (x) => db.prepare("SELECT count(*) n FROM cards WHERE location LIKE ? || '|%'").get(x).n;
const nextDirect = (at) => at.replace(/(\d+)$/, (n) => String(Number(n) + 1));
const numbered = (x) => /[/|]\d+$/.test(x);
const tree = (x) => x.split("/")[0];
const elsewhere = (x) => tree(x) !== tree(sides);
const pair = agent.find((x) => elsewhere(x) && numbered(x) && has(nextDirect(x)) && sideCount(x) === 0 && sideCount(nextDirect(x)) === 0);
const leaf = agent.find((x) => elsewhere(x) && tree(x) !== tree(pair) && numbered(x) && sideCount(x) === 0 && !has(nextDirect(x)) && x !== pair && x !== nextDirect(pair));
// Leaves (no successors of any kind) of one more tree, for the batch moves; each goes to `side_of`
// one parent in its own tree, so no Literature Card leaves its tree.
const trees = [...new Set(agent.map(tree))].filter((t) => ![tree(sides), tree(pair), tree(leaf)].includes(t));
const leavesOf = (t) => agent.filter((x) => tree(x) === t && numbered(x) && sideCount(x) === 0 && !has(nextDirect(x)));
const movesTree = trees.sort((a, b) => leavesOf(b).length - leavesOf(a).length)[0];
const movesParent = movesTree + '/1';
const movable = leavesOf(movesTree).filter((x) => x !== movesParent && !x.startsWith(movesParent + '|'));
db.close();

const child = spawn(zt, ['mcp'], { env, stdio: ['pipe', 'pipe', 'pipe'] });
const lines = createInterface({ input: child.stdout });
const waiting = [];
lines.on('line', (l) => waiting.shift()?.(JSON.parse(l)));
let id = 0;
const request = (method, params) => new Promise((res) => { waiting.push(res); child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: ++id, method, params }) + '\n'); });
await request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'bench', version: '1' } });
child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
const call = async (name, a) => {
  const t = process.hrtime.bigint();
  const reply = await request('tools/call', { name, arguments: a });
  const ms = Number(process.hrtime.bigint() - t) / 1e6;
  const r = reply.result;
  if (r.isError) throw new Error(`${name} ${JSON.stringify(a)}: ${JSON.stringify(r.structuredContent ?? r.content)}`);
  return { ms, value: r.structuredContent };
};
const body = '这是一张计时用的卡片。它说明一次树操作要花多少时间，只在副本上写。';
const results = { label, zt: args.zt, cards: agent.length, sides, pair, leaf, timings: {} };
const time = async (key, name, a) => { const { ms, value } = await call(name, a); (results.timings[key] ??= []).push(Math.round(ms)); return value; };
const ops = (args.ops ?? 'read,move,alone,plan,insert,leaf,batch').split(',');

try {
if (ops.includes('read')) for (let i = 0; i < 3; i++) await time('get_card', 'get_card', { at: sides });
if (ops.includes('plan')) for (let i = 0; i < 2; i++) await time('move_card_plan', 'move_card', { at: sides + '|b', before: sides + '|a' });
// A subtree moved before a Side sibling joins that sibling's line, so each move takes a fresh pair.
if (ops.includes('move')) await time('move_card_subtree', 'move_card', { at: sides + '|c', before: sides + '|a', confirm: 'move' });
if (ops.includes('alone')) for (let i = 0; i < 2; i++) {
  await time('move_card_alone', 'move_card', { at: nextDirect(pair), before: pair, alone: true, confirm: 'move' });
}
if (ops.includes('insert')) await time('insert_card', 'insert_card', { before: sides + '|b', title: '计时插入', body, confirm: 'insert' });
if (ops.includes('leaf')) {
  const made = await time('create_successor', 'create_successor', { at: leaf, kind: 'side', title: '计时叶', body });
  const plan = await call('delete_card', { at: made.address ?? made.at ?? leaf + '|a' });
  await time('delete_card', 'delete_card', { at: made.address ?? leaf + '|a', confirm: plan.value.confirm });
}
// Tree-operation batches (S3, #231): one move, then twenty, each previewed then confirmed by its hash.
const moveBatch = async (key, leaves) => {
  const items = leaves.map((at, n) => ({ tool: 'move_card', op_key: `bench/${label}/1/${key}/i${n}`, client_ref: `m${n}`, arguments: { at, side_of: movesParent } }));
  const preview = await time(key + '_preview', 'apply_batch', { items, preview: true });
  await time(key, 'apply_batch', { items, confirm: preview.plan_hash });
};
if (ops.includes('batchmoves') && movable.length >= 21) {
  results.movesTree = movesTree;
  await moveBatch('apply_batch_1_move', movable.slice(0, 1));
  await moveBatch('apply_batch_20_moves', movable.slice(1, 21));
}
if (ops.includes('batch')) {
  const items = Array.from({ length: 20 }, (_, n) => ({ tool: 'create_successor', op_key: `bench/${label}/1/place/i${n}`, client_ref: `p${n}`, arguments: { at: sides, kind: 'side', before: sides + '|a', title: `计时行${n}`, body } }));
  await time('apply_batch_20_side_placements', 'apply_batch', { items });
}
} catch (error) {
  results.error = String(error.message ?? error).slice(0, 2000);
} finally {
  child.stdin.end();
  child.kill();
  run('down');
}
writeFileSync(join(here, `result-${label}.json`), JSON.stringify(results, null, 2));
console.log(JSON.stringify(results));
