# AGENTS.md

A small multi-command PDF toolkit. `compress` is a Rust binary (crate
`pdf-toolkit`, MuPDF engine); `merge` and the Flask web UI remain Python
(PyMuPDF). No CI. The Python part is installable locally: `pip install -e .`
exposes a `pdf-toolkit` command (`merge`) and `pdf-toolkit-web` on PATH.

## Layout

    Cargo.toml            # Rust crate `pdf-toolkit` (bin `pdf-toolkit`)
    src/main.rs           # clap CLI + `compress` subcommand
    vendor/mupdf/         # vendored mupdf crate (Windows/MSVC patch)

    pdf_toolkit/
        cli.py            # argparse: builds subcommands from the registry
        command.py        # the Command contract (name / help / add_arguments / run)
        utils.py          # shared helpers (human_size, parse_size)
        commands/
            __init__.py   # registry: import + register() per command
            merge.py      # `merge` subcommand (concatenate PDFs in order)
        web/
            app.py        # Flask; /compress delegates to the Rust binary

    pyproject.toml        # Python packaging + console entry points

`compress` lives in `src/main.rs`. Python commands are modules in
`pdf_toolkit/commands/` exposing a `command` instance; the registry in
`commands/__init__.py` wires them into the CLI.

## Run

Python (`merge`):

    pip install -e .                                          # once; exposes `pdf-toolkit` + `pdf-toolkit-web`
    pdf-toolkit merge a.pdf b.pdf c.pdf                       # a-merged.pdf (pages in order)
    python -m pdf_toolkit                                     # also works from the project root

Rust (`compress`):

    cargo build --release                                     # produces target/release/pdf-toolkit.exe
    target\release\pdf-toolkit.exe compress input.pdf         # input-compressed.pdf (150dpi, quality 70)
    target\release\pdf-toolkit.exe compress input.pdf -o out.pdf --dpi 200 --quality 60
    target\release\pdf-toolkit.exe compress input.pdf --target 24mb

During the transition there are two `pdf-toolkit` binaries (the Rust one with
`compress`, the Python console-script with `merge`). They are unified once
`merge` is ported and the Python package is retired.

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

## Adding a command

(Python commands only.)

1. Create `pdf_toolkit/commands/<name>.py` exposing `command = Command(...)`.
2. Import and register it in `pdf_toolkit/commands/__init__.py`:

       from . import <name>
       register(<name>.command)

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

- Rust unit tests: `cargo test` (covers `parse_size` / `human_size`).
- Manual: `cargo run -- compress` on a sample PDF and confirm the
  `input:` / `images:` / `output:` / `saved:` lines and that the result opens.
