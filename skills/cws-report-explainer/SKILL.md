---
name: cws-report-explainer
description: Explain Cloud Waste Scanner findings and report exports for operators, finance reviewers, and executives. Use when the user wants a local-first explanation of scan findings, ranked next actions, owner-ready summaries, or role-specific writeups from the CWS local API or exported JSON/CSV/TXT evidence.
metadata:
  short-description: Explain CWS findings and exports
---

# CWS Report Explainer

Use this skill when the task is about interpreting Cloud Waste Scanner evidence, not running cloud scans.

This skill is for:

- Explaining what a finding means in operator language
- Ranking cleanup actions by confidence, likely savings, and execution risk
- Converting findings into summaries for a boss, finance, or an execution owner
- Turning local API output or exported files into a weekly review brief

This skill is not for:

- Direct cloud API access
- Holding or requesting cloud credentials
- Replacing the desktop app as the scan engine
- Multi-user workflow orchestration beyond a single exported handoff
- Automatic owner routing, escalation, approval, or shared governance state

## Inputs

This skill supports two runtime modes:

- `connected-mode`: use the installed app and local API
- `file-only-mode`: use exported evidence files

Read `../../SKILL_RUNTIME_MODES.md` when you need the product rule for these modes.

Prefer local API evidence first when available:

- `GET /status`
- `GET /v1/findings`
- `GET /v1/scans`
- `GET /v1/reports`
- `GET /v1/openapi.json`

Fallback inputs are exported files:

- Findings JSON
- Findings CSV
- Report JSON
- Plain text notes copied from a handoff or PDF

If the user gives no data, ask for one of:

- local API base URL and token
- one exported JSON or CSV file
- pasted findings text

## Workflow

1. Determine the evidence source.
2. If local API is available, use `scripts/build_context.py --base-url ...`.
3. If exported files are provided, use `scripts/build_context.py --input ...`.
4. Read the normalized bundle and produce the smallest useful answer for the user's audience.

## Output modes

Choose one or more based on the request:

- `operator`: explain concrete findings, risk, likely cleanup sequence, blockers
- `finance`: focus on projected monthly waste, confidence, what is safe to count now
- `executive`: focus on concentration, ownership gaps, operating risk, next-step decisions
- `weekly-action-list`: produce an owner-ready list with priority, rationale, and due-soon items

## Required behavior

- Stay evidence-first. Do not invent savings numbers that are not present in the data.
- Separate facts from inference.
- Prefer "safe next action" over aggressive deletion advice.
- Call out when a finding still needs owner confirmation.
- If the input is incomplete, say what is missing without turning the answer into process overhead.
- Stop at explanation and recommended next action. Do not simulate paid Team workflow features such as assignment routing, SLA enforcement, or approval state.

## References

- For local API inputs, endpoints, and normalized bundle shape, read `references/api-and-inputs.md`.
- For output patterns by audience, read `references/output-patterns.md`.
- For ready-to-use prompts, read `references/prompt-templates.md`.
- For concrete input/output examples, read `references/examples.md`.

## Scripts

- `scripts/build_context.py`
  - Builds a normalized context bundle from local API or exported files.
  - Use `--help` for arguments.
