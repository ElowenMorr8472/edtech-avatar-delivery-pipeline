#!/bin/sh
set -eu

: "${AVATAR_FILE:?set AVATAR_FILE to a local image path}"
IMAGE_BYTES=$(od -An -v -tu1 "$AVATAR_FILE" | tr -s ' \n' ',' | sed 's/^,//; s/,$//')
curl --fail-with-body --request POST http://127.0.0.1:3000/avatars \
  --header 'Content-Type: application/json' \
  --data "{\"user_id\":\"learner-42\",\"course_id\":\"rust-101\",\"filename\":\"avatar.png\",\"image_bytes\":[${IMAGE_BYTES}],\"submitted_at_epoch\":1726000000,\"deadline_epoch\":1726000060,\"operation_id\":\"avatar-learner-42-v1\"}"
