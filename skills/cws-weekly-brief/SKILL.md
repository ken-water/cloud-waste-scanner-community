---
name: cws-weekly-brief
description: Summarize the latest Cloud Waste Scanner evidence against recent runs to produce a single-operator weekly brief. Use when the user wants a current-vs-prior scan summary, repeated waste signals, top concentration points, or a short weekly review note from the CWS local API or exported evidence.
metadata:
  short-description: Build a weekly brief from CWS evidence
---

# CWS Weekly Brief

Use this skill when the task is to create a concise weekly review from Cloud Waste Scanner evidence.

This skill is for:

- Summarizing the latest scan in plain language
- Comparing the current result with a prior run when available
- Highlighting repeated or recurring waste patterns
- Producing a one-page weekly note for an operator, manager, or founder

This skill is not for:

- Direct cloud API access
- Running new scans against cloud providers
- Shared workflow state or cross-user coordination
- Automatic owner routing, SLA enforcement, escalation, or approval

## Inputs

This skill supports two runtime modes:

- `connected-mode`: installed app + local API
- `file-only-mode`: exported evidence files

Read `../../SKILL_RUNTIME_MODES.md` for the product rule behind these modes.

Prefer local API evidence first:

- `GET /status`
- `GET /v1/scans`
- `GET /v1/findings`
- `GET /v1/reports`

Fallback inputs:

- current findings JSON or CSV
- current and prior exported scan bundles
- pasted notes from a weekly handoff or review

If the user provides only one run, still build the brief and say that historical comparison is limited.

## Workflow

1. Determine whether current and prior evidence both exist.
2. Use `scripts/build_weekly_brief_context.py` to normalize the input.
3. Produce a short weekly brief with:
   - headline
   - what changed
   - biggest concentration points
   - repeated waste signals
   - safest next review actions

## Output modes

- `operator-weekly-brief`
- `founder-weekly-brief`
- `finance-checkpoint`

## Required behavior

- Stay evidence-first.
- Separate measured change from interpretation.
- If prior-run comparison is missing, say so directly.
- Prefer simple weekly review language over generic cloud-cost theory.
- Stop at explanation and suggested next review action.

## References

- `references/weekly-brief-inputs.md`
- `references/weekly-brief-patterns.md`
- `references/prompt-templates.md`
- `references/examples.md`

## Scripts

- `scripts/build_weekly_brief_context.py`
  - Builds a normalized weekly-brief bundle from local API or exported evidence.
