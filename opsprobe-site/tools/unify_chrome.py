#!/usr/bin/env python3
"""Normalize header and footer across current public pages."""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def depth_for(path: Path) -> str:
    rel = path.relative_to(ROOT)
    return "../" * (len(rel.parts) - 1)


def active_for(path: Path) -> str:
    rel = path.relative_to(ROOT)
    first = rel.parts[0]
    name = rel.name
    if first == "blog" or name in {"blog.html", "blog-series.html"}:
        return "Blog"
    if first == "solutions" or name == "solutions.html":
        return "Solutions"
    if first == "download":
        return "Download"
    if name == "skills.html":
        return "Skills"
    if name == "pricing.html":
        return "Beta"
    if name in {"help.html", "recover.html"}:
        return "Help"
    return ""


def header(depth: str, active: str) -> str:
    links = [
        ("Home", "index.html"),
        ("Beta", "pricing.html"),
        ("Skills", "skills.html"),
        ("Blog", "blog.html"),
        ("Solutions", "solutions.html"),
        ("Help", "help.html"),
    ]
    nav = "".join(
        f'<a href="{depth}{href}"{" aria-current=\"page\"" if label == active else ""}>{label}</a>'
        for label, href in links
    )
    return f"""<header class="nav">
    <div class="container nav-inner">
      <a class="brand" href="{depth}index.html"><img src="{depth}assets/product-icon.svg" alt="Cloud Waste Scanner"><span>Cloud Waste Scanner</span></a>
      <nav class="nav-links" aria-label="Primary">{nav}</nav>
      <div class="nav-actions"><a class="btn primary" href="{depth}download/">Download</a></div>
    </div>
  </header>"""


def footer(depth: str) -> str:
    return f"""<footer class="footer">
    <div class="container footer-grid">
      <div><a class="brand" href="{depth}index.html"><img src="{depth}assets/product-icon.svg" alt="Cloud Waste Scanner"><span>Cloud Waste Scanner</span></a><p style="margin-top:14px; max-width:320px;">Local-first cloud waste review with commercial licensing and clear evidence outputs.</p></div>
      <div><h3>Product</h3><div class="footer-links"><a href="{depth}pricing.html">Public Beta</a><a href="{depth}download/">Download</a><a href="{depth}solutions.html">Solutions</a><a href="{depth}api.html">API</a></div></div>
      <div><h3>Learn</h3><div class="footer-links"><a href="{depth}blog.html">Blog</a><a href="{depth}blog-series.html">Blog Series</a><a href="{depth}skills.html">Skills</a><a href="{depth}roadmap.html">Roadmap</a></div></div>
      <div><h3>Support</h3><div class="footer-links"><a href="{depth}help.html">Help</a><a href="{depth}feedback.html">Feedback</a><a href="{depth}recover.html">Recover License</a></div></div>
      <div><h3>Legal</h3><div class="footer-links"><a href="{depth}license.html">License</a><a href="{depth}security.html">Security</a><a href="{depth}privacy.html">Privacy</a><a href="{depth}terms.html">Terms</a></div></div>
    </div>
  </footer>"""


def should_skip(path: Path) -> bool:
    rel = path.relative_to(ROOT)
    if rel.parts[0] == "archive" and rel.parts != ("archive", "index.html"):
        return True
    return path.name == "googleabfb229cbfdeaabd.html"


def main() -> None:
    changed = 0
    for path in ROOT.rglob("*.html"):
        if should_skip(path):
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        depth = depth_for(path)
        new_text = re.sub(
            r'<header class="nav">.*?</header>',
            header(depth, active_for(path)),
            text,
            count=1,
            flags=re.S,
        )
        new_text = re.sub(
            r'<footer class="footer">.*?</footer>',
            footer(depth),
            new_text,
            count=1,
            flags=re.S,
        )
        if '<footer class="footer">' not in new_text:
            new_text = new_text.replace(
                "\n</body>",
                "\n  " + footer(depth) + "\n</body>",
                1,
            )
        if new_text != text:
            path.write_text(new_text, encoding="utf-8")
            changed += 1
    print(f"normalized {changed} pages")


if __name__ == "__main__":
    main()
