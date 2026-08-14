# Cloud Waste Scanner GEO Analysis

## GEO Readiness Score: 87/100

The site is strong on technical accessibility, structured data, and first-scan positioning. The latest round improves entity consistency and gives AI search systems clearer citation targets for product value, pricing, help, and security. The remaining gap is mostly off-site authority and original evidence that outside sources can cite.

## Platform Breakdown

- Google AI Overviews: 90/100
- ChatGPT web search: 84/100
- Perplexity: 82/100

Why:
- Google can read the static HTML, structured data, concise homepage answers, and stronger brand graph.
- ChatGPT and Perplexity now have clearer `llms.txt` citation guidance and more self-contained answer blocks.
- Off-site mentions and original research are still modest, which limits confidence for competitive and market-level answers.

## AI Crawler Access Status

Current `robots.txt` is open except for `/archive/`.

Allowed by default:
- GPTBot
- OAI-SearchBot
- ClaudeBot
- PerplexityBot
- CCBot
- Google-Extended
- Google-CloudVertexBot

Notes:
- The site does not currently use crawler-specific blocks.
- That is good for visibility, but it also means training crawlers are not separated from search crawlers.
- If you want tighter control later, add explicit user-agent rules for training crawlers only.

## llms.txt Status

Status: present and stronger after update.

What is good:
- Markdown format with an H1.
- Clear access and usage intent.
- Strong boundary language about local scans and customer data.
- Good page map for product, pricing, help, blog, security, and API.
- A dedicated "best pages for citation" section.
- A short list of product facts that AI systems can quote directly.

What to improve later:
- Keep the page updated when pricing or product positioning changes.

## Brand Mention Analysis

Current external signals are limited but real.

Found in public web search:
- Reddit: a `r/devops` thread about an update to Cloud Waste Scanner.
- LinkedIn: multiple posts from the founder/profile describing launches and article shares.

Not found in this search:
- Wikipedia
- YouTube

Interpretation:
- LinkedIn is the strongest external brand surface right now.
- Reddit exists, but the signal is still early and niche.
- No Wikipedia or YouTube presence means the broader entity graph is still thin.

## Passage-Level Citability

Best current citation block:
- Homepage section "What is Cloud Waste Scanner?" is about 158 words and sits in the first third of the page.

Other good candidates:
- Help and security answer blocks.
- Security whitepaper Part 1 trust-boundary answer.
- FAQ answers on the homepage.
- Short comparison verdict sections on the major industry pages.

What still hurts citability:
- Some remaining whitepaper and comparison pages are still long-form and need page-specific answer blocks.
- The site still lacks an original benchmark, dataset, or field study that would give AI systems unique evidence to cite.

Target:
- Keep the best answer blocks in the 134 to 167 word range.
- Put the direct answer in the first 40 to 60 words of the section.

## Server-Side Rendering Check

Result: strong.

The public site is static HTML with light JS. Core content is present in the HTML, not assembled only in the browser. That is good for AI crawlers and for AI answer extraction.

Observed patterns:
- Main content is server-rendered or prebuilt.
- JavaScript is mostly for nav, analytics, filters, and UI helpers.
- Search-relevant content does not depend on client-side rendering.

## Top 5 Highest-Impact Changes

1. Add more external entity links with `sameAs` on the organization and author objects when verified public profiles are available, especially LinkedIn and YouTube.
2. Publish one original benchmark, dataset, or field study that gives AI systems something unique to cite.
3. Rewrite the first 1 to 2 paragraphs of the strongest whitepaper and comparison pages into direct-answer blocks.
4. Add a few more short FAQ sections on pricing, help, security, and the main product pages.
5. Keep refreshing dates and update timestamps on the pages that already rank or get cited.

## Schema Recommendations

Already good:
- Organization
- SoftwareApplication
- FAQPage
- Article
- BreadcrumbList
- ItemList

Still missing or thin:
- `sameAs` links beyond GitHub, because only verified current profiles should be added
- fuller author entity data
- richer author bio pages or profile pages
- a more explicit WebSite graph for the brand

Best next move:
- Connect the site entity to LinkedIn, YouTube, and other public founder/product profiles once the exact URLs are confirmed.

## Implemented Changes

- Added Organization and WebSite graph nodes across current public HTML pages outside the archive.
- Added GitHub and sponsor `sameAs` links to Cloud Waste Scanner organization entities and Ken Rich author entities.
- Added a citation-focused quick answer to the local-first security whitepaper entry page.
- Expanded `llms.txt` with best citation pages and product facts to quote.
- Kept `opsprobe.com` explicitly separate from `cloud-waste-scanner.com`.

## Content Reformatting Suggestions

Rewrite these types of passages:
- Whitepaper openings that explain the topic before answering it
- Comparison pages that delay the verdict
- Guide pages that bury the first useful action below too much framing

Use this pattern:
- one sentence definition
- one sentence on why it matters
- one short list or table
- one clear next step

Keep:
- specific numbers
- dates
- ownership language
- export and evidence language
- local-first boundary language
