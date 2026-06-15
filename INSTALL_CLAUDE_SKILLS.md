# Install CWS Skills for Claude Code

Cloud Waste Scanner skills can be packaged for Claude Code under `.claude/skills`.

## Build the Claude package

```bash
./scripts/validate-skills.sh
./scripts/package-skills.sh
```

This creates:

```text
dist/skills-claude.tar.gz
dist/skills-claude/.claude/skills/
```

The Claude package intentionally excludes `agents/openai.yaml`; those files are Codex/OpenAI UI metadata and are not required for Claude.

## Project install

From a target project repository:

```bash
tar -xzf /path/to/cloud-waste-scanner/dist/skills-claude.tar.gz
```

The target project should then contain:

```text
.claude/skills/cws-report-explainer/SKILL.md
.claude/skills/cws-weekly-brief/SKILL.md
.claude/skills/cws-playbook-writer/SKILL.md
.claude/skills/cws-export-auditor/SKILL.md
```

## Smoke test prompts

```text
Use the cws-report-explainer skill with skills/examples/handoff_manifest_1.1.0.json and explain the findings for finance.
```

```text
Use the cws-export-auditor skill with skills/examples/handoff_findings.csv and identify missing context before sharing it.
```

## Runtime modes

- Connected mode: use local CWS API evidence when the app is installed and running.
- File-only mode: use exported evidence files when the app is not available.

Do not put cloud credentials into skill prompts. The desktop app remains the scan engine.
