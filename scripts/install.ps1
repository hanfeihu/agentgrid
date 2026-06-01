param(
  [string]$Repo = "hanfeihu/agentgrid",
  [string]$Version = "latest",
  [string]$InstallDir = "$env:ProgramFiles\AgentGrid",
  [string]$HubUrl = "http://127.0.0.1:20181",
  [string]$NodeId = "",
  [string]$NodeName = "",
  [string]$JoinToken = "",
  [int]$MaxConcurrentJobs = 4,
  [int]$IntervalSeconds = 5,
  [switch]$InstallWorker,
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

function Add-MachinePath {
  param([string]$PathToAdd)
  $normalized = [System.IO.Path]::GetFullPath($PathToAdd).TrimEnd('\')
  $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
  $parts = @()
  if (-not [string]::IsNullOrWhiteSpace($machinePath)) {
    $parts = $machinePath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
  }
  foreach ($part in $parts) {
    try {
      if ([System.IO.Path]::GetFullPath($part).TrimEnd('\').ToLowerInvariant() -eq $normalized.ToLowerInvariant()) {
        return
      }
    } catch {
      if ($part.TrimEnd('\').ToLowerInvariant() -eq $normalized.ToLowerInvariant()) {
        return
      }
    }
  }
  $newPath = if ([string]::IsNullOrWhiteSpace($machinePath)) { $normalized } else { "$machinePath;$normalized" }
  [Environment]::SetEnvironmentVariable("Path", $newPath, "Machine")
  $env:Path = "$env:Path;$normalized"
}

Assert-Admin

if ($Version -eq "latest") {
  $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases?per_page=1"
  if ($release -is [array]) {
    $release = $release[0]
  }
  $Version = $release.tag_name
  if ([string]::IsNullOrWhiteSpace($Version)) {
    throw "Could not resolve latest AgentGrid release for $Repo."
  }
}

if ([string]::IsNullOrWhiteSpace($NodeId)) {
  $NodeId = if ([string]::IsNullOrWhiteSpace($env:AG_NODE_ID)) { "$env:COMPUTERNAME-windows" } else { $env:AG_NODE_ID }
}
if ([string]::IsNullOrWhiteSpace($NodeName)) {
  $NodeName = if ([string]::IsNullOrWhiteSpace($env:AG_NODE_NAME)) { "$env:COMPUTERNAME" } else { $env:AG_NODE_NAME }
}
if ([string]::IsNullOrWhiteSpace($JoinToken)) {
  $JoinToken = if ([string]::IsNullOrWhiteSpace($env:AGENTGRID_JOIN_TOKEN)) { $env:AG_JOIN_TOKEN } else { $env:AGENTGRID_JOIN_TOKEN }
}
if (-not $DesktopHelper -and $env:AG_DESKTOP_HELPER -match '^(1|true|yes)$') {
  $DesktopHelper = $true
}

$Package = "agentgrid-$Version-windows-x86_64"
$Archive = "$Package.zip"
$Url = "https://github.com/$Repo/releases/download/$Version/$Archive"
$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) "agentgrid-install-$([System.Guid]::NewGuid().ToString('N'))"
$ZipPath = Join-Path $TempDir $Archive
$BinDir = Join-Path $InstallDir "bin"
$WebDir = Join-Path $InstallDir "web"
$DocsDir = Join-Path $InstallDir "docs"
$ExamplesDir = Join-Path $InstallDir "examples"
$LogDir = Join-Path $InstallDir "logs"

New-Item -ItemType Directory -Force -Path $TempDir | Out-Null
try {
  Write-Host "Downloading AgentGrid $Version from $Url"
  Invoke-WebRequest -Uri $Url -OutFile $ZipPath
  Expand-Archive -Path $ZipPath -DestinationPath $TempDir -Force
  $PackageDir = Join-Path $TempDir $Package
  if (-not (Test-Path $PackageDir)) {
    throw "Invalid release archive: package directory not found."
  }

  New-Item -ItemType Directory -Force -Path $BinDir,$WebDir,$DocsDir,$ExamplesDir,$LogDir | Out-Null
  Copy-Item "$PackageDir\bin\*.exe" $BinDir -Force
  Copy-Item "$PackageDir\web\*" $WebDir -Recurse -Force
  Copy-Item "$PackageDir\docs\*" $DocsDir -Recurse -Force
  Copy-Item "$PackageDir\examples\*" $ExamplesDir -Recurse -Force
  Add-MachinePath -PathToAdd $BinDir

  if ($InstallWorker) {
    $WorkerExe = Join-Path $BinDir "agentgrid-worker.exe"
    $RunnerScript = Join-Path $InstallDir "run-agentgrid-worker.ps1"
    $TaskName = "AgentGridWorker"
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
while (`$true) {
  `$stamp = Get-Date -Format o
  Add-Content -Path (Join-Path `$LogDir "worker-supervisor.log") -Value "`$stamp starting agentgrid-worker"
  & `$WorkerExe @ArgsList >> (Join-Path `$LogDir "worker.out.log") 2>> (Join-Path `$LogDir "worker.err.log")
  `$code = `$LASTEXITCODE
  `$stamp = Get-Date -Format o
  Add-Content -Path (Join-Path `$LogDir "worker-supervisor.log") -Value "`$stamp agentgrid-worker exited with code `$code; restarting in 5s"
  Start-Sleep -Seconds 5
}
"@
    Set-Content -Path $RunnerScript -Value $RunnerContent -Encoding UTF8
    if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
      Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
      Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
    }
    $Action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-NoProfile -ExecutionPolicy Bypass -File `"$RunnerScript`""
    $Trigger = New-ScheduledTaskTrigger -AtStartup
    $Principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -RunLevel Highest
    $Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Days 3650) -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)
    Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Principal $Principal -Settings $Settings -Description "AgentGrid Worker supervisor" | Out-Null
    Start-ScheduledTask -TaskName $TaskName
  }

  if ($DesktopHelper) {
    $WorkerExe = Join-Path $BinDir "agentgrid-worker.exe"
    $DesktopWorkerExe = Join-Path $BinDir "agentgrid-worker-desktop.exe"
    Copy-Item -Path $WorkerExe -Destination $DesktopWorkerExe -Force
    $DesktopRunnerScript = Join-Path $InstallDir "run-agentgrid-desktop-helper.ps1"
    $DesktopTaskName = "AgentGridDesktopHelper"
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
      Stop-ScheduledTask -TaskName $DesktopTaskName -ErrorAction SilentlyContinue
      Unregister-ScheduledTask -TaskName $DesktopTaskName -Confirm:$false
    }
    $InteractiveUser = (Get-CimInstance Win32_ComputerSystem).UserName
    if ([string]::IsNullOrWhiteSpace($InteractiveUser)) {
      throw "Desktop Helper requires a real logged-in Windows user."
    }
    $Action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$DesktopRunnerScript`""
    $Trigger = New-ScheduledTaskTrigger -AtLogOn
    $Principal = New-ScheduledTaskPrincipal -UserId $InteractiveUser -LogonType Interactive -RunLevel Highest
    $Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Days 3650) -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)
    Register-ScheduledTask -TaskName $DesktopTaskName -Action $Action -Trigger $Trigger -Principal $Principal -Settings $Settings -Description "AgentGrid interactive desktop helper" | Out-Null
    Start-ScheduledTask -TaskName $DesktopTaskName
  }

  Write-Host ""
  Write-Host "AgentGrid $Version installed."
  Write-Host "CLI: agentgrid --help"
  Write-Host "Run Hub: agentgrid-hub --host 127.0.0.1 --port 20181 --db `"$InstallDir\agentgrid-hub.db`" --web-dir `"$WebDir`""
  Write-Host "Console: http://127.0.0.1:20181"
} finally {
  Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
}
