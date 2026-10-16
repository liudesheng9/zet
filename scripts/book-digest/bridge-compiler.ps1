#Requires -Version 7.5
<#
.SYNOPSIS
  Plan, check, publish, and explicitly apply report-first cross-book Bridges.
  Every step prints compact JSON last. Plan, precheck, report, and preview never edit Cards.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Step,
    [Parameter(Mandatory)][string]$Staging,
    [Parameter(Mandatory)][string]$CitationKey,
    [string]$Work = '', [string]$Profile = 'book',
    $MaxSources = 60, $Candidates = 5, [string]$Books = '',
    [string]$Job = '', [string]$ReportId = '',
    [string]$Accept = '', [string]$AcceptEarlier = '', [switch]$Apply
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
. (Join-Path $PSScriptRoot 'book-of.ps1')
. (Join-Path $PSScriptRoot 'relatedness.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
function New-Map { , ([Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)) }
function New-Set { , ([Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)) }
function Canonical-Value($Value) {
    if ($null -eq $Value) { return $null }
    if ($Value -is [string]) { return [string]$Value }
    if ($Value -is [ValueType]) { return $Value }
    if ($Value -is [Collections.IDictionary]) {
        $keys = [string[]]@($Value.Keys); [Array]::Sort($keys, [StringComparer]::Ordinal)
        $result = [ordered]@{}
        foreach ($key in $keys) { $result[$key] = Canonical-Value $Value[$key] }
        return , $result
    }
    if ($Value -is [pscustomobject]) {
        $map = New-Map
        foreach ($property in $Value.PSObject.Properties) { $map[$property.Name] = $property.Value }
        return Canonical-Value $map
    }
    if ($Value -is [Collections.IEnumerable] -and $Value -isnot [string]) {
        $result = [Collections.Generic.List[object]]::new()
        foreach ($item in $Value) { $result.Add((Canonical-Value $item)) }
        return , $result.ToArray()
    }
    $Value
}
function Canonical-Json($Value) { ConvertTo-Json -InputObject (Canonical-Value $Value) -Depth 80 -Compress }
function Hash-Value($Value) { [Convert]::ToHexStringLower([Security.Cryptography.SHA256]::HashData($utf8.GetBytes((Canonical-Json $Value)))) }
function Check-JsonKeys($Element) {
    if ($Element.ValueKind -eq [Text.Json.JsonValueKind]::Object) {
        $keys = New-Set
        foreach ($property in $Element.EnumerateObject()) {
            if (-not $keys.Add($property.Name)) { throw [ArgumentException]::new("duplicate JSON key $($property.Name)") }
            Check-JsonKeys $property.Value
        }
    } elseif ($Element.ValueKind -eq [Text.Json.JsonValueKind]::Array) { foreach ($item in $Element.EnumerateArray()) { Check-JsonKeys $item } }
}
function Read-Json([string]$Path) {
    $raw = Get-Content -LiteralPath $Path -Raw -Encoding utf8
    $document = $null
    try { $document = [Text.Json.JsonDocument]::Parse($raw); Check-JsonKeys $document.RootElement }
    catch { throw [ArgumentException]::new("invalid JSON: $($_.Exception.Message)", $_.Exception) }
    finally { if ($document) { $document.Dispose() } }
    $raw | ConvertFrom-Json -AsHashtable -DateKind String
}
function Save-Json([string]$Path, $Value) {
    $temp = "$Path.$PID.tmp"
    [IO.File]::WriteAllText($temp, (ConvertTo-Json -InputObject $Value -Depth 80), $utf8)
    [IO.File]::Move($temp, $Path, $true)
}
function Read-Zt($Calls) {
    $Calls = @($Calls)
    foreach ($call in $Calls) { if ($call.tool -cnotin @('list_trees', 'get_tree', 'get_card', 'get_model')) { throw "Bridge read step refuses $($call.tool)" } }
    if (-not $Calls.Count) { return }
    if ($Calls.Count -gt 40) {
        $root=$PSScriptRoot
        $chunks=for($i=0;$i -lt $Calls.Count;$i+=40){,@{i=$i;calls=$Calls[$i..([Math]::Min($i+39,$Calls.Count-1))]}}
        $done=@($chunks|ForEach-Object -ThrottleLimit 6 -Parallel {
            . (Join-Path $using:root 'zt-calls.ps1')
            $part=$_
            try{$values=@(Invoke-Zt $part.calls);@{i=$part.i;values=$values;error=''}}catch{@{i=$part.i;values=@();error=$_.Exception.Message}}
        })
        $ordered=[object[]]::new($Calls.Count)
        foreach($part in $done){
            $expected=[Math]::Min(40,$Calls.Count-$part.i)
            if($part.error -or $part.values.Count -ne $expected){throw "incomplete Bridge read chunk $($part.i): $($part.error)"}
            for($k=0;$k -lt $expected;$k++){$ordered[$part.i+$k]=$part.values[$k]}
        }
        if($done.Count -ne $chunks.Count){throw 'incomplete Bridge read chunks'}
        return $ordered
    }
    $values = @(Invoke-Zt $Calls)
    if ($values.Count -ne @($Calls).Count) { throw 'incomplete Bridge read' }
    $values
}
function Integer-Option($Value, [string]$Name, [int]$Low, [int]$High) {
    $number = 0.0
    if (-not [double]::TryParse([string]$Value, [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$number) -or -not [double]::IsFinite($number) -or [Math]::Floor($number) -ne $number -or $number -lt $Low -or $number -gt $High) { throw [ArgumentException]::new("$Name must be an integer in $Low..$High") }
    [int]$number
}
function PS-Literal([string]$Value) { "'" + $Value.Replace("'", "''") + "'" }
function Body-Text($Value) { ([string]$Value).Replace("`r`n", "`n").Replace("`r", "`n") }
function Endpoint($Card, $Index) {
    $record = [ordered]@{}
    foreach ($name in @('address','workspace','tree','kind','type','facet','emphasis','level','form','title','body')) { $record[$name] = $Card.$name }
    $record.category = $Index.Category[[string]$Card.address]
    $record.book = if ($Index.BookOf.ContainsKey([string]$Card.address)) { $Index.BookOf[[string]$Card.address] } else { $null }
    $record.title = Body-Text $record.title; $record.body = Body-Text $record.body
    $values = @($record.workspace,$record.tree,$record.kind,$record.type,$record.facet,$record.emphasis,$record.level,$record.form,$record.title,$record.body)
    $record.fingerprint = 'sha256:' + (Hash-Value $values)
    $record.links = @($Card.outbound | Where-Object { -not $_.broken } | ForEach-Object address)
    $record.model = $Card.model
    $record
}
function Substantive($Card, $Index) {
    $Card.kind -ceq 'regular' -and $Index.Category[[string]$Card.address] -cin @('idea','event','finding','model','part') -and $Card.form -cne 'theory' -and ([string]$Card.title) -notmatch '^(符号表|(?i:notation))'
}
function Source-Rank($Card) {
    if ($Card.type -ceq 'model') { return $(if ($Card.level -ceq 'formal') { 0 } elseif ($Card.level -ceq 'concept') { 1 } else { 2 }) }
    if ($Card.type -ceq 'idea') { return 3 }
    if ($Card.type -ceq 'finding') { return 4 }
    5
}
function Plan-Result($Plan) {
    $jobs = @()
    if (-not (Test-Path -LiteralPath $proposalFile)) {
        Ensure-PlanFiles $Plan
        foreach ($entry in $Plan.jobs) {
            $answer=Join-Path $Work "out\$($entry.id).json"
            if(Test-Path -LiteralPath $answer){try{if(-not @( (Test-BridgeAnswer (Read-Json $answer) $Plan $entry).faults).Count){continue}}catch{}}
            $jobs += [ordered]@{ id = $entry.id; files = @((Join-Path $Work 'dsh-bridge-instructions.zh.md'), (Join-Path $Work "jobs\$($entry.id).md")); prompt = '按任务文件完成桥接判断，运行 SELF-CHECK，只回复一行 JSON。' }
        }
    }
    [ordered]@{ ok = $true; step = 'plan'; status = 'done'; plan_id = $Plan.plan_id; selected = $Plan.counts.selected; no_candidates = @($Plan.no_candidates).Count; jobs = $jobs }
}
function Ensure-PlanFiles($Plan) {
    # A process can stop after saving plan.json but before rendering its inputs.
    # Resume from the frozen snapshot; retain every input that already exists.
    New-Item -ItemType Directory -Force -Path $Work, (Join-Path $Work 'jobs'), (Join-Path $Work 'out'), (Join-Path $Work 'tmp') | Out-Null
    $instructions = Join-Path $Work 'dsh-bridge-instructions.zh.md'
    if (-not (Test-Path -LiteralPath $instructions)) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'dsh-bridge-instructions.zh.md') -Destination $instructions }
    $allow = Join-Path $Work 'read-only.allow.txt'
    if (-not (Test-Path -LiteralPath $allow)) { [IO.File]::WriteAllText($allow, '', $utf8) }
    foreach ($entry in $Plan.jobs) {
        $jobPath = Join-Path $Work "jobs\$($entry.id).md"
        if (Test-Path -LiteralPath $jobPath) { continue }
        $text = "# Bridge job $($entry.id)`n`nPlan: $($Plan.plan_id)`nProfile: $($Plan.profile)`nSnapshot: $(PS-Literal $planFile)`nOutput: $(PS-Literal (Join-Path $Work "out\$($entry.id).json"))`n"
        $jobTargets = New-Set
        foreach ($address in $entry.sources) {
            $s=$Plan.cards[$address];$cs=@($Plan.sources|Where-Object address -CEQ $address)[0].candidates
            $text += "`n## Source $address — $($s.title) ($($s.type), $($s.form))`n$($s.body)`n`nCandidates:`n"
            foreach($candidate in $cs){$c=$Plan.cards[$candidate.address];[void]$jobTargets.Add([string]$c.address);$text+="- $($c.address) — $($c.title); type=$($c.type); form=$($c.form); facet=$($c.facet); workspace=$($c.workspace); book=$($c.book); via=$($candidate.via -join ',')`n"}
        }
        $targetLiterals=@($jobTargets|ForEach-Object{PS-Literal $_}) -join ','
        $text += "`nRead only this job's frozen candidate bodies (never relocate by title):`n``````powershell`n`$snapshot = Get-Content -LiteralPath $(PS-Literal $planFile) -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String`n@($targetLiterals) | ForEach-Object { `$c = `$snapshot.cards[`$_]; @{address=`$c.address;workspace=`$c.workspace;book=`$c.book;type=`$c.type;form=`$c.form;facet=`$c.facet;title=`$c.title;body=`$c.body} | ConvertTo-Json -Depth 5 }`n```````n"
        $text += "`nSELF-CHECK: pwsh -NoProfile -File $(PS-Literal $PSCommandPath) -Step precheck -Staging $(PS-Literal $Staging) -CitationKey $(PS-Literal $CitationKey) -Work $(PS-Literal $Work) -Job $($entry.id)`n"
        [IO.File]::WriteAllText($jobPath, $text, $utf8)
    }
}
function Load-Plan {
    $value = Read-Json $planFile
    if ($value.schema -ne 1 -or $value.citation_key -cne $CitationKey) { throw [ArgumentException]::new('invalid plan schema or Citation key') }
    $inputValue = New-Map
    foreach ($name in $value.Keys) { if ($name -cnotin @('plan_id','created_at')) { $inputValue[$name] = $value[$name] } }
    if ($value.plan_id -cne ('p-' + (Hash-Value $inputValue))) { throw [ArgumentException]::new('plan fingerprint does not match its frozen content') }
    $value
}
$kindWords = @{
    '对应'='corresponds';'类比'='corresponds';'对照'='contrasts';'区别'='contrasts'
    '例示'='illustrates';'印证'='illustrates';'案例'='illustrates';'放宽'='relaxes';'推广'='relaxes'
    '反驳'='objects';'批评'='objects';'依赖'='rests-on';'基于'='rests-on';'特例'='specializes'
}
function Unit-Head($Card) { $Card.type -ceq 'model' -and -not $Card.facet -and $Card.form -cin @('definition','axiom','theorem','proposition','lemma','corollary') }
function Carrier-Allowed($Card) { $Card.workspace -ceq 'agent' -and $Card.kind -ceq 'regular' -and $Card.category -cin @('idea','event','finding','model') -and -not $Card.facet -and $Card.form -cne 'theory' -and ([string]$Card.title) -notmatch '^(符号表|(?i:notation))' }
function Sentence-Faults($Bridge, $Plan) {
    $draft = @{address='bridge-draft';title='跨书关联';body=$Bridge.sentence;category='idea';parent='';links=@($Bridge.target);kinds=@($Bridge.target)}
    $entries = @($draft); $setting = ''
    $carrier = $Plan.cards[$Bridge.carrier]
    # The model's setting when it holds a term table, else the carrier book's term table.
    $at = [string]$carrier.model.build.setting
    if (-not ($at -and $Plan.cards.ContainsKey($at) -and ([string]$Plan.cards[$at].body).Contains('术语表'))) { $at = '' }
    if (-not $at -and $carrier.book -and $Plan.term_tables.ContainsKey($carrier.book)) { $at = [string]$Plan.term_tables[$carrier.book] }
    if ($at -and $Plan.cards.ContainsKey($at) -and ([string]$Plan.cards[$at].body).Contains('术语表')) {
        $setting = $at; $entries += @{address=$at;title=$Plan.cards[$at].title;body=$Plan.cards[$at].body;category='terms'}
    }
    $Bridge.term_check = if ($setting) { 'passed' } else { 'unavailable' }
    $draftFile = Join-Path $Work "tmp\precheck-draft-$PID.json"
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $draftFile) | Out-Null
    Save-Json $draftFile $entries
    $lines = @(Invoke-ZtCheck -Json $draftFile -Setting $setting)
    @($lines | Where-Object { $_ -match '^bridge-draft (L1|L2|R2|R3|R4|R5|R6|R7|R9)\b' })
}
function Test-BridgeAnswer($Answer, $Plan, $Assignment) {
    $faults = [Collections.Generic.List[string]]::new()
    $proposals = [Collections.Generic.List[object]]::new()
    $none = [Collections.Generic.List[object]]::new()
    if ($Answer -isnot [Collections.IDictionary] -or @('schema','plan_id','job','sources'|Where-Object{$_ -cnotin @($Answer.Keys)}).Count -or $Answer.schema -isnot [long] -and $Answer.schema -isnot [int] -or $Answer.schema -ne 1 -or $Answer.plan_id -cne $Plan.plan_id -or $Answer.job -cne $Assignment.id -or $Answer.sources -isnot [array]) {
        return @{faults=@('BR1 invalid answer envelope, schema, plan, or job');proposals=@();none=@();envelope_ok=$false}
    }
    $seen = New-Set; $pairs = New-Set; $envelopeOk = $true
    foreach ($result in $Answer.sources) {
        if ($result -isnot [Collections.IDictionary] -or @('source','bridges','none'|Where-Object{$_ -cnotin @($result.Keys)}).Count -or $result.source -isnot [string] -or $result.source -cnotin @($Assignment.sources) -or -not $seen.Add($result.source) -or $result.bridges -isnot [array] -or $result.none -isnot [string]) {
            $faults.Add('BR1 invalid, unassigned, or repeated source result'); $envelopeOk = $false; continue
        }
        $source = [string]$result.source
        if (-not @($result.bridges).Count) {
            if (-not $result.none.Trim() -or $result.none -match '[\r\n]') { $faults.Add("BR8 $source needs a one-line none reason") }
            else { $none.Add(@{source=$source;why=$result.none}) }
            continue
        }
        if ($result.none -or @($result.bridges).Count -gt 2) { $faults.Add("BR6 $source needs one or two Bridges and an empty none"); continue }
        $sourcePlan = @($Plan.sources | Where-Object { $_.address -ceq $source })[0]
        $targets = @($sourcePlan.candidates | ForEach-Object address)
        foreach ($answerBridge in $result.bridges) {
            $start = $faults.Count
            if ($answerBridge -isnot [Collections.IDictionary]) { $faults.Add("BR1 $source Bridge is not an object"); continue }
            $fieldsOk = $true
            foreach ($field in @('carrier','target','word','sentence','why')) {
                if ($field -cnotin @($answerBridge.Keys) -or $answerBridge[$field] -isnot [string] -or -not $answerBridge[$field].Trim()) { $faults.Add("BR8 $source invalid $field"); $fieldsOk = $false }
            }
            if (-not $fieldsOk) { continue }
            $bridge = @{source=$source;carrier=$answerBridge.carrier;target=$answerBridge.target;word=$answerBridge.word;sentence=$answerBridge.sentence;why=$answerBridge.why}
            $new = $bridge.carrier -ceq $source -and $bridge.target -cin $targets
            $earlier = $bridge.target -ceq $source -and $bridge.carrier -cin $targets
            if (-not $new -and -not $earlier) { $faults.Add("BR2 $source pair is not a supplied candidate in either direction") }
            if (-not $Plan.cards.ContainsKey($bridge.carrier) -or -not $Plan.cards.ContainsKey($bridge.target)) { $faults.Add("BR3 $source endpoint snapshot missing"); continue }
            $carrier = $Plan.cards[$bridge.carrier]; $target = $Plan.cards[$bridge.target]
            if (-not (Carrier-Allowed $carrier) -or $target.category -cnotin @('idea','event','finding','model','part') -or $target.kind -cne 'regular' -or $target.form -ceq 'theory' -or ([string]$target.title) -match '^(符号表|(?i:notation))') { $faults.Add("BR3 $source ineligible carrier or target") }
            if ($Plan.cards[$source].workspace -cne 'agent' -or $Plan.cards[$source].book -cne $Plan.citation_key -or ($new -and $target.book -ceq $Plan.citation_key)) { $faults.Add("BR3 $source endpoint is not outside the selected source book") }
            $other=if($new){$target}else{$carrier}
            if($other.book -and @($Plan.options.books).Count -and $other.book -cnotin @($Plan.options.books)){ $faults.Add("BR3 $source endpoint is outside the target-book filter") }
            $bridge.section = if ($new) { 'new' } else { 'earlier' }
            $bridge.kind = if ($kindWords.ContainsKey($bridge.word)) { $kindWords[$bridge.word] } else { '' }
            if (-not $bridge.kind) { $faults.Add("BR4 $source unknown kind word") }
            if ($carrier.type -ceq 'model' -and $target.facet) { $faults.Add("BR4 $source a Model Card cannot take in a Model part through a Bridge") }
            if ($carrier.type -ceq 'model' -and $bridge.word -cin @('印证','案例')) { $faults.Add("BR4 $source a Model Card carrier takes no evidence word (印证, 案例)") }
            if ($bridge.kind -ceq 'rests-on' -and ($target.facet -or ($carrier.type -ceq 'model' -and -not (Unit-Head $target)))) { $faults.Add("BR4 $source invalid rests-on boundary") }
            if ($bridge.kind -ceq 'specializes' -and ($carrier.form -cne 'definition' -or $target.form -cne 'definition')) { $faults.Add("BR4 $source specialization requires two Definition heads") }
            if ($earlier -and (-not (Unit-Head $target) -or $carrier.type -cnotin @('idea','model') -or -not $carrier.book -or $carrier.book -ceq $Plan.citation_key -or $bridge.word -cnotin @('依赖','基于'))) { $faults.Add("BR4 $source invalid earlier-carrier exception") }
            $links = @([regex]::Matches($bridge.sentence, '\[\[([^\]\r\n]+)\]\]'))
            if ($links.Count -ne 1 -or $links[0].Groups[1].Value -cne $bridge.target -or $bridge.sentence -cnotmatch ([regex]::Escape($bridge.word) + '\s*\[\[' + [regex]::Escape($bridge.target) + '\]\]')) { $faults.Add("BR5 $source needs exactly one correctly targeted Link immediately after its kind word") }
            $pair = $bridge.carrier + "`0" + $bridge.target
            if ($pairs.Contains($bridge.target + "`0" + $bridge.carrier)) { $faults.Add("BR6 $source the reverse pair is already proposed: one relation per pair of Cards") }
            if (-not $pairs.Add($pair)) {
                $faults.Add("BR6 $source repeated directed pair")
                # A conflicting second sentence cannot silently leave the first executable.
                for($i=$proposals.Count-1;$i -ge 0;$i--){
                    $prior=$proposals[$i]
                    if($prior.carrier -ceq $bridge.carrier -and $prior.target -ceq $bridge.target -and ($prior.word -cne $bridge.word -or $prior.sentence -cne $bridge.sentence)){$proposals.RemoveAt($i)}
                }
            }
            if ($bridge.target -cin @($carrier.links) -or $bridge.carrier -cin @($target.links)) { $faults.Add("BR7 $source pair is already linked") }
            if ($bridge.sentence -match '[\r\n]|(?m)^\s*#|<!--|```|~~~|(?i:<(?:figure|div|script|svg|table)\b)' -or $bridge.why -match '[\r\n]|(?m)^\s*#|<!--|```|~~~|(?i:<(?:figure|div|script|svg|table)\b)') { $faults.Add("BR8 $source sentence and why must be plain single lines") }
            if ($faults.Count -eq $start) { foreach ($fault in @(Sentence-Faults $bridge $Plan)) { $faults.Add("BR9 $source $fault") } }
            if ($faults.Count -eq $start) { $proposals.Add($bridge) }
        }
    }
    if ($seen.Count -ne @($Assignment.sources).Count) { $faults.Add('BR1 every assigned source must appear exactly once'); $envelopeOk = $false }
    @{faults=@($faults.ToArray());proposals=$(if ($envelopeOk) { @($proposals.ToArray()) } else { @() });none=$(if ($envelopeOk) { @($none.ToArray()) } else { @() });envelope_ok=$envelopeOk}
}
function Content-Hash($Value, [string[]]$Except) {
    $copy = New-Map
    foreach ($name in $Value.Keys) { if ($name -cnotin $Except) { $copy[$name] = $Value[$name] } }
    Hash-Value $copy
}
function Load-Published {
    $value = Read-Json $proposalFile
    if ($value.schema -ne 1 -or $value.citation_key -cne $CitationKey -or $value.report_id -cne ('r-' + (Content-Hash $value @('report_id','created_at')))) { throw [ArgumentException]::new('invalid published report schema, Citation key, or fingerprint') }
    $value
}
function Publish-Report($Plan) {
    $proposals = [Collections.Generic.List[object]]::new()
    $refused = [Collections.Generic.List[object]]::new()
    $sourceOutcomes = New-Map
    foreach ($entry in $Plan.sources) { $sourceOutcomes[$entry.address] = @{source=$entry.address;outcome=$(if (@($entry.candidates).Count) {'failed'} else {'no_candidates'});why=$(if (@($entry.candidates).Count) {'No valid job output.'} else {'No eligible candidate.'})} }
    $completedJobs = 0
    foreach ($assignment in $Plan.jobs) {
        $answerPath = Join-Path $Work "out\$($assignment.id).json"
        if (-not (Test-Path -LiteralPath $answerPath)) { $refused.Add(@{job=$assignment.id;fault='missing job output'}); continue }
        try { $check = Test-BridgeAnswer (Read-Json $answerPath) $Plan $assignment } catch { $check = @{faults=@("BR1 $($_.Exception.Message)");proposals=@();none=@();envelope_ok=$false} }
        foreach ($fault in $check.faults) { $refused.Add(@{job=$assignment.id;fault=$fault}) }
        foreach ($bridge in $check.proposals) { $proposals.Add($bridge) }
        foreach ($entry in $check.none) { $sourceOutcomes[$entry.source] = @{source=$entry.source;outcome='none';why=$entry.why} }
        if (@($check.proposals).Count -or @($check.none).Count) { $completedJobs++ }
        foreach ($source in $assignment.sources) {
            if (@($check.proposals | Where-Object { $_.source -ceq $source }).Count) { $sourceOutcomes[$source] = @{source=$source;outcome='proposed';why=''} }
            elseif ($sourceOutcomes[$source].outcome -cne 'none') { $sourceOutcomes[$source] = @{source=$source;outcome='refused';why=(@($check.faults) -join ' | ')} }
        }
    }
    $pairs = New-Map
    foreach ($bridge in $proposals) {
        $key = $bridge.carrier + "`0" + $bridge.target
        if (-not $pairs.ContainsKey($key)) { $pairs[$key] = @{bridge=$bridge;conflict=$false}; continue }
        if ($pairs[$key].bridge.sentence -cne $bridge.sentence -or $pairs[$key].bridge.word -cne $bridge.word) { $pairs[$key].conflict=$true; $refused.Add(@{job='report';fault="conflicting sentences for $($bridge.carrier) -> $($bridge.target)"}) }
    }
    $ordered = [Collections.Generic.SortedDictionary[string, object]]::new([StringComparer]::Ordinal)
    foreach ($item in $pairs.Values) {
        if ($item.conflict) { continue }
        $bridge = $item.bridge
        $key = @($bridge.source,$bridge.carrier,$bridge.target,$bridge.word,$bridge.sentence) -join "`0"
        $ordered.Add($key,$bridge)
    }
    $published = @(); $n = 0
    foreach ($bridge in $ordered.Values) {
        $n++; $bridge.id = 'b{0:D3}' -f $n
        $bridge.carrier_fingerprint = $Plan.cards[$bridge.carrier].fingerprint
        $bridge.target_fingerprint = $Plan.cards[$bridge.target].fingerprint
        $published += $bridge
    }
    foreach ($entry in $sourceOutcomes.Values) {
        if ($entry.outcome -ceq 'proposed' -and -not @($published | Where-Object { $_.source -ceq $entry.source }).Count) { $entry.outcome='refused';$entry.why='Conflicting pair was refused.' }
    }
    $counts = [ordered]@{eligible=$Plan.counts.eligible;selected=$Plan.counts.selected;omitted=$Plan.counts.omitted;with_candidates=(@($Plan.sources).Count-@($Plan.no_candidates).Count);proposals=$published.Count;new=@($published | Where-Object section -CEQ 'new').Count;earlier=@($published | Where-Object section -CEQ 'earlier').Count;refused=$refused.Count}
    foreach ($class in @('proposed','none','no_candidates','refused','failed')) { $counts["${class}_sources"] = @($sourceOutcomes.Values | Where-Object { $_.outcome -ceq $class }).Count }
    $status = if (@($Plan.jobs).Count -and -not $completedJobs) { 'failed' } elseif ($refused.Count) { 'partial' } else { 'done' }
    $payload = [ordered]@{schema=1;plan_id=$Plan.plan_id;citation_key=$CitationKey;book_title=[string]$Plan['book_title'];profile=$Plan.profile;options=$Plan.options;read_coverage=$Plan.read_coverage;empty_reason=$Plan.empty_reason;cards=$Plan.cards;term_tables=$Plan.term_tables;plan_sources=$Plan.sources;proposals=$published;sources=@($sourceOutcomes.Values);omitted_sources=@($Plan.omitted_sources);refused=@($refused.ToArray());counts=$counts;status=$status}
    $payload.report_id = 'r-' + (Content-Hash $payload @('report_id','created_at'))
    $payload.created_at = [DateTime]::UtcNow.ToString('o')
    Save-Json $proposalFile $payload
    $payload
}
function Get-State($Published) {
    $file = Join-Path $Work 'state.json'
    if (-not (Test-Path -LiteralPath $file)) { return @{schema=1;report_id=$Published.report_id;accepted=@{new=@();earlier=@()};outcomes=(New-Map);attempts=@();status='done'} }
    $state = Read-Json $file
    if ($state.schema -ne 1 -or $state.report_id -cne $Published.report_id) { throw [ArgumentException]::new('apply state belongs to a different report') }
    $state
}
function Markdown-Cell($Value) { ([string]$Value).Replace('&','&amp;').Replace('<','&lt;').Replace('>','&gt;').Replace('|','\|').Replace("`r",' ').Replace("`n",' ') }
function Render-Report($Published, $State) {
    $lines = [Collections.Generic.List[string]]::new()
    $bookTitle = if ($Published['book_title']) { "$($Published.book_title)（$($Published.citation_key)）" } else { $Published.citation_key }
    $lines.Add("# 跨书桥接报告：$bookTitle")
    $lines.Add(''); $lines.Add("Plan: $($Published.plan_id)"); $lines.Add("Report: $($Published.report_id)")
    $lines.Add("Generated: $($Published.created_at). Profile: $($Published.profile). Source limit: $($Published.options.max_sources). Similarity candidates: $($Published.options.candidates), plus at most two term matches.")
    $lines.Add('Retrieval uses title twice plus excerpt, existing 80-term TF-IDF, and literal preferred terms. This is a bounded search, not an exhaustive semantic search. Attribution is inferred at planning time.')
    $lines.Add("Status: $($Published.status). Counts: $(ConvertTo-Json -InputObject $Published.counts -Compress).")
    $lines.Add("Read coverage: $(ConvertTo-Json -InputObject $Published.read_coverage -Compress).")
    if($Published.empty_reason){$lines.Add([string]$Published.empty_reason)}
    $lines.Add('Acceptance is for these exact sentences and this report id. New and earlier carriers require separate selections. No acceptance is inferred from this report or a timer.')
    $kindOrder = @('corresponds','contrasts','illustrates','relaxes','objects','rests-on','specializes')
    $lines.Add(''); $lines.Add('## 按目标书'); $lines.Add(''); $lines.Add('| Target book or bucket | Proposals | ' + ($kindOrder -join ' | ') + ' |'); $lines.Add('| --- | --- |' + (' --- |' * $kindOrder.Count))
    $groups = [Collections.Generic.SortedDictionary[string, object]]::new([StringComparer]::Ordinal)
    foreach ($bridge in $Published.proposals) {
        $other = if ($bridge.section -ceq 'earlier') { $Published.cards[$bridge.carrier] } else { $Published.cards[$bridge.target] }
        $bucket = if ($other.workspace -ceq 'human') { 'human' } elseif ($other.book) { $other.book } else { 'unattributed' }
        if (-not $groups.ContainsKey($bucket)) { $groups[$bucket] = @{total=0} }
        $groups[$bucket].total++; $groups[$bucket][[string]$bridge.kind] = 1 + [int]$groups[$bucket][[string]$bridge.kind]
    }
    foreach ($key in $groups.Keys) { $g = $groups[$key]; $lines.Add("| $(Markdown-Cell $key) | $($g.total) | " + (($kindOrder | ForEach-Object { [int]$g[$_] }) -join ' | ') + ' |') }
    foreach ($section in @('new','earlier')) {
        $lines.Add(''); $lines.Add($(if($section -ceq 'new'){'## 写入新书卡片的提议'}else{'## 写入旧书卡片的提议（需要单独同意）'})); $lines.Add('')
        $lines.Add('| id | Source | Carrier | Relation | Target | Exact sentence | Reason | State |'); $lines.Add('| --- | --- | --- | --- | --- | --- | --- | --- |')
        foreach ($bridge in @($Published.proposals | Where-Object { $_.section -ceq $section })) {
            $carrier=$Published.cards[$bridge.carrier];$target=$Published.cards[$bridge.target]
            $status=if($State.outcomes.ContainsKey($bridge.id)){$State.outcomes[$bridge.id].status}else{'open'}
            $cells=@($bridge.id,"$($bridge.source) $($Published.cards[$bridge.source].title)","$($bridge.carrier) $($carrier.title)",$bridge.word,"$($bridge.target) $($target.title)",$bridge.sentence,$bridge.why,$status)
            $lines.Add('| '+(($cells|ForEach-Object{Markdown-Cell $_}) -join ' | ')+' |')
        }
    }
    $lines.Add('');$lines.Add('## 没有桥接的源卡');$lines.Add('');$lines.Add('| Source | Outcome | Reason |');$lines.Add('| --- | --- | --- |')
    foreach($entry in $Published.sources){if($entry.outcome -cne 'proposed'){$sourceTitle=if($Published.cards.ContainsKey($entry.source)){$Published.cards[$entry.source].title}else{''};$lines.Add('| '+((@("$($entry.source) $sourceTitle",$entry.outcome,$entry.why)|ForEach-Object{Markdown-Cell $_})-join ' | ')+' |')}}
    $lines.Add('');$lines.Add('## 因上限而未选的源卡');$lines.Add('');$lines.Add('| Source | Title | Reason |');$lines.Add('| --- | --- | --- |')
    foreach($entry in $Published.omitted_sources){if($entry){$lines.Add("| $(Markdown-Cell $entry.address) | $(Markdown-Cell $entry.title) | Source cap |")}}
    $lines.Add('');$lines.Add('## 拒绝和失败');$lines.Add('');$lines.Add('| Job | Fault |');$lines.Add('| --- | --- |')
    foreach($entry in $Published.refused){$lines.Add("| $(Markdown-Cell $entry.job) | $(Markdown-Cell $entry.fault) |")}
    $lines.Add('');$lines.Add('## 应用与未完成的验证');$lines.Add('');$lines.Add('| id | Carrier | State | Detail |');$lines.Add('| --- | --- | --- | --- |')
    foreach($entry in $State.outcomes.GetEnumerator()){$lines.Add('| '+((@($entry.Key,$entry.Value.carrier,$entry.Value.status,$entry.Value.detail)|ForEach-Object{Markdown-Cell $_})-join ' | ')+' |')}
    [IO.File]::WriteAllText((Join-Path $Work 'report.md'), (($lines -join "`n")+"`n"), $utf8)
}
function Report-Summary($Published, $State, [string]$What = 'report') {
    $status = if ($Published.status -ceq 'done') { $State.status } else { $Published.status }
    $outcomes=@(foreach($bridge in $Published.proposals){if($State.outcomes.ContainsKey($bridge.id)){$State.outcomes[$bridge.id]}else{@{id=$bridge.id;carrier=$bridge.carrier;target=$bridge.target;status='open';detail='Not accepted or applied.'}}})
    $byStatus = [ordered]@{}; foreach ($o in $outcomes) { $byStatus[[string]$o.status] = 1 + [int]$byStatus[[string]$o.status] }
    $shown = @(if ($outcomes.Count -le 30) { $outcomes })
    [ordered]@{ok=($status -cin @('done','skipped'));step=$What;status=$status;citation_key=$CitationKey;plan_id=$Published.plan_id;report_id=$Published.report_id;counts=$Published.counts;outcome_counts=$byStatus;outcomes=$shown;outcomes_in=$(if ($outcomes.Count -le 30) { '' } else { 'report.md and state.json' });changed_carriers=@($State.attempts|Where-Object{$_.status -cin @('verified','written_unverified')}|ForEach-Object carrier|Sort-Object -Unique -CaseSensitive);report=(Join-Path $Work 'report.md')}
}
function Select-Proposals($Published) {
    if (($Accept -or $AcceptEarlier -or $Apply) -and $ReportId -cne $Published.report_id) { throw [ArgumentException]::new('acceptance must name this exact published report id') }
    $chosen = [Collections.Generic.List[object]]::new(); $ids = New-Set
    foreach ($section in @('new','earlier')) {
        $inputIds = if ($section -ceq 'new') { $Accept } else { $AcceptEarlier }
        if (-not $inputIds) { continue }
        $values = if ($inputIds -ceq 'all') { @($Published.proposals | Where-Object { $_.section -ceq $section } | ForEach-Object id) } else { @($inputIds.Split(',') | ForEach-Object { $_.Trim() }) }
        foreach ($id in $values) {
            if ($id -cnotmatch '^b[0-9]{3}$' -or -not $ids.Add($id)) { throw [ArgumentException]::new('invalid or repeated accepted proposal id') }
            $found = @($Published.proposals | Where-Object { $_.id -ceq $id -and $_.section -ceq $section })
            if ($found.Count -ne 1) { throw [ArgumentException]::new('unknown id or id in the wrong acceptance section') }
            $chosen.Add($found[0])
        }
    }
    if (-not $chosen.Count -and ($Apply -or $ReportId)) { throw [ArgumentException]::new('a report id or write switch requires a nonempty accepted selection') }
    if ($chosen.Count) {
        $sources = @($chosen | ForEach-Object source | Sort-Object -Unique -CaseSensitive)
        $answer = @{schema=1;plan_id=$Published.plan_id;job='apply';sources=@()}
        foreach ($source in $sources) { $answer.sources += @{source=$source;bridges=@($chosen | Where-Object { $_.source -ceq $source });none=''} }
        $view = @{plan_id=$Published.plan_id;citation_key=$Published.citation_key;profile=$Published.profile;options=$Published.options;cards=$Published.cards;term_tables=$Published.term_tables;sources=$Published.plan_sources}
        $checked = Test-BridgeAnswer $answer $view @{id='apply';sources=$sources}
        if (@($checked.faults).Count) { throw [ArgumentException]::new("accepted proposals fail validation: $($checked.faults -join ' | ')") }
    }
    , $chosen.ToArray()
}
function Current-Context($Addresses) {
    $trees = @((Read-Zt @(@{tool='list_trees';arguments=@{}}))[0].trees)
    $details = @(Read-Zt @($trees | ForEach-Object { @{tool='get_tree';arguments=@{tree=$_.tree}} }))
    $summaries = @($details | ForEach-Object cards)
    $index = Get-BookIndex -Trees $trees -Cards $summaries
    $byAddress = New-Map; $cards = New-Map; $errors = New-Map
    foreach ($card in $summaries) { $byAddress[[string]$card.address] = $card }
    foreach ($address in $Addresses) {
        if (-not $byAddress.ContainsKey($address)) { continue }
        try { $cards[$address] = (Read-Zt @(@{tool='get_card';arguments=@{at=$address}}))[0] }
        catch { $errors[$address] = $_.Exception.Message }
    }
    @{index=$index;summaries=$byAddress;cards=$cards;errors=$errors}
}
function Expected-Fingerprint($Snapshot, $State) {
    $latest = @($State.attempts | Where-Object { $_.carrier -ceq $Snapshot.address -and $_.status -ceq 'verified' })
    if ($latest.Count) { return $latest[-1].intended_fingerprint }
    $Snapshot.fingerprint
}
function Expected-Book($Snapshot, $State) {
    $latest = @($State.attempts | Where-Object { $_.carrier -ceq $Snapshot.address -and $_.status -ceq 'verified' -and $_.ContainsKey('intended_book') })
    if ($latest.Count) { return $latest[-1].intended_book }
    $Snapshot.book
}
function Selection-States($Chosen, $Published, $State, $Context) {
    $results = @()
    foreach ($bridge in $Chosen) {
        $outcome = @{id=$bridge.id;carrier=$bridge.carrier;target=$bridge.target;status='ready';detail=''}
        $expected = New-Map; $actual = New-Map
        foreach ($address in @($bridge.carrier,$bridge.target)) {
            if (-not $Context.cards.ContainsKey($address)) { $outcome.status='stale';$outcome.detail="missing endpoint $address"; continue }
            $snapshot = $Published.cards[$address]; $now = Endpoint $Context.cards[$address] $Context.index
            $expected[$address] = $snapshot; $actual[$address] = $now
            foreach ($field in @('workspace','tree','kind','type','facet','emphasis','level','form','title')) {
                if ($now[$field] -cne $snapshot[$field]) { $outcome.status='stale';$outcome.detail="changed endpoint $address $field" }
            }
            if ($now.book -cne (Expected-Book $snapshot $State)) { $outcome.status='stale';$outcome.detail="changed endpoint $address book" }
        }
        if ($outcome.status -ceq 'stale') { $results += $outcome;continue }
        $journal = $State.outcomes.ContainsKey($bridge.id) -and $State.outcomes[$bridge.id].status -ceq 'verified'
        $knownPair=@($Published.proposals|Where-Object{
            $State.outcomes.ContainsKey($_.id) -and $State.outcomes[$_.id].status -ceq 'verified' -and
            $_.carrier -ceq $bridge.carrier -and $_.target -ceq $bridge.target
        }).Count -gt 0
        $linked = $bridge.target -cin @($actual[$bridge.carrier].links) -or $bridge.carrier -cin @($actual[$bridge.target].links)
        if ($linked -and -not $journal -and -not $knownPair) { $outcome.status='already_linked';$outcome.detail='An existing Link is preserved; this is not evidence of an applied explanation.' }
        else {
            foreach ($address in @($bridge.carrier,$bridge.target)) {
                if ($actual[$address].fingerprint -cne (Expected-Fingerprint $expected[$address] $State)) { $outcome.status='stale';$outcome.detail="changed body or endpoint $address" }
            }
            if ($outcome.status -ceq 'ready' -and $journal) { $outcome.status='verified';$outcome.detail='Previously verified accepted paragraph.' }
        }
        $results += $outcome
    }
    , $results
}
function Body-Fingerprint($Snapshot, [string]$Body) {
    'sha256:' + (Hash-Value @($Snapshot.workspace,$Snapshot.tree,$Snapshot.kind,$Snapshot.type,$Snapshot.facet,$Snapshot.emphasis,$Snapshot.level,$Snapshot.form,$Snapshot.title,(Body-Text $Body)))
}
function Model-Shape($Model) {
    if (-not $Model) { return @{members=@();faults=@()} }
    $members = @($Model.parts | ForEach-Object { [string]$_.address } | Sort-Object -Unique -CaseSensitive)
    $faults = @()
    foreach ($kind in @('closure','reach_in','forward','unanchored','beyond_level')) {
        foreach ($fault in @($Model.build.$kind)) {
            if ($null -eq $fault) { continue }
            $faults += "$kind|$($fault.from)|$($fault.to)|$($fault.unit)|$($fault.address)"
        }
    }
    @{members=$members;faults=$faults}
}
function Verify-Attempt($Attempt, $Published, $State, $Context) {
    if (-not $Context.cards.ContainsKey($Attempt.carrier)) { throw 'written carrier cannot be reread' }
    $current = Endpoint $Context.cards[$Attempt.carrier] $Context.index
    if ($current.fingerprint -cne $Attempt.intended_fingerprint) { throw 'persisted carrier differs from saved intent' }
    $before=$Published.cards[$Attempt.carrier]
    $drafts=@(
        @{address='before';title=$before.title;body=$before.body;category='idea';parent='';links=@($before.links);kinds=@($before.kinds.Keys)},
        @{address='after';title=$current.title;body=$current.body;category='idea';parent='';links=@($current.links);kinds=@($Context.summaries[$Attempt.carrier].kinds.PSObject.Properties|ForEach-Object Name)}
    )
    $draftFile=Join-Path $Work "tmp\changed-card-$PID.json";Save-Json $draftFile $drafts
    $checks=@(Invoke-ZtCheck -Json $draftFile)
    $old=@($checks|Where-Object{$_ -match '^before L[12]\b'}|ForEach-Object{$_ -replace '^before ',''})
    foreach($fault in @($checks|Where-Object{$_ -match '^after L[12]\b'}|ForEach-Object{$_ -replace '^after ',''})){if($fault -cnotin $old){throw "new changed-Card fault: $fault"}}
    foreach ($id in $Attempt.ids) {
        $bridge = @($Published.proposals | Where-Object id -CEQ $id)[0]
        if (-not $Context.cards.ContainsKey($bridge.target)) { throw "target cannot be reread: $($bridge.target)" }
        $target = Endpoint $Context.cards[$bridge.target] $Context.index
        if ($target.fingerprint -cne (Expected-Fingerprint $Published.cards[$bridge.target] $State)) { throw "target changed: $($bridge.target)" }
        if ($bridge.target -cnotin @($current.links) -or $Context.summaries[$Attempt.carrier].kinds.($bridge.target) -cne $bridge.kind) { throw 'persisted Relation kind or Link differs from accepted sentence' }
        if ($Attempt.carrier -cnotin @($Context.cards[$bridge.target].inbound | ForEach-Object address)) { throw 'generated Reverse link is missing' }
        $view = @{cards=$Published.cards;term_tables=$Published.term_tables}
        if (@(Sentence-Faults $bridge $view).Count) { throw 'accepted sentence no longer passes the Card checks' }
    }
    foreach ($address in @($Attempt.carrier) + @($Attempt.targets)) {
        $baseline = if ($Attempt.ContainsKey('model_before') -and $Attempt.model_before.ContainsKey($address)) { $Attempt.model_before[$address] } else { Model-Shape $Published.cards[$address].model }
        $actual = Model-Shape $Context.cards[$address].model
        if ((Canonical-Json $baseline.members) -cne (Canonical-Json $actual.members)) { throw "model membership changed at $address" }
        foreach ($fault in $actual.faults) { if ($fault -cnotin $baseline.faults) { throw "new model fault at ${address}: $fault" } }
    }
    # Accepted Links can change the heuristic vote. Remember only a fully verified
    # result, alongside the exact intended body fingerprint, for later retries.
    $Attempt.intended_book = $current.book
}
function Save-State($State) { Save-Json (Join-Path $Work 'state.json') $State }
function Set-Outcomes($State, $Attempt, [string]$Status, [string]$Detail) {
    foreach ($id in $Attempt.ids) {
        $State.outcomes[$id] = @{id=$id;carrier=$Attempt.carrier;status=$Status;detail=$Detail}
    }
}
function Apply-Selected($Chosen, $Published) {
    # FileShare.None is the lock. An abandoned file is safe to reopen after process exit.
    $lock = $null
    try {
        try { $lock = [IO.File]::Open((Join-Path $Work 'apply.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None) }
        catch { throw [ArgumentException]::new('another apply owns this report lock') }
        $state = Get-State $Published
        if(-not $state.ContainsKey('accepted')){$state.accepted=@{new=@();earlier=@()}}
        foreach($section in @('new','earlier')){$state.accepted[$section]=@(@($state.accepted[$section])+@($Chosen|Where-Object section -CEQ $section|ForEach-Object id)|Sort-Object -Unique -CaseSensitive)}
        $addresses = New-Set
        foreach ($bridge in $Chosen) { [void]$addresses.Add($bridge.carrier);[void]$addresses.Add($bridge.target) }
        $context = Current-Context $addresses
        # Reconcile saved intents before considering another paragraph on the same carrier.
        foreach ($attempt in @($state.attempts | Where-Object { $_.status -cnotin @('verified','abandoned') })) {
            if ($attempt.carrier -cnotin @($Chosen | ForEach-Object carrier)) { continue }
            $now = if ($context.cards.ContainsKey($attempt.carrier)) { Endpoint $context.cards[$attempt.carrier] $context.index } else { $null }
            if (@($attempt.ids | Where-Object { $_ -cnotin @($Chosen | ForEach-Object id) }).Count) {
                if ($now -and $now.fingerprint -ceq $attempt.expected_fingerprint) {
                    $attempt.status = 'abandoned'
                    Set-Outcomes $state $attempt 'open' 'A saved intent was never written and the new selection differs; it was dropped.'
                } elseif ($now -and $now.fingerprint -ceq $attempt.intended_fingerprint) {
                    try { Verify-Attempt $attempt $Published $state $context; $attempt.status='verified';Set-Outcomes $state $attempt 'verified' 'Recovered persisted accepted paragraph.' }
                    catch { $attempt.status='written_unverified';Set-Outcomes $state $attempt 'written_unverified' $_.Exception.Message }
                } else {
                    $attempt.status = 'conflict'; Set-Outcomes $state $attempt 'conflict' 'A saved intent holds ids outside this selection, and the carrier changed.'
                }
                continue
            }
            if ($now -and $now.fingerprint -ceq $attempt.intended_fingerprint) {
                try { Verify-Attempt $attempt $Published $state $context; $attempt.status='verified';Set-Outcomes $state $attempt 'verified' 'Recovered persisted accepted paragraph.' }
                catch { $attempt.status='written_unverified';Set-Outcomes $state $attempt 'written_unverified' $_.Exception.Message }
            } elseif (-not $now -or $now.fingerprint -cne $attempt.expected_fingerprint) {
                $attempt.status='conflict';Set-Outcomes $state $attempt 'conflict' 'Current carrier matches neither saved expected nor intended content.'
            } else {
                $attempt.status='prepared'
            }
        }
        foreach ($carrier in @($Chosen | ForEach-Object carrier | Sort-Object -Unique -CaseSensitive)) {
            $group = @($Chosen | Where-Object carrier -CEQ $carrier | Sort-Object id -CaseSensitive)
            $context = Current-Context $addresses
            $outcomes = Selection-States $group $Published $state $context
            $pending = @($state.attempts | Where-Object { $_.carrier -ceq $carrier -and $_.status -cnotin @('verified','abandoned') })
            if (@($pending | Where-Object { $_.status -cin @('conflict','written_unverified') }).Count) {
                # C9. Every accepted id gets an outcome: a blocked carrier names the reason.
                foreach ($bridge in $group) {
                    $known = $state.outcomes.ContainsKey($bridge.id) -and $state.outcomes[$bridge.id].status -cin @('verified','conflict','written_unverified')
                    if (-not $known) { $state.outcomes[$bridge.id] = @{id=$bridge.id;carrier=$carrier;target=$bridge.target;status='blocked';detail='The carrier has an earlier conflict or unverified write; resolve it first.'} }
                }
                continue
            }
            foreach ($outcome in $outcomes) { $state.outcomes[$outcome.id]=$outcome }
            $ready = @($group | Where-Object { $state.outcomes[$_.id].status -ceq 'ready' })
            if($pending.Count){
                foreach($bridge in @($ready|Where-Object{$_.id -cnotin $pending[-1].ids})){$state.outcomes[$bridge.id]=@{id=$bridge.id;carrier=$carrier;status='open';detail='Resume the saved intent first; apply this additional id afterward.'}}
                $ready=@($ready|Where-Object{$_.id -cin $pending[-1].ids})
            }
            if (-not $ready.Count) { continue }
            $current = $context.cards[$carrier]
            $body = Body-Text $current.body
            foreach ($bridge in $ready) { $body += "`n`n跨书关联：$($bridge.sentence)" }
            $attempt = if ($pending.Count) { $pending[-1] } else {
                @{carrier=$carrier;ids=@($ready | ForEach-Object id);targets=@($ready | ForEach-Object target);expected_fingerprint=(Endpoint $current $context.index).fingerprint;intended_body=$body;intended_fingerprint=(Body-Fingerprint $Published.cards[$carrier] $body);status='prepared'}
            }
            if (-not $pending.Count) {
                # The model shape of the carrier and targets just before the edit: verification compares with it.
                $attempt.model_before = @{}
                foreach ($address in @($carrier) + @($attempt.targets)) { if ($context.cards.ContainsKey($address)) { $attempt.model_before[$address] = Model-Shape $context.cards[$address].model } }
                $state.attempts += $attempt
            }
            Set-Outcomes $state $attempt 'prepared' 'Saved intent; no write verified yet.'
            Save-State $state
            # Pass the raw reread as base. No metadata, title, or address mutation is possible here.
            $allow = Join-Path $Work 'tmp\accepted-carriers.allow.txt'
            [IO.File]::WriteAllText($allow, "$carrier`n", $utf8)
            $call = @{tool='edit_card';arguments=@{at=$carrier;body=$attempt.intended_body;base=$current.text}}
            try {
                $lines = @((ConvertTo-Json -InputObject @($call) -Depth 15) | pwsh -NoProfile -File $script:ZtHelper - -Allow $allow)
                if ($LASTEXITCODE -ne 0 -or -not @($lines | Where-Object { $_ -match '^0 ok ' }).Count) { throw "guarded edit was not confirmed: $($lines -join ' | ')" }
                $context = Current-Context $addresses
                Verify-Attempt $attempt $Published $state $context
                $attempt.status='verified';Set-Outcomes $state $attempt 'verified' 'Persisted text, Relation, Reverse link, and model boundaries verified.'
            } catch {
                $detail=$_.Exception.Message
                try{$read=(Read-Zt @(@{tool='get_card';arguments=@{at=$carrier}}))[0];$fp=(Endpoint $read $context.index).fingerprint}catch{$fp=''}
                if($fp -and $fp -cne $attempt.intended_fingerprint){$attempt.status='conflict';Set-Outcomes $state $attempt 'conflict' $detail}
                else{$attempt.status='written_unverified';Set-Outcomes $state $attempt 'written_unverified' $detail}
            }
            Save-State $state
        }
        $state.status = if (@($state.outcomes.Values | Where-Object { $_.status -cin @('stale','conflict','prepared','written_unverified','open','blocked') }).Count) { 'partial' } else { 'done' }
        Save-State $state
        Render-Report $Published $state
        $state
    } finally { if ($lock) { $lock.Dispose() } }
}
try {
    if ($Step -cnotin @('plan','precheck','report','apply')) { throw [ArgumentException]::new('unknown Bridge step') }
    if ($Profile -cnotin @('book','math')) { throw [ArgumentException]::new('unknown Bridge profile') }
    $MaxSources = Integer-Option $MaxSources 'MaxSources' 1 120
    $Candidates = Integer-Option $Candidates 'Candidates' 1 8
    $Staging = [IO.Path]::GetFullPath($Staging)
    if (-not $Work) { $Work = Join-Path $Staging 'bridge' }
    $Work = [IO.Path]::GetFullPath($Work)
    $planFile = Join-Path $Work 'plan.json'; $proposalFile = Join-Path $Work 'proposals.json'
    $manifestFile = Join-Path $Staging 'manifest.json'
    if (Test-Path -LiteralPath $manifestFile) { if ((Read-Json $manifestFile).citation_key -cne $CitationKey) { throw [ArgumentException]::new('manifest Citation key disagrees with request') } }
    $bookFilter = [string[]]@($Books.Split(',') | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    [Array]::Sort($bookFilter, [StringComparer]::Ordinal)
    $options = [ordered]@{ max_sources = $MaxSources; candidates = $Candidates; books = $bookFilter }
    if ($Step -ceq 'precheck') {
        $plan = Load-Plan
        if ($Job -cnotmatch '^br-[0-9]{3}$') { throw [ArgumentException]::new('invalid job id') }
        $assignment = @($plan.jobs | Where-Object { $_.id -ceq $Job })
        if ($assignment.Count -ne 1) { throw [ArgumentException]::new('job is not in the frozen plan') }
        $answer = Read-Json (Join-Path $Work "out\$Job.json")
        $check = Test-BridgeAnswer $answer $plan $assignment[0]
        foreach ($fault in $check.faults) { Write-Output $fault }
        Write-Output "faults: $(@($check.faults).Count)"
        [ordered]@{ok=(@($check.faults).Count -eq 0);step='precheck';job=$Job;faults=@($check.faults).Count;status=$(if (@($check.faults).Count) {'failed'} else {'done'})} | ConvertTo-Json -Compress
        exit $(if (@($check.faults).Count) {1} else {0})
    }
    if ($Step -ceq 'report') {
        $published = if (Test-Path -LiteralPath $proposalFile) { Load-Published } else { Publish-Report (Load-Plan) }
        $state = Get-State $published
        Render-Report $published $state
        Report-Summary $published $state | ConvertTo-Json -Depth 30 -Compress
        exit 0
    }
    if ($Step -ceq 'apply') {
        $published = Load-Published
        $chosen = Select-Proposals $published
        $state = Get-State $published
        $wanted = New-Set
        foreach ($bridge in $chosen) { [void]$wanted.Add([string]$bridge.carrier);[void]$wanted.Add([string]$bridge.target) }
        $context = Current-Context $wanted
        $outcomes = Selection-States $chosen $published $state $context
        if ($Apply) {
            $state = Apply-Selected $chosen $published
            Report-Summary $published $state 'apply' | ConvertTo-Json -Depth 30 -Compress
            exit 0
        }
        $summary = Report-Summary $published $state 'apply'
        $summary.preview = $true; $summary.outcomes = $outcomes
        if (@($outcomes | Where-Object { $_.status -cin @('stale','conflict','written_unverified') }).Count) { $summary.ok=$false;$summary.status='partial' }
        $summary | ConvertTo-Json -Depth 30 -Compress
        exit 0
    }
    if (Test-Path -LiteralPath $planFile) {
        $plan = Load-Plan
        if ($plan.citation_key -cne $CitationKey -or $plan.profile -cne $Profile -or (Canonical-Json $plan.options) -cne (Canonical-Json $options)) { throw [ArgumentException]::new('frozen plan inputs differ; use a fresh output folder') }
        Plan-Result $plan | ConvertTo-Json -Depth 20 -Compress
        exit 0
    }
    $trees = @((Read-Zt @(@{ tool='list_trees'; arguments=@{} }))[0].trees)
    $details = @(Read-Zt @($trees | ForEach-Object { @{tool='get_tree';arguments=@{tree=$_.tree}} }))
    $summaries = @($details | ForEach-Object cards)
    $index = Get-BookIndex -Trees $trees -Cards $summaries
    if (-not $index.BookSet.Contains($CitationKey)) { throw [ArgumentException]::new('Citation key is not an existing agent Literature tree') }
    foreach ($book in $bookFilter) { if (-not $index.BookSet.Contains($book)) { throw [ArgumentException]::new("unknown target book $book") } }
    $byAddress = New-Map
    foreach ($summary in $summaries) { $byAddress.Add([string]$summary.address, $summary) }
    $mathPriority = New-Map
    if ($Profile -ceq 'math') {
        $mathHeads = @($summaries | Where-Object { $_.workspace -ceq 'agent' -and $index.BookOf[[string]$_.address] -ceq $CitationKey -and $_.type -ceq 'model' -and -not $_.facet -and $_.form })
        $mathReports = @(Read-Zt @($mathHeads | ForEach-Object { @{tool='get_model';arguments=@{at=$_.address}} }))
        foreach ($report in $mathReports) {
            if ($report.form -ceq 'theory') {
                foreach ($at in @($report.math.main_results)+@($report.math.objects)) { if ($at) { $mathPriority[[string]$at]=0 } }
            }
        }
        foreach ($report in $mathReports) {
            $at=[string]$report.head.address
            if ($mathPriority.ContainsKey($at)) { continue }
            if ($report.form -ceq 'definition') { $mathPriority[$at]=1 }
            elseif ($report.form -cin @('axiom','theorem','proposition','lemma','corollary') -and $report.math.load -ge 2) { $mathPriority[$at]=2 }
        }
    }
    $ordered = [Collections.Generic.SortedDictionary[string, object]]::new([StringComparer]::Ordinal)
    foreach ($card in $summaries) {
        if ($card.workspace -cne 'agent' -or -not $index.BookOf.ContainsKey([string]$card.address) -or $index.BookOf[[string]$card.address] -cne $CitationKey -or -not (Substantive $card $index) -or $card.facet) { continue }
        if ($Profile -ceq 'math') {
            if (-not (Unit-Head $card) -or -not $mathPriority.ContainsKey([string]$card.address)) { continue }
            $key = '{0:D2}|{1}' -f $mathPriority[[string]$card.address],$card.address
        } else {
            if ($card.type -ceq 'idea' -and $index.BookSet.Contains([string]$card.tree)) { continue }
            $key = '{0:D2}|{1:D10}|{2}' -f (Source-Rank $card), $(if ($card.type -ceq 'idea') { [int]::MaxValue - [int]$card.inbound } else { 0 }), $card.address
        }
        $ordered.Add($key, $card)
    }
    $sources = @($ordered.Values | Select-Object -First $MaxSources)
    $termCards = @($summaries | Where-Object { $_.workspace -ceq 'agent' -and $index.Category[[string]$_.address] -ceq 'terms' })
    $termReads = if ($termCards.Count) { @(Read-Zt @($termCards | ForEach-Object { @{tool='get_card';arguments=@{at=$_.address}} })) } else { @() }
    $termTables = New-Map; $terms = New-Set
    foreach ($card in $termReads) {
        if (-not $index.BookOf.ContainsKey([string]$card.address)) { continue }
        $book = $index.BookOf[[string]$card.address]
        $termTables[$book] = [string]$card.address
        if ($book -cne $CitationKey) { continue }
        foreach ($line in ([string]$card.body -split "`n")) {
            if ($line -match '^\s*\|\s*([^|]+)\|') {
                $term = $Matches[1].Trim()
                if ($term -notmatch '^[-: ]+$' -and $term -notmatch '^(术语|词语|term)$') { [void]$terms.Add($term) }
            }
        }
    }
    $termHits = New-Map
    function Term-Hits([string]$Term) {
        if (-not $termHits.ContainsKey($Term)) {
            $hits = New-Set
            foreach ($card in $summaries) { if (("$($card.title) $($card.excerpt)").IndexOf($Term, [StringComparison]::OrdinalIgnoreCase) -ge 0) { [void]$hits.Add([string]$card.address) } }
            $termHits[$Term] = $hits
        }
        , $termHits[$Term]
    }
    function Source-Terms($Source) {
        $own = New-Set
        $text = "$($Source.title) $($Source.excerpt)"
        foreach ($term in $terms) { if ($term.Length -ge 2 -and $text.IndexOf($term, [StringComparison]::OrdinalIgnoreCase) -ge 0) { [void]$own.Add($term) } }
        if ($Source.type -ceq 'model') {
            # The model's or unit's name: its title up to the first colon.
            $name = ([string]$Source.title -split '[：:]', 2)[0].Trim()
            if ($name.Length -ge 2) { [void]$own.Add($name) }
        }
        , $own
    }
    $documents = @($summaries | ForEach-Object { @{ id = [string]$_.address; text = "$($_.title) $($_.title) $($_.excerpt)" } })
    $relatedness = New-Relatedness $documents 80
    $selected = @(); $none = @(); $wanted = New-Set
    foreach ($source in $sources) {
        $allowed = New-Set
        foreach ($target in $summaries) {
            $targetBook = if ($index.BookOf.ContainsKey([string]$target.address)) { $index.BookOf[[string]$target.address] } else { $null }
            if ($target.address -ceq $source.address -or $targetBook -ceq $CitationKey -or -not (Substantive $target $index)) { continue }
            if ($source.type -ceq 'model' -and $target.facet) { continue }
            if ($targetBook -and $bookFilter.Count -and $targetBook -cnotin $bookFilter) { continue }
            if ($target.address -cin @($source.links) -or $source.address -cin @($target.links)) { continue }
            [void]$allowed.Add([string]$target.address)
        }
        $targets = @(Find-Related $relatedness ([string]$source.address) $Candidates { param($id) -not $allowed.Contains($id) })
        $candidatesForSource = @($targets | ForEach-Object { @{ address = $_; via = @('relatedness') } })
        $termMatches = [Collections.Generic.SortedDictionary[string, object]]::new([StringComparer]::Ordinal)
        $termCounts = New-Map
        foreach ($term in (Source-Terms $source)) {
            foreach ($address in (Term-Hits $term)) { if ($allowed.Contains($address)) { $termCounts[$address] = 1 + $(if ($termCounts.ContainsKey($address)) { $termCounts[$address] } else { 0 }) } }
        }
        foreach ($address in @($termCounts.Keys)) {
            $termCount = $termCounts[$address]
            if ($address -cin $targets) {
                foreach ($candidate in $candidatesForSource) { if ($candidate.address -ceq $address) { $candidate.via += 'term' } }
            } else { $termMatches.Add(('{0:D10}|{1}' -f ([int]::MaxValue-$termCount),$address),$address) }
        }
        foreach ($address in @($termMatches.Values | Select-Object -First 2)) { $candidatesForSource += @{address=$address;via=@('term')} }
        $targets = @($candidatesForSource | ForEach-Object address)
        $selected += @{ address = [string]$source.address; candidates = $candidatesForSource }
        [void]$wanted.Add([string]$source.address)
        foreach ($target in $targets) { [void]$wanted.Add([string]$target) }
        if (-not $targets.Count) { $none += [string]$source.address }
    }
    $cards = New-Map
    $reads = if ($wanted.Count) { @(Read-Zt @($wanted | ForEach-Object { @{tool='get_card';arguments=@{at=$_}} })) } else { @() }
    foreach ($card in $reads) {
        $summary = $byAddress[[string]$card.address]
        foreach ($field in @('workspace','tree','kind','type','facet','emphasis','level','form','title')) { if ($card.$field -cne $summary.$field) { throw "selected Card changed during planning: $($card.address) $field" } }
        $cards[[string]$card.address] = Endpoint $card $index
        $cards[[string]$card.address].kinds=$summary.kinds
    }
    $settings=New-Set
    foreach($card in @($cards.Values)){if($card.model.build.setting -and -not $cards.ContainsKey([string]$card.model.build.setting)){[void]$settings.Add([string]$card.model.build.setting)}}
    foreach($card in @(Read-Zt @($settings|ForEach-Object{@{tool='get_card';arguments=@{at=$_}}}))){$cards[[string]$card.address]=Endpoint $card $index}
    foreach ($card in $termReads) { $cards[[string]$card.address] = Endpoint $card $index }
    $jobs = @(); $withCandidates = @($selected | Where-Object { @($_.candidates).Count })
    for ($i = 0; $i -lt $withCandidates.Count; $i += 12) {
        $jobs += @{ id = ('br-{0:D3}' -f (1 + [int]($i / 12))); sources = @($withCandidates[$i..([Math]::Min($i+11,$withCandidates.Count-1))] | ForEach-Object address) }
    }
    $plan = [ordered]@{ schema=1; citation_key=$CitationKey; profile=$Profile; options=$options; counts=@{eligible=$ordered.Count;selected=$sources.Count;omitted=$ordered.Count-$sources.Count}; cards=$cards; book_of=$index.BookOf; term_tables=$termTables; sources=$selected; jobs=$jobs; no_candidates=$none; omitted_sources=@($ordered.Values | Select-Object -Skip $MaxSources | ForEach-Object { @{address=$_.address;title=$_.title} }) }
    $plan.book_title = [string]@($summaries | Where-Object { [string]$_.address -ceq $CitationKey })[0].title
    $plan.read_coverage=@{trees=$trees.Count;summaries=$summaries.Count;endpoints=$wanted.Count;term_tables=$termReads.Count;model_settings=$settings.Count;complete=$true}
    $plan.empty_reason=if(-not $sources.Count){if($Profile -ceq 'math'){'No qualifying mathematical unit head: theory main results/objects, definitions, or units with load at least two.'}else{'No eligible substantive source Card for this book.'}}else{''}
    $plan.plan_id = 'p-' + (Hash-Value $plan)
    $plan.created_at = [DateTime]::UtcNow.ToString('o')
    New-Item -ItemType Directory -Force -Path $Work, (Join-Path $Work 'jobs'), (Join-Path $Work 'out'), (Join-Path $Work 'tmp') | Out-Null
    Save-Json $planFile $plan
    Plan-Result $plan | ConvertTo-Json -Depth 20 -Compress
} catch {
    $code = if ($_.Exception -is [ArgumentException]) { 1 } else { 2 }
    $failure = [ordered]@{ok=$false;step=$Step;status='failed';errors=@($_.Exception.Message)}
    if ($Step -ceq 'precheck') { Write-Output ("BR1 " + ($_.Exception.Message -replace '[\r\n]+',' ')); Write-Output 'faults: 1'; $failure.faults=1 }
    $failure | ConvertTo-Json -Depth 8 -Compress
    exit $code
}
