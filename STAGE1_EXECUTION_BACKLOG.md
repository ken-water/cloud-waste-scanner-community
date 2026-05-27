# Stage 1 Execution Backlog (Operational Closure)

This backlog translates Stage 1 of `ROADMAP_EXECUTION_V2.md` into implementation-ready work items.

## S1-1 Remediation Lifecycle State Machine

- Goal: track each finding from detection to verified closure.
- Scope:
  - Add lifecycle states in data model and APIs.
  - Add owner, due date, closure evidence, and reopen reason fields.
  - Add timeline view in UI.
- Acceptance:
  - Findings can move only through valid transitions.
  - API returns lifecycle metadata for all finding lists.
  - Reopened findings retain prior closure evidence.

## S1-2 Owner Assignment and SLA Enforcement

- Goal: remove “unowned finding” drift.
- Scope:
  - Required owner assignment for actionable findings.
  - SLA clock starts when finding is triaged.
  - Overdue flags in dashboard and weekly report.
- Acceptance:
  - `owner_id` coverage >= 80% for actionable findings.
  - Overdue items are visible in UI and report export.

## S1-3 Risk-Bucketed Action Templates

- Goal: make safe cleanup executable without ad hoc decisions.
- Scope:
  - Classify findings into high-confidence/low-regret first.
  - Provide action template with pre-check, execute, verify, rollback.
  - Include required guardrails for production resources.
- Acceptance:
  - Top 5 waste categories have templates.
  - Every template includes rollback steps.

## S1-4 Dual-Impact Recommendation Output

- Goal: each recommendation reports cost and utilization impact.
- Scope:
  - Add `projected_savings` and `utilization_delta` fields.
  - Standardize confidence level and estimation rationale.
  - Include impacts in API and PDF/report outputs.
- Acceptance:
  - New findings include both impact dimensions.
  - Report output renders both values and confidence.

## S1-5 Kubernetes Runtime Drift Controls

- Goal: surface request/limit and idle drift as closure-ready findings.
- Scope:
  - Node allocatable vs pod requests/limits checks.
  - Under-utilized node pool and over-requested workload signals.
  - Owner mapping from namespace/team labels.
- Acceptance:
  - K8s findings include owner/team hints when labels exist.
  - Weekly report shows top K8s drift buckets.

## S1-6 AI Device Utilization Scan (GPU Focus)

- Goal: replace low-value generic AI analysis with actionable GPU governance.
- Scope:
  - GPU utilization, memory pressure, queue wait signals.
  - Idle GPU windows and reclaim opportunities.
  - Finding-level recommendations with safe execution hints.
- Acceptance:
  - AI device view is available in UI and report export.
  - Findings distinguish “capacity risk” vs “waste opportunity”.

## S1-7 Weekly Governance Report v2

- Goal: make weekly operating loop measurable and repeatable.
- Scope:
  - Include: assignment rate, closure rate, reopen rate, SLA breaches.
  - Include trend deltas vs previous week.
  - Export API payload + report artifact.
- Acceptance:
  - Weekly report can be generated from API and UI.
  - All Stage 1 KPIs are present in output.

## S1-8 Test-Gate Automation for Content + Runtime

- Goal: block production rollout unless test environment passes checks.
- Scope:
  - Verify key pages, 404 behavior, newest-post consistency.
  - Verify release-note/tag coupling and release body completeness.
  - Produce machine-readable gate report artifact.
- Acceptance:
  - Failed checks block production promotion.
  - Gate report is retained for audit.

## Dependency Order

1. S1-1, S1-2
2. S1-3, S1-4
3. S1-5, S1-6
4. S1-7
5. S1-8

## Definition of Done (Stage 1)

- Operational loop is complete: detection -> assignment -> closure -> verification.
- Weekly report proves KPI movement with evidence history.
- AI/K8s findings are actionable, owner-mapped, and rollback-safe.
