#Requires -Version 7
# A test stand-in for skills\zt\scripts\zt-mcp.ps1: it forwards every run to the real helper ($env:ZT_REAL_HELPER).
# Once, before the first run that edits the Card $env:ZT_RACE_TARGET, it appends $env:ZT_RACE_LINE to that Card
# itself, as a parallel digest stage would between a compiler's read and its write. $env:ZT_RACE_FLAG marks it done.
# The compiler tests copy it into a copy of the scripts tree, where zt-calls.ps1 finds it in place of the helper.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
$in = [Console]::In.ReadToEnd()
$real = $env:ZT_REAL_HELPER
$target = $env:ZT_RACE_TARGET
if ($target -and -not (Test-Path -LiteralPath $env:ZT_RACE_FLAG)) {
    $calls = @($in | ConvertFrom-Json -AsHashtable)
    if (@($calls | Where-Object { $_.tool -eq 'edit_card' -and $_.arguments.at -eq $target }).Count) {
        New-Item -ItemType File -Path $env:ZT_RACE_FLAG | Out-Null
        $read = @((ConvertTo-Json -InputObject @(@{ tool = 'get_card'; arguments = @{ at = $target } }) -Depth 5) | pwsh -NoProfile -File $real -)
        $card = ([string]$read[0] -replace '^0 ', '') | ConvertFrom-Json
        $edit = ConvertTo-Json -InputObject @(@{ tool = 'edit_card'; arguments = @{ at = $target; body = ($card.body.TrimEnd() + "`n`n" + $env:ZT_RACE_LINE) } }) -Depth 5
        $null = $edit | pwsh -NoProfile -File $real -
    }
}
$in | pwsh -NoProfile -File $real @args
exit $LASTEXITCODE
