# Community Scope

## Positioning
Community is the local-first discovery and evidence layer.

It is intended for operators and small teams who need to:
- scan cloud resources locally
- inspect findings on one machine
- export evidence for manual review
- automate local workflows through the local API and SDKs

## Included in Community

### Discovery and analysis
- Multi-cloud resource scanning
- Local Kubernetes and container waste checks
- Local AI/GPU device utilization views when implemented in the client
- Historical run inspection
- Resource-level evidence review
- Findings filters, search, and local drill-down

### Local operator workflows
- Single-machine finding review
- Local notes and local status tracking
- Basic report export
- PDF/CSV export
- Local handoff artifact generation for manual sending

### Developer surface
- Local HTTP API
- OpenAPI contract
- Python and TypeScript SDKs
- Detection logic visibility where shipped in this repository

## Not included in Community
- Org structure management
- Shared owner directory
- Team lifecycle coordination
- SLA and overdue governance workflows
- Centralized audit enforcement
- SSO/SCIM
- Centralized policy administration

## Product rule
Community should help a user find, understand, and export cloud waste evidence without requiring hosted infrastructure.

## Open-source priority
These categories are good candidates for continued Community expansion:
- stronger detection quality
- clearer evidence presentation
- richer local exports
- local-first API and SDK quality
- single-operator workflow improvements
