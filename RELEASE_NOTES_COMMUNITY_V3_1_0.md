# Cloud Waste Scanner Community v3.1.0

Community `v3.1.0` is a workflow-closure release.

It strengthens the layer after detection: how findings are packaged for handoff, how governance review measures execution health, and how closed findings are allowed back into active work.

## Highlights

- persistent local handoff packages
- recent handoff package re-export from stored records
- governance scorecard lifecycle metrics
- closure, overdue-open, and reopened-open rate tracking
- explicit reopen reason requirement before closed findings move back into owner assignment
- tighter lifecycle timestamp semantics and regression coverage

## Included in this release

- local storage for handoff packages
- local API support for creating, listing, and fetching handoff package records
- Tauri commands for stored handoff package workflows
- scan-results UI support for recent handoff packages and re-export
- governance lifecycle execution summary calculations
- governance UI cards and exports for closure-oriented metrics
- reopen gating in owner assignment flows for closed findings
- targeted tests for handoff persistence and lifecycle/governance summary logic

## Why this release exists

Cloud waste detection by itself is not enough.

Teams also need:

- a stable local record of what was handed off
- a way to measure whether findings are actually closing
- a clear operator-visible rule for reopening work that was previously marked closed

`v3.1.0` closes those gaps without moving away from the local-first Community model.

## What is new

### 1. Persistent handoff packages

Handoff is now treated as a local package object, not only as transient export files.

This means operators can:

- create a handoff package from selected or filtered findings
- keep a stored local record of that package
- re-export later without rebuilding the context manually
- review recent handoff packages from the scan results workflow

### 2. Governance closure metrics

Governance views now include execution-side lifecycle metrics such as:

- lifecycle total
- lifecycle open
- lifecycle assigned
- lifecycle in progress
- lifecycle verified
- lifecycle closed
- overdue open
- reopened open
- closure rate
- overdue-open rate
- reopened-open rate

This makes the governance surface more useful for weekly review and manager follow-up.

### 3. Explicit reopen controls

Closed findings no longer move back into owner assignment silently.

If a closed finding must be reassigned:

- the workflow now requires an explicit reopen reason
- the UI prompts for that reason before reassignment
- lifecycle timestamps are cleaned up so the record reflects the real state transition

This reduces ambiguity in audit-sensitive or manager-reviewed workflows.

### 4. Regression coverage

Added targeted validation for:

- handoff package persistence and retrieval
- lifecycle execution summary math
- governance scorecard derived rates
- reopen and rollback timestamp behavior

## Operator value

- less evidence drift after export
- safer reassignment of previously closed work
- stronger weekly review signal from closure metrics
- better continuity between scan, handoff, and owner execution

## Upgrade guidance

1. create handoff packages from filtered or selected findings instead of relying only on one-off exports
2. use recent handoff package history when the same package needs to be re-shared
3. review closure rate, overdue open, and reopened open in Governance
4. expect a reopen-reason prompt when assigning a closed finding back to an owner

## Versioning note

This release is aligned to the active Community line.

Use `v3.1.0` as the current Community release identifier for both repository messaging and shipped application packaging.

Apply this versioning rule on current `main`:

- active Community releases use a Community-only forward line and must not reuse a Pro-assigned public version
- archived Pro-era releases stay under `v1.x`
- archived pre-reset local-first releases stay under `v2.9.x`

That keeps the current release from conflicting with:

- the existing `v3.0.1` Community release
- the Pro `v3.0.3` release line
- older Pro-era `v1.x` release material
- earlier internal desktop package numbering
