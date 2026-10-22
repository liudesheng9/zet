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
    # --- an empty shortlist with the optional concept inputs absent is reported, never substituted ---
    $bare = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','中央银行的最后贷款人职能','-Question','中央银行在挤兑时如何充当最后贷款人？')
    Assert ($bare.outcome -eq 'new' -and @($bare.shortlist).Count -eq 0) 'an empty shortlist clears new creation'
    $absent = @($bare.missing | ForEach-Object { $_.input })
    Assert (($absent -contains 'register') -and ($absent -contains 'term_table')) 'a missing register and term table are reported, never substituted'
    Assert ($bare.complete) 'the required inputs (the title catalogue and the questions) are complete'
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
    # The gate file the run's managed writers already name (`prepare` writes it into every group's
    # write command, `ledger` group-apply guards with it), not a second file beside it.
    $lock = Join-Path $out 'topic-gate.lock'
    Assert ($prepared.lock -eq $lock) 'the gate names the run Topic gate file its managed writers already use'
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
    $duplicated = Join-Path $scratch 'answer-duplicate.json'
    [IO.File]::WriteAllText($duplicated, '{"proposed_title":"均衡多重性","question":"为什么同一市场会同时存在多个均衡？","verdict":"different","verdict":"different","reason":"重复键。","evidence":["' + $multi + '"],"inputs_fingerprint":"' + $prepared.inputs_fingerprint + '","candidates":[]}', $utf8)
    $dupe = Try-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',$duplicated)
    Assert ($dupe.code -eq 1 -and $dupe.text.Contains('duplicate JSON key')) 'a decision that repeats a JSON key is refused'
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
    # A cleared creation waits for the run Topic gate the managed writers share, and writes nothing
    # while another writer holds it.
    $gateHold = [IO.File]::Open($lock,[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::ReadWrite)
    $queued = $null
    $resultFile = Join-Path $out 'gate/create.json'
    $beforeResult = if (Test-Path -LiteralPath $resultFile) { [IO.File]::ReadAllText($resultFile) } else { $null }
    try {
        $gateHold.Lock(0,1)
        $queued = Start-Process -FilePath $Digest -ArgumentList @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix',$prefix) -PassThru -RedirectStandardOutput (Join-Path $scratch 'create-waiting.out') -RedirectStandardError (Join-Path $scratch 'create-waiting.err') -WindowStyle Hidden
        Start-Sleep -Seconds 3
        Assert (-not $queued.HasExited) 'a cleared creation waits for the run Topic gate lock'
        $heldResult = if (Test-Path -LiteralPath $resultFile) { [IO.File]::ReadAllText($resultFile) } else { $null }
        Assert ($heldResult -eq $beforeResult) 'a creation held at the gate has written no result'
    } finally { $gateHold.Dispose() }
    $queued.WaitForExit()
    Assert ($queued.ExitCode -eq 0) 'the creation completes once the run gate is released'
    $created = @(Get-Content -LiteralPath (Join-Path $scratch 'create-waiting.out') | Where-Object { $_ -match '\S' })[-1] | ConvertFrom-Json -AsHashtable
    Assert ($created.created -and $created.address -and $created.verdict -eq 'different') 'a cleared decision creates the Topic through its receipt'
    Assert ($created.note -match 'coordinates managed digest processes only') 'the creation states the bounded guarantee in its own result'
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

    # --- distinct senses of learning are shown, then kept apart by a different decision ---
    $senses = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','学习曲线','-Question','累计产量翻倍时单位成本如何下降？')
    Assert ($senses.outcome -eq 'judge') 'a third sense of learning is a plausible candidate'
    $sense = @($senses.shortlist | Where-Object { $_.address -eq $learning })
    Assert ($sense.Count -eq 1) 'the behavioural Topic 学习 is shortlisted beside the algorithmic one'
    $labelled = @($sense[0].definitions | Where-Object { $_.term -eq '学习' })
    Assert ($labelled.Count -eq 1 -and $labelled[0].meaning -eq '经验改变行为') 'the term table labels the other sense with its meaning'
    $senseCandidates = @($senses.shortlist | ForEach-Object { @{ address = $_.address; verdict = 'different' } })
    $senseEvidence = @($senses.shortlist | ForEach-Object { $_.address } | Where-Object { $_ -in @($learning, $ml) })
    $decidedSense = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-senses.json' @{
        proposed_title='学习曲线'; question='累计产量翻倍时单位成本如何下降？'; verdict='different'
        reason='行为意义上的学习、算法的机器学习与学习曲线是三个不同的问题。'
        evidence=$senseEvidence; inputs_fingerprint=$senses.inputs_fingerprint; candidates=$senseCandidates
    }))
    Assert ($decidedSense.verdict -eq 'different') 'the judge keeps the distinct senses apart'
    # An unrelated Card edit changes no concept input, so the frozen evidence stays valid.
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$debt; kind='direct'; title='债务契约减少代理成本'; body='债务契约让经理承担破产成本。来源：[[Book2020]] §2。' } })
    $debtEntry = (@(Invoke-Zt @(@{ tool='get_card'; arguments=@{ at=$debt } }))[0]).direct.address
    $null = Invoke-Zt @(@{ tool='edit_card'; arguments=@{ at=$debtEntry; text="# 债务契约减少代理成本`n`n债务契约让经理承担破产成本，从而减少在职消费。来源：[[Book2020]] §2。`n" } })
    $curve = Run-Digest @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix','gate-run/topic/8/create')
    Assert ($curve.created -and $curve.title -eq '学习曲线') 'an unrelated Card edit does not stale the frozen concept evidence'
    Assert (@((Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles | Where-Object { $_.title -eq '学习曲线' }).Count -eq 1) 'the distinct sense becomes one new Topic'

    # --- the TF-IDF order over titles and questions reaches a Topic that names no shared term ---
    $null = Invoke-Zt @(@{ tool='create_topic'; arguments=@{ title='货币政策传导'; body='利率变化如何影响投资与产出？' } })
    $transmission = (@(Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles | Where-Object { $_.title -eq '货币政策传导' } | Select-Object -First 1).address
    $ranked = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','利率传导','-Question','利率变化如何影响投资与产出？')
    $tfidf = @($ranked.shortlist | Where-Object { $_.address -eq $transmission })
    Assert ($tfidf.Count -eq 1 -and $tfidf[0].ranked -eq 'tfidf') 'the TF-IDF order over titles and questions finds a Topic that names no term row'

    # --- the identified helper accepts the concept gate and still refuses legacy combinations ---
    $fresh = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','中央银行的最后贷款人职能','-Question','中央银行在挤兑时如何充当最后贷款人？')
    Assert ($fresh.outcome -eq 'new' -and @($fresh.shortlist).Count -eq 0) 'an empty complete shortlist clears new creation deterministically'
    $null = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-new.json' @{
        proposed_title='中央银行的最后贷款人职能'; question='中央银行在挤兑时如何充当最后贷款人？'; verdict='different'
        reason='短名单为空，没有同一主题。'; evidence=@(); inputs_fingerprint=$fresh.inputs_fingerprint; candidates=@()
    }))
    $callsFile = Join-Path $scratch 'gate-calls.json'
    [IO.File]::WriteAllText($callsFile, '[{"tool":"create_topic","arguments":{"title":"中央银行的最后贷款人职能","body":"中央银行在挤兑时如何充当最后贷款人？"}}]', $utf8)
    $viaLines = @(& $Digest call -Open $callsFile -OperationPrefix 'gate-run/topic/6/create' -ConceptGate (Join-Path $out 'gate/decision.json') 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($viaLines -join ' ') -match '0 ok ') 'the identified helper accepts the concept gate with OperationPrefix'
    $viaAgain = @(& $Digest call -Open $callsFile -OperationPrefix 'gate-run/topic/6/create' -ConceptGate (Join-Path $out 'gate/decision.json') 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($viaAgain -join ' ') -eq ($viaLines -join ' ')) 'a replayed helper operation returns its committed Topic before any new decision'
    Assert (@((Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles | Where-Object { $_.title -eq '中央银行的最后贷款人职能' }).Count -eq 1) 'a lost helper response replays to one Topic, never two'
    $lender = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','最后贷款人','-Question','最后贷款人如何阻止挤兑？')
    $null = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-lender.json' @{
        proposed_title='最后贷款人'; question='最后贷款人如何阻止挤兑？'; verdict='different'
        reason='短名单中的 Topic 追问中央银行的具体职能，不是一般意义上的最后贷款人。'
        evidence=@($lender.shortlist | ForEach-Object { $_.address }); inputs_fingerprint=$lender.inputs_fingerprint
        candidates=@($lender.shortlist | ForEach-Object { @{ address=$_.address; verdict='different' } })
    }))
    $otherCalls = Join-Path $scratch 'gate-calls-other.json'
    [IO.File]::WriteAllText($otherCalls, '[{"tool":"create_topic","arguments":{"title":"最后贷款人的道德风险","body":"最后贷款人为什么会引起道德风险？"}}]', $utf8)
    $uncleared = Try-Digest @('call','-Open',$otherCalls,'-OperationPrefix','gate-run/topic/11/create','-ConceptGate',(Join-Path $out 'gate/decision.json'))
    Assert ($uncleared.code -eq 1 -and $uncleared.text.Contains('did not clear')) 'a managed creation refuses a Topic title the concept decision did not clear'
    $legacy = Try-Digest @('call','-Open',$callsFile,'-OperationPrefix','gate-run/topic/7/create','-TopicGate',$lock)
    Assert ($legacy.code -eq 1 -and $legacy.text.Contains('cannot combine with legacy resume, logs, moves or Topic-gate options')) 'the legacy Topic-gate options stay refused beside OperationPrefix'
    $legacyLog = Try-Digest @('call','-Open',$callsFile,'-OperationPrefix','gate-run/topic/7/create','-Log',(Join-Path $scratch 'owned.txt'))
    Assert ($legacyLog.code -eq 1 -and $legacyLog.text.Contains('cannot combine with legacy resume, logs, moves or Topic-gate options')) 'the legacy log option stays refused beside OperationPrefix'
    $orphan = Try-Digest @('call','-Open',$callsFile,'-ConceptGate',(Join-Path $out 'gate/decision.json'))
    Assert ($orphan.code -eq 1 -and $orphan.text.Contains('ConceptGate requires OperationPrefix')) 'the concept gate alone is refused: it owns receipts'

    # --- two concurrent managed writers create one Topic, not two ---
    $parity = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','利率平价','-Question','两国的利率差如何决定汇率变动？')
    $null = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-parity.json' @{
        proposed_title='利率平价'; question='两国的利率差如何决定汇率变动？'; verdict='different'
        reason='短名单里的 Topic 追问别的问题，利率平价要新建。'
        evidence=@($parity.shortlist | ForEach-Object { $_.address }); inputs_fingerprint=$parity.inputs_fingerprint
        candidates=@($parity.shortlist | ForEach-Object { @{ address = $_.address; verdict = 'different' } })
    }))
    $decisionFile = Join-Path $out 'gate/decision.json'
    $trials = @()
    foreach ($n in 9,10) {
        $prefixN = "gate-run/topic/$n/create"
        $process = Start-Process -FilePath $Digest -ArgumentList @('gate','-Step','create','-Staging',$out,'-Decision',$decisionFile,'-OperationPrefix',$prefixN) -PassThru -RedirectStandardOutput (Join-Path $scratch "writer-$n.out") -RedirectStandardError (Join-Path $scratch "writer-$n.err") -WindowStyle Hidden
        $trials += [pscustomobject]@{ prefix=$prefixN; process=$process; out=(Join-Path $scratch "writer-$n.out") }
    }
    $trials | ForEach-Object { $_.process.WaitForExit() }
    $answers = @(foreach ($trial in $trials) {
        $lines = @(Get-Content -LiteralPath $trial.out | Where-Object { $_ -match '\S' })
        [pscustomobject]@{ prefix=$trial.prefix; answer=$(if ($lines.Count) { $lines[-1] | ConvertFrom-Json -AsHashtable } else { $null }) }
    })
    Assert (@($answers | Where-Object { $_.answer.created }).Count -eq 1) 'two concurrent managed writers create one Topic'
    Assert (@((Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles | Where-Object { $_.title -eq '利率平价' }).Count -eq 1) 'the archive holds one 利率平价 Topic after the race'
    $winner = @($answers | Where-Object { $_.answer.created })[0]
    $loser = @($answers | Where-Object { -not $_.answer.created })[0]
    Assert ($loser.answer.detail -match 'stale_decision|topic exists') 'the writer that lost the race creates nothing and is told to regenerate or follow the existing Topic'
    # Its regeneration finds the committed Topic: the fast check names it, and a cleared decision is
    # still refused under the lock by the exact-title check.
    $reseen = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','利率平价','-Question','两国的利率差如何决定汇率变动？')
    Assert (@($reseen.fast.exact) -contains $winner.answer.address) 'the regenerated fast check names the committed Topic'
    $null = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-parity-again.json' @{
        proposed_title='利率平价'; question='两国的利率差如何决定汇率变动？'; verdict='different'
        reason='重跑后的决定仍被锁内的同一标题检查挡住。'
        evidence=@($reseen.shortlist | ForEach-Object { $_.address }); inputs_fingerprint=$reseen.inputs_fingerprint
        candidates=@($reseen.shortlist | ForEach-Object { @{ address = $_.address; verdict = 'different' } })
    }))
    $afterRace = Run-Digest @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix','gate-run/topic/12/create')
    Assert ($afterRace.created -eq $false -and $afterRace.detail -match 'topic exists') 'a regenerated decision cannot create the raced Topic a second time'

    # --- a replay whose payload changed is refused under its own operation key ---
    $moved = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','利率平价','-Question','资本如何流动直到两国的预期收益相等？')
    $null = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-parity-changed.json' @{
        proposed_title='利率平价'; question='资本如何流动直到两国的预期收益相等？'; verdict='different'
        reason='改写后的追问仍要新建，但同一操作前缀不能再写一次。'
        evidence=@($moved.shortlist | ForEach-Object { $_.address }); inputs_fingerprint=$moved.inputs_fingerprint
        candidates=@($moved.shortlist | ForEach-Object { @{ address = $_.address; verdict = 'different' } })
    }))
    $conflict = Try-Digest @('gate','-Step','create','-Staging',$out,'-Decision',$decisionFile,'-OperationPrefix',$winner.prefix)
    Assert ($conflict.code -eq 1 -and $conflict.text.Contains('operation key conflict')) 'a replay whose payload changed is refused by its operation key'
    Assert (@((Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles | Where-Object { $_.title -eq '利率平价' }).Count -eq 1) 'a changed-payload replay creates no second Topic'

    # --- the shortlist stays bounded at ten plausible Topics ---
    $many = @(1..12 | ForEach-Object { @{ tool='create_topic'; arguments=@{ title="均衡应用$_"; body="第 $_ 个均衡应用追问：同一市场为什么会出现第 $_ 个均衡？" } } })
    $null = Invoke-Zt $many
    $bounded = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','均衡多重性','-Question','为什么同一市场会同时存在多个均衡？')
    Assert ($bounded.outcome -eq 'judge' -and @($bounded.shortlist).Count -eq 10) 'the shortlist is bounded at ten plausible Topics'

    # --- a term table that exists but cannot be read is not absent evidence ---
    $out2 = Join-Path $scratch 'prepared2'
    New-Item -ItemType Directory -Force $out2 | Out-Null
    New-Item -ItemType Directory -Force (Join-Path $out2 'terms.md') | Out-Null
    $unread = Run-Digest @('gate','-Step','prepare','-Staging',$out2,'-Title','利率平价','-Question','两国的利率差如何决定汇率变动？')
    Assert ($unread.complete -eq $false -and $unread.outcome -eq 'incomplete') 'a term table that cannot be read leaves the concept inputs incomplete'
    $unreadTerm = @($unread.missing | Where-Object { $_.input -eq 'term_table' })
    Assert ($unreadTerm.Count -eq 1 -and $unreadTerm[0].status -eq 'failed') 'an unreadable term table is reported as failed, not substituted'

    # --- an ambiguous register and the optional gaps reach both reports ---
    $null = Invoke-Zt @(@{ tool='create_topic'; arguments=@{ title='索引二'; body='第二个概念索引。' } })
    $secondIndex = (@(Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles | Where-Object { $_.title -eq '索引二' } | Select-Object -First 1).address
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$secondIndex; kind='direct'; title='索引二'; body='第二个概念索引。' } })
    $secondRegister = (@(Invoke-Zt @(@{ tool='get_card'; arguments=@{ at=$secondIndex } }))[0]).direct.address
    $null = Invoke-Zt @(@{ tool='edit_card'; arguments=@{ at=$secondRegister; type='hub'; level='register'; text="# 索引二`n`n## 词条`n`n- 学习：[[$learning]]`n" } })
    $ambiguous = Run-Digest @('gate','-Step','prepare','-Staging',$out,'-Title','均衡多重性','-Question','为什么同一市场会同时存在多个均衡？')
    $badRegister = @($ambiguous.missing | Where-Object { $_.input -eq 'register' })
    Assert ($badRegister.Count -eq 1 -and $badRegister[0].status -eq 'ambiguous') 'two registers are reported as ambiguous input, never chosen between'
    Assert ($ambiguous.complete -eq $false -and $ambiguous.outcome -eq 'incomplete') 'an ambiguous register leaves the concept inputs incomplete'
    $null = Run-Digest @('gate','-Step','decide','-Staging',$out,'-Answer',(Write-Answer 'answer-ambiguous.json' @{
        proposed_title='均衡多重性'; question='为什么同一市场会同时存在多个均衡？'; verdict='different'
        reason='短名单中的 Topic 追问别的问题，概念输入里没有可采纳的登记簿。'
        evidence=@($ambiguous.shortlist | ForEach-Object { $_.address }); inputs_fingerprint=$ambiguous.inputs_fingerprint
        candidates=@($ambiguous.shortlist | ForEach-Object { @{ address = $_.address; verdict = 'different' } })
    }))
    $incomplete = Try-Digest @('gate','-Step','create','-Staging',$out,'-Decision',(Join-Path $out 'gate/decision.json'),'-OperationPrefix','gate-run/topic/13/create')
    Assert ($incomplete.code -eq 3 -and $incomplete.text.Contains('incomplete_evidence')) 'an incomplete concept input cannot clear a new Topic'
    $report = Run-Digest @('gate','-Step','report','-Staging',$out)
    Assert ($report.complete -and $report.outcome -eq 'incomplete' -and $report.shortlist -eq 10) 'the gate report carries the prepared outcome and the shortlist size'
    Assert ($report.evidence_complete -eq $false) 'the report names the incomplete concept evidence'
    Assert (@($report.missing | Where-Object { $_.status -eq 'ambiguous' }).Count -eq 1) 'the report carries the ambiguous concept input'
    $markdown = [IO.File]::ReadAllText((Join-Path $out 'gate/gate-report.md'))
    Assert ($markdown.Contains('register: ambiguous')) 'the Markdown report carries the optional-missing concept input'
    Assert ($markdown.Contains('raw Owner or GUI creation is outside it')) 'the report states that the gate binds managed writers only'
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_TRACE_TOOLS -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'