#!/usr/bin/env python3
from __future__ import annotations

import argparse
import http.client
import os
import posixpath
import ssl
import sys
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit


DEFAULT_ROOT = "/home/ken/tmp-cws-test"
DEFAULT_PROXY_BASE = "https://cloud-waste-scanner.com"
PROXY_PREFIXES = ("/api/",)
HOP_BY_HOP_HEADERS = {
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
    "accept-encoding",
}


def strip_secure_from_cookie(cookie: str) -> str:
    parts = [part.strip() for part in cookie.split(";")]
    filtered = [part for part in parts if part.lower() != "secure"]
    return "; ".join(filtered)


class PreviewHandler(SimpleHTTPRequestHandler):
    server_version = "CWSPreview/1.0"

    def __init__(self, *args, directory: str, proxy_base: str, **kwargs):
        self.preview_directory = directory
        self.proxy_base = proxy_base.rstrip("/")
        super().__init__(*args, directory=directory, **kwargs)

    def do_GET(self) -> None:
        if self.path == "/help.html":
            self.send_response(302)
            self.send_header("Location", "/documentation.html")
            self.end_headers()
            return
        if self._should_proxy():
            self._proxy_request()
            return
        super().do_GET()

    def do_HEAD(self) -> None:
        if self.path == "/help.html":
            self.send_response(302)
            self.send_header("Location", "/documentation.html")
            self.end_headers()
            return
        if self._should_proxy():
            self._proxy_request()
            return
        super().do_HEAD()

    def do_POST(self) -> None:
        if self._should_proxy():
            self._proxy_request()
            return
        self.send_error(405, "Method Not Allowed")

    def do_PUT(self) -> None:
        if self._should_proxy():
            self._proxy_request()
            return
        self.send_error(405, "Method Not Allowed")

    def do_DELETE(self) -> None:
        if self._should_proxy():
            self._proxy_request()
            return
        self.send_error(405, "Method Not Allowed")

    def do_OPTIONS(self) -> None:
        if self._should_proxy():
            self._proxy_request()
            return
        self.send_response(204)
        self.send_header("Allow", "GET, HEAD, POST, PUT, DELETE, OPTIONS")
        self.end_headers()

    def _should_proxy(self) -> bool:
        return any(self.path.startswith(prefix) for prefix in PROXY_PREFIXES)

    def _proxy_request(self) -> None:
        target = urlsplit(f"{self.proxy_base}{self.path}")
        conn_cls = http.client.HTTPSConnection if target.scheme == "https" else http.client.HTTPConnection
        context = ssl.create_default_context() if target.scheme == "https" else None
        conn = conn_cls(target.netloc, context=context, timeout=30) if context else conn_cls(target.netloc, timeout=30)

        body = None
        if self.command in {"POST", "PUT", "PATCH"}:
            length = int(self.headers.get("Content-Length", "0") or "0")
            body = self.rfile.read(length) if length > 0 else b""

        headers = {}
        for key, value in self.headers.items():
            lower = key.lower()
            if lower in HOP_BY_HOP_HEADERS:
                continue
            headers[key] = value
        headers["Host"] = target.netloc
        headers["X-Forwarded-Proto"] = "http"
        headers["X-Forwarded-Host"] = self.headers.get("Host", "")
        headers["X-Forwarded-For"] = self.client_address[0]

        try:
            conn.request(self.command, target.path + (f"?{target.query}" if target.query else ""), body=body, headers=headers)
            resp = conn.getresponse()
            payload = resp.read()
        except Exception as exc:
            self.send_error(502, f"Proxy request failed: {exc}")
            return
        finally:
            conn.close()

        self.send_response(resp.status, resp.reason)
        for key, value in resp.getheaders():
            lower = key.lower()
            if lower in HOP_BY_HOP_HEADERS:
                continue
            if lower == "set-cookie":
                self.send_header(key, strip_secure_from_cookie(value))
                continue
            self.send_header(key, value)
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(payload)

    def translate_path(self, path: str) -> str:
        path = path.split("?", 1)[0].split("#", 1)[0]
        trailing_slash = path.rstrip().endswith("/")
        try:
            path = posixpath.normpath(os.path.join("/", path))
        except TypeError:
            path = posixpath.normpath(path)
        words = [word for word in path.split("/") if word]
        full_path = self.preview_directory
        for word in words:
            if os.path.dirname(word) or word in (os.curdir, os.pardir):
                continue
            full_path = os.path.join(full_path, word)
        if trailing_slash:
            full_path += "/"
        return full_path


def main() -> int:
    parser = argparse.ArgumentParser(description="Serve the local CWS preview with API proxy support.")
    parser.add_argument("--root", default=DEFAULT_ROOT, help="Directory to serve.")
    parser.add_argument("--port", type=int, default=18081, help="Port to listen on.")
    parser.add_argument("--proxy-base", default=DEFAULT_PROXY_BASE, help="Remote base URL for /api proxying.")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    if not root.exists():
        print(f"preview root does not exist: {root}", file=sys.stderr)
        return 1

    def handler(*handler_args, **handler_kwargs):
        return PreviewHandler(
            *handler_args,
            directory=str(root),
            proxy_base=args.proxy_base,
            **handler_kwargs,
        )

    httpd = ThreadingHTTPServer(("0.0.0.0", args.port), handler)
    print(f"Serving {root} on http://0.0.0.0:{args.port} with proxy {args.proxy_base}", flush=True)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        httpd.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
