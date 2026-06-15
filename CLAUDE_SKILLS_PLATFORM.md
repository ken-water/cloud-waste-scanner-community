# Claude Code Skills Platform Notes

Cloud Waste Scanner Claude Code skills are packaged under `.claude/skills`.

## Package layout

```text
skills-claude/
  .claude/skills/<skill-name>/SKILL.md
  .claude/skills/<skill-name>/references/
  .claude/skills/<skill-name>/scripts/
  examples/
  skills-manifest.json
```

## Claude-specific packaging rule

Claude packages exclude Codex/OpenAI UI metadata:

```text
agents/openai.yaml
```

The source tree keeps `agents/openai.yaml` for Codex, but `scripts/package-skills.sh` removes `agents/` from the Claude package.

## Triggering guidance

Claude uses `SKILL.md` to understand when a skill applies. Keep the skill description explicit and evidence-oriented.

Strong trigger examples:

```text
Use the cws-report-explainer skill with this handoff manifest and explain what finance should trust.
```

```text
Use the cws-export-auditor skill to review this findings CSV before sharing it with a manager.
```

## Install mode

Project install:

```bash
tar -xzf dist/skills-claude.tar.gz -C /path/to/project
```

Expected result:

```text
/path/to/project/.claude/skills/cws-report-explainer/SKILL.md
```

## Boundary

Claude skills should use sanitized CWS evidence, local API output, or exported files. Do not paste cloud credentials, tokens, or unredacted customer exports into prompts.
