# AE 85 Install Tree Map

- Target: `85.239.48.31` / `VDSWIN2K22`
- Collected UTC: `2026-05-04T17:35:01.189228+00:00`
- Collection path: Timeweb API credential lookup in memory, then WinRM stdout from `Administrator`.
- Manifest: `target/reverse/ae_2026/remote_tree_manifest.json`
- Entries captured: `1305` (`.aex` plus plugin/required/MediaCore or keyword `.dll`)

## Roots
- `C:\Program Files (x86)\Common Files\Adobe` - 1603 files, 235.8 MiB
- `C:\Program Files\Adobe\Adobe After Effects 2026` - 9392 files, 5.3 GiB
- `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files` - 9389 files, 5.3 GiB
- `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins` - 1201 files, 512.7 MiB
- `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects` - 1122 files, 439.7 MiB
- `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Required` - 26 files, 51.9 MiB
- `C:\Program Files\Adobe\Common` - 3000 files, 1.1 GiB
- `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore` - 3000 files, 1.1 GiB
- `C:\Program Files\Common Files\Adobe` - 9674 files, 3.0 GiB
- `C:\Program Files\Common Files\Adobe\Plug-Ins` - 6 files, 261.7 MiB

## Scan Counts
| Base | Files | .aex | .dll |
|---|---:|---:|---:|
| `C:\Program Files\Adobe\Adobe After Effects 2026` | 9392 | 366 | 559 |
| `C:\Program Files\Common Files\Adobe` | 9674 | 0 | 57 |
| `C:\Program Files\Adobe\Common` | 3000 | 775 | 23 |
| `C:\Program Files (x86)\Common Files\Adobe` | 1603 | 0 | 62 |

## Keyword Hits
### Transform
| File | Size | Modified UTC | Path |
|---|---:|---|---|
| `ArriImageSdkTransformsCpu_module.8.3.1.dll` | 5324808 | `2026-03-04T12:57:55.6842786Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\arriimagesdk_plugins\ArriImageSdkTransformsCpu_module.8.3.1.dll` |
| `ArriImageSdkTransformsCpuFma_module.8.3.1.dll` | 5322760 | `2026-03-04T12:57:55.5094653Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\arriimagesdk_plugins\ArriImageSdkTransformsCpuFma_module.8.3.1.dll` |
| `ArriImageSdkTransformsCuda_module.8.3.1.dll` | 14006792 | `2026-03-04T12:57:55.9879742Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\arriimagesdk_plugins\ArriImageSdkTransformsCuda_module.8.3.1.dll` |
| `ArriImageSdkTransformsOpenCl_module.8.3.1.dll` | 5670920 | `2026-03-04T12:57:56.1644924Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\arriimagesdk_plugins\ArriImageSdkTransformsOpenCl_module.8.3.1.dll` |
| `OCIOCDLTransform.aex` | 171016 | `2026-03-04T12:59:20.8190717Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\OCIOCDLTransform.aex` |
| `OCIOColorSpaceTransform.aex` | 237576 | `2026-03-04T12:59:20.8290645Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\OCIOColorSpaceTransform.aex` |
| `OCIODisplayTransform.aex` | 246280 | `2026-03-04T12:59:20.8397180Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\OCIODisplayTransform.aex` |
| `OCIOFileTransform.aex` | 162824 | `2026-03-04T12:59:20.8480170Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\OCIOFileTransform.aex` |
| `OCIOLookTransform.aex` | 244744 | `2026-03-04T12:59:20.8566825Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\OCIOLookTransform.aex` |
| `Transform.aex` | 159752 | `2026-03-04T12:59:23.4400459Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Transform.aex` |
| `Transform.aex` | 33904 | `2025-07-14T22:52:50.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Transform.aex` |
| `S_OCIOTransform.aex` | 59392 | `2025-07-26T05:28:52.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Adjust\S_OCIOTransform.aex` |
| `S_WarpTransform.aex` | 59904 | `2025-07-26T05:25:32.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Distort\S_WarpTransform.aex` |

### Geometry
- none captured

### Blur
| File | Size | Modified UTC | Path |
|---|---:|---|---|
| `Box_Blur.aex` | 106504 | `2026-03-04T12:59:09.6619157Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Box_Blur.aex` |
| `Channel_Blur.aex` | 51720 | `2026-03-04T12:59:09.7398213Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Channel_Blur.aex` |
| `Compound_Blur.aex` | 54280 | `2026-03-04T12:59:09.9361626Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Compound_Blur.aex` |
| `CrossBlur.aex` | 45064 | `2026-03-04T12:59:10.0533990Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\CrossBlur.aex` |
| `RadialBlur.aex` | 68104 | `2026-03-04T12:59:10.2035433Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\RadialBlur.aex` |
| `RadialFastBlur.aex` | 49160 | `2026-03-04T12:59:10.2095797Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\RadialFastBlur.aex` |
| `VectorBlur.aex` | 70664 | `2026-03-04T12:59:10.3043691Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\VectorBlur.aex` |
| `Deblur.aex` | 286728 | `2026-03-04T12:59:10.3258992Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Deblur.aex` |
| `DirectionalBlur.aex` | 111624 | `2026-03-04T12:59:10.3481848Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\DirectionalBlur.aex` |
| `Fast_Blur.aex` | 61448 | `2026-03-04T12:59:10.4098983Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Fast_Blur.aex` |
| `Gaussian_Blur.aex` | 44552 | `2026-03-04T12:59:10.5115661Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Gaussian_Blur.aex` |
| `Gaussian_Blur_MC.aex` | 49160 | `2026-03-04T12:59:10.5146203Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Gaussian_Blur_MC.aex` |
| `Radial_Blur.aex` | 72200 | `2026-03-04T12:59:20.9892294Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Radial_Blur.aex` |
| `ShapeBlur.aex` | 146952 | `2026-03-04T12:59:21.1055491Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ShapeBlur.aex` |
| `SmartBlur.aex` | 53256 | `2026-03-04T12:59:21.1367331Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\SmartBlur.aex` |
| `VRGaussianBlur.aex` | 747528 | `2026-03-04T12:59:23.6472543Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\VRGaussianBlur.aex` |
| `BCCBlur.aex` | 54384 | `2025-07-14T22:26:34.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCBlur.aex` |
| `BCCBlur4.aex` | 54384 | `2025-07-14T22:26:36.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCBlur4.aex` |
| `BCCBlurDissolve.aex` | 54384 | `2025-07-14T22:26:38.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCBlurDissolve.aex` |
| `BCCBlurDissolvePrTr.aex` | 54384 | `2025-07-14T22:26:40.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCBlurDissolvePrTr.aex` |
| `BCCDirectionalBlur.aex` | 54384 | `2025-07-14T22:28:50.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCDirectionalBlur.aex` |
| `BCCFastBlur.aex` | 54384 | `2025-07-14T22:29:24.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCFastBlur.aex` |
| `BCCFastLensBlur.aex` | 54384 | `2025-07-14T22:29:38.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCFastLensBlur.aex` |
| `BCCGaussianBlur.aex` | 54384 | `2025-07-14T22:30:06.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCGaussianBlur.aex` |
| `BCCLensBlur.aex` | 54384 | `2025-07-14T22:30:56.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCLensBlur.aex` |
| `BCCLensBlurDissolve.aex` | 54384 | `2025-07-14T22:30:58.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCLensBlurDissolve.aex` |
| `BCCLensBlurDissolvePrTr.aex` | 54384 | `2025-07-14T22:31:00.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCLensBlurDissolvePrTr.aex` |
| `BCCMotionBlur.aex` | 54384 | `2025-07-14T22:32:38.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCMotionBlur.aex` |
| `BCCRadialBlur.aex` | 54384 | `2025-07-14T22:33:52.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCRadialBlur.aex` |
| `BCCRGBBlurDissolve.aex` | 54384 | `2025-07-14T22:34:34.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCRGBBlurDissolve.aex` |

### Glow
| File | Size | Modified UTC | Path |
|---|---:|---|---|
| `Glow.aex` | 116232 | `2026-03-04T12:59:10.5191136Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Glow.aex` |
| `PSL_Inner_Glow.aex` | 78856 | `2026-03-04T12:59:20.9667461Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\PSL_Inner_Glow.aex` |
| `PSL_Outer_Glow.aex` | 78344 | `2026-03-04T12:59:20.9746886Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\PSL_Outer_Glow.aex` |
| `VRGlow.aex` | 755720 | `2026-03-04T12:59:23.6694250Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\VRGlow.aex` |
| `Atmospheric Glow Dissolve.aex` | 33904 | `2025-07-14T22:47:22.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Atmospheric Glow Dissolve.aex` |
| `Atmospheric Glow.aex` | 33904 | `2025-07-14T22:47:24.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Atmospheric Glow.aex` |
| `BCCColorizeGlow.aex` | 54384 | `2025-07-14T22:27:36.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCColorizeGlow.aex` |
| `BCCColorizeGlowDissolve.aex` | 54384 | `2025-07-14T22:27:38.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCColorizeGlowDissolve.aex` |
| `BCCColorizeGlowDissolvePrTr.aex` | 54384 | `2025-07-14T22:27:40.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCColorizeGlowDissolvePrTr.aex` |
| `BCCFastFilmGlow.aex` | 54384 | `2025-07-14T22:29:26.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCFastFilmGlow.aex` |
| `BCCFastFilmGlowDissolve.aex` | 54384 | `2025-07-14T22:29:28.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCFastFilmGlowDissolve.aex` |
| `BCCFastFilmGlowDissolvePrTr.aex` | 54384 | `2025-07-14T22:29:30.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCFastFilmGlowDissolvePrTr.aex` |
| `BCCFilmGlow.aex` | 54384 | `2025-07-14T22:29:42.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCFilmGlow.aex` |
| `BCCFilmGlowDissolve.aex` | 54384 | `2025-07-14T22:29:44.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCFilmGlowDissolve.aex` |
| `BCCFilmGlowDissolvePrTr.aex` | 54384 | `2025-07-14T22:29:46.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCFilmGlowDissolvePrTr.aex` |
| `BCCGlow2.aex` | 54384 | `2025-07-14T22:30:18.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCGlow2.aex` |
| `BCCGlowEdges.aex` | 54384 | `2025-07-14T22:30:20.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCGlowEdges.aex` |
| `BCCGlowMatte.aex` | 54384 | `2025-07-14T22:30:22.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCGlowMatte.aex` |
| `BCCRoughGlow.aex` | 54384 | `2025-07-14T22:35:06.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCRoughGlow.aex` |
| `Film Glow Dissolve.aex` | 33904 | `2025-07-14T22:49:12.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Film Glow Dissolve.aex` |
| `Film Glow.aex` | 33904 | `2025-07-14T22:49:14.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Film Glow.aex` |
| `Glow Darks.aex` | 33904 | `2025-07-14T22:49:38.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Glow Darks.aex` |
| `Glow Edges.aex` | 33904 | `2025-07-14T22:49:40.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Glow Edges.aex` |
| `Glow.aex` | 33904 | `2025-07-14T22:49:42.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Glow.aex` |
| `Swish Glow.aex` | 33904 | `2025-07-14T22:52:32.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Swish Glow.aex` |
| `S_Glow.aex` | 59392 | `2025-07-26T05:22:34.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Lighting\S_Glow.aex` |
| `S_GlowAura.aex` | 59392 | `2025-07-26T05:22:44.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Lighting\S_GlowAura.aex` |
| `S_GlowDarks.aex` | 59392 | `2025-07-26T05:22:46.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Lighting\S_GlowDarks.aex` |
| `S_GlowDist.aex` | 59392 | `2025-07-26T05:22:38.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Lighting\S_GlowDist.aex` |
| `S_GlowEdges.aex` | 59392 | `2025-07-26T05:22:48.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Lighting\S_GlowEdges.aex` |

### Minimax
| File | Size | Modified UTC | Path |
|---|---:|---|---|
| `Minimax.aex` | 135688 | `2026-03-04T12:59:11.0520022Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Minimax.aex` |

### Turbulent
| File | Size | Modified UTC | Path |
|---|---:|---|---|
| `TurbulentDisplace.aex` | 124424 | `2026-03-04T12:59:23.4509679Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\TurbulentDisplace.aex` |
| `TurbulentNoise.aex` | 100360 | `2026-03-04T12:59:23.4555611Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\TurbulentNoise.aex` |

### Text
| File | Size | Modified UTC | Path |
|---|---:|---|---|
| `dvatexteditor.dll` | 227336 | `2026-03-04T12:58:13.8255137Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\dvatexteditor.dll` |
| `Basic_Text.aex` | 677896 | `2026-03-04T12:59:09.6157266Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Basic_Text.aex` |
| `ColorTexture.aex` | 237576 | `2026-03-04T12:59:09.8920913Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ColorTexture.aex` |
| `Path_Text.aex` | 735240 | `2026-03-04T12:59:20.9140612Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Path_Text.aex` |
| `Texturize.aex` | 41992 | `2026-03-04T12:59:23.3959953Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Texturize.aex` |
| `BCCExtrudedText.aex` | 53872 | `2025-07-14T22:29:22.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCExtrudedText.aex` |
| `BCCTexturedLight.aex` | 54384 | `2025-07-14T22:36:12.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCTexturedLight.aex` |
| `BCCTexturedWipe.aex` | 54384 | `2025-07-14T22:36:14.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCTexturedWipe.aex` |
| `BCCTexturedWipePrTr.aex` | 54384 | `2025-07-14T22:36:16.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCTexturedWipePrTr.aex` |
| `BCCTypeOnText.aex` | 46704 | `2025-07-14T22:36:58.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCTypeOnText.aex` |
| `Texture Wipe.aex` | 33904 | `2025-07-14T22:52:42.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Texture Wipe.aex` |
| `Textures.aex` | 33904 | `2025-07-14T22:52:44.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Textures.aex` |
| `S_TextureCells.aex` | 59904 | `2025-07-26T05:27:52.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureCells.aex` |
| `S_TextureChromaSpiral.aex` | 59904 | `2025-07-26T05:27:54.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureChromaSpiral.aex` |
| `S_TextureFlux.aex` | 59392 | `2025-07-26T05:28:00.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureFlux.aex` |
| `S_TextureFolded.aex` | 59904 | `2025-07-26T05:27:40.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureFolded.aex` |
| `S_TextureLoops.aex` | 59904 | `2025-07-26T05:27:58.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureLoops.aex` |
| `S_TextureMicro.aex` | 59904 | `2025-07-26T05:28:02.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureMicro.aex` |
| `S_TextureMoire.aex` | 59904 | `2025-07-26T05:27:44.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureMoire.aex` |
| `S_TextureNeurons.aex` | 59904 | `2025-07-26T05:27:56.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureNeurons.aex` |
| `S_TextureNoiseEmboss.aex` | 59904 | `2025-07-26T05:27:46.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureNoiseEmboss.aex` |
| `S_TextureNoisePaint.aex` | 59392 | `2025-07-26T05:27:48.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureNoisePaint.aex` |
| `S_TexturePlasma.aex` | 59904 | `2025-07-26T05:27:44.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TexturePlasma.aex` |
| `S_TextureSpots.aex` | 59904 | `2025-07-26T05:27:50.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureSpots.aex` |
| `S_TextureTiles.aex` | 59392 | `2025-07-26T05:27:56.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureTiles.aex` |
| `S_TextureWeave.aex` | 59904 | `2025-07-26T05:27:42.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureWeave.aex` |

### Composite
| File | Size | Modified UTC | Path |
|---|---:|---|---|
| `Composite.aex` | 47624 | `2026-03-04T12:59:10.0474527Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\Composite.aex` |
| `CompositeObsolete.aex` | 41992 | `2026-03-04T12:59:10.0514487Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\CompositeObsolete.aex` |
| `SolidComposite.aex` | 47624 | `2026-03-04T12:59:21.1480185Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\SolidComposite.aex` |
| `BCCAdvancedComposite.aex` | 54384 | `2025-07-14T22:26:14.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCAdvancedComposite.aex` |
| `BCCCompositeChoker.aex` | 54384 | `2025-07-14T22:27:50.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCCompositeChoker.aex` |
| `BCCCompositeDissolve.aex` | 54384 | `2025-07-14T22:27:52.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCCompositeDissolve.aex` |
| `BCCCompositeDissolvePrTr.aex` | 54384 | `2025-07-14T22:27:54.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCCompositeDissolvePrTr.aex` |
| `Composite.aex` | 33904 | `2025-07-14T22:48:16.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Composite.aex` |
| `Edge Composite.aex` | 33904 | `2025-07-14T22:49:02.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Edge Composite.aex` |
| `Holdout Composite.aex` | 33904 | `2025-07-14T22:49:58.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Holdout Composite.aex` |
| `Math Composite.aex` | 33904 | `2025-07-14T22:50:40.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\Math Composite.aex` |
| `S_EdgeFlash.aex` | 59392 | `2025-07-26T05:28:20.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Composite\S_EdgeFlash.aex` |
| `S_Layer.aex` | 59392 | `2025-07-26T05:29:00.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Composite\S_Layer.aex` |
| `S_MathOps.aex` | 59392 | `2025-07-26T05:28:58.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Composite\S_MathOps.aex` |
| `S_MatteOps.aex` | 59392 | `2025-07-26T05:28:26.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Composite\S_MatteOps.aex` |
| `S_MatteOpsComp.aex` | 59904 | `2025-07-26T05:28:28.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Composite\S_MatteOpsComp.aex` |
| `S_ZComp.aex` | 59392 | `2025-07-26T05:28:22.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Composite\S_ZComp.aex` |

### Color
| File | Size | Modified UTC | Path |
|---|---:|---|---|
| `ColorSpaceConverter.dll` | 1089032 | `2026-03-04T12:58:07.2136701Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\ColorSpaceConverter.dll` |
| `mc_trans_video_colorspace.dll` | 1223176 | `2026-03-04T12:58:37.6721029Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\mc_trans_video_colorspace.dll` |
| `ApplyColorLUT.aex` | 146952 | `2026-03-04T12:59:09.5083540Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ApplyColorLUT.aex` |
| `AutoColor.aex` | 155144 | `2026-03-04T12:59:09.5777388Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\AutoColor.aex` |
| `Broadcast_Colors.aex` | 42504 | `2026-03-04T12:59:09.6654865Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Broadcast_Colors.aex` |
| `Change_Color.aex` | 108552 | `2026-03-04T12:59:09.7308178Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Change_Color.aex` |
| `ChangeToColor.aex` | 51720 | `2026-03-04T12:59:09.7252040Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ChangeToColor.aex` |
| `Color_Balance.aex` | 39944 | `2026-03-04T12:59:09.8950375Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Color_Balance.aex` |
| `Color_Balance_2.aex` | 49160 | `2026-03-04T12:59:09.8968598Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Color_Balance_2.aex` |
| `Color_Diff.aex` | 104968 | `2026-03-04T12:59:09.9041441Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Color_Diff.aex` |
| `Color_Emboss.aex` | 96264 | `2026-03-04T12:59:09.9079884Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Color_Emboss.aex` |
| `Color_HLS.aex` | 94728 | `2026-03-04T12:59:09.9178451Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Color_HLS.aex` |
| `Color_Key.aex` | 45064 | `2026-03-04T12:59:09.9242872Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Color_Key.aex` |
| `Color_Range.aex` | 100360 | `2026-03-04T12:59:09.9292660Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Color_Range.aex` |
| `Colorama.aex` | 157704 | `2026-03-04T12:59:09.7672479Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Colorama.aex` |
| `ColorAndContrast.aex` | 380936 | `2026-03-04T12:59:09.7778050Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ColorAndContrast.aex` |
| `ColorLink.aex` | 48648 | `2026-03-04T12:59:09.7815812Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ColorLink.aex` |
| `ColorShift.aex` | 4245512 | `2026-03-04T12:59:09.8805439Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ColorShift.aex` |
| `ColorsQuad.aex` | 75784 | `2026-03-04T12:59:09.8846232Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ColorsQuad.aex` |
| `ColorTexture.aex` | 237576 | `2026-03-04T12:59:09.8920913Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ColorTexture.aex` |
| `ColorNeutralizer.aex` | 62984 | `2026-03-04T12:59:10.0406902Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\ColorNeutralizer.aex` |
| `ColorOffset.aex` | 42504 | `2026-03-04T12:59:10.0442982Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\ColorOffset.aex` |
| `Leave_Color.aex` | 41992 | `2026-03-04T12:59:10.8588020Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Leave_Color.aex` |
| `OpenColorIO.dll` | 6742024 | `2026-03-04T12:59:15.2207337Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\mochaAE\Resources\mochaui\bin\OpenColorIO.dll` |
| `OCIOColorSpaceTransform.aex` | 237576 | `2026-03-04T12:59:20.8290645Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\OCIOColorSpaceTransform.aex` |
| `Three_Way_Color_Corrector.aex` | 86024 | `2026-03-04T12:59:23.4018155Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Three_Way_Color_Corrector.aex` |
| `VRColorGradient.aex` | 750088 | `2026-03-04T12:59:23.5425380Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\VRColorGradient.aex` |
| `BCC3WayColorGrade.aex` | 54384 | `2025-07-14T22:26:10.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCC3WayColorGrade.aex` |
| `BCCColorBalance.aex` | 54384 | `2025-07-14T22:27:28.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCColorBalance.aex` |
| `BCCColorChoker.aex` | 54384 | `2025-07-14T22:27:30.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\BCCColorChoker.aex` |

### Render
| File | Size | Modified UTC | Path |
|---|---:|---|---|
| `adobeusd_usdRender.dll` | 210952 | `2026-03-04T12:57:45.6402444Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\adobeusd_usdRender.dll` |
| `AudioRenderer.dll` | 1295880 | `2026-03-04T12:57:56.4287048Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\AudioRenderer.dll` |
| `ImageRenderer.dll` | 2325000 | `2026-03-04T12:58:21.4634203Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\ImageRenderer.dll` |
| `RendererCPU.dll` | 444424 | `2026-03-04T12:59:37.3648190Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\RendererCPU.dll` |
| `RendererGPU.dll` | 1950728 | `2026-03-04T12:59:37.4031408Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\RendererGPU.dll` |
| `VideoRenderer.dll` | 2795016 | `2026-03-04T12:59:43.1182490Z` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\VideoRenderer.dll` |
| `S_Aurora.aex` | 59392 | `2025-07-26T05:29:54.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Aurora.aex` |
| `S_Caustics.aex` | 59392 | `2025-07-26T05:30:22.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Caustics.aex` |
| `S_Clouds.aex` | 59392 | `2025-07-26T05:27:24.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Clouds.aex` |
| `S_CloudsColorSmooth.aex` | 59904 | `2025-07-26T05:27:28.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_CloudsColorSmooth.aex` |
| `S_CloudsMultColor.aex` | 59904 | `2025-07-26T05:27:26.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_CloudsMultColor.aex` |
| `S_CloudsPerspective.aex` | 59904 | `2025-07-26T05:27:30.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_CloudsPerspective.aex` |
| `S_CloudsPsyko.aex` | 59392 | `2025-07-26T05:27:34.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_CloudsPsyko.aex` |
| `S_CloudsVortex.aex` | 59904 | `2025-07-26T05:27:32.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_CloudsVortex.aex` |
| `S_Dust.aex` | 59392 | `2025-07-26T05:31:04.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Dust.aex` |
| `S_Gradient.aex` | 59392 | `2025-07-26T05:28:08.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Gradient.aex` |
| `S_GradientMulti.aex` | 59904 | `2025-07-26T05:28:12.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_GradientMulti.aex` |
| `S_GradientRadial.aex` | 59904 | `2025-07-26T05:28:10.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_GradientRadial.aex` |
| `S_Grid.aex` | 59392 | `2025-07-26T05:28:12.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Grid.aex` |
| `S_Grunge.aex` | 59392 | `2025-07-26T05:27:38.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Grunge.aex` |
| `S_LaserBeam.aex` | 59392 | `2025-07-26T05:30:24.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_LaserBeam.aex` |
| `S_Luna.aex` | 59392 | `2025-07-26T05:30:44.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Luna.aex` |
| `S_MuzzleFlash.aex` | 59392 | `2025-07-26T05:30:22.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_MuzzleFlash.aex` |
| `S_NightSky.aex` | 59392 | `2025-07-26T05:27:36.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_NightSky.aex` |
| `S_Shape.aex` | 59392 | `2025-07-26T05:28:14.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Shape.aex` |
| `S_Sparkles.aex` | 59392 | `2025-07-26T05:28:04.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_Sparkles.aex` |
| `S_SparklesColor.aex` | 59904 | `2025-07-26T05:28:06.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_SparklesColor.aex` |
| `S_TextureCells.aex` | 59904 | `2025-07-26T05:27:52.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureCells.aex` |
| `S_TextureChromaSpiral.aex` | 59904 | `2025-07-26T05:27:54.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureChromaSpiral.aex` |
| `S_TextureFlux.aex` | 59392 | `2025-07-26T05:28:00.0000000Z` | `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\Sapphire Render\S_TextureFlux.aex` |

## Notes
- No binaries were copied; this map is metadata from WinRM stdout only.
- JSON manifest contains full captured listing with `size_bytes`, ISO `mtime_utc`, relative path, keyword hits, and plugin/required flags.
