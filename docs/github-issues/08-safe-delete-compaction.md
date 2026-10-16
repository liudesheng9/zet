## Parent

#1

## What to build

Implement safe delete for regular Cards and Topics, including verification charts, confirmations, Broken link behavior, side-successor compaction, link rewriting, Reverse link regeneration, and pointer movement.

## Acceptance criteria

- [ ] `zt del` deletes the current Card and successors from the Session command bar.
- [ ] Shell `zt del --at <location>` deletes the Card at `<location>` and successors.
- [ ] `zt del` can delete a Topic Card and removes the entire Topic.
- [ ] Topic deletion removes all Cards under the Topic and the Topic no longer exists.
- [ ] Before deletion, `zt del` shows a verification chart of Cards that will be deleted.
- [ ] The delete chart shows the number of successor Cards that will be deleted.
- [ ] Regular Card deletion requires typing `delete`.
- [ ] Topic deletion requires typing the Topic Location, for example `1/0`.
- [ ] Deleting a direct successor does not renumber direct successor Locations.
- [ ] Deleting a side successor moves later side-successor subtrees under the same parent to fill the side-label gap.
- [ ] Delete charts show side-successor compaction mappings.
- [ ] Side-successor compaction happens before Broken link reporting.
- [ ] Affected link macros are rewritten when side-successor compaction moves subtrees.
- [ ] Only links to actually deleted Cards become Broken links.
- [ ] Delete compaction regenerates Reverse link sections immediately as part of the delete operation.
- [ ] After deleting a side successor, the Session pointer lands on the parent Card.
- [ ] After deleting the current Card generally, the pointer moves to the parent Location if it still exists, otherwise `ROOT`.
- [ ] Delete runs as one SQLite transaction.

## Blocked by

#5
