#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import json
import re
import subprocess
from datetime import datetime
from pathlib import Path


DEFAULT_SSH_HOST = "ae85"
DEFAULT_REMOTE_DIR = r"C:\ae_dev\screenshots"
DEFAULT_TASK_USER = r"VDSWIN2K22\Administrator"


CAPTURE_PS1 = r"""
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$out = $env:AE_SCREENSHOT_OUT
"session=$([System.Diagnostics.Process]::GetCurrentProcess().SessionId) user=$([Environment]::UserName) interactive=$([Environment]::UserInteractive)" |
  Out-File -FilePath ($out + '.meta.txt') -Encoding UTF8

$screen = [System.Windows.Forms.Screen]::PrimaryScreen
$bounds = $screen.Bounds
"bounds=$($bounds.X),$($bounds.Y),$($bounds.Width),$($bounds.Height)" |
  Add-Content -Path ($out + '.meta.txt') -Encoding UTF8

$bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($bounds.X, $bounds.Y, 0, 0, $bounds.Size)
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
"""


def safe_tag(value: str) -> str:
    value = re.sub(r"[^A-Za-z0-9_.-]+", "_", value.strip())
    return value[:120] or "screenshot"


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


def scp_from_ssh(host: str, remote_path: str, local_path: Path) -> None:
    local_path.parent.mkdir(parents=True, exist_ok=True)
    scp_path = remote_path.replace("\\", "/")
    result = subprocess.run(
        ["scp", f"{host}:{scp_path}", str(local_path)],
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise subprocess.CalledProcessError(
            result.returncode,
            result.args,
            output=result.stdout,
            stderr=result.stderr,
        )


def capture_interactive_screenshot(
    *,
    host: str,
    remote_dir: str,
    task_user: str,
    tag: str,
    wait_seconds: int,
) -> dict[str, object]:
    ps1_b64 = base64.b64encode(CAPTURE_PS1.encode("utf-8-sig")).decode("ascii")
    ps = f"""
$ErrorActionPreference='Stop'
$dir = {ps_quote(remote_dir)}
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$tag = {ps_quote(tag)}
$ps1 = Join-Path $dir ($tag + '.ps1')
$cmd = Join-Path $dir ($tag + '.cmd')
$out = Join-Path $dir ($tag + '.png')
$stdout = Join-Path $dir ($tag + '.stdout.txt')
$stderr = Join-Path $dir ($tag + '.stderr.txt')
$taskName = 'ae_native_screen_' + $tag
[IO.File]::WriteAllBytes($ps1, [Convert]::FromBase64String({ps_quote(ps1_b64)}))
$cmdText = '@echo off' + "`r`n"
$cmdText += 'set "AE_SCREENSHOT_OUT=' + $out + '"' + "`r`n"
$cmdText += 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "' + $ps1 + '" > "' + $stdout + '" 2> "' + $stderr + '"' + "`r`n"
$cmdText += 'exit /b %ERRORLEVEL%' + "`r`n"
Set-Content -Path $cmd -Value $cmdText -Encoding ASCII
$action = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument ('/c "' + $cmd + '"')
$trigger = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddMinutes(10))
$principal = New-ScheduledTaskPrincipal -UserId {ps_quote(task_user)} -LogonType Interactive -RunLevel Highest
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal -Force | Out-Null
Start-ScheduledTask -TaskName $taskName
Start-Sleep -Seconds {wait_seconds}
$info = Get-ScheduledTaskInfo -TaskName $taskName
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
$files = Get-ChildItem $dir -Filter ($tag + '*') | Select-Object Name,Length,LastWriteTime
[pscustomobject]@{{tag=$tag; out=$out; result=$info.LastTaskResult; files=$files}} | ConvertTo-Json -Depth 5 -Compress
"""
    output = run_ps_ssh(host, ps, timeout=wait_seconds + 40).strip()
    if not output:
        raise RuntimeError("interactive screenshot task returned no JSON")
    payload = json.loads(output)
    if payload.get("result") != 0:
        raise RuntimeError(f"interactive screenshot task failed: {payload}")
    return payload


def main() -> int:
    ap = argparse.ArgumentParser("Capture the visible desktop on the reserved AE 85 node.")
    ap.add_argument("--ssh-host", default=DEFAULT_SSH_HOST)
    ap.add_argument("--remote-dir", default=DEFAULT_REMOTE_DIR)
    ap.add_argument("--task-user", default=DEFAULT_TASK_USER)
    ap.add_argument("--tag", default="")
    ap.add_argument("--out-dir", default="target/ae85_screenshots")
    ap.add_argument("--wait-seconds", type=int, default=6)
    ap.add_argument("--no-download", action="store_true")
    args = ap.parse_args()

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    tag = safe_tag(args.tag or f"interactive_screen_{stamp}")
    if not tag.endswith(stamp) and not args.tag:
        tag = f"{tag}_{stamp}"

    payload = capture_interactive_screenshot(
        host=args.ssh_host,
        remote_dir=args.remote_dir,
        task_user=args.task_user,
        tag=tag,
        wait_seconds=args.wait_seconds,
    )
    remote_png = str(payload["out"])
    result: dict[str, object] = {"remote": payload}
    if not args.no_download:
        out_dir = Path(args.out_dir).resolve()
        local_png = out_dir / f"{tag}.png"
        scp_from_ssh(args.ssh_host, remote_png, local_png)
        remote_base = str(Path(remote_png).with_suffix(""))
        for suffix in [".png.meta.txt", ".stdout.txt", ".stderr.txt"]:
            try:
                scp_from_ssh(args.ssh_host, remote_base + suffix, out_dir / f"{tag}{suffix}")
            except subprocess.CalledProcessError:
                pass
        result["local_png"] = str(local_png)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
