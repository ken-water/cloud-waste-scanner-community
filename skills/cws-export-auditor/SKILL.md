---
name: cws-export-auditor
description: Audit Cloud Waste Scanner exported evidence for missing assumptions, unclear summaries, missing owner context, and weak report structure. Use when the user wants to improve how an exported pack reads before sharing it with finance, leadership, or another reviewer.
metadata:
  short-description: Audit CWS exports before sharing
---

# CWS Export Auditor

Use this skill when the task is to review a Cloud Waste Scanner export before it is shared.

This skill is for:

- checking whether an export is understandable to a non-user
- finding missing assumptions or unclear wording
- spotting where owner context or evidence context is weak
- improving exported packs before they go to finance, leadership, or audit reviewers

This skill is not for:

- direct cloud API access
- automated owner routing
- workflow enforcement
- approval workflow
- enterprise audit policy administration

## Inputs

This skill supports two runtime modes:

- `connected-mode`: installed app + local API with recent reports or exports
- `file-only-mode`: exported JSON / CSV / TXT / pasted report text

Read `../../SKILL_RUNTIME_MODES.md` for the product rule behind these modes.

Preferred inputs:

- exported report text
- exported findings CSV
- handoff summary text
- normalized bundle from local API

## Workflow

1. Identify what kind of export is being reviewed.
2. Check whether a non-product reader can understand:
   - headline
   - scope
   - assumptions
   - evidence basis
   - action implication
3. Produce an audit with:
   - what is clear
   - what is missing
   - what is misleading
   - how to tighten the export

## Output modes

- `reader-audit`
- `finance-readiness-audit`
- `leadership-readiness-audit`

## Required behavior

- Focus on clarity and trustworthiness, not generic writing advice.
- Separate missing evidence from missing presentation.
- Prefer concise, practical corrections.
- Do not simulate Team workflow enforcement.

## References

- `references/audit-patterns.md`
- `references/prompt-templates.md`
- `references/examples.md`
