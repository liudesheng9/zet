/* zt gui — a slip-box desk for ZT Cards. */
(() => {
  'use strict';

  const TOKEN = document.querySelector('meta[name="zt-token"]').content;

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
  const cssVar = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

  // ------------------------------------------------------------------ state
  const state = {
    overview: null,
    nodes: [],            // every Card summary, for link completion and the sky
    fingerprint: '',
    pointer: null,        // Card address, or null for ROOT
    card: null,
    tree: null,
    outline: null,        // the current tree as an outline (layout.js)
    expandAll: store.get('zt-expand') === '1',
    mode: store.get('zt-mode') === 'constellation' ? 'constellation' : 'map',
    // The Workspace shown at ROOT and given new trees; it follows the Pointer.
    workspace: store.get('zt-workspace') === 'agent' ? 'agent' : 'human',
    editing: false,
  };

  const WORKSPACES = {
    human: { label: 'Human', short: 'human', mcp: 'read-only for AI agents' },
    agent: { label: 'AI agent', short: 'agent', mcp: 'written by AI agents over MCP' },
  };
  const wsLabel = (id) => (WORKSPACES[id] || WORKSPACES.human).label;
  const nodeWorkspace = (address) => state.nodes.find((node) => node.address === address)?.workspace;

  // Card types: most Cards hold an idea; a Regular Card may record an event or present a model.
  const CARD_TYPES = {
    idea: { label: 'Idea', note: 'one idea or claim' },
    event: { label: 'Event', note: 'one real-world event: what, when, where, who, and the source' },
    model: { label: 'Model', note: 'one whole model: setup, assumptions, solution, results, limits' },
  };
  // A model is presented by its Model Card and by Model parts filed under it. A part's facet is
  // a short label its author chose; these common facets are only suggestions (mirrors
  // `COMMON_FACETS` in src/model.rs). An emphasis marks a part `core` or `aside`.
  const FACETS = {
    setting: { label: 'Setting', note: 'agents, assets, actions, and payoffs' },
    assumptions: { label: 'Assumptions', note: 'what the model assumes, and what each assumption buys' },
    timing: { label: 'Timing', note: 'the order of moves and who knows what when' },
    dynamics: { label: 'Dynamics', note: 'how the state evolves: laws of motion, best responses, feedback' },
    solution: { label: 'Solution', note: 'the equilibrium or optimum and how it is derived' },
    results: { label: 'Results', note: 'propositions and comparative statics' },
    intuition: { label: 'Intuition', note: 'why the results hold, in words' },
    limits: { label: 'Limits', note: 'where the model stops applying, and its critics' },
  };
  const EMPHASIS = {
    core: { label: 'Core', note: 'the heart of the model: read it first' },
    aside: { label: 'Aside', note: 'detail a reader may skip' },
  };
  /** How a facet reads: a common facet by its English name, any other label as written. */
  const facetLabel = (facet) => FACETS[facet]?.label || facet || '';
  /** The rubber stamp an Event or Model Card carries; Idea Cards carry none. A Model part's
   *  stamp names its facet; a core part's stamp carries a dot. */
  const typeStamp = (type, title, facet, emphasis) => {
    if (!CARD_TYPES[type] || type === 'idea') return null;
    const part = type === 'model' && facet;
    const note = part && [FACETS[facet]?.note, EMPHASIS[emphasis] && `${EMPHASIS[emphasis].label}: ${EMPHASIS[emphasis].note}`].filter(Boolean).join(' · ');
    return h('span', {
      class: 'type-stamp ' + type + (part ? ' part' : '') + (part && emphasis ? ' ' + emphasis : ''),
      title: title || (part ? `Model part · ${facetLabel(facet)}${note ? ' — ' + note : ''}` : `${CARD_TYPES[type].label} Card — ${CARD_TYPES[type].note}`),
      text: part ? `${type} · ${facet}` : type,
    });
  };

  // ------------------------------------------------------------------ math
  // TeX renders with the bundled KaTeX: `$…$` and `$$…$$` in bodies arrive as math spans
  // from the server; titles are plain text, so their `$…$` runs are typeset here.
  const katexReady = () => typeof window.katex !== 'undefined';
  function typeset(root) {
    if (!katexReady()) return;
    for (const node of root.querySelectorAll('span.math-inline, span.math-display')) {
      if (node.dataset.typeset) continue;
      const tex = node.textContent;
      try {
        window.katex.render(tex, node, { displayMode: node.classList.contains('math-display'), throwOnError: false, output: 'htmlAndMathml' });
        node.dataset.typeset = '1';
        node.title = tex;
      } catch { /* leave the TeX as text */ }
    }
  }
  const titleCache = new Map();
  /** A title as nodes, with its `$…$` runs typeset. */
  function titleNodes(text) {
    if (!text || !text.includes('$') || !katexReady()) return [text];
    let html = titleCache.get(text);
    if (html === undefined) {
      const escape = (value) => value.replace(/[&<>"]/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]));
      html = text.split(/(\$[^$]+\$)/).map((part) => {
        if (part.length > 2 && part.startsWith('$') && part.endsWith('$')) {
          try { return window.katex.renderToString(part.slice(1, -1), { throwOnError: false }); } catch { return escape(part); }
        }
        return escape(part);
      }).join('');
      if (titleCache.size > 4000) titleCache.clear();
      titleCache.set(text, html);
    }
    const holder = document.createElement('span');
    holder.innerHTML = html;
    return [...holder.childNodes];
  }

  // ---------------------------------------------------------------- charts
  // A Card's HTML chart runs in a sandboxed frame served by `/chart/<id>`; it can only post
  // its height and the Card a reader clicked.
  const theme = () => (document.documentElement.dataset.theme === 'dark'
    || (!document.documentElement.dataset.theme && matchMedia('(prefers-color-scheme: dark)').matches) ? 'dark' : 'light');
  function wireCharts(root) {
    for (const frame of root.querySelectorAll('iframe.zt-chart-frame[data-chart]')) {
      frame.src = `/chart/${frame.dataset.chart}?theme=${theme()}`;
    }
  }
  window.addEventListener('message', (event) => {
    const data = event.data;
    if (!data || typeof data !== 'object') return;
    const frame = [...document.querySelectorAll('iframe.zt-chart-frame')].find((item) => item.contentWindow === event.source);
    if (!frame) return;
    if (data.zt === 'height' && Number.isFinite(data.height)) {
      frame.style.height = Math.max(40, Math.min(4000, data.height)) + 'px';
    } else if (data.zt === 'go' && typeof data.target === 'string') {
      if (state.nodes.some((node) => node.address === data.target)) go(data.target);
      else toast(`card \`${data.target}\` does not exist`, 'err');
    }
  });

  function setWorkspace(id, { toRoot } = {}) {
    if (!WORKSPACES[id]) throw new Error(`unknown workspace \`${id}\`; expected human or agent`);
    state.workspace = id;
    store.set('zt-workspace', id);
    if (toRoot) go(null);
  }

  // --------------------------------------------------------------- commands
  // Same vocabulary and availability as the terminal Session.
  const COMMANDS = [
    ['go [<target>]', 'go to a Location or Citation key; bare go returns to ROOT', 'always'],
    ['root', 'return to ROOT', 'always'],
    ['ws [<workspace>]', 'list workspaces, or switch to the human or agent workspace ROOT', 'always'],
    ['up', 'go to the parent card; a tree root goes to ROOT', 'card'],
    ['ls', 'list Topics and Literature at ROOT, or the current tree', 'always'],
    ['t <title>', 'create a Topic in the current workspace', 'always'],
    ['l', 'create a Literature Card in the current workspace from one BibTeX entry', 'always'],
    ['n', 'create the direct successor', 'card'],
    ['b', 'create the next side successor', 'regular'],
    ['e', 'edit the current card', 'card'],
    ['del', 'delete the current card and its successors', 'card'],
    ['mv <new-location>', 'move the current card and its successors', 'regular'],
    ['ty <idea|event|model> [<facet>]', 'make the current card an Idea, Event, or Model Card; a facet (any short label, e.g. setting, dynamics, 均衡) makes it a Model part', 'regular'],
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
      case 'ws':
        if (rest.length > 1) throw new Error('usage: ws [<workspace>]');
        if (!rest.length) return showWorkspaces();
        return setWorkspace(rest[0] === 'ai' ? 'agent' : rest[0].toLowerCase(), { toRoot: true });
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
      case 'ty':
        needCard();
        if (rest.length < 1) throw new Error('usage: ty <idea|event|model> [<facet>]');
        // A facet is the rest of the line, so it may hold spaces.
        return setCardType(rest[0].toLowerCase(), rest.slice(1).join(' '));
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
    if (state.card.workspace && state.card.workspace !== state.workspace) setWorkspace(state.card.workspace);
    const treeId = state.card.tree;
    if (!state.tree || state.tree.tree !== treeId) {
      state.tree = await api('tree?id=' + enc(treeId));
      state.outline = ZtLayout.outline(state.tree.cards, state.tree.root);
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
  function renderWorkspaceSwitch() {
    const summaries = state.overview.workspaces || [];
    $('#workspace-switch').replaceChildren(...Object.keys(WORKSPACES).map((id) => {
      const summary = summaries.find((item) => item.id === id) || { topic_trees: 0, literature_trees: 0, cards: 0 };
      return h('button', {
        class: 'ws-tab ' + id + (state.workspace === id ? ' on' : ''),
        role: 'tab',
        'aria-selected': state.workspace === id ? 'true' : 'false',
        title: `${wsLabel(id)} workspace — ${WORKSPACES[id].mcp}`,
        onclick: () => setWorkspace(id, { toRoot: true }),
      },
        h('span', { class: 'ws-dot' }),
        h('span', { class: 'ws-name', text: wsLabel(id) }),
        h('span', { class: 'ws-count', text: String(summary.cards) }),
      );
    }));
    $('#workspace-note').textContent = state.workspace === 'agent'
      ? 'Trees AI agents write over MCP. Links may cross workspaces; cards never move between them.'
      : 'Your own trees. AI agents can read them over MCP but never change them.';
  }

  function renderRail() {
    renderWorkspaceSwitch();
    const trees = state.overview.trees.filter((tree) => (tree.workspace || 'human') === state.workspace);
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
    fill('#topic-drawers', 'topic', state.workspace === 'agent' ? 'No agent Topics yet.' : 'No Topics yet — press t.');
    fill('#literature-drawers', 'literature', state.workspace === 'agent' ? 'No agent Literature yet.' : 'No Literature yet — press l.');
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
          h('span', { class: 'top' }, h('span', { class: 'addr', text: hit.address }), h('span', { class: 't' }, marked(hit.title || '(untitled)')), typeStamp(hit.type, null, hit.facet, hit.emphasis), hit.workspace === 'agent' ? h('span', { class: 'ws-tag agent', text: 'agent' }) : null),
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
    const trail = [
      h('button', {
        class: 'ws-tag ' + state.workspace, title: 'Switch workspace (w)',
        text: wsLabel(state.workspace),
        onclick: () => setWorkspace(state.workspace === 'agent' ? 'human' : 'agent', { toRoot: true }),
      }),
      h('a', { href: '#/', text: 'ROOT' }),
    ];
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
  const reduceMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

  function setMode(mode) {
    state.mode = mode;
    store.set('zt-mode', mode);
    for (const button of document.querySelectorAll('.seg button')) {
      button.classList.toggle('on', button.dataset.mode === mode);
    }
    renderStage();
  }

  function setExpandAll(on) {
    state.expandAll = on;
    store.set('zt-expand', on ? '1' : '0');
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
    const lens = $('#lens-toggle');
    lens.hidden = !inTree;
    lens.classList.toggle('on', state.expandAll);
    lens.title = state.expandAll ? 'Focus on the pointer and fold the rest (x)' : 'Unfold the whole tree (x)';
    lens.querySelector('span').textContent = state.expandAll ? 'Whole tree' : 'Focus';
    const title = $('#stage-title');
    if (state.mode === 'constellation') {
      title.textContent = `Constellation · ${state.overview.trees.length} trees · ${state.nodes.length} cards`;
      sky.show();
    } else if (!state.card) {
      title.textContent = `The cabinet · ${wsLabel(state.workspace)} workspace`;
      renderDesk();
    } else {
      map.render();
      const tree = state.tree;
      const shown = map.shownCount();
      const total = tree.cards.length;
      title.textContent = `${tree.kind === 'literature' ? 'Literature' : 'Topic'} ${tree.tree} · ${tree.title} — `
        + (shown < total ? `${shown} of ${total} cards in view` : `${total} card${total === 1 ? '' : 's'}`);
    }
  }

  function renderDesk() {
    const agent = state.workspace === 'agent';
    const trees = state.overview.trees.filter((tree) => (tree.workspace || 'human') === state.workspace);
    const zone = (kind) => trees.filter((tree) => tree.kind === kind);
    const largest = Math.max(1, ...trees.map((tree) => tree.count));
    // A stack shows a stamp for each kind of non-Idea Card it holds, with a tally past one.
    const tally = (type, count) => (count
      ? h('span', { class: 'tally' }, typeStamp(type, `${count} ${CARD_TYPES[type].label} Card${count === 1 ? '' : 's'}`), count > 1 ? `×${count}` : null)
      : null);
    // A bigger tree is a thicker stack: up to four sheets under the face.
    const stack = (tree) => h('button', {
      class: 'stack ' + tree.kind, onclick: () => go(tree.root), title: tree.title,
    },
      tree.count > 6 ? h('span', { class: 'sheet s4' }) : null,
      tree.count > 2 ? h('span', { class: 'sheet s3' }) : null,
      tree.count > 1 ? h('span', { class: 'sheet s2' }) : null,
      h('span', { class: 'face' },
        h('span', { class: 'kind', text: `${tree.kind === 'literature' ? 'Literature' : 'Topic'} · ${tree.root}` }),
        h('span', { class: 't', text: tree.title || '(untitled)' }),
        tree.excerpt ? h('span', { class: 'x', text: tree.excerpt }) : null,
        h('span', { class: 'foot' },
          h('span', { class: 'types' },
            h('span', { class: 'count', text: `${tree.count} card${tree.count === 1 ? '' : 's'}` }),
            tally('event', tree.events),
            tally('model', tree.models),
          ),
          h('span', { class: 'gauge', title: `${tree.count} of the ${largest} cards in the largest tree` },
            h('i', { style: `width: ${Math.max(6, Math.round(100 * tree.count / largest))}%` })),
        ),
      ),
    );
    const newStack = (label, key, action) => h('button', { class: 'stack new', onclick: action },
      h('span', { class: 'face' }, h('span', { class: 'plus', text: '+' }), h('span', { text: label }), h('kbd', { text: key })));
    const zoneSection = (kind, label, newLabel, key, action) => [
      h('h2', { class: 'zone-head ' + kind }, h('span', { text: label }), h('span', { class: 'n', text: String(zone(kind).length) })),
      h('div', { class: 'desk-grid' }, zone(kind).map(stack), newStack(newLabel, key, action)),
    ];
    $('#desk').replaceChildren(
      h('div', { class: 'desk-intro' },
        h('span', { class: 'ws-tag ' + state.workspace, text: `${wsLabel(state.workspace)} workspace` }),
        h('h1', { text: agent
          ? (trees.length ? 'The agent’s slip-box' : 'The agent’s slip-box is empty')
          : (trees.length ? 'Your slip-box' : 'An empty slip-box') }),
        h('p', { text: agent
          ? 'AI agents grow these trees through the zt MCP server. Their cards may link to yours; yours stay read-only to them.'
          : (trees.length
            ? 'Every Topic and Literature Card roots its own tree. Open a stack to walk its cards.'
            : 'Start a Topic for your own ideas, or a Literature Card for a work you are reading.') }),
      ),
      ...zoneSection('topic', 'Topic zone', 'New Topic', 't', () => newCard('topic', '')),
      ...zoneSection('literature', 'Literature zone', 'New Literature', 'l', () => newLiterature()),
    );
  }

  // -------------------------------------------------------------------- map
  // The Map is a focus lens over the current tree (see layout.js). The Pointer card is
  // pulled out of the drawer; its path, parent, children, and run neighbours stand up as
  // cards; the rest of what is shown lies as slips; everything else is folded into "+N"
  // counts. Only the Pointer's Links are drawn. Each move of the Pointer re-runs the
  // layout and glides the cards to their new places.
  const map = (() => {
    const els = new Map();   // address -> node element
    const paths = new Map(); // edge key -> SVG path
    let drawn = new Map();   // address -> geometry on screen now
    let view = null;
    let hovered = null;
    let frame = 0;
    const cam = camera.map;

    const STAGE_TOP = 62;
    const lerp = (a, b, t) => a + (b - a) * t;

    function content(el, card, n, outline) {
      const external = card.links.filter((target) => !outline.card.has(target)).length;
      const signature = [n.lod, n.role, n.folded, card.title, card.type, card.facet, card.emphasis, card.inbound, external, card.excerpt].join('\u0001');
      el.className = ['node', n.lod, 'role-' + n.role,
        card.edge === 'root' ? 'root ' + card.kind : '',
        card.type && card.type !== 'idea' ? 'type-' + card.type : '',
        card.facet ? 'model-part' : '',
        n.folded ? 'folded' : '',
        state.linkedIn && state.linkedIn.has(card.address) ? 'in' : '',
        state.linkedOut && state.linkedOut.has(card.address) ? 'out' : '',
      ].filter(Boolean).join(' ');
      if (el.dataset.sig === signature) return;
      el.dataset.sig = signature;
      el.title = card.title + (card.excerpt ? '\n\n' + card.excerpt : '');
      const fold = n.folded ? h('span', {
        class: 'badge fold', title: `${n.folded} more card${n.folded === 1 ? '' : 's'} filed behind — open this card to unfold them`,
        text: '+' + n.folded,
      }) : null;
      const title = h('span', { class: 't' + (card.title ? '' : ' untitled') }, card.title ? titleNodes(card.title) : 'untitled');
      const slash = card.address.indexOf('/');
      // Inside a Literature tree the long Citation key is known, so cards show the rest.
      const short = card.edge !== 'root' && slash > 4 ? '…' + card.address.slice(slash) : card.address;
      if (n.lod === 'slip') {
        el.replaceChildren(...[h('span', { class: 'addr', text: short }), title, typeStamp(card.type, null, card.facet, card.emphasis), fold].filter(Boolean));
        return;
      }
      el.replaceChildren(...[
        h('span', { class: 'top' },
          h('span', { class: 'addr', text: n.lod === 'focus' ? card.address : short }),
          h('span', { class: 'badges' },
            typeStamp(card.type, null, card.facet, card.emphasis),
            card.inbound ? h('span', { class: 'badge in', title: `Referred by ${card.inbound}`, text: '←' + card.inbound }) : null,
            external ? h('span', { class: 'badge', title: 'Links to other trees', text: '↗' + external }) : null,
            fold,
          ),
        ),
        title,
        n.lod === 'focus' && card.excerpt ? h('span', { class: 'x', text: card.excerpt }) : null,
      ].filter(Boolean));
    }

    /** Where the camera should be: a view that fits is centred and stays still; a larger
     *  one follows the Pointer, held a little left of centre and in the upper third. */
    function cameraTarget(k) {
      const viewport = $('#viewport');
      const vw = viewport.clientWidth; const vh = viewport.clientHeight;
      const b = view.bounds;
      const f = view.index.get(view.focus);
      const room = { w: vw - 64, h: vh - STAGE_TOP - 56 };
      const reach = 110; // Link arcs swing out to the right of the cards.
      let x; let y;
      if ((b.w + reach) * k <= room.w) x = 32 + (room.w - (b.w + reach) * k) / 2;
      else {
        // Show the right edge (the pointer's successors) and as much context to the left
        // as fits, but never push the pointer card itself out of view.
        const lo = vw - 32 - (f.x + f.w) * k;
        const hi = 32 - f.x * k;
        x = Math.min(40, Math.max(lo, Math.min(hi, vw - 32 - b.w * k)));
      }
      if (b.h * k <= room.h) y = STAGE_TOP + (room.h - b.h * k) / 2;
      else {
        y = STAGE_TOP + room.h * 0.36 - (f.y + f.h / 2) * k;
        y = Math.min(STAGE_TOP + 10, Math.max(vh - 56 - b.h * k, y));
      }
      return { x, y, k };
    }

    function paint(geometry) {
      for (const [address, g] of geometry) {
        const el = els.get(address);
        if (!el) continue;
        el.style.transform = `translate(${g.x}px, ${g.y}px)`;
        el.style.width = g.w + 'px';
        el.style.height = g.h + 'px';
      }
      const edges = $('#edges');
      const keep = new Set();
      const put = (key, d, cls) => {
        keep.add(key);
        let path = paths.get(key);
        if (!path) { path = svg('path', {}); paths.set(key, path); edges.append(path); }
        path.setAttribute('d', d);
        path.setAttribute('class', cls);
      };
      for (const edge of view.edges) {
        const a = geometry.get(edge.from); const b = geometry.get(edge.to);
        if (a && b) put('e' + edge.from + '>' + edge.to, ZtLayout.edgePath(edge, a, b), 'e-' + edge.kind + (edge.hot ? ' hot' : ''));
      }
      const focus = geometry.get(view.focus);
      if (focus) {
        const near = structural(view.focus);
        for (const address of state.linkedOut || []) {
          const b = geometry.get(address);
          if (b && !near.has(address)) put('o' + address, ZtLayout.linkArc(focus, b), 'e-link out');
        }
        for (const address of state.linkedIn || []) {
          const b = geometry.get(address);
          if (b && !near.has(address) && !state.linkedOut.has(address)) put('i' + address, ZtLayout.linkArc(b, focus), 'e-link in');
        }
      }
      const over = hovered && hovered !== view.focus ? geometry.get(hovered) : null;
      if (over) {
        const near = structural(hovered);
        for (const target of state.outline.card.get(hovered).links) {
          const b = geometry.get(target);
          if (b && target !== hovered && !near.has(target)) put('h' + target, ZtLayout.linkArc(over, b), 'e-link peek');
        }
      }
      for (const [key, path] of paths) if (!keep.has(key)) { path.remove(); paths.delete(key); }
    }

    /** Cards the tree structure already ties to `address`; a Link arc to one adds nothing. */
    function structural(address) {
      const outline = state.outline;
      const up = outline.up.get(address);
      const tied = new Set([...outline.kids.get(address), up, outline.card.get(address).parent]);
      for (const sibling of up != null ? outline.kids.get(up) : []) {
        if (outline.card.get(sibling).parent === address) tied.add(sibling);
      }
      return tied;
    }

    function glide(from, to, camFrom, camTo, ms) {
      const id = ++frame;
      const t0 = performance.now();
      const step = (now) => {
        if (id !== frame) return;
        const t = ms <= 0 || reduceMotion() ? 1 : Math.min(1, (now - t0) / ms);
        const e = 1 - Math.pow(1 - t, 3);
        const geometry = new Map();
        for (const [address, b] of to) {
          const a = from.get(address) || b;
          geometry.set(address, { x: lerp(a.x, b.x, e), y: lerp(a.y, b.y, e), w: lerp(a.w, b.w, e), h: lerp(a.h, b.h, e) });
        }
        drawn = geometry;
        paint(geometry);
        cam.x = lerp(camFrom.x, camTo.x, e); cam.y = lerp(camFrom.y, camTo.y, e); cam.k = lerp(camFrom.k, camTo.k, e);
        applyCamera();
        if (t < 1) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    }

    function render() {
      const card = state.card;
      const outline = state.outline;
      if (!card || !outline) return;
      const inTree = (address) => outline.card.has(address) && address !== card.address;
      state.linkedOut = new Set(card.outbound.map((link) => link.address).filter(inTree));
      const inbound = card.inbound.map((link) => link.address).filter(inTree);
      // A Card that half the tree refers to (a tree root) would light up everything.
      state.linkedIn = new Set(inbound.length <= 16 ? inbound : []);
      const fresh = !view || view.tree !== state.tree.tree;
      view = ZtLayout.focusLens(outline, card.address, {
        links: [...state.linkedOut, ...state.linkedIn], expandAll: state.expandAll,
      });
      view.tree = state.tree.tree;

      const nodes = $('#nodes');
      const from = new Map(drawn);
      const to = new Map(view.nodes.map((n) => [n.address, { x: n.x, y: n.y, w: n.w, h: n.h }]));
      if (fresh) {
        for (const el of els.values()) el.remove();
        for (const path of paths.values()) path.remove();
        els.clear(); paths.clear(); from.clear();
      }
      for (const n of view.nodes) {
        let el = els.get(n.address);
        if (!el) {
          const address = n.address;
          el = h('button', {
            class: 'node',
            dataset: { address },
            onclick: (event) => { event.stopPropagation(); go(address); },
            onpointerenter: () => { hovered = address; paint(drawn); },
            onpointerleave: () => { if (hovered === address) { hovered = null; paint(drawn); } },
          });
          el.classList.add('entering');
          els.set(address, el);
          nodes.append(el);
          // A newly shown card slides out from behind its nearest card already on screen.
          let at = outline.up.get(address);
          while (at != null && !from.has(at)) at = outline.up.get(at);
          const origin = at != null ? from.get(at) : null;
          if (origin) from.set(address, { x: origin.x, y: origin.y, w: n.w, h: n.h });
          setTimeout(() => el.classList.remove('entering'), 400);
        }
        content(el, outline.card.get(n.address), n, outline);
      }
      for (const [address, el] of els) {
        if (to.has(address)) continue;
        els.delete(address);
        el.classList.add('leaving');
        setTimeout(() => el.remove(), 240);
      }
      if (hovered && !to.has(hovered)) hovered = null;
      const edges = $('#edges');
      edges.setAttribute('width', view.bounds.w + 400);
      edges.setAttribute('height', view.bounds.h + 40);
      // Reading needs legible type: navigating out of a zoomed-out overview zooms back in.
      const k = fresh || cam.k < 0.8 ? 1 : cam.k;
      const target = cameraTarget(k);
      if (fresh) glide(to, to, target, target, 0);
      else glide(from, to, { ...cam }, target, 360);
    }

    return {
      render,
      shownCount: () => (view ? view.nodes.length : 0),
      fit() {
        if (!view) return;
        const viewport = $('#viewport');
        const room = { w: viewport.clientWidth - 64, h: viewport.clientHeight - STAGE_TOP - 56 };
        const k = Math.max(0.22, Math.min(1.1, room.w / (view.bounds.w + 110), room.h / view.bounds.h));
        glide(drawn, drawn, { ...cam }, cameraTarget(k), 300);
      },
      reset() { view = null; drawn = new Map(); },
    };
  })();

  function applyCamera() {
    const cam = camera.map;
    $('#plane').style.transform = `translate(${cam.x}px, ${cam.y}px) scale(${cam.k})`;
  }

  function fitView() {
    if (state.mode === 'constellation') sky.fit();
    else map.fit();
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
    viewport.addEventListener('pointerleave', () => { if (state.mode === 'constellation') sky.leave(); });
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
  // Every tree as a radial outline, the trees set around one ring (layout.js), and the
  // Links between two trees bundled into one arc whose weight is their count. Attention
  // goes to the Pointer's tree (or, at ROOT, the open Workspace); the rest recedes. Single
  // Links are drawn only for the Pointer and the hovered card.
  const sky = (() => {
    let layout = null;
    let hovered = null;
    let hoveredTree = null;
    let userMoved = false;
    const cam = camera.constellation;
    const canvas = () => $('#sky');

    const hue = (tree) => {
      const index = state.overview ? state.overview.trees.findIndex((entry) => entry.tree === tree) : -1;
      if (index >= 0) return (24 + index * 137.508) % 360;
      let hash = 0;
      for (const ch of tree) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
      return hash % 360;
    };

    function build() {
      layout = ZtLayout.constellation(state.nodes, state.overview ? state.overview.trees : []);
      layout.dots = state.nodes.map((node) => ({
        ...node, ...layout.pos.get(node.address),
        hue: hue(node.tree),
        r: node.edge === 'root' ? 5.5 : 2.3 + Math.min(node.inbound, 12) * 0.3,
      }));
      layout.byAddress = new Map(layout.dots.map((dot) => [dot.address, dot]));
      layout.byTree = new Map(layout.trees.map((tree) => [tree.id, tree]));
      layout.extent = layout.R + Math.max(...layout.trees.map((tree) => tree.r), 0) + 150;
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

    /** A Link between two cards: a curve that bows toward the middle of the ring. */
    function arc(ctx, a, b) {
      ctx.moveTo(a.x, a.y);
      const bow = a.tree === b.tree ? 0.75 : 0.35;
      ctx.quadraticCurveTo((a.x + b.x) / 2 * bow, (a.y + b.y) / 2 * bow, b.x, b.y);
    }

    function draw() {
      if (!layout || state.mode !== 'constellation') return;
      const c = canvas();
      const { w, h: hh, dpr } = resize();
      const ctx = c.getContext('2d');
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.clearRect(0, 0, c.width, c.height);
      ctx.setTransform(dpr * cam.k, 0, 0, dpr * cam.k, dpr * (w / 2 + cam.x), dpr * (hh / 2 + cam.y));
      const px = 1 / cam.k;
      const dark = document.documentElement.dataset.theme === 'dark'
        || (!document.documentElement.dataset.theme && matchMedia('(prefers-color-scheme: dark)').matches);
      const light = dark ? 64 : 40;
      const ink = cssVar('--ink'); const faint = cssVar('--ink-faint'); const link = cssVar('--link');
      const paper = cssVar('--paper');
      const focus = state.pointer ? layout.byAddress.get(state.pointer) : null;
      const focusTree = focus ? focus.tree : null;
      const lit = (id) => {
        if (hoveredTree) return id === hoveredTree || id === focusTree;
        if (focusTree) return id === focusTree;
        return (layout.byTree.get(id).workspace || 'human') === state.workspace;
      };

      // Bundled Links between trees.
      ctx.lineCap = 'round';
      for (const bundle of layout.bundles) {
        const a = layout.byTree.get(bundle.a); const b = layout.byTree.get(bundle.b);
        const touch = [hoveredTree, focusTree].some((id) => id && (id === bundle.a || id === bundle.b));
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.quadraticCurveTo((a.x + b.x) * 0.16, (a.y + b.y) * 0.16, b.x, b.y);
        ctx.strokeStyle = touch ? link : faint;
        ctx.globalAlpha = touch ? 0.42 : (focusTree || hoveredTree ? 0.07 : 0.16);
        ctx.lineWidth = Math.max(px, 1 + 1.7 * Math.log2(1 + bundle.count));
        ctx.stroke();
      }

      // Each tree's disc and its outline edges.
      for (const tree of layout.trees) {
        const on = lit(tree.id);
        ctx.globalAlpha = on ? 0.9 : 0.5;
        ctx.beginPath();
        ctx.arc(tree.x, tree.y, tree.r, 0, Math.PI * 2);
        ctx.fillStyle = paper;
        ctx.fill();
        ctx.lineWidth = (on ? 1.4 : 1) * px;
        ctx.strokeStyle = cssVar(tree.workspace === 'agent' ? '--agent' : '--human');
        ctx.globalAlpha = on ? 0.55 : 0.2;
        ctx.stroke();
        ctx.beginPath();
        for (const [address, p] of tree.local) {
          const up = tree.outline.up.get(address);
          if (up == null) continue;
          const q = tree.local.get(up);
          ctx.moveTo(tree.x + q.x, tree.y + q.y);
          ctx.lineTo(tree.x + p.x, tree.y + p.y);
        }
        ctx.strokeStyle = `hsl(${hue(tree.id)} 40% ${light}%)`;
        ctx.globalAlpha = on ? 0.5 : 0.16;
        ctx.lineWidth = 1.1 * px;
        ctx.stroke();
      }

      // Cards.
      for (const dot of layout.dots) {
        ctx.globalAlpha = lit(dot.tree) ? 1 : 0.32;
        ctx.beginPath();
        ctx.arc(dot.x, dot.y, dot.r, 0, Math.PI * 2);
        ctx.fillStyle = `hsl(${dot.hue} ${dot.kind === 'literature' ? 62 : 52}% ${light}%)`;
        ctx.fill();
        if (dot.edge === 'root') { ctx.lineWidth = 2 * px; ctx.strokeStyle = paper; ctx.stroke(); }
      }

      // Single Links: the Pointer's in blue, the hovered card's in ink.
      const spray = (from, color, alpha) => {
        if (!from) return;
        ctx.beginPath();
        for (const target of from.links) {
          const to = layout.byAddress.get(target);
          if (to && to !== from) arc(ctx, from, to);
        }
        ctx.strokeStyle = color; ctx.globalAlpha = alpha; ctx.lineWidth = 1.3 * px;
        ctx.stroke();
        for (const target of from.links) {
          const to = layout.byAddress.get(target);
          if (!to || to === from) continue;
          ctx.beginPath();
          ctx.arc(to.x, to.y, to.r + 2.5 * px, 0, Math.PI * 2);
          ctx.lineWidth = 1.4 * px; ctx.strokeStyle = color; ctx.globalAlpha = Math.min(1, alpha + 0.2);
          ctx.stroke();
        }
      };
      spray(focus, link, 0.75);
      if (hovered && hovered !== focus) spray(hovered, ink, 0.55);
      if (focus) {
        ctx.globalAlpha = 1;
        ctx.beginPath();
        ctx.arc(focus.x, focus.y, focus.r + 5 * px, 0, Math.PI * 2);
        ctx.lineWidth = 2.5 * px;
        ctx.strokeStyle = cssVar('--pointer');
        ctx.stroke();
      }

      // Tree labels sit outside the ring, so they never cover a disc.
      ctx.textBaseline = 'middle';
      for (const tree of layout.trees) {
        const on = lit(tree.id);
        const dx = Math.cos(tree.angle); const dy = Math.sin(tree.angle);
        const x = tree.x + dx * (tree.r + 12 * px); const y = tree.y + dy * (tree.r + 12 * px);
        ctx.textAlign = Math.abs(dx) < 0.3 ? 'center' : dx > 0 ? 'left' : 'right';
        const title = tree.title || tree.id;
        const label = title.length > 18 ? title.slice(0, 17) + '…' : title;
        ctx.font = `${on ? 600 : 400} ${13 * px}px ${cssVar('--serif')}`;
        ctx.fillStyle = on ? ink : faint;
        ctx.globalAlpha = on ? 0.95 : 0.7;
        const ly = y + (Math.abs(dx) < 0.3 ? dy * 6 * px : 0);
        ctx.fillText(label, x, ly);
        ctx.font = `${10.5 * px}px ${cssVar('--mono')}`;
        ctx.fillStyle = faint;
        ctx.fillText(`${tree.id} · ${tree.count}`, x, ly + 15 * px);
      }

      // The two Workspaces, named inside the ring at the middle of their arcs.
      ctx.textAlign = 'center';
      for (const id of ['human', 'agent']) {
        const own = layout.trees.filter((tree) => (tree.workspace || 'human') === id);
        if (!own.length || !layout.R) continue;
        const sx = own.reduce((s, t) => s + Math.cos(t.angle), 0);
        const sy = own.reduce((s, t) => s + Math.sin(t.angle), 0);
        const len = Math.hypot(sx, sy) || 1;
        const inner = Math.max(40, layout.R - Math.max(...own.map((t) => t.r)) - 46);
        ctx.font = `700 ${10.5 * px}px ${cssVar('--sans')}`;
        ctx.fillStyle = cssVar(id === 'agent' ? '--agent' : '--human');
        ctx.globalAlpha = id === state.workspace ? 0.9 : 0.5;
        ctx.fillText(`${wsLabel(id).toUpperCase()} · ${own.reduce((s, t) => s + t.count, 0)}`, sx / len * inner, sy / len * inner);
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
      if (!layout) return { dot: null, tree: null };
      const p = screenToWorld(event);
      let dot = null;
      let best = (11 / cam.k) ** 2;
      for (const node of layout.dots) {
        const d = (node.x - p.x) ** 2 + (node.y - p.y) ** 2;
        if (d < Math.max(best, node.r * node.r)) { dot = node; best = d; }
      }
      const tree = layout.trees.find((t) => (t.x - p.x) ** 2 + (t.y - p.y) ** 2 <= (t.r + 6) ** 2) || null;
      return { dot, tree };
    }

    const self = {
      invalidate() { if (layout) build(); },
      show() {
        if (!layout) build();
        if (!userMoved) self.fit();
        draw();
      },
      draw,
      fit() {
        if (!layout) return;
        const c = canvas();
        cam.k = Math.max(0.15, Math.min(2.2, c.clientWidth / (2 * layout.extent), (c.clientHeight - 60) / (2 * layout.extent)));
        cam.x = 0;
        cam.y = 20;
        draw();
      },
      pan(dx, dy) { userMoved = true; cam.x += dx; cam.y += dy; draw(); },
      zoom(factor, px, py) {
        userMoved = true;
        const c = canvas();
        const ox = px - c.clientWidth / 2; const oy = py - c.clientHeight / 2;
        const k = Math.max(0.15, Math.min(6, cam.k * factor));
        cam.x = ox - (ox - cam.x) * (k / cam.k);
        cam.y = oy - (oy - cam.y) * (k / cam.k);
        cam.k = k;
        draw();
      },
      hover(event) {
        const { dot, tree } = pick(event);
        const label = $('#sky-label');
        const treeId = dot ? null : tree ? tree.id : null;
        if (dot !== hovered || treeId !== hoveredTree) { hovered = dot; hoveredTree = treeId; draw(); }
        if (!dot && !tree) { label.hidden = true; canvas().style.cursor = ''; return; }
        const rect = canvas().getBoundingClientRect();
        const at = dot || tree;
        label.hidden = false;
        label.replaceChildren(...(dot
          ? [h('span', { class: 'addr', text: dot.address + '  ' }), dot.title || '(untitled)']
          : [h('span', { class: 'addr', text: tree.id + '  ' }), `${tree.title || '(untitled)'} · ${tree.count} cards`]));
        label.style.left = (rect.width / 2 + cam.x + at.x * cam.k) + 'px';
        label.style.top = (rect.height / 2 + cam.y + (at.y - (dot ? dot.r : tree.r)) * cam.k) + 'px';
        canvas().style.cursor = 'pointer';
      },
      leave() {
        if (hovered || hoveredTree) { hovered = null; hoveredTree = null; draw(); }
        $('#sky-label').hidden = true;
      },
      click(event) {
        const { dot, tree } = pick(event);
        if (dot) go(dot.address);
        else if (tree) go(tree.root);
      },
    };
    return self;
  })();

  // ----------------------------------------------------------------- reader
  function chip(link, extraClass) {
    const cross = !link.broken && link.workspace && state.card && link.workspace !== state.card.workspace;
    return h('button', {
      class: 'chip' + (link.broken ? ' broken' : '') + (cross ? ' cross ' + link.workspace : '') + (extraClass ? ' ' + extraClass : ''),
      title: link.broken ? 'Broken link' : cross ? `${link.title} · ${wsLabel(link.workspace)} workspace` : link.title,
      onclick: link.broken ? null : () => go(link.address),
    }, h('span', { class: 'addr', text: link.address }), h('span', { class: 't' }, link.broken ? 'missing' : titleNodes(link.title || '(untitled)')));
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
      const summary = (state.overview.workspaces || []).find((item) => item.id === state.workspace)
        || { cards: 0, topic_trees: 0, literature_trees: 0 };
      reader.replaceChildren(h('div', { class: 'welcome' },
        h('h2', { text: 'ROOT' }),
        h('p', {}, h('span', { class: 'ws-tag ' + state.workspace, text: `${wsLabel(state.workspace)} workspace` })),
        h('p', { text: `${summary.cards} cards in ${summary.topic_trees} Topic trees and ${summary.literature_trees} Literature trees. Pick a stack on the desk, a drawer in the cabinet, or type a command above.` }),
        h('dl', {}, [
          ['go <target>', 'open a Location or Citation key'],
          ['t <title>', 'start a Topic'],
          ['l', 'add a Literature Card from BibTeX'],
          [': or /', 'command bar · search'],
          ['↑ ↓ →', 'walk parent · direct · side in the map'],
          ['x', 'unfold the whole tree, or fold it back around the pointer'],
          ['v', 'switch Map / Constellation'],
          ['w · ws', 'switch the human / agent workspace'],
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
    const regular = card.context === 'regular';
    // The article is the physical index card; links and tools lie on the desk below it.
    reader.replaceChildren(h('article', { class: 'card ' + card.kind + (card.workspace === 'agent' ? ' agent-ws' : '') + (card.type && card.type !== 'idea' ? ' type-' + card.type : '') + (card.model ? ' in-model' : '') },
      card.model ? modelTabs(card) : null,
      h('header', { class: 'card-head' },
        h('div', { class: 'card-meta' },
          h('span', { class: 'kind-tag ' + card.kind, text: kindLabel }),
          h('span', { class: 'ws-tag ' + card.workspace, title: `${wsLabel(card.workspace)} workspace — ${(WORKSPACES[card.workspace] || WORKSPACES.human).mcp}`, text: wsLabel(card.workspace) }),
          typeStamp(card.type, null, card.facet, card.emphasis),
          h('button', {
            class: 'stamp', title: 'Copy address', text: card.address,
            onclick: () => navigator.clipboard?.writeText(card.address).then(() => toast(`copied ${card.address}`), () => {}),
          }),
        ),
        h('h1', { class: 'title' }, titleNodes(card.title || '(untitled)')),
      ),
      md,
    ), h('div', { class: 'card-tools' },
      card.model ? modelBoard(card) : null,
      card.kind === 'literature' ? h('details', { class: 'biblio' },
        h('summary', { text: `BibTeX · ${card.citation_key}` }),
        h('pre', { text: card.bibtex })) : null,
      card.inbound.length ? [h('div', { class: 'section-label', text: `Referred by · ${card.inbound.length}` }), h('div', { class: 'chips' }, card.inbound.map((link) => chip(link)))] : null,
      card.outbound.length ? [h('div', { class: 'section-label', text: `Links out · ${card.outbound.length}` }), h('div', { class: 'chips' }, card.outbound.map((link) => chip(link)))] : null,
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

  /**
   * The divider tabs along the top of a Model Card or Model part: the Model Card, then one tab
   * per part in the order its author filed them, named by the part's own facet. The open Card's
   * tab stands up; core tabs are inked, aside tabs are narrow and pale.
   */
  function modelTabs(card) {
    const model = card.model;
    const head = model.head;
    const tab = (label, address, here, title, extra) => h(address ? 'button' : 'span', {
      class: 'model-tab' + (here ? ' here' : '') + (extra ? ' ' + extra : ''),
      title,
      onclick: address && !here ? () => go(address) : null,
    }, label);
    const tabs = [head
      ? tab('Model', head.address, head.address === card.address, `Model Card ${head.address} — ${head.title}`, 'head')
      : tab('Model', null, false, 'This part is not filed under a Model Card', 'head missing')];
    for (const part of model.parts) {
      tabs.push(tab(facetLabel(part.facet), part.address, part.address === card.address,
        `${facetLabel(part.facet)}${part.emphasis ? ` (${part.emphasis})` : ''} · ${part.address} — ${part.title}`, part.emphasis));
    }
    return h('nav', { class: 'model-tabs', 'aria-label': 'Model structure' }, tabs);
  }

  /** A part's title without a leading `facet：` its author wrote for the Folgezettel map. */
  const bareTitle = (title) => (title || '').replace(/^[^：:$]{1,10}[：:]\s*/, '') || title || '(untitled)';

  /**
   * The model on the desk below its card. The Model Card heads the board; its parts lie below
   * as small index cards, each under the parts it links (what it rests on), with a thread for
   * every such relation. The board is laid out for a reader's attention: the open part is
   * lifted and fully titled, the parts it relates to stay inked with their threads drawn and
   * their reasons listed, core parts stay legible, and every other part recedes to its label.
   * On the Model Card itself, the core parts lead. Loose parts, which relate to no other part,
   * lie apart in a tray. The layout is computed, not simulated: one pass over the parts.
   */
  function modelBoard(card) {
    const model = card.model;
    if (!model.head) {
      return h('div', { class: 'model-issues' }, model.issues.map((issue) => h('p', { text: issue })));
    }
    const parts = model.parts;
    const here = card.address;
    const onHead = here === model.head.address;
    const order = new Map(parts.map((part, at) => [part.address, at]));
    const facetOf = new Map(parts.map((part) => [part.address, part.facet]));
    const loose = new Set(model.loose || []);
    const relations = (model.relations || []).filter((r) => order.has(r.from) && order.has(r.to));
    const restsOn = new Map(parts.map((part) => [part.address, []]));
    const supports = new Map(parts.map((part) => [part.address, []]));
    for (const relation of relations) {
      restsOn.get(relation.from).push(relation);
      supports.get(relation.to).push(relation);
    }

    // Depth: one row below the deepest part a part rests on; a Link that closes a cycle is
    // left out of the depth (its thread is still drawn).
    const depth = new Map();
    const visiting = new Set();
    const depthOf = (address) => {
      if (depth.has(address)) return depth.get(address);
      visiting.add(address);
      let d = 0;
      for (const relation of restsOn.get(address)) {
        if (!visiting.has(relation.to)) d = Math.max(d, depthOf(relation.to) + 1);
      }
      visiting.delete(address);
      depth.set(address, d);
      return d;
    };
    const rows = [];
    for (const part of parts) {
      if (!loose.has(part.address)) (rows[depthOf(part.address)] ||= []).push(part);
    }
    // Within a row, a part sits under the parts it rests on (their mean slot), else in filing order.
    const slot = new Map();
    for (const row of rows.filter(Boolean)) {
      const key = (part) => {
        const xs = restsOn.get(part.address).map((r) => slot.get(r.to)).filter((x) => x !== undefined);
        return xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : (order.get(part.address) + 0.5) / parts.length;
      };
      const keys = new Map(row.map((part) => [part.address, key(part)]));
      row.sort((a, b) => keys.get(a.address) - keys.get(b.address) || order.get(a.address) - order.get(b.address));
      row.forEach((part, at) => slot.set(part.address, (at + 0.5) / row.length));
    }

    // Attention: what the reader has open, what it relates to, what its author called core.
    const near = new Set();
    if (!onHead) {
      for (const r of restsOn.get(here) || []) near.add(r.to);
      for (const r of supports.get(here) || []) near.add(r.from);
    }
    const level = (part) => {
      if (part.address === here) return 'here';
      if (onHead) {
        if (part.emphasis === 'core') return 'near';
        return part.emphasis === 'aside' ? 'far' : 'mid';
      }
      if (near.has(part.address)) return 'near';
      return part.emphasis === 'core' ? 'mid' : 'far';
    };
    const isHot = (relation) => (onHead
      ? level(parts[order.get(relation.from)]) === 'near' && level(parts[order.get(relation.to)]) === 'near'
      : relation.from === here || relation.to === here);

    const slips = new Map();
    const slip = (part) => {
      const at = level(part);
      const node = h(part.address === here ? 'div' : 'button', {
        class: `mb-slip ${at}${part.emphasis ? ' ' + part.emphasis : ''}`,
        title: `${facetLabel(part.facet)}${part.emphasis ? ` · ${part.emphasis}` : ''} · ${part.address}\n${part.title}`,
        'aria-current': part.address === here ? 'true' : null,
        onclick: part.address === here ? null : () => go(part.address),
      },
        h('span', { class: 'mb-facet', text: facetLabel(part.facet) }),
        h('span', { class: 'mb-title' }, titleNodes(bareTitle(part.title))),
      );
      slips.set(part.address, node);
      return node;
    };
    const headSlip = h(onHead ? 'div' : 'button', {
      class: 'mb-head' + (onHead ? ' here' : ''),
      title: `Model Card ${model.head.address}\n${model.head.title}`,
      onclick: onHead ? null : () => go(model.head.address),
    }, h('span', { class: 'mb-facet', text: 'Model' }), h('span', { class: 'mb-title' }, titleNodes(model.head.title)));

    const threads = svg('svg', { class: 'mb-threads', 'aria-hidden': 'true' });
    const board = h('div', { class: 'model-board' + (onHead ? ' on-head' : '') },
      threads,
      headSlip,
      rows.filter(Boolean).map((row) => h('div', { class: 'mb-row' + (row.length > 3 ? ' wide' : '') }, row.map(slip))),
      loose.size ? h('div', { class: 'mb-loose' },
        h('span', { class: 'mb-tray', text: 'Loose', title: 'Parts that relate to no other part of the model' }),
        parts.filter((part) => loose.has(part.address)).map(slip)) : null,
    );
    // Threads run from a part down to the parts that rest on it; faint threads first, so the
    // ones that matter now lie on top.
    const draw = () => {
      const box = board.getBoundingClientRect();
      if (!box.width) return;
      threads.setAttribute('width', box.width);
      threads.setAttribute('height', box.height);
      threads.setAttribute('viewBox', `0 0 ${box.width} ${box.height}`);
      threads.replaceChildren();
      const place = (address) => {
        const r = slips.get(address)?.getBoundingClientRect();
        return r && { l: r.left - box.left, r: r.right - box.left, t: r.top - box.top, b: r.bottom - box.top, x: r.left - box.left + r.width / 2, y: r.top - box.top + r.height / 2 };
      };
      const drawn = relations
        .filter((r) => !loose.has(r.from) && !loose.has(r.to))
        .map((r) => ({ r, hot: isHot(r) }))
        .sort((a, b) => Number(a.hot) - Number(b.hot));
      for (const { r, hot } of drawn) {
        const from = place(r.to);
        const to = place(r.from);
        if (!from || !to) continue;
        let d;
        let tip;
        if (from.b + 4 <= to.t) {
          const mid = (from.b + to.t) / 2;
          d = `M ${from.x} ${from.b} C ${from.x} ${mid}, ${to.x} ${mid}, ${to.x} ${to.t - 2}`;
          tip = `M ${to.x - 3.5} ${to.t - 7} L ${to.x} ${to.t - 2} L ${to.x + 3.5} ${to.t - 7}`;
        } else {
          // A Link against the grain (up or across): an arc beside the two cards.
          const side = Math.max(from.r, to.r) + 16;
          d = `M ${from.r} ${from.y} C ${side} ${from.y}, ${side} ${to.y}, ${to.r + 2} ${to.y}`;
          tip = `M ${to.r + 7} ${to.y - 3.5} L ${to.r + 2} ${to.y} L ${to.r + 7} ${to.y + 3.5}`;
        }
        const group = svg('g', { class: 'mb-thread' + (hot ? ' hot' : '') });
        const hit = svg('path', { d, class: 'hit' });
        const label = svg('title', {});
        label.textContent = `${facetLabel(facetOf.get(r.from))} rests on ${facetLabel(facetOf.get(r.to))}: ${r.note}`;
        hit.append(label);
        group.append(svg('path', { d, class: 'line' }), svg('path', { d: tip, class: 'tip' }), hit);
        threads.append(group);
      }
    };
    new ResizeObserver(() => requestAnimationFrame(draw)).observe(board);

    // The reasons behind the open part's relations, in its own and its neighbours' words.
    const reasons = onHead ? [] : [
      ...(restsOn.get(here) || []).map((r) => ({ way: 'up', at: r.to, note: r.note, label: 'rests on' })),
      ...(supports.get(here) || []).map((r) => ({ way: 'down', at: r.from, note: r.note, label: 'carries' })),
    ];
    const kept = parts.length - loose.size;
    return [
      h('div', { class: 'section-label', text: [
        `Model · ${parts.length} part${parts.length === 1 ? '' : 's'}`,
        relations.length ? `${relations.length} relation${relations.length === 1 ? '' : 's'}` : null,
        loose.size && kept ? `${loose.size} loose` : null,
        model.discussion ? `discussed by ${model.discussion}` : null,
      ].filter(Boolean).join(' · ') }),
      board,
      reasons.length ? h('div', { class: 'mb-reasons', 'aria-label': 'Why this part relates to the others' },
        reasons.map((reason) => h('button', { class: 'mb-reason ' + reason.way, onclick: () => go(reason.at), title: `${reason.label} ${reason.at}` },
          h('span', { class: 'way', text: reason.way === 'up' ? '↑' : '↓' }),
          h('span', { class: 'mb-facet', text: facetLabel(facetOf.get(reason.at)) }),
          h('span', { class: 'note', text: reason.note }),
        ))) : null,
      model.issues && model.issues.length ? h('div', { class: 'model-issues', title: 'Links that keep the model readable from every Card' },
        model.issues.map((issue) => h('p', { text: issue }))) : null,
    ];
  }

  function wireMarkdown(root) {
    typeset(root);
    wireCharts(root);
    // Link chips carry their target's title, which may hold TeX.
    for (const title of root.querySelectorAll('.zt-link-title')) {
      if (title.textContent.includes('$')) title.replaceChildren(...titleNodes(title.textContent));
    }
    for (const anchor of root.querySelectorAll('a[href]')) {
      if (anchor.classList.contains('zt-link')) {
        const targetWorkspace = nodeWorkspace(anchor.dataset.target);
        const here = state.card ? state.card.workspace : state.workspace;
        if (targetWorkspace && targetWorkspace !== here) {
          anchor.classList.add('cross', targetWorkspace);
          anchor.title = `${wsLabel(targetWorkspace)} workspace`;
        }
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

  function showWorkspaces() {
    const summaries = state.overview.workspaces || [];
    listDialog('Workspaces', (close) => [
      h('div', { class: 'ws-list' }, Object.keys(WORKSPACES).map((id) => {
        const summary = summaries.find((item) => item.id === id) || { topic_trees: 0, literature_trees: 0, cards: 0 };
        return h('button', {
          class: 'ws-row ' + id + (state.workspace === id ? ' on' : ''),
          onclick: () => { close(); setWorkspace(id, { toRoot: true }); },
        },
          h('span', { class: 'ws-tag ' + id, text: wsLabel(id) }),
          h('span', { class: 't', text: `${summary.topic_trees} Topic trees · ${summary.literature_trees} Literature trees · ${summary.cards} cards` }),
          h('span', { class: 'x', text: WORKSPACES[id].mcp }),
        );
      })),
      h('p', { class: 'hint', text: 'Links may cross workspaces; cards and trees never move between them.' }),
    ]);
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
      h('p', { text: 'Keys: : command · / search · g go · n b e m · Del delete · u up · ↑↓→ walk · v view · x unfold · w workspace · f fit · Esc close' }),
    ]);
  }

  // ----------------------------------------------------------- the editor
  /**
   * The writing desk: raw Markdown on the left, rendered on the right.
   * `save(text)` persists and returns the address to open afterwards.
   */
  function openEditor({ heading, initial, literature, bibtex, save, bibtexCheck, cardType, facet, emphasis }) {
    return new Promise((resolve) => {
      // Regular Cards choose a Card type; Topic and Literature Cards are always ideas.
      // A Model Card is the head of its model, or one of its parts when it has a facet: any
      // short label, with the common facets offered as suggestions. A part may be core or aside.
      let chosenType = cardType;
      let chosenFacet = facet || '';
      let chosenEmphasis = emphasis || '';
      const facetInput = cardType ? h('input', {
        class: 'facet-input', 'aria-label': 'Model facet', list: 'zt-facets', maxlength: 32, spellcheck: 'false',
        placeholder: 'whole model', value: chosenFacet,
        title: 'Empty: the Model Card of a whole model. A label (setting, 动态, a worked example…): a Model part filed under it',
        oninput: (event) => { chosenFacet = event.target.value.trim(); dirty = true; emphasisPick.hidden = !chosenFacet; update(); },
      }) : null;
      const emphasisPick = cardType ? h('select', {
        class: 'emphasis-pick', 'aria-label': 'Emphasis', hidden: !chosenFacet,
        title: 'How much of a reader\'s attention this part asks for',
        onchange: (event) => { chosenEmphasis = event.target.value; dirty = true; update(); },
      },
        h('option', { value: '', text: 'regular' }),
        Object.entries(EMPHASIS).map(([id, info]) => h('option', { value: id, text: id, title: info.note })),
      ) : null;
      if (emphasisPick) emphasisPick.value = chosenEmphasis;
      const facetPick = cardType ? h('span', { class: 'facet-pick', hidden: cardType !== 'model' },
        h('span', { class: 'lbl', text: 'Part' }), facetInput, emphasisPick,
        h('datalist', { id: 'zt-facets' }, Object.entries(FACETS).map(([id, info]) => h('option', { value: id, label: info.note }))),
      ) : null;
      const typePick = cardType ? h('div', { class: 'type-pick', role: 'radiogroup', 'aria-label': 'Card type' },
        h('span', { class: 'lbl', text: 'Type' }),
        Object.entries(CARD_TYPES).map(([id, info]) => h('button', {
          class: id + (id === cardType ? ' on' : ''), role: 'radio', 'aria-checked': id === cardType ? 'true' : 'false',
          title: `${info.label} Card — ${info.note}`, text: info.label, dataset: { type: id },
          onclick: (event) => {
            chosenType = id;
            dirty = dirty || id !== cardType;
            for (const button of event.currentTarget.parentNode.querySelectorAll('button')) {
              button.classList.toggle('on', button.dataset.type === id);
              button.setAttribute('aria-checked', button.dataset.type === id ? 'true' : 'false');
            }
            facetPick.hidden = id !== 'model';
            update();
          },
        })),
        facetPick,
      ) : null;
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
          preview.prepend(h('h1', { class: 'title' + (title ? '' : ' untitled') }, title ? titleNodes(title) : 'Untitled — start with “# Title”'));
          const stamp = chosenType && typeStamp(chosenType, null, chosenType === 'model' ? chosenFacet : null, chosenFacet ? chosenEmphasis : null);
          if (stamp) preview.prepend(h('div', { class: 'card-meta' }, stamp));
          wireMarkdown(preview);
          if (!bib || !msg.classList.contains('err')) setMsg(rendered.error || (literature ? 'The title comes from BibTeX.' : ''), rendered.error ? 'err' : null);
        } catch (error) { setMsg(error.message, 'err'); }
      }, 160);

      const close = openOverlay(h('div', { class: 'sheet-dlg', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: heading }), typePick),
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
          const address = await save(text.value, bib ? bib.value : undefined, chosenType, chosenType === 'model' ? chosenFacet : '', chosenType === 'model' && chosenFacet ? chosenEmphasis : '');
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
    const heading = kind === 'topic' ? `New Topic · ${wsLabel(state.workspace)} workspace`
      : kind === 'direct' ? `New direct successor of ${parent.address}` : `New side successor of ${parent.address}`;
    // Same template as the terminal: heading, blank line, body line, marker.
    const initial = `# ${kind === 'topic' ? title || '' : ''}\n\n\n<!-- zt:reverse-links -->\n`;
    const address = await openEditor({
      heading,
      initial,
      // Every successor is a Regular Card, so it may be an Event or Model Card.
      cardType: kind === 'topic' ? null : 'idea',
      save: (text, _bibtex, type, facet, emphasis) => api('create', { kind, at: parent?.address, text, workspace: state.workspace, type: type || undefined, facet: facet || undefined, emphasis: emphasis || undefined }).then((result) => result.address),
    });
    if (address) await afterChange(address, `created ${address}`);
  }

  async function newLiterature() {
    const address = await openEditor({
      heading: `New Literature Card · ${wsLabel(state.workspace)} workspace`,
      initial: '',
      literature: {},
      bibtex: '',
      bibtexCheck: async (bibtex) => {
        const checked = await api('literature/check', { bibtex });
        if (checked.exists) return { ...checked, warn: true, note: `Literature Card \`${checked.citation_key}\` already exists` };
        return { ...checked, note: `Citation key ${checked.citation_key} · “${checked.title}”` };
      },
      save: (text, bibtex) => api('literature', { bibtex, text, workspace: state.workspace }).then((result) => result.address),
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
      cardType: card.context === 'regular' ? card.type || 'idea' : null,
      facet: card.facet,
      emphasis: card.emphasis,
      save: (text, _bibtex, type, facet, emphasis) => api('edit', {
        at: card.address, text, base: card.text, type: type || undefined,
        facet: type === 'model' ? facet || '' : undefined, emphasis: type === 'model' ? emphasis || '' : undefined,
      }).then((result) => result.address),
    });
    if (address) await afterChange(address, `saved ${address}`);
  }

  /** `ty <idea|event|model> [<facet>]`: change only the current Regular Card's type and facet. */
  async function setCardType(type, facet) {
    const card = state.card;
    if (!CARD_TYPES[type]) throw new Error(`unknown card type \`${type}\`; expected idea, event, or model`);
    if (card.context !== 'regular') throw new Error('only Regular Cards can be Event or Model Cards');
    if (facet && type !== 'model') throw new Error('only Model Cards can have a Model facet');
    const result = await api('edit', { at: card.address, type, facet: type === 'model' ? facet || '' : undefined });
    await afterChange(result.address, facet
      ? `${result.address} is the ${facetLabel(facet)} part of its model`
      : `${result.address} is ${type === 'idea' ? 'an Idea' : type === 'event' ? 'an Event' : 'a Model'} Card`);
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
        case 'w': return run(() => setWorkspace(state.workspace === 'agent' ? 'human' : 'agent', { toRoot: true }));
        case 'f': return run(() => fitView());
        case 'x': return state.card && state.mode === 'map' ? run(() => setExpandAll(!state.expandAll)) : undefined;
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
      wireCharts(document);
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
    $('#lens-toggle').addEventListener('click', () => setExpandAll(!state.expandAll));
    $('#quit').addEventListener('click', () => quit().catch(fail));
    $('#broken-pill').addEventListener('click', () => showBroken().catch(fail));
    $('#search').addEventListener('input', (event) => runSearch(event.target.value));
    for (const button of document.querySelectorAll('[data-action]')) {
      button.addEventListener('click', () => (button.dataset.action === 'new-topic' ? newCard('topic', '') : newLiterature()).catch(fail));
    }
    window.addEventListener('hashchange', route);
    window.addEventListener('resize', debounce(() => renderStage(), 120));
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
