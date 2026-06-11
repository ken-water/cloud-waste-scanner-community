# Community Skills Index

This document is the landing page for Community skills.

## Purpose

Community skills help a single operator get more value from local Cloud Waste Scanner evidence.

They are designed to:

- explain findings faster
- summarize weekly change
- convert findings into safer local action guidance

They are not designed to replace Team workflow coordination.

## Runtime model

Community skills support two runtime modes:

### Connected mode

- installed app
- local API enabled
- best result quality

### File-only mode

- exported JSON / CSV / TXT evidence
- lower-friction usage
- weaker than connected mode, but still useful

Reference:

- `SKILL_RUNTIME_MODES.md`

## Available Community skills

### 1. `cws-report-explainer`

Use when you need:

- operator explanation
- finance summary
- executive brief
- weekly action list

Input:

- local API
- exported findings or reports

Files:

- `skills/cws-report-explainer/SKILL.md`

### 2. `cws-weekly-brief`

Use when you need:

- current vs prior scan summary
- repeated waste signals
- concise weekly review note
- founder or operator weekly checkpoint

Input:

- local API
- current and prior exported evidence

Files:

- `skills/cws-weekly-brief/SKILL.md`

### 3. `cws-playbook-writer`

Use when you need:

- pre-check
- execute guidance
- verification step
- rollback caution

Input:

- local API findings
- exported findings
- normalized evidence bundle

Files:

- `skills/cws-playbook-writer/SKILL.md`

## Which skill to use

Use this simple selection rule:

- need explanation for people: `cws-report-explainer`
- need weekly change summary: `cws-weekly-brief`
- need practical next-step guidance: `cws-playbook-writer`

## Community boundary

These skills stay in Community because they:

- read local evidence
- help one operator understand and prepare action
- stop before shared workflow coordination

They do not provide:

- shared owner routing
- org structure management
- SLA tracking
- approval workflow
- enterprise audit control

Reference:

- `SKILLS_EDITION_MATRIX.md`
- `SKILL_MONETIZATION_POLICY.md`

## Near-term next candidates

- `cws-export-auditor`
- stronger examples for SDK-driven local automation
- stronger per-finding confidence/rationale presentation
