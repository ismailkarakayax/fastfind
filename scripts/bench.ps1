# Compares fastfind with the built-in Windows tools using hyperfine.
#
# Usage:
#   .\scripts\bench.ps1                       # searches "report" under your user folder
#   .\scripts\bench.ps1 -Pattern invoice -Path D:\
#
# Requires: hyperfine (winget install sharkdp.hyperfine) and a release build
# (cargo build --release). Hidden files are included in every command so
# that all tools do the same amount of work.

param(
    [string]$Pattern = "report",
    [string]$Path = $env:USERPROFILE,
    [int]$Runs = 5
)

$ErrorActionPreference = "Stop"

$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $PSScriptRoot "..\target" }
$fastfind = Join-Path $targetDir "release\fastfind.exe"
if (-not (Test-Path $fastfind)) {
    throw "Release build not found at $fastfind. Run 'cargo build --release' first."
}
if (-not (Get-Command hyperfine -ErrorAction SilentlyContinue)) {
    throw "hyperfine not found. Install it with: winget install sharkdp.hyperfine"
}

$fastfindCmd = "`"$fastfind`" -H -q --color never $Pattern `"$Path`""
$dirCmd = "cmd /c dir /s /b /a `"$Path\*$Pattern*`""
$gciCmd = "powershell -NoProfile -Command `"Get-ChildItem -LiteralPath '$Path' -Recurse -Force -Filter '*$Pattern*' -ErrorAction SilentlyContinue | Out-Null`""

$ErrorActionPreference = "Continue"

# Entry count, to put the numbers in context (also warms the disk cache).
$count = (& $fastfind -H -q --color never -g "*" "$Path" | Measure-Object -Line).Lines
Write-Host ("Searching {0:N0} entries under {1}`n" -f $count, $Path)

# Windows PowerShell 5.1 (and PowerShell < 7.3) does not escape embedded
# double quotes when calling native programs, so do it by hand.
function Quote([string]$s) {
    if ($PSVersionTable.PSVersion -lt [version]"7.3") { $s -replace '"', '\"' } else { $s }
}

hyperfine --warmup 1 --runs $Runs --ignore-failure --export-markdown bench-results.md `
    -n "fastfind" (Quote $fastfindCmd) `
    -n "dir /s" (Quote $dirCmd) `
    -n "Get-ChildItem -Recurse" (Quote $gciCmd)

Write-Host "`nResults saved to bench-results.md"
