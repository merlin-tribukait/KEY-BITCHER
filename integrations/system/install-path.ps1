# system/install-path.ps1 - add the key-bitcher wrappers to the current user's PATH.
#
# Copies the terminal wrappers into a bin\ folder and adds it to the user PATH,
# so `key-bitcher sync-secrets` works from any console (cmd, PowerShell, Windows
# Terminal) after opening a new terminal.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File integrations\system\install-path.ps1

param(
  [string]$Root = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
)

$binDir = Join-Path $Root 'bin'
New-Item -ItemType Directory -Force $binDir | Out-Null

Copy-Item (Join-Path $Root 'integrations\terminal\key-bitcher.ps1') $binDir -Force
Copy-Item (Join-Path $Root 'integrations\terminal\key-bitcher.cmd') $binDir -Force

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$binDir*") {
  [Environment]::SetEnvironmentVariable('Path', "$userPath;$binDir", 'User')
  Write-Host "Added $binDir to the user PATH. Open a new terminal and try: key-bitcher sync-secrets"
} else {
  Write-Host "$binDir is already on the user PATH."
}

Write-Host "Wrappers installed in $binDir"
