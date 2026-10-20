# PixelForge

PixelForge is an offline-first Rust desktop image converter with a batch queue and a small command-line interface. The workspace is split into a reusable conversion core, a bounded background worker, the native egui app, and the CLI.

## Run

Install a current stable Rust toolchain, Git, pkg-config, Python, Meson, Ninja, NASM, and a C/C++ toolchain. The AVIF decoder compiles dav1d from upstream source during the first build. On Windows, install the Visual Studio C++ Build Tools workload and Windows SDK, then install pkg-config with winget install --id bloodrock.pkg-config-lite -e and add NASM to PATH (the official Windows ZIP is available from nasm.us). On macOS install NASM with brew install nasm; on Linux install it with the distribution package manager. The build scripts enable dav1d's internal source build automatically. Run the PowerShell build helper from a normal PowerShell window:

~~~
.\scripts\build.ps1
.\target\release\pixelforge-desktop.exe
~~~

~~~
cargo build --release --workspace
./target/release/pixelforge-desktop
~~~

Convert from the command line:

~~~
cargo run -p pixelforge-cli -- --input photo.png --format webp --quality 82
cargo run -p pixelforge-cli -- --input photo.png --format avif --resize 1600x1200 --output ./exports
~~~

The desktop app supports file dialogs, folder import, recursive scanning, operating-system file drops, multiple output formats per queued source, a persistent output directory, built-in presets, quality controls, lossless WebP, resize-to-fit, JPEG transparency compositing, a privacy cleanup toggle, per-output results, and retry of failed inputs. Original files are not modified. Existing destination names receive a numbered suffix.

## Build and verify

~~~
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
~~~

On Linux, the eframe native window needs the X11 development packages used by winit (including libxcb-render0-dev, libxcb-shape0-dev, libxcb-xfixes0-dev, and libxkbcommon-dev) and a C build toolchain. Install Python build helpers with python -m pip install meson ninja. The AVIF decoder downloads and compiles dav1d during the first Cargo build. The WebP lossy adapter compiles its bundled libwebp C sources. After building, core conversions make no network requests. Linux runtime graphics libraries may also be required by the distribution.

The desktop binary is target/release/pixelforge-desktop (or pixelforge-desktop.exe). The CLI binary is target/release/pixelforge (or pixelforge.exe). CI compiles both on Windows, macOS, and Ubuntu. On Windows, `scripts/package-windows.ps1` creates a portable ZIP with both programs and the Visual C++ runtime DLLs; extract it and run `pixelforge-desktop.exe` without installing Rust or the codec build tools. Windows 10 or newer supplies the OS graphics and C runtime components. The package has been dependency-audited on the build host, but not tested on a clean Windows machine. Release signing/notarization and native installer publication are not configured.

## Formats and honest limits

PNG, JPEG, WebP, AVIF, GIF, BMP, TIFF, ICO, QOI, TGA, PNM, HDR, OpenEXR, and Farbfeld are registered for reading and writing. DDS is decode-only. Format detection comes from image contents rather than filename extensions. The format capability registry is in pixelforge-core.

JPEG, WebP, and AVIF expose quality controls. WebP can be encoded lossy or lossless. PNG alpha is preserved; JPEG alpha is composited on the selected background. Resize uses a Lanczos filter and preserves aspect ratio without upscaling. Every export is freshly encoded from decoded pixels, so source metadata is not copied. The enabled privacy cleanup toggle also removes ancillary PNG chunks, JPEG APP/comment segments, and WebP metadata or unknown chunks. This removes common AI labels and tracking metadata, but does not identify or remove marks encoded into image pixels, and cannot promise removal of every custom scheme. Built-in presets are provided, but custom preset management and persistent conversion history are not implemented. Animated and multipage inputs currently convert from their first decoded frame/page; animated export, crop/rotate/flip, metadata preservation, ICC controls, and multi-page export are not implemented. SVG, HEIF, JXL, PSD, camera RAW, and PDF are not registered.

AVIF decoding uses dav1d through the image crate's native feature. The webp crate statically builds its bundled libwebp C sources and is licensed MIT/Apache-2.0; libwebp carries its own BSD-style notice. Review bundled codec licenses before redistribution. The raster codec dependencies are not all pure Rust.

## Architecture

See docs/ARCHITECTURE.md and docs/FORMATS.md. The app stores settings in the platform config directory. Core conversion is local and does not make network requests.

## License

PixelForge source is MIT or Apache-2.0. Codec dependencies retain their own licenses; see each dependency's crate metadata before redistribution.

