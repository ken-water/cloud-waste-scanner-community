# Community Backlog Next

This backlog translates `COMMUNITY_ROADMAP_NEXT.md` into implementation-ready work.

## C1 Detection Quality

### C1-1 High-confidence waste category pass
- Goal: improve trust in the first findings users see.
- Scope:
  - review top Community waste categories
  - reduce obvious false positives
  - prefer high-confidence / low-regret candidates first
- Acceptance:
  - top categories have explicit confidence rationale
  - obvious noisy categories are either improved or downgraded

### C1-2 Per-finding explanation model
- Goal: every finding should explain itself.
- Scope:
  - add fields for:
    - detection reason
    - evidence summary
    - action caution
    - estimation rationale
  - render these consistently in UI and exports
- Acceptance:
  - sampled findings are understandable without internal product knowledge

### C1-3 Dual-impact output in Community
- Goal: report more than cost.
- Scope:
  - add projected savings
  - add projected utilization delta where possible
  - include confidence notes
- Acceptance:
  - exported reports carry both dimensions when data exists

## C2 Runtime Depth

### C2-1 Kubernetes runtime drift pass
- Goal: Community should expose modern runtime waste, not only classic cloud waste.
- Scope:
  - request/limit drift
  - node allocatable mismatch
  - under-utilized node pool signals
  - namespace or team hints when labels exist
- Acceptance:
  - K8s findings are actionable and grouped clearly

### C2-2 GPU / AI device waste signals
- Goal: replace low-value generic AI wording with runtime evidence.
- Scope:
  - utilization
  - memory pressure
  - idle windows
  - reclaim hints
- Acceptance:
  - AI device findings distinguish waste from capacity risk

### C2-3 Historical recurrence comparison
- Goal: show repeated waste patterns across runs.
- Scope:
  - compare current vs prior runs
  - highlight repeated categories and repeated assets where possible
- Acceptance:
  - user can tell whether a finding is new, recurring, or unresolved

## C3 Single-Machine Workflow

### C3-1 Findings review ergonomics
- Goal: reduce friction in local review.
- Scope:
  - stronger filters
  - grouping modes
  - clearer sort defaults
  - better bulk review flow
- Acceptance:
  - common review actions take fewer steps

### C3-2 Export quality upgrade
- Goal: make exports usable outside the product.
- Scope:
  - clearer headings
  - clearer assumptions
  - cleaner summaries
  - better audience-neutral wording
- Acceptance:
  - exported pack can be read by a non-user without product training

### C3-3 Playbook presentation
- Goal: show safe next steps for common findings.
- Scope:
  - pre-check
  - execution suggestion
  - verification suggestion
  - rollback caution
- Acceptance:
  - top waste categories include a readable action pattern

## C4 Local API and SDK Surface

### C4-1 OpenAPI consistency pass
- Goal: make the local API a stable machine contract.
- Scope:
  - review schema completeness
  - align list/filter/cursor semantics
  - tighten example coverage
- Acceptance:
  - major Community endpoints are described consistently

### C4-2 SDK usability pass
- Goal: make Python and TypeScript SDKs practical for local automation.
- Scope:
  - clearer examples
  - typed error handling
  - pagination helpers
  - export/report examples
- Acceptance:
  - top automation scenarios work from reference examples

### C4-3 Local automation examples
- Goal: reduce onboarding time for developers.
- Scope:
  - example scripts for:
    - run scan
    - fetch findings
    - export report
    - compare recent runs
- Acceptance:
  - new developer can complete a basic automation flow without reverse engineering

## Priority Order

1. C1 detection quality
2. C2 runtime depth
3. C3 single-machine workflow
4. C4 local API and SDK surface

## Done Criteria

- Community users get better signal, not just more features.
- Reports and findings are easier to trust and reuse.
- Single-machine workflow is stronger without requiring Team concepts.
- Local automation is easier to adopt and maintain.
