param(
  [string]$HubUrl = "https://hub.example.com/agentgrid",
  [string]$NodeId = "",
  [string]$NodeName = "",
  [int]$MaxConcurrentJobs = 0,
  [int]$IntervalSeconds = 0,
  [string]$JoinToken = "",
  [string]$InstallDir = "$env:ProgramFiles\AgentGridWorker",
  [string]$ToolsDir = "$env:ProgramFiles\AgentGridTools\bin",
  [switch]$DesktopHelper
)

$ErrorActionPreference = "Stop"

function Assert-Admin {
  $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
  $principal = New-Object Security.Principal.WindowsPrincipal($identity)
  if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Please run PowerShell as Administrator."
  }
}

Assert-Admin

function Add-MachinePath {
  param([string]$PathToAdd)
  if ([string]::IsNullOrWhiteSpace($PathToAdd)) {
    return
  }
  $normalized = [System.IO.Path]::GetFullPath($PathToAdd).TrimEnd('\')
  $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
  $parts = @()
  if (-not [string]::IsNullOrWhiteSpace($machinePath)) {
    $parts = $machinePath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
  }
  $exists = $false
  foreach ($part in $parts) {
    try {
      if ([System.IO.Path]::GetFullPath($part).TrimEnd('\').ToLowerInvariant() -eq $normalized.ToLowerInvariant()) {
        $exists = $true
        break
      }
    } catch {
      if ($part.TrimEnd('\').ToLowerInvariant() -eq $normalized.ToLowerInvariant()) {
        $exists = $true
        break
      }
    }
  }
  if (-not $exists) {
    $newPath = if ([string]::IsNullOrWhiteSpace($machinePath)) { $normalized } else { "$machinePath;$normalized" }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "Machine")
    $env:Path = "$env:Path;$normalized"
    Write-Host "Added machine PATH: $normalized"
  }
}

if ([string]::IsNullOrWhiteSpace($NodeId)) {
  $NodeId = if ([string]::IsNullOrWhiteSpace($env:AG_NODE_ID)) { "$env:COMPUTERNAME-windows" } else { $env:AG_NODE_ID }
}
if ([string]::IsNullOrWhiteSpace($NodeName)) {
  $NodeName = if ([string]::IsNullOrWhiteSpace($env:AG_NODE_NAME)) { "$env:COMPUTERNAME" } else { $env:AG_NODE_NAME }
}
if ($MaxConcurrentJobs -le 0) {
  $MaxConcurrentJobs = if ([string]::IsNullOrWhiteSpace($env:AG_MAX_JOBS)) { 4 } else { [int]$env:AG_MAX_JOBS }
}
if ($IntervalSeconds -le 0) {
  $IntervalSeconds = if ([string]::IsNullOrWhiteSpace($env:AG_INTERVAL_SECONDS)) { 5 } else { [int]$env:AG_INTERVAL_SECONDS }
}
if ([string]::IsNullOrWhiteSpace($JoinToken)) {
  $JoinToken = if ([string]::IsNullOrWhiteSpace($env:AGENTGRID_JOIN_TOKEN)) { $env:AG_JOIN_TOKEN } else { $env:AGENTGRID_JOIN_TOKEN }
}
if (-not $DesktopHelper -and $env:AG_DESKTOP_HELPER -match '^(1|true|yes)$') {
  $DesktopHelper = $true
}

$WorkerUrl = "$HubUrl/api/worker/download/windows-x86_64"
$WorkerExe = Join-Path $InstallDir "agentgrid-worker.exe"
$DesktopWorkerExe = Join-Path $InstallDir "agentgrid-worker-desktop.exe"
$LogDir = Join-Path $InstallDir "logs"
$RunnerScript = Join-Path $InstallDir "run-agentgrid-worker.ps1"
$TaskName = "AgentGridWorker"
$DesktopTaskName = "AgentGridDesktopHelper"
$DesktopRunnerScript = Join-Path $InstallDir "run-agentgrid-desktop-helper.ps1"

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
New-Item -ItemType Directory -Force -Path $ToolsDir | Out-Null
Add-MachinePath -PathToAdd $ToolsDir

Write-Host "Downloading AgentGrid Worker from $WorkerUrl"
try {
  Invoke-WebRequest -Uri $WorkerUrl -OutFile $WorkerExe
} catch {
  throw "Failed to download agentgrid-worker.exe from $WorkerUrl. The Windows worker package may not be published yet."
}

if (-not (Test-Path $WorkerExe)) {
  throw "agentgrid-worker.exe download failed."
}
if ((Get-Item $WorkerExe).Length -lt 1048576) {
  Remove-Item $WorkerExe -Force -ErrorAction SilentlyContinue
  throw "Downloaded worker package is too small. The Windows worker package may not be published yet."
}

$WorkerArgs = @(
  "--hub", $HubUrl,
  "--id", $NodeId,
  "--name", $NodeName,
  "--tag", "worker",
  "--tag", "windows",
  "--capability", "http",
  "--capability", "command",
  "--capability", "file",
  "--capability", "git",
  "--capability", "docker",
  "--capability", "browser",
  "--capability", "session",
  "--capability", "agentmessage",
  "--capability", "plugin",
  "--max-concurrent-jobs", "$MaxConcurrentJobs",
  "--interval-seconds", "$IntervalSeconds"
)
if (-not [string]::IsNullOrWhiteSpace($JoinToken)) {
  $WorkerArgs += @("--join-token", $JoinToken)
}

$WorkerArgsLiteral = ($WorkerArgs | ForEach-Object { "'" + ($_ -replace "'", "''") + "'" }) -join ", "
$RunnerContent = @"
`$ErrorActionPreference = "Continue"
`$WorkerExe = '$($WorkerExe -replace "'", "''")'
`$LogDir = '$($LogDir -replace "'", "''")'
`$ArgsList = @($WorkerArgsLiteral)
New-Item -ItemType Directory -Force -Path `$LogDir | Out-Null
`$env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
while (`$true) {
  `$stamp = Get-Date -Format o
  Add-Content -Path (Join-Path `$LogDir "supervisor.log") -Value "`$stamp starting agentgrid-worker"
  & `$WorkerExe @ArgsList >> (Join-Path `$LogDir "worker.out.log") 2>> (Join-Path `$LogDir "worker.err.log")
  `$code = `$LASTEXITCODE
  `$stamp = Get-Date -Format o
  Add-Content -Path (Join-Path `$LogDir "supervisor.log") -Value "`$stamp agentgrid-worker exited with code `$code; restarting in 5s"
  Start-Sleep -Seconds 5
}
"@
Set-Content -Path $RunnerScript -Value $RunnerContent -Encoding UTF8

if (Get-Service agentgrid-worker -ErrorAction SilentlyContinue) {
  Write-Host "Stopping old agentgrid-worker service"
  Stop-Service agentgrid-worker -ErrorAction SilentlyContinue
  sc.exe delete agentgrid-worker | Out-Null
  Start-Sleep -Seconds 2
}

if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
  Write-Host "Removing old AgentGridWorker scheduled task"
  Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
  Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
}

Write-Host "Installing AgentGridWorker scheduled task"
$Action = New-ScheduledTaskAction `
  -Execute "powershell.exe" `
  -Argument "-NoProfile -ExecutionPolicy Bypass -File `"$RunnerScript`""
$Trigger = New-ScheduledTaskTrigger -AtStartup
$Principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -RunLevel Highest
$Settings = New-ScheduledTaskSettingsSet `
  -AllowStartIfOnBatteries `
  -DontStopIfGoingOnBatteries `
  -ExecutionTimeLimit (New-TimeSpan -Days 3650) `
  -RestartCount 3 `
  -RestartInterval (New-TimeSpan -Minutes 1)
Register-ScheduledTask `
  -TaskName $TaskName `
  -Action $Action `
  -Trigger $Trigger `
  -Principal $Principal `
  -Settings $Settings `
  -Description "AgentGrid Worker supervisor" | Out-Null

if ($DesktopHelper) {
  Write-Host "Preparing independent Desktop Helper worker binary"
  Stop-ScheduledTask -TaskName $DesktopTaskName -ErrorAction SilentlyContinue
  Get-Process -Name "agentgrid-worker-desktop" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Copy-Item -Path $WorkerExe -Destination $DesktopWorkerExe -Force

  $DesktopNodeId = "$NodeId-desktop"
  $DesktopArgs = @(
    "--hub", $HubUrl,
    "--id", $DesktopNodeId,
    "--name", "$NodeName Desktop",
    "--tag", "worker",
    "--tag", "windows",
    "--tag", "desktop",
    "--capability", "desktop",
    "--max-concurrent-jobs", "1",
    "--interval-seconds", "$IntervalSeconds",
    "--no-auto-update"
  )
  if (-not [string]::IsNullOrWhiteSpace($JoinToken)) {
    $DesktopArgs += @("--join-token", $JoinToken)
  }
  $DesktopArgsLiteral = ($DesktopArgs | ForEach-Object { "'" + ($_ -replace "'", "''") + "'" }) -join ", "
  $DesktopRunnerContent = @"
`$ErrorActionPreference = "Continue"
`$WorkerExe = '$($DesktopWorkerExe -replace "'", "''")'
`$LogDir = '$($LogDir -replace "'", "''")'
`$ArgsList = @($DesktopArgsLiteral)
New-Item -ItemType Directory -Force -Path `$LogDir | Out-Null
`$env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
while (`$true) {
  `$stamp = Get-Date -Format o
  Add-Content -Path (Join-Path `$LogDir "desktop-helper.log") -Value "`$stamp starting agentgrid desktop helper"
  & `$WorkerExe @ArgsList >> (Join-Path `$LogDir "desktop-helper.out.log") 2>> (Join-Path `$LogDir "desktop-helper.err.log")
  `$code = `$LASTEXITCODE
  `$stamp = Get-Date -Format o
  Add-Content -Path (Join-Path `$LogDir "desktop-helper.log") -Value "`$stamp agentgrid desktop helper exited with code `$code; restarting in 5s"
  Start-Sleep -Seconds 5
}
"@
  Set-Content -Path $DesktopRunnerScript -Value $DesktopRunnerContent -Encoding UTF8

  if (Get-ScheduledTask -TaskName $DesktopTaskName -ErrorAction SilentlyContinue) {
    Write-Host "Removing old AgentGridDesktopHelper scheduled task"
    Stop-ScheduledTask -TaskName $DesktopTaskName -ErrorAction SilentlyContinue
    Unregister-ScheduledTask -TaskName $DesktopTaskName -Confirm:$false
  }

  Write-Host "Installing AgentGridDesktopHelper scheduled task for the interactive user"
  $DesktopAction = New-ScheduledTaskAction `
    -Execute "powershell.exe" `
    -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$DesktopRunnerScript`""
  $DesktopTrigger = New-ScheduledTaskTrigger -AtLogOn
  $InteractiveUser = (Get-CimInstance Win32_ComputerSystem).UserName
  if ([string]::IsNullOrWhiteSpace($InteractiveUser)) {
    $InteractiveUser = "$env:USERDOMAIN\$env:USERNAME"
  }
  if ($InteractiveUser -match '^(NT AUTHORITY\\SYSTEM|SYSTEM)$') {
    throw "Desktop Helper requires a real logged-in Windows user. Please log in to the Windows desktop first."
  }
  $DesktopPrincipal = New-ScheduledTaskPrincipal -UserId $InteractiveUser -LogonType Interactive -RunLevel Highest
  Register-ScheduledTask `
    -TaskName $DesktopTaskName `
    -Action $DesktopAction `
    -Trigger $DesktopTrigger `
    -Principal $DesktopPrincipal `
    -Settings $Settings `
    -Description "AgentGrid interactive desktop helper" | Out-Null
  Start-ScheduledTask -TaskName $DesktopTaskName
  Write-Host "Desktop helper node id: $DesktopNodeId"
  Write-Host "Desktop helper user: $InteractiveUser"
}

Start-ScheduledTask -TaskName $TaskName
Start-Sleep -Seconds 5

$task = Get-ScheduledTask -TaskName $TaskName
$nodeUrl = "$HubUrl/api/nodes"

Write-Host "Task state: $($task.State)"
Write-Host "Run account: SYSTEM"
Write-Host "Run level: Highest"
Write-Host "Node id: $NodeId"
Write-Host "Hub: $HubUrl"
Write-Host "Logs: $LogDir"
Write-Host "Machine tools PATH: $ToolsDir"
Write-Host "Check in console: $nodeUrl"

Get-ScheduledTask -TaskName $TaskName
