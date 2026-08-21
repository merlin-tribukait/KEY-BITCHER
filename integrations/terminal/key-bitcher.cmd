@echo off
rem key-bitcher - cmd.exe / Windows Terminal shim for Key-Bitcher.
rem Put this file (or the bin folder) on your PATH, then:
rem   key-bitcher sync-secrets
rem   key-bitcher benchmark
setlocal

set "KEY_BITCHER_ROOT=%~dp0"
:walk
if exist "%KEY_BITCHER_ROOT%Cargo.toml" goto found
set "KEY_BITCHER_PARENT=%KEY_BITCHER_ROOT%.."
if "%KEY_BITCHER_PARENT%"=="%KEY_BITCHER_ROOT%" goto notfound
set "KEY_BITCHER_ROOT=%KEY_BITCHER_PARENT%\"
goto walk
:notfound
echo key-bitcher project root (Cargo.toml) not found. 1>&2
exit /b 1

:found
set "KEY_BITCHER_BIN=%KEY_BITCHER_ROOT%target\release\key-bitcher.exe"
if not exist "%KEY_BITCHER_BIN%" (
  echo key-bitcher.exe not found. Run: cargo build --release in %KEY_BITCHER_ROOT% 1>&2
  exit /b 1
)
pushd "%KEY_BITCHER_ROOT%"
"%KEY_BITCHER_BIN%" %*
set "KEY_BITCHER_EXIT=%ERRORLEVEL%"
popd
exit /b %KEY_BITCHER_EXIT%
