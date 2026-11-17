@echo off
setlocal
set "ROOT=%~dp0"
if exist "%ROOT%src-tauri\target\debug\phadmin-cli.exe" (
  "%ROOT%src-tauri\target\debug\phadmin-cli.exe" %*
  exit /b %ERRORLEVEL%
)
if exist "%ROOT%src-tauri\target\release\phadmin-cli.exe" (
  "%ROOT%src-tauri\target\release\phadmin-cli.exe" %*
  exit /b %ERRORLEVEL%
)
echo phadmin-cli.exe not found. Run: cargo build --manifest-path src-tauri\Cargo.toml --bin phadmin-cli 1>&2
exit /b 1
