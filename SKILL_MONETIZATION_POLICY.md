# Skill Monetization Policy

This document defines how Cloud Waste Scanner can open-source skills without giving away the paid product boundary.

## Core rule

Open-source skills are allowed to explain, summarize, and format evidence.

Paid product boundaries begin where the system starts coordinating people, enforcing workflow, or operating as a managed control layer.

That means a skill can be open while the business still charges for:

- team coordination
- owner assignment workflows
- shared governance state
- role-aware delivery and approval
- managed distribution and compliance control

## What can stay open

These are good Community skill candidates:

- finding explanation
- report summarization
- export formatting
- local API helpers
- SDK examples
- operator playbook generation
- finance or executive write-up generation from local evidence

These improve product adoption and trust. They do not, by themselves, create a workflow moat problem.

## What must remain paid

Do not put these into open Community skills if the goal is to preserve Team and Enterprise value:

- shared owner directory
- org structure sync
- role-based assignment routing
- approval workflow
- overdue escalation
- SLA tracking
- centralized audit trail
- managed report delivery
- secure sharing portals
- SSO or enterprise identity mapping
- cross-instance governance rollups
- hosted knowledge memory across teams

If a skill starts deciding who receives a finding, who must approve it, when it escalates, or how it is tracked across people, it has crossed into paid workflow territory.

## Pricing logic

The open skill is a distribution and activation surface.

It helps users:

- understand the scanner faster
- get value from exports faster
- trust the evidence faster

It should increase conversion into paid editions by making the evidence clearer, not by replacing governance execution.

The paid offer should be framed as:

- Community: find and explain waste locally
- Team: assign, track, review, and close waste with accountable owners
- Enterprise: control identity, audit, retention, and policy across organization boundaries

## Safe open-source pattern

Use this pattern for Community skills:

1. Read local evidence
2. Normalize it
3. Explain it for one audience
4. Suggest safe next actions
5. Stop before workflow coordination

This is safe because the output still requires a human or a paid product layer to route, approve, and enforce action.

## Unsafe open-source pattern

Avoid these in Community skills:

1. Persisting shared owner state
2. Maintaining org hierarchy
3. Sending action packs to role-specific recipients automatically
4. Running recurring governance cycles on behalf of a team
5. Maintaining approval or closure history across users

Those features are not "just convenience." They are the commercial control surface.

## Recommended commercial packaging

### Community

- Open-source local scanner
- Open-source local API and SDKs
- Open-source explainer skills
- Manual export and manual sharing

### Team

- Paid workflow pack
- owner directory
- org mapping
- weekly governance pack
- assignment and review lifecycle
- audience-aware handoff controls

### Enterprise

- Paid centralized control pack
- SSO and SCIM
- centralized evidence retention
- audit enforcement
- cross-team reporting
- policy administration

## Decision test for future skills

Before open-sourcing a new skill, ask:

1. Does it only interpret evidence, or does it coordinate people?
2. Does it stop at advice, or does it enforce workflow?
3. Does it help one operator, or does it become shared team infrastructure?
4. If copied by a user, would they still need Team or Enterprise to run governance properly?

If the answer to the last question is "no", the skill is probably giving away paid value.

## Current recommendation

`skills/cws-report-explainer` is safe to keep open because it:

- reads local evidence
- explains findings
- produces role-specific summaries
- does not maintain shared workflow state
- does not manage organization structure
- does not automate approvals or escalation

It should be used as a funnel asset, not a paid SKU by itself.
