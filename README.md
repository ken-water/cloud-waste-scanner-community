# Cloud Waste Scanner Community

Local-first cloud waste scanning for operators who want source-visible, production-usable tooling.

[![Sponsor](https://img.shields.io/badge/Support-GitHub%20Sponsors-0f172a?logo=githubsponsors&logoColor=white)](https://github.com/sponsors/ken-water)
[![Buy us a coffee](https://img.shields.io/badge/Buy%20Us%20A%20Coffee-Keep%20CWS%20Shipping-f59e0b)](https://github.com/sponsors/ken-water)

> If Cloud Waste Scanner saves your team time or cloud spend, support continued client-side open work on GitHub Sponsors:
> https://github.com/sponsors/ken-water

## Repository Scope

This repository is the **Community Edition**.

Community release line: `3.0.0`

Note:

- `3.0.0` marks the Community release line reset for the open client-first roadmap.
- This does not automatically imply that every binary artifact in the repository has already changed to `3.0.0`.
- Binary version increments should still happen only when the shipped desktop or SDK artifacts themselves change.

- Community is local-first discovery and evidence generation.
- Team is governance execution for owners, org units, and handoff workflows.
- Enterprise is centralized control for identity, audit, and compliance scale.
- No hosted license activation is required for core local workflows.
- This repository is distributed under a **non-commercial license** (see `LICENSE`).

For commercial use, managed support, or enterprise rollout terms, contact the maintainers via the project website.

Scope references:
- Community: `COMMUNITY_SCOPE.md`
- Community roadmap: `COMMUNITY_ROADMAP_NEXT.md`
- Community backlog: `COMMUNITY_BACKLOG_NEXT.md`
- Community skills strategy: `COMMUNITY_SKILLS_STRATEGY.md`
- Community skills index: `COMMUNITY_SKILLS_INDEX.md`
- Skills edition matrix: `SKILLS_EDITION_MATRIX.md`
- Skills usage (ZH): `SKILLS_USAGE_ZH.md`
- Skill monetization policy: `SKILL_MONETIZATION_POLICY.md`
- Skill runtime modes: `SKILL_RUNTIME_MODES.md`
- Team: `TEAM_SCOPE.md`
- Enterprise: `ENTERPRISE_SCOPE.md`

## Legal Transition Notice

This repository previously contained Pro-era documentation and release metadata.

- Older tags/commits may still include historical Pro wording.
- The current `main` branch is the legal source of truth for Community licensing.
- Transition details are documented in `LEGAL-TRANSITION.md`.

## Core Principles

- Local-first execution: credentials remain on the operator machine.
- Read-only scanning by default.
- Evidence-oriented outputs for operator and finance review.
- Commercial boundaries are defined by workflow coordination and centralized control, not by hiding core local discovery value.

## Local API and SDKs

Community includes a local HTTP API for automation around scans, findings, reports, Kubernetes/container waste checks, and governance evidence.

- OpenAPI export: `GET /v1/openapi.json`
- Compatibility policy: `GET /v1/meta/compatibility`
- Cursor pagination: add `envelope=true&limit=50&cursor=<next_cursor>` to supported list endpoints.
- Webhook signing: `X-CWS-Signature` uses `HMAC-SHA256(secret, "<timestamp>.<raw_json_body>")`.
- SDKs: see `sdks/python` and `sdks/typescript`.

### Community Skills

Cloud Waste Scanner Community now includes local-first skills that help a single operator get more value from scan evidence.

- `cws-report-explainer`: explain findings for operators, finance, and leadership
- `cws-weekly-brief`: summarize current vs prior evidence
- `cws-playbook-writer`: turn findings into safer action playbooks
- `cws-export-auditor`: review exports before sharing them

Community skills work best when connected to the CWS local API, but can also run from exported evidence files.

Runtime positioning:

- Preferred: `connected mode` through the installed app and local API
- Supported fallback: `file-only mode` from exported evidence
- Reference: `SKILL_RUNTIME_MODES.md`
- Index: `COMMUNITY_SKILLS_INDEX.md`
- Copy pack: `COMMUNITY_SKILLS_COPY_PACK.md`

Build a normalized evidence bundle from the local API:

```bash
python3 skills/cws-report-explainer/scripts/build_context.py \
  --base-url http://127.0.0.1:43177 \
  --token local-api-token \
  --output ./tmp/cws-context.json
```

Or from exported files:

```bash
python3 skills/cws-report-explainer/scripts/build_context.py \
  --input ./exports/findings.json \
  --output ./tmp/cws-context.json
```

Then invoke the skill against the bundle for:

- operator explanation
- finance summary
- executive brief
- weekly action list

Prompt templates and sample outputs:

- `skills/cws-report-explainer/references/prompt-templates.md`
- `skills/cws-report-explainer/references/examples.md`

## Support This Project

If Cloud Waste Scanner helps your team, you can support ongoing maintenance, client-side open work, and new operator workflows.

- GitHub Sponsors: https://github.com/sponsors/ken-water
- Buy us a coffee: same GitHub Sponsors page, no separate checkout flow

## Development

### Prerequisites

- Rust (stable)
- Node.js 18+
- Tauri CLI

### Build GUI

```bash
cd gui
npm ci --no-audit --no-fund
npm run tauri dev
npm run tauri build
```

### Fast Build

```bash
USE_SCCACHE=auto AUTO_INSTALL_SCCACHE=1 ./fast_build.sh
```

### Local Site/API Stack

```bash
./scripts/local_stack_up.sh
./scripts/local_stack_check.sh
./scripts/local_stack_down.sh
```

## Notes

- This repository may still contain historical compatibility modules while the community split is being finalized.
- If a module appears to reference legacy commercial behavior, treat `main` documentation and active runtime behavior as authoritative.
