#!/usr/bin/env python3
"""Validate static-site meta descriptions without assuming attribute order."""
from __future__ import annotations

import sys
from html.parser import HTMLParser
from pathlib import Path

MIN_LEN = 50
MAX_LEN = 155
DEFAULT_ROOT = Path('releases/site')
SKIP_FILENAMES = {'googleabfb229cbfdeaabd.html'}
SKIP_DIRS = {'archive'}

class MetaDescriptionParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.descriptions: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag.lower() != 'meta':
            return
        attr_map = {key.lower(): value or '' for key, value in attrs if key}
        if attr_map.get('name', '').lower() == 'description':
            self.descriptions.append(attr_map.get('content', '').strip())

def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_ROOT
    failures: list[str] = []
    for path in sorted(root.rglob('*.html')):
        rel_parts = path.relative_to(root).parts
        if any(part in SKIP_DIRS for part in rel_parts):
            continue
        if path.name in SKIP_FILENAMES:
            continue
        parser = MetaDescriptionParser()
        parser.feed(path.read_text(errors='ignore'))
        if not parser.descriptions:
            failures.append(f'MISSING\t{path}')
            continue
        if len(parser.descriptions) != 1:
            failures.append(f'MULTIPLE\t{len(parser.descriptions)}\t{path}')
            continue
        desc = parser.descriptions[0]
        if not (MIN_LEN <= len(desc) <= MAX_LEN):
            failures.append(f'LENGTH\t{len(desc)}\t{path}\t{desc}')
    if failures:
        print('\n'.join(failures))
        return 1
    print(f'OK: meta descriptions are present and {MIN_LEN}-{MAX_LEN} chars under {root}')
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
