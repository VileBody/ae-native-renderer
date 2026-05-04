# Collected AE 2026 Blur / Glow / Shadow Binaries

Collection date: 2026-05-04.

Remote node: `85.239.48.31`, Windows, Adobe After Effects 2026.

Collection method: Timeweb API was used to resolve the current Windows password without printing secrets; files were staged into a zip on the node through `pywinrm` (`Administrator` over `http://85.239.48.31:5985/wsman`) and downloaded through base64 stdout chunks. No Ghidra/headless pass was run.

Local root: `target/reverse/ae_2026/effects_blur_glow_shadow/`.

Scope: Adobe After Effects 2026 bundled `Support Files\Plug-ins\Effects` binaries matching blur/glow/shadow families. `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore` was scanned, but third-party BorisFX/Sapphire matches were not copied.

## Inventory

| Binary | SHA-256 | Remote path | Local path | Guessed / observed effect matchNames |
| --- | --- | --- | --- | --- |
| `Box_Blur.aex` | `aa8f2eb1489ebb492ee2d37873bfec394cfeb7dcfbcf6fa8f29af0e4a7d46b98` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Box_Blur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Box_Blur.aex` | `ADBE Box Blur2`; `ADBE Box Blur` legacy string |
| `Channel_Blur.aex` | `3c335ac2458c65bb60ab48b295089b7ceb204393452cecc08645a65f4a2fc4ff` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Channel_Blur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Channel_Blur.aex` | `ADBE Channel Blur` |
| `Compound_Blur.aex` | `67bab4f9826a66cdca35d19b3f0883654c4827630cfb980e1a4d91714dec4527` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Compound_Blur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Compound_Blur.aex` | `ADBE Compound Blur` |
| `CycoreFXHD/CrossBlur.aex` | `ad4c8e3c04f28c1c666162e3a6ca9fa6721bb698a9eca077b7428a41c9cad5f9` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\CrossBlur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/CycoreFXHD/CrossBlur.aex` | `CC Cross Blur` |
| `CycoreFXHD/RadialBlur.aex` | `56519d11187f645b88acd78815ca0af825cb408bd27651fb7ec5a73ada6fb79a` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\RadialBlur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/CycoreFXHD/RadialBlur.aex` | `CC Radial Blur` |
| `CycoreFXHD/RadialFastBlur.aex` | `f8de34f3e3707ad29e2e283b013ca0dc699d4c5dc1450d922a2f088fd1ba98d4` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\RadialFastBlur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/CycoreFXHD/RadialFastBlur.aex` | `CC Radial Fast Blur` |
| `CycoreFXHD/VectorBlur.aex` | `80ffbc0bf2aca57dcf864af5aa860bf9474f221c2d64998be210677602923501` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\VectorBlur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/CycoreFXHD/VectorBlur.aex` | `CC Vector Blur` |
| `Deblur.aex` | `8425c532cc014dfd1d2845fd3c7aacbff3ace828508a463e8ec61ac01ccf2026` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Deblur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Deblur.aex` | `ADBE CameraShakeDeblur` |
| `DirectionalBlur.aex` | `13f3df6188aaaf2168c60f8cdd189f5123f0fdf98f5fa153249b6036a0b86e2b` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\DirectionalBlur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/DirectionalBlur.aex` | `ADBE Motion Blur` |
| `Drop_Shadow.aex` | `f5ef57b00fa3607125c5ede95766af6d84b5612fdadfb9de3b6d6c1495d0f9fe` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Drop_Shadow.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Drop_Shadow.aex` | `ADBE Drop Shadow` |
| `Fast_Blur.aex` | `9e5f9bd9b1628e456f8f25d01b5b2f2065f3297388ef58cd5e2fa100232e71e8` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Fast_Blur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Fast_Blur.aex` | `ADBE Fast Blur`; `ADBE Reduce Interlace Flicker` |
| `Gaussian_Blur.aex` | `28be268bc78f2b3784dc3bee9d4977618ed61ddd7169f9949862b135a83aaf6e` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Gaussian_Blur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Gaussian_Blur.aex` | `ADBE Gaussian Blur` |
| `Gaussian_Blur_MC.aex` | `23d7c11e0719a78388e8a7c063f0db204e80ec9e18bc517cc31a96d82b0edbe6` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Gaussian_Blur_MC.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Gaussian_Blur_MC.aex` | `ADBE Gaussian Blur 2` |
| `Glow.aex` | `bb8f60143cbb342fdb3c074a129647b65c5e593772c305593707e0dee9a9b2d4` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Glow.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Glow.aex` | `ADBE Glo2` |
| `PSL_Drop_Shadow.aex` | `6cfb02e44f42ca729c3a73291e7356df1a892bc76f3b6c694f09a782ad408228` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\PSL_Drop_Shadow.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/PSL_Drop_Shadow.aex` | `ADBE PSL Drop Shadow` |
| `PSL_Inner_Glow.aex` | `8ea8f247da229ea996ab3ad1c74c2cfa5e808453f465a11e73f439012202bcbd` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\PSL_Inner_Glow.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/PSL_Inner_Glow.aex` | `ADBE PSL Inner Glow` |
| `PSL_Inner_Shadow.aex` | `1802c8f264ebf18667ee9473c68b9cfea014831c77b2c0d2fdc06e12414d956e` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\PSL_Inner_Shadow.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/PSL_Inner_Shadow.aex` | `ADBE PSL Inner Shadow` |
| `PSL_Outer_Glow.aex` | `3bc79cc3402e60b1f0832f4783f356187fb89309208bb98f35200900bb785a6c` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\PSL_Outer_Glow.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/PSL_Outer_Glow.aex` | `ADBE PSL Outer Glow` |
| `Radial_Blur.aex` | `5309fdcdc4e71ce2f96fe7a09335a5c02b2b8b9745a9ff6fcc34e7528ce3b1b7` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\Radial_Blur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/Radial_Blur.aex` | `ADBE Radial Blur` |
| `RadialShadow.aex` | `dafd6dcb8da2d659cec76e674de4a3bfbb907a1b85ac2a27bf37739daa26e774` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\RadialShadow.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/RadialShadow.aex` | `ADBE Radial Shadow` |
| `ShadowHighlight.aex` | `fea30b6bc3e879af7f021b2120149eedc9008a8baf2f9b03e61fdde83afcf630` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ShadowHighlight.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/ShadowHighlight.aex` | `ADBE ShadowHighlight` |
| `ShapeBlur.aex` | `47987040b1311e43ddc07ba6784643fb14fbc472589468b897ef33c15f1e76fd` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\ShapeBlur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/ShapeBlur.aex` | `ADBE Camera Lens Blur` |
| `SmartBlur.aex` | `37eff1016eeff40deafa57fb1049cc9a914b433c51f21ddb14ee109943063326` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\SmartBlur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/SmartBlur.aex` | `ADBE Smart Blur` |
| `VRGaussianBlur.aex` | `3ddfdbb2a5af31e46a262423f3c4daf5685c8f830d496fa2f3ebfe0dad6e9ad4` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\VRGaussianBlur.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/VRGaussianBlur.aex` | `VR Blur` / `VRGaussianBlur`; no `ADBE ...` string found |
| `VRGlow.aex` | `bc9ab671d144a58784823092be6acc9b8524925d62a5a419e49df0264d4ac3c3` | `C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\VRGlow.aex` | `target/reverse/ae_2026/effects_blur_glow_shadow/VRGlow.aex` | `VR Glow` / `VRGlow`; no `ADBE ...` string found |

## Not Copied

- `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\BorisFX\Continuum\...` blur/glow/shadow matches: scanned but skipped as third-party Common MediaCore binaries.
- `C:\Program Files\Adobe\Common\Plug-ins\7.0\MediaCore\Sapphire Plug-ins\...` blur/glow/shadow matches: scanned but skipped as third-party Common MediaCore binaries.
- No matching first-party `.dll` files were found in the searched AE plug-in roots.

## Notes

- Key strings observed locally after collection:
  - `Box_Blur.aex`: `ADBE Box Blur2`, `ADBE Box Blur`.
  - `Drop_Shadow.aex`: `ADBE Drop Shadow`.
  - `Glow.aex`: `ADBE Glo2`.
  - `Drop_Shadow.aex` also contains `FastBoxBlur` / `BoxBlurOptions` symbols, useful for later kernel comparison.
- A machine-readable manifest is stored beside the binaries at `target/reverse/ae_2026/effects_blur_glow_shadow/collection_manifest.json`.
