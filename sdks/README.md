# Cloud Waste Scanner SDKs

These SDKs target the local-first CWS API exposed by the desktop app.

Current coverage:

- OpenAPI discovery: `GET /v1/openapi.json`
- Compatibility policy: `GET /v1/meta/compatibility`
- Cursor pagination envelopes for scans, findings, reports, events, and history
- HMAC webhook signature verification
- Kubernetes scan helpers

SDKs intentionally call the local API. They do not require a hosted CWS service.

## Python

See `sdks/python`.

## TypeScript

See `sdks/typescript`.
