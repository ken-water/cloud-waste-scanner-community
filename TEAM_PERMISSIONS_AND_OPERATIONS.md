# Team Permissions and Operations Guide

## Purpose
This document defines the Team edition permission boundary, operator role controls, and safe operations for owner/governance workflows.

## Edition Boundary
- Community: local scan, basic findings, basic exports.
- Team: org units, owner directory, finding lifecycle ownership, handoff workflows.
- Enterprise: advanced RBAC, audit/security policy extensions, SSO/SCIM.

## Team Governance Features
- Org unit management (`org_units`).
- Owner directory (`finding_owners`) with role (`owner` or `manager`).
- Finding owner assignment and lifecycle progression.
- Org lifecycle summary in Governance.
- Handoff package with audience templates (`exec`, `owner`, `audit`).

## Runtime Operator Role
Runtime operator role controls manager-only actions in the UI.

- Setting key: `runtime_operator_role`
- Allowed values: `owner`, `manager`
- Default fallback: `owner`

### Role Capabilities
- `owner`
  - View findings and exports.
  - Assign single finding owner (if Team entitlements available).
  - No manager-only batch assignment.
- `manager`
  - All owner capabilities.
  - Batch assignment of selected findings in Scan Results.
  - Team coordination controls.

## Role Admin Control
To prevent local self-escalation, changing role settings is protected by admin user mapping.

- Setting key: `runtime_role_admin_users`
- Format: comma-separated local usernames, lowercase preferred.
- Example: `ken,alice,bob`
- Empty value is rejected.

### Enforcement
Changing these keys requires current local user to be in admin mapping:
- `runtime_operator_role`
- `runtime_role_admin_users`

Current local user resolution:
1. `SUDO_USER` if present
2. otherwise `USER`

If user is not allowed, save is rejected.

## Settings UI Behavior (Team)
In `Settings -> Org & Owners -> Team Operator Role`:
- Current local user is displayed.
- Role admin list is displayed.
- `Operator Role` save and `Role Admin Users` save are disabled for non-admin users.

## Audit Logging
Changes to these keys are logged in config audit path:
- `runtime_operator_role`
- `runtime_role_admin_users`

## Safe Operations Checklist
1. Keep at least two admin users in `runtime_role_admin_users`.
2. Do not remove all admin users; empty list is blocked.
3. Use manager role only for operators who perform assignment coordination.
4. Review owner deactivation transfer behavior before bulk role changes.

## Recovery
If admin mapping is incorrect and no mapped admin remains:
1. Stop app.
2. Update local settings storage for `runtime_role_admin_users` with at least one valid local username.
3. Restart app and verify `Team Operator Role` controls are re-enabled.

## Recommended Defaults
- `runtime_operator_role=owner`
- `runtime_role_admin_users=ken`
- Add second admin before wider team rollout.
