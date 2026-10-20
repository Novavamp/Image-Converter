//! PixelForge's offline image inspection and conversion engine.

use image::{
    DynamicImage, ImageEncoder, ImageFormat, ImageReader,
    codecs::{avif::AvifEncoder, jpeg::JpegEncoder},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Seek, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
use thiserror::Error;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Png,
    Jpeg,
    Webp,
    Avif,
    Gif,
    Bmp,
    Tiff,
    Ico,
    Qoi,
    Tga,
    Pnm,
    Hdr,
    Exr,
    Farbfeld,
}

impl OutputFormat {
    pub const PRIMARY: [Self; 9] = [
        Self::Png,
        Self::Jpeg,
        Self::Webp,
        Self::Avif,
        Self::Gif,
        Self::Bmp,
        Self::Tiff,
        Self::Ico,
        Self::Qoi,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Avif => "avif",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Ico => "ico",
            Self::Qoi => "qoi",
            Self::Tga => "tga",
            Self::Pnm => "ppm",
            Self::Hdr => "hdr",
            Self::Exr => "exr",
            Self::Farbfeld => "ff",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Webp => "WebP",
            Self::Avif => "AVIF",
            Self::Gif => "GIF",
            Self::Bmp => "BMP",
            Self::Tiff => "TIFF",
            Self::Ico => "ICO",
            Self::Qoi => "QOI",
            Self::Tga => "TGA",
            Self::Pnm => "PNM",
            Self::Hdr => "HDR",
            Self::Exr => "OpenEXR",
            Self::Farbfeld => "Farbfeld",
        }
    }

    pub fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Webp => ImageFormat::WebP,
            Self::Avif => ImageFormat::Avif,
            Self::Gif => ImageFormat::Gif,
            Self::Bmp => ImageFormat::Bmp,
            Self::Tiff => ImageFormat::Tiff,
            Self::Ico => ImageFormat::Ico,
            Self::Qoi => ImageFormat::Qoi,
            Self::Tga => ImageFormat::Tga,
            Self::Pnm => ImageFormat::Pnm,
            Self::Hdr => ImageFormat::Hdr,
            Self::Exr => ImageFormat::OpenExr,
            Self::Farbfeld => ImageFormat::Farbfeld,
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" | "jpe" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            "avif" => Some(Self::Avif),
            "gif" => Some(Self::Gif),
            "bmp" => Some(Self::Bmp),
            "tif" | "tiff" => Some(Self::Tiff),
            "ico" => Some(Self::Ico),
            "qoi" => Some(Self::Qoi),
            "tga" => Some(Self::Tga),
            "pnm" | "ppm" | "pgm" | "pbm" => Some(Self::Pnm),
            "hdr" => Some(Self::Hdr),
            "exr" => Some(Self::Exr),
            "ff" => Some(Self::Farbfeld),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FormatCapability {
    pub name: &'static str,
    pub format: Option<OutputFormat>,
    pub reads: bool,
    pub writes: bool,
    pub animation: bool,
    pub alpha: bool,
    pub lossless: bool,
    pub quality_control: bool,
    pub bit_depth: &'static str,
    pub notes: &'static str,
}

/// Capabilities reflect the encoders this build actually registers.
pub const FORMAT_CAPABILITIES: [FormatCapability; 15] = [
    capability(
        OutputFormat::Png,
        true,
        true,
        false,
        true,
        true,
        false,
        "8/16",
        "Metadata is stripped on export.",
    ),
    capability(
        OutputFormat::Jpeg,
        true,
        true,
        false,
        false,
        false,
        true,
        "8",
        "Transparency is composited over the chosen background.",
    ),
    capability(
        OutputFormat::Webp,
        true,
        true,
        false,
        true,
        true,
        true,
        "8",
        "Still images; lossy and lossless encoding.",
    ),
    capability(
        OutputFormat::Avif,
        true,
        true,
        false,
        true,
        false,
        true,
        "8",
        "Still images; native dav1d is bundled through Cargo.",
    ),
    capability(
        OutputFormat::Gif,
        true,
        true,
        true,
        true,
        false,
        false,
        "8",
        "Animated input is reduced to its first frame.",
    ),
    capability(
        OutputFormat::Bmp,
        true,
        true,
        false,
        true,
        true,
        false,
        "8",
        "Metadata is stripped on export.",
    ),
    capability(
        OutputFormat::Tiff,
        true,
        true,
        true,
        true,
        true,
        false,
        "8/16",
        "Export writes one page; multi-page input uses its first frame.",
    ),
    capability(
        OutputFormat::Ico,
        true,
        true,
        false,
        true,
        true,
        false,
        "8",
        "Export writes one icon image.",
    ),
    capability(
        OutputFormat::Qoi,
        true,
        true,
        false,
        true,
        true,
        false,
        "8",
        "Metadata is stripped on export.",
    ),
    capability(
        OutputFormat::Tga,
        true,
        true,
        false,
        true,
        true,
        false,
        "8",
        "Metadata is stripped on export.",
    ),
    capability(
        OutputFormat::Pnm,
        true,
        true,
        false,
        false,
        true,
        false,
        "8/16",
        "Export uses binary PPM.",
    ),
    capability(
        OutputFormat::Hdr,
        true,
        true,
        false,
        false,
        true,
        false,
        "32 float",
        "High dynamic range is encoded as RGBE.",
    ),
    capability(
        OutputFormat::Exr,
        true,
        true,
        false,
        true,
        true,
        false,
        "32 float",
        "OpenEXR support is provided by image-rs.",
    ),
    capability(
        OutputFormat::Farbfeld,
        true,
        true,
        false,
        true,
        true,
        false,
        "16",
        "Metadata is stripped on export.",
    ),
    FormatCapability {
        name: "DDS",
        format: None,
        reads: true,
        writes: false,
        animation: false,
        alpha: true,
        lossless: false,
        quality_control: false,
        bit_depth: "variant-dependent",
        notes: "Decode-only; support depends on the DDS pixel/compression variant.",
    },
];

#[allow(clippy::too_many_arguments)] // Keep capability registry rows aligned with the public fields.
const fn capability(
    format: OutputFormat,
    reads: bool,
    writes: bool,
    animation: bool,
    alpha: bool,
    lossless: bool,
    quality_control: bool,
    bit_depth: &'static str,
    notes: &'static str,
) -> FormatCapability {
    FormatCapability {
        name: format.label(),
        format: Some(format),
        reads,
        writes,
        animation,
        alpha,
        lossless,
        quality_control,
        bit_depth,
        notes,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ResizeBounds {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConversionOptions {
    /// Quality percentage for JPEG, WebP, and AVIF.
    pub quality: u8,
    /// Fit within these dimensions without changing aspect ratio. Upscaling is disabled.
    pub resize: Option<ResizeBounds>,
    /// Background used when transparency must be removed for JPEG.
    pub jpeg_background: [u8; 3],
    /// Enable WebP lossless mode.
    pub webp_lossless: bool,
    /// Remove recognized metadata chunks from the encoded output.
    pub strip_metadata: bool,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            quality: 85,
            resize: None,
            jpeg_background: [255, 255, 255],
            webp_lossless: false,
            strip_metadata: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub path: PathBuf,
    pub format: Option<ImageFormat>,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub has_alpha: bool,
    pub bit_depth: &'static str,
}

#[derive(Debug, Clone)]
pub struct ConversionReport {
    pub input: PathBuf,
    pub output: PathBuf,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub elapsed_ms: u128,
}

impl ConversionReport {
    pub fn size_change_percent(&self) -> f64 {
        if self.input_bytes == 0 {
            return 0.0;
        }
        (self.input_bytes as f64 - self.output_bytes as f64) * 100.0 / self.input_bytes as f64
    }
}

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("could not read image: {0}")]
    Read(#[from] image::ImageError),
    #[error("could not open input file: {0}")]
    Open(#[from] std::io::Error),
    #[error("could not detect an image format from the file contents")]
    UnknownFormat,
    #[error("output format {0} cannot encode this image")]
    UnsupportedOutput(&'static str),
    #[error("image dimensions are too large ({width} × {height})")]
    TooLarge { width: u32, height: u32 },
    #[error("conversion cancelled")]
    Cancelled,
    #[error("privacy cleanup failed: {0}")]
    PrivacyCleanup(String),
}

pub fn inspect(path: impl AsRef<Path>) -> Result<ImageInfo, ConversionError> {
    let path = path.as_ref();
    let bytes = fs::metadata(path)?.len();
    let (format, image) = read_image(path)?;
    let (width, height) = (image.width(), image.height());
    Ok(ImageInfo {
        path: path.to_path_buf(),
        format,
        width,
        height,
        bytes,
        has_alpha: image.color().has_alpha(),
        bit_depth: match image.color().bits_per_pixel()
            / u16::from(image.color().channel_count()).max(1)
        {
            1..=8 => "8-bit",
            9..=16 => "16-bit",
            _ => "32-bit / high depth",
        },
    })
}

/// Decodes content-sniffed input after checking its dimensions and decoder memory limits.
pub fn read_image(
    path: impl AsRef<Path>,
) -> Result<(Option<ImageFormat>, DynamicImage), ConversionError> {
    let path = path.as_ref();
    let reader = open_reader(path)?;
    let format = reader.format();
    if format.is_none() {
        return Err(ConversionError::UnknownFormat);
    }
    let (width, height) = reader.into_dimensions()?;
    check_dimensions(width, height)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(512 * 1024 * 1024);
    let mut reader = open_reader(path)?;
    reader.limits(limits);
    let image = reader.decode()?;
    Ok((format, image))
}

fn open_reader(path: &Path) -> Result<ImageReader<BufReader<File>>, ConversionError> {
    let mut reader = ImageReader::new(BufReader::new(File::open(path)?)).with_guessed_format()?;
    if reader.format().is_none() {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if extension.eq_ignore_ascii_case("dds") {
            reader.set_format(ImageFormat::Dds);
        } else if let Some(format) = OutputFormat::from_extension(extension) {
            reader.set_format(format.image_format());
        }
    }
    Ok(reader)
}

pub fn convert_file(
    input: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
    format: OutputFormat,
    options: &ConversionOptions,
) -> Result<ConversionReport, ConversionError> {
    let input = input.as_ref();
    let output_dir = output_dir.as_ref();
    let start = Instant::now();
    let source_bytes = fs::metadata(input)?.len();
    let (actual_format, mut decoded) = read_image(input)?;
    if actual_format.is_none() {
        return Err(ConversionError::UnknownFormat);
    }
    if let Some(bounds) = options.resize {
        if bounds.width == 0 || bounds.height == 0 {
            return Err(ConversionError::UnsupportedOutput(
                "resize dimensions must be positive",
            ));
        }
        let (width, height) = fit_dimensions(decoded.width(), decoded.height(), bounds);
        if (width, height) != (decoded.width(), decoded.height()) {
            decoded = decoded.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
        }
    }
    fs::create_dir_all(output_dir)?;
    let base = input.file_stem().unwrap_or_default();
    let proposed = output_dir.join(base).with_extension(format.extension());
    let output = unique_path(&proposed, input)?;
    let (temp_path, mut writer) = create_temp(&output)?;
    let publish = encode(&decoded, format, options, &mut writer).and_then(|()| {
        let file = writer
            .into_inner()
            .map_err(|error| ConversionError::Open(error.into_error()))?;
        file.sync_all()?;
        drop(file);
        if options.strip_metadata {
            sanitize_output_metadata(&temp_path, format)?;
        }
        fs::rename(&temp_path, &output)?;
        Ok(())
    });
    if let Err(error) = publish {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    let output_bytes = fs::metadata(&output)?.len();
    tracing::info!(input = %input.display(), output = %output.display(), bytes = output_bytes, "image converted");
    Ok(ConversionReport {
        input: input.to_path_buf(),
        output,
        input_bytes: source_bytes,
        output_bytes,
        width: decoded.width(),
        height: decoded.height(),
        elapsed_ms: start.elapsed().as_millis(),
    })
}

fn check_dimensions(width: u32, height: u32) -> Result<(), ConversionError> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 100_000_000 {
        return Err(ConversionError::TooLarge { width, height });
    }
    Ok(())
}

pub fn fit_dimensions(width: u32, height: u32, bounds: ResizeBounds) -> (u32, u32) {
    if width == 0 || height == 0 || bounds.width == 0 || bounds.height == 0 {
        return (0, 0);
    }
    let ratio = f64::min(
        bounds.width as f64 / width as f64,
        bounds.height as f64 / height as f64,
    )
    .min(1.0);
    (
        (width as f64 * ratio).round().max(1.0) as u32,
        (height as f64 * ratio).round().max(1.0) as u32,
    )
}

fn unique_path(proposed: &Path, input: &Path) -> Result<PathBuf, std::io::Error> {
    let stem = proposed.file_stem().unwrap_or_default().to_string_lossy();
    let ext = proposed.extension().unwrap_or_default().to_string_lossy();
    let parent = proposed.parent().unwrap_or_else(|| Path::new("."));
    for number in 0u32..=9999 {
        let name = if number == 0 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem} ({number}).{ext}")
        };
        let candidate = parent.join(name);
        if candidate == input {
            continue;
        }
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not find a free output filename after 10,000 attempts",
    ))
}

fn create_temp(output: &Path) -> Result<(PathBuf, BufWriter<File>), std::io::Error> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    for _ in 0..100 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(".pixelforge-{}-{sequence}.tmp", std::process::id());
        let path = parent.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, BufWriter::new(file))),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create a temporary output file",
    ))
}

fn encode(
    image: &DynamicImage,
    format: OutputFormat,
    options: &ConversionOptions,
    writer: &mut (impl Write + Seek),
) -> Result<(), ConversionError> {
    let quality = options.quality.clamp(1, 100);
    match format {
        OutputFormat::Jpeg => {
            let rgb = composite(image, options.jpeg_background);
            JpegEncoder::new_with_quality(writer, quality).write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )?;
        }
        OutputFormat::Webp => {
            let rgba = image.to_rgba8();
            let encoder = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
            let bytes = if options.webp_lossless {
                encoder.encode_lossless()
            } else {
                encoder.encode(f32::from(quality))
            };
            writer.write_all(&bytes)?;
        }
        OutputFormat::Avif => {
            let rgba = image.to_rgba8();
            AvifEncoder::new_with_speed_quality(writer, 6, quality).write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            )?;
        }
        OutputFormat::Ico if image.width() > 256 || image.height() > 256 => {
            return Err(ConversionError::UnsupportedOutput(format.label()));
        }
        _ => image
            .write_to(writer, format.image_format())
            .map_err(|e| match e {
                image::ImageError::Unsupported(_) => {
                    ConversionError::UnsupportedOutput(format.label())
                }
                other => ConversionError::Read(other),
            })?,
    }
    Ok(())
}

fn sanitize_output_metadata(path: &Path, format: OutputFormat) -> Result<(), ConversionError> {
    let encoded = fs::read(path).map_err(privacy_error)?;
    let sanitized = match format {
        OutputFormat::Jpeg => strip_jpeg_metadata(&encoded),
        OutputFormat::Png => strip_png_metadata(&encoded),
        OutputFormat::Webp => strip_webp_metadata(&encoded),
        // The remaining encoders emit new pixel-only files; they do not copy source metadata.
        _ => Ok(encoded),
    }
    .map_err(privacy_error)?;

    fs::write(path, sanitized).map_err(privacy_error)?;
    OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(privacy_error)?;
    Ok(())
}

fn privacy_error(error: std::io::Error) -> ConversionError {
    ConversionError::PrivacyCleanup(error.to_string())
}

fn strip_png_metadata(encoded: &[u8]) -> std::io::Result<Vec<u8>> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    let invalid = || std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid PNG output");
    if !encoded.starts_with(SIGNATURE) {
        return Err(invalid());
    }

    let mut sanitized = Vec::with_capacity(encoded.len());
    sanitized.extend_from_slice(SIGNATURE);
    let mut position = SIGNATURE.len();
    let mut found_end = false;

    while position < encoded.len() {
        if encoded.len() - position < 12 {
            return Err(invalid());
        }
        let length = u32::from_be_bytes(
            encoded[position..position + 4]
                .try_into()
                .map_err(|_| invalid())?,
        ) as usize;
        let chunk_end = position
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .filter(|end| *end <= encoded.len())
            .ok_or_else(invalid)?;
        let chunk_type = &encoded[position + 4..position + 8];
        let is_end = chunk_type == b"IEND";
        let is_transparency = chunk_type == b"tRNS";
        let is_required_image_chunk = chunk_type == b"IHDR"
            || chunk_type == b"PLTE"
            || chunk_type == b"IDAT"
            || chunk_type == b"IEND";

        // Keep only required image data and palette transparency. Remove every other chunk,
        // including EXIF, text, XMP, ICC profiles, timestamps, and unknown tracking data.
        if is_required_image_chunk || is_transparency {
            sanitized.extend_from_slice(&encoded[position..chunk_end]);
        }
        position = chunk_end;
        if is_end {
            found_end = true;
            break;
        }
    }

    if !found_end {
        return Err(invalid());
    }
    Ok(sanitized)
}

fn strip_jpeg_metadata(encoded: &[u8]) -> std::io::Result<Vec<u8>> {
    let invalid = || std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid JPEG output");
    if !encoded.starts_with(b"\xff\xd8") {
        return Err(invalid());
    }

    let mut sanitized = encoded[..2].to_vec();
    let mut position = 2;
    while position < encoded.len() {
        let marker_start = position;
        if encoded[position] != 0xff {
            return Err(invalid());
        }
        while position < encoded.len() && encoded[position] == 0xff {
            position += 1;
        }
        let marker = *encoded.get(position).ok_or_else(invalid)?;
        position += 1;

        if marker == 0xda {
            // PixelForge's JPEG encoder writes a single scan. Copy the compressed image data
            // unchanged after removing metadata from the header.
            sanitized.extend_from_slice(&encoded[marker_start..]);
            return Ok(sanitized);
        }
        if marker == 0xd9 {
            sanitized.extend_from_slice(&encoded[marker_start..position]);
            return Ok(sanitized);
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            sanitized.extend_from_slice(&encoded[marker_start..position]);
            continue;
        }

        let length_end = position
            .checked_add(2)
            .filter(|end| *end <= encoded.len())
            .ok_or_else(invalid)?;
        let length = u16::from_be_bytes(
            encoded[position..length_end]
                .try_into()
                .map_err(|_| invalid())?,
        ) as usize;
        if length < 2 {
            return Err(invalid());
        }
        let segment_end = position
            .checked_add(length)
            .filter(|end| *end <= encoded.len())
            .ok_or_else(invalid)?;
        let is_metadata = (0xe0..=0xef).contains(&marker) || marker == 0xfe;
        if !is_metadata {
            sanitized.extend_from_slice(&encoded[marker_start..segment_end]);
        }
        position = segment_end;
    }
    Err(invalid())
}

fn strip_webp_metadata(encoded: &[u8]) -> std::io::Result<Vec<u8>> {
    let invalid = || std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid WebP output");
    if encoded.len() < 12 || &encoded[..4] != b"RIFF" || &encoded[8..12] != b"WEBP" {
        return Err(invalid());
    }
    let riff_end = 8usize
        .checked_add(u32::from_le_bytes(encoded[4..8].try_into().map_err(|_| invalid())?) as usize)
        .filter(|end| *end <= encoded.len() && *end >= 12)
        .ok_or_else(invalid)?;
    let mut sanitized = encoded[..12].to_vec();
    let mut position = 12;

    while position < riff_end {
        if riff_end - position < 8 {
            return Err(invalid());
        }
        let chunk_type = &encoded[position..position + 4];
        let length = u32::from_le_bytes(
            encoded[position + 4..position + 8]
                .try_into()
                .map_err(|_| invalid())?,
        ) as usize;
        let payload_start = position + 8;
        let chunk_end = payload_start
            .checked_add(length)
            .filter(|end| *end <= riff_end)
            .ok_or_else(invalid)?;
        let padded_end = chunk_end
            .checked_add(length & 1)
            .filter(|end| *end <= riff_end)
            .ok_or_else(invalid)?;

        let known_image_chunk = chunk_type == b"VP8X"
            || chunk_type == b"VP8 "
            || chunk_type == b"VP8L"
            || chunk_type == b"ALPH"
            || chunk_type == b"ANIM"
            || chunk_type == b"ANMF";
        if known_image_chunk {
            sanitized.extend_from_slice(&encoded[position..padded_end]);
            if chunk_type == b"VP8X" {
                let flags_index = sanitized.len() - (padded_end - position) + 8;
                let flags = sanitized.get_mut(flags_index).ok_or_else(invalid)?;
                // Clear ICC, EXIF, and XMP presence flags while retaining alpha/animation bits.
                *flags &= !0x2c;
            }
        }
        position = padded_end;
    }

    let riff_size = u32::try_from(sanitized.len() - 8).map_err(|_| invalid())?;
    sanitized[4..8].copy_from_slice(&riff_size.to_le_bytes());
    Ok(sanitized)
}

fn composite(image: &DynamicImage, bg: [u8; 3]) -> image::RgbImage {
    let rgba = image.to_rgba8();
    image::ImageBuffer::from_fn(rgba.width(), rgba.height(), |x, y| {
        let pixel = rgba.get_pixel(x, y).0;
        let alpha = u16::from(pixel[3]);
        image::Rgb(std::array::from_fn(|channel| {
            ((u16::from(pixel[channel]) * alpha + u16::from(bg[channel]) * (255 - alpha) + 127)
                / 255) as u8
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch() -> PathBuf {
        std::env::temp_dir().join(format!(
            "pixelforge-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn extension_detection_is_case_insensitive_and_handles_aliases() {
        assert_eq!(
            OutputFormat::from_extension("JpEg"),
            Some(OutputFormat::Jpeg)
        );
        assert_eq!(
            OutputFormat::from_extension("tif"),
            Some(OutputFormat::Tiff)
        );
        assert_eq!(OutputFormat::from_extension("not-image"), None);
    }

    #[test]
    fn resizing_keeps_aspect_ratio_and_does_not_upscale() {
        assert_eq!(
            fit_dimensions(
                4000,
                2000,
                ResizeBounds {
                    width: 1000,
                    height: 1000
                }
            ),
            (1000, 500)
        );
        assert_eq!(
            fit_dimensions(
                200,
                100,
                ResizeBounds {
                    width: 1000,
                    height: 1000
                }
            ),
            (200, 100)
        );
    }

    #[test]
    fn png_to_jpeg_composites_transparency_and_keeps_original() {
        let root = scratch();
        let out = root.join("out");
        fs::create_dir_all(&root).unwrap();
        let input = root.join("alpha.png");
        let mut pixels = image::RgbaImage::new(2, 1);
        pixels.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        pixels.put_pixel(1, 0, image::Rgba([0, 0, 0, 0]));
        pixels.save(&input).unwrap();
        let report = convert_file(
            &input,
            &out,
            OutputFormat::Jpeg,
            &ConversionOptions::default(),
        )
        .unwrap();
        let converted = image::open(&report.output).unwrap().to_rgb8();
        assert_eq!(converted.dimensions(), (2, 1));
        assert!(converted.get_pixel(1, 0)[0] > 240);
        assert!(input.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn qoi_conversion_round_trips_pixels() {
        let root = scratch();
        let out = root.join("out");
        fs::create_dir_all(&root).unwrap();
        let input = root.join("fixture.png");
        let img = image::RgbaImage::from_fn(7, 5, |x, y| image::Rgba([x as u8, y as u8, 50, 255]));
        img.save(&input).unwrap();
        let report = convert_file(
            &input,
            &out,
            OutputFormat::Qoi,
            &ConversionOptions::default(),
        )
        .unwrap();
        assert_eq!(image::open(report.output).unwrap().to_rgba8(), img);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn format_detection_prefers_content_over_the_extension() {
        let root = scratch();
        fs::create_dir_all(&root).unwrap();
        let input = root.join("misleading.jpg");
        image::RgbImage::new(3, 2)
            .save_with_format(&input, ImageFormat::Png)
            .unwrap();
        assert_eq!(inspect(input).unwrap().format, Some(ImageFormat::Png));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn webp_lossy_and_lossless_encoders_write_decodable_images() {
        let root = scratch();
        fs::create_dir_all(&root).unwrap();
        let input = root.join("fixture.png");
        let pixels = image::RgbaImage::from_fn(16, 12, |x, y| {
            image::Rgba([x as u8 * 8, y as u8 * 10, 70, 255])
        });
        pixels.save(&input).unwrap();
        let mut options = ConversionOptions::default();
        let lossy = convert_file(&input, root.join("lossy"), OutputFormat::Webp, &options).unwrap();
        assert_eq!(image::open(&lossy.output).unwrap().dimensions(), (16, 12));
        options.webp_lossless = true;
        let lossless =
            convert_file(&input, root.join("lossless"), OutputFormat::Webp, &options).unwrap();
        assert_eq!(image::open(&lossless.output).unwrap().to_rgba8(), pixels);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn jpeg_to_avif_to_png_round_trip_keeps_dimensions() {
        let root = scratch();
        fs::create_dir_all(&root).unwrap();
        let input = root.join("photo.jpg");
        image::RgbImage::from_fn(24, 18, |x, y| image::Rgb([x as u8 * 5, y as u8 * 7, 110]))
            .save_with_format(&input, ImageFormat::Jpeg)
            .unwrap();
        let avif = convert_file(
            &input,
            root.join("avif"),
            OutputFormat::Avif,
            &ConversionOptions {
                quality: 70,
                ..ConversionOptions::default()
            },
        )
        .unwrap();
        let png = convert_file(
            &avif.output,
            root.join("png"),
            OutputFormat::Png,
            &ConversionOptions::default(),
        )
        .unwrap();
        assert_eq!(image::open(&png.output).unwrap().dimensions(), (24, 18));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gif_and_tiff_inputs_convert_to_png() {
        let root = scratch();
        fs::create_dir_all(&root).unwrap();
        for (filename, format) in [
            ("input.gif", ImageFormat::Gif),
            ("input.tiff", ImageFormat::Tiff),
        ] {
            let input = root.join(filename);
            image::RgbImage::new(9, 7)
                .save_with_format(&input, format)
                .unwrap();
            let report = convert_file(
                &input,
                root.join(format!("{filename}.out")),
                OutputFormat::Png,
                &ConversionOptions::default(),
            )
            .unwrap();
            assert_eq!(image::open(report.output).unwrap().dimensions(), (9, 7));
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn existing_names_get_numbered_and_corrupt_input_is_an_error() {
        let root = scratch();
        let out = root.join("out");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&out).unwrap();
        let input = root.join("same.png");
        image::RgbImage::new(2, 2).save(&input).unwrap();
        let options = ConversionOptions::default();
        let first = convert_file(&input, &out, OutputFormat::Png, &options).unwrap();
        let second = convert_file(&input, &out, OutputFormat::Png, &options).unwrap();
        assert_ne!(first.output, second.output);
        let broken = root.join("broken.png");
        fs::write(&broken, b"not an image").unwrap();
        assert!(convert_file(&broken, &out, OutputFormat::Png, &options).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn privacy_cleanup_removes_metadata_chunks_from_png_jpeg_and_webp() {
        let image = image::RgbaImage::from_fn(16, 12, |x, y| {
            image::Rgba([x as u8 * 8, y as u8 * 10, 70, 255])
        });
        let metadata = b"AI-Generated GPSLatitude tracking-id";

        let mut png = Vec::new();
        DynamicImage::ImageRgba8(image.clone())
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        insert_png_chunk(&mut png, b"tEXt", metadata);
        insert_png_chunk(&mut png, b"AIxx", b"custom-tracker");
        let clean_png = strip_png_metadata(&png).unwrap();
        assert!(
            !clean_png
                .windows(metadata.len())
                .any(|bytes| bytes == metadata)
        );
        assert!(
            !clean_png
                .windows(b"custom-tracker".len())
                .any(|bytes| bytes == b"custom-tracker")
        );
        assert_eq!(
            image::load_from_memory(&clean_png).unwrap().dimensions(),
            (16, 12)
        );

        let mut jpeg = Vec::new();
        let rgb = DynamicImage::ImageRgba8(image.clone()).to_rgb8();
        JpegEncoder::new(&mut jpeg)
            .write_image(&rgb.into_raw(), 16, 12, image::ExtendedColorType::Rgb8)
            .unwrap();
        insert_jpeg_segment(&mut jpeg, 0xe1, metadata);
        let clean_jpeg = strip_jpeg_metadata(&jpeg).unwrap();
        assert!(
            !clean_jpeg
                .windows(metadata.len())
                .any(|bytes| bytes == metadata)
        );
        assert_eq!(
            image::load_from_memory(&clean_jpeg).unwrap().dimensions(),
            (16, 12)
        );

        let webp = webp::Encoder::from_rgba(image.as_raw(), 16, 12)
            .encode_lossless()
            .to_vec();
        let webp = add_webp_exif_chunk(&webp, metadata, 16, 12);
        assert!(webp.windows(metadata.len()).any(|bytes| bytes == metadata));
        let clean_webp = strip_webp_metadata(&webp).unwrap();
        assert!(
            !clean_webp
                .windows(metadata.len())
                .any(|bytes| bytes == metadata)
        );
        assert_eq!(
            image::load_from_memory(&clean_webp).unwrap().dimensions(),
            (16, 12)
        );
    }

    fn insert_png_chunk(encoded: &mut Vec<u8>, chunk_type: &[u8; 4], payload: &[u8]) {
        assert_eq!(&encoded[..8], b"\x89PNG\r\n\x1a\n");
        let first_length = u32::from_be_bytes(encoded[8..12].try_into().unwrap()) as usize;
        let position = 8 + first_length + 12;
        let mut chunk = Vec::with_capacity(payload.len() + 12);
        chunk.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        chunk.extend_from_slice(chunk_type);
        chunk.extend_from_slice(payload);
        let checksum = crc32fast::hash(&chunk[4..]);
        chunk.extend_from_slice(&checksum.to_be_bytes());
        encoded.splice(position..position, chunk);
    }

    fn insert_jpeg_segment(encoded: &mut Vec<u8>, marker: u8, payload: &[u8]) {
        let length = u16::try_from(payload.len() + 2).unwrap();
        let mut segment = vec![0xff, marker];
        segment.extend_from_slice(&length.to_be_bytes());
        segment.extend_from_slice(payload);
        encoded.splice(2..2, segment);
    }

    fn add_webp_exif_chunk(encoded: &[u8], metadata: &[u8], width: u32, height: u32) -> Vec<u8> {
        assert_eq!(&encoded[..4], b"RIFF");
        let mut result = encoded[..12].to_vec();
        let mut vp8x = [0u8; 10];
        vp8x[0] = 0x08; // EXIF metadata-present flag.
        vp8x[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
        vp8x[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
        append_webp_chunk(&mut result, b"VP8X", &vp8x);
        result.extend_from_slice(&encoded[12..]);
        append_webp_chunk(&mut result, b"EXIF", metadata);
        let riff_size = u32::try_from(result.len() - 8).unwrap();
        result[4..8].copy_from_slice(&riff_size.to_le_bytes());
        result
    }

    fn append_webp_chunk(encoded: &mut Vec<u8>, chunk_type: &[u8; 4], payload: &[u8]) {
        encoded.extend_from_slice(chunk_type);
        encoded.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        encoded.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            encoded.push(0);
        }
    }
}
