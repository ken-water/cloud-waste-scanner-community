# Community Roadmap Next

This roadmap sets the near-term priority for Cloud Waste Scanner Community.

The goal is to strengthen the product where Community users feel value first:
- better detection quality
- clearer evidence
- stronger single-machine workflows
- more usable local automation

For this period, Team and Enterprise expansion is not the primary focus.

## Direction

Community is the local-first discovery and evidence layer.

That means the next releases should improve:
1. what the scanner can detect
2. how clearly it explains findings
3. how easily one operator can review and export evidence
4. how reliably developers can automate around the local API

## Release sequence

## Community vNext-1

Objective: improve finding quality and explanation.

### Scope
- Raise confidence on top waste categories first.
- Add clearer per-finding rationale:
  - why the item is flagged
  - what evidence was used
  - what risk exists before cleanup
  - what the expected savings or utilization gain may be
- Improve report wording so exported evidence is easier to read without product context.

### User outcome
- Fewer low-signal findings.
- Faster trust in the scan output.
- Better “can I safely act on this?” judgment.

## Community vNext-2

Objective: improve infrastructure-specific evidence depth.

### Scope
- Strengthen Kubernetes drift findings:
  - request vs limit drift
  - idle node pool signals
  - under-utilized workload signals
- Strengthen GPU / AI device views:
  - utilization
  - memory pressure
  - idle windows
  - reclaim hints
- Improve historical comparison for repeated waste patterns.

### User outcome
- Better signal quality for modern runtime environments.
- Easier identification of recurring waste instead of one-off snapshots.

## Community vNext-3

Objective: improve local operator workflow and exports.

### Scope
- Stronger filtering, grouping, and saved review paths for findings.
- Better history comparison UX.
- Better export packs for:
  - operator review
  - manager summary
  - manual external sharing
- Evidence-to-action playbook presentation for top categories.

### User outcome
- Faster single-machine review loops.
- Better exports for manual review with finance, managers, or auditors.
- Less need to re-explain findings outside the app.

## Community vNext-4

Objective: improve Community as a developer and automation surface.

### Scope
- Tighten OpenAPI completeness and consistency.
- Improve Python and TypeScript SDK usability.
- Standardize cursor/filter behavior across local API endpoints.
- Add more tested automation examples and reference scripts.

### User outcome
- Easier integration into local operator tooling.
- Less guesswork when building automation around scans and reports.
- Better reliability for repeatable local workflows.

## What is intentionally deprioritized

These areas are not removed, but are not the first priority in this phase:
- deeper org structure workflows
- more Team role variants
- richer multi-person handoff process
- Enterprise identity and centralized audit expansion

## Decision rule

When choosing work in this phase, prefer changes that help a single Community user:
- detect waste more accurately
- understand findings more quickly
- export evidence more clearly
- automate locally with less friction

Defer changes whose primary value is:
- team coordination
- org policy enforcement
- centralized identity
- centralized compliance control
