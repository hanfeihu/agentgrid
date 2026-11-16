# AgentGrid 一键安装(给 AgentGrid-Setup.cmd 调用,放在 hub 的 web/downloads/ 下,改这里不用重新发 .cmd)。
# hub 地址固定用 https://agentgrid.tminos.com:国内解析到阿里云前置机、国外解析到美国搬瓦工,都转到 chenqi。
param([string]$NodeName = "")
$ErrorActionPreference = "Stop"
$Hub = "https://agentgrid.tminos.com"
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

# 装过的电脑沿用原来的节点 ID、名字和桌面助手:重装、升级都接回原节点,不会多出一台
$installDir = Join-Path $env:ProgramFiles "AgentGridWorker"
$runner = Join-Path $installDir "run-agentgrid-worker.ps1"
$nodeId = ""
$existingName = ""
if (Test-Path $runner) {
  $text = Get-Content $runner -Raw
  if ($text -match "'--id', '((?:[^']|'')+)'") { $nodeId = $Matches[1] -replace "''", "'" }
  if ($text -match "'--name', '((?:[^']|'')+)'") { $existingName = $Matches[1] -replace "''", "'" }
}
if ([string]::IsNullOrWhiteSpace($NodeName)) {
  $NodeName = if ($existingName) { $existingName } else { $env:COMPUTERNAME }
}
$desktopHelper = Test-Path (Join-Path $installDir "run-agentgrid-desktop-helper.ps1")
if ($nodeId) { Write-Host "Existing installation found: keeping node $nodeId" }

$installer = Join-Path $env:TEMP "agentgrid-install.ps1"
Invoke-WebRequest -UseBasicParsing "$Hub/install/windows.ps1" -OutFile $installer
$params = @{ HubUrl = $Hub; NodeName = $NodeName }
if ($nodeId) { $params.NodeId = $nodeId }
if ($desktopHelper) { $params.DesktopHelper = $true }
& $installer @params
