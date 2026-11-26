# AGENTS.md

A small multi-command PDF toolkit. `compress` and `merge` are subcommands of a
single Rust binary (crate `pdf-toolkit`, MuPDF engine). The Flask web UI is
Python; both `/compress` and `/merge` delegate to the Rust binary. No CI. The
Python part is installable locally: `pip install -e .` exposes
`pdf-toolkit-web` on PATH (`flask` is its only dependency).

## Layout

    Cargo.toml            # Rust crate `pdf-toolkit` (bin `pdf-toolkit`)
    src/main.rs           # clap CLI + `compress` and `merge` subcommands
    vendor/mupdf/         # vendored mupdf crate (Windows/MSVC patch)

    pdf_toolkit/
        web/
            app.py        # Flask; /compress and /merge delegate to the Rust binary

    pyproject.toml        # Python packaging + console entry point

`compress` and `merge` live in `src/main.rs`.

## Run

Rust (`compress` / `merge`):

    cargo build --release                                     # produces target/release/pdf-toolkit.exe
    target\release\pdf-toolkit.exe compress input.pdf         # input-compressed.pdf (150dpi, quality 70)
    target\release\pdf-toolkit.exe compress input.pdf -o out.pdf --dpi 200 --quality 60
    target\release\pdf-toolkit.exe compress input.pdf --target 24mb
    target\release\pdf-toolkit.exe merge a.pdf b.pdf c.pdf    # a-merged.pdf (pages in order)

Web:

    pip install -e .                                          # exposes `pdf-toolkit-web`
    pdf-toolkit-web                                           # serves the UI on 127.0.0.1:5000

`compress` default output is `<input-stem>-compressed.pdf`; it overwrites
without prompting. `merge` default output is `<first-stem>-merged.pdf` and
refuses to write if the output would overwrite one of its inputs.

## Rust toolchain (Windows)

- Rust (rustup, `stable-x86_64-pc-windows-msvc`).
- Visual Studio 2022 Build Tools with the C++ workload
  (`Microsoft.VisualStudio.Workload.VCTools`) for `cl.exe`/`msbuild.exe` + SDK.
- LLVM for `libclang` (used by `mupdf-sys`'s bindgen step). Set `LIBCLANG_PATH`
  to LLVM's `bin`, e.g. `$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"`.

`mupdf` is vendored in `vendor/mupdf` with a one-line patch
(`device/native.rs`): on Windows/MSVC bindgen doesn't emit `max_align_t`, so it
is replaced with `usize` (same 8-byte alignment on x64). See
messense/mupdf-rs#222.

## Gotchas

- The web UI (`pdf-toolkit-web`) needs the Rust binary built. It resolves it via
  `PDF_TOOLKIT_BIN`, then `target/release`, `target/debug`, then PATH.
- `--min-dpi` is now functional (was a no-op in the Python version): images with
  effective DPI ≤ `--min-dpi` are left untouched. `effective_dpi` comes from the
  image's embedded resolution (`Image::resolution()`, fallback 72), not from
  placement rects like PyMuPDF — a documented divergence.
- `recibo-de-sueldo-*.pdf` is a real personal payslip (name + salary data),
  not a test fixture. Don't commit or publish it; it's a throwaway input for
  manual checks.

## Verify

- Rust unit tests: `cargo test` (covers `parse_size` / `human_size` /
  `default_output`).
- Manual: `cargo run -- compress` and `cargo run -- merge` on sample PDFs and
  confirm the printed lines and that the result opens.
