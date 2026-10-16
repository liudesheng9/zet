#Requires -Version 7.5
# External transport fixture: a named tree, or a target after a confirmed edit, is unavailable.
$ErrorActionPreference='Stop'
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$inputJson=[Console]::In.ReadToEnd()
$calls=@($inputJson|ConvertFrom-Json -AsHashtable -DateKind String)
if(@($calls|Where-Object{$_.tool -ceq 'get_tree' -and $_.arguments.tree -ceq $env:ZT_FAIL_TREE}).Count){Write-Output '0 error simulated unavailable tree';exit 1}
if($env:ZT_FAIL_FLAG -and (Test-Path -LiteralPath $env:ZT_FAIL_FLAG) -and @($calls|Where-Object{$_.tool -ceq 'get_card' -and $_.arguments.at -ceq $env:ZT_FAIL_CARD}).Count){Write-Output '0 error simulated unavailable target after persistence';exit 1}
$inputJson|pwsh -NoProfile -File $env:ZT_REAL_HELPER @args
$code=$LASTEXITCODE
if(-not $code -and $env:ZT_FAIL_AFTER_EDIT -and @($calls|Where-Object{$_.tool -ceq 'edit_card' -and $_.arguments.at -ceq $env:ZT_FAIL_AFTER_EDIT}).Count){New-Item -ItemType File -Force -Path $env:ZT_FAIL_FLAG|Out-Null}
exit $code
