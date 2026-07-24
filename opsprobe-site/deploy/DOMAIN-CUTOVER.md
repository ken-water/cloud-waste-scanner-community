# Domain Cutover Plan

This server can be used as the new origin if the public surface stays split by purpose.

## Recommended host split

- `cloud-waste-scanner.com` or `www.cloud-waste-scanner.com`: static marketing site
- `download.cloud-waste-scanner.com` or `dl.cloud-waste-scanner.com`: optional static download mirror
- `admin.cloud-waste-scanner.com`: backend/admin UI only
- `api.cloud-waste-scanner.com`: backend API only

## Why this matters

- Keeps the public site static and cheap to serve.
- Prevents the blog/download pages from inheriting backend failures.
- Makes rollback easier because each surface can move independently.

## DNS

- Point the chosen hostnames to this server's public IP.
- Use a short TTL before the first cutover.
- Do not move all surfaces at once unless the backend is already stable.

## TLS

- Terminate TLS at Nginx on this machine.
- Use certbot or an existing certificate workflow.
- Install separate certs or one wildcard cert for all subdomains.

## Cutover order

1. Bring up the static site on `www`.
2. Verify blog, pricing, download, and checkout pages.
3. Add `download` if you want a separate asset host.
4. Move `admin` and `api` only after the public site is stable.

## Rollback

- Keep the old DNS target alive until the new origin passes download and checkout tests.
- If the cutover fails, restore DNS first; do not debug on a broken public hostname.

## Notes

- The public site should not depend on `/api` for core page rendering.
- Gumroad should stay hosted, not self-hosted.
- Build artifacts should be published before any download links are made live.
- Set `PUBLIC_SITE_BASE_URL` on the admin backend after the DNS cutover, otherwise generated links may point to the old website.
