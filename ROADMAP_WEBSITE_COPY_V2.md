# Roadmap Copy (Website Edition)

## Roadmap Direction

Cloud Waste Scanner is moving from static waste detection toward a full execution system: detect, assign, remediate, verify, and prevent recurrence.

We are prioritizing outcomes customers can use in weekly operations, not feature count.

## Now: Operational Closure

Focus: make findings operationally executable.

- Unified remediation lifecycle from detection to verified closure
- Owner assignment and SLA tracking for every actionable finding
- Risk-bucketed cleanup templates with rollback instructions
- Dual-impact recommendations (cost + utilization)
- Kubernetes runtime drift controls
- AI device utilization scans focused on GPU/runtime efficiency
- Weekly governance report with closure and recurrence metrics

## Next: API as Product Surface

Focus: make automation stable and predictable.

- OpenAPI export as canonical contract
- Official Python and TypeScript SDKs
- Signed webhooks with replay-window controls
- Consistent pagination and filter contracts
- Compatibility governance (versioning, deprecation windows, migration notes)

## Future: Capacity Intelligence

Focus: move from cost scanning to capacity governance.

- AI runtime capacity risk and throughput analysis
- Forecasting for cost and utilization risk
- Change simulation before production execution
- Org-level accountability by team, app, and environment

## Release Governance

- Binary release:
  - app metadata version increases
  - packaged artifacts are produced
- Content/governance release:
  - complete release notes are required
  - scope is explicit (no hidden binary changes)

## What customers should expect

- Faster conversion from findings to real savings actions
- Less ambiguity in owner accountability
- Better Kubernetes and AI device utilization visibility
- Safer automation integrations through stable API contracts
