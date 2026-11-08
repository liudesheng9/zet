#!/usr/bin/env bash
# The Cited backfill of one finished book whose reading jobs have all passed (2026-11-07,
# docs/cited-list-reading-proposal.md): write the list, carry the live Cards, write the new rows' Cards,
# publish them, point recorded Links back to their works, and run the Term-link pass.
#
#   bash cited-backfill.sh <staging> <key> <inbound.json>
set -euo pipefail
S="$1"; K="$2"; IN="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
last() { tail -1; }
# The plan keeps the headings it was made with (morony2005iraq: AUTHOR INDEX); a plan without them is another plan
# and would discard every reading answer.
H=$(node -e "try{process.stdout.write(require(process.argv[1]+'/cited/plan.json').heading||'')}catch(e){}" "$S")
if [ -n "$H" ]; then plan=$(zt-digest cited -Step plan -Staging "$S" -Heading "$H" | last); else plan=$(zt-digest cited -Step plan -Staging "$S" | last); fi
echo "plan $plan"
echo "$plan" | grep -q '"jobs":0' || { echo "reading not finished: $plan"; exit 1; }
listed=$(zt-digest cited -Step apply -Staging "$S" -Apply 2>&1 | last)
echo "list $listed"
echo "$listed" | grep -q '"ok":true' || { echo "list refused"; exit 1; }
echo "cards-plan $(zt-digest cited-cards -Step plan -Staging "$S" | last)"
node "$HERE/cited-read-drive.mjs" "$S" --cards --parallel 8 | last
check=$(zt-digest cited-cards -Step check -Staging "$S" | last)
echo "check $check"
echo "$check" | grep -q '"not_ready":0' || { echo "rows not ready"; exit 1; }
applied=$(zt-digest cited-cards -Step apply -Staging "$S" -Apply 2>&1 | last)
echo "apply $applied"
echo "$applied" | grep -q '"complete":true' || { echo "publication incomplete"; exit 1; }
echo "relink $(node "$HERE/cited-relink.mjs" apply "$IN" "$K" | last)"
echo "terms $(pwsh -NoProfile -File "$HERE/term-relink.ps1" -CitationKey "$K" 2>&1 | last)"
