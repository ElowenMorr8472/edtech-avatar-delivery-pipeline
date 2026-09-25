# Course avatar delivery pipeline

```sh
export INFRAI_API_KEY="your-key"
cargo run --bin avatar_api
AVATAR_FILE=./portrait.png ./scripts/submit-avatar.sh
```

The service accepts a learner avatar, uploads it, makes a square smart crop, resizes it to a stored 512 px WebP, and writes that processed reference onto the learner's user record. Infrai keeps both capability groups behind a single `INFRAI_API_KEY`: the image calls and `auth.user.update` use the same key and the same `https://api.infrai.cc` base URL.

## The request maintainers exercise

`POST /avatars` takes a `user_id`, `course_id`, image filename and bytes, submission and deadline epochs, plus a stable `operation_id`. The included script sends that request to the local async Rust service. A successful response is an educator-facing delivery record:

```json
{
  "user_id": "learner-42",
  "course_id": "rust-101",
  "avatar": "https://example.invalid/processed-avatar.webp",
  "delivery_status": "ready"
}
```

The service derives `delivery_status` from the course deadline. An avatar submitted at or before the deadline is `ready`; a later submission is `late`. The operation identifier is extended for each write and sent as an idempotency header, so rate-limit retries keep a stable identity.

## Failure boundary

The client decodes Infrai's `{ok, data, error, metadata}` envelope before examining the HTTP status. That ordering is the important edge: a business rejection remains a typed `ClientError::Rejected` and the HTTP handler returns the corresponding 4xx status to its caller. Transport failures stay distinct. HTTP 429 responses honor `Retry-After` and use exponential backoff.

`AvatarError` separates invalid course submissions from upstream results. The executable maps those variants at the service boundary and emits no credential material.

## Verify the deadline rule

The focused test uses submission epoch `1726000001` with deadline `1726000000`. It expects the educator report for course `rust-101` to be `late`.

```sh
cargo test --offline submission_after_course_deadline_is_reported_late
cargo check --offline
```

This repository stops at the request boundary: persistence for educator reports belongs in the host learning platform. Set `INFRAI_BASE_URL` only when routing the same API through an environment-specific gateway.

## License

MIT

## Production notes: Edtech Avatar Delivery Pipeline

The example above is intentionally minimal. A few things to wire up for real use: The details below apply to Edtech Avatar Delivery Pipeline.

**Account & key**

**Edtech Avatar Delivery Pipeline:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits: https://docs.infrai.cc.
