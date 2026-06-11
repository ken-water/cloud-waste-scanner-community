# Examples

## Example 1: Normalized bundle excerpt

```json
{
  "source": "files",
  "generated_at": "2026-06-11T09:57:03Z",
  "summary": {
    "findings": 3,
    "estimated_monthly_waste": 2530.5,
    "currencies": ["USD"],
    "top_providers": [
      { "label": "aws", "estimated_monthly_waste": 1550.5 },
      { "label": "gcp", "estimated_monthly_waste": 980.0 }
    ],
    "top_resource_types": [
      { "label": "ebs-volume", "estimated_monthly_waste": 1200.5 },
      { "label": "compute-instance", "estimated_monthly_waste": 980.0 },
      { "label": "snapshot", "estimated_monthly_waste": 350.0 }
    ],
    "top_accounts": [
      { "label": "prod-a", "estimated_monthly_waste": 1550.5 },
      { "label": "ml-team", "estimated_monthly_waste": 980.0 }
    ]
  },
  "findings": [
    {
      "provider": "aws",
      "resource_type": "ebs-volume",
      "account_name": "prod-a",
      "estimated_monthly_waste": 1200.5,
      "resource_name": "vol-orphan-1",
      "owner": ""
    },
    {
      "provider": "aws",
      "resource_type": "snapshot",
      "account_name": "prod-a",
      "estimated_monthly_waste": 350.0,
      "resource_name": "snap-stale-7",
      "owner": "platform"
    },
    {
      "provider": "gcp",
      "resource_type": "compute-instance",
      "account_name": "ml-team",
      "estimated_monthly_waste": 980.0,
      "resource_name": "gpu-idle-2",
      "owner": ""
    }
  ]
}
```

## Example 2: Operator output

```text
Headline: The current evidence shows 3 findings worth about $2,530.50 per month. The biggest buckets are orphaned EBS storage in prod-a and one idle GCP compute instance in ml-team.

Best low-risk actions:
1. Review and remove vol-orphan-1 if it is not attached to a live recovery path. It is the largest single storage item at about $1,200.50 per month.
2. Remove stale snapshot snap-stale-7 after confirming retention policy. This is lower value but already has a platform owner, so it should close quickly.

Needs owner confirmation:
1. gpu-idle-2 is expensive, but compute instances can still back scheduled or standby workloads. Do not terminate it without ml-team confirmation.
2. vol-orphan-1 has no owner recorded. That is an execution risk even if the technical signal is strong.

Safe next actions this week:
- assign an owner for vol-orphan-1 today
- confirm snapshot retention with platform and close snap-stale-7
- ask ml-team whether gpu-idle-2 is active, scheduled, or stale
```

## Example 3: Finance output

```text
Projected monthly waste in the current evidence set is about $2,530.50 USD.

Most of the value is concentrated in AWS storage ($1,550.50) and one GCP compute finding ($980.00). Of the total, the stale snapshot looks like a relatively high-confidence cleanup candidate, while the orphaned volume and idle compute item still require owner confirmation before they should be treated as realized savings.

What can likely move before the next invoice:
- the stale snapshot if retention review is straightforward
- the orphaned volume if ownership is confirmed quickly

What should remain in review:
- the idle compute instance until ml-team confirms it is not part of a valid runtime or standby requirement
```

## Example 4: Executive output

```text
The scan did not expose a tooling gap first. It exposed an ownership gap. Current findings total about $2.5K per month, with waste concentrated in AWS storage and one GCP compute workload. The technical signals are usable, but two of the three findings still have no clear owner, which means savings will stall unless leadership forces assignment and review dates this week.

Largest concentration points:
- prod-a AWS storage: about $1.55K per month
- ml-team idle compute: about $980 per month

Leadership decision this week:
- require every finding above a defined monthly threshold to have an owner and a decision date before the next review cycle
```
