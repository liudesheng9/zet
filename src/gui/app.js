/* zt gui — a slip-box desk for ZT Cards. */
(() => {
  'use strict';

  const TOKEN = document.querySelector('meta[name="zt-token"]').content;
  const NODE_W = 196;
  const NODE_H = 78;
  const COL_W = 236;
  const ROW_H = 112;

  // ---------------------------------------------------------------- helpers
  const $ = (selector, root = document) => root.querySelector(selector);

  function h(tag, props, ...kids) {
    const node = document.createElement(tag);
    for (const [key, value] of Object.entries(props || {})) {
      if (value == null || value === false) continue;
      if (key === 'class') node.className = value;
      else if (key === 'text') node.textContent = value;
      else if (key === 'html') node.innerHTML = value;
      else if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
      else if (key === 'dataset') Object.assign(node.dataset, value);
      else node.setAttribute(key, value === true ? '' : value);
    }
    for (const kid of kids.flat(Infinity)) {
      if (kid == null || kid === false) continue;
      node.append(kid instanceof Node ? kid : String(kid));
    }
    return node;
  }

  function svg(tag, attrs) {
    const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [key, value] of Object.entries(attrs || {})) node.setAttribute(key, value);
    return node;
  }

  async function api(path, body) {
    const options = { headers: { 'X-ZT-Token': TOKEN } };
    if (body !== undefined) {
      options.method = 'POST';
      options.headers['Content-Type'] = 'application/json';
      options.body = JSON.stringify(body);
    }
    let response;
    try {
      response = await fetch('/api/' + path, options);
    } catch {
      throw new Error('the zt gui server is not responding');
    }
    let data = {};
    try { data = await response.json(); } catch { /* empty body */ }
    if (!response.ok) throw new Error(data.error || response.statusText);
    return data;
  }

  const enc = encodeURIComponent;
  const debounce = (fn, ms) => {
    let timer;
    return (...args) => { clearTimeout(timer); timer = setTimeout(() => fn(...args), ms); };
  };
  const store = {
    get(key) { try { return localStorage.getItem(key); } catch { return null; } },
    set(key, value) { try { localStorage.setItem(key, value); } catch { /* private mode */ } },
  };
  const isTyping = (target) =>
    target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable);

  function toast(message, kind) {
    const node = h('div', { class: 'toast' + (kind === 'err' ? ' err' : ''), text: message });
    $('#toasts').append(node);
    setTimeout(() => node.remove(), kind === 'err' ? 6000 : 3200);
  }
  const fail = (error) => toast(error.message || String(error), 'err');

  /** Mirrors `parent_location` in the backend. */
  function parentOf(address) {
    const slash = address.indexOf('/');
    if (slash < 0) return null;
    const tree = address.slice(0, slash);
    const segments = address.slice(slash + 1).split('|');
    if (segments.length === 1 && segments[0] === '0') return null;
    const last = segments[segments.length - 1];
    if (/^[a-z]+$/.test(last)) return tree + '/' + segments.slice(0, -1).join('|');
    const number = parseInt(last, 10);
    if (number > 1) {
      segments[segments.length - 1] = String(number - 1);
      return tree + '/' + segments.join('|');
    }
    if (segments.length > 1) return tree + '/' + segments.slice(0, -1).join('|');
    return /^\d+$/.test(tree) ? tree + '/0' : tree;
  }
  const treeOf = (address) => address.split('/')[0];
  const sideRank = (address) => {
    const label = address.slice(address.lastIndexOf('|') + 1);
    return [...label].reduce((acc, ch) => acc * 26 + ch.charCodeAt(0) - 96, 0);
  };
  const cssVar = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

  // ------------------------------------------------------------------ state
  const state = {
    overview: null,
    nodes: [],            // every Card summary, for link completion and the sky
    fingerprint: '',
    pointer: null,        // Card address, or null for ROOT
    card: null,
    tree: null,
    mode: store.get('zt-mode') === 'constellation' ? 'constellation' : 'map',
    editing: false,
  };

  // --------------------------------------------------------------- commands
  // Same vocabulary and availability as the terminal Session.
  const COMMANDS = [
    ['go [<target>]', 'go to a Location or Citation key; bare go returns to ROOT', 'always'],
    ['root', 'return to ROOT', 'always'],
    ['up', 'go to the parent card; a tree root goes to ROOT', 'card'],
    ['ls', 'list Topics and Literature at ROOT, or the current tree', 'always'],
    ['t <title>', 'create a Topic', 'always'],
    ['l', 'create a Literature Card from one BibTeX entry', 'always'],
    ['n', 'create the direct successor', 'card'],
    ['b', 'create the next side successor', 'regular'],
    ['e', 'edit the current card', 'card'],
    ['del', 'delete the current card and its successors', 'card'],
    ['mv <new-location>', 'move the current card and its successors', 'regular'],
    ['stats', 'count cards by kind', 'always'],
    ['status', 'show service status', 'always'],
    ['lsbk', 'list broken links', 'always'],
    ['help', 'show the commands available here', 'always'],
    ['q', 'stop zt gui', 'always'],
  ];
  const context = () => (!state.card ? 'root' : state.card.context === 'tree-root' ? 'tree-root' : 'regular');
  const available = (scope) =>
    scope === 'always' || (scope === 'card' && context() !== 'root') || (scope === 'regular' && context() === 'regular');

  async function runCommand(line) {
    const parts = line.trim().split(/\s+/).filter(Boolean);
    const [name, ...rest] = parts;
    if (!name) return;
    const needCard = () => {
      if (!state.card) throw new Error(`${name} needs a Card; use go <target> first`);
    };
    switch (name) {
      case 'q': return quit();
      case 'root': return go(null);
      case 'go':
        if (rest.length > 1) throw new Error('usage: go [<target>]');
        if (!rest.length) return go(null);
        if (!state.nodes.some((node) => node.address === rest[0])) {
          await refresh();
          if (!state.nodes.some((node) => node.address === rest[0])) {
            throw new Error(`target \`${rest[0]}\` does not exist`);
          }
        }
        return go(rest[0]);
      case 'up':
        if (rest.length) throw new Error('usage: up');
        return state.pointer ? go(parentOf(state.pointer)) : undefined;
      case 'ls':
        if (!state.pointer) { openRail(true); return; }
        setMode('map'); return fitView();
      case 't': return newCard('topic', rest.join(' '));
      case 'l': return newLiterature();
      case 'n': needCard(); return newCard('direct');
      case 'b':
        needCard();
        if (context() !== 'regular') throw new Error(`side successors cannot start from a ${state.card.kind === 'literature' ? 'Literature' : 'Topic'} Card`);
        return newCard('side');
      case 'e': needCard(); return editCard();
      case 'del': needCard(); return deleteCard();
      case 'mv':
        needCard();
        if (rest.length !== 1) throw new Error('usage: mv <new-location>');
        return moveCard(rest[0]);
      case 'stats': {
        const s = state.overview.stats;
        return toast(`total: ${s.total}\ntopics: ${s.topics}\nregular: ${s.regular}\nliterature: ${s.literature}`);
      }
      case 'status':
        return toast(`state: up\narchive_root: ${state.overview.archive_root}\ncards: ${state.overview.stats.total}`);
      case 'lsbk': return showBroken();
      case 'help': return showHelp();
      default:
        throw new Error(`unknown session command: ${name}`);
    }
  }

  // --------------------------------------------------------------- routing
  function go(address) {
    const hash = address ? '#/card/' + enc(address) : '#/';
    if (location.hash === hash) route();
    else location.hash = hash;
  }

  function route() {
    const match = location.hash.match(/^#\/card\/(.+)$/);
    const pointer = match ? decodeURIComponent(match[1]) : null;
    state.pointer = pointer;
    loadPointer().catch(fail);
  }

  async function loadPointer() {
    if (!state.pointer) {
      state.card = null;
      renderAll();
      return;
    }
    try {
      state.card = await api('card?at=' + enc(state.pointer));
    } catch (error) {
      // Another client removed this Card: fall back to its nearest ancestor.
      const known = new Set(state.nodes.map((node) => node.address));
      let ancestor = parentOf(state.pointer);
      while (ancestor && !known.has(ancestor)) ancestor = parentOf(ancestor);
      toast(`card \`${state.pointer}\` no longer exists`, 'err');
      go(ancestor);
      return;
    }
    const treeId = state.card.tree;
    if (!state.tree || state.tree.tree !== treeId) {
      const sameTree = state.lastTreeId === treeId;
      state.tree = await api('tree?id=' + enc(treeId));
      state.lastTreeId = treeId;
      layoutMap(!sameTree);
    }
    renderAll();
  }

  /** Reload everything, e.g. after a change here or from the CLI. */
  async function refresh() {
    const [overview, graph] = await Promise.all([api('overview'), api('graph')]);
    state.overview = overview;
    state.fingerprint = overview.fingerprint;
    state.nodes = graph.nodes;
    state.tree = null;
    sky.invalidate();
    renderRail();
    renderTopbar();
    await loadPointer();
  }

  // ------------------------------------------------------------------- rail
  function renderRail() {
    const trees = state.overview.trees;
    const drawer = (tree) => h('button', {
      class: 'drawer ' + tree.kind + (state.card && state.card.tree === tree.tree ? ' on' : ''),
      title: tree.title,
      onclick: () => { go(tree.root); openRail(false); },
    },
      h('span', { class: 'addr', text: tree.root }),
      h('span', { class: 't', text: tree.title || '(untitled)' }),
      h('span', { class: 'n', text: String(tree.count) }),
      tree.excerpt ? h('span', { class: 'x', text: tree.excerpt }) : null,
    );
    const fill = (id, kind, hint) => {
      const list = trees.filter((tree) => tree.kind === kind);
      $(id).replaceChildren(...(list.length ? list.map(drawer) : [h('div', { class: 'empty-hint', text: hint })]));
    };
    fill('#topic-drawers', 'topic', 'No Topics yet — press t.');
    fill('#literature-drawers', 'literature', 'No Literature yet — press l.');
    $('#archive-root').textContent = state.overview.archive_root;
  }

  function openRail(open) {
    $('#app').classList.toggle('rail-open', open ?? !$('#app').classList.contains('rail-open'));
  }

  const runSearch = debounce(async (query) => {
    const box = $('#search-results');
    if (!query.trim()) { box.hidden = true; return; }
    try {
      const { results } = await api('search?q=' + enc(query));
      box.hidden = false;
      const lower = query.trim().toLowerCase();
      const marked = (text) => {
        const at = text.toLowerCase().indexOf(lower);
        if (at < 0) return [text];
        return [text.slice(0, at), h('mark', { text: text.slice(at, at + lower.length) }), text.slice(at + lower.length)];
      };
      box.replaceChildren(
        ...(results.length ? results.map((hit) => h('button', {
          class: 'hit', onclick: () => { go(hit.address); openRail(false); },
        },
          h('span', { class: 'top' }, h('span', { class: 'addr', text: hit.address }), h('span', { class: 't' }, marked(hit.title || '(untitled)'))),
          h('span', { class: 's' }, marked(hit.snippet)),
        )) : [h('div', { class: 'empty-hint', text: 'No matching cards.' })]),
      );
    } catch (error) { fail(error); }
  }, 140);

  // ----------------------------------------------------------------- topbar
  function renderTopbar() {
    const stats = state.overview.stats;
    $('#stats').textContent = `${stats.total} cards · ${stats.topics} topics · ${stats.literature} literature`;
    const pill = $('#broken-pill');
    pill.hidden = !state.overview.broken;
    pill.textContent = `${state.overview.broken} broken`;
    const trail = [h('a', { href: '#/', text: 'ROOT' })];
    if (state.pointer) {
      const chain = [];
      for (let at = state.pointer; at; at = parentOf(at)) chain.unshift(at);
      const shown = chain.length > 5 ? [chain[0], '…', ...chain.slice(-3)] : chain;
      for (const at of shown) {
        trail.push(h('span', { class: 'sep', text: '›' }));
        if (at === '…') trail.push(h('span', { class: 'sep', text: '…' }));
        else if (at === state.pointer) trail.push(h('span', { class: 'cur', text: at }));
        else trail.push(h('a', { href: '#/card/' + enc(at), text: at }));
      }
    }
    $('#trail').replaceChildren(...trail);
  }

  // ------------------------------------------------------------------ stage
  const camera = { map: { x: 40, y: 70, k: 1 }, constellation: { x: 0, y: 0, k: 1 } };

  function setMode(mode) {
    state.mode = mode;
    store.set('zt-mode', mode);
    for (const button of document.querySelectorAll('.seg button')) {
      button.classList.toggle('on', button.dataset.mode === mode);
    }
    renderStage();
  }

  function renderStage() {
    if (!state.overview) return;
    const inTree = state.mode === 'map' && state.card;
    $('#desk').hidden = !(state.mode === 'map' && !state.card);
    $('#plane').hidden = !inTree;
    $('#sky').hidden = state.mode !== 'constellation';
    $('#sky-label').hidden = true;
    $('#legend').hidden = !inTree;
    $('.zoom').hidden = state.mode === 'map' && !state.card;
    const title = $('#stage-title');
    if (state.mode === 'constellation') {
      title.textContent = `Constellation · ${state.nodes.length} cards`;
      sky.show();
    } else if (!state.card) {
      title.textContent = 'The cabinet';
      renderDesk();
    } else {
      const tree = state.tree;
      title.textContent = `${tree.kind === 'literature' ? 'Literature' : 'Topic'} ${tree.tree} · ${tree.title} — ${tree.cards.length} card${tree.cards.length === 1 ? '' : 's'}`;
      highlightMap();
    }
  }

  function renderDesk() {
    const trees = state.overview.trees;
    const stack = (tree) => h('button', {
      class: 'stack ' + tree.kind, onclick: () => go(tree.root), title: tree.title,
    },
      tree.count > 2 ? h('span', { class: 'sheet s3' }) : null,
      tree.count > 1 ? h('span', { class: 'sheet s2' }) : null,
      h('span', { class: 'face' },
        h('span', { class: 'kind', text: `${tree.kind === 'literature' ? 'Literature' : 'Topic'} · ${tree.root}` }),
        h('span', { class: 't', text: tree.title || '(untitled)' }),
        tree.excerpt ? h('span', { class: 'x', text: tree.excerpt }) : null,
        h('span', { class: 'foot' }, h('span', { text: `${tree.count} card${tree.count === 1 ? '' : 's'}` }), h('span', { class: 'addr', text: '→' })),
      ),
    );
    const newStack = (label, key, action) => h('button', { class: 'stack new', onclick: action },
      h('span', { class: 'face' }, h('span', { class: 'plus', text: '+' }), h('span', { text: label }), h('kbd', { text: key })));
    $('#desk').replaceChildren(
      h('div', { class: 'desk-intro' },
        h('h1', { text: trees.length ? 'Your slip-box' : 'An empty slip-box' }),
        h('p', { text: trees.length
          ? 'Every Topic and Literature Card roots its own tree. Open a stack to walk its cards.'
          : 'Start a Topic for your own ideas, or a Literature Card for a work you are reading.' }),
      ),
      h('div', { class: 'desk-grid' },
        trees.map(stack),
        newStack('New Topic', 't', () => newCard('topic', '')),
        newStack('New Literature', 'l', () => newLiterature()),
      ),
    );
  }

  // Folgezettel layout: a Direct successor continues its column downward; each
  // Side successor starts a new column to the right, one row below its parent.
  function layoutMap(resetCamera) {
    const tree = state.tree;
    const byAddress = new Map(tree.cards.map((card) => [card.address, card]));
    const children = new Map();
    for (const card of tree.cards) {
      if (!card.parent || !byAddress.has(card.parent)) continue;
      const entry = children.get(card.parent) || { direct: null, sides: [] };
      if (card.edge === 'side') entry.sides.push(card.address);
      else entry.direct = card.address;
      children.set(card.parent, entry);
    }
    for (const entry of children.values()) entry.sides.sort((a, b) => sideRank(a) - sideRank(b));
    const position = new Map();
    let nextColumn = 0;
    const spine = (start, column, row) => {
      const chain = [];
      for (let at = start, r = row; at; at = children.get(at)?.direct, r += 1) {
        position.set(at, { col: column, row: r });
        chain.push(at);
      }
      for (const at of chain) {
        for (const side of children.get(at)?.sides || []) {
          nextColumn += 1;
          spine(side, nextColumn, position.get(at).row + 1);
        }
      }
    };
    spine(tree.root, 0, 0);
    for (const card of tree.cards) {
      if (!position.has(card.address)) { nextColumn += 1; spine(card.address, nextColumn, 0); }
    }
    const xy = (address) => {
      const p = position.get(address);
      return { x: p.col * COL_W, y: p.row * ROW_H };
    };
    tree.xy = xy;

    const edges = $('#edges');
    edges.replaceChildren();
    const nodes = $('#nodes');
    nodes.replaceChildren();
    let maxX = 0;
    let maxY = 0;
    for (const card of tree.cards) {
      const { x, y } = xy(card.address);
      maxX = Math.max(maxX, x + NODE_W);
      maxY = Math.max(maxY, y + NODE_H);
      if (card.parent && byAddress.has(card.parent)) {
        const p = xy(card.parent);
        const cx = x + NODE_W / 2;
        const d = card.edge === 'side'
          ? `M ${p.x + NODE_W} ${p.y + NODE_H / 2} H ${cx - 14} Q ${cx} ${p.y + NODE_H / 2} ${cx} ${p.y + NODE_H / 2 + 14} V ${y}`
          : `M ${p.x + NODE_W / 2} ${p.y + NODE_H} V ${y}`;
        edges.append(svg('path', { d, class: card.edge === 'side' ? 'e-side' : 'e-direct' }));
      }
      for (const target of card.links) {
        if (!byAddress.has(target) || target === card.address) continue;
        const t = xy(target);
        const ax = x + NODE_W / 2; const ay = y + NODE_H / 2;
        const bx = t.x + NODE_W / 2; const by = t.y + NODE_H / 2;
        const mx = (ax + bx) / 2; const my = (ay + by) / 2;
        const bend = 0.28;
        const cx = mx - (by - ay) * bend; const cy = my + (bx - ax) * bend;
        edges.append(svg('path', { d: `M ${ax} ${ay} Q ${cx} ${cy} ${bx} ${by}`, class: 'e-link', 'data-a': card.address, 'data-b': target }));
      }
      const external = card.links.filter((target) => !byAddress.has(target)).length;
      const node = h('button', {
        class: 'node' + (card.edge === 'root' ? ' root ' + card.kind : ''),
        dataset: { address: card.address },
        title: card.excerpt || card.title,
        onclick: (event) => { event.stopPropagation(); go(card.address); },
      },
        h('span', { class: 'top' },
          h('span', { class: 'addr', text: card.address }),
          h('span', { class: 'badges' },
            card.inbound ? h('span', { class: 'badge in', title: 'Referred by', text: '←' + card.inbound }) : null,
            external ? h('span', { class: 'badge', title: 'Links to other trees', text: '↗' + external }) : null,
          ),
        ),
        h('span', { class: 't' + (card.title ? '' : ' untitled'), text: card.title || 'untitled' }),
      );
      node.style.left = x + 'px';
      node.style.top = y + 'px';
      nodes.append(node);
    }
    edges.setAttribute('width', maxX + 40);
    edges.setAttribute('height', maxY + 40);
    tree.bounds = { w: maxX, h: maxY };
    if (resetCamera) requestAnimationFrame(() => fitMap(true));
  }

  function highlightMap() {
    const card = state.card;
    if (!card || !state.tree) return;
    const linked = new Set([...card.outbound.map((l) => l.address), ...card.inbound.map((l) => l.address)]);
    for (const node of document.querySelectorAll('.node')) {
      const address = node.dataset.address;
      node.classList.toggle('on', address === card.address);
      node.classList.toggle('linked', linked.has(address));
    }
    for (const path of document.querySelectorAll('.e-link')) {
      path.classList.toggle('hot', path.dataset.a === card.address || path.dataset.b === card.address);
    }
    revealPointer();
  }

  function applyCamera() {
    const cam = camera.map;
    $('#plane').style.transform = `translate(${cam.x}px, ${cam.y}px) scale(${cam.k})`;
    if (state.mode === 'constellation') sky.draw();
  }

  function fitView() {
    if (state.mode === 'constellation') sky.fit();
    else fitMap(false);
  }

  function fitMap(preferPointer) {
    const viewport = $('#viewport');
    const vw = viewport.clientWidth; const vh = viewport.clientHeight;
    const tree = state.tree;
    if (!tree || !tree.bounds) return;
    const pad = 60;
    const k = Math.max(0.3, Math.min(1.05, (vw - pad * 2) / tree.bounds.w, (vh - pad * 2 - 40) / tree.bounds.h));
    const cam = camera.map;
    cam.k = k;
    cam.x = Math.max(pad, (vw - tree.bounds.w * k) / 2);
    cam.y = Math.max(pad + 16, (vh - tree.bounds.h * k) / 2);
    applyCamera();
    if (preferPointer) revealPointer();
  }

  function revealPointer() {
    const tree = state.tree;
    if (!tree || !tree.xy || !state.card || state.mode !== 'map') return;
    const viewport = $('#viewport');
    const cam = camera.map;
    const { x, y } = tree.xy(state.card.address);
    const sx = cam.x + x * cam.k; const sy = cam.y + y * cam.k;
    const margin = 60;
    const w = NODE_W * cam.k; const hh = NODE_H * cam.k;
    let dx = 0; let dy = 0;
    if (sx < margin) dx = margin - sx;
    else if (sx + w > viewport.clientWidth - margin) dx = viewport.clientWidth - margin - (sx + w);
    if (sy < margin + 20) dy = margin + 20 - sy;
    else if (sy + hh > viewport.clientHeight - margin) dy = viewport.clientHeight - margin - (sy + hh);
    if (!dx && !dy) return;
    const start = { x: cam.x, y: cam.y };
    const t0 = performance.now();
    const step = (now) => {
      const t = Math.min(1, (now - t0) / 260);
      const ease = 1 - Math.pow(1 - t, 3);
      cam.x = start.x + dx * ease; cam.y = start.y + dy * ease;
      applyCamera();
      if (t < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  function zoomAt(factor, px, py) {
    if (state.mode === 'constellation') { sky.zoom(factor, px, py); return; }
    const cam = camera.map;
    const k = Math.max(0.2, Math.min(2.5, cam.k * factor));
    cam.x = px - (px - cam.x) * (k / cam.k);
    cam.y = py - (py - cam.y) * (k / cam.k);
    cam.k = k;
    applyCamera();
  }

  function installPanZoom() {
    const viewport = $('#viewport');
    let drag = null;
    viewport.addEventListener('pointerdown', (event) => {
      if (event.button !== 0 || event.target.closest('.node, .stack, .desk')) return;
      drag = { x: event.clientX, y: event.clientY, moved: false };
      viewport.setPointerCapture(event.pointerId);
      viewport.classList.add('dragging');
    });
    viewport.addEventListener('pointermove', (event) => {
      if (!drag) { if (state.mode === 'constellation') sky.hover(event); return; }
      const dx = event.clientX - drag.x; const dy = event.clientY - drag.y;
      if (Math.abs(dx) + Math.abs(dy) > 3) drag.moved = true;
      drag.x = event.clientX; drag.y = event.clientY;
      if (state.mode === 'constellation') sky.pan(dx, dy);
      else { camera.map.x += dx; camera.map.y += dy; applyCamera(); }
    });
    const end = (event) => {
      if (drag && !drag.moved && state.mode === 'constellation') sky.click(event);
      drag = null;
      viewport.classList.remove('dragging');
    };
    viewport.addEventListener('pointerup', end);
    viewport.addEventListener('pointercancel', () => { drag = null; viewport.classList.remove('dragging'); });
    viewport.addEventListener('wheel', (event) => {
      if (state.mode === 'map' && !state.card) return; // the desk scrolls normally
      event.preventDefault();
      const rect = viewport.getBoundingClientRect();
      if (event.ctrlKey || event.metaKey || state.mode === 'constellation') {
        zoomAt(Math.exp(-event.deltaY * 0.0022), event.clientX - rect.left, event.clientY - rect.top);
      } else {
        camera.map.x -= event.deltaX; camera.map.y -= event.deltaY; applyCamera();
      }
    }, { passive: false });
    for (const button of document.querySelectorAll('[data-zoom]')) {
      button.addEventListener('click', () => {
        const kind = button.dataset.zoom;
        if (kind === 'fit') return fitView();
        zoomAt(kind === 'in' ? 1.2 : 1 / 1.2, viewport.clientWidth / 2, viewport.clientHeight / 2);
      });
    }
  }

  // ---------------------------------------------------------- constellation
  const sky = (() => {
    let nodes = null;
    let byAddress = new Map();
    let edges = [];
    let alpha = 0;
    let running = false;
    let hovered = null;
    const cam = camera.constellation;
    const canvas = () => $('#sky');

    const hue = (tree) => {
      const index = state.overview ? state.overview.trees.findIndex((entry) => entry.tree === tree) : -1;
      if (index >= 0) return (24 + index * 137.508) % 360;
      let hash = 0;
      for (const ch of tree) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
      return hash % 360;
    };
    let fitted = false;
    let userMoved = false;

    function build() {
      const previous = byAddress;
      nodes = state.nodes.map((node, index) => {
        const old = previous.get(node.address);
        const angle = index * 2.399963;
        const radius = 12 * Math.sqrt(index + 1);
        return {
          ...node,
          x: old ? old.x : Math.cos(angle) * radius,
          y: old ? old.y : Math.sin(angle) * radius,
          vx: 0, vy: 0,
          hue: hue(node.tree),
          r: node.edge === 'root' ? 7 : 3.6 + Math.min(node.inbound, 6) * 0.7,
        };
      });
      byAddress = new Map(nodes.map((node) => [node.address, node]));
      edges = [];
      for (const node of nodes) {
        if (node.parent && byAddress.has(node.parent)) edges.push({ a: byAddress.get(node.parent), b: node, kind: 'tree' });
        for (const target of node.links) {
          if (byAddress.has(target) && target !== node.address) edges.push({ a: node, b: byAddress.get(target), kind: 'link' });
        }
      }
      alpha = previous.size ? 0.4 : 1;
    }

    function tick() {
      const list = nodes;
      for (let i = 0; i < list.length; i += 1) {
        const a = list[i];
        for (let j = i + 1; j < list.length; j += 1) {
          const b = list[j];
          let dx = a.x - b.x; let dy = a.y - b.y;
          let d2 = dx * dx + dy * dy;
          if (d2 < 0.01) { dx = Math.random() - 0.5; dy = Math.random() - 0.5; d2 = 0.5; }
          if (d2 > 90000) continue;
          const force = (a.tree === b.tree ? 260 : 700) / d2 * alpha;
          a.vx += dx * force; a.vy += dy * force;
          b.vx -= dx * force; b.vy -= dy * force;
        }
      }
      for (const edge of edges) {
        const length = edge.kind === 'tree' ? 34 : 120;
        const strength = edge.kind === 'tree' ? 0.12 : 0.02;
        const dx = edge.b.x - edge.a.x; const dy = edge.b.y - edge.a.y;
        const d = Math.sqrt(dx * dx + dy * dy) || 1;
        const pull = (d - length) / d * strength * alpha;
        edge.a.vx += dx * pull; edge.a.vy += dy * pull;
        edge.b.vx -= dx * pull; edge.b.vy -= dy * pull;
      }
      for (const node of list) {
        node.vx -= node.x * 0.01 * alpha; node.vy -= node.y * 0.01 * alpha;
        node.vx *= 0.82; node.vy *= 0.82;
        node.x += node.vx; node.y += node.vy;
      }
      alpha *= 0.985;
    }

    function loop() {
      if (state.mode !== 'constellation' || alpha < 0.01) {
        running = false;
        if (state.mode === 'constellation' && !userMoved) { fitted = true; self.fit(); }
        draw();
        return;
      }
      if (!fitted && alpha < 0.3) { fitted = true; self.fit(); }
      tick(); tick();
      draw();
      requestAnimationFrame(loop);
    }

    function resize() {
      const c = canvas();
      const dpr = window.devicePixelRatio || 1;
      const w = c.clientWidth; const hh = c.clientHeight;
      if (c.width !== Math.round(w * dpr) || c.height !== Math.round(hh * dpr)) {
        c.width = Math.round(w * dpr); c.height = Math.round(hh * dpr);
      }
      return { w, h: hh, dpr };
    }

    function draw() {
      if (!nodes || state.mode !== 'constellation') return;
      const c = canvas();
      const { w, h: hh, dpr } = resize();
      const ctx = c.getContext('2d');
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.clearRect(0, 0, c.width, c.height);
      ctx.setTransform(dpr * cam.k, 0, 0, dpr * cam.k, dpr * (w / 2 + cam.x), dpr * (hh / 2 + cam.y));
      const dark = document.documentElement.dataset.theme === 'dark'
        || (!document.documentElement.dataset.theme && matchMedia('(prefers-color-scheme: dark)').matches);
      const light = dark ? 62 : 42;
      const pointer = state.pointer;
      const focus = hovered || (pointer && byAddress.get(pointer));
      for (const edge of edges) {
        const hot = focus && (edge.a === focus || edge.b === focus);
        ctx.beginPath();
        ctx.moveTo(edge.a.x, edge.a.y);
        if (edge.kind === 'link') {
          const mx = (edge.a.x + edge.b.x) / 2 - (edge.b.y - edge.a.y) * 0.2;
          const my = (edge.a.y + edge.b.y) / 2 + (edge.b.x - edge.a.x) * 0.2;
          ctx.quadraticCurveTo(mx, my, edge.b.x, edge.b.y);
          ctx.setLineDash([4 / cam.k, 4 / cam.k]);
          ctx.strokeStyle = cssVar('--link');
          ctx.globalAlpha = hot ? 0.95 : 0.35;
          ctx.lineWidth = (hot ? 1.8 : 1) / cam.k;
        } else {
          ctx.lineTo(edge.b.x, edge.b.y);
          ctx.setLineDash([]);
          ctx.strokeStyle = `hsl(${edge.b.hue} 45% ${light}%)`;
          ctx.globalAlpha = hot ? 0.9 : 0.4;
          ctx.lineWidth = 1.4 / cam.k;
        }
        ctx.stroke();
      }
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
      for (const node of nodes) {
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.r, 0, Math.PI * 2);
        ctx.fillStyle = `hsl(${node.hue} ${node.kind === 'literature' ? 62 : 55}% ${light}%)`;
        ctx.fill();
        if (node.edge === 'root') {
          ctx.lineWidth = 2 / cam.k;
          ctx.strokeStyle = cssVar('--paper');
          ctx.stroke();
        }
        if (node.address === pointer) {
          ctx.beginPath();
          ctx.arc(node.x, node.y, node.r + 5 / cam.k, 0, Math.PI * 2);
          ctx.lineWidth = 2.5 / cam.k;
          ctx.strokeStyle = cssVar('--pointer');
          ctx.stroke();
        }
      }
      ctx.font = `${12 / cam.k}px ${cssVar('--sans')}`;
      ctx.fillStyle = cssVar('--ink');
      ctx.textAlign = 'center';
      for (const node of nodes) {
        if (node.edge === 'root' || node.address === pointer || cam.k > 1.7) {
          const label = node.edge === 'root' ? node.title || node.address : node.address;
          ctx.globalAlpha = node.edge === 'root' ? 0.95 : 0.7;
          ctx.fillText(label.length > 28 ? label.slice(0, 27) + '…' : label, node.x, node.y - node.r - 5 / cam.k);
        }
      }
      ctx.globalAlpha = 1;
    }

    function screenToWorld(event) {
      const rect = canvas().getBoundingClientRect();
      return {
        x: (event.clientX - rect.left - rect.width / 2 - cam.x) / cam.k,
        y: (event.clientY - rect.top - rect.height / 2 - cam.y) / cam.k,
      };
    }

    function pick(event) {
      if (!nodes) return null;
      const p = screenToWorld(event);
      let best = null;
      let bestD = (12 / cam.k) ** 2;
      for (const node of nodes) {
        const d = (node.x - p.x) ** 2 + (node.y - p.y) ** 2;
        if (d < Math.max(bestD, node.r * node.r)) { best = node; bestD = d; }
      }
      return best;
    }

    const self = {
      invalidate() { if (nodes) build(); },
      show() {
        if (!nodes) build();
        if (!running) { running = true; requestAnimationFrame(loop); }
        draw();
      },
      draw,
      fit() {
        if (!nodes || !nodes.length) return;
        const xs = nodes.map((n) => n.x); const ys = nodes.map((n) => n.y);
        const w = Math.max(...xs) - Math.min(...xs) + 80;
        const hh = Math.max(...ys) - Math.min(...ys) + 80;
        const c = canvas();
        cam.k = Math.max(0.2, Math.min(2.2, c.clientWidth / w, c.clientHeight / hh));
        cam.x = -((Math.max(...xs) + Math.min(...xs)) / 2) * cam.k;
        cam.y = -((Math.max(...ys) + Math.min(...ys)) / 2) * cam.k;
        draw();
      },
      pan(dx, dy) { userMoved = true; cam.x += dx; cam.y += dy; draw(); },
      zoom(factor, px, py) {
        userMoved = true;
        const c = canvas();
        const ox = px - c.clientWidth / 2; const oy = py - c.clientHeight / 2;
        const k = Math.max(0.15, Math.min(5, cam.k * factor));
        cam.x = ox - (ox - cam.x) * (k / cam.k);
        cam.y = oy - (oy - cam.y) * (k / cam.k);
        cam.k = k;
        draw();
      },
      hover(event) {
        const node = pick(event);
        const label = $('#sky-label');
        if (node !== hovered) { hovered = node; draw(); }
        if (!node) { label.hidden = true; canvas().style.cursor = ''; return; }
        const rect = canvas().getBoundingClientRect();
        label.hidden = false;
        label.replaceChildren(h('span', { class: 'addr', text: node.address + '  ' }), node.title || '(untitled)');
        label.style.left = (rect.width / 2 + cam.x + node.x * cam.k) + 'px';
        label.style.top = (rect.height / 2 + cam.y + node.y * cam.k - node.r * cam.k) + 'px';
        canvas().style.cursor = 'pointer';
      },
      click(event) {
        const node = pick(event);
        if (node) go(node.address);
      },
    };
    return self;
  })();

  // ----------------------------------------------------------------- reader
  function chip(link, extraClass) {
    return h('button', {
      class: 'chip' + (link.broken ? ' broken' : '') + (extraClass ? ' ' + extraClass : ''),
      title: link.broken ? 'Broken link' : link.title,
      onclick: link.broken ? null : () => go(link.address),
    }, h('span', { class: 'addr', text: link.address }), h('span', { class: 't', text: link.broken ? 'missing' : link.title || '(untitled)' }));
  }

  function actionButton(label, key, handler, options = {}) {
    return h('button', {
      class: 'btn' + (options.primary ? ' primary' : '') + (options.danger ? ' danger' : ''),
      disabled: options.disabled,
      title: options.title,
      onclick: () => Promise.resolve().then(handler).catch(fail),
    }, label, key ? h('kbd', { text: key }) : null);
  }

  function renderReader() {
    const reader = $('#reader');
    const card = state.card;
    if (!card) {
      const stats = state.overview.stats;
      reader.replaceChildren(h('div', { class: 'welcome' },
        h('h2', { text: 'ROOT' }),
        h('p', { text: `${stats.total} cards in ${stats.topics} Topic trees and ${stats.literature} Literature trees. Pick a stack on the desk, a drawer in the cabinet, or type a command above.` }),
        h('dl', {}, [
          ['go <target>', 'open a Location or Citation key'],
          ['t <title>', 'start a Topic'],
          ['l', 'add a Literature Card from BibTeX'],
          [': or /', 'command bar · search'],
          ['↑ ↓ →', 'walk parent · direct · side in the map'],
          ['v', 'switch Map / Constellation'],
        ].flatMap(([key, text]) => [h('dt', { text: key }), h('dd', { text })]),
        ),
        h('div', { class: 'actions' },
          actionButton('New Topic', 't', () => newCard('topic', ''), { primary: true }),
          actionButton('New Literature', 'l', () => newLiterature()),
        ),
      ));
      return;
    }
    const kindLabel = { topic: 'Topic', literature: 'Literature', regular: 'Card' }[card.kind];
    const md = h('div', { class: 'md' });
    if (card.body.trim()) md.innerHTML = card.html;
    else md.append(h('p', { class: 'empty', text: 'No body yet — press e to write in Markdown.' }));
    wireMarkdown(md);
    const nav = (label, glyph, items) => h('div', { class: 'nav-cell' },
      h('span', { class: 'lbl' }, glyph, ' ', label),
      items.length
        ? items.map((item) => h('button', { onclick: () => go(item.address), title: item.title },
          h('span', { class: 'addr', text: item.address }), h('span', { class: 't', text: item.title || '(untitled)' })))
        : h('span', { class: 'none', text: '—' }));
    const regular = card.context === 'regular';
    reader.replaceChildren(h('article', { class: 'card' },
      h('div', { class: 'card-head' },
        h('span', { class: 'kind-tag ' + card.kind, text: kindLabel }),
        h('button', {
          class: 'stamp', title: 'Copy address', text: card.address,
          onclick: () => navigator.clipboard?.writeText(card.address).then(() => toast(`copied ${card.address}`), () => {}),
        }),
      ),
      h('h1', { class: 'title', text: card.title || '(untitled)' }),
      card.kind === 'literature' ? h('details', { class: 'biblio' },
        h('summary', { text: `BibTeX · ${card.citation_key}` }),
        h('pre', { text: card.bibtex })) : null,
      md,
      card.inbound.length ? [h('div', { class: 'section-label', text: `Referred by · ${card.inbound.length}` }), h('div', { class: 'chips' }, card.inbound.map((link) => chip(link)))] : null,
      card.outbound.length ? [h('div', { class: 'section-label', text: `Links out · ${card.outbound.length}` }), h('div', { class: 'chips' }, card.outbound.map((link) => chip(link)))] : null,
      h('div', { class: 'nav-grid' },
        nav('parent', '↑', card.parent ? [card.parent] : []),
        nav('direct', '↓', card.direct ? [card.direct] : []),
        nav('side', '→', card.sides),
      ),
      h('div', { class: 'actions' },
        actionButton('Edit', 'e', editCard, { primary: true }),
        actionButton('Direct', 'n', () => newCard('direct'), { title: 'New direct successor' }),
        actionButton('Side', 'b', () => newCard('side'), { disabled: !regular, title: regular ? 'New side successor' : 'Tree roots have no side successors' }),
        actionButton('Move', 'm', () => moveCard(), { disabled: !regular, title: regular ? 'Move with successors' : 'Tree roots cannot move' }),
        actionButton('Delete', 'del', deleteCard, { danger: true }),
      ),
      h('details', { class: 'raw-toggle' }, h('summary', { text: 'Stored Markdown' }), h('pre', { text: card.text })),
    ));
    reader.scrollTop = 0;
  }

  function wireMarkdown(root) {
    for (const anchor of root.querySelectorAll('a[href]')) {
      if (anchor.classList.contains('zt-link')) {
        anchor.addEventListener('click', (event) => { event.preventDefault(); go(anchor.dataset.target); });
      } else {
        anchor.target = '_blank';
        anchor.rel = 'noopener noreferrer';
      }
    }
  }

  function renderAll() {
    renderTopbar();
    renderRail();
    renderStage();
    renderReader();
  }

  // ---------------------------------------------------------------- dialogs
  const layers = [];

  /** Open a dialog layer; dialogs stack, so a confirmation can sit over the editor. */
  function openOverlay(dialog) {
    const overlay = $('#overlay');
    const layer = h('div', { class: 'layer' }, dialog);
    overlay.append(layer);
    overlay.hidden = false;
    layers.push(layer);
    state.editing = true;
    return () => {
      const index = layers.indexOf(layer);
      if (index < 0) return;
      layers.splice(index, 1);
      layer.remove();
      overlay.hidden = !layers.length;
      state.editing = layers.length > 0;
    };
  }

  /** Type-to-confirm, exactly like the CLI's `type ... to confirm` prompt. */
  function confirmPlan(title, lines, expected) {
    return new Promise((resolve) => {
      const input = h('input', { autocomplete: 'off', spellcheck: 'false', 'aria-label': 'Confirmation' });
      const ok = h('button', { class: 'btn primary', disabled: true, text: 'Confirm' });
      const cancel = h('button', { class: 'btn', text: 'Cancel' });
      input.addEventListener('input', () => { ok.disabled = input.value.trim() !== expected; });
      input.addEventListener('keydown', (event) => {
        if (event.key === 'Enter' && !ok.disabled) ok.click();
        if (event.key === 'Escape') cancel.click();
      });
      const close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: title })),
        h('div', { class: 'confirm-body' },
          h('pre', { text: lines.join('\n') }),
          h('p', {}, 'Type ', h('code', { text: expected }), ' to confirm:'),
          input,
        ),
        h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), cancel, ok),
      ));
      ok.addEventListener('click', () => { close(); resolve(true); });
      cancel.addEventListener('click', () => { close(); resolve(false); });
      input.focus();
    });
  }

  function promptValue(title, label, initial) {
    return new Promise((resolve) => {
      const input = h('input', { value: initial || '', autocomplete: 'off', spellcheck: 'false' });
      const ok = h('button', { class: 'btn primary', text: 'Continue' });
      const cancel = h('button', { class: 'btn', text: 'Cancel' });
      const close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: title })),
        h('div', { class: 'confirm-body field' }, h('p', { text: label }), input),
        h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), cancel, ok),
      ));
      input.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') ok.click();
        if (event.key === 'Escape') cancel.click();
      });
      ok.addEventListener('click', () => { close(); resolve(input.value.trim()); });
      cancel.addEventListener('click', () => { close(); resolve(null); });
      input.focus();
      input.select();
    });
  }

  function listDialog(title, rows) {
    let close = null;
    const content = typeof rows === 'function' ? rows(() => close()) : rows;
    close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog' },
      h('div', { class: 'dlg-head' }, h('h3', { text: title })),
      h('div', { class: 'confirm-body' }, content),
      h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), h('button', { class: 'btn primary', text: 'Close', onclick: () => close() })),
    ));
  }

  async function showBroken() {
    const { broken } = await api('broken');
    listDialog(`Broken links · ${broken.length}`, (close) => (broken.length
      ? h('div', { class: 'chips' }, broken.map((item) => h('button', {
        class: 'chip broken', title: item.line,
        onclick: () => { close(); go(item.source); },
      }, h('span', { class: 'addr', text: item.source }), h('span', { class: 't', text: `${item.title} → ${item.target}` }))))
      : h('p', { text: 'no broken links' })));
  }

  function showHelp() {
    const rows = COMMANDS.filter(([, , scope]) => available(scope));
    listDialog('Commands', [
      h('p', { text: 'The command bar speaks the Session’s language:' }),
      h('pre', { text: rows.map(([syntax, desc]) => `${syntax.padEnd(20)} ${desc}`).join('\n') }),
      h('p', { text: 'Keys: : command · / search · g go · n b e m · Del delete · u up · ↑↓→ walk · v view · f fit · Esc close' }),
    ]);
  }

  // ----------------------------------------------------------- the editor
  /**
   * The writing desk: raw Markdown on the left, rendered on the right.
   * `save(text)` persists and returns the address to open afterwards.
   */
  function openEditor({ heading, initial, literature, bibtex, save, bibtexCheck }) {
    return new Promise((resolve) => {
      const text = h('textarea', { spellcheck: 'true', 'aria-label': 'Card Markdown' });
      text.value = initial;
      const bib = bibtex !== undefined
        ? h('textarea', { class: 'bib', spellcheck: 'false', 'aria-label': 'BibTeX', placeholder: '@book{Smith2024,\n  title = {Example Book}\n}' })
        : null;
      if (bib) bib.value = bibtex;
      const preview = h('div', { class: 'preview md' });
      const msg = h('span', { class: 'msg' });
      const saveBtn = h('button', { class: 'btn primary' }, 'Save', h('kbd', { text: 'Ctrl S' }));
      const cancelBtn = h('button', { class: 'btn', text: 'Cancel' });
      const completion = h('div', { class: 'command-menu', hidden: true });
      let bibTitle = null;
      let dirty = false;

      const setMsg = (message, kind) => { msg.textContent = message || ''; msg.className = 'msg' + (kind ? ' ' + kind : ''); };
      const update = debounce(async () => {
        try {
          if (bib) {
            if (bib.value.trim()) {
              try {
                const checked = await bibtexCheck(bib.value);
                bibTitle = checked.title;
                setMsg(checked.note || `Citation key ${checked.citation_key}`, checked.warn ? 'err' : 'ok');
              } catch (error) {
                bibTitle = null;
                setMsg(error.message, 'err');
              }
            } else { bibTitle = null; setMsg('Paste or type one BibTeX entry.'); }
          }
          const rendered = await api('render', { text: text.value, literature: !!literature });
          const title = literature ? (bibTitle ?? literature.title ?? '') : rendered.title;
          preview.innerHTML = rendered.html;
          preview.prepend(h('h1', { class: 'title' + (title ? '' : ' untitled'), text: title || 'Untitled — start with “# Title”' }));
          wireMarkdown(preview);
          if (!bib || !msg.classList.contains('err')) setMsg(rendered.error || (literature ? 'The title comes from BibTeX.' : ''), rendered.error ? 'err' : null);
        } catch (error) { setMsg(error.message, 'err'); }
      }, 160);

      const close = openOverlay(h('div', { class: 'sheet-dlg', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: heading })),
        h('div', { class: 'dlg-body' },
          h('div', { class: 'pane' },
            bib ? h('div', { class: 'pane-label' }, h('span', { text: 'BibTeX metadata' })) : null,
            bib,
            h('div', { class: 'pane-label' }, h('span', { text: 'Markdown' }), h('span', { text: '[[ links · text below the reverse-links marker is generated' })),
            text,
          ),
          h('div', { class: 'pane' }, h('div', { class: 'pane-label' }, h('span', { text: 'Preview' })), preview),
        ),
        h('div', { class: 'dlg-foot' }, msg, cancelBtn, saveBtn),
        completion,
      ));

      const finish = (address) => { close(); resolve(address); };
      const doSave = async () => {
        saveBtn.disabled = true;
        try {
          const address = await save(text.value, bib ? bib.value : undefined);
          if (address !== undefined) finish(address);
        } catch (error) {
          setMsg(error.message, 'err');
        } finally { saveBtn.disabled = false; }
      };
      const doCancel = () => {
        if (dirty && !window.confirm('Discard your changes to this card?')) return;
        finish(null);
      };
      saveBtn.addEventListener('click', doSave);
      cancelBtn.addEventListener('click', doCancel);

      // [[ link completion
      let options = [];
      let selected = 0;
      const hideCompletion = () => { completion.hidden = true; options = []; };
      const showCompletion = () => {
        const before = text.value.slice(0, text.selectionStart);
        const match = before.match(/\[\[([^\]\s]*)$/);
        if (!match) return hideCompletion();
        const query = match[1].toLowerCase();
        options = state.nodes
          .filter((node) => node.address.toLowerCase().includes(query) || (node.title || '').toLowerCase().includes(query))
          .slice(0, 8);
        if (!options.length) return hideCompletion();
        selected = Math.min(selected, options.length - 1);
        completion.replaceChildren(h('div', { class: 'group', text: 'Link to card' }), ...options.map((node, index) => h('button', {
          class: index === selected ? 'on' : '',
          onmousedown: (event) => { event.preventDefault(); accept(node, match[1].length); },
        }, h('span', { class: 'syntax', text: node.address }), h('span', { class: 'desc', text: node.title || '(untitled)' }))));
        const rect = text.getBoundingClientRect();
        completion.style.position = 'fixed';
        completion.style.left = rect.left + 16 + 'px';
        completion.style.top = 'auto';
        completion.style.bottom = (window.innerHeight - rect.bottom + 12) + 'px';
        completion.style.width = '340px';
        completion.style.right = 'auto';
        completion.hidden = false;
      };
      const accept = (node, typed) => {
        const at = text.selectionStart;
        const after = text.value.slice(at);
        const closing = after.startsWith(']]') ? '' : ']]';
        text.setRangeText(node.address + closing, at - typed, at, 'end');
        if (!closing) text.selectionStart = text.selectionEnd = at - typed + node.address.length + 2;
        hideCompletion();
        dirty = true;
        update();
      };

      text.addEventListener('input', () => { dirty = true; selected = 0; showCompletion(); update(); });
      text.addEventListener('blur', () => setTimeout(hideCompletion, 120));
      if (bib) bib.addEventListener('input', () => { dirty = true; update(); });
      const keys = (event) => {
        if (!completion.hidden && event.target === text) {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault();
            selected = (selected + (event.key === 'ArrowDown' ? 1 : options.length - 1)) % options.length;
            showCompletion();
            return;
          }
          if (event.key === 'Enter' || event.key === 'Tab') {
            event.preventDefault();
            const match = text.value.slice(0, text.selectionStart).match(/\[\[([^\]\s]*)$/);
            accept(options[selected], match ? match[1].length : 0);
            return;
          }
          if (event.key === 'Escape') { event.preventDefault(); hideCompletion(); return; }
        }
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') { event.preventDefault(); doSave(); }
        else if (event.key === 'Escape') { event.preventDefault(); doCancel(); }
        else if (event.key === 'Tab' && event.target.tagName === 'TEXTAREA' && !event.shiftKey) {
          event.preventDefault();
          event.target.setRangeText('  ', event.target.selectionStart, event.target.selectionEnd, 'end');
        }
      };
      text.addEventListener('keydown', keys);
      if (bib) bib.addEventListener('keydown', keys);
      update();
      (bib && !bib.value ? bib : text).focus();
      if (!bib) {
        // Put the caret after the title heading, ready to type.
        const firstLine = text.value.indexOf('\n');
        const caret = text.value.startsWith('# \n') ? 2 : firstLine < 0 ? text.value.length : Math.min(text.value.length, firstLine + 2);
        text.setSelectionRange(caret, caret);
      }
    });
  }

  // ------------------------------------------------------------- mutations
  async function afterChange(address, message) {
    if (message) toast(message);
    await refresh();
    go(address);
  }

  async function newCard(kind, title) {
    if (kind !== 'topic' && !state.card) throw new Error(`${kind === 'side' ? 'b' : 'n'} needs a Card; use go <target> first`);
    const parent = state.card;
    if (kind === 'direct' && parent.direct) throw new Error(`direct successor already exists: ${parent.direct.address}`);
    if (kind === 'side' && parent.context !== 'regular') {
      throw new Error(`side successors cannot start from a ${parent.kind === 'literature' ? 'Literature' : 'Topic'} Card`);
    }
    const heading = kind === 'topic' ? 'New Topic'
      : kind === 'direct' ? `New direct successor of ${parent.address}` : `New side successor of ${parent.address}`;
    // Same template as the terminal: heading, blank line, body line, marker.
    const initial = `# ${kind === 'topic' ? title || '' : ''}\n\n\n<!-- zt:reverse-links -->\n`;
    const address = await openEditor({
      heading,
      initial,
      save: (text) => api('create', { kind, at: parent?.address, text }).then((result) => result.address),
    });
    if (address) await afterChange(address, `created ${address}`);
  }

  async function newLiterature() {
    const address = await openEditor({
      heading: 'New Literature Card',
      initial: '',
      literature: {},
      bibtex: '',
      bibtexCheck: async (bibtex) => {
        const checked = await api('literature/check', { bibtex });
        if (checked.exists) return { ...checked, warn: true, note: `Literature Card \`${checked.citation_key}\` already exists` };
        return { ...checked, note: `Citation key ${checked.citation_key} · “${checked.title}”` };
      },
      save: (text, bibtex) => api('literature', { bibtex, text }).then((result) => result.address),
    });
    if (address) await afterChange(address, `created ${address}`);
  }

  async function editCard() {
    const card = state.card;
    if (!card) throw new Error('e needs a Card; use go <target> first');
    if (card.kind === 'literature') {
      const part = await choosePart();
      if (part === 'metadata') return editMetadata(card);
      if (part !== 'text') return;
    }
    const address = await openEditor({
      heading: `Edit ${card.address}`,
      initial: card.text,
      literature: card.kind === 'literature' ? { title: card.title } : null,
      save: (text) => api('edit', { at: card.address, text, base: card.text }).then((result) => result.address),
    });
    if (address) await afterChange(address, `saved ${address}`);
  }

  function choosePart() {
    return new Promise((resolve) => {
      let open = true;
      const pick = (part) => { open = false; close(); resolve(part); };
      const close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: 'Edit Literature Card' })),
        h('div', { class: 'confirm-body' },
          h('p', { text: 'Which part do you want to edit?' }),
          h('div', { class: 'actions' },
            actionButton('Card text (Markdown)', 't', () => pick('text'), { primary: true }),
            actionButton('BibTeX metadata', 'm', () => pick('metadata')),
          ),
        ),
        h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), h('button', { class: 'btn', text: 'Cancel', onclick: () => pick(null) })),
      ));
      const onKey = (event) => {
        if (!open) { document.removeEventListener('keydown', onKey, true); return; }
        const key = event.key.toLowerCase();
        if (key === 't' || key === 'm' || key === 'escape') {
          event.preventDefault(); event.stopPropagation();
          document.removeEventListener('keydown', onKey, true);
          pick(key === 't' ? 'text' : key === 'm' ? 'metadata' : null);
        }
      };
      document.addEventListener('keydown', onKey, true);
    });
  }

  async function editMetadata(card) {
    const address = await openEditor({
      heading: `Edit metadata · ${card.address}`,
      initial: card.text,
      literature: { title: card.title },
      bibtex: card.bibtex,
      bibtexCheck: async (bibtex) => {
        const checked = await api('literature/check', { bibtex });
        if (checked.citation_key !== card.citation_key) {
          return { ...checked, warn: checked.exists, note: checked.exists
            ? `Citation key \`${checked.citation_key}\` conflicts with an existing Literature Card`
            : `Renames ${card.citation_key} → ${checked.citation_key} (the whole tree and every Link)` };
        }
        return { ...checked, note: `Citation key ${checked.citation_key}` };
      },
      save: async (_text, bibtex) => {
        let result = await api('metadata', { at: card.address, bibtex, base: card.text });
        if (result.confirm) {
          const ok = await confirmPlan('Rename Citation key', result.lines, result.confirm);
          if (!ok) return undefined;
          result = await api('metadata', { at: card.address, bibtex, base: card.text, confirm: result.confirm });
        }
        return result.address;
      },
    });
    if (address) await afterChange(address, `saved ${address}`);
  }

  async function deleteCard() {
    const card = state.card;
    if (!card) throw new Error('del needs a Card; use go <target> first');
    const plan = await api('delete', { at: card.address });
    const ok = await confirmPlan(`Delete ${card.address}`, plan.lines, plan.confirm);
    if (!ok) return;
    const result = await api('delete', { at: card.address, confirm: plan.confirm });
    const moved = Object.entries(result.moved || {});
    await afterChange(result.address || null,
      `deleted: ${result.deleted}` + (moved.length ? `\ncompacted:\n${moved.map(([a, b]) => `${a} -> ${b}`).join('\n')}` : ''));
  }

  async function moveCard(target) {
    const card = state.card;
    if (!card) throw new Error('mv needs a Card; use go <target> first');
    if (card.context !== 'regular') throw new Error(card.kind === 'literature'
      ? 'Literature Card cannot be moved; edit metadata to change its Citation key'
      : 'Topic Cards cannot be moved');
    const to = target || await promptValue(`Move ${card.address}`, 'New Location (the card keeps its successors):', card.address);
    if (!to) return;
    const plan = await api('move', { at: card.address, to });
    const ok = await confirmPlan(`Move ${card.address}`, plan.lines, plan.confirm);
    if (!ok) return;
    const result = await api('move', { at: card.address, to, confirm: plan.confirm });
    await afterChange(result.address, `moved: ${Object.keys(result.moved).length}`);
  }

  async function quit() {
    if (!window.confirm('Stop zt gui? The service keeps running; reopen with `zt gui`.')) return;
    try { await api('quit', {}); } catch { /* already gone */ }
    document.body.append(h('div', { class: 'stopped' }, h('div', {},
      h('h2', { text: 'zt gui stopped' }),
      h('p', { text: 'You can close this window. Run `zt gui` to open it again.' }))));
  }

  // ----------------------------------------------------------- command bar
  function installCommandBar() {
    const input = $('#command-input');
    const menu = $('#command-menu');
    let items = [];
    let selected = 0;

    const suggestions = () => {
      const value = input.value;
      const trimmed = value.trimStart();
      const list = [];
      const goMatch = trimmed.match(/^go\s+(\S*)$/);
      const mvMatch = trimmed.match(/^mv\s+(\S*)$/);
      if (goMatch) {
        const query = goMatch[1].toLowerCase();
        for (const node of state.nodes) {
          if (list.length >= 9) break;
          if (node.address.toLowerCase().startsWith(query) || (query && (node.title || '').toLowerCase().includes(query))) {
            list.push({ syntax: node.address, desc: node.title || '(untitled)', run: `go ${node.address}`, group: 'Cards' });
          }
        }
      } else if (!mvMatch) {
        const word = trimmed.split(/\s/)[0] || '';
        for (const [syntax, desc, scope] of COMMANDS) {
          if (!available(scope)) continue;
          const name = syntax.split(' ')[0];
          if (!word || name.startsWith(word)) {
            const takesArg = syntax.includes('<');
            list.push({ syntax, desc, group: 'Commands', fill: takesArg && !trimmed.includes(' ') ? name + ' ' : null, run: takesArg ? null : name });
          }
        }
        if (trimmed && !list.some((item) => item.syntax.split(' ')[0] === word) && trimmed.length > 1) {
          list.push({ syntax: `search “${trimmed}”`, desc: 'find cards by text', group: 'Search', search: trimmed });
        }
      }
      return list;
    };

    const render = () => {
      items = suggestions();
      if (!items.length || document.activeElement !== input) { menu.hidden = true; return; }
      selected = Math.min(selected, items.length - 1);
      let lastGroup = null;
      const nodes = [];
      items.forEach((item, index) => {
        if (item.group !== lastGroup) { nodes.push(h('div', { class: 'group', text: item.group })); lastGroup = item.group; }
        nodes.push(h('button', {
          class: index === selected ? 'on' : '', role: 'option',
          onmousedown: (event) => { event.preventDefault(); choose(item); },
        }, h('span', { class: 'syntax', text: item.syntax }), h('span', { class: 'desc', text: item.desc })));
      });
      menu.replaceChildren(...nodes);
      menu.hidden = false;
    };

    const execute = (line) => {
      input.value = '';
      menu.hidden = true;
      input.blur();
      Promise.resolve().then(() => runCommand(line)).catch(fail);
    };

    const choose = (item) => {
      if (item.search !== undefined) {
        input.value = '';
        menu.hidden = true;
        openRail(true);
        $('#search').value = item.search;
        runSearch(item.search);
        return;
      }
      if (item.fill) { input.value = item.fill; selected = 0; render(); return; }
      execute(item.run || input.value);
    };

    input.addEventListener('input', () => { selected = 0; render(); });
    input.addEventListener('focus', render);
    input.addEventListener('blur', () => setTimeout(() => { menu.hidden = true; }, 100));
    input.addEventListener('keydown', (event) => {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        if (!items.length) return;
        selected = (selected + (event.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length;
        render();
      } else if (event.key === 'Tab' && items.length) {
        event.preventDefault();
        const item = items[selected];
        input.value = item.fill || item.run || input.value;
        render();
      } else if (event.key === 'Enter') {
        event.preventDefault();
        const typed = input.value.trim();
        const item = items[selected];
        const word = typed.split(/\s+/)[0];
        const exact = COMMANDS.some(([syntax]) => syntax.split(' ')[0] === word);
        if (typed && (exact || !item)) execute(typed);
        else if (item) choose(item);
      } else if (event.key === 'Escape') {
        input.value = '';
        input.blur();
      }
    });
  }

  // ------------------------------------------------------------- keyboard
  function installKeys() {
    document.addEventListener('keydown', (event) => {
      if (state.editing || isTyping(event.target) || event.ctrlKey || event.metaKey || event.altKey) return;
      const card = state.card;
      const run = (fn) => { event.preventDefault(); Promise.resolve().then(fn).catch(fail); };
      switch (event.key) {
        case ':': return run(() => $('#command-input').focus());
        case '/': return run(() => { openRail(true); $('#search').focus(); });
        case 'g': return run(() => { const input = $('#command-input'); input.value = 'go '; input.focus(); input.dispatchEvent(new Event('input')); });
        case 'n': return run(() => runCommand('n'));
        case 'b': return run(() => runCommand('b'));
        case 'e': return run(() => runCommand('e'));
        case 't': return run(() => newCard('topic', ''));
        case 'l': return run(() => newLiterature());
        case 'm': return card ? run(() => moveCard()) : undefined;
        case 'Delete': return card ? run(deleteCard) : undefined;
        case 'u': case 'ArrowUp': case 'ArrowLeft':
          return card ? run(() => go(card.parent ? card.parent.address : null)) : undefined;
        case 'ArrowDown': return card?.direct ? run(() => go(card.direct.address)) : undefined;
        case 'ArrowRight': return card?.sides.length ? run(() => go(card.sides[0].address)) : undefined;
        case 'v': return run(() => setMode(state.mode === 'map' ? 'constellation' : 'map'));
        case 'f': return run(() => fitView());
        case '?': return run(showHelp);
        case 'Escape': return run(() => openRail(false));
        default: return undefined;
      }
    });
  }

  // ---------------------------------------------------------------- theme
  function installTheme() {
    const saved = store.get('zt-theme');
    if (saved === 'light' || saved === 'dark') document.documentElement.dataset.theme = saved;
    $('#theme-toggle').addEventListener('click', () => {
      const current = document.documentElement.dataset.theme
        || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
      const next = current === 'dark' ? 'light' : 'dark';
      document.documentElement.dataset.theme = next;
      store.set('zt-theme', next);
      sky.draw();
    });
  }

  // ------------------------------------------------------------------ boot
  async function watchForChanges() {
    if (document.hidden || state.editing) return;
    try {
      const overview = await api('overview');
      if (overview.fingerprint !== state.fingerprint) await refresh();
    } catch { /* the server may be restarting */ }
  }

  async function boot() {
    installTheme();
    installPanZoom();
    installCommandBar();
    installKeys();
    for (const button of document.querySelectorAll('.seg button')) {
      button.addEventListener('click', () => setMode(button.dataset.mode));
    }
    setMode(state.mode);
    $('#rail-toggle').addEventListener('click', () => openRail());
    $('#quit').addEventListener('click', () => quit().catch(fail));
    $('#broken-pill').addEventListener('click', () => showBroken().catch(fail));
    $('#search').addEventListener('input', (event) => runSearch(event.target.value));
    for (const button of document.querySelectorAll('[data-action]')) {
      button.addEventListener('click', () => (button.dataset.action === 'new-topic' ? newCard('topic', '') : newLiterature()).catch(fail));
    }
    window.addEventListener('hashchange', route);
    window.addEventListener('resize', debounce(() => { if (state.mode === 'constellation') sky.draw(); }, 100));
    window.addEventListener('focus', () => watchForChanges());
    setInterval(watchForChanges, 4000);
    try {
      const [overview, graph] = await Promise.all([api('overview'), api('graph')]);
      state.overview = overview;
      state.fingerprint = overview.fingerprint;
      state.nodes = graph.nodes;
      route();
    } catch (error) {
      fail(error);
    }
  }

  boot();
})();
