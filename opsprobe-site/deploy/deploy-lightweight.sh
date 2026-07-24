#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SITE_DIR="$ROOT_DIR/opsprobe-site"
NGINX_SRC="$SITE_DIR/deploy/nginx-lightweight.conf"
NGINX_DEST="/etc/nginx/conf.d/cws.conf"
BACKEND_ENV_SRC="$SITE_DIR/deploy/cws-admin-backend.env.example"
BACKEND_ENV_DEST="/etc/cws-admin-backend.env"
BACKEND_SERVICE_SRC="$SITE_DIR/deploy/cws-admin-backend.service"
BACKEND_SERVICE_DEST="/etc/systemd/system/cws-admin-backend.service"

usage() {
  cat <<'EOF'
Usage:
  ./opsprobe-site/deploy/deploy-lightweight.sh check
  ./opsprobe-site/deploy/deploy-lightweight.sh install-nginx
  ./opsprobe-site/deploy/deploy-lightweight.sh install-backend-service
  ./opsprobe-site/deploy/deploy-lightweight.sh verify-local

Notes:
  - DNS changes are intentionally not automated.
  - TLS is intentionally not automated; add certbot or your certificate workflow after HTTP works.
EOF
}

require_file() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    echo "missing file: $path" >&2
    exit 1
  fi
}

require_dir() {
  local path="$1"
  if [[ ! -d "$path" ]]; then
    echo "missing directory: $path" >&2
    exit 1
  fi
}

check_site() {
  require_dir "$SITE_DIR"
  require_file "$SITE_DIR/index.html"
  require_file "$SITE_DIR/pricing.html"
  require_file "$SITE_DIR/checkout.html"
  require_file "$SITE_DIR/blog.html"
  require_file "$SITE_DIR/download/index.html"
  require_file "$SITE_DIR/js/site-config.js"
  require_file "$NGINX_SRC"

  if ! command -v nginx >/dev/null 2>&1; then
    echo "nginx is not installed or not in PATH" >&2
    exit 1
  fi

  echo "site root: $SITE_DIR"
  echo "nginx source: $NGINX_SRC"
  echo "check passed"
}

test_nginx_config() {
  local tmpdir
  tmpdir="$(mktemp -d)"
  mkdir -p "$tmpdir/logs" "$tmpdir/client_body"
  perl -pe 's/listen 80;/listen 18080;/g; s/listen \[::\]:80;/listen [::]:18080;/g' "$NGINX_SRC" > "$tmpdir/cws.conf"
  cat > "$tmpdir/nginx.conf" <<EOF
pid $tmpdir/nginx.pid;
events {}
http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;
    access_log $tmpdir/logs/access.log;
    error_log $tmpdir/logs/error.log;
    client_body_temp_path $tmpdir/client_body;
    include $tmpdir/cws.conf;
}
EOF
  nginx -t -p "$tmpdir" -c "$tmpdir/nginx.conf"
}

install_nginx() {
  check_site
  test_nginx_config
  sudo cp "$NGINX_SRC" "$NGINX_DEST"
  sudo nginx -t
  sudo systemctl reload nginx
  echo "installed nginx config: $NGINX_DEST"
}

install_backend_service() {
  require_file "$BACKEND_ENV_SRC"
  require_file "$BACKEND_SERVICE_SRC"

  if [[ ! -f "$BACKEND_ENV_DEST" ]]; then
    sudo cp "$BACKEND_ENV_SRC" "$BACKEND_ENV_DEST"
    echo "created env template: $BACKEND_ENV_DEST"
    echo "edit $BACKEND_ENV_DEST before starting the backend service"
  else
    echo "env already exists, not overwritten: $BACKEND_ENV_DEST"
  fi

  sudo cp "$BACKEND_SERVICE_SRC" "$BACKEND_SERVICE_DEST"
  sudo systemctl daemon-reload
  echo "installed systemd service: $BACKEND_SERVICE_DEST"
}

verify_local() {
  for host in cloud-waste-scanner.com www.cloud-waste-scanner.com download.cloud-waste-scanner.com dl.cloud-waste-scanner.com; do
    echo "checking http://127.0.0.1/ with Host: $host"
    curl -fsSI -H "Host: $host" http://127.0.0.1/ | sed -n '1,5p'
  done
}

cmd="${1:-}"
case "$cmd" in
  check)
    check_site
    test_nginx_config
    ;;
  install-nginx)
    install_nginx
    ;;
  install-backend-service)
    install_backend_service
    ;;
  verify-local)
    verify_local
    ;;
  *)
    usage
    exit 1
    ;;
esac
