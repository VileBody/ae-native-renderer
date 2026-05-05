#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import json
import os
import ssl
import subprocess
import sys
import time
import urllib.request
from datetime import datetime
from pathlib import Path

import winrm


DEFAULT_IAC_ENV = Path("/Users/ergin/Desktop/blast_mj_final/.env.iac")
DEFAULT_S3_ENV = Path("/Users/ergin/Desktop/blast_mj_final/.env")
DEFAULT_NODE = "http://85.239.48.31:8000"
DEFAULT_WINRM = "http://85.239.48.31:5985/wsman"
DEFAULT_SERVER_ID = "6849259"
DEFAULT_PACK = Path("fixtures/ae_probe_pack/shadow_blur_discriminator")
DEFAULT_CASES = ["SHBL_SOFT_001", "SHBL_SOFT_008", "SHBL_SOFT_018", "SHBL_SOFT_032"]


REMOTE_TRACER = r'''
from __future__ import annotations

import argparse
import json
import sys
import time

import frida


STD_OPTIONS = "?StandardOptions@BoxBlurOptions@GF@@SA?AU12@_N00HMM@Z"
FAST_BOX_BLUR = "?FastBoxBlur@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXHHHHHPEAXHHHHHUPixelFormat@dvamediatypes@@AEBUBoxBlurOptions@1@@Z"
SET_ALPHA_ONLY = "?SetBlurAlphaChannelOnly@BoxBlurOptions@GF@@QEAAXXZ"

JS = r"""
const maxEvents = MAX_EVENTS_PLACEHOLDER;
const processInfo = PROCESS_INFO_PLACEHOLDER;
let eventCount = 0;

function hexptr(p) {
  if (p === null || p === undefined) {
    return null;
  }
  return ptr(p).toString();
}

function safeReadU32(p) {
  try { return ptr(p).readU32(); } catch (e) { return null; }
}

function safeReadS32(p) {
  try { return ptr(p).readS32(); } catch (e) { return null; }
}

function safeReadFloat(p) {
  try { return ptr(p).readFloat(); } catch (e) { return null; }
}

function dumpBoxBlurOptions(p) {
  const q = ptr(p);
  return {
    ptr: q.toString(),
    flags: safeReadU32(q.add(0x00)),
    src_alpha_type: safeReadU32(q.add(0x04)),
    dest_alpha_type: safeReadU32(q.add(0x08)),
    iterations: safeReadS32(q.add(0x0c)),
    radius_h: safeReadFloat(q.add(0x10)),
    radius_v: safeReadFloat(q.add(0x14)),
    using_v2: safeReadU32(q.add(0x18)) & 0xff,
    force_v2_vertical: (safeReadU32(q.add(0x18)) >>> 8) & 0xff,
    using_16_bit_compute: (safeReadU32(q.add(0x18)) >>> 16) & 0xff
  };
}

function moduleOffset(addr) {
  const m = Process.findModuleByAddress(ptr(addr));
  if (m === null) {
    return {module: null, offset: null, addr: ptr(addr).toString()};
  }
  return {module: m.name, offset: ptr(addr).sub(m.base).toString(), addr: ptr(addr).toString()};
}

function backtrace(ctx) {
  return Thread.backtrace(ctx, Backtracer.ACCURATE).slice(0, 24).map(moduleOffset);
}

function hasDropShadowFrame(ctx) {
  const drop = Process.findModuleByName("Drop_Shadow.aex");
  if (drop === null) {
    return false;
  }
  const start = drop.base;
  const end = drop.base.add(drop.size);
  return Thread.backtrace(ctx, Backtracer.ACCURATE).some(function (a) {
    return ptr(a).compare(start) >= 0 && ptr(a).compare(end) < 0;
  });
}

function emit(kind, payload) {
  if (eventCount >= maxEvents) {
    return;
  }
  eventCount += 1;
  send(Object.assign({
    kind: kind,
    seq: eventCount,
    timestamp_ms: Date.now(),
    process: processInfo
  }, payload));
}

function hookByExport(moduleName, exportName, kind, install) {
  const module = Process.findModuleByName(moduleName);
  if (module === null) {
    send({kind: "hook_missing_module", module: moduleName, export_name: exportName});
    return;
  }
  const exp = module.enumerateExports().find(function (candidate) {
    return candidate.name === exportName;
  });
  const address = exp === undefined ? null : exp.address;
  if (address === null) {
    send({kind: "hook_missing", module: moduleName, export_name: exportName});
    return;
  }
  install(address);
  send({kind: "hook_installed", hook: kind, module: moduleName, export_name: exportName, address: address.toString()});
}

hookByExport("GPUFoundation.DLL", STD_OPTIONS_PLACEHOLDER, "standard_options", function (address) {
  Interceptor.attach(address, {
    onEnter: function (args) {
      this.capture = true;
      this.retbuf = ptr(args[0]);
      emit("standard_options_enter", {
        has_drop_shadow_frame: hasDropShadowFrame(this.context),
        retbuf: this.retbuf.toString(),
        bool_a: ptr(args[1]).toInt32() & 0xff,
        bool_b: ptr(args[2]).toInt32() & 0xff,
        bool_c: ptr(args[3]).toInt32() & 0xff,
        iterations_or_alpha: this.context.rsp.add(0x28).readS32(),
        radius_h_arg: this.context.rsp.add(0x30).readFloat(),
        radius_v_arg: this.context.rsp.add(0x38).readFloat(),
        backtrace: backtrace(this.context)
      });
    },
    onLeave: function () {
      if (!this.capture) {
        return;
      }
      emit("standard_options_leave", {
        retbuf: this.retbuf.toString(),
        options: dumpBoxBlurOptions(this.retbuf)
      });
    }
  });
});

hookByExport("GPUFoundation.DLL", SET_ALPHA_ONLY_PLACEHOLDER, "set_alpha_only", function (address) {
  Interceptor.attach(address, {
    onEnter: function (args) {
      this.capture = true;
      this.options = ptr(args[0]);
      emit("set_alpha_only_enter", {
        has_drop_shadow_frame: hasDropShadowFrame(this.context),
        options: dumpBoxBlurOptions(this.options),
        backtrace: backtrace(this.context)
      });
    },
    onLeave: function () {
      if (!this.capture) {
        return;
      }
      emit("set_alpha_only_leave", {
        options: dumpBoxBlurOptions(this.options)
      });
    }
  });
});

hookByExport("GPUFoundation.DLL", FAST_BOX_BLUR_PLACEHOLDER, "fast_box_blur", function (address) {
  Interceptor.attach(address, {
    onEnter: function (args) {
      const rsp = this.context.rsp;
      const optionsPtr = rsp.add(0x78).readPointer();
      emit("fast_box_blur_enter", {
        has_drop_shadow_frame: hasDropShadowFrame(this.context),
        p3: ptr(args[2]).toInt32(),
        p4: ptr(args[3]).toInt32(),
        p5: rsp.add(0x28).readS32(),
        p6: rsp.add(0x30).readS32(),
        p7: rsp.add(0x38).readS32(),
        p9: rsp.add(0x48).readS32(),
        p10: rsp.add(0x50).readS32(),
        p11: rsp.add(0x58).readS32(),
        p12: rsp.add(0x60).readS32(),
        p13: rsp.add(0x68).readS32(),
        pixel_format_or_p14: rsp.add(0x70).readU32(),
        options: dumpBoxBlurOptions(optionsPtr),
        backtrace: backtrace(this.context)
      });
    }
  });
});
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--duration", type=float, default=90.0)
    ap.add_argument("--max-events", type=int, default=80)
    args = ap.parse_args()

    device = frida.get_local_device()
    procs = [p for p in device.enumerate_processes() if p.name.lower() == "afterfx.exe"]
    if not procs:
      raise RuntimeError("AfterFX.exe is not running")

    script_text = JS.replace("MAX_EVENTS_PLACEHOLDER", str(args.max_events))
    script_text = script_text.replace("STD_OPTIONS_PLACEHOLDER", json.dumps(STD_OPTIONS))
    script_text = script_text.replace("FAST_BOX_BLUR_PLACEHOLDER", json.dumps(FAST_BOX_BLUR))
    script_text = script_text.replace("SET_ALPHA_ONLY_PLACEHOLDER", json.dumps(SET_ALPHA_ONLY))

    with open(args.out, "a", encoding="utf-8") as f:
        f.write(json.dumps({"kind": "trace_start", "duration": args.duration}) + "\n")
        f.flush()

        def on_message(message, data):
            f.write(json.dumps(message, ensure_ascii=False) + "\n")
            f.flush()

        attached = {}

        def should_attach(proc):
            name = proc.name.lower()
            return name in {"afterfx.exe", "afterfx.com", "aerender.exe"}

        def attach(proc):
            if proc.pid in attached:
                return
            try:
                session = device.attach(proc.pid)
                proc_info = {"pid": proc.pid, "name": proc.name}
                per_process_script = script_text.replace(
                    "PROCESS_INFO_PLACEHOLDER",
                    json.dumps(proc_info),
                )
                script = session.create_script(per_process_script)
                script.on("message", on_message)
                script.load()
                attached[proc.pid] = session
                f.write(json.dumps({"kind": "trace_attached", **proc_info}) + "\n")
                f.flush()
            except Exception as exc:
                f.write(
                    json.dumps(
                        {
                            "kind": "trace_attach_error",
                            "pid": proc.pid,
                            "name": proc.name,
                            "error": repr(exc),
                        }
                    )
                    + "\n"
                )
                f.flush()

        deadline = time.time() + args.duration
        while time.time() < deadline:
            for proc in device.enumerate_processes():
                if should_attach(proc):
                    attach(proc)
            time.sleep(0.5)
        for session in list(attached.values()):
            try:
                session.detach()
            except Exception:
                pass
        f.write(json.dumps({"kind": "trace_stop"}) + "\n")
        f.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
'''


def load_env(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.is_file():
        return values
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" not in line or line.strip().startswith("#"):
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip().strip('"').strip("'")
    return values


def load_s3_env_file(path: Path) -> None:
    values = load_env(path)
    for key, value in values.items():
        os.environ.setdefault(key, value)
    if os.getenv("S3_ACCESS_KEY_ID") and not os.getenv("AWS_ACCESS_KEY_ID"):
        os.environ["AWS_ACCESS_KEY_ID"] = os.getenv("S3_ACCESS_KEY_ID", "")
    if os.getenv("S3_SECRET_ACCESS_KEY") and not os.getenv("AWS_SECRET_ACCESS_KEY"):
        os.environ["AWS_SECRET_ACCESS_KEY"] = os.getenv("S3_SECRET_ACCESS_KEY", "")


def s3_client():
    import boto3

    endpoint = os.getenv("S3_ENDPOINT_URL") or None
    region = os.getenv("AWS_DEFAULT_REGION") or os.getenv("AWS_REGION") or os.getenv("S3_REGION") or "ru-1"
    return boto3.session.Session().client("s3", endpoint_url=endpoint, region_name=region)


def resolve_s3_bucket(explicit: str) -> str:
    bucket = explicit.strip() or os.getenv("S3_BUCKET_JOB_ARTIFACTS") or os.getenv("S3_BUCKET_ASSET_STORAGE") or ""
    if not bucket:
        raise RuntimeError("No S3 bucket: pass --bucket or set S3_BUCKET_JOB_ARTIFACTS")
    return bucket


def upload_s3_text(*, bucket: str, key: str, text: str, content_type: str) -> None:
    s3_client().put_object(
        Bucket=bucket,
        Key=key,
        Body=text.encode("utf-8"),
        ContentType=content_type,
    )


def download_s3_text(*, bucket: str, key: str) -> str:
    body = s3_client().get_object(Bucket=bucket, Key=key)["Body"].read()
    return body.decode("utf-8", errors="replace")


def presign_s3_get(*, bucket: str, key: str, expires_s: int = 3600) -> str:
    return s3_client().generate_presigned_url(
        "get_object",
        Params={"Bucket": bucket, "Key": key},
        ExpiresIn=expires_s,
    )


def presign_s3_put(*, bucket: str, key: str, expires_s: int = 3600) -> str:
    return s3_client().generate_presigned_url(
        "put_object",
        Params={"Bucket": bucket, "Key": key},
        ExpiresIn=expires_s,
    )


def timeweb_root_password(env_path: Path, server_id: str) -> str:
    token = load_env(env_path).get("TWC_TOKEN") or os.environ.get("TWC_TOKEN", "")
    if not token:
        raise RuntimeError(f"TWC_TOKEN not found in {env_path}")
    req = urllib.request.Request(
        f"https://api.timeweb.cloud/api/v1/servers/{server_id}",
        headers={"Authorization": f"Bearer {token}"},
    )
    ctx = ssl._create_unverified_context()
    with urllib.request.urlopen(req, timeout=30, context=ctx) as resp:
        payload = json.loads(resp.read().decode("utf-8"))
    password = payload["server"]["root_pass"]
    if not isinstance(password, str) or not password:
        raise RuntimeError("Timeweb response did not include root_pass")
    return password


def run_ps(session: winrm.Session, script: str, *, check: bool = True) -> str:
    result = session.run_ps("$ProgressPreference='SilentlyContinue'\n" + script)
    stdout = result.std_out.decode("utf-8", errors="replace")
    stderr = result.std_err.decode("utf-8", errors="replace")
    if check and result.status_code != 0:
        raise RuntimeError(f"PowerShell failed ({result.status_code})\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}")
    return stdout


def ps_quote(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def upload_text(session: winrm.Session, remote_path: str, text: str) -> None:
    encoded = base64.b64encode(text.encode("utf-8")).decode("ascii")
    remote_b64 = remote_path + ".b64"
    run_ps(
        session,
        "\n".join(
            [
                f"$path = {ps_quote(remote_path)}",
                "New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null",
                f"if (Test-Path {ps_quote(remote_b64)}) {{ Remove-Item -Path {ps_quote(remote_b64)} -Force }}",
            ]
        ),
    )
    for idx in range(0, len(encoded), 500):
        chunk = encoded[idx : idx + 500]
        run_ps(
            session,
            f"Add-Content -Path {ps_quote(remote_b64)} -Value {ps_quote(chunk)} -NoNewline -Encoding ASCII",
        )
    run_ps(
        session,
        "\n".join(
            [
                f"$b64 = Get-Content -Raw -Path {ps_quote(remote_b64)}",
                f"[IO.File]::WriteAllBytes({ps_quote(remote_path)}, [Convert]::FromBase64String($b64))",
                f"Remove-Item -Path {ps_quote(remote_b64)} -Force",
            ]
        ),
    )


def install_remote_script_from_url(session: winrm.Session, remote_path: str, url: str) -> None:
    run_ps(
        session,
        "\n".join(
            [
                f"$path = {ps_quote(remote_path)}",
                "New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null",
                f"Invoke-WebRequest -Uri {ps_quote(url)} -OutFile $path -UseBasicParsing | Out-Null",
            ]
        ),
    )


def upload_remote_file_to_url(session: winrm.Session, remote_path: str, url: str) -> None:
    run_ps(
        session,
        "\n".join(
            [
                f"$path = {ps_quote(remote_path)}",
                "if (-not (Test-Path $path)) { exit 2 }",
                f"Invoke-WebRequest -Method Put -Uri {ps_quote(url)} -InFile $path -UseBasicParsing | Out-Null",
            ]
        ),
    )


def start_remote_trace(session: winrm.Session, remote_script: str, remote_log: str, duration: int, max_events: int) -> int:
    stdout_log = remote_log + ".stdout.txt"
    stderr_log = remote_log + ".stderr.txt"
    ps = "\n".join(
        [
            "$old = Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*ae_trace_drop_shadow_remote.py*' }",
            "foreach ($p in $old) { Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue }",
            f"if (Test-Path {ps_quote(remote_log)}) {{ Remove-Item -Path {ps_quote(remote_log)} -Force }}",
            f"if (Test-Path {ps_quote(stdout_log)}) {{ Remove-Item -Path {ps_quote(stdout_log)} -Force }}",
            f"if (Test-Path {ps_quote(stderr_log)}) {{ Remove-Item -Path {ps_quote(stderr_log)} -Force }}",
            "$argsList = @(",
            f"  '-u', {ps_quote(remote_script)},",
            f"  '--out', {ps_quote(remote_log)},",
            f"  '--duration', '{duration}',",
            f"  '--max-events', '{max_events}'",
            ")",
            "$p = Start-Process -FilePath 'python' -ArgumentList $argsList -WindowStyle Hidden -PassThru "
            f"-RedirectStandardOutput {ps_quote(stdout_log)} -RedirectStandardError {ps_quote(stderr_log)}",
            "$p.Id",
        ]
    )
    return int(run_ps(session, ps).strip().splitlines()[-1])


def fetch_remote_text(session: winrm.Session, remote_path: str) -> str:
    ps = "\n".join(
        [
            f"$path = {ps_quote(remote_path)}",
            "if (-not (Test-Path $path)) { exit 2 }",
            "$bytes = [IO.File]::ReadAllBytes($path)",
            "[Convert]::ToBase64String($bytes)",
        ]
    )
    encoded = run_ps(session, ps).strip()
    return base64.b64decode(encoded).decode("utf-8", errors="replace")


def fetch_remote_text_if_exists(session: winrm.Session, remote_path: str) -> str:
    try:
        return fetch_remote_text(session, remote_path)
    except Exception:
        return ""


def run_case(case_id: str, job_id: str, node: str) -> None:
    subprocess.run(
        [
            sys.executable,
            "scripts/ae_remote_pack.py",
            str(DEFAULT_PACK),
            "--entry-script",
            "jsx/build_shadow_blur_discriminator_project.jsx",
            "--node",
            node,
            "--job-id",
            job_id,
            "--case",
            case_id,
            "--poll-interval-s",
            "5",
            "--timeout-s",
            "900",
        ],
        check=True,
    )


def parse_trace(text: str) -> list[dict[str, object]]:
    events = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict) and payload.get("type") == "send" and isinstance(payload.get("payload"), dict):
            events.append(payload["payload"])
        elif isinstance(payload, dict):
            events.append(payload)
    return events


def summarize_case(case_id: str, events: list[dict[str, object]]) -> dict[str, object]:
    std = [e for e in events if e.get("kind") == "standard_options_enter"]
    leave = [e for e in events if e.get("kind") == "standard_options_leave"]
    fast = [e for e in events if e.get("kind") == "fast_box_blur_enter"]
    alpha = [e for e in events if e.get("kind") == "set_alpha_only_leave"]
    return {
        "case_id": case_id,
        "standard_options_enter": std[:4],
        "standard_options_leave": leave[:4],
        "set_alpha_only_leave": alpha[:4],
        "fast_box_blur_enter": fast[:4],
        "event_count": len(events),
    }


def main() -> int:
    ap = argparse.ArgumentParser("Trace AE Drop Shadow softness blur options through Frida on the 85 node.")
    ap.add_argument("--iac-env", default=str(DEFAULT_IAC_ENV))
    ap.add_argument("--server-id", default=DEFAULT_SERVER_ID)
    ap.add_argument("--winrm", default=DEFAULT_WINRM)
    ap.add_argument("--node", default=DEFAULT_NODE)
    ap.add_argument("--s3-env", default=str(DEFAULT_S3_ENV))
    ap.add_argument("--bucket", default="")
    ap.add_argument("--prefix", default="ae_dynamic_traces/drop_shadow_softness")
    ap.add_argument("--case", action="append", default=[])
    ap.add_argument("--duration", type=int, default=90)
    ap.add_argument("--max-events", type=int, default=80)
    ap.add_argument("--out-dir", default="")
    args = ap.parse_args()

    case_ids = args.case or DEFAULT_CASES
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    out_dir = Path(args.out_dir or f"target/dynamic_tools_85/drop_shadow_softness_{stamp}").resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    load_s3_env_file(Path(args.s3_env).expanduser())
    bucket = resolve_s3_bucket(args.bucket)
    prefix = args.prefix.strip("/").strip()
    s3_root = f"{prefix}/{stamp}" if prefix else stamp

    password = timeweb_root_password(Path(args.iac_env).expanduser(), args.server_id)
    session = winrm.Session(args.winrm, auth=("Administrator", password), transport="ntlm")
    remote_root = f"C:\\ae_dev\\frida_traces\\drop_shadow_softness_{stamp}"
    remote_script = remote_root + "\\ae_trace_drop_shadow_remote.py"
    tracer_key = f"{s3_root}/ae_trace_drop_shadow_remote.py"
    upload_s3_text(bucket=bucket, key=tracer_key, text=REMOTE_TRACER, content_type="text/x-python")
    install_remote_script_from_url(
        session,
        remote_script,
        presign_s3_get(bucket=bucket, key=tracer_key, expires_s=3600),
    )

    summaries = []
    for case_id in case_ids:
        remote_log = remote_root + f"\\{case_id}.jsonl"
        pid = start_remote_trace(session, remote_script, remote_log, args.duration, args.max_events)
        time.sleep(3.0)
        job_id = f"drop_shadow_soft_trace_{case_id}_{stamp}"
        try:
            run_case(case_id, job_id, args.node)
        finally:
            run_ps(session, f"Wait-Process -Id {pid} -Timeout 8", check=False)
            run_ps(session, f"Stop-Process -Id {pid} -Force -ErrorAction SilentlyContinue", check=False)

        log_key = f"{s3_root}/{case_id}.jsonl"
        try:
            upload_remote_file_to_url(
                session,
                remote_log,
                presign_s3_put(bucket=bucket, key=log_key, expires_s=3600),
            )
        except Exception as exc:
            stdout = fetch_remote_text_if_exists(session, remote_log + ".stdout.txt")
            stderr = fetch_remote_text_if_exists(session, remote_log + ".stderr.txt")
            raise RuntimeError(
                f"remote Frida trace did not produce/upload log for {case_id}\n"
                f"upload error: {exc}\n"
                f"remote stdout:\n{stdout}\n"
                f"remote stderr:\n{stderr}"
            ) from exc
        trace_text = download_s3_text(bucket=bucket, key=log_key)
        local_log = out_dir / f"{case_id}.jsonl"
        local_log.write_text(trace_text, encoding="utf-8")
        summary = summarize_case(case_id, parse_trace(trace_text))
        summary["s3_key"] = log_key
        summaries.append(summary)
        print(json.dumps({"event": "case_done", "case_id": case_id, "log": str(local_log), "summary": summary}, ensure_ascii=False))

    summary_path = out_dir / "summary.json"
    summary_path.write_text(
        json.dumps(
            {
                "s3": {"bucket": bucket, "root": s3_root, "tracer_key": tracer_key},
                "cases": summaries,
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf-8",
    )
    print(json.dumps({"event": "done", "summary": str(summary_path)}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
