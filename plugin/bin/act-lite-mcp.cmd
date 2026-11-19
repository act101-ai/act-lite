@echo off
rem Start act-lite for the plugin's MCP server on Windows, with no separate install.
rem
rem plugin/.mcp.json names bin/act-lite-mcp with no extension. On Linux and
rem macOS that is the sh launcher; on Windows the command lookup adds the
rem PATHEXT extensions and finds this file. act-lite-mcp.ps1 finds or downloads
rem act-lite.exe and prints its path; stdout is the MCP channel, so nothing
rem else may write to it.
setlocal
set "ACT_LITE_EXE="
for /f "usebackq delims=" %%P in (`powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0act-lite-mcp.ps1"`) do set "ACT_LITE_EXE=%%P"
if not defined ACT_LITE_EXE exit /b 1
"%ACT_LITE_EXE%" %*
exit /b %ERRORLEVEL%
