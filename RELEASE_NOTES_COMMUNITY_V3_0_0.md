# Cloud Waste Scanner Community v3.0.0

## Release type

Community release-line reset.

This release marks the start of the Community `3.0.0` line for the open client-first roadmap.

It is primarily a product-boundary, documentation, and skill-surface release.

It does not claim that every desktop or SDK binary artifact has already been rebuilt as `3.0.0`.

## Why this release exists

The project needed a cleaner Community-era baseline.

The previous `2.9.x` line mixed:

- historical Pro-era assumptions
- evolving Community boundaries
- incomplete language around paid workflow layers

`v3.0.0` resets the Community story around a clearer rule:

- Community: local-first discovery and evidence
- Team: governance execution
- Enterprise: centralized identity, audit, and control

## What is new

### 1. First Community skill: `cws-report-explainer`

Added a first local-first Community skill for turning CWS evidence into usable summaries.

It supports:

- operator explanation
- finance summary
- executive brief
- weekly action list generation

It works from:

- CWS local API
- exported JSON / CSV / TXT evidence

Artifacts:

- `skills/cws-report-explainer/SKILL.md`
- `skills/cws-report-explainer/scripts/build_context.py`
- prompt templates
- examples
- audience output guidance

### 2. Skills strategy and edition boundary

Defined a formal skills model so future skills do not blur Community, Team, and Enterprise.

Added:

- Community skills strategy
- skills edition matrix
- skill monetization policy
- skill runtime modes
- Team skills gap assessment

This makes the following rules explicit:

- Community skills explain and prepare
- Team skills coordinate and execute
- Enterprise skills control and audit

### 3. Runtime mode clarification

Skills now have a consistent runtime model:

- `connected-mode`: installed app + local API
- `file-only-mode`: exported evidence only

This gives users a low-friction trial path without weakening the local-first product direction.

### 4. Chinese usage guidance

Added a Chinese-language skills usage guide for consistent external communication.

## What did not change

This release does not claim completion of:

- Team skill family
- Enterprise skill family
- centralized identity workflows
- enterprise audit workflow automation

It also does not imply that unchanged binaries were version-bumped.

## Current product position after v3.0.0

### Community

Active now:

- local-first scanning
- local evidence review
- exports
- local API and SDKs
- Community explainer skills

### Team

Current status:

- local Team MVP foundation exists
- Team skills should still be treated as preview/planned

### Enterprise

Current status:

- planned only

## Recommended next steps

Near-term Community direction:

1. `cws-weekly-brief`
2. `cws-playbook-writer`
3. stronger finding explanation quality
4. stronger local automation examples

## Scope note

This release should be described as:

- a Community release-line reset
- a first skill-surface release
- a clarification of edition boundaries

It should not be described as:

- a major binary rebuild
- completion of Team or Enterprise product lines
