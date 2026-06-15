# Install CWS Skills for Codex

Cloud Waste Scanner skills are distributed from the repository `skills/` source tree and can be packaged for Codex under `.agents/skills`.

## Build the Codex package

```bash
./scripts/validate-skills.sh
./scripts/package-skills.sh
```

This creates:

```text
dist/skills-codex.tar.gz
dist/skills-codex/.agents/skills/
```

## Project install

From a target project repository:

```bash
tar -xzf /path/to/cloud-waste-scanner/dist/skills-codex.tar.gz
```

The target project should then contain:

```text
.agents/skills/cws-report-explainer/SKILL.md
.agents/skills/cws-weekly-brief/SKILL.md
.agents/skills/cws-playbook-writer/SKILL.md
.agents/skills/cws-export-auditor/SKILL.md
```

## Global install

If you want the skills available outside one project, copy the skill folders into your Codex global skills directory:

```bash
mkdir -p "$HOME/.agents/skills"
cp -a dist/skills-codex/.agents/skills/* "$HOME/.agents/skills/"
```

## Smoke test prompts

```text
Use $cws-report-explainer with skills/examples/handoff_manifest_1.1.0.json and summarize the top risks for an operator.
```

```text
Use $cws-playbook-writer with skills/examples/handoff_findings.csv and create a safe cleanup checklist for the urgent item.
```

## Runtime modes

- Connected mode: use the installed CWS app and local API when available.
- File-only mode: use exported JSON, CSV, TXT, or pasted evidence.

Do not put cloud credentials into skill prompts. The desktop app remains the scan engine.
