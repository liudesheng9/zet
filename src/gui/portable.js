/* zt portable export — the page's link to `cards.json`.
 *
 * This runs before `app.js`. It asks for the export, answers the page's `/api/*` requests through
 * `ZtPortableRules`, and starts the GUI when the data is in. A page opened from disk cannot read the
 * file beside it, so it tries `fetch` and otherwise asks the reader for the file.
 */
window.ZtPortable = (() => {
  'use strict';

  const rules = window.ZtPortableRules;
  const nativeFetch = window.fetch.bind(window);
  const byId = (id) => document.getElementById(id);
  let index = null;
  let begin;
  const ready = new Promise((ok) => { begin = ok; });

  const stored = {
    key: () => `zt-portable-spreads:${index.data.tree.id}:${index.data.exported?.archive_revision}`,
    read() {
      try { return JSON.parse(localStorage.getItem(this.key())); } catch { return null; }
    },
    write(value) {
      try { localStorage.setItem(this.key(), JSON.stringify(value)); } catch { /* a page keeps working without it */ }
    },
  };

  /** A figure's image, for the page's figure dialog. */
  const figure = (sha) => rules.figureSource(index?.data.artifacts?.[sha]) ?? '';

  /**
   * Card HTML as the GUI expects it, holding only what the renderer writes: the file may come from
   * someone else, and the export's Links are leaner than the service's.
   */
  function dress(html) {
    const parsed = new DOMParser().parseFromString(html || '', 'text/html');
    for (const node of [...parsed.body.querySelectorAll('*')]) {
      const tag = node.tagName.toLowerCase();
      if (!rules.keepTag(tag)) {
        node.remove();
        continue;
      }
      for (const { name, value } of [...node.attributes]) {
        if (!rules.keepAttr(tag, name.toLowerCase(), value)) node.removeAttribute(name);
      }
    }
    for (const link of parsed.body.querySelectorAll('a.zt-link')) {
      const facts = rules.linkFacts(link.getAttribute('href'), link.getAttribute('title'));
      if (!facts) continue;
      link.dataset.target = facts.target;
      if (link.classList.contains('zt-term')) link.dataset.meaning = facts.meaning;
    }
    for (const node of parsed.body.querySelectorAll('.zt-figure')) {
      const source = figure(node.dataset.artifact);
      if (source) node.querySelector('img')?.setAttribute('src', source);
      else node.dataset.artifact = '';
    }
    return parsed.body.innerHTML;
  }

  /** The service's answer, from the export. */
  function answer(url, options = {}) {
    const method = options.method || 'GET';
    const body = options.body ? JSON.parse(options.body) : undefined;
    const reply = rules.respond(index, method, url, body);
    if (reply.status === 200 && typeof reply.body.html === 'string') {
      reply.body = { ...reply.body, html: dress(reply.body.html) };
    }
    if (method === 'POST' && url.startsWith('/api/spreads')) stored.write(reply.body);
    return new Response(JSON.stringify(reply.body), {
      status: reply.status,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  window.fetch = async (input, options) => {
    const url = typeof input === 'string' ? input : input.url;
    return index && url.startsWith('/api/') ? answer(url, options) : nativeFetch(input, options);
  };

  const chartParts = () => ({
    frame: byId('zt-chart-frame').textContent,
    katexCss: byId('zt-katex-css').textContent,
    katexJs: byId('zt-katex').textContent,
  });

  /** A chart runs in a sandboxed frame of its own, with an opaque origin. */
  function fillChart(frame, theme) {
    const page = index?.data.charts?.[frame.dataset.chart];
    if (typeof page !== 'string') {
      frame.replaceWith(document.createTextNode('chart not in this export'));
      return;
    }
    frame.setAttribute('sandbox', 'allow-scripts');
    frame.srcdoc = rules.chartDocument(page, theme, chartParts());
  }

  function slip(note) {
    byId('zt-slip').hidden = false;
    if (note) {
      byId('zt-slip-note').textContent = note;
      byId('zt-slip').classList.add('refused');
    }
  }

  function start(data) {
    const why = rules.refusal(data);
    if (why) {
      slip(why);
      return;
    }
    index = rules.open(data);
    index.spreads = stored.read() ?? index.spreads;
    document.title = `${data.tree.title} · portable`;
    byId('zt-slip').hidden = true;
    if (!location.hash.startsWith('#/card/')) {
      history.replaceState(null, '', `#/card/${encodeURIComponent(data.tree.root)}`);
    }
    begin();
  }

  async function chooseFile(file) {
    if (!file || index) return;
    let data;
    try {
      data = JSON.parse(await file.text());
    } catch {
      slip('This file is not a ZET export.');
      return;
    }
    start(data);
  }

  async function load() {
    byId('zt-slip-file').addEventListener('change', (event) => chooseFile(event.target.files[0]));
    document.addEventListener('dragover', (event) => { if (!index) event.preventDefault(); });
    document.addEventListener('drop', (event) => {
      if (index) return;
      event.preventDefault();
      chooseFile(event.dataTransfer.files[0]);
    });
    let data = null;
    try {
      const reply = await nativeFetch('cards.json');
      if (reply.ok) data = await reply.json();
    } catch { /* a page opened from disk cannot read the file beside it */ }
    if (data) start(data);
    else slip();
  }

  load();
  /** A figure's explanation, copied from the Scholar Archive when the tree was exported. */
  const explanation = (sha) => index?.data.artifacts?.[sha]?.explanation ?? null;

  return { ready, figure, explanation, fillChart, notice: rules.notice, elsewhere: rules.elsewhere };
})();
