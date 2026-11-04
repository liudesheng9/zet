#!/usr/bin/env node
// Drives a real browser over one portable export, as a reader would: it opens index.html from
// disk, chooses cards.json through the file input, and uses the real GUI the export carries. It
// clicks every function that needs the service and expects the portable notice.
//
//   node scripts/portable-export/check-viewer.mjs --folder <export> [--scene <scene.json>] [--browser <exe>]
//
// The scene names the facts of a scratch export (see `browser_check` in tests/portable_export.rs).
// Without one the export counts as a whole real book: it times the moves, draws every chart, opens
// the Model boards, and scans every Card for anything the page's filter would take away. It prints
// one JSON line and exits 0 when all passed.

import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { setTimeout as sleep } from 'node:timers/promises';

const args = Object.fromEntries(
  process.argv.slice(2).reduce((rows, word, at, all) => {
    if (word.startsWith('--')) rows.push([word.slice(2), all[at + 1]]);
    return rows;
  }, []),
);
const folder = resolve(args.folder ?? '.');
const scene = args.scene ? JSON.parse(readFileSync(args.scene, 'utf8')) : null;

const BROWSERS = [
  args.browser,
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
].filter(Boolean);
const browser = BROWSERS.find(existsSync);
if (!browser) {
  console.log(JSON.stringify({ skipped: 'no browser' }));
  process.exit(0);
}

const NOTICE = 'is not available in the portable version';
const failures = [];
const metrics = {};
const passed = [];
const check = (name, ok, detail) => {
  if (ok) passed.push(name);
  else failures.push({ name, detail });
};

const profile = mkdtempSync(join(tmpdir(), 'zt-viewer-'));
const child = spawn(
  browser,
  ['--headless=new', '--disable-gpu', '--no-first-run', '--no-default-browser-check', '--window-size=1400,1000',
    '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank'],
  { stdio: 'ignore' },
);

/** One CDP connection to one page. */
class Page {
  constructor(socket) {
    this.socket = socket;
    this.next = 1;
    this.waiting = new Map();
    this.events = [];
    socket.addEventListener('message', ({ data }) => {
      const message = JSON.parse(data);
      if (message.id && this.waiting.has(message.id)) {
        const { ok, no } = this.waiting.get(message.id);
        this.waiting.delete(message.id);
        if (message.error) no(new Error(JSON.stringify(message.error)));
        else ok(message.result);
      } else if (message.method) this.events.push(message);
    });
  }

  send(method, params = {}) {
    return new Promise((ok, no) => {
      const id = this.next++;
      this.waiting.set(id, { ok, no });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async value(expression) {
    const { result, exceptionDetails } = await this.send('Runtime.evaluate', {
      expression, returnByValue: true, awaitPromise: true,
    });
    if (exceptionDetails) throw new Error(`${expression}: ${JSON.stringify(exceptionDetails.exception)}`);
    return result.value;
  }

  async until(expression, what, ms = 8000) {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      if (await this.value(expression)) return true;
      await sleep(40);
    }
    failures.push({ name: `waited for ${what}`, detail: expression });
    return false;
  }

  async go(url) {
    await this.send('Page.navigate', { url });
    await sleep(150);
    await this.until(`document.readyState === 'complete'`, `load of ${url}`);
  }

  async chooseFile(path) {
    const { root } = await this.send('DOM.getDocument');
    const { nodeId } = await this.send('DOM.querySelector', { nodeId: root.nodeId, selector: '#zt-slip-file' });
    await this.send('DOM.setFileInputFiles', { nodeId, files: [path] });
  }

  async click(x, y) {
    for (const type of ['mousePressed', 'mouseReleased']) {
      await this.send('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1 });
    }
  }

  async press(key, code = key, windowsVirtualKeyCode = 0) {
    for (const type of ['keyDown', 'keyUp']) {
      await this.send('Input.dispatchKeyEvent', { type, key, code, windowsVirtualKeyCode, ...(type === 'keyDown' && key.length === 1 ? { text: key } : {}) });
    }
  }
}

async function connect() {
  const portFile = join(profile, 'DevToolsActivePort');
  for (let tries = 0; tries < 200 && !existsSync(portFile); tries += 1) await sleep(50);
  const port = readFileSync(portFile, 'utf8').split('\n')[0];
  const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
  const socket = new WebSocket(targets.find((target) => target.type === 'page').webSocketDebuggerUrl);
  await new Promise((ok) => socket.addEventListener('open', ok, { once: true }));
  const page = new Page(socket);
  await page.send('Page.enable');
  await page.send('Runtime.enable');
  await page.send('DOM.enable');
  return page;
}

const visible = (selector) => `(() => { const n = document.querySelector(${JSON.stringify(selector)}); return !!n && !n.hidden && n.getClientRects().length > 0; })()`;
const CARD = '#reader article.card';
const titleIs = (title) => `document.querySelector('#reader h1.title')?.textContent === ${JSON.stringify(title)}`;
const stampIs = (address) => `[...document.querySelectorAll('#reader .card-meta .stamp')].some((n) => n.textContent === ${JSON.stringify(address)})`;
const hashTo = (address) => `location.hash = ${JSON.stringify(`#/card/${encodeURIComponent(address)}`)}`;
const button = (label) => `[...document.querySelectorAll('#reader .btn')].find((b) => b.textContent.trim().startsWith(${JSON.stringify(label)}))`;
const clearToasts = `document.querySelectorAll('#toasts .toast').forEach((n) => n.remove())`;
const toastHas = (text) => `[...document.querySelectorAll('#toasts .toast')].some((n) => n.textContent.includes(${JSON.stringify(text)}))`;

async function serveFolder() {
  const server = createServer((request, response) => {
    const name = request.url === '/' ? 'index.html' : request.url.split('?')[0].slice(1);
    const file = join(folder, name);
    if (!existsSync(file)) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { 'content-type': name.endsWith('.html') ? 'text/html; charset=utf-8' : 'application/json' });
    response.end(readFileSync(file));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  return server;
}

async function checkFromDisk(page) {
  const url = pathToFileURL(join(folder, 'index.html')).href;
  await page.go(url);
  check('from disk the page offers the file slip', await page.until(visible('#zt-slip'), 'the slip'));

  const wrong = join(profile, 'wrong.json');
  writeFileSync(wrong, JSON.stringify({ format: 'something-else' }));
  await page.chooseFile(wrong);
  check(
    'a file of another format is named',
    await page.until(`document.querySelector('#zt-slip-note').textContent.includes('not a ZET export')`, 'the refusal'),
  );

  await page.chooseFile(join(folder, 'cards.json'));
  check('choosing cards.json opens the tree', await page.until(visible(CARD), 'the Card', 60000));
  check('the slip leaves', !(await page.value(visible('#zt-slip'))));
  const data = JSON.parse(readFileSync(join(folder, 'cards.json'), 'utf8'));
  check('the root Card opens first', await page.until(stampIs(data.cards[0].address), 'the root Card'));
  check('the page wears the tree title', (await page.value('document.title')).includes(data.tree.title));
}

async function checkSameGui(page) {
  check('the GUI shell is there', await page.value(`!!document.querySelector('.topbar') && !!document.querySelector('#rail') && !!document.querySelector('#stage')`));
  check('the Map draws', await page.until(`document.querySelectorAll('#stage .node').length > 1`, 'the Map'));
  check('the cabinet lists the tree', await page.until(`document.querySelector('#rail').textContent.length > 0`, 'the cabinet'));
  await page.value(`document.querySelector('[data-mode="constellation"]').click()`);
  check('the Constellation draws', await page.until(`[...document.querySelectorAll('#stage canvas')].some((c) => c.getClientRects().length > 0)`, 'the Constellation'));
  await page.value(`document.querySelector('[data-mode="table"]').click()`);
  await sleep(100);
  await page.value(`${button('To the Table')}.click()`);
  check('a Card goes to the Table', await page.until(`document.getElementById('table-count').textContent.trim() === '1'`, 'the Table count'));
  await page.value(`document.querySelector('[data-mode="map"]').click()`);
  await sleep(100);
}

async function checkReader(page) {
  await page.value(hashTo(scene.shows));
  check('a Card opens by its route', await page.until(titleIs(scene.shows_title), 'the Card'));
  check('math typesets', await page.until(`document.querySelectorAll('#reader .md .katex').length > 0`, 'KaTeX'));

  const inside = `[...document.querySelectorAll('#reader .md a.zt-link')].find((a) => a.dataset.target === ${JSON.stringify(scene.first)})`;
  await page.value(`${inside}.click()`);
  check('a Link opens its Card', await page.until(titleIs(scene.first_title), 'the Link target'));
  await page.value('history.back()');
  check('back retraces the move', await page.until(titleIs(scene.shows_title), 'back'));
  await page.value('history.forward()');
  check('forward retraces it again', await page.until(titleIs(scene.first_title), 'forward'));
  await page.value(hashTo(scene.shows));
  await page.until(titleIs(scene.shows_title), 'the Card again');

  const hash = await page.value('location.hash');
  await page.value(clearToasts);
  await page.value(`document.querySelector('#reader .md .zt-link.outside').click()`);
  check('an outside Link says it is not here', await page.until(toastHas('not in this portable export'), 'the outside notice'));
  check('an outside Link goes nowhere', (await page.value('location.hash')) === hash);
  check('an outside Link keeps its title', (await page.value(`document.querySelector('#reader .md .zt-link.outside').textContent`)).includes(scene.far_title));
  const term = `document.querySelector('#reader .md a.zt-term')`;
  check('a Term link carries its meaning', (await page.value(`${term}.dataset.meaning`)) === scene.term_meaning);
  check('a Term link shows its slip', (await page.value(`${term}.textContent`)).includes(scene.term_meaning));

  await page.value(hashTo(scene.head));
  await page.until(titleIs(scene.head_title), 'the Term head');
  const uses = await page.value(`document.querySelector('#reader details.term-uses summary')?.textContent ?? ''`);
  check('Term uses count only this tree', uses === 'Term uses · 1', uses);
  check('no Card of another tree is named', !(await page.value(`document.querySelector('#reader').textContent`)).includes(scene.other_tree_title));

  await page.value(hashTo(scene.first));
  await page.until(titleIs(scene.first_title), 'the first Card');
  check('the neighbours show', await page.until(`document.querySelector('#reader').textContent.includes(${JSON.stringify(scene.shows_title)})`, 'the neighbours'));
}

async function checkContent(page) {
  await page.value(hashTo(scene.shows));
  await page.until(titleIs(scene.shows_title), 'the Card');
  await page.until(`document.querySelectorAll('#reader .md .zt-figure img').length > 0`, 'the figures');
  const images = await page.value(`[...document.querySelectorAll('#reader .md .zt-figure img')].map((img) => img.src.slice(0, 22))`);
  check('a figure shows from the export', images.length === 2 && images.every((src) => src === 'data:image/png;base64,'), images);
  check('a lost figure says so', await page.value(`document.querySelector('#reader .md .zt-figure.missing')?.textContent.includes('figure not in this export')`));
  check('a Document link is its label', await page.value(`!document.querySelector('#reader .md .zt-doc-label')?.closest('a') && document.querySelector('#reader .md .zt-doc-label').textContent === '原书'`));

  await page.value(`document.querySelector('#reader .md .zt-figure img').click()`);
  check('a figure opens large', await page.until(`[...document.querySelectorAll('img')].filter((img) => !img.closest('#reader')).some((img) => img.src.startsWith('data:image/png'))`, 'the figure dialog'));
  await page.press('Escape', 'Escape', 27);
  await sleep(250);

  const frame = `document.querySelector('#reader iframe.zt-chart-frame')`;
  check('a chart sits in a sandboxed frame', (await page.value(`${frame}.getAttribute('sandbox')`)) === 'allow-scripts');
  check('a chart runs and sizes itself', await page.until(`!!${frame}.style.height`, 'the chart height'));
  await page.value(`${frame}.scrollIntoView({ block: 'center' })`);
  await sleep(300);
  const box = await page.value(`(() => { const r = ${frame}.getBoundingClientRect(); return { x: r.x, y: r.y }; })()`);
  await page.click(box.x + 12, box.y + 10);
  check('a Link in a chart opens its Card', await page.until(titleIs(scene.first_title), 'the Card a chart opens'));
  await page.value(hashTo(scene.shows));
  await page.until(titleIs(scene.shows_title), 'the Card again');
}

/** Everything in the GUI that needs the service, reached the way a reader reaches it. */
async function checkUnsupported(page) {
  await page.value(hashTo(scene.shows));
  await page.until(titleIs(scene.shows_title), 'the Card');
  const reach = async (name, expression) => {
    await page.value(`${clearToasts}; 0`);
    await page.value(`${expression}; 0`);
    const shown = await page.until(toastHas(NOTICE), `the notice of ${name}`, 3000);
    const stray = await page.value(`document.querySelectorAll('[role="dialog"]:not(.finder-tray)').length`);
    check(`${name} says it is not available`, shown && stray === 0, { stray });
  };
  for (const label of ['Edit', 'Reply', 'Direct', 'Side', 'Insert', 'Move', 'Delete', 'Bridge report']) {
    await reach(`the ${label} button`, `${button(label)}.click()`);
  }
  // The notice comes first, before any question a function would ask or any rule it would raise.
  await page.value(hashTo(scene.root));
  await page.until(stampIs(scene.root), 'the root Card');
  await reach('Edit on a Literature Card', `${button('Edit')}.click()`);
  await page.value(hashTo(scene.first));
  await page.until(titleIs(scene.first_title), 'a Card with a Direct successor');
  await reach('Direct where one exists', `${button('Direct')}.click()`);
  await page.value(hashTo(scene.shows));
  await page.until(titleIs(scene.shows_title), 'the Card');
  await reach('Ask the box', `document.getElementById('ask-open').click()`);
  await reach('the Lock panel', `document.getElementById('locks-open').click()`);
  await reach('a new Topic', `document.querySelector('[data-action="new-topic"]').click()`);
  await reach('a new Literature Card', `document.querySelector('[data-action="new-literature"]').click()`);
  const command = (line) => `(() => { const input = document.getElementById('command-input'); input.value = ${JSON.stringify(line)}; input.focus(); input.dispatchEvent(new Event('input', { bubbles: true })); input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()`;
  await reach('the command `e`', command('e'));
  await reach('the command `del`', command('del'));
  await reach('the command `lv formal`', command('lv formal'));
  const key = async (name, letter) => {
    await page.value(`${clearToasts}; document.activeElement?.blur(); 0`);
    await page.press(letter);
    check(`${name} says it is not available`, await page.until(toastHas(NOTICE), `the notice of ${name}`, 3000));
  };
  await key('the key e', 'e');
  await key('the key t', 't');
  await page.value(clearToasts);
  check('reading works on after every refusal', await page.value(`document.querySelector('#reader h1.title').textContent === ${JSON.stringify(scene.shows_title)}`));
}

async function search(page, words) {
  const input = `document.querySelector('.finder-tray input')`;
  if (!(await page.value(`!!${input} && ${input}.getClientRects().length > 0`))) {
    await page.value(`document.getElementById('search-open').click()`);
    await page.until(`!!${input}`, 'the search tray');
  }
  await page.value(`(() => { const input = ${input}; input.value = ${JSON.stringify(words)}; input.dispatchEvent(new Event('input', { bubbles: true })); })()`);
}

async function checkSearchAndTheme(page) {
  await search(page, scene.first_title);
  check('a title word finds its Card', await page.until(`[...document.querySelectorAll('.finder-tray .fslip-title')].some((n) => n.textContent === ${JSON.stringify(scene.first_title)})`, 'the title result'));
  await search(page, scene.body_word);
  check('a body word finds its Card', await page.until(`[...document.querySelectorAll('.finder-tray .fslip-title')].some((n) => n.textContent.includes(${JSON.stringify(scene.shows_title)}))`, 'the body result'));
  await page.press('Escape', 'Escape', 27);
  await sleep(300);

  await page.value(hashTo(scene.shows));
  await page.until(titleIs(scene.shows_title), 'the Card');
  await page.until(`!!document.querySelector('#reader iframe.zt-chart-frame')?.srcdoc`, 'the chart');
  const before = await page.value(`document.documentElement.dataset.theme || 'light'`);
  await page.value(`document.getElementById('theme-toggle').click()`);
  const after = await page.value('document.documentElement.dataset.theme');
  check('the theme button turns the page', before !== after && ['light', 'dark'].includes(after), { before, after });
  check('the theme reaches an open chart', (await page.value(`document.querySelector('#reader iframe.zt-chart-frame').srcdoc`)).includes(`data-theme="${after}"`));
  await page.send('Page.reload');
  await sleep(200);
  await page.until(`document.readyState === 'complete'`, 'the reload');
  await page.chooseFile(join(folder, 'cards.json'));
  await page.until(visible(CARD), 'the Card after a reload');
  check('the theme survives a reload', (await page.value('document.documentElement.dataset.theme')) === after);
}

async function checkRefusedStorage(page) {
  const { identifier } = await page.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `Object.defineProperty(window, 'localStorage', { get() { throw new Error('storage refused'); } });`,
  });
  await page.go(pathToFileURL(join(folder, 'index.html')).href);
  await page.chooseFile(join(folder, 'cards.json'));
  check('a browser that refuses storage still reads', await page.until(visible(CARD), 'the reader without storage'));
  await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier });
}

async function checkHostileFile(page) {
  let hits = 0;
  const trap = createServer((request, response) => {
    hits += 1;
    response.writeHead(200, { 'access-control-allow-origin': '*' }).end('reached');
  });
  await new Promise((ok) => trap.listen(0, '127.0.0.1', ok));
  const url = `http://127.0.0.1:${trap.address().port}/ping`;
  const probe = [
    '<!doctype html><html data-theme="__ZT_THEME__"><head><!--zt:katex--><!--zt:frame--></head><body><div id="zt-chart">probe</div><script>',
    `(async () => {
      const send = (zt, value) => parent.postMessage({ zt, value }, '*');
      let top = 'read';
      try { void parent.document.title; } catch { top = 'refused'; }
      send('probe-parent', top);
      send('probe-fetch', await fetch(${JSON.stringify(url)}).then(() => 'reached', () => 'refused'));
      send('probe-katex', !!window.katex);
      await new Promise((ok) => { const image = new Image(); image.onload = image.onerror = ok; image.src = ${JSON.stringify(`${url}?image`)}; });
    })();`,
    '</script></body></html>',
  ].join('');
  const html = [
    '<p id="safe">safe text</p>',
    '<img src="x" onerror="window.pwned = 1">',
    '<script>window.pwned = 2</script>',
    '<a id="evil" href="javascript:window.pwned = 3">evil</a>',
    '<a id="away" href="https://example.com/">away</a>',
    '<span id="hover" onmouseover="window.pwned = 4" style="position:fixed">hover</span>',
    '<svg onload="window.pwned = 5"><script>window.pwned = 6</script></svg>',
    `<img src="${url}?body">`,
    `<iframe src="${url}?frame"></iframe>`,
    '<form action="https://example.com/"><input name="x"></form>',
    '<figure class="zt-chart"><iframe class="zt-chart-frame" data-chart="probe" srcdoc="<script>parent.pwned = 7</script>"></iframe></figure>',
    '<span class="zt-figure" data-artifact="bad"><img alt="x"></span>',
  ].join('');
  const evil = '<img src=x onerror="window.pwned = 8">$<img src=x onerror="window.pwned = 9">$';
  const hostile = {
    format: 'zt-portable-export', version: 2,
    tree: { id: 'x', root: 'x', kind: 'literature', workspace: 'agent', title: evil },
    exported: { archive_revision: 1 },
    cards: [{ address: 'x', edge: 'root', kind: 'literature', type: 'idea', title: 'Hostile', excerpt: evil, text: '# Hostile\n\nbody', html, links: [], term_use_targets: [] },
      { address: 'x/1', edge: 'direct', parent: 'x', kind: 'regular', type: 'idea', title: evil, excerpt: evil, text: `# t\n\n${evil}`, html: '<p>next</p>', links: [], term_use_targets: [] }],
    artifacts: { bad: { type: 'text/html', base64: 'PHNjcmlwdD4=' } },
    charts: { probe },
    outside: [],
    broken: [],
  };
  const file = join(profile, 'hostile.json');
  writeFileSync(file, JSON.stringify(hostile));
  await page.go(pathToFileURL(join(folder, 'index.html')).href);
  await page.value(`window.probes = {}; addEventListener('message', (event) => { if (String(event.data?.zt).startsWith('probe-')) window.probes[event.data.zt] = event.data.value; }); 0`);
  await page.chooseFile(file);
  await page.until(titleIs('Hostile'), 'the hostile export');
  check('hostile: its plain text still reads', await page.value(`document.querySelector('#reader .md').textContent.includes('safe text')`));
  await page.until(`Object.keys(window.probes).length >= 3`, 'the probe chart');
  await sleep(400);
  await page.value(`document.getElementById('hover')?.dispatchEvent(new MouseEvent('mouseover', { bubbles: true })); document.getElementById('evil')?.click(); 0`);
  await page.value(hashTo('x/1'));
  await sleep(400);
  check('hostile: no injected code ran', (await page.value('window.pwned === undefined')), await page.value('window.pwned'));
  await page.value(hashTo('x'));
  await page.until(titleIs('Hostile'), 'the hostile root');
  const left = await page.value(`(() => { const b = document.querySelector('#reader .md'); return { scripts: b.querySelectorAll('script, svg, form').length, handlers: [...b.querySelectorAll('*')].filter((n) => [...n.attributes].some((a) => a.name.startsWith('on'))).length, hrefs: [...b.querySelectorAll('a')].map((a) => a.getAttribute('href')).filter(Boolean), sources: [...b.querySelectorAll('img')].map((i) => i.getAttribute('src')).filter(Boolean), framed: [...b.querySelectorAll('iframe')].map((f) => f.getAttribute('src')).filter(Boolean) }; })()`);
  check('hostile: nothing it wrote survives the filter', left.scripts === 0 && left.handlers === 0 && left.hrefs.length === 0 && left.sources.length === 0 && left.framed.length === 0, left);
  check('hostile: a wrong figure type is refused', await page.value(`document.querySelector('#reader .md .zt-figure.missing') !== null`));
  const probes = await page.value('window.probes');
  check('a chart cannot read the page', probes['probe-parent'] === 'refused', probes);
  check('a chart cannot call out', probes['probe-fetch'] === 'refused', probes);
  check('a chart gets KaTeX when it asks', probes['probe-katex'] === true, probes);
  const out = await page.value(`fetch(${JSON.stringify(url)}).then(() => 'reached', () => 'refused')`);
  check('the page cannot call out', out === 'refused', out);
  await sleep(200);
  check('nothing reached the network', hits === 0, hits);
  trap.close();
}

async function checkFromHttp(page) {
  const server = await serveFolder();
  await page.go(`http://127.0.0.1:${server.address().port}/`);
  check('served over HTTP it opens with no slip', await page.until(visible(CARD), 'the reader over HTTP'));
  check('no slip over HTTP', !(await page.value(visible('#zt-slip'))));
  await page.value(hashTo(scene.shows));
  await page.until(titleIs(scene.shows_title), 'the Card');
  await page.send('Page.reload');
  await sleep(200);
  check('a reload stays on its Card', await page.until(titleIs(scene.shows_title), 'the Card after a reload'));
  server.close();
}

/** A whole real export: timings, every chart, the Model boards, and what the filter would remove. */
async function checkBook(page) {
  const data = JSON.parse(readFileSync(join(folder, 'cards.json'), 'utf8'));
  const addresses = data.cards.map((card) => card.address);
  await page.go(pathToFileURL(join(folder, 'index.html')).href);
  await page.until(visible('#zt-slip'), 'the slip');
  let begin = Date.now();
  await page.chooseFile(join(folder, 'cards.json'));
  await page.until(visible(CARD), 'the reader', 60000);
  metrics.openMs = Date.now() - begin;

  const charted = data.cards.filter((card) => card.html.includes('data-chart')).map((card) => card.address);
  const kids = new Map();
  for (const card of data.cards) if (card.edge === 'side') kids.set(card.parent, (kids.get(card.parent) ?? 0) + 1);
  const widest = [...kids].sort((a, b) => b[1] - a[1])[0]?.[0];
  const longList = data.cards.filter((card) => card.parent === widest && card.edge === 'side').at(-1)?.address;
  const sample = addresses.filter((_, at) => at % Math.max(1, Math.floor(addresses.length / 40)) === 0);
  const moves = [];
  let katexErrors = 0;
  for (const address of [...sample, ...(longList ? [longList] : [])]) {
    begin = Date.now();
    await page.value(hashTo(address));
    await page.until(stampIs(address), `Card ${address}`, 15000);
    moves.push(Date.now() - begin);
    katexErrors += await page.value(`document.querySelectorAll('#reader .md .katex-error').length`);
  }
  moves.sort((a, b) => a - b);
  metrics.moves = moves.length;
  metrics.moveMedianMs = moves[Math.floor(moves.length / 2)];
  metrics.moveMaxMs = moves.at(-1);
  metrics.widestList = kids.get(widest) ?? 0;
  check('every sampled Card typesets its math', katexErrors === 0, katexErrors);

  const heads = Object.keys(data.models ?? {});
  let boards = 0;
  for (const head of heads.slice(0, 12)) {
    await page.value(hashTo(head));
    await page.until(stampIs(head), `Model Card ${head}`, 15000);
    if (await page.until(`!!document.querySelector('#reader .model-board, #reader [class*="mbn-"], #reader [class*="theory"]')`, `the board of ${head}`, 15000)) boards += 1;
  }
  metrics.modelsOpened = Math.min(heads.length, 12);
  metrics.models = heads.length;
  check('every opened model draws its board', boards === Math.min(heads.length, 12), { boards });

  let drawn = 0;
  for (const address of charted) {
    await page.value(hashTo(address));
    await page.until(stampIs(address), `chart Card ${address}`, 15000);
    if (await page.until(`[...document.querySelectorAll('#reader .md iframe')].every((frame) => !!frame.style.height)`, `the charts of ${address}`, 15000)) drawn += 1;
  }
  metrics.chartCards = charted.length;
  check('every chart Card draws', drawn === charted.length, { drawn, charted: charted.length });

  metrics.cardsWithLostFigures = data.cards.filter((card) => {
    const shown = [...card.html.matchAll(/data-artifact="([0-9a-f]*)"/g)].map((match) => match[1]);
    return shown.some((digest) => !data.artifacts[digest]);
  }).length;

  begin = Date.now();
  await search(page, data.cards[Math.floor(addresses.length / 2)].title.slice(0, 6));
  await page.until(`document.querySelectorAll('.finder-tray .fslip').length > 0`, 'search results', 15000);
  metrics.searchMs = Date.now() - begin;

  const server = await serveFolder();
  await page.go(`http://127.0.0.1:${server.address().port}/`);
  await page.until(visible(CARD), 'the reader over HTTP', 60000);
  const scan = await page.value(`(async () => {
    const data = await (await fetch('cards.json')).json();
    const rules = window.ZtPortableRules;
    const have = new Set(data.cards.map((card) => card.address));
    const tags = {}, attributes = {}, routes = [];
    let bareUrls = 0, localImages = 0;
    for (const card of data.cards) {
      const body = new DOMParser().parseFromString(card.html, 'text/html').body;
      for (const node of body.querySelectorAll('*')) {
        const tag = node.tagName.toLowerCase();
        if (!rules.keepTag(tag)) tags[tag] = (tags[tag] || 0) + 1;
        for (const { name, value } of node.attributes) {
          // A bare URL keeps its text, so taking its route loses nothing. A web link with another
          // label would lose the address, and that fails below.
          if (tag === 'a' && name === 'href' && /^https?:/.test(value) && node.textContent.trim() === value) {
            bareUrls += 1;
            continue;
          }
          // A Markdown image that is no Artifact names a file outside the archive: it does not
          // show in the GUI either, and nothing of it can travel.
          if (tag === 'img' && name === 'src') {
            localImages += 1;
            continue;
          }
          if (!rules.keepAttr(tag, name, value)) {
            const key = tag + ' ' + name + (name === 'href' || name === 'style' ? ' ' + value.slice(0, 24) : '');
            attributes[key] = (attributes[key] || 0) + 1;
          }
        }
        if (tag === 'a' && node.getAttribute('href')?.startsWith('#')) {
          const target = rules.linkFacts(node.getAttribute('href'))?.target;
          if (!target || !have.has(target)) routes.push(node.getAttribute('href'));
        }
      }
    }
    return { tags, attributes, routes: routes.slice(0, 5), deadRoutes: routes.length, bareUrls, localImages };
  })()`);
  server.close();
  metrics.bareUrls = scan.bareUrls;
  metrics.localImages = scan.localImages;
  check('the filter takes no tag the renderer writes', Object.keys(scan.tags).length === 0, scan.tags);
  check('the filter takes no attribute the renderer writes', Object.keys(scan.attributes).length === 0, scan.attributes);
  check('every route leads to a Card of the tree', scan.deadRoutes === 0, scan.routes);
}

let page;
try {
  page = await connect();
  await checkFromDisk(page);
  await checkSameGui(page);
  if (scene) {
    await checkReader(page);
    await checkContent(page);
    await checkUnsupported(page);
    await checkSearchAndTheme(page);
    await checkRefusedStorage(page);
    await checkHostileFile(page);
    await checkFromHttp(page);
  } else {
    await checkBook(page);
  }
  const thrown = page.events.filter((event) => event.method === 'Runtime.exceptionThrown')
    .map((event) => event.params.exceptionDetails.exception?.description ?? event.params.exceptionDetails.text);
  check('the page threw nothing', thrown.length === 0, thrown);
} catch (error) {
  const thrown = page?.events.filter((event) => event.method === 'Runtime.exceptionThrown')
    .map((event) => event.params.exceptionDetails.exception?.description);
  failures.push({ name: 'the check ran', detail: String(error?.stack ?? error), thrown });
} finally {
  page?.socket.close();
  child.kill();
  await sleep(300);
  try { rmSync(profile, { recursive: true, force: true, maxRetries: 3 }); } catch { /* the browser may hold it a moment */ }
}
console.log(JSON.stringify({ browser, passed: passed.length, metrics, failures }));
process.exit(failures.length ? 1 : 0);
