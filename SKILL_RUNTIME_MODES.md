# Skill Runtime Modes

This document defines how Cloud Waste Scanner skills are expected to run.

## Core rule

Cloud Waste Scanner skills should support two runtime modes:

1. `connected-mode`
2. `file-only-mode`

They do not require the desktop app in every case, but their highest-value operation should prefer the local app and local API.

## 1. Connected mode

Connected mode means:

- the user has installed Cloud Waste Scanner
- the local API is enabled
- the skill reads live or recent evidence from the local machine

Typical inputs:

- `GET /status`
- `GET /v1/findings`
- `GET /v1/scans`
- `GET /v1/reports`
- `GET /v1/openapi.json`

### Why connected mode is preferred

Connected mode gives the best result because:

- evidence is fresher
- fields are more complete
- scan, history, and report context are available
- the skill does not depend on manual file prep

### Product role

Connected mode is the primary product experience for Community skills.

## 2. File-only mode

File-only mode means:

- the user does not need the app running
- the skill works from exported evidence only

Typical inputs:

- findings JSON
- findings CSV
- report JSON
- pasted text from report or handoff pack

### Why file-only mode matters

File-only mode is useful because it lowers adoption friction:

- a user can trial the skill before installing the app
- a user can share an exported pack for interpretation
- external reviewers can use the output without needing the full product

### Limits of file-only mode

File-only mode is intentionally weaker than connected mode:

- data may be stale
- fields may be missing
- historical context may be incomplete
- no fresh scan can be triggered

## 3. No-data mode

If the user has neither:

- a local app/API
- nor exported evidence

then the skill should not pretend it can provide real product value.

At that point it can only:

- explain methodology
- describe expected inputs
- tell the user how to get usable evidence

It should not behave like a general cloud scanner or request cloud credentials.

## Product guidance

### Community skills

Community skills should support both:

- connected mode
- file-only mode

This gives a clean funnel:

1. low-friction trial through exported evidence
2. higher-value usage through installed local app

### Team and Enterprise skills

Team and Enterprise skills may still accept exported evidence, but their primary value should increasingly depend on product-controlled workflow state, organization context, and governed delivery.

## Positioning guidance

Use this language consistently:

- "The skill does not always require the app, but it works best when connected to the CWS local API."
- "Without the app, the skill can still explain exported evidence, but it cannot provide the full local-first workflow experience."
- "Without app data or exported evidence, the skill cannot produce a trustworthy Cloud Waste Scanner result."

## Decision rule for new skills

Before shipping a skill, verify:

1. Does it support connected mode?
2. Can it degrade gracefully into file-only mode?
3. Does it avoid pretending to work without evidence?

If the answer to 3 is no, the skill is poorly scoped.
