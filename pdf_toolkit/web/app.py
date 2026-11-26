"""Flask web app that runs `compress` and `merge` entirely in memory.

Endpoints accept multipart uploads and return the processed PDF as an
attachment. Nothing is written to disk.
"""

import argparse
import io
import os

from flask import Flask, jsonify, request, send_file, send_from_directory

from ..commands.compress import compress_bytes, compress_bytes_to_target
from ..commands.merge import merge_bytes
from ..utils import parse_size

app = Flask(__name__)


# --------------------------------------------------------------------------- #
# Form helpers
# --------------------------------------------------------------------------- #
def _int_field(name, default, minimum=None, maximum=None):
    """Parse an integer form field, falling back to `default` when empty."""
    raw = request.form.get(name)
    if raw is None or raw == "":
        return default
    try:
        value = int(raw)
    except ValueError:
        raise ValueError(f"invalid {name}: {raw!r}")
    if minimum is not None and value < minimum:
        raise ValueError(f"{name} must be >= {minimum}")
    if maximum is not None and value > maximum:
        raise ValueError(f"{name} must be <= {maximum}")
    return value


def _bool_field(name):
    raw = request.form.get(name)
    return raw is not None and raw.lower() in ("1", "true", "on", "yes", "y")


def _target_field():
    """Parse the optional target size (accepts bytes or "1mb"-style strings)."""
    raw = request.form.get("target")
    if raw is None or raw.strip() == "":
        return None
    try:
        return int(parse_size(raw))
    except (ValueError, argparse.ArgumentTypeError):
        raise ValueError(f"invalid target: {raw!r}")


def _stem(filename):
    base = os.path.basename(filename or "")
    stem, _ = os.path.splitext(base)
    return stem or "document"


def _pdf_response(data, filename):
    return send_file(
        io.BytesIO(data),
        mimetype="application/pdf",
        as_attachment=True,
        download_name=filename,
    )


# --------------------------------------------------------------------------- #
# Routes
# --------------------------------------------------------------------------- #
@app.get("/")
def index():
    return send_from_directory(app.static_folder, "index.html")


@app.post("/compress")
def compress():
    file = request.files.get("file")
    if file is None or file.filename == "":
        return jsonify({"error": "no file uploaded"}), 400

    try:
        dpi = _int_field("dpi", 150, minimum=1)
        quality = _int_field("quality", 70, minimum=1, maximum=100)
        grayscale = _bool_field("grayscale")
        target = _target_field()
    except ValueError as exc:
        return jsonify({"error": str(exc)}), 400

    data = file.read()
    try:
        if target is not None:
            out, _, _ = compress_bytes_to_target(data, dpi, quality, grayscale, target)
        else:
            out = compress_bytes(data, dpi, quality, grayscale)
    except Exception as exc:
        return jsonify({"error": str(exc)}), 400

    return _pdf_response(out, f"{_stem(file.filename)}-compressed.pdf")


@app.post("/merge")
def merge():
    files = [f for f in request.files.getlist("files") if f.filename != ""]
    if not files:
        return jsonify({"error": "no files uploaded"}), 400

    try:
        out = merge_bytes([f.read() for f in files])
    except Exception as exc:
        return jsonify({"error": str(exc)}), 400

    return _pdf_response(out, f"{_stem(files[0].filename)}-merged.pdf")


# --------------------------------------------------------------------------- #
# Entry point
# --------------------------------------------------------------------------- #
def main(argv=None):
    parser = argparse.ArgumentParser(
        prog="pdf-toolkit-web",
        description="Serve the pdf-toolkit web interface.",
    )
    parser.add_argument("--host", default="127.0.0.1", help="interface to bind to")
    parser.add_argument("--port", type=int, default=5000, help="port to listen on")
    args = parser.parse_args(argv)
    app.run(host=args.host, port=args.port)


if __name__ == "__main__":
    main()
