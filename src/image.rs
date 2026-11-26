use image::RgbImage;
use mupdf::pdf::PdfDocument;
use mupdf::{Image, Pixmap};

use crate::Result;

pub(crate) fn cmyk_to_rgb(c: u8, m: u8, y: u8, k: u8) -> (u8, u8, u8) {
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
pub(crate) fn pixmap_to_rgb_and_alpha(pix: &Pixmap) -> (RgbImage, Option<Vec<u8>>) {
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
pub(crate) fn flatten_on_white(rgb: &RgbImage, alpha: Option<&[u8]>) -> RgbImage {
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
pub(crate) fn smask_xref(doc: &PdfDocument, xref: i32) -> Result<Option<i32>> {
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
pub(crate) fn decode_alpha(doc: &PdfDocument, xref: i32) -> Option<Vec<u8>> {
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
