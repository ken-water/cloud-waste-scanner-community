# Team Closure Plan

This document turns the Team gap assessment into an execution checklist.

## Goal

Move Team from local MVP foundation to a product state strong enough for real Team skill previews and clearer commercial positioning.

## Workstreams

### T1 Lifecycle integrity

Objective:

- make lifecycle state trustworthy enough for workflow reasoning

Tasks:

- define required transitions for:
  - `detected`
  - `triaged`
  - `assigned`
  - `in_progress`
  - `verified`
  - `closed`
- add clear rules for reopened items
- add due date and review date semantics
- ensure lifecycle summaries are consistent in UI and exports

Done when:

- lifecycle state is not just stored, but interpretable
- reopened and stale items are visible in reporting

### T2 Governance cadence

Objective:

- make the weekly governance loop explicit

Tasks:

- define weekly governance pack contract
- add stable metrics:
  - assignment rate
  - closure rate
  - overdue rate
  - reopen rate
- define current-slice vs weekly-summary behavior
- make Governance outputs reusable by future Team skills

Done when:

- a manager can use one weekly view without reconstructing the workflow manually

### T3 Overdue and review signals

Objective:

- make unresolved work visible, not implicit

Tasks:

- define overdue logic
- define review-date logic
- surface overdue concentration by owner and org unit
- surface assignment freshness and unresolved backlog

Done when:

- future Team skills can safely identify what needs follow-up this week

### T4 Distribution and governed handoff

Objective:

- improve the path from evidence to controlled team delivery

Tasks:

- define audience contract for:
  - `exec`
  - `owner`
  - `audit`
- tighten handoff pack structure
- define what is manual today versus future controlled delivery
- avoid promising shared-workspace behavior that does not exist yet

Done when:

- Team output is consistent enough for preview Team skills without overstating collaboration maturity

## Priority order

1. T1 lifecycle integrity
2. T2 governance cadence
3. T3 overdue and review signals
4. T4 distribution and governed handoff

## Skill implications

Only after T1 and T2 are stable should these Team skill previews move forward:

1. `cws-weekly-governance-pack`
2. `cws-manager-review-copilot`
3. `cws-owner-router`

## Positioning rule

Until this plan is substantially complete:

- Community skills are active
- Team skills remain preview/planned
- Enterprise skills remain planned
