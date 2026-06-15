# CWS Skills Quickstart

Cloud Waste Scanner Community skills help an operator explain, summarize, audit, and turn scan evidence into safer next actions.

## Available skills

- `cws-report-explainer`: explain findings for operators, finance, and leadership.
- `cws-weekly-brief`: create a short weekly review from current and prior evidence.
- `cws-playbook-writer`: convert findings into cautious cleanup playbooks.
- `cws-export-auditor`: review exports before sending them to another reviewer.

## Validate the source tree

```bash
./scripts/validate-skills.sh
```

## Build packages

```bash
./scripts/package-skills.sh
```

Outputs:

```text
dist/skills-codex.tar.gz
dist/skills-claude.tar.gz
dist/skills-generic.tar.gz
```

## Try with sample evidence

```text
Use $cws-report-explainer with skills/examples/handoff_manifest_1.1.0.json and produce an operator summary.
```

```text
Use $cws-weekly-brief with skills/examples/scan_results.json and create a one-page weekly brief.
```

## Evidence sources

Preferred:

- CWS local API from the installed desktop app
- Handoff manifest `cws_handoff_manifest 1.1.0`
- Handoff findings CSV
- Scan results JSON

Fallback:

- pasted report text
- copied findings table
- PDF text extracted by the user or tool

## Boundaries

Community skills do not run cloud scans, hold cloud credentials, execute deletions, enforce approvals, or coordinate multi-user workflow state. They explain and transform evidence produced by Cloud Waste Scanner.
