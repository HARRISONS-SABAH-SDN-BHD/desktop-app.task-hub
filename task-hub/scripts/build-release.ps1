param([string]$SigningKeyPath=$env:TASK_HUB_SIGNING_KEY_PATH)
$ErrorActionPreference='Stop'
# Import only Desktop OAuth registration metadata. Never copy the JSON into source.
if ($env:TASK_HUB_GOOGLE_CREDENTIALS_PATH) {
  $googleClient = Get-Content -LiteralPath $env:TASK_HUB_GOOGLE_CREDENTIALS_PATH -Raw | ConvertFrom-Json
  if (!$googleClient.installed -or !$googleClient.installed.client_id -or !$googleClient.installed.client_secret) { throw 'Expected Google Desktop app credentials JSON.' }
  $env:TASK_HUB_GOOGLE_CLIENT_ID = $googleClient.installed.client_id
  $env:TASK_HUB_GOOGLE_CLIENT_SECRET = $googleClient.installed.client_secret
  $googleClient = $null
}
if (!$env:TASK_HUB_GOOGLE_CLIENT_ID -or !$env:TASK_HUB_GOOGLE_CLIENT_ID.EndsWith('.apps.googleusercontent.com')) { throw 'Configure Task Hub Google Desktop OAuth credentials before building a release. See config/Google-signin-setup.txt.' }
if ($SigningKeyPath) {
  if (!(Test-Path -LiteralPath $SigningKeyPath -PathType Leaf)) { throw 'Updater signing key file not found.' }
  $env:TAURI_SIGNING_PRIVATE_KEY=(Resolve-Path -LiteralPath $SigningKeyPath).Path
}
if (!$env:TAURI_SIGNING_PRIVATE_KEY) { throw 'Set TASK_HUB_SIGNING_KEY_PATH to the private updater key, or provide TAURI_SIGNING_PRIVATE_KEY through a GitHub Actions secret. Keep the key outside the project.' }
if (!$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD='' }
npm run runtime:ensure
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
npm run desktop:build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
npm run release:prepare
exit $LASTEXITCODE
