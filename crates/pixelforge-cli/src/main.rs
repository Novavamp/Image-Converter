use clap::Parser;
use pixelforge_core::{ConversionOptions, OutputFormat, ResizeBounds, convert_file};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "pixelforge",
    version,
    about = "Offline image conversion and optimization"
)]
struct Args {
    /// Source image file.
    #[arg(short, long)]
    input: PathBuf,
    /// Destination directory. Defaults to a sibling PixelForge Output folder.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output format (png, jpeg, webp, avif, gif, bmp, tiff, ico, qoi, tga, pnm, hdr, exr, ff).
    #[arg(short, long)]
    format: String,
    /// Quality for JPEG, WebP, and AVIF, from 1 to 100.
    #[arg(short, long, default_value_t = 85)]
    quality: u8,
    /// Bound dimensions as WIDTHxHEIGHT (keeps aspect ratio; does not upscale).
    #[arg(long, value_parser = parse_resize)]
    resize: Option<ResizeBounds>,
    /// Encode WebP losslessly.
    #[arg(long)]
    webp_lossless: bool,
}

fn parse_resize(value: &str) -> Result<ResizeBounds, String> {
    let (width, height) = value
        .split_once(['x', 'X'])
        .ok_or("use WIDTHxHEIGHT, such as 1280x720")?;
    let width = width
        .parse::<u32>()
        .map_err(|_| "width must be a positive integer")?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "height must be a positive integer")?;
    if width == 0 || height == 0 {
        return Err("resize dimensions must be positive".into());
    }
    Ok(ResizeBounds { width, height })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();
    let args = Args::parse();
    let format = OutputFormat::from_extension(&args.format)
        .ok_or_else(|| format!("unsupported output format: {}", args.format))?;
    let output_dir = args.output.unwrap_or_else(|| {
        args.input
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("PixelForge Output")
    });
    let options = ConversionOptions {
        quality: args.quality.clamp(1, 100),
        resize: args.resize,
        webp_lossless: args.webp_lossless,
        ..ConversionOptions::default()
    };
    let report = convert_file(&args.input, output_dir, format, &options)?;
    println!("Saved {}", report.output.display());
    println!(
        "{} × {} · {} → {} · {:.1}% size change · {} ms",
        report.width,
        report.height,
        report.input_bytes,
        report.output_bytes,
        report.size_change_percent(),
        report.elapsed_ms
    );
    Ok(())
}
