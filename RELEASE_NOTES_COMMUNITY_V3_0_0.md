# Cloud Waste Scanner Community v3.0.0

Community `v3.0.0` resets the release line for the open client-first roadmap.

This is a Community boundary and skill-surface release. It establishes a cleaner product story for what Community includes now, what Team means, and what remains Enterprise-only. It also introduces the first Community skill family around local evidence explanation and weekly review.

This release does **not** claim that every desktop or SDK binary artifact has already been rebuilt as `3.0.0`. The release-line reset and the binary-version line are intentionally kept separate.

## Highlights

- Community release line reset to `3.0.0`
- clearer Community / Team / Enterprise boundary
- first Community skill: `cws-report-explainer`
- second Community skill: `cws-weekly-brief`
- additional Community skills: `cws-playbook-writer` and `cws-export-auditor`
- GitHub generic, Codex, and Claude-compatible skills packages
- GitHub Community Skills release workflow and validation gates
- website launch article for Community Skills
- formal Community skill strategy, runtime modes, and monetization guardrails
- conservative Team skill readiness assessment

## Included in this release

- Community product-boundary clarification
- Community skills strategy
- Community skills edition matrix
- Community skill runtime rules
- Community skill monetization rules
- `cws-report-explainer`
- `cws-weekly-brief`
- `cws-playbook-writer`
- `cws-export-auditor`
- `skills/manifest.json`
- `scripts/validate-skills.sh`
- `scripts/package-skills.sh`
- `INSTALL_CODEX_SKILLS.md`
- `INSTALL_CLAUDE_SKILLS.md`
- `GITHUB_SKILLS_COMMUNITY_RELEASE.md`
- `CODEX_SKILLS_PLATFORM.md`
- `CLAUDE_SKILLS_PLATFORM.md`
- `SKILLS_PLATFORM_MATRIX.md`
- Team skill readiness assessment

## Not included in this release

- Team product GA
- Enterprise product GA
- centralized identity or audit rollout
- mandatory binary rebuild for unchanged desktop artifacts

## Why this release exists

The project needed a cleaner Community-era baseline.

The previous `2.9.x` line mixed:

- historical Pro-era assumptions
- evolving Community boundaries
- incomplete language around paid workflow layers

`v3.0.0` resets the Community story around a clearer rule:

- Community: local-first discovery and evidence
- Team: governance execution
- Enterprise: centralized identity, audit, and control

## What is new

### 1. First Community skill: `cws-report-explainer`

Added a first local-first Community skill for turning CWS evidence into usable summaries.

It supports:

- operator explanation
- finance summary
- executive brief
- weekly action list generation

It works from:

- CWS local API
- exported JSON / CSV / TXT evidence

Artifacts:

- `skills/cws-report-explainer/SKILL.md`
- `skills/cws-report-explainer/scripts/build_context.py`
- prompt templates
- examples
- audience output guidance

### 2. Skills strategy and edition boundary

Defined a formal skills model so future skills do not blur Community, Team, and Enterprise.

Added:

- Community skills strategy
- skills edition matrix
- skill monetization policy
- skill runtime modes
- Team skills gap assessment

This makes the following rules explicit:

- Community skills explain and prepare
- Team skills coordinate and execute
- Enterprise skills control and audit

### 3. Runtime mode clarification

Skills now have a consistent runtime model:

- `connected-mode`: installed app + local API
- `file-only-mode`: exported evidence only

This gives users a low-friction trial path without weakening the local-first product direction.

### 4. Chinese usage guidance

Added a Chinese-language skills usage guide for consistent external communication.

### 5. Second Community skill: `cws-weekly-brief`

Added a second Community skill for current-vs-prior weekly review.

It supports:

- latest-scan weekly summary
- current vs prior evidence comparison
- repeated waste signal highlighting
- founder/operator weekly brief output

It works from:

- CWS local API
- exported evidence files

### 6. Community skill rules

This release also makes the following product rules explicit:

- Community skills explain and prepare
- Team skills coordinate and execute
- Enterprise skills control and audit
- Community skills should support both:
  - `connected-mode`
  - `file-only-mode`

### 7. Multi-platform skills packaging

Added packaging for three distribution targets:

- GitHub generic package: `dist/skills-generic.tar.gz`
- Codex package: `dist/skills-codex.tar.gz`
- Claude package: `dist/skills-claude.tar.gz`

The packages are generated from the same source tree under `skills/`.

Validation and packaging commands:

```bash
./scripts/validate-skills.sh
./scripts/package-skills.sh
```

### 8. Community Skills launch article

Added a website launch article:

- `Community Skills Now Ship for GitHub, Codex, and Claude`

The article explains what the skills do, how the packages differ, what evidence they consume, and what they deliberately do not do.

## What did not change

This release does not claim completion of:

- Team skill family
- Enterprise skill family
- centralized identity workflows
- enterprise audit workflow automation

It also does not imply that unchanged binaries were version-bumped.

## Upgrade guidance

If you are using Community today:

1. keep using the local-first desktop workflow
2. use `cws-report-explainer` for role-specific summaries
3. use `cws-weekly-brief` for weekly current-vs-prior review
4. use `cws-playbook-writer` for cautious cleanup checklists
5. use `cws-export-auditor` before sharing exports
6. treat Team and Enterprise skill ideas as roadmap, not current GA surface

## Suggested release-page CTA

- Try Community locally
- Use `cws-report-explainer` to explain findings
- Use `cws-weekly-brief` to summarize weekly change
- Use `cws-playbook-writer` to prepare cleanup checks
- Use `cws-export-auditor` before sharing exports
- Download GitHub, Codex, or Claude-compatible skills packages
- Follow Team features as preview/planned work, not current GA

## Validation checklist

- Community boundary is documented
- Community skills index is documented
- skill runtime modes are documented
- Community skill monetization boundary is documented
- `cws-report-explainer` validates successfully
- `cws-weekly-brief` validates successfully
- `cws-playbook-writer` validates successfully
- `cws-export-auditor` validates successfully
- skills packages build successfully
- Claude package does not include Codex/OpenAI `agents/openai.yaml`
- Team readiness is described conservatively, not overstated

## Current product position after v3.0.0

### Community

Active now:

- local-first scanning
- local evidence review
- exports
- local API and SDKs
- Community explainer skills

### Team

Current status:

- local Team MVP foundation exists
- Team skills should still be treated as preview/planned

### Enterprise

Current status:

- planned only

## Recommended next steps

Near-term Community direction:

1. `cws-weekly-brief`
2. `cws-playbook-writer`
3. stronger finding explanation quality
4. stronger local automation examples

## Scope note

This release should be described as:

- a Community release-line reset
- a first skill-surface release
- a clarification of edition boundaries

It should not be described as:

- a major binary rebuild
- completion of Team or Enterprise product lines
