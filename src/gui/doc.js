// The reading page of a Scholar Archive document: typeset TeX as it nears the screen, and bring
// a figure a Card opened (`#fig-…`) to the middle of the screen once its image has loaded.
(() => {
  const typeset = (node) => {
    if (node.dataset.typeset || !window.katex) return;
    node.dataset.typeset = '1';
    try {
      window.katex.render(node.textContent, node, { displayMode: node.classList.contains('math-display'), throwOnError: false });
    } catch { /* leave the TeX as text */ }
  };
  const nodes = document.querySelectorAll('span.math-inline, span.math-display');
  if ('IntersectionObserver' in window) {
    const watcher = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        typeset(entry.target);
        watcher.unobserve(entry.target);
      }
    }, { rootMargin: '1200px 0px' });
    nodes.forEach((node) => watcher.observe(node));
  } else {
    nodes.forEach(typeset);
  }
  // Layout moves while figures load and TeX sets, so centre again once the target's image and
  // then the page have loaded, and whenever the fragment changes.
  const center = () => {
    const id = decodeURIComponent(location.hash.slice(1));
    const target = id && document.getElementById(id);
    if (!target) return;
    target.scrollIntoView({ block: 'center' });
    if (target.tagName === 'IMG' && !target.complete) {
      target.loading = 'eager';
      target.addEventListener('load', () => target.scrollIntoView({ block: 'center' }), { once: true });
    }
  };
  center();
  window.addEventListener('load', center, { once: true });
  window.addEventListener('hashchange', center);
})();
