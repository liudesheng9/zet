#!/usr/bin/env node
// term-explain-repair.mjs: rewrite the `## 释义` of Term Cards that list the book instead of teaching the term.
//
//   node term-explain-repair.mjs plan  --key <citation key> --list <Term list address> --out <folder> [--per-job 12] [--only t1,t2]
//   node term-explain-repair.mjs check --out <folder> --job r001
//   node term-explain-repair.mjs apply --out <folder> [--only t1,t2]
//
// Jobs of zt-digest 0.47.0 to 0.47.5 wrote `§小节 说……` lines into `explanation`, and read formulas in foreign words
// set in math italics, so a history book got mathematical Cards with invented models (morony2005iraq).
// A job holds all its material, answers with new text in one JSON file, and checks it with `check`.
// `apply` replaces `## 释义` alone on a concept Card. A false mathematical Card is rebuilt as a concept
// Card from its own sections, and its parts are deleted. Run term-relink.ps1 after `apply`.
// The rule on an explanation is the one in tools/zt-digest/src/term_cards.rs (`source_narration`).

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const NARRATION = /§|第[0-9一二三四五六七八九十百]+[章节]|本书|(作者|书中|同一小节)(说|称|认为|指出|提到|写|记|用|把|讲|谈|论|主张|强调)/;
const MATH_WORDS = /\$|公式|本卡|模型|补：/;
const MIN_LINES = 3;
const MAX_LINES = 6;

function args(argv) {
  const out = { _: argv[0] };
  for (let i = 1; i < argv.length; i += 2) out[argv[i].replace(/^--/, '')] = argv[i + 1];
  return out;
}

class Zt {
  constructor() {
    this.id = 1; this.pending = new Map(); this.buf = '';
    this.proc = spawn('zt', ['mcp'], { stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
    this.proc.stdout.setEncoding('utf8');
    this.proc.stdout.on('data', (d) => {
      this.buf += d;
      for (let nl; (nl = this.buf.indexOf('\n')) >= 0;) {
        const line = this.buf.slice(0, nl).trim(); this.buf = this.buf.slice(nl + 1);
        if (!line) continue;
        const msg = JSON.parse(line); const p = this.pending.get(msg.id);
        if (p) { this.pending.delete(msg.id); msg.error ? p.reject(new Error(JSON.stringify(msg.error))) : p.resolve(msg.result); }
      }
    });
  }
  request(method, params) {
    const id = this.id++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.proc.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
    });
  }
  async init() {
    await this.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'term-explain-repair', version: '1' } });
    this.proc.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized', params: {} }) + '\n');
  }
  async call(name, a) {
    const r = await this.request('tools/call', { name, arguments: a });
    const text = (r.content || []).map((c) => c.text || '').join('');
    let value = r.structuredContent;
    if (value === undefined) { try { value = JSON.parse(text); } catch { value = text; } }
    if (r.isError) throw new Error(`${name}: ${text.slice(0, 600)}`);
    return value;
  }
  close() { this.proc.stdin.end(); setTimeout(() => this.proc.kill(), 1000).unref(); }
}

async function cards(zt, addresses, termLinks) {
  const out = [];
  for (let i = 0; i < addresses.length; i += 100) {
    const r = await zt.call('get_cards', { addresses: addresses.slice(i, i + 100), view: 'text', term_links: termLinks });
    out.push(...r.cards);
  }
  return out;
}

/** The body's `## name` sections, in order, with the head before the first and the source line after the last. */
export function sections(body) {
  const parts = body.split(/^(?=## )/m);
  const head = parts.shift();
  const list = parts.map((part) => {
    const nl = part.indexOf('\n');
    return { name: part.slice(3, nl).trim(), text: part.slice(nl + 1).trim() };
  });
  return { head, list };
}

function section(body, name) { return sections(body).list.find((s) => s.name === name)?.text ?? ''; }
const lines = (text) => text.split('\n').map((l) => l.trim()).filter(Boolean);
const sourceLine = (body) => (body.match(/^来源：.*$/m) || [''])[0];
const withoutSource = (text) => lines(text).filter((l) => !l.startsWith('来源：') && l !== '（续见下卡）');

/** A published explanation that tells the book's account. */
export function narrated(body) {
  const list = lines(section(body, '释义'));
  return list.filter((l) => NARRATION.test(l)).length;
}

async function plan(a) {
  const out = a.out; const perJob = Number(a['per-job'] || 12);
  const only = onlyTerms(a);
  fs.mkdirSync(path.join(out, 'jobs'), { recursive: true });
  fs.mkdirSync(path.join(out, 'answers'), { recursive: true });
  const zt = new Zt(); await zt.init();
  // get_terms carries every Term use and passes the reply limit on a big book, so read the list Card.
  const list = await zt.call('get_card', { at: a.list, term_links: 'label' });
  const avoid = new Map(); const meaning = new Map();
  for (const m of list.body.matchAll(/^\| (.+?) \| (.+?) \| (.+?) \| (.+?) \|$/gm)) {
    avoid.set(m[1], m[2].split('、').filter((w) => w !== '—')); meaning.set(m[1], m[3]);
  }
  const heads = list.sides.filter((h) => !only || only.has(h.title));
  const read = await cards(zt, heads.map((h) => h.address), 'label');
  const partsOf = new Map(); const byAddress = new Map();
  for (const card of read.filter((c) => /^类别：数学概念$/m.test(c.body))) {
    const chain = [];
    for (let next = (await zt.call('get_card', { at: card.address })).direct; next; ) {
      chain.push(next.address);
      const part = await zt.call('get_card', { at: next.address, term_links: 'label' });
      byAddress.set(next.address, part); next = part.direct;
    }
    partsOf.set(card.address, chain);
  }
  zt.close();
  const items = [];
  for (const card of read) {
    const math = /^类别：数学概念$/m.test(card.body);
    const count = narrated(card.body);
    if (!math && !count) continue;
    const parts = (partsOf.get(card.address) || []).map((x) => byAddress.get(x)?.body || '');
    const all = [card.body, ...parts].join('\n\n');
    const item = {
      term: card.term, address: card.address, kind: math ? 'math' : 'concept', parts: partsOf.get(card.address) || [],
      avoid: avoid.get(card.term) || [], meaning: meaning.get(card.term) || '',
      explanation: lines(section(card.body, '释义')), book: withoutSource(section(card.body, '书中的说法')),
    };
    if (math) {
      item.distinctions = withoutSource(section(all, '区别'));
      item.misreadings = withoutSource(section(all, '常见误读'));
      item.role = withoutSource(section(all, '在本书中的作用'));
    }
    items.push(item);
  }
  // Mathematical rows go first, in small jobs of their own: each has three fields to write.
  const math = items.filter((i) => i.kind === 'math'); const concept = items.filter((i) => i.kind === 'concept');
  const jobs = [];
  for (const [list, size] of [[math, 6], [concept, perJob]]) {
    for (let i = 0; i < list.length; i += size) jobs.push(list.slice(i, i + size));
  }
  const index = jobs.map((job, n) => {
    const id = `r${String(n + 1).padStart(3, '0')}`;
    fs.writeFileSync(path.join(out, 'jobs', `${id}.md`), jobText(out, id, job), 'utf8');
    return { id, file: path.join(out, 'jobs', `${id}.md`), answer: path.join(out, 'answers', `${id}.json`), terms: job.map((t) => t.term) };
  });
  fs.writeFileSync(path.join(out, 'items.json'), JSON.stringify(items, null, 1), 'utf8');
  fs.writeFileSync(path.join(out, 'jobs.json'), JSON.stringify(index, null, 1), 'utf8');
  console.log(JSON.stringify({ ok: true, heads: read.length, concept: concept.length, math: math.length, jobs: index.length, bytes: Math.max(...index.map((j) => fs.statSync(j.file).size)) }));
}

function jobText(out, id, job) {
  const math = job.some((t) => t.kind === 'math');
  const sample = {
    term: '牧首',
    explanation: ['牧首是基督教大城市的最高主教，地位在一般主教之上。', '他领导所在城市及其周边的教会，负责任命主教和裁断教义争端。', '他的权力来自教会等级，也来自帝国的支持，所以他常常同时参与政治。', '城市受到围攻时，牧首可以代表居民与进攻者谈判。', '不同的教义派别各立牧首，因此一座城市的基督徒可能不只承认一位牧首。'],
    ...(math ? { misreadings: ['把牧首当成世俗官员是误读。§埃及 说他是教会领袖。'], role_in_book: ['§埃及 用牧首的谈判说明城市如何归顺。'] } : {}),
  };
  const body = job.map((t) => {
    const rows = [`### ${t.term}`, `- 不用：${t.avoid.join('、') || '—'}`, `- 一句话意思：${t.meaning}`, '- 现在的释义：', ...t.explanation.map((l) => `  - ${l}`), '- 书中的说法：', ...t.book.map((l) => `  - ${l}`)];
    if (t.kind === 'math') rows.push('- 现在的常见误读：', ...t.misreadings.map((l) => `  - ${l}`), '- 现在的本书作用：', ...t.role.map((l) => `  - ${l}`));
    return rows.join('\n');
  }).join('\n\n');
  return `# 术语释义重写任务 ${id}

下面每个术语卡的释义写成了书中记述的罗列，读者读不懂这个术语。你为每个术语重写释义。读者是刚入门的研究者。

## 工作方式

判断要用的材料全在本条消息里。不读取、不列出、不搜索任何文件，不写脚本。

- 写之前想两遍，都在推理里做，不用工具：第一遍逐个术语回答“读者只能记住一件事，那是什么”，再决定哪些事实帮助读者懂这件事；第二遍逐句核对，每个事实都来自下面的材料，每句都接着上一句。
- 然后一次写完答案文件，运行一次 SELF-CHECK。只有 faults 不是 0 才改答案再运行。faults 是 0 就结束。

## \`explanation\` 怎么写

- 现在的释义是按出处堆起来的句子。不要逐句改写它，要重新组织。
- 写 ${MIN_LINES}–${MAX_LINES} 句。第一句下定义。后面每句回答一个问题：它怎样运作，由谁做，为什么这样，带来什么结果。
- 每句都和上一句有因果、条件、对比或先后关系，读起来是一段连贯的讲解。不要把互不相干的事实并列成一串。帮不上读者懂这个术语的事实就删去：它们留在卡片的其他部分。
- 不用同一个主语连写多句（不要「他们……他们……他们……」）。
- 不写小节号和章号，不提「本书」「作者」「同一小节」。
- 只用下面材料里的事实，不补充材料之外的事实。
- 遵守 \`asd-ste100-zh\`：一句只说一件事，不用分号，主动语态，不用模糊词和夸奖词。术语用给定写法，不用「不用」一栏的词。正文不写 \`[[…]]\`。
${math ? `
## 被误写成数学术语的卡

这些术语没有数学内容。卡上的公式和模型是误写的，全部作废。\`explanation\` 不提公式、模型和本卡。
另外重写两个字段，去掉其中与公式、模型、\`$\` 符号、「本卡」有关的句子，其余照原样保留：
- \`misreadings\`：2–5 条。
- \`role_in_book\`：1–4 句。这两个字段保留「§小节名 说」的写法。
` : ''}
## 术语

${body}

## 输出

把结果写入 \`${path.join(out, 'answers', `${id}.json`)}\`，UTF-8，只写这一个文件。格式：\`{"terms":[…]}\`，本组每个术语一项。样例：

\`\`\`json
${JSON.stringify(sample)}
\`\`\`

SELF-CHECK: node "${process.argv[1]}" check --out "${out}" --job ${id}
写完运行这一行命令自查，按故障行改正，直到最后一行是 \`faults: 0\`（最多三次）。
写完后只回复一行 JSON：\`{"terms":术语数}\`。
`;
}

function faultsOf(items, job, answer) {
  const faults = [];
  const byTerm = new Map((answer?.terms || []).map((t) => [t.term, t]));
  for (const term of job.terms) {
    const item = items.get(term); const t = byTerm.get(term);
    if (!t) { faults.push(`${term} 缺少这一项`); continue; }
    const list = Array.isArray(t.explanation) ? t.explanation : [];
    if (list.length < MIN_LINES || list.length > MAX_LINES) faults.push(`${term} explanation 有 ${list.length} 句，应为 ${MIN_LINES}–${MAX_LINES} 句`);
    list.forEach((line, i) => {
      if (typeof line !== 'string' || !line.trim()) faults.push(`${term} explanation[${i}] 是空的`);
      else if (NARRATION.test(line)) faults.push(`${term} explanation[${i}] 在转述书：${line}`);
      else if (/\[\[/.test(line)) faults.push(`${term} explanation[${i}] 写了链接`);
      for (const word of item.avoid) if (word && word !== '—' && line.includes(word)) faults.push(`${term} explanation[${i}] 用了不用词 ${word}`);
      if (item.kind === 'math' && MATH_WORDS.test(line)) faults.push(`${term} explanation[${i}] 提到公式或模型：${line}`);
    });
    if (item.kind === 'math') {
      for (const field of ['misreadings', 'role_in_book']) {
        const rows = Array.isArray(t[field]) ? t[field] : [];
        if (!rows.length) faults.push(`${term} 缺少 ${field}`);
        rows.forEach((line, i) => { if (MATH_WORDS.test(line)) faults.push(`${term} ${field}[${i}] 提到公式或模型：${line}`); });
      }
    }
  }
  for (const term of byTerm.keys()) if (!job.terms.includes(term)) faults.push(`${term} 不是本组术语`);
  return faults;
}

function check(a) {
  const jobs = JSON.parse(fs.readFileSync(path.join(a.out, 'jobs.json'), 'utf8'));
  const items = new Map(JSON.parse(fs.readFileSync(path.join(a.out, 'items.json'), 'utf8')).map((i) => [i.term, i]));
  const job = jobs.find((j) => j.id === a.job);
  let answer = null;
  try { answer = JSON.parse(fs.readFileSync(job.answer, 'utf8')); } catch (e) { console.log(`答案文件读不了：${e.message}`); }
  const faults = answer ? faultsOf(items, job, answer) : ['没有答案'];
  for (const f of faults) console.log(f);
  console.log(`faults: ${faults.length}`);
}

/** The new body of a concept Card: every section as it stands, `## 释义` replaced. */
export function replaceExplanation(body, explanation) {
  const open = '## 释义\n\n'; const at = body.indexOf(open);
  const end = body.indexOf('\n\n## ', at + open.length);
  if (at < 0 || end < 0) throw new Error('no 释义 section');
  return body.slice(0, at + open.length) + explanation.join('\n') + body.slice(end);
}

/** A false mathematical Card rebuilt as a concept Card, from its own sections. */
export function rebuildConcept(body, parts, answer) {
  const all = [body, ...parts].join('\n\n');
  const list = (name) => withoutSource(section(all, name));
  const headEnd = body.indexOf('## ');
  const head = body.slice(0, headEnd).replace(/^类别：数学概念$/m, '类别：概念').trimEnd();
  const out = [head, `## 释义\n\n${answer.explanation.join('\n')}`, `## 书中的说法\n\n${list('书中的说法').join('\n')}`];
  const distinctions = list('区别');
  if (distinctions.length) out.push(`## 区别\n\n${distinctions.join('\n')}`);
  out.push(`## 常见误读\n\n${answer.misreadings.map((l) => `- ${l.replace(/^- /, '')}`).join('\n')}`);
  out.push(`## 在本书中的作用\n\n${answer.role_in_book.map((l) => l.replace(/^- /, '')).join('\n')}`);
  out.push(sourceLine(all));
  return out.join('\n\n') + '\n';
}

async function apply(a) {
  const jobs = JSON.parse(fs.readFileSync(path.join(a.out, 'jobs.json'), 'utf8'));
  const list = JSON.parse(fs.readFileSync(path.join(a.out, 'items.json'), 'utf8'));
  const items = new Map(list.map((i) => [i.term, i]));
  const only = onlyTerms(a);
  const answers = new Map();
  for (const job of jobs) {
    if (!fs.existsSync(job.answer)) continue;
    const answer = JSON.parse(fs.readFileSync(job.answer, 'utf8'));
    const faults = faultsOf(items, job, answer);
    if (faults.length) throw new Error(`${job.id} still has faults: ${faults[0]}`);
    for (const t of answer.terms) answers.set(t.term, t);
  }
  const zt = new Zt(); await zt.init();
  const chosen = [...answers.keys()].filter((t) => !only || only.has(t));
  const raw = new Map((await cards(zt, chosen.map((t) => items.get(t).address), 'raw')).map((c) => [c.term, c]));
  const result = { ok: true, concept: 0, math: 0, parts_deleted: 0, refused: [] };
  for (const term of chosen) {
    const item = items.get(term); const card = raw.get(term); const answer = answers.get(term);
    try {
      if (item.kind === 'concept') {
        await zt.call('edit_card', { at: item.address, body: replaceExplanation(card.body, answer.explanation) });
        result.concept++;
      } else {
        const parts = (await cards(zt, item.parts, 'raw')).map((c) => c.body);
        const body = rebuildConcept(card.body, parts, answer);
        for (const part of [...item.parts].reverse()) { await zt.call('delete_card', { at: part, confirm: 'delete' }); result.parts_deleted++; }
        await zt.call('edit_card', { at: item.address, body });
        result.math++;
      }
    } catch (e) { result.refused.push({ term, error: e.message.slice(0, 300) }); }
  }
  zt.close();
  result.ok = result.refused.length === 0;
  console.log(JSON.stringify(result));
}

// ---- judge: is a mathematical Term Card mathematics in the book, or a formula the writer built? ----

const word = (f) => {
  const inner = f.replace(/^\$+|\$+$/g, '').replaceAll('\\text', '').replaceAll('\\ ', ' ');
  const letters = [...inner].filter((c) => /\p{L}/u.test(c));
  return [...inner].every((c) => /\p{L}/u.test(c) || " '’‘ʿʾ{}-".includes(c)) && (letters.length >= 4 || letters.some((c) => c.charCodeAt(0) > 127));
};
const footnote = (f) => /^\$\s*(\{\s*\})?\s*\^\s*\{?([\d,\s-]+|[a-z])\}?\s*\$$/.test(f);
const realFormulas = (line) => (line.match(/\$\$[^$]*\$\$|\$[^$\n]+\$/g) || []).filter((f) => f.length > 2 && !word(f) && !footnote(f));
const around = (line, term, width = 160) => {
  const at = line.indexOf(term);
  if (at < 0 || line.length <= 2 * width) return line.slice(0, 2 * width);
  return '…' + line.slice(Math.max(0, at - width), at + term.length + width) + '…';
};

async function judgePlan(a) {
  const out = a.out; const perJob = Number(a['per-job'] || 12);
  fs.mkdirSync(path.join(out, 'judge', 'jobs'), { recursive: true });
  fs.mkdirSync(path.join(out, 'judge', 'answers'), { recursive: true });
  // A row the plan classified from a real formula, a notation table or a setting symbol is mathematics in the book.
  const planFile = path.join(a.staging, 'term-cards', 'plan.json');
  const planned = new Map();
  if (fs.existsSync(planFile)) {
    for (const r of JSON.parse(fs.readFileSync(planFile, 'utf8')).rows) {
      const real = r.classification.evidence.filter((e) => !(e.kind.endsWith('formula') && (word(e.formula) || footnote(e.formula))));
      planned.set(r.term, real.length > 0);
    }
  }
  const sectionsDir = path.join(a.staging, 'sections');
  const book = fs.readdirSync(sectionsDir).sort().flatMap((file) => fs.readFileSync(path.join(sectionsDir, file), 'utf8').split('\n').map((text, i) => ({ file, line: i + 1, text })));
  const zt = new Zt(); await zt.init();
  const list = await zt.call('get_card', { at: a.list, term_links: 'label' });
  const meaning = new Map([...list.body.matchAll(/^\| (.+?) \| (.+?) \| (.+?) \| (.+?) \|$/gm)].map((m) => [m[1], m[3]]));
  const read = await cards(zt, list.sides.map((h) => h.address), 'label');
  zt.close();
  const items = [];
  for (const card of read) {
    if (!/^类别：数学概念$/m.test(card.body) || planned.get(card.term) === true) continue;
    const def = (card.body.split('## 数学定义')[1] || '').split('\n符号表')[0];
    const formula = (def.match(/\$\$[\s\S]*?\$\$/) || [''])[0];
    const reading = lines(def.slice(def.indexOf(formula) + formula.length)).slice(0, 3);
    const marked = (def.match(/补：书中未写/g) || []).length;
    const withFormula = book.filter((l) => l.text.includes(card.term) && realFormulas(l.text).length).slice(0, 4);
    const plain = withFormula.length ? [] : book.filter((l) => l.text.includes(card.term)).slice(0, 3);
    items.push({
      id: `m${String(items.length + 1).padStart(3, '0')}`, term: card.term, address: card.address, meaning: meaning.get(card.term) || '', formula, reading, marked,
      lines: [...withFormula, ...plain].map((l) => ({ formula: realFormulas(l.text).length > 0, text: around(l.text, card.term) })),
    });
  }
  const jobs = [];
  for (let i = 0; i < items.length; i += perJob) jobs.push(items.slice(i, i + perJob));
  const index = jobs.map((job, n) => {
    const id = `j${String(n + 1).padStart(3, '0')}`;
    const file = path.join(out, 'judge', 'jobs', `${id}.md`); const answer = path.join(out, 'judge', 'answers', `${id}.json`);
    fs.writeFileSync(file, judgeText(out, id, job, answer), 'utf8');
    return { id, file, answer, ids: job.map((t) => t.id) };
  });
  fs.writeFileSync(path.join(out, 'judge', 'items.json'), JSON.stringify(items, null, 1), 'utf8');
  fs.writeFileSync(path.join(out, 'judge', 'jobs.json'), JSON.stringify(index, null, 1), 'utf8');
  console.log(JSON.stringify({ ok: true, math: read.filter((c) => /^类别：数学概念$/m.test(c.body)).length, judged: items.length, jobs: index.length, bytes: Math.max(0, ...index.map((j) => fs.statSync(j.file).size)) }));
}

function judgeText(out, id, job, answer) {
  const body = job.map((t) => [
    `### ${t.id} ${t.term}`, `- 一句话意思：${t.meaning}`, '- 卡上的定义公式：', t.formula || '（没有）', ...t.reading.map((l) => `  ${l}`),
    `- 定义部分标「补：书中未写」的次数：${t.marked}`,
    t.lines.length ? '- 书中含这个术语的行：' : '- 书中没有含这个术语原文写法的行。',
    ...t.lines.map((l) => `  - [${l.formula ? '有公式' : '无公式'}] ${l.text}`),
  ].join('\n')).join('\n\n');
  const tick = '`';
  return `# 数学术语卡判定任务 ${id}

下面每张术语卡都写成了数学概念：卡上有定义公式、符号、推导。有些卡的公式来自书，有些是写卡的人自己构造的。你判定每张卡属于哪一种。

## 工作方式

判断要用的材料全在本条消息里。不读取、不列出、不搜索任何文件，不写脚本。逐个术语在推理里判断，然后一次写完答案文件，运行一次 SELF-CHECK。

## 判定规则

- ${tick}math${tick}：书本身用公式或模型定义、推导或使用这个术语。例如书给出它的公式，或书的模型里它是一个变量、一条曲线、一个条件、一个均衡、一个比率。卡上的公式与书的公式一致，或是书的公式的直接写法。
- ${tick}concept${tick}：书只用文字讨论这个术语，卡上的公式是写卡的人为它构造的。例如一种制度、一项政策、一个历史现象、一个群体、一种观点，书没有给它公式，卡却给它造了效用函数、成本函数或集合模型。
- 书中这一行有公式，不等于这个术语是数学概念：公式可能讲的是别的量。看公式里有没有这个术语对应的量。
- 拿不准时写 ${tick}math${tick}：误删书中的数学，比留下一张多余的推导代价大。

## 术语

${body}

## 输出

把结果写入 ${tick}${answer}${tick}，UTF-8，只写这一个文件。格式：${tick}{"terms":[{"id":"m001","verdict":"math","reason":"一句理由"}]}${tick}，本组每个术语一项，id 照抄。

SELF-CHECK: node "${process.argv[1]}" judge-check --out "${out}" --job ${id}
写完运行这一行命令自查，按故障行改正，直到最后一行是 ${tick}faults: 0${tick}（最多三次）。
写完后只回复一行 JSON：${tick}{"math":数,"concept":数}${tick}。
`;
}

function judgeFaults(job, answer) {
  const faults = []; const seen = new Map((answer?.terms || []).map((t) => [t.id, t]));
  for (const id of job.ids) {
    const t = seen.get(id);
    if (!t) faults.push(`${id} 缺少这一项`);
    else if (!['math', 'concept'].includes(t.verdict)) faults.push(`${id} verdict 必须是 math 或 concept`);
    else if (!t.reason || !String(t.reason).trim()) faults.push(`${id} 缺少 reason`);
  }
  for (const id of seen.keys()) if (!job.ids.includes(id)) faults.push(`${id} 不是本组的 id`);
  return faults;
}

function judgeCheck(a) {
  const jobs = JSON.parse(fs.readFileSync(path.join(a.out, 'judge', 'jobs.json'), 'utf8'));
  const job = jobs.find((j) => j.id === a.job); let answer = null;
  try { answer = JSON.parse(fs.readFileSync(job.answer, 'utf8')); } catch (e) { console.log(`答案文件读不了：${e.message}`); }
  const faults = answer ? judgeFaults(job, answer) : ['没有答案'];
  for (const f of faults) console.log(f);
  console.log(`faults: ${faults.length}`);
}

/** The terms judged concept, one a line, for `plan --only @file`. */
function judgeList(a) {
  const jobs = JSON.parse(fs.readFileSync(path.join(a.out, 'judge', 'jobs.json'), 'utf8'));
  const items = new Map(JSON.parse(fs.readFileSync(path.join(a.out, 'judge', 'items.json'), 'utf8')).map((i) => [i.id, i]));
  const concept = []; let math = 0, missing = 0;
  for (const job of jobs) {
    if (!fs.existsSync(job.answer)) { missing += job.ids.length; continue; }
    const answer = JSON.parse(fs.readFileSync(job.answer, 'utf8'));
    if (judgeFaults(job, answer).length) throw new Error(`${job.id} still has faults`);
    for (const t of answer.terms) { if (t.verdict === 'concept') concept.push(items.get(t.id).term); else math++; }
  }
  fs.writeFileSync(path.join(a.out, 'judge', 'concept.txt'), concept.join('\n') + '\n', 'utf8');
  console.log(JSON.stringify({ ok: missing === 0, math, concept: concept.length, missing, list: path.join(a.out, 'judge', 'concept.txt') }));
}

/** `--only t1,t2`, or `--only @file` with one term a line. */
function onlyTerms(a) {
  if (!a.only) return null;
  const list = a.only.startsWith('@') ? fs.readFileSync(a.only.slice(1), 'utf8').split(/\r?\n/) : a.only.split(',');
  return new Set(list.map((t) => t.trim()).filter(Boolean));
}

const a = args(process.argv.slice(2));
if (import.meta.url === `file:///${process.argv[1].replace(/\\/g, '/').replace(/^\//, '')}`) {
  if (a._ === 'plan') await plan(a);
  else if (a._ === 'check') check(a);
  else if (a._ === 'apply') await apply(a);
  else if (a._ === 'judge-plan') await judgePlan(a);
  else if (a._ === 'judge-check') judgeCheck(a);
  else if (a._ === 'judge-list') judgeList(a);
  else throw new Error('usage: plan | check | apply');
}
