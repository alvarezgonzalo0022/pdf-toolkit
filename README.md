# pdf-toolkit

Un toolkit de línea de comandos para manipular PDFs.

- **`compress`** es un binario **Rust** (crate `pdf-toolkit`) construido sobre
  MuPDF. Reemplaza al `compress` de Python con paridad funcional.
- **`merge`** sigue en **Python** (PyMuPDF). La **web** (Flask) delega el
  `compress` al binario Rust y corre el `merge` en Python.

## Requisitos

- Para `merge` (Python): Python 3.10+ y `pip install -e .`.
- Para `compress` (Rust): ver [Rust (compress)](#rust-compress).

## Instalación

El paquete **Python** (para `merge` y la web):

```bash
pip install -e .
```

Esto instala las dependencias (`pymupdf`, `pillow`, `flask`) y deja disponibles
el comando `pdf-toolkit` (con `merge`) y `pdf-toolkit-web` en el PATH. El modo
editable (`-e`) refleja los cambios al instante.

El binario **Rust** de `compress` se compila aparte (ver
[Rust (compress)](#rust-compress)).

## Uso

El `compress` es el binario **Rust**; `merge` es el comando **Python**:

```bash
# compress (Rust) — ver "Rust (compress)" para el build
pdf-toolkit compress input.pdf                                 # input-compressed.pdf (150 dpi, quality 70)
pdf-toolkit compress input.pdf -o out.pdf --dpi 200 --quality 60
pdf-toolkit compress input.pdf --target 24mb                   # baja dpi/quality iterativamente hasta < 24 MB
pdf-toolkit compress input.pdf --grayscale                     # convierte imágenes a escala de grises

# merge (Python)
pdf-toolkit merge a.pdf b.pdf c.pdf                            # a-merged.pdf (hojas en orden)
```

Si no querés instalar el paquete Python, podés correr `merge` con
`python -m pdf_toolkit` desde la raíz del proyecto:

```bash
python -m pdf_toolkit                                            # lista los comandos / help
python -m pdf_toolkit merge a.pdf b.pdf c.pdf                    # a-merged.pdf (hojas en orden)
```

### Comando `compress`

Reduce el tamaño de un PDF:

- **downsampling** de imágenes incrustadas a un DPI objetivo,
- **re-encodificación** a JPEG con calidad configurable,
- **aplanado de transparencia** (elimina alpha/soft-masks redundantes),
- opcionalmente conversión a **escala de grises**,
- **limpieza** de metadatos y objetos sin uso.

| Flag          | Descripción                                                                 |
| ------------- | --------------------------------------------------------------------------- |
| `-o, --output`| Archivo de salida (por defecto `<nombre>-compressed.pdf`).                   |
| `--dpi`       | Resolución objetivo de las imágenes (default `150`).                         |
| `--quality`   | Calidad JPEG, 1–100 (default `70`).                                         |
| `--min-dpi`   | Salta imágenes cuyo DPI efectivo ≤ `--min-dpi` (default `72`).           |
| `--grayscale` | Convierte las imágenes a escala de grises.                                   |
| `--target`    | Tamaño máximo objetivo (p. ej. `24mb`); ajusta dpi/quality hasta lograrlo.   |
| `--verbose`   | Imprime el detalle por imagen.                                              |

### Comando `merge`

Pega varios PDFs en uno solo, en el orden en que se pasan:

- copia **todas las páginas** de cada entrada con `insert_pdf` (preserva el orden),
- falla con un error claro si algún input no existe o está **cifrado**,
- se niega a escribir si el output apuntaría a uno de los inputs.

| Flag          | Descripción                                                                 |
| ------------- | --------------------------------------------------------------------------- |
| `inputs`      | PDFs a pegar, en orden (posicional, uno o más).                              |
| `-o, --output`| Archivo de salida (por defecto `<stem-del-primero>-merged.pdf`).             |
| `--verbose`   | Imprime el detalle por archivo.                                             |

## Rust (compress)

El subcomando `compress` es un binario Rust con el crate `mupdf` (motor MuPDF).
No usa Python ni depende de pip.

### Build

```bash
cargo build --release      # produce target/release/pdf-toolkit.exe
```

Requisitos del toolchain (probado en Windows x64):

- **Rust** (rustup, toolchain `stable-x86_64-pc-windows-msvc`).
- **Visual Studio 2022 Build Tools** con el workload *Desktop development with
  C++* (`Microsoft.VisualStudio.Workload.VCTools`): aporta `cl.exe`, `msbuild.exe`
  y el Windows SDK.
- **LLVM** (provee `libclang.dll`/`clang`, que `mupdf-sys` usa en el paso de
  `bindgen`). Asegurate de que `LIBCLANG_PATH` apunte a `bin/` de LLVM, p. ej.:
  `$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"`.

`mupdf` está vendoreado en `vendor/mupdf` con un patch de una línea
(`device/native.rs`): en Windows/MSVC `bindgen` no emite el tipo `max_align_t`
(solo lo define MSVC en modo C++), así que se reemplaza por `usize` (misma
alineación de 8 bytes en x64). Ver [messense/mupdf-rs#222](https://github.com/messense/mupdf-rs/issues/222).

### Uso

```bash
target\release\pdf-toolkit.exe compress input.pdf                 # input-compressed.pdf
target\release\pdf-toolkit.exe compress input.pdf --target 24mb
target\release\pdf-toolkit.exe compress input.pdf --grayscale
```

Nota: durante la transición conviven dos binarios `pdf-toolkit` (el Rust con
`compress` y el console-script de Python con `merge`). Se unifican al portar
`merge` y retirar el paquete Python.

## Estructura

```
# Rust (compress)
Cargo.toml
src/main.rs              # CLI clap + subcomando `compress`
vendor/mupdf/            # crate mupdf vendoreado (patch Windows/MSVC)

# Python (merge + web)
pdf_toolkit/
├── cli.py            # argparse: construye los subcomandos desde el registry
├── command.py        # contrato Command (name / help / add_arguments / run)
├── utils.py          # helpers compartidos (human_size, parse_size)
├── commands/
│   ├── __init__.py   # registry: import + register() por comando
│   └── merge.py      # subcomando `merge`
└── web/
    └── app.py        # Flask; /compress delega al binario Rust
```

## Añadir un comando

1. Crea `pdf_toolkit/commands/<nombre>.py` exponiendo `command = Command(...)`.
2. Impórtalo y regístralo en `pdf_toolkit/commands/__init__.py`:

```python
from . import <nombre>
register(<nombre>.command)
```

`cli.py` se encarga del resto automáticamente.

## Roadmap

Ideas para futuros comandos: `split`, `rotate`, `extract-text`,
`images`, `encrypt`, `watermark`, `info`.
