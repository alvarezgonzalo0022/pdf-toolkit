# pdf-toolkit

Un toolkit de línea de comandos para manipular PDFs.

- **`compress`** y **`merge`** son subcomandos de un único binario **Rust**
  (crate `pdf-toolkit`) construido sobre MuPDF.
- La **web** (Flask) delega ambos comandos al binario Rust.

## Requisitos

- Binario (`compress` y `merge`): Rust + MuPDF. Ver [Rust](#rust).
- Web: Python 3.10+ y `pip install -e .` (la única dependencia es `flask`).

## Instalación

El paquete **Python** (solo la web):

```bash
pip install -e .
```

Esto instala `flask` y deja disponible el comando `pdf-toolkit-web` en el PATH.
El modo editable (`-e`) refleja los cambios al instante.

El binario **Rust** se compila aparte (ver [Rust](#rust)).

## Web

La web necesita el binario Rust compilado (ver [Build](#build)) y el paquete
instalado. Para levantarla:

```bash
pip install -e .        # una vez; deja `pdf-toolkit-web` en el PATH
pdf-toolkit-web         # sirve la interfaz en http://127.0.0.1:5000
```

Desde ahí se suben PDFs para `compress` y `merge`. Opciones útiles:

```bash
pdf-toolkit-web --host 0.0.0.0 --port 8000    # cambia host/puerto
```

La web resuelve el binario en este orden: la variable de entorno
`PDF_TOOLKIT_BIN`, `target/release/pdf-toolkit(.exe)`,
`target/debug/pdf-toolkit(.exe)`, y por último `pdf-toolkit` en el PATH. Para
apuntar a un binario concreto (PowerShell):

```powershell
$env:PDF_TOOLKIT_BIN = "C:\ruta\a\pdf-toolkit.exe"
pdf-toolkit-web
```

## Uso

```bash
# compress (Rust) — ver "Rust" para el build
pdf-toolkit compress input.pdf                                 # input-compressed.pdf (150 dpi, quality 70)
pdf-toolkit compress input.pdf -o out.pdf --dpi 200 --quality 60
pdf-toolkit compress input.pdf --target 24mb                   # baja dpi/quality iterativamente hasta < 24 MB
pdf-toolkit compress input.pdf --grayscale                     # convierte imágenes a escala de grises

# merge (Rust)
pdf-toolkit merge a.pdf b.pdf c.pdf                            # a-merged.pdf (hojas en orden)
pdf-toolkit merge a.pdf b.pdf -o out.pdf --verbose
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

## Rust

Los subcomandos `compress` y `merge` viven en un único binario Rust con el crate
`mupdf` (motor MuPDF). No usan Python ni dependen de pip.

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
target\release\pdf-toolkit.exe compress input.pdf                # input-compressed.pdf
target\release\pdf-toolkit.exe compress input.pdf --target 24mb
target\release\pdf-toolkit.exe compress input.pdf --grayscale
target\release\pdf-toolkit.exe merge a.pdf b.pdf c.pdf           # a-merged.pdf
```

Nota sobre el retiro del Python: si antes instalaste el paquete Python, puede
quedar un console-script `pdf-toolkit` viejo en el PATH que gane por orden de
resolución al binario Rust. Corré `pip uninstall pdf-toolkit` (o reinstalá
limpio) para eliminarlo.

## Estructura

```
# Rust (compress + merge)
Cargo.toml
src/lib.rs               # módulos + `pub type Result<T>`
src/main.rs              # binario: parsea `Cli` y despacha a `run_compress`/`run_merge`
src/cli.rs               # structs clap (`Cli`, `Command`, `CompressArgs`, `MergeArgs`)
src/utils.rs             # `human_size`, `parse_size`, `default_output`
src/image.rs             # helpers de imagen (privado, solo lo usa `compress`)
src/compress.rs          # pipeline de `compress` + `run_compress`
src/merge.rs             # lógica de `merge` + `run_merge`
tests/utils.rs           # tests de integración (`parse_size` / `human_size` / `default_output`)
vendor/mupdf/            # crate mupdf vendoreado (patch Windows/MSVC)

# Python (web)
pdf_toolkit/
└── web/
    └── app.py        # Flask; /compress y /merge delegan al binario Rust
```

## Roadmap

Ideas para futuros comandos: `split`, `rotate`, `extract-text`,
`images`, `encrypt`, `watermark`, `info`.
