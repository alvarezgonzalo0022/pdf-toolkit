# AGENTS.md

A small multi-command PDF toolkit (CLI) built on PyMuPDF. No tests, no CI,
not a git repo. Installable locally: `pip install -e .` exposes a
`pdf-toolkit` command on PATH (see `pyproject.toml`).

## Layout

    pdf_toolkit/
        cli.py            # argparse: builds subcommands from the registry
        command.py        # the Command contract (name / help / add_arguments / run)
        utils.py          # shared helpers (human_size, parse_size)
        commands/
            __init__.py   # registry: import + register() per command
            compress.py   # `compress` subcommand (downsample/re-encode images)
            merge.py      # `merge` subcommand (concatenate PDFs in order)
    pyproject.toml        # packaging + `pdf-toolkit` console entry point

Each command is a module in `pdf_toolkit/commands/` exposing a `command`
instance (`Command`). The registry in `commands/__init__.py` wires it into the
CLI; `cli.py` never needs to change to add a command.

## Run

    pip install -e .                                          # once; exposes `pdf-toolkit` on PATH

    pdf-toolkit                                               # list commands / help
    pdf-toolkit compress input.pdf                            # input-compressed.pdf (150dpi, quality 70)
    pdf-toolkit compress input.pdf -o out.pdf --dpi 200 --quality 60
    pdf-toolkit compress input.pdf --target 24mb              # iteratively lowers dpi/quality until < 24MB
    pdf-toolkit merge a.pdf b.pdf c.pdf                       # a-merged.pdf (pages in order)

Imports `pymupdf as fitz` — install `pymupdf`, not the old `fitz` package.
`python -m pdf_toolkit` also works without installing, from the project root.

`compress` default output is `<input-stem>-compressed.pdf`; it overwrites
without prompting. `merge` default output is `<first-stem>-merged.pdf` and
refuses to write if the output would overwrite one of its inputs.

## Adding a command

1. Create `pdf_toolkit/commands/<name>.py` exposing `command = Command(...)`.
2. Import and register it in `pdf_toolkit/commands/__init__.py`:

       from . import <name>
       register(<name>.command)

## Gotchas

- `--min-dpi` is parsed but never used — it's a no-op. Downsampling already
  skips images at or below `--dpi`.
- `recibo-de-sueldo-*.pdf` is a real personal payslip (name + salary data),
  not a test fixture. Don't commit or publish it; it's a throwaway input for
  manual checks.

## Verify

No automated tests. Manual check: run `python -m pdf_toolkit compress` on the
sample PDF and confirm the `output:` / `saved:` lines and that the result opens.
