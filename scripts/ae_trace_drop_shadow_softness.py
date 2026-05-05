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
DEFAULT_SSH_HOST = "ae85"
DEFAULT_REMOTE_PYTHON = r"C:\Python314\python.exe"
DEFAULT_PACK = Path("fixtures/ae_probe_pack/shadow_blur_discriminator")
DEFAULT_ENTRY_SCRIPT = "jsx/build_shadow_blur_discriminator_project.jsx"
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
GET_EFFECT_PROC = "?GetEffectProc@FLT_FCSpec@@UEBAP6AHXZXZ"
SET_EFFECT_PROC = "?SetEffectProc@FLT_FCSpec@@QEAAXP6AHXZ@Z"
BOX_OPTIONS_FACTORIES = [
    "?StandardOptions@BoxBlurOptions@GF@@SA?AU12@HHMM@Z",
    "?StandardOptions@BoxBlurOptions@GF@@SA?AU12@_N00HMM@Z",
    "?StandardOptionsExt@BoxBlurOptions@GF@@SA?AU12@HHMM_N00@Z",
    "?StandardOptionsExt@BoxBlurOptions@GF@@SA?AU12@_N00HMM000@Z",
]
BOX_OPTIONS_SETTERS = [
    "?SetBlurAlphaChannelOnly@BoxBlurOptions@GF@@QEAAXXZ",
    "?SetBlurFlags@BoxBlurOptions@GF@@QEAAXH@Z",
    "?SetBlurRadiusHoriz@BoxBlurOptions@GF@@QEAAXM@Z",
    "?SetBlurRadiusVert@BoxBlurOptions@GF@@QEAAXM@Z",
    "?SetDestAlphaType@BoxBlurOptions@GF@@QEAAXW4AlphaType@2@@Z",
    "?SetIterations@BoxBlurOptions@GF@@QEAAXH@Z",
    "?SetSrcAlphaType@BoxBlurOptions@GF@@QEAAXW4AlphaType@2@@Z",
]
RENDER_EXPORTS = [
    {
        "module": "GPUFoundation.DLL",
        "name": "??0Blur_1DImgOpInfo@GF@@QEAA@W4AlphaType@1@HH0HH@Z",
        "kind": "gf_blur_1d_imgop_ctor",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "??0BoxBlur_1DImgOpInfo@GF@@QEAA@MHHW4AlphaType@1@HHH0HHH_N11@Z",
        "kind": "gf_box_blur_1d_imgop_ctor_full",
    },
    {"module": "GPUFoundation.DLL", "name": FAST_BOX_BLUR, "kind": "gf_fast_box_blur"},
    {
        "module": "GPUFoundation.DLL",
        "name": "?GaussianBlur@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXHHHPEAXHHHUPixelFormat@dvamediatypes@@MM_NW4IR_BlurDirection@@4@Z",
        "kind": "gf_gaussian_blur",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "?SingleChannelBlur@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEAXHUPixelFormat@dvamediatypes@@HHMM_N1@Z",
        "kind": "gf_single_channel_blur",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "?Composite@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXH1HPEAXHUPixelFormat@dvamediatypes@@HHMW4IR_BlendMode@@_N5@Z",
        "kind": "gf_composite",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "??0TransformOperation@GF@@QEAA@USampleQuality@@V?$vector@V?$MatrixT@N@geom@dvacore@@V?$allocator@V?$MatrixT@N@geom@dvacore@@@std@@@std@@_NVFrameGeometry@1@3V?$RectT@H@geom@dvacore@@V?$optional@VMaskGeometry@GF@@@4@N2@Z",
        "kind": "gf_transform_operation_ctor",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "?GetOpacityMultiplier@TransformOperation@GF@@QEBANXZ",
        "kind": "gf_transform_operation_opacity_multiplier",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "?Quality@TransformOperation@GF@@QEBA?BUSampleQuality@@XZ",
        "kind": "gf_transform_operation_quality",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "?TransformToMatrix@GF@@YA?AV?$MatrixT@N@geom@dvacore@@AEBUTransformation@1@VPixelAspectRatio@dvamediatypes@@@Z",
        "kind": "gf_transform_to_matrix",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "?TransformedBounds@GF@@YA?AV?$RectT@H@geom@dvacore@@V234@AEBV?$MatrixT@N@34@@Z",
        "kind": "gf_transformed_bounds",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "?TransformedBoundsUnion@GF@@YA?AV?$RectT@H@geom@dvacore@@V234@AEBV?$vector@V?$MatrixT@N@geom@dvacore@@V?$allocator@V?$MatrixT@N@geom@dvacore@@@std@@@std@@H_N@Z",
        "kind": "gf_transformed_bounds_union",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "?TransformsToMatrices@GF@@YA?AV?$vector@V?$MatrixT@N@geom@dvacore@@V?$allocator@V?$MatrixT@N@geom@dvacore@@@std@@@std@@AEBV?$vector@UTransformation@GF@@V?$allocator@UTransformation@GF@@@std@@@3@VPixelAspectRatio@dvamediatypes@@@Z",
        "kind": "gf_transforms_to_matrices",
    },
    {
        "module": "GPUFoundation.DLL",
        "name": "?TransformWithMotionBlur@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXHHHPEAXHHHUPixelFormat@dvamediatypes@@AEBVTransformOperation@1@@Z",
        "kind": "gf_transform_with_motion_blur",
    },
    {"module": "ImageRenderer.dll", "name": "IR_BoxBlur", "kind": "ir_box_blur"},
    {"module": "ImageRenderer.dll", "name": "IR_GaussianBlur", "kind": "ir_gaussian_blur"},
    {"module": "ImageRenderer.dll", "name": "IR_Composite", "kind": "ir_composite"},
    {"module": "ImageRenderer.dll", "name": "IR_CompositeWithBlendMode", "kind": "ir_composite_with_blend_mode"},
]
CPU_EFFECT_EXPORTS = [
    {
        "module": "FLT.dll",
        "name": "?FLT_GeneralEffectCallPlus@@YAHPEAVFLT_FCSeqSpec@@PEBUT_Time@@HPEAXFFPEAH@Z",
        "kind": "flt_general_effect_call_plus",
    },
    {
        "module": "FLT.dll",
        "name": "?FLT_CompletelyGeneralEffectCall@@YAHPEAVFLT_FCSeqSpec@@PEBUT_Time@@PEAX@Z",
        "kind": "flt_completely_general_effect_call",
    },
    {
        "module": "FLT.dll",
        "name": "?FLT_FastBlur@@YAXPEAUPF_ProgressInfo@@PEAVPF_World@@MMHHHH1@Z",
        "kind": "flt_fast_blur",
    },
    {
        "module": "FLT.dll",
        "name": "?FLT_DirectionalBlur@@YAXPEAUPF_ProgressInfo@@HNNNNPEAUPF_LayerDef@@1@Z",
        "kind": "flt_directional_blur",
    },
    {
        "module": "PF.dll",
        "name": "??$PFp_Convolve@VPF_Pixel8@@@@YAHPEAUPF_ProgressInfo@@PEBV?$PF_WorldX@VPF_Pixel8@@@@PEBUM_LRect@@FFIHPEAH333PEAV1@@Z",
        "kind": "pfp_convolve_pixel8",
    },
    {
        "module": "PF.dll",
        "name": "??0PF_SummedAreaTable@@QEAA@PEBVPF_World@@_N@Z",
        "kind": "pf_summed_area_table_ctor",
    },
    {
        "module": "PF.dll",
        "name": "??0?$PF_HorizontalSumTable@VPF_Pixel8@@@@QEAA@PEBV?$PF_WorldX@VPF_Pixel8@@@@HHNNNNNN_NV?$shared_ptr@$$CBVPF_ColorSettings@@@std@@1@Z",
        "kind": "pf_horizontal_sum_table_pixel8_ctor",
    },
    {
        "module": "BEE.dll",
        "name": "?BEE_WorkQueue_RenderToOutput@@YA_KV?$shared_ptr@VBEE_WorkQueue_Client@@@boost@@V?$shared_ptr@VBEE_WorkQueue_RenderToOutput_Params@@@2@V?$shared_ptr@VBEE_WorkQueueIdList@@@2@AEBV?$function@$$A6AX_KV?$shared_ptr@VBEE_WorkQueue_RenderToOutput_Result@@@boost@@@Z@2@AEBV?$function@$$A6AHW4ItemChangeType@@V?$shared_ptr@VBEE_WorkQueue_Item@@@boost@@@Z@2@@Z",
        "kind": "bee_workqueue_render_to_output",
    },
    {
        "module": "BEE.dll",
        "name": "?BEEp_WorkQueue_GetRenderGuidWithRO@@YA_KAEBV?$shared_ptr@VBEE_WorkQueue_Client@@@boost@@HHAEBV?$basic_string@_WU?$char_traits@_W@std@@U?$STLAllocator@_W@allocator@dvacore@@@std@@AEBVBEE_LayerRenderOptions@@AEBV?$shared_ptr@VBEE_WorkQueueIdList@@@2@AEBV?$function@$$A6AX_KHAEBVGuid@utility@dvacore@@@Z@2@AEBV?$function@$$A6AHW4ItemChangeType@@V?$shared_ptr@VBEE_WorkQueue_Item@@@boost@@@Z@2@PEAVBEE_Project@@@Z",
        "kind": "bee_workqueue_get_render_guid_with_ro",
    },
    {
        "module": "BEE.dll",
        "name": "?BEE_CheckoutLayerFrame@@YAXPEBVBEE_AVLayer@@AEBVBEE_LayerRenderOptions@@W4BEE_LayerCheckoutType@@PEAVBEE_CheckoutReceiptPtr@@PEAF@Z",
        "kind": "bee_checkout_layer_frame",
    },
    {
        "module": "BEE.dll",
        "name": "?BEE_CheckoutCachedLayerFrame@@YA_NPEBVBEE_AVLayer@@AEBVBEE_LayerRenderOptions@@W4BEE_LayerCheckoutType@@W4BEE_CacheHitType@@PEAVBEE_CheckoutReceiptPtr@@PEAF@Z",
        "kind": "bee_checkout_cached_layer_frame",
    },
    {
        "module": "BEE.dll",
        "name": "?BEE_CheckoutOrRenderLayerFrameAsyncRedraw@@YAHPEAVBEE_PFContextAsyncJobManager@@IPEAVBEE_AVLayer@@AEAVBEE_LayerRenderOptions@@PEAVBEE_CheckoutReceiptPtr@@@Z",
        "kind": "bee_checkout_or_render_layer_frame_async_redraw",
    },
    {
        "module": "BEE.dll",
        "name": "?Rasterize2DGraph@BEE_AVLayer@@UEBA?AV?$IntrusivePtr@VRG_RenderNode@@@RefCountedInterface@utility@dvacore@@AEBVBEE_LayerRenderOptions@@PEBVBEE_RenderTrace@@_NPEAVRG_RenderNode@@@Z",
        "kind": "bee_avlayer_rasterize_2d_graph",
    },
    {
        "module": "BEE.dll",
        "name": "?GetOutputWorld@FrameTask@bee@@UEAAPEBVPF_World@@XZ",
        "kind": "bee_frame_task_get_output_world",
    },
    {
        "module": "BEE.dll",
        "name": "?GetNextRenderTask@BEE_CheckoutItemFrameRange_RenderTaskHelper@@UEAA?AV?$shared_ptr@VRenderTask@bee@@@std@@XZ",
        "kind": "bee_get_next_render_task",
    },
    {
        "module": "PIN.dll",
        "name": "?PIN_AddFrame@@YAHPEAVPIN_OutSpec@@HHAEBVPF_MaybeWritableWorld@@PEBUM_LPoint@@AEBV?$shared_ptr@$$CBVPF_ColorSettings@@@std@@PEAUPIN_InterruptFuncs@@E@Z",
        "kind": "pin_add_frame",
    },
    {
        "module": "PIN.dll",
        "name": "?PIN_OutputFrame@@YAHPEAVPIN_OutSpec@@AEBV?$shared_ptr@$$CBVPF_ColorSettings@@@std@@AEBVPF_MaybeWritableWorld@@PEAUPIN_InterruptFuncs@@E@Z",
        "kind": "pin_output_frame",
    },
    {
        "module": "PIN.dll",
        "name": "?PIN_ReserveFrame@@YAHPEAVPIN_OutSpec@@HHPEAE@Z",
        "kind": "pin_reserve_frame",
    },
    {
        "module": "PIN.dll",
        "name": "?PIN_UnReserveFrame@@YAHPEAVPIN_OutSpec@@HHPEAE@Z",
        "kind": "pin_unreserve_frame",
    },
    {
        "module": "PIN.dll",
        "name": "?PIN_ColorManageOutputWorld@@YAX_N0PEAVPIN_ColorSettings@@AEBV?$shared_ptr@$$CBVPF_ColorSettings@@@std@@PEAUPIN_InterruptFuncs@@PEAVPF_World@@H@Z",
        "kind": "pin_color_manage_output_world",
    },
    {
        "module": "PIN.dll",
        "name": "?PIN_ConvertPFWorldToVideoFrame@@YA_NPEBVPF_World@@_N1HHPEAVPIN_VideoFrameWrapper@@@Z",
        "kind": "pin_convert_pf_world_to_video_frame",
    },
]
PLUGIN_ENTRY_EXPORTS = [
    {"module": "Drop_Shadow.aex", "name": "EffectMainExtra", "kind": "drop_shadow_effect_main_extra"},
    {"module": "Box_Blur.aex", "name": "EffectMainExtra", "kind": "box_blur_effect_main_extra"},
    {"module": "Box_Blur.aex", "name": "EffectMainExtra2", "kind": "box_blur_effect_main_extra2"},
    {"module": "Glow.aex", "name": "EffectMain", "kind": "glow_effect_main"},
    {"module": "Transform.aex", "name": "EffectMain", "kind": "transform_effect_main"},
    {"module": "Transform.aex", "name": "EffectMainExtra", "kind": "transform_effect_main_extra"},
    {"module": "Transform.aex", "name": "EffectMainExtra2", "kind": "transform_effect_main_extra2"},
]
OFFSET_HOOKS: list[dict[str, str]] = []

JS = r"""
const maxEvents = MAX_EVENTS_PLACEHOLDER;
const broadCoverage = BROAD_COVERAGE_PLACEHOLDER;
const maxGenericHooks = MAX_GENERIC_HOOKS_PLACEHOLDER;
const processInfo = PROCESS_INFO_PLACEHOLDER;
let eventCount = 0;
let genericHookCount = 0;
const installedHooks = {};
const missingNotified = {};
const genericCallCounts = {};
const moduleSnapshots = {};

const WATCH_MODULES = [
  "GPUFoundation.DLL",
  "ImageRenderer.dll",
  "RendererCPU.dll",
  "PF.dll",
  "FLT.dll",
  "BEE.dll",
  "PIN.dll",
  "MEE.dll",
  "RendererGPU.dll",
  "AfterFXLib.dll",
  "Drop_Shadow.aex",
  "Box_Blur.aex",
  "Glow.aex",
  "Transform.aex"
];
const GENERIC_HOOK_MODULES = {
  "GPUFoundation.DLL": true,
  "ImageRenderer.dll": true,
  "RendererCPU.dll": true,
  "PF.dll": true,
  "FLT.dll": true,
  "BEE.dll": false,
  "PIN.dll": false,
  "MEE.dll": false,
  "Drop_Shadow.aex": true,
  "Box_Blur.aex": true,
  "Glow.aex": true,
  "Transform.aex": true
};
const HOOK_ALL_EXPORT_MODULES = {
  "Drop_Shadow.aex": true,
  "Box_Blur.aex": true,
  "Glow.aex": true,
  "Transform.aex": true
};
const GENERIC_EXPORT_RE = /(blur|box|gauss|alpha|premult|unpremult|compos|blend|shadow|glow|mask|effect|render|world|iterate|filter|kernel|convol|soft|transform|geometry|matrix|matrices|sample|quality|bounds|resampl|resize|pixel|opacity|motion)/i;
const NOISY_CXX_EXPORT_RE = /^\?\?[0148]/;
const boxOptionsFactories = BOX_OPTIONS_FACTORIES_PLACEHOLDER;
const boxOptionsSetters = BOX_OPTIONS_SETTERS_PLACEHOLDER;
const renderExports = RENDER_EXPORTS_PLACEHOLDER;
const cpuEffectExports = CPU_EFFECT_EXPORTS_PLACEHOLDER;
const pluginEntryExports = PLUGIN_ENTRY_EXPORTS_PLACEHOLDER;
const offsetHooks = OFFSET_HOOKS_PLACEHOLDER;
let moduleObserverInstalled = false;
const effectProcHooks = {};
const effectProcCallCounts = {};

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

function safeReadPointer(p) {
  try { return ptr(p).readPointer().toString(); } catch (e) { return null; }
}

function memoryWords(p, count) {
  if (p === null || p === undefined || ptr(p).isNull()) {
    return [];
  }
  const q = ptr(p);
  const out = [];
  for (let i = 0; i < count; i++) {
    const off = i * 8;
    const r = q.add(off);
    out.push({
      offset: off,
      ptr: safeReadPointer(r),
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

function memoryBytes(p, count) {
  if (p === null || p === undefined || ptr(p).isNull()) {
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

function pointerArray(p, count) {
  if (p === null || p === undefined || ptr(p).isNull()) {
    return [];
  }
  const q = ptr(p);
  const out = [];
  for (let i = 0; i < count; i++) {
    const pp = safeReadPointer(q.add(i * Process.pointerSize));
    out.push({index: i, ptr: pp});
  }
  return out;
}

function dumpPfParamDef(p) {
  if (p === null || p === undefined || ptr(p).isNull()) {
    return null;
  }
  const q = ptr(p);
  return {
    ptr: q.toString(),
    words: memoryWords(q, 24)
  };
}

function dumpPfWorldLike(p) {
  if (p === null || p === undefined || ptr(p).isNull()) {
    return null;
  }
  const q = ptr(p);
  return {
    ptr: q.toString(),
    words: memoryWords(q, 20)
  };
}

function dumpAeEffectCall(ctx, moduleName, exportName) {
  const paramsPtr = ptr(ctx.r9);
  const paramPtrs = pointerArray(paramsPtr, 24);
  return {
    module: moduleName,
    export_name: exportName,
    // AE effect entrypoints are normally PF_Cmd, PF_InData*, PF_OutData*, PF_ParamDef*[],
    // then PF_LayerDef* output and extra args on the Windows x64 stack.
    pf_cmd: ptr(ctx.rcx).toString(),
    pf_cmd_s32: ptr(ctx.rcx).toInt32(),
    in_data: ptr(ctx.rdx).toString(),
    out_data: ptr(ctx.r8).toString(),
    params: paramsPtr.toString(),
    output_stack_0x28: safeReadPointer(ctx.rsp.add(0x28)),
    extra_stack_0x30: safeReadPointer(ctx.rsp.add(0x30)),
    in_data_words: memoryWords(ptr(ctx.rdx), 24),
    out_data_words: memoryWords(ptr(ctx.r8), 16),
    params_array: paramPtrs,
    param_defs: paramPtrs.slice(0, 12).map(function (entry) {
      return {index: entry.index, ptr: entry.ptr, def: entry.ptr === null ? null : dumpPfParamDef(ptr(entry.ptr))};
    }),
    output_words: memoryWords(safeReadPointer(ctx.rsp.add(0x28)), 16),
    extra_words: memoryWords(safeReadPointer(ctx.rsp.add(0x30)), 12)
  };
}

function installEffectProcPointer(address, origin) {
  if (address === null || address === undefined) {
    return;
  }
  const p = ptr(address);
  if (p.isNull()) {
    return;
  }
  const loc = moduleOffset(p);
  if (loc.module === null) {
    return;
  }
  const key = p.toString();
  if (effectProcHooks[key]) {
    return;
  }
  try {
    Interceptor.attach(p, {
      onEnter: function () {
        const seen = effectProcCallCounts[key] || 0;
        if (seen >= 80) {
          return;
        }
        effectProcCallCounts[key] = seen + 1;
        const current = moduleOffset(p);
        emit("effect_proc_enter", {
          origin: origin,
          address: p.toString(),
          module: current.module,
          offset: current.offset,
          call_index: seen + 1,
          ae_effect_call: dumpAeEffectCall(this.context, current.module || "unknown", "effect_proc_pointer"),
          backtrace: backtrace(this.context)
        });
      },
      onLeave: function (retval) {
        emit("effect_proc_leave", {
          origin: origin,
          address: p.toString(),
          retval: ptr(retval).toString()
        });
      }
    });
    effectProcHooks[key] = true;
    meta("effect_proc_hook_installed", {
      origin: origin,
      address: p.toString(),
      module: loc.module,
      offset: loc.offset
    });
  } catch (e) {
    effectProcHooks[key] = true;
    meta("effect_proc_hook_error", {
      origin: origin,
      address: p.toString(),
      module: loc.module,
      offset: loc.offset,
      error: String(e)
    });
  }
}

function dumpCpuEffectCall(ctx, target) {
  return {
    module: target.module,
    export_name: target.name,
    cpu_kind: target.kind,
    regs: regSnapshot(ctx),
    stack: stackSnapshot(ctx),
    seq_spec_words: memoryWords(ptr(ctx.rcx), 20),
    time_words: memoryWords(ptr(ctx.rdx), 8),
    r8_words: memoryWords(ptr(ctx.r8), 16),
    r9_words: memoryWords(ptr(ctx.r9), 16),
    rcx_world_like: dumpPfWorldLike(ptr(ctx.rcx)),
    rdx_world_like: dumpPfWorldLike(ptr(ctx.rdx)),
    r8_world_like: dumpPfWorldLike(ptr(ctx.r8)),
    r9_world_like: dumpPfWorldLike(ptr(ctx.r9)),
    stack_0x28_world_like: dumpPfWorldLike(safeReadPointer(ctx.rsp.add(0x28))),
    stack_0x30_world_like: dumpPfWorldLike(safeReadPointer(ctx.rsp.add(0x30))),
    stack_0x38_world_like: dumpPfWorldLike(safeReadPointer(ctx.rsp.add(0x38))),
    backtrace: backtrace(ctx)
  };
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

function dumpBoxBlur1DImgOpInfo(p) {
  if (p === null || p === undefined || ptr(p).isNull()) {
    return null;
  }
  const q = ptr(p);
  return {
    ptr: q.toString(),
    vtable: safeReadPointer(q),
    field_08: safeReadS32(q.add(0x08)),
    field_0c: safeReadS32(q.add(0x0c)),
    field_10: safeReadS32(q.add(0x10)),
    field_14: safeReadS32(q.add(0x14)),
    field_18: safeReadS32(q.add(0x18)),
    field_1c: safeReadS32(q.add(0x1c)),
    radius_float_20: safeReadFloat(q.add(0x20)),
    rounded_radius_24: safeReadS32(q.add(0x24)),
    flags_or_alpha_28: safeReadU32(q.add(0x28)),
    field_38: safeReadS32(q.add(0x38)),
    span_mode_3c: safeReadU32(q.add(0x3c)) & 0xff,
    premult_or_channel_3d: (safeReadU32(q.add(0x3c)) >>> 8) & 0xff,
    bool_3e: (safeReadU32(q.add(0x3c)) >>> 16) & 0xff,
    bool_3f: (safeReadU32(q.add(0x3c)) >>> 24) & 0xff,
    raw_00_80: memoryBytes(q, 0x80),
    words: memoryWords(q, 16)
  };
}

function decodeXmmValue(value) {
  if (value === undefined || value === null) {
    return null;
  }
  const out = {raw: String(value)};
  try {
    if (value instanceof ArrayBuffer) {
      const view = new DataView(value);
      const bytes = new Uint8Array(value);
      out.hex = Array.prototype.map.call(bytes, function (b) {
        return ("0" + b.toString(16)).slice(-2);
      }).join("");
      out.f32_0_le = view.getFloat32(0, true);
      out.f32_1_le = view.getFloat32(4, true);
      out.f64_0_le = view.getFloat64(0, true);
    }
  } catch (e) {
    out.error = String(e);
  }
  return out;
}

function xmmSnapshot(ctx) {
  const names = ["xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7"];
  const out = {};
  names.forEach(function (name) {
    try {
      out[name] = decodeXmmValue(ctx[name]);
    } catch (e) {
      out[name] = {error: String(e)};
    }
  });
  return out;
}

function meta(kind, payload) {
  send(Object.assign({
    kind: kind,
    timestamp_ms: Date.now(),
    process: processInfo
  }, payload));
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
  const key = moduleName + "!" + exportName + "!" + kind;
  if (installedHooks[key]) {
    return;
  }
  const module = Process.findModuleByName(moduleName);
  if (module === null) {
    if (!missingNotified[key]) {
      missingNotified[key] = true;
      meta("hook_missing_module", {module: moduleName, export_name: exportName});
    }
    return;
  }
  const exp = module.enumerateExports().find(function (candidate) {
    return candidate.name === exportName;
  });
  const address = exp === undefined ? null : exp.address;
  if (address === null) {
    if (!missingNotified[key]) {
      missingNotified[key] = true;
      meta("hook_missing", {module: moduleName, export_name: exportName});
    }
    return;
  }
  try {
    install(address);
    installedHooks[key] = true;
    meta("hook_installed", {hook: kind, module: moduleName, export_name: exportName, address: address.toString()});
  } catch (e) {
    installedHooks[key] = true;
    meta("hook_install_error", {hook: kind, module: moduleName, export_name: exportName, error: String(e)});
  }
}

function installKnownHooks() {
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
}

function installBoxOptionHooks() {
  boxOptionsFactories.forEach(function (exportName) {
    hookByExport("GPUFoundation.DLL", exportName, "box_options_factory", function (address) {
      Interceptor.attach(address, {
        onEnter: function () {
          this.retbuf = ptr(this.context.rcx);
          emit("box_options_factory_enter", {
            export_name: exportName,
            retbuf: this.retbuf.toString(),
            has_drop_shadow_frame: hasDropShadowFrame(this.context),
            regs: regSnapshot(this.context),
            stack: stackSnapshot(this.context),
            backtrace: backtrace(this.context)
          });
        },
        onLeave: function () {
          emit("box_options_factory_leave", {
            export_name: exportName,
            retbuf: this.retbuf.toString(),
            options: dumpBoxBlurOptions(this.retbuf)
          });
        }
      });
    });
  });

  boxOptionsSetters.forEach(function (exportName) {
    hookByExport("GPUFoundation.DLL", exportName, "box_options_setter", function (address) {
      Interceptor.attach(address, {
        onEnter: function () {
          this.options = ptr(this.context.rcx);
          emit("box_options_setter_enter", {
            export_name: exportName,
            options_before: dumpBoxBlurOptions(this.options),
            has_drop_shadow_frame: hasDropShadowFrame(this.context),
            regs: regSnapshot(this.context),
            stack: stackSnapshot(this.context),
            backtrace: backtrace(this.context)
          });
        },
        onLeave: function () {
          emit("box_options_setter_leave", {
            export_name: exportName,
            options_after: dumpBoxBlurOptions(this.options)
          });
        }
      });
    });
  });
}

function installRenderExportHooks() {
  renderExports.forEach(function (target) {
    hookByExport(target.module, target.name, "render_export:" + target.kind, function (address) {
      Interceptor.attach(address, {
        onEnter: function () {
          this.thisPtr = ptr(this.context.rcx);
          this.renderKind = target.kind;
          emit("render_export_enter", {
            render_kind: target.kind,
            module: target.module,
            export_name: target.name,
            address: address.toString(),
            has_drop_shadow_frame: hasDropShadowFrame(this.context),
            regs: regSnapshot(this.context),
            xmm: xmmSnapshot(this.context),
            stack: stackSnapshot(this.context),
            box_blur_1d_op_before: target.kind === "gf_box_blur_1d_imgop_ctor_full" ? dumpBoxBlur1DImgOpInfo(this.thisPtr) : null,
            backtrace: backtrace(this.context)
          });
        },
        onLeave: function (retval) {
          if (this.renderKind === "gf_box_blur_1d_imgop_ctor_full") {
            emit("render_export_leave", {
              render_kind: target.kind,
              module: target.module,
              export_name: target.name,
              retval: ptr(retval).toString(),
              box_blur_1d_op_after: dumpBoxBlur1DImgOpInfo(this.thisPtr)
            });
          }
        }
      });
    });
  });
}

function installCpuEffectHooks() {
  cpuEffectExports.forEach(function (target) {
    hookByExport(target.module, target.name, "cpu_effect:" + target.kind, function (address) {
      Interceptor.attach(address, {
        onEnter: function () {
          this.target = target;
          emit("cpu_effect_enter", dumpCpuEffectCall(this.context, target));
        },
        onLeave: function (retval) {
          emit("cpu_effect_leave", {
            module: target.module,
            export_name: target.name,
            cpu_kind: target.kind,
            retval: ptr(retval).toString()
          });
        }
      });
    });
  });
}

function installPluginEntryHooks() {
  pluginEntryExports.forEach(function (target) {
    hookByExport(target.module, target.name, "plugin_entry:" + target.kind, function (address) {
      Interceptor.attach(address, {
        onEnter: function () {
          emit("plugin_entry_enter", {
            module: target.module,
            export_name: target.name,
            plugin_kind: target.kind,
            address: address.toString(),
            offset: ptr(address).sub(Process.findModuleByName(target.module).base).toString(),
            ae_effect_call: dumpAeEffectCall(this.context, target.module, target.name),
            backtrace: backtrace(this.context)
          });
        },
        onLeave: function (retval) {
          emit("plugin_entry_leave", {
            module: target.module,
            export_name: target.name,
            plugin_kind: target.kind,
            retval: ptr(retval).toString()
          });
        }
      });
    });
  });
}

function installEffectProcDiscoveryHooks() {
  hookByExport("FLT.dll", GET_EFFECT_PROC_PLACEHOLDER, "effect_proc_getter", function (address) {
    Interceptor.attach(address, {
      onEnter: function () {
        this.fcSpec = ptr(this.context.rcx);
      },
      onLeave: function (retval) {
        const proc = ptr(retval);
        const loc = moduleOffset(proc);
        emit("effect_proc_get_leave", {
          fc_spec: this.fcSpec.toString(),
          proc: proc.toString(),
          proc_module: loc.module,
          proc_offset: loc.offset
        });
        installEffectProcPointer(proc, "GetEffectProc");
      }
    });
  });
  hookByExport("FLT.dll", SET_EFFECT_PROC_PLACEHOLDER, "effect_proc_setter", function (address) {
    Interceptor.attach(address, {
      onEnter: function () {
        const proc = ptr(this.context.rdx);
        const loc = moduleOffset(proc);
        emit("effect_proc_set_enter", {
          fc_spec: ptr(this.context.rcx).toString(),
          proc: proc.toString(),
          proc_module: loc.module,
          proc_offset: loc.offset,
          backtrace: backtrace(this.context)
        });
        installEffectProcPointer(proc, "SetEffectProc");
      }
    });
  });
}

function installOffsetHooks() {
  if (!broadCoverage || offsetHooks.length === 0) {
    return;
  }
  offsetHooks.forEach(function (target) {
    const key = target.module + "!" + target.offset + "!offset:" + target.kind;
    if (installedHooks[key]) {
      return;
    }
    const module = Process.findModuleByName(target.module);
    if (module === null) {
      return;
    }
    const address = module.base.add(ptr(target.offset));
    try {
      Interceptor.attach(address, {
        onEnter: function () {
          emit("offset_hook_enter", {
            offset_kind: target.kind,
            module: target.module,
            offset: target.offset,
            address: address.toString(),
            regs: regSnapshot(this.context),
            xmm: xmmSnapshot(this.context),
            stack: stackSnapshot(this.context),
            rax_words: memoryWords(ptr(this.context.rax), 20),
            rbx_words: memoryWords(ptr(this.context.rbx), 20),
            rcx_words: memoryWords(ptr(this.context.rcx), 20),
            rdx_words: memoryWords(ptr(this.context.rdx), 20),
            rsi_words: memoryWords(ptr(this.context.rsi), 20),
            rdi_words: memoryWords(ptr(this.context.rdi), 20),
            r8_words: memoryWords(ptr(this.context.r8), 20),
            r9_words: memoryWords(ptr(this.context.r9), 20),
            rcx_world_like: dumpPfWorldLike(ptr(this.context.rcx)),
            rdx_world_like: dumpPfWorldLike(ptr(this.context.rdx)),
            r8_world_like: dumpPfWorldLike(ptr(this.context.r8)),
            r9_world_like: dumpPfWorldLike(ptr(this.context.r9)),
            backtrace: backtrace(this.context)
          });
        }
      });
      installedHooks[key] = true;
      meta("offset_hook_installed", {
        offset_kind: target.kind,
        module: target.module,
        offset: target.offset,
        address: address.toString()
      });
    } catch (e) {
      installedHooks[key] = true;
      meta("offset_hook_error", {
        offset_kind: target.kind,
        module: target.module,
        offset: target.offset,
        address: address.toString(),
        error: String(e)
      });
    }
  });
}

function stackSnapshot(ctx) {
  const offsets = [0x20, 0x28, 0x30, 0x38, 0x40, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70, 0x78, 0x80, 0x88];
  return offsets.map(function (off) {
    const p = ctx.rsp.add(off);
    return {
      offset: off,
      ptr: safeReadPointer(p),
      s32: safeReadS32(p),
      u32: safeReadU32(p),
      f32: safeReadFloat(p)
    };
  });
}

function regSnapshot(ctx) {
  return {
    rax: ptr(ctx.rax).toString(),
    rbx: ptr(ctx.rbx).toString(),
    rcx: ptr(ctx.rcx).toString(),
    rdx: ptr(ctx.rdx).toString(),
    rsi: ptr(ctx.rsi).toString(),
    rdi: ptr(ctx.rdi).toString(),
    rbp: ptr(ctx.rbp).toString(),
    r8: ptr(ctx.r8).toString(),
    r9: ptr(ctx.r9).toString(),
    r10: ptr(ctx.r10).toString(),
    r11: ptr(ctx.r11).toString(),
    r12: ptr(ctx.r12).toString(),
    r13: ptr(ctx.r13).toString(),
    r14: ptr(ctx.r14).toString(),
    r15: ptr(ctx.r15).toString(),
    rsp: ptr(ctx.rsp).toString()
  };
}

function moduleSnapshot(moduleName, module, exports) {
  const key = moduleName + "@" + module.base.toString();
  if (moduleSnapshots[key]) {
    return;
  }
  moduleSnapshots[key] = true;
  const allMatching = exports
    .filter(function (e) { return e.type === "function" && GENERIC_EXPORT_RE.test(e.name); });
  const matching = allMatching
    .slice(0, 120)
    .map(function (e) { return {name: e.name, offset: ptr(e.address).sub(module.base).toString()}; });
  const exportSamples = exports
    .filter(function (e) { return e.type === "function"; })
    .slice(0, 40)
    .map(function (e) { return {name: e.name, offset: ptr(e.address).sub(module.base).toString()}; });
  meta("module_snapshot", {
    module: moduleName,
    base: module.base.toString(),
    size: module.size,
    path: module.path,
    export_count: exports.length,
    matching_export_count: allMatching.length,
    matching_exports: matching,
    export_samples: exportSamples
  });
}

function installGenericExportHook(moduleName, module, exp) {
  if (genericHookCount >= maxGenericHooks) {
    return;
  }
  if (exp.type !== "function") {
    return;
  }
  if (!HOOK_ALL_EXPORT_MODULES[moduleName] && !GENERIC_EXPORT_RE.test(exp.name)) {
    return;
  }
  if ((moduleName === "BEE.dll" || moduleName === "PIN.dll" || moduleName === "MEE.dll") && NOISY_CXX_EXPORT_RE.test(exp.name)) {
    return;
  }
  const key = moduleName + "!" + exp.name + "!generic";
  if (installedHooks[key]) {
    return;
  }
  installedHooks[key] = true;
  genericHookCount += 1;
  try {
    Interceptor.attach(exp.address, {
      onEnter: function () {
        const callKey = key;
        const seen = genericCallCounts[callKey] || 0;
        if (seen >= 3) {
          return;
        }
        genericCallCounts[callKey] = seen + 1;
        emit("generic_export_enter", {
          module: moduleName,
          export_name: exp.name,
          address: exp.address.toString(),
          offset: ptr(exp.address).sub(module.base).toString(),
          call_index: seen + 1,
          has_drop_shadow_frame: hasDropShadowFrame(this.context),
          regs: regSnapshot(this.context),
          stack: stackSnapshot(this.context),
          ae_effect_call: HOOK_ALL_EXPORT_MODULES[moduleName] ? dumpAeEffectCall(this.context, moduleName, exp.name) : null,
          backtrace: backtrace(this.context).slice(0, 16)
        });
      }
    });
    meta("generic_hook_installed", {
      module: moduleName,
      export_name: exp.name,
      address: exp.address.toString(),
      offset: ptr(exp.address).sub(module.base).toString(),
      generic_hook_count: genericHookCount
    });
  } catch (e) {
    meta("generic_hook_error", {
      module: moduleName,
      export_name: exp.name,
      address: exp.address.toString(),
      error: String(e)
    });
  }
}

function installBroadHooks() {
  if (!broadCoverage) {
    return;
  }
  WATCH_MODULES.forEach(function (moduleName) {
    const module = Process.findModuleByName(moduleName);
    if (module === null) {
      return;
    }
    const exports = module.enumerateExports();
    moduleSnapshot(moduleName, module, exports);
    if (!GENERIC_HOOK_MODULES[moduleName]) {
      return;
    }
    exports.forEach(function (exp) {
      installGenericExportHook(moduleName, module, exp);
    });
  });
}

function installAllHooks() {
  installKnownHooks();
  installBoxOptionHooks();
  installRenderExportHooks();
  installCpuEffectHooks();
  installPluginEntryHooks();
  installEffectProcDiscoveryHooks();
  installOffsetHooks();
  installBroadHooks();
}

function installModuleObserver() {
  if (moduleObserverInstalled || typeof Process.attachModuleObserver !== "function") {
    return;
  }
  moduleObserverInstalled = true;
  Process.attachModuleObserver({
    onAdded: function (module) {
      if (WATCH_MODULES.indexOf(module.name) === -1) {
        return;
      }
      meta("module_added", {
        module: module.name,
        base: module.base.toString(),
        size: module.size,
        path: module.path
      });
      installAllHooks();
    }
  });
}

installModuleObserver();
installAllHooks();
setInterval(installAllHooks, 500);
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--duration", type=float, default=90.0)
    ap.add_argument("--max-events", type=int, default=80)
    ap.add_argument("--broad-coverage", action="store_true")
    ap.add_argument("--generic-hook-limit", type=int, default=160)
    ap.add_argument("--offset-hook", action="append", default=[])
    args = ap.parse_args()

    script_text = JS.replace("MAX_EVENTS_PLACEHOLDER", str(args.max_events))
    script_text = script_text.replace("BROAD_COVERAGE_PLACEHOLDER", json.dumps(args.broad_coverage))
    script_text = script_text.replace("MAX_GENERIC_HOOKS_PLACEHOLDER", str(args.generic_hook_limit))
    script_text = script_text.replace("STD_OPTIONS_PLACEHOLDER", json.dumps(STD_OPTIONS))
    script_text = script_text.replace("FAST_BOX_BLUR_PLACEHOLDER", json.dumps(FAST_BOX_BLUR))
    script_text = script_text.replace("SET_ALPHA_ONLY_PLACEHOLDER", json.dumps(SET_ALPHA_ONLY))
    script_text = script_text.replace("GET_EFFECT_PROC_PLACEHOLDER", json.dumps(GET_EFFECT_PROC))
    script_text = script_text.replace("SET_EFFECT_PROC_PLACEHOLDER", json.dumps(SET_EFFECT_PROC))
    script_text = script_text.replace("BOX_OPTIONS_FACTORIES_PLACEHOLDER", json.dumps(BOX_OPTIONS_FACTORIES))
    script_text = script_text.replace("BOX_OPTIONS_SETTERS_PLACEHOLDER", json.dumps(BOX_OPTIONS_SETTERS))
    script_text = script_text.replace("RENDER_EXPORTS_PLACEHOLDER", json.dumps(RENDER_EXPORTS))
    script_text = script_text.replace("CPU_EFFECT_EXPORTS_PLACEHOLDER", json.dumps(CPU_EFFECT_EXPORTS))
    script_text = script_text.replace("PLUGIN_ENTRY_EXPORTS_PLACEHOLDER", json.dumps(PLUGIN_ENTRY_EXPORTS))
    offset_hooks = []
    for item in args.offset_hook:
        module, _, rest = item.partition(":")
        offset, _, kind = rest.partition(":")
        if module and offset.lower().startswith("0x"):
            offset_hooks.append({"module": module, "offset": offset, "kind": kind or (module + "_" + offset)})
    script_text = script_text.replace("OFFSET_HOOKS_PLACEHOLDER", json.dumps(offset_hooks))

    with open(args.out, "a", encoding="utf-8") as f:
        f.write(json.dumps({"kind": "trace_start", "duration": args.duration}) + "\n")
        f.flush()

        def on_message(message, data):
            f.write(json.dumps(message, ensure_ascii=False) + "\n")
            f.flush()

        attached = {}
        device = frida.get_local_device()

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


def run_ps_ssh(host: str, script: str, *, check: bool = True, timeout: float | None = None) -> str:
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
    if check and result.returncode != 0:
        raise RuntimeError(
            f"SSH PowerShell failed ({result.returncode})\nSTDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}"
        )
    return result.stdout


def ps_quote(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def ps_array(values: list[str]) -> str:
    return "@(" + ", ".join(ps_quote(value) for value in values) + ")"


def remote_trace_args(
    *,
    remote_script: str,
    remote_log: str,
    duration: int,
    max_events: int,
    broad_coverage: bool,
    generic_hook_limit: int,
    offset_hooks: list[dict[str, str]],
) -> list[str]:
    args = [
        "-u",
        remote_script,
        "--out",
        remote_log,
        "--duration",
        str(duration),
        "--max-events",
        str(max_events),
        "--generic-hook-limit",
        str(generic_hook_limit),
    ]
    if broad_coverage:
        args.append("--broad-coverage")
    for hook in offset_hooks:
        args.extend(["--offset-hook", f"{hook['module']}:{hook['offset']}:{hook['kind']}"])
    return args


def parse_offset_hook(value: str) -> dict[str, str]:
    parts = value.split(":", 2)
    if len(parts) < 2:
        raise argparse.ArgumentTypeError("offset hook must be MODULE:0xOFFSET[:kind]")
    module = parts[0].strip()
    offset = parts[1].strip()
    kind = parts[2].strip() if len(parts) > 2 else f"{module}_{offset}"
    if not module or not offset.lower().startswith("0x"):
        raise argparse.ArgumentTypeError("offset hook must be MODULE:0xOFFSET[:kind]")
    return {"module": module, "offset": offset, "kind": kind}


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


def install_remote_script_from_url_ssh(host: str, remote_path: str, url: str) -> None:
    run_ps_ssh(
        host,
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


def upload_remote_file_to_url_ssh(host: str, remote_path: str, url: str) -> None:
    run_ps_ssh(
        host,
        "\n".join(
            [
                f"$path = {ps_quote(remote_path)}",
                "if (-not (Test-Path $path)) { exit 2 }",
                "$tmp = $path + '.upload.tmp'",
                "$lastError = $null",
                "for ($i = 0; $i -lt 20; $i++) {",
                "  try {",
                "    $src = [IO.File]::Open($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)",
                "    try {",
                "      $dst = [IO.File]::Open($tmp, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::None)",
                "      try { $src.CopyTo($dst) } finally { $dst.Dispose() }",
                "    } finally { $src.Dispose() }",
                f"    Invoke-WebRequest -Method Put -Uri {ps_quote(url)} -InFile $tmp -UseBasicParsing | Out-Null",
                "    Remove-Item -Path $tmp -Force -ErrorAction SilentlyContinue",
                "    exit 0",
                "  } catch {",
                "    $lastError = $_",
                "    Remove-Item -Path $tmp -Force -ErrorAction SilentlyContinue",
                "    Start-Sleep -Milliseconds 500",
                "  }",
                "}",
                "throw $lastError",
            ]
        ),
    )


def start_remote_trace(
    session: winrm.Session,
    remote_python: str,
    remote_script: str,
    remote_log: str,
    duration: int,
    max_events: int,
    broad_coverage: bool,
    generic_hook_limit: int,
    offset_hooks: list[dict[str, str]],
) -> int:
    stdout_log = remote_log + ".stdout.txt"
    stderr_log = remote_log + ".stderr.txt"
    args_list = ps_array(
        remote_trace_args(
            remote_script=remote_script,
            remote_log=remote_log,
            duration=duration,
            max_events=max_events,
            broad_coverage=broad_coverage,
            generic_hook_limit=generic_hook_limit,
            offset_hooks=offset_hooks,
        )
    )
    ps = "\n".join(
        [
            "$old = Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('python.exe', 'python3.exe') -and $_.CommandLine -like '*ae_trace_drop_shadow_remote.py*' }",
            "foreach ($p in $old) { Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue }",
            f"if (Test-Path {ps_quote(remote_log)}) {{ Remove-Item -Path {ps_quote(remote_log)} -Force }}",
            f"if (Test-Path {ps_quote(stdout_log)}) {{ Remove-Item -Path {ps_quote(stdout_log)} -Force }}",
            f"if (Test-Path {ps_quote(stderr_log)}) {{ Remove-Item -Path {ps_quote(stderr_log)} -Force }}",
            f"$argsList = {args_list}",
            f"$p = Start-Process -FilePath {ps_quote(remote_python)} -ArgumentList $argsList -WindowStyle Hidden -PassThru "
            f"-RedirectStandardOutput {ps_quote(stdout_log)} -RedirectStandardError {ps_quote(stderr_log)}",
            "$p.Id",
        ]
    )
    return int(run_ps(session, ps).strip().splitlines()[-1])


def start_remote_trace_foreground_ssh(
    host: str,
    remote_python: str,
    remote_script: str,
    remote_log: str,
    duration: int,
    max_events: int,
    broad_coverage: bool,
    generic_hook_limit: int,
    offset_hooks: list[dict[str, str]],
) -> subprocess.Popen[str]:
    stdout_log = remote_log + ".stdout.txt"
    stderr_log = remote_log + ".stderr.txt"
    cleanup = "\n".join(
        [
            "$old = Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('python.exe', 'python3.exe') -and $_.CommandLine -like '*ae_trace_drop_shadow_remote.py*' }",
            "foreach ($p in $old) { Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue }",
            f"if (Test-Path {ps_quote(remote_log)}) {{ Remove-Item -Path {ps_quote(remote_log)} -Force }}",
            f"if (Test-Path {ps_quote(stdout_log)}) {{ Remove-Item -Path {ps_quote(stdout_log)} -Force }}",
            f"if (Test-Path {ps_quote(stderr_log)}) {{ Remove-Item -Path {ps_quote(stderr_log)} -Force }}",
        ]
    )
    run_ps_ssh(host, cleanup, check=False)
    trace_args = " ".join(
        ps_quote(arg)
        for arg in remote_trace_args(
            remote_script=remote_script,
            remote_log=remote_log,
            duration=duration,
            max_events=max_events,
            broad_coverage=broad_coverage,
            generic_hook_limit=generic_hook_limit,
            offset_hooks=offset_hooks,
        )
    )
    script = "\n".join(
        [
            f"& {ps_quote(remote_python)} {trace_args} "
            f"> {ps_quote(stdout_log)} 2> {ps_quote(stderr_log)}",
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


def stop_remote_traces_ssh(host: str) -> None:
    run_ps_ssh(
        host,
        "\n".join(
            [
                "$old = Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('python.exe', 'python3.exe') -and $_.CommandLine -like '*ae_trace_drop_shadow_remote.py*' }",
                "foreach ($p in $old) { Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue }",
            ]
        ),
        check=False,
    )


def start_remote_tail_ssh(host: str, remote_path: str) -> subprocess.Popen[str]:
    script = "\n".join(
        [
            f"$path = {ps_quote(remote_path)}",
            "while (-not (Test-Path $path)) { Start-Sleep -Milliseconds 250 }",
            "Get-Content -Path $path -Wait",
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


def stop_tail(proc: subprocess.Popen[str] | None) -> None:
    if proc is None or proc.poll() is not None:
        return
    proc.terminate()
    try:
        proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=3)


def stop_ssh_process(proc: subprocess.Popen[str] | None, *, timeout: float = 8.0) -> None:
    if proc is None:
        return
    try:
        proc.wait(timeout=timeout)
        return
    except subprocess.TimeoutExpired:
        pass
    proc.terminate()
    try:
        proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=3)


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


def fetch_remote_text_ssh(host: str, remote_path: str) -> str:
    ps = "\n".join(
        [
            f"$path = {ps_quote(remote_path)}",
            "if (-not (Test-Path $path)) { exit 2 }",
            "$src = [IO.File]::Open($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)",
            "$ms = New-Object IO.MemoryStream",
            "$src.CopyTo($ms)",
            "$src.Dispose()",
            "$encoded = [Convert]::ToBase64String($ms.ToArray())",
            "$ms.Dispose()",
            "Write-Output $encoded",
        ]
    )
    encoded = run_ps_ssh(host, ps).strip()
    return base64.b64decode(encoded).decode("utf-8", errors="replace")


def fetch_remote_text_if_exists_ssh(host: str, remote_path: str) -> str:
    try:
        return fetch_remote_text_ssh(host, remote_path)
    except Exception:
        return ""


def run_case(case_id: str, job_id: str, node: str, pack: Path, entry_script: str) -> None:
    subprocess.run(
        [
            sys.executable,
            "scripts/ae_remote_pack.py",
            str(pack),
            "--entry-script",
            entry_script,
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


def capture_failure_screenshot_ssh(host: str, out_dir: Path, tag: str) -> None:
    screenshot_dir = out_dir / "screenshots"
    subprocess.run(
        [
            sys.executable,
            "scripts/ae85_screenshot.py",
            "--ssh-host",
            host,
            "--tag",
            tag,
            "--out-dir",
            str(screenshot_dir),
        ],
        check=False,
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
    box_factory = [e for e in events if e.get("kind") == "box_options_factory_leave"]
    box_setter = [e for e in events if e.get("kind") == "box_options_setter_leave"]
    render_exports = [e for e in events if e.get("kind") == "render_export_enter"]
    cpu_effects = [e for e in events if e.get("kind") == "cpu_effect_enter"]
    plugin_entries = [e for e in events if e.get("kind") == "plugin_entry_enter"]
    effect_proc_get = [e for e in events if e.get("kind") == "effect_proc_get_leave"]
    effect_proc_set = [e for e in events if e.get("kind") == "effect_proc_set_enter"]
    effect_proc_entries = [e for e in events if e.get("kind") == "effect_proc_enter"]
    offset_hooks = [e for e in events if e.get("kind") == "offset_hook_enter"]
    generic = [e for e in events if e.get("kind") == "generic_export_enter"]
    ae_effect = [
        e
        for e in generic
        if e.get("module") in {"Drop_Shadow.aex", "Box_Blur.aex", "Glow.aex"}
        and isinstance(e.get("ae_effect_call"), dict)
    ]
    generic_hooks = [e for e in events if e.get("kind") == "generic_hook_installed"]
    modules = [e for e in events if e.get("kind") == "module_snapshot"]
    return {
        "case_id": case_id,
        "standard_options_enter": std[:4],
        "standard_options_leave": leave[:4],
        "set_alpha_only_leave": alpha[:4],
        "fast_box_blur_enter": fast[:4],
        "box_options_factory_leave": box_factory[:8],
        "box_options_setter_leave": box_setter[:8],
        "render_export_enter": render_exports[:12],
        "render_export_count": len(render_exports),
        "cpu_effect_enter": cpu_effects[:12],
        "cpu_effect_count": len(cpu_effects),
        "plugin_entry_enter": plugin_entries[:12],
        "plugin_entry_count": len(plugin_entries),
        "effect_proc_get_leave": effect_proc_get[:12],
        "effect_proc_get_count": len(effect_proc_get),
        "effect_proc_set_enter": effect_proc_set[:12],
        "effect_proc_set_count": len(effect_proc_set),
        "effect_proc_enter": effect_proc_entries[:12],
        "effect_proc_count": len(effect_proc_entries),
        "offset_hook_enter": offset_hooks[:12],
        "offset_hook_count": len(offset_hooks),
        "generic_export_enter": generic[:12],
        "generic_export_count": len(generic),
        "ae_effect_call_enter": ae_effect[:12],
        "ae_effect_call_count": len(ae_effect),
        "generic_hook_count": len(generic_hooks),
        "module_snapshot": modules[:8],
        "event_count": len(events),
    }


def main() -> int:
    ap = argparse.ArgumentParser("Trace AE Drop Shadow softness blur options through Frida on the 85 node.")
    ap.add_argument("--iac-env", default=str(DEFAULT_IAC_ENV))
    ap.add_argument("--server-id", default=DEFAULT_SERVER_ID)
    ap.add_argument("--winrm", default=DEFAULT_WINRM)
    ap.add_argument("--transport", choices=["ssh", "winrm"], default="ssh")
    ap.add_argument("--ssh-host", default=DEFAULT_SSH_HOST)
    ap.add_argument("--remote-python", default=DEFAULT_REMOTE_PYTHON)
    ap.add_argument("--node", default=DEFAULT_NODE)
    ap.add_argument("--s3-env", default=str(DEFAULT_S3_ENV))
    ap.add_argument("--bucket", default="")
    ap.add_argument("--prefix", default="ae_dynamic_traces/drop_shadow_softness")
    ap.add_argument("--pack", default=str(DEFAULT_PACK), help="AE probe pack directory to render")
    ap.add_argument("--entry-script", default=DEFAULT_ENTRY_SCRIPT, help="JSX path relative to --pack")
    ap.add_argument("--case", action="append", default=[])
    ap.add_argument("--duration", type=int, default=90)
    ap.add_argument("--max-events", type=int, default=80)
    ap.add_argument("--broad-coverage", action="store_true", help="Hook blur/composite-like exports in candidate AE modules")
    ap.add_argument("--generic-hook-limit", type=int, default=160, help="Maximum broad generic export hooks per process")
    ap.add_argument(
        "--offset-hook",
        action="append",
        type=parse_offset_hook,
        default=[],
        help="Dangerous opt-in hook, format MODULE:0xOFFSET[:kind]; use one known function start at a time",
    )
    ap.add_argument("--live-tail", action="store_true", help="Stream remote Frida JSONL while the render is running")
    ap.add_argument("--out-dir", default="")
    args = ap.parse_args()

    case_ids = args.case or DEFAULT_CASES
    pack = Path(args.pack).expanduser().resolve()
    entry_script = args.entry_script.strip()
    if not pack.is_dir():
        raise RuntimeError(f"pack dir not found: {pack}")
    if not entry_script:
        raise RuntimeError("--entry-script is required")
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    out_dir = Path(args.out_dir or f"target/dynamic_tools_85/drop_shadow_softness_{stamp}").resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    load_s3_env_file(Path(args.s3_env).expanduser())
    bucket = resolve_s3_bucket(args.bucket)
    prefix = args.prefix.strip("/").strip()
    s3_root = f"{prefix}/{stamp}" if prefix else stamp

    remote_root = f"C:\\ae_dev\\frida_traces\\drop_shadow_softness_{stamp}"
    remote_script = remote_root + "\\ae_trace_drop_shadow_remote.py"
    tracer_key = f"{s3_root}/ae_trace_drop_shadow_remote.py"
    upload_s3_text(bucket=bucket, key=tracer_key, text=REMOTE_TRACER, content_type="text/x-python")
    session: winrm.Session | None = None
    script_url = presign_s3_get(bucket=bucket, key=tracer_key, expires_s=3600)
    if args.transport == "winrm":
        password = timeweb_root_password(Path(args.iac_env).expanduser(), args.server_id)
        session = winrm.Session(args.winrm, auth=("Administrator", password), transport="ntlm")
        install_remote_script_from_url(session, remote_script, script_url)
    else:
        install_remote_script_from_url_ssh(args.ssh_host, remote_script, script_url)

    summaries = []
    for case_id in case_ids:
        remote_log = remote_root + f"\\{case_id}.jsonl"
        pid: int | None = None
        trace_proc: subprocess.Popen[str] | None = None
        if args.transport == "winrm":
            assert session is not None
            pid = start_remote_trace(
                session,
                args.remote_python,
                remote_script,
                remote_log,
                args.duration,
                args.max_events,
                args.broad_coverage,
                args.generic_hook_limit,
                args.offset_hook,
            )
        else:
            trace_proc = start_remote_trace_foreground_ssh(
                args.ssh_host,
                args.remote_python,
                remote_script,
                remote_log,
                args.duration,
                args.max_events,
                args.broad_coverage,
                args.generic_hook_limit,
                args.offset_hook,
            )
        tail_proc = start_remote_tail_ssh(args.ssh_host, remote_log) if args.live_tail and args.transport == "ssh" else None
        time.sleep(3.0)
        job_id = f"ae_trace_{case_id}_{stamp}"
        try:
            run_case(case_id, job_id, args.node, pack, entry_script)
        except Exception:
            if args.transport == "ssh":
                capture_failure_screenshot_ssh(args.ssh_host, out_dir, f"{job_id}_failure")
            raise
        finally:
            if args.transport == "winrm":
                assert session is not None
                assert pid is not None
                run_ps(session, f"Wait-Process -Id {pid} -Timeout 8", check=False)
                run_ps(session, f"Stop-Process -Id {pid} -Force -ErrorAction SilentlyContinue", check=False)
            else:
                stop_ssh_process(trace_proc)
                stop_remote_traces_ssh(args.ssh_host)
            stop_tail(tail_proc)

        log_key = f"{s3_root}/{case_id}.jsonl"
        try:
            if args.transport == "winrm":
                assert session is not None
                upload_remote_file_to_url(
                    session,
                    remote_log,
                    presign_s3_put(bucket=bucket, key=log_key, expires_s=3600),
                )
                trace_text = download_s3_text(bucket=bucket, key=log_key)
            else:
                trace_text = fetch_remote_text_ssh(args.ssh_host, remote_log)
                upload_s3_text(bucket=bucket, key=log_key, text=trace_text, content_type="application/jsonl")
        except Exception as exc:
            if args.transport == "winrm":
                assert session is not None
                stdout = fetch_remote_text_if_exists(session, remote_log + ".stdout.txt")
                stderr = fetch_remote_text_if_exists(session, remote_log + ".stderr.txt")
            else:
                stdout = fetch_remote_text_if_exists_ssh(args.ssh_host, remote_log + ".stdout.txt")
                stderr = fetch_remote_text_if_exists_ssh(args.ssh_host, remote_log + ".stderr.txt")
            raise RuntimeError(
                f"remote Frida trace did not produce/upload log for {case_id}\n"
                f"upload error: {exc}\n"
                f"remote stdout:\n{stdout}\n"
                f"remote stderr:\n{stderr}"
            ) from exc
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
