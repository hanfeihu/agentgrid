@echo off
setlocal
title AgentGrid Setup
rem AgentGrid one-click setup for Windows.
rem Double-click this file, then click "Yes" when Windows asks for permission.

net session >nul 2>&1
if errorlevel 1 (
  echo Asking Windows for permission to install...
  powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
  exit /b
)

echo.
echo   AgentGrid Setup
echo   ===============
echo.
set "AG_NAME="
set /p "AG_NAME=Give this computer a name (or just press Enter): "
echo.
echo Installing, please wait about one minute...
echo.
powershell -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference='Stop'; [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12; $f = Join-Path $env:TEMP 'AgentGrid-Setup.ps1'; Invoke-WebRequest -UseBasicParsing 'https://agentgrid.tminos.com/downloads/AgentGrid-Setup.ps1' -OutFile $f; & $f -NodeName $env:AG_NAME"
if errorlevel 1 (
  echo.
  echo   Setup did NOT finish. Please take a screenshot of this window and send it to us.
) else (
  echo.
  echo   Done! This computer is now connected. You can close this window.
)
echo.
pause
