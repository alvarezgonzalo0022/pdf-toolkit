"""Shared helpers used across commands."""

import argparse


def human_size(n):
    """Format a byte count as a human-readable string."""
    for unit in ("B", "KB", "MB", "GB"):
        if n < 1024:
            return f"{n:.1f} {unit}"
        n /= 1024
    return f"{n:.1f} TB"


def parse_size(text):
    """Convert '24mb' / '1.5mb' / '900kb' / '1048576' to bytes."""
    t = text.strip().lower()
    mult = 1
    for suffix, m in (("kb", 1024), ("mb", 1024 ** 2), ("gb", 1024 ** 3), ("b", 1)):
        if t.endswith(suffix):
            mult, t = m, t[: -len(suffix)]
            break
    try:
        return float(t) * mult
    except ValueError:
        raise argparse.ArgumentTypeError(f"invalid size: {text!r}")
