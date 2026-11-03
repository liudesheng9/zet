# GUI benchmarks (#246)

Two bounded tools that measure the GUI with very large trees. Run them on a test box, never on the live box. On 2026-11-02 a probe of the live GUI, together with stale digest watchers, locked the whole machine (#238).

## `lens-bench.mjs`: the Map's lens, without a browser

```bash
node scripts/gui-bench/lens-bench.mjs graph.json --bound 400
```

The input is a saved `/api/graph` answer (or one `/api/tree` answer). For every tree it reports the largest Map lens over every focus, the focus that gives it, and the size of the Whole tree view. It exits 1 when a lens is above `--bound`. `--every n` measures every n-th Card of a large tree. It runs `src/gui/layout.js` itself (`--layout` names another copy).

## `probe.js`: Map moves in a test box's page

Load the file into the GUI page of a test box, then run `await ztProbe(['B/1|a|5', 'B/1|a|6'])`. For each move it reports the main-thread time blocked (long tasks), the cards drawn, the bundles, and the page's element count. It refuses port 4717 (the live box), takes at most 12 moves, and never unfolds the whole tree. Keep the browser pane visible: a hidden pane throttles its frames, so only the long tasks are valid there.

## A test box

1. Copy the trees you need with `scripts/book-digest/copy-tree.mjs --tree <key> --to-config <dir>` (read tools only on the live box). Do not copy `D:\zt\zt.sqlite3`.
2. Copy the `zt.exe` under test into `<dir>\bin` (the running service locks the build folder's exe).
3. Start the box's own service with its own config and port:

   ```powershell
   $env:ZT_CONFIG_DIR = '<dir>'; $env:ZT_GUI_PORT = '48777'; & '<dir>\bin\zt.exe' up
   ```

4. Open `http://127.0.0.1:48777/`, run the probe, then `zt down` with the same environment and remove the box (Recycle Bin).

Check free commit memory before a build or a probe, and run one heavy thing at a time.
