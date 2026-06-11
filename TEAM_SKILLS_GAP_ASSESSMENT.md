# Team Skills Gap Assessment

This document assesses whether Cloud Waste Scanner is ready for a true Team skill family.

## Executive conclusion

Cloud Waste Scanner already contains a meaningful local Team MVP foundation.

However, the product is not yet complete enough to claim a mature Team skill family is product-ready.

Current position:

- Community skills: ready to define and ship
- Team skills: can be defined conceptually, but should be treated as planned or limited-preview
- Enterprise skills: architecture only, not implementation focus

## What already exists

### 1. Organization and owner foundation

Implemented:

- org unit records
- owner directory
- owner role model: `owner`, `manager`
- owner deactivation and reassignment flow

Evidence:

- `Settings -> Org & Owners` supports org and owner maintenance
- Team scope and Team user guide already define this model

## 2. Team execution foundation

Implemented:

- single finding owner assignment
- manager-only batch assignment
- lifecycle record loading in Scan Results
- org/owner filters in Scan Results
- audience-aware handoff packs: `exec`, `owner`, `audit`

Evidence:

- `ResourcesTable` loads lifecycle, owner, and org unit records
- Team-entitled actions are gated in UI and backend

## 3. Local safety controls

Implemented:

- runtime operator role
- admin guard for role changes
- local role-admin mapping

Evidence:

- Team permissions and operations guide
- runtime role APIs and settings UI

## What is only partially complete

### 1. Lifecycle workflow depth

Partially present:

- lifecycle records exist
- ownership exists

Missing or unclear as a mature product loop:

- enforced triage discipline
- strong due-date workflow
- consistent verified/closed loop reporting
- reopened finding governance behavior

Impact:

This weakens any Team skill that tries to reason over reliable execution state.

## 2. Weekly governance loop

Partially present:

- governance screen exists
- weekly pack language exists
- handoff export exists

Missing or unclear:

- explicit recurring review cycle management
- overdue review logic
- SLA breach logic
- manager follow-up loop

Impact:

This means a `cws-weekly-governance-pack` skill can be named, but would still rely on incomplete product state.

## 3. Shared team coordination model

Partially present:

- local Team workflow on one machine

Missing:

- true shared workspace model
- multi-user synchronization
- governed delivery between people
- persistent review responsibility across users

Impact:

This is the biggest reason not to overstate Team readiness.

The product currently supports local governance execution patterns more than true team-wide collaborative infrastructure.

## What is still missing for real Team skill readiness

These gaps block a serious Team skill family from being called complete:

### 1. Reliable workflow state

Need:

- stronger lifecycle state integrity
- due dates and review dates used consistently
- reopened and overdue behavior modeled clearly

### 2. Team coordination signals

Need:

- manager review state
- assignment freshness
- overdue concentration
- queue of unresolved items by org/owner

### 3. Governance cadence outputs

Need:

- stable weekly governance pack contract
- explicit metrics:
  - assignment rate
  - closure rate
  - overdue rate
  - reopen rate

### 4. Distribution model

Need:

- stronger controlled sharing flow
- repeatable delivery model for owner/manager/audit audiences

Without this, Team skills can summarize work, but not truly operate as workflow copilots.

## Skill-family readiness by edition

### Community skills

Status: ready now

Reason:

- depend on local evidence
- do not require shared workflow coordination
- align with current product maturity

Examples:

- `cws-report-explainer`
- `cws-weekly-brief`
- `cws-playbook-writer`

### Team skills

Status: limited-preview only

Reason:

- product has meaningful foundation
- but workflow loop is not yet strong enough to treat Team skills as fully mature

Allowed near-term direction:

- `cws-weekly-governance-pack`
- `cws-manager-review-copilot`
- `cws-owner-router`

Positioning rule:

Describe these as planned or preview Team skills until workflow-state and cadence gaps are closed.

### Enterprise skills

Status: not ready

Reason:

- product focus is not on centralized identity/audit control yet
- enterprise context is mostly architectural, not operational

## Recommended product language

Use this language internally and externally:

- Community skills are active and shippable now.
- Team product capabilities exist in local MVP form.
- Team skills should be treated as preview concepts until governance workflow maturity improves.
- Enterprise skills should remain planned only.

## Recommended next steps

### Before expanding Team skills

Do these first:

1. tighten lifecycle integrity
2. add overdue and review-date model
3. stabilize weekly governance metrics
4. improve governed handoff and distribution path

### After those are stable

Then build:

1. `cws-weekly-governance-pack`
2. `cws-manager-review-copilot`
3. `cws-owner-router`

## Bottom line

Team is not imaginary.

But Team is not yet mature enough that a full Team skill family should be marketed as ready.

The correct position today is:

- Community skills: real
- Team skills: preview/planned
- Enterprise skills: planned
