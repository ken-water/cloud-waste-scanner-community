# API And Inputs

## Local API priority

Use the local API first when the user is running the Community desktop app with local API enabled.

Preferred endpoints:

- `GET /status`
- `GET /v1/findings?limit=200&envelope=true`
- `GET /v1/scans?limit=50&envelope=true`
- `GET /v1/reports?limit=50&envelope=true`
- `GET /v1/openapi.json`

Bearer token:

- Send `Authorization: Bearer <token>` when provided.

## Export fallback

When local API is not reachable, accept:

- JSON exports from CWS
- CSV findings exports
- plain text notes copied from reports or handoff packs

## Normalized bundle

`scripts/build_context.py` emits a JSON object with this high-level shape:

```json
{
  "source": "api",
  "generated_at": "2026-06-11T00:00:00Z",
  "status": {},
  "summary": {
    "findings": 0,
    "estimated_monthly_waste": 0.0,
    "currencies": ["USD"],
    "top_providers": [],
    "top_resource_types": [],
    "top_accounts": []
  },
  "findings": [],
  "scans": [],
  "reports": [],
  "notes": []
}
```

## Interpretation guidance

- Treat `estimated_monthly_waste` as projected impact, not booked savings.
- Treat provider and resource-type ranking as prioritization hints.
- Treat missing owners as an execution risk worth calling out explicitly.
- If multiple currencies appear, do not sum across currencies in prose unless the source already normalized them.
