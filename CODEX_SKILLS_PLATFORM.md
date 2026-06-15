# Codex Skills Platform Notes

Cloud Waste Scanner Codex skills are packaged under `.agents/skills`.

## Package layout

```text
skills-codex/
  .agents/skills/<skill-name>/SKILL.md
  .agents/skills/<skill-name>/agents/openai.yaml
  .agents/skills/<skill-name>/references/
  .agents/skills/<skill-name>/scripts/
  examples/
  skills-manifest.json
```

## Codex-specific metadata

Each skill may include:

```text
agents/openai.yaml
```

This file provides UI-facing metadata such as display name, short description, default prompt, and implicit invocation policy.

## Triggering guidance

Codex discovers skills from `SKILL.md` frontmatter. Keep `description` explicit about:

- what the skill does
- when to use it
- accepted evidence sources
- boundaries around cloud credentials and remediation

## Install modes

Project install:

```bash
tar -xzf dist/skills-codex.tar.gz -C /path/to/project
```

Global install:

```bash
mkdir -p "$HOME/.agents/skills"
tar -xzf dist/skills-codex.tar.gz -C /tmp/cws-skills
cp -a /tmp/cws-skills/skills-codex/.agents/skills/* "$HOME/.agents/skills/"
```

## Smoke tests

```text
Use $cws-report-explainer with examples/handoff_manifest_1.1.0.json and produce an operator summary.
```

```text
Use $cws-weekly-brief with examples/scan_results.json and create a weekly review note.
```

## Boundary

Codex skills should explain and transform CWS evidence. They should not ask for cloud credentials or execute cleanup actions.
