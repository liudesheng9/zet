#Requires -Version 7.5
<#
.SYNOPSIS
  The one writer of a step script's last line (#180). Dot-source it in every step script:
  `. (Join-Path $PSScriptRoot 'step-line.ps1')`.

.DESCRIPTION
  A DeepSeek drive job's Haiku manager copies the step's last line into its `answer` field. On the live run of
  issue #180 a long line (a gate's unresolved unit list, a unit's results, a report body) came back retyped as
  prose, the module read no job list, and the stage ended `done` with eight units still claimed. So a step's
  last line is ONE compact JSON object of at most 400 characters:

    - `ok` first, always a boolean;
    - then the counts and file paths the module reads, and nothing else: no nested gate, no unit list, no
      report body;
    - the full data goes to a detail file under <Out>\drive-lines, named in the line as `detail`.

  zt-workflow-lib's `ztDrive` accepts only a last non-empty line that parses as a JSON object with a boolean
  top-level `ok`; a prose answer or a nested object fails the stage.

  A line that is still over 400 characters after a long error is cut is an authoring fault, not data: the
  helper writes the whole line to its detail file and answers `{ok:false, step, error:"line_overflow: ...", detail}`,
  so the stage stops instead of reading a line without its fields as a success; the step scripts' own tests
  assert every answer of theirs fits.
#>
$StepLineMax = 400

# The folder of a run's step detail files.
function Step-LineDir([string]$Out) {
    $dir = Join-Path $Out 'drive-lines'
    [void][System.IO.Directory]::CreateDirectory($dir)
    return $dir
}

# The named fields of an answer as one ordered dictionary (a hashtable or an ordered one both read).
function ConvertTo-StepFields($Value) {
    $fields = [ordered]@{}
    if ($Value -is [System.Collections.IDictionary]) {
        foreach ($k in $Value.Keys) { $fields["$k"] = $Value[$k] }
    } elseif ($Value) {
        foreach ($p in $Value.PSObject.Properties) { $fields[$p.Name] = $p.Value }
    }
    return $fields
}

# The fields a line keeps: `ok` first as a boolean, then every named value a module can read. A zero, an empty
# string, an empty array and a false are left out: the module reads an absent count as 0, an absent string as
# empty and an absent flag as false, and the line stays inside its 400 characters (#180).
function Step-LineFields($Fields) {
    $out = [ordered]@{}
    $ok = if ($Fields.Contains('ok')) { $Fields['ok'] } else { $true }
    $out['ok'] = [bool]$ok
    foreach ($k in @($Fields.Keys)) {
        if ("$k" -eq 'ok') { continue }
        $v = $Fields[$k]
        if ($null -eq $v) { continue }
        if ($v -is [bool] -and $v -eq $false) { continue }
        if ($v -is [string] -and $v.Trim() -eq '') { continue }
        if ($v -is [int] -or $v -is [long] -or $v -is [double] -or $v -is [decimal]) { if ([double]$v -eq 0) { continue } }
        if ($v -is [array] -and $v.Count -eq 0) { continue }
        $out["$k"] = $v
    }
    return $out
}

# How many entries a list-typed field names; 0 when the tool printed none (an absent list is not one null entry).
function Count-Of($Value) {
    if ($null -eq $Value) { return 0 }
    return @($Value).Count
}
# How many properties a map-typed field names (a count per rule, a form count); 0 when it printed none.
function Keys-Of($Value) {
    if ($null -eq $Value) { return 0 }
    if ($Value -is [System.Collections.IDictionary]) { return $Value.Keys.Count }
    if ($Value -is [string] -or $Value -is [array]) { return 0 }
    return @($Value.PSObject.Properties).Count
}

# Fits the line into the 400 characters: a long error is the only allowed cut; anything else that still does not
# fit moves the whole line to its detail file and answers {ok, detail}.
function Step-FitLine($Flat, [string]$Name, [string]$Dir) {
    $text = $Flat | ConvertTo-Json -Depth 8 -Compress
    if ($text.Length -le $StepLineMax) { return $text }
    if ($Flat['error'] -is [string] -and $Flat['error'].Length -gt 0) {
        $over = $text.Length - $StepLineMax
        $keep = [Math]::Max(0, $Flat['error'].Length - $over - 4)
        $Flat['error'] = if ($keep -gt 0) { $Flat['error'].Substring(0, $keep) } else { 'see the detail file' }
        $text = $Flat | ConvertTo-Json -Depth 8 -Compress
        if ($text.Length -le $StepLineMax) { return $text }
    }
    $path = $null
    if ($Dir) {
        $path = Join-Path $Dir "$Name.json"
        $Flat | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $path -Encoding utf8
    }
    # Live run 2026-10-26 (staging4): `{ok:true, detail}` read as a success with no jobs, and six models stayed
    # unwritten. A line that does not fit is a failed step, never a silent success.
    $short = [ordered]@{ ok = $false; step = $Name; error = "line_overflow: the step's line is $($text.Length) characters (max $StepLineMax); see the detail file"; detail = $path }
    return ($short | ConvertTo-Json -Depth 4 -Compress)
}

# Print one step's last line and exit 0. `Line` holds the flat fields the module reads; `Detail` holds the full
# data the line no longer carries and is written beside the line (the detail file also keeps the flat fields, so
# a person can read one file per step).
function Send-StepLine($Line, $Detail = $null, [string]$Name = 'step', [string]$Out = '') {
    $fields = ConvertTo-StepFields $Line
    $dir = $null
    if ($Out) { $dir = Step-LineDir $Out }
    $path = $null
    if ($dir) {
        $path = Join-Path $dir "$Name.json"
        $full = [ordered]@{ line = (Step-LineFields $fields) }
        if ($null -ne $Detail) { $full['detail'] = $Detail }
        $full | ConvertTo-Json -Depth 24 | Set-Content -LiteralPath $path -Encoding utf8
    }
    $flat = Step-LineFields $fields
    if ($path) { $flat['detail'] = $path }
    Write-Output (Step-FitLine $flat $Name $dir)
    exit 0
}

# The step's stop on a failed command: the reason and the step's name, never a stack of output lines.
function Send-StepFail([string]$Step, [string]$Reason, [string]$Name = 'step', [string]$Out = '', $Detail = $null, $Extra = $null) {
    $line = [ordered]@{ ok = $false; step = $Step; error = "$Reason".Trim() }
    if ($Extra -is [System.Collections.IDictionary]) { foreach ($k in $Extra.Keys) { $line["$k"] = $Extra[$k] } }
    Send-StepLine $line $Detail $Name $Out
}


