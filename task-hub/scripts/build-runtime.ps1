param([switch]$Ensure)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$source = Join-Path $project 'integrations\runtime'
$resources = Join-Path $project 'src-tauri\resources\runtime'
$worker = Join-Path $resources 'taskhub-runtime.exe'
$manifestPath = Join-Path $resources 'build-manifest.json'
$requirements = Join-Path $source 'requirements-build.txt'

function Text-Hash([string]$Text) {
  $algorithm = [Security.Cryptography.SHA256]::Create()
  try { return ([BitConverter]::ToString($algorithm.ComputeHash([Text.Encoding]::UTF8.GetBytes($Text)))).Replace('-', '').ToLowerInvariant() }
  finally { $algorithm.Dispose() }
}
function File-Hash([string]$Path) {
  $algorithm = [Security.Cryptography.SHA256]::Create()
  $stream = [IO.File]::OpenRead($Path)
  try { return ([BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace('-', '').ToLowerInvariant() }
  finally { $stream.Dispose(); $algorithm.Dispose() }
}

# Git checkout timestamps are not a reliable way to decide whether a build changed.
$inputs = @(Get-ChildItem -LiteralPath $source -Recurse -File | Where-Object {
  $_.Extension -in '.py', '.json', '.txt' -and $_.Name -ne 'test_runtime.py' -and $_.FullName -notmatch '[\\/]__pycache__[\\/]'
} | Sort-Object FullName | ForEach-Object { $_.FullName.Substring($source.Length) + ':' + (File-Hash $_.FullName) })
$sourceHash = Text-Hash (($inputs + ('builder:' + (File-Hash $PSCommandPath))) -join "`n")
function Runtime-IsCurrent {
  if (!(Test-Path -LiteralPath $manifestPath -PathType Leaf)) { return $false }
  try {
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.source_hash -ne $sourceHash) { return $false }
    foreach ($name in @('taskhub-runtime.exe', 'settings.json', 'report_styles.json')) {
      $file = Join-Path $resources $name
      if (!(Test-Path -LiteralPath $file -PathType Leaf) -or (File-Hash $file) -ne $manifest.files.$name) { return $false }
    }
    return $true
  } catch { return $false }
}
if ($Ensure -and (Runtime-IsCurrent)) { Write-Output 'Report runtime is up to date.'; exit 0 }

$projectKey = (Text-Hash $project.ToLowerInvariant()).Substring(0, 16)
$cache = Join-Path ([IO.Path]::GetTempPath()) "task-hub-runtime-build\$projectKey"
$environment = Join-Path $cache 'python-env'
$runtimePython = Join-Path $environment 'Scripts\python.exe'
$mutex = New-Object Threading.Mutex($false, "Local\TaskHubRuntime-$projectKey")
$ownsMutex = $false
$oldNoUserSite = $env:PYTHONNOUSERSITE
$oldUserBase = $env:PYTHONUSERBASE
try {
  try { $ownsMutex = $mutex.WaitOne([TimeSpan]::FromMinutes(5)) }
  catch [Threading.AbandonedMutexException] { $ownsMutex = $true }
  if (!$ownsMutex) { throw 'Another report-runtime build is still running. Wait for it to finish and retry.' }
  if ($Ensure -and (Runtime-IsCurrent)) { Write-Output 'Report runtime is up to date.'; exit 0 }
  $env:PYTHONNOUSERSITE = '1'
  $env:PYTHONUSERBASE = Join-Path $cache 'python-user-base'
  New-Item -ItemType Directory -Force -Path $cache, $resources | Out-Null

  if (!(Test-Path -LiteralPath $runtimePython -PathType Leaf)) {
    $candidates = @()
    if ($env:TASK_HUB_RUNTIME_PYTHON) {
      $candidates += @{ command = $env:TASK_HUB_RUNTIME_PYTHON; arguments = @() }
    } else {
      $launcher = Get-Command py -ErrorAction SilentlyContinue
      if ($launcher) { $candidates += @{ command = $launcher.Source; arguments = @('-3.12') } }
      $python = Get-Command python -ErrorAction SilentlyContinue
      if ($python -and $python.Source -notmatch 'WindowsApps') { $candidates += @{ command = $python.Source; arguments = @() } }
    }
    $basePython = $null
    foreach ($candidate in $candidates) {
      try {
        $command = $candidate.command
        $prefix = $candidate.arguments
        $found = & $command @prefix -I -c 'import sys; assert sys.version_info[:2] == (3,12); print(sys.executable)' 2>$null
        if ($LASTEXITCODE -eq 0 -and $found -and (Test-Path -LiteralPath ([string]$found))) { $basePython = [string]$found; break }
      } catch { continue }
    }
    if (!$basePython) { throw 'Python 3.12 is required to build Task Hub from source. Install Python 3.12 with its Windows launcher, or set TASK_HUB_RUNTIME_PYTHON to its python.exe, then run npm run desktop again. Installed-app users do not need Python.' }
    Write-Output 'Creating an isolated Python environment for the report engine...'
    & $basePython -I -m venv $environment
    if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $runtimePython)) { throw 'Could not create the report build environment. Check Python 3.12 and write access to your temporary folder.' }
  }

  $dependencyStamp = Join-Path $cache 'dependencies.sha256'
  $requirementsHash = File-Hash $requirements
  $probe = @'
import importlib.metadata as metadata, sys
from pathlib import Path
for line in Path(sys.argv[1]).read_text().splitlines():
    if not line.strip() or line.lstrip().startswith('#'): continue
    package, version = line.strip().split('==', 1)
    if metadata.version(package) != version: raise SystemExit(1)
import PyInstaller, openpyxl
'@
  $dependenciesReady = $false
  try {
    if ((Test-Path -LiteralPath $dependencyStamp) -and (Get-Content -LiteralPath $dependencyStamp -Raw).Trim() -eq $requirementsHash) {
      & $runtimePython -I -c $probe $requirements 2>$null
      $dependenciesReady = $LASTEXITCODE -eq 0
    }
  } catch { $dependenciesReady = $false }
  if (!$dependenciesReady) {
    Write-Output 'Installing the pinned report build packages (first setup requires internet)...'
    & $runtimePython -I -m pip install --disable-pip-version-check -r $requirements
    if ($LASTEXITCODE -ne 0) { throw 'Could not install the report build packages. See the pip error above; check internet/proxy access, then run npm run desktop again. Your normal Python environment has not been changed.' }
    & $runtimePython -I -c $probe $requirements
    if ($LASTEXITCODE -ne 0) { throw 'The report build packages could not be verified. See the Python error above.' }
    Set-Content -LiteralPath $dependencyStamp -Value $requirementsHash -Encoding ASCII
  }

  Write-Output 'Building the bundled report engine...'
  & $runtimePython -I -m PyInstaller --noconfirm --clean --onefile --console --name taskhub-runtime --paths (Join-Path $source 'vendor') --hidden-import engine --distpath $resources --workpath (Join-Path $cache 'build') --specpath $cache (Join-Path $source 'worker.py')
  if ($LASTEXITCODE -ne 0) { throw 'Report runtime compilation failed. See the PyInstaller error above. Close any Task Hub report operation if Windows reports a locked executable, then retry.' }
  Copy-Item -LiteralPath (Join-Path $source 'settings.json'), (Join-Path $source 'report_styles.json') -Destination $resources -Force
  $files = @{}
  foreach ($name in @('taskhub-runtime.exe', 'settings.json', 'report_styles.json')) { $files[$name] = File-Hash (Join-Path $resources $name) }
  @{ source_hash = $sourceHash; files = $files } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
  Write-Output 'Report runtime is ready.'
} finally {
  $env:PYTHONNOUSERSITE = $oldNoUserSite
  $env:PYTHONUSERBASE = $oldUserBase
  if ($ownsMutex) { $mutex.ReleaseMutex() }
  $mutex.Dispose()
}
