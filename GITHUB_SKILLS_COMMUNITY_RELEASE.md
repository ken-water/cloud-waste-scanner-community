# GitHub Community Skills Release

This document defines the GitHub community release gate for Cloud Waste Scanner skills.

## Release goal

Publish source-visible skills that can be used from:

- the repository source tree
- generic downloaded packages
- Codex project installs
- Claude Code project installs

The GitHub release is the source-of-truth community distribution. Platform-specific packages are derived from the same `skills/` source tree.

## Source of truth

- Skill source: `skills/<skill-name>/`
- Skill index: `skills/manifest.json`
- Examples: `skills/examples/`
- Validation: `scripts/validate-skills.sh`
- Packaging: `scripts/package-skills.sh`

Generated packages are written to `dist/` and must not be committed.

## Required checks

Before publishing a GitHub community skills release:

```bash
./scripts/validate-skills.sh
./scripts/package-skills.sh
```

Required package outputs:

```text
dist/skills-generic.tar.gz
dist/skills-codex.tar.gz
dist/skills-claude.tar.gz
```

## Artifact contract

### Generic package

Path inside archive:

```text
skills-generic/skills/<skill-name>/SKILL.md
skills-generic/examples/
skills-generic/skills-manifest.json
```

### Codex package

Path inside archive:

```text
skills-codex/.agents/skills/<skill-name>/SKILL.md
skills-codex/examples/
skills-codex/skills-manifest.json
```

### Claude package

Path inside archive:

```text
skills-claude/.claude/skills/<skill-name>/SKILL.md
skills-claude/examples/
skills-claude/skills-manifest.json
```

Claude packages must not include Codex/OpenAI-specific `agents/openai.yaml` metadata.

## Release notes checklist

A community skills release note should include:

- included skills
- supported platforms
- runtime modes
- package names
- validation commands
- known boundaries

## Boundaries

Community skills explain and transform Cloud Waste Scanner evidence. They do not:

- run cloud scans by themselves
- store cloud credentials
- execute deletion or remediation
- enforce approval workflow
- coordinate Team/Enterprise multi-user state

## GitHub Actions

Use the `Release Community Skills` workflow to validate and upload package artifacts from a selected ref.

The workflow is manual by default to avoid consuming build quota on every push.
