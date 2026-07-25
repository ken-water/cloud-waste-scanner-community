#!/usr/bin/env python3
"""Normalize static-site meta descriptions for CWS pages.

The script is intentionally conservative: it only rewrites missing, overlong,
truncated, or generic template descriptions. Attribute order is preserved where
possible so generated HTML stays close to the source.
"""
from __future__ import annotations

import argparse
import re
from html.parser import HTMLParser
from pathlib import Path

MIN_LEN = 50
MAX_LEN = 155
GENERIC_PREFIXES = (
    'Scan cloud costs and optimize faster with Cloud Waste Scanner.',
    'Scan and optimization guide.',
)
TRUNCATED_ENDINGS = ('and', 'with', 'for', 'to', 'from', 'across', 'in', 'of', 'Waste', 'durable')
SKIP_NAMES = {'googleabfb229cbfdeaabd.html'}
SKIP_DIRS = {'archive'}

OVERRIDES = {
    'blog-series.html': 'Browse Cloud Waste Scanner article series by track, including product guides, governance playbooks, founder notes, release notes, and security whitepapers.',
    'skills.html': 'Download free Cloud Waste Scanner Community Skills for GitHub, Codex, and Claude to explain scan results, draft briefs, and audit exported evidence.',
    'blog/community-skills-github-codex-claude-release.html': 'Cloud Waste Scanner Community Skills ship as GitHub, Codex, and Claude packages for explaining, auditing, and acting on local scan evidence.',
    'blog/ai-runtime-k8s-gpu-governance-v2-9-21.html': 'Cloud Waste Scanner v2.9.21 adds local-first AI runtime scans, Kubernetes GPU capacity summaries, and governance report APIs for cost control.',
    'blog/industry-intelligence-harness-finops-vs-local-first-cws.html': 'Harness FinOps vs Cloud Waste Scanner in 2026: platform cost orchestration compared with local-first infrastructure forensics and reclamation.',
    'blog/kubernetes-container-scanning-v2-9-19.html': 'Cloud Waste Scanner v2.9.19 adds local-first Kubernetes and container waste scans with read-only kubectl, governance reports, and action plans.',
    'blog/customer-value-30-day-cloud-waste-control.html': 'A 30-day cloud waste turnaround showing how one team found ownership gaps, removed orphaned resources, and locked a weekly cleanup cadence.',
    'api-playbooks.html': 'Operational API playbooks for recurring scans, account targeting, report delivery, and automation handoff in Cloud Waste Scanner.',
    'api-reference.html': 'Detailed Cloud Waste Scanner API reference for local automation endpoints, auth behavior, request formats, and response contracts.',
    'metrics-definition.html': 'Definitions for Cloud Waste Scanner analytics, funnel calculations, app usage events, download attribution, and reporting metrics.',
}

class PageParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.descriptions: list[str] = []
        self.title_parts: list[str] = []
        self.h1_parts: list[str] = []
        self._in_title = False
        self._in_h1 = False

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        tag = tag.lower()
        if tag == 'title':
            self._in_title = True
        elif tag == 'h1':
            self._in_h1 = True
        elif tag == 'meta':
            attr_map = {key.lower(): value or '' for key, value in attrs if key}
            if attr_map.get('name', '').lower() == 'description':
                self.descriptions.append(attr_map.get('content', '').strip())

    def handle_endtag(self, tag: str) -> None:
        tag = tag.lower()
        if tag == 'title':
            self._in_title = False
        elif tag == 'h1':
            self._in_h1 = False

    def handle_data(self, data: str) -> None:
        if self._in_title:
            self.title_parts.append(data)
        if self._in_h1:
            self.h1_parts.append(data)


def clean_text(value: str) -> str:
    value = re.sub(r'\s+', ' ', value).strip()
    value = re.sub(r'\s+\|\s+CWS$', '', value)
    value = re.sub(r'\s+\|\s+Cloud Waste Scanner$', '', value)
    value = re.sub(r'\s+for cloud governance tools$', '', value, flags=re.I)
    value = value.replace('CloudWaste Scanner', 'Cloud Waste Scanner')
    return value.strip(' -')


def needs_rewrite(desc: str) -> bool:
    if not desc:
        return True
    if len(desc) < MIN_LEN or len(desc) > MAX_LEN:
        return True
    if any(desc.startswith(prefix) for prefix in GENERIC_PREFIXES):
        return True
    if 'for cloud governance tools' in desc.lower():
        return True
    tail = desc.rstrip(' .,:;').split(' ')[-1]
    return tail in TRUNCATED_ENDINGS


def clamp_sentence(desc: str) -> str:
    desc = clean_text(desc)
    if len(desc) <= MAX_LEN:
        return desc
    words = desc.split()
    while words:
        cut = ' '.join(words).rstrip(' ,;:')
        if len(cut) + (0 if cut.endswith('.') else 1) <= MAX_LEN:
            tail = cut.rstrip(' .,:;').split(' ')[-1]
            if tail not in TRUNCATED_ENDINGS:
                return cut + ('' if cut.endswith('.') else '.')
        words.pop()
    return desc[:MAX_LEN].rstrip(' ,;:') + '.'


def description_for(rel: str, parser: PageParser) -> str:
    if rel in OVERRIDES:
        return OVERRIDES[rel]

    title = clean_text(' '.join(parser.title_parts))
    h1 = clean_text(' '.join(parser.h1_parts))
    topic = h1 or title or 'Cloud Waste Scanner'
    topic = re.sub(r'^Cloud Waste Scanner\s*[:|-]\s*', '', topic)
    topic = re.sub(r'\s+for cloud governance tools$', '', topic, flags=re.I)

    if rel.startswith('solutions/'):
        topic = topic.replace(' | Cloud Waste Scanner', '')
        return clamp_sentence(f'{topic}: local-first evidence and risk-reviewed cleanup guidance for cloud waste teams.')

    if rel.startswith('playbooks/'):
        return clamp_sentence(f'{topic}: detection signals, cleanup steps, provider notes, and owner-ready evidence for weekly cloud waste review.')

    if rel.startswith('blog/industry-solutions-whitepaper'):
        return clamp_sentence(f'{topic}: local-first governance guidance for evidence review, rollout planning, compliance, and executive reporting.')

    if rel.startswith('blog/security-whitepaper'):
        return clamp_sentence(f'{topic}: security review guidance for local-first cloud governance, credential boundaries, controls, and audit readiness.')

    if rel.startswith('blog/technical-whitepaper'):
        return clamp_sentence(f'{topic}: technical architecture guidance for CWS data models, reliability, auditability, and rollout patterns.')

    if rel.startswith('blog/interview-origins'):
        return clamp_sentence(f'{topic}: founder note on local-first governance, execution safety, product tradeoffs, and savings discipline.')

    if rel.startswith('blog/'):
        return clamp_sentence(f'{topic}: practical CWS guidance for local-first cost review, evidence handoff, and safer cleanup decisions.')

    if rel.startswith('api-'):
        return clamp_sentence(f'{topic}: CWS API guidance for local automation, authentication, error handling, reports, and operational workflows.')

    return clamp_sentence(f'{topic}: CWS guidance for local-first cloud cost review, evidence handoff, and safer operational decisions.')


def set_meta_description(html: str, desc: str) -> str:
    meta_re = re.compile(r'<meta\s+[^>]*name=["\']description["\'][^>]*>', re.I)

    def replace_content(match: re.Match[str]) -> str:
        tag = match.group(0)
        if re.search(r'\scontent=["\'][^"\']*["\']', tag, re.I):
            return re.sub(r'\scontent=["\'][^"\']*["\']', f' content="{desc}"', tag, count=1, flags=re.I)
        return tag[:-1] + f' content="{desc}">'

    if meta_re.search(html):
        return meta_re.sub(replace_content, html, count=1)

    title_match = re.search(r'\n(\s*)<title>.*?</title>', html, re.I | re.S)
    if not title_match:
        raise ValueError('missing <title>')
    return html[:title_match.end()] + f'\n{title_match.group(1)}<meta name="description" content="{desc}">' + html[title_match.end():]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('root', type=Path)
    ap.add_argument('--check', action='store_true')
    args = ap.parse_args()

    changed: list[str] = []
    failures: list[str] = []
    for path in sorted(args.root.rglob('*.html')):
        rel = path.relative_to(args.root).as_posix()
        if any(part in SKIP_DIRS for part in path.relative_to(args.root).parts):
            continue
        if path.name in SKIP_NAMES:
            continue
        html = path.read_text(errors='ignore')
        parser = PageParser()
        parser.feed(html)
        desc = parser.descriptions[0] if parser.descriptions else ''
        if not needs_rewrite(desc):
            continue
        new_desc = description_for(rel, parser)
        if not (MIN_LEN <= len(new_desc) <= MAX_LEN):
            failures.append(f'{len(new_desc)}\t{rel}\t{new_desc}')
            continue
        changed.append(rel)
        if not args.check:
            path.write_text(set_meta_description(html, new_desc))

    if failures:
        print('Invalid generated descriptions:')
        print('\n'.join(failures))
        return 1
    action = 'would update' if args.check else 'updated'
    print(f'{action} {len(changed)} files')
    for rel in changed:
        print(rel)
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
