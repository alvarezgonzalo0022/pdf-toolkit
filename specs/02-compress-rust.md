# SPEC 02 — Port de `compress` a Rust

> **Estado:** Implementado
> **Depende de:** —
> **Fecha:** 2026-11-26
> **Objetivo:** Crear un binario Rust `pdf-toolkit` con el subcomando `compress` que reemplace al `compress` de Python, con paridad funcional y `mupdf` como motor PDF.

## Why

Portar `compress` a Rust da un único binario sin runtime de Python ni dependencias de pip. Es el primer paso de un port completo; `merge` y la web se portan en specs futuros.

## Scope

**In:**

- Crate Cargo `pdf-toolkit` en la raíz del repo (`Cargo.toml`), binario `pdf-toolkit`.
- Subcomando `compress` con flags `-o/--output`, `--dpi` (default 150), `--quality` (default 70), `--min-dpi` (default 72, ahora funcional), `--grayscale`, `--target` (tamaño ej. `24mb`), `--verbose`.
- Motor `mupdf` (bindings) para leer/escribir el PDF y decodificar imágenes; crate `image` para resize Lanczos3 y encode JPEG.
- Downsampling y re-encode JPEG, flatten de alpha sobre blanco, grayscale, strip de metadata y garbage collection.
- Lógica de `--target` (presets descendentes + last-resort grayscale), idéntica a la de Python.
- Eliminar `pdf_toolkit/commands/compress.py` y su `register` en `pdf_toolkit/commands/__init__.py`.

**Out of scope (for future specs):**

- `merge` y la web: siguen en Python.
- Retiro completo del paquete Python y su entry point `pdf-toolkit`.
- Paridad byte-idéntica con el Python.
- Comandos nuevos (rotate, split, encrypt…).

## Data model

No hay datos persistentes; todo es en memoria. Estructuras nuevas:

```rust
// args del subcomando (clap derive)
struct CompressArgs {
    input: PathBuf,
    output: Option<PathBuf>,   // default <stem>-compressed.pdf
    dpi: u32,                  // 150
    quality: u8,               // 70
    min_dpi: u32,              // 72 (ahora funcional)
    grayscale: bool,
    target: Option<u64>,       // bytes, desde "24mb" / "1.5mb" / "900kb" / "1048576"
    verbose: bool,
}

// registro por imagen durante el pase
struct ImageRecord { xref: i32, width: u32, height: u32, effective_dpi: f64 }
```

Regla de decisión por imagen (reemplaza `process_images` de Python):

```
si effective_dpi <= min_dpi  -> dejar intacta (skip total)
scale = min(1.0, dpi / effective_dpi)
necesita = (scale < 1.0) || grayscale || tiene_alpha/smask
si !necesita -> skip
```

Presets de `--target` (igual que `_target_steps`): `[(dpi,85),(200,80),(150,75),(150,60),(120,55),(100,50),(80,45),(72,40)]`, con `min(d, dpi)`/`min(q, quality)` y dedupe; last-resort en grayscale al preset más agresivo.

## Implementation plan

1. Crear `Cargo.toml` en la raíz (`pdf-toolkit`) y `src/main.rs` con `clap` derive: subcomando `compress` como stub (acepta `input` y `--help`). Verificar: `cargo build` y `cargo run -- compress --help`.
2. Implementar `parse_size` y `human_size` (port de `utils.py`). Verificar con tests unitarios.
3. Implementar `compress_once`: abrir doc con `mupdf`, recolectar imágenes con `page.images()`, calcular `effective_dpi`, aplicar skip por `--min-dpi`, decodificar a Pixmap, flatten alpha, grayscale, resize Lanczos3, encode JPEG con `image`, `replace_image`, drop SMask, strip metadata, guardar con `save_to_buffer` + opciones de GC/deflate/clean. Verificar: `pdf-toolkit compress input.pdf` genera `input-compressed.pdf` y abre bien.
4. Implementar `--target`: loop de presets + last-resort grayscale. Verificar: `--target 1mb` produce un PDF ≤ 1 MB.
5. Conectar `--verbose` (mensajes por imagen) y los prints `input:` / `images:` / `output:` / `saved:`.
6. Eliminar `compress.py` y su `register`; verificar que el CLI Python ya no lista `compress` y que `pdf-toolkit merge` sigue funcionando.

## Acceptance criteria

- [ ] `cargo build` produce `pdf-toolkit` que lista `compress` en el help.
- [ ] `pdf-toolkit compress input.pdf` genera `input-compressed.pdf` (nombre default) y sobrescribe sin preguntar.
- [ ] `-o/--output`, `--dpi`, `--quality`, `--min-dpi`, `--grayscale`, `--target`, `--verbose` se aceptan y surten efecto.
- [ ] `--target 24mb` produce un PDF ≤ 24 MB, o aplica el last-resort grayscale.
- [ ] `--min-dpi` salta imágenes con DPI efectivo ≤ min-dpi (antes era un no-op).
- [ ] `--grayscale` convierte imágenes a escala de grises.
- [ ] El CLI Python ya no expone `compress`; `pdf-toolkit merge` sigue funcionando.
- [ ] Comprimir un PDF de muestra reduce el tamaño y el resultado abre sin errores.
- [ ] Exit code 2 si el input no existe; 0 en éxito.

## Decisions

- **Sí:** `mupdf` (bindings). Mismo motor que PyMuPDF; `replace_image`/`Pixmap` ya expuestos, minimiza el riesgo de port.
- **Sí:** crate `image` para Lanczos3 + JPEG. El `Pixmap` de `mupdf` no escribe JPEG.
- **Sí:** `clap` derive para el CLI.
- **Sí:** `effective_dpi` = resolución embebida (`Image::resolution()`, fallback 72). Divergencia conocida vs PyMuPDF (que usa rects de placement); se documenta.
- **Sí:** `--min-dpi` implementado de verdad: imágenes con DPI efectivo ≤ min-dpi quedan intactas.
- **No:** `lopdf`. Más trabajo manual (filtros/espacios de color) sin necesidad real aquí.
- **No:** paridad byte-idéntica. El encoder JPEG difiere; el objetivo es paridad funcional.
- **No:** retirar todo el Python en este spec. `merge` y la web siguen.

## Risks

| Riesgo | Mitigación |
| --- | --- |
| `mupdf` (y MuPDF) es **AGPL-3.0** | Para uso personal está bien; para distribución propietaria hace falta licencia comercial de MuPDF. Decisión de licencia a confirmar. |
| `effective_dpi` por resolución embebida difiere de PyMuPDF en placements escalados | Documentado; si se necesita paridad exacta, escanear content streams en un spec futuro. |
| Dos binarios `pdf-toolkit` durante la transición (Rust `compress` vs console-script Python `merge`) | Se unifica al portar `merge` y retirar el paquete Python. |
| Build de `mupdf-sys` en Windows (CMake/MSVC) | Documentar toolchain en README. |

## What is **not** in this spec

- Port de `merge` o de la web (siguen en Python).
- Retiro del paquete Python y su entry point.
- Paridad byte-idéntica.
- Comandos nuevos.

Cada una de esas cosas, si llega, va en su propia spec.
