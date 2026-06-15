## Summary

- 

## Scope

- [ ] Community local-first scope
- [ ] Skills / docs only
- [ ] Desktop app / local API
- [ ] Other

## Validation

- [ ] `./scripts/validate-skills.sh` if skills changed
- [ ] `./scripts/package-skills.sh` if skills packaging changed
- [ ] `npm --prefix gui run build` if frontend changed
- [ ] Rust formatting/checks if Rust changed

## Safety

- [ ] No cloud credentials, local API tokens, or customer exports are committed
- [ ] Generated `dist/` artifacts are not committed
- [ ] Team/Enterprise workflow boundaries are not moved into Community unintentionally
