#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_NODE = "http://85.239.48.31:8001"
DEFAULT_SSH_HOST = "ae85"
DEFAULT_REMOTE_PYTHON = r"C:\Python314\python.exe"
DEFAULT_S3_ENV = Path("/Users/ergin/Desktop/blast_mj_final/.env")
DEFAULT_PACK = Path("fixtures/ae_conformance_pack")
DEFAULT_ENTRY_SCRIPT = "jsx/build_conformance_project.jsx"
DEFAULT_CASES = ["TXT_010", "TXT_020", "TXT_040", "GPH_010"]


def load_trace_helpers():
    helper_path = REPO_ROOT / "scripts" / "ae_trace_drop_shadow_softness.py"
    spec = importlib.util.spec_from_file_location("ae_trace_helpers", helper_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import helper script: {helper_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


REMOTE_TRACER = r'''
from __future__ import annotations

import argparse
import json
import time

import frida


JS = r"""
const maxEvents = MAX_EVENTS_PLACEHOLDER;
const processInfo = PROCESS_INFO_PLACEHOLDER;
const hookProfile = "HOOK_PROFILE_PLACEHOLDER";
let eventCount = 0;
const installedHooks = {};
const moduleSnapshots = {};

const COOLTYPE_HOOKS = [
  {module: "CoolType.dll", name: "CTTextGetBoundingBox", offset: 0x29fb60, surface: "bbox_int"},
  {module: "CoolType.dll", name: "CTTextGetQuickBoundingBox", offset: 0x2a0c00, surface: "bbox_quick_float"},
  {module: "CoolType.dll", name: "CTTextGetOrientedBBox", offset: 0x2a0850, surface: "bbox_oriented_float"},
  {module: "CoolType.dll", name: "CoreTextBoundingBox", offset: 0x1239a4, surface: "core_bbox_int"},
  {module: "CoolType.dll", name: "CoreTextFloatBoundingBox", offset: 0x12417c, surface: "core_bbox_float"},
  {module: "CoolType.dll", name: "CoreTextQuickBoundingBox", offset: 0x1261ac, surface: "core_quick_bbox_float"},
  {module: "CoolType.dll", name: "CoreTextGlyphRowExtract", offset: 0x127238, surface: "glyph_row_extract"},
  {module: "CoolType.dll", name: "CoreTextOutlines", offset: 0x124e0c, surface: "core_text_outlines"},
  {module: "CoolType.dll", name: "CoreTextOutlinesV2", offset: 0x1257d4, surface: "core_text_outlines_v2"},
  {module: "CoolType.dll", name: "CoreTextGlyphRecordRender", offset: 0x171af0, surface: "core_text_glyph_record_render"},
  {module: "CoolType.dll", name: "CoreTextGlyphRunSlice", offset: 0x171cb8, surface: "core_text_glyph_run_slice"},
  {module: "CoolType.dll", name: "CoreTextEmitGlyphRecord", offset: 0x16fd54, surface: "core_text_emit_glyph_record"},
  {module: "CoolType.dll", name: "CTTextGetGlyphs_V2", offset: 0x2a02a0, surface: "glyph_pointer_groups"},
  {module: "CoolType.dll", name: "CTTextGetTextGlyphs", offset: 0x2a1380, surface: "glyph_text_rows"},
  {module: "CoolType.dll", name: "CTFontInstanceGetWidths", offset: 0x292d20, surface: "font_widths"},
  {module: "CoolType.dll", name: "CTFontInstanceGetBBoxes", offset: 0x291900, surface: "font_bboxes"},
  {module: "CoolType.dll", name: "CoreWidthsBatch", offset: 0x15ac10, surface: "core_widths"},
  {module: "CoolType.dll", name: "CoreBBoxBatch", offset: 0x15a870, surface: "core_bboxes"}
];

const TXT_HOOKS = [
  {module: "TXT.dll", name: "TXT_DrawChar_entry_413d0", offset: 0x0413d0, surface: "txt_drawchar_entry"},
  {module: "TXT.dll", name: "TXTp_DrawChar1_fallback_41720", offset: 0x041720, surface: "txt_drawchar_fallback"},
  {module: "TXT.dll", name: "TXTp_DrawChar2_AGM_41e00", offset: 0x041e00, surface: "txt_drawchar_agm"},
  {module: "TXT.dll", name: "TXTp_DrawChar3_ARE_42110", offset: 0x042110, surface: "txt_drawchar_are"},
  {module: "TXT.dll", name: "TXT_DrawChar_outline_core_42b80", offset: 0x042b80, surface: "txt_drawchar_outline_core"},
  {module: "TXT.dll", name: "TXT_Font_HaveOutlines_47e20", offset: 0x047e20, surface: "txt_font_have_outlines"},
  {module: "TXT.dll", name: "TXT_ARE_Render_8bpc_3c360", offset: 0x03c360, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_Render_8bpc_fill_3d200", offset: 0x03d200, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_Render_8bpc_stroke_3d960", offset: 0x03d960, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_OutputComposite_8bpc_3de50", offset: 0x03de50, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_PixelWriter8_span_3b8c0", offset: 0x03b8c0, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_PixelWriter8_type2_load_3ba1b", offset: 0x03ba1b, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_PixelWriter8_type2_stride_mul_3ba2e", offset: 0x03ba2e, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_PixelWriter8_type2_stride_add_3ba5b", offset: 0x03ba5b, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_PixelWriter8_type2_span_count_3ba71", offset: 0x03ba71, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_PixelWriter8_type2_span_ready_3ba74", offset: 0x03ba74, surface: "txt_are_spans"},
  {module: "TXT.dll", name: "TXT_ARE_PixelWriter8_type2_sample_byte_3ba80", offset: 0x03ba80, surface: "txt_are_pixel_samples"},
  {module: "TXT.dll", name: "TXT_PlayCharOutlines", offset: 0x0416c0, surface: "txt_play_char_outlines"},
  {module: "TXT.dll", name: "TXT_PlayCharOutlines_impl", offset: 0x042ab0, surface: "txt_play_char_outlines_impl"},
  {module: "TXT.dll", name: "TXT_FUN_sourceRect_batch_bboxes_214120", offset: 0x214120, surface: "txt_source_rect_batch_bboxes"},
  {module: "TXT.dll", name: "TXT_FUN_sourceRect_batch_widths_214a00", offset: 0x214a00, surface: "txt_source_rect_batch_widths"},
  {module: "TXT.dll", name: "TXT_FUN_text_bbox_caller_042b80", offset: 0x042b80, surface: "txt_text_bbox_caller"},
  {module: "TXT.dll", name: "TXT_GridChar_EnsureGlyphMetricsCached", offset: 0x053ff0, surface: "txt_gridchar_cache"},
  {module: "TXT.dll", name: "TXT_GridChar_GetGlyphMetrics", offset: 0x03fac0, surface: "txt_gridchar_metrics"},
  {module: "TXT.dll", name: "TXT_GridChar_GetCharacterAlignmentBounds", offset: 0x0546b0, surface: "txt_gridchar_alignment_bounds"},
  {module: "TXT.dll", name: "TXT_GridChar_GetGlyphBoundsPlus", offset: 0x054b00, surface: "txt_gridchar_bounds_plus"},
  {module: "TXT.dll", name: "TXT_GridChar_GetGlyphMetricsInLineSpace", offset: 0x054cf0, surface: "txt_gridchar_line_space"},
  {module: "TXT.dll", name: "TXT_GridChar_GetGlyphMetricsPlus", offset: 0x054f80, surface: "txt_gridchar_metrics_plus"},
  {module: "TXT.dll", name: "TXT_GridChar_GetRenderExtent", offset: 0x055760, surface: "txt_gridchar_render_extent"},
  {module: "TXT.dll", name: "TXT_GridChar_GetTransformedGlyphAdvance", offset: 0x055e70, surface: "txt_gridchar_transformed_advance"},
  {module: "TXT.dll", name: "TXT_GridChar_GetTransformedGlyphMetrics", offset: 0x055f30, surface: "txt_gridchar_transformed_metrics"},
  {module: "TXT.dll", name: "TXT_Grid_InitializeAnchorPoints", offset: 0x056110, surface: "txt_grid_anchor_points"}
];

const BEE_HOOKS = [
  {module: "BEE.dll", name: "BEE_TextLayer_GetTextGrid_entry_5bccb0", offset: 0x5bccb0, surface: "bee_text_entry"},
  {module: "BEE.dll", name: "BEE_TextLayer_SubLayerRenderCount_entry_5bf0e0", offset: 0x5bf0e0, surface: "bee_text_entry"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_ctor_5c0780", offset: 0x5c0780, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_SubLayerRenderNode_ctor_4bfba0", offset: 0x4bfba0, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_SubLayerRenderNode_render_impl_4c2720", offset: 0x4c2720, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_SubLayerRenderNode_vtable_f0_4c1f10", offset: 0x4c1f10, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_RasterizeAVSubLayerNode_builder_2a2920", offset: 0x2a2920, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_RasterizeAVLayerNode_render_impl_2a3ca0", offset: 0x2a3ca0, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_PSLRasterizeLayer_wrapper_825d20", offset: 0x825d20, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_dtor_or_reset_5c0870", offset: 0x5c0870, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_bounds_or_grid_5c0920", offset: 0x5c0920, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_vtable_88_5c08f0", offset: 0x5c08f0, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_vtable_90_5c0960", offset: 0x5c0960, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_vtable_98_5c0990", offset: 0x5c0990, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_vtable_a0_5c0aa0", offset: 0x5c0aa0, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_vtable_a8_5c0b40", offset: 0x5c0b40, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_vtable_b0_5c0b60", offset: 0x5c0b60, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_vtable_b8_5c0b80", offset: 0x5c0b80, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_sublayer_opacity_c0_5c0a80", offset: 0x5c0a80, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_vtable_c8_5c0930", offset: 0x5c0930, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_composite_mode_d0_5c0b00", offset: 0x5c0b00, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_TextRenderNode_render_payload_d8_5c0c20", offset: 0x5c0c20, surface: "bee_text_render_node"},
  {module: "BEE.dll", name: "BEE_render_graph_entry_497510", offset: 0x497510, surface: "bee_render_graph_entry"},
  {module: "BEE.dll", name: "BEE_render_graph_entry_49bba0", offset: 0x49bba0, surface: "bee_render_graph_entry"},
  {module: "BEE.dll", name: "BEE_render_graph_entry_4a2580", offset: 0x4a2580, surface: "bee_render_graph_entry"},
  {module: "BEE.dll", name: "BEE_render_graph_entry_49abf0", offset: 0x49abf0, surface: "bee_render_graph_entry"},
  {module: "BEE.dll", name: "BEE_render_graph_entry_49eee0", offset: 0x49eee0, surface: "bee_render_graph_entry"},
  {module: "BEE.dll", name: "BEE_text_outline_callsite_5bcf29", offset: 0x5bcf29, surface: "bee_text_callsite"},
  {module: "BEE.dll", name: "BEE_text_outline_callsite_5bf1a1", offset: 0x5bf1a1, surface: "bee_text_callsite"},
  {module: "BEE.dll", name: "BEE_text_render_callsite_49779e", offset: 0x49779e, surface: "bee_text_callsite"},
  {module: "BEE.dll", name: "BEE_text_render_callsite_49bfa3", offset: 0x49bfa3, surface: "bee_text_callsite"},
  {module: "BEE.dll", name: "BEE_text_render_callsite_4a2be3", offset: 0x4a2be3, surface: "bee_text_callsite"},
  {module: "BEE.dll", name: "BEE_text_render_callsite_49aeb2", offset: 0x49aeb2, surface: "bee_text_callsite"},
  {module: "BEE.dll", name: "BEE_text_render_callsite_49efed", offset: 0x49efed, surface: "bee_text_callsite"}
];

const BEE_IMPORT_HOOKS = [
  {module: "BEE.dll", name: "BEE_IMPORT_TXT_DrawChar_edf6b0", iatOffset: 0xedf6b0, surface: "bee_text_drawchar_target"}
];

const TXT_IMPORT_HOOKS = [
  {module: "TXT.dll", name: "TXT_IMPORT_PF_TransferRect_694f30", iatOffset: 0x694f30, surface: "txt_pf_transferrect"}
];

const TXT_DYNAMIC_POINTERS = [
  {name: "TXT_malloc_or_realloc_DAT_18087c000", offset: 0x87c000},
  {name: "TXT_free_DAT_18087c008", offset: 0x87c008},
  {name: "TXT_path_retain_DAT_18087c038", offset: 0x87c038},
  {name: "TXT_path_release_DAT_18087c040", offset: 0x87c040},
  {name: "TXT_version_ptr_DAT_18087bd08", offset: 0x87bd08},
  {name: "TXT_bib_table_DAT_18087bd40", offset: 0x87bd40},
  {name: "TXT_bib_make_path_DAT_18087beb8", offset: 0x87beb8},
  {name: "TXT_bib_release_path_DAT_18087bec0", offset: 0x87bec0},
  {name: "TXT_bib_path_vtable_DAT_18087bec8", offset: 0x87bec8},
  {name: "TXT_object_alloc_DAT_18087bff8", offset: 0x87bff8},
  {name: "TXT_are_path_builder_DAT_18087f778", offset: 0x87f778},
  {name: "TXT_are_rasterizer_DAT_18087f780", offset: 0x87f780}
];

function hookEnabled(hook) {
  if (hookProfile === "all") {
    return true;
  }
  if (hookProfile === "bee-text-render") {
    return [
      "bee_text_render_node",
      "bee_text_drawchar_target"
    ].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "txt-drawchar") {
    return [
      "bee_text_drawchar_target",
      "txt_drawchar_are",
      "txt_drawchar_agm",
      "txt_drawchar_fallback",
      "txt_drawchar_outline_core",
      "txt_font_have_outlines",
      "txt_play_char_outlines",
      "txt_play_char_outlines_impl"
    ].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "txt-are-spans") {
    return [
      "bee_text_drawchar_target",
      "txt_drawchar_are",
      "txt_drawchar_outline_core",
      "txt_are_spans",
      "txt_pf_transferrect"
    ].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "txt-are-byte-samples") {
    return [
      "bee_text_drawchar_target",
      "txt_drawchar_are",
      "txt_drawchar_outline_core",
      "txt_are_spans",
      "txt_are_pixel_samples",
      "txt_pf_transferrect"
    ].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "bee-text-raster") {
    return hook.module === "BEE.dll" || [
      "core_text_outlines",
      "core_text_outlines_v2",
      "txt_play_char_outlines",
      "txt_play_char_outlines_impl"
    ].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "txt-source-rect") {
    return hook.module === "TXT.dll" || ["core_widths", "core_bboxes"].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "txt-gridchar") {
    return hook.module === "TXT.dll";
  }
  if (hookProfile === "text-raster") {
    return [
      "core_text_outlines",
      "core_text_outlines_v2",
      "core_text_glyph_record_render",
      "core_text_glyph_run_slice",
      "core_text_emit_glyph_record",
      "txt_play_char_outlines",
      "txt_play_char_outlines_impl",
      "txt_gridchar_cache",
      "txt_gridchar_metrics",
      "txt_gridchar_alignment_bounds",
      "txt_gridchar_bounds_plus",
      "txt_gridchar_line_space",
      "txt_gridchar_metrics_plus",
      "txt_gridchar_render_extent",
      "txt_gridchar_transformed_advance",
      "txt_gridchar_transformed_metrics",
      "core_bboxes",
      "core_widths"
    ].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "text-raster-render-only") {
    return [
      "core_text_outlines",
      "core_text_outlines_v2",
      "core_text_glyph_record_render",
      "core_text_glyph_run_slice",
      "core_text_emit_glyph_record",
      "txt_play_char_outlines",
      "txt_play_char_outlines_impl"
    ].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "font-metrics") {
    return ["font_widths", "font_bboxes", "core_widths", "core_bboxes"].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "source-rect") {
    return [
      "bbox_int",
      "bbox_quick_float",
      "bbox_oriented_float",
      "core_bbox_int",
      "core_bbox_float",
      "core_quick_bbox_float",
      "glyph_row_extract",
      "glyph_pointer_groups",
      "glyph_text_rows",
      "core_bboxes"
    ].indexOf(hook.surface) !== -1;
  }
  return true;
}

function allHooks() {
  return COOLTYPE_HOOKS.concat(TXT_HOOKS).concat(BEE_HOOKS);
}

function selectedHooks(moduleName) {
  return allHooks().filter(function (hook) {
    return hook.module === moduleName && hookEnabled(hook);
  });
}

function safePtr(p) {
  try { return ptr(p); } catch (e) { return null; }
}

function hexptr(p) {
  const q = safePtr(p);
  return q === null ? null : q.toString();
}

function isNullPtr(p) {
  const q = safePtr(p);
  return q === null || q.isNull();
}

function safeReadU8(p) {
  try { return ptr(p).readU8(); } catch (e) { return null; }
}

function safeReadU16(p) {
  try { return ptr(p).readU16(); } catch (e) { return null; }
}

function safeReadS16(p) {
  try { return ptr(p).readS16(); } catch (e) { return null; }
}

function safeReadU32(p) {
  try { return ptr(p).readU32(); } catch (e) { return null; }
}

function safeReadS32(p) {
  try { return ptr(p).readS32(); } catch (e) { return null; }
}

function safeReadS64Number(p) {
  try { return Number(ptr(p).readS64()); } catch (e) { return null; }
}

function nativeArgU32(p) {
  try { return ptr(p).toInt32() >>> 0; } catch (e) { return null; }
}

function nativeArgS32(p) {
  try { return ptr(p).toInt32(); } catch (e) { return null; }
}

function safeReadFloat(p) {
  try { return ptr(p).readFloat(); } catch (e) { return null; }
}

function safeReadDouble(p) {
  try { return ptr(p).readDouble(); } catch (e) { return null; }
}

function safeReadPointer(p) {
  try { return ptr(p).readPointer(); } catch (e) { return null; }
}

function safeReadPointerString(p) {
  const q = safeReadPointer(p);
  return q === null ? null : q.toString();
}

function memoryBytes(p, count) {
  if (isNullPtr(p)) {
    return null;
  }
  try {
    const bytes = new Uint8Array(ptr(p).readByteArray(count));
    const out = [];
    for (let i = 0; i < bytes.length; i++) {
      out.push(("0" + bytes[i].toString(16)).slice(-2));
    }
    return out.join("");
  } catch (e) {
    return null;
  }
}

function memoryWords(p, count) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  const out = [];
  for (let i = 0; i < count; i++) {
    const off = i * 8;
    const r = q.add(off);
    out.push({
      offset: off,
      ptr: safeReadPointerString(r),
      s32_0: safeReadS32(r),
      u32_0: safeReadU32(r),
      f32_0: safeReadFloat(r),
      s32_4: safeReadS32(r.add(4)),
      u32_4: safeReadU32(r.add(4)),
      f32_4: safeReadFloat(r.add(4))
    });
  }
  return out;
}

function readStdVectorF32(p, maxCount) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  const begin = safeReadPointer(q);
  const end = safeReadPointer(q.add(Process.pointerSize));
  const cap = safeReadPointer(q.add(Process.pointerSize * 2));
  let count = null;
  const values = [];
  if (begin !== null && end !== null && !begin.isNull() && !end.isNull()) {
    try {
      count = Number(end.sub(begin)) / 4;
      const n = Math.max(0, Math.min(Math.floor(count), maxCount || 64));
      for (let i = 0; i < n; i++) {
        values.push(safeReadFloat(begin.add(i * 4)));
      }
    } catch (e) {
      count = null;
    }
  }
  return {
    ptr: q.toString(),
    begin: begin === null ? null : begin.toString(),
    end: end === null ? null : end.toString(),
    cap: cap === null ? null : cap.toString(),
    count: count,
    values: values
  };
}

function pointerFieldSamples(p, offsets) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  return offsets.map(function (off) {
    const target = safeReadPointer(q.add(off));
    return {
      offset: off,
      ptr: target === null ? null : target.toString(),
      target: target === null ? null : moduleOffset(target),
      bytes_64: target === null ? null : memoryBytes(target, 64),
      words_8: target === null ? [] : memoryWords(target, 8)
    };
  });
}

function dumpPfWorldForText(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return {
    ptr: q.toString(),
    vtable: safeReadPointerString(q),
    width_0x2c_s32: safeReadS32(q.add(0x2c)),
    height_0x30_s32: safeReadS32(q.add(0x30)),
    words_0x00_0x90: memoryWords(q, 18),
    pointer_fields: pointerFieldSamples(q, [
      0x08, 0x10, 0x18, 0x20, 0x28, 0x38, 0x40, 0x48,
      0x50, 0x58, 0x60, 0x68, 0x70, 0x78, 0x80
    ])
  };
}

function readPixel8(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return {
    ptr: q.toString(),
    a: safeReadU8(q),
    r: safeReadU8(q.add(1)),
    g: safeReadU8(q.add(2)),
    b: safeReadU8(q.add(3)),
    u32: safeReadU32(q),
    raw: memoryBytes(q, 4)
  };
}

function readSpanCells(p, maxCells) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  const n = Math.max(0, Math.min(maxCells || 0, 24));
  const out = [];
  for (let i = 0; i < n; i++) {
    const cell = q.add(i * 8);
    out.push({
      index: i,
      span_type_s32: safeReadS32(cell),
      span_type_u32: safeReadU32(cell),
      end_x_s32: safeReadS32(cell.add(4)),
      end_x_u32: safeReadU32(cell.add(4)),
      raw: memoryBytes(cell, 8)
    });
  }
  return out;
}

function dumpAreObject(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return {
    ptr: q.toString(),
    flags: {
      fill_enabled_0x08: safeReadU8(q.add(0x08)),
      stroke_enabled_0x09: safeReadU8(q.add(0x09)),
      fill_stroke_order_0x0a: safeReadU8(q.add(0x0a)),
      render_context_0x0b: safeReadU8(q.add(0x0b)),
      parity_skip_0x1c: safeReadU32(q.add(0x1c))
    },
    fill_color_f32_0x20: readPixelFloat4(q.add(0x20)),
    stroke_color_f32_0x30: readPixelFloat4(q.add(0x30)),
    maybe_temp_or_path_0x10: safeReadPointerString(q.add(0x10)),
    stroke_width_0x40_f64: safeReadDouble(q.add(0x40)),
    line_join_0x48_s32: safeReadS32(q.add(0x48)),
    miter_limit_0x50_f64: safeReadDouble(q.add(0x50)),
    pf_world_0x58: safeReadPointerString(q.add(0x58)),
    clip_shorts: {
      top_0x60: safeReadS16(q.add(0x60)),
      left_0x62: safeReadS16(q.add(0x62)),
      bottom_0x64: safeReadS16(q.add(0x64)),
      right_0x66: safeReadS16(q.add(0x66))
    },
    matrix6_f32_0x68: readMatrix6(q.add(0x68)),
    words_0x00_0xc0: memoryWords(q, 24)
  };
}

function dumpCoverageObject(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  const plane = safeReadPointer(q.add(0x08));
  const rowGetter = safeReadPointer(q.add(0x10));
  const rowContext = safeReadPointer(q.add(0x18));
  const planeLayout = plane === null ? null : {
    width_0x08_s32: safeReadS32(plane.add(0x08)),
    height_0x0c_s32: safeReadS32(plane.add(0x0c)),
    base_0x10: safeReadPointerString(plane.add(0x10)),
    base_0x28: safeReadPointerString(plane.add(0x28)),
    stride_0x20_s64: safeReadS64Number(plane.add(0x20)),
    stride_0x30_s64: safeReadS64Number(plane.add(0x30)),
    aux_0x38: safeReadPointerString(plane.add(0x38))
  };
  return {
    ptr: q.toString(),
    plane_0x08: plane === null ? null : plane.toString(),
    plane_0x08_target: plane === null ? null : moduleOffset(plane),
    row_getter_0x10: rowGetter === null ? null : rowGetter.toString(),
    row_getter_0x10_target: rowGetter === null ? null : moduleOffset(rowGetter),
    row_context_0x18: rowContext === null ? null : rowContext.toString(),
    row_context_0x18_target: rowContext === null ? null : moduleOffset(rowContext),
    plane_layout: planeLayout,
    plane_base_0x10: planeLayout === null ? null : planeLayout.base_0x10,
    plane_stride_0x20: planeLayout === null ? null : planeLayout.stride_0x20_s64,
    plane_stride_0x30: planeLayout === null ? null : planeLayout.stride_0x30_s64,
    plane_words: plane === null ? [] : memoryWords(plane, 8),
    words: memoryWords(q, 8)
  };
}

function dumpPathTuple(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  const coverage = safeReadPointer(q.add(0x10));
  return {
    ptr: q.toString(),
    words: memoryWords(q, 8),
    coverage_object_0x10: coverage === null ? null : coverage.toString(),
    coverage_object: dumpCoverageObject(coverage)
  };
}

function coverageBytesForSpan(arePtr, tuplePtr, y, startX, endX) {
  if (isNullPtr(arePtr) || isNullPtr(tuplePtr)) {
    return null;
  }
  if (y === null || startX === null || endX === null || endX <= startX) {
    return null;
  }
  try {
    const are = ptr(arePtr);
    const tuple = ptr(tuplePtr);
    const coverage = safeReadPointer(tuple.add(0x10));
    if (coverage === null || coverage.isNull()) {
      return null;
    }
    const plane = safeReadPointer(coverage.add(0x08));
    if (plane === null || plane.isNull()) {
      return null;
    }
    const width = safeReadS32(plane.add(0x08));
    const height = safeReadS32(plane.add(0x0c));
    const base = safeReadPointer(plane.add(0x10));
    const baseMirror = safeReadPointer(plane.add(0x28));
    const strideCandidates = [
      {offset: "0x20", value: safeReadS64Number(plane.add(0x20))},
      {offset: "0x30", value: safeReadS64Number(plane.add(0x30))},
      {offset: "0x20_s32", value: safeReadS32(plane.add(0x20))},
      {offset: "0x30_s32", value: safeReadS32(plane.add(0x30))}
    ];
    const minStride = width === null ? endX - startX : Math.max(width, endX - startX);
    let strideInfo = null;
    for (let i = 0; i < strideCandidates.length; i++) {
      const cand = strideCandidates[i];
      if (cand.value !== null && cand.value >= minStride && cand.value < 1048576) {
        strideInfo = cand;
        break;
      }
    }
    const stride = strideInfo === null ? null : strideInfo.value;
    const top = safeReadS16(are.add(0x60));
    const left = safeReadS16(are.add(0x62));
    if (base === null || base.isNull() || stride === null || top === null || left === null) {
      return null;
    }
    const offset = (y - top) * stride + (startX - left);
    const count = Math.max(0, Math.min(endX - startX, 64));
    const samplePtr = base.add(offset);
    return {
      base: base.toString(),
      base_mirror_0x28: baseMirror === null ? null : baseMirror.toString(),
      width: width,
      height: height,
      stride: stride,
      stride_source: strideInfo.offset,
      stride_candidates: strideCandidates,
      top: top,
      left: left,
      sample_ptr: samplePtr.toString(),
      sample_count: count,
      sample_hex: memoryBytes(samplePtr, count)
    };
  } catch (e) {
    return {error: String(e)};
  }
}

function dumpPlanePointerCandidate(label, p) {
  if (isNullPtr(p)) {
    return {label: label, ptr: ptr(p).toString(), null_ptr: true};
  }
  const q = ptr(p);
  const base10 = safeReadPointer(q.add(0x10));
  const base28 = safeReadPointer(q.add(0x28));
  return {
    label: label,
    ptr: q.toString(),
    target: moduleOffset(q),
    width_0x08_s32: safeReadS32(q.add(0x08)),
    height_0x0c_s32: safeReadS32(q.add(0x0c)),
    base_0x10: base10 === null ? null : base10.toString(),
    stride_0x20_s64: safeReadS64Number(q.add(0x20)),
    base_0x28: base28 === null ? null : base28.toString(),
    stride_0x30_s64: safeReadS64Number(q.add(0x30)),
    aux_0x38: safeReadPointerString(q.add(0x38)),
    words_0x00_0x50: memoryWords(q, 10)
  };
}

function regSnapshotWide(ctx) {
  return {
    rax: ptr(ctx.rax).toString(),
    rbx: ptr(ctx.rbx).toString(),
    rcx: ptr(ctx.rcx).toString(),
    rdx: ptr(ctx.rdx).toString(),
    rsi: ptr(ctx.rsi).toString(),
    rdi: ptr(ctx.rdi).toString(),
    r8: ptr(ctx.r8).toString(),
    r9: ptr(ctx.r9).toString(),
    r10: ptr(ctx.r10).toString(),
    r11: ptr(ctx.r11).toString(),
    rsp: ptr(ctx.rsp).toString()
  };
}

function smallRegisterCount(v) {
  try {
    const n = ptr(v).toInt32();
    if (n >= 0 && n <= 4096) {
      return n;
    }
  } catch (e) {
  }
  return null;
}

function type2SpanCount(ctx, hook) {
  if (hook.name === "TXT_ARE_PixelWriter8_type2_stride_add_3ba5b") {
    return {count: smallRegisterCount(ctx.rax), source: "rax"};
  }
  if (hook.name === "TXT_ARE_PixelWriter8_type2_span_count_3ba71") {
    return {count: smallRegisterCount(ctx.rax), source: "rax"};
  }
  if (hook.name === "TXT_ARE_PixelWriter8_type2_span_ready_3ba74") {
    return {count: smallRegisterCount(ctx.rsi), source: "rsi"};
  }
  return {count: null, source: null};
}

function dumpTxtArePlaneLoadProbe(ctx, hook) {
  const spanInfo = type2SpanCount(ctx, hook);
  const spanCount = spanInfo.count;
  const spanSampleCount = spanCount === null ? null : Math.min(spanCount, 128);
  return {
    hook: hook.name,
    regs: regSnapshotWide(ctx),
    stack: stackArgs(ctx),
    memory_samples: {
      rax_u8_32: memoryBytes(ptr(ctx.rax), 32),
      rbx_u8_32: memoryBytes(ptr(ctx.rbx), 32),
      rdx_u8_32: memoryBytes(ptr(ctx.rdx), 32),
      rdi_u8_32: memoryBytes(ptr(ctx.rdi), 32)
    },
    actual_span_count: spanCount,
    actual_span_count_source: spanInfo.source,
    actual_span_count_rax: smallRegisterCount(ctx.rax),
    actual_span_count_rsi: smallRegisterCount(ctx.rsi),
    actual_span_rbx_hex: spanSampleCount === null ? null : memoryBytes(ptr(ctx.rbx), spanSampleCount),
    actual_span_first_ptr: ptr(ctx.rbx).toString(),
    actual_span_last_ptr: spanCount === null || spanCount <= 0 ? null : ptr(ctx.rbx).add(spanCount - 1).toString(),
    candidates: [
      dumpPlanePointerCandidate("rax", ptr(ctx.rax)),
      dumpPlanePointerCandidate("rbx", ptr(ctx.rbx)),
      dumpPlanePointerCandidate("rcx", ptr(ctx.rcx)),
      dumpPlanePointerCandidate("rdx", ptr(ctx.rdx)),
      dumpPlanePointerCandidate("rsi", ptr(ctx.rsi)),
      dumpPlanePointerCandidate("rdi", ptr(ctx.rdi)),
      dumpPlanePointerCandidate("r8", ptr(ctx.r8)),
      dumpPlanePointerCandidate("r9", ptr(ctx.r9)),
      dumpPlanePointerCandidate("r10", ptr(ctx.r10)),
      dumpPlanePointerCandidate("r11", ptr(ctx.r11))
    ],
    stack_words: memoryWords(ptr(ctx.rsp), 12),
    backtrace: backtrace(ctx)
  };
}

function readMatrix6(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  const out = [];
  for (let i = 0; i < 6; i++) {
    out.push(safeReadFloat(q.add(i * 4)));
  }
  return out;
}

function readBBoxI32(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return [safeReadS32(q), safeReadS32(q.add(4)), safeReadS32(q.add(8)), safeReadS32(q.add(12))];
}

function readBBoxF32(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return [safeReadFloat(q), safeReadFloat(q.add(4)), safeReadFloat(q.add(8)), safeReadFloat(q.add(12))];
}

function readFloatRectD64(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return [safeReadDouble(q), safeReadDouble(q.add(8)), safeReadDouble(q.add(16)), safeReadDouble(q.add(24))];
}

function readVector2D64(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return [safeReadDouble(q), safeReadDouble(q.add(8))];
}

function readMatrix3D64(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  const out = [];
  for (let i = 0; i < 9; i++) {
    out.push(safeReadDouble(q.add(i * 8)));
  }
  return out;
}

function readPixelFloat4(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return [safeReadFloat(q), safeReadFloat(q.add(4)), safeReadFloat(q.add(8)), safeReadFloat(q.add(12))];
}

function dumpGridChar(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return {
    ptr: q.toString(),
    matrix_0x10_d64: readMatrix3D64(q.add(0x10)),
    style_matrix_0x58_d64: readMatrix3D64(q.add(0x58)),
    aux_matrix_0xa0_d64: readMatrix3D64(q.add(0xa0)),
    orientation_0x390: safeReadS32(q.add(0x390)),
    glyph_id_primary_0x394_u32: safeReadU32(q.add(0x394)),
    glyph_id_primary_0x394_s32: safeReadS32(q.add(0x394)),
    alt_char_0x39c_s32: safeReadS32(q.add(0x39c)),
    alt_delta_0x3a4_s16: safeReadU16(q.add(0x3a4)),
    line_group_0x37c: safeReadU32(q.add(0x37c)),
    word_group_0x380: safeReadU32(q.add(0x380)),
    virtual_font_0x3e8: safeReadPointerString(q.add(0x3e8)),
    render_pad_x_0x3d8: safeReadDouble(q.add(0x3d8)),
    render_pad_y_0x3e0: safeReadDouble(q.add(0x3e0)),
    primary_cache_flag_0x3f0: safeReadU32(q.add(0x3f0)),
    primary_bbox_0x3f8_d64: readFloatRectD64(q.add(0x3f8)),
    primary_advance_0x418_d64: readVector2D64(q.add(0x418)),
    alt_cache_flag_0x428: safeReadU32(q.add(0x428)),
    alt_bbox_0x430_d64: readFloatRectD64(q.add(0x430)),
    alt_advance_0x450_d64: readVector2D64(q.add(0x450)),
    raw_0x360_0x460_words: memoryWords(q.add(0x360), 32)
  };
}

function readGlyphRows12(p, count) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  const n = Math.max(0, Math.min(count || 0, 48));
  const out = [];
  for (let i = 0; i < n; i++) {
    const r = q.add(i * 12);
    out.push({
      index: i,
      glyph_id_u32: safeReadU32(r),
      glyph_id_s32: safeReadS32(r),
      x_f32: safeReadFloat(r.add(4)),
      y_f32: safeReadFloat(r.add(8)),
      x_s32: safeReadS32(r.add(4)),
      y_s32: safeReadS32(r.add(8)),
      raw: memoryBytes(r, 12)
    });
  }
  return out;
}

function readBBoxes16(p, count) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  const n = Math.max(0, Math.min(count || 0, 48));
  const out = [];
  for (let i = 0; i < n; i++) {
    const r = q.add(i * 16);
    out.push({
      index: i,
      fixed_i32: readBBoxI32(r),
      float_if_f32: readBBoxF32(r),
      scaled_16_16: [
        safeReadS32(r) === null ? null : safeReadS32(r) / 65536.0,
        safeReadS32(r.add(4)) === null ? null : safeReadS32(r.add(4)) / 65536.0,
        safeReadS32(r.add(8)) === null ? null : safeReadS32(r.add(8)) / 65536.0,
        safeReadS32(r.add(12)) === null ? null : safeReadS32(r.add(12)) / 65536.0
      ],
      raw: memoryBytes(r, 16)
    });
  }
  return out;
}

function readScalarArray(p, count) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  const n = Math.max(0, Math.min(count || 0, 64));
  const out = [];
  for (let i = 0; i < n; i++) {
    const r = q.add(i * 4);
    const s32 = safeReadS32(r);
    out.push({
      index: i,
      s32: s32,
      u32: safeReadU32(r),
      f32: safeReadFloat(r),
      scaled_16_16: s32 === null ? null : s32 / 65536.0
    });
  }
  return out;
}

function readGlyphRows48(p, count) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  const n = Math.max(0, Math.min(count || 0, 32));
  const out = [];
  for (let i = 0; i < n; i++) {
    const r = q.add(i * 48);
    out.push({
      index: i,
      origin_x_s32: safeReadS32(r),
      origin_y_s32: safeReadS32(r.add(4)),
      maybe_size_w: safeReadS32(r.add(24)),
      maybe_size_h: safeReadS32(r.add(28)),
      maybe_payload: safeReadPointerString(r.add(8)),
      words: memoryWords(r, 6),
      raw: memoryBytes(r, 48)
    });
  }
  return out;
}

function readGlyphRecords24(p, count) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  const n = Math.max(0, Math.min(count || 0, 48));
  const out = [];
  for (let i = 0; i < n; i++) {
    const r = q.add(i * 24);
    out.push({
      index: i,
      dword0_s32: safeReadS32(r),
      dword0_u32: safeReadU32(r),
      dword1_s32: safeReadS32(r.add(4)),
      dword1_u32: safeReadU32(r.add(4)),
      glyph_id_u32: safeReadU32(r.add(8)),
      glyph_id_s32: safeReadS32(r.add(8)),
      dword3_s32: safeReadS32(r.add(12)),
      dword3_u32: safeReadU32(r.add(12)),
      dword4_s32: safeReadS32(r.add(16)),
      dword4_u32: safeReadU32(r.add(16)),
      dword5_s32: safeReadS32(r.add(20)),
      dword5_u32: safeReadU32(r.add(20)),
      raw: memoryBytes(r, 24)
    });
  }
  return out;
}

function pointerArrayWithModules(p, count) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  const out = [];
  for (let i = 0; i < count; i++) {
    const entryPtr = q.add(i * Process.pointerSize);
    const target = safeReadPointer(entryPtr);
    out.push({
      index: i,
      ptr: target === null ? null : target.toString(),
      target: target === null ? null : moduleOffset(target)
    });
  }
  return out;
}

function dumpOutlinePlayer(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  const vtable = safeReadPointer(q);
  return {
    ptr: q.toString(),
    vtable: vtable === null ? null : vtable.toString(),
    object_words: memoryWords(q, 16),
    vtable_entries: vtable === null ? [] : pointerArrayWithModules(vtable, 24)
  };
}

function dumpTextRasterSignature(ctx, hook) {
  const stack = stackArgs(ctx);
  const out = {
    hook: hook.name,
    surface: hook.surface,
    regs: regSnapshot(ctx),
    stack: stack
  };
  if (hook.name === "TXT_PlayCharOutlines") {
    out.txt_play_char_outlines = {
      glyph_id_s32: ptr(ctx.rcx).toInt32(),
      glyph_id_u32: nativeArgU32(ctx.rcx),
      glyph_matrix: readMatrix3D64(ptr(ctx.rdx)),
      text_matrix: readMatrix3D64(ptr(ctx.r8)),
      orientation_s32: ptr(ctx.r9).toInt32(),
      font_dict_ptr: stack.p5_0x28,
      synthetic_vector_ptr: stack.p6_0x30,
      outline_player_ptr: stack.p7_0x38,
      outline_player: dumpOutlinePlayer(stack.p7_0x38),
      bool_flag_p8: stack.p8_0x40_s32,
      render_context_p9: safeReadS32(stack.p9_0x48)
    };
  } else if (hook.name === "TXT_PlayCharOutlines_impl") {
    out.txt_play_char_outlines_impl = {
      bool_flag: ptr(ctx.rcx).toInt32(),
      glyph_id_s32: ptr(ctx.rdx).toInt32(),
      glyph_id_u32: nativeArgU32(ctx.rdx),
      glyph_matrix: readMatrix3D64(ptr(ctx.r8)),
      text_matrix: readMatrix3D64(ptr(ctx.r9)),
      orientation_s32: safeReadS32(stack.p5_0x28),
      font_dict_ptr: stack.p6_0x30,
      synthetic_vector_ptr: stack.p7_0x38,
      outline_player_ptr: stack.p8_0x40_ptr,
      outline_player: dumpOutlinePlayer(stack.p8_0x40_ptr),
      render_context_p9: safeReadS32(stack.p9_0x48)
    };
  } else if (hook.name === "CoreTextGlyphRecordRender") {
    const count = ptr(ctx.r8).toInt32();
    out.core_glyph_record_render = {
      font_or_text_ptr: ptr(ctx.rcx).toString(),
      records_ptr: ptr(ctx.rdx).toString(),
      record_count: count,
      records24: readGlyphRecords24(ptr(ctx.rdx), count)
    };
  } else if (hook.name === "CoreTextEmitGlyphRecord") {
    out.core_emit_glyph_record = {
      font_ptr: ptr(ctx.rcx).toString(),
      glyph_id_u32: nativeArgU32(ctx.rdx),
      glyph_id_s32: nativeArgS32(ctx.rdx),
      cache_out_ptr: ptr(ctx.r8).toString(),
      subpixel_or_flags_u32: nativeArgU32(ctx.r9),
      subpixel_or_flags_s32: nativeArgS32(ctx.r9),
      stack_param5_s32: safeReadS32(stack.p5_0x28),
      stack_param5_u32: safeReadU32(stack.p5_0x28),
      stack_param6_ptr: stack.p6_0x30,
      stack_param7_ptr: stack.p7_0x38,
      stack_param8_ptr: stack.p8_0x40_ptr,
      stack_param9_s32: safeReadS32(stack.p9_0x48),
      stack_param10_s32: safeReadS32(stack.p10_0x50)
    };
  } else if (hook.name === "CoreTextOutlines" || hook.name === "CoreTextOutlinesV2") {
    out.core_text_outlines = {
      text_object: dumpTextObject(ptr(ctx.rcx)),
      matrix_arg1: readMatrix6(ptr(ctx.rdx)),
      arg2_ptr: ptr(ctx.r8).toString(),
      flags_arg3_u32: nativeArgU32(ctx.r9)
    };
  }
  return out;
}

function dumpTxtDrawCharOutlineCore(ctx) {
  const stack = stackArgs(ctx);
  const outlinePlayer = stack.p8_0x40_ptr;
  return {
    regs: regSnapshot(ctx),
    bool_or_context_arg1_s32: nativeArgS32(ctx.rcx),
    glyph_id_arg2_u32: nativeArgU32(ctx.rdx),
    glyph_id_arg2_s32: nativeArgS32(ctx.rdx),
    glyph_matrix_arg3: readMatrix3D64(ptr(ctx.r8)),
    text_matrix_arg4: readMatrix3D64(ptr(ctx.r9)),
    orientation_arg5_s32: safeReadS32(stack.p5_0x28),
    font_dict_arg6: stack.p6_0x30,
    font_dict_arg6_words: memoryWords(stack.p6_0x30, 8),
    vector_float_arg7: stack.p7_0x38,
    vector_float_arg7_values: readStdVectorF32(stack.p7_0x38, 64),
    outline_player_arg8: outlinePlayer,
    outline_player_arg8_snapshot: dumpOutlinePlayer(outlinePlayer),
    render_context_arg9_s32: safeReadS32(stack.p9_0x48),
    txt_dynamic_pointers: txtDynamicPointers()
  };
}

function dumpBeePointerSnapshot(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  return {
    ptr: q.toString(),
    words: memoryWords(q, 24),
    pointer_targets: pointerArrayWithModules(q, 16),
    bytes_0x00_0x80: memoryBytes(q, 0x80)
  };
}

function readI32Array(p, count) {
  if (isNullPtr(p)) {
    return [];
  }
  const q = ptr(p);
  const n = Math.max(0, Math.min(count || 0, 64));
  const out = [];
  for (let i = 0; i < n; i++) {
    const r = q.add(i * 4);
    out.push({
      index: i,
      s32: safeReadS32(r),
      u32: safeReadU32(r)
    });
  }
  return out;
}

function dumpBeeRenderNodeSnapshot(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  const sublayerCount = safeReadS32(q.add(0x27c));
  const sublayerStatePtr = safeReadPointer(q.add(0x280));
  return {
    ptr: q.toString(),
    vtable_0x00: moduleOffset(safeReadPointer(q)),
    secondary_cookie_0x10: safeReadPointerString(q.add(0x10)),
    layer_ptr_0x110: safeReadPointerString(q.add(0x110)),
    selected_sublayer_0x118_s32: safeReadS32(q.add(0x118)),
    selected_sublayer_0x118_u32: safeReadU32(q.add(0x118)),
    cache_time_or_key_0x120: safeReadPointerString(q.add(0x120)),
    cache_ptr_0x128: safeReadPointerString(q.add(0x128)),
    render_options_0x130_words: memoryWords(q.add(0x130), 12),
    sublayer_count_0x27c: sublayerCount,
    sublayer_state_ptr_0x280: sublayerStatePtr === null ? null : sublayerStatePtr.toString(),
    sublayer_state_i32: sublayerStatePtr === null ? [] : readI32Array(sublayerStatePtr, sublayerCount || 0),
    text_layer_ptr_0x290: safeReadPointerString(q.add(0x290)),
    refcount_iface_0x298: safeReadPointerString(q.add(0x298)),
    words_0x00_0x180: memoryWords(q, 48),
    words_0x180_0x2c0: memoryWords(q.add(0x180), 40)
  };
}

function dumpBeeTxtDrawCharCallsite(ctx) {
  const stack = preCallStackArgs(ctx);
  const arg0 = nativeArgU32(ctx.rcx);
  return {
    regs: regSnapshot(ctx),
    pre_call_stack: stack,
    field_is_two_arg0_u32: arg0,
    field_is_two_arg0_u8: arg0 === null ? null : arg0 & 0xff,
    clip_or_status_arg1_s32: nativeArgS32(ctx.rdx),
    glyph_id_arg2_u32: nativeArgU32(ctx.r8),
    glyph_id_arg2_s32: nativeArgS32(ctx.r8),
    glyph_matrix_arg3: readMatrix3D64(ptr(ctx.r9)),
    text_matrix_arg5: readMatrix3D64(stack.p5_0x20),
    draw_fill_arg6_s32: stack.p6_0x28_s32,
    draw_stroke_arg7_s32: stack.p7_0x30_s32,
    gridchar_flag_arg8_s32: stack.p8_0x38_s32,
    fill_color_arg9_f32: readPixelFloat4(stack.p9_0x40),
    stroke_color_arg10_f32: readPixelFloat4(stack.p10_0x48),
    stroke_width_arg11_f64: stack.p11_0x50_f64,
    line_join_arg12_s32: stack.p12_0x58_s32,
    miter_limit_arg13_f64: stack.p13_0x60_f64,
    orientation_arg14_s32: stack.p14_0x68_s32,
    font_dict_ref_arg15: stack.p15_0x70,
    font_dict_ref_arg15_words: memoryWords(stack.p15_0x70, 8),
    vector_float_arg16: stack.p16_0x78,
    vector_float_arg16_values: readStdVectorF32(stack.p16_0x78, 64),
    pf_world_arg17: stack.p17_0x80,
    pf_world_arg17_snapshot: dumpPfWorldForText(stack.p17_0x80)
  };
}

function drawCharEntryStackArgs(ctx) {
  const rsp = ptr(ctx.rsp);
  return {
    p5_matrix_0x28: safeReadPointerString(rsp.add(0x28)),
    p6_draw_fill_0x30_s32: safeReadS32(rsp.add(0x30)),
    p7_draw_stroke_0x38_s32: safeReadS32(rsp.add(0x38)),
    p8_gridchar_flag_0x40_s32: safeReadS32(rsp.add(0x40)),
    p9_fill_color_0x48: safeReadPointerString(rsp.add(0x48)),
    p10_stroke_color_0x50: safeReadPointerString(rsp.add(0x50)),
    p11_stroke_width_0x58_f64: safeReadDouble(rsp.add(0x58)),
    p12_line_join_0x60_s32: safeReadS32(rsp.add(0x60)),
    p13_miter_limit_0x68_f64: safeReadDouble(rsp.add(0x68)),
    p14_orientation_0x70_s32: safeReadS32(rsp.add(0x70)),
    p15_font_dict_ref_0x78: safeReadPointerString(rsp.add(0x78)),
    p16_vector_float_0x80: safeReadPointerString(rsp.add(0x80)),
    p17_pf_world_0x88: safeReadPointerString(rsp.add(0x88))
  };
}

function dumpTxtDrawCharEntry(ctx) {
  const stack = drawCharEntryStackArgs(ctx);
  const arg0 = nativeArgU32(ctx.rcx);
  return {
    regs: regSnapshot(ctx),
    stack: stack,
    field_is_two_arg0_u32: arg0,
    field_is_two_arg0_u8: arg0 === null ? null : arg0 & 0xff,
    clip_or_status_arg1_s32: nativeArgS32(ctx.rdx),
    glyph_id_arg2_u32: nativeArgU32(ctx.r8),
    glyph_id_arg2_s32: nativeArgS32(ctx.r8),
    glyph_matrix_arg3: readMatrix3D64(ptr(ctx.r9)),
    text_matrix_arg5: readMatrix3D64(stack.p5_matrix_0x28),
    draw_fill_arg6_s32: stack.p6_draw_fill_0x30_s32,
    draw_stroke_arg7_s32: stack.p7_draw_stroke_0x38_s32,
    gridchar_flag_arg8_s32: stack.p8_gridchar_flag_0x40_s32,
    fill_color_arg9_f32: readPixelFloat4(stack.p9_fill_color_0x48),
    stroke_color_arg10_f32: readPixelFloat4(stack.p10_stroke_color_0x50),
    stroke_width_arg11_f64: stack.p11_stroke_width_0x58_f64,
    line_join_arg12_s32: stack.p12_line_join_0x60_s32,
    miter_limit_arg13_f64: stack.p13_miter_limit_0x68_f64,
    orientation_arg14_s32: stack.p14_orientation_0x70_s32,
    font_dict_ref_arg15: stack.p15_font_dict_ref_0x78,
    font_dict_ref_arg15_words: memoryWords(stack.p15_font_dict_ref_0x78, 8),
    vector_float_arg16: stack.p16_vector_float_0x80,
    vector_float_arg16_values: readStdVectorF32(stack.p16_vector_float_0x80, 64),
    pf_world_arg17: stack.p17_pf_world_0x88,
    pf_world_arg17_snapshot: dumpPfWorldForText(stack.p17_pf_world_0x88)
  };
}

function dumpTxtAreSpanHook(ctx, hook) {
  if (hook.name.indexOf("TXT_ARE_PixelWriter8_type2_") === 0) {
    return dumpTxtArePlaneLoadProbe(ctx, hook);
  }
  const rsp = ptr(ctx.rsp);
  const stack = stackArgs(ctx);
  const base = {
    hook: hook.name,
    regs: regSnapshot(ctx),
    stack: stack,
    are_object_arg1: ptr(ctx.rcx).toString(),
    are_object: dumpAreObject(ptr(ctx.rcx)),
    path_tuple_arg2: ptr(ctx.rdx).toString(),
    path_tuple: dumpPathTuple(ptr(ctx.rdx)),
    source_pixel8_arg3: readPixel8(ptr(ctx.r8)),
    pf_world_arg4: ptr(ctx.r9).toString(),
    pf_world_arg4_snapshot: dumpPfWorldForText(ptr(ctx.r9)),
    stack_words: memoryWords(rsp, 12)
  };
  if (hook.name === "TXT_ARE_PixelWriter8_span_3b8c0") {
    const spanType = nativeArgS32(ctx.r8);
    const y = nativeArgS32(ctx.r9);
    const startX = safeReadS32(rsp.add(0x28));
    const endX = safeReadS32(rsp.add(0x30));
    const sourcePixelPtr = safeReadPointerString(rsp.add(0x38));
    const pfWorldPtr = safeReadPointerString(rsp.add(0x40));
    base.pixel_writer8_span = {
      span_type_arg3_s32: spanType,
      y_arg4_s32: y,
      start_x_arg5_s32: startX,
      end_x_arg6_s32: endX,
      source_pixel_arg7: sourcePixelPtr,
      source_pixel8_arg7: readPixel8(sourcePixelPtr),
      pf_world_arg8: pfWorldPtr,
      pf_world_arg8_snapshot: dumpPfWorldForText(pfWorldPtr),
      coverage_bytes_if_type2: spanType === 2
        ? coverageBytesForSpan(ptr(ctx.rcx), ptr(ctx.rdx), y, startX, endX)
        : null
    };
  } else if (hook.name === "TXT_ARE_OutputComposite_8bpc_3de50") {
    base.output_composite8 = {
      source_pixel_arg3: ptr(ctx.r8).toString(),
      source_pixel8_arg3: readPixel8(ptr(ctx.r8)),
      pf_world_arg4: ptr(ctx.r9).toString(),
      pf_world_arg4_snapshot: dumpPfWorldForText(ptr(ctx.r9))
    };
  } else if (hook.name === "TXT_ARE_Render_8bpc_fill_3d200" || hook.name === "TXT_ARE_Render_8bpc_stroke_3d960") {
    base.render8_fill_or_stroke = {
      clip_or_bounds_arg2_words: memoryWords(ptr(ctx.rdx), 8),
      source_pixel_arg3: ptr(ctx.r8).toString(),
      source_pixel8_arg3: readPixel8(ptr(ctx.r8)),
      pf_world_arg4: ptr(ctx.r9).toString(),
      pf_world_arg4_snapshot: dumpPfWorldForText(ptr(ctx.r9))
    };
  } else if (hook.name === "TXT_ARE_Render_8bpc_3c360") {
    base.render8_dispatch = {
      arg2: ptr(ctx.rdx).toString(),
      arg2_words: memoryWords(ptr(ctx.rdx), 8)
    };
  }
  return base;
}

function dumpBeeSignature(ctx, hook) {
  const stack = stackArgs(ctx);
  return {
    hook: hook.name,
    surface: hook.surface,
    regs: regSnapshot(ctx),
    stack: stack,
    rcx_object: dumpBeePointerSnapshot(ptr(ctx.rcx)),
    rdx_object: dumpBeePointerSnapshot(ptr(ctx.rdx)),
    r8_object: dumpBeePointerSnapshot(ptr(ctx.r8)),
    r9_object: dumpBeePointerSnapshot(ptr(ctx.r9)),
    stack_p5_object: dumpBeePointerSnapshot(stack.p5_0x28),
    stack_p6_object: dumpBeePointerSnapshot(stack.p6_0x30),
    stack_p7_object: dumpBeePointerSnapshot(stack.p7_0x38),
    stack_p8_object: dumpBeePointerSnapshot(stack.p8_0x40_ptr),
    rcx_render_node: dumpBeeRenderNodeSnapshot(ptr(ctx.rcx)),
    rdx_render_node: dumpBeeRenderNodeSnapshot(ptr(ctx.rdx)),
    txt_draw_char_callsite: hook.name === "BEE_TextRenderNode_TXT_DrawChar_callsite_5c100a" ? dumpBeeTxtDrawCharCallsite(ctx) : null,
    backtrace: backtrace(ctx)
  };
}

function dumpPfTransferRectImport(ctx, target) {
  const rsp = ptr(ctx.rsp);
  return {
    regs: regSnapshot(ctx),
    target: moduleOffset(target),
    arg_words: memoryWords(rsp, 18),
    rcx_words: memoryWords(ptr(ctx.rcx), 12),
    rdx_words: memoryWords(ptr(ctx.rdx), 12),
    r8_words: memoryWords(ptr(ctx.r8), 12),
    r9_words: memoryWords(ptr(ctx.r9), 12),
    rcx_world: dumpPfWorldForText(ptr(ctx.rcx)),
    rdx_world: dumpPfWorldForText(ptr(ctx.rdx)),
    r8_world: dumpPfWorldForText(ptr(ctx.r8)),
    r9_world: dumpPfWorldForText(ptr(ctx.r9))
  };
}

function dumpTextObject(p) {
  if (isNullPtr(p)) {
    return null;
  }
  const q = ptr(p);
  const rowBytes = safeReadS32(q.add(0x50));
  const rowCount = rowBytes === null ? null : Math.floor(rowBytes / 12);
  const rowsPtr = safeReadPointer(q.add(0x48));
  return {
    ptr: q.toString(),
    font_ptr_0x10: safeReadPointerString(q.add(0x10)),
    matrix_0x18: readMatrix6(q.add(0x18)),
    flags_0x38: safeReadU32(q.add(0x38)),
    group_0x40: safeReadPointerString(q.add(0x40)),
    rows_ptr_0x48: rowsPtr === null ? null : rowsPtr.toString(),
    rows_bytes_0x50: rowBytes,
    rows_count: rowCount,
    group_0x58: safeReadPointerString(q.add(0x58)),
    group_0x70: safeReadPointerString(q.add(0x70)),
    field_0x88: safeReadU32(q.add(0x88)),
    field_0x10c: safeReadS32(q.add(0x10c)),
    rows12: rowsPtr === null ? [] : readGlyphRows12(rowsPtr, rowCount || 0),
    words_0x00: memoryWords(q, 24)
  };
}

function stackArgs(ctx) {
  const rsp = ptr(ctx.rsp);
  return {
    p5_0x28: safeReadPointerString(rsp.add(0x28)),
    p6_0x30: safeReadPointerString(rsp.add(0x30)),
    p7_0x38: safeReadPointerString(rsp.add(0x38)),
    p8_0x40_ptr: safeReadPointerString(rsp.add(0x40)),
    p8_0x40_s32: safeReadS32(rsp.add(0x40)),
    p9_0x48: safeReadPointerString(rsp.add(0x48)),
    p10_0x50: safeReadPointerString(rsp.add(0x50))
  };
}

function preCallStackArgs(ctx) {
  const rsp = ptr(ctx.rsp);
  return {
    p5_0x20: safeReadPointerString(rsp.add(0x20)),
    p6_0x28_s32: safeReadS32(rsp.add(0x28)),
    p7_0x30_s32: safeReadS32(rsp.add(0x30)),
    p8_0x38_s32: safeReadS32(rsp.add(0x38)),
    p9_0x40: safeReadPointerString(rsp.add(0x40)),
    p10_0x48: safeReadPointerString(rsp.add(0x48)),
    p11_0x50_f64: safeReadDouble(rsp.add(0x50)),
    p12_0x58_s32: safeReadS32(rsp.add(0x58)),
    p13_0x60_f64: safeReadDouble(rsp.add(0x60)),
    p14_0x68_s32: safeReadS32(rsp.add(0x68)),
    p15_0x70: safeReadPointerString(rsp.add(0x70)),
    p16_0x78: safeReadPointerString(rsp.add(0x78)),
    p17_0x80: safeReadPointerString(rsp.add(0x80))
  };
}

function regSnapshot(ctx) {
  return {
    rcx: ptr(ctx.rcx).toString(),
    rdx: ptr(ctx.rdx).toString(),
    r8: ptr(ctx.r8).toString(),
    r9: ptr(ctx.r9).toString(),
    rsp: ptr(ctx.rsp).toString()
  };
}

function moduleOffset(addr) {
  try {
    const p = ptr(addr);
    const m = Process.findModuleByAddress(p);
    if (m === null) {
      return {module: null, offset: null, addr: p.toString()};
    }
    return {module: m.name, offset: p.sub(m.base).toString(), addr: p.toString()};
  } catch (e) {
    return {module: null, offset: null, addr: String(addr), error: String(e)};
  }
}

function modulePointerByOffset(moduleName, offset) {
  try {
    const module = Process.findModuleByName(moduleName);
    if (module === null) {
      return null;
    }
    const slot = module.base.add(offset);
    const value = safeReadPointer(slot);
    return {
      module: moduleName,
      offset: "0x" + offset.toString(16),
      slot: slot.toString(),
      value: value === null ? null : value.toString(),
      target: value === null ? null : moduleOffset(value),
      deref_s32: value === null || value.isNull() ? null : safeReadS32(value)
    };
  } catch (e) {
    return {module: moduleName, offset: "0x" + offset.toString(16), error: String(e)};
  }
}

function txtDynamicPointers() {
  return TXT_DYNAMIC_POINTERS.map(function (entry) {
    return Object.assign({name: entry.name}, modulePointerByOffset("TXT.dll", entry.offset));
  });
}

function backtrace(ctx) {
  try {
    return Thread.backtrace(ctx, Backtracer.ACCURATE).slice(0, 18).map(moduleOffset);
  } catch (e) {
    return [{error: String(e)}];
  }
}

function sendEvent(kind, payload, forceMeta) {
  if (!forceMeta) {
    if (eventCount >= maxEvents) {
      return;
    }
    eventCount += 1;
  }
  send(Object.assign({
    kind: kind,
    seq: forceMeta ? null : eventCount,
    timestamp_ms: Date.now(),
    process: processInfo
  }, payload));
}

function meta(kind, payload) {
  sendEvent(kind, payload, true);
}

function emit(kind, payload) {
  sendEvent(kind, payload, false);
}

function moduleSnapshot(module) {
  const key = module.name + "@" + module.base.toString();
  if (moduleSnapshots[key]) {
    return;
  }
  moduleSnapshots[key] = true;
  meta("cooltype_module_snapshot", {
    module: module.name,
    base: module.base.toString(),
    size: module.size,
    path: module.path,
    hook_profile: hookProfile,
    dynamic_pointers: module.name === "TXT.dll" ? txtDynamicPointers() : [],
    hooks: selectedHooks(module.name).map(function (hook) {
      return {name: hook.name, offset: "0x" + hook.offset.toString(16), address: module.base.add(hook.offset).toString()};
    })
  });
}

function dumpEnterCommon(ctx, hook) {
  const stack = stackArgs(ctx);
  const payload = {
    hook: hook.name,
    surface: hook.surface,
    regs: regSnapshot(ctx),
    stack: stack,
    arg0_text_or_font: ptr(ctx.rcx).toString(),
    arg1: ptr(ctx.rdx).toString(),
    arg2: ptr(ctx.r8).toString(),
    arg3: ptr(ctx.r9).toString(),
    arg1_matrix6: readMatrix6(ptr(ctx.rdx)),
    arg2_bbox_i32_before: readBBoxI32(ptr(ctx.r8)),
    arg2_bbox_f32_before: readBBoxF32(ptr(ctx.r8)),
    arg0_words: memoryWords(ptr(ctx.rcx), 16),
    arg1_words: memoryWords(ptr(ctx.rdx), 8),
    arg2_words: memoryWords(ptr(ctx.r8), 8),
    arg3_words: memoryWords(ptr(ctx.r9), 8),
    backtrace: backtrace(ctx)
  };
  if (hook.name.indexOf("CoreText") === 0) {
    payload.text_object = dumpTextObject(ptr(ctx.rcx));
  }
  if (hook.name === "CoreWidthsBatch" || hook.name === "CoreBBoxBatch") {
    const count = ptr(ctx.r8).toInt32();
    payload.core_metric_count = count;
    payload.core_metric_rows12 = readGlyphRows12(ptr(ctx.rdx), count);
    payload.core_metric_out_bboxes_or_widths_before = readBBoxes16(stack.p5_0x28, count);
  }
  if (hook.name === "CTFontInstanceGetWidths" || hook.name === "CTFontInstanceGetBBoxes") {
    const maybeCount = ptr(ctx.r8).toInt32();
    payload.font_metric_count_arg2 = maybeCount;
    payload.font_metric_rows12_before = readGlyphRows12(ptr(ctx.rdx), maybeCount);
  }
  if (hook.name === "CTTextGetTextGlyphs") {
    const capacity = safeReadS32(ptr(ctx.rsp).add(0x40));
    payload.cttext_textglyphs_capacity = capacity;
    payload.cttext_textglyphs_out_before = readGlyphRows48(stack.p7_0x38, capacity);
  }
  if (hook.module === "TXT.dll") {
    payload.txt_signature = txtSignatureSnapshot(ctx, hook);
  }
  if (hook.surface.indexOf("txt_drawchar_") === 0) {
    payload.txt_draw_char_entry = dumpTxtDrawCharEntry(ctx);
  }
  if (hook.surface === "txt_are_spans") {
    payload.txt_are_spans = dumpTxtAreSpanHook(ctx, hook);
  }
  if (hook.name === "TXT_DrawChar_outline_core_42b80") {
    payload.txt_draw_char_outline_core = dumpTxtDrawCharOutlineCore(ctx);
  }
  if (hook.module === "BEE.dll") {
    payload.bee_signature = dumpBeeSignature(ctx, hook);
  }
  if (hook.surface.indexOf("core_text_") === 0 || hook.surface.indexOf("txt_play_char_outlines") === 0) {
    payload.text_raster_signature = dumpTextRasterSignature(ctx, hook);
  }
  return payload;
}

function txtSignatureSnapshot(ctx, hook) {
  const rcx = ptr(ctx.rcx);
  const rdx = ptr(ctx.rdx);
  const r8 = ptr(ctx.r8);
  const r9 = ptr(ctx.r9);
  const base = {
    hook: hook.name,
    surface: hook.surface,
    regs: regSnapshot(ctx),
    rcx_gridchar_candidate: dumpGridChar(rcx),
    rdx_gridchar_candidate: dumpGridChar(rdx),
    rdx_rect_d64: readFloatRectD64(rdx),
    r8_rect_d64: readFloatRectD64(r8),
    r9_rect_d64: readFloatRectD64(r9),
    rdx_vec_d64: readVector2D64(rdx),
    r8_vec_d64: readVector2D64(r8),
    r9_vec_d64: readVector2D64(r9),
    r8_matrix3_d64: readMatrix3D64(r8)
  };
  if (hook.name === "TXT_GridChar_GetRenderExtent") {
    base.inferred_gridchar = dumpGridChar(rdx);
    base.return_rect_buffer = readFloatRectD64(rcx);
    base.extra_matrix = readMatrix3D64(r8);
    base.extra_vec = readVector2D64(r9);
  } else if (hook.name === "TXT_GridChar_GetGlyphMetricsPlus") {
    base.inferred_gridchar = dumpGridChar(rcx);
    base.use_primary_bool = ptr(ctx.rdx).toInt32();
    base.out_rect = readFloatRectD64(r8);
    base.out_vec = readVector2D64(r9);
  } else if (hook.name === "TXT_GridChar_GetGlyphBoundsPlus") {
    base.inferred_gridchar = dumpGridChar(rcx);
    base.use_primary_bool = ptr(ctx.rdx).toInt32();
    base.matrix = readMatrix3D64(r8);
    base.out_rect = readFloatRectD64(r9);
  } else {
    base.inferred_gridchar = dumpGridChar(rcx);
    base.out_rect = readFloatRectD64(rdx);
    base.out_vec = readVector2D64(rdx);
  }
  return base;
}

function installHook(module, hook) {
  const key = hook.name + "@" + hook.offset.toString(16);
  if (installedHooks[key]) {
    return;
  }
  const address = module.base.add(hook.offset);
  try {
    Interceptor.attach(address, {
      onEnter: function () {
        this.hook = hook;
        this.ctx = {
          rcx: ptr(this.context.rcx),
          rdx: ptr(this.context.rdx),
          r8: ptr(this.context.r8),
          r9: ptr(this.context.r9),
          rsp: ptr(this.context.rsp)
        };
        this.stack = stackArgs(this.context);
        this.count = null;
        if (hook.name === "CoreWidthsBatch" || hook.name === "CoreBBoxBatch") {
          this.count = ptr(this.context.r8).toInt32();
        } else if (hook.name === "CTFontInstanceGetWidths" || hook.name === "CTFontInstanceGetBBoxes") {
          this.count = ptr(this.context.r8).toInt32();
        } else if (hook.name === "CTTextGetTextGlyphs") {
          this.count = safeReadS32(ptr(this.context.rsp).add(0x40));
        }
        emit("cooltype_hook_enter", dumpEnterCommon(this.context, hook));
      },
      onLeave: function (retval) {
        const hook = this.hook;
        if (hook.name.indexOf("TXT_ARE_PixelWriter8_type2_") === 0) {
          return;
        }
        const stack = this.stack || {};
        const payload = {
          hook: hook.name,
          surface: hook.surface,
          retval: ptr(retval).toString(),
          arg0_text_or_font: this.ctx.rcx.toString(),
          arg1: this.ctx.rdx.toString(),
          arg2: this.ctx.r8.toString(),
          arg3: this.ctx.r9.toString(),
          stack: stack,
          arg2_bbox_i32_after: readBBoxI32(this.ctx.r8),
          arg2_bbox_f32_after: readBBoxF32(this.ctx.r8)
        };
        if (hook.name === "CoreTextFloatBoundingBox" || hook.name === "CoreTextQuickBoundingBox") {
          payload.text_object_after = dumpTextObject(this.ctx.rcx);
          payload.float_bbox_after = readBBoxF32(this.ctx.r8);
        }
        if (hook.name === "CoreTextBoundingBox" || hook.name === "CTTextGetBoundingBox") {
          payload.int_bbox_after = readBBoxI32(this.ctx.r8);
        }
        if (hook.name === "CTTextGetQuickBoundingBox" || hook.name === "CTTextGetOrientedBBox") {
          payload.float_bbox_after = readBBoxF32(this.ctx.r8);
        }
        if (hook.name === "CoreWidthsBatch") {
          payload.core_width_rows12_after = readGlyphRows12(this.ctx.rdx, this.count || 0);
          payload.core_width_out_words_after = memoryWords(stack.p5_0x28, Math.min(this.count || 0, 16));
          payload.core_width_out_candidates_after = {
            r9: readScalarArray(this.ctx.r9, this.count || 0),
            stack_p5: readScalarArray(stack.p5_0x28, this.count || 0),
            stack_p5_minus_4: isNullPtr(stack.p5_0x28) ? [] : readScalarArray(ptr(stack.p5_0x28).sub(4), this.count || 0),
            stack_p8: readScalarArray(stack.p8_0x40_ptr, this.count || 0),
            stack_p9: readScalarArray(stack.p9_0x48, this.count || 0)
          };
        }
        if (hook.name === "CoreBBoxBatch") {
          payload.core_bbox_rows12_after = readGlyphRows12(this.ctx.rdx, this.count || 0);
          payload.core_bbox_out_after = readBBoxes16(stack.p5_0x28, this.count || 0);
        }
        if (hook.name === "CTFontInstanceGetWidths" || hook.name === "CTFontInstanceGetBBoxes") {
          payload.font_metric_rows12_after = readGlyphRows12(this.ctx.rdx, this.count || 0);
        }
        if (hook.name === "CTTextGetTextGlyphs") {
          payload.cttext_textglyphs_out_after = readGlyphRows48(stack.p7_0x38, this.count || 0);
          payload.cttext_status_out = safeReadU32(stack.p9_0x48);
        }
        if (hook.name === "CTTextGetGlyphs_V2") {
          payload.group1_ptr_out = safeReadPointerString(this.ctx.rdx);
          payload.group2_ptr_out = safeReadPointerString(this.ctx.r8);
          payload.group3_ptr_out = safeReadPointerString(this.ctx.r9);
          payload.group1_desc = memoryWords(safeReadPointer(this.ctx.rdx), 8);
          payload.group2_desc = memoryWords(safeReadPointer(this.ctx.r8), 8);
          payload.group3_desc = memoryWords(safeReadPointer(this.ctx.r9), 8);
        }
        if (hook.module === "TXT.dll") {
          payload.txt_signature_after = txtSignatureSnapshot(this.ctx, hook);
        }
        if (hook.surface.indexOf("txt_drawchar_") === 0) {
          payload.txt_draw_char_entry_after = dumpTxtDrawCharEntry(this.ctx);
        }
        if (hook.surface === "txt_are_spans") {
          payload.txt_are_spans_after = dumpTxtAreSpanHook(this.ctx, hook);
        }
        if (hook.name === "TXT_DrawChar_outline_core_42b80") {
          payload.txt_draw_char_outline_core_after = dumpTxtDrawCharOutlineCore(this.ctx);
        }
        if (hook.module === "BEE.dll") {
          payload.bee_signature_after = dumpBeeSignature(this.ctx, hook);
        }
        if (hook.surface.indexOf("core_text_") === 0 || hook.surface.indexOf("txt_play_char_outlines") === 0) {
          payload.text_raster_signature_after = dumpTextRasterSignature(this.ctx, hook);
          if (hook.name === "CoreTextEmitGlyphRecord") {
            payload.core_emit_cache_out_after = memoryWords(this.ctx.r8, 8);
          }
        }
        emit("cooltype_hook_leave", payload);
      }
    });
    installedHooks[key] = true;
    meta("cooltype_hook_installed", {
      hook: hook.name,
      surface: hook.surface,
      offset: "0x" + hook.offset.toString(16),
      address: address.toString()
    });
  } catch (e) {
    installedHooks[key] = true;
    meta("cooltype_hook_error", {
      hook: hook.name,
      offset: "0x" + hook.offset.toString(16),
      address: address.toString(),
      error: String(e)
    });
  }
}

function installBeeImportHooks(module) {
  BEE_IMPORT_HOOKS.filter(hookEnabled).forEach(function (hook) {
    const iatAddress = module.base.add(hook.iatOffset);
    const target = safeReadPointer(iatAddress);
    if (target === null || target.isNull()) {
      return;
    }
    const key = hook.name + "@" + target.toString();
    if (installedHooks[key]) {
      return;
    }
    try {
      Interceptor.attach(target, {
        onEnter: function () {
          this.hook = hook;
          this.ctx = {
            rcx: ptr(this.context.rcx),
            rdx: ptr(this.context.rdx),
            r8: ptr(this.context.r8),
            r9: ptr(this.context.r9),
            rsp: ptr(this.context.rsp)
          };
          emit("cooltype_hook_enter", {
            hook: hook.name,
            surface: hook.surface,
            regs: regSnapshot(this.context),
            target: moduleOffset(target),
            iat_address: iatAddress.toString(),
            txt_draw_char_entry: dumpTxtDrawCharEntry(this.context),
            backtrace: backtrace(this.context)
          });
        },
        onLeave: function (retval) {
          emit("cooltype_hook_leave", {
            hook: hook.name,
            surface: hook.surface,
            retval: ptr(retval).toString(),
            target: moduleOffset(target),
            txt_draw_char_entry_after: dumpTxtDrawCharEntry(this.ctx)
          });
        }
      });
      installedHooks[key] = true;
      meta("cooltype_hook_installed", {
        hook: hook.name,
        surface: hook.surface,
        iat_offset: "0x" + hook.iatOffset.toString(16),
        iat_address: iatAddress.toString(),
        target: target.toString(),
        target_module: moduleOffset(target)
      });
    } catch (e) {
      installedHooks[key] = true;
      meta("cooltype_hook_error", {
        hook: hook.name,
        iat_offset: "0x" + hook.iatOffset.toString(16),
        iat_address: iatAddress.toString(),
        target: target.toString(),
        target_module: moduleOffset(target),
        error: String(e)
      });
    }
  });
}

function installTxtImportHooks(module) {
  TXT_IMPORT_HOOKS.filter(hookEnabled).forEach(function (hook) {
    const iatAddress = module.base.add(hook.iatOffset);
    const target = safeReadPointer(iatAddress);
    if (target === null || target.isNull()) {
      return;
    }
    const key = hook.name + "@" + target.toString();
    if (installedHooks[key]) {
      return;
    }
    try {
      Interceptor.attach(target, {
        onEnter: function () {
          this.hook = hook;
          this.target = target;
          emit("cooltype_hook_enter", {
            hook: hook.name,
            surface: hook.surface,
            iat_address: iatAddress.toString(),
            target: moduleOffset(target),
            pf_transfer_rect: dumpPfTransferRectImport(this.context, target),
            backtrace: backtrace(this.context)
          });
        },
        onLeave: function (retval) {
          emit("cooltype_hook_leave", {
            hook: hook.name,
            surface: hook.surface,
            retval: ptr(retval).toString(),
            target: moduleOffset(target)
          });
        }
      });
      installedHooks[key] = true;
      meta("cooltype_hook_installed", {
        hook: hook.name,
        surface: hook.surface,
        iat_offset: "0x" + hook.iatOffset.toString(16),
        iat_address: iatAddress.toString(),
        target: target.toString(),
        target_module: moduleOffset(target)
      });
    } catch (e) {
      installedHooks[key] = true;
      meta("cooltype_hook_error", {
        hook: hook.name,
        iat_offset: "0x" + hook.iatOffset.toString(16),
        iat_address: iatAddress.toString(),
        target: target.toString(),
        target_module: moduleOffset(target),
        error: String(e)
      });
    }
  });
}

function installAll() {
  ["CoolType.dll", "TXT.dll", "BEE.dll"].forEach(function (moduleName) {
    const module = Process.findModuleByName(moduleName);
    if (module === null) {
      return;
    }
    moduleSnapshot(module);
    selectedHooks(module.name).forEach(function (hook) {
      installHook(module, hook);
    });
    if (module.name === "BEE.dll") {
      installBeeImportHooks(module);
    }
    if (module.name === "TXT.dll") {
      installTxtImportHooks(module);
    }
  });
}

if (typeof Process.attachModuleObserver === "function") {
  Process.attachModuleObserver({
    onAdded: function (module) {
      if (module.name === "CoolType.dll" || module.name === "TXT.dll" || module.name === "BEE.dll") {
        installAll();
      }
    }
  });
}
installAll();
setInterval(installAll, 500);
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--duration", type=float, default=180.0)
    ap.add_argument("--attach-delay", type=float, default=0.0)
    ap.add_argument("--max-events", type=int, default=1200)
    ap.add_argument("--hook-profile", choices=["source-rect", "font-metrics", "txt-source-rect", "txt-gridchar", "text-raster", "text-raster-render-only", "bee-text-raster", "bee-text-render", "txt-drawchar", "txt-are-spans", "all"], default="source-rect")
    # Accepted for compatibility with ae_trace_drop_shadow_softness helpers.
    ap.add_argument("--generic-hook-limit", type=int, default=0)
    ap.add_argument("--max-stalk-render-calls", type=int, default=0)
    ap.add_argument("--broad-coverage", action="store_true")
    ap.add_argument("--stalk-transform-render", action="store_true")
    ap.add_argument("--offset-hook", action="append", default=[])
    args = ap.parse_args()

    script_text = JS.replace("MAX_EVENTS_PLACEHOLDER", str(args.max_events))
    script_text = script_text.replace("HOOK_PROFILE_PLACEHOLDER", args.hook_profile)

    with open(args.out, "a", encoding="utf-8") as f:
        f.write(json.dumps({"kind": "trace_start", "duration": args.duration, "attach_delay": args.attach_delay}) + "\n")
        f.flush()
        if args.attach_delay > 0:
            time.sleep(args.attach_delay)

        def on_message(message, data):
            f.write(json.dumps(message, ensure_ascii=False) + "\n")
            f.flush()

        attached = {}
        device = frida.get_local_device()

        def should_attach(proc):
          return proc.name.lower() in {"afterfx.exe", "afterfx.com", "aerender.exe"}

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
                f.write(json.dumps({
                    "kind": "trace_attach_error",
                    "pid": proc.pid,
                    "name": proc.name,
                    "error": repr(exc),
                }) + "\n")
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


def parse_trace(text: str) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
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


def summarize_events(case_id: str, events: list[dict[str, Any]]) -> dict[str, Any]:
    hooks_installed = [
        e for e in events if e.get("kind") == "cooltype_hook_installed"
    ]
    hook_events = [
        e for e in events if e.get("kind") in {"cooltype_hook_enter", "cooltype_hook_leave"}
    ]
    counts: dict[str, int] = {}
    first_by_hook: dict[str, dict[str, Any]] = {}
    bbox_samples: dict[str, Any] = {}
    glyph_row_samples: dict[str, Any] = {}

    def compact_event(event: dict[str, Any]) -> dict[str, Any]:
        rows = (
            event.get("cttext_textglyphs_out_after")
            or event.get("core_bbox_rows12_after")
            or event.get("core_width_rows12_after")
            or event.get("font_metric_rows12_after")
            or event.get("core_metric_rows12")
            or []
        )
        backtrace = event.get("backtrace") or []
        return {
            "kind": event.get("kind"),
            "seq": event.get("seq"),
            "hook": event.get("hook"),
            "surface": event.get("surface"),
            "process": event.get("process"),
            "regs": event.get("regs"),
            "stack": event.get("stack"),
            "retval": event.get("retval"),
            "core_metric_count": event.get("core_metric_count"),
            "glyph_rows_sample": rows[:8] if isinstance(rows, list) else [],
            "bbox_sample": (event.get("core_bbox_out_after") or [])[:4],
            "width_candidates_sample": event.get("core_width_out_candidates_after"),
            "txt_are_spans": event.get("txt_are_spans") or event.get("txt_are_spans_after"),
            "pf_transfer_rect": event.get("pf_transfer_rect"),
            "backtrace": backtrace[:10] if isinstance(backtrace, list) else [],
        }

    for event in hook_events:
        hook = str(event.get("hook") or "")
        counts[hook] = counts.get(hook, 0) + 1
        first_by_hook.setdefault(hook, compact_event(event))
        if event.get("kind") == "cooltype_hook_leave":
            if hook not in bbox_samples and (
                event.get("float_bbox_after") is not None
                or event.get("int_bbox_after") is not None
            ):
                bbox_samples[hook] = {
                    "int_bbox_after": event.get("int_bbox_after"),
                    "float_bbox_after": event.get("float_bbox_after"),
                    "arg0": event.get("arg0_text_or_font"),
                }
            if hook not in glyph_row_samples:
                rows = (
                    event.get("cttext_textglyphs_out_after")
                    or event.get("core_bbox_rows12_after")
                    or event.get("core_width_rows12_after")
                    or event.get("font_metric_rows12_after")
                )
                if rows:
                    glyph_row_samples[hook] = rows[:8]

    return {
        "case_id": case_id,
        "event_count": len(events),
        "hooks_installed": [e.get("hook") for e in hooks_installed],
        "hook_event_counts": counts,
        "bbox_samples": bbox_samples,
        "glyph_row_samples": glyph_row_samples,
        "first_events": first_by_hook,
        "trace_errors": [e for e in events if str(e.get("kind", "")).endswith("_error")][:12],
    }


def batch_case_label(batch: list[str]) -> str:
    if len(batch) == 1:
        return batch[0]
    digest = hashlib.sha1("\n".join(batch).encode("utf-8")).hexdigest()[:12]
    return f"batch_{len(batch)}_{digest}"


def start_remote_cooltype_trace_ssh(
    helpers: Any,
    host: str,
    remote_python: str,
    remote_script: str,
    remote_log: str,
    duration: int,
    max_events: int,
    hook_profile: str,
    attach_delay: int,
) -> subprocess.Popen[str]:
    stdout_log = remote_log + ".stdout.txt"
    stderr_log = remote_log + ".stderr.txt"
    cleanup = "\n".join(
        [
            "$old = Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('python.exe', 'python3.exe') -and $_.CommandLine -like '*ae_trace_drop_shadow_remote.py*' }",
            "foreach ($p in $old) { Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue }",
            f"if (Test-Path {helpers.ps_quote(remote_log)}) {{ Remove-Item -Path {helpers.ps_quote(remote_log)} -Force }}",
            f"if (Test-Path {helpers.ps_quote(stdout_log)}) {{ Remove-Item -Path {helpers.ps_quote(stdout_log)} -Force }}",
            f"if (Test-Path {helpers.ps_quote(stderr_log)}) {{ Remove-Item -Path {helpers.ps_quote(stderr_log)} -Force }}",
        ]
    )
    helpers.run_ps_ssh(host, cleanup, check=False)
    trace_args = helpers.remote_trace_args(
        remote_script=remote_script,
        remote_log=remote_log,
        duration=duration,
        max_events=max_events,
        broad_coverage=False,
        generic_hook_limit=0,
        offset_hooks=[],
        stalk_transform_render=False,
        max_stalk_render_calls=0,
    )
    trace_args.extend(["--hook-profile", hook_profile])
    if attach_delay > 0:
        trace_args.extend(["--attach-delay", str(attach_delay)])
    args_text = " ".join(helpers.ps_quote(arg) for arg in trace_args)
    script = "\n".join(
        [
            f"& {helpers.ps_quote(remote_python)} {args_text} "
            f"> {helpers.ps_quote(stdout_log)} 2> {helpers.ps_quote(stderr_log)}",
            "exit $LASTEXITCODE",
        ]
    )
    proc = subprocess.Popen(
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
        stdin=subprocess.PIPE,
        text=True,
    )
    assert proc.stdin is not None
    proc.stdin.write("$ProgressPreference='SilentlyContinue'\n" + script)
    proc.stdin.close()
    return proc


def main() -> int:
    ap = argparse.ArgumentParser("Trace CoolType text layout calls on AE85 through Frida.")
    ap.add_argument("--ssh-host", default=DEFAULT_SSH_HOST)
    ap.add_argument("--remote-python", default=DEFAULT_REMOTE_PYTHON)
    ap.add_argument("--node", default=DEFAULT_NODE)
    ap.add_argument("--s3-env", default=str(DEFAULT_S3_ENV))
    ap.add_argument("--bucket", default="")
    ap.add_argument("--prefix", default="ae_dynamic_traces/cooltype_text")
    ap.add_argument("--pack", default=str(DEFAULT_PACK))
    ap.add_argument("--entry-script", default=DEFAULT_ENTRY_SCRIPT)
    ap.add_argument("--case", action="append", default=[])
    ap.add_argument("--batch-cases", action="store_true")
    ap.add_argument("--duration", type=int, default=240)
    ap.add_argument("--attach-delay", type=int, default=0)
    ap.add_argument("--max-events", type=int, default=1200)
    ap.add_argument("--hook-profile", choices=["source-rect", "font-metrics", "txt-source-rect", "txt-gridchar", "text-raster", "text-raster-render-only", "bee-text-raster", "bee-text-render", "txt-drawchar", "txt-are-spans", "all"], default="source-rect")
    ap.add_argument("--allow-render-failure", action="store_true")
    ap.add_argument("--out-dir", default="")
    ap.add_argument("--live-tail", action="store_true")
    args = ap.parse_args()

    helpers = load_trace_helpers()
    case_ids = args.case or DEFAULT_CASES
    pack = Path(args.pack).expanduser().resolve()
    entry_script = args.entry_script.strip()
    if not pack.is_dir():
        raise RuntimeError(f"pack dir not found: {pack}")

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    out_dir = Path(args.out_dir or f"target/dynamic_tools_85/cooltype_text_{stamp}").resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    helpers.load_s3_env_file(Path(args.s3_env).expanduser())
    bucket = helpers.resolve_s3_bucket(args.bucket)
    prefix = args.prefix.strip("/").strip()
    s3_root = f"{prefix}/{stamp}" if prefix else stamp

    remote_root = f"C:\\ae_dev\\frida_traces\\cooltype_text_{stamp}"
    # Keep this basename so the reused SSH cleanup helper can stop stale tracers.
    remote_script = remote_root + "\\ae_trace_drop_shadow_remote.py"
    tracer_key = f"{s3_root}/ae_trace_cooltype_text_remote.py"
    helpers.upload_s3_text(bucket=bucket, key=tracer_key, text=REMOTE_TRACER, content_type="text/x-python")
    helpers.install_remote_script_from_url_ssh(
        args.ssh_host,
        remote_script,
        helpers.presign_s3_get(bucket=bucket, key=tracer_key, expires_s=3600),
    )

    summaries: list[dict[str, Any]] = []
    case_batches = [case_ids] if args.batch_cases else [[case_id] for case_id in case_ids]
    for batch in case_batches:
        label = batch_case_label(batch)
        remote_log = remote_root + f"\\{label}.jsonl"
        trace_proc = start_remote_cooltype_trace_ssh(
            helpers,
            args.ssh_host,
            args.remote_python,
            remote_script,
            remote_log,
            args.duration,
            args.max_events,
            args.hook_profile,
            args.attach_delay,
        )
        tail_proc = helpers.start_remote_tail_ssh(args.ssh_host, remote_log) if args.live_tail else None
        render_error: str | None = None
        try:
            helpers.run_cases(batch, f"ae_trace_cooltype_{label}_{stamp}", args.node, pack, entry_script)
        except Exception as exc:
            render_error = repr(exc)
            helpers.capture_failure_screenshot_ssh(args.ssh_host, out_dir, f"cooltype_{label}_failure")
        finally:
            helpers.stop_ssh_process(trace_proc)
            helpers.stop_remote_traces_ssh(args.ssh_host)
            helpers.stop_tail(tail_proc)

        trace_text = helpers.fetch_remote_text_if_exists_ssh(args.ssh_host, remote_log)
        local_log = out_dir / f"{label}.jsonl"
        local_log.write_text(trace_text, encoding="utf-8")
        helpers.upload_s3_text(
            bucket=bucket,
            key=f"{s3_root}/{label}.jsonl",
            text=trace_text,
            content_type="application/jsonl",
        )
        summary = summarize_events(label, parse_trace(trace_text))
        summary["log"] = str(local_log)
        summary["s3_key"] = f"{s3_root}/{label}.jsonl"
        summary["hook_profile"] = args.hook_profile
        summary["render_error"] = render_error
        summaries.append(summary)
        print(json.dumps({"event": "case_done", "case_id": label, "summary": summary}, ensure_ascii=False))
        if render_error and not args.allow_render_failure:
            raise RuntimeError(render_error)

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
