/* zt gui lock — pure, DOM-free rules for the Tree Lock panel and its dialogs (S01, #124), for
 * which writes a locked or pinned Card offers (S05, #128), and for how current and latched
 * integrity faults read and keep the owner's recovery reachable (S06, #129).
 *
 * The owner locks a whole tree, edits a Lock's note, and unlocks it only through the GUI routes
 * `POST /api/lock`, `POST /api/lock/note` and `POST /api/unlock`: each answers a plan first and
 * commits only with the plan's confirmation word and `plan_hash`. `GET /api/locks` answers every
 * Lock with its members, pins, history and the current integrity result. These rules decide what
 * the page shows from those answers and when a dialog's plan is still the one the owner reviews.
 * The server recomputes every fact; nothing here is authoritative. `draft.js`, `ask.js` and
 * `features.js` export the same way, so this module runs in the page and under Node.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.ZtLock = api;
})(typeof self !== 'undefined' ? self : this, () => {
  'use strict';

  /** The longest note a Lock keeps, in Unicode scalar values (the server's own bound). */
  const NOTE_LIMIT = 500;

  /** The seal as the page shows it: the first 12 characters of the full lowercase digest. */
  function sealShort(seal) {
    return String(seal || '').slice(0, 12);
  }

  /**
   * A note as the owner typed it, checked the way the server checks it: at most 500 Unicode scalar
   * values (counted by code point, so one emoji is one), no NUL character, and a blank note means
   * no note (`null`). `count` is what the counter under the field shows.
   */
  function noteCheck(raw) {
    const text = raw == null ? '' : String(raw);
    const count = Array.from(text).length;
    if (text.includes('\u0000')) return { ok: false, note: null, count, error: 'a Lock note cannot contain a NUL character' };
    if (count > NOTE_LIMIT) return { ok: false, note: null, count, error: `a Lock note is at most ${NOTE_LIMIT} characters` };
    return { ok: true, note: text.trim() ? text : null, count, error: null };
  }

  /**
   * Whether the plan in a dialog is still the one the owner may confirm for the note now in the
   * field. A Lock or note plan binds its note, so any change to the note needs a new plan first;
   * an unlock plan carries no note of its own, so only a missing plan is stale.
   */
  function planCurrent(plan, action, rawNote) {
    if (!plan || !plan.plan_hash) return false;
    if (action === 'unlock') return true;
    const checked = noteCheck(rawNote);
    if (!checked.ok) return false;
    return (plan.note == null ? null : plan.note) === checked.note;
  }

  /**
   * What one answer of a Lock route means for its dialog: `applied` (committed), `changed` (the
   * hash was stale; the answer holds the renewed plan and nothing was written) or `plan` (a plan to
   * review).
   */
  function answer(response) {
    if (response && response.applied === true) return 'applied';
    if (response && response.plan_changed === true) return 'changed';
    return 'plan';
  }

  const CODES = {
    locked: 'locked card',
    locked_tree: 'locked tree',
    pinned: 'pinned target',
    index_integrity: 'link index mismatch',
  };

  /**
   * The line a refused write shows: the refusal code first, so the page names why, then the
   * server's own message, which names the Locked tree, the Lock and the pinning source Cards.
   * `null` when the error is not a Lock refusal.
   */
  function refusalText(refusal, message) {
    if (!refusal || !refusal.code) return null;
    const said = message || refusal.error || '';
    return `${refusal.code} (${CODES[refusal.code] || 'lock refusal'}): ${said}`;
  }

  /** The tree a refusal names, so the page can open its lock panel; `null` when none. */
  function refusalTree(refusal) {
    if (!refusal) return null;
    if (refusal.locked_tree) return refusal.locked_tree;
    const first = (refusal.pinned_by || [])[0];
    return first ? first.tree : null;
  }

  /** The active Lock of one Tree root in a `GET /api/locks` answer, else `null`. */
  function activeLock(exported, rootAddress) {
    const locks = (exported && exported.locks) || [];
    return locks.find((lock) => lock.root === rootAddress && !lock.unlocked_at) || null;
  }

  /**
   * Whether a recorded fault (a latch) is the record of one current fault: the same identifier, or
   * for a schema fault (no Lock of its own, latched on every active Lock) the same kind and subject.
   */
  function sameFault(fault, latch) {
    if (!fault || !latch) return false;
    if (fault.id && fault.id === latch.id) return true;
    return fault.lock_id == null && latch.kind === fault.kind && (latch.card ?? null) === (fault.card ?? null);
  }

  /**
   * The panel's reading of a `GET /api/locks` answer: active Locks and ended ones, each in
   * creation order (Lock id); the integrity faults of each Lock; and the faults that belong to no
   * listed Lock (a schema fault). A fault never hides a Lock or a Card; it is shown beside it.
   *
   * S06 (#129): `integrity.latched` is recorded evidence that a later clean comparison never
   * erases. Each Lock's list merges its current faults and its latches into one row per fault: a
   * current fault has `current: true` and, once recorded, its record's `at`; a latch the current
   * comparison no longer sees has `current: false` and its `at`. The latch of a current schema
   * fault is that same fault, so it is counted once. `broken` names each active Lock whose seal is
   * broken: by a current or latched fault of its own, by a current schema fault, or by the route's
   * own `seal_intact: false`. The seal itself is never read from anything but the Lock.
   */
  function panel(exported) {
    const locks = ((exported && exported.locks) || []).slice().sort((a, b) => a.lock_id - b.lock_id);
    const integrity = (exported && exported.integrity) || { ok: true, faults: [] };
    const faults = integrity.faults || [];
    const latched = integrity.latched || [];
    const ids = new Set(locks.map((lock) => lock.lock_id));
    const faultsOf = {};
    const loose = [];
    const recordOf = (fault, lockId) => latched.find((latch) => (lockId == null || latch.lock_id === lockId) && sameFault(fault, latch));
    for (const fault of faults) {
      const own = fault.lock_id != null && ids.has(fault.lock_id);
      const record = recordOf(fault, own ? fault.lock_id : null);
      const row = { ...fault, current: true, at: record ? record.at : null };
      if (own) (faultsOf[fault.lock_id] ||= []).push(row);
      else loose.push(row);
    }
    let latchedOnly = 0;
    for (const latch of latched) {
      const current = faults.find((fault) => sameFault(fault, latch));
      if (current && current.lock_id != null) continue; // already that Lock's current row
      if (!current) latchedOnly += 1;
      const row = { ...latch, current: Boolean(current) };
      if (ids.has(latch.lock_id)) (faultsOf[latch.lock_id] ||= []).push(row);
      else if (!current) loose.push(row);
    }
    const schemaNow = faults.some((fault) => fault.lock_id == null);
    const active = locks.filter((lock) => !lock.unlocked_at);
    const broken = {};
    for (const lock of active) {
      broken[lock.lock_id] = Boolean((faultsOf[lock.lock_id] || []).length) || schemaNow || lock.seal_intact === false;
    }
    const faultCount = faults.length + latchedOnly;
    return {
      ok: integrity.ok !== false && !faultCount,
      active,
      ended: locks.filter((lock) => lock.unlocked_at),
      faultsOf,
      loose,
      broken,
      faultCount,
      latchedCount: latchedOnly,
    };
  }

  /**
   * The one page-level line for integrity faults, or `null` when every Lock is intact. It counts
   * the current faults and the latches no current fault repeats, and names the latched share.
   */
  function faultLine(exported) {
    const read = panel(exported);
    if (!read.faultCount) return null;
    const latched = read.latchedCount ? ` (${read.latchedCount} latched)` : '';
    return `${read.faultCount} lock fault${read.faultCount === 1 ? '' : 's'}${latched}`;
  }

  /**
   * One fault row's words: what is wrong, where, and when (S06, #129). A current fault reads
   * `now`, then its record's time once it is recorded; a latch reads `latched · recorded <at>`.
   */
  function faultText(row) {
    const at = row && row.at ? String(row.at).replace('T', ' ').replace(/:\d\d(\.\d+)?Z$/, ' UTC') : null;
    const when = row && row.current
      ? (at ? `now · recorded ${at}` : 'now')
      : `latched · recorded ${at || 'earlier'}`;
    return {
      kind: String((row && row.kind) || 'fault'),
      where: [row && row.card, row && row.detail].filter(Boolean).join(' · '),
      when,
    };
  }

  /**
   * The actions of one active Lock's entry in the panel, in order (S06, #129). `Open tree` needs
   * the root Card; `Edit note…` and `Unlock tree…` act on the Lock itself, so they stay reachable
   * when the root Card is gone or the Lock is faulted: the owner's recovery never depends on the
   * damaged tree. An ended Lock offers nothing.
   */
  function entryActions(lock, rootPresent) {
    if (!lock || lock.unlocked_at) return [];
    return [
      rootPresent ? { id: 'open', label: 'Open tree' } : null,
      { id: 'note', label: 'Edit note…' },
      { id: 'unlock', label: 'Unlock tree…', danger: true },
    ].filter(Boolean);
  }

  /**
   * A Lock's pins grouped by distinct target, each with the locked source Cards that link it. The
   * stored pin rows name source rows; the Lock's own members name their addresses.
   */
  function pinGroups(lock) {
    const addressOf = new Map(((lock && lock.members) || []).map((member) => [member.row_id, member.address]));
    const groups = new Map();
    for (const pin of (lock && lock.pins) || []) {
      const group = groups.get(pin.target) || { target: pin.target, target_row: pin.target_row, sources: [] };
      // A plan's pin names its sources; a stored pin row names one source row.
      const sources = Array.isArray(pin.sources) ? pin.sources : [addressOf.get(pin.source_row) || `row ${pin.source_row}`];
      for (const item of sources) if (!group.sources.includes(item)) group.sources.push(item);
      groups.set(pin.target, group);
    }
    return [...groups.values()];
  }

  /** One history line for a Lock event: its time, its action and the facts that matter. */
  function eventLine(event) {
    const detail = (event && event.detail && typeof event.detail === 'object') ? event.detail : {};
    const at = String((event && event.at) || '').replace('T', ' ').replace(/\.\d+Z$/, 'Z');
    switch (event && event.action) {
      case 'lock':
        return `${at} · locked ${detail.member_count ?? '?'} cards · seal ${sealShort(detail.seal)}`;
      case 'unlock':
        return `${at} · unlocked · released ${detail.member_count ?? '?'} cards and ${detail.pin_count ?? '?'} pinned targets`;
      case 'note':
        return `${at} · note: ${detail.previous_note ?? 'none'} → ${detail.note ?? 'none'}`;
      case 'reseal':
        return `${at} · resealed · seal ${sealShort(detail.seal)}`;
      case 'integrity-fault': {
        // The record keeps the fault's identifier and its evidence (S06, #129).
        const evidence = detail.evidence && typeof detail.evidence === 'object' ? detail.evidence : {};
        const said = [evidence.kind, evidence.card, evidence.detail].filter(Boolean).join(' · ');
        return `${at} · integrity fault · ${said || detail.fault || ''}`;
      }
      default:
        return `${at} · ${(event && event.action) || 'event'}`;
    }
  }

  /**
   * The quiet Lock facts one Card carries (S03, #126). Every Card projection answers `lock` (null,
   * or `{lock_id, tree, since}` with `tree` the Locked tree's root address) and `pinned_by` (the
   * sorted Locked roots whose Cards link it, `[]` when none). Answers `null` for a Card with
   * neither, else what the reader's lock line says: a Locked Card's tree and since, with why its
   * text waits for an unlock; a Pinned target's roots, with what the pin leaves open. A Card can be
   * both. `root` is true when the Card is its own Locked tree's root, whose own line (the seal and
   * the unlock action) already says it.
   */
  function cardState(card) {
    if (!card || typeof card !== 'object') return null;
    const lock = card.lock && typeof card.lock === 'object' && card.lock.tree ? card.lock : null;
    const pinnedBy = Array.isArray(card.pinned_by) ? [...new Set(card.pinned_by.map(String))] : [];
    if (!lock && !pinnedBy.length) return null;
    const one = pinnedBy.length === 1;
    const roots = pinnedBy.join(', ');
    return {
      locked: Boolean(lock),
      root: Boolean(lock) && lock.tree === card.address,
      tree: lock ? String(lock.tree) : null,
      lockId: lock ? lock.lock_id : null,
      since: lock ? lock.since || null : null,
      lockedWhy: lock ? `Its text waits until tree ${lock.tree} is unlocked; it stays readable, selectable and linkable.` : null,
      pinnedBy,
      pinnedText: pinnedBy.length ? `Pinned by locked ${one ? 'tree' : 'trees'} ${roots}` : null,
      pinnedWhy: pinnedBy.length
        ? `Editing stays open. Moving or deleting it is refused while ${one ? 'that Locked tree holds a Link' : 'those Locked trees hold Links'} to it.`
        : null,
    };
  }

  /**
   * The writes a Card offers (S05, #128), each with the words an explanation uses. Every entry
   * point — the reader's buttons, the keys, the command bar and the menus — asks `writeGate` with
   * one of these names, so they all honour the same state.
   */
  const WRITES = {
    edit: 'edit it',
    bibtex: 'edit its BibTeX',
    type: 'change its type',
    level: 'set its level',
    form: 'set its Math form',
    direct: 'add a direct successor to it',
    side: 'add a side successor to it',
    insert: 'insert a card in its place (that moves it down)',
    move: 'move it',
    delete: 'delete it',
  };

  /** The writes that move or remove the Card itself: the only ones a pin refuses. */
  const PIN_REFUSES = ['insert', 'move', 'delete'];

  /** The command-bar words that write the open Card, by the write they ask for. */
  const COMMAND_WRITES = { e: 'edit', ty: 'type', lv: 'level', mf: 'form', n: 'direct', b: 'side', ins: 'insert', mv: 'move', del: 'delete' };

  /**
   * Which active Locks pin one target, and through which of their source Cards (from a
   * `GET /api/locks` answer). A root the answer does not list keeps its name with no sources.
   */
  function pinHolders(exported, target, roots) {
    const locks = (exported && exported.locks) || [];
    return (roots || []).map((root) => {
      const lock = locks.find((item) => item.root === root && !item.unlocked_at);
      const group = lock ? pinGroups(lock).find((item) => item.target === target) : null;
      return { tree: root, lockId: lock ? lock.lock_id : null, sources: group ? group.sources : [] };
    });
  }

  /**
   * Whether the page offers one write on one Card, and if not, why in words (S05, #128). A Card in
   * a Locked tree (its root included) refuses every write until that tree is unlocked. A Pinned
   * target keeps editing and new successors (the server checks what a successor would really
   * move); only moving, inserting in its place and deleting it are refused, naming the Locked
   * trees that pin it and the source Cards that hold the Links. The server still decides; this
   * only keeps the page from offering what it would refuse.
   */
  function writeGate(card, action, exported) {
    const verb = WRITES[action];
    if (!verb) throw new Error(`unknown write \`${action}\``);
    const facts = cardState(card);
    if (!facts) return { allowed: true };
    if (facts.locked) {
      return {
        allowed: false, code: 'locked', tree: facts.tree, lockId: facts.lockId, holders: [],
        short: `locked with tree ${facts.tree}`,
        why: `${card.address} is locked with tree ${facts.tree}: you cannot ${verb} until that tree is unlocked. Unlock it from the tree's lock panel.`,
      };
    }
    if (facts.pinnedBy.length && PIN_REFUSES.includes(action)) {
      const holders = pinHolders(exported, card.address, facts.pinnedBy);
      const one = holders.length === 1;
      const named = holders.map((item) => (item.sources.length ? `${item.tree} (Links from ${item.sources.join(', ')})` : item.tree)).join('; ');
      return {
        allowed: false, code: 'pinned', tree: holders[0].tree, lockId: holders[0].lockId, holders,
        short: `pinned by locked ${one ? 'tree' : 'trees'} ${facts.pinnedBy.join(', ')}`,
        why: `${card.address} is pinned by locked ${one ? 'tree' : 'trees'} ${named}: you cannot ${verb} while ${one ? 'that tree holds' : 'those trees hold'} Links to it. Editing it and adding successors stay open; unlocking a pinning tree in its lock panel releases its pin.`,
      };
    }
    return { allowed: true };
  }

  /**
   * Whether a reply may be filed in one human Topic: `null` when it may, else why not. A Topic
   * inside a Locked tree takes no new Card; a pin alone leaves it open. The page never picks
   * another Topic in its place and never unlocks one.
   */
  function destinationLock(topic) {
    const facts = cardState(topic);
    if (!facts || !facts.locked) return null;
    return {
      tree: facts.tree,
      why: `Topic ${topic.address} is in locked tree ${facts.tree}: a reply cannot be filed there until it is unlocked. Choose another Topic; your draft stays.`,
    };
  }

  /**
   * The actions of one tree's cabinet menu, in order: open it, then what its Lock state allows.
   * An active Lock offers its panel, its note and its unlock; an unlocked tree offers Lock tree. A
   * tree the overview calls locked while the Locks are unread offers only the panel, which reads them.
   */
  function treeActions(root, locked, exported) {
    const open = { id: 'open', label: 'Open tree' };
    const lock = exported ? activeLock(exported, root) : null;
    if (lock) {
      return [open,
        { id: 'panel', label: 'Lock panel', note: `seal ${sealShort(lock.seal)}` },
        { id: 'note', label: 'Edit lock note…' },
        { id: 'unlock', label: 'Unlock tree…', danger: true }];
    }
    if (locked) return [open, { id: 'panel', label: 'Lock panel', note: 'locked' }];
    return [open, { id: 'lock', label: 'Lock tree…', note: 'review, then freeze' }];
  }

  /** The words on a Locked tree's band: the state first, then the seal's 12 characters. */
  function bandText(lock) {
    return lock && lock.seal ? `locked · seal ${sealShort(lock.seal)}` : 'locked';
  }

  return {
    NOTE_LIMIT, sealShort, noteCheck, planCurrent, answer, refusalText, refusalTree, activeLock, sameFault, panel, faultLine, faultText, entryActions, pinGroups, eventLine, cardState,
    WRITES, COMMAND_WRITES, pinHolders, writeGate, destinationLock, treeActions, bandText,
  };
});
