# system/env-loader.ps1 - load the project .env into the current process env.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File system\env-loader.ps1
# or dot-source it:
#   . .\integrations\system\env-loader.ps1
# or, to also write them at User scope (persist across sessions), pass -Persist.

param(
  [switch]$Persist,
  [string]$Root = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
)

$envFile = Join-Path $Root '.env'
if (-not (Test-Path $envFile)) {
  Write-Error ".env not found at $envFile"
  exit 1
}

$scope = if ($Persist) { 'User' } else { 'Process' }
$count = 0
Get-Content $envFile | ForEach-Object {
  $line = $_.Trim()
  if ($line -and -not $line.StartsWith('#')) {
    $i = $line.IndexOf('=')
    if ($i -gt 0) {
      $k = $line.Substring(0, $i).Trim()
      $v = $line.Substring($i + 1).Trim()
      [Environment]::SetEnvironmentVariable($k, $v, $scope)
      $count++
    }
  }
}

Write-Host "Loaded $count env vars from $envFile into the $scope scope."
