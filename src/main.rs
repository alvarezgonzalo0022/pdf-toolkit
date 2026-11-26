use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, RgbImage};
use mupdf::pdf::{InsertPdfOptions, PdfDocument, PdfWriteOptions};
use mupdf::{Image, PageImageInfo, PageImageSource, Pixmap};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// --------------------------------------------------------------------------- //
// CLI
// --------------------------------------------------------------------------- //

#[derive(Parser)]
#[command(
    name = "pdf-toolkit",
    version,
    about = "A small multi-command toolkit for PDF files."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// reduce a PDF's file size
    Compress(CompressArgs),

    /// concatenate multiple PDFs into one
    Merge(MergeArgs),
}

#[derive(Args)]
struct CompressArgs {
    /// input PDF file
    input: PathBuf,

    /// output PDF file (default: <name>-compressed.pdf)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// target image resolution in DPI
    #[arg(long, default_value_t = 150)]
    dpi: u32,

    /// JPEG quality (1-100)
    #[arg(long, default_value_t = 70)]
    quality: u8,

    /// skip images already at or below this DPI
    #[arg(long, default_value_t = 72)]
    min_dpi: u32,

    /// convert images to grayscale
    #[arg(long)]
    grayscale: bool,

    /// target max size (e.g. 24mb); iteratively lowers quality/dpi to fit
    #[arg(long, value_parser = parse_size)]
    target: Option<u64>,

    /// print per-image details
    #[arg(long)]
    verbose: bool,
}

#[derive(Args)]
struct MergeArgs {
    /// PDF files to merge, in order
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    /// output PDF file (default: <first-stem>-merged.pdf)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// print per-file details
    #[arg(long)]
    verbose: bool,
}

// --------------------------------------------------------------------------- //
// utils (port of utils.py)
// --------------------------------------------------------------------------- //

/// Format a byte count as a human-readable string.
fn human_size(n: i64) -> String {
    let mut v = n as f64;
    for unit in ["B", "KB", "MB", "GB"] {
        if v < 1024.0 {
            return format!("{v:.1} {unit}");
        }
        v /= 1024.0;
    }
    format!("{v:.1} TB")
}

/// Convert '24mb' / '1.5mb' / '900kb' / '1048576' to bytes.
fn parse_size(text: &str) -> std::result::Result<u64, String> {
    let t = text.trim().to_lowercase();
    let mut mult: f64 = 1.0;
    let mut rest: &str = t.as_str();
    for (suffix, m) in [
        ("kb", 1024.0_f64),
        ("mb", 1024.0 * 1024.0),
        ("gb", 1024.0 * 1024.0 * 1024.0),
        ("b", 1.0),
    ] {
        if rest.ends_with(suffix) {
            mult = m;
            rest = &rest[..rest.len() - suffix.len()];
            break;
        }
    }
    let value: f64 = rest
        .trim()
        .parse()
        .map_err(|_| format!("invalid size: {text:?}"))?;
    Ok((value * mult) as u64)
}

// --------------------------------------------------------------------------- //
// image helpers
// --------------------------------------------------------------------------- //

fn cmyk_to_rgb(c: u8, m: u8, y: u8, k: u8) -> (u8, u8, u8) {
    let c = c as f32 / 255.0;
    let m = m as f32 / 255.0;
    let y = y as f32 / 255.0;
    let k = k as f32 / 255.0;
    let r = (255.0 * (1.0 - c) * (1.0 - k)).round();
    let g = (255.0 * (1.0 - m) * (1.0 - k)).round();
    let b = (255.0 * (1.0 - y) * (1.0 - k)).round();
    (r as u8, g as u8, b as u8)
}

/// Extract RGB pixels and (optionally) an alpha plane from a decoded pixmap.
fn pixmap_to_rgb_and_alpha(pix: &Pixmap) -> (RgbImage, Option<Vec<u8>>) {
    let w = pix.width() as usize;
    let h = pix.height() as usize;
    let n = pix.n() as usize;
    let has_alpha = pix.alpha();
    let color_n = if has_alpha { n - 1 } else { n };
    let stride = pix.stride().max(0) as usize;
    let samples = pix.samples();

    let mut rgb = RgbImage::new(w as u32, h as u32);
    let mut alpha = if has_alpha { Some(vec![0u8; w * h]) } else { None };

    for y in 0..h {
        let row = y * stride;
        for x in 0..w {
            let i = row + x * n;
            if i + n > samples.len() {
                continue;
            }
            let (r, g, b) = match color_n {
                1 => {
                    let v = samples[i];
                    (v, v, v)
                }
                3 => (samples[i], samples[i + 1], samples[i + 2]),
                4 => cmyk_to_rgb(samples[i], samples[i + 1], samples[i + 2], samples[i + 3]),
                _ => (0, 0, 0),
            };
            rgb.put_pixel(x as u32, y as u32, image::Rgb([r, g, b]));
            if let Some(a) = alpha.as_mut() {
                a[y * w + x] = samples[i + color_n];
            }
        }
    }

    (rgb, alpha)
}

/// Composite an RGB image over white using its alpha; no-op if fully opaque.
fn flatten_on_white(rgb: &RgbImage, alpha: Option<&[u8]>) -> RgbImage {
    let Some(alpha) = alpha else {
        return rgb.clone();
    };
    let (w, h) = rgb.dimensions();
    if (w as usize) * (h as usize) != alpha.len() {
        return rgb.clone();
    }

    let mut out = RgbImage::new(w, h);
    for (x, y, px) in rgb.enumerate_pixels() {
        let a = alpha[(y * w + x) as usize];
        if a == 255 {
            out.put_pixel(x, y, *px);
        } else {
            let a = a as u32;
            let blend = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
            let p = px.0;
            out.put_pixel(x, y, image::Rgb([blend(p[0]), blend(p[1]), blend(p[2])]));
        }
    }
    out
}

/// Return the xref of an image XObject's `/SMask`, if it has one.
fn smask_xref(doc: &PdfDocument, xref: i32) -> Result<Option<i32>> {
    let obj = doc.new_indirect(xref, 0)?;
    let Some(resolved) = obj.resolve()? else {
        return Ok(None);
    };
    let Some(smask) = resolved.get_dict("SMask")? else {
        return Ok(None);
    };
    if smask.is_null()? || !smask.is_indirect()? {
        return Ok(None);
    }
    Ok(Some(smask.as_indirect()?))
}

/// Decode a grayscale image (used as a soft mask) into a byte alpha plane.
fn decode_alpha(doc: &PdfDocument, xref: i32) -> Option<Vec<u8>> {
    let ext = doc.extract_image(xref).ok()?;
    let img = Image::from_bytes(&ext.encoded).ok()?;
    let pix = img.to_pixmap().ok()?;
    let w = pix.width() as usize;
    let h = pix.height() as usize;
    let n = pix.n() as usize;
    let stride = pix.stride().max(0) as usize;
    let samples = pix.samples();

    let mut alpha = vec![255u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * stride + x * n;
            if i < samples.len() {
                alpha[y * w + x] = samples[i];
            }
        }
    }
    Some(alpha)
}

// --------------------------------------------------------------------------- //
// core
// --------------------------------------------------------------------------- //

/// Downsample + re-encode one image. Returns `None` when it should be left
/// untouched (skipped by `--min-dpi`, no work needed, or undecodable).
#[allow(clippy::too_many_arguments)]
fn reencode_image(
    doc: &PdfDocument,
    info: &PageImageInfo,
    dpi: u32,
    quality: u8,
    grayscale: bool,
    min_dpi: u32,
) -> Result<Option<(Vec<u8>, u32, u32, f64)>> {
    let ext = doc.extract_image(info.xref)?;
    let img = match Image::from_bytes(&ext.encoded) {
        Ok(img) => img,
        Err(_) => return Ok(None),
    };

    let (xres, _) = img.resolution();
    let effective_dpi = if xres > 0 { xres as f64 } else { 72.0 };

    // New (functional) rule: leave images at or below `--min-dpi` fully intact.
    if effective_dpi <= min_dpi as f64 {
        return Ok(None);
    }

    let scale = (dpi as f64 / effective_dpi).min(1.0);

    let pix = match img.to_pixmap() {
        Ok(pix) => pix,
        Err(_) => return Ok(None),
    };

    let smask = smask_xref(doc, info.xref)?;
    let alpha = match smask {
        Some(sxref) => decode_alpha(doc, sxref),
        None if pix.alpha() => {
            let (_, a) = pixmap_to_rgb_and_alpha(&pix);
            a
        }
        None => None,
    };

    let needs_work = scale < 1.0 || grayscale || alpha.is_some();
    if !needs_work {
        return Ok(None);
    }

    let (rgb, _) = pixmap_to_rgb_and_alpha(&pix);
    let mut rgb = flatten_on_white(&rgb, alpha.as_deref());

    if grayscale {
        let gray = image::imageops::grayscale(&rgb);
        rgb = image::DynamicImage::ImageLuma8(gray).to_rgb8();
    }

    let new_w = (rgb.width() as f64 * scale).round().max(1.0) as u32;
    let new_h = (rgb.height() as f64 * scale).round().max(1.0) as u32;
    if (new_w, new_h) != rgb.dimensions() {
        rgb = image::imageops::resize(&rgb, new_w, new_h, image::imageops::FilterType::Lanczos3);
    }

    let mut jpeg = Vec::new();
    {
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg, quality.clamp(1, 100));
        encoder.encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )?;
    }

    Ok(Some((jpeg, new_w, new_h, effective_dpi)))
}

fn process_images(
    doc: &mut PdfDocument,
    dpi: u32,
    quality: u8,
    grayscale: bool,
    min_dpi: u32,
    verbose: bool,
) -> Result<(usize, usize)> {
    let page_count = doc.page_count()?;
    let mut total = 0usize;
    let mut changed = 0usize;

    for page_no in 0..page_count {
        let mut page = doc.load_pdf_page(page_no)?;
        let infos = page.images()?;

        for info in &infos {
            total += 1;
            let Some((jpeg, new_w, new_h, effective_dpi)) =
                reencode_image(doc, info, dpi, quality, grayscale, min_dpi)?
            else {
                continue;
            };

            page.replace_image(
                doc,
                info.xref,
                PageImageSource::Bytes {
                    data: &jpeg,
                    format_hint: None,
                },
            )?;

            if verbose {
                println!(
                    "  image xref {}: {}x{} -> {}x{} ({:.0} dpi -> {} dpi)",
                    info.xref, info.width, info.height, new_w, new_h, effective_dpi, dpi
                );
            }
            changed += 1;
        }
    }

    Ok((changed, total))
}

/// Best-effort metadata strip: drop the XMP stream and the `/Info` dict.
fn strip_metadata(doc: &PdfDocument) {
    if let Ok(mut catalog) = doc.catalog() {
        let _ = catalog.dict_delete("Metadata");
    }
    if let Ok(mut trailer) = doc.trailer() {
        let _ = trailer.dict_delete("Info");
    }
}

/// Open `data`, re-encode its images, and return the resulting PDF bytes.
fn compress_doc(
    data: &[u8],
    dpi: u32,
    quality: u8,
    grayscale: bool,
    min_dpi: u32,
    verbose: bool,
) -> Result<(Vec<u8>, usize, usize)> {
    let mut doc = PdfDocument::from_bytes(data)?;
    let (changed, total) = process_images(&mut doc, dpi, quality, grayscale, min_dpi, verbose)?;
    strip_metadata(&doc);

    let mut options = PdfWriteOptions::default();
    options.set_garbage_level(4); // garbage=4
    options.set_compress(true); // deflate=True
    options.set_clean(true); // clean=True

    let mut out = Vec::new();
    doc.write_to_with_options(&mut out, options)?;
    Ok((out, changed, total))
}

fn default_output(input: &Path, suffix: &str) -> PathBuf {
    let mut name = input
        .file_stem()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    input.with_file_name(name)
}

fn compress_once(args: &CompressArgs, output: &Path) -> Result<(usize, usize)> {
    let data = fs::read(&args.input)?;
    let (out, changed, total) =
        compress_doc(&data, args.dpi, args.quality, args.grayscale, args.min_dpi, args.verbose)?;
    fs::write(output, out)?;
    Ok((changed, total))
}

fn target_steps(dpi: u32, quality: u8) -> Vec<(u32, u8)> {
    let presets: [(u32, u8); 8] = [
        (dpi, 85),
        (200, 80),
        (150, 75),
        (150, 60),
        (120, 55),
        (100, 50),
        (80, 45),
        (72, 40),
    ];
    let mut steps = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (d, q) in presets {
        let dd = d.min(dpi);
        let qq = q.min(quality);
        if seen.insert((dd, qq)) {
            steps.push((dd, qq));
        }
    }
    steps
}

fn compress_to_target(
    args: &CompressArgs,
    output: &Path,
    target: u64,
) -> Result<(u64, u32, u8)> {
    let data = fs::read(&args.input)?;
    let steps = target_steps(args.dpi, args.quality);
    let mut last = *steps.last().expect("target steps must not be empty");

    for (dd, qq) in &steps {
        let (out, _, _) =
            compress_doc(&data, *dd, *qq, args.grayscale, args.min_dpi, args.verbose)?;
        let size = out.len() as u64;
        if args.verbose {
            println!("  try dpi={dd} quality={qq} -> {}", human_size(size as i64));
        }
        if size <= target {
            fs::write(output, out)?;
            return Ok((size, *dd, *qq));
        }
        last = (*dd, *qq);
    }

    // Last resort: grayscale at the most aggressive step.
    let (out, _, _) = compress_doc(&data, last.0, last.1, true, args.min_dpi, args.verbose)?;
    let size = out.len() as u64;
    fs::write(output, out)?;
    Ok((size, last.0, last.1))
}

// --------------------------------------------------------------------------- //
// command entry
// --------------------------------------------------------------------------- //

fn run_compress(args: CompressArgs) -> Result<u8> {
    if !args.input.is_file() {
        eprintln!("error: input not found: {}", args.input.display());
        return Ok(2);
    }

    let output = args
        .output
        .clone()
        .unwrap_or_else(|| default_output(&args.input, "-compressed.pdf"));

    let src_size = fs::metadata(&args.input)?.len();
    println!("input : {}  ({})", args.input.display(), human_size(src_size as i64));

    if let Some(target) = args.target {
        let (_, dd, qq) = compress_to_target(&args, &output, target)?;
        println!(
            "target: <= {} -> used dpi={dd} quality={qq}",
            human_size(target as i64)
        );
    } else {
        let (changed, total) = compress_once(&args, &output)?;
        println!("images: {changed} of {total} re-encoded");
    }

    let out_size = fs::metadata(&output)?.len();
    let ratio = if out_size > 0 {
        src_size as f64 / out_size as f64
    } else {
        0.0
    };
    println!("output: {}  ({})", output.display(), human_size(out_size as i64));
    println!(
        "saved : {} ({ratio:.1}x smaller)",
        human_size(src_size as i64 - out_size as i64)
    );
    Ok(0)
}

fn abspath(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

#[derive(Debug)]
struct EncryptedPdfError {
    path: PathBuf,
}

impl fmt::Display for EncryptedPdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "encrypted PDF (password required): {}",
            self.path.display()
        )
    }
}

impl std::error::Error for EncryptedPdfError {}

fn merge_docs(inputs: &[PathBuf], verbose: bool) -> Result<(Vec<u8>, usize)> {
    let mut doc = PdfDocument::new();
    let mut total = 0usize;

    for path in inputs {
        let data = fs::read(path)?;
        let src = PdfDocument::from_bytes(&data)?;
        if src.needs_password()? {
            return Err(Box::new(EncryptedPdfError { path: path.clone() }));
        }
        let pages = src.page_count()? as usize;
        doc.insert_pdf(&src, InsertPdfOptions::default())?;
        if verbose {
            println!("  + {} ({} pages)", path.display(), pages);
        }
        total += pages;
    }

    let mut options = PdfWriteOptions::default();
    options.set_garbage_level(4); // garbage=4
    options.set_compress(true); // deflate=True

    let mut out = Vec::new();
    doc.write_to_with_options(&mut out, options)?;
    Ok((out, total))
}

fn run_merge(args: MergeArgs) -> Result<u8> {
    for input in &args.inputs {
        if !input.is_file() {
            eprintln!("error: input not found: {}", input.display());
            return Ok(2);
        }
    }

    let output = args
        .output
        .clone()
        .unwrap_or_else(|| default_output(&args.inputs[0], "-merged.pdf"));

    let out_abs = abspath(&output);
    for input in &args.inputs {
        if abspath(input) == out_abs {
            eprintln!(
                "error: output would overwrite an input: {}",
                output.display()
            );
            return Ok(2);
        }
    }

    println!("inputs: {} files", args.inputs.len());

    let (out, total) = match merge_docs(&args.inputs, args.verbose) {
        Ok(merged) => merged,
        Err(err) => {
            if err.downcast_ref::<EncryptedPdfError>().is_some() {
                eprintln!("error: {err}");
                return Ok(2);
            }
            return Err(err);
        }
    };

    fs::write(&output, out)?;

    let out_size = fs::metadata(&output)?.len();
    println!(
        "output: {}  ({}, {} pages)",
        output.display(),
        human_size(out_size as i64),
        total
    );
    Ok(0)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Compress(args) => match run_compress(args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("error: {err}");
                1
            }
        },
        Command::Merge(args) => match run_merge(args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("error: {err}");
                1
            }
        },
    };
    ExitCode::from(code)
}

// --------------------------------------------------------------------------- //
// tests
// --------------------------------------------------------------------------- //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1048576").unwrap(), 1048576);
        assert_eq!(parse_size("900kb").unwrap(), 900 * 1024);
        assert_eq!(parse_size("1.5mb").unwrap(), (1.5 * 1024.0 * 1024.0) as u64);
        assert_eq!(parse_size("24mb").unwrap(), 24 * 1024 * 1024);
        assert_eq!(parse_size("1gb").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("100b").unwrap(), 100);
        assert!(parse_size("abc").is_err());
        assert!(parse_size("").is_err());
    }

    #[test]
    fn test_human_size() {
        assert_eq!(human_size(0), "0.0 B");
        assert_eq!(human_size(1023), "1023.0 B");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1048576), "1.0 MB");
        assert_eq!(human_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_default_output() {
        assert_eq!(
            default_output(Path::new("a.pdf"), "-merged.pdf"),
            PathBuf::from("a-merged.pdf")
        );
        assert_eq!(
            default_output(Path::new("a.b.pdf"), "-merged.pdf"),
            PathBuf::from("a.b-merged.pdf")
        );
        assert_eq!(
            default_output(Path::new("input.pdf"), "-compressed.pdf"),
            PathBuf::from("input-compressed.pdf")
        );
    }
}
