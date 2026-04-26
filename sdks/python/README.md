# Cloud Waste Scanner Python SDK

Dependency-free helper for the local-first CWS API.

```python
from cws_client import CwsClient

client = CwsClient(base_url="http://127.0.0.1:43177", token="local-api-token")

print(client.status())
print(client.openapi())
print(client.list_findings(limit=25))
```

Webhook verification:

```python
from cws_client import verify_webhook_signature

ok = verify_webhook_signature(
    secret="shared-secret",
    timestamp=headers["X-CWS-Timestamp"],
    body=raw_body,
    signature=headers["X-CWS-Signature"],
)
```
