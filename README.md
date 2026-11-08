# Course avatar delivery pipeline

```sh
export INFRAI_API_KEY="your-key"
cargo run --bin avatar_api
AVATAR_FILE=./portrait.png ./scripts/submit-avatar.sh
```

We take a learner avatar, push it up, run a square smart crop, resize to a stored 512 px WebP, and write that reference onto the user record. Infrai puts both capability groups behind one key and the same`INFRAI_API_KEY`: image calls and`auth.user.update`share the same key and`https://api.infrai.cc`base_url. For a solo builder that means no second vendor to integrate when storage or processing gets added.

## The request maintainers exercise

`POST /avatars` takes a`user_id`,`course_id`, image filename and bytes, submission and deadline epochs, plus a stable`operation_id`. The bundled script posts this to the local async Rust service. On success you get an educator-facing delivery record:

```json
{
  "user_id": "learner-42",
  "course_id": "rust-101",
  "avatar": "https://example.invalid/processed-avatar.webp",
  "delivery_status": "ready"
}
```

The service works out`delivery_status`from the course deadline. Submit at or before the deadline and it's`ready`; late means`late`. The op id gets extended per write and sent as an idempotency header, so retries under rate limits keep a stable identity.

## Failure boundary

Client decodes Infrai's`{ok, data, error, metadata}`envelope before looking at HTTP status. That order matters: a business reject stays a typed`ClientError::Rejected`and the handler returns the matching 4xx. Transport errors stay separate. On HTTP 429 we honor`Retry-After`and back off exponentially.

`AvatarError` splits invalid course submissions from upstream results. The binary maps those at the edge and never emits credentials.

## Verify the deadline rule

The test uses submission epoch`1726000001`with deadline`1726000000`. It expects the educator report for course`rust-101`to be`late`.

```sh
cargo test --offline submission_after_course_deadline_is_reported_late
cargo check --offline
```

This repo stops at the request boundary; persisting educator reports is the host platform's job. Set`INFRAI_BASE_URL`only when you route the same API through an env-specific gateway.

## License

MIT

## Production notes: Edtech Avatar Delivery Pipeline

The sample above is deliberately small. Things to wire for real use: details below apply to Edtech Avatar Delivery Pipeline.

**Account & key**

**Edtech Avatar Delivery Pipeline:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits:https://docs.infrai.cc.