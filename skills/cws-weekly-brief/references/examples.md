# Examples

## Example bundle excerpt

```json
{
  "source": "files",
  "generated_at": "2026-06-11T11:00:00Z",
  "current": {
    "summary": {
      "findings": 18,
      "estimated_monthly_waste": 7800.0,
      "top_providers": [
        { "label": "aws", "estimated_monthly_waste": 5100.0 },
        { "label": "gcp", "estimated_monthly_waste": 1900.0 }
      ],
      "top_resource_types": [
        { "label": "ebs-volume", "estimated_monthly_waste": 2200.0 },
        { "label": "compute-instance", "estimated_monthly_waste": 1800.0 }
      ]
    }
  },
  "previous": {
    "summary": {
      "findings": 21,
      "estimated_monthly_waste": 9200.0
    }
  },
  "delta": {
    "findings": -3,
    "estimated_monthly_waste": -1400.0
  }
}
```

## Example operator weekly brief

```text
Weekly headline: the latest scan still shows meaningful waste, but the picture improved versus the prior run. Current projected waste is about $7,800 per month across 18 findings, down from about $9,200 and 21 findings previously.

What changed:
- projected waste improved by about $1,400 per month
- total findings dropped by 3

Where the problem is still concentrated:
- AWS remains the largest waste bucket at about $5,100 per month
- EBS volumes remain the top resource-type driver at about $2,200 per month

Recurring concern:
- storage-related waste is still showing up as the main repeat category, which suggests cleanup is happening slower than detection

Next review action:
- verify which AWS storage findings are repeats from the prior run and close the highest-confidence ones first
```
