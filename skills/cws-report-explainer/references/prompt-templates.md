# Prompt Templates

Use these directly, then replace the placeholder bundle or file path.

## 1. Operator explanation

```text
Use $cws-report-explainer to explain this Cloud Waste Scanner evidence for an operator.

Goal:
- identify the safest cleanup actions first
- separate high-confidence actions from owner-confirmation items
- keep the answer concrete and short

Input:
<normalized_bundle_or_export_here>
```

## 2. Finance summary

```text
Use $cws-report-explainer to summarize this Cloud Waste Scanner evidence for finance.

Goal:
- quantify projected monthly waste using only source numbers
- show the biggest cost buckets
- state what looks safe to count now versus what still needs review

Input:
<normalized_bundle_or_export_here>
```

## 3. Executive brief

```text
Use $cws-report-explainer to turn this Cloud Waste Scanner evidence into a short executive brief.

Goal:
- explain what happened
- identify the largest concentration points
- call out ownership or operating-model gaps
- end with the one decision leadership should force this week

Input:
<normalized_bundle_or_export_here>
```

## 4. Weekly action list

```text
Use $cws-report-explainer to convert this Cloud Waste Scanner evidence into a weekly action list.

Goal:
- produce a flat prioritized list
- include expected impact, risk, owner status, and next action
- avoid generic cloud-cost advice

Input:
<normalized_bundle_or_export_here>
```

## 5. File-based invocation

```text
Use $cws-report-explainer on the JSON bundle at ./tmp/cws-context.json.
Produce:
1. operator summary
2. finance summary
3. top 5 actions this week
```

## 6. API-first invocation

```text
Use $cws-report-explainer with data collected from the local API at http://127.0.0.1:43177.
Assume the bearer token is already available.
Explain the findings for an execution owner and then produce a boss-ready summary.
```
