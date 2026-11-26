# pdf-toolkit

Un toolkit de línea de comandos para manipular PDFs, construido sobre
[PyMuPDF](https://pymupdf.readthedocs.io/). Por ahora solo tiene el comando
`compress`, pero la arquitectura está pensada para ir sumando funciones
(`merge`, `split`, `rotate`, `encrypt`, etc.) sin tocar el CLI.

## Requisitos

- Python 3.10+

## Instalación

```bash
pip install -e .
```

Esto instala las dependencias (`pymupdf`, `pillow`) y deja disponible el comando
`pdf-toolkit` en el PATH, así podés usarlo desde cualquier carpeta. El modo
editable (`-e`) hace que los cambios en el código se reflejen al instante.

## Uso

```bash
pdf-toolkit                                                     # lista los comandos / help
pdf-toolkit compress input.pdf                                 # input-compressed.pdf (150 dpi, quality 70)
pdf-toolkit compress input.pdf -o out.pdf --dpi 200 --quality 60
pdf-toolkit compress input.pdf --target 24mb                   # baja dpi/quality iterativamente hasta < 24 MB
pdf-toolkit compress input.pdf --grayscale                     # convierte imágenes a escala de grises
pdf-toolkit merge a.pdf b.pdf c.pdf                            # a-merged.pdf (hojas en orden)
```

Si no querés instalarlo, podés correrlo igual con `python -m pdf_toolkit` desde
la raíz del proyecto.

```bash
python -m pdf_toolkit                                            # lista los comandos / help
python -m pdf_toolkit compress input.pdf                         # input-compressed.pdf (150 dpi, quality 70)
python -m pdf_toolkit compress input.pdf -o out.pdf --dpi 200 --quality 60
python -m pdf_toolkit compress input.pdf --target 24mb           # baja dpi/quality iterativamente hasta < 24 MB
python -m pdf_toolkit compress input.pdf --grayscale             # convierte imágenes a escala de grises
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
| `--min-dpi`   | Sin efecto por ahora (ver `AGENTS.md`).                                     |
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

## Estructura

```
pdf_toolkit/
├── cli.py            # argparse: construye los subcomandos desde el registry
├── command.py        # contrato Command (name / help / add_arguments / run)
├── utils.py          # helpers compartidos (human_size, parse_size)
└── commands/
    ├── __init__.py   # registry: import + register() por comando
    ├── compress.py   # subcomando `compress`
    └── merge.py      # subcomando `merge`
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
