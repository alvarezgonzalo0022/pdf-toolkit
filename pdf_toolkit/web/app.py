"""Flask web app for `compress` and `merge`.

Endpoints accept multipart uploads and return the processed PDF as an
attachment. Both `compress` and `merge` delegate to the Rust `pdf-toolkit`
binary via a temporary directory.
"""

import argparse
import io
import os
import shutil
import subprocess
import tempfile

from flask import Flask, jsonify, request, send_file, send_from_directory

app = Flask(__name__)


def parse_size(text):
    """Convert '24mb' / '1.5mb' / '900kb' / '1048576' to bytes."""
    t = text.strip().lower()
    mult = 1
    for suffix, m in (("kb", 1024), ("mb", 1024 ** 2), ("gb", 1024 ** 3), ("b", 1)):
        if t.endswith(suffix):
            mult, t = m, t[: -len(suffix)]
            break
    try:
        return float(t) * mult
    except ValueError:
        raise argparse.ArgumentTypeError(f"invalid size: {text!r}")


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


def _find_binary():
    """Locate the Rust `pdf-toolkit` binary.

    Resolution order: the `PDF_TOOLKIT_BIN` env var, the `target/release` and
    `target/debug` builds relative to the project root, then `PATH`.
    """
    override = os.environ.get("PDF_TOOLKIT_BIN")
    if override:
        return override

    root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    exe = ".exe" if os.name == "nt" else ""
    for profile in ("release", "debug"):
        candidate = os.path.join(root, "target", profile, f"pdf-toolkit{exe}")
        if os.path.isfile(candidate):
            return candidate

    return shutil.which("pdf-toolkit")


def _run_rust(args):
    """Run the Rust `pdf-toolkit` binary with `args`.

    Returns the completed process, raising `RuntimeError` if the binary is
    missing or the process exits non-zero.
    """
    binary = _find_binary()
    if binary is None:
        raise RuntimeError(
            "el binario Rust `pdf-toolkit` no está disponible; "
            "compile con `cargo build --release`"
        )

    proc = subprocess.run([binary, *args], capture_output=True, text=True)
    if proc.returncode != 0:
        message = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(message or f"pdf-toolkit falló (exit {proc.returncode})")
    return proc


def _compress_rust(data, dpi, quality, grayscale, target):
    """Run the Rust `compress` binary and return the resulting PDF bytes."""
    with tempfile.TemporaryDirectory() as tmp:
        in_path = os.path.join(tmp, "input.pdf")
        out_path = os.path.join(tmp, "output.pdf")
        with open(in_path, "wb") as fh:
            fh.write(data)

        args = [
            "compress", in_path, "-o", out_path,
            "--dpi", str(dpi), "--quality", str(quality),
        ]
        if grayscale:
            args.append("--grayscale")
        if target is not None:
            args += ["--target", str(target)]

        _run_rust(args)

        with open(out_path, "rb") as fh:
            return fh.read()


def _merge_rust(data_files):
    """Run the Rust `merge` binary on the input bytes and return the PDF bytes."""
    with tempfile.TemporaryDirectory() as tmp:
        in_paths = []
        for i, data in enumerate(data_files):
            path = os.path.join(tmp, f"input-{i}.pdf")
            with open(path, "wb") as fh:
                fh.write(data)
            in_paths.append(path)

        out_path = os.path.join(tmp, "output.pdf")
        _run_rust(["merge", *in_paths, "-o", out_path])

        with open(out_path, "rb") as fh:
            return fh.read()


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
        out = _compress_rust(data, dpi, quality, grayscale, target)
    except Exception as exc:
        return jsonify({"error": str(exc)}), 400

    return _pdf_response(out, f"{_stem(file.filename)}-compressed.pdf")


@app.post("/merge")
def merge():
    files = [f for f in request.files.getlist("files") if f.filename != ""]
    if not files:
        return jsonify({"error": "no files uploaded"}), 400

    try:
        out = _merge_rust([f.read() for f in files])
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
