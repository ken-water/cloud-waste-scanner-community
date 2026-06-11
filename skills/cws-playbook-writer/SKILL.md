---
name: cws-playbook-writer
description: Convert Cloud Waste Scanner findings into practical cleanup playbooks with pre-checks, execution steps, verification, and rollback caution. Use when the user wants safe next-step guidance from local CWS evidence without crossing into shared Team workflow enforcement.
metadata:
  short-description: Turn CWS findings into action playbooks
---

# CWS Playbook Writer

Use this skill when the task is to turn findings into practical, low-regret action guidance.

This skill is for:

- writing cleanup checklists from findings
- separating pre-check, execute, verify, and rollback caution
- helping one operator move from evidence to action safely
- converting exported findings into reusable local runbooks

This skill is not for:

- direct cloud API execution
- automatic remediation
- shared owner coordination
- approval workflow
- SLA or escalation enforcement

## Inputs

This skill supports two runtime modes:

- `connected-mode`: installed app + local API
- `file-only-mode`: exported evidence files

Read `../../SKILL_RUNTIME_MODES.md` for the product rule behind these modes.

Preferred inputs:

- local API findings
- normalized bundles from `cws-report-explainer`
- exported findings JSON or CSV
- pasted finding details

## Workflow

1. Identify the main finding or category.
2. Extract the available evidence.
3. Produce a playbook with:
   - pre-check
   - execution suggestion
   - verification step
   - rollback caution
4. Keep the output cautious when evidence is incomplete.

## Output modes

- `single-finding-playbook`
- `category-playbook`
- `low-regret-first-batch`

## Required behavior

- Never imply that deletion is safe without stating the required pre-check.
- If the finding is ambiguous, say what must be confirmed first.
- Prefer low-regret candidates first.
- Do not simulate Team workflow enforcement.

## References

- `references/playbook-patterns.md`
- `references/prompt-templates.md`
- `references/examples.md`
