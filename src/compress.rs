use std::fs;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::ExtendedColorType;
use mupdf::pdf::{PdfDocument, PdfWriteOptions};
use mupdf::{Image, PageImageInfo, PageImageSource};

use crate::cli::CompressArgs;
use crate::image::{
    decode_alpha, flatten_on_white, pixmap_to_rgb_and_alpha, smask_xref,
};
use crate::utils::{default_output, human_size};
use crate::Result;

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

pub fn run_compress(args: CompressArgs) -> Result<u8> {
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
