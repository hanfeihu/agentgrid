param(
  [string]$HubUrl = "https://hub.example.com/agentgrid",
  [string]$NodeId = "$env:COMPUTERNAME-windows",
  [string]$NodeName = "$env:COMPUTERNAME",
  [string]$WorkerExe = ".\agentgrid-worker.exe",
  [int]$MaxConcurrentJobs = 4,
  [int]$IntervalSeconds = 5,
  [string]$JoinToken = "",
  [string]$ToolsDir = "$env:ProgramFiles\AgentGridTools\bin"
)

$InstallDir = "$env:ProgramFiles\AgentGridWorker"
$TargetExe = Join-Path $InstallDir "agentgrid-worker.exe"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $ToolsDir | Out-Null

$machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
if (($machinePath -split ';' | Where-Object { $_.TrimEnd('\').ToLowerInvariant() -eq $ToolsDir.TrimEnd('\').ToLowerInvariant() }).Count -eq 0) {
  [Environment]::SetEnvironmentVariable("Path", "$machinePath;$ToolsDir", "Machine")
}

Copy-Item $WorkerExe $TargetExe -Force

$Args = "--hub `"$HubUrl`" --id `"$NodeId`" --name `"$NodeName`" --tag worker --tag windows --capability http --capability command --capability file --capability git --capability docker --capability browser --capability session --capability agentmessage --capability plugin --max-concurrent-jobs $MaxConcurrentJobs --interval-seconds $IntervalSeconds"
if ([string]::IsNullOrWhiteSpace($JoinToken)) {
  $JoinToken = if ([string]::IsNullOrWhiteSpace($env:AGENTGRID_JOIN_TOKEN)) { $env:AG_JOIN_TOKEN } else { $env:AGENTGRID_JOIN_TOKEN }
}
if (-not [string]::IsNullOrWhiteSpace($JoinToken)) {
  $Args = "$Args --join-token `"$JoinToken`""
}

if (Get-Service agentgrid-worker -ErrorAction SilentlyContinue) {
  Stop-Service agentgrid-worker -ErrorAction SilentlyContinue
  sc.exe delete agentgrid-worker | Out-Null
}

New-Service -Name "agentgrid-worker" -DisplayName "AgentGrid Worker" -BinaryPathName "`"$TargetExe`" $Args" -StartupType Automatic
Start-Service agentgrid-worker
Get-Service agentgrid-worker
