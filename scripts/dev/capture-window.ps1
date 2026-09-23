# Captures a top-level window (by process name) to a PNG for visual QA.
# Usage: pwsh scripts/dev/capture-window.ps1 -Process moco -Out shot.png [-Title "Mocó"]
param(
  [string]$Process = "moco",
  [string]$Title = "",
  [Parameter(Mandatory = $true)][string]$Out
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class Win {
  public delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

[Win]::SetProcessDPIAware() | Out-Null
$pids = @(Get-Process -Name $Process -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
if ($pids.Count -eq 0) { Write-Error "process '$Process' not running"; exit 1 }

$script:found = [IntPtr]::Zero
$cb = [Win+EnumProc] {
  param($h, $p)
  $wpid = 0
  [Win]::GetWindowThreadProcessId($h, [ref]$wpid) | Out-Null
  if ($pids -contains [int]$wpid -and [Win]::IsWindowVisible($h)) {
    $sb = New-Object System.Text.StringBuilder 256
    [Win]::GetWindowText($h, $sb, 256) | Out-Null
    $t = $sb.ToString()
    if ($t.Length -gt 0 -and ($Title -eq "" -or $t -like "*$Title*")) { $script:found = $h; return $false }
  }
  return $true
}
[Win]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($script:found -eq [IntPtr]::Zero) { Write-Error "no visible window for '$Process'"; exit 1 }

[Win]::ShowWindow($script:found, 9) | Out-Null   # SW_RESTORE
[Win]::SetForegroundWindow($script:found) | Out-Null
Start-Sleep -Milliseconds 400

$r = New-Object Win+RECT
# DWMWA_EXTENDED_FRAME_BOUNDS = 9 (excludes the invisible resize border)
[Win]::DwmGetWindowAttribute($script:found, 9, [ref]$r, [System.Runtime.InteropServices.Marshal]::SizeOf($r)) | Out-Null
$w = $r.Right - $r.Left; $hgt = $r.Bottom - $r.Top
$bmp = New-Object System.Drawing.Bitmap $w, $hgt
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "saved $Out ($w x $hgt)"
