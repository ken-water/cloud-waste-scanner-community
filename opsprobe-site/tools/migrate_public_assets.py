#!/usr/bin/env python3
"""Generate lightweight current-site pages from the legacy public site."""

from __future__ import annotations

import html
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OLD = ROOT / "releases" / "site"
NEW = ROOT / "opsprobe-site"


BLOG_SERIES = {
    "Product Guides": [
        "first-scan-in-15-minutes.html",
        "network-proxy-setup-restricted-environments.html",
        "scan-results-to-team-weekly-rhythm.html",
        "cloud-waste-scanner-user-guide-core-setup.html",
        "cloud-waste-scanner-user-guide-account-proxy-notification-scan-result.html",
        "cloud-waste-scanner-user-guide-dashboard-monitor-governance-scan-result.html",
        "cloud-waste-scanner-user-guide-choose-what-to-scan-first.html",
        "cloud-governance-tools-taxonomy-trends-api.html",
        "cloud-governance-framework-weekly-operating-playbook.html",
        "notification-driven-weekly-cloud-governance.html",
    ],
    "Product Updates": [
        "governance-handoff-lifecycle-v3-1-0.html",
        "ai-runtime-k8s-gpu-governance-v2-9-21.html",
        "kubernetes-container-scanning-v2-9-19.html",
        "v2-launch-global.html",
        "evolution-of-control-v1-16.html",
        "enterprise-grade-safety-v1-14.html",
        "global-rightsizing.html",
        "rightsizing-launch.html",
        "storage-tiering-launch.html",
        "v1-launch.html",
    ],
    "Whitepapers": [
        "security-whitepaper-local-first-part1-threat-model-and-trust-boundary.html",
        "security-whitepaper-local-first-part2-control-matrix-and-operational-safeguards.html",
        "security-whitepaper-local-first-part3-verification-checklist-and-incident-readiness.html",
        "security-whitepaper-local-first-part4-token-lifecycle-and-api-hardening.html",
        "security-whitepaper-local-first-part5-transport-integrity-and-operational-auditability.html",
        "technical-whitepaper-local-first-part1-engine-architecture-and-component-boundaries.html",
        "technical-whitepaper-local-first-part2-findings-data-model-and-policy-evaluation.html",
        "technical-whitepaper-local-first-part3-performance-reliability-and-rollout-patterns.html",
        "technical-whitepaper-local-first-part4-quality-gates-auditability-and-delivery-discipline.html",
        "technical-whitepaper-local-first-part5-rust-tauri-runtime-tradeoffs-and-platform-roadmap.html",
        "industry-solutions-whitepaper-part1-invisible-cloud-debt-and-sovereign-governance.html",
        "industry-solutions-whitepaper-part2-regulated-environments-control-evidence-and-change-review.html",
        "industry-solutions-whitepaper-part3-engineering-integration-ci-cd-and-runtime-guardrails.html",
        "industry-solutions-whitepaper-part4-industry-playbooks-finance-saas-and-platform-teams.html",
        "industry-solutions-whitepaper-part5-rollout-roadmap-governance-kpis-and-executive-reporting.html",
        "industry-solutions-whitepaper-part6-security-and-compliance-mapping-for-local-first-governance.html",
        "industry-solutions-whitepaper-part7-procurement-and-selection-matrix-local-first-vs-saas.html",
        "industry-solutions-whitepaper-appendix-terms-metrics-templates-and-checklists.html",
    ],
    "Industry Intelligence": [
        "customer-value-30-day-cloud-waste-control.html",
        "industry-intelligence-harness-finops-vs-local-first-cws.html",
        "industry-intelligence-spotio-vs-local-first-cws.html",
        "industry-intelligence-cloudcustodian-vs-local-first-cws.html",
        "industry-intelligence-cast-ai-vs-local-first-cws.html",
        "industry-intelligence-kubecost-vs-local-first-cws.html",
        "industry-intelligence-cloudhealth-vs-local-first-cws.html",
        "industry-intelligence-prosperops-vs-local-first-cws.html",
        "industry-intelligence-cloudzero-vs-local-first-cws.html",
        "industry-intelligence-vantage-vs-local-first-cws.html",
    ],
    "Incident Series": [
        "boss-saw-the-cloud-bill-and-asked-if-i-was-secretly-mining.html",
        "march-rain-disappearing-coins.html",
        "when-cloud-prices-go-up-we-found-lost-profit-in-15-minutes.html",
        "the-christmas-gift-in-the-server-room.html",
        "cloud-cost-story-p5-the-ghost-resources-nobody-owned.html",
        "cloud-waste-horror-stories.html",
    ],
    "Founder Notes": [
        "interview-origins-part1.html",
        "interview-origins-part2-local-first.html",
        "interview-origins-part3.html",
        "interview-origins-part4-safe-automation.html",
        "interview-origins-part5-policy-simulation-edge-cases-reporting.html",
        "interview-origins-part6-operating-model-compounding-governance.html",
        "customer-first-shipping-feedback-loop.html",
        "engineering-trust-customer-value-esg.html",
        "local-first-finops.html",
        "deep-finops-anatomy.html",
        "hidden-cloud-costs.html",
        "the-idle-fallacy.html",
    ],
}

EXTRA_CURRENT_BLOGS = [
    {
        "file": "first-scan-simple-local-product.html",
        "title": "How the product stays easy to use",
        "description": "Practical guidance on keeping Cloud Waste Scanner simple for buyers and useful for operators.",
        "teaser": "The rebuild keeps the buying and setup experience simple while the scanner itself stays local and trustworthy.",
        "date": "2026-07-19",
        "category": "Article",
    },
    {
        "file": "download-playbook.html",
        "title": "Pick the right package first",
        "description": "Choose the right Cloud Waste Scanner package for Windows, Linux, or macOS and verify it before installation.",
        "teaser": "Choose the right installer, verify the checksum, and avoid making users guess which package they need.",
        "date": "2026-07-19",
        "category": "Guide",
    },
]


def read(path: Path) -> str:
    return repair_mojibake(path.read_text(encoding="utf-8", errors="ignore"))


def repair_mojibake(text: str) -> str:
    replacements = {
        "\u00e2\u0080\u00a2": "\u2022",
        "\u00e2\u0086\u0092": "\u2192",
        "\u00e2\u0080\u0099": "\u2019",
        "\u00e2\u0080\u009c": "\u201c",
        "\u00e2\u0080\u009d": "\u201d",
        "\u00e2\u0080\u0094": "\u2014",
        "\u00e2\u0080\u0093": "\u2013",
        "\u00e2\u0080\u00a6": "\u2026",
        "\u00c2\u00b7": "\u00b7",
        "\u00c2\u00a9": "\u00a9",
        "\u00c3\u0097": "\u00d7",
    }
    for bad, good in replacements.items():
        text = text.replace(bad, good)
    return text


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def one(pattern: str, text: str, default: str = "") -> str:
    match = re.search(pattern, text, re.I | re.S)
    return html.unescape(match.group(1).strip()) if match else default


def clean_title(title: str) -> str:
    title = re.sub(r"\s*\|\s*CWS\s*$", "", title)
    title = re.sub(r"\s*-\s*Cloud Waste Scanner\s*$", "", title)
    title = title.replace("CWS Pro", "Cloud Waste Scanner")
    title = title.replace("Cloud Waste Scanner Pro", "Cloud Waste Scanner")
    title = title.replace("Community Skills", "AI Evidence Skills")
    title = title.replace("Community", "Product")
    return title.strip() or "Cloud Waste Scanner"


def plain(text: str) -> str:
    text = re.sub(r"<script\b[^>]*>.*?</script>", "", text, flags=re.I | re.S)
    text = re.sub(r"<style\b[^>]*>.*?</style>", "", text, flags=re.I | re.S)
    text = re.sub(r"<[^>]+>", " ", text)
    text = re.sub(r"\s+", " ", html.unescape(text)).strip()
    return text


def sanitize_fragment(fragment: str, depth: str) -> str:
    fragment = re.sub(r"<script\b[^>]*>.*?</script>", "", fragment, flags=re.I | re.S)
    fragment = re.sub(r"<style\b[^>]*>.*?</style>", "", fragment, flags=re.I | re.S)
    fragment = re.sub(r"<iframe\b[^>]*>.*?</iframe>", "", fragment, flags=re.I | re.S)
    fragment = re.sub(r"\sclass=\"[^\"]*\"", "", fragment)
    fragment = re.sub(r"\sstyle=\"[^\"]*\"", "", fragment)
    fragment = re.sub(r"\sdata-[a-z0-9_-]+=\"[^\"]*\"", "", fragment, flags=re.I)
    fragment = fragment.replace("Cloud Waste Scanner Pro", "Cloud Waste Scanner")
    fragment = fragment.replace("CWS Pro", "Cloud Waste Scanner")
    fragment = fragment.replace("Community Skills", "AI Evidence Skills")
    fragment = fragment.replace("Community capability", "commercial product capability")
    fragment = fragment.replace("Community boundary", "commercial product boundary")
    fragment = fragment.replace("Community line", "current product line")
    fragment = fragment.replace("Community repository", "release package")
    fragment = fragment.replace("community repository", "release package")
    fragment = fragment.replace("Community users", "current users")
    fragment = fragment.replace("Community", "current product")
    fragment = re.sub(r"\bPro line\b", "current product line", fragment)
    fragment = re.sub(r"\bPro\b", "product", fragment)
    fragment = fragment.replace("GitHub, Codex, and Claude", "local evidence workflows")
    fragment = fragment.replace("GitHub", "release package")
    fragment = fragment.replace("Team and Enterprise work", "planned workflow work")
    fragment = fragment.replace("release-ledger updates", "release notes")
    fragment = fragment.replace("Our backend handles licensing and updates", "The licensing service handles licensing and updates")
    fragment = fragment.replace("one giant backend", "one giant hosted service")
    fragment = fragment.replace("View Sample Report", "Open Blog")
    fragment = fragment.replace("Download Free Scanner", "Download App")
    fragment = fragment.replace("Download Trial", "Download App")
    fragment = fragment.replace("free scanner", "scanner")
    fragment = fragment.replace("GitHub", "release package")
    fragment = re.sub(r'href="../skills\.html[^"]*"', f'href="{depth}help.html"', fragment)
    fragment = re.sub(r'href="../documentation\.html[^"]*"', f'href="{depth}help.html"', fragment)
    fragment = re.sub(r'href="../api-playbooks\.html[^"]*"', f'href="{depth}api.html"', fragment)
    fragment = re.sub(r'href="../api-reference\.html[^"]*"', f'href="{depth}api.html"', fragment)
    fragment = re.sub(r'href="../api-token-guide\.html[^"]*"', f'href="{depth}api.html"', fragment)
    fragment = re.sub(r'href="../api-errors\.html[^"]*"', f'href="{depth}help.html"', fragment)
    fragment = re.sub(r'href="../metrics-definition\.html[^"]*"', f'href="{depth}api.html"', fragment)
    fragment = re.sub(r'href="../providers\.html[^"]*"', f'href="{depth}help.html#provider-setup"', fragment)
    fragment = re.sub(r'href="../provider-credentials\.html[^"]*"', f'href="{depth}help.html#provider-setup"', fragment)
    fragment = re.sub(r'href="../playbooks\.html[^"]*"', f'href="{depth}solutions.html"', fragment)
    fragment = re.sub(r'href="../whitepapers\.html[^"]*"', f'href="{depth}blog-series.html"', fragment)
    fragment = re.sub(r'href="../company\.html[^"]*"', f'href="{depth}index.html"', fragment)
    fragment = re.sub(r'href="../enterprise\.html[^"]*"', f'href="{depth}pricing.html"', fragment)
    fragment = re.sub(r'href="../release-ledger\.html[^"]*"', f'href="{depth}blog-series.html"', fragment)
    fragment = re.sub(r'href="files/[^"]+"', f'href="{depth}blog.html"', fragment)
    fragment = re.sub(r'href="../files/[^"]+"', f'href="{depth}security.html"', fragment)
    fragment = re.sub(r'href="../sample-report\.html[^"]*"', f'href="{depth}blog.html"', fragment)
    fragment = re.sub(r'href="../api-center\.html[^"]*"', f'href="{depth}api.html"', fragment)
    fragment = re.sub(r'href="../provider-credentials\.html[^"]*"', f'href="{depth}help.html#provider-setup"', fragment)
    fragment = re.sub(r'href="../roadmap\.html[^"]*"', f'href="{depth}roadmap.html"', fragment)
    fragment = re.sub(r'href="../download/"', f'href="{depth}download/"', fragment)
    fragment = re.sub(r'href="../pricing\.html"', f'href="{depth}pricing.html"', fragment)
    fragment = re.sub(r'href="../security\.html"', f'href="{depth}security.html"', fragment)
    fragment = re.sub(r'href="../blog\.html"', f'href="{depth}blog.html"', fragment)
    fragment = re.sub(r'href="../index\.html"', f'href="{depth}index.html"', fragment)
    fragment = re.sub(r'src="images/', 'src="images/', fragment)
    fragment = re.sub(r'src="../../images/', 'src="../images/', fragment)
    fragment = fragment.replace('images/product-guide-part8-2026/part8-notification-methods.webp?v=2.5.14.2', 'images/product-guide-part8-2026/part8-dashboard-overview.webp')
    fragment = fragment.replace('images/product-guide-part8-2026/part8-add-account-routing.webp?v=2.5.14.2', 'images/product-guide-part8-2026/part8-dashboard-overview.webp')
    fragment = fragment.replace('images/product-guide-part8-2026/part8-scan-wizard-target.webp?v=2.5.14.2', 'images/product-guide-part8-2026/part8-dashboard-overview.webp')
    fragment = fragment.replace('images/product-guide-part8-2026/part8-scan-result-actions.webp?v=2.5.14.2', 'images/product-guide-part8-2026/part8-dashboard-overview.webp')
    fragment = fragment.replace('images/product-guide-part8-2026/part8-execution-plan-review.webp?v=2.5.14.2', 'images/product-guide-part8-2026/part8-dashboard-overview.webp')
    fragment = fragment.replace('images/product-guide-part8-2026/part8-export-pdf-config.webp?v=2.5.14.2', 'images/product-guide-part8-2026/part8-dashboard-overview.webp')
    fragment = fragment.replace('images/product-guide-part8-2026/part8-exported-pdf-checklist.webp?v=2.5.14.2', 'images/product-guide-part8-2026/part8-dashboard-overview.webp')
    fragment = fragment.replace('images/product-guide-part8-2026/part8-monitor-trend.webp?v=2.5.14.2', 'images/product-guide-part8-2026/part8-dashboard-overview.webp')
    fragment = fragment.replace('images/product-guide-part8-2026/part8-governance-trend.webp?v=2.5.14.2', 'images/product-guide-part8-2026/part8-dashboard-overview.webp')
    fragment = fragment.replace('images/industry-intelligence-2026/kubecost-vs-cws-semantic-penetration.svg?v=20260323-2', 'images/industry-intelligence-2026/kubecost-vs-cws-cfo-vs-hunter.svg')
    fragment = re.sub(r'href="../../index\.html#download"', f'href="{depth}download/"', fragment)
    return fragment


def extract_blog_body(text: str) -> str:
    body = one(r'<div[^>]+data-blog-body[^>]*>(.*?)</div>\s*<div class="cws-cta-panel', text)
    if body:
        body = body.strip()
        body = re.sub(r'^<header>.*?</header>\s*', '', body, flags=re.S)
        body = re.sub(r'^<div[^>]*data-blog-body[^>]*>\s*', '', body, flags=re.S)
        body = re.sub(r'\s*</div>\s*$', '', body, flags=re.S)
        return body
    body = one(r"<article[^>]*>(.*?)</article>", text)
    if body:
        body = body.strip()
        body = re.sub(r'^<header>.*?</header>\s*', '', body, flags=re.S)
        return body
    body = one(r"<main[^>]*>(.*?)</main>", text)
    body = body.strip()
    body = re.sub(r'^<header>.*?</header>\s*', '', body, flags=re.S)
    return body


def extract_solution_body(text: str) -> str:
    body = one(r"<article[^>]*>(.*?)</article>", text)
    if body:
        return body
    return one(r"<main[^>]*>(.*?)</main>", text)


def strip_embedded_title_blocks(fragment: str) -> str:
    fragment = re.sub(r"^\s*<header>.*?</header>\s*", "", fragment, flags=re.S)
    fragment = re.sub(r"^\s*<h1[^>]*>.*?</h1>\s*", "", fragment, count=1, flags=re.S)
    return fragment.strip()


def header(depth: str, active: str = "") -> str:
    links = [("Home", "index.html"), ("Pricing", "pricing.html"), ("Skills", "skills.html"), ("Blog", "blog.html"), ("Solutions", "solutions.html"), ("Help", "help.html")]
    nav = "".join(f'<a href="{depth}{href}"{" aria-current=\"page\"" if label == active else ""}>{label}</a>' for label, href in links)
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
      <div><h3>Product</h3><div class="footer-links"><a href="{depth}pricing.html">Pricing</a><a href="{depth}download/">Download</a><a href="{depth}solutions.html">Solutions</a><a href="{depth}api.html">API</a></div></div>
      <div><h3>Learn</h3><div class="footer-links"><a href="{depth}blog.html">Blog</a><a href="{depth}blog-series.html">Blog Series</a><a href="{depth}skills.html">Skills</a><a href="{depth}help.html">Help</a><a href="{depth}roadmap.html">Roadmap</a></div></div>
      <div><h3>Trust</h3><div class="footer-links"><a href="{depth}license.html">License</a><a href="{depth}security.html">Security</a><a href="{depth}privacy.html">Privacy</a><a href="{depth}terms.html">Terms</a></div></div>
    </div>
</footer>"""


def page(title: str, description: str, body: str, depth: str = "", active: str = "") -> str:
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{html.escape(title)} | Cloud Waste Scanner</title>
  <meta name="description" content="{html.escape(description[:155])}">
  <link rel="icon" href="{depth}assets/product-icon.svg">
  <link rel="stylesheet" href="{depth}styles.css">
  <script defer src="{depth}js/blog-toc.js"></script>
</head>
<body>
  {header(depth, active)}
  {body}
  {footer(depth)}
</body>
</html>
"""


def generate_blog_articles() -> list[dict[str, str]]:
    items = []
    for path in sorted((OLD / "blog").glob("*.html")):
        raw = read(path)
        title = clean_title(one(r"<title>(.*?)</title>", raw, path.stem.replace("-", " ").title()))
        description = one(r'<meta\s+name="description"\s+content="([^"]*)"', raw, "")
        description = description.replace("Cloud Waste Scanner Pro", "Cloud Waste Scanner").replace("CWS Pro", "Cloud Waste Scanner")
        description = description.replace("Community Skills", "AI Evidence Skills").replace("Community", "current product").replace("GitHub, Codex, and Claude", "local evidence workflows").replace("GitHub", "release package")
        if not description:
            description = plain(extract_blog_body(raw))[:155]
        body = sanitize_fragment(extract_blog_body(raw), "../")
        h1 = one(r"<h1[^>]*>(.*?)</h1>", body, title)
        body = strip_embedded_title_blocks(body)
        teaser = plain(one(r"<p[^>]*>(.*?)</p>", body, description))[:190]
        date = one(r'<time[^>]*datetime="([^"]+)"[^>]*>', raw, "")
        if not date:
            date = plain(one(r"<time[^>]*>(.*?)</time>", raw, ""))
        if not date:
            date = one(r'"datePublished"\s*:\s*"([^"]+)"', raw, "")
        category = "Article"
        for series, filenames in BLOG_SERIES.items():
            if path.name in filenames:
                category = series
                break
        article = f"""<main class="section">
  <div class="container article-layout">
    <aside class="blog-sidebar" aria-label="Article table of contents">
      <section class="blog-sidebar-card" data-blog-toc-card>
        <h2>On this page</h2>
        <div class="toc-links" data-blog-toc></div>
      </section>
    </aside>
    <article class="article">
      <div class="eyebrow">{html.escape(category)}</div>
      <h1>{h1}</h1>
      <p class="lead">{html.escape(description)}</p>
      <div class="meta-line">{html.escape(date)} · {html.escape(category)}</div>
      <div class="article-body" data-article-body>{body}</div>
      <div class="card article-cta"><h3>Run the same review locally.</h3><p>Use Cloud Waste Scanner to scan locally, review evidence, and export findings for your team.</p><a class="btn primary" href="../download/">Download App</a></div>
    </article>
  </div>
</main>"""
        write(NEW / "blog" / path.name, page(title, description, article, "../", "Blog"))
        items.append({"file": path.name, "title": plain(h1), "description": description, "teaser": teaser, "date": date, "category": category})
    return items


def generate_blog_index(items: list[dict[str, str]]) -> None:
    cards = "\n".join(
        f"""<article class="card"><div class="eyebrow">{html.escape(item['category'])}</div><h3><a href="blog/{item['file']}">{html.escape(item['title'])}</a></h3><p style="margin-top:12px;">{html.escape(item['teaser'] or item['description'])}</p><p class="meta-line">{html.escape(item['date'])}</p></article>"""
        for item in items
    )
    category_order = list(BLOG_SERIES) + ["Article", "Guide"]
    count_by_category = {category: sum(1 for item in items if item["category"] == category) for category in category_order}
    pills = "".join(
        f'<span class="pill">{html.escape(name)} {count}</span>'
        for name, count in count_by_category.items()
        if count
    )
    body = f"""<main class="section">
  <div class="container">
    <div class="section-title">
      <div class="eyebrow">Knowledge Hub</div>
      <h1>Cloud waste guides, product notes, and operating playbooks.</h1>
      <p>The current site carries {len(items)} published articles with a simplified commercial product message and current navigation.</p>
    </div>
    <div class="pill-row">{pills}</div>
    <div class="grid two article-list" style="margin-top:28px;">{cards}</div>
  </div>
</main>"""
    write(NEW / "blog.html", page("Blog", "Cloud Waste Scanner guides, trust notes, release notes, and operating playbooks.", body, "", "Blog"))


def generate_blog_series(items: list[dict[str, str]]) -> None:
    lookup = {item["file"]: item for item in items}
    sections = []
    for series, filenames in BLOG_SERIES.items():
        rows = []
        for filename in filenames:
            item = lookup.get(filename)
            if not item:
                continue
            rows.append(f'<li><a href="blog/{filename}">{html.escape(item["title"])}</a><span>{html.escape(item["date"])}</span></li>')
        sections.append(f"""<section class="card series-card" id="{re.sub(r'[^a-z0-9]+', '-', series.lower()).strip('-')}"><div class="eyebrow">Series</div><h3>{html.escape(series)}</h3><p>{len(rows)} articles</p><ul>{''.join(rows)}</ul></section>""")
    body = f"""<main class="section">
  <div class="container">
    <div class="section-title"><div class="eyebrow">Series Library</div><h2>Read by workflow, not only by date.</h2><p>Series pages preserve the original reading paths while using the new simplified commercial site structure.</p></div>
    <div class="grid two" style="margin-top:28px;">{''.join(sections)}</div>
  </div>
</main>"""
    write(NEW / "blog-series.html", page("Blog Series", "Browse Cloud Waste Scanner articles by product guides, release notes, trust, industry, and operating stories.", body, "", "Blog"))


def generate_solutions() -> list[dict[str, str]]:
    items = []
    for path in sorted((OLD / "solutions").glob("*.html")):
        raw = read(path)
        title = clean_title(one(r"<title>(.*?)</title>", raw, path.stem.replace("-", " ").title()))
        description = one(r'<meta\s+name="description"\s+content="([^"]*)"', raw, "")
        description = description.replace("Cloud Waste Scanner Pro", "Cloud Waste Scanner").replace("CWS Pro", "Cloud Waste Scanner")
        description = description.replace("Community", "current product").replace("GitHub", "release package")
        body = strip_embedded_title_blocks(sanitize_fragment(extract_solution_body(raw), "../"))
        body = body.replace("Cloud Waste Scanner automates", "Cloud Waste Scanner helps with")
        body = body.replace("Cloud Waste Scanner Pro", "Cloud Waste Scanner")
        teaser = plain(body)[:180]
        content = f"""<main class="section">
  <div class="container article">
    <div class="eyebrow">Solution</div>
    <h1>{html.escape(title)}</h1>
    <p class="lead">{html.escape(description or teaser)}</p>
    <div class="article-body">{body}</div>
    <div class="card article-cta"><h3>Review this waste locally.</h3><p>Install the app, connect the provider with read-only access first, and export evidence before cleanup.</p><a class="btn primary" href="../download/">Download App</a></div>
  </div>
</main>"""
        write(NEW / "solutions" / path.name, page(title, description or teaser, content, "../", "Solutions"))
        items.append({"file": path.name, "title": title, "description": description or teaser})
    return items


def generate_solutions_index(items: list[dict[str, str]]) -> None:
    cards = "\n".join(f"""<article class="card"><h3><a href="solutions/{item['file']}">{html.escape(item['title'])}</a></h3><p style="margin-top:12px;">{html.escape(item['description'])}</p></article>""" for item in items)
    body = f"""<main class="section">
  <div class="container">
    <div class="section-title">
      <div class="eyebrow">Solutions</div>
      <h1>Resource-level cloud waste cleanup playbooks.</h1>
      <p>{len(items)} current solution pages cover common compute, storage, network, and database waste across supported providers.</p>
    </div>
    <div class="grid three" style="margin-top:28px;">{cards}</div>
  </div>
</main>"""
    write(NEW / "solutions.html", page("Solutions", "Resource-level Cloud Waste Scanner cleanup playbooks across supported cloud providers.", body, "", "Solutions"))


def generate_sitemap(blog_items: list[dict[str, str]], solution_items: list[dict[str, str]]) -> None:
    urls = [
        "index.html", "pricing.html", "skills.html", "download/", "checkout.html", "blog.html", "blog-series.html",
        "api.html", "solutions.html", "license.html", "help.html", "security.html", "privacy.html",
        "terms.html", "roadmap.html", "recover.html",
    ]
    urls += [f"blog/{item['file']}" for item in blog_items]
    urls += [f"solutions/{item['file']}" for item in solution_items]
    body = '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
    body += "\n".join(f"  <url><loc>/{url}</loc></url>" for url in urls)
    body += "\n</urlset>\n"
    write(NEW / "sitemap.xml", body)


def main() -> None:
    blog_items = generate_blog_articles()
    existing_files = {item["file"] for item in blog_items}
    blog_items.extend(item for item in EXTRA_CURRENT_BLOGS if item["file"] not in existing_files)
    generate_blog_index(blog_items)
    generate_blog_series(blog_items)
    solution_items = generate_solutions()
    generate_solutions_index(solution_items)
    generate_sitemap(blog_items, solution_items)
    print(f"Generated {len(blog_items)} blog articles and {len(solution_items)} solution pages.")


if __name__ == "__main__":
    main()
