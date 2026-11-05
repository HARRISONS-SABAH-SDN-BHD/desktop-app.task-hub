$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$parameters = $env:TASK_HUB_PARAMETERS | ConvertFrom-Json
Write-Output $parameters.label
Write-Output ('Generated at: ' + [DateTime]::UtcNow.ToString('o'))
foreach ($section in $parameters.sections) {
    switch ($section) {
        'Operating system' { Write-Output ('Operating system: ' + [Environment]::OSVersion.VersionString) }
        'Runtime' { Write-Output ('PowerShell: ' + $PSVersionTable.PSVersion.ToString()) }
        'Working directory' { Write-Output ('Working directory: ' + (Get-Location).Path) }
    }
}
