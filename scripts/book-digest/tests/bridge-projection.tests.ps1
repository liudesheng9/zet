#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
# The read-only nearby projection of one selected Bridge report (S20, #112, design §10.3):
# `zt-digest bridge -Step project -Report <proposals.json> -At <address>` reuses the report's own
# id and the apply path's endpoint fingerprints, returns only current candidates for the Card,
# is bounded to twenty, writes no Card and publishes no report. The GUI's optional helper and the
# pure distant selection consume that answer; this suite exercises the compiler seam alone.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "bridge-projection-test-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
$work = Join-Path $scratch 'bridge'
New-Item -ItemType Directory -Force -Path $staging | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'; New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition, [string]$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Compile($arguments) {
    $lines = @(& $Digest bridge @arguments 2>&1 | ForEach-Object { [string]$_ })
    $script:CompilerExit = $LASTEXITCODE
    try { $result = $lines[-1] | ConvertFrom-Json -AsHashtable -DateKind String } catch { throw "compiler returned no final JSON: $($lines -join ' | ')" }
    $result
}
# The same Content-Hash the Bridge compiler uses for a report id: SHA-256 over the compact
# canonical JSON of the object without its excluded keys (ledger-publication.tests.ps1).
function Get-ContentHash($value, [string[]]$except) {
    function Sort-Keys($v) {
        if ($v -is [System.Collections.IDictionary]) {
            $sorted = [ordered]@{}
            foreach ($key in ([string[]]$v.Keys | Sort-Object -CaseSensitive)) { $sorted[$key] = Sort-Keys $v[$key] }
            return $sorted
        }
        if ($v -is [System.Collections.IEnumerable] -and $v -isnot [string]) { return , @($v | ForEach-Object { Sort-Keys $_ }) }
        return $v
    }
    $filtered = [ordered]@{}
    foreach ($key in $value.Keys) { if ($except -notcontains $key) { $filtered[$key] = $value[$key] } }
    $json = (Sort-Keys $filtered) | ConvertTo-Json -Compress -Depth 40
    $bytes = [Text.Encoding]::UTF8.GetBytes($json)
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
}
# Read a report, replace its proposals, and re-identify it exactly as the compiler does.
function Publish([string]$Source, [string]$Path, $Proposals) {
    $value = Get-Content -LiteralPath $Source -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    $value.Remove('report_id'); $value.Remove('created_at')
    $value['proposals'] = @($Proposals)
    $value['report_id'] = 'r-' + (Get-ContentHash $value @('report_id', 'created_at'))
    $value['created_at'] = '2026-10-19T00:00:00.0000000Z'
    [IO.File]::WriteAllText($Path, (ConvertTo-Json -InputObject $value -Depth 40), $utf8)
    $value['report_id']
}
function Card-Hash([string]$At) {
    $read = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $At } }))[0]
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($read.text)))
}
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $made = Write-Cards @(
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{New2026,title={New Book},year={2026}}' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'New2026'; title = '第一组'; body = '§1 控制权' } },
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Old2020,title={Old Book},year={2020}}' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Old2020'; title = '第一组'; body = '§1 控制权' } },
        # docs/literature-only-digest-design.md: Bridge sources, targets and carriers are Literature-tree
        # Cards, so the source is in the book's own tree and the target in the earlier book's tree.
        @{ tool = 'create_successor'; arguments = @{ at = 'New2026/1'; kind = 'side'; title = '控制权决定债权人的议价能力'; body = "债权人掌握控制权后决定清算。控制权决定事后议价。`n`n来源：[[New2026]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Old2020/1'; kind = 'side'; title = '控制权决定企业的议价能力'; body = "企业掌握控制权后决定投资。控制权决定事后议价。`n`n来源：[[Old2020]] §1" } }
    )
    $source = $made[4]; $target = $made[5]
    $result = Compile @('-Step', 'plan', '-Staging', $staging, '-Work', $work, '-CitationKey', 'New2026', '-MaxSources', 1, '-Candidates', 1)
    $plan = Get-Content -LiteralPath (Join-Path $work 'plan.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    $sentence = "对应 [[$target]]：两处都由控制权决定事后议价。"
    $answer = @{ schema = 1; plan_id = $plan.plan_id; job = 'br-001'; sources = @(@{ source = $source; bridges = @(
                    @{ carrier = $source; target = $target; word = '对应'; sentence = $sentence; why = '两张卡片讨论同一控制权机制。' }
                ); none = ''
            })
    }
    New-Item -ItemType Directory -Force (Join-Path $work 'out') | Out-Null
    [IO.File]::WriteAllText((Join-Path $work 'out\br-001.json'), (ConvertTo-Json -InputObject $answer -Depth 20), $utf8)
    $published = Compile @('-Step', 'report', '-Staging', $staging, '-Work', $work, '-CitationKey', 'New2026')
    $reportFile = Join-Path $work 'proposals.json'
    $reportId = $published.report_id
    Assert ($published.ok -and $published.counts.proposals -eq 1) 'the fixture publishes one exact Bridge proposal'

    # No Card, and no other report, may change because a report is projected.
    $reportHash = (Get-FileHash -LiteralPath $reportFile -Algorithm SHA256).Hash
    $beforeSource = Card-Hash $source
    $beforeTarget = Card-Hash $target

    # A current proposal answers for either endpoint: the reviewed target for the carrier, and
    # the earlier carrier for the target. Nothing is applied and no new report is published.
    $result = Compile @('-Step', 'project', '-Report', $reportFile, '-At', $source)
    Assert ($CompilerExit -eq 0 -and $result.ok -and $result.status -ceq 'done') "a selected report projects its current candidate for the carrier ($($result | ConvertTo-Json -Compress))"
    Assert ($result.report_id -ceq $reportId -and $result.citation_key -ceq 'New2026') 'the projection names the report it read'
    Assert (@($result.candidates).Count -eq 1 -and $result.candidates[0].target -ceq $target -and $result.candidates[0].relation -ceq 'corresponds' -and $result.candidates[0].section -ceq 'new' -and $result.candidates[0].focus -ceq 'carrier') 'the candidate names its exact proposal, target, relation, and direction'
    Assert ($result.candidates[0].proposal -ceq 'b001' -and $result.candidates[0].report -ceq $reportId -and $result.candidates[0].sentence -ceq $sentence) 'the projection carries the exact proposal id and sentence'
    Assert ($result.counts.at -eq 1 -and $result.counts.current -eq 1 -and $result.counts.stale -eq 0 -and $result.bounds.candidates_max -eq 20) 'the counts and the fixed twenty-candidate bound are reported'
    $reverse = Compile @('-Step', 'project', '-Report', $reportFile, '-At', $target)
    Assert ($reverse.status -ceq 'done' -and $reverse.candidates[0].target -ceq $source -and $reverse.candidates[0].focus -ceq 'target') 'the other endpoint of a current proposal answers too, naming the direction'
    $other = Compile @('-Step', 'project', '-Report', $reportFile, '-At', 'New2026/1')
    Assert ($CompilerExit -eq 0 -and $other.ok -and $other.status -ceq 'not_in_report' -and @($other.candidates).Count -eq 0) 'a Card the report does not name has no candidate and says so'
    Assert ((Get-FileHash -LiteralPath $reportFile -Algorithm SHA256).Hash -ceq $reportHash -and (Card-Hash $source) -ceq $beforeSource -and (Card-Hash $target) -ceq $beforeTarget) 'projecting writes no Card and publishes no report'

    # An earlier-carrier proposal is projected the same way: the earlier card is the carrier and
    # the new book's card is its target.
    $publishedJson = Get-Content -LiteralPath $reportFile -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    $earlier = @($publishedJson.proposals)[0].Clone()
    $earlier['id'] = 'b002'; $earlier['section'] = 'earlier'; $earlier['carrier'] = $target; $earlier['target'] = $source
    $earlierFile = Join-Path $scratch 'earlier-report.json'
    $earlierId = Publish $reportFile $earlierFile @($earlier)
    $result = Compile @('-Step', 'project', '-Report', $earlierFile, '-At', $target)
    Assert ($result.status -ceq 'done' -and $result.report_id -ceq $earlierId -and $result.candidates[0].target -ceq $source -and $result.candidates[0].section -ceq 'earlier') 'an earlier-carrier proposal is projected with its section and its recalculated report id'

    # The bound is twenty candidates even when the report names more.
    $many = @()
    for ($i = 1; $i -le 25; $i++) {
        $copy = @($publishedJson.proposals)[0].Clone()
        $copy['id'] = 'b{0:d3}' -f (100 + $i)
        $many += $copy
    }
    $manyFile = Join-Path $scratch 'many-report.json'
    $manyId = Publish $reportFile $manyFile $many
    $result = Compile @('-Step', 'project', '-Report', $manyFile, '-At', $source)
    Assert ($result.status -ceq 'done' -and @($result.candidates).Count -eq 20 -and $result.counts.at -eq 25 -and $result.counts.omitted -eq 5) 'the projection returns at most twenty candidates and reports the omission'
    Assert ($result.candidates[0].proposal -ceq 'b101' -and $result.candidates[19].proposal -ceq 'b120') 'the bounded candidates keep the report order and report the bound'

    # A path with shell metacharacters is one argument: it is read, never interpreted.
    $odd = Join-Path $scratch 'odd & del; report.json'
    Copy-Item -LiteralPath $reportFile -Destination $odd
    $result = Compile @('-Step', 'project', '-Report', $odd, '-At', $source)
    Assert ($CompilerExit -eq 0 -and $result.status -ceq 'done' -and $result.candidates[0].target -ceq $target) 'a report path with shell metacharacters is read as one argument'
    $oddAddress = Compile @('-Step', 'project', '-Report', $reportFile, '-At', 'x; Remove-Item -Recurse C:\ -Force')
    Assert ($CompilerExit -eq 0 -and $oddAddress.status -ceq 'not_in_report') 'an address with shell metacharacters is an address, never a command'

    # A changed endpoint is stale evidence: the projection says why, and answers again after the
    # Card is restored.
    Write-Cards @(@{ tool = 'edit_card'; arguments = @{ at = $target; title = '控制权改变了企业边界' } }) | Out-Null
    $result = Compile @('-Step', 'project', '-Report', $reportFile, '-At', $source)
    Assert ($CompilerExit -eq 0 -and $result.status -ceq 'stale' -and @($result.candidates).Count -eq 0) 'a changed endpoint leaves no current candidate'
    Assert ($result.stale[0].proposal -ceq 'b001' -and $result.stale[0].why -match 'changed endpoint .* title') "the stale entry names the changed field ($($result.stale | ConvertTo-Json -Compress))"
    Write-Cards @(@{ tool = 'edit_card'; arguments = @{ at = $target; title = '控制权决定企业的议价能力' } }) | Out-Null
    $result = Compile @('-Step', 'project', '-Report', $reportFile, '-At', $source)
    Assert ($result.status -ceq 'done' -and $result.candidates[0].target -ceq $target) 'a restored endpoint is current again on the next projection'

    # An endpoint the archive no longer holds is a missing endpoint, not a failed projection: the
    # report remains readable and the other endpoint alone explains why nothing is current.
    Write-Cards @(@{ tool = 'delete_card'; arguments = @{ at = $target; confirm = 'delete' } }) | Out-Null
    $result = Compile @('-Step', 'project', '-Report', $reportFile, '-At', $source)
    Assert ($CompilerExit -eq 0 -and $result.status -ceq 'stale' -and @($result.candidates).Count -eq 0) 'a deleted endpoint leaves no current candidate'
    Assert ($result.stale[0].why -match "missing endpoint $([regex]::Escape($target))") "the stale entry names the missing endpoint ($($result.stale | ConvertTo-Json -Compress))"

    # The report's own id and schema are authoritative: an edited or missing report writes
    # nothing and is a named input refusal.
    $tampered = Get-Content -LiteralPath $reportFile -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    @($tampered.proposals)[0]['sentence'] = '对应 [[Old2020/1|a]]：被改过的一句话。'
    $tamperedFile = Join-Path $scratch 'tampered-report.json'
    [IO.File]::WriteAllText($tamperedFile, (ConvertTo-Json -InputObject $tampered -Depth 40), $utf8)
    $result = Compile @('-Step', 'project', '-Report', $tamperedFile, '-At', $source)
    Assert ($CompilerExit -eq 1 -and -not $result.ok -and $result.status -ceq 'invalid_report' -and $result.errors[0] -match 'fingerprint') "an edited report is refused as an invalid report ($($result | ConvertTo-Json -Compress))"
    $result = Compile @('-Step', 'project', '-Report', (Join-Path $scratch 'missing.json'), '-At', $source)
    Assert ($CompilerExit -eq 1 -and -not $result.ok -and $result.status -ceq 'invalid_report') 'a missing report path is a named input refusal'
    $result = Compile @('-Step', 'project', '-Report', $reportFile, '-At', ('x' * 513))
    Assert ($CompilerExit -eq 1 -and -not $result.ok -and $result.status -ceq 'invalid_report') 'an over-long address is refused before any read'
    $result = Compile @('-Step', 'project', '-Report', $reportFile)
    Assert ($CompilerExit -ne 0 -and -not $result.ok) 'a projection without its Card is refused'
    Assert ((Get-FileHash -LiteralPath $reportFile -Algorithm SHA256).Hash -ceq $reportHash -and (Card-Hash $source) -ceq $beforeSource) 'the refused projections wrote and published nothing'
} finally {
    try { zt down | Out-Null } catch { }
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
