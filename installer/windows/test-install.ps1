# Checks the installer as a user would meet it: installs it silently (for the current user, into a
# temporary folder), checks what it did, starts the app, uninstalls it, and checks that everything is
# as before (the PATH, the Run key, the Start menu, Settings > Apps). Exit code 1 on any difference.
#
#   pwsh installer\windows\test-install.ps1 -Setup dist\aequalis-0.2.9-x86_64-windows-setup.exe
param([Parameter(Mandatory)][string]$Setup)

$dir = Join-Path $env:TEMP "aequalis-install-test"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{6B0E5F3A-4C1D-4E9B-9C55-2A7E3F8D1B42}_is1"
$runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$shortcut = Join-Path ([Environment]::GetFolderPath("Programs")) "Aequalis.lnk"
$failed = 0
function UserPath { (Get-ItemProperty HKCU:\Environment -Name Path -ErrorAction SilentlyContinue).Path }
function RunValue { (Get-ItemProperty $runKey -Name Aequalis -ErrorAction SilentlyContinue).Aequalis }
function Check([string]$what, [bool]$ok) {
    "{0}  {1}" -f ($(if ($ok) { "ok  " } else { "FAIL" }), $what)
    if (-not $ok) { $script:failed++ }
}

$pathBefore = UserPath
$runBefore = RunValue

"== install"
Start-Process $Setup -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/DIR=`"$dir`"", "/TASKS=autostart,path" -Wait
Check "the programs are installed" ((Test-Path "$dir\aequalis-tray.exe") -and (Test-Path "$dir\aequalis.exe"))
Check "it starts with Windows" ((RunValue) -eq "`"$dir\aequalis-tray.exe`" --autostart")
Check "the command line is on the PATH" ((UserPath) -split ';' -contains $dir)
Check "a Start menu entry" (Test-Path $shortcut)
$app = Get-ItemProperty $uninstallKey -ErrorAction SilentlyContinue
Check "in Settings > Apps: $($app.DisplayName) $($app.DisplayVersion)" ($app.DisplayName -eq "Aequalis")
Check "the app's file says Aequalis" ((Get-Item "$dir\aequalis-tray.exe").VersionInfo.FileDescription -eq "Aequalis")
Check "the command line answers: $(& "$dir\aequalis.exe" 10 km mi)" ((& "$dir\aequalis.exe" 10 km mi) -eq "6.2137 mi")
Start-Process "$dir\aequalis-tray.exe" -ArgumentList "--autostart"
Start-Sleep -Seconds 2
Check "the app runs" ([bool](Get-Process aequalis-tray -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$dir*" }))

"== uninstall"
Start-Process "$dir\unins000.exe" -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART" -Wait
Start-Sleep -Seconds 2
Check "the app is stopped" (-not (Get-Process aequalis-tray -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$dir*" }))
Check "the programs are gone" (-not (Test-Path "$dir\aequalis-tray.exe"))
Check "the Run key is as before" ((RunValue) -eq $runBefore)
Check "the PATH is as before" ((UserPath) -eq $pathBefore)
Check "no Start menu entry" (-not (Test-Path $shortcut))
Check "not in Settings > Apps" (-not (Test-Path $uninstallKey))
Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue

if ($failed) { "$failed check(s) failed"; exit 1 }
"all checks passed"
