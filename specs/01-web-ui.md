# SPEC 01 — Interfaz web para el toolkit

> **Estado:** Implementado
> **Depende de:** —
> **Fecha:** 2026-11-25
> **Objetivo:** Añadir una página web local que ejecute `compress` y `merge` de `pdf_toolkit` en memoria, subiendo PDFs desde el navegador y devolviendo el resultado como descarga automática.

## Scope

**In:**

- Servidor web Flask en `pdf_toolkit/web/` con endpoints `/compress` y `/merge`.
- Página HTML estática con un formulario para `compress` (dpi, quality, grayscale, target) y otro para `merge` (varios archivos en orden).
- Refactor de las funciones núcleo de `compress.py` y `merge.py` para operar en memoria (bytes → bytes), sin tocar el comportamiento del CLI.
- Nuevo entry point `pdf-toolkit-web` en `pyproject.toml` y la dependencia `flask`.

**Out of scope (for future specs):**

- Autenticación o soporte multiusuario.
- Autodescubrimiento de comandos futuros (split, rotate, encrypt…).
- Persistencia de archivos: nada de lo subido ni generado se guarda en disco.
- Exponer `--verbose` y `--min-dpi` en la interfaz.
- Pantalla de resumen con métricas antes/después (la entrega es descarga directa).

## Data model

Esta feature no introduce datos persistentes. Todo el flujo es en memoria. Las firmas nuevas son:

```python
# pdf_toolkit/commands/compress.py
def compress_bytes(data: bytes, dpi: int, quality: int, grayscale: bool) -> bytes:
    """Comprime un PDF en memoria y devuelve los bytes resultantes."""

def compress_bytes_to_target(
    data: bytes, dpi: int, quality: int, grayscale: bool, target: int
) -> tuple[bytes, int, int]:
    """Baja dpi/quality iterativamente hasta que len(bytes) <= target.
    Devuelve (bytes, dpi_usado, quality_usado)."""

# pdf_toolkit/commands/merge.py
def merge_bytes(pdfs: list[bytes]) -> bytes:
    """Concatena varios PDFs (en orden) y devuelve los bytes resultantes."""
```

Contrato HTTP (multipart):

- `POST /compress` — campo `file` + campos `dpi`, `quality`, `grayscale`, `target` (opcional). Responde `200` con el PDF adjunto (`Content-Disposition: attachment; filename="<nombre>-compressed.pdf"`), o `400` con `{"error": "<mensaje>"}`.
- `POST /merge` — campo `files` (múltiples, en orden). Responde `200` con el PDF adjunto (`<nombre-del-primero>-merged.pdf`), o `400` con `{"error": "<mensaje>"}`.

## Implementation plan

1. Extraer `compress_bytes` y `compress_bytes_to_target` en `compress.py`, operando sobre `fitz.open(stream=io.BytesIO(data))` y serializando con `doc.tobytes(garbage=4, deflate=True, clean=True)`. Convertir `compress_once`/`compress_to_target` en envoltorios que leen/escriben archivo para el CLI. Verificar: `pdf-toolkit compress input.pdf` sigue funcionando.
2. Extraer `merge_bytes` en `merge.py` (abre cada PDF desde un stream, `insert_pdf`, serializa con `tobytes`). Mantener `merge_pdfs`/`run` como envoltorios del CLI. Verificar: `pdf-toolkit merge a.pdf b.pdf` sigue funcionando.
3. Crear `pdf_toolkit/web/__init__.py` y `pdf_toolkit/web/app.py` con la app Flask y los endpoints `/compress` y `/merge` (leer multipart → llamar a `*_bytes` → devolver adjunto o `400`).
4. Crear `pdf_toolkit/web/static/` con `index.html`, estilos y JS mínimo: envío con `fetch`/`FormData`, descarga automática del blob en `200`, y mensaje de error en línea en `400`.
5. Añadir `flask` a `dependencies`, registrar `pdf-toolkit-web = "pdf_toolkit.web.app:main"` en `[project.scripts]`, e incluir `pdf_toolkit.web` en `packages`. Aceptar `--host` y `--port` (default `127.0.0.1:5000`).
6. Verificación manual de punta a punta: `pip install -e .`, `pdf-toolkit-web`, y usar ambas herramientas desde el navegador.

## Acceptance criteria

- [ ] `pdf-toolkit compress` y `pdf-toolkit merge` del CLI siguen funcionando exactamente igual (mismas salidas y nombres por defecto).
- [ ] `pip install -e .` deja disponible el comando `pdf-toolkit-web`.
- [ ] `pdf-toolkit-web` sirve la página en `http://127.0.0.1:5000`.
- [ ] Comprimir un PDF desde la web descarga `<nombre>-compressed.pdf`.
- [ ] Unir dos o más PDFs desde la web descarga un único PDF con las páginas en orden.
- [ ] Los controles dpi/quality/grayscale/target se reflejan en el resultado (p. ej. `target 1mb` produce un PDF < 1 MB).
- [ ] Un PDF cifrado en `merge` muestra un error claro y no descarga nada.
- [ ] No se escribe nada en disco: no queda ningún archivo temporal tras procesar.

## Decisions

- **Sí:** Flask como backend. Dependencia mínima y suficiente para dos endpoints de subida/descarga.
- **Sí:** refactor a `bytes` en memoria. El usuario pidió "procesar y devolver, sin guardar".
- **Sí:** descarga automática directa (sin pantalla de métricas), con errores en línea.
- **Sí:** exponer dpi, quality, grayscale y target; ocultar `--verbose` y el no-op `--min-dpi`.
- **No:** archivos temporales en disco. Incompatible con el requisito de no guardar.
- **No:** FastAPI. Aporta async/validación que este caso no necesita.
- **No:** autenticación ni multiusuario. Es local y de un solo usuario.
- **No:** autodescubrimiento del registry. Hoy son solo dos comandos.

## Risks

| Riesgo | Mitigación |
| --- | --- |
| `doc.tobytes()` puede no replicar exactamente `save(garbage=4, deflate=True, clean=True)`. | Verificar la equivalencia en el paso 1; si difiere, envolver con `fitz.Stream`. |
| Regresión del CLI tras el refactor. | Cada paso de refactor incluye una verificación del CLI antes de continuar. |
| Subidas muy grandes agotan memoria (no hay límite de tamaño). | Aceptable en uso local/single-user; documentado, no bloqueante. |

## What is **not** in this spec

- Editor visual, rotación, split o cualquier comando nuevo.
- Cuentas de usuario, login o despliegue público.
- Almacenamiento (temporal o permanente) de PDFs subidos o generados.

Cada una de esas cosas, si llega, va en su propia spec.
