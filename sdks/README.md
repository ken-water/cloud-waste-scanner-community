# Cloud Waste Scanner SDKs

These SDKs target the local-first CWS API exposed by the desktop app.

Current coverage:

- OpenAPI discovery: `GET /v1/openapi.json`
- Compatibility policy: `GET /v1/meta/compatibility`
- Cursor pagination envelopes for scans, findings, reports, events, and history
- HMAC webhook signature verification
- Kubernetes scan helpers

SDKs intentionally call the local API. They do not require a hosted CWS service.

## Use with the report explainer skill

If you want to turn local API data into readable weekly summaries instead of building your own formatter, use:

- `skills/cws-report-explainer`

The expected flow is:

1. call the local API from the desktop app
2. build a normalized JSON bundle with `skills/cws-report-explainer/scripts/build_context.py`
3. feed that bundle to the skill for operator, finance, or executive output

## Python

See `sdks/python`.

## TypeScript

See `sdks/typescript`.
