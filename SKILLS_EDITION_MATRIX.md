# Skills Edition Matrix

This document defines which kinds of skills belong to Community, Team, and Enterprise.

## Core rule

Skills should be classified by product boundary, not by how many prompts or scripts they contain.

The deciding question is:

- does the skill explain evidence
- or does it coordinate people and enforce workflow
- or does it control policy, identity, and audit across organizational boundaries

## Edition matrix

| Edition | Primary role | Allowed skill behavior | Typical examples | Must not include |
|---|---|---|---|---|
| Community | Explain and prepare | Read local evidence, normalize exports, explain findings, rank next actions, draft summaries | finding explainer, finance summary, executive brief, weekly operator brief, export auditor, playbook writer | shared owner state, org structure, automatic routing, SLA tracking, approval workflow |
| Team | Coordinate and execute | Use owner/org context, help assignment review, build governance packs, support lifecycle follow-up | weekly governance pack builder, owner assignment assistant, org summary assistant, overdue review assistant, manager handoff copilot | SSO/SCIM, centralized enterprise policy, cross-instance control plane |
| Enterprise | Control and audit | Use identity, retention, policy, audit, and cross-team governance context | audit evidence automation, policy exception review, compliance rollup, enterprise governance assistant | anything that assumes only single-machine local context |

## Community skill criteria

A Community skill is valid when it:

- works from local API or exported evidence
- improves understanding, explanation, or local preparation
- stops before shared workflow coordination

Examples:

- `cws-report-explainer`
- `cws-weekly-brief`
- `cws-playbook-writer`
- `cws-export-auditor`

## Team skill criteria

A Team skill starts where the product begins coordinating execution between people.

Typical signals:

- owner directory required
- org unit required
- lifecycle state required
- assignment or review rhythm required
- audience-aware team handoff required

Examples:

- `cws-owner-router`
- `cws-weekly-governance-pack`
- `cws-manager-review-copilot`
- `cws-overdue-review`

## Enterprise skill criteria

An Enterprise skill starts where centralized control and governed operations become necessary.

Typical signals:

- identity-backed access context
- audit and retention policy
- policy exception management
- cross-team or cross-instance rollups
- compliance evidence control

Examples:

- `cws-audit-pack-automation`
- `cws-policy-exception-review`
- `cws-cross-team-governance-rollup`
- `cws-compliance-retention-summary`

## Current placement

### Community

- `cws-report-explainer`

Reason:

- explains findings
- works from local API or exported files
- does not maintain shared workflow state
- does not coordinate approvals or escalation

### Team

Planned, not yet established as product-ready skill family:

- `cws-weekly-governance-pack`
- `cws-owner-router`
- `cws-manager-review-copilot`

### Enterprise

Planned only:

- `cws-audit-pack-automation`
- `cws-policy-exception-review`

## Decision checklist

Before assigning a skill to an edition, answer:

1. Does it still work meaningfully with only one operator and local evidence?
2. Does it require shared owner, org, or lifecycle state?
3. Does it require enterprise identity, policy, or audit context?

Decision:

- If only 1 is true, it is likely Community.
- If 2 is true, it is likely Team.
- If 3 is true, it is likely Enterprise.

## Product rule

Community skills should drive adoption.

Team skills should drive coordination value.

Enterprise skills should drive control and compliance value.
