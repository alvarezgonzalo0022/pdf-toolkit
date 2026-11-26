# SPEC 03 — Port de `merge` a Rust

> **Estado:** Implementado
> **Depende de:** SPEC 01, SPEC 02
> **Fecha:** 2026-11-26
> **Objetivo:** Portar el subcomando `merge` al binario Rust con paridad funcional, delegar `/merge` de la web al binario y retirar el CLI Python dejando solo la web Flask.

## Why

`merge` es el último comando que queda en Python. Portarlo completa la migración a Rust, unifica el binario `pdf-toolkit` (desaparece la convivencia de dos binarios del mismo nombre) y permite retirar `pymupdf`/`pillow` de las dependencias de la web.

## Scope

**In:**

- Subcomando `merge` en `src/main.rs` (crate `pdf-toolkit`) con posicionales `inputs` (uno o más, en orden), `-o/--output` y `--verbose`.
- Concatenación con `mupdf`: `PdfDocument::new()` + `insert_pdf(src, InsertPdfOptions::default())` por cada input, y serialización con `write_to_with_options` (`garbage=4`, `compress=true`).
- Paridad funcional con el Python: default de salida `<stem-del-primero>-merged.pdf`, rechazo si el output apuntaría a un input, exit 2 en input inexistente / cifrado / overwrite, exit 0 en éxito, y los prints `inputs:` / `+ <path> (<pages> pages)` (verbose) / `output:`.
- Detección de cifrado con `needs_password()` (equivale a `needs_pass` de PyMuPDF).
- Web: reemplazar `merge_bytes` en `/merge` por delegación al binario Rust vía directorio temporal (mismo patrón que `/compress`).
- Retirar el CLI Python: eliminar `cli.py`, `command.py`, `commands/`, `utils.py`, el entry point `pdf-toolkit` de `pyproject.toml` y las dependencias `pymupdf`/`pillow`. Queda `pdf-toolkit-web` (Flask) con `flask` como única dependencia.
- Mover `parse_size` al interior de `pdf_toolkit/web/app.py` (el único consumidor restante, para `--target` de `/compress`).
- Actualizar `README.md` y `AGENTS.md`.

**Out of scope (for future specs):**

- Port de la web a Rust (sigue en Flask).
- Comandos nuevos (`rotate`, `split`, `encrypt`…).
- Paridad byte-idéntica con el Python.
- Delegación en memoria (stdin/stdout) para `merge`; se usa directorio temporal como en `/compress`.
- Autenticación, multiusuario o despliegue público.

## Data model

No hay datos persistentes; todo es en memoria o en un `tempfile.TemporaryDirectory` descartable. Estructuras nuevas en Rust:

```rust
// subcomando (clap derive)
struct MergeArgs {
    inputs: Vec<PathBuf>,       // uno o más, en orden
    output: Option<PathBuf>,    // default <first-stem>-merged.pdf
    verbose: bool,
}
```

Contrato del CLI `merge` (paridad con el Python):

```
pdf-toolkit merge a.pdf b.pdf c.pdf           # -> a-merged.pdf
pdf-toolkit merge a.pdf -o out.pdf --verbose
```

Mensajes y códigos de salida (idénticos al Python):

- `error: input not found: <path>` → exit 2.
- `error: output would overwrite an input: <output>` → exit 2.
- `error: encrypted PDF (password required): <path>` → exit 2.
- `inputs: <N> files`, `  + <path> (<pages> pages)` (con `--verbose`), `output: <path>  (<size>, <N> pages)` → exit 0.

Contrato HTTP de la web: sin cambios. `POST /merge` sigue recibiendo el campo `files` (múltiples, en orden) y responde `200` con el PDF adjunto (`<nombre-del-primero>-merged.pdf`) o `400` con `{"error": "<mensaje>"}`.

## Implementation plan

1. Añadir la variante `Merge(MergeArgs)` al enum `Command` en `src/main.rs`, con `MergeArgs` y un `run_merge` stub que valide inputs, imprima `inputs: <N> files` y salga 0. Verificar: `cargo build` y `cargo run -- merge --help` listan los flags.
2. Implementar `merge_docs`: abrir cada input con `PdfDocument::from_bytes`, comprobar `needs_password()` (error cifrado), y `insert_pdf(src, InsertPdfOptions::default())` sobre un `PdfDocument::new()`, luego `write_to_with_options`. Implementar `run_merge` con I/O de archivos, default de salida, rechazo de overwrite y prints de paridad. Verificar: `cargo run -- merge a.pdf b.pdf` genera `a-merged.pdf` con las páginas en orden y abre bien.
3. Conectar `--verbose` (líneas `  + <path> (<pages> pages)`) y el `output:` final con `human_size`. Verificar paridad de mensajes contra el Python.
4. Añadir test unitario de `default_output` y del nombre default `<first-stem>-merged.pdf`. Verificar: `cargo test` sigue en verde.
5. Refactorizar la web: extraer un helper genérico `_run_rust(args)` de `_compress_rust`; añadir `_merge_rust(data_files)` que escribe cada input a un `TemporaryDirectory` e invoca `pdf-toolkit merge <in1> … -o out.pdf`; cambiar `/merge` para usarlo. Mover `parse_size` a `app.py` y quitar `from ..commands.merge import merge_bytes` y `from ..utils import parse_size`. Verificar: `pdf-toolkit-web` sirve la página y `/compress` y `/merge` funcionan desde el navegador.
6. Retirar el CLI Python: borrar `cli.py`, `command.py`, `commands/`, `utils.py`; editar `pyproject.toml` (quitar el entry point `pdf-toolkit`, las deps `pymupdf`/`pillow` y el paquete `pdf_toolkit.commands` de `packages`). Verificar: `pip install -e .` deja `pdf-toolkit-web` y ya no deja `pdf-toolkit` de Python.
7. Actualizar `README.md` y `AGENTS.md`: `merge` como subcomando Rust, web delegando ambos comandos al binario, y eliminación de las secciones de `merge`/CLI Python.

## Acceptance criteria

- [ ] `cargo build` produce `pdf-toolkit` que lista `compress` y `merge` en el help.
- [ ] `pdf-toolkit merge a.pdf b.pdf` genera `a-merged.pdf` (nombre default) con las páginas en orden y abre sin errores.
- [ ] `-o/--output` y `--verbose` se aceptan y surten efecto.
- [ ] Exit 2 si un input no existe; exit 2 si el output sobrescribiría un input; exit 0 en éxito.
- [ ] Un PDF cifrado produce `error: encrypted PDF (password required): <path>` y exit 2.
- [ ] `pdf-toolkit-web` sirve la página y `/merge` delega al binario Rust (descarga `<first-stem>-merged.pdf`), igual que `/compress`.
- [ ] `pip install -e .` deja disponible `pdf-toolkit-web` y ya no deja `pdf-toolkit` de Python ni `python -m pdf_toolkit merge`.
- [ ] El paquete Python ya no depende de `pymupdf`/`pillow`; solo `flask`.
- [ ] `README.md` y `AGENTS.md` reflejan `merge` en Rust y la web delegando ambos comandos.

## Decisions

- **Sí:** `insert_pdf` de `mupdf` (mismo motor que PyMuPDF, con `copy_links`/`copy_annotations`/`copy_widgets` por defecto; minimiza el riesgo de port).
- **Sí:** `needs_password()` para detectar cifrado (equivale a `needs_pass` de PyMuPDF).
- **Sí:** delegación de `/merge` vía `TemporaryDirectory`, igual que `/compress`. Consistente con el patrón existente; la propiedad "nada en disco" ya se relajó en SPEC 02.
- **Sí:** retirar el CLI Python completo (no solo `merge.py`): sin comandos, el paquete CLI no tiene razón de ser y `pymupdf`/`pillow` dejan de ser necesarios.
- **No:** port de la web a Rust. Sigue en Flask; solo cambia cómo resuelve `merge`.
- **No:** stdin/stdout en memoria para `merge`. Más código y flags nuevos sin beneficio real frente al temp dir.
- **No:** paridad byte-idéntica. Mismo criterio que SPEC 02: paridad funcional, no bytes idénticos.

## Risks

| Riesgo | Mitigación |
| --- | --- |
| `insert_pdf` puede diferir de PyMuPDF en metadatos/estructura de salida (links, outlines). | Paridad funcional, no byte-idéntica; verificar que las páginas y el orden coinciden. |
| El console-script Python `pdf-toolkit` puede quedar en PATH y ganar por orden de resolución al binario Rust. | Documentar `pip uninstall pdf-toolkit` (o reinstalar limpio) en README tras el retiro. |
| MuPDF es **AGPL-3.0** (riesgo heredado de SPEC 02). | Para uso personal está bien; distribución propietaria requiere licencia comercial de MuPDF. |

## What is **not** in this spec

- Port de la web a Rust.
- Comandos nuevos (`rotate`, `split`, `encrypt`…).
- Paridad byte-idéntica con el Python.
- Delegación en memoria (stdin/stdout).

Cada una de esas cosas, si llega, va en su propia spec.
