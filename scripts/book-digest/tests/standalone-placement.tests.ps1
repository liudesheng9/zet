#Requires -Version 7.5
# S06 (#103): a standalone claim filed beside earlier thought, with the kinded reason in its first
# sentence, and a book-bound claim kept in the Literature tree.
#
# Two seams are exercised on a scratch archive. First the writer's helper (`zt-digest call -Beneath`):
# a Side successor beneath an earlier agent Topic Card that the group read in full is permitted, a
# Direct successor there, an edit there and any unlisted parent are refused, a human Card is refused,
# and the earlier Card's authored text never changes. Second the group: `group-start` publishes the
# place-beside file from its earlier-context pack, and a draft that files one Card beside the earlier
# thought and one under its own anchor completes through the receipt-owned path, replays without a
# second Card, and leaves every older authored body byte-identical.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "standalone-placement-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST=$Digest; $env:ZT_CONFIG_DIR=Join-Path $scratch 'config'; $env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$out = Join-Path $scratch 'prepared'
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "digest failed: $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Try-Digest([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    return [pscustomobject]@{ code=$LASTEXITCODE; text=($lines -join ' ') }
}
function Titles { @(Invoke-Zt @(@{ tool='export_titles'; arguments=@{} })).titles }
function Card([string]$at) { @(Invoke-Zt @(@{ tool='get_card'; arguments=@{ at=$at } }))[0] }
function Gui {
    $url = (& zt gui) -split "`n" | Select-Object -First 1
    $port = [int]([regex]::Match($url,'127\.0\.0\.1:(\d+)').Groups[1].Value)
    $page = (Invoke-WebRequest -Uri "http://127.0.0.1:$port/" -UseBasicParsing).Content
    $token = [regex]::Match($page,'name="zt-token" content="([0-9a-f]{32})"').Groups[1].Value
    @{ port=$port; token=$token }
}
function Gui-Create($gui, $body) {
    Invoke-RestMethod -Uri "http://127.0.0.1:$($gui.port)/api/create" -Method Post -Headers @{ 'X-ZT-Token'=$gui.token } -ContentType 'application/json' -Body ($body | ConvertTo-Json -Compress)
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null

    # --- the earlier thought: an agent Topic with one Regular Card and its own continuation -------------
    $null = Invoke-Zt @(@{ tool='create_topic'; arguments=@{ title='剩余控制权的谈判地位'; body='什么决定谈判中的地位？剩余控制权与所有权的差别在哪里？' } })
    $earlierTopic = (Titles | Where-Object { $_.title -eq '剩余控制权的谈判地位' } | Select-Object -First 1).address
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$earlierTopic; kind='direct'; title='剩余控制权决定谈判地位'; body='剩余控制权决定谈判地位。所有权人借此取得事后谈判优势。' } })
    $earlier = (Card $earlierTopic).direct.address
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$earlier; kind='side'; title='谈判地位也来自信息优势'; body='谈判地位也来自信息优势，不只来自所有权。' } })
    $earlierSide = (Card $earlier).sides[0].address
    $earlierText = (Card $earlier).body
    $earlierSideText = (Card $earlierSide).body
    Assert ($earlier -and $earlierSide) "the earlier thought exists at $earlier with its side successor"

    # --- a human Card, which may be a Link target but never a place to file an agent Card ---------------
    $gui = Gui
    $human = Gui-Create $gui @{ kind='topic'; workspace='human'; text="# 人类的工作台`n`n只供人写的地方。`n" }
    $humanAt = $human.address
    Assert $humanAt 'a human Topic exists to be refused as a destination'

    # --- the writer's helper: -Beneath permits one Side successor and nothing else ----------------------
    $allow = Join-Path $scratch 'allow.txt'; [IO.File]::WriteAllText($allow, '', $utf8)
    $beside = Join-Path $scratch 'place-beside.txt'; [IO.File]::WriteAllText($beside, "$earlier`n", $utf8)
    $log = Join-Path $scratch 'owned.txt'
    $placed = @(& $Digest call - -Allow $allow -Beneath $beside -Log $log 2>&1) ; $code = $LASTEXITCODE
    Assert ($code -ne 0) 'an empty call file is an error, not a silent no-op'
    $callsFile = Join-Path $scratch 'calls.json'
    $claim = '本文的学习外部性放宽 [[{0}]] 的固定技术假设：把技术水平写成可积累的。' -f $earlier
    $calls = @(@{ tool='create_successor'; arguments=@{ at=$earlier; kind='side'; title='学习外部性让技术可以积累'; body=$claim } })
    [IO.File]::WriteAllText($callsFile, (ConvertTo-Json -InputObject $calls -Depth 10), $utf8)
    $lines = @(& $Digest call $callsFile -Allow $allow -Beneath $beside -Log $log 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($lines -join ' ') -match '0 ok ') 'the helper files a Side successor beneath an earlier agent Topic Card'
    $created = (Get-Content -LiteralPath $log | Select-Object -Last 1)
    $newCard = Card $created
    Assert ($created.startsWith("$earlier|") -and $newCard.title -eq '学习外部性让技术可以积累') "the new claim is the earlier Card's Side successor ($created)"
    Assert (((Card $earlier).body -ceq $earlierText) -and ((Card $earlierSide).body -ceq $earlierSideText)) 'filing beside an earlier Card changes no earlier authored text'
    Assert ((Card $earlier).text.Contains($created)) 'the earlier Card gains only the generated Reverse link to the new claim'
    $body = (Card $created).body
    Assert ($body.TrimStart() -match ('^' + [regex]::Escape('本文的学习外部性放宽 ') + '\[\[' + [regex]::Escape($earlier) + '\]\]')) 'the first sentence states the relation to the earlier Card'
    Assert ($body -match '固定技术假设' -and $body -match '可积累') 'the claim states its own conditions, not only a pointer'

    $directFile = Join-Path $scratch 'direct.json'
    [IO.File]::WriteAllText($directFile, (ConvertTo-Json -InputObject @(@{ tool='create_successor'; arguments=@{ at=$earlier; kind='direct'; title='直接接上'; body='把新卡接成直接后继。' } }) -Depth 10), $utf8)
    $direct = Try-Digest @('call',$directFile,'-Allow',$allow,'-Beneath',$beside,'-Log',$log)
    Assert ($direct.code -eq 1 -and $direct.text.Contains('only as a Side successor')) 'a Direct successor beneath an earlier Card is refused: its chain belongs to its own line of thought'

    $editFile = Join-Path $scratch 'edit.json'
    [IO.File]::WriteAllText($editFile, (ConvertTo-Json -InputObject @(@{ tool='edit_card'; arguments=@{ at=$earlier; title='改写早前卡'; body='改写别人的卡。' } }) -Depth 10), $utf8)
    $edit = Try-Digest @('call',$editFile,'-Allow',$allow,'-Beneath',$beside,'-Log',$log)
    Assert ($edit.code -eq 1 -and $edit.text.Contains('is not one of your Cards')) 'an earlier Card stays unwritable even when it may be continued'

    $unlisted = Join-Path $scratch 'unlisted.json'
    [IO.File]::WriteAllText($unlisted, (ConvertTo-Json -InputObject @(@{ tool='create_successor'; arguments=@{ at=$earlierSide; kind='side'; title='未读过的卡'; body='没有读全的卡不能接续。' } }) -Depth 10), $utf8)
    $refused = Try-Digest @('call',$unlisted,'-Allow',$allow,'-Beneath',$beside,'-Log',$log)
    Assert ($refused.code -eq 1 -and $refused.text.Contains('is not one of your Cards')) 'a Card that is not on the place-beside list is refused'

    $humanFile = Join-Path $scratch 'human.json'
    [IO.File]::WriteAllText($humanFile, (ConvertTo-Json -InputObject @(@{ tool='create_successor'; arguments=@{ at=$humanAt; kind='side'; title='放进人类树'; body='把代理卡放进人类树。' } }) -Depth 10), $utf8)
    [IO.File]::WriteAllText($beside, "$earlier`n$humanAt`n", $utf8)
    $humanTry = Try-Digest @('call',$humanFile,'-Allow',$allow,'-Beneath',$beside,'-Log',$log)
    Assert ($humanTry.code -eq 1 -and $humanTry.text.Contains('human')) 'an agent Card is never filed in a human tree, even when the Card is listed'

    # --- the group: the pack publishes the place-beside file, and the draft is receipt-owned -----------
    $source = Join-Path $scratch 'book.md'; $options = Join-Path $scratch 'options.json'
    [IO.File]::WriteAllText($source, "# 导论`n`n剩余控制权决定谈判地位。所有者取得事后谈判优势。`n`n# 参考`n未选择。", $utf8)
    [IO.File]::WriteAllText($options, '{"terms":false}', $utf8)
    $manifest = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','LaterBook','-BibTeX','@book{LaterBook,title={Later book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    $next = Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','group-manager')
    $unit = @($next.units)[0]
    $started = Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($unit.token)")
    $besideFile = [string]$started.place_beside
    $listed = @(Get-Content -LiteralPath $started.job | ConvertFrom-Json -AsHashtable).artifacts.place_beside.path
    Assert ($listed -and (Test-Path -LiteralPath $listed)) 'group-start publishes the place-beside file as job evidence'
    $besideLines = @(Get-Content -LiteralPath $listed | Where-Object { $_.Trim() })
    Assert ($besideLines -contains $earlier) "the earlier Card the pack read is offered for continuation ($($besideLines -join ', '))"
    Assert (-not ($besideLines -contains $earlierTopic)) 'a Topic root is never offered as a place to continue'

    $anchor = "LaterBook/1"
    $anchorFingerprint = ([string]$started.job | ForEach-Object { (Get-Content -LiteralPath $_ -Raw | ConvertFrom-Json -AsHashtable).inputs.anchor.fingerprint })
    $draftPath = Join-Path ([IO.Path]::GetDirectoryName($started.job)) 'draft.json'
    $draft = @{
        schema_version = 1
        unit = $unit.id
        token = $unit.token
        input_hash = $started.input_hash
        disposition = 'cards'
        calls = @(
            @{ tool='create_successor'; arguments=@{ at=$anchor; kind='side'; title='本组对剩余控制权的界定'; body='本组把剩余控制权写成对资产剩余收益的控制。来源：[[LaterBook]] §1。' } },
            @{ tool='create_successor'; arguments=@{ at=$earlier; kind='side'; title='剩余控制权也约束经理'; body=('对照 [[{0}]]：剩余控制权也约束经理，因为所有者能撤换经理。这个效果要求经理的任期短于所有者。来源：[[LaterBook]] §1。' -f $earlier) } },
            @{ tool='edit_card'; arguments=@{ at=$anchor; title='剩余控制权界定导论'; body='本组界定剩余控制权。'; base_fingerprint=$anchorFingerprint } }
        )
    }
    [IO.File]::WriteAllText($draftPath, ($draft | ConvertTo-Json -Depth 10), $utf8)
    $check = Run-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($unit.token)",'-Draft',$draftPath,'-CheckOnly')
    Assert ($check.preview -eq $true -or $check.check_only -eq $true) 'the group preview accepts one Card under its anchor and one beside the earlier Topic Card'
    $finished = Run-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($unit.token)",'-Draft',$draftPath)
    $placed = @($finished.owned | Where-Object { $_.StartsWith("$earlier|") })
    Assert ($placed.Count -eq 1 -and @($finished.owned) -contains $anchor -and @($finished.owned | Where-Object { $_.StartsWith("$anchor|") }).Count -eq 1) "the group owns one Card under its anchor and one beside earlier thought ($(($finished.owned) -join ', '))"
    Assert (@($finished.substantive) -contains $placed[0] -and @($finished.receipt_keys).Count -eq 3) 'the placed Card is substantive and its creation has a receipt'
    $again = Run-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($unit.token)",'-Draft',$draftPath)
    Assert ($again.reused -eq $true -and (@($again.owned) -join ',') -eq (@($finished.owned) -join ',')) 'a receipt-owned replay returns the same Cards and creates none'
    Assert (((Card $earlier).body -ceq $earlierText) -and ((Card $earlierSide).body -ceq $earlierSideText)) 'the earlier thought and its chain keep their authored bodies after the group'
    $placedCard = Card $placed[0]
    Assert ($placedCard.body.TrimStart().StartsWith('对照 [[') -and $placedCard.body.Contains($earlier)) 'the placed Card states its relation to the earlier Card in its first sentence'
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_TRACE_TOOLS -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
}
Write-Output 'all passed'