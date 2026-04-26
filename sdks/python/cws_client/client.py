"""Small dependency-free client for the Cloud Waste Scanner local API."""

from __future__ import annotations

import hashlib
import hmac
import json
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Any, Mapping


class CwsClientError(RuntimeError):
    """Raised when the local API returns a non-2xx response."""

    def __init__(self, status: int, message: str, payload: Any | None = None) -> None:
        super().__init__(f"CWS API error {status}: {message}")
        self.status = status
        self.payload = payload


def verify_webhook_signature(
    secret: str,
    timestamp: str | int,
    body: str | bytes,
    signature: str,
    *,
    tolerance_seconds: int = 300,
    now: int | None = None,
) -> bool:
    """Verify an X-CWS-Signature value.

    CWS signs the exact raw JSON body as:
    HMAC-SHA256(secret, "<timestamp>.<raw_json_body>")
    and sends the result as "sha256=<hex>".
    """

    if not secret or not signature.startswith("sha256="):
        return False
    try:
        ts = int(timestamp)
    except (TypeError, ValueError):
        return False
    current = int(time.time() if now is None else now)
    if tolerance_seconds >= 0 and abs(current - ts) > tolerance_seconds:
        return False
    raw_body = body.decode("utf-8") if isinstance(body, bytes) else body
    expected = hmac.new(
        secret.encode("utf-8"),
        f"{ts}.{raw_body}".encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()
    return hmac.compare_digest(f"sha256={expected}", signature)


@dataclass(frozen=True)
class CwsClient:
    """Client for the local-first Cloud Waste Scanner HTTP API."""

    base_url: str = "http://127.0.0.1:43177"
    token: str | None = None
    timeout: float = 30.0

    def _url(self, path: str, query: Mapping[str, Any] | None = None) -> str:
        normalized = path if path.startswith("/") else f"/{path}"
        url = self.base_url.rstrip("/") + normalized
        if not query:
            return url
        clean = {
            key: value
            for key, value in query.items()
            if value is not None and value != ""
        }
        if not clean:
            return url
        return f"{url}?{urllib.parse.urlencode(clean)}"

    def request(
        self,
        method: str,
        path: str,
        *,
        query: Mapping[str, Any] | None = None,
        json_body: Mapping[str, Any] | None = None,
    ) -> Any:
        body = None
        headers = {"Accept": "application/json"}
        if json_body is not None:
            body = json.dumps(json_body).encode("utf-8")
            headers["Content-Type"] = "application/json"
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        req = urllib.request.Request(
            self._url(path, query),
            data=body,
            headers=headers,
            method=method.upper(),
        )
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as response:
                payload = response.read().decode("utf-8")
                return json.loads(payload) if payload else None
        except urllib.error.HTTPError as exc:
            payload_text = exc.read().decode("utf-8")
            try:
                payload = json.loads(payload_text) if payload_text else None
            except json.JSONDecodeError:
                payload = payload_text
            message = (
                payload.get("error")
                if isinstance(payload, dict) and isinstance(payload.get("error"), str)
                else exc.reason
            )
            raise CwsClientError(exc.code, str(message), payload) from exc

    def status(self) -> Any:
        return self.request("GET", "/status")

    def openapi(self) -> Any:
        return self.request("GET", "/v1/openapi.json")

    def compatibility(self) -> Any:
        return self.request("GET", "/v1/meta/compatibility")

    def list_scans(
        self, *, cursor: str | None = None, limit: int = 50, envelope: bool = True
    ) -> Any:
        return self.request(
            "GET",
            "/v1/scans",
            query={"cursor": cursor, "limit": limit, "envelope": str(envelope).lower()},
        )

    def run_scan(self, payload: Mapping[str, Any]) -> Any:
        return self.request("POST", "/v1/scans", json_body=payload)

    def list_findings(
        self, *, cursor: str | None = None, limit: int = 50, envelope: bool = True
    ) -> Any:
        return self.request(
            "GET",
            "/v1/findings",
            query={"cursor": cursor, "limit": limit, "envelope": str(envelope).lower()},
        )

    def list_reports(
        self, *, cursor: str | None = None, limit: int = 50, envelope: bool = True
    ) -> Any:
        return self.request(
            "GET",
            "/v1/reports",
            query={"cursor": cursor, "limit": limit, "envelope": str(envelope).lower()},
        )

    def list_k8s_contexts(self) -> Any:
        return self.request("GET", "/v1/k8s/contexts")

    def run_k8s_scan(
        self, *, kubeconfig_path: str | None = None, kube_context: str | None = None
    ) -> Any:
        return self.request(
            "POST",
            "/v1/k8s/scans",
            json_body={
                "kubeconfig_path": kubeconfig_path,
                "kube_context": kube_context,
            },
        )
