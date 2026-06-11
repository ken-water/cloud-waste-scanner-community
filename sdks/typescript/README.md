# Cloud Waste Scanner TypeScript SDK

Typed helper for the local-first CWS API.

```ts
import { CwsClient } from "@cloud-waste-scanner/client";

const client = new CwsClient({
  baseUrl: "http://127.0.0.1:43177",
  token: "local-api-token",
});

console.log(await client.status());
console.log(await client.openapi());
console.log(await client.listFindings({ limit: 25 }));
```

For role-based summaries, pair the local API output with:

- `skills/cws-report-explainer`

Typical flow:

1. call the local API
2. normalize findings and reports into one JSON bundle
3. run the explainer skill for operator, finance, or executive output

Webhook verification:

```ts
import { verifyWebhookSignature } from "@cloud-waste-scanner/client";

const ok = verifyWebhookSignature({
  secret: "shared-secret",
  timestamp: request.headers["x-cws-timestamp"],
  body: rawJsonBody,
  signature: request.headers["x-cws-signature"],
});
```
