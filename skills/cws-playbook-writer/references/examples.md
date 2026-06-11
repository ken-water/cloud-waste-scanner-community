# Examples

## Example output

```text
Finding meaning:
This looks like an orphaned storage volume with recurring monthly cost and no clear active attachment context in the exported evidence.

Pre-check:
- confirm it is not attached to a production instance
- confirm it is not part of a recovery or rollback plan
- confirm no automation expects this volume id

Execution suggestion:
- snapshot first if policy requires recovery protection
- remove the volume only after attachment and retention checks pass

Verification:
- confirm the volume no longer appears in the next scan
- confirm projected waste drops in the next review
- confirm no restore or workload error occurs after removal

Rollback caution:
- if application recovery depends on this asset, deletion may create a hidden restore gap
- if attachment history is unclear, stop and escalate to a human reviewer before removal
```
