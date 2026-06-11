# Prompt Templates

## 1. Single finding

```text
Use $cws-playbook-writer to turn this Cloud Waste Scanner finding into a safe cleanup playbook.

Goal:
- write pre-check, execute, verify, and rollback caution
- stay conservative where evidence is incomplete

Input:
<finding_or_bundle_here>
```

## 2. Category playbook

```text
Use $cws-playbook-writer to create a reusable playbook for this Cloud Waste Scanner finding category.

Goal:
- explain low-regret actions first
- call out false-positive traps
- make the output usable by one operator

Input:
<category_evidence_here>
```
