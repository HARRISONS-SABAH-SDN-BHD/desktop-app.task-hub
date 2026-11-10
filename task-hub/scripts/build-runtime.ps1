param([switch]$Ensure)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$worker=Join-Path $project 'src-tauri\resources\runtime\taskhub-runtime.exe'
if ($Ensure -and (Test-Path -LiteralPath $worker)) {
  $sources=Get-ChildItem -LiteralPath (Join-Path $project 'integrations\runtime') -Recurse -File | Where-Object { $_.Extension -in '.py','.json' }
  $newer=$sources | Where-Object { $_.LastWriteTimeUtc -gt (Get-Item -LiteralPath $worker).LastWriteTimeUtc }
  if (!$newer -and (Test-Path -LiteralPath (Join-Path (Split-Path $worker) 'settings.json')) -and (Test-Path -LiteralPath (Join-Path (Split-Path $worker) 'report_styles.json'))) { exit 0 }
}
$runtimePython=$env:TASK_HUB_RUNTIME_PYTHON
if (!$runtimePython) { $runtimePython='python' }
$work=Join-Path $env:TEMP 'task-hub-runtime-build'
$env:PYTHONNOUSERSITE='1'
$env:PYTHONUSERBASE=Join-Path $work 'python-user-base'
New-Item -ItemType Directory -Force -Path $work,(Split-Path $worker) | Out-Null
& $runtimePython -m PyInstaller --noconfirm --clean --onefile --console --name taskhub-runtime --paths (Join-Path $project 'integrations\runtime\vendor') --hidden-import engine --distpath (Split-Path $worker) --workpath (Join-Path $work 'build') --specpath $work (Join-Path $project 'integrations\runtime\worker.py')
if ($LASTEXITCODE -ne 0) { throw 'Runtime build failed. Install integrations/runtime/requirements-build.txt using your Python environment.' }
Copy-Item -LiteralPath (Join-Path $project 'integrations\runtime\settings.json'),(Join-Path $project 'integrations\runtime\report_styles.json') -Destination (Split-Path $worker) -Force
