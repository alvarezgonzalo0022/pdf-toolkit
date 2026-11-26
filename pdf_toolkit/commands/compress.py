"""`compress` — reduce a PDF's file size.

It shrinks PDFs by:
  * downsampling oversized embedded images to a target DPI
  * re-encoding them as JPEG at a configurable quality
  * flattening transparency (dropping redundant alpha/soft-masks)
  * optionally converting to grayscale
  * stripping metadata and garbage-collecting unused objects
"""

import io
import os
import sys

import pymupdf as fitz
from PIL import Image

from ..command import Command
from ..utils import human_size, parse_size

IMAGE_JPEG = "JPEG"


# --------------------------------------------------------------------------- #
# Image helpers
# --------------------------------------------------------------------------- #
def _pix_alpha(pix):
    """Return a grayscale PIL image of the pixmap's alpha channel, or None."""
    if not pix.alpha:
        return None
    n = pix.n
    samples = pix.samples
    return Image.frombytes(
        "L",
        (pix.width, pix.height),
        bytes(samples[i] for i in range(n - 1, len(samples), n)),
    )


def _pix_to_rgb(pix):
    """Return an RGB PIL image of the pixmap."""
    n = pix.n
    cs = pix.colorspace
    samples = bytes(pix.samples)
    if cs is None or n == 1:
        img = Image.frombytes("L", (pix.width, pix.height), samples)
        return img.convert("RGB")
    if n == 2:
        img = Image.frombytes("LA", (pix.width, pix.height), samples)
        return img.convert("RGB")
    if n == 3:
        return Image.frombytes("RGB", (pix.width, pix.height), samples)
    if n == 4:
        img = Image.frombytes("RGBA", (pix.width, pix.height), samples)
        return img.convert("RGB")
    # CMYK or unusual colorspace: let PyMuPDF convert it.
    rgb = fitz.Pixmap(fitz.csRGB, pix)
    return Image.frombytes("RGB", (rgb.width, rgb.height), bytes(rgb.samples))


def decode_image(doc, xref, smask):
    """Return (RGB PIL image, alpha PIL image-or-None) for an image xref."""
    pix = fitz.Pixmap(doc, xref)
    rgb = _pix_to_rgb(pix)
    alpha = _pix_alpha(pix)
    if smask:
        mask = fitz.Pixmap(doc, smask)
        alpha = Image.frombytes("L", (mask.width, mask.height), bytes(mask.samples))
    return rgb, alpha


def flatten_on_white(rgb_img, alpha):
    """Composite an image over white using its alpha; no-op if fully opaque."""
    if alpha is None:
        return rgb_img
    if alpha.getextrema() == (255, 255):
        return rgb_img  # mask is fully opaque -> just drop it
    bg = Image.new("RGB", rgb_img.size, (255, 255, 255))
    bg.paste(rgb_img, mask=alpha)
    return bg


def strip_metadata(doc):
    doc.set_metadata({})
    try:
        doc.del_xml_metadata()
    except Exception:
        pass


# --------------------------------------------------------------------------- #
# Core
# --------------------------------------------------------------------------- #
def collect_images(doc):
    """Return {xref: (smask, width, height, max_dpi)} for every unique image."""
    images = {}
    for page in doc:
        for img in page.get_images(full=True):
            xref, smask, w, h = img[0], img[1], img[2], img[3]
            if xref in images:
                continue
            dpi = 72.0
            for r in page.get_image_rects(xref):
                dpi = max(dpi, w / max(r.width, 1) * 72, h / max(r.height, 1) * 72)
            images[xref] = (smask, w, h, dpi)
    return images


def process_images(doc, dpi, quality, grayscale, min_dpi, verbose=False):
    """Downsample + re-encode all images. Returns (changed, total)."""
    images = collect_images(doc)
    total = len(images)
    changed = 0

    for xref, (smask, w, h, cur_dpi) in images.items():
        scale = min(1.0, dpi / max(cur_dpi, 1.0))
        needs_work = (scale < 1.0) or grayscale or (smask != 0)
        if not needs_work:
            continue

        rgb_img, alpha = decode_image(doc, xref, smask)
        rgb_img = flatten_on_white(rgb_img, alpha)

        if grayscale:
            rgb_img = rgb_img.convert("L").convert("RGB")

        new_w = max(1, int(round(rgb_img.width * scale)))
        new_h = max(1, int(round(rgb_img.height * scale)))
        if (new_w, new_h) != rgb_img.size:
            rgb_img = rgb_img.resize((new_w, new_h), Image.Resampling.LANCZOS)

        buf = io.BytesIO()
        rgb_img.save(buf, IMAGE_JPEG, quality=quality, optimize=True, progressive=True)

        doc[0].replace_image(xref, stream=buf.getvalue())
        if smask:
            doc.xref_set_key(xref, "SMask", "null")

        if verbose:
            print(
                f"  image xref {xref}: {w}x{h} -> {new_w}x{new_h} "
                f"({cur_dpi:.0f} dpi -> {dpi} dpi)"
            )
        changed += 1

    return changed, total


def _compress_doc(data, dpi, quality, grayscale, min_dpi=0, verbose=False):
    """Open `data`, re-encode its images, and return (bytes, changed, total)."""
    doc = fitz.open(stream=io.BytesIO(data), filetype="pdf")
    try:
        changed, total = process_images(doc, dpi, quality, grayscale, min_dpi, verbose)
        strip_metadata(doc)
        return doc.tobytes(garbage=4, deflate=True, clean=True), changed, total
    finally:
        doc.close()


def compress_bytes(data, dpi, quality, grayscale):
    """Compress a PDF in memory and return the resulting bytes."""
    out, _, _ = _compress_doc(data, dpi, quality, grayscale)
    return out


def _target_steps(dpi, quality):
    """Return the ordered (dpi, quality) presets used by the target loop."""
    presets = [
        (dpi, 85),
        (200, 80),
        (150, 75),
        (150, 60),
        (120, 55),
        (100, 50),
        (80, 45),
        (72, 40),
    ]
    steps = []
    seen = set()
    for d, q in presets:
        dd, qq = min(d, dpi), min(q, quality)
        if (dd, qq) not in seen:
            seen.add((dd, qq))
            steps.append((dd, qq))
    return steps


def compress_bytes_to_target(data, dpi, quality, grayscale, target):
    """Iteratively lower dpi/quality until len(bytes) <= target.

    Returns (bytes, dpi_used, quality_used).
    """
    steps = _target_steps(dpi, quality)
    last = steps[-1]
    for dd, qq in steps:
        out = compress_bytes(data, dd, qq, grayscale)
        if len(out) <= target:
            return out, dd, qq
        last = (dd, qq)

    # Last resort: grayscale at the most aggressive step.
    out = compress_bytes(data, last[0], last[1], True)
    return out, last[0], last[1]


def compress_once(src, dst, dpi, quality, grayscale, min_dpi, verbose):
    """CLI wrapper: read `src` from disk, compress in memory, write `dst`."""
    with open(src, "rb") as f:
        data = f.read()
    out, changed, total = _compress_doc(data, dpi, quality, grayscale, min_dpi, verbose)
    with open(dst, "wb") as f:
        f.write(out)
    return changed, total


def compress_to_target(src, dst, dpi, quality, grayscale, min_dpi, target, verbose):
    """CLI wrapper: try progressively more aggressive settings until it fits."""
    with open(src, "rb") as f:
        data = f.read()

    steps = _target_steps(dpi, quality)
    last = steps[-1]
    out = None
    for dd, qq in steps:
        out, _, _ = _compress_doc(data, dd, qq, grayscale, min_dpi, verbose)
        size = len(out)
        if verbose:
            print(f"  try dpi={dd} quality={qq} -> {human_size(size)}")
        if size <= target:
            with open(dst, "wb") as f:
                f.write(out)
            return size, dd, qq
        last = (dd, qq)

    # Last resort: grayscale at the most aggressive step.
    out, _, _ = _compress_doc(data, last[0], last[1], True, min_dpi, verbose)
    with open(dst, "wb") as f:
        f.write(out)
    return len(out), last[0], last[1]


# --------------------------------------------------------------------------- #
# Command definition
# --------------------------------------------------------------------------- #
def add_arguments(parser):
    parser.add_argument("input", help="input PDF file")
    parser.add_argument(
        "-o", "--output", help="output PDF file (default: <name>-compressed.pdf)"
    )
    parser.add_argument(
        "--dpi", type=int, default=150, help="target image resolution in DPI"
    )
    parser.add_argument(
        "--quality", type=int, default=70, help="JPEG quality (1-100)"
    )
    parser.add_argument(
        "--min-dpi",
        type=int,
        default=72,
        help="skip images already at or below this DPI",
    )
    parser.add_argument(
        "--grayscale", action="store_true", help="convert images to grayscale"
    )
    parser.add_argument(
        "--target",
        metavar="SIZE",
        type=parse_size,
        help="target max size (e.g. 24mb); iteratively lowers quality/dpi to fit",
    )
    parser.add_argument("--verbose", action="store_true", help="print per-image details")


def run(args):
    if not os.path.isfile(args.input):
        print(f"error: input not found: {args.input}", file=sys.stderr)
        return 2

    if args.output is None:
        stem, _ = os.path.splitext(args.input)
        args.output = f"{stem}-compressed.pdf"

    src_size = os.path.getsize(args.input)
    print(f"input : {args.input}  ({human_size(src_size)})")

    if args.target is not None:
        size, dd, qq = compress_to_target(
            args.input, args.output, args.dpi, args.quality,
            args.grayscale, args.min_dpi, args.target, args.verbose,
        )
        print(f"target: <= {human_size(args.target)} -> used dpi={dd} quality={qq}")
    else:
        changed, total = compress_once(
            args.input, args.output, args.dpi, args.quality,
            args.grayscale, args.min_dpi, args.verbose,
        )
        print(f"images: {changed} of {total} re-encoded")

    out_size = os.path.getsize(args.output)
    ratio = src_size / out_size if out_size else 0
    print(f"output: {args.output}  ({human_size(out_size)})")
    print(f"saved : {human_size(src_size - out_size)} ({ratio:.1f}x smaller)")
    return 0


command = Command("compress", "reduce a PDF's file size", add_arguments, run)
