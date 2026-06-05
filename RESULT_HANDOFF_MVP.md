# Result Handoff MVP (Stage 1)

## Goal
Enable teams to hand off scan findings between people and org units with minimal friction, while keeping local-first control and auditability.

## Scope (MVP)
- In-app weekly handoff package generation from current filtered scope.
- Share package includes:
  - Summary text (for chat/email copy).
  - CSV export (machine-readable triage list).
  - PDF export (human-readable review packet).
- Scope-aware handoff:
  - Global scope (all findings in current window).
  - Filtered scope (provider/account/org unit/owner/current query).
- Receiver path:
  - No-install path: consume PDF/CSV in chat/email.
  - App path: import CSV IDs into local app filter and continue lifecycle actions.

## Out of Scope (MVP)
- Public URL share service.
- External identity federation.
- Real-time collaborative editing.

## UX Entry Points
1. Scan Results
- Add `Create Handoff Pack` action near existing export controls.
- Reuse current filter state as default scope.

2. Governance
- Keep `Copy Weekly Pack`, `PDF`, `CSV` and include org lifecycle rows (implemented).

3. Settings > Org & Owners
- Existing source of truth for org and owner assignment.

## Permission Model (MVP)
- Local desktop authority model (same as current app runtime).
- Soft roles in payload for downstream handling:
  - `exec`: summary-level audience.
  - `owner`: actionable subset audience.
  - `audit`: full evidence audience.
- Sensitive field handling (default): masked identifiers in summary text; full IDs remain in CSV/PDF only if operator enables `include sensitive fields`.

## Data Contract (Handoff Manifest)
Create a sidecar JSON next to exported files:

```json
{
  "version": "1.0",
  "generated_at": "2026-05-27T00:00:00Z",
  "window_days": 30,
  "scope": {
    "type": "filtered",
    "provider": ["aws"],
    "accounts": ["prod-account-a"],
    "org_units": ["platform"],
    "owners": ["owner_123"]
  },
  "metrics": {
    "findings": 128,
    "identified_savings_monthly": 7800,
    "estimated_co2e_kg_monthly": 420.4
  },
  "artifacts": {
    "summary_txt": "governance_weekly_pack_2026-05-27.txt",
    "findings_csv": "cloud_waste_report_2026-05-27.csv",
    "report_pdf": "governance_weekly_pack_2026-05-27.pdf"
  }
}
```

## API / Command Evolution (Phase-Next)
- `POST /v1/handoff/packages` create handoff metadata locally.
- `GET /v1/handoff/packages` list packages.
- `GET /v1/handoff/packages/:id` details and artifact pointers.
- Tauri commands:
  - `create_handoff_package`
  - `list_handoff_packages`

## Acceptance Criteria
1. Operator can generate handoff artifacts from current scope in <= 3 clicks.
2. Generated pack always contains scope metadata and generated timestamp.
3. Governance export includes org lifecycle summary in both PDF and CSV.
4. Receiver without app can read actionable context from summary + PDF.
5. Receiver with app can map CSV rows back to lifecycle items using resource_id.

## Operational Notes
- Keep artifacts local by default.
- If teams need link-based sharing later, add optional self-hosted handoff endpoint and signed temporary URLs.
- All handoff generation should append audit log records (`event=handoff_package_created`).
