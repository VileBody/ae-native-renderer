#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any


DEFAULT_ENV = Path("/Users/ergin/Desktop/blast_mj_final/.env")
DEFAULT_NODE = "http://85.239.48.31:8000"
DEFAULT_OUT = Path("target/ae_experiments/ae_job_pipeline_smoke")
DEFAULT_PREFIX = "ae_job_pipeline_smoke"

EXCLUDED_DIRS = {
    ".git",
    "__pycache__",
    "ae_goldens",
    "ae_probe_outputs",
    "logs",
    "logs_resume",
    "metadata",
    "metadata_with_slate_20260503_160541_85",
    "png",
    "png8",
    "preview",
    "tiff",
    "tiff8",
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


def should_skip(rel: Path, *, include_fonts: bool) -> bool:
    parts = set(rel.parts)
    if rel.name == ".DS_Store":
        return True
    if rel.name.startswith("ae_outputs_round"):
        return True
    if not include_fonts and "assets" in parts and "fonts" in parts:
        return True
    return any(part in EXCLUDED_DIRS for part in rel.parts)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def make_pack_zip(pack_dir: Path, dest: Path, *, include_fonts: bool) -> tuple[str, int]:
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
    return root_name, count


def read_manifest(pack_dir: Path) -> dict[str, Any]:
    path = pack_dir / "manifest.json"
    if not path.is_file():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def infer_entry(pack_dir: Path, manifest: dict[str, Any]) -> str:
    for key_path in (
        ("builder", "jsx"),
        ("ae_project_builder", "script"),
    ):
        node: Any = manifest
        for key in key_path:
            if not isinstance(node, dict) or key not in node:
                node = None
                break
            node = node[key]
        if isinstance(node, str) and node.strip():
            return node.strip()

    candidates = {
        "ae_conformance_pack": "jsx/build_conformance_project.jsx",
        "glow_shadow": "jsx/build_glow_shadow_probe_project.jsx",
        "minimax": "jsx/build_minimax_probe_project.jsx",
        "turbulent_field": "jsx/build_turbulent_field_probe_project.jsx",
    }
    if pack_dir.name in candidates:
        return candidates[pack_dir.name]

    jsx = sorted((pack_dir / "jsx").glob("*.jsx"))
    if len(jsx) == 1:
        return str(jsx[0].relative_to(pack_dir)).replace("\\", "/")
    raise RuntimeError("--entry-script is required; could not infer JSX entry")


def entry_inside_zip(entry_script: str, *, root_name: str) -> str:
    entry = entry_script.replace("\\", "/").lstrip("/")
    if entry.startswith(root_name + "/"):
        return entry
    return f"{root_name}/{entry}"


def s3_client():
    import boto3

    endpoint = os.getenv("S3_ENDPOINT_URL") or None
    region = os.getenv("AWS_DEFAULT_REGION") or os.getenv("AWS_REGION") or os.getenv("S3_REGION") or "ru-1"
    return boto3.session.Session().client("s3", endpoint_url=endpoint, region_name=region)


def upload_zip(path: Path, *, bucket: str, key: str) -> str:
    s3_client().upload_file(str(path), bucket, key, ExtraArgs={"ContentType": "application/zip"})
    return f"s3://{bucket}/{key}"


def write_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def build_contract_note() -> dict[str, Any]:
    return {
        "node": DEFAULT_NODE,
        "verified_health": "GET /health -> 200 {'status':'ok'} on 2026-05-05",
        "verified_openapi": {
            "title": "AE Render Node (Async Render API)",
            "version": "0.6.0",
            "paths": [
                "GET /health",
                "POST /render",
                "GET /render/{render_id}",
                "POST /pack-render",
                "GET /pack-render/{render_id}",
                "POST /jobs",
            ],
        },
        "pack_render_request": {
            "required": ["entry_script"],
            "fields": {
                "job_id": "optional external pack job id",
                "pack_uri": "s3:// or http(s) zip with AE pack files",
                "pack_s3_uri": "alias for pack_uri",
                "entry_script": "JSX path inside zip extraction root",
                "output_s3_bucket": "S3 bucket for output zip",
                "output_s3_key": "S3 key for output zip",
                "output_module_template": "AE output module template; default TIFF Sequence with Alpha",
                "case_ids": "optional subset of case ids to render",
                "skip_preview": "skip master/preview queue items by default",
                "convert_tiff_to_png": "create PNG siblings from TIFF outputs",
            },
        },
        "pack_render_response": {
            "accepted": ["status", "render_id", "job_id"],
            "status": [
                "status",
                "render_id",
                "job_id",
                "success",
                "message",
                "archive_path",
                "output_url",
                "output_s3_uri",
                "job_dir",
            ],
        },
    }


def main() -> int:
    ap = argparse.ArgumentParser("Create an AE probe pack zip and /pack-render payload without submitting it.")
    ap.add_argument("pack", help="Pack directory, e.g. fixtures/ae_probe_pack/glow_shadow")
    ap.add_argument("--entry-script", default="", help="JSX path relative to pack dir; inferred from manifest when omitted")
    ap.add_argument("--job-id", default="", help="Job id; default generated from pack name and timestamp")
    ap.add_argument("--out-dir", default=str(DEFAULT_OUT), help="Directory for zip/payload artifacts")
    ap.add_argument("--env", default=str(DEFAULT_ENV), help="Env file for S3 bucket/credentials; values are not written")
    ap.add_argument("--bucket", default="", help="S3 bucket; default S3_BUCKET_JOB_ARTIFACTS or S3_BUCKET_ASSET_STORAGE")
    ap.add_argument("--prefix", default=DEFAULT_PREFIX, help="S3 key prefix for pack and output zip")
    ap.add_argument("--pack-uri", default="", help="Existing s3:// or http(s) URI for the zip; overrides computed URI")
    ap.add_argument("--upload", action="store_true", help="Upload the generated zip to S3 and make payload submit-ready")
    ap.add_argument("--case", action="append", default=[], help="Optional case id subset; repeatable")
    ap.add_argument("--output-template", default="TIFF Sequence with Alpha", help="AE output module template")
    ap.add_argument("--no-convert-tiff-to-png", action="store_true", help="Disable node-side TIFF to PNG conversion")
    ap.add_argument("--include-fonts", action="store_true", help="Include assets/fonts in the zip")
    args = ap.parse_args()

    load_env_file(Path(args.env).expanduser())

    pack_dir = Path(args.pack).expanduser().resolve()
    manifest = read_manifest(pack_dir)
    job_id = args.job_id.strip() or f"{pack_dir.name}_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    out_dir = Path(args.out_dir).expanduser().resolve()
    zip_path = out_dir / f"{job_id}_pack.zip"
    payload_path = out_dir / f"{job_id}_pack_render_payload.json"
    manifest_path = out_dir / f"{job_id}_packaging_manifest.json"
    contract_path = out_dir / "pack_render_contract_85.json"

    entry = args.entry_script.strip() or infer_entry(pack_dir, manifest)
    root_name, file_count = make_pack_zip(pack_dir, zip_path, include_fonts=args.include_fonts)
    zip_sha256 = sha256_file(zip_path)

    bucket = args.bucket.strip() or os.getenv("S3_BUCKET_JOB_ARTIFACTS") or os.getenv("S3_BUCKET_ASSET_STORAGE") or ""
    prefix = args.prefix.strip("/").strip()
    pack_key = f"{prefix}/{job_id}/{zip_path.name}" if prefix else f"{job_id}/{zip_path.name}"
    output_key = f"{prefix}/{job_id}/{job_id}_outputs.zip" if prefix else f"{job_id}/{job_id}_outputs.zip"

    pack_uri = args.pack_uri.strip()
    planned_pack_uri = f"s3://{bucket}/{pack_key}" if bucket else ""
    uploaded = False
    if args.upload:
        if not bucket:
            raise RuntimeError("No S3 bucket: pass --bucket or set S3_BUCKET_JOB_ARTIFACTS")
        pack_uri = upload_zip(zip_path, bucket=bucket, key=pack_key)
        uploaded = True

    payload = {
        "job_id": job_id,
        "pack_uri": pack_uri or "UPLOAD_ZIP_AND_REPLACE_WITH_S3_OR_HTTP_URI",
        "entry_script": entry_inside_zip(entry, root_name=root_name),
        "output_s3_bucket": bucket or None,
        "output_s3_key": output_key if bucket else None,
        "output_module_template": args.output_template,
        "case_ids": args.case,
        "skip_preview": True,
        "convert_tiff_to_png": not args.no_convert_tiff_to_png,
    }

    packaging = {
        "schema": "ae-native-renderer.ae-probe-job-packaging.v1",
        "job_id": job_id,
        "node": DEFAULT_NODE,
        "pack_dir": str(pack_dir),
        "pack_root_name": root_name,
        "entry_script_relative": entry,
        "entry_script_payload": payload["entry_script"],
        "zip_path": str(zip_path),
        "zip_sha256": zip_sha256,
        "zip_size_bytes": zip_path.stat().st_size,
        "file_count": file_count,
        "uploaded": uploaded,
        "pack_uri": payload["pack_uri"],
        "planned_pack_uri": planned_pack_uri or None,
        "output_s3_bucket": bucket or None,
        "output_s3_key": payload["output_s3_key"],
        "case_ids": args.case,
        "submit_command": (
            f"curl -sS -X POST {DEFAULT_NODE}/pack-render "
            f"-H 'Content-Type: application/json' --data-binary '@{payload_path}'"
        ),
        "poll_command_template": f"curl -sS {DEFAULT_NODE}/pack-render/<render_id>",
    }

    write_json(payload_path, payload)
    write_json(manifest_path, packaging)
    write_json(contract_path, build_contract_note())

    print(json.dumps({
        "job_id": job_id,
        "zip": str(zip_path),
        "payload": str(payload_path),
        "packaging_manifest": str(manifest_path),
        "contract": str(contract_path),
        "uploaded": uploaded,
        "pack_uri": payload["pack_uri"],
        "submit_command": packaging["submit_command"],
    }, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
