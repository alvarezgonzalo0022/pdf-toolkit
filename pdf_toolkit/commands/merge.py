"""`merge` — concatenate multiple PDFs into one.

It joins PDFs in the order given, producing a single document whose pages are
the pages of each input, back to back. `Document.insert_pdf` copies whole
documents, so page order is preserved exactly.
"""

import io
import os
import sys

import pymupdf as fitz

from ..command import Command
from ..utils import human_size


def _append_pdf(out, data, label):
    """Open one PDF from `data`, insert it into `out`, and return its page count.

    `label` identifies the source in error messages (a path or "file #N").
    """
    src = fitz.open(stream=io.BytesIO(data), filetype="pdf")
    try:
        if src.needs_pass:
            raise ValueError(f"encrypted PDF (password required): {label}")
        pages = src.page_count
        out.insert_pdf(src)
        return pages
    finally:
        src.close()


def merge_bytes(pdfs):
    """Concatenate `pdfs` (list of bytes, in order) and return the bytes."""
    out = fitz.open()
    try:
        for i, data in enumerate(pdfs):
            _append_pdf(out, data, f"file #{i + 1}")
        return out.tobytes(garbage=4, deflate=True)
    finally:
        out.close()


def merge_pdfs(inputs, output, verbose=False):
    """CLI wrapper: concatenate `inputs` (in order) into `output`.

    Returns total page count.
    """
    pdfs = [open(path, "rb").read() for path in inputs]

    out = fitz.open()
    try:
        total = 0
        for path, data in zip(inputs, pdfs):
            pages = _append_pdf(out, data, path)
            if verbose:
                print(f"  + {path} ({pages} pages)")
            total += pages
        data_out = out.tobytes(garbage=4, deflate=True)
    finally:
        out.close()

    with open(output, "wb") as f:
        f.write(data_out)
    return total


# --------------------------------------------------------------------------- #
# Command definition
# --------------------------------------------------------------------------- #
def add_arguments(parser):
    parser.add_argument("inputs", nargs="+", help="PDF files to merge, in order")
    parser.add_argument(
        "-o", "--output", help="output PDF file (default: <first-stem>-merged.pdf)"
    )
    parser.add_argument("--verbose", action="store_true", help="print per-file details")


def run(args):
    for path in args.inputs:
        if not os.path.isfile(path):
            print(f"error: input not found: {path}", file=sys.stderr)
            return 2

    if args.output is None:
        stem, _ = os.path.splitext(args.inputs[0])
        args.output = f"{stem}-merged.pdf"

    out_abs = os.path.abspath(args.output)
    for path in args.inputs:
        if os.path.abspath(path) == out_abs:
            print(
                f"error: output would overwrite an input: {args.output}",
                file=sys.stderr,
            )
            return 2

    print(f"inputs: {len(args.inputs)} files")

    try:
        total = merge_pdfs(args.inputs, args.output, args.verbose)
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    print(f"output: {args.output}  ({human_size(os.path.getsize(args.output))}, {total} pages)")
    return 0


command = Command("merge", "concatenate multiple PDFs into one", add_arguments, run)
