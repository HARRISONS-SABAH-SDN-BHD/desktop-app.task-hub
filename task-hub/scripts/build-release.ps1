param([string]$SigningKeyPath=$env:TASK_HUB_SIGNING_KEY_PATH)
$ErrorActionPreference='Stop'
if ($SigningKeyPath) {
  if (!(Test-Path -LiteralPath $SigningKeyPath -PathType Leaf)) { throw 'Updater signing key file not found.' }
  $env:TAURI_SIGNING_PRIVATE_KEY=(Resolve-Path -LiteralPath $SigningKeyPath).Path
}
if (!$env:TAURI_SIGNING_PRIVATE_KEY) { throw 'Set TASK_HUB_SIGNING_KEY_PATH to the private updater key, or provide TAURI_SIGNING_PRIVATE_KEY through a GitHub Actions secret. Keep the key outside the project.' }
if (!$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD='' }
npm run desktop:build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
npm run release:prepare
exit $LASTEXITCODE
