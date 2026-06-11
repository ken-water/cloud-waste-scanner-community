# Cloud Waste Scanner Execution Roadmap v2

This plan turns the current roadmap into a staged execution model focused on user value, API stability, and scalable governance outcomes.

## Stage 1: Operational Closure (Next 1-2 releases)

Objective: move from “finding generation” to a repeatable execution loop.

### Scope

- Implement a unified remediation lifecycle:
  - `detected -> triaged -> assigned -> in_progress -> verified -> closed`
- Add risk-bucketed action templates:
  - high-confidence / low-regret cleanup playbooks first
  - rollback instructions and guardrails required
- Add dual-impact outputs per recommendation:
  - projected cost reduction
  - projected utilization improvement
- Add dedicated Kubernetes + AI device utilization views:
  - GPU utilization, memory pressure, request/limit drift, idle capacity

### Deliverables

1. Remediation state model in UI + API.
2. Evidence-to-action playbook templates for top waste categories.
3. Weekly governance report includes closure rate and drift recurrence.
4. AI device utilization scan panel replacing low-value generic AI analysis.

### Acceptance criteria

- At least 80% of new findings can be assigned to an owner and tracked to closure state.
- Weekly report includes closure SLA and reopened finding rate.
- AI/K8s views expose actionable signals (not only descriptive metrics).

## Stage 2: API as Product Surface (Following 2-3 releases)

Objective: make automation integration stable, typed, and predictable.

### Scope

- OpenAPI export becomes canonical machine contract.
- Python/TypeScript official SDKs with:
  - retries, backoff, typed errors, pagination iterators
- Webhook signing with replay-window controls.
- Unified cursor + filter contract across scans/events/findings/reports.
- Compatibility governance:
  - version policy
  - deprecation window
  - migration notes

### Deliverables

1. `GET /v1/openapi.json` tied to release contract.
2. `sdks/python` and `sdks/typescript` with tested examples.
3. Webhook verification guide and sample code.
4. API compatibility matrix in release notes.

### Acceptance criteria

- No breaking API changes without explicit version/deprecation note.
- SDK examples cover top 80% automation scenarios.
- Cursor/filter behavior consistent across all list endpoints.

## Stage 3: Capacity Intelligence and Governance Expansion (Quarterly horizon)

Objective: evolve from cost scanning into capacity operations intelligence.

### Scope

- AI device capacity intelligence:
  - queue pressure, throughput loss, unit cost per workload
- Predictive governance:
  - risk forecasts for next billing and capacity cycle
- Change simulation before execution:
  - estimated cost/performance impact of rightsizing or cleanup
- Org accountability layer:
  - team/app/environment ownership, SLA, recurrence trends

### Deliverables

1. AI runtime governance report with financial + performance impact.
2. Forecast module for cost/utilization risk.
3. Simulation mode for high-impact remediation actions.
4. Org-level governance dashboard and export.

### Acceptance criteria

- Teams can estimate impact before applying high-risk changes.
- Recurrence of repeated waste patterns decreases release-over-release.
- Management reports support finance, ops, and platform reviews with one shared evidence model.

## Priority and sequencing rules

1. Contract stability before ecosystem expansion:
   - compatibility governance and pagination/filter consistency must lead SDK and MCP expansion.
2. Evidence quality before automation scale:
   - avoid automating noisy findings.
3. Test-gate before production promotion:
   - all release content verified in test environment before production rollout.

## KPIs for execution tracking

- Finding closure rate (weekly/monthly)
- Reopened finding rate
- Mean time to owner assignment
- Mean time to verified closure
- Monthly projected savings from closed findings
- AI device utilization efficiency delta
- API integration success rate (SDK + webhook consumers)

## Release governance policy (binary vs content)

- Binary release:
  - app metadata version increments
  - packaged artifacts produced
- Content/governance release:
  - release tag + complete release notes
  - no binary version bump required
  - must clearly declare scope in release notes
- Community release line reset:
  - the new Community-era release line may start at `3.0.0`
  - this reset is a release-line decision, not an automatic binary-version decision
  - if binaries are unchanged, keep executable/package versions unchanged until a binary release is actually made
