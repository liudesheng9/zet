## Parent

#1

## What to build

Implement safe move for regular Card subtrees. Move must validate target Locations, preserve successor rules, rewrite Card Locations and link macros, regenerate Reverse links, and provide a confirmation chart.

## Acceptance criteria

- [ ] Session `zt mv <new-location>` moves the current Card and successors.
- [ ] Shell `zt mv --at <location> <new-location>` moves the Card at `<location>` and successors.
- [ ] `zt mv` rejects Topic Cards.
- [ ] `zt mv` rejects invalid target Locations.
- [ ] `zt mv` rejects moves that violate successor rules.
- [ ] `zt mv` rejects side-successor targets that skip the next available side label.
- [ ] `zt mv` rejects moving a Card into one of its own successor Locations.
- [ ] `zt mv` rejects when any destination Location already exists outside the moved subtree and shows conflicting destination Locations.
- [ ] Before applying a move, `zt mv` shows a verification chart.
- [ ] The move chart shows old Location to new Location mappings.
- [ ] The move chart shows number of moved Cards and number of link macros that will be rewritten.
- [ ] `zt mv` requires typing `move` to confirm.
- [ ] Moving rewrites link macros in stored Card text so links keep pointing to moved Cards.
- [ ] Moving regenerates Reverse link sections.
- [ ] After moving the current Card, the Session pointer moves to the moved Card's new Location.
- [ ] Move runs as one SQLite transaction.

## Blocked by

#5
