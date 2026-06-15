# Contributing to Cloud Waste Scanner Community

Thank you for helping improve Cloud Waste Scanner Community.

## Scope

This repository is the Community Edition. Contributions should stay within the local-first, source-visible scope:

- local scan evidence
- local API and SDK usability
- community skills
- report/export clarity
- docs, examples, and non-commercial operator workflows

Do not add hosted license enforcement, credential collection, automatic destructive remediation, or centralized Team/Enterprise workflow control to Community contributions.

## Before opening a pull request

Run the relevant checks:

```bash
./scripts/validate-skills.sh
```

For frontend changes:

```bash
npm --prefix gui run build
```

For Rust changes, use the narrowest relevant check first. Network access to crates.io may affect local `cargo check` reliability.

## Skills contributions

Community skills live under `skills/` and are described by `skills/manifest.json`.

When changing skills:

1. Keep `SKILL.md` concise and trigger-focused.
2. Put detailed patterns in `references/`.
3. Put deterministic helpers in `scripts/`.
4. Update `skills/manifest.json` if paths, examples, scripts, or platform support change.
5. Run:

```bash
./scripts/validate-skills.sh
./scripts/package-skills.sh
```

## Pull request checklist

- The change is inside Community scope.
- Documentation is updated when behavior changes.
- Skill packages still validate if skills changed.
- No cloud credentials, tokens, or customer data are committed.
- Generated `dist/` packages are not committed.

## Licensing

By contributing, you agree that your contribution is provided under this repository's license terms.
