#!/usr/bin/env python3
"""Measure AE Minimax enum/radius/edge probe renders.

The script reads the pack manifest, measures first-frame PNG renders, and
derives compact JSON/CSV summaries for operation order, channel lane effects,
direction/orientation, fractional-radius quantization, and edge policy.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


PACK_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RENDER_ROOT = PACK_ROOT / "ae_goldens" / "png"
DEFAULT_METADATA_ROOT = PACK_ROOT / "ae_goldens" / "metadata"
ASSET_ROOT = PACK_ROOT / "assets" / "primitives"
THRESHOLD = 2


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pack", type=Path, default=PACK_ROOT)
    parser.add_argument("--render-root", type=Path)
    parser.add_argument("--metadata-root", type=Path)
    parser.add_argument("--frame", type=int, default=0)
    parser.add_argument("--generate-assets", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--threshold", type=int, default=THRESHOLD)
    args = parser.parse_args()

    require_pillow()

    if args.generate_assets:
        generated = generate_assets(args.pack / "assets" / "primitives")
        print(f"generated {len(generated)} assets in {args.pack / 'assets' / 'primitives'}")
        if not args.self_test:
            return 0

    if args.self_test:
        return run_self_test(args.pack, args.threshold)

    render_root = args.render_root or args.pack / "ae_goldens" / "png"
    metadata_root = args.metadata_root or args.pack / "ae_goldens" / "metadata"
    result = measure_pack(args.pack, render_root, metadata_root, args.frame, args.threshold)
    print(f"wrote {result['json_path']}")
    print(f"wrote {result['csv_path']}")
    print(f"wrote {result['semantic_csv_path']}")
    return 0


def require_pillow() -> None:
    try:
        import PIL  # noqa: F401
    except ImportError as exc:
        raise SystemExit("Pillow is required: python3 -m pip install Pillow") from exc


def load_manifest(pack: Path) -> dict[str, Any]:
    return json.loads((pack / "manifest.json").read_text(encoding="utf-8"))


def measure_pack(
    pack: Path,
    render_root: Path,
    metadata_root: Path,
    frame: int,
    threshold: int,
) -> dict[str, str]:
    from PIL import Image

    manifest = load_manifest(pack)
    cases = manifest["cases"]
    refs = reference_case_ids(cases)
    measured: dict[str, dict[str, Any]] = {}

    for order, case in enumerate(cases):
        case_id = case["id"]
        png = select_png(render_root / case_id, frame)
        entry: dict[str, Any] = {
            "id": case_id,
            "order": order,
            "role": case.get("role"),
            "source_id": case.get("source_id"),
            "effect_params": case.get("effect_params"),
            "png": str(png) if png else None,
            "status": "missing_png" if png is None else "ok",
        }
        if png is not None:
            with Image.open(png) as opened:
                image = opened.convert("RGBA")
                entry.update(image_stats(image, threshold))
                entry["pixel_sha256"] = hashlib.sha256(image.tobytes()).hexdigest()
                entry["_image"] = image.copy()
        measured[case_id] = entry

    for case in cases:
        case_id = case["id"]
        entry = measured[case_id]
        source_id = case.get("source_id")
        ref_id = refs.get(source_id)
        if entry["status"] == "ok" and ref_id and measured.get(ref_id, {}).get("status") == "ok":
            entry["reference_case_id"] = ref_id
            entry["diff_from_reference"] = diff_stats(
                measured[ref_id]["_image"], entry["_image"], threshold
            )

    output = {
        "pack_id": manifest.get("pack_id"),
        "schema": "ae-native-renderer.minimax-enum-radius-edge-measurements.v1",
        "render_root": str(render_root),
        "metadata_root": str(metadata_root),
        "threshold": threshold,
        "stage_order": derive_stage_order(manifest, measured),
        "operation_order": derive_operation_order(cases, measured),
        "channel_lane_effects": derive_channel_lane_effects(manifest, cases, measured),
        "orientation": derive_orientation(cases, measured),
        "radius_quantization": derive_radius_quantization(cases, measured),
        "edge_policy": derive_edge_policy(cases, measured),
        "cases": strip_images([measured[case["id"]] for case in cases]),
    }

    metadata_root.mkdir(parents=True, exist_ok=True)
    json_path = metadata_root / "minimax_enum_radius_edge_measurements.json"
    csv_path = metadata_root / "minimax_enum_radius_edge_measurements.csv"
    semantic_csv_path = metadata_root / "minimax_enum_radius_edge_semantics.csv"
    json_path.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    write_csv(csv_path, output["cases"])
    write_semantic_csv(semantic_csv_path, output)
    return {
        "json_path": str(json_path),
        "csv_path": str(csv_path),
        "semantic_csv_path": str(semantic_csv_path),
    }


def reference_case_ids(cases: list[dict[str, Any]]) -> dict[str, str]:
    refs = {}
    for case in cases:
        if case.get("role") == "pre_reference":
            refs[case["source_id"]] = case["id"]
    return refs


def select_png(case_dir: Path, frame: int) -> Path | None:
    files = sorted(case_dir.glob("*.png"))
    if not files:
        return None
    if frame < 0 or frame >= len(files):
        return files[0]
    return files[frame]


def image_stats(image: Any, threshold: int) -> dict[str, Any]:
    return {
        "size": list(image.size),
        "rgba_nonzero_bbox": rgba_nonzero_bbox(image, threshold),
        "alpha_nonzero_bbox": channel_bbox(image, 3, 0),
        "channel_bboxes": {
            name: channel_bbox(image, idx, threshold)
            for idx, name in enumerate(["r", "g", "b", "a"])
        },
        "channel_sums": channel_sums(image),
    }


def rgba_nonzero_bbox(image: Any, threshold: int) -> list[int] | None:
    pixels = image.load()
    width, height = image.size
    bbox = BBox()
    for y in range(height):
        for x in range(width):
            if max(pixels[x, y]) > threshold:
                bbox.add(x, y)
    return bbox.value()


def channel_bbox(image: Any, channel: int, threshold: int) -> list[int] | None:
    pixels = image.load()
    width, height = image.size
    bbox = BBox()
    for y in range(height):
        for x in range(width):
            if pixels[x, y][channel] > threshold:
                bbox.add(x, y)
    return bbox.value()


def channel_sums(image: Any) -> dict[str, int]:
    pixels = image.load()
    width, height = image.size
    sums = [0, 0, 0, 0]
    for y in range(height):
        for x in range(width):
            px = pixels[x, y]
            for idx in range(4):
                sums[idx] += px[idx]
    return dict(zip(["r", "g", "b", "a"], sums))


def diff_stats(reference: Any, image: Any, threshold: int) -> dict[str, Any]:
    pixels_a = reference.load()
    pixels_b = image.load()
    width, height = image.size
    bbox = BBox()
    channel_bboxs = [BBox(), BBox(), BBox(), BBox()]
    sums = [0, 0, 0, 0]
    changed = 0
    edge_counts = {"top": 0, "bottom": 0, "left": 0, "right": 0}
    for y in range(height):
        for x in range(width):
            da = [abs(pixels_b[x, y][idx] - pixels_a[x, y][idx]) for idx in range(4)]
            for idx, delta in enumerate(da):
                sums[idx] += delta
                if delta > threshold:
                    channel_bboxs[idx].add(x, y)
            if max(da) > threshold:
                changed += 1
                bbox.add(x, y)
                if y <= 1:
                    edge_counts["top"] += 1
                if y >= height - 2:
                    edge_counts["bottom"] += 1
                if x <= 1:
                    edge_counts["left"] += 1
                if x >= width - 2:
                    edge_counts["right"] += 1
    return {
        "changed_pixels": changed,
        "bbox": bbox.value(),
        "bbox_area": bbox.area(),
        "channel_delta_sums": dict(zip(["r", "g", "b", "a"], sums)),
        "channel_delta_bboxes": {
            name: channel_bboxs[idx].value()
            for idx, name in enumerate(["r", "g", "b", "a"])
        },
        "edge_changed_pixels": edge_counts,
    }


def derive_stage_order(manifest: dict[str, Any], measured: dict[str, dict[str, Any]]) -> dict[str, Any]:
    return {
        "pipeline": [
            "generate_or_import_source_png",
            "place_source_layer",
            "apply_ADBE_Minimax_params",
            "render_first_png_frame",
            "measure_reference_delta",
            "derive_semantics",
        ],
        "case_order": [
            {
                "order": order,
                "case_id": case["id"],
                "role": case.get("role"),
                "source_id": case.get("source_id"),
                "status": measured[case["id"]]["status"],
            }
            for order, case in enumerate(manifest["cases"])
        ],
    }


def derive_operation_order(
    cases: list[dict[str, Any]], measured: dict[str, dict[str, Any]]
) -> list[dict[str, Any]]:
    rows = []
    for case in cases:
        if case.get("role") != "operation_probe":
            continue
        entry = measured[case["id"]]
        diff = entry.get("diff_from_reference", {})
        params = case.get("effect_params") or {}
        rows.append(
            {
                "case_id": case["id"],
                "source_id": case.get("source_id"),
                "operation_0001": params.get("0001"),
                "radius_0002": params.get("0002"),
                "changed_pixels": diff.get("changed_pixels"),
                "diff_bbox": diff.get("bbox"),
                "diff_bbox_area": diff.get("bbox_area"),
                "channel_delta_sums": diff.get("channel_delta_sums"),
            }
        )
    return rows


def derive_channel_lane_effects(
    manifest: dict[str, Any],
    cases: list[dict[str, Any]],
    measured: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    lanes = manifest.get("analysis", {}).get("channel_lanes", [])
    rows = []
    for case in cases:
        if case.get("role") != "channel_probe":
            continue
        entry = measured[case["id"]]
        ref_id = entry.get("reference_case_id")
        if entry.get("status") != "ok" or not ref_id or measured[ref_id].get("status") != "ok":
            rows.append({"case_id": case["id"], "status": entry.get("status")})
            continue
        lane_rows = []
        for lane in lanes:
            lane_rows.append(
                {
                    "lane": lane["id"],
                    "expected_channel": lane["channel"],
                    "delta_sum": roi_delta_sum(
                        measured[ref_id]["_image"], entry["_image"], lane["roi"]
                    ),
                }
            )
        dominant = max(lane_rows, key=lambda row: row["delta_sum"]) if lane_rows else None
        rows.append(
            {
                "case_id": case["id"],
                "channel_0003": (case.get("effect_params") or {}).get("0003"),
                "dominant_lane": dominant,
                "lanes": lane_rows,
                "total_channel_delta_sums": entry.get("diff_from_reference", {}).get(
                    "channel_delta_sums"
                ),
            }
        )
    return rows


def roi_delta_sum(reference: Any, image: Any, roi: list[int]) -> int:
    pixels_a = reference.load()
    pixels_b = image.load()
    x0, y0, x1, y1 = roi
    total = 0
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            total += sum(abs(pixels_b[x, y][idx] - pixels_a[x, y][idx]) for idx in range(4))
    return total


def derive_orientation(
    cases: list[dict[str, Any]], measured: dict[str, dict[str, Any]]
) -> list[dict[str, Any]]:
    rows = []
    for case in cases:
        if case.get("role") != "direction_probe":
            continue
        diff = measured[case["id"]].get("diff_from_reference", {})
        bbox = diff.get("bbox")
        width = bbox[2] - bbox[0] + 1 if bbox else 0
        height = bbox[3] - bbox[1] + 1 if bbox else 0
        if width > height * 1.25:
            inferred = "horizontal"
        elif height > width * 1.25:
            inferred = "vertical"
        elif width and height:
            inferred = "isotropic_or_square"
        else:
            inferred = "no_change"
        rows.append(
            {
                "case_id": case["id"],
                "direction_0004": (case.get("effect_params") or {}).get("0004"),
                "diff_bbox": bbox,
                "diff_width": width,
                "diff_height": height,
                "aspect_width_over_height": round(width / height, 4) if height else None,
                "inferred_orientation": inferred,
            }
        )
    return rows


def derive_radius_quantization(
    cases: list[dict[str, Any]], measured: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    rows = []
    clusters: dict[str, list[float]] = {}
    for case in cases:
        if case.get("role") != "radius_probe":
            continue
        entry = measured[case["id"]]
        params = case.get("effect_params") or {}
        radius = params.get("0002")
        digest = entry.get("pixel_sha256")
        if digest:
            clusters.setdefault(digest, []).append(radius)
        diff = entry.get("diff_from_reference", {})
        rows.append(
            {
                "case_id": case["id"],
                "radius_0002": radius,
                "status": entry.get("status"),
                "pixel_sha256": digest,
                "diff_bbox": diff.get("bbox"),
                "diff_bbox_area": diff.get("bbox_area"),
                "changed_pixels": diff.get("changed_pixels"),
            }
        )
    adjacent = []
    for prev, cur in zip(rows, rows[1:]):
        adjacent.append(
            {
                "a_radius": prev["radius_0002"],
                "b_radius": cur["radius_0002"],
                "same_pixels": bool(prev.get("pixel_sha256") and prev["pixel_sha256"] == cur.get("pixel_sha256")),
            }
        )
    return {
        "cases": rows,
        "pixel_clusters": [
            {"pixel_sha256": digest, "radii": radii} for digest, radii in clusters.items()
        ],
        "adjacent_equalities": adjacent,
    }


def derive_edge_policy(
    cases: list[dict[str, Any]], measured: dict[str, dict[str, Any]]
) -> list[dict[str, Any]]:
    rows = []
    for case in cases:
        if case.get("role") != "boundary_probe":
            continue
        diff = measured[case["id"]].get("diff_from_reference", {})
        rows.append(
            {
                "case_id": case["id"],
                "boundary_0005": (case.get("effect_params") or {}).get("0005"),
                "diff_bbox": diff.get("bbox"),
                "changed_pixels": diff.get("changed_pixels"),
                "edge_changed_pixels": diff.get("edge_changed_pixels"),
            }
        )
    return rows


def write_csv(path: Path, cases: list[dict[str, Any]]) -> None:
    fields = [
        "case_id",
        "status",
        "role",
        "source_id",
        "effect_params",
        "png",
        "pixel_sha256",
        "reference_case_id",
        "diff_changed_pixels",
        "diff_bbox",
        "diff_bbox_area",
        "delta_r",
        "delta_g",
        "delta_b",
        "delta_a",
    ]
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for case in cases:
            diff = case.get("diff_from_reference") or {}
            sums = diff.get("channel_delta_sums") or {}
            writer.writerow(
                {
                    "case_id": case["id"],
                    "status": case["status"],
                    "role": case.get("role"),
                    "source_id": case.get("source_id"),
                    "effect_params": json.dumps(case.get("effect_params"), sort_keys=True),
                    "png": case.get("png"),
                    "pixel_sha256": case.get("pixel_sha256"),
                    "reference_case_id": case.get("reference_case_id"),
                    "diff_changed_pixels": diff.get("changed_pixels"),
                    "diff_bbox": json.dumps(diff.get("bbox")),
                    "diff_bbox_area": diff.get("bbox_area"),
                    "delta_r": sums.get("r"),
                    "delta_g": sums.get("g"),
                    "delta_b": sums.get("b"),
                    "delta_a": sums.get("a"),
                }
            )


def write_semantic_csv(path: Path, output: dict[str, Any]) -> None:
    fields = ["section", "case_id", "key", "value"]
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()

        def row(section: str, case_id: str | None, key: str, value: Any) -> None:
            writer.writerow(
                {
                    "section": section,
                    "case_id": case_id or "",
                    "key": key,
                    "value": json.dumps(value, sort_keys=True),
                }
            )

        row("stage_order", None, "pipeline", output["stage_order"]["pipeline"])
        for item in output["stage_order"]["case_order"]:
            row("stage_order", item["case_id"], "case_order", item)
        for item in output["operation_order"]:
            row("operation_order", item["case_id"], "signature", item)
        for item in output["channel_lane_effects"]:
            row("channel_lane_effects", item.get("case_id"), "summary", item)
        for item in output["orientation"]:
            row("orientation", item["case_id"], "summary", item)
        for item in output["radius_quantization"]["cases"]:
            row("radius_quantization", item["case_id"], "case", item)
        for item in output["radius_quantization"]["pixel_clusters"]:
            row("radius_quantization", None, "pixel_cluster", item)
        for item in output["radius_quantization"]["adjacent_equalities"]:
            row("radius_quantization", None, "adjacent_equality", item)
        for item in output["edge_policy"]:
            row("edge_policy", item["case_id"], "summary", item)


def strip_images(cases: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out = []
    for case in cases:
        clean = dict(case)
        clean.pop("_image", None)
        out.append(clean)
    return out


@dataclass
class BBox:
    min_x: int = 1 << 30
    min_y: int = 1 << 30
    max_x: int = -1
    max_y: int = -1

    def add(self, x: int, y: int) -> None:
        self.min_x = min(self.min_x, x)
        self.min_y = min(self.min_y, y)
        self.max_x = max(self.max_x, x)
        self.max_y = max(self.max_y, y)

    def value(self) -> list[int] | None:
        if self.max_x < 0:
            return None
        return [self.min_x, self.min_y, self.max_x, self.max_y]

    def area(self) -> int | None:
        if self.max_x < 0:
            return None
        return (self.max_x - self.min_x + 1) * (self.max_y - self.min_y + 1)


def generate_assets(asset_root: Path) -> list[Path]:
    from PIL import Image

    asset_root.mkdir(parents=True, exist_ok=True)
    size = 128
    assets = {
        "impulse_dot.png": make_impulse_dot(size),
        "ramp_steps.png": make_ramp_steps(size),
        "rgba_lanes.png": make_rgba_lanes(size),
        "orientation_impulses.png": make_orientation_impulses(size),
        "boundary_corner.png": make_boundary_corner(size),
    }
    written = []
    for filename, pixels in assets.items():
        path = asset_root / filename
        image = Image.new("RGBA", (size, size))
        image.putdata(pixels)
        image.save(path)
        written.append(path)
    manifest = [
        {
            "id": Path(path).stem,
            "path": f"assets/primitives/{path.name}",
            "width": size,
            "height": size,
        }
        for path in written
    ]
    (asset_root / "assets_manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    return written


def make_impulse_dot(size: int) -> list[tuple[int, int, int, int]]:
    center = size // 2
    pixels = []
    for y in range(size):
        for x in range(size):
            if x == center and y == center:
                pixels.append((255, 255, 255, 255))
            else:
                pixels.append((0, 0, 0, 255))
    return pixels


def make_ramp_steps(size: int) -> list[tuple[int, int, int, int]]:
    pixels = []
    for y in range(size):
        for x in range(size):
            r = int(round(255 * x / (size - 1)))
            g = int(round(255 * y / (size - 1)))
            b = 255 if ((x // 16) + (y // 16)) & 1 else 0
            pixels.append((r, g, b, 255))
    return pixels


def make_rgba_lanes(size: int) -> list[tuple[int, int, int, int]]:
    pixels = []
    for y in range(size):
        for x in range(size):
            r = 255 if 18 <= x <= 21 and 18 <= y <= 109 else 0
            g = 255 if 38 <= x <= 41 and 18 <= y <= 109 else 0
            b = 255 if 58 <= x <= 61 and 18 <= y <= 109 else 0
            a = 255
            if 78 <= x <= 81 and 18 <= y <= 109:
                a = 0
            pixels.append((r, g, b, a))
    return pixels


def make_orientation_impulses(size: int) -> list[tuple[int, int, int, int]]:
    center = size // 2
    pixels = []
    for y in range(size):
        for x in range(size):
            if x == center and 28 <= y <= 100:
                pixels.append((0, 220, 0, 255))
            elif y == center and 28 <= x <= 100:
                pixels.append((220, 0, 0, 255))
            elif x == center and y == center:
                pixels.append((255, 255, 255, 255))
            else:
                pixels.append((0, 0, 0, 255))
    return pixels


def make_boundary_corner(size: int) -> list[tuple[int, int, int, int]]:
    pixels = []
    for y in range(size):
        for x in range(size):
            if x <= 2 or y <= 2 or x >= size - 3 or y >= size - 3:
                pixels.append((255, 255, 255, 255))
            elif 12 <= x <= 20 and 12 <= y <= 20:
                pixels.append((180, 180, 180, 255))
            else:
                pixels.append((0, 0, 0, 255))
    return pixels


def run_self_test(pack: Path, threshold: int) -> int:
    manifest = load_manifest(pack)
    generate_assets(pack / "assets" / "primitives")
    with tempfile.TemporaryDirectory(prefix="mmer_self_test_") as tmp:
        tmp_path = Path(tmp)
        render_root = tmp_path / "png"
        metadata_root = tmp_path / "metadata"
        synthesize_dummy_renders(pack, manifest, render_root)
        result = measure_pack(pack, render_root, metadata_root, 0, threshold)
        measured = json.loads(Path(result["json_path"]).read_text(encoding="utf-8"))
        ok_cases = [case for case in measured["cases"] if case["status"] == "ok"]
        missing = [case["id"] for case in measured["cases"] if case["status"] != "ok"]
        if missing:
            raise SystemExit(f"self-test missing dummy renders: {missing}")
        print(f"self-test ok: {len(ok_cases)} dummy cases measured")
        print(f"self-test json: {result['json_path']}")
        print(f"self-test csv: {result['csv_path']}")
        print(f"self-test semantic csv: {result['semantic_csv_path']}")
    return 0


def synthesize_dummy_renders(pack: Path, manifest: dict[str, Any], render_root: Path) -> None:
    from PIL import Image

    source_to_asset = {
        source["id"]: pack / source["path"] for source in manifest.get("sources", [])
    }
    for case in manifest["cases"]:
        image = Image.open(source_to_asset[case["source_id"]]).convert("RGBA")
        params = case.get("effect_params")
        if params:
            image = dummy_minimax(image, params)
        case_dir = render_root / case["id"]
        case_dir.mkdir(parents=True, exist_ok=True)
        image.save(case_dir / f"{case['id']}_00000.png")


def dummy_minimax(image: Any, params: dict[str, Any]) -> Any:
    from PIL import Image

    op = int(params.get("0001", 2))
    radius = int(math.floor(float(params.get("0002", 0)) + 0.5))
    channel = int(params.get("0003", 1))
    direction = int(params.get("0004", 1))
    boundary = int(params.get("0005", 0))
    if radius <= 0:
        return image.copy()
    data = list(image.getdata())
    width, height = image.size
    planes = [[px[idx] for px in data] for idx in range(4)]
    selected = selected_channels(channel)
    out_planes = [list(plane) for plane in planes]
    for idx in selected:
        plane = planes[idx]
        if op == 1:
            out_planes[idx] = morph_plane(plane, width, height, radius, direction, boundary, "min")
        elif op == 2:
            out_planes[idx] = morph_plane(plane, width, height, radius, direction, boundary, "max")
        elif op == 3:
            tmp_plane = morph_plane(plane, width, height, radius, direction, boundary, "min")
            out_planes[idx] = morph_plane(tmp_plane, width, height, radius, direction, boundary, "max")
        elif op == 4:
            tmp_plane = morph_plane(plane, width, height, radius, direction, boundary, "max")
            out_planes[idx] = morph_plane(tmp_plane, width, height, radius, direction, boundary, "min")
    out = Image.new("RGBA", image.size)
    out.putdata(
        [
            (out_planes[0][i], out_planes[1][i], out_planes[2][i], out_planes[3][i])
            for i in range(width * height)
        ]
    )
    return out


def selected_channels(channel: int) -> list[int]:
    mapping = {
        1: [0, 1, 2, 3],
        2: [0, 1, 2],
        3: [0],
        4: [1],
        5: [2],
        6: [3],
    }
    return mapping.get(channel, [0, 1, 2, 3])


def morph_plane(
    plane: list[int],
    width: int,
    height: int,
    radius: int,
    direction: int,
    boundary: int,
    mode: str,
) -> list[int]:
    outside = 255 if mode == "min" else 0
    out = [0] * (width * height)
    for y in range(height):
        for x in range(width):
            values = []
            y_range = range(y - radius, y + radius + 1) if direction != 2 else range(y, y + 1)
            x_range = range(x - radius, x + radius + 1) if direction != 3 else range(x, x + 1)
            for yy in y_range:
                for xx in x_range:
                    sx = xx
                    sy = yy
                    if boundary:
                        sx = min(width - 1, max(0, sx))
                        sy = min(height - 1, max(0, sy))
                        values.append(plane[sy * width + sx])
                    elif 0 <= sx < width and 0 <= sy < height:
                        values.append(plane[sy * width + sx])
                    else:
                        values.append(outside)
            out[y * width + x] = min(values) if mode == "min" else max(values)
    return out


if __name__ == "__main__":
    raise SystemExit(main())
