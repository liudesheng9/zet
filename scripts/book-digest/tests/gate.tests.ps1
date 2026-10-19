#Requires -Version 7.5
# S07 (#100): the concept-meaning Topic gate, at its public seams.
# A scratch archive with two spellings of equilibrium multiplicity, two senses of learning, a book
# term table and an accepted register term proves the shortlist, the judge rules, the locked
# revalidation and the receipt-owned creation. Raw Owner creation stays available and is reported.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "gate-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST=$Digest; $env:ZT_CONFIG_DIR=Join-Path $scratch 'config'; $env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$out = Join-Path $scratch 'prepared'
New-Item -ItemType Directory -Force $out | Out-Null
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "digest failed: $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Try-Digest([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    return [pscustomobject]@{ code=$LASTEXITCODE; text=($lines -join ' '); json=$(if ($lines.Count) { try { $lines[-1] | ConvertFrom-Json -AsHashtable } catch { $null } } else { $null }) }
}
function Write-Answer($name, $answer) {
    $path = Join-Path $scratch $name
    [IO.File]::WriteAllText($path, ($answer | ConvertTo-Json -Depth 10), $utf8)
    return $path
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $null = Invoke-Zt @(
        @{ tool='create_topic'; arguments=@{ title='多重均衡'; body='什么导致同一市场出现多个均衡？两个均衡能否同时存在？' } },
        @{ tool='create_topic'; arguments=@{ title='均衡的多重性'; body='一个市场为什么会有多个解？协调失败如何发生？' } },
        @{ tool='create_topic'; arguments=@{ title='学习'; body='经验如何改变人的行为？' } },
        @{ tool='create_topic'; arguments=@{ title='机器学习'; body='算法如何从数据中改进预测？' } },
        @{ tool='create_topic'; arguments=@{ title='债务契约'; body='债务契约如何约束经理？' } }
    )
    $titles = @(Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles
    $addressOf = { param($title) ($titles | Where-Object { $_.title -eq $title } | Select-Object -First 1).address }
    $multi = & $addressOf '多重均衡'
    $plural = & $addressOf '均衡的多重性'
    $learning = & $addressOf '学习'
    $ml = & $addressOf '机器学习'
    $debt = & $addressOf '债务契约'
    if (-not ($multi -and $plural -and $learning -and $ml)) { throw 'the four fixture Topics were not created' }
    # A Literature Card, so the fixture Card's source line is a real Link.
    $null = Invoke-Zt @(@{ tool='create_literature'; arguments=@{ bibtex='@book{Book2020, title={Multiple Equilibria}, author={Cooper}, year={2020}}'; text='2020 年出版的书。'; workspace='agent' } })
    # A Regular Card under the first Topic: the `same` decision's Regular entry.
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$multi; kind='direct'; title='协调失败给出两个均衡'; body='策略互补时，协调失败使同一个市场停在两个均衡之一。来源：[[Book2020]] §1。' } })
    $entry = (@(Invoke-Zt @(@{ tool='get_card'; arguments=@{ at=$multi } }))[0]).direct.address
    # The concept register: a level-register hub whose 词条 names the term.
    $null = Invoke-Zt @(@{ tool='create_topic'; arguments=@{ title='索引'; body='本工作区的概念索引。' } })
    $index = (@(Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles | Where-Object { $_.title -eq '索引' } | Select-Object -First 1).address
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$index; kind='direct'; title='索引'; body='本工作区的概念索引。' } })
    $registerCard = (@(Invoke-Zt @(@{ tool='get_card'; arguments=@{ at=$index } }))[0]).direct.address
    $null = Invoke-Zt @(@{ tool='edit_card'; arguments=@{ at=$registerCard; type='hub'; level='register'; text="# 索引`n`n## 词条`n`n- 均衡：[[$multi]]`n" } })
    [IO.File]::WriteAllText((Join-Path $out 'terms.md'), "| 术语 | 不用 | 意思 | 首次定义 |`n|---|---|---|---|`n| 均衡 | 平衡 | 各方的策略互相最佳反应 | 1.2 |`n| 学习 | 学习效应 | 经验改变行为 | 3.1 |`n", $utf8)

    # --- prepare: the shortlist compares questions, terms and register entries ---
    $prepared = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','均衡多重性','-Question','为什么同一市场会同时存在多个均衡？')
    Assert ($prepared.outcome -eq 'judge' -and $prepared.complete) 'a plausible shortlist asks for a concept judgment'
    $addresses = @($prepared.shortlist | ForEach-Object { $_.address })
    Assert ($addresses -contains $multi -and $addresses -contains $plural) 'both spellings of equilibrium multiplicity are shortlisted'
    Assert (-not ($addresses -contains $debt)) 'a Topic that shares no concept with the proposal is not shortlisted'
    Assert ($addresses.Count -le 10) 'the shortlist never exceeds ten Topics'
    Assert (($prepared.fast.title_key -eq '均衡多重性') -and (@($prepared.fast.exact).Count -eq 0) -and (@($prepared.fast.near).Count -eq 0)) 'the title check alone misses the second spelling: the concept stage is what finds it'
    $exact = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','均衡的多重性','-Question','一个市场为什么会有多个解？')
    Assert (@($exact.fast.exact) -contains $plural) 'an exactly equal normalized title is the fast exact refusal'
    $register = @($prepared.shortlist | Where-Object { @($_.matched_terms | Where-Object { $_.source -eq 'register' }).Count -gt 0 })
    Assert (@($register).Count -ge 1) 'an accepted register term names a candidate'
    $representatives = @($prepared.shortlist[0].representative)
    Assert ($representatives.Count -ge 2 -and ($representatives | ForEach-Object { $_.excerpt } | Where-Object { $_ -match '来源：' })) 'the judge gets representative Cards with their bodies and source lines'
    $definitions = @($prepared.shortlist | ForEach-Object { $_.definitions } | Where-Object { $_.term -eq '均衡' })
    Assert (@($definitions).Count -ge 1 -and $definitions[0].meaning -eq '各方的策略互相最佳反应') 'a differing meaning is carried as a labelled definition'
    Assert (@($prepared.missing | Where-Object { $_.input -eq 'term_table' }).Count -eq 0) 'a present term table is not reported missing'

    # --- the lock is never held while preparing or judging ---
    $lock = Join-Path $out 'gate/topic-gate.lock'
    New-Item -ItemType Directory -Force (Split-Path -Parent $lock) | Out-Null
    $held = [IO.File]::Open($lock,[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::ReadWrite)
    try {
        $held.Lock(0,1)
        $again = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','均衡多重性','-Question','为什么同一市场会同时存在多个均衡？')
        Assert ($again.outcome -eq 'judge') 'preparing a shortlist never waits for the Topic gate lock'
        $decidedWhileHeld = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-held.json' @{
            proposed_title='均衡多重性'; question='为什么同一市场会同时存在多个均衡？'
            verdict='different'; reason='两个 Topic 追问的问题不同：一个问来源，一个问共存。'
            evidence=@($multi)
            inputs_fingerprint=$prepared.inputs_fingerprint
            candidates=@($prepared.shortlist | ForEach-Object { @{address=$_.address; verdict='different'} })
        }))
        Assert ($decidedWhileHeld.verdict -eq 'different') 'judging never waits for the Topic gate lock either'
    } finally { $held.Dispose() }

    # --- decide: the answer rules ---
    $evidenceId = $prepared.evidence_id
    $answer = {
        param($verdict, $candidates, $extra = @{})
        $value = @{
            proposed_title='均衡多重性'
            question='为什么同一市场会同时存在多个均衡？'
            verdict=$verdict
            reason='两个 Topic 追问的问题不同：一个问来源，一个问共存。'
            evidence=@($multi)
            inputs_fingerprint=$prepared.inputs_fingerprint
            candidates=$candidates
        }
        foreach ($key in $extra.Keys) { $value[$key] = $extra[$key] }
        $value
    }
    $partial = Try-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-partial.json' (& $answer 'different' @(@{address=$multi; verdict='different'}))))
    Assert ($partial.code -eq 1 -and $partial.text.Contains('must account for every shortlisted Topic')) 'a partial `different` is refused and names what is missing'
    $contradiction = Try-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-contradiction.json' (& $answer 'different' @($prepared.shortlist | ForEach-Object { @{ address = $_.address; verdict = $(if ($_.address -eq $multi) { 'same' } else { 'different' }) } }))))
    Assert ($contradiction.code -eq 1 -and $contradiction.text.Contains('cannot also call')) 'a contradictory decision is refused'
    $unknown = Try-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-unknown.json' (& $answer 'maybe' @(@{address=$multi; verdict='different'}, @{address=$plural; verdict='different'}))))
    Assert ($unknown.code -eq 1 -and $unknown.text.Contains('unknown verdict')) 'an unknown verdict is refused'
    $candidates = @($prepared.shortlist | ForEach-Object { $_.address })
    $all = @($candidates | ForEach-Object { @{ address = $_; verdict = 'different' } })
    $withSame = @($candidates | ForEach-Object { @{ address = $_; verdict = $(if ($_ -eq $multi) { 'same' } else { 'different' }) } })
    $withUncertain = @($candidates | ForEach-Object { @{ address = $_; verdict = $(if ($_ -eq $multi) { 'uncertain' } else { 'different' }) } })
    $different = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-different.json' (& $answer 'different' $all)))
    Assert ($different.verdict -eq 'different' -and $different.id.Length -eq 64 -and $different.evidence_id -eq $evidenceId) 'a complete `different` clears new creation and binds the frozen evidence'

    # --- create: receipt-owned, locked, revalidated ---
    $prefix = 'gate-run/topic/1/create'
    $same = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-same.json' (& $answer 'same' $withSame @{selected_topic=$multi; selected_entry=$entry})))
    $refused = Run-Digest @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix','gate-run/topic/2/create')
    Assert ($refused.created -eq $false -and $refused.reason -eq 'same' -and $refused.existing.topic -eq $multi -and $refused.existing.entry -eq $entry) 'a `same` decision refuses creation and returns the Topic and its Regular entry'
    $before = @(Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles.Count
    $uncertain = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-uncertain.json' (& $answer 'uncertain' $withUncertain @{missing=@('本书对「学习」的定义')})))
    $waits = Run-Digest @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix','gate-run/topic/3/create')
    Assert ($waits.created -eq $false -and $waits.reason -eq 'uncertain' -and @($waits.missing).Count -eq 1) 'an `uncertain` decision names its missing context and creates nothing'
    Assert (@(Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles.Count -eq $before) 'a refused or uncertain gate writes no Topic'

    $null = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-different.json' (& $answer 'different' $all)))
    $created = Run-Digest @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix',$prefix)
    Assert ($created.created -and $created.address -and $created.verdict -eq 'different') 'a cleared decision creates the Topic through its receipt'
    $createdCard = @(Invoke-Zt @(@{ tool='get_card'; arguments=@{ at=$created.address } }))[0]
    Assert ($createdCard.title -eq '均衡多重性' -and $createdCard.workspace -eq 'agent') 'the created Topic is an agent Topic with the proposed title'
    $replayed = Run-Digest @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix',$prefix)
    Assert ($replayed.created -eq $false -and $replayed.reason -eq 'replayed' -and $replayed.address -eq $created.address) 'replaying a committed operation returns its Topic before any new decision'
    $countAfterReplay = @(Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles.Count
    Assert ($countAfterReplay -eq $before + 1) 'a lost response replays to one Topic, never two'

    # A competing raw Owner creation with the same title is reported, and raw creation stays possible.
    $null = Invoke-Zt @(@{ tool='create_topic'; arguments=@{ title='均衡 多重性'; body='另一位写作者用原始工具建的同一主题。' } })
    Assert ((Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles | Where-Object { $_.title -eq '均衡 多重性' }) 'a raw Owner Topic creation still works outside the managed gate'
    # The fast check survives under the lock: a Topic whose normalised title equals the proposal is the
    # same Topic, even when a judge called the shortlist different.
    $contested = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','均衡 多重性','-Question','为什么同一市场会同时存在多个均衡？')
    $contestedCandidates = @($contested.shortlist | ForEach-Object { @{ address = $_.address; verdict = 'different' } })
    $null = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-contested.json' (& $answer 'different' $contestedCandidates @{proposed_title='均衡 多重性'; inputs_fingerprint=$contested.inputs_fingerprint})))
    $blocked = Run-Digest @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix','gate-run/topic/4/create')
    Assert (($blocked.created -eq $false) -and ($blocked.detail -match 'topic exists')) 'the exact-title refusal survives the concept judge'
    Assert (@($created.competing).Count -eq 0 -and $created.ContainsKey('competing')) 'a cleared creation reports the competing-raw-creation field'

    # --- a stale decision is reported, never re-judged into a creation ---
    $null = Invoke-Zt @(@{ tool='edit_card'; arguments=@{ at=$plural; text="# 均衡的多重性`n`n一个市场为什么会有多个解？协调失败如何发生？多重均衡的稳定性是什么？`n" } })
    $stale = Try-Digest @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix','gate-run/topic/5/create')
    Assert ($stale.code -eq 3 -and $stale.text.Contains('stale_decision')) 'a changed concept input needs a fresh decision, not a forced creation'

    # --- the identified helper accepts the concept gate and still refuses legacy combinations ---
    $fresh = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','中央银行的最后贷款人职能','-Question','中央银行在挤兑时如何充当最后贷款人？')
    Assert ($fresh.outcome -eq 'new' -and @($fresh.shortlist).Count -eq 0) 'an empty complete shortlist clears new creation deterministically'
    $null = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-new.json' @{
        proposed_title='中央银行的最后贷款人职能'; question='中央银行在挤兑时如何充当最后贷款人？'; verdict='different'
        reason='短名单为空，没有同一主题。'; evidence=@(); inputs_fingerprint=$fresh.inputs_fingerprint; candidates=@()
    }))
    $callsFile = Join-Path $scratch 'gate-calls.json'
    [IO.File]::WriteAllText($callsFile, '[{"tool":"create_topic","arguments":{"title":"中央银行的最后贷款人职能","body":"中央银行在挤兑时如何充当最后贷款人？"}}]', $utf8)
    $viaLines = @(& $Digest call $callsFile -OperationPrefix 'gate-run/topic/6/create' -ConceptGate (Join-Path $out 'gate/decision.json') 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($viaLines -join ' ') -match '0 ok ') 'the identified helper accepts the concept gate with OperationPrefix'
    $legacy = Try-Digest @('call',$callsFile,'-OperationPrefix','gate-run/topic/7/create','-TopicGate',$lock)
    Assert ($legacy.code -eq 1 -and $legacy.text.Contains('cannot combine with legacy resume, logs, moves or Topic-gate options')) 'the legacy Topic-gate options stay refused beside OperationPrefix'
    $orphan = Try-Digest @('call',$callsFile,'-ConceptGate',(Join-Path $out 'gate/decision.json'))
    Assert ($orphan.code -eq 1 -and $orphan.text.Contains('ConceptGate requires OperationPrefix')) 'the concept gate alone is refused: it owns receipts'

    # --- the report states the bounded guarantee ---
    $report = Run-Digest @('gate','-Step','report','-Staging',$out)
    Assert ($report.complete -and $report.outcome -eq 'prepare' -or $report.outcome) 'the gate report carries the prepared outcome'
    Assert (([IO.File]::ReadAllText((Join-Path $out 'gate/gate-report.md'))).Contains('raw Owner or GUI creation is outside it')) 'the report states that the gate binds managed writers only'
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_TRACE_TOOLS -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'