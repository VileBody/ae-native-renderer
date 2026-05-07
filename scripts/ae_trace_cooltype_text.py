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
  {module: "CoolType.dll", name: "CTTextGetGlyphs_V2", offset: 0x2a02a0, surface: "glyph_pointer_groups"},
  {module: "CoolType.dll", name: "CTTextGetTextGlyphs", offset: 0x2a1380, surface: "glyph_text_rows"},
  {module: "CoolType.dll", name: "CTFontInstanceGetWidths", offset: 0x292d20, surface: "font_widths"},
  {module: "CoolType.dll", name: "CTFontInstanceGetBBoxes", offset: 0x291900, surface: "font_bboxes"},
  {module: "CoolType.dll", name: "CoreWidthsBatch", offset: 0x15ac10, surface: "core_widths"},
  {module: "CoolType.dll", name: "CoreBBoxBatch", offset: 0x15a870, surface: "core_bboxes"}
];

const TXT_HOOKS = [
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

function hookEnabled(hook) {
  if (hookProfile === "all") {
    return true;
  }
  if (hookProfile === "txt-source-rect") {
    return hook.module === "TXT.dll" || ["core_widths", "core_bboxes"].indexOf(hook.surface) !== -1;
  }
  if (hookProfile === "txt-gridchar") {
    return hook.module === "TXT.dll";
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
  return COOLTYPE_HOOKS.concat(TXT_HOOKS);
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

function safeReadU32(p) {
  try { return ptr(p).readU32(); } catch (e) { return null; }
}

function safeReadS32(p) {
  try { return ptr(p).readS32(); } catch (e) { return null; }
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

function installAll() {
  ["CoolType.dll", "TXT.dll"].forEach(function (moduleName) {
    const module = Process.findModuleByName(moduleName);
    if (module === null) {
      return;
    }
    moduleSnapshot(module);
    selectedHooks(module.name).forEach(function (hook) {
      installHook(module, hook);
    });
  });
}

if (typeof Process.attachModuleObserver === "function") {
  Process.attachModuleObserver({
    onAdded: function (module) {
      if (module.name === "CoolType.dll" || module.name === "TXT.dll") {
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
    ap.add_argument("--max-events", type=int, default=1200)
    ap.add_argument("--hook-profile", choices=["source-rect", "font-metrics", "txt-source-rect", "txt-gridchar", "all"], default="source-rect")
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
        f.write(json.dumps({"kind": "trace_start", "duration": args.duration}) + "\n")
        f.flush()

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
    ap.add_argument("--max-events", type=int, default=1200)
    ap.add_argument("--hook-profile", choices=["source-rect", "font-metrics", "txt-source-rect", "txt-gridchar", "all"], default="source-rect")
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
