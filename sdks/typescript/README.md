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
