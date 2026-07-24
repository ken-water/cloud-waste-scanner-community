#!/usr/bin/env python3
"""Recolor content SVG diagrams to match the current site palette."""

from __future__ import annotations

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]

# Keep the brand icon and provider logos unchanged.
SKIP_PATH_PARTS = {"logos"}
SKIP_FILES = {"product-icon.svg"}

REPLACEMENTS = [
    ("#f8fbff", "#fffaf0"),
    ("#eef3ff", "#eef8f6"),
    ("#f0f9ff", "#fffaf0"),
    ("#f8fafc", "#fffaf0"),
    ("#eff6ff", "#fdf7ea"),
    ("#ecfeff", "#eef8f6"),
    ("#f0fdfa", "#eef8f6"),
    ("#e0f2fe", "#d8e1ee"),
    ("#dbeafe", "#d8e1ee"),
    ("#bfdbfe", "#d8e1ee"),
    ("#93c5fd", "#d8e1ee"),
    ("#cbd5e1", "#d8e1ee"),
    ("#e2e8f0", "#e1e9f3"),
    ("#0a66c2", "#f0a83b"),
    ("#3b82f6", "#49c6b7"),
    ("#2563eb", "#1f6feb"),
    ("#1d4ed8", "#0c1724"),
    ("#0ea5e9", "#49c6b7"),
    ("#0284c7", "#1f6feb"),
    ("#6366f1", "#0c1724"),
    ("#0f172a", "#0c1724"),
    ("#111827", "#0c1724"),
    ("#1e293b", "#0c1724"),
    ("#334155", "#0c1724"),
    ("#475569", "#42526b"),
]


def should_skip(path: Path) -> bool:
    if path.name in SKIP_FILES:
        return True
    return any(part in SKIP_PATH_PARTS for part in path.parts)


def recolor(text: str) -> str:
    out = text
    for src, dst in REPLACEMENTS:
        out = re.sub(re.escape(src), dst, out, flags=re.I)
    return out


def main() -> None:
    changed = 0
    for path in list((ROOT / "images").rglob("*.svg")) + list((ROOT / "blog/images").rglob("*.svg")):
        if should_skip(path):
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        new_text = recolor(text)
        if new_text != text:
            path.write_text(new_text, encoding="utf-8")
            changed += 1
    print(f"recolored {changed} svg files")


if __name__ == "__main__":
    main()
