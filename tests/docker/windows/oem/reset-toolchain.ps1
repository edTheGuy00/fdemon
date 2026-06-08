# tests/docker/windows/oem/reset-toolchain.ps1
#
# Reset the Flutter/Android toolchain state INSIDE the Windows guest so the
# install wizard can be re-tested from a clean toolchain WITHOUT reinstalling
# Windows (much faster than `win-vm.sh fresh`). Use `fresh` only when you need a
# pristine OS (registry/Store-shim/first-run experience).
#
# Shipped into the image via the /oem folder → copied to C:\OEM, and staged to
# C:\fdemon\reset-toolchain.ps1 by install.bat.
#
# Run in the guest (PowerShell), then RESTART fdemon and open a NEW terminal:
#   powershell -ExecutionPolicy Bypass -File C:\fdemon\reset-toolchain.ps1          # prompts
#   powershell -ExecutionPolicy Bypass -File C:\fdemon\reset-toolchain.ps1 -Force   # no prompt
#
# What it does (best-effort, throwaway-VM-safe):
#   - removes the managed Flutter SDK (~\fvm) and the Android SDK ($ANDROID_HOME + defaults)
#   - winget-uninstalls git and a JDK (so the Prerequisites step has work again)
#   - clears fdemon's user-PATH additions (flutter/Android/cmdline-tools/platform-tools/fvm)
#     and removes ANDROID_HOME from HKCU:\Environment
#   - broadcasts WM_SETTINGCHANGE so new shells see the cleaned PATH
[CmdletBinding()]
param([switch]$Force)

$ErrorActionPreference = 'Continue'

function Step($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }

if (-not $Force) {
  Write-Host "This removes the installed Flutter/Android SDKs and uninstalls git + JDK in THIS Windows guest."
  $ans = Read-Host "Proceed? (y/N)"
  if ($ans -notmatch '^[Yy]') { Write-Host "Aborted."; exit 1 }
}

# 1. Stop a running fdemon so files aren't locked.
Step "stopping fdemon (if running)"
Get-Process fdemon -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

# 2. Remove SDK directories.
$androidHome = [Environment]::GetEnvironmentVariable('ANDROID_HOME','User')
$dirs = @(
  (Join-Path $env:USERPROFILE 'fvm'),                              # managed Flutter (default install root)
  (Join-Path $env:USERPROFILE 'flutter'),                          # common manual layout
  $androidHome,
  (Join-Path $env:LOCALAPPDATA 'Android\Sdk'),                     # default Android SDK root
  (Join-Path $env:USERPROFILE '.android\sdk')
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -Unique
foreach ($d in $dirs) {
  Step "removing $d"
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $d
}

# 3. Uninstall git + a JDK via winget (best-effort).
Step "winget uninstall git + JDK (best-effort)"
winget uninstall --id Git.Git -e --silent --disable-interactivity 2>$null | Out-Null
foreach ($jdk in @('EclipseAdoptium.Temurin.17.JDK','Microsoft.OpenJDK.17','Oracle.JDK.17')) {
  winget uninstall --id $jdk -e --silent --disable-interactivity 2>$null | Out-Null
}

# 4. Clean the user PATH (registry) of fdemon's additions, and drop ANDROID_HOME.
Step "cleaning HKCU:\Environment (PATH + ANDROID_HOME)"
$envKey = 'HKCU:\Environment'
$drop = 'flutter|fvm|Android\\Sdk|\.android|cmdline-tools|platform-tools'
try {
  $rk = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
  if ($rk) {
    $kind = $rk.GetValueKind('Path')                                # preserve REG_EXPAND_SZ vs REG_SZ
    $raw  = $rk.GetValue('Path', '', 'DoNotExpandEnvironmentNames') # unexpanded, keep %VARS%
    $kept = ($raw -split ';' | Where-Object { $_ -and ($_ -notmatch $drop) }) -join ';'
    $rk.SetValue('Path', $kept, $kind)
    $rk.Close()
    Write-Host "    PATH trimmed (kept non-toolchain entries; preserved $kind)"
  }
} catch { Write-Host "    PATH clean skipped: $_" }
Remove-ItemProperty -Path $envKey -Name 'ANDROID_HOME' -ErrorAction SilentlyContinue

# 5. Broadcast WM_SETTINGCHANGE so new shells pick up the cleaned env.
Step "broadcasting WM_SETTINGCHANGE"
try {
  Add-Type -Namespace Win32 -Name NativeMethods -MemberDefinition @"
[System.Runtime.InteropServices.DllImport("user32.dll", SetLastError=true, CharSet=System.Runtime.InteropServices.CharSet.Auto)]
public static extern System.IntPtr SendMessageTimeout(System.IntPtr hWnd, uint Msg, System.UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out System.UIntPtr lpdwResult);
"@
  $res = [UIntPtr]::Zero
  [void][Win32.NativeMethods]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$res)
} catch { Write-Host "    broadcast skipped: $_" }

Write-Host ""
Write-Host "Toolchain reset complete." -ForegroundColor Green
Write-Host "Now: close this terminal, open a NEW one, and relaunch fdemon so it starts with a clean environment:"
Write-Host "    fdemon C:\test-project"
