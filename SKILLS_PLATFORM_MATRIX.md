# Skills Platform Matrix

| Platform | Package | Install Path | Metadata | Status |
| --- | --- | --- | --- | --- |
| GitHub generic | `dist/skills-generic.tar.gz` | `skills/` | `skills-manifest.json` | Ready |
| Codex | `dist/skills-codex.tar.gz` | `.agents/skills` | `agents/openai.yaml` + `skills-manifest.json` | Ready |
| Claude Code | `dist/skills-claude.tar.gz` | `.claude/skills` | `SKILL.md` + `skills-manifest.json` | Ready |

## Shared source

All platform packages are generated from the same source tree:

```text
skills/<skill-name>/
```

Do not manually edit generated `dist/` packages. Change the source skill and run:

```bash
./scripts/validate-skills.sh
./scripts/package-skills.sh
```

## Platform differences

- Codex package keeps `agents/openai.yaml`.
- Claude package removes `agents/`.
- Generic package keeps the source layout under `skills/`.
