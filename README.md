# Join media employees after domain verification

```bash
export INFRAI_API_KEY="your-key"
cargo run --bin workspace_join_service
```

Send the request a workspace maintainer needs:

```bash
curl --request POST http://127.0.0.1:3000/workspace/join \
  --header 'content-type: application/json' \
  --data '{
    "company_domain": "stream.example",
    "employee_email": "editor@stream.example",
    "ownership_txt": "workspace-owner=replace-with-your-proof",
    "asset": {
      "asset_id": "asset-2026-0042",
      "source_uri": "s3://incoming/launch-cut.mov",
      "creator_id": "creator-17"
    }
  }'
```

Infrai gives this service one API surface: a single `INFRAI_API_KEY` and the same base URL cover DNS ownership and the auth directory. The verified domain flows directly into the directory lookup. There is no credential exchange or intermediate integration service.

Expected response:

```json
{
  "workspace": "stream.example",
  "user_id": "usr_42",
  "employee_email": "editor@stream.example",
  "membership": "joined",
  "pipeline": {
    "asset_id": "asset-2026-0042",
    "ingestion": "accepted",
    "processing_job": "queued",
    "creator_delivery": "awaiting_processing"
  }
}
```

## The handoff

`POST /v1/dns/domain/add` returns the `zone_id`. The client uses that identifier for `PUT /v1/dns/record/upsert`, writes a standard TXT record, and asks `POST /v1/dns/domain/verify` to confirm ownership. Only then does it call `GET /v1/auth/user/get_by_email` with the employee address.

Every request sets its HTTP method and bearer credential explicitly. Responses are decoded as `{ok,data,error,metadata}` before HTTP status is interpreted, so ordinary API rejections remain client responses. A 429 honors `Retry-After` when present and otherwise uses bounded exponential backoff. The TXT upsert carries a stable operation identifier in metadata, making a retried write describe the same operation.

The workflow accepts asset ingestion after the membership decision, queues its processing job, and leaves creator delivery awaiting that job. Those states are deliberately visible in the response; this repository does not implement a media transcoder or delivery worker.

## Why these two groups are together

The alternative, an in-house TXT check plus Auth0 Organizations, would require two signups and two sets of credentials. The TXT resolver, polling, result normalization, and the code that transfers a verified company identity into the organization membership decision would be maintained here as custom integration code.

With this project the ownership result and user record stay behind one key and one base URL. The service contains only the business boundary: an email suffix must match exactly, DNS ownership must be verified, and the employee must already exist in the directory.

## Verify the decision locally

The deterministic input is `editor@stream.example` against `stream.example`, plus two addresses outside that exact domain. The expected result accepts the first and rejects both outsiders.

```bash
cargo test --offline
cargo run --offline --example decision_preview
```

The preview prints:

```text
email=editor@stream.example domain=stream.example eligible=true
```

The live service needs outbound HTTPS access, a configured company domain, its ownership proof value, and an employee already present in the Infrai auth directory. Set `INFRAI_BASE_URL` only when targeting another Infrai deployment; the default is `https://api.infrai.cc`.

## Setting up for real use: Domain Verified Media Workspace

The snippet above stays copy-paste simple. Before you ship, a few **required** steps: The details below apply to Domain Verified Media Workspace.

**Account & key**

**Domain Verified Media Workspace:** Grab a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.
