# OpsProbe Static Site Rebuild

This is the clean rebuild target for the Cloud Waste Scanner public beta website.

## Scope

- Static homepage, public beta, download, help, blog, security, privacy, and terms pages.
- Free public beta distribution configured in `js/site-config.js`.
- No self-hosted website backend required for the public marketing and feedback path.
- No Community, Pro, Team, Enterprise, GitHub, or Skills navigation in the new public funnel.

## Local Preview

```bash
python3 -m http.server 19080 --directory opsprobe-site
```

Open:

```text
http://127.0.0.1:19080/
```

For LAN review, replace `127.0.0.1` with this machine's LAN IP.

## Deployment

Use `opsprobe-site/` as the static hosting root. Vercel, Cloudflare Pages, Netlify, and object-storage static hosting all work because the site has no build step.

## Vercel Setup

- Import the repository in Vercel.
- Set the project root directory to `opsprobe-site`.
- Leave build command empty.
- Leave output directory as `.`.
- Add `cloud-waste-scanner.com` and `www.cloud-waste-scanner.com` as project domains.
- Keep large release packages under `/download/files/` only if they fit the hosting limits; otherwise point the download URLs to GitHub Releases, R2, or another static asset host.

## Public Beta Setup

The website currently avoids checkout and paid packaging. The primary funnel is:

- Download Free
- Run a local read-only scan
- Send first-scan feedback

Keep `commerce.provider` as `public-beta` in `js/site-config.js` until the product has enough real usage feedback to define paid packaging.

Recommended domain split:

- `cloud-waste-scanner.com` and `www.cloud-waste-scanner.com` for the marketing site
- `download.cloud-waste-scanner.com` or `dl.cloud-waste-scanner.com` for release assets
- `admin.cloud-waste-scanner.com` for the backend UI
- `api.cloud-waste-scanner.com` for backend endpoints

Before publishing:

- Keep public beta messaging aligned across `index.html`, `pricing.html`, `checkout.html`, `license.html`, and `recover.html`.
- Replace download URLs and SHA256 values after the three platform builds are uploaded.
- Keep the admin backend outside this static site. Use a separate low-cost or on-demand backend only if feedback automation, download telemetry, or future license automation becomes necessary.
- Keep `license.html` and `terms.html` aligned with any beta access or future paid policy changes.
