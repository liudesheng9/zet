#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\link-gaps.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
<#
.SYNOPSIS
  The Link pass works only the receipt-owned semantic gaps of a run (issue #110).

.DESCRIPTION
  §5.5 selects a gap set from a completed group's own write receipts: an Idea Card with no valid
  kinded semantic Link to another group or relevant earlier material, a substantive orphan, and an
  anchor whose one coordinator-owned `在全书中的位置` paragraph is missing. This suite drives the
  real boundaries (prepare-link -> the claimed Link unit -> link-start's job file -> link-finish)
  and proves:

    * another book's Card with matching `§` numbers, an older Card that mentions one, and the host
      Card of a newly created Card are never owned or writable (AC02, AC03);
    * an older Card of another Literature tree stays unwritable: a digest writes only its own
      book's Literature tree (docs/literature-only-digest-design.md);
    * source locators, register membership and a parent-only Link do not close a semantic gap; a
      kinded Link to another group or earlier material does (AC01, AC04);
    * an empty gap set starts no provider job and still settles the Link unit (AC07);
    * a Link paragraph preserves the old body and formula/figure bytes; the anchor keeps one
      `在全书中的位置` paragraph, and a repeated completion adds no second copy (AC05, AC08);
    * `none` (no_link) is recorded with a reason; an orphan's unresolved required fault stays
      visible instead of inventing a relation (AC06);
    * a changed draft conflicts and a lost final reply recovers from the accepted plan (AC08).

  It starts no provider run and no workflow, and it does not touch the live archive.
#>
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-link-gaps-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$here = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
$repo = Join-Path $PSScriptRoot '..\..\..' | Resolve-Path | ForEach-Object Path
. (Join-Path $here 'zt-calls.ps1')
$out = Join-Path $scratch 'prepared'
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1 | ForEach-Object { "$_" })
    $json = $null
    if ($lines.Count) { try { $json = $lines[-1] | ConvertFrom-Json -AsHashtable -DateKind String } catch { } }
    [pscustomobject]@{ exit = $LASTEXITCODE; text = ($lines -join ' | '); json = $json }
}
function Run-Digest([string[]]$Arguments) {
    $got = Digest $Arguments
    if ($got.exit -ne 0) { throw "digest failed: $($got.text)" }
    $got.json
}
function Save-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 30), $utf8) }
function Revision { (Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } })).archive_revision }
function BodyOf($at) { (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } })).body }
function TextOf($at) { (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } })).text }
function PositionCount($body) { @(($body -split "`n`n") | Where-Object { $_.Trim().StartsWith('在全书中的位置') }).Count }

try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null

    # --- earlier material: an earlier book's Literature tree with an old Card and a cost argument, and
    # another book with the same section numbers as this run's source (docs/literature-only-digest-
    # design.md: the digest reads and links Literature trees only) ---
    $null = Invoke-Zt @(@{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{Other2020,title={Another book}}'; body = '另一本书。' } })
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = 'Other2020'; title = '风险分担说明租金'; body = '风险分担说明租金。' } })
    $null = Invoke-Zt @(@{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{Earlier2019,title={An earlier book}}'; body = '更早的一本书。' } })
    $oldAddr = 'Earlier2019/1'
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = 'Earlier2019'; title = '更早一本书里的旧卡片'; body = "旧卡片提到 §1.2 与 §2.1，但它属于另一条思路。`n`n来源：[[Other2020]] §1.2" } })
    $costAddr = 'Earlier2019/1|a'
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = $oldAddr; kind = 'side'; title = '交易成本决定企业边界'; body = '交易成本决定企业的边界。' } })
    Assert ((BodyOf $oldAddr).Contains('§1.2')) 'an older Card and another book''s Card name the same section numbers as the run'

    # --- one prepared run of the pilot source, three groups ---
    $source = Join-Path $repo 'docs\digest-pilot-short-source.zh.md'
    $prepared = Run-Digest @('prepare', '-Variant', 'econ', '-Source', $source, '-Out', $out, '-CitationKey', 'ScopeBook',
        '-BookTitle', 'Link gap book', '-BibTeX', '@book{ScopeBook,title={Link gap book},author={Scope, Ada},year={2026}}',
        '-RunOptions', '{"terms":false}', '-Apply')
    Assert (@($prepared.groups).Count -eq 3) "prepare binds the pilot source with three groups ($(@($prepared.groups).Count))"
    # Another book's Card that names this book's section, created after this book exists.
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = 'Other2020/1'; kind = 'side'; title = '风险与租金'; body = "风险分担说明租金。`n`n来源：[[ScopeBook]] §1.2" } })
    Assert ((BodyOf 'Other2020/1|a').Contains('§1.2')) 'another book''s Card cites this book''s §1.2'

    # --- group 1: one linked Card, one unlinked Card and one orphan, all in the book's own tree ---
    $claim = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'gap-manager')
    $started = Run-Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($claim.units[0].token)")
    $job = Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $prefix1 = "$($started.run_id)/digest-g001/1/apply"
    $anchorBody = "§1.1 - 企业的边界`n`n§1.2 - 市场失灵"
    $draft1 = @{ schema_version = 1; unit = $claim.units[0].id; token = $claim.units[0].token; input_hash = $started.input_hash; disposition = 'cards'; calls = @(
            @{ tool = 'edit_card'; arguments = @{ at = 'ScopeBook/1'; title = '企业因市场失灵而出现'; body = $anchorBody; base_fingerprint = $job.inputs.anchor.fingerprint } },
            @{ tool = 'create_successor'; arguments = @{ at = 'ScopeBook/1'; kind = 'side'; title = '交易成本决定企业的边界'; body = "依赖 [[$costAddr]]，后者给出相同的成本论证。" } },
            @{ tool = 'create_successor'; arguments = @{ at = 'ScopeBook/1'; kind = 'side'; title = '市场失灵让企业替代市场'; body = '市场失灵让企业替代市场，交易成本因此下降。' } },
            @{ tool = 'create_successor'; arguments = @{ at = 'ScopeBook/1'; kind = 'side'; title = '敲竹杠使专用性投资不足'; body = '敲竹杠使专用性投资不足。' } },

            @{ tool = 'edit_card'; arguments = @{ at = 'ScopeBook/1'; base_receipt = "$prefix1/item-000000"; body = "$anchorBody`n`n承接 [[@@1]]，后者用交易成本说明边界。`n`n承接 [[@@2]]，后者说明市场失灵。" } }
        ) }
    Save-Json $started.draft $draft1
    $group1 = Run-Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($claim.units[0].token)", '-Draft', $started.draft)
    $owned1 = @($group1.owned)
    $created = @($owned1 | Where-Object { $_ -like 'ScopeBook/1|*' })
    Assert ($group1.state -eq 'done' -and $created.Count -eq 3 -and $owned1.Count -eq 4) "group 1 owns its anchor and three new Cards ($($owned1 -join ', '))"
    $cardsClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'cards', '-Take', '1', '-Owner', 'gap-manager')).units[0]
    $null = Run-Digest @('cards-finish', '-Out', $out, '-Group', '1', '-Token', "$($cardsClaim.token)")
    $cardA = $created[0]; $cardB = $created[1]; $cardC = $created[2]
    Assert ((BodyOf $cardA).Contains($costAddr) -and -not (BodyOf $cardB).Contains('[[')) 'the group wrote one linked Card and two Cards with no Link'

    # --- prepare-link: the gap set, not the owned set, becomes the job; older bodies stay unwritable ---
    $before = Revision
    $prep = Run-Digest @('prepare-link', '-Staging', $out)
    Assert ((@($prep.pending) -join ',') -eq '1' -and $prep.groups[0].gaps -eq 3 -and $prep.groups[0].skipped -eq 1) "prepare-link schedules only group 1 and reports 3 gaps among 4 owned Cards ($(@($prep.pending) -join ','); gaps $($prep.groups[0].gaps), skipped $($prep.groups[0].skipped))"
    Assert ($prep.groups[0].position -eq 'anchor_position_missing' -and $prep.gap_total -eq 3 -and $prep.scheduled -eq 1) "the public scope names the anchor's missing position paragraph and the run totals ($($prep.gap_total) gaps, $($prep.scheduled) scheduled)"
    $allow1 = @(Get-Content -LiteralPath (Join-Path $out 'allow-g01.txt') -Encoding utf8 | Where-Object { $_ -ne '' })
    Assert (((($allow1 | Sort-Object) -join ',')) -eq (((@($cardB, $cardC, 'ScopeBook/1') | Sort-Object) -join ','))) "the write fence is exactly the gap set ($($allow1 -join ', '))"
    Assert (($allow1 -notcontains $oldAddr) -and ($allow1 -notcontains 'Other2020') -and ($allow1 -notcontains 'Other2020/1|a') -and ($allow1 -notcontains $cardA)) 'no older Card, no other book''s Card and no already-linked Card is writable'
    $scopeDoc = Get-Content -LiteralPath (Join-Path $out 'link-g01.md') -Raw -Encoding utf8
    Assert ($scopeDoc.Contains('缺口：no_semantic_link') -and -not $scopeDoc.Contains("### ``$cardA``")) "the group document lists only gap Cards ($($scopeDoc.Length) chars)"
    $gapRecord = Get-Content -LiteralPath (Join-Path $out 'link-gap-g01.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($gapRecord.group.gaps).Count -eq 3 -and $gapRecord.group.skipped -contains $cardA) 'the frozen gap record names every gap and every skipped Card'

    # --- link-start freezes the scope and writes the one job ---
    $linkClaim = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'link', '-Take', '1', '-Owner', 'gap-manager')
    Assert ($linkClaim.units[0].id -eq 'link/g001@1') "the completed group releases its Link unit ($($linkClaim.units[0].id))"
    $linkStart = Run-Digest @('link-start', '-Out', $out, '-Group', '1', '-Token', "$($linkClaim.units[0].token)")
    Assert ($linkStart.expected_jobs -eq 1 -and @($linkStart.gap).Count -eq 3 -and $null -eq $linkStart.no_op) 'link-start publishes one job for the three gaps'
    # acemoglu2025nobel trial 1: the Link jobs run with cwd <out>\link-jobs, which nothing created.
    Assert (Test-Path -LiteralPath (Join-Path $out 'link-jobs') -PathType Container) 'link-start creates the link-jobs folder its DeepSeek job runs in'
    $startManifest = Get-Content -LiteralPath $linkStart.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $linkJob = Get-Content -LiteralPath $startManifest.artifacts.job.path -Raw -Encoding utf8
    $gapLines = @((($linkJob -split '## 缺口卡片')[-1] -split "`n") | Where-Object { $_ -match 'base_fingerprint=' })
    Assert ($linkJob.Contains('SELF-CHECK') -and $linkJob.Contains($linkStart.input_hash) -and $gapLines.Count -eq 3) "the job file carries the binding, the SELF-CHECK line and exactly the gap Cards (gap lines $($gapLines.Count))"

    $draftPath = $linkStart.draft
    $base = $group1.receipt_keys
    $anchorParagraph = "在全书中的位置：本部分承接 [[$costAddr]] 的成本论证，引出下一部分的融资问题。"
    $bParagraph = "对照 [[$costAddr]]，两种失灵都让市场交易变得昂贵。"
    $anchorBefore = BodyOf 'ScopeBook/1'; $bBefore = BodyOf $cardB
    $draft = @{ schema_version = 1; unit = 'link/g001@1'; token = $linkClaim.units[0].token; input_hash = $linkStart.input_hash; calls = @(
            @{ tool = 'edit_card'; arguments = @{ at = 'ScopeBook/1'; base_receipt = $base[4]; owned_paragraph = @{ text = $anchorParagraph } } },
            @{ tool = 'edit_card'; arguments = @{ at = $cardB; base_receipt = $base[2]; owned_paragraph = @{ text = $bParagraph } } }
        ); dispositions = @(@{ at = $cardC; outcome = 'no_link'; reason = '候选卡片都与它没有真实关系；本卡只陈述敲竹杠的后果。' }) }
    $checkArgs = @('link-finish', '-Out', $out, '-Group', '1', '-Token', "$($linkClaim.units[0].token)", '-Draft', $draftPath, '-CheckOnly')
    Save-Json $draftPath $draft
    $checked = Run-Digest $checkArgs
    Assert ($checked.check_only -and $checked.complete -and @($checked.outcomes).Count -eq 4 -and $checked.self_check.count -eq 0 -and (Revision) -eq $before) 'a complete gap draft checks every owned Card without writing'

    # Refusals: a Card outside the gap set, an older body, and a paragraph that links only a source place.
    $badGap = $draft.Clone(); $badGap.calls = @($draft.calls) + @(@{ tool = 'edit_card'; arguments = @{ at = $cardA; base_receipt = $base[1]; owned_paragraph = @{ text = "依赖 [[$costAddr]]，后者给出相同的成本论证。" } } })
    Save-Json $draftPath $badGap
    $r = Digest $checkArgs
    Assert ($r.exit -ne 0 -and $r.text.Contains('not a semantic gap')) 'a Card outside the gap set cannot receive a Link job'
    $badOld = $draft.Clone(); $badOld.calls = @($draft.calls) + @(@{ tool = 'edit_card'; arguments = @{ at = $oldAddr; base_fingerprint = 'x'; owned_paragraph = @{ text = '依赖 [[ScopeBook/1]]，后者是这一部分的索引。' } } })
    Save-Json $draftPath $badOld
    $r = Digest $checkArgs
    Assert ($r.exit -ne 0 -and $r.text.Contains('not an owned Card')) 'an older body in another Literature tree is never writable'
    $badPlace = $draft.Clone(); $badPlace.calls = @($draft.calls[0], @{ tool = 'edit_card'; arguments = @{ at = $cardB; base_receipt = $base[2]; owned_paragraph = @{ text = '承接 [[ScopeBook/1]]，后者是这一部分的索引。' } } })
    Save-Json $draftPath $badPlace
    $r = Digest $checkArgs
    Assert ($r.exit -ne 0 -and $r.text.Contains('names no Card outside its own group')) 'a Link to the group''s own anchor is a source place, not a semantic Link'
    Save-Json $draftPath $draft

    # --- apply: exact write targets, old bodies preserved, one paragraph per anchor. The final
    # ledger commit is lost once, so the accepted plan and its receipts are the recovery evidence. ---
    $before = Revision
    $env:ZT_LEDGER_COMPLETE_FAULT = 'after_outputs'
    $lost = Digest @('link-finish', '-Out', $out, '-Group', '1', '-Token', "$($linkClaim.units[0].token)", '-Draft', $draftPath)
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    Assert ($lost.exit -ne 0 -and $lost.text.Contains('injected completion fault') -and (Revision) -eq ($before + 1)) 'a lost final reply leaves the written paragraphs and the accepted plan behind (AC08)'
    $changed = Get-Content -LiteralPath $draftPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $changed.calls[1].arguments.owned_paragraph.text = "对照 [[$costAddr]]，两种失灵都让市场交易变得昂贵，这一点很重要。"
    Save-Json $draftPath $changed
    $r = Digest @('link-finish', '-Out', $out, '-Group', '1', '-Token', "$($linkClaim.units[0].token)", '-Draft', $draftPath)
    Assert ($r.exit -ne 0 -and $r.text.Contains('accepted_plan_changed')) 'a changed Link body conflicts with the accepted plan instead of rewriting it'
    Save-Json $draftPath $draft
    $applied = Run-Digest @('link-finish', '-Out', $out, '-Group', '1', '-Token', "$($linkClaim.units[0].token)", '-Draft', $draftPath)
    Assert ($applied.state -eq 'done' -and $applied.complete -and (Revision) -eq ($before + 1)) 'the lost final reply recovers from its accepted plan without a second archive write'
    Assert ((BodyOf 'ScopeBook/1') -eq "$anchorBefore`n`n$anchorParagraph" -and (PositionCount (BodyOf 'ScopeBook/1')) -eq 1) 'the anchor gains exactly one 在全书中的位置 paragraph'
    Assert ((BodyOf $cardB) -eq "$bBefore`n`n$bParagraph") 'each written Card keeps its old body and gains only the owned sentence'
    Assert ((BodyOf $cardC) -eq '敲竹杠使专用性投资不足。' -and (BodyOf $oldAddr).Contains('§1.2') -and (BodyOf 'Other2020/1|a').Contains('来源：[[ScopeBook]] §1.2')) 'the orphan, the older Card and the other book''s Card are byte-unchanged'
    Assert ((BodyOf $cardA) -eq "依赖 [[$costAddr]]，后者给出相同的成本论证。") 'the already-linked Card was never written'
    Assert ((@($applied.changed) | Sort-Object) -join ',' -eq (@($cardB, 'ScopeBook/1') | Sort-Object) -join ',') "the completion enumerates exactly the changed Cards ($(@($applied.changed) -join ', '))"
    Assert ($applied.counts.gaps -eq 3 -and $applied.counts.skipped -eq 1 -and $applied.counts.linked -eq 2 -and $applied.counts.no_link -eq 1 -and $applied.counts.already_linked -eq 1) "the completion reports gap, skipped and outcome counts ($($applied.counts | ConvertTo-Json -Compress))"
    Assert ($applied.quality.rules -eq 'L1,L2' -and $applied.quality.faults -eq 0) 'the completion carries its L1/L2 quality result'
    $audit = Run-Digest @('audit', '-CitationKey', 'ScopeBook', '-Out', (Join-Path $scratch 'audit'), '-SkipModels')
    Assert ($audit.books[0].orphans -ge 1) "the unresolved orphan stays a reported required fault after the none outcome ($($audit.books[0].orphans) orphan(s))"

    # --- repeated completion reuses its receipts and adds no second anchor paragraph ---
    $again = Run-Digest @('link-finish', '-Out', $out, '-Group', '1', '-Token', "$($linkClaim.units[0].token)", '-Draft', $draftPath)
    Assert ($again.reused -and (Revision) -eq ($before + 1) -and (PositionCount (BodyOf 'ScopeBook/1')) -eq 1) 'a repeated completion reuses its receipts and adds no second paragraph'

    # --- group 2: every Card already linked, the anchor already states its position ---
    [IO.File]::WriteAllText((Join-Path $out 'linked-g01.txt'), "done`n", $utf8)
    $claim2 = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'gap-manager')
    $started2 = Run-Digest @('group-start', '-Out', $out, '-Group', '2', '-Token', "$($claim2.units[0].token)")
    $job2 = Get-Content -LiteralPath $started2.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $prefix2 = "$($started2.run_id)/digest-g002/1/apply"
    $anchorBody2 = "§2.1 - 融资约束`n`n在全书中的位置：本部分承接 [[$costAddr]] 的成本论证。"
    $draft2 = @{ schema_version = 1; unit = $claim2.units[0].id; token = $claim2.units[0].token; input_hash = $started2.input_hash; disposition = 'cards'; calls = @(
            @{ tool = 'edit_card'; arguments = @{ at = 'ScopeBook/2'; title = '融资约束改变企业的投资选择'; body = $anchorBody2; base_fingerprint = $job2.inputs.anchor.fingerprint } },
            @{ tool = 'create_successor'; arguments = @{ at = 'ScopeBook/2'; kind = 'side'; title = '贷款收紧限制企业投入'; body = "依赖 [[$costAddr]]，后者给出成本上升的论证。" } },
            @{ tool = 'edit_card'; arguments = @{ at = 'ScopeBook/2'; base_receipt = "$prefix2/item-000000"; body = "$anchorBody2`n`n承接 [[@@1]]，后者说明贷款收紧对投入的限制。" } }
        ) }
    Save-Json $started2.draft $draft2
    $group2 = Run-Digest @('group-finish', '-Out', $out, '-Group', '2', '-Token', "$($claim2.units[0].token)", '-Draft', $started2.draft)
    $cardsClaim2 = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'cards', '-Take', '1', '-Owner', 'gap-manager')).units[0]
    $null = Run-Digest @('cards-finish', '-Out', $out, '-Group', '2', '-Token', "$($cardsClaim2.token)")
    $owned2 = @($group2.owned)
    $cardD = @($owned2 | Where-Object { $_ -like 'ScopeBook/2|*' })[0]
    Assert ($group2.state -eq 'done' -and $cardD) "group 2 completes with every Card already linked ($($owned2 -join ', '))"
    $prep2 = Run-Digest @('prepare-link', '-Staging', $out)
    Assert ((@($prep2.pending) -join ',') -eq '' -and (@($prep2.no_gap) -join ',') -eq '2' -and $prep2.groups[1].gaps -eq 0) "a group with no semantic gap is not scheduled ($(@($prep2.pending) -join ','); no_gap $(@($prep2.no_gap) -join ','))"
    $allow2 = @(Get-Content -LiteralPath (Join-Path $out 'allow-g02.txt') -Encoding utf8 | Where-Object { $_ -ne '' })
    Assert ($allow2.Count -eq 0) 'an empty gap set has an empty write fence'
    $linkClaim2 = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'link', '-Take', '1', '-Owner', 'gap-manager')
    Assert ($linkClaim2.units[0].id -eq 'link/g002@1') "the gap-free group still releases its Link unit ($($linkClaim2.units[0].id))"
    $start2 = Run-Digest @('link-start', '-Out', $out, '-Group', '2', '-Token', "$($linkClaim2.units[0].token)")
    Assert ($start2.expected_jobs -eq 0 -and $start2.no_op.reason -eq 'no_semantic_gaps' -and @($start2.gap).Count -eq 0) 'an empty gap set starts no provider job'
    $before = Revision
    $done2 = Run-Digest @('link-finish', '-Out', $out, '-Group', '2', '-Token', "$($linkClaim2.units[0].token)", '-Draft', $start2.draft)
    Assert ($done2.state -eq 'done' -and -not $done2.applied -and @($done2.changed).Count -eq 0 -and (Revision) -eq $before) 'the skipped group completes its Link unit without writing a Card'
    Assert (@($done2.outcomes).Count -eq 2 -and @($done2.outcomes | Where-Object { $_.outcome -eq 'already_linked' }).Count -eq 2) 'every owned Card is recorded as already linked'

    # --- a failed group gets no Link job; conflicting receipts are a preparation fault ---
    $claim3 = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'gap-manager')
    $fault = @{ schema_version = 1; unit = $claim3.units[0].id; input_hash = $claim3.units[0].input_hash; code = 'check_failed'; cause = 'test: the group fails so its Link unit stays blocked' }
    $null = Run-Digest @('ledger', 'fail', '-Out', $out, '-Unit', $claim3.units[0].id, '-Token', "$($claim3.units[0].token)", '-Class', 'content', '-Fault', ($fault | ConvertTo-Json -Depth 6 -Compress))
    [IO.File]::WriteAllText((Join-Path $out 'linked-g02.txt'), "done`n", $utf8)
    $prep3 = Run-Digest @('prepare-link', '-Staging', $out)
    $blocked3 = @($prep3.blocked) | Where-Object { $_.n -eq 3 }
    Assert ($blocked3 -and $blocked3.state -eq 'failed' -and @($prep3.pending).Count -eq 0) "the failed group's Link unit is blocked, not scheduled ($($blocked3.state))"
    $noUnit = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'link', '-Take', '1', '-Owner', 'gap-manager')
    Assert (@($noUnit.units).Count -eq 0) 'no ready Link unit exists for a failed group'
    [IO.File]::WriteAllText((Join-Path $out 'owned-g02.txt'), "ScopeBook/1`n", $utf8)
    $conflict = Digest @('prepare-link', '-Staging', $out)
    Assert ($conflict.exit -ne 0 -and $conflict.text.Contains('ownership fault')) 'two groups naming one Card is an explicit preparation fault'
    Remove-Item -LiteralPath (Join-Path $out 'owned-g02.txt') -Force
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failed) { Write-Output "$failed failed"; exit 1 }
Write-Output 'all passed'










