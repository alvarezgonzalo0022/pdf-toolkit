# SPEC 04 — Modularizar el binario Rust

> **Estado:** Aprobado
> **Depende de:** SPEC 02, SPEC 03
> **Fecha:** 2026-11-26
> **Objetivo:** Reorganizar `src/main.rs` (hoy todo el código del binario) en una biblioteca con módulos separados (`cli`, `utils`, `image`, `compress`, `merge`) y tests de integración, sin cambiar el comportamiento.

## Why

`src/main.rs` concentra en un solo archivo el CLI, los helpers de imagen, la lógica de `compress`, la de `merge` y los tests. Separar responsabilidades en módulos hace el código navegable, testable desde fuera (vía `lib.rs`) y prepara el terreno para reusar la lógica sin depender de un binario.

## Scope

**In:**

- Crear `src/lib.rs` con las declaraciones de módulos y el alias `pub type Result<T>`.
- Crear `src/cli.rs` (structs clap derive `Cli`, `Command`, `CompressArgs`, `MergeArgs`).
- Crear `src/utils.rs` (`human_size`, `parse_size`, `default_output`).
- Crear `src/image.rs` (helpers de imagen), módulo privado consumido solo por `compress`.
- Crear `src/compress.rs` (pipeline de `compress` completo, incluido `run_compress`).
- Crear `src/merge.rs` (lógica de `merge`, incluido `run_merge`).
- Reducir `src/main.rs` a `main()` + `use` (sin lógica de negocio).
- Mover los tests unitarios actuales a `tests/utils.rs` (tests de integración).
- Actualizar `README.md` (sección *Estructura*) y `AGENTS.md` (sección *Layout*).

**Out of scope (for future specs):**

- Comandos nuevos (`split`, `rotate`, `encrypt`…).
- Cambios de comportamiento: flags, defaults, mensajes, exit codes y salidas quedan idénticos.
- Cambios en la web (Flask) o en su delegación por subproceso.
- Exponer la lógica de `compress`/`merge` como API de biblioteca para que la web la consuma en proceso (sigue delegando al binario).
- Cambios en `Cargo.toml` (la biblioteca se auto-detecta por `src/lib.rs`).

## Data model

No hay datos persistentes ni estructuras nuevas; solo se reorganiza el código existente. Disposición de módulos y visibilidad:

```rust
// src/lib.rs
pub mod cli;
pub mod compress;
pub mod merge;
pub mod utils;
mod image; // privado, solo lo usa compress

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
```

Contenido y visibilidad por archivo:

| Archivo | Items | Visibilidad |
| --- | --- | --- |
| `src/cli.rs` | `Cli`, `Command` (variantes `Compress`/`Merge`), `CompressArgs`, `MergeArgs` | `pub` (los usa el bin `main.rs`) |
| `src/utils.rs` | `human_size`, `parse_size`, `default_output` | `pub` (los testean los tests de integración y los usan `cli`/`compress`/`merge`) |
| `src/image.rs` | `cmyk_to_rgb`, `pixmap_to_rgb_and_alpha`, `flatten_on_white`, `smask_xref`, `decode_alpha` | `pub(crate)` (solo `compress`) |
| `src/compress.rs` | `run_compress` (`pub`); `reencode_image`, `process_images`, `strip_metadata`, `compress_doc`, `compress_once`, `target_steps`, `compress_to_target` (privados) | `run_compress` es `pub` (lo llama `main.rs`) |
| `src/merge.rs` | `run_merge` (`pub`); `merge_docs`, `abspath`, `EncryptedPdfError` (privados) | `run_merge` es `pub` (lo llama `main.rs`) |
| `src/main.rs` | `fn main() -> ExitCode` | bin; parsea `Cli` y despacha a `run_compress`/`run_merge` |

El alias `Result<T>` es `pub` en `lib.rs` porque `run_compress` y `run_merge` son `pub` y devuelven `Result<u8>`; un alias `pub(crate)` en una firma pública generaría un aviso de tipo privado en interfaz pública.

## Implementation plan

1. Crear `src/lib.rs` (`pub mod utils;` + `pub type Result<T>`) y `src/utils.rs` con `human_size`, `parse_size` y `default_output` como `pub`. En `src/main.rs`, borrar esas tres funciones y añadir `use pdf_toolkit::utils::{human_size, parse_size, default_output};`. Crear `tests/utils.rs` con los tres tests portados tal cual (`test_parse_size`, `test_human_size`, `test_default_output`) y eliminar el `#[cfg(test)] mod tests` de `main.rs`. Verificar: `cargo test` (tests de integración verdes) y `cargo build` (bin compila usando la lib).
2. Crear `src/cli.rs` con `Cli`, `Command`, `CompressArgs` y `MergeArgs` como `pub`, usando `crate::utils::parse_size` en el `value_parser` de `--target`. Añadir `pub mod cli;` a `lib.rs`. En `main.rs`, borrar las definiciones clap y añadir `use pdf_toolkit::cli::{Cli, Command};`. Verificar: `cargo run -- --help`, `compress --help` y `merge --help` muestran los mismos flags y descripciones.
3. Crear `src/image.rs` (`mod image;` privado) y `src/compress.rs`. Mover a `compress.rs` todo el pipeline (`reencode_image`, `process_images`, `strip_metadata`, `compress_doc`, `compress_once`, `target_steps`, `compress_to_target`, `run_compress`) y a `image.rs` los helpers que usa. En `main.rs`, borrar ese código y despachar `Command::Compress` a `pdf_toolkit::compress::run_compress`. Verificar: `cargo run -- compress input.pdf` produce la misma salida y los mismos `println!`.
4. Crear `src/merge.rs` con `EncryptedPdfError`, `merge_docs`, `abspath` y `run_merge`. En `main.rs`, borrar el código de merge y despachar `Command::Merge` a `pdf_toolkit::merge::run_merge`. Verificar: `cargo run -- merge a.pdf b.pdf` produce `a-merged.pdf` con los mismos prints y exit codes (incluidos input inexistente, cifrado y overwrite).
5. Actualizar `README.md` (sección *Estructura*) y `AGENTS.md` (sección *Layout*) para reflejar `src/lib.rs` + módulos. Verificar que la documentación coincide con el árbol real.

## Acceptance criteria

- [ ] `cargo build` compila sin warnings y produce `pdf-toolkit`.
- [ ] `cargo test` pasa: los tres tests de integración (`parse_size`, `human_size`, `default_output`) en `tests/utils.rs` están verdes.
- [ ] `pdf-toolkit --help`, `pdf-toolkit compress --help` y `pdf-toolkit merge --help` muestran exactamente los mismos flags y descripciones que antes del refactor.
- [ ] `pdf-toolkit compress input.pdf` genera `input-compressed.pdf` con los mismos prints (`input:`/`images:`/`output:`/`saved:`) y sobrescribe sin preguntar.
- [ ] `pdf-toolkit compress input.pdf --target 24mb` produce un PDF ≤ 24 MB (mismo comportamiento).
- [ ] `pdf-toolkit merge a.pdf b.pdf` genera `a-merged.pdf` con los mismos prints y exit codes (exit 2 en input inexistente / cifrado / overwrite, exit 0 en éxito).
- [ ] `src/main.rs` queda reducido a `main()` + `use`, sin lógica de negocio.
- [ ] `README.md` (Estructura) y `AGENTS.md` (Layout) reflejan `src/lib.rs` + los cinco módulos.

## Decisions

- **Sí:** `lib.rs` + cinco módulos (`cli`, `utils`, `image`, `compress`, `merge`). Refleja las secciones que ya separa `main.rs` con comentarios; `main.rs` queda solo como binario.
- **Sí:** tests de integración en `tests/utils.rs`. Obliga a que `utils` sea `pub`, lo que deja una superficie pública mínima y testable.
- **Sí:** `image` como módulo privado (`mod image`). Solo lo consume `compress`; no hay razón para exponerlo.
- **Sí:** `pub type Result<T>` en `lib.rs`. Necesario porque `run_compress`/`run_merge` son `pub` y devuelven `Result<u8>`.
- **Sí:** refactor puro. Cero cambio de comportamiento: los mismos flags, defaults, mensajes y exit codes.
- **No:** exponer más API pública que la mínima (solo `run_compress`, `run_merge` y `utils`). El resto queda privado o `pub(crate)`.
- **No:** tocar `Cargo.toml`. El `[[bin]]` explícito se mantiene y `src/lib.rs` se auto-detecta como biblioteca del paquete.
- **No:** hacer que la web consuma la biblioteca en proceso. Sigue delegando al binario por subproceso.

## Risks

| Riesgo | Mitigación |
| --- | --- |
| Avisos de *dead code* en pasos intermedios al mover funciones antes de conectarlas. | Ordenar el plan de hojas a raíz (`utils` → `cli` → `image`/`compress` → `merge`) y borrar de `main.rs` en el mismo paso que se mueve. |
| Deriva de comportamiento por mover código sin re-verificar. | Acceptance criteria con smoke de `compress` y `merge` y paridad de mensajes/exit codes. |

## What is **not** in this spec

- Comandos nuevos (`split`, `rotate`, `encrypt`…).
- Cambios de comportamiento (flags, defaults, mensajes, exit codes).
- Cambios en la web (Flask) o en su delegación por subproceso.
- API de biblioteca amplia para consumir `compress`/`merge` en proceso.

Cada una de esas cosas, si llega, va en su propia spec.
