# Architecture

## Workspace

- pixelforge-core: content-sniffed format inspection, codec capability registry, dimension limits, resize calculations, alpha compositing, format-specific encoding, collision naming, and atomic output writes.
- pixelforge-jobs: one dedicated conversion thread, a one-batch bounded command queue, progress events, pause/resume between images, and cooperative cancellation between outputs.
- pixelforge-desktop: native eframe UI, native file/folder dialogs, OS drop handling, asynchronous preview generation, user settings, queue/results view, and platform folder opening.
- pixelforge-cli: one-input conversion with output format, quality, resize bounds, and destination options.

The UI owns settings and queue state. A BatchRequest is copied into the worker. Each file/format pair is converted independently, so a decode or encode error becomes an item failure and does not abort later pairs. The core writes a temporary sibling file and renames only after a successful encode; output naming never overwrites the input or an existing file.

## Codec choices

The image crate is the common pixel and raster layer. The pinned feature list enables PNG, JPEG, WebP decode, AVIF, GIF, BMP, TIFF, ICO, QOI, TGA, PNM, HDR, OpenEXR, Farbfeld, and DDS decode. The webp adapter supplies lossy encoding because image's WebP writer is lossless-only. AVIF uses image's ravif encoder and dav1d decoder feature. The enabled privacy cleanup pass strips ancillary PNG chunks, JPEG APP/comment segments, and WebP metadata/unknown chunks from encoded output before the temporary file is published.

Inputs are identified from signatures with ImageReader::with_guessed_format; file extensions only influence the suggested output name/filter. A 100-megapixel pixel limit is applied before conversion. Settings use serde-compatible JSON with schema version 1; migrations should be added before changing the persisted schema incompatibly.

## Limits and extension points

Conversion decodes a still DynamicImage; animated/multipage sources therefore use a single decoded frame. The capability registry marks this behavior. All output is newly encoded from decoded pixels, so source metadata does not pass through. The optional extra cleanup pass removes recognized and ancillary container metadata from PNG, JPEG, and WebP. It cannot determine whether arbitrary image pixels encode a watermark or other mark. Format-specific adapters should be added behind the core encoder boundary and reflected in the capability registry before UI exposure.

The worker checks pause/cancel between files and formats. An active codec call cannot be interrupted, so cancelling a large single image takes effect after that image finishes. Preview decoding happens on a separate thread from the UI.

