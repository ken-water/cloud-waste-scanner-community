# Team Edition User Guide

## 1. What Team Edition Adds
Team edition is designed for collaborative waste remediation.

You get:
- Org and owner modeling.
- Ownership lifecycle control.
- Manager-led batch assignment.
- Audience-based handoff packages.

## 2. Team Roles
- Owner: handles assigned findings and execution follow-up.
- Manager: coordinates ownership at scale, including batch assignment.

Role is runtime-controlled by `runtime_operator_role` and can be changed only by mapped role admins.

## 3. Initial Setup
1. Open `Settings -> Org & Owners`.
2. Create org units.
3. Create owners and bind each owner to one org unit.
4. Set owner role (`owner` or `manager`).
5. In Team Operator Role section:
- verify current local user,
- verify role admin list,
- set runtime operator role if needed.

## 4. Daily Workflow
### Step A: Triage Findings
1. Open `Scan Results`.
2. Filter by provider/search/delete-only as needed.
3. Team mode adds filters:
- `All org units`
- `All owners`

### Step B: Assign Ownership
- Single assignment: use row-level owner assignment.
- Manager batch assignment:
1. Select multiple findings.
2. Choose target owner in floating action bar.
3. Click `Batch Assign`.

### Step C: Build Handoff Pack
Use `Create Handoff Pack` from Scan Results.

Audience template:
- `exec`: summary-oriented, sensitive fields masked.
- `owner`: operational handoff, sensitive fields toggle available.
- `audit`: full detail, sensitive fields included.

Generated artifacts:
- `handoff_<scope>_<date>.txt`
- `handoff_<scope>_<date>.csv`
- `handoff_<scope>_<date>.json`

## 5. Governance Review
Open `Governance` for execution-level tracking.

In Team mode, org lifecycle summary is available and included in exports.

## 6. Common Scenarios
### Scenario: Owner transfer
When deactivating an owner with open findings:
- transfer target is required,
- open findings are reassigned before deactivation.

### Scenario: Non-admin cannot change role settings
Expected behavior. Ask a mapped role admin user to update:
- `runtime_operator_role`
- `runtime_role_admin_users`

## 7. Rollout Recommendations
1. Start with one manager and 2-5 owners.
2. Use org/owner filters in weekly review.
3. Use `exec` handoff for leadership, `owner` handoff for execution.
4. Keep role admin mapping explicit and reviewed monthly.

## 8. Validation Checklist
- Org units created.
- Owners linked to org units.
- Manager batch assign visible and working for manager role.
- Handoff packs generated for all three audiences.
- Governance export includes org summary in Team mode.
