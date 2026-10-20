# Format capability summary

Capability data is exposed from pixelforge_core::FORMAT_CAPABILITIES. “Animation” means the container can carry multiple frames; current conversion still writes a single frame/page. Bit depth describes supported/common encoder behavior, not a guarantee that every input variant or color profile is preserved.

| Format | Read | Write | Alpha | Lossless | Quality | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| PNG | Yes | Yes | Yes | Yes | No | 8/16-bit raster |
| JPEG | Yes | Yes | No | No | Yes | Alpha is composited |
| WebP | Yes | Yes | Yes | Yes | Yes | Still images; lossy and lossless |
| AVIF | Yes | Yes | Yes | No | Yes | Still images; bundled native decoder |
| GIF | Yes | Yes | Yes | Yes | No | First frame converted |
| BMP | Yes | Yes | Yes | Yes | No | Metadata stripped |
| TIFF | Yes | Yes | Yes | Yes | No | Single-page output |
| ICO | Yes | Yes | Yes | Yes | No | Output dimensions limited to 256×256 |
| QOI | Yes | Yes | Yes | Yes | No | Lossless |
| TGA | Yes | Yes | Yes | Yes | No | Metadata stripped |
| PNM | Yes | Yes | No | Yes | No | Writes binary PPM |
| HDR | Yes | Yes | No | Yes | No | RGBE |
| OpenEXR | Yes | Yes | Yes | Yes | No | 32-bit float formats |
| Farbfeld | Yes | Yes | Yes | Yes | No | 16-bit channels |
| DDS | Yes | No | Variant-dependent | — | — | Decode-only |

SVG rasterization and HEIF, JPEG XL, PSD, RAW, and PDF adapters are not enabled.

