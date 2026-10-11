/* zt gui — the page inside a Card's HTML chart frame.
 *
 * A chart runs in a sandboxed frame (an opaque origin): it cannot reach the GUI page,
 * its token, or the API. This script gives it the card's look and talks to the page
 * only by postMessage: the chart's height, and the Card a reader clicked.
 */
(() => {
  'use strict';

  // The card's palette, as --zt-* tokens a chart can use in CSS, SVG fill/stroke, or JS.
  const style = document.createElement('style');
  style.textContent = `
:root {
  --zt-paper: #fffefa; --zt-ink: #1c1915; --zt-ink-soft: #5a5348; --zt-ink-faint: #9b9281;
  --zt-rule: #d3e0ee; --zt-line: #e3dacb; --zt-accent: #b4572a; --zt-accent-soft: #f6e5d9;
  --zt-link: #2c5aa8; --zt-link-soft: #e1e9f7; --zt-topic: #19776a; --zt-topic-soft: #dcefea;
  --zt-model: #26457d; --zt-model-soft: #e2e8f4; --zt-event: #b3262f; --zt-event-soft: #f8e0e1;
  --zt-series-1: #26457d; --zt-series-2: #b4572a; --zt-series-3: #19776a; --zt-series-4: #8a5a2b;
  --zt-series-5: #5a4fb0; --zt-series-6: #b3262f;
  --zt-serif: "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, "Noto Serif SC", "Source Han Serif SC", "Songti SC", "SimSun", serif;
  --zt-sans: "Inter", "Segoe UI Variable Text", system-ui, -apple-system, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif;
  --zt-mono: "JetBrains Mono", "Cascadia Code", "SF Mono", Consolas, monospace;
  color-scheme: light;
}
:root[data-theme="dark"] {
  --zt-paper: #1f2228; --zt-ink: #efe9de; --zt-ink-soft: #b7afa0; --zt-ink-faint: #7a7467;
  --zt-rule: #34404e; --zt-line: #2f333c; --zt-accent: #e88d55; --zt-accent-soft: #35261c;
  --zt-link: #93b5f5; --zt-link-soft: #1f2b42; --zt-topic: #5ec4b2; --zt-topic-soft: #18302c;
  --zt-model: #8fb0ea; --zt-model-soft: #1d2638; --zt-event: #f08a90; --zt-event-soft: #3b2224;
  --zt-series-1: #8fb0ea; --zt-series-2: #e88d55; --zt-series-3: #5ec4b2; --zt-series-4: #dcab76;
  --zt-series-5: #b0a6f0; --zt-series-6: #f08a90;
  color-scheme: dark;
}
html, body { margin: 0; padding: 0; background: transparent; }
body { color: var(--zt-ink); font: 14px/1.55 var(--zt-sans); overflow: hidden; }
#zt-chart { padding: 2px; }
#zt-chart > :first-child { margin-top: 0; }
#zt-chart > :last-child { margin-bottom: 0; }
svg { max-width: 100%; height: auto; }
svg text { fill: currentColor; font-family: var(--zt-sans); }
table { border-collapse: collapse; width: 100%; font-size: 13px; }
th, td { border-bottom: 1px solid var(--zt-line); padding: 5px 8px; text-align: left; vertical-align: top; }
th { color: var(--zt-ink-soft); font-weight: 600; }
figure { margin: 0; }
figcaption { color: var(--zt-ink-soft); font-size: 12.5px; margin-top: 6px; }
a.zt-link { color: var(--zt-link); background: var(--zt-link-soft); border-radius: 4px; padding: 0 4px;
  text-decoration: none; font-size: .92em; cursor: pointer; }
a.zt-link.broken { color: var(--zt-event); background: var(--zt-event-soft); cursor: default; }
[data-zt] { cursor: pointer; }
.katex { font-size: 1.05em; }
`;
  document.head.append(style);

  const links = window.ZT_LINKS || {};
  const go = (target) => parent.postMessage({ zt: 'go', target }, '*');

  /** `[[address]]` in the chart's text becomes a link to that Card. */
  function linkText(root) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode: (node) => (/\[\[[^\[\]\n]+\]\]/.test(node.data)
        && !node.parentElement.closest('script, style, textarea, code, pre')
        ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT),
    });
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    for (const node of nodes) {
      const inSvg = node.parentElement.closest('svg');
      if (inSvg) {
        // SVG text cannot hold HTML: show the address and make the text element a link.
        node.data = node.data.replace(/\[\[([^\[\]\n]+)\]\]/g, (_, target) => {
          node.parentElement.dataset.zt = target;
          return target;
        });
        continue;
      }
      const parts = node.data.split(/\[\[([^\[\]\n]+)\]\]/);
      const fragment = document.createDocumentFragment();
      parts.forEach((part, index) => {
        if (index % 2 === 0) { if (part) fragment.append(part); return; }
        const info = links[part];
        const anchor = document.createElement('a');
        anchor.className = 'zt-link' + (info && info.exists ? '' : ' broken');
        anchor.textContent = part;
        if (info && info.title) anchor.title = info.title;
        if (info && info.exists) anchor.dataset.zt = part;
        fragment.append(anchor);
      });
      node.replaceWith(fragment);
    }
  }

  /** TeX in `$...$`, `$$...$$`, `\(...\)`, or `\[...\]` inside the chart's HTML text. */
  function typeset(root) {
    if (!window.katex) return;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode: (node) => (/\$|\\\(|\\\[/.test(node.data)
        && !node.parentElement.closest('script, style, textarea, code, pre, svg, .katex')
        ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT),
    });
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    const pattern = /\$\$([\s\S]+?)\$\$|\\\[([\s\S]+?)\\\]|\$([^$\n]+?)\$|\\\(([\s\S]+?)\\\)/g;
    for (const node of nodes) {
      const text = node.data;
      let last = 0;
      let match;
      const fragment = document.createDocumentFragment();
      pattern.lastIndex = 0;
      while ((match = pattern.exec(text))) {
        if (match.index > last) fragment.append(text.slice(last, match.index));
        const display = match[1] != null || match[2] != null;
        const tex = match[1] ?? match[2] ?? match[3] ?? match[4];
        const span = document.createElement('span');
        try {
          window.katex.render(tex, span, { displayMode: display, throwOnError: false, output: 'htmlAndMathml' });
        } catch { span.textContent = match[0]; }
        fragment.append(span);
        last = pattern.lastIndex;
      }
      if (last === 0) continue;
      if (last < text.length) fragment.append(text.slice(last));
      node.replaceWith(fragment);
    }
  }

  let lastHeight = 0;
  function report() {
    const root = document.getElementById('zt-chart');
    if (!root) return;
    const height = Math.ceil(Math.max(root.scrollHeight, root.getBoundingClientRect().height)) + 4;
    if (Math.abs(height - lastHeight) < 2) return;
    lastHeight = height;
    parent.postMessage({ zt: 'height', height }, '*');
  }

  document.addEventListener('click', (event) => {
    const target = event.target.closest('[data-zt], a[href^="#/card/"]');
    if (!target) return;
    event.preventDefault();
    const address = target.dataset.zt
      || decodeURIComponent(target.getAttribute('href').slice('#/card/'.length));
    if (address) go(address);
  });

  document.addEventListener('DOMContentLoaded', () => {
    const root = document.getElementById('zt-chart');
    linkText(root);
    typeset(root);
    report();
    if (window.ResizeObserver) new ResizeObserver(report).observe(root);
    window.addEventListener('load', report);
    // Charts that draw late (fonts, async libraries) settle within a second or two.
    setTimeout(report, 300);
    setTimeout(report, 1500);
  });
})();
