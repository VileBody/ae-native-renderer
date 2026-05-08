#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from fontTools.pens.recordingPen import RecordingPen
from fontTools.ttLib import TTFont


@dataclass(frozen=True)
class Point:
    x: float
    y: float


def iter_payloads(path: Path):
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict) and isinstance(obj.get("payload"), dict):
            obj = obj["payload"]
        if isinstance(obj, dict):
            yield obj


def trace_path_builders(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    active_glyph: dict[str, Any] | None = None
    for event in iter_payloads(path):
        if event.get("kind") != "cooltype_hook_enter":
            continue
        hook = event.get("hook")
        if hook == "TXTp_DrawChar3_ARE_42110":
            draw = event.get("txt_draw_char_entry") or {}
            active_glyph = {
                "seq": event.get("seq"),
                "glyph_id": draw.get("glyph_id_arg2_u32"),
                "glyph_matrix": draw.get("glyph_matrix_arg3"),
                "text_matrix": draw.get("text_matrix_arg5"),
            }
            continue
        if hook != "TXT_ARE_PathBuilder_40580":
            continue
        path_input = (
            (event.get("txt_are_producer") or {})
            .get("path_builder", {})
            .get("path_input", {})
        )
        coords = path_input.get("coords_f32_sample") or []
        commands = path_input.get("commands_u32_sample") or []
        points = [
            Point(float(coords[index]), float(coords[index + 1]))
            for index in range(0, len(coords) - 1, 2)
        ]
        events.append(
            {
                "seq": event.get("seq"),
                "trace_path": str(path),
                "active_glyph": active_glyph,
                "count": path_input.get("count"),
                "sample_point_count": len(points),
                "sample_command_count": len(commands),
                "commands_sample": commands,
                "points": points,
            }
        )
    return events


def glyph_scale(active_glyph: dict[str, Any] | None, fallback: float) -> float:
    if not isinstance(active_glyph, dict):
        return fallback
    matrix = active_glyph.get("glyph_matrix")
    if isinstance(matrix, list) and matrix:
        try:
            return float(matrix[0])
        except (TypeError, ValueError):
            return fallback
    return fallback


def raw_contours(font: TTFont, glyph_id: int, font_size: float) -> list[list[Point]]:
    glyph_name = font.getGlyphOrder()[glyph_id]
    glyf = font["glyf"]
    glyph = glyf[glyph_name]
    coords, end_pts, _flags = glyph.getCoordinates(glyf)
    scale = font_size / float(font["head"].unitsPerEm)
    contours: list[list[Point]] = []
    start = 0
    for end in end_pts:
        contour = [
            Point(float(coords[index][0]) * scale, -float(coords[index][1]) * scale)
            for index in range(start, end + 1)
        ]
        contours.append(contour)
        start = end + 1
    return contours


def recording_ops(font: TTFont, glyph_id: int) -> list[dict[str, Any]]:
    glyph_name = font.getGlyphOrder()[glyph_id]
    pen = RecordingPen()
    font.getGlyphSet()[glyph_name].draw(pen)
    return [{"op": op, "args": args} for op, args in pen.value]


def split_sampled_line_contours(points: list[Point], commands: list[int]) -> list[list[Point]]:
    contours: list[list[Point]] = []
    current: list[Point] = []
    for point, command in zip(points, commands):
        if command == 0:
            if current:
                contours.append(trim_duplicate_close(current))
            current = [point]
        elif command == 3:
            if current:
                contours.append(trim_duplicate_close(current))
                current = []
        else:
            current.append(point)
    if current:
        contours.append(trim_duplicate_close(current))
    return contours


def trim_duplicate_close(contour: list[Point]) -> list[Point]:
    if len(contour) >= 2 and distance(contour[0], contour[-1]) < 1e-4:
        return contour[:-1]
    return contour


def distance(a: Point, b: Point) -> float:
    return math.hypot(a.x - b.x, a.y - b.y)


def best_cyclic_error(a: list[Point], b: list[Point]) -> dict[str, Any]:
    if not a or not b:
        return {"matched": False, "reason": "empty_contour"}
    if len(a) != len(b):
        return {
            "matched": False,
            "reason": "point_count_mismatch",
            "ae_points": len(a),
            "ttf_points": len(b),
        }
    best: dict[str, Any] | None = None
    for shift in range(len(b)):
        errors = [distance(a[index], b[(index + shift) % len(b)]) for index in range(len(a))]
        candidate = {
            "matched": True,
            "shift": shift,
            "max_error": max(errors),
            "mean_error": sum(errors) / len(errors),
        }
        if best is None or candidate["max_error"] < best["max_error"]:
            best = candidate
    return best or {"matched": False, "reason": "no_candidate"}


def classify_path_order(
    font: TTFont,
    entry: dict[str, Any],
    default_font_size: float,
) -> dict[str, Any]:
    active = entry.get("active_glyph")
    glyph_id = None
    if isinstance(active, dict):
        glyph_id = active.get("glyph_id")
    if glyph_id is None:
        return {"status": "no_active_glyph"}
    glyph_id = int(glyph_id)
    glyph_name = font.getGlyphOrder()[glyph_id]
    font_size = glyph_scale(active, default_font_size)
    commands = [int(value) for value in entry["commands_sample"]]
    points: list[Point] = entry["points"]
    uses_curves = any(command == 2 for command in commands)
    sampled_contours = split_sampled_line_contours(points, commands)
    contours = raw_contours(font, glyph_id, font_size)
    result: dict[str, Any] = {
        "status": "analyzed",
        "glyph_id": glyph_id,
        "glyph_name": glyph_name,
        "font_size": font_size,
        "path_count": entry.get("count"),
        "sample_point_count": entry.get("sample_point_count"),
        "sample_command_count": entry.get("sample_command_count"),
        "uses_curves": uses_curves,
        "ae_contour_count": len(sampled_contours),
        "ttf_contour_count": len(contours),
        "command_histogram": {
            str(command): sum(1 for value in commands if value == command)
            for command in sorted(set(commands))
        },
    }
    if uses_curves:
        result["order_evidence"] = "curve_path_requires_producer_decode"
        result["recording_ops"] = [
            {"op": item["op"], "arg_count": len(item["args"])}
            for item in recording_ops(font, glyph_id)
        ]
        if sampled_contours and contours:
            result["first_ae_point"] = point_json(sampled_contours[0][0])
            result["first_ttf_normal_point"] = point_json(contours[0][0])
            result["first_ttf_reversed_point"] = point_json(list(reversed(contours[0]))[0])
        return result

    comparisons = []
    for contour_index, ae_contour in enumerate(sampled_contours):
        if contour_index >= len(contours):
            comparisons.append({"contour_index": contour_index, "status": "missing_ttf_contour"})
            continue
        ttf = contours[contour_index]
        normal = best_cyclic_error(ae_contour, ttf)
        reversed_result = best_cyclic_error(ae_contour, list(reversed(ttf)))
        winner = "incomplete"
        if normal.get("matched"):
            winner = "normal"
        if reversed_result.get("matched") and (
            not normal.get("matched")
            or float(reversed_result["max_error"]) < float(normal["max_error"])
        ):
            winner = "reversed"
        comparisons.append(
            {
                "contour_index": contour_index,
                "ae_points": len(ae_contour),
                "ttf_points": len(ttf),
                "normal": normal,
                "reversed": reversed_result,
                "winner": winner,
            }
        )
    result["line_contour_comparisons"] = comparisons
    winners = {item.get("winner") for item in comparisons if item.get("winner")}
    if winners == {"reversed"}:
        result["order_evidence"] = "line_contours_match_reversed_ttf"
    elif winners == {"normal"}:
        result["order_evidence"] = "line_contours_match_normal_ttf"
    else:
        result["order_evidence"] = "line_contours_mixed_or_incomplete"
    return result


def point_json(point: Point) -> list[float]:
    return [round(point.x, 6), round(point.y, 6)]


def compact_entry(entry: dict[str, Any], analysis: dict[str, Any]) -> dict[str, Any]:
    active = entry.get("active_glyph")
    return {
        "trace_path": entry.get("trace_path"),
        "seq": entry.get("seq"),
        "active_glyph": active,
        "count": entry.get("count"),
        "sample_point_count": entry.get("sample_point_count"),
        "sample_command_count": entry.get("sample_command_count"),
        "analysis": analysis,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("traces", nargs="+", type=Path)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--font-size", type=float, default=96.0)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()

    font = TTFont(str(args.font))
    entries: list[dict[str, Any]] = []
    for trace in args.traces:
        for entry in trace_path_builders(trace):
            analysis = classify_path_order(font, entry, args.font_size)
            entries.append(compact_entry(entry, analysis))

    result = {
        "schema": "ae-native-renderer.text-path-order-analysis.v1",
        "font": str(args.font),
        "trace_count": len(args.traces),
        "path_builder_count": len(entries),
        "entries": entries,
    }
    text = json.dumps(result, indent=2, ensure_ascii=False, default=str)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text + "\n", encoding="utf-8")
    for item in entries:
        analysis = item["analysis"]
        active = item.get("active_glyph") or {}
        print(
            f"{Path(str(item['trace_path'])).name} seq={item['seq']} "
            f"gid={analysis.get('glyph_id', active.get('glyph_id'))} "
            f"name={analysis.get('glyph_name')} "
            f"evidence={analysis.get('order_evidence', analysis.get('status'))}"
        )
        for comparison in analysis.get("line_contour_comparisons") or []:
            normal = comparison.get("normal") or {}
            reversed_result = comparison.get("reversed") or {}
            print(
                "  contour={contour_index} winner={winner} "
                "normal_max={normal_max} reversed_max={reversed_max}".format(
                    contour_index=comparison.get("contour_index"),
                    winner=comparison.get("winner"),
                    normal_max=normal.get("max_error"),
                    reversed_max=reversed_result.get("max_error"),
                )
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
