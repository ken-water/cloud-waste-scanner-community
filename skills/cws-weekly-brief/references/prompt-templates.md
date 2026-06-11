# Prompt Templates

## 1. Operator weekly brief

```text
Use $cws-weekly-brief to turn this Cloud Waste Scanner evidence into an operator weekly brief.

Goal:
- summarize the latest scan
- compare against the prior run if available
- highlight repeated or recurring waste
- end with the next review action

Input:
<normalized_weekly_bundle_here>
```

## 2. Founder weekly brief

```text
Use $cws-weekly-brief to create a founder-friendly weekly update from this Cloud Waste Scanner evidence.

Goal:
- explain whether the picture improved or worsened
- show the biggest concentration points
- keep the answer concise and operational

Input:
<normalized_weekly_bundle_here>
```

## 3. Finance checkpoint

```text
Use $cws-weekly-brief to summarize this Cloud Waste Scanner evidence for finance.

Goal:
- describe current projected waste
- state direction versus the prior run
- identify the largest cost bucket
- separate stable signal from review-needed signal

Input:
<normalized_weekly_bundle_here>
```
