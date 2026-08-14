# Cloud Waste Scanner SEO Audit

Overall score: **89/100**

Business type: Local-first desktop SaaS for cloud waste discovery and cost reduction

## Executive Summary
- Solution detail pages were highly repetitive and had to be deindexed
- Production clean URLs are now normalized and canonicalized
- Template repetition was concentrated in guide and comparison endings
- 38 solution detail pages were deindexed to protect crawl quality
- llms.txt now has explicit access and usage intent

## Quick Wins
- Keep the /solutions collection page as the indexable entry point for resource playbooks.
- Add one page-specific summary block to the strongest remaining comparison and whitepaper pages.
- Compress any new hero or article screenshots before publishing them.
- Continue writing blog endings as unique next steps instead of shared boilerplate.

## Technical SEO (91/100)
What works:
- robots.txt allows crawling sitewide and blocks only /archive/.
- sitemap.xml now contains 86 URLs and excludes the 38 templated solution detail pages.
- Canonical URLs are present on all sampled core pages.
Findings:
- **High** Solution detail pages were highly repetitive and had to be deindexed: 38 solution detail pages shared a near-template body and would have diluted crawl budget and index quality. They are now marked noindex, follow and removed from sitemap. Recommendation: Keep the pages for users, but let the /solutions collection page carry the indexation weight. Refresh the collection page when adding new solution pages.
- **Low** Clean URL routing is now consistent in production: `/pricing.html`, `/download/`, and similar legacy forms redirect to the canonical clean routes. Recommendation: Keep publishing only clean URLs in internal links and sitemap entries.

## Content Quality (79/100)
What works:
- 6 page types are represented and the homepage, blog index, help, solutions, pricing, and download pages all carry clear product intent.
- Title and description uniqueness is high across core pages; most duplicate text came from templates, not metadata.
Findings:
- **High** Template repetition was concentrated in guide and comparison endings: Blog cleanup removed repeated endings from 69 blog posts, but the most repetitive content pattern was the copied closing structure, not the article bodies themselves. Recommendation: Keep writing each article ending as a page-specific next step. Avoid one universal “execution paths” or “declarative conclusions” block across multiple posts.
- **Medium** The remaining whitepaper repetition is intentional but still highly similar: The security and technical whitepaper series still shares repeated scaffold phrases like ordered reading sequences and checklist language. Those blocks help series navigation but also create cross-page similarity. Recommendation: Keep the series structure, but make each part introduce a distinct problem, artifact, or decision rule early in the page so the page is not only a series waypoint.
- **Low** Repeated CTA endings have been cleaned up: article CTA headings and body copy are now page-specific across the blog set. Recommendation: Keep future CTA copy tied to the article topic instead of reusing one global line.
- **Low** The site now leads with clear cost value, which is strong for intent matching: Homepage copy consistently says the product finds cloud waste and helps cut costs fast. That is a strong commercial intent match and reduces topical drift. Recommendation: Preserve this framing and continue using savings-language first, technical detail second.

## On-Page SEO (90/100)
What works:
- 125 unique titles across 126 pages is acceptable for a content-heavy site.
- 125 unique meta descriptions and 125 unique H1s show strong page-level differentiation.
Findings:
- **Medium** Some solution detail pages have generic “Review X in Y” titles: The titles are descriptive, but the pattern repeats 38 times. Because those pages are now noindexed, the repetition is acceptable for users but no longer needs to drive search intent. Recommendation: Keep these titles for usability, but if any solution page becomes indexable later, rewrite it around a more distinctive task or use case.
- **Low** The blog index is strong but still broad: The blog index mixes product guides, founder notes, whitepapers, comparisons, and incident stories. This is useful for navigation but can blur topical clustering for search. Recommendation: Keep the current structure, but reinforce cluster pages for product guides, whitepapers, and comparisons with short intro copy that explains the difference between sections.

## Schema / Structured Data (92/100)
What works:
- Homepage includes Organization, SoftwareApplication, and FAQPage schema.
- Blog articles carry Article schema with Person author/reviewer and update dates.
- Solutions pages use BreadcrumbList + FAQPage schema and the collection page uses CollectionPage + ItemList.
Findings:
- **Info** Schema coverage is strong, but solution pages are now noindex: The schema is valid and helpful, but the detail pages are intentionally not meant to rank. Their structured data now serves user understanding rather than indexation. Recommendation: Leave the schema in place; it still helps crawlers and internal reuse even when pages are noindexed.
- **Low** Some blog pages would benefit from more explicit how-to / FAQ substructures: Many blog articles are useful, but not every page has a Q&A or direct-answer block near the top. Recommendation: Add one short answer block or FAQ section to the highest-value guides and comparison pages, not every page.

## Performance (CWV) (81/100)
What works:
- The site is static HTML/CSS with light JS, which is structurally favorable for LCP and INP.
- Most heavy content comes from optimized image assets rather than client-side rendering.
Findings:
- **Medium** Large hero and report images are the main performance risk: The homepage and several article pages use large preview images and full-width screenshots. That is visually strong, but image weight is the main likely LCP driver. Recommendation: Keep the visuals, but continue compressing new images aggressively and prefer modern formats with explicit dimensions.
- **Low** Third-party scripts are minimal but present on every page: The site loads Vercel insights and shared navigation scripts globally. This is a light footprint, but it still adds network requests on every page. Recommendation: Keep the shared scripts unless they become measurable bottlenecks; if you need to shave more time later, defer nonessential analytics on content-only pages.

## AI Search Readiness (90/100)
What works:
- llms.txt is now Markdown with an H1 and explicit usage intent.
- Public pages clearly state the product, its local-first boundary, and the value proposition of fast cost savings.
- Author, reviewer, and update dates are visible on blog pages.
Findings:
- **Medium** AI-citable passages are now better, but some pages still share too much wording: Even after cleanup, a few repeated phrases remain across series pages and comparison pages. This is acceptable for navigation, but it lowers distinct passage value for AI extraction. Recommendation: Keep pushing each article toward a unique first paragraph and one page-specific takeaway that is not copied elsewhere.
- **Low** The site is now better aligned to fast savings queries than generic cloud-governance queries: The strongest language now centers on cutting costs fast, local scanning, and review evidence. That is good for query intent and AI answer extraction. Recommendation: Continue phrasing new content around “find waste” and “cut costs fast” rather than abstract governance language first.

## Images (84/100)
What works:
- Hero and report images are descriptive and support the product story.
- Most key images include alt text.
Findings:
- **Medium** Image asset quality is mixed across old and new content: The homepage imagery and recent product screenshots are strong, but older article visuals and series images vary in sharpness and style consistency. Recommendation: Standardize future blog art direction and keep a strict image compression / resolution policy for all new assets.
- **Low** A handful of pages still rely on screenshot-heavy layouts: This is acceptable for a desktop product, but it can slow pages if new screenshots are added without optimization. Recommendation: Use explicit width/height on images and audit new uploads before publishing.

## Crawl Metrics
- Pages found: 126
- Blog detail pages: 69
- Solution detail pages: 38
- Sitemap URLs: 86
- Solution detail URLs in sitemap: 0
- Missing H1 pages: 0 public pages, plus 1 verification file that is intentionally not indexable
- Multiple H1 pages: 0
- Pages with noindex: 39
- Broken internal links detected: 0
- Duplicate content groups over threshold: 0 public clusters above threshold

## Action Plan
### Phase 1: Critical Fixes (Week 1)
- Keep solution detail pages noindexed and out of sitemap.
- Audit any future indexable solution pages before enabling them.

### Phase 2: High-Impact Improvements (Weeks 2-3)
- Rewrite the best comparison pages with more distinct opening paragraphs.
- Add one more short answer block to the highest-intent guides.

### Phase 3: Content & Authority (Month 2)
- Continue trimming repeated sentence patterns in whitepaper and comparison series.
- Add more author-specific context or editorial notes to high-value posts.

### Phase 4: Monitoring & Iteration (Ongoing)
- Monitor sitemap coverage, indexation, and any new duplicate content clusters monthly.
- Review title, description, and H1 uniqueness whenever large batches of pages are added.

## Notes
- The audit treats the 38 templated solution detail pages as intentional user-facing pages that should not compete in organic search.
- The site is structurally strong for technical SEO, so most remaining gains are content distinctiveness and crawl-surface hygiene.
- Public pages now have unique titles, descriptions, and H1s across the crawl set.
