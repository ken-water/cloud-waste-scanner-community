# Team Scope

## Positioning
Team is the governance execution layer.

## Current Status

Team should currently be treated as preview/planned rather than GA.

This repository already contains some gated local foundations for Team workflows, but the full Team promise should not be treated as substantially complete until `TEAM_CLOSURE_PLAN.md` is substantially complete.

It is intended for teams that already have findings and now need to:
- assign ownership
- track remediation lifecycle
- organize people into operating structure
- produce audience-specific handoff packages

## Intended Team Capabilities

### Organization and ownership
- Org unit management
- Owner directory
- Owner role model:
  - `owner`
  - `manager`
- Mapping findings to owners and org units

### Governance execution
- Finding lifecycle workflow
- Manager-only batch assignment
- Weekly governance pack
- Org summary export
- Audience-aware handoff generation:
  - `exec`
  - `owner`
  - `audit`

### Local safety controls
- Runtime operator role
- Local admin guard for role changes
- Sensitive-field handling by audience

## Not included in Team
- Enterprise SSO/SCIM
- Centralized multi-instance control plane
- Enterprise-wide policy federation
- Organization-wide audit and compliance administration

## Product rule
Team should help a group move from evidence to accountable execution on a repeatable weekly rhythm.

## Commercial boundary
Team is where Cloud Waste Scanner stops being only a scanner and becomes a workflow system.

This is a paid boundary because the value comes from:
- coordination
- accountability
- workflow enforcement
- management-ready outputs
