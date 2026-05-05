#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import json
import re
import subprocess
from datetime import datetime


DEFAULT_SSH_HOST = "ae85"
DEFAULT_REMOTE_DIR = r"C:\ae_dev\screenshots"
DEFAULT_TASK_USER = r"VDSWIN2K22\Administrator"


CLICKER_PS1 = r"""
$ErrorActionPreference = 'Continue'
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32Clicker {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
}
"@

function Click-Point([int]$x, [int]$y) {
  [Win32Clicker]::SetCursorPos($x, $y) | Out-Null
  Start-Sleep -Milliseconds 80
  [Win32Clicker]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 80
  [Win32Clicker]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
}

$deadline = (Get-Date).AddSeconds([int]$env:AE_CONTINUE_SECONDS)
$x = [int]$env:AE_CONTINUE_X
$y = [int]$env:AE_CONTINUE_Y
$intervalMs = [int]$env:AE_CONTINUE_INTERVAL_MS
$count = 0
while ((Get-Date) -lt $deadline) {
  Click-Point $x $y
  $count += 1
  Start-Sleep -Milliseconds $intervalMs
}
"clicked=$count x=$x y=$y"
"""


def safe_tag(value: str) -> str:
    value = re.sub(r"[^A-Za-z0-9_.-]+", "_", value.strip())
    return value[:120] or "click_continue"


def ps_quote(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def run_ps_ssh(host: str, script: str, *, timeout: float | None = None) -> str:
    result = subprocess.run(
        [
            "ssh",
            host,
            "powershell",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "-",
        ],
        input="$ProgressPreference='SilentlyContinue'\n" + script,
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"SSH PowerShell failed ({result.returncode})\nSTDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}"
        )
    return result.stdout


def start_clicker(
    *,
    host: str,
    remote_dir: str,
    task_user: str,
    tag: str,
    seconds: int,
    x: int,
    y: int,
    interval_ms: int,
) -> dict[str, object]:
    ps1_b64 = base64.b64encode(CLICKER_PS1.encode("utf-8-sig")).decode("ascii")
    ps = f"""
$ErrorActionPreference='Stop'
$dir = {ps_quote(remote_dir)}
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$tag = {ps_quote(tag)}
$ps1 = Join-Path $dir ($tag + '.ps1')
$cmd = Join-Path $dir ($tag + '.cmd')
$stdout = Join-Path $dir ($tag + '.stdout.txt')
$stderr = Join-Path $dir ($tag + '.stderr.txt')
$taskName = 'ae_native_continue_' + $tag
[IO.File]::WriteAllBytes($ps1, [Convert]::FromBase64String({ps_quote(ps1_b64)}))
$cmdText = '@echo off' + "`r`n"
$cmdText += 'set "AE_CONTINUE_SECONDS={seconds}"' + "`r`n"
$cmdText += 'set "AE_CONTINUE_X={x}"' + "`r`n"
$cmdText += 'set "AE_CONTINUE_Y={y}"' + "`r`n"
$cmdText += 'set "AE_CONTINUE_INTERVAL_MS={interval_ms}"' + "`r`n"
$cmdText += 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "' + $ps1 + '" > "' + $stdout + '" 2> "' + $stderr + '"' + "`r`n"
$cmdText += 'exit /b %ERRORLEVEL%' + "`r`n"
Set-Content -Path $cmd -Value $cmdText -Encoding ASCII
$action = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument ('/c "' + $cmd + '"')
$trigger = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddMinutes(10))
$principal = New-ScheduledTaskPrincipal -UserId {ps_quote(task_user)} -LogonType Interactive -RunLevel Highest
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal -Force | Out-Null
Start-ScheduledTask -TaskName $taskName
[pscustomobject]@{{tag=$tag; task=$taskName; stdout=$stdout; stderr=$stderr; seconds={seconds}; x={x}; y={y}}} | ConvertTo-Json -Depth 4 -Compress
"""
    return json.loads(run_ps_ssh(host, ps, timeout=40).strip())


def main() -> int:
    ap = argparse.ArgumentParser("Click AE Crash Repair Options Continue in the visible AE85 desktop session.")
    ap.add_argument("--ssh-host", default=DEFAULT_SSH_HOST)
    ap.add_argument("--remote-dir", default=DEFAULT_REMOTE_DIR)
    ap.add_argument("--task-user", default=DEFAULT_TASK_USER)
    ap.add_argument("--tag", default="")
    ap.add_argument("--seconds", type=int, default=180)
    ap.add_argument("--x", type=int, default=1040)
    ap.add_argument("--y", type=int, default=631)
    ap.add_argument("--interval-ms", type=int, default=1200)
    args = ap.parse_args()

    tag = safe_tag(args.tag or f"click_continue_{datetime.now().strftime('%Y%m%d_%H%M%S')}")
    print(
        json.dumps(
            start_clicker(
                host=args.ssh_host,
                remote_dir=args.remote_dir,
                task_user=args.task_user,
                tag=tag,
                seconds=args.seconds,
                x=args.x,
                y=args.y,
                interval_ms=args.interval_ms,
            ),
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
