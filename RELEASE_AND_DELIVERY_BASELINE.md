# Release and Delivery Baseline

This file records the current source-of-truth baseline for versioning and delivery status on `main`.

## Release Lines

- Community content release line: `3.1.0`
- Desktop app package version currently present in this repository: `3.1.0`
- Internal Rust workspace crate version currently present in this repository: `0.1.0`

These values are intentionally not forced to move together.

- The Community content release line tracks repository-level documentation, skills, and scope positioning.
- Desktop app package versions should change only when the shipped desktop binary changes.
- Internal workspace crate versions are implementation metadata and are not the public Community release line.
- Active Community releases on current `main` should move on a Community-only forward line and must not reuse a Pro-assigned public version.
- Historical Pro release material remains archived under `v1.x`.
- Pre-reset local-first release material remains archived under `v2.9.x`.

## Delivery Status

### Community

Community is the shipped baseline on `main`.

Community should be treated as the stable source of truth for:

- local-first scanning
- evidence review
- local exports and handoff artifacts for manual sending
- local API and SDK automation
- Community skills

### Team

Team should currently be treated as preview/planned, not GA.

The repository contains local foundations and gated workflow pieces for Team, including:

- owner and org-unit data models
- lifecycle state storage
- audience-aware handoff concepts
- governance-oriented views and summaries

Do not describe Team as fully shipped until `TEAM_CLOSURE_PLAN.md` is substantially complete.

### Enterprise

Enterprise remains planned only.

Enterprise scope documents the intended commercial boundary, not a currently shipped control plane.

## Interpretation Order

When versioning, licensing, or delivery wording conflict, interpret the current repository in this order:

1. `LICENSE`
2. `README.md`
3. `RELEASE_AND_DELIVERY_BASELINE.md`
4. scope and roadmap documents
5. older release bodies, historical tags, and legacy package metadata
