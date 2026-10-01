# Lightweight Deployment

Use Nginx only.

## Services

- Static site: `opsprobe-site/`
- Backend: `127.0.0.1:3100`

## Domain map

- `cloud-waste-scanner.com` or `www.cloud-waste-scanner.com` -> static site
- `download.cloud-waste-scanner.com` or `dl.cloud-waste-scanner.com` -> static download site
- `cloud-waste-scanner.com/admin/` -> backend UI
- `api.cloud-waste-scanner.com` -> backend

## Backend env

Set the backend to bind locally only:

```bash
ADMIN_BIND=127.0.0.1:3100
ADMIN_DATABASE_URL=...
PUBLIC_SITE_BASE_URL=https://cloud-waste-scanner.com
LEGACY_ADMIN_BASE_URL=https://admin.cloud-waste-scanner.com
```

Use `deploy/cws-admin-backend.env.example` as the template for `/etc/cws-admin-backend.env`.

Use `deploy/cws-admin-backend.service` as the optional systemd template if the admin/API backend needs to stay online.

## Nginx steps

1. Run `./opsprobe-site/deploy/deploy-lightweight.sh check`.
2. Run `./opsprobe-site/deploy/deploy-lightweight.sh install-nginx`.
3. Run `./opsprobe-site/deploy/deploy-lightweight.sh verify-local`.
4. Add TLS later with certbot or your preferred certificate workflow.

Manual equivalent:

```bash
sudo cp opsprobe-site/deploy/nginx-lightweight.conf /etc/nginx/conf.d/cws.conf
sudo nginx -t
sudo systemctl reload nginx
```

Optional backend service:

```bash
./opsprobe-site/deploy/deploy-lightweight.sh install-backend-service
sudo editor /etc/cws-admin-backend.env
sudo systemctl enable --now cws-admin-backend
```

DNS cutover:

```text
cloud-waste-scanner.com      A/AAAA -> this server
www.cloud-waste-scanner.com  A/AAAA -> this server
download.cloud-waste-scanner.com or dl.cloud-waste-scanner.com -> this server
admin.cloud-waste-scanner.com -> this server
api.cloud-waste-scanner.com -> this server
```

## Notes

- Keep the public site static.
- Keep Gumroad hosted.
- Keep downloads as static files or release assets.
