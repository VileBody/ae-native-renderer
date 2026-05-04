#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import time
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

import boto3
import requests


DEFAULT_ENV = Path("/Users/ergin/Desktop/blast_mj_final/.env")
DEFAULT_NODE = "http://85.239.48.31:8000"
EXCLUDED_DIRS = {
    ".git",
    "__pycache__",
    "ae_goldens",
    "ae_probe_outputs",
    "logs",
    "logs_resume",
    "metadata_with_slate_20260503_160541_85",
    "metadata",
    "preview",
    "tiff",
    "tiff8",
    "png",
    "png8",
    "work",
}


def load_env_file(path: Path) -> None:
    if not path.is_file():
        return
    for line in path.read_text(encoding="utf-8").splitlines():
        s = line.strip()
        if not s or s.startswith("#") or "=" not in s:
            continue
        key, value = s.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip().strip('"').strip("'"))
    if os.getenv("S3_ACCESS_KEY_ID") and not os.getenv("AWS_ACCESS_KEY_ID"):
        os.environ["AWS_ACCESS_KEY_ID"] = os.getenv("S3_ACCESS_KEY_ID", "")
    if os.getenv("S3_SECRET_ACCESS_KEY") and not os.getenv("AWS_SECRET_ACCESS_KEY"):
        os.environ["AWS_SECRET_ACCESS_KEY"] = os.getenv("S3_SECRET_ACCESS_KEY", "")


def s3_client():
    endpoint = os.getenv("S3_ENDPOINT_URL") or None
    region = os.getenv("AWS_DEFAULT_REGION") or os.getenv("AWS_REGION") or os.getenv("S3_REGION") or "ru-1"
    return boto3.session.Session().client("s3", endpoint_url=endpoint, region_name=region)


def parse_s3_uri(uri: str) -> tuple[str, str]:
    if not uri.startswith("s3://"):
        raise ValueError(f"not an s3 uri: {uri!r}")
    rest = uri[5:]
    bucket, _, key = rest.partition("/")
    if not bucket or not key:
        raise ValueError(f"invalid s3 uri: {uri!r}")
    return bucket, key


def should_skip(path: Path, *, include_fonts: bool) -> bool:
    parts = set(path.parts)
    if path.name == ".DS_Store":
        return True
    if path.name.startswith("ae_outputs_round"):
        return True
    if not include_fonts and "assets" in parts and "fonts" in parts:
        return True
    return any(part in EXCLUDED_DIRS for part in path.parts)


def make_pack_zip(pack_dir: Path, dest: Path, *, include_fonts: bool = False) -> tuple[Path, str, int]:
    pack_dir = pack_dir.resolve()
    if not pack_dir.is_dir():
        raise RuntimeError(f"pack dir not found: {pack_dir}")
    root_name = pack_dir.name
    dest.parent.mkdir(parents=True, exist_ok=True)
    count = 0
    with zipfile.ZipFile(dest, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
        for src in sorted(pack_dir.rglob("*")):
            rel = src.relative_to(pack_dir)
            if should_skip(rel, include_fonts=include_fonts):
                continue
            if src.is_dir():
                continue
            zf.write(src, Path(root_name) / rel)
            count += 1
    if count <= 0:
        raise RuntimeError(f"pack zip would be empty: {pack_dir}")
    return dest, root_name, count


def upload_file(path: Path, *, bucket: str, key: str) -> str:
    s3 = s3_client()
    s3.upload_file(str(path), bucket, key, ExtraArgs={"ContentType": "application/zip"})
    return f"s3://{bucket}/{key}"


def download_uri(uri_or_url: str, dest: Path) -> Path:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if uri_or_url.startswith("s3://"):
        bucket, key = parse_s3_uri(uri_or_url)
        s3_client().download_file(bucket, key, str(dest))
        return dest
    parsed = urlparse(uri_or_url)
    if parsed.scheme not in {"http", "https"}:
        raise RuntimeError(f"cannot download unsupported URI: {uri_or_url!r}")
    with requests.get(uri_or_url, stream=True, timeout=1200) as resp:
        resp.raise_for_status()
        with open(dest, "wb") as f:
            for chunk in resp.iter_content(chunk_size=1024 * 1024):
                if chunk:
                    f.write(chunk)
    return dest


def entry_inside_zip(entry_script: str, *, root_name: str) -> str:
    e = entry_script.replace("\\", "/").lstrip("/")
    if e.startswith(root_name + "/"):
        return e
    return f"{root_name}/{e}"


def submit_pack(node_url: str, payload: dict[str, Any]) -> dict[str, Any]:
    url = node_url.rstrip("/") + "/pack-render"
    resp = requests.post(url, json=payload, timeout=30)
    resp.raise_for_status()
    data = resp.json()
    if not isinstance(data, dict) or not data.get("render_id"):
        raise RuntimeError(f"bad pack-render response: {data!r}")
    return data


def poll_pack(node_url: str, render_id: str, *, interval_s: float, timeout_s: float) -> dict[str, Any]:
    url = node_url.rstrip("/") + f"/pack-render/{render_id}"
    started = time.time()
    while True:
        resp = requests.get(url, timeout=30)
        resp.raise_for_status()
        data = resp.json()
        status = str(data.get("status") or "").lower()
        print(json.dumps({"event": "poll", "status": status, "render_id": render_id}, ensure_ascii=False))
        if status in {"succeeded", "failed"}:
            return data
        if time.time() - started > timeout_s:
            raise TimeoutError(f"pack render timed out after {timeout_s}s render_id={render_id}")
        time.sleep(interval_s)


def convert_tiffs_to_png(root: Path) -> int:
    try:
        from PIL import Image
    except Exception:
        return 0
    converted = 0
    for src in list(root.rglob("*")):
        if not src.is_file() or src.suffix.lower() not in {".tif", ".tiff"}:
            continue
        parts = list(src.parts)
        lower_parts = [p.lower() for p in parts]
        out = src.with_suffix(".png")
        for idx, part in enumerate(lower_parts):
            if part == "tiff":
                parts[idx] = "png"
                out = Path(*parts).with_suffix(".png")
                break
            if part == "tiff8":
                parts[idx] = "png8"
                out = Path(*parts).with_suffix(".png")
                break
        out.parent.mkdir(parents=True, exist_ok=True)
        with Image.open(src) as im:
            im.save(out)
        converted += 1
    return converted


def infer_entry(pack_dir: Path) -> str:
    name = pack_dir.name
    candidates = {
        "ae_conformance_pack": "jsx/build_conformance_project.jsx",
        "glow_shadow": "jsx/build_glow_shadow_probe_project.jsx",
        "minimax": "jsx/build_minimax_probe_project.jsx",
        "turbulent_field": "jsx/build_turbulent_field_probe_project.jsx",
    }
    if name in candidates:
        return candidates[name]
    raise RuntimeError("--entry-script is required for this pack")


def main() -> int:
    ap = argparse.ArgumentParser("AE remote pack runner")
    ap.add_argument("pack", help="Pack directory, e.g. fixtures/ae_conformance_pack")
    ap.add_argument("--entry-script", default="", help="JSX path relative to pack dir")
    ap.add_argument("--node", default=DEFAULT_NODE, help="AE node base URL")
    ap.add_argument("--env", default=str(DEFAULT_ENV), help="Env file with S3 credentials")
    ap.add_argument("--job-id", default="", help="Job id; default generated")
    ap.add_argument("--bucket", default="", help="S3 bucket; default S3_BUCKET_JOB_ARTIFACTS")
    ap.add_argument("--prefix", default="ae_remote_packs", help="S3 key prefix")
    ap.add_argument("--output-dir", default="target/ae_remote", help="Local output root")
    ap.add_argument("--case", action="append", default=[], help="Render only this case id; repeatable")
    ap.add_argument("--output-template", default="TIFF Sequence with Alpha", help="AE output module template")
    ap.add_argument("--include-fonts", action="store_true", help="Include assets/fonts in the uploaded zip")
    ap.add_argument("--no-extract", action="store_true", help="Do not extract downloaded output zip")
    ap.add_argument("--no-local-convert", action="store_true", help="Do not run local TIFF->PNG conversion after extract")
    ap.add_argument("--poll-interval-s", type=float, default=10.0)
    ap.add_argument("--timeout-s", type=float, default=7200.0)
    args = ap.parse_args()

    load_env_file(Path(args.env).expanduser())
    pack_dir = Path(args.pack).expanduser().resolve()
    job_id = args.job_id.strip() or f"{pack_dir.name}_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    bucket = args.bucket.strip() or os.getenv("S3_BUCKET_JOB_ARTIFACTS") or os.getenv("S3_BUCKET_ASSET_STORAGE") or ""
    if not bucket:
        raise RuntimeError("No S3 bucket: pass --bucket or set S3_BUCKET_JOB_ARTIFACTS")

    entry = args.entry_script.strip() or infer_entry(pack_dir)
    out_root = Path(args.output_dir).expanduser().resolve() / job_id
    local_zip = out_root / "input" / f"{job_id}_pack.zip"
    pack_zip, root_name, file_count = make_pack_zip(pack_dir, local_zip, include_fonts=args.include_fonts)

    pack_key = f"{args.prefix.strip('/').strip()}/{job_id}/{pack_zip.name}"
    output_key = f"{args.prefix.strip('/').strip()}/{job_id}/{job_id}_outputs.zip"
    pack_uri = upload_file(pack_zip, bucket=bucket, key=pack_key)

    payload = {
        "job_id": job_id,
        "pack_uri": pack_uri,
        "entry_script": entry_inside_zip(entry, root_name=root_name),
        "output_s3_bucket": bucket,
        "output_s3_key": output_key,
        "output_module_template": args.output_template,
        "case_ids": args.case,
        "skip_preview": True,
        "convert_tiff_to_png": True,
    }

    print(json.dumps({
        "event": "submit",
        "job_id": job_id,
        "node": args.node,
        "pack_uri": pack_uri,
        "files": file_count,
        "entry_script": payload["entry_script"],
        "cases": args.case,
    }, ensure_ascii=False))
    accepted = submit_pack(args.node, payload)
    status = poll_pack(
        args.node,
        str(accepted["render_id"]),
        interval_s=args.poll_interval_s,
        timeout_s=args.timeout_s,
    )
    if not bool(status.get("success")):
        raise RuntimeError(f"remote pack render failed: {status}")

    remote_output = str(status.get("output_s3_uri") or status.get("output_url") or "").strip()
    if not remote_output:
        raise RuntimeError(f"pack render succeeded but returned no output URI: {status}")

    output_zip = out_root / "outputs" / f"{job_id}_outputs.zip"
    download_uri(remote_output, output_zip)
    extracted_dir = ""
    converted = 0
    if not args.no_extract:
        extract_root = out_root / "extracted"
        extract_root.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(output_zip) as zf:
            zf.extractall(extract_root)
        extracted_dir = str(extract_root)
        if not args.no_local_convert:
            converted = convert_tiffs_to_png(extract_root)

    result = {
        "event": "done",
        "job_id": job_id,
        "render_id": accepted["render_id"],
        "output_zip": str(output_zip),
        "extracted_dir": extracted_dir,
        "local_converted_png": converted,
        "remote": status,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
