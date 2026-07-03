# Cloud Waste Scanner Community v3.1.0

Community `v3.1.0` strengthens the workflow after detection.

This release makes handoff persistent, adds lifecycle closure metrics to governance review, and requires explicit reopen reasoning before closed findings move back into owner execution.

## Highlights

- persistent local handoff packages
- recent handoff package re-export
- governance closure / overdue / reopened metrics
- explicit reopen reason requirement for closed findings
- stronger lifecycle regression coverage

## Included

- handoff package local storage and retrieval
- local API routes for handoff package records
- Tauri commands for stored handoff workflows
- scan results UI support for recent handoff packages
- governance lifecycle summary and derived rates
- governance UI cards and export coverage for closure metrics
- closed-finding reopen gating in assignment flows
- targeted tests for handoff and governance lifecycle logic

## What teams can use now

- package findings locally and re-export later without rebuilding context
- review closure rate, overdue open findings, and reopened open findings in Governance
- require explicit reopen context when previously closed work returns to active assignment

## Why it matters

This release improves three operational gaps:

1. evidence continuity after export
2. visibility into whether findings are actually closing
3. auditability when closed work re-enters execution

## Recommended positioning

Describe `v3.1.0` as:

- a Community workflow-closure release
- a local-first handoff and governance improvement release
- a safer execution-semantics update

Do not describe it as:

- a Pro release
- a rollback to earlier `2.9.x` numbering
- a Team or Enterprise GA milestone
