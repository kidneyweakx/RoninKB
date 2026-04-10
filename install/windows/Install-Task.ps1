# Register the RoninKB daemon as a per-user scheduled task that runs at
# logon. Run this script from an elevated PowerShell prompt if you want it
# to install for all users; otherwise `-TaskName` is created under the
# current user's task folder.
#
# Uninstall:
#     Unregister-ScheduledTask -TaskName "RoninKB Daemon" -Confirm:$false

$ExePath = "C:\Program Files\RoninKB\hhkb-daemon.exe"

if (-not (Test-Path $ExePath)) {
    Write-Warning "hhkb-daemon.exe not found at $ExePath. Copy the binary there before running this script, or edit `$ExePath above."
}

$action = New-ScheduledTaskAction -Execute $ExePath
$trigger = New-ScheduledTaskTrigger -AtLogOn
$settings = New-ScheduledTaskSettingsSet `
    -StartWhenAvailable `
    -DontStopIfGoingOnBatteries `
    -AllowStartIfOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Hours 0)

Register-ScheduledTask `
    -TaskName "RoninKB Daemon" `
    -Action $action `
    -Trigger $trigger `
    -Settings $settings `
    -Description "RoninKB HHKB daemon (HTTP+WS on 127.0.0.1:7331)"
