# Community Skills Strategy

This document defines how Community skills should support the product.

## Why skills matter

Skills can become the fastest way for a new user to understand Cloud Waste Scanner.

They reduce time-to-value in four places:

- after the first scan
- when reading exports
- when explaining findings to a boss or finance
- when building small local automations

## Product role of Community skills

Community skills should act as:

- explainer layer
- formatting layer
- lightweight local automation helper

They should not become:

- shared workflow engine
- team control plane
- organization system of record

## Priority order

### S1 Explain evidence better

Examples:

- finding explainer
- finance summary generator
- executive brief generator
- weekly action list from local evidence

### S2 Improve local automation

Examples:

- local API fetch helpers
- report comparison scripts
- export normalization helpers

### S3 Strengthen trust in outputs

Examples:

- confidence notes
- rationale templates
- recurring finding comparison
- evidence-to-action playbook suggestions

## Non-goals for Community skills

- shared assignment state
- org chart management
- automatic cross-user delivery
- overdue escalation engine
- enterprise approval control

## Current first skill

`cws-report-explainer`

Purpose:

- explain findings
- rank actions
- produce audience-specific summaries

Why this is the right first skill:

- immediate user value
- low implementation risk
- strong fit with Community scope
- low risk of eroding paid workflow boundaries

## What should come next

Recommended next Community skills:

1. `cws-weekly-brief`
   - summarize current vs prior run
   - highlight repeated waste
   - produce one weekly operator brief

2. `cws-playbook-writer`
   - convert findings into safe cleanup checklists
   - include pre-check, action, verification, rollback caution

3. `cws-export-auditor`
   - inspect exported packs for missing assumptions, missing owners, and unclear evidence wording

## Commercial guardrail

Every new Community skill must preserve this boundary:

- explain and prepare
- do not coordinate and enforce
