// GUI probe (#246): a bounded measurement of Map moves, run inside a test box's GUI page.
//
// Paste this file into the page's console (or send it with a browser tool's JavaScript call),
// then run `await ztProbe(['<address>', '<address>', ...])`. Each address is opened in turn; the
// probe waits `settle` ms after each move and reports, per move, the long tasks (main-thread
// blocks of more than 50 ms), the Map's cards in the lens and on screen, and the page's element
// count. It refuses to run on port 4717, the live box's GUI: measure on a test box only
// (scripts/gui-bench/README.md). It takes at most 12 moves and never unfolds the whole tree.
(() => {
  const MAX_MOVES = 12;
  window.ztProbe = async function ztProbe(addresses, { settle = 1500 } = {}) {
    if (location.port === '4717') throw new Error('ztProbe refuses the live box (port 4717); use a test box');
    if (!Array.isArray(addresses) || !addresses.length) throw new Error('give the addresses to open');
    if (addresses.length > MAX_MOVES) throw new Error(`at most ${MAX_MOVES} moves`);
    const rows = [];
    for (const address of addresses) {
      const long = [];
      const observer = new PerformanceObserver((list) => { for (const entry of list.getEntries()) long.push(Math.round(entry.duration)); });
      try { observer.observe({ type: 'longtask', buffered: false }); } catch { /* no long-task timing */ }
      location.hash = '#/card/' + encodeURIComponent(address);
      await new Promise((done) => setTimeout(done, settle));
      observer.disconnect();
      const title = document.querySelector('#stage-title')?.textContent || '';
      rows.push({
        address,
        blocked: long.reduce((sum, ms) => sum + ms, 0),
        longest: long.length ? Math.max(...long) : 0,
        drawn: document.querySelectorAll('#nodes .node').length,
        bundles: document.querySelectorAll('#nodes .node.bundle').length,
        elements: document.querySelectorAll('*').length,
        title,
      });
    }
    return rows;
  };
})();
