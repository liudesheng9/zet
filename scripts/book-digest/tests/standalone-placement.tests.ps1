#Requires -Version 7.5
# S06 (#103): a standalone claim filed beside earlier thought, with the kinded reason in its first
# sentence, and a book-bound claim kept in the Literature tree.
#
# Three seams are exercised on a scratch archive. First the writer's helper (`zt-digest call -Beneath`):
# a Side successor beneath an earlier agent Topic Card that the group read in full is permitted, a
# Direct successor there, an edit there and any unlisted parent are refused, a human Card is refused,
# and the earlier Card's authored text never changes. Second the prompt the frozen workflows assemble:
# `prepare` builds shared-prefix.md from `dsh-digest-base.zh.md` plus the book type's files, so that
# assembled prompt must carry the ordered filing decision and the inline-Link rule with no fixed
# quota. Third the group (docs/literature-only-digest-design.md): `group-start` publishes the
# place-beside file, which lists no Topic Card, `group-finish` refuses a draft that files a Card
# beside the earlier Topic Card, and a draft that files its Card under its own anchor completes
# through the receipt-owned path, replays without a second Card, and leaves every older authored
# body byte-identical.
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
    $humanText = (Card $humanAt).body
    Assert $humanAt 'a human Topic exists to be refused as a destination'

    # --- the writer's helper: -Beneath permits one Side successor and nothing else ----------------------
    $allow = Join-Path $scratch 'allow.txt'; [IO.File]::WriteAllText($allow, '', $utf8)
    $beside = Join-Path $scratch 'place-beside.txt'; [IO.File]::WriteAllText($beside, "$earlier`n", $utf8)
    $log = Join-Path $scratch 'owned.txt'
    $placed = @(& $Digest call -Open - -Allow $allow -Beneath $beside -Log $log 2>&1) ; $code = $LASTEXITCODE
    Assert ($code -ne 0) 'an empty call file is an error, not a silent no-op'
    $callsFile = Join-Path $scratch 'calls.json'
    $claim = '本文的学习外部性放宽 [[{0}]] 的固定技术假设：把技术水平写成可积累的。' -f $earlier
    $calls = @(@{ tool='create_successor'; arguments=@{ at=$earlier; kind='side'; title='学习外部性让技术可以积累'; body=$claim } })
    [IO.File]::WriteAllText($callsFile, (ConvertTo-Json -InputObject $calls -Depth 10), $utf8)
    $lines = @(& $Digest call -Open $callsFile -Allow $allow -Beneath $beside -Log $log 2>&1)
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
    $direct = Try-Digest @('call','-Open',$directFile,'-Allow',$allow,'-Beneath',$beside,'-Log',$log)
    Assert ($direct.code -eq 1 -and $direct.text.Contains('only as a Side successor')) 'a Direct successor beneath an earlier Card is refused: its chain belongs to its own line of thought'

    $editFile = Join-Path $scratch 'edit.json'
    [IO.File]::WriteAllText($editFile, (ConvertTo-Json -InputObject @(@{ tool='edit_card'; arguments=@{ at=$earlier; title='改写早前卡'; body='改写别人的卡。' } }) -Depth 10), $utf8)
    $edit = Try-Digest @('call','-Open',$editFile,'-Allow',$allow,'-Beneath',$beside,'-Log',$log)
    Assert ($edit.code -eq 1 -and $edit.text.Contains('is not one of your Cards')) 'an earlier Card stays unwritable even when it may be continued'

    $unlisted = Join-Path $scratch 'unlisted.json'
    [IO.File]::WriteAllText($unlisted, (ConvertTo-Json -InputObject @(@{ tool='create_successor'; arguments=@{ at=$earlierSide; kind='side'; title='未读过的卡'; body='没有读全的卡不能接续。' } }) -Depth 10), $utf8)
    $refused = Try-Digest @('call','-Open',$unlisted,'-Allow',$allow,'-Beneath',$beside,'-Log',$log)
    Assert ($refused.code -eq 1 -and $refused.text.Contains('is not one of your Cards')) 'a Card that is not on the place-beside list is refused'

    $humanFile = Join-Path $scratch 'human.json'
    [IO.File]::WriteAllText($humanFile, (ConvertTo-Json -InputObject @(@{ tool='create_successor'; arguments=@{ at=$humanAt; kind='side'; title='放进人类树'; body='把代理卡放进人类树。' } }) -Depth 10), $utf8)
    [IO.File]::WriteAllText($beside, "$earlier`n$humanAt`n", $utf8)
    $humanTry = Try-Digest @('call','-Open',$humanFile,'-Allow',$allow,'-Beneath',$beside,'-Log',$log)
    Assert ($humanTry.code -eq 1 -and $humanTry.text.Contains('human')) 'an agent Card is never filed in a human tree, even when the Card is listed'

    # --- the group: the pack publishes the place-beside file, and the draft is receipt-owned -----------
    $source = Join-Path $scratch 'book.md'; $options = Join-Path $scratch 'options.json'
    [IO.File]::WriteAllText($source, "# 导论`n`n剩余控制权决定谈判地位。所有者取得事后谈判优势。`n`n# 参考`n未选择。", $utf8)
    [IO.File]::WriteAllText($options, '{"terms":false}', $utf8)
    $manifest = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','LaterBook','-BibTeX','@book{LaterBook,title={Later book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Instructions','dsh-digest-base.zh.md,dsh-model-cards.zh.md','-Apply')
    # The frozen econ/math workflows send `dsh-digest-base.zh.md` plus their book type's files; the
    # writer reads the assembled shared-prefix.md. The S06 filing decision has to reach that prompt,
    # not only the standalone `dsh-instructions.zh.md` the workflows no longer load.
    $prefixText = [IO.File]::ReadAllText((Join-Path $out 'shared-prefix.md'))
    Assert ($prefixText.Contains('直接后继') -and $prefixText.Contains('锚点的侧后继')) 'the live digest prompt states the reading thread and the anchor placement'
    # docs/literature-only-digest-design.md: a digest files every Card in its book's Literature tree, so
    # the prompt offers no earlier Topic Card to continue and names no -Beneath call.
    Assert (-not $prefixText.Contains('可续写的早前主题卡') -and -not $prefixText.Contains('-Beneath')) 'the live digest prompt offers no place beside an earlier Topic Card'
    Assert ($prefixText.Contains('没有固定的 Link 条数') -and -not $prefixText.Contains('至少一半的卡片链接全书') -and -not $prefixText.Contains('父卡以外')) 'the live digest prompt writes Links in place with no fixed quota'
    $next = Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','group-manager')
    $unit = @($next.units)[0]
    $started = Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($unit.token)")
    $listed = @(Get-Content -LiteralPath $started.job | ConvertFrom-Json -AsHashtable).artifacts.place_beside.path
    Assert ($listed -and (Test-Path -LiteralPath $listed)) 'group-start publishes the place-beside file as job evidence'
    $besideLines = @(Get-Content -LiteralPath $listed | Where-Object { $_.Trim() })
    Assert (-not ($besideLines -contains $earlier) -and -not ($besideLines -contains $earlierTopic)) "no earlier Topic Card is offered for continuation ($($besideLines -join ', '))"

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
    # The group applies its draft through group-finish, which refuses any call the Literature write
    # scope refuses: a Card filed beside the earlier Topic Card is a Topic-tree write.
    [IO.File]::WriteAllText($draftPath, ($draft | ConvertTo-Json -Depth 10), $utf8)
    $topicGroup = Try-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($unit.token)",'-Draft',$draftPath,'-CheckOnly')
    Assert ($topicGroup.code -eq 1 -and $topicGroup.text.Contains("Literature tree") -and $topicGroup.text.Contains($earlier)) "the group refuses a Card beside the earlier Topic Card ($($topicGroup.text))"
    $draft.calls = @($draft.calls[0], $draft.calls[2])
    [IO.File]::WriteAllText($draftPath, ($draft | ConvertTo-Json -Depth 10), $utf8)
    # The writer's SELF-CHECK prints the checker's fault lines and `faults: N`, like every other
    # read-only draft precheck (S28, #186): the writer reads a count, never a JSON shape.
    $check = @(& $Digest group-finish -Out $out -Group 1 -Token $unit.token -Draft $draftPath -CheckOnly 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and @($check)[-1] -eq 'faults: 0' -and -not (@($check) -match '^\{')) "the group preview accepts the Card under its anchor and prints faults: 0 ($(@($check) -join ' | '))"
    $finished = Run-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($unit.token)",'-Draft',$draftPath)
    Assert (@($finished.owned | Where-Object { $_.StartsWith("$earlier|") }).Count -eq 0 -and @($finished.owned) -contains $anchor -and @($finished.owned | Where-Object { $_.StartsWith("$anchor|") }).Count -eq 1) "the group owns its anchor and one Card under it ($(($finished.owned) -join ', '))"
    Assert (@($finished.receipt_keys).Count -eq 2) 'the creation and the anchor edit have receipts'
    $again = Run-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($unit.token)",'-Draft',$draftPath)
    Assert ($again.reused -eq $true -and (@($again.owned) -join ',') -eq (@($finished.owned) -join ',')) 'a receipt-owned replay returns the same Cards and creates none'
    Assert (((Card $earlier).body -ceq $earlierText) -and ((Card $earlierSide).body -ceq $earlierSideText)) 'the earlier thought and its chain keep their authored bodies after the group'
    Assert ((Card $humanAt).body -ceq $humanText) 'the human Card keeps its authored text'
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_TRACE_TOOLS -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
}
Write-Output 'all passed'
