#Requires -Version 7.5
<#
.SYNOPSIS
  Audit the Cards of a digested book, or of every book in the agent Workspace. Read-only: the
  script makes only zt read calls and changes no Card. This script forwards to `zt-digest audit`
  (tools/zt-digest/src/audit.rs); callers should call the tool directly.

.DESCRIPTION
  Phase 1 of docs/digest-card-layers-proposal.md. For each book (an agent Literature tree), the
  script counts and lists:
    cards        the Cards of the book by category (root, anchor, idea, event, finding, model,
                 part, hub, terms, structure)
                 and Zone. A Card in a Topic tree is of the book whose Literature tree it links most.
                 A Link to the Literature Card or an anchor (a source line) counts 10, a Link to
                 another Card of that tree counts 1. A Topic-tree Card that links no book is
                 "unattributed".
    topics       the Topic trees that hold Cards of the book, and the trees where the book holds
                 most of the Cards.
    style        the Card language faults (zt-check, rules R2 to R21) per rule and per
                 category, with the standalone heuristics R20 and R21 counted apart as advice
                 (advisory and advice_by_rule). A model is checked with its own term table and level,
                 the other Cards of a book with the book's term table (a Literature-tree Card titled
                 术语表…) when it has one.
    layer        the Card layer faults (rules I1 to L3 of zt-check) per rule, with the
                 advisory rules (I1, I6, E5, L3, and I7's unknown-source line) counted apart. The
                 script gives the checker each Card's category, Links, kinded Links, parent, and
                 inbound Links.
    orphans      Regular Cards that no Card links to (inbound 0).
    no_source    idea, event, and finding Cards with no source line: no Link to the book's Literature Card or
                 to one of its anchors.
    unkinded     idea, event, and finding Cards with no kinded Link to a Card other than their parent, and
                 the count of Links that ZT reads as "refers" (Links to a Literature Card or an
                 anchor are source lines and do not count).
    models       each Model Card's level, parts, and build faults. A formal model is
                 self-sufficient when build.closure, build.forward, and build.unanchored are
                 empty. A concept model also needs an empty build.beyond_level.
    figures      Cards that name a figure (`图 2.C.1`) but do not show it (`![图 2.C.1](artifact:…)`).
                 "known" means another Card of the book shows that figure, so ZET holds it.
                 "unknown" means no Card of the book shows it.
    bridges      semantic Bridges: kinded Links (not "refers") between substantive Cards (idea,
                 finding, event, model, part) of this book and of another book, an unattributed
                 Topic-tree Card, or a human Card, out and in, by bucket (book-of.ps1 attribution).
                 island is true when there are none, null when a tree could not be read or no
                 substantive Card outside the book exists.
  It reads the trees of both Workspaces: human Cards are Bridge endpoints, never part of a book.
  For the Workspace it adds the hub counts, the missing hubs (list_hubs), the broken Links, and
  the islands.
  Writes audit.json, audit.md, audit-faults.txt (every Card language and Card layer fault), and
  audit-cards.json (every Card checked outside a model, with its body and Links) in -Out, and
  prints one compact JSON summary line last.

  Parameters: -CitationKey <key> (one book; empty: every agent Literature tree), -Out <folder>
  (required), -MaxSentences 12, -MaxChars 400, -MaxSteps 6, -Throttle 6, -Chunk 40, -Sample 15
  (addresses listed per finding in audit.md), -SkipModels (no get_model, no model checks).

  This script forwards every argument to `zt-digest audit`, which does the work in one process
  with one `zt mcp` and zt-check in-process. ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
& $exe audit @args
exit $LASTEXITCODE
