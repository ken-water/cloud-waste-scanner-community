# Cloud Waste Scanner Community

Local-first cloud waste scanning for operators who want source-visible, production-usable tooling.

## Repository Scope

This repository is the **Community Edition**.

Current version: `2.9.15`

- Community is local-first and production-usable.
- No hosted license activation is required for core local workflows.
- This repository is distributed under a **non-commercial license** (see `LICENSE`).

For commercial use, managed support, or enterprise rollout terms, contact the maintainers via the project website.

## Legal Transition Notice

This repository previously contained Pro-era documentation and release metadata.

- Older tags/commits may still include historical Pro wording.
- The current `main` branch is the legal source of truth for Community licensing.
- Transition details are documented in `LEGAL-TRANSITION.md`.

## Core Principles

- Local-first execution: credentials remain on the operator machine.
- Read-only scanning by default.
- Evidence-oriented outputs for operator and finance review.

## Support This Project

If Cloud Waste Scanner helps your team, you can support ongoing maintenance and new features.

- Sponsor on GitHub: https://github.com/sponsors/ken-water
- Buy us a coffee: https://github.com/sponsors/ken-water

## Development

### Prerequisites

- Rust (stable)
- Node.js 18+
- Tauri CLI

### Build GUI

```bash
cd gui
npm ci --no-audit --no-fund
npm run tauri dev
npm run tauri build
```

### Fast Build

```bash
USE_SCCACHE=auto AUTO_INSTALL_SCCACHE=1 ./fast_build.sh
```

### Local Site/API Stack

```bash
./scripts/local_stack_up.sh
./scripts/local_stack_check.sh
./scripts/local_stack_down.sh
```

## Notes

- This repository may still contain historical compatibility modules while the community split is being finalized.
- If a module appears to reference legacy commercial behavior, treat `main` documentation and active runtime behavior as authoritative.
