# Collected Temporal / Noise / Distort AE 2026 Binaries

Collection date: 2026-05-04

Source node: Windows AE node `85.239.48.31`

Remote root:
`C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects`

Local root:
`target/reverse/ae_2026/effects_temporal_noise_distort`

Ghidra was not run for this pass. The priority was collection, and the copied
binary strings already exposed match names, parameter labels, GPU kernel names,
and PDB path hints. If a later pass opens these in Ghidra, use
`/tmp/ae-native-renderer-ghidra.lock`.

## Inventory

| Binary | SHA-256 | Size | Remote path | Local path | Guessed matchNames / functions |
| --- | --- | ---: | --- | --- | --- |
| `Posterize_Time.aex` | `9e91bcee785766f54cdb2c0ab90a7cee6bcee5b34fdee660ce608ad0a9484d60` | 65544 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Posterize_Time.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/Posterize_Time.aex` | `ADBE_Posterize_Time`, `ADBE Posterize Time`; param `Frame Rate`; `EffectMainExtra`, `PluginDataEntryFunction`, `Private AE InData Internal Query Suite`. |
| `Minimax.aex` | `95afa7a3b4a539e8389c60901149f285e8cd3e809d9ad11a08f80d979fa19b32` | 135688 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Minimax.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/Minimax.aex` | `ADBE_Minimax`, `ADBE Minimax`; params `Operation`, `Radius`, `Channel`, `Direction`, `Don't Shrink Edges`; kernels `MinimaxBuildSTreeLeafNodesKernel`, `MinimaxBuildSTreeNonLeafNodesKernel`, `MinimaxTraverseSTreeKernel`, `AEFX_Minimax`. |
| `TurbulentDisplace.aex` | `f3eedc80b4ebe033a497f6dc50f7822059c3770113016e7a68231b25d578180b` | 124424 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\TurbulentDisplace.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentDisplace.aex` | `ADBE_Turbulent_Displace`, `ADBE Turbulent Displace`; params `Displacement`, `Amount`, `Size`, `Offset (Turbulence)`, `Complexity`, `Evolution`, `Random Seed`, `Pinning`, `Resize Layer`; kernels `TurbulentDisplaceFrac1DKernel`, `TurbulentDisplaceFracAllKernel`; source hint `AEFilterTurbulentDisplace\Src\TDispMain.cpp`. |
| `TurbulentNoise.aex` | `7acc6f0d84b4d682fd2ad3b818ffecd219e2abb0dee83d8c2ccaed9b564b72ce` | 100360 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\TurbulentNoise.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentNoise.aex` | `ADBE_AIF_Perlin_Noise_3D`, `ADBE AIF Perlin Noise 3D`; UI name `Turbulent Noise`; params mirror fractal/turbulent controls plus `Turbulence Factor`, `Evolution Detail`; `EffectMainExtra`, `PluginDataEntryFunction`. |
| `FractalNoise.aex` | `13f6d1ca939387cf1e57ab6d441b1928054c2a640b1e3ae189e7e8154973c99b` | 216584 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\FractalNoise.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/FractalNoise.aex` | `ADBE_Fractal_Noise`, `ADBE Fractal Noise`; GPU module `PrGPUFilterModule<FractalNoiseEffect>`; kernels `FractalNoise_FracBASICKernel`, `FracTURBSMO`, `FracTURBBASIC`, `FracTURBSHARP`, `FracDYNSTD`, `FracDYNPRO`, `FracDYNTWI`, `FracEXTRA*`, `FractalNoise_SetWorldToHue`, `FractalNoise_SetWorldToSaturation`. |
| `Noise.aex` | `1bb0fcd23b0f2dbd160912bd56e16e6724bd0d19676de326f1edd228f202eab0` | 114696 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Noise.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/Noise.aex` | `ADBE_Noise`, `ADBE Noise`, `ADBE Noise2`; params `Amount of Noise`, `Noise Type`, `Clipping`, `Use Color Noise`, `Clip Result Values`; `AE_NOISE::DoRenderGPU`, `NoiseKernel`, `AEFX_Noise`, `GetNoiseParams`; source hint `AEFX_Noise2.cpp`. |
| `NoiseAlpha.aex` | `4a85cdc8115f014b22b30f87a11aaf46a0c4009d66be4b22b1918d97564b2e84` | 53256 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\NoiseAlpha.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/NoiseAlpha.aex` | `ADBE_Noise_Alpha`, `ADBE Noise Alpha`, `ADBE Noise Alpha2`; params `Noise`, `Amount`, `Original Alpha`, `Overflow`, `Cycle Noise`, `Random Seed`, `Noise Phase`; `PluginDataEntryFunction`. |
| `NoiseHLS.aex` | `7760cbe42c0ac0664c64c8b2d53e47af428fb8ebfd65449e2ae3b5b57dede232` | 62984 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\NoiseHLS.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/NoiseHLS.aex` | `ADBE_Noise_HLS`, `ADBE Noise HLS`, `ADBE Noise HLS2`; params `Noise`, `Hue`, `Lightness`, `Saturation`, `Grain Size`, `Noise Phase`; `PluginDataEntryFunction`. |
| `NoiseHLSAuto.aex` | `0153972fb004b6ec87837ed0bed35b06af15d2658c316a883619a3c71f2470e9` | 64008 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\NoiseHLSAuto.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/NoiseHLSAuto.aex` | `ADBE_Noise_HLS_Auto`, `ADBE Noise HLS Auto`, `ADBE Noise HLS Auto2`; params `Noise`, `Hue`, `Lightness`, `Saturation`, `Grain Size`, `Noise Animation Speed`; `PluginDataEntryFunction`. |
| `Time_Displace.aex` | `710abb9bd2f3d8442fa26cef69a08f69420f6eda8341e8b3949cefa4b301dceb` | 48648 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Time_Displace.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/Time_Displace.aex` | `ADBE_Time_Displacement`, `ADBE Time Displacement`; params `Stretch Map to Fit`, `Time Displacement Layer`, `Max Displacement Time [sec]`, `Time Resolution [fps]`, `If Layer Sizes Differ`; `EffectMain`, `PluginDataEntryFunction`. |
| `Displacement.aex` | `5d4c419dbb0a4682cbc6506adcfbcf16f53c5a51e458bb6a4d3edfc34a624a87` | 60936 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Displacement.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/Displacement.aex` | `ADBE_Displacement_Map`, `ADBE Displacement Map`; params `Displacement Map Layer`, horizontal/vertical channel selectors, max displacements, map behavior, edge behavior, `Expand Output`; `EffectMainExtra`, `PluginDataEntryFunction`, `PF SamplingFloat Suite`. |
| `ChannelCombiner.aex` | `5516a5773dabf0db1f32fd70e121a35657b7a95e033a897c4571fb863f962ec9` | 131592 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ChannelCombiner.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/ChannelCombiner.aex` | `ADBE_Channel_Combiner`, `ADBE Channel Combiner`; conversion/source params; GPU kernel `ChannelCombinerKernel`, `AEFX_ChannelCombiner`; uses `GPUFoundation.dll`. |
| `Set_Channels.aex` | `d1e4bf36578b062640b856d293af1c225c30bc3a58041585d339a46a7bc09d3f` | 110088 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Set_Channels.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/Set_Channels.aex` | `ADBE_Set_Channels`, `ADBE Set Channels`; params source layers 1-4, set red/green/blue/alpha to channel choices, size behavior; GPU kernel `SetChannelsKernel`, `AEFX_SetChannels`; uses `GPUFoundation.dll`. |
| `Shift_Channels.aex` | `59279dde72785d797344e2ef5ab457058d9bbcb8e55a171082ec3b1b0e107c86` | 105480 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Shift_Channels.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/Shift_Channels.aex` | `ADBE_Shift_Channels`, `ADBE Shift Channels`; params `Take Alpha From`, `Take Red From`, `Take Green From`, `Take Blue From`; GPU kernel `ShiftChannelsKernel`, `AEFX_Shift_Channels`, `AEFilter.ShiftChannels`; uses `GPUFoundation.dll`. |
| `Aux_Channel_Extract.aex` | `4176e429281fb336100d1f3f09b0ab859c2c1f0f1ec1a178d5f99211f61ef869` | 61960 | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Aux_Channel_Extract.aex` | `target/reverse/ae_2026/effects_temporal_noise_distort/Aux_Channel_Extract.aex` | `ADBE_AUX_CHANNEL_EXTRACT`, `ADBE AUX CHANNEL EXTRACT`; params 3D channel selector, black/white point, anti-alias, clamp output, invert depth map; `PF AE Channel Suite`, `AEGP Layer Suite`, `PluginDataEntryFunction`. |

## Notes

- Direct target binaries found: `Posterize_Time.aex`, `Minimax.aex`,
  `TurbulentDisplace.aex`.
- No monolithic `Time.aex`, `Channel.aex`, `Distort.aex`, or `Noise.aex`
  bundle was observed for these categories under the AE 2026 `Plug-ins\Effects`
  catalog. AE 2026 ships this set mostly as individual effect `.aex` files.
  `Noise.aex` exists, but it is the individual Noise effect, not a category
  bundle.
- Several GPU-capable effects reference `GPUFoundation.dll`; a copy already
  exists at `target/reverse/ae_2026/GPUFoundation.dll`.
- `remote_manifest.json` in the local root records the Timeweb/WinRM collection
  manifest without credentials.
