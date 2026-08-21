# key-bitcher - PowerShell wrapper for Key-Bitcher.
#
# Install (add to your $PROFILE, e.g. notepad $PROFILE):
#   . C:\path\to\key-bitcher\integrations\terminal\key-bitcher.ps1
#
# Commands:
#   key-bitcher            run the full auto flow (sync + wave import + benchmark)
#   key-bitcher sync-secrets
#   key-bitcher benchmark
#   key-bitcher wave-import
#   key-bitcher list        (future: list secret names)
#   Export-KeyBitcherEnv    load .env values into the current session
#   kb                      alias for key-bitcher

function Get-KeyBitcherRoot {
    $root = $PSScriptRoot
    while ($root -and -not (Test-Path (Join-Path $root 'Cargo.toml'))) {
        $root = Split-Path -Parent $root
    }
    if (-not $root) { throw 'Could not locate key-bitcher project root (Cargo.toml not found).' }
    return $root
}

$script:KeyBitcherRoot = Get-KeyBitcherRoot
$script:KeyBitcherExe = Join-Path $script:KeyBitcherRoot 'target\release\key-bitcher.exe'

function key-bitcher {
    param([Parameter(Position = 0, ValueFromRemainingArguments = $true)][string[]]$CmdArgs)
    if (-not (Test-Path $script:KeyBitcherExe)) {
        Write-Error "key-bitcher.exe not found. Run: cargo build --release  in $script:KeyBitcherRoot"
        return
    }
    Push-Location $script:KeyBitcherRoot
    try {
        if ($CmdArgs.Count -eq 0) { & $script:KeyBitcherExe } else { & $script:KeyBitcherExe @CmdArgs }
    }
    finally {
        Pop-Location
    }
}

function Sync-KeyBitcher {
    key-bitcher sync-secrets @args
}

function Invoke-KeyBitcherBench {
    key-bitcher benchmark @args
}

function Export-KeyBitcherEnv {
    # Loads KEY=VALUE lines from the project .env into the current process env.
    $envFile = Join-Path $script:KeyBitcherRoot '.env'
    if (-not (Test-Path $envFile)) {
        Write-Warning ".env not found at $envFile"
        return
    }
    Get-Content $envFile | ForEach-Object {
        $line = $_.Trim()
        if ($line -and -not $line.StartsWith('#')) {
            $i = $line.IndexOf('=')
            if ($i -gt 0) {
                $k = $line.Substring(0, $i).Trim()
                $v = $line.Substring($i + 1).Trim()
                [Environment]::SetEnvironmentVariable($k, $v, 'Process')
            }
        }
    }
    Write-Host "Loaded env vars from $envFile into the current session."
}

Set-Alias -Name kb -Value key-bitcher
