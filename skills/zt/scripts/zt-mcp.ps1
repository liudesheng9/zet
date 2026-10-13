#Requires -Version 7
<#
.SYNOPSIS
  Call zt MCP tools from a shell, for agents whose harness has no `zt` tools (DeepSeek Harness).

.DESCRIPTION
  Starts `zt mcp` (agent Access, the same server Claude Code and Codex use), runs the calls in
  order, and prints one compact line per call. Stops at the first failed call.

  -File takes a UTF-8 JSON file holding one call or an array of calls, or `-` to read the same
  JSON from stdin (so one shell command can carry a whole batch in a here-string):
    [{"tool": "create_successor", "arguments": {"at": "Key", "title": "...", "body": "..."}},
     {"tool": "create_successor", "arguments": {"at": "@@0", "kind": "side", "title": "...", "body": "... [[@@0]] ..."}}]
  Inside any string argument, `@@N` is replaced by the address returned by an earlier call N
  (0-based). -Tool with -Json runs a single call given inline.

  Output: `N ok <address>` for writes, `N <json>` for reads, `N error <message>` on failure.
  After a failure the last line is `resume: -From N -Addresses a0,a1,...`: fix call N in the same
  input and run again with those two arguments; calls before N are skipped and their addresses
  still resolve `@@`.

  -Log <file> appends the address of every Card a create call makes, one per line, so a
  coordinator knows which Cards each agent owns. -Allow <file> (one address per line) limits the
  run to those Cards: edit_card and create_successor must target a listed address or a Card made
  earlier in the same run, and create_topic, move_card, insert_card and delete_card are refused.

  insert_card and move_card renumber Cards: an address read before them may name another Card
  after them, so read again (or use the `moved` map they return) before the next write.
#>
[CmdletBinding(DefaultParameterSetName = 'File')]
param(
    [Parameter(ParameterSetName = 'File', Mandatory, Position = 0)][string]$File,
    [Parameter(ParameterSetName = 'File')][int]$From = 0,
    [Parameter(ParameterSetName = 'File')][string]$Addresses = '',
    [Parameter(ParameterSetName = 'File')][string]$Log = '',
    [Parameter(ParameterSetName = 'File')][string]$Allow = '',
    [Parameter(ParameterSetName = 'Inline', Mandatory)][string]$Tool,
    [Parameter(ParameterSetName = 'Inline')][string]$Json = '{}'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)

$calls = @(if ($PSCmdlet.ParameterSetName -eq 'File') {
    $source = if ($File -eq '-') { [Console]::In.ReadToEnd() } else { Get-Content -LiteralPath $File -Raw -Encoding utf8 }
    $source | ConvertFrom-Json -AsHashtable -DateKind String
} else {
    @{ tool = $Tool; arguments = ($Json | ConvertFrom-Json -AsHashtable -DateKind String) }
})

$allowed = $null
if ($Allow) {
    $allowed = [Collections.Generic.HashSet[string]]::new([string[]]@(
        Get-Content -LiteralPath $Allow -Encoding utf8 | ForEach-Object { $_.Trim() } | Where-Object { $_ }))
}

$zt = (Get-Command zt -ErrorAction Stop).Source
$start = [Diagnostics.ProcessStartInfo]::new($zt, 'mcp')
$start.RedirectStandardInput = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
$start.UseShellExecute = $false
$start.StandardInputEncoding = [Text.UTF8Encoding]::new($false)
$start.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
$server = [Diagnostics.Process]::Start($start)

function Send($message) {
    $server.StandardInput.WriteLine(($message | ConvertTo-Json -Depth 50 -Compress))
    $server.StandardInput.Flush()
}
function Receive($id) {
    while ($null -ne ($line = $server.StandardOutput.ReadLine())) {
        $reply = $line | ConvertFrom-Json -AsHashtable -DateKind String
        if ($reply.id -eq $id) { return $reply }
    }
    throw "zt mcp closed: $($server.StandardError.ReadToEnd())"
}
function Resolve-Refs($value, $addresses) {
    if ($value -is [string]) {
        return [regex]::Replace($value, '@@(\d+)', {
            param($m)
            $n = [int]$m.Groups[1].Value
            if ($n -ge $addresses.Count -or -not $addresses[$n]) { throw "@@$n has no address yet" }
            $addresses[$n]
        })
    }
    if ($value -is [System.Collections.IDictionary]) {
        $copy = [ordered]@{}
        foreach ($key in $value.Keys) { $copy[$key] = Resolve-Refs $value[$key] $addresses }
        return $copy
    }
    if ($value -is [System.Collections.IList]) { return @($value | ForEach-Object { Resolve-Refs $_ $addresses }) }
    return $value
}

try {
    Send @{ jsonrpc = '2.0'; id = 0; method = 'initialize'; params = @{ protocolVersion = '2025-06-18' } }
    $null = Receive 0
    Send @{ jsonrpc = '2.0'; method = 'notifications/initialized' }

    $resolved = [Collections.Generic.List[string]]::new()
    $known = @($Addresses -split ',' | ForEach-Object { $_.Trim() })
    for ($i = 0; $i -lt $From; $i++) { $resolved.Add($(if ($i -lt $known.Count) { $known[$i] } else { '' })) }
    function Fail($i, $message) {
        Write-Output "$i error $($message -replace '\s*\r?\n\s*', ' ')"
        Write-Output "resume: -From $i -Addresses $($resolved -join ',')"
        exit 1
    }
    for ($i = $From; $i -lt $calls.Count; $i++) {
        $call = $calls[$i]
        try {
            $arguments = Resolve-Refs ($call.arguments ?? @{}) $resolved
        } catch {
            Fail $i $_.Exception.Message
        }
        if ($null -ne $allowed) {
            if ($call.tool -in 'create_topic', 'move_card', 'insert_card', 'delete_card', 'create_literature', 'edit_literature_metadata') {
                Fail $i "$($call.tool) is not allowed in this run"
            }
            if ($call.tool -in 'edit_card', 'append_card', 'create_successor' -and -not $allowed.Contains([string]$arguments.at)) {
                Fail $i "``$($arguments.at)`` is not one of your Cards; you may only change your own Cards"
            }
        }
        $tool = $call.tool
        if ($tool -eq 'append_card') {
            # Helper-only call: add a paragraph to the end of a Card's body without resending it.
            Send @{ jsonrpc = '2.0'; id = "read$i"; method = 'tools/call'; params = @{ name = 'get_card'; arguments = @{ at = $arguments.at } } }
            $current = Receive "read$i"
            if ($current.error -or $current.result.isError) { Fail $i "cannot read ``$($arguments.at)``" }
            $body = ([string]$current.result.structuredContent['body']).TrimEnd()
            $arguments = @{ at = $arguments.at; body = "$body`n`n$(([string]$arguments.text).Trim())" }
            $tool = 'edit_card'
        }
        Send @{ jsonrpc = '2.0'; id = $i + 1; method = 'tools/call'; params = @{ name = $tool; arguments = $arguments } }
        $reply = Receive ($i + 1)
        $text = ($reply.result.content | ForEach-Object { $_.text }) -join "`n"
        if ($reply.error -or $reply.result.isError) {
            $message = $reply.error.message ?? $text
            try { $message = ($message | ConvertFrom-Json -AsHashtable).error ?? $message } catch { }
            Fail $i $message
        }
        $structured = $reply.result.structuredContent
        $address = if ($structured -is [System.Collections.IDictionary]) { $structured['address'] } else { $null }
        $resolved.Add($address)
        if ($address -and $call.tool -match '^create') {
            if ($null -ne $allowed) { [void]$allowed.Add($address) }
            if ($Log) { [IO.File]::AppendAllText($Log, "$address`n", [Text.UTF8Encoding]::new($false)) }
        }
        if ($address -and $call.tool -match '^(create|edit|append|move|insert|delete)') { Write-Output "$i ok $address" }
        else {
            try { $text = $text | ConvertFrom-Json -DateKind String | ConvertTo-Json -Depth 50 -Compress } catch { }
            Write-Output "$i $text"
        }
    }
} finally {
    $server.StandardInput.Close()
    if (-not $server.WaitForExit(5000)) { $server.Kill() }
}
