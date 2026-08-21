param(
    [ValidateSet("release", "debug")]
    [string]$Profile = "release",
    [switch]$Test,
    [switch]$Clippy,
    [switch]$Fmt
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $Root

if ($Fmt) {
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

if ($Clippy) {
    cargo clippy --profile $Profile -- -D warnings
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

if ($Test) {
    cargo test --profile $Profile
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

cargo build --profile $Profile
exit $LASTEXITCODE
