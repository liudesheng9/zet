#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\install-smoke.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# The installation smoke test of issue #90 (release qualification section 8): the installed set answers
# one `zt mcp` read, one identified batch, one Card read-back, one checker run and one Link-index
# comparison on a scratch archive. It finds a broken MCP, helper, receipt or checker boundary before a
# real digest run starts on the live archive. Every call is a real process: no seam is used.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "install-smoke-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the intended temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
$ztExe = (Get-Command zt -CommandType Application | Select-Object -First 1).Source
$checkExe = (Get-Command zt-check -CommandType Application | Select-Object -First 1).Source
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }

# One real `zt mcp` stdio session: the handshake, then a single tool call. The reply's text is
# returned as it arrived, so a caller parses exactly what a workflow's helper would receive.
function Invoke-RawMcp([string]$exe, [string]$tool, $arguments) {
    $psi = [Diagnostics.ProcessStartInfo]::new($exe, 'mcp')
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.StandardInputEncoding = [Text.UTF8Encoding]::new($false)
    $psi.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $process = [Diagnostics.Process]::Start($psi)
    try {
        # stderr is not read; a full stderr pipe must not block the server.
        $process.BeginErrorReadLine()
        $process.StandardInput.WriteLine((@{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = @{ protocolVersion = '2025-06-18' } } | ConvertTo-Json -Compress -Depth 8))
        $process.StandardInput.Flush()
        $handshake = $false
        while ($null -ne ($line = $process.StandardOutput.ReadLine())) {
            if (($line | ConvertFrom-Json -AsHashtable).id -eq 1) { $handshake = $true; break }
        }
        if (-not $handshake) { throw 'zt mcp did not answer the initialize request' }
        $process.StandardInput.WriteLine((@{ jsonrpc = '2.0'; method = 'notifications/initialized' } | ConvertTo-Json -Compress))
        $process.StandardInput.WriteLine((@{ jsonrpc = '2.0'; id = 2; method = 'tools/call'; params = @{ name = $tool; arguments = $arguments } } | ConvertTo-Json -Compress -Depth 8))
        $process.StandardInput.Flush()
        while ($null -ne ($line = $process.StandardOutput.ReadLine())) {
            $reply = $line | ConvertFrom-Json -AsHashtable -DateKind String
            if ($reply.id -ne 2) { continue }
            $text = ($reply.result.content | ForEach-Object { $_.text }) -join "`n"
            if ($reply.error -or $reply.result.isError) { throw "zt mcp $tool failed: $($text -replace '\s+', ' ')" }
            return $text
        }
        throw "zt mcp closed before answering $tool"
    } finally {
        try { $process.StandardInput.Close(); if (-not $process.WaitForExit(3000)) { $process.Kill() } } catch { }
    }
}
function EmptyRead([string]$exe) { (Invoke-RawMcp $exe 'get_cards' @{ addresses = @(); view = 'text' }) | ConvertFrom-Json -DateKind String }

try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $version = @(zt version)[0]
    Assert ($version -match '^zt \d+\.\d+\.\d+$') "the installed zt answers with its version ($version)"
    $digestVersion = @(& $Digest version)[0]
    Assert ($digestVersion -match '^zt-digest \d+\.\d+\.\d+$') "the installed zt-digest answers with its version ($digestVersion)"

    # 1. The read boundary: one `zt mcp` process, `get_cards` on an empty address list.
    $empty = EmptyRead $ztExe
    Assert ($empty.complete -eq $true -and @($empty.cards).Count -eq 0 -and $empty.missing.Count -eq 0 -and $empty.archive_revision -eq 0) 'zt mcp answers a complete empty read at revision 0'

    # 2. The identified-batch boundary: the public helper chunks, commits and verifies its outputs.
    #    The last Card names an earlier result of the same batch, so one batch also proves that a
    #    local reference resolves to the committed address of the call it names.
    $callsFile = Join-Path $scratch 'install-smoke-calls.json'
    $calls = @(
        @{ tool = 'create_topic'; arguments = @{ title = '安装冒烟检查'; body = '这一主题只检验安装后的写入边界。正文由安装后的工具写入，不进入真实的书稿，也不进入任何真实的摘要运行。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '标识批次写出的第一张卡片'; body = '这张卡片检验标识批次和它的提交凭据。同一批次重新运行时地址保持不变，档案不增加新的修订。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@1'; kind = 'side'; title = '标识批次写出的第二张卡片'; body = '这张卡片检验同一批次内解析的本地引用。它依赖 [[@@0]]，因为那一张卡片给出了这次安装检查的起点。批次重复执行时地址保持不变。' } }
    )
    [IO.File]::WriteAllText($callsFile, ($calls | ConvertTo-Json -Depth 8 -AsArray), $utf8)
    $prefix = 'install-smoke/scratch/1/build'
    $first = @(& $Digest call $callsFile -OperationPrefix $prefix 2>&1 | ForEach-Object { [string]$_ })
    $okLines = @($first | Where-Object { $_ -match '^\d+ ok \S+$' })
    Assert ($LASTEXITCODE -eq 0 -and $okLines.Count -eq 3 -and @($okLines | ForEach-Object { ($_ -split ' ok ', 2)[1] } | Sort-Object -Unique).Count -eq 3) "zt-digest call commits the identified batch through zt mcp ($($first -join ' | '))"
    $committed = @($okLines | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
    $replay = @(& $Digest call $callsFile -OperationPrefix $prefix 2>&1 | ForEach-Object { [string]$_ })
    Assert ($LASTEXITCODE -eq 0 -and ($replay -join "`n") -eq ($first -join "`n")) 'a replay of the same accepted batch returns the original addresses and no new revision'
    $after = EmptyRead $ztExe
    Assert ($after.archive_revision -eq 1) "the batch publishes exactly one archive revision (revision $($after.archive_revision))"

    # 3. The read-back: the archive holds the authored text with its local reference resolved.
    if ($committed.Count -eq 3) {
        $expectedBody = $calls[2].arguments.body.Replace('@@0', $committed[0])
        $card = (Invoke-RawMcp $ztExe 'get_card' @{ at = $committed[2] }) | ConvertFrom-Json -DateKind String
        Assert ($card.body -eq $expectedBody -and $card.title -eq $calls[2].arguments.title) "zt mcp reads the committed Card back byte for byte with [[$($committed[0])]] resolved ($($committed[2]))"

        # 4. The checker boundary: zt-check reads the live Card and reports its fault count.
        $check = @(& $checkExe --cards $committed[2] 2>&1 | ForEach-Object { [string]$_ })
        $faultLines = @($check | Where-Object { $_ -match '^faults: \d+$' })
        Assert ($faultLines.Count -eq 1 -and $faultLines[0] -eq 'faults: 0') "zt-check runs on the written Card and reports no fault ($($check -join ' | '))"

        # 5. The derived Link index still matches the full parser after the batch.
        $links = (zt status --check-links 2>&1 | Out-String) | ConvertFrom-Json
        Assert ($links.ok -eq $true -and @($links.faults).Count -eq 0 -and $links.cards -eq 3) "zt status --check-links reports the batch's three Cards and no fault ($($links.cards) Cards)"
    } else {
        Assert $false "the batch committed $($committed.Count) of 3 addresses, so the read-back, the checker and the Link-index comparisons did not run"
    }
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
